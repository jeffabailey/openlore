//! Integration tests for the index-store extension of the
//! `no_cross_table_join_elides_author` anti-merging rule (WD-120 / I-AV-2),
//! added to `cargo xtask check-arch` in step 01-03.
//!
//! Where the slice-03/04 rule ([`classify_sql_literal`]) keys on a CROSS-STORE
//! `claims` + `peer_claims` reference, the slice-05 index store is a SINGLE
//! `indexed_claims` table — the anti-merging risk is a SINGLE-TABLE aggregate
//! (`GROUP BY` / `COUNT` / `SUM` / `AVG` across authors) that DROPS
//! `author_did`, fabricating a faceless "network consensus" row (WD-103). The
//! NEW pure classifier [`classify_index_store_sql_literal`] catches exactly that.
//!
//! Rule contract: a SQL string literal that AGGREGATES over `indexed_claims`
//! (mentions the `indexed_claims` table AND an aggregation construct — `GROUP
//! BY`, `COUNT(`, `SUM(`, `AVG(`) MUST also project `author_did`; otherwise it
//! is a violation. A per-claim `SELECT` that projects `author_did` (the SAFE
//! read-side query shape) is clean, even with a `WHERE`. A literal that does
//! not touch `indexed_claims`, or one that aggregates but still projects
//! `author_did`, is clean.
//!
//! This is the STRUCTURAL enforcement layer of the 3-layer anti-merging
//! defense for the index store (TYPE layer = `IndexedClaim.author_did`
//! non-`Option` from 01-02; BEHAVIORAL layer = AV-9/AV-2/AVC-2 in later
//! phases).

use proptest::prelude::*;
use xtask::check_arch::classify_index_store_sql_literal;

// -----------------------------------------------------------------------------
// Verbatim fixtures from data-models.md §"Read-side query shapes" /
// §"FORBIDDEN pattern".
// -----------------------------------------------------------------------------

/// The SAFE headline read-side query verbatim from data-models.md §"Dimension
/// query — by object" — a per-claim `SELECT` projecting `author_did`, NO
/// aggregation across authors. MUST pass (negative test: the rule does not
/// false-positive on the correct query).
const SAFE_QUERY_BY_OBJECT: &str = "\
SELECT ic.author_did, ic.cid, ic.subject, ic.predicate, ic.object,
       ic.confidence, ic.composed_at, ic.verified_against, ic.signed_record_path
FROM indexed_claims ic
WHERE ic.object = ?object;";

/// The FORBIDDEN aggregating query verbatim from data-models.md §"FORBIDDEN
/// pattern" — a `GROUP BY object` over `indexed_claims` that DROPS `author_did`,
/// merging the network into a faceless consensus row (WD-103 / I-AV-2). MUST be
/// rejected (positive test: the rule catches the bug).
const FORBIDDEN_GROUP_BY_OBJECT: &str = "\
SELECT object, COUNT(*) AS faux_consensus, AVG(confidence) AS faux_network_confidence
FROM indexed_claims
GROUP BY object;";

/// The SAFE counter-annotation lookup verbatim from data-models.md §"Counter-
/// relationship annotation" — a JOIN over `indexed_claims` ↔
/// `indexed_claim_references` (SAME store) that PROJECTS the countering claim's
/// author (`ic2.author_did AS counter_author`). NOT a cross-store join; it
/// preserves attribution. MUST pass.
const SAFE_COUNTER_ANNOTATION_JOIN: &str = "\
SELECT icr.referencing_cid, icr.referenced_cid, icr.ref_type, ic2.author_did AS counter_author
FROM indexed_claim_references icr
JOIN indexed_claims ic2 ON ic2.cid = icr.referencing_cid
WHERE icr.referenced_cid IN (?result_cids) AND icr.ref_type IN ('counters','retracts');";

#[test]
fn forbidden_group_by_object_eliding_author_did_is_rejected() {
    let verdict = classify_index_store_sql_literal(FORBIDDEN_GROUP_BY_OBJECT);
    assert!(
        verdict.is_some(),
        "FORBIDDEN `GROUP BY object` over `indexed_claims` that drops \
         `author_did` MUST be flagged (I-AV-2 / WD-103); classifier returned None"
    );
}

