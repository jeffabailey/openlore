//! Slice-02 layer-2 acceptance — `scraper-domain` pure derivation
//! properties + signal->predicate mapping SSOT conformance.
//!
//! Layer 2 (in-memory acceptance — pure-core direct invocation, NO CLI
//! subprocess) per nw-tdd-methodology Layered Test Discipline matrix +
//! DD-SCR-6. Sibling to slice-03's `lexicon_counter_claim.rs`; same shape,
//! same file role. The driving port here is the PURE function signature
//! (`scraper_domain::derive_candidates` / `load_mapping`) — calling it
//! directly IS port-to-port testing at the domain layer.
//!
//! Per Mandate 9 (layer-dependent PBT mode): layers 1-2 may use PBT full.
//! The auditability invariant (every candidate names a source signal), the
//! no-auto-inflation invariant (every candidate confidence == 0.25), and
//! the determinism invariant are `@property` scenarios runnable via
//! proptest. The mapping-SSOT conformance + the collapse + the
//! empty-on-no-match scenarios are example-pinned (single fixture each).
//!
//! These are the LOAD-BEARING auditability + human-gate properties the
//! whole feature thesis rests on (KPI-SCR-2 + KPI-SCR-3); pinning them as
//! generative properties at the cheap layer-2 boundary means the
//! example-only layer-3 subprocess tests (`scrape_candidates.rs`) only need
//! to verify the user-visible RENDERING of these already-proven invariants.
//!
//! The EXHAUSTIVE per-arm unit coverage (each `SignalKind` -> predicate,
//! malformed-mapping errors, boundary parsing) is DELIVER's inner TDD loop
//! in `crates/scraper-domain/src/`'s `#[cfg(test)] mod tests` block (out of
//! DISTILL scope per DD-SCR-7, symmetric with slice-03 DD-FED-7).
//!
//! Covers:
//! - US-SCR-002: pure candidate derivation (auditability + no-inflation +
//!   collapse + empty-on-no-match)
//! - US-SCR-006: signal->predicate mapping SSOT conformance
//! - WD-52 / I-SCR-3: confidence 0.25, never above 0.3 (property)
//! - WD-53 / I-SCR-4: every candidate names its source signal (property)
//! - WD-53 / WD-67 / I-SCR-5: mapping embedded from jobs.yaml SSOT,
//!   `mapping_matches_ssot` (no divergent hardcode)
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_imports)]

// NOTE — unlike the subprocess-driven scrape_* tests, this file invokes
// `scraper_domain` directly (layer 2). It does NOT use `support/mod.rs`'s
// TestEnv (no subprocess). Same pattern as slice-03's
// `lexicon_counter_claim.rs`.

// =============================================================================
// US-SCR-002 — auditability invariant (PROPERTY; KPI-SCR-3 / I-SCR-4)
// =============================================================================

