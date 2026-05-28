//! Canonical GitHub-harvest fixtures for slice-02 acceptance scenarios.
//!
//! Symmetric with `fixtures.rs` (slice-01 claim fixtures) and
//! `fixtures_peer.rs` (slice-03 peer-claim fixtures): one free function per
//! well-known fixture used across US-SCR-001..005. Re-exported flat from
//! `lib.rs` so acceptance files write
//! `openlore_test_support::fixture_cargo_five_signals()` directly.
//!
//! The signal set mirrors `docs/product/jobs.yaml ::
//! J-004.signal_predicate_mapping` (the SSOT, 5 entries): every fixture
//! signal's `kind` maps to exactly one mapping predicate, so a happy-path
//! harvest of `rust-lang/cargo` derives 5 candidates (one per mapping
//! entry), each at the conservative default confidence 0.25 (WD-52). The
//! fixtures NEVER hardcode the predicate or the 0.25 default — those are the
//! pure `scraper-domain` derivation's job (the fixture only supplies the
//! raw harvested signal). This keeps the fixtures honest about the
//! pure/effect split (WD-56): a fixture is harvested EFFECT data; the
//! candidate is a PURE derivation downstream.

#![allow(dead_code)]

use crate::fake_github::FakeSignal;

/// The five public signals a happy-path harvest of `rust-lang/cargo`
/// returns — one per `jobs.yaml` mapping entry, in mapping order:
///
/// 1. `DependencyManifestPinned`  — "Cargo.lock committed (exact pins)"
/// 2. `DocsPresentAndSubstantial` — "docs/ present + README 412 lines"
/// 3. `TestRatioOrCiMatrix`       — "test/source ratio 0.61"
/// 4. `SemverAndChangelog`        — "CHANGELOG present + semver tags"
/// 5. `MemorySafetyLanguage`      — "Rust + no unsafe blocks"
///
/// Each carries a public GitHub `source_url` that becomes the candidate's
/// evidence. Used by SG-1 (walking skeleton), SG-2 (5-candidate render),
/// and SC-1 (auditability: each candidate names its source signal).
pub fn fixture_cargo_five_signals() -> Vec<FakeSignal> {
    vec![
        FakeSignal::new(
            "DependencyManifestPinned",
            "Cargo.lock committed (exact pins)",
            "https://github.com/rust-lang/cargo/blob/master/Cargo.lock",
        ),
        FakeSignal::new(
            "DocsPresentAndSubstantial",
            "docs/ present + README 412 lines",
            "https://github.com/rust-lang/cargo/tree/master/src/doc",
        ),
        FakeSignal::new(
            "TestRatioOrCiMatrix",
            "test/source ratio 0.61",
            "https://github.com/rust-lang/cargo/tree/master/tests",
        ),
        FakeSignal::new(
            "SemverAndChangelog",
            "CHANGELOG present + semver tags",
            "https://github.com/rust-lang/cargo/blob/master/CHANGELOG.md",
        ),
        FakeSignal::new(
            "MemorySafetyLanguage",
            "Rust + no unsafe blocks",
            "https://github.com/rust-lang/cargo",
        ),
    ]
}

/// A bounded cross-repo aggregate signal set for the `torvalds` USER
/// target (US-SCR-001 Ex 2; WD-64 bounded aggregate, deep triangulation
/// deferred to slice-04). Fewer, coarser signals than the repo fixture —
/// the aggregate is intentionally shallow in slice-02.
pub fn fixture_torvalds_user_aggregate_signals() -> Vec<FakeSignal> {
    vec![
        FakeSignal::new(
            "TestRatioOrCiMatrix",
            "aggregate: CI test matrix across pinned repos",
            "https://github.com/torvalds",
        ),
        FakeSignal::new(
            "SemverAndChangelog",
            "aggregate: tagged releases + changelogs",
            "https://github.com/torvalds?tab=repositories",
        ),
    ]
}

/// Three DISTINCT signals that ALL map to the single `documentation-first`
/// predicate (docs/ directory + a 400-line README + high doc-comment
/// density). Used by SC-3 to assert `scraper-domain` collapses them into
/// ONE candidate whose source-signal line lists all three (US-SCR-002 Ex 4
/// / I-SCR-4). The collapse is the PURE derivation's job; the fixture only
/// supplies the three raw signals.
pub fn fixture_three_docs_signals_one_predicate() -> Vec<FakeSignal> {
    vec![
        FakeSignal::new(
            "DocsDirectoryPresent",
            "docs/ directory present",
            "https://github.com/some-org/well-documented/tree/main/docs",
        ),
        FakeSignal::new(
            "ReadmeSubstantial",
            "README 412 lines (> 200)",
            "https://github.com/some-org/well-documented/blob/main/README.md",
        ),
        FakeSignal::new(
            "DocCommentDensityHigh",
            "doc-comment density high (0.34)",
            "https://github.com/some-org/well-documented",
        ),
    ]
}
