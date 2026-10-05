//! `config` — the indexer's configuration, parsed purely from a variable
//! lookup (the env-var seams now; `config.toml` later).
//!
//! [`parse_config`] is total: every environment either loads as an
//! [`IndexerConfig`] or is refused with a [`ConfigError`] naming the variable
//! and the offending value (ADR-077/078, data-models §4). Refusals happen
//! before any wiring, so a bad configuration contacts nothing.
//!
//! `OPENLORE_INDEXER_SOURCE_URL` keeps its name but is the optional FALLBACK
//! listing source (ADR-077): a DID whose document cannot be resolved is read
//! there, always as relay origin. The fallback is built only here.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::Duration;

use appview_domain::ingest_pass::pds_endpoint_admissible;
use appview_domain::FallbackUrl;
use claim_domain::Did;
use ports::net_policy::TransportPolicy;

/// The repo DIDs one ingest pass enumerates.
pub const REPO_DIDS_VAR: &str = "OPENLORE_INDEXER_REPO_DIDS";
/// The optional fallback listing source.
pub const FALLBACK_VAR: &str = "OPENLORE_INDEXER_SOURCE_URL";
/// TEST-ONLY: admits plain http to loopback in a debug build.
pub const LOOPBACK_SEAM_VAR: &str = "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP";
/// How many DIDs are fetched at once.
pub const MAX_CONCURRENT_FETCHES_VAR: &str = "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES";
/// One DID's whole-fetch deadline, in seconds.
pub const PER_DID_TIMEOUT_SECS_VAR: &str = "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS";
/// The SEPARATE `index.duckdb` path.
const INDEX_PATH_VAR: &str = "OPENLORE_INDEXER_INDEX_PATH";
/// The home directory the default index path lives under.
const HOME_VAR: &str = "OPENLORE_HOME";
/// The query surface listen address.
const LISTEN_ADDR_VAR: &str = "OPENLORE_INDEXER_LISTEN_ADDR";
/// The PLC directory repo DIDs are resolved from.
const PLC_ENDPOINT_VAR: &str = "OPENLORE_INDEXER_PLC_ENDPOINT";

/// The production PLC directory (ADR-026 §"Config + default").
const DEFAULT_PLC_ENDPOINT: &str = "https://plc.directory";

/// The ephemeral localhost listen address (the parallel-safe default).
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:0";

/// How many DIDs are fetched at once (ADR-078).
const DEFAULT_MAX_CONCURRENT_FETCHES: u64 = 4;
const MAX_CONCURRENT_FETCHES_RANGE: RangeInclusive<u64> = 1..=16;

/// One DID's whole fetch — resolving plus every listing page (ADR-078).
const DEFAULT_PER_DID_TIMEOUT_SECS: u64 = 30;
const PER_DID_TIMEOUT_SECS_RANGE: RangeInclusive<u64> = 1..=600;

/// The DID methods that can name a repo with a PDS.
const REPO_DID_METHODS: [&str; 2] = ["plc", "web"];

/// The longest DID the ATProto DID syntax admits.
const MAX_DID_LENGTH: usize = 2048;

/// Which build this binary is: the loopback test seam exists only in
/// development builds (the review-app pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Release,
    Development,
}

impl BuildProfile {
    pub fn of_this_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Development
        } else {
            Self::Release
        }
    }
}

/// The indexer's resolved configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerConfig {
    /// The SEPARATE `index.duckdb` path (ADR-023; never the user's openlore.duckdb).
    pub index_path: PathBuf,
    /// Where unresolvable DIDs are listed from; `None` = no fallback.
    pub fallback: Option<FallbackUrl>,
    /// The HTTP/XRPC query surface listen address (ADR-027).
    pub listen_addr: String,
    /// The distinct repo DIDs one ingest pass enumerates, first-seen order (DWD-9).
    pub repo_dids: Vec<Did>,
    /// The PLC directory each repo DID's document is resolved from.
    pub plc_endpoint: String,
    /// Which PDS addresses may be contacted (DD-IPF-5).
    pub policy: TransportPolicy,
    /// How many DIDs are fetched at once (ADR-078).
    pub max_concurrent_fetches: usize,
    /// The single deadline each DID's fetch runs under (ADR-078).
    pub per_did_time_budget: Duration,
}