/// SD-1 / Property (Mandate 9 layer 2 PBT full): EVERY candidate
/// `derive_candidates` produces has a NON-EMPTY `source_signals`. This is
/// the auditability invariant (I-SCR-4 / KPI-SCR-3): a candidate the user
/// cannot trace to a signal is unauditable and must not exist. The negative
/// is load-bearing — a candidate with zero source signals is a bug, not a
/// proposal.
///
///     forall (signals, mapping):
///         derive_candidates(signals, mapping).all(|c| !c.source_signals.is_empty())
///
/// @property @us-scr-002 @j-004b @i-scr-4 @kpi-scr-3
#[test]
fn scraper_domain_every_candidate_names_at_least_one_source_signal_property() {
    use ports::{Signal, SignalKind};
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;
    use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

    // Layer-2 @property (Mandate 9; DD-SCR): pure-core direct invocation, NO
    // CLI subprocess. The driving port IS the pure `derive_candidates`
    // signature; we drive it over an arbitrary signal set x the embedded SSOT
    // mapping and assert the auditability invariant from data-models.md
    // (I-SCR-4 / KPI-SCR-3):
    //
    //     forall (signals, mapping):
    //         derive_candidates(signals, mapping).all(|c| !c.source_signals().is_empty())
    //
    // The negative is LOAD-BEARING: a candidate with zero source signals is
    // unauditable (the user cannot trace the proposal to a public signal) and
    // must not exist. The guarantee comes from routing all construction through
    // `CandidateClaim::try_new` (step 01-02); this layer-2 property PINS that
    // contract at the derivation boundary so a future refactor of
    // `derive_candidates` that bypasses the smart constructor fails LOUDLY.
    const SUBJECT: &str = "github:rust-lang/cargo";

    // Any one of the five bounded SignalKind variants the SSOT mapping uses.
    fn arb_signal_kind() -> impl Strategy<Value = SignalKind> {
        prop_oneof![
            Just(SignalKind::DependencyManifestPinned),
            Just(SignalKind::DocsPresentAndSubstantial),
            Just(SignalKind::TestRatioOrCiMatrix),
            Just(SignalKind::SemverAndChangelog),
            Just(SignalKind::MemorySafetyLanguage),
        ]
    }

    // A single Signal with an arbitrary kind, printable value, and a
    // GitHub-shaped public URL.
    fn arb_signal() -> impl Strategy<Value = Signal> {
        (
            arb_signal_kind(),
            "[ -~]{0,64}",
            "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}",
        )
            .prop_map(|(kind, value, source_url)| Signal {
                kind,
                value,
                source_url,
            })
    }

    // Non-vacuous signal set: a guaranteed mapping-matching head signal so at
    // least one candidate is derived on every case, plus an arbitrary tail
    // (0..6 further signals) to explore collapse + drop shapes. Without the
    // forced head the property could pass vacuously on the empty-candidate
    // case; the head makes "every candidate names a signal" a real assertion.
    fn arb_signal_set() -> impl Strategy<Value = Vec<Signal>> {
        (
            arb_signal(),
            proptest::collection::vec(arb_signal(), 0..6),
        )
            .prop_map(|(head, mut tail)| {
                tail.insert(0, head);
                tail
            })
    }

    let mapping = load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse");

    let mut runner = TestRunner::default();
    runner
        .run(&arb_signal_set(), |signals| {
            let candidates = derive_candidates(SUBJECT, &signals, &mapping);
            // Non-vacuity guard: the forced mapping-matching head signal means
            // at least one candidate is always produced, so this property
            // actually exercises the per-candidate assertion below.
            prop_assert!(
                !candidates.is_empty(),
                "generator forces >=1 mapping-matching signal, so >=1 candidate is expected \
                 (else the auditability property is vacuous)"
            );
            for candidate in &candidates {
                prop_assert!(
                    !candidate.source_signals().is_empty(),
                    "every derived CandidateClaim must name >=1 source signal (I-SCR-4): \
                     an untraceable proposal is unauditable and must not exist"
                );
            }
            Ok(())
        })
        .expect(
            "auditability invariant (I-SCR-4): every derived candidate must name >=1 source \
             signal for all generated signal sets",
        );
}

// =============================================================================
// US-SCR-002 — no-auto-inflation invariant (PROPERTY; KPI-SCR-2 / I-SCR-3)
// =============================================================================

