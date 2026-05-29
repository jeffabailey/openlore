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

use claim_domain::{Cid, ReferenceType};

use crate::{CounterRef, IndexedClaim, NetworkResultRow, NetworkSearchResult, SearchDimension};

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
    // OD-AV-7 (shown-not-applied): derive the counter/retract annotations BEFORE
    // mapping rows. This is a pure annotation pass — it NEVER removes, filters, or
    // down-weights any row (D-D40); the countered row is preserved and merely
    // carries a `counter_annotation`.
    let counters = annotate_counter_relationship(&rows);
    let by_author = group_by_author(
        rows.into_iter()
            .map(|claim| to_result_row(claim, &counters)),
    );
    let distinct_author_count = by_author.len() as u32;
    NetworkSearchResult {
        by_author,
        distinct_author_count,
        total_claims,
        suggestion: None,
    }
}

/// PURE OD-AV-7 counter-relationship detection (D-D40). Scans the indexed claims
/// for typed references with `ref_type ∈ {Counters, Retracts}` and builds the map
/// `countered_cid -> CounterRef` describing WHICH countering claim annotates each
/// countered row. This is the SHOWN-not-applied default (WD-119; mirrors slice-04
/// WD-85): it produces an ANNOTATION and NOTHING else — it never removes, filters,
/// or down-weights a row. The countered row stays in the result; only its
/// `counter_annotation` is populated by [`to_result_row`].
///
/// A claim K "counters"/"retracts" claim C when K carries a [`ClaimReference`]
/// whose `cid == C.cid` and whose `ref_type` is `Counters` or `Retracts`. The map
/// records `CounterRef { referencing_cid: K.cid, counter_author: K.author_did,
/// ref_type }`. References at CIDs that are NOT present in this result set still
/// produce a map entry keyed by that CID — applying it is a lookup by the
/// countered row's own CID, so an absent target simply never matches a row.
///
/// **Multi-counter tiebreak (determinism, 02-05):** if several claims counter the
/// SAME C, the annotation is the one whose countering CID is lexicographically
/// LOWEST. The choice is independent of input order, so the same input multiset
/// always yields a byte-identical result (DESIGN 5.1 inv 4). A later `Retracts`
/// and an earlier `Counters` on the same C are compared purely by countering CID;
/// the lowest-CID counterer wins regardless of relationship kind.
fn annotate_counter_relationship(claims: &[IndexedClaim]) -> BTreeMap<String, CounterRef> {
    let mut by_countered: BTreeMap<String, CounterRef> = BTreeMap::new();
    for counterer in claims {
        for reference in &counterer.references {
            if !is_counter_relationship(reference.ref_type) {
                continue;
            }
            let candidate = CounterRef {
                referencing_cid: counterer.cid.clone(),
                counter_author: counterer.author_did.clone(),
                ref_type: reference.ref_type,
            };
            let key = reference.cid.0.clone();
            match by_countered.get(&key) {
                // Deterministic tiebreak: keep the lowest countering CID.
                Some(existing) if existing.referencing_cid.0 <= candidate.referencing_cid.0 => {}
                _ => {
                    by_countered.insert(key, candidate);
                }
            }
        }
    }
    by_countered
}

/// The OD-AV-7 relationship kinds that annotate a countered row. `Counters` and
/// its sibling `Retracts` are SHOWN, never applied; `Corrects` / `Supersedes` are
/// not counter relationships and never annotate.
fn is_counter_relationship(ref_type: ReferenceType) -> bool {
    matches!(ref_type, ReferenceType::Counters | ReferenceType::Retracts)
}

/// Map one verified [`IndexedClaim`] to its [`NetworkResultRow`], carrying every
/// load-bearing field through unchanged. `author_did` (anti-merging, WD-103) and
/// `verified_against` (the `[verified]` marker, WD-104) are preserved byte-equal.
/// `counter_annotation` is set from `counters` keyed by this row's OWN `cid` (the
/// OD-AV-7 shown-not-applied annotation, 02-08) — `None` when nothing counters it.
/// The `relationship` is carried through; the CLI resolves the final per-user
/// relationship at render time.
fn to_result_row(claim: IndexedClaim, counters: &BTreeMap<String, CounterRef>) -> NetworkResultRow {
    let counter_annotation = annotation_for(&claim.cid, counters);
    NetworkResultRow {
        author_did: claim.author_did,
        cid: claim.cid,
        subject: claim.subject,
        predicate: claim.predicate,
        object: claim.object,
        confidence: claim.confidence,
        verified_against: claim.verified_against,
        relationship: claim.relationship,
        counter_annotation,
    }
}