/// Why the configuration was refused: the variable, its offending value (the
/// single bad entry of a list, not the whole list) and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub variable: &'static str,
    pub value: String,
    pub problem: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={:?}: {}", self.variable, self.value, self.problem)
    }
}

impl ConfigError {
    fn new(variable: &'static str, value: &str, problem: impl Into<String>) -> Self {
        Self {
            variable,
            value: value.to_string(),
            problem: problem.into(),
        }
    }
}

/// Parse the configuration from `lookup` (a variable name → its value) for a
/// build of `profile`. A release build refuses the loopback test seam before
/// anything else is read.
pub fn parse_config(
    lookup: impl Fn(&str) -> Option<String>,
    profile: BuildProfile,
) -> Result<IndexerConfig, ConfigError> {
    let setting = |name: &str| {
        lookup(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let policy = transport_policy(setting(LOOPBACK_SEAM_VAR).as_deref(), profile)?;
    let repo_dids = parse_repo_dids(setting(REPO_DIDS_VAR).as_deref().unwrap_or_default())?;
    let fallback = setting(FALLBACK_VAR)
        .map(|url| fallback_url(&url, policy))
        .transpose()?;
    let max_concurrent_fetches = bounded_number(
        MAX_CONCURRENT_FETCHES_VAR,
        setting(MAX_CONCURRENT_FETCHES_VAR).as_deref(),
        MAX_CONCURRENT_FETCHES_RANGE,
        DEFAULT_MAX_CONCURRENT_FETCHES,
    )?;
    let per_did_timeout_secs = bounded_number(
        PER_DID_TIMEOUT_SECS_VAR,
        setting(PER_DID_TIMEOUT_SECS_VAR).as_deref(),
        PER_DID_TIMEOUT_SECS_RANGE,
        DEFAULT_PER_DID_TIMEOUT_SECS,
    )?;
    Ok(IndexerConfig {
        index_path: lookup(INDEX_PATH_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_index_path(lookup(HOME_VAR))),
        fallback,
        listen_addr: lookup(LISTEN_ADDR_VAR).unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string()),
        repo_dids,
        plc_endpoint: lookup(PLC_ENDPOINT_VAR).unwrap_or_else(|| DEFAULT_PLC_ENDPOINT.to_string()),
        policy,
        max_concurrent_fetches: usize::try_from(max_concurrent_fetches).unwrap_or(usize::MAX),
        per_did_time_budget: Duration::from_secs(per_did_timeout_secs),
    })
}

/// `HttpsOrLoopbackHttp` only for the TEST-ONLY seam set to `1` in a
/// development build; a release build refuses the seam whatever its value.
fn transport_policy(
    seam: Option<&str>,
    profile: BuildProfile,
) -> Result<TransportPolicy, ConfigError> {
    match (seam, profile) {
        (Some(value), BuildProfile::Release) => Err(ConfigError::new(
            LOOPBACK_SEAM_VAR,
            value,
            "is a test-only seam; a release build refuses to start with it set",
        )),
        (Some("1"), BuildProfile::Development) => Ok(TransportPolicy::HttpsOrLoopbackHttp),
        _ => Ok(TransportPolicy::HttpsPublicOnly),
    }
}

/// The distinct repo DIDs of a comma- or whitespace-separated list, in
/// first-seen order; the first malformed entry refuses the whole list.
fn parse_repo_dids(list: &str) -> Result<Vec<Did>, ConfigError> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .map(repo_did)
        .try_fold(Vec::new(), |mut distinct, did| {
            let did = did?;
            if !distinct.contains(&did) {
                distinct.push(did);
            }
            Ok(distinct)
        })
}

/// One list entry as a repo DID: ATProto DID syntax, method `plc` or `web`.
fn repo_did(entry: &str) -> Result<Did, ConfigError> {
    let refuse = |problem: &str| ConfigError::new(REPO_DIDS_VAR, entry, problem);
    let (method, identifier) = entry
        .strip_prefix("did:")
        .and_then(|rest| rest.split_once(':'))
        .ok_or_else(|| refuse("is not a DID (did:<method>:<identifier>)"))?;
    if entry.len() > MAX_DID_LENGTH
        || method.is_empty()
        || !method.chars().all(|c| c.is_ascii_lowercase())
        || !did_identifier_valid(identifier)
    {
        return Err(refuse("is not a well-formed DID"));
    }
    if !REPO_DID_METHODS.contains(&method) {
        return Err(refuse("names no repo: only did:plc and did:web DIDs do"));
    }
    Ok(Did(entry.to_string()))
}

