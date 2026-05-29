//! `compose` — the PURE anti-merging search composition (WD-103 / I-AV-2).
//!
//! [`compose_results`] groups verified, attributed [`IndexedClaim`] rows into a
//! [`NetworkSearchResult`]. It groups BY AUTHOR (or by subject under an author)
//! and NEVER merges authors: two identical-content claims by different authors
//! produce two rows under two authors. `distinct_author_count` is a COUNT over
//! the rows, NEVER a stored or computed merge. There is no API that yields a
//! faceless "network consensus" row — the absence is the design (WD-103).
//!
//! The body is intentionally `todo!()` at the 01-01 bootstrap; the grouping +
//! attribution-preservation behavior is driven by the Phase 02+ search
//! scenarios (KPI-AV-2 `network_result_preserves_attribution`) + the OD-AV-7
//! counter-annotation scenario. Split into its own module for mutation-test
//! clarity (D-D40).
//
// SCAFFOLD: true

use std::collections::BTreeMap;

use crate::{IndexedClaim, NetworkResultRow, NetworkSearchResult, SearchDimension};

/// The PURE anti-merging-preserving search composition. Groups by author; NEVER
/// merges authors; computes `distinct_author_count` from the rows (a COUNT, never
/// a merge). Deterministic; no I/O.
///
/// The pipeline is three small, named steps:
/// `rows -> map to NetworkResultRow -> group by author_did -> count distinct`.
/// `by_author` is stable-sorted by `author_did`; within each group rows are
/// stable-sorted by `cid`, so the same `(rows, dimension)` always yields a
/// byte-identical result (the 02-05 determinism precondition; DESIGN 5.1 inv 4).
pub fn compose_results(
    rows: Vec<IndexedClaim>,
    _dimension: SearchDimension,
) -> NetworkSearchResult {
    let total_claims = rows.len() as u32;
    let by_author = group_by_author(rows.into_iter().map(to_result_row));
    let distinct_author_count = by_author.len() as u32;
    NetworkSearchResult {
        by_author,
        distinct_author_count,
        total_claims,
        suggestion: None,
    }
}

/// Map one verified [`IndexedClaim`] to its [`NetworkResultRow`], carrying every
/// load-bearing field through unchanged. `author_did` (anti-merging, WD-103) and
/// `verified_against` (the `[verified]` marker, WD-104) are preserved byte-equal.
/// `counter_annotation` is `None` at this step (the OD-AV-7 annotation lands in
/// 02-08). The `relationship` is carried through; the CLI resolves the final
/// per-user relationship at render time.
fn to_result_row(claim: IndexedClaim) -> NetworkResultRow {
    NetworkResultRow {
        author_did: claim.author_did,
        cid: claim.cid,
        subject: claim.subject,
        predicate: claim.predicate,
        object: claim.object,
        confidence: claim.confidence,
        verified_against: claim.verified_against,
        relationship: claim.relationship,
        counter_annotation: None,
    }
}

