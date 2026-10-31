//! The Lambda environment, read by a pure parser (ADR-007): a function of the
//! variables, the mode and the build's profile, with no default for anything a
//! deployment must say. `main` refuses on what this returns, before it connects
//! to anything. The variables the self-host indexer shares go through its
//! `parse_config`, so a release build refuses its test seams here in every mode.

use std::fmt;
use std::time::Duration;

use adapter_s3_index_object::{S3Location, SsmLocation, StoreCall};
use openlore_indexer::config::{parse_config, BuildProfile, ConfigError, REPO_DIDS_FILE_VAR};
use ports::{ProbeRefusalReason, ProbeRefused};
use serde_json::json;

use crate::Environment;

/// The bucket holding the index object and the pass record.
pub const BUCKET_VAR: &str = "OPENLORE_INDEXER_LAMBDA_BUCKET";
/// The bucket key of the index object.
pub const INDEX_KEY_VAR: &str = "OPENLORE_INDEXER_LAMBDA_INDEX_KEY";
/// The bucket key of `last-pass.json`.
pub const LAST_PASS_KEY_VAR: &str = "OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY";
/// The parameter holding the operator's DID list (the pass's).
pub const DID_PARAMETER_VAR: &str = "OPENLORE_INDEXER_LAMBDA_DID_PARAMETER";
/// How often a warm search checks the object (UI-5, D-STZ-10).
pub const FRESHNESS_TTL_VAR: &str = "OPENLORE_INDEXER_LAMBDA_FRESHNESS_TTL_SECS";
/// How long the probe lets a pass go missing before it says not live (UI-5).
pub const LAST_PASS_STALE_VAR: &str = "OPENLORE_INDEXER_LAMBDA_LAST_PASS_STALE_SECS";
/// The public edge the probe calls (UI-5).
pub const PROBE_BASE_URL_VAR: &str = "OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL";

/// Lambda sets this for every function.
const REGION_VAR: &str = "AWS_REGION";
/// Moto in the CI lambda-smoke; unset in production.
const ENDPOINT_URL_VAR: &str = "AWS_ENDPOINT_URL";

/// The namespace this root owns, so a name in it that nothing reads is a typo.
const LAMBDA_VAR_PREFIX: &str = "OPENLORE_INDEXER_LAMBDA_";
const LAMBDA_VARS: [&str; 7] = [
    BUCKET_VAR,
    INDEX_KEY_VAR,
    LAST_PASS_KEY_VAR,
    DID_PARAMETER_VAR,
    FRESHNESS_TTL_VAR,
    LAST_PASS_STALE_VAR,
    PROBE_BASE_URL_VAR,
];

/// Which function this process is (D-STZ-24: the image's `command`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Search,
    Pass,
    Probe,
}

impl Mode {
    /// The mode the process's arguments name: one word, and one of the three.
    pub fn of_command(arguments: &[String]) -> Result<Self, ConfigRefused> {
        match arguments {
            [word] if word == "search" => Ok(Self::Search),
            [word] if word == "pass" => Ok(Self::Pass),
            [word] if word == "probe" => Ok(Self::Probe),
            _ => Err(ConfigRefused::of(
                COMMAND,
                &arguments.join(" "),
                "is not exactly one of search, pass or probe",
            )),
        }
    }

    /// The budget this mode's `fetch_to` runs under (§ Timeouts): search has
    /// Lambda's init limit to meet, so its fetch does not get a pass's 60 s.
    #[must_use]
    pub const fn fetch_call(self) -> StoreCall {
        match self {
            Self::Search => StoreCall::SearchInitGet,
            Self::Pass | Self::Probe => StoreCall::PassGet,
        }
    }
}

/// What a refused command names in place of a variable.
const COMMAND: &str = "command";

/// A setting this function cannot start with, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRefused {
    pub variable: String,
    pub value: String,
    pub problem: String,
}

impl ConfigRefused {
    fn of(variable: &str, value: &str, problem: &str) -> Self {
        Self {
            variable: variable.to_string(),
            value: value.to_string(),
            problem: problem.to_string(),
        }
    }

    pub(crate) fn missing(variable: &str) -> Self {
        Self::of(variable, "", "is not set")
    }
}

impl fmt::Display for ConfigRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={:?}: {}", self.variable, self.value, self.problem)
    }
}

impl From<ConfigError> for ConfigRefused {
    fn from(refused: ConfigError) -> Self {
        Self::of(refused.variable, &refused.value, &refused.problem)
    }
}

