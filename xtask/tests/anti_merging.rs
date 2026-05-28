//! Integration tests for the `no_cross_table_join_elides_author` rule
//! (WD-30 / I-FED-1) added to `cargo xtask check-arch` in step 01-06.
//!
//! These exercise the PURE classifier [`xtask::check_arch::classify_sql_literal`]
//! against the EXACT fixtures from
//! `docs/feature/openlore-federated-read/design/data-models.md`
//! §"Cross-store query examples" — the SAFE `UNION ALL` (must pass) and the
//! FORBIDDEN `JOIN` (must be rejected).
//!
//! The classifier is the anti-merging structural enforcement layer 2 of 3
//! (layer 1 = `FederatedRow.author_did` non-Option from 01-01; layer 3 =
//! behavioral integration test `federation_attribution_preserved` in Phase 05).
//!
//! Rule contract: a SQL string literal that mentions BOTH the standalone
//! `claims` table AND the `peer_claims` table MUST also project `author_did`
//! in its SELECT list; otherwise it is a violation. A literal mentioning only
//! one of the tables, or one that mentions both but DOES project `author_did`,
//! is clean.

use xtask::check_arch::classify_sql_literal;

/// The FORBIDDEN pattern verbatim from data-models.md §"Cross-store query
/// examples" — a JOIN across `claims` + `peer_claims` that elides
/// `author_did`. MUST be rejected (positive test: the rule catches the bug).
const FORBIDDEN_JOIN: &str = "\
SELECT c.subject, c.predicate, c.object, COUNT(*) AS total
FROM claims c
JOIN peer_claims pc ON c.subject = pc.subject AND c.predicate = pc.predicate
WHERE c.subject = ?subject
GROUP BY c.subject, c.predicate, c.object;";

/// The SAFE pattern verbatim from data-models.md — a UNION ALL across both
/// tables that DOES project `author_did`. MUST pass (negative test: the rule
/// does not false-positive on the correct query).
const SAFE_UNION_ALL: &str = "\
SELECT
    c.author_did AS author_did,
    c.cid        AS cid,
    c.predicate  AS predicate,
    c.object     AS object,
    c.confidence AS confidence,
    c.composed_at AS composed_at,
    c.artifact_path AS artifact_path,
    'Own'        AS source_table
FROM claims c
WHERE c.subject = ?subject

UNION ALL

SELECT
    pc.author_did AS author_did,
    pc.cid        AS cid,
    pc.predicate  AS predicate,
    pc.object     AS object,
    pc.confidence AS confidence,
    pc.composed_at AS composed_at,
    pc.signed_record_path AS artifact_path,
    'Peer'        AS source_table
FROM peer_claims pc
WHERE pc.subject = ?subject;";

#[test]
fn forbidden_join_eliding_author_did_is_rejected() {
    let verdict = classify_sql_literal(FORBIDDEN_JOIN);
    assert!(
        verdict.is_some(),
        "FORBIDDEN JOIN mentioning both `claims` and `peer_claims` without \
         `author_did` MUST be flagged; classifier returned None"
    );
}

#[test]
fn safe_union_all_with_author_did_passes() {
    let verdict = classify_sql_literal(SAFE_UNION_ALL);
    assert!(
        verdict.is_none(),
        "SAFE UNION ALL projecting `author_did` across both tables MUST pass; \
         classifier flagged it as a violation: {verdict:?}"
    );
}

#[test]
fn literal_mentioning_only_peer_claims_is_not_a_cross_table_violation() {
    // schema DDL / single-table inserts mention `peer_claims` (and thus the
    // substring `claims`) but NOT the standalone `claims` table. The rule
    // must use word-boundary matching so single-table SQL never trips it.
    let single_table = "\
        INSERT INTO peer_claims (cid, author_did, subject) VALUES (?, ?, ?)";
    assert!(
        classify_sql_literal(single_table).is_none(),
        "single-table `peer_claims` INSERT must NOT be treated as a cross-table \
         JOIN — `claims` substring inside `peer_claims` must not count"
    );

    // A bare `claims`-only SELECT (slice-01 own-store query) likewise must not
    // trip the cross-table rule.
    let claims_only = "SELECT author_did, cid FROM claims WHERE subject = ?";
    assert!(
        classify_sql_literal(claims_only).is_none(),
        "single-table `claims` SELECT must NOT be a cross-table violation"
    );
}

