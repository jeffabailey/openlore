//! The app's configuration contract (DWD-7): environment names and secrets
//! files. Parsing is PURE and TOTAL — every environment maps to a config or
//! a named error; reading the environment and the secrets directory is the
//! wiring's job.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use ports::duckdb_caps::DuckDbCap;

/// The granular scope set (ADR-073 / SPIKE-3): claim and post creation only.
pub(crate) const DEFAULT_OAUTH_SCOPES: &str =
    "atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create";

/// DuckDB's memory limit for the private store, in MiB (B11).
pub(crate) const DB_MEMORY_LIMIT_MB_VAR: &str = "REVIEW_DB_MEMORY_LIMIT_MB";
/// DuckDB's worker threads for the private store (B11).
pub(crate) const DB_THREADS_VAR: &str = "REVIEW_DB_THREADS";
/// The admissible ranges, as literals: the property tests' oracle for the
/// shared [`DuckDbCap::range`].
#[cfg(test)]
const DB_MEMORY_LIMIT_MB_RANGE: RangeInclusive<u64> = 16..=1024;
#[cfg(test)]
const DB_THREADS_RANGE: RangeInclusive<u64> = 1..=4;
/// The production sizes (architecture-design §3): 48 MiB, one thread.
const DEFAULT_DB_MEMORY_LIMIT_MB: u64 = 48;
const DEFAULT_DB_THREADS: u64 = 1;

/// Which kind of binary is running. Only a development build may accept
/// plain-HTTP loopback origins and upstreams (the test seam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildProfile {
    Release,
    Development,
}

impl BuildProfile {
    pub(crate) fn of_this_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Development
        } else {
            Self::Release
        }
    }
}

/// Which URLs the app accepts for itself and its upstreams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportPolicy {
    HttpsOnly,
    HttpsOrLoopbackHttp,
}

/// A configuration the app refuses to start with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigError {
    Missing(&'static str),
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
    /// A number outside its range (or not a number), with the value given.
    OutOfRange {
        name: &'static str,
        value: String,
        range: RangeInclusive<u64>,
    },
    /// `REVIEW_APP_ALLOW_LOOPBACK_HTTP` in a release build.
    LoopbackHttpInRelease,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "{name} is not set"),
            Self::Invalid { name, reason } => write!(f, "{name} is invalid: {reason}"),
            Self::OutOfRange { name, value, range } => write!(
                f,
                "{name} is invalid: {value:?} must be a whole number from {} to {}",
                range.start(),
                range.end()
            ),
            Self::LoopbackHttpInRelease => write!(
                f,
                "REVIEW_APP_ALLOW_LOOPBACK_HTTP is a test seam; a release build refuses it"
            ),
        }
    }
}

/// The validated environment configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppConfig {
    /// The public origin, without a trailing slash (`https://app.…`).
    pub(crate) origin: String,
    pub(crate) listen: SocketAddr,
    /// The operator listener; always loopback.
    pub(crate) admin_listen: SocketAddr,
    pub(crate) review_db: PathBuf,
    /// DuckDB `memory_limit` for the private store, in MiB (B11).
    pub(crate) db_memory_limit_mb: u64,
    /// DuckDB `threads` for the private store (B11).
    pub(crate) db_threads: u64,
    pub(crate) secrets_dir: PathBuf,
    pub(crate) oauth_scopes: String,
    pub(crate) github_api_base: String,
    /// PLC directory base (identity resolution lands with sign-in, 01-02).
    #[allow(dead_code)]
    pub(crate) plc_url: String,
    /// `resolveHandle` base (identity resolution lands with sign-in, 01-02).
    #[allow(dead_code)]
    pub(crate) handle_resolver_url: String,
}

