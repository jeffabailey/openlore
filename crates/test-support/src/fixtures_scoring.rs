//! Canonical scoring + traversal fixtures for slice-04 acceptance.
//!
//! Symmetric with `fixtures_peer.rs` (slice-03) and `fixtures_github.rs`
//! (slice-02): each fixture is a free function returning a fresh, immutable
//! value. No shared mutable state. Tests compose by passing values through.
//!
//! Slice-04 is a READ slice over the LOCAL federated graph. These fixtures
//! describe the GRAPH SHAPE a scenario seeds (which authors assert which
//! philosophy on which projects at which confidence) — the
//! `tests/acceptance/support/mod.rs::seed_federated_graph` orchestrator turns
//! a fixture into REAL DuckDB rows via the slice-03 `claim add` / `peer add` /
//! `peer pull` seam (no new external fake — scoring/traversal is local
//! read-only analysis over the real store).
//!
//! Naming convention (matches DISCUSS/journey YAML + data-models.md worked
//! examples):
//! - `scoring_fixture_dependency_pinning_worked_example` — the canonical
//!   cargo/nixpkgs/deno weighted fixture whose arithmetic `--explain` must
//!   reproduce by hand (US-GRAPH-003/005 + data-models §"Worked example").
//! - `scoring_fixture_single_sparse_claim` — the 1-claim/1-author/no-span
//!   sparse fixture (US-GRAPH-003 Example 2; Gate 3).
//! - `scoring_fixture_rachel_spans_two_projects` — the cross-project span the
//!   traversal must surface + the SCORE-1 breadth-counts bucket fixture
//!   (US-GRAPH-004 Example 1 + Q-DELIVER-SCORE-1).
//!
//! The constants here (author bonus 0.25, triangulation bonus 0.5) MIRROR the
//! WD-77 / WD-86 defaults that live as the SSOT in `crates/scoring`'s
//! `ScoringConfig::DEFAULT`; this module re-states them ONLY to express the
//! EXPECTED worked-arithmetic targets the acceptance tests assert against (the
//! test is allowed to know the expected answer; the production constants stay
//! the single source of truth).
//!
//! SCAFFOLD: true (slice-04) — the fixture SHAPES are load-bearing now (so the
//! seeder + the layer-2 `scoring_core.rs` compile against stable signatures);
//! DELIVER materializes the concrete claim recipes per `# bodies are todo!()`
//! when the scoring crate + extended StoragePort land in step 07-01.
//
// SCAFFOLD: true

#![allow(dead_code)]

/// One author's assertion of a philosophy on a project, at a numeric
/// confidence — the atom every slice-04 graph fixture is built from. Maps to a
/// single signed claim seeded into the real store (own claim or peer claim).
///
/// `author_did` is the BARE DID (no `#fragment`); the seeder appends the
/// signing-key fragment when it routes through `claim add` / `peer pull`.
/// `relationship` is the slice-03 author-relationship the seeded claim should
/// carry once read back (You / SubscribedPeer / UnsubscribedCache).
#[derive(Debug, Clone, PartialEq)]
pub struct ScoringClaimSpec {
    pub author_did: String,
    pub subject: String,
    pub object: String,
    pub confidence: f64,
    pub relationship: ScoringRelationship,
}

/// The slice-03 author-relationship a seeded scoring claim carries when read
/// back (mirrors `ports::AuthorRelationship`; re-stated here so the fixture
/// module has no dependency on the read-side port surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringRelationship {
    You,
    SubscribedPeer,
    UnsubscribedCache,
}

/// The WD-77 / WD-86 formula constants, RE-STATED here as the expected
/// worked-arithmetic targets the acceptance tests assert against. The SSOT is
/// `crates/scoring::ScoringConfig::DEFAULT`; if DELIVER tunes a constant there,
/// these expected-target constants change in lockstep (a code + test update,
/// never a learned weight — WD-71/WD-86).
pub const EXPECTED_AUTHOR_DISTINCT_BONUS: f64 = 0.25;
pub const EXPECTED_CROSS_PROJECT_TRIANGULATION_BONUS: f64 = 0.50;

/// The canonical weighted fixture from US-GRAPH-003 Example 1 + US-GRAPH-005
/// Example 1 + data-models.md §"Worked example".
///
/// dependency-pinning across three projects:
///   - cargo:   Rachel 0.91 (Rachel ALSO asserts dependency-pinning on nixpkgs
///              -> cross-project triangulation; the SCORE-1 breadth case)
///   - nixpkgs: Rachel 0.88
///   - deno:    Tobias 0.55 + Maria 0.40 (two distinct authors)
///
/// Worked arithmetic the `--explain` output must reproduce by hand:
///   deno  = 0.55 (Tobias, first author) + 0.40*1.25 (Maria, +0.25 2nd-author)
///         = 0.55 + 0.50 = 1.05   -> bucket Moderate (breadth: 2 authors)
///   cargo = 0.91 + 0.50 (Rachel's cargo+nixpkgs triangulation) = 1.41
///         -> NOT Sparse (cross-project span is breadth; Q-DELIVER-SCORE-1)
pub fn scoring_fixture_dependency_pinning_worked_example() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return the 4-claim dependency-pinning worked-example spec (Rachel/cargo \
         0.91 + Rachel/nixpkgs 0.88 + Tobias/deno 0.55 + Maria/deno 0.40) — the canonical \
         weighted/explain fixture whose arithmetic --explain reproduces by hand"
    )
}

