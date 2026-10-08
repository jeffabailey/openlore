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

use adapter_xrpc_query_server::rate_limit::{DEFAULT_RATE_BURST, DEFAULT_RATE_PER_SEC};
use adapter_xrpc_query_server::{RateLimit, TrustedProxies};
use appview_domain::did_list::read_did_list;
#[cfg(test)]
use appview_domain::did_list::MAX_DID_LENGTH;
use appview_domain::ingest_pass::pds_endpoint_admissible;
use appview_domain::FallbackUrl;
use claim_domain::Did;
use ports::net_policy::TransportPolicy;

/// The repo DIDs one ingest pass enumerates.
pub const REPO_DIDS_VAR: &str = "OPENLORE_INDEXER_REPO_DIDS";
/// The DID list FILE every in-`serve` pass reads afresh (ADR-081).
pub const REPO_DIDS_FILE_VAR: &str = "OPENLORE_INDEXER_REPO_DIDS_FILE";
/// The Unix control socket `serve` listens on for `trigger` (ADR-080 §3).
pub const CONTROL_SOCKET_VAR: &str = "OPENLORE_INDEXER_CONTROL_SOCKET";
/// Purge authors removed from the DID list (ADR-082): `1` or unset.
pub const PURGE_UNLISTED_VAR: &str = "OPENLORE_INDEXER_PURGE_UNLISTED";
/// The optional fallback listing source.
pub const FALLBACK_VAR: &str = "OPENLORE_INDEXER_SOURCE_URL";
/// TEST-ONLY: admits plain http to loopback in a debug build.
pub const LOOPBACK_SEAM_VAR: &str = "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP";
/// How many DIDs are fetched at once.
pub const MAX_CONCURRENT_FETCHES_VAR: &str = "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES";
/// One DID's whole-fetch deadline, in seconds.
pub const PER_DID_TIMEOUT_SECS_VAR: &str = "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS";
/// DuckDB's memory limit for the index, in MiB (B9); unset keeps DuckDB's default.
pub const DUCKDB_MEMORY_LIMIT_MB_VAR: &str = "OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB";
/// DuckDB's worker threads for the index (B9); unset keeps DuckDB's default.
pub const DUCKDB_THREADS_VAR: &str = "OPENLORE_INDEXER_DUCKDB_THREADS";
/// How long one pass may run before it ends with exit 2 (B13), in seconds.
pub const PASS_DEADLINE_SECS_VAR: &str = "OPENLORE_INDEXER_PASS_DEADLINE_SECS";
/// Searches one client may make per second, sustained (review H3).
pub const RATE_LIMIT_PER_SEC_VAR: &str = "OPENLORE_INDEXER_RATE_LIMIT_PER_SEC";
/// Searches one client may make at once after being idle (review H3).
pub const RATE_LIMIT_BURST_VAR: &str = "OPENLORE_INDEXER_RATE_LIMIT_BURST";
/// The proxies (addresses or CIDR networks, e.g. Caddy's container network)
/// whose `X-Forwarded-For` names the client; loopback is always trusted.
pub const TRUSTED_PROXIES_VAR: &str = "OPENLORE_INDEXER_TRUSTED_PROXIES";
/// TEST-ONLY: provokes a fault a real process cannot be driven into from
/// outside (DISTILL decision 1); a release build refuses to start with it set.
pub const TEST_FAULT_VAR: &str = "OPENLORE_INDEXER_TEST_FAULT";
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

/// The DuckDB caps' admissible ranges (data-models §1); production 48 MiB, 1 thread.
const DUCKDB_MEMORY_LIMIT_MB_RANGE: RangeInclusive<u64> = 16..=1024;
const DUCKDB_THREADS_RANGE: RangeInclusive<u64> = 1..=4;

/// One pass's deadline (ADR-080 §8): 25 minutes unless configured.
const DEFAULT_PASS_DEADLINE_SECS: u64 = 1500;
const PASS_DEADLINE_SECS_RANGE: RangeInclusive<u64> = 60..=7200;

/// The per-client rate limit's admissible ranges.
const RATE_LIMIT_PER_SEC_RANGE: RangeInclusive<u64> = 1..=1000;
const RATE_LIMIT_BURST_RANGE: RangeInclusive<u64> = 1..=10_000;