/// Parse the environment (pure, total).
pub(crate) fn parse_config(
    env: &BTreeMap<String, String>,
    profile: BuildProfile,
) -> Result<AppConfig, ConfigError> {
    let get = |name: &str| env.get(name).map(|v| v.trim()).filter(|v| !v.is_empty());
    let policy = match (get("REVIEW_APP_ALLOW_LOOPBACK_HTTP") == Some("1"), profile) {
        (false, _) => TransportPolicy::HttpsOnly,
        (true, BuildProfile::Development) => TransportPolicy::HttpsOrLoopbackHttp,
        (true, BuildProfile::Release) => return Err(ConfigError::LoopbackHttpInRelease),
    };
    if !matches!(get("LOG_FORMAT"), None | Some("json")) {
        return Err(ConfigError::Invalid {
            name: "LOG_FORMAT",
            reason: "only json is supported",
        });
    }
    let url = |name: &'static str, default: &str| {
        checked_base_url(name, get(name).unwrap_or(default), policy)
    };
    let origin = get("APP_ORIGIN").ok_or(ConfigError::Missing("APP_ORIGIN"))?;
    let admin_listen = socket_addr(
        "ADMIN_LISTEN_ADDR",
        get("ADMIN_LISTEN_ADDR").unwrap_or("127.0.0.1:8081"),
    )?;
    if !admin_listen.ip().is_loopback() {
        return Err(ConfigError::Invalid {
            name: "ADMIN_LISTEN_ADDR",
            reason: "the operator listener must be loopback",
        });
    }
    Ok(AppConfig {
        origin: checked_origin(origin, policy)?,
        listen: socket_addr("LISTEN_ADDR", get("LISTEN_ADDR").unwrap_or("0.0.0.0:8080"))?,
        admin_listen,
        review_db: PathBuf::from(get("REVIEW_DB").unwrap_or("/data/review-app.duckdb")),
        db_memory_limit_mb: db_cap(
            DB_MEMORY_LIMIT_MB_VAR,
            get(DB_MEMORY_LIMIT_MB_VAR),
            DuckDbCap::MemoryLimitMb,
            DEFAULT_DB_MEMORY_LIMIT_MB,
        )?,
        db_threads: db_cap(
            DB_THREADS_VAR,
            get(DB_THREADS_VAR),
            DuckDbCap::Threads,
            DEFAULT_DB_THREADS,
        )?,
        secrets_dir: PathBuf::from(get("SECRETS_DIR").unwrap_or("/run/secrets")),
        oauth_scopes: get("OAUTH_SCOPES")
            .unwrap_or(DEFAULT_OAUTH_SCOPES)
            .to_string(),
        github_api_base: url("OPENLORE_GITHUB_API_BASE", "https://api.github.com")?,
        plc_url: url("REVIEW_APP_PLC_URL", "https://plc.directory")?,
        handle_resolver_url: url("REVIEW_APP_HANDLE_RESOLVER_URL", "https://bsky.social")?,
    })
}

/// A DuckDB cap within its shared range, or `default` when unset (pure, total).
fn db_cap(
    name: &'static str,
    value: Option<&str>,
    cap: DuckDbCap,
    default: u64,
) -> Result<u64, ConfigError> {
    value.map_or(Ok(default), |value| {
        cap.parse(value).map_err(|refused| ConfigError::OutOfRange {
            name,
            value: value.to_string(),
            range: refused.range,
        })
    })
}

fn socket_addr(name: &'static str, value: &str) -> Result<SocketAddr, ConfigError> {
    value.parse().map_err(|_| ConfigError::Invalid {
        name,
        reason: "not a host:port socket address",
    })
}

/// A base URL the policy admits, without a trailing slash.
fn checked_base_url(
    name: &'static str,
    value: &str,
    policy: TransportPolicy,
) -> Result<String, ConfigError> {
    let invalid = |reason| ConfigError::Invalid { name, reason };
    let parsed = url::Url::parse(value).map_err(|_| invalid("not a URL"))?;
    let loopback = matches!(
        parsed.host(),
        Some(url::Host::Ipv4(ip)) if ip.is_loopback()
    ) || matches!(parsed.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback())
        || parsed.host_str() == Some("localhost");
    match (parsed.scheme(), policy, loopback) {
        ("https", _, _) if parsed.host().is_some() => {}
        ("http", TransportPolicy::HttpsOrLoopbackHttp, true) => {}
        ("http", _, _) => return Err(invalid("must be https")),
        _ => return Err(invalid("must be an https URL")),
    }
    Ok(value.trim_end_matches('/').to_string())
}

/// The public origin: a base URL with no path, query or fragment.
fn checked_origin(value: &str, policy: TransportPolicy) -> Result<String, ConfigError> {
    let origin = checked_base_url("APP_ORIGIN", value, policy)?;
    let parsed = url::Url::parse(&origin).map_err(|_| ConfigError::Invalid {
        name: "APP_ORIGIN",
        reason: "not a URL",
    })?;
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ConfigError::Invalid {
            name: "APP_ORIGIN",
            reason: "must be an origin (scheme://host[:port]) with no path",
        });
    }
    Ok(origin)
}

/// The server GitHub token. Its bytes leave only as a request header.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GithubToken(String);

impl GithubToken {
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for GithubToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GithubToken(<redacted>)")
    }
}

/// The secrets files as read from `SECRETS_DIR` (absent = `None`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RawSecrets {
    pub(crate) client_jwk: Option<String>,
    pub(crate) data_key: Option<String>,
    pub(crate) github_token: Option<String>,
    pub(crate) log_salt: Option<String>,
}

/// The validated secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Secrets {
    pub(crate) client_jwk: String,
    pub(crate) data_key: DataKeyMaterial,
    pub(crate) github_token: GithubToken,
}

