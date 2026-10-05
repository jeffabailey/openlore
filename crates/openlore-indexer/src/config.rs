//! `config` — the indexer's configuration, parsed purely from a variable
//! lookup (the env-var seams now; `config.toml` later).
//!
//! `OPENLORE_INDEXER_SOURCE_URL` keeps its name but is the optional FALLBACK
//! listing source (ADR-077): a DID whose document cannot be resolved is read
//! there, always as relay origin. The fallback is built only here.

use std::path::PathBuf;

use appview_domain::FallbackUrl;
use claim_domain::Did;

/// The production PLC directory (ADR-026 §"Config + default").
const DEFAULT_PLC_ENDPOINT: &str = "https://plc.directory";

/// The ephemeral localhost listen address (the parallel-safe default).
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:0";

/// The indexer's resolved configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerConfig {
    /// The SEPARATE `index.duckdb` path (ADR-023; never the user's openlore.duckdb).
    pub index_path: PathBuf,
    /// Where unresolvable DIDs are listed from; `None` = no fallback.
    pub fallback: Option<FallbackUrl>,
    /// The HTTP/XRPC query surface listen address (ADR-027).
    pub listen_addr: String,
    /// The repo DIDs one ingest pass enumerates (DWD-9).
    pub repo_dids: Vec<Did>,
    /// The PLC directory each repo DID's document is resolved from.
    pub plc_endpoint: String,
}

/// Parse the configuration from `lookup` (a variable name → its value).
pub fn parse_config(lookup: impl Fn(&str) -> Option<String>) -> IndexerConfig {
    IndexerConfig {
        index_path: lookup("OPENLORE_INDEXER_INDEX_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_index_path(lookup("OPENLORE_HOME"))),
        fallback: lookup("OPENLORE_INDEXER_SOURCE_URL").and_then(|url| FallbackUrl::new(&url)),
        listen_addr: lookup("OPENLORE_INDEXER_LISTEN_ADDR")
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string()),
        repo_dids: parse_repo_dids(&lookup("OPENLORE_INDEXER_REPO_DIDS").unwrap_or_default()),
        plc_endpoint: lookup("OPENLORE_INDEXER_PLC_ENDPOINT")
            .unwrap_or_else(|| DEFAULT_PLC_ENDPOINT.to_string()),
    }
}

/// The repo DIDs named in a comma- or whitespace-separated list.
fn parse_repo_dids(list: &str) -> Vec<Did> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|did| !did.is_empty())
        .map(|did| Did(did.to_string()))
        .collect()
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

    fn parse(env: &BTreeMap<&str, String>) -> IndexerConfig {
        parse_config(|name| env.get(name).cloned())
    }

    proptest! {
        /// Universe {repo_dids, fallback}: the DIDs keep their listed order
        /// whatever separators surround them, and a blank fallback is no
        /// fallback while a set one loses only its trailing slashes.
        #[test]
        fn repo_dids_and_fallback_are_read_as_configured(
            dids in proptest::collection::vec("did:plc:[a-z0-9]{1,12}", 0..6),
            separator in prop_oneof![Just(","), Just(" "), Just(", "), Just("\n")],
            fallback in proptest::option::of("https://[a-z]{1,10}\\.[a-z]{2,4}"),
            slashes in 0usize..3,
        ) {
            let mut env = BTreeMap::new();
            env.insert("OPENLORE_INDEXER_REPO_DIDS", dids.join(separator));
            if let Some(url) = &fallback {
                env.insert("OPENLORE_INDEXER_SOURCE_URL", format!("{url}{}", "/".repeat(slashes)));
            }
            let config = parse(&env);
            let parsed: Vec<String> = config.repo_dids.iter().map(|d| d.0.clone()).collect();
            prop_assert_eq!(parsed, dids);
            prop_assert_eq!(
                config.fallback.map(|f| f.as_str().to_string()),
                fallback
            );
        }
    }

    // bypass: a single absent-variable example pins the documented defaults.
    #[test]
    fn unset_variables_take_the_documented_defaults() {
        let config = parse(&BTreeMap::new());
        assert_eq!(config.fallback, None);
        assert_eq!(config.plc_endpoint, DEFAULT_PLC_ENDPOINT);
        assert_eq!(config.listen_addr, DEFAULT_LISTEN_ADDR);
        assert!(config.repo_dids.is_empty());
    }
}