/// SD-2 / Property: EVERY candidate `derive_candidates` produces has
/// `confidence == 0.25` (the mapping default), and NONE is above 0.3. The
/// scraper has weak evidence; only the human may raise confidence (WD-52 /
/// WD-10). This is the proposal-time half of the
/// `candidate_confidence_no_autoinflate` guardrail, proven generatively.
///
///     forall (signals, mapping):
///         derive_candidates(signals, mapping).all(|c| c.confidence == 0.25)
///
/// @property @us-scr-002 @j-004b @wd-52 @i-scr-3 @kpi-scr-2
#[test]
fn scraper_domain_every_candidate_confidence_is_the_quarter_default_property() {
    use ports::{Signal, SignalKind};
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;
    use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

    // Layer-2 @property (Mandate 9; DD-SCR): pure-core direct invocation, NO
    // CLI subprocess. The driving port IS the pure `derive_candidates`
    // signature; we drive it over an arbitrary signal set x the embedded SSOT
    // mapping and assert the no-auto-inflation invariant from data-models.md
    // (I-SCR-3 / KPI-SCR-2 / WD-52 / WD-10):
    //
    //     forall (signals, mapping):
    //         derive_candidates(signals, mapping).all(|c| c.confidence == 0.25)
    //
    // The scraper has only weak public-signal evidence; it MUST stamp the
    // conservative mapping default and NEVER auto-inflate. Only the human, at
    // sign time, may consciously raise confidence (slice-01 pipeline). This
    // layer-2 property PINS that `derive_candidates` propagates the mapping's
    // 0.25 default verbatim onto every candidate (via `entry.default_confidence`
    // -> `CandidateClaim.confidence`), so a future change that derives a higher
    // proposal-time confidence fails LOUDLY here.
    //
    // Generators are inlined (not shared from a proptest_strategies module)
    // because that module is #[cfg(test)]-gated for the crate's own inner loop;
    // same approach as SD-1 (02-01).
    const SUBJECT: &str = "github:rust-lang/cargo";

    // Any one of the five bounded SignalKind variants the SSOT mapping uses.
    fn arb_signal_kind() -> impl Strategy<Value = SignalKind> {
        prop_oneof![
            Just(SignalKind::DependencyManifestPinned),
            Just(SignalKind::DocsPresentAndSubstantial),
            Just(SignalKind::TestRatioOrCiMatrix),
            Just(SignalKind::SemverAndChangelog),
            Just(SignalKind::MemorySafetyLanguage),
        ]
    }

    // A single Signal with an arbitrary kind, printable value, and a
    // GitHub-shaped public URL.
    fn arb_signal() -> impl Strategy<Value = Signal> {
        (
            arb_signal_kind(),
            "[ -~]{0,64}",
            "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}",
        )
            .prop_map(|(kind, value, source_url)| Signal {
                kind,
                value,
                source_url,
            })
    }

    // Non-vacuous signal set: a guaranteed mapping-matching head signal so at
    // least one candidate is derived on every case, plus an arbitrary tail
    // (0..6 further signals) to explore collapse + drop shapes. Without the
    // forced head the property could pass vacuously on the empty-candidate
    // case; the head makes "every candidate confidence == 0.25" a real
    // assertion over >=1 candidate.
    fn arb_signal_set() -> impl Strategy<Value = Vec<Signal>> {
        (
            arb_signal(),
            proptest::collection::vec(arb_signal(), 0..6),
        )
            .prop_map(|(head, mut tail)| {
                tail.insert(0, head);
                tail
            })
    }

    let mapping = load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse");

    let mut runner = TestRunner::default();
    runner
        .run(&arb_signal_set(), |signals| {
            let candidates = derive_candidates(SUBJECT, &signals, &mapping);
            // Non-vacuity guard: the forced mapping-matching head signal means
            // at least one candidate is always produced, so this property
            // actually exercises the per-candidate confidence assertion below.
            prop_assert!(
                !candidates.is_empty(),
                "generator forces >=1 mapping-matching signal, so >=1 candidate is expected \
                 (else the no-auto-inflation property is vacuous)"
            );
            for candidate in &candidates {
                prop_assert_eq!(
                    candidate.confidence,
                    0.25_f64,
                    "every derived CandidateClaim must carry the conservative mapping default \
                     0.25 (WD-52 / I-SCR-3): the scraper never auto-inflates proposal-time \
                     confidence; only the human raises it at sign time"
                );
                prop_assert!(
                    candidate.confidence <= 0.3_f64,
                    "no derived candidate may exceed 0.3 (no auto-inflation guardrail; \
                     KPI-SCR-2)"
                );
            }
            Ok(())
        })
        .expect(
            "no-auto-inflation invariant (WD-52 / I-SCR-3): every derived candidate must carry \
             confidence == 0.25 for all generated signal sets",
        );
}