/// Validate the secrets (pure, total). Every secret is required.
pub(crate) fn parse_secrets(raw: RawSecrets) -> Result<Secrets, ConfigError> {
    let required = |value: Option<String>, name: &'static str| {
        value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or(ConfigError::Missing(name))
    };
    let client_jwk = required(raw.client_jwk, "client-jwk")?;
    let data_key = required(raw.data_key, "data-key")?;
    let github_token = required(raw.github_token, "github-token")?;
    required(raw.log_salt, "log-salt")?;
    Ok(Secrets {
        client_jwk,
        data_key: parse_data_key("data-key", &data_key)?,
        github_token: GithubToken(github_token),
    })
}

/// A data key as configured: its key id and its 32 bytes (never printed).
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DataKeyMaterial {
    pub(crate) kid: Option<String>,
    pub(crate) key: [u8; 32],
}

impl std::fmt::Debug for DataKeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DataKeyMaterial({:?}, <redacted>)", self.kid)
    }
}

const DATA_KEY_SHAPE: &str =
    "must be JSON {\"kid\":\"<id>\",\"key\":\"<base64 of 32 bytes>\"} (or legacy 64 hex characters)";

/// Parse the `data-key` / `data-key-previous` secret (pure): the documented
/// `{"kid","key"}` JSON (infrastructure-integration §5), or the legacy bare
/// 64-hex key, which has no kid. Anything else refuses startup.
pub(crate) fn parse_data_key(
    name: &'static str,
    text: &str,
) -> Result<DataKeyMaterial, ConfigError> {
    let invalid = ConfigError::Invalid {
        name,
        reason: DATA_KEY_SHAPE,
    };
    let text = text.trim();
    if !text.starts_with('{') {
        return parse_hex_key(text)
            .map(|key| DataKeyMaterial { kid: None, key })
            .ok_or(invalid);
    }
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| invalid.clone())?;
    let kid = value["kid"].as_str().filter(|kid| {
        !kid.is_empty() && kid.len() <= 32 && kid.bytes().all(|b| b.is_ascii_graphic())
    });
    let key = value["key"].as_str().and_then(|b64| {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
    });
    match (kid, key) {
        (Some(kid), Some(key)) => Ok(DataKeyMaterial {
            kid: Some(kid.to_string()),
            key,
        }),
        _ => Err(invalid),
    }
}