impl From<ConfigRefused> for ProbeRefused {
    fn from(refused: ConfigRefused) -> Self {
        Self {
            reason: ProbeRefusalReason::IndexerConfigInvalid,
            detail: refused.to_string(),
            structured: json!({
                "variable": refused.variable,
                "value": refused.value,
            }),
        }
    }
}

/// What search reads at init.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSettings {
    pub freshness_ttl: Duration,
}

impl SearchSettings {
    pub(crate) fn read(env: &Environment) -> Result<Self, ConfigRefused> {
        Ok(Self {
            freshness_ttl: seconds(env, FRESHNESS_TTL_VAR)?,
        })
    }
}

/// What the probe reads at init. An unset base URL is refused here, or it would
/// report not live for ever with nothing wrong in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeSettings {
    pub base_url: String,
    pub last_pass_stale_after: Duration,
}

impl ProbeSettings {
    pub(crate) fn read(env: &Environment) -> Result<Self, ConfigRefused> {
        Ok(Self {
            base_url: required(env, PROBE_BASE_URL_VAR)?,
            last_pass_stale_after: seconds(env, LAST_PASS_STALE_VAR)?,
        })
    }
}

/// What only one mode reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeSettings {
    Search(SearchSettings),
    Pass { did_list: SsmLocation },
    Probe(ProbeSettings),
}

/// What `main` composes one function from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LambdaConfig {
    pub object_store: S3Location,
    pub settings: ModeSettings,
}

/// Read `env` for `mode`, in a build of `profile`. Every refusal names its
/// variable. Nothing here has a default, the bucket least of all.
pub fn parse_lambda_config(
    env: &Environment,
    mode: Mode,
    profile: BuildProfile,
) -> Result<LambdaConfig, ConfigRefused> {
    refuse_an_unknown_setting(env)?;
    parse_config(|name| env.0.get(name).cloned(), profile)?;
    let region = required(env, REGION_VAR)?;
    let endpoint_url = optional(env, ENDPOINT_URL_VAR);
    let object_store = S3Location {
        region: region.clone(),
        endpoint_url: endpoint_url.clone(),
        bucket: required(env, BUCKET_VAR)?,
        index_key: required(env, INDEX_KEY_VAR)?,
        last_pass_key: required(env, LAST_PASS_KEY_VAR)?,
    };
    let settings = match mode {
        Mode::Search => ModeSettings::Search(SearchSettings::read(env)?),
        Mode::Probe => ModeSettings::Probe(ProbeSettings::read(env)?),
        Mode::Pass => {
            required(env, REPO_DIDS_FILE_VAR)?;
            ModeSettings::Pass {
                did_list: SsmLocation {
                    region,
                    endpoint_url,
                    parameter: required(env, DID_PARAMETER_VAR)?,
                },
            }
        }
    };
    Ok(LambdaConfig {
        object_store,
        settings,
    })
}

/// A name in this root's namespace that nothing reads would otherwise leave its
/// setting silently at no effect.
fn refuse_an_unknown_setting(env: &Environment) -> Result<(), ConfigRefused> {
    let unknown = env.0.iter().find(|(name, _)| {
        name.starts_with(LAMBDA_VAR_PREFIX) && !LAMBDA_VARS.contains(&name.as_str())
    });
    unknown.map_or(Ok(()), |(name, value)| {
        Err(ConfigRefused::of(
            name,
            value,
            "is not a setting of this function",
        ))
    })
}

/// The value of a variable a deployment must set; blank counts as unset.
fn required(env: &Environment, variable: &str) -> Result<String, ConfigRefused> {
    optional(env, variable).ok_or_else(|| ConfigRefused::missing(variable))
}