/// A fault the TEST-ONLY seam provokes inside a development build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFault {
    /// The first in-`serve` pass panics.
    FirstPassPanics,
    /// The store's connection mutex is poisoned at the next pass's start.
    StorePoisoned,
    /// Every search read of the store fails.
    SearchStoreReadFails,
    /// The first author purge fails; later ones succeed.
    PurgeFails,
}

impl TestFault {
    const ALL: [Self; 4] = [
        Self::FirstPassPanics,
        Self::StorePoisoned,
        Self::SearchStoreReadFails,
        Self::PurgeFails,
    ];

    /// The seam's value naming this fault.
    pub const fn token(self) -> &'static str {
        match self {
            Self::FirstPassPanics => "first_pass_panics",
            Self::StorePoisoned => "store_poisoned",
            Self::SearchStoreReadFails => "search_store_read_fails",
            Self::PurgeFails => "purge_fails",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|fault| fault.token() == token)
    }
}

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
    /// The DID list file each in-`serve` pass reads afresh (ADR-081);
    /// `None` = the passes use `repo_dids`.
    pub repo_dids_file: Option<PathBuf>,
    /// The ABSOLUTE path of `serve`'s Unix control socket (ADR-080 §3);
    /// `None` = no control channel.
    pub control_socket: Option<PathBuf>,
    /// The PLC directory each repo DID's document is resolved from.
    pub plc_endpoint: String,
    /// Which PDS addresses may be contacted (DD-IPF-5).
    pub policy: TransportPolicy,
    /// How many DIDs are fetched at once (ADR-078).
    pub max_concurrent_fetches: usize,
    /// The single deadline each DID's fetch runs under (ADR-078).
    pub per_did_time_budget: Duration,
    /// Each in-`serve` pass purges the authors its loaded list no longer
    /// names (ADR-082); off, a removed author's claims stay (ADR-078).
    pub purge_unlisted: bool,
    /// DuckDB's memory limit in MiB; `None` keeps DuckDB's default (B9).
    pub duckdb_memory_limit_mb: Option<u64>,
    /// DuckDB's worker threads; `None` keeps DuckDB's default (B9).
    pub duckdb_threads: Option<u64>,
    /// How long one pass may run before it ends with exit 2 (B13).
    pub pass_deadline: Duration,
    /// How often one client may search (review H3).
    pub rate_limit: RateLimit,
    /// Whose `X-Forwarded-For` names the client (loopback always).
    pub trusted_proxies: TrustedProxies,
    /// The TEST-ONLY fault to provoke; always `None` in a release build.
    pub test_fault: Option<TestFault>,
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
    let test_fault = test_fault(lookup(TEST_FAULT_VAR).as_deref(), profile)?;
    let inline_list = setting(REPO_DIDS_VAR);
    let repo_dids = parse_repo_dids(inline_list.as_deref().unwrap_or_default())?;
    let repo_dids_file = setting(REPO_DIDS_FILE_VAR)
        .map(|file| list_file(&file, inline_list.is_some()))
        .transpose()?;
    let control_socket = setting(CONTROL_SOCKET_VAR)
        .map(|path| control_socket_path(&path))
        .transpose()?;
    let fallback = setting(FALLBACK_VAR)
        .map(|url| fallback_url(&url, policy))
        .transpose()?;
    let plc_endpoint = lookup(PLC_ENDPOINT_VAR)
        .map_or(Ok(DEFAULT_PLC_ENDPOINT.to_string()), |url| {
            plc_endpoint(url.trim(), policy)
        })?;
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
    let purge_unlisted = purge_switch(setting(PURGE_UNLISTED_VAR).as_deref())?;
    let set = |name: &str| lookup(name).map(|value| value.trim().to_string());
    let duckdb_memory_limit_mb = optional_number(
        DUCKDB_MEMORY_LIMIT_MB_VAR,
        set(DUCKDB_MEMORY_LIMIT_MB_VAR).as_deref(),
        DUCKDB_MEMORY_LIMIT_MB_RANGE,
    )?;
    let duckdb_threads = optional_number(
        DUCKDB_THREADS_VAR,
        set(DUCKDB_THREADS_VAR).as_deref(),
        DUCKDB_THREADS_RANGE,
    )?;
    let pass_deadline_secs = bounded_number(
        PASS_DEADLINE_SECS_VAR,
        set(PASS_DEADLINE_SECS_VAR).as_deref(),
        PASS_DEADLINE_SECS_RANGE,
        DEFAULT_PASS_DEADLINE_SECS,
    )?;
    let rate_limit = rate_limit(
        set(RATE_LIMIT_PER_SEC_VAR).as_deref(),
        set(RATE_LIMIT_BURST_VAR).as_deref(),
    )?;
    let trusted_proxies = trusted_proxies(setting(TRUSTED_PROXIES_VAR).as_deref())?;
    Ok(IndexerConfig {
        index_path: lookup(INDEX_PATH_VAR)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_index_path(lookup(HOME_VAR))),
        fallback,
        listen_addr: lookup(LISTEN_ADDR_VAR).unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string()),
        repo_dids,
        repo_dids_file,
        control_socket,
        plc_endpoint,
        policy,
        max_concurrent_fetches: usize::try_from(max_concurrent_fetches).unwrap_or(usize::MAX),
        per_did_time_budget: Duration::from_secs(per_did_timeout_secs),
        purge_unlisted,
        duckdb_memory_limit_mb,
        duckdb_threads,
        pass_deadline: Duration::from_secs(pass_deadline_secs),
        rate_limit,
        trusted_proxies,
        test_fault,
    })
}