// =============================================================================
// US-SCR-002 — determinism invariant (PROPERTY)
// =============================================================================

/// SD-3 / Property: `derive_candidates` is DETERMINISTIC — the same signals
/// + mapping produce the same candidates in the same order. Determinism is
/// load-bearing for auditability (a re-run shows the user the SAME proposals
/// they reviewed) and for reproducible candidate-list rendering across
/// invocations (SG-9's pure-read contract at the CLI layer).
///
///     forall (signals, mapping):
///         derive_candidates(signals, mapping) == derive_candidates(signals, mapping)
///
/// @property @us-scr-002 @j-004b
#[test]
fn scraper_domain_derive_candidates_is_deterministic_property() {
    use ports::{Signal, SignalKind};
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;
    use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

    // Layer-2 @property (Mandate 9; DD-SCR): pure-core direct invocation, NO
    // CLI subprocess. The driving port IS the pure `derive_candidates`
    // signature; we drive it over an arbitrary signal set x the embedded SSOT
    // mapping and assert the determinism invariant (component-boundaries.md):
    //
    //     forall (signals, mapping):
    //         derive_candidates(signals, mapping) == derive_candidates(signals, mapping)
    //
    // Determinism is load-bearing for auditability (a re-run shows the user the
    // SAME proposals they reviewed) and for reproducible candidate-list
    // rendering across invocations (SG-9's pure-read contract at the CLI layer).
    // It is structural here: grouping preserves first-appearance order via a
    // Vec (no HashMap iteration-order leak) and confidence is the verbatim
    // mapping default (no float arithmetic / NaN). This layer-2 property PINS
    // that contract so a future refactor that introduces a HashMap-ordered (or
    // otherwise nondeterministic) grouping fails LOUDLY at the derivation
    // boundary.
    //
    // Generators are inlined (not shared from a proptest_strategies module)
    // because that module is #[cfg(test)]-gated for the crate's own inner loop;
    // same approach as SD-1 (02-01) and SD-2 (02-02).
    const SUBJECT: &str = "github:rust-lang/cargo";

    // Any one of the five bounded SignalKind variants the SSOT mapping uses.
    fn arb_signal_kind() -> impl Strategy<Value = SignalKind> {
        prop_oneof![
            Just(SignalKind::DependencyManifestPinned),
            Just(SignalKind::DocsPresentAndSubstantial),
            Just(SignalKind::TestRatioOrCiMatrix),
            Just(SignalKind::SemverAndChangelog),
            Just(SignalKind::MemorySafetyLanguage),
        ]
    }

    // A single Signal with an arbitrary kind, printable value, and a
    // GitHub-shaped public URL.
    fn arb_signal() -> impl Strategy<Value = Signal> {
        (
            arb_signal_kind(),
            "[ -~]{0,64}",
            "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}",
        )
            .prop_map(|(kind, value, source_url)| Signal {
                kind,
                value,
                source_url,
            })
    }

    // Arbitrary signal set (0..7 signals) exercising collapse, drop, and order
    // shapes. No forced head here: determinism must hold for EVERY input,
    // including the empty-candidate case (Vec::new() == Vec::new()).
    fn arb_signal_set() -> impl Strategy<Value = Vec<Signal>> {
        proptest::collection::vec(arb_signal(), 0..7)
    }

    let mapping = load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse");

    let mut runner = TestRunner::default();
    runner
        .run(&arb_signal_set(), |signals| {
            let first = derive_candidates(SUBJECT, &signals, &mapping);
            let second = derive_candidates(SUBJECT, &signals, &mapping);
            prop_assert_eq!(
                first,
                second,
                "derive_candidates must be DETERMINISTIC: the same signals + mapping must yield \
                 the same candidates in the same order (a re-run shows the user the SAME \
                 proposals they reviewed)"
            );
            Ok(())
        })
        .expect(
            "determinism invariant: derive_candidates(signals, mapping) must equal a second call \
             with the SAME inputs for all generated signal sets",
        );
}

