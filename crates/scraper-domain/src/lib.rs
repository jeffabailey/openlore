//! `scraper-domain` — the PURE slice-02 candidate-derivation core (WD-56).
//!
//! Derives auditable [`CandidateClaim`](ports::CandidateClaim)s from
//! already-harvested public GitHub [`Signal`](ports::Signal)s via the
//! `jobs.yaml :: J-004.signal_predicate_mapping` SSOT. PURE: values in, values
//! out; NO I/O (no tokio/reqwest/duckdb/keyring/std::fs/net). The effect shell
//! (`adapter-github`, `cli`) supplies the signals and carries candidates into
//! the slice-01 sign pipeline at the human's gesture (I-SCR-1).
//!
//! ## Public surface
//!
//! - [`detect_signals`] + [`RepoFacts`] + [`MEMORY_SAFE_LANGUAGES`] — the pure
//!   signal detection over real repo facts (RGSD-1; sibling of the derivation).
//! - [`derive_candidates`] — the pure derivation (J-004b load-bearing surface).
//! - [`load_mapping`] + [`SignalPredicateMapping`] — parse the embedded SSOT.
//! - [`EMBEDDED_MAPPING_YAML`] — the compile-time SSOT snapshot.
//! - [`MappingError`] / [`MappingEntry`] / [`EMBODIES_PHILOSOPHY`].
//!
//! The `Signal` / `CandidateClaim` / `SignalKind` value types live in `ports`
//! (Q-DELIVER-3); this crate consumes and produces them.

#![forbid(unsafe_code)]

mod derive;
mod detect;
mod mapping;

#[cfg(test)]
mod proptest_strategies;

pub use derive::derive_candidates;
pub use detect::{detect_signals, RepoFacts, MEMORY_SAFE_LANGUAGES};
pub use mapping::{
    load_mapping, MappingEntry, MappingError, SignalPredicateMapping, EMBEDDED_MAPPING_YAML,
    EMBODIES_PHILOSOPHY,
};