/// The per-client rate limit: each number within its range, unset taking
/// the default (10 a second, bursts of 50).
fn rate_limit(per_sec: Option<&str>, burst: Option<&str>) -> Result<RateLimit, ConfigError> {
    let per_sec = bounded_number(
        RATE_LIMIT_PER_SEC_VAR,
        per_sec,
        RATE_LIMIT_PER_SEC_RANGE,
        u64::from(DEFAULT_RATE_PER_SEC),
    )?;
    let burst = bounded_number(
        RATE_LIMIT_BURST_VAR,
        burst,
        RATE_LIMIT_BURST_RANGE,
        u64::from(DEFAULT_RATE_BURST),
    )?;
    let within = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(RateLimit::new(within(per_sec), within(burst)).unwrap_or(RateLimit::DEFAULT))
}

/// The trusted proxies; the first entry that is neither an address nor a
/// CIDR network refuses the list.
fn trusted_proxies(list: Option<&str>) -> Result<TrustedProxies, ConfigError> {
    list.map_or(Ok(TrustedProxies::loopback_only()), |list| {
        TrustedProxies::parse(list).map_err(|entry| {
            ConfigError::new(
                TRUSTED_PROXIES_VAR,
                &entry,
                "must be an IP address or CIDR network (e.g. 172.18.0.0/16)",
            )
        })
    })
}

/// The TEST-ONLY fault seam: refused whenever it is set in a release build
/// (like the loopback seam); in a development build it must name a known
/// fault. Unset (or blank) provokes nothing.
fn test_fault(seam: Option<&str>, profile: BuildProfile) -> Result<Option<TestFault>, ConfigError> {
    match (seam.map(str::trim), profile) {
        (None, _) => Ok(None),
        (Some(value), BuildProfile::Release) => Err(ConfigError::new(
            TEST_FAULT_VAR,
            value,
            "is a test-only seam; a release build refuses to start with it set",
        )),
        (Some(""), BuildProfile::Development) => Ok(None),
        (Some(value), BuildProfile::Development) => TestFault::from_token(value)
            .map(Some)
            .ok_or_else(|| ConfigError::new(TEST_FAULT_VAR, value, "names no known fault")),
    }
}