// =============================================================================
// US-SCR-002 — collapse + empty-on-no-match (example-pinned)
// =============================================================================

/// SD-4 (US-SCR-002 Ex 4; I-SCR-4): multiple signals mapping to ONE
/// predicate collapse into a SINGLE candidate whose `source_signals` lists
/// all contributing signals (no near-duplicate candidates). Example-pinned:
/// three docs-signals -> one `documentation-first` candidate with three
/// source signals.
///
/// Given three distinct signals (docs/ dir, long README, high doc-comment
/// density) that all map to documentation-first; When derive_candidates
/// runs; Then exactly ONE candidate is produced and its source_signals has
/// length 3.
///
/// @us-scr-002 @j-004b @i-scr-4
#[test]
fn scraper_domain_multiple_signals_for_one_predicate_collapse_into_one_candidate() {
    use ports::{Signal, SignalKind};
    use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

    // Layer-2 example (Mandate 9; DD-SCR): pure-core direct invocation, NO CLI
    // subprocess. The driving port IS the pure `derive_candidates` signature.
    // Example-pinned (not a property) per the file header: the collapse rule is
    // documented by a single representative fixture — three distinct
    // documentation-first signals (docs/ dir present, long README, high
    // doc-comment density) ALL map to the ONE predicate
    // org.openlore.philosophy.documentation-first (the only SignalKind reaching
    // that predicate is DocsPresentAndSubstantial). They MUST collapse into a
    // SINGLE candidate that names all three contributing signals — not three
    // near-duplicate candidates (US-SCR-002 Example 4 / I-SCR-4).
    const SUBJECT: &str = "github:rust-lang/cargo";

    let docs_dir = Signal {
        kind: SignalKind::DocsPresentAndSubstantial,
        value: "docs/ directory present".to_string(),
        source_url: "https://github.com/rust-lang/cargo/tree/master/src/doc".to_string(),
    };
    let long_readme = Signal {
        kind: SignalKind::DocsPresentAndSubstantial,
        value: "README > 200 lines".to_string(),
        source_url: "https://github.com/rust-lang/cargo/blob/master/README.md".to_string(),
    };
    let doc_comment_density = Signal {
        kind: SignalKind::DocsPresentAndSubstantial,
        value: "doc-comment density high".to_string(),
        source_url: "https://github.com/rust-lang/cargo/blob/master/src/cargo/lib.rs".to_string(),
    };

    let mapping = load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse");

    let candidates = derive_candidates(
        SUBJECT,
        &[
            docs_dir.clone(),
            long_readme.clone(),
            doc_comment_density.clone(),
        ],
        &mapping,
    );

    assert_eq!(
        candidates.len(),
        1,
        "three signals for ONE predicate must collapse into ONE candidate, not three \
         near-duplicates (US-SCR-002 Ex 4)"
    );
    let candidate = &candidates[0];
    assert_eq!(
        candidate.object, "org.openlore.philosophy.documentation-first",
        "the collapsed candidate's object is the shared philosophy predicate"
    );
    assert_eq!(
        candidate.source_signals().len(),
        3,
        "the collapsed candidate must name ALL three contributing signals \
         (auditability; I-SCR-4)"
    );
    assert_eq!(
        candidate.evidence,
        vec![
            docs_dir.source_url.clone(),
            long_readme.source_url.clone(),
            doc_comment_density.source_url.clone(),
        ],
        "evidence aggregates each contributing signal's source_url, in first-appearance order"
    );
}