/// Group rows BY AUTHOR into a stable, deterministic per-author structure. Each
/// row lands under ITS OWN `author_did` — two identical-content rows by distinct
/// authors land in DISTINCT groups (anti-merging, WD-103). A `BTreeMap` keyed by
/// the DID string gives a stable author ordering without relying on HashMap
/// iteration order; rows within each group are stable-sorted by `cid`. There is
/// no merge across authors anywhere — the per-author shape is the only output.
fn group_by_author(
    rows: impl Iterator<Item = NetworkResultRow>,
) -> Vec<(claim_domain::Did, Vec<NetworkResultRow>)> {
    let mut groups: BTreeMap<String, (claim_domain::Did, Vec<NetworkResultRow>)> = BTreeMap::new();
    for row in rows {
        let key = row.author_did.0.clone();
        groups
            .entry(key)
            .or_insert_with(|| (row.author_did.clone(), Vec::new()))
            .1
            .push(row);
    }
    groups
        .into_values()
        .map(|(did, mut rows)| {
            rows.sort_by(|a, b| a.cid.0.cmp(&b.cid.0));
            (did, rows)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthorRelationship;
    use chrono::{TimeZone, Utc};
    use claim_domain::{Cid, Did, KeyId};

    /// Build a verified `IndexedClaim` for the in-crate compose tests. The CID is
    /// caller-supplied so identical-content-distinct-author rows stay DISTINCT
    /// multiset members; `verified_against` is non-empty (verified-before-index).
    fn claim(
        author: &str,
        cid: &str,
        subject: &str,
        object: &str,
        confidence: f64,
    ) -> IndexedClaim {
        IndexedClaim {
            author_did: Did(author.to_string()),
            cid: Cid(cid.to_string()),
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            confidence,
            composed_at: Utc.with_ymd_and_hms(2026, 5, 26, 12, 0, 0).unwrap(),
            verified_against: KeyId(format!("{author}#org.openlore.application")),
            evidence: Vec::new(),
            references: Vec::new(),
            relationship: AuthorRelationship::NetworkUnfollowed,
        }
    }

    /// Canonical single-author case: two claims by ONE author compose to ONE
    /// group holding both rows; `distinct_author_count == 1`; `total_claims == 2`.
    #[test]
    fn single_author_composes_to_one_group_with_all_rows() {
        let rows = vec![
            claim("did:plc:priya", "cidA", "github:a/a", "phil.x", 0.7),
            claim("did:plc:priya", "cidB", "github:b/b", "phil.y", 0.6),
        ];

        let result = compose_results(rows, SearchDimension::Object);

        assert_eq!(result.distinct_author_count, 1);
        assert_eq!(result.total_claims, 2);
        assert_eq!(result.by_author.len(), 1, "exactly one author group");
        let (did, group) = &result.by_author[0];
        assert_eq!(did, &Did("did:plc:priya".to_string()));
        assert_eq!(
            group.len(),
            2,
            "both rows preserved under the single author"
        );
    }

    /// Canonical multi-author case: claims by THREE authors compose to THREE
    /// groups; `distinct_author_count == 3`; `total_claims == 3`; groups are
    /// stable-sorted by `author_did`.
    #[test]
    fn multi_author_composes_to_one_group_per_author_stable_sorted() {
        // Insert in NON-sorted author order to pin the stable-by-author-did sort.
        let rows = vec![
            claim("did:plc:sven", "cidS", "github:a/a", "phil.x", 0.5),
            claim("did:plc:rachel", "cidR", "github:b/b", "phil.y", 0.9),
            claim("did:plc:priya", "cidP", "github:c/c", "phil.z", 0.7),
        ];

        let result = compose_results(rows, SearchDimension::Object);

        assert_eq!(result.distinct_author_count, 3);
        assert_eq!(result.total_claims, 3);
        let keys: Vec<&Did> = result.by_author.iter().map(|(did, _)| did).collect();
        assert_eq!(
            keys,
            vec![
                &Did("did:plc:priya".to_string()),
                &Did("did:plc:rachel".to_string()),
                &Did("did:plc:sven".to_string()),
            ],
            "by_author must be stable-sorted by author_did (priya < rachel < sven)"
        );
    }

    /// The LOAD-BEARING anti-merging case (WD-103): two claims with IDENTICAL
    /// (subject, object) but DISTINCT authors compose to TWO SEPARATE single-row
    /// groups — never collapsed into a merged multi-author row.
    /// `distinct_author_count == 2`; `total_claims == 2`.
    #[test]
    fn identical_content_distinct_author_never_merges() {
        let rows = vec![
            claim(
                "did:plc:priya",
                "cidP",
                "github:denoland/deno",
                "phil.deppin",
                0.70,
            ),
            claim(
                "did:plc:sven",
                "cidS",
                "github:denoland/deno",
                "phil.deppin",
                0.65,
            ),
        ];

        let result = compose_results(rows, SearchDimension::Object);

        assert_eq!(
            result.distinct_author_count, 2,
            "distinct_author_count is a COUNT over attributed rows (never a merge)"
        );
        assert_eq!(result.total_claims, 2, "no row dropped or merged away");
        assert_eq!(
            result.by_author.len(),
            2,
            "two authors with identical content stay in TWO separate groups"
        );
        for (did, group) in &result.by_author {
            assert_eq!(
                group.len(),
                1,
                "each author's group holds its own single row"
            );
            assert_eq!(
                &group[0].author_did, did,
                "every row lives under its OWN author_did key (no merged key)"
            );
        }
    }

    /// Within an author group, rows are stable-sorted by `cid` (the 02-05
    /// determinism precondition) and the per-row mapping carries every
    /// load-bearing field through (`author_did`, `verified_against` non-empty,
    /// `counter_annotation == None` at this step).
    #[test]
    fn rows_within_group_are_stable_sorted_by_cid_and_fields_carry_through() {
        let rows = vec![
            claim("did:plc:priya", "cid-z", "github:a/a", "phil.x", 0.7),
            claim("did:plc:priya", "cid-a", "github:b/b", "phil.y", 0.6),
        ];

        let result = compose_results(rows, SearchDimension::Object);

        let (_did, group) = &result.by_author[0];
        let cids: Vec<&Cid> = group.iter().map(|r| &r.cid).collect();
        assert_eq!(
            cids,
            vec![&Cid("cid-a".to_string()), &Cid("cid-z".to_string())],
            "rows within an author group must be stable-sorted by cid"
        );
        for row in group {
            assert!(
                !row.verified_against.0.is_empty(),
                "every mapped row carries a non-empty verified_against (WD-104)"
            );
            assert_eq!(
                row.counter_annotation, None,
                "counter_annotation is None at this step (lands in 02-08)"
            );
        }
    }
}