/// The purge switch: on only for `1`, off when unset; anything else refused.
fn purge_switch(value: Option<&str>) -> Result<bool, ConfigError> {
    match value {
        None => Ok(false),
        Some("1") => Ok(true),
        Some(other) => Err(ConfigError::new(
            PURGE_UNLISTED_VAR,
            other,
            "must be 1 (purge authors removed from the DID list) or unset",
        )),
    }
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

/// The control socket must be an absolute path (a relative one would depend on
/// the working directory of whoever starts `serve` or `trigger`).
fn control_socket_path(path: &str) -> Result<PathBuf, ConfigError> {
    let socket = PathBuf::from(path);
    if socket.is_absolute() {
        Ok(socket)
    } else {
        Err(ConfigError::new(
            CONTROL_SOCKET_VAR,
            path,
            "must be an absolute path",
        ))
    }
}

/// The list file, unless the inline list is set too: the two are mutually
/// exclusive (ADR-081 §6), so a deployment never wonders which one won.
fn list_file(path: &str, inline_list_set: bool) -> Result<PathBuf, ConfigError> {
    if inline_list_set {
        Err(ConfigError::new(
            REPO_DIDS_FILE_VAR,
            path,
            format!("is mutually exclusive with {REPO_DIDS_VAR}: set one of them"),
        ))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// The distinct repo DIDs of a comma- or whitespace-separated list, in
/// first-seen order; the first malformed entry refuses the whole list.
pub fn parse_repo_dids(list: &str) -> Result<Vec<Did>, ConfigError> {
    read_did_list(list)
        .map_err(|bad| ConfigError::new(REPO_DIDS_VAR, &bad.entry, bad.problem.describe()))
}

/// A source URL the indexer may be configured with: absolute, without query
/// or fragment, passing the production pre-check (https to a public host, no
/// userinfo) — or, under the test policy only, plain http to a loopback
/// address. A hostname passes here; what it resolves to is judged at runtime.
fn source_url_admissible(url: &str, policy: TransportPolicy) -> bool {
    let plain_http_under_test_policy = policy == TransportPolicy::HttpsOrLoopbackHttp
        && url.to_ascii_lowercase().starts_with("http://");
    !url.contains(['?', '#'])
        && (pds_endpoint_admissible(url, TransportPolicy::HttpsPublicOnly).is_ok()
            || (plain_http_under_test_policy && pds_endpoint_admissible(url, policy).is_ok()))
}

const SOURCE_URL_PROBLEM: &str =
    "must be an absolute https URL to a public host, without credentials, query or fragment";

/// The fallback source: an admissible source URL.
fn fallback_url(url: &str, policy: TransportPolicy) -> Result<FallbackUrl, ConfigError> {
    source_url_admissible(url, policy)
        .then(|| FallbackUrl::new(url))
        .flatten()
        .ok_or_else(|| ConfigError::new(FALLBACK_VAR, url, SOURCE_URL_PROBLEM))
}

/// The PLC directory: an admissible source URL, exactly as the guarded client
/// that resolves through it would admit it. Set but blank is refused (only an
/// UNSET variable means the default directory).
fn plc_endpoint(url: &str, policy: TransportPolicy) -> Result<String, ConfigError> {
    if url.is_empty() {
        return Err(ConfigError::new(
            PLC_ENDPOINT_VAR,
            url,
            format!(
                "is set but blank; unset it to use the default directory {DEFAULT_PLC_ENDPOINT}"
            ),
        ));
    }
    source_url_admissible(url, policy)
        .then(|| url.to_string())
        .ok_or_else(|| ConfigError::new(PLC_ENDPOINT_VAR, url, SOURCE_URL_PROBLEM))
}

/// A set whole number within `range`; unset takes `default`.
fn bounded_number(
    variable: &'static str,
    value: Option<&str>,
    range: RangeInclusive<u64>,
    default: u64,
) -> Result<u64, ConfigError> {
    value.map_or(Ok(default), |value| ranged_number(variable, value, range))
}

/// A set whole number within `range`; unset stays `None` (the consumer's
/// own default applies).
fn optional_number(
    variable: &'static str,
    value: Option<&str>,
    range: RangeInclusive<u64>,
) -> Result<Option<u64>, ConfigError> {
    value
        .map(|value| ranged_number(variable, value, range))
        .transpose()
}

/// `value` as a whole number within `range`, else refused naming `variable`
/// (a blank value is not a number).
fn ranged_number(
    variable: &'static str,
    value: &str,
    range: RangeInclusive<u64>,
) -> Result<u64, ConfigError> {
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
        /// The control socket loads iff its path is absolute (DISTILL decision
        /// 4); a refusal names the variable and the value as given.
        #[test]
        fn a_control_socket_loads_iff_its_path_is_absolute(
            rooted in any::<bool>(),
            segments in proptest::collection::vec("[a-z0-9._-]{1,12}", 1..4),
        ) {
            let relative = segments.join("/");
            let path = if rooted { format!("/{relative}") } else { relative };
            let env = BTreeMap::from([(CONTROL_SOCKET_VAR, path.clone())]);
            match parse(&env) {
                Ok(config) => {
                    prop_assert!(rooted);
                    prop_assert_eq!(config.control_socket, Some(PathBuf::from(&path)));
                }
                Err(error) => {
                    prop_assert!(!rooted);
                    prop_assert_eq!(error.variable, CONTROL_SOCKET_VAR);
                    prop_assert_eq!(error.value, path);
                }
            }
        }
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

        /// Universe {rate_limit}: the rate and burst load exactly within
        /// 1..=1000 and 1..=10000, else the first one out of range is refused
        /// naming its variable and value.
        #[test]
        fn rate_limit_bounds_load_exactly_within_their_range(
            per_sec in 0u64..1_100, burst in 0u64..10_100
        ) {
            let env = BTreeMap::from([
                (RATE_LIMIT_PER_SEC_VAR, per_sec.to_string()),
                (RATE_LIMIT_BURST_VAR, burst.to_string()),
            ]);
            let per_sec_ok = (1..=1000).contains(&per_sec);
            let burst_ok = (1..=10_000).contains(&burst);
            match parse(&env) {
                Ok(config) => {
                    prop_assert!(per_sec_ok && burst_ok);
                    prop_assert_eq!(u64::from(config.rate_limit.per_sec()), per_sec);
                    prop_assert_eq!(u64::from(config.rate_limit.burst()), burst);
                }
                Err(refusal) if !per_sec_ok => {
                    prop_assert_eq!(refusal.variable, RATE_LIMIT_PER_SEC_VAR);
                    prop_assert_eq!(refusal.value, per_sec.to_string());
                }
                Err(refusal) => {
                    prop_assert!(!burst_ok);
                    prop_assert_eq!(refusal.variable, RATE_LIMIT_BURST_VAR);
                    prop_assert_eq!(refusal.value, burst.to_string());
                }
            }
        }

        /// Universe {trusted_proxies}: a list of CIDR networks loads; one bad
        /// entry refuses the list naming that entry.
        #[test]
        fn a_trusted_proxy_list_loads_iff_every_entry_is_a_network(
            octet in 0u8..=255, prefix in 0u8..=40, junk in "[a-z]{1,8}"
        ) {
            let network = format!("10.{octet}.0.0/{prefix}");
            let env = BTreeMap::from([(TRUSTED_PROXIES_VAR, format!("127.0.0.1, {network}"))]);
            match parse(&env) {
                Ok(config) => {
                    prop_assert!(prefix <= 32);
                    prop_assert!(config.trusted_proxies.trusts(
                        std::net::Ipv4Addr::new(10, octet, 0, 1).into()
                    ) || prefix > 24);
                }
                Err(refusal) => {
                    prop_assert!(prefix > 32);
                    prop_assert_eq!(refusal.variable, TRUSTED_PROXIES_VAR);
                    prop_assert_eq!(refusal.value, network);
                }
            }
            let env = BTreeMap::from([(TRUSTED_PROXIES_VAR, format!("10.0.0.0/8 {junk}"))]);
            let refusal = parse(&env).expect_err("a name is not a network");
            prop_assert_eq!(refusal.variable, TRUSTED_PROXIES_VAR);
            prop_assert_eq!(refusal.value, junk);
        }
    }

    /// The shapes a PLC directory endpoint can take, each with the verdict
    /// the transport policy gives it (no DNS: hostnames are judged at runtime).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PlcShape {
        PublicHttps,
        LoopbackHttp,
        PlainHttpToAHost,
        PrivateLiteral,
        Userinfo,
        QueryOrFragment,
        Blank,
        Garbage,
    }

    fn arb_plc_endpoint() -> impl Strategy<Value = (PlcShape, String)> {
        let host = "[a-z]{1,10}\\.[a-z]{2,6}";
        prop_oneof![
            host.prop_map(|h| (PlcShape::PublicHttps, format!("https://{h}"))),
            (1u16..=65535)
                .prop_map(|port| (PlcShape::LoopbackHttp, format!("http://127.0.0.1:{port}"))),
            host.prop_map(|h| (PlcShape::PlainHttpToAHost, format!("http://{h}"))),
            prop_oneof![
                (0u8..=255, 0u8..=255).prop_map(|(b, c)| format!("https://10.{b}.{c}.1")),
                (0u8..=255).prop_map(|c| format!("https://192.168.{c}.1")),
                Just("https://127.0.0.1".to_string()),
                Just("https://[::1]".to_string()),
            ]
            .prop_map(|url| (PlcShape::PrivateLiteral, url)),
            host.prop_map(|h| (PlcShape::Userinfo, format!("https://u:p@{h}"))),
            (host, prop_oneof![Just("?a=1"), Just("#frag")])
                .prop_map(|(h, tail)| (PlcShape::QueryOrFragment, format!("https://{h}/{tail}"))),
            "[ \t]{0,3}".prop_map(|blank| (PlcShape::Blank, blank)),
            "[a-z][a-z ]{0,19}".prop_map(|text| (PlcShape::Garbage, text)),
        ]
    }

    proptest! {
        /// Universe = the whole loaded [`IndexerConfig`]. A PLC endpoint is
        /// accepted iff the transport policy admits it (loopback http only
        /// under the test seam; a set-but-blank value is refused); accepting it
        /// changes ONLY `plc_endpoint`, and refusing it names
        /// `PLC_ENDPOINT_VAR` and the (trimmed) value.
        #[test]
        fn a_plc_endpoint_is_accepted_iff_the_policy_admits_it(
            (shape, url) in arb_plc_endpoint(),
            seam in any::<bool>(),
            padding in prop_oneof![Just(""), Just(" "), Just("\n")],
        ) {
            let mut env = BTreeMap::new();
            if seam {
                env.insert(LOOPBACK_SEAM_VAR, "1".to_string());
            }
            let baseline = parse(&env).expect("the seam alone loads");
            env.insert(PLC_ENDPOINT_VAR, format!("{padding}{url}{padding}"));
            let admitted = match shape {
                PlcShape::PublicHttps => true,
                PlcShape::LoopbackHttp => seam,
                _ => false,
            };
            match parse(&env) {
                Ok(config) => {
                    prop_assert!(admitted, "{:?} {:?} was accepted", shape, url);
                    let expected = IndexerConfig { plc_endpoint: url.clone(), ..baseline };
                    prop_assert_eq!(config, expected);
                }
                Err(refusal) => {
                    prop_assert!(!admitted, "{:?} {:?} was refused: {}", shape, url, refusal);
                    prop_assert_eq!(refusal.variable, PLC_ENDPOINT_VAR);
                    prop_assert_eq!(refusal.value, url.trim());
                }
            }
        }
    }

    /// What a development build does with one fault-seam value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SeamOutcome {
        Loads(TestFault),
        /// Blank: provokes nothing, like an unset seam.
        NoFault,
        Refused,
    }

    /// The fault seam's documented values, written out literally (never
    /// derived from `TestFault::ALL`, `token` or `from_token`), with near
    /// misses of each. Every setting is trimmed, so surrounding whitespace
    /// still names the fault, and a blank value provokes nothing.
    const FAULT_SEAM_TABLE: [(&str, SeamOutcome); 18] = [
        (
            "first_pass_panics",
            SeamOutcome::Loads(TestFault::FirstPassPanics),
        ),
        (
            "store_poisoned",
            SeamOutcome::Loads(TestFault::StorePoisoned),
        ),
        (
            "search_store_read_fails",
            SeamOutcome::Loads(TestFault::SearchStoreReadFails),
        ),
        ("purge_fails", SeamOutcome::Loads(TestFault::PurgeFails)),
        ("purge_fails ", SeamOutcome::Loads(TestFault::PurgeFails)),
        (
            "\tstore_poisoned\n",
            SeamOutcome::Loads(TestFault::StorePoisoned),
        ),
        ("", SeamOutcome::NoFault),
        ("   ", SeamOutcome::NoFault),
        ("FIRST_PASS_PANICS", SeamOutcome::Refused),
        ("Store_Poisoned", SeamOutcome::Refused),
        ("first_pass", SeamOutcome::Refused),
        ("purge", SeamOutcome::Refused),
        ("search_store_read_fail", SeamOutcome::Refused),
        ("store_poisoned_", SeamOutcome::Refused),
        ("first pass panics", SeamOutcome::Refused),
        ("first-pass-panics", SeamOutcome::Refused),
        ("purge_fails,store_poisoned", SeamOutcome::Refused),
        ("none", SeamOutcome::Refused),
    ];

    /// The literal token of each fault. No wildcard arm: a new fault does not
    /// compile until it has a literal row.
    const fn literal_token_of(fault: TestFault) -> &'static str {
        match fault {
            TestFault::FirstPassPanics => "first_pass_panics",
            TestFault::StorePoisoned => "store_poisoned",
            TestFault::SearchStoreReadFails => "search_store_read_fails",
            TestFault::PurgeFails => "purge_fails",
        }
    }

    /// A value from the literal table, or a generated lower-case string that
    /// is none of the four literal tokens (so always refused).
    fn arb_fault_seam_value() -> impl Strategy<Value = (String, SeamOutcome)> {
        prop_oneof![
            proptest::sample::select(FAULT_SEAM_TABLE.to_vec())
                .prop_map(|(value, outcome)| (value.to_string(), outcome)),
            "[a-z_]{1,24}"
                .prop_filter("not a literal fault token", |value| {
                    ![
                        "first_pass_panics",
                        "store_poisoned",
                        "search_store_read_fails",
                        "purge_fails",
                    ]
                    .contains(&value.as_str())
                })
                .prop_map(|value| (value, SeamOutcome::Refused)),
        ]
    }

    proptest! {
        /// Universe = the whole loaded [`IndexerConfig`]. The TEST-ONLY fault
        /// seam: a release build refuses it whatever its value, naming it; a
        /// development build loads exactly the literal fault its value names
        /// (changing only `test_fault`), provokes nothing for a blank value,
        /// and refuses any other value naming it.
        #[test]
        fn the_fault_seam_loads_only_known_faults_and_only_in_a_development_build(
            (value, outcome) in arb_fault_seam_value(),
        ) {
            let env = BTreeMap::from([(TEST_FAULT_VAR, value.clone())]);
            let release = parse_config(|name| env.get(name).cloned(), BuildProfile::Release);
            prop_assert_eq!(
                release.map_err(|r| (r.variable, r.value)),
                Err((TEST_FAULT_VAR, value.trim().to_string()))
            );
            let baseline = parse(&BTreeMap::new()).expect("an empty environment loads");
            match (parse(&env), outcome) {
                (Ok(config), SeamOutcome::Loads(fault)) => {
                    prop_assert_eq!(config, IndexerConfig { test_fault: Some(fault), ..baseline });
                }
                (Ok(config), SeamOutcome::NoFault) => prop_assert_eq!(config, baseline),
                (Err(refusal), SeamOutcome::Refused) => {
                    prop_assert_eq!(refusal.variable, TEST_FAULT_VAR);
                    prop_assert_eq!(refusal.value, value);
                }
                (actual, expected) => {
                    prop_assert!(false, "{:?} for {:?}: expected {:?}", actual, value, expected);
                }
            }
        }
    }

    // bypass: a coverage check over the closed set of faults; a generator adds nothing.
    /// Every fault has exactly one literal row naming it by its literal token,
    /// so the table cannot silently fall behind a new fault.
    #[test]
    fn every_test_fault_has_a_literal_row() {
        assert_eq!(TestFault::ALL.len(), 4, "a new fault needs a literal row");
        for fault in TestFault::ALL {
            let rows: Vec<&str> = FAULT_SEAM_TABLE
                .iter()
                .filter(|(value, outcome)| {
                    *outcome == SeamOutcome::Loads(fault) && *value == literal_token_of(fault)
                })
                .map(|(value, _)| *value)
                .collect();
            assert_eq!(rows, [literal_token_of(fault)], "{fault:?}");
        }
    }

    /// The settings `deploy/indexer/host/compose.yaml` ships load in a release
    /// build, and the trusted proxies cover the PDS compose network Caddy
    /// forwards from (any Docker default bridge pool) and nothing public.
    #[test]
    fn the_shipped_compose_settings_load_and_trust_only_the_pds_network() {
        let compose_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../deploy/indexer/host/compose.yaml");
        let compose = std::fs::read_to_string(&compose_path).expect("the shipped compose");
        let env: BTreeMap<String, String> = compose
            .lines()
            .filter_map(|line| line.trim().split_once(": "))
            .filter(|(key, _)| key.starts_with("OPENLORE_INDEXER_"))
            .map(|(key, value)| (key.to_string(), value.trim().trim_matches('"').to_string()))
            .collect();
        assert!(env.contains_key(TRUSTED_PROXIES_VAR), "{env:?}");
        let config = parse_config(|name| env.get(name).cloned(), BuildProfile::Release)
            .expect("the shipped compose configuration loads");
        for caddy in ["172.17.0.2", "172.18.0.4", "172.31.255.9", "192.168.16.3"] {
            let caddy: std::net::IpAddr = caddy.parse().expect("address");
            assert!(config.trusted_proxies.trusts(caddy), "{caddy} is trusted");
        }
        for public in ["203.0.113.9", "10.0.0.5", "172.32.0.1", "8.8.8.8"] {
            let public: std::net::IpAddr = public.parse().expect("address");
            assert!(
                !config.trusted_proxies.trusts(public),
                "{public} is not trusted"
            );
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
        assert_eq!(config.duckdb_memory_limit_mb, None);
        assert_eq!(config.duckdb_threads, None);
        assert_eq!(config.pass_deadline, Duration::from_secs(1500));
        assert_eq!(config.rate_limit, RateLimit::new(10, 50).expect("limit"));
        assert_eq!(config.trusted_proxies, TrustedProxies::loopback_only());
        assert_eq!(config.test_fault, None);
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
        plc_endpoint: String,
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
                plc_endpoint: cfg.plc_endpoint,
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

    const KNOWN_VARIABLES: [&str; 6] = [
        "OPENLORE_INDEXER_REPO_DIDS",
        "OPENLORE_INDEXER_PLC_ENDPOINT",
        "OPENLORE_INDEXER_SOURCE_URL",
        "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP",
        "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES",
        "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS",
    ];

    /// A PLC endpoint the transport policy admits under either policy: unset
    /// (the default directory) or a public https URL; set-but-blank is refused.
    fn plc_endpoint_admitted(raw: Option<&str>) -> bool {
        let Some(trimmed) = raw.map(str::trim) else {
            return true;
        };
        trimmed.starts_with("https://")
            && !trimmed.contains(['@', '?', '#'])
            && !trimmed.starts_with("https://10.")
            && !trimmed.starts_with("https://192.168.")
    }

    /// The PLC dimension: admissible, plain http, a private literal,
    /// userinfo, blank, garbage.
    fn arb_plc_value() -> impl Strategy<Value = String> {
        prop_oneof![
            arb_public_https_url(),
            arb_host().prop_map(|h| format!("http://{h}")),
            (0u8..=255).prop_map(|b| format!("https://10.0.{b}.1")),
            (0u8..=255).prop_map(|c| format!("https://192.168.{c}.1")),
            arb_host().prop_map(|h| format!("https://u:p@{h}")),
            "[ \\t]{0,3}",
            "[a-z][a-z ]{0,19}",
        ]
    }

    fn arb_env() -> impl Strategy<Value = BTreeMap<String, String>> {
        (arb_other_env(), proptest::option::of(arb_plc_value())).prop_map(|(mut env, plc)| {
            env.remove("OPENLORE_INDEXER_PLC_ENDPOINT");
            if let Some(plc) = plc {
                env.insert("OPENLORE_INDEXER_PLC_ENDPOINT".to_string(), plc);
            }
            env
        })
    }

    fn arb_other_env() -> impl Strategy<Value = BTreeMap<String, String>> {
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
                    let plc = env.get("OPENLORE_INDEXER_PLC_ENDPOINT").map(String::as_str);
                    prop_assert!(plc_endpoint_admitted(plc), "a refused PLC endpoint {:?} was accepted", plc);
                    prop_assert!(!cfg.plc_endpoint.is_empty(), "unset takes the default directory");
                }
                Err(refusal) if refusal.variable == "OPENLORE_INDEXER_PLC_ENDPOINT" => {
                    let raw = env.get(&refusal.variable).cloned().unwrap_or_default();
                    prop_assert!(!plc_endpoint_admitted(Some(&raw)), "an admissible {:?} was refused", raw);
                    prop_assert_eq!(refusal.value, raw.trim());
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

/// indexer-deployment CORE-8 (new settings ranges), moved VERBATIM from
/// `tests/acceptance/indexer_deployment_core.rs` for the same reason as
/// `pass_core_properties` above.
#[cfg(test)]
#[path = "deployment_settings_properties.rs"]
mod deployment_settings_properties;