/// SD-5 (US-SCR-002 Ex 2): a signal set with ZERO mapping-matching entries
/// derives an EMPTY candidate list (NOT an error). Nothing to propose is a
/// valid outcome the pure core returns as `Vec::new()`; the CLI layer (SG-7)
/// renders the "no candidates derived" message + exit 0 from this empty Vec.
///
/// Given a signal set with no entries the mapping can use; When
/// derive_candidates runs; Then the result is an empty Vec (not an error,
/// not a panic).
///
/// @us-scr-002 @j-004b @edge
#[test]
fn scraper_domain_zero_matching_signals_derive_an_empty_candidate_list() {
    use ports::{Signal, SignalKind};
    use scraper_domain::{derive_candidates, SignalPredicateMapping};

    // Layer-2 example (Mandate 9; DD-SCR): pure-core direct invocation, NO CLI
    // subprocess. The driving port IS the pure `derive_candidates` signature.
    // Example-pinned (not a property) per the file header: the empty-on-no-match
    // rule is documented by a single representative fixture.
    //
    // GIVEN a non-empty set of real harvested signals whose kinds match NO entry
    // in the mapping (here an EMPTY mapping, so every kind misses); WHEN
    // derive_candidates runs; THEN the result is an EMPTY Vec — not an Err, not a
    // panic, and crucially NOT a spurious placeholder candidate (US-SCR-002 Ex 2 /
    // I-SCR-4). Nothing-to-propose is a valid outcome the pure core returns as
    // `Vec::new()`; the CLI layer (SG-7) renders the "no candidates derived"
    // message + exit 0 from this empty Vec.
    //
    // Using a non-empty signal set against an empty mapping (rather than an empty
    // signal set) makes this exercise the DROP path in group_signals_by_predicate
    // — the only place a signal can vanish — so a future regression that emits a
    // placeholder candidate for an unmapped signal fails LOUDLY here.
    const SUBJECT: &str = "github:rust-lang/cargo";

    let signals = vec![
        Signal {
            kind: SignalKind::DependencyManifestPinned,
            value: "Cargo.lock committed".to_string(),
            source_url: "https://github.com/rust-lang/cargo/blob/master/Cargo.lock".to_string(),
        },
        Signal {
            kind: SignalKind::MemorySafetyLanguage,
            value: "primary language Rust".to_string(),
            source_url: "https://github.com/rust-lang/cargo".to_string(),
        },
    ];

    // An EMPTY mapping: no entry matches any signal kind, so every signal is
    // dropped by group_signals_by_predicate (entry_for -> None).
    let mapping = SignalPredicateMapping {
        entries: Vec::new(),
    };

    let candidates = derive_candidates(SUBJECT, &signals, &mapping);

    assert!(
        candidates.is_empty(),
        "zero mapping-matching signals must derive an EMPTY candidate list (US-SCR-002 Ex 2): \
         nothing-to-propose is a valid outcome returned as Vec::new(), not a spurious \
         placeholder candidate and not an error"
    );
}

// =============================================================================
// US-SCR-006 — mapping SSOT conformance (WD-53 / WD-67 / I-SCR-5)
// =============================================================================