/// Look up the counter annotation for a row by its OWN `cid`. The annotation is
/// SHOWN on the countered row; absent CIDs simply have no entry (returns `None`).
fn annotation_for(cid: &Cid, counters: &BTreeMap<String, CounterRef>) -> Option<CounterRef> {
    counters.get(&cid.0).cloned()
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
    use crate::{AuthorRelationship, CounterRef};
    use chrono::{TimeZone, Utc};
    use claim_domain::{Cid, ClaimReference, Did, KeyId, ReferenceType};

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
        claim_with_refs(author, cid, subject, object, confidence, Vec::new())
    }

    /// Like [`claim`] but carrying typed `references` (for the OD-AV-7 counter /
    /// retract annotation tests). A countering claim K references the countered
    /// claim C's CID with `ref_type ∈ {Counters, Retracts}`.
    fn claim_with_refs(
        author: &str,
        cid: &str,
        subject: &str,
        object: &str,
        confidence: f64,
        references: Vec<ClaimReference>,
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
            references,
            relationship: AuthorRelationship::NetworkUnfollowed,
        }
    }

    /// Find the single composed row whose `cid` matches `target` across all author
    /// groups (the flattened presence-universe the OD-AV-7 tests assert over).
    fn row_for<'a>(result: &'a NetworkSearchResult, target: &Cid) -> Option<&'a NetworkResultRow> {
        result
            .by_author
            .iter()
            .flat_map(|(_did, rows)| rows.iter())
            .find(|row| &row.cid == target)
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
                "no-counter rows carry no counter_annotation"
            );
        }
    }

    /// OD-AV-7 (shown-not-applied): a claim C countered by an indexed claim K is
    /// STILL present (`total_claims` unchanged, C in the output) and C's row
    /// carries `counter_annotation == Some(CounterRef{ by: K.author, cid: K.cid,
    /// Counters })`. The counter is an ANNOTATION, NEVER a filter/removal — a code
    /// path that dropped C would fail the presence assertion below.
    #[test]
    fn counter_annotates_the_countered_row_and_never_removes_it() {
        let c = claim("did:plc:priya", "cidC", "github:a/a", "phil.x", 0.82);
        let k = claim_with_refs(
            "did:plc:sven",
            "cidK",
            "github:a/a",
            "phil.x",
            0.55,
            vec![ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: Cid("cidC".to_string()),
            }],
        );

        let result = compose_results(vec![c, k], SearchDimension::Object);

        // Presence: neither row removed/filtered/down-weighted.
        assert_eq!(
            result.total_claims, 2,
            "the counter never drops a row (OD-AV-7)"
        );
        let countered = row_for(&result, &Cid("cidC".to_string()))
            .expect("the countered claim C is STILL present after annotation");
        assert_eq!(
            countered.counter_annotation,
            Some(CounterRef {
                referencing_cid: Cid("cidK".to_string()),
                counter_author: Did("did:plc:sven".to_string()),
                ref_type: ReferenceType::Counters,
            }),
            "C carries the counter annotation pointing at K (shown, not applied)"
        );
        // The countering claim K itself is NOT annotated (one-directional).
        let counter = row_for(&result, &Cid("cidK".to_string()))
            .expect("the countering claim K is also present");
        assert_eq!(
            counter.counter_annotation, None,
            "K is the COUNTERING claim; it carries no annotation of its own"
        );
    }

    /// `Retracts` is the sibling case of `Counters`: a retraction is also an
    /// annotation on the retracted row, carrying `ref_type == Retracts`, and the
    /// retracted row is likewise never removed.
    #[test]
    fn retract_annotates_the_retracted_row_with_retracts_ref_type() {
        let c = claim("did:plc:priya", "cidC", "github:a/a", "phil.x", 0.82);
        let k = claim_with_refs(
            "did:plc:priya",
            "cidK",
            "github:a/a",
            "phil.x",
            0.10,
            vec![ClaimReference {
                ref_type: ReferenceType::Retracts,
                cid: Cid("cidC".to_string()),
            }],
        );

        let result = compose_results(vec![c, k], SearchDimension::Object);

        assert_eq!(result.total_claims, 2, "a retraction never drops a row");
        let retracted = row_for(&result, &Cid("cidC".to_string()))
            .expect("the retracted claim C is STILL present");
        assert_eq!(
            retracted.counter_annotation,
            Some(CounterRef {
                referencing_cid: Cid("cidK".to_string()),
                counter_author: Did("did:plc:priya".to_string()),
                ref_type: ReferenceType::Retracts,
            }),
            "C carries a Retracts annotation (the sibling of Counters)"
        );
    }

    /// References that are NOT counter/retract relationships (e.g. `Corrects`,
    /// `Supersedes`), or references at CIDs not in the result set, leave every
    /// row's `counter_annotation` as `None` — only the load-bearing OD-AV-7
    /// relationships annotate.
    #[test]
    fn non_counter_references_leave_annotation_none() {
        let c = claim("did:plc:priya", "cidC", "github:a/a", "phil.x", 0.82);
        let k = claim_with_refs(
            "did:plc:sven",
            "cidK",
            "github:a/a",
            "phil.x",
            0.55,
            vec![
                // A non-counter relationship at C — must NOT annotate.
                ClaimReference {
                    ref_type: ReferenceType::Corrects,
                    cid: Cid("cidC".to_string()),
                },
                // A Counters reference at a CID NOT in the result — must NOT annotate.
                ClaimReference {
                    ref_type: ReferenceType::Counters,
                    cid: Cid("cid-absent".to_string()),
                },
            ],
        );

        let result = compose_results(vec![c, k], SearchDimension::Object);

        for (_did, rows) in &result.by_author {
            for row in rows {
                assert_eq!(
                    row.counter_annotation, None,
                    "neither Corrects nor a counter at an absent CID annotates any row"
                );
            }
        }
    }
}