#[test]
fn cross_table_join_with_author_did_projected_passes() {
    // Symmetry guard: a JOIN (not a UNION) is fine *as long as* author_did is
    // projected. The rule is about eliding attribution, not about JOIN syntax.
    let joined_with_author = "\
        SELECT c.author_did, pc.author_did, c.subject \
         FROM claims c JOIN peer_claims pc ON c.subject = pc.subject";
    assert!(
        classify_sql_literal(joined_with_author).is_none(),
        "a cross-table query that DOES project `author_did` must pass even if \
         it uses JOIN syntax"
    );
}

// -----------------------------------------------------------------------------
// Slice-04 (scoring + graph) extension fixtures — component-boundaries.md §xtask
// + data-models.md §"Scoring-feed query" / §"Traversal query".
// -----------------------------------------------------------------------------
//
// The classifier scans EVERY `.rs` file in `adapter-duckdb/src` (incl. the new
// `graph_query.rs`), so the slice-04 scoring-feed `UNION ALL` and recursive-CTE
// `traverse_graph` literals fall under the SAME `no_cross_table_join_elides_author`
// pass. These fixtures pin the NEW query shapes against the classifier so that
// when the live SQL lands (Phase 03/04/05) the rule already covers it: the safe
// cross-store scoring UNION-ALL (and the recursive CTE) — both projecting
// `author_did` — pass; the FORBIDDEN aggregating query that elides `author_did`
// is rejected.

/// SAFE scoring-feed: the `query_attributed_for_scoring(ByObject)` UNION ALL —
/// identical shape to `query_by_object`, projecting `author_did` from BOTH
/// stores. Aggregation (the weight) happens in the pure `scoring` core in Rust,
/// NEVER in SQL — so this literal stays per-claim and attribution-preserving.
/// MUST pass (the rule must not false-positive on the correct scoring feed).
const SAFE_SCORING_FEED_UNION_ALL: &str = "\
SELECT c.author_did, c.cid, c.subject, c.predicate, c.object, c.confidence, c.composed_at
FROM claims c
WHERE c.object = ?object
UNION ALL
SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object, pc.confidence, pc.composed_at
FROM peer_claims pc
WHERE pc.object = ?object;";

/// FORBIDDEN aggregating scoring query verbatim from data-models.md
/// §"Scoring-feed query" — the outer `GROUP BY` over a `claims`+`peer_claims`
/// UNION subquery DROPS `author_did`, merging the weight in SQL across authors
/// (I-GRAPH-2 violation). MUST be rejected: it mentions both tables and elides
/// `author_did`.
const FORBIDDEN_AGGREGATING_GROUP_BY: &str = "\
SELECT subject, object, SUM(confidence) AS faux_weight
FROM (SELECT subject, object, confidence FROM claims
      UNION ALL SELECT subject, object, confidence FROM peer_claims)
GROUP BY subject, object;";

/// SAFE recursive-CTE traversal base verbatim shape from data-models.md
/// §"Traversal query" — the `edges_base` CTE UNION-ALLs `claims` + `peer_claims`
/// and projects `author_did` on EVERY edge row (Gate 5 / I-GRAPH-2). MUST pass.
const SAFE_TRAVERSAL_RECURSIVE_CTE: &str = "\
SELECT author_did, cid AS claim_cid, subject, object FROM claims
UNION ALL
SELECT author_did, cid AS claim_cid, subject, object FROM peer_claims";

#[test]
fn safe_scoring_feed_union_all_with_author_did_passes() {
    assert!(
        classify_sql_literal(SAFE_SCORING_FEED_UNION_ALL).is_none(),
        "the slice-04 scoring-feed UNION ALL projects `author_did` across both \
         stores and MUST pass; classifier flagged it: {:?}",
        classify_sql_literal(SAFE_SCORING_FEED_UNION_ALL)
    );
}

#[test]
fn forbidden_aggregating_group_by_eliding_author_did_is_rejected() {
    assert!(
        classify_sql_literal(FORBIDDEN_AGGREGATING_GROUP_BY).is_some(),
        "the FORBIDDEN aggregating query unions `claims`+`peer_claims` and \
         GROUP BYs without `author_did` — it merges the weight in SQL and MUST \
         be flagged (I-GRAPH-2); classifier returned None"
    );
}

#[test]
fn safe_traversal_recursive_cte_base_with_author_did_passes() {
    assert!(
        classify_sql_literal(SAFE_TRAVERSAL_RECURSIVE_CTE).is_none(),
        "the recursive-CTE traversal base projects `author_did` on every edge \
         across both stores and MUST pass; classifier flagged it: {:?}",
        classify_sql_literal(SAFE_TRAVERSAL_RECURSIVE_CTE)
    );
}