/// SD-6 (gate-equivalent `mapping_matches_ssot`, I-SCR-5): the
/// signal->predicate mapping `scraper-domain` consumes is the EMBEDDED
/// snapshot of `docs/product/jobs.yaml :: J-004.signal_predicate_mapping`
/// (the SSOT) — no divergent hardcode. The loaded mapping has exactly the 5
/// slice-02 entries, each predicate is an `org.openlore.philosophy.*` NSID,
/// and each default_confidence is 0.25. This is the forward-defense against
/// the mapping drifting from product's auditable SSOT (WD-53 / WD-67).
///
/// Given the embedded jobs.yaml mapping snapshot; When load_mapping parses
/// it; Then it has 5 entries, every predicate is org.openlore.philosophy.*,
/// every default_confidence is 0.25, and the parsed entries equal the
/// jobs.yaml SSOT (no drift).
///
/// @us-scr-006 @j-004b @wd-53 @wd-67 @i-scr-5
#[test]
fn scraper_domain_embedded_mapping_matches_jobs_yaml_ssot() {
    use scraper_domain::{load_mapping, EMBEDDED_MAPPING_YAML};

    // Layer-2 example (Mandate 9; DD-SCR): pure-core direct invocation, NO CLI
    // subprocess. The driving port IS the pure `load_mapping` signature. This is
    // the acceptance-layer pin of the SSOT-conformance gate (I-SCR-5 / WD-53 /
    // WD-67): the signal->predicate mapping `scraper-domain` consumes must be the
    // EMBEDDED snapshot of `docs/product/jobs.yaml :: J-004.signal_predicate_mapping`
    // (the SSOT) — never a divergent hardcode.
    //
    // The crate's own build-time `mapping_matches_ssot` (01-02) enforces the same
    // invariant inside scraper-domain; reading the live jobs.yaml HERE (the test
    // is the effect shell, so filesystem I/O is fine — the crate itself never
    // reads the file) makes the drift gate visible at the acceptance boundary too,
    // so a future SSOT edit that is NOT mirrored into the embedded snapshot fails
    // LOUDLY in this slice's acceptance suite.

    // --- (a) structural invariants on the embedded snapshot ---------------------
    let embedded =
        load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping snapshot must parse");

    assert_eq!(
        embedded.entries.len(),
        5,
        "slice-02 SSOT has exactly 5 signal->predicate entries"
    );
    for entry in &embedded.entries {
        assert!(
            entry.object.starts_with("org.openlore.philosophy."),
            "every mapped predicate must be an org.openlore.philosophy.* NSID, got {}",
            entry.object
        );
        assert_eq!(
            entry.default_confidence, 0.25_f64,
            "every SSOT entry's default confidence is the conservative 0.25 (WD-52)"
        );
    }

    // --- (b) no-drift gate: embedded == live jobs.yaml J-004 SSOT ---------------
    // Read the authoritative block straight from disk and assert the embedded
    // snapshot parses to the IDENTICAL typed mapping. jobs.yaml lives at the
    // workspace root; this test target is owned by the `cli` crate, so the path
    // is relative to that crate's manifest dir (symmetric with the crate's own
    // mapping_matches_ssot unit test).
    let jobs_yaml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/product/jobs.yaml");
    let jobs_yaml = std::fs::read_to_string(jobs_yaml_path)
        .unwrap_or_else(|e| panic!("read SSOT jobs.yaml at {jobs_yaml_path}: {e}"));

    let ssot_block = extract_ssot_mapping_block(&jobs_yaml);
    assert!(
        ssot_block.contains("org.openlore.philosophy.dependency-pinning"),
        "the extractor must capture the J-004 signal_predicate_mapping block; got:\n{ssot_block}"
    );

    let from_ssot = load_mapping(&ssot_block).expect("the live jobs.yaml SSOT block must parse");
    assert_eq!(
        embedded, from_ssot,
        "embedded signal->predicate mapping snapshot diverged from the live \
         docs/product/jobs.yaml J-004 SSOT (drift; WD-53 / WD-67 / I-SCR-5): re-sync \
         crates/scraper-domain/src/signal_predicate_mapping.yaml from the \
         signal_predicate_mapping block (the embedded copy must match jobs.yaml)"
    );
}

/// Extract the `signal_predicate_mapping:` block from the full jobs.yaml text,
/// dedent it to column 0, and strip inline `#` comments — yielding the canonical
/// form `load_mapping` parses. Mirrors the crate's own `extract_ssot_block`
/// (scraper-domain `mapping_matches_ssot`); duplicated here because that helper
/// is `#[cfg(test)]`-gated inside the crate and unreachable from this separate
/// acceptance compilation unit.
fn extract_ssot_mapping_block(jobs_yaml: &str) -> String {
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