/// The sparse fixture (US-GRAPH-003 Example 2; Gate 3): a single actor-model
/// claim on tokio by one author at confidence 0.50, with NO co-author and NO
/// cross-project span — must bucket [SPARSE] regardless of confidence magnitude
/// (WD-74/WD-90). The SC-3 leg of the bucket rule.
pub fn scoring_fixture_single_sparse_claim() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return the single-claim single-author no-span tokio/actor-model spec \
         (conf 0.50) — the sparse fixture that must bucket [SPARSE] at any confidence (Gate 3)"
    )
}

/// The cross-project span fixture (US-GRAPH-004 Example 1 + Q-DELIVER-SCORE-1):
/// did:plc:rachel-test asserts dependency-pinning on BOTH cargo and nixpkgs.
/// The traversal must surface Rachel as spanning 2 projects; the bucket rule
/// must treat the span as evidence breadth (cargo NOT Sparse despite being a
/// single claim — the SCORE-1 breadth-counts leg).
pub fn scoring_fixture_rachel_spans_two_projects() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return the cross-project span spec (Rachel asserts dependency-pinning \
         on BOTH cargo and nixpkgs) — the traversal connection + the SCORE-1 breadth-counts bucket \
         fixture"
    )
}

/// The multi-author triangulation fixture (US-GRAPH-003 Example 3): deno with
/// reproducible-builds claims from two distinct authors (Aanya 0.40 + Tobias
/// 0.55) plus a single-author comparator project at similar max confidence, so
/// the per-additional-author bonus's effect on the ranking is observable.
pub fn scoring_fixture_reproducible_builds_multi_author() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return the multi-author reproducible-builds spec (deno: Aanya 0.40 + \
         Tobias 0.55) + a single-author comparator at similar max confidence — the triangulation \
         lift fixture (US-GRAPH-003 Example 3)"
    )
}

/// The conflicting-confidences fixture (US-GRAPH-003 Example 4): one project +
/// philosophy claimed by two authors at sharply-disagreeing confidences (0.85
/// and 0.20). Both must contribute per their confidence; nothing dropped or
/// averaged away.
pub fn scoring_fixture_conflicting_confidences_one_project() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return the conflicting-confidences spec (one project, two authors at \
         0.85 and 0.20) — both contribute per confidence, nothing dropped (US-GRAPH-003 Example 4)"
    )
}

/// The dense fan-out fixture (US-GRAPH-004 Example 3): one contributor whose
/// claims fan out to many philosophies + co-claimants beyond traversal depth 2,
/// so the default depth bound omits edges and `--depth 3` reveals them.
pub fn scoring_fixture_dense_fan_out_beyond_depth_two() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return a dense contributor-centric spec whose claims fan out beyond \
         depth 2 (many philosophies + co-claimants) — the bounded-traversal + --depth-override \
         fixture (US-GRAPH-004 Example 3)"
    )
}

/// A cyclic two-claim graph (A<->B) for the `adapter-duckdb` recursive-CTE
/// TERMINATION + cycle-safety probe (ADR-021 / adapter probe #2). Two claims
/// form a cycle (author/subject overlap closes the loop); the bounded
/// depth-limited visited-set CTE MUST terminate within the 250ms budget and
/// emit each edge exactly once. This fixture backs the adapter probe, not a
/// user-visible scenario — the DuckDB recursive-CTE does NOT auto-detect
/// cycles (the slice-04 "what if the substrate lies" check).
pub fn scoring_fixture_cyclic_two_claim_graph() -> Vec<ScoringClaimSpec> {
    todo!(
        "DELIVER (slice-04): return a two-claim cyclic graph (A<->B) for the adapter-duckdb \
         recursive-CTE termination + cycle-safety probe (ADR-021 probe #2) — the visited-set guard \
         must terminate within 250ms and emit each edge once"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The expected-target constants mirror the WD-77 defaults. This guards
    /// against the test-side worked-arithmetic targets silently drifting from
    /// the documented formula constants. (DELIVER cross-checks these against
    /// `crates/scoring::ScoringConfig::DEFAULT` once that crate exists.)
    #[test]
    fn expected_formula_constants_match_wd77_defaults() {
        assert_eq!(
            EXPECTED_AUTHOR_DISTINCT_BONUS, 0.25,
            "WD-77 author_distinct_bonus default"
        );
        assert_eq!(
            EXPECTED_CROSS_PROJECT_TRIANGULATION_BONUS, 0.50,
            "WD-77 cross_project_triangulation_bonus default"
        );
    }
}