fn optional(env: &Environment, variable: &str) -> Option<String> {
    env.0
        .get(variable)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// A required length of time. Zero is refused: it would mean a check on every
/// request, or a probe that is never live.
fn seconds(env: &Environment, variable: &str) -> Result<Duration, ConfigRefused> {
    let value = required(env, variable)?;
    value
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .ok_or_else(|| ConfigRefused::of(variable, &value, "is not a positive number of seconds"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::*;

    const REGION: &str = "us-east-1";
    const BUCKET: &str = "openlore-indexer-index-091153021562";
    const INDEX_KEY: &str = "index/index.duckdb";
    const LAST_PASS_KEY: &str = "index/last-pass.json";
    const DID_PARAMETER: &str = "/openlore/prod/indexer/repo-dids";
    const PROBE_BASE_URL: &str = "https://d111111abcdef8.cloudfront.net";

    /// What Lambda and the tofu root hand each function, DESIGN's figures included.
    fn production_environment(mode: Mode) -> BTreeMap<String, String> {
        let shared = [
            (REGION_VAR, REGION),
            ("AWS_LAMBDA_FUNCTION_NAME", "openlore-indexer"),
            ("HOME", "/tmp"),
            (BUCKET_VAR, BUCKET),
            (INDEX_KEY_VAR, INDEX_KEY),
            (LAST_PASS_KEY_VAR, LAST_PASS_KEY),
        ];
        let own: &[(&str, &str)] = match mode {
            Mode::Search => &[
                ("OPENLORE_INDEXER_INDEX_PATH", "/tmp/index.duckdb"),
                ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", "48"),
                ("OPENLORE_INDEXER_DUCKDB_THREADS", "1"),
                (FRESHNESS_TTL_VAR, "60"),
            ],
            Mode::Pass => &[
                ("OPENLORE_INDEXER_INDEX_PATH", "/tmp/index.duckdb"),
                ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", "48"),
                ("OPENLORE_INDEXER_DUCKDB_THREADS", "1"),
                (REPO_DIDS_FILE_VAR, "/tmp/config/repo-dids"),
                ("OPENLORE_INDEXER_PURGE_UNLISTED", "1"),
                ("OPENLORE_INDEXER_PASS_DEADLINE_SECS", "780"),
                ("OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES", "4"),
                ("OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS", "30"),
                (DID_PARAMETER_VAR, DID_PARAMETER),
            ],
            Mode::Probe => &[
                (PROBE_BASE_URL_VAR, PROBE_BASE_URL),
                (LAST_PASS_STALE_VAR, "2700"),
            ],
        };
        shared
            .iter()
            .chain(own)
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    /// The one thing wrong with an environment, if anything.
    #[derive(Debug, Clone)]
    enum Fault {
        None,
        Unset(String),
        Blank(String),
        NotSeconds(String),
        Unknown(String),
    }

    /// The settings a mode cannot start without.
    fn required_of(mode: Mode) -> Vec<String> {
        let shared = [REGION_VAR, BUCKET_VAR, INDEX_KEY_VAR, LAST_PASS_KEY_VAR];
        let own: &[&str] = match mode {
            Mode::Search => &[FRESHNESS_TTL_VAR],
            Mode::Pass => &[DID_PARAMETER_VAR, REPO_DIDS_FILE_VAR],
            Mode::Probe => &[PROBE_BASE_URL_VAR, LAST_PASS_STALE_VAR],
        };
        shared.iter().chain(own).map(ToString::to_string).collect()
    }

    fn lengths_of_time(mode: Mode) -> Vec<String> {
        match mode {
            Mode::Search => vec![FRESHNESS_TTL_VAR.to_string()],
            Mode::Pass => vec![],
            Mode::Probe => vec![LAST_PASS_STALE_VAR.to_string()],
        }
    }

    fn any_mode() -> impl Strategy<Value = Mode> {
        prop_oneof![Just(Mode::Search), Just(Mode::Pass), Just(Mode::Probe)]
    }

    fn any_fault(mode: Mode) -> impl Strategy<Value = Fault> {
        let required = prop::sample::select(required_of(mode));
        let unknown = "[A-Z]{1,8}".prop_map(|name| format!("{LAMBDA_VAR_PREFIX}TYPO_{name}"));
        let timed = lengths_of_time(mode);
        let not_seconds = prop::sample::select(if timed.is_empty() {
            required_of(mode)
        } else {
            timed
        });
        prop_oneof![
            Just(Fault::None),
            required.clone().prop_map(Fault::Unset),
            required.prop_map(Fault::Blank),
            not_seconds.prop_map(Fault::NotSeconds),
            unknown.prop_map(Fault::Unknown),
        ]
    }

    /// `mode`'s environment with `fault` in it, and the variable that must then
    /// be named. `NotSeconds` on a setting that is no length of time is no fault.
    fn with_fault(mode: Mode, fault: &Fault) -> (BTreeMap<String, String>, Option<String>) {
        let mut env = production_environment(mode);
        let named = match fault {
            Fault::None => None,
            Fault::Unset(variable) => {
                env.remove(variable);
                Some(variable.clone())
            }
            Fault::Blank(variable) => {
                env.insert(variable.clone(), "  ".to_string());
                Some(variable.clone())
            }
            Fault::NotSeconds(variable) => {
                let timed = lengths_of_time(mode).contains(variable);
                timed.then(|| {
                    env.insert(variable.clone(), "0".to_string());
                    variable.clone()
                })
            }
            Fault::Unknown(variable) => {
                env.insert(variable.clone(), "1".to_string());
                Some(variable.clone())
            }
        };
        (env, named)
    }

    fn object_store(endpoint_url: Option<String>) -> S3Location {
        S3Location {
            region: REGION.to_string(),
            endpoint_url,
            bucket: BUCKET.to_string(),
            index_key: INDEX_KEY.to_string(),
            last_pass_key: LAST_PASS_KEY.to_string(),
        }
    }

    fn settings_of(mode: Mode, endpoint_url: Option<String>) -> ModeSettings {
        match mode {
            Mode::Search => ModeSettings::Search(SearchSettings {
                freshness_ttl: Duration::from_secs(60),
            }),
            Mode::Pass => ModeSettings::Pass {
                did_list: SsmLocation {
                    region: REGION.to_string(),
                    endpoint_url,
                    parameter: DID_PARAMETER.to_string(),
                },
            },
            Mode::Probe => ModeSettings::Probe(ProbeSettings {
                base_url: PROBE_BASE_URL.to_string(),
                last_pass_stale_after: Duration::from_secs(2700),
            }),
        }
    }

    fn release_parse(
        env: BTreeMap<String, String>,
        mode: Mode,
    ) -> Result<LambdaConfig, ConfigRefused> {
        parse_lambda_config(&Environment(env), mode, BuildProfile::Release)
    }

    proptest! {
        // The whole config, compared whole, so no setting can move on its own;
        // and each kind of wrong setting, named. No bucket, region or key has a
        // default to fall back on.
        #[test]
        fn the_lambda_environment_parses_or_names_what_is_wrong(
            (mode, fault) in any_mode().prop_flat_map(|mode| (Just(mode), any_fault(mode))),
            endpoint_url in prop::option::of("http://[a-z]{1,8}:5000"),
        ) {
            let (mut env, named) = with_fault(mode, &fault);
            if let Some(url) = &endpoint_url {
                env.insert(ENDPOINT_URL_VAR.to_string(), url.clone());
            }

            let parsed = release_parse(env, mode);

            match named {
                Some(variable) => {
                    let refused = parsed.expect_err("a setting is wrong");
                    prop_assert_eq!(refused.variable, variable);
                }
                None => prop_assert_eq!(parsed.expect("the production environment"), LambdaConfig {
                    object_store: object_store(endpoint_url.clone()),
                    settings: settings_of(mode, endpoint_url),
                }),
            }
        }

        // A release build takes neither seam in any mode, the probe's included,
        // though only search and the pass reach the store it guards.
        #[test]
        fn a_release_build_refuses_the_loopback_seam_by_name_in_every_mode(mode in any_mode()) {
            let seam = "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP";
            let mut env = production_environment(mode);
            env.insert(seam.to_string(), "1".to_string());

            let development =
                parse_lambda_config(&Environment(env.clone()), mode, BuildProfile::Development);
            let refused = release_parse(env, mode).expect_err("a release build refuses the seam");

            prop_assert_eq!(refused.variable, seam);
            prop_assert!(development.is_ok());
        }

        // Exactly one word, spelled exactly: anything else starts nothing.
        #[test]
        fn only_a_mode_by_its_exact_name_starts_a_function(
            arguments in prop::collection::vec("[a-z]{0,6}|search|pass|probe|Search", 0..4),
        ) {
            let expected = match arguments.as_slice() {
                [word] if word == "search" => Some(Mode::Search),
                [word] if word == "pass" => Some(Mode::Pass),
                [word] if word == "probe" => Some(Mode::Probe),
                _ => None,
            };

            prop_assert_eq!(Mode::of_command(&arguments).ok(), expected);
        }
    }

    // bypass: three fixed modes and one table; `the_lambda_environment_parses...`
    // covers the settings around it.
    #[test]
    fn search_fetches_on_its_own_budget_and_the_others_on_a_passes() {
        assert_eq!(Mode::Search.fetch_call(), StoreCall::SearchInitGet);
        assert_eq!(Mode::Pass.fetch_call(), StoreCall::PassGet);
        assert_eq!(Mode::Probe.fetch_call(), StoreCall::PassGet);
    }

    // bypass: one fixed mapping into the startup line's fields.
    #[test]
    fn a_refused_setting_names_its_variable_in_the_startup_line() {
        let refused: ProbeRefused = ConfigRefused::missing(BUCKET_VAR).into();

        assert_eq!(refused.reason, ProbeRefusalReason::IndexerConfigInvalid);
        assert!(refused.detail.contains(BUCKET_VAR), "{}", refused.detail);
        assert_eq!(refused.structured["variable"], BUCKET_VAR);
    }
}
