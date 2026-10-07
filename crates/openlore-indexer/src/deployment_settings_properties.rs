//! indexer-deployment CORE-8 — moved VERBATIM (property, binding) from
//! `tests/acceptance/indexer_deployment_core.rs` (roadmap review F2): the
//! settings parser is private to this BINARY crate, which the `cli` test target
//! cannot link (check-arch CLI_FORBIDDEN_INDEXER_DEPS / I-3). Precedent:
//! `config::pass_core_properties`.
//!
//! RED scaffold: DELIVER 02-03 replaces the `sut_parse_setting` body with ONE
//! call into this crate's config parsing (data-models §1).
//
// SCAFFOLD: true

use proptest::prelude::*;

fn sut_parse_setting(_variable: &str, _value: &str) -> Result<(), String> {
    todo!("DELIVER 02-03: bind to openlore-indexer config parsing of the new settings (data-models §1)")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-8 @US-IXD-006 @data-models-1 @property @C1b @C6a @contract-shape:pure-function
    /// Every new numeric setting accepts exactly its range and refuses
    /// anything else, including non-numbers.
    #[test]
    #[ignore = "DELIVER 02-03: new settings parse"]
    fn each_new_setting_accepts_exactly_its_range(
        n in -10i64..10_000,
        junk in "[a-z ]{1,6}",
    ) {
        let ranges: [(&str, i64, i64); 3] = [
            ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", 16, 1024),
            ("OPENLORE_INDEXER_DUCKDB_THREADS", 1, 4),
            ("OPENLORE_INDEXER_PASS_DEADLINE_SECS", 60, 7200),
        ];
        for (variable, lo, hi) in ranges {
            prop_assert_eq!(sut_parse_setting(variable, &n.to_string()).is_ok(), (lo..=hi).contains(&n), "{}={}", variable, n);
            prop_assert!(sut_parse_setting(variable, &junk).is_err(), "{}={:?}", variable, junk);
        }
        prop_assert_eq!(sut_parse_setting("OPENLORE_INDEXER_PURGE_UNLISTED", &n.to_string()).is_ok(), n == 1);
    }
}