fn parse_hex_key(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let mut key = [0u8; 32];
    for (byte, pair) in key.iter_mut().zip(hex.as_bytes().chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const NAMES: [&str; 12] = [
        "APP_ORIGIN",
        "LISTEN_ADDR",
        "ADMIN_LISTEN_ADDR",
        "REVIEW_DB",
        "SECRETS_DIR",
        "LOG_FORMAT",
        "OAUTH_SCOPES",
        "OPENLORE_GITHUB_API_BASE",
        "REVIEW_APP_PLC_URL",
        "REVIEW_APP_HANDLE_RESOLVER_URL",
        "REVIEW_APP_ALLOW_LOOPBACK_HTTP",
        "UNRELATED",
    ];

    fn value() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("1".to_string()),
            Just("json".to_string()),
            Just("https://app.example".to_string()),
            Just("http://127.0.0.1:8080".to_string()),
            Just("http://evil.example".to_string()),
            Just("127.0.0.1:9000".to_string()),
            Just("0.0.0.0:9000".to_string()),
            ".{0,24}",
        ]
    }

    fn env() -> impl Strategy<Value = BTreeMap<String, String>> {
        proptest::collection::btree_map(
            proptest::sample::select(NAMES.to_vec()).prop_map(str::to_string),
            value(),
            0..NAMES.len(),
        )
    }

    fn profile() -> impl Strategy<Value = BuildProfile> {
        prop_oneof![Just(BuildProfile::Release), Just(BuildProfile::Development)]
    }

    proptest! {
        /// Universe: every DB-cap variable x {unset, any number near or far
        /// from its range, noise}. Unset yields the production default; a set
        /// value is accepted iff it is a whole number in range (and yields
        /// that number); anything else is refused naming the variable and
        /// echoing the value.
        #[test]
        fn a_db_cap_is_accepted_exactly_within_its_range(
            cap in prop_oneof![
                Just((DB_MEMORY_LIMIT_MB_VAR, DB_MEMORY_LIMIT_MB_RANGE, DEFAULT_DB_MEMORY_LIMIT_MB)),
                Just((DB_THREADS_VAR, DB_THREADS_RANGE, DEFAULT_DB_THREADS)),
            ],
            value in proptest::option::of(prop_oneof![
                (0u64..1100).prop_map(|n| n.to_string()),
                any::<u64>().prop_map(|n| n.to_string()),
                "[a-z -]{0,8}",
            ]),
        ) {
            let (name, range, default) = cap;
            let mut env = BTreeMap::from([("APP_ORIGIN".to_string(), "https://app.example".to_string())]);
            if let Some(value) = &value {
                env.insert(name.to_string(), value.clone());
            }
            let parsed = parse_config(&env, BuildProfile::Release)
                .map(|c| if name == DB_THREADS_VAR { c.db_threads } else { c.db_memory_limit_mb });
            let trimmed = value.as_deref().map(str::trim).filter(|v| !v.is_empty());
            match trimmed {
                None => prop_assert_eq!(parsed, Ok(default)),
                Some(v) => match v.parse::<u64>().ok().filter(|n| range.contains(n)) {
                    Some(number) => prop_assert_eq!(parsed, Ok(number)),
                    None => prop_assert_eq!(
                        parsed,
                        Err(ConfigError::OutOfRange { name, value: v.to_string(), range: range.clone() })
                    ),
                },
            }
        }

        /// Universe: data-key texts — the documented JSON with any kid and
        /// key length, legacy hex, and noise. Exactly a printable kid with a
        /// 32-byte base64 key (or 64 hex) is accepted, with its kid.
        #[test]
        fn a_data_key_parses_only_in_its_documented_shape(
            kid in "[ -~]{0,40}", len in 0usize..48, byte in any::<u8>(), noise in ".{0,40}"
        ) {
            use base64::Engine as _;
            let key = base64::engine::general_purpose::STANDARD.encode(vec![byte; len]);
            let json = serde_json::json!({"kid": kid, "key": key}).to_string();
            let good_kid = !kid.is_empty() && kid.len() <= 32 && kid.bytes().all(|b| b.is_ascii_graphic());
            let parsed = parse_data_key("data-key", &json);
            prop_assert_eq!(parsed.is_ok(), good_kid && len == 32);
            if let Ok(material) = parsed {
                prop_assert_eq!(material.kid, Some(kid));
                prop_assert_eq!(material.key, [byte; 32]);
            }
            let hex: String = std::iter::repeat_n(format!("{byte:02x}"), 32).collect();
            prop_assert_eq!(parse_data_key("data-key", &hex).map(|m| (m.kid, m.key)), Ok((None, [byte; 32])));
            prop_assume!(!noise.trim().starts_with('{') && noise.trim().len() != 64);
            prop_assert!(parse_data_key("data-key", &noise).is_err());
        }

        /// Totality + transport invariant over the declared universe of
        /// environments: parsing never panics, and every accepted config
        /// speaks HTTPS unless a development build opted into loopback HTTP,
        /// in which case any plain-HTTP URL is loopback.
        #[test]
        fn every_accepted_config_is_https_or_opted_in_loopback(env in env(), profile in profile()) {
            let opted_in = env.get("REVIEW_APP_ALLOW_LOOPBACK_HTTP").map(|v| v.trim()) == Some("1");
            if let Ok(config) = parse_config(&env, profile) {
                prop_assert!(config.admin_listen.ip().is_loopback());
                for url in [&config.origin, &config.github_api_base, &config.plc_url, &config.handle_resolver_url] {
                    let https = url.starts_with("https://");
                    let loopback_http = url.starts_with("http://127.") || url.starts_with("http://localhost");
                    prop_assert!(https || (opted_in && profile == BuildProfile::Development && loopback_http), "{url}");
                }
            }
        }

        /// A release build refuses the loopback test seam whatever else is set.
        #[test]
        fn a_release_build_refuses_the_loopback_seam(mut env in env()) {
            env.insert("REVIEW_APP_ALLOW_LOOPBACK_HTTP".into(), "1".into());
            prop_assert_eq!(parse_config(&env, BuildProfile::Release), Err(ConfigError::LoopbackHttpInRelease));
        }

        /// Universe: each secret present or absent. Secrets parse iff every
        /// secret is present (with a well-formed data key); otherwise the
        /// first missing one is named.
        #[test]
        fn secrets_parse_iff_all_are_present(
            jwk in any::<bool>(), key in any::<bool>(), token in any::<bool>(), salt in any::<bool>()
        ) {
            let raw = RawSecrets {
                client_jwk: jwk.then(|| "{}".to_string()),
                data_key: key.then(|| "ab".repeat(32)),
                github_token: token.then(|| "ghp_x".to_string()),
                log_salt: salt.then(|| "salt".to_string()),
            };
            let expected = [(jwk, "client-jwk"), (key, "data-key"), (token, "github-token"), (salt, "log-salt")]
                .into_iter()
                .find(|(present, _)| !present)
                .map(|(_, name)| ConfigError::Missing(name));
            prop_assert_eq!(parse_secrets(raw).err(), expected);
        }
    }
}