// Re-export the shared value types so consumers can reach the whole derivation
// vocabulary through `scraper_domain::` without also importing `ports::`.
pub use ports::{CandidateClaim, Signal, SignalKind};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proptest_strategies::{arb_distinct_signals, arb_signal};
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    const SUBJECT: &str = "github:rust-lang/cargo";

    fn ssot_mapping() -> SignalPredicateMapping {
        load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse")
    }

    // -------------------------------------------------------------------------
    // mapping_matches_ssot (WD-67 / I-SCR-5) — no divergent hardcode.
    //
    // The embedded snapshot must carry the SAME signal/predicate/confidence
    // values as `docs/product/jobs.yaml :: J-004.signal_predicate_mapping`.
    // Tests are the effect shell (filesystem read is fine here); the crate
    // itself never reads the file. We extract the SSOT block from jobs.yaml,
    // dedent it, and assert byte-equality with the embedded snapshot — so any
    // value drift fails the build.
    // -------------------------------------------------------------------------

    /// Extract the `signal_predicate_mapping:` block from the full jobs.yaml
    /// text and dedent it to column 0, stripping inline `#` comments — yielding
    /// the canonical form the embedded snapshot is stored in.
    fn extract_ssot_block(jobs_yaml: &str) -> String {
        let mut out = String::new();
        let mut in_block = false;
        let mut base_indent = 0usize;
        for line in jobs_yaml.lines() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim_start();
            if !in_block {
                if trimmed.starts_with("signal_predicate_mapping:") {
                    in_block = true;
                    base_indent = indent;
                    out.push_str("signal_predicate_mapping:\n");
                }
                continue;
            }
            // A blank line or a line dedented to/under the block key ends it.
            if trimmed.is_empty() || indent <= base_indent {
                break;
            }
            let dedented = &line[base_indent..];
            // Strip a trailing inline comment (the SSOT's first entry has one).
            let no_comment = match dedented.find(" #") {
                Some(pos) => dedented[..pos].trim_end(),
                None => dedented.trim_end(),
            };
            out.push_str(no_comment);
            out.push('\n');
        }
        out
    }

    #[test]
    fn mapping_matches_ssot() {
        let jobs_yaml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/product/jobs.yaml");
        let jobs_yaml = std::fs::read_to_string(jobs_yaml_path)
            .unwrap_or_else(|e| panic!("read SSOT jobs.yaml at {jobs_yaml_path}: {e}"));

        let ssot_block = extract_ssot_block(&jobs_yaml);
        assert!(
            ssot_block.contains("org.openlore.philosophy.dependency-pinning"),
            "extractor must capture the SSOT mapping block; got:\n{ssot_block}"
        );

        assert_eq!(
            ssot_block, EMBEDDED_MAPPING_YAML,
            "embedded snapshot diverged from jobs.yaml SSOT (WD-67 / I-SCR-5): \
             regenerate crates/scraper-domain/src/signal_predicate_mapping.yaml \
             from the J-004.signal_predicate_mapping block"
        );

        // And the parsed-shape equivalence: both parse to the same mapping.
        let from_embedded = load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded parses");
        let from_ssot = load_mapping(&ssot_block).expect("SSOT block parses");
        assert_eq!(
            from_embedded, from_ssot,
            "embedded and SSOT mappings must parse to identical typed values"
        );
    }

    #[test]
    fn load_mapping_parses_all_five_ssot_entries() {
        let mapping = ssot_mapping();
        assert_eq!(mapping.entries.len(), 5, "slice-02 SSOT has 5 entries");
        for entry in &mapping.entries {
            assert!(
                entry.object.starts_with("org.openlore.philosophy."),
                "every mapping object is a philosophy NSID, got {}",
                entry.object
            );
            assert_eq!(
                entry.default_confidence, 0.25,
                "every SSOT entry's default confidence is 0.25 (WD-52)"
            );
        }
    }

    #[test]
    fn load_mapping_rejects_unknown_signal_description() {
        let bogus = "signal_predicate_mapping:\n  - signal: \"not a real signal\"\n    \
                     predicate: org.openlore.philosophy.mystery\n    default_confidence: 0.25\n";
        let result = load_mapping(bogus);
        assert!(
            matches!(result, Err(MappingError::MalformedEntry(_))),
            "an unrecognized signal description must be a MalformedEntry, got {result:?}"
        );
    }

    // -------------------------------------------------------------------------
    // derive_candidates — PBT invariants (component-boundaries.md §Probe).
    // -------------------------------------------------------------------------

    proptest! {
        /// I-SCR-3: every derived candidate carries confidence exactly 0.25.
        #[test]
        fn every_candidate_confidence_is_025(signals in arb_distinct_signals()) {
            let mapping = ssot_mapping();
            let candidates = derive_candidates(SUBJECT, &signals, &mapping);
            for candidate in &candidates {
                prop_assert_eq!(candidate.confidence, 0.25);
            }
        }

        /// I-SCR-4: every derived candidate names at least one source signal.
        #[test]
        fn every_candidate_names_a_source_signal(signals in arb_distinct_signals()) {
            let mapping = ssot_mapping();
            let candidates = derive_candidates(SUBJECT, &signals, &mapping);
            for candidate in &candidates {
                prop_assert!(!candidate.source_signals().is_empty());
            }
        }

        /// Determinism: identical inputs yield identical output (same candidates,
        /// same order).
        #[test]
        fn derive_is_deterministic(signals in arb_distinct_signals()) {
            let mapping = ssot_mapping();
            let first = derive_candidates(SUBJECT, &signals, &mapping);
            let second = derive_candidates(SUBJECT, &signals, &mapping);
            prop_assert_eq!(first, second);
        }

        /// Collapse: at most one candidate per distinct predicate (object). Even
        /// across many signals, no predicate is proposed twice.
        #[test]
        fn at_most_one_candidate_per_predicate(
            signals in proptest::collection::vec(arb_signal(), 0..40)
        ) {
            let mapping = ssot_mapping();
            let candidates = derive_candidates(SUBJECT, &signals, &mapping);
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for candidate in &candidates {
                prop_assert!(
                    seen.insert(candidate.object.clone()),
                    "predicate {} produced more than one candidate (collapse failed)",
                    candidate.object
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // derive_candidates — example-based corner cases (US-SCR-002).
    // -------------------------------------------------------------------------

    #[test]
    fn zero_signals_yields_empty_vec() {
        // US-SCR-002 Example 2: no signals -> no candidates (not an error).
        let candidates = derive_candidates(SUBJECT, &[], &ssot_mapping());
        assert!(candidates.is_empty());
    }

    #[test]
    fn multiple_signals_for_one_predicate_collapse_into_one_candidate() {
        // US-SCR-002 Example 4: two TestRatioOrCiMatrix signals collapse into a
        // single test-driven candidate listing BOTH signals + BOTH evidence URLs.
        let s1 = Signal {
            kind: SignalKind::TestRatioOrCiMatrix,
            value: "test/source ratio 0.61".to_string(),
            source_url: "https://github.com/rust-lang/cargo/tree/master/tests".to_string(),
        };
        let s2 = Signal {
            kind: SignalKind::TestRatioOrCiMatrix,
            value: "CI runs a test matrix".to_string(),
            source_url: "https://github.com/rust-lang/cargo/blob/master/.github/workflows/ci.yml"
                .to_string(),
        };
        let candidates = derive_candidates(SUBJECT, &[s1.clone(), s2.clone()], &ssot_mapping());

        assert_eq!(
            candidates.len(),
            1,
            "two signals for one predicate -> one candidate"
        );
        let candidate = &candidates[0];
        assert_eq!(candidate.object, "org.openlore.philosophy.test-driven");
        assert_eq!(candidate.predicate, EMBODIES_PHILOSOPHY);
        assert_eq!(candidate.subject, SUBJECT);
        assert_eq!(
            candidate.source_signals().len(),
            2,
            "names BOTH contributing signals"
        );
        assert_eq!(
            candidate.evidence,
            vec![s1.source_url.clone(), s2.source_url.clone()],
            "evidence carries each contributing signal's source_url, in order"
        );
        assert_eq!(candidate.confidence, 0.25);
    }
}