/// `[A-Za-z0-9._:%-]+`, not ending in `:` (so no `#fragment`, no empty id).
fn did_identifier_valid(identifier: &str) -> bool {
    !identifier.is_empty()
        && !identifier.ends_with(':')
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-'))
}

/// The fallback source: an absolute URL without query or fragment that passes
/// the production pre-check (https to a public host, no userinfo) — or, under
/// the test policy only, plain http to a loopback address.
fn fallback_url(url: &str, policy: TransportPolicy) -> Result<FallbackUrl, ConfigError> {
    let plain_http_under_test_policy = policy == TransportPolicy::HttpsOrLoopbackHttp
        && url.to_ascii_lowercase().starts_with("http://");
    let admissible = !url.contains(['?', '#'])
        && (pds_endpoint_admissible(url, TransportPolicy::HttpsPublicOnly).is_ok()
            || (plain_http_under_test_policy && pds_endpoint_admissible(url, policy).is_ok()));
    admissible
        .then(|| FallbackUrl::new(url))
        .flatten()
        .ok_or_else(|| {
            ConfigError::new(
                FALLBACK_VAR,
                url,
                "must be an absolute https URL to a public host, without credentials, query or fragment",
            )
        })
}

/// A set whole number within `range`; unset takes `default`.
fn bounded_number(
    variable: &'static str,
    value: Option<&str>,
    range: RangeInclusive<u64>,
    default: u64,
) -> Result<u64, ConfigError> {
    value.map_or(Ok(default), |value| {
        value
            .parse::<u64>()
            .ok()
            .filter(|number| range.contains(number))
            .ok_or_else(|| {
                ConfigError::new(
                    variable,
                    value,
                    format!(
                        "must be a whole number from {} to {}",
                        range.start(),
                        range.end()
                    ),
                )
            })
    })
}

/// `<home>/.local/share/openlore-indexer/index.duckdb`.
fn default_index_path(home: Option<String>) -> PathBuf {
    home.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("share")
        .join("openlore-indexer")
        .join("index.duckdb")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    fn parse(env: &BTreeMap<&str, String>) -> Result<IndexerConfig, ConfigError> {
        parse_config(|name| env.get(name).cloned(), BuildProfile::Development)
    }

    proptest! {
        /// Universe {repo_dids, fallback}: the DIDs keep their listed order
        /// whatever separators surround them, and a blank fallback is no
        /// fallback while a set one loses only its trailing slashes.
        #[test]
        fn repo_dids_and_fallback_are_read_as_configured(
            dids in proptest::collection::btree_set("did:plc:[a-z0-9]{1,12}", 0..6),
            separator in prop_oneof![Just(","), Just(" "), Just(", "), Just("\n")],
            fallback in proptest::option::of("https://[a-z]{1,10}\\.[a-z]{2,4}"),
            slashes in 0usize..3,
        ) {
            let dids: Vec<String> = dids.into_iter().collect();
            let mut env = BTreeMap::new();
            env.insert(REPO_DIDS_VAR, dids.join(separator));
            if let Some(url) = &fallback {
                env.insert(FALLBACK_VAR, format!("{url}{}", "/".repeat(slashes)));
            }
            let config = parse(&env).expect("valid configuration loads");
            let parsed: Vec<String> = config.repo_dids.iter().map(|d| d.0.clone()).collect();
            prop_assert_eq!(parsed, dids);
            prop_assert_eq!(
                config.fallback.map(|f| f.as_str().to_string()),
                fallback
            );
        }

        /// Universe {max_concurrent_fetches, per_did_time_budget}: a number is
        /// loaded exactly when it lies in its range, else it is refused naming
        /// its variable and value.
        #[test]
        fn fan_out_bounds_load_exactly_within_their_range(
            fetches in 0u64..40, secs in 0u64..1200
        ) {
            let env = BTreeMap::from([
                (MAX_CONCURRENT_FETCHES_VAR, fetches.to_string()),
                (PER_DID_TIMEOUT_SECS_VAR, secs.to_string()),
            ]);
            match parse(&env) {
                Ok(config) => {
                    prop_assert!(MAX_CONCURRENT_FETCHES_RANGE.contains(&fetches));
                    prop_assert!(PER_DID_TIMEOUT_SECS_RANGE.contains(&secs));
                    prop_assert_eq!(config.max_concurrent_fetches as u64, fetches);
                    prop_assert_eq!(config.per_did_time_budget, Duration::from_secs(secs));
                }
                Err(refusal) if !MAX_CONCURRENT_FETCHES_RANGE.contains(&fetches) => {
                    prop_assert_eq!(refusal.variable, MAX_CONCURRENT_FETCHES_VAR);
                    prop_assert_eq!(refusal.value, fetches.to_string());
                }
                Err(refusal) => {
                    prop_assert!(!PER_DID_TIMEOUT_SECS_RANGE.contains(&secs));
                    prop_assert_eq!(refusal.variable, PER_DID_TIMEOUT_SECS_VAR);
                    prop_assert_eq!(refusal.value, secs.to_string());
                }
            }
        }
    }

    // bypass: a single absent-variable example pins the documented defaults.
    #[test]
    fn unset_variables_take_the_documented_defaults() {
        let config = parse(&BTreeMap::new()).expect("an empty environment loads");
        assert_eq!(config.fallback, None);
        assert_eq!(config.plc_endpoint, DEFAULT_PLC_ENDPOINT);
        assert_eq!(config.listen_addr, DEFAULT_LISTEN_ADDR);
        assert!(config.repo_dids.is_empty());
        assert_eq!(config.policy, TransportPolicy::HttpsPublicOnly);
        assert_eq!(config.max_concurrent_fetches, 4);
        assert_eq!(config.per_did_time_budget, Duration::from_secs(30));
    }
}