#[test]
fn safe_query_by_object_projecting_author_did_passes() {
    let verdict = classify_index_store_sql_literal(SAFE_QUERY_BY_OBJECT);
    assert!(
        verdict.is_none(),
        "SAFE per-claim query projecting `author_did` MUST pass; classifier \
         flagged it as a violation: {verdict:?}"
    );
}

#[test]
fn safe_counter_annotation_same_store_join_projecting_author_passes() {
    let verdict = classify_index_store_sql_literal(SAFE_COUNTER_ANNOTATION_JOIN);
    assert!(
        verdict.is_none(),
        "the same-store counter-annotation JOIN projects `counter_author` \
         (author_did) and MUST pass; classifier flagged it: {verdict:?}"
    );
}

#[test]
fn literal_not_touching_indexed_claims_is_not_an_index_store_violation() {
    // The slice-01/03/04 `claims`/`peer_claims` queries live in adapter-duckdb
    // and are governed by the OTHER classifier. An index-store DDL statement
    // for the references table (no `indexed_claims` aggregation) is clean here.
    let unrelated = "INSERT INTO indexed_claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)";
    assert!(
        classify_index_store_sql_literal(unrelated).is_none(),
        "an insert into a non-aggregated child table must not be an \
         index-store anti-merging violation"
    );
}

// -----------------------------------------------------------------------------
// Property-based adversarial-SQL generation (nw-tdd-methodology PBT mandate;
// roadmap: generate adversarial aggregating SQL strings and assert the rule
// FIRES; assert SAFE explicit-author_did SELECTs PASS).
// -----------------------------------------------------------------------------

/// An aggregation construct over `indexed_claims` that drops `author_did`.
/// Strategy over the dimension column + the aggregate function — every member
/// of the equivalence class "author-eliding aggregate over the index" MUST
/// fire the rule.
fn forbidden_aggregate_strategy() -> impl Strategy<Value = String> {
    let dimension = prop_oneof![Just("object"), Just("subject"), Just("predicate")];
    let aggregate = prop_oneof![
        Just("COUNT(*)"),
        Just("SUM(confidence)"),
        Just("AVG(confidence)"),
    ];
    (dimension, aggregate).prop_map(|(dim, agg)| {
        format!(
            "SELECT {dim}, {agg} AS faux_consensus \
             FROM indexed_claims GROUP BY {dim}"
        )
    })
}

/// A SAFE per-claim read-side query over `indexed_claims` that PROJECTS
/// `author_did`. Strategy over the WHERE dimension — every member of the
/// equivalence class "attributed per-claim read" MUST pass.
fn safe_attributed_query_strategy() -> impl Strategy<Value = String> {
    let dimension = prop_oneof![Just("object"), Just("subject"), Just("author_did")];
    dimension.prop_map(|dim| {
        format!(
            "SELECT ic.author_did, ic.cid, ic.subject, ic.predicate, ic.object, \
             ic.confidence, ic.composed_at, ic.verified_against \
             FROM indexed_claims ic WHERE ic.{dim} = ?value"
        )
    })
}

proptest! {
    /// Property: ANY aggregate over `indexed_claims` that omits `author_did` is
    /// flagged. The whole forbidden equivalence class fires the rule.
    #[test]
    fn all_author_eliding_aggregates_over_index_are_rejected(sql in forbidden_aggregate_strategy()) {
        prop_assert!(
            classify_index_store_sql_literal(&sql).is_some(),
            "an author-eliding aggregate over `indexed_claims` MUST be flagged, \
             but the classifier passed it: {sql}"
        );
    }

    /// Property: ANY per-claim attributed read (projecting `author_did`) passes,
    /// regardless of which dimension it filters on. The whole safe equivalence
    /// class is clean — no false positives.
    #[test]
    fn all_attributed_per_claim_reads_pass(sql in safe_attributed_query_strategy()) {
        prop_assert!(
            classify_index_store_sql_literal(&sql).is_none(),
            "a per-claim attributed read projecting `author_did` MUST pass, \
             but the classifier flagged it: {sql}"
        );
    }
}