/// CORE-7 / CORE-8 / CORE-9 of `tests/acceptance/indexer_pass_core.rs`,
/// moved here VERBATIM (with their view types, generators and binding):
/// `parse_config` is private to this binary crate, and a lib target would make
/// `cli` link the indexer's crates (check-arch I-3).
#[cfg(test)]
mod pass_core_properties {
    use std::collections::{BTreeMap, BTreeSet};

    use proptest::prelude::*;

    use super::{parse_config, BuildProfile};
    use ports::net_policy::TransportPolicy;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Policy {
        HttpsPublicOnly,
        HttpsOrLoopbackHttp,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct LoadedConfig {
        repo_dids: Vec<String>,
        fallback: Option<String>,
        max_concurrent_fetches: u64,
        per_did_time_budget_secs: u64,
        policy: Policy,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ConfigRefusal {
        variable: String,
        value: String,
    }

    fn sut_parse_config(
        env: &BTreeMap<String, String>,
        release_build: bool,
    ) -> Result<LoadedConfig, ConfigRefusal> {
        let profile = if release_build {
            BuildProfile::Release
        } else {
            BuildProfile::Development
        };
        parse_config(|name| env.get(name).cloned(), profile)
            .map(|cfg| LoadedConfig {
                repo_dids: cfg.repo_dids.into_iter().map(|did| did.0).collect(),
                fallback: cfg.fallback.map(|f| f.as_str().to_string()),
                max_concurrent_fetches: cfg.max_concurrent_fetches as u64,
                per_did_time_budget_secs: cfg.per_did_time_budget.as_secs(),
                policy: match cfg.policy {
                    TransportPolicy::HttpsPublicOnly => Policy::HttpsPublicOnly,
                    TransportPolicy::HttpsOrLoopbackHttp => Policy::HttpsOrLoopbackHttp,
                },
            })
            .map_err(|error| ConfigRefusal {
                variable: error.variable.to_string(),
                value: error.value,
            })
    }

    fn arb_host() -> impl Strategy<Value = String> {
        "[a-z]{1,10}(\\.[a-z]{2,8}){1,2}"
    }

    fn arb_public_https_url() -> impl Strategy<Value = String> {
        arb_host().prop_map(|h| format!("https://{h}"))
    }

    fn arb_did() -> impl Strategy<Value = String> {
        ("(plc|web)", "[a-z0-9]{1,24}").prop_map(|(m, id)| format!("did:{m}:{id}"))
    }

    const KNOWN_VARIABLES: [&str; 5] = [
        "OPENLORE_INDEXER_REPO_DIDS",
        "OPENLORE_INDEXER_SOURCE_URL",
        "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP",
        "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES",
        "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS",
    ];

    fn arb_env() -> impl Strategy<Value = BTreeMap<String, String>> {
        proptest::collection::btree_map(
            proptest::sample::select(KNOWN_VARIABLES.to_vec()).prop_map(str::to_string),
            prop_oneof![
                Just(String::new()),
                "[ -~]{0,40}",
                "[0-9]{1,4}",
                arb_did(),
                arb_public_https_url(),
            ],
            0..5,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// CORE-7 @property @US-IPF-004 @AC-004.1 @AC-004.2 @AC-004.3 @C6a @C6c @contract-shape:pure-function
        /// Config parsing is total: for any environment it either loads or refuses
        /// naming one of the indexer's variables and a value taken from it.
        #[test]
        fn config_parsing_is_total_and_every_refusal_names_a_variable_and_its_value(env in arb_env()) {
            match sut_parse_config(&env, false) {
                Ok(cfg) => {
                    prop_assert!((1..=16).contains(&cfg.max_concurrent_fetches));
                    prop_assert!((1..=600).contains(&cfg.per_did_time_budget_secs));
                    let distinct: BTreeSet<&String> = cfg.repo_dids.iter().collect();
                    prop_assert_eq!(distinct.len(), cfg.repo_dids.len(), "duplicates collapsed");
                }
                Err(refusal) => {
                    prop_assert!(KNOWN_VARIABLES.contains(&refusal.variable.as_str()), "{:?}", refusal);
                    let raw = env.get(&refusal.variable).cloned().unwrap_or_default();
                    prop_assert!(raw.contains(&refusal.value), "{:?} not in {:?}", refusal, raw);
                }
            }
        }

        /// CORE-8 @property @US-IPF-004 @AC-004.5 @R-IPF-9 @release-gate @contract-shape:pure-function
        /// A release build refuses the loopback test seam whatever else is set; a
        /// debug build turns it into the loopback-http test policy.
        #[test]
        fn a_release_build_refuses_the_loopback_seam_whatever_else_is_set(mut env in arb_env()) {
            env.insert("OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP".to_string(), "1".to_string());
            let refusal = sut_parse_config(&env, true);
            prop_assert!(
                matches!(&refusal, Err(r) if r.variable == "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP"),
                "{:?}", refusal
            );
            env.retain(|k, _| k == "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP");
            prop_assert_eq!(
                sut_parse_config(&env, false).map(|c| c.policy),
                Ok(Policy::HttpsOrLoopbackHttp)
            );
        }

        /// CORE-9 @property @US-IPF-004 @AC-004.1 @AC-002.7 @C3 @C4a @contract-shape:pure-function
        /// Any list of valid DIDs (comma or whitespace separated, with repeats)
        /// loads as the distinct DIDs in first-seen order.
        #[test]
        fn a_list_of_valid_dids_loads_as_its_distinct_dids_in_order(
            dids in proptest::collection::vec(arb_did(), 0..12), comma in any::<bool>()
        ) {
            let mut repeated = dids.clone();
            repeated.extend(dids.iter().take(3).cloned());
            let sep = if comma { "," } else { " " };
            let env = BTreeMap::from([(
                "OPENLORE_INDEXER_REPO_DIDS".to_string(),
                repeated.join(sep),
            )]);
            let mut expected: Vec<String> = Vec::new();
            for d in &dids {
                if !expected.contains(d) {
                    expected.push(d.clone());
                }
            }
            prop_assert_eq!(sut_parse_config(&env, false).map(|c| c.repo_dids), Ok(expected));
        }
    }
}

/// Behaviour contracts pinning DID syntax edges, refusal messages, the
/// fallback's test-seam rule and the default index path (DELIVER Phase 5).
#[cfg(test)]
#[path = "config_contracts.rs"]
mod contracts;
