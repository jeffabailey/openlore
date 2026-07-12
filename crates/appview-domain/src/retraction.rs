//! `retraction` — the PURE retraction-aware view filter (slice-01 CLI +
//! slice-02 viewer; ADR-060 / feature `retraction-aware-search-filter`).
//!
//! [`partition_retracted`] is the SINGLE pure decision both surfaces
//! (`openlore search --hide-retracted`, `GET /search?hide_retracted=1`)
//! invoke on the RAW attributed rows ([`ports::NetworkResultRowRaw`], which
//! retain the full `references` graph — NOT `compose_results`' lossy
//! single-slot `counter_annotation`, ADR-060 §subtlety-1). It is opt-in,
//! non-destructive, and self-disclosing:
//!
//! - `hide_retracted == false` ⇒ `survivors == rows` (unchanged, original
//!   order + verbatim confidence) and `hidden_count == 0` — the byte-identical
//!   default guard (I-RF-1 / D-RF-D6).
//! - `hide_retracted == true` ⇒ every AUTHOR-SELF-RETRACTED claim C AND its
//!   same-author retraction marker K are removed; survivors keep their original
//!   relative order + verbatim confidence (I-RF-2 / D-RF-D6). `Counters` and any
//!   different-author `Retracts` NEVER hide (D-3 / I-RF-4 — no heckler's veto).
//!
//! Self-retraction rule (D-RF-D3, literal): C is author-self-retracted ⟺ ∃ a
//! row K in the set with `K.author_did == C.author_did` carrying a reference
//! `{ ref_type == Retracts, cid == C.cid }`.
//!
//! Retraction EVENT (D-RF-D4): the withdrawn original C AND its same-author
//! marker K are ONE event, both hidden together.
//!
//! `hidden_count` = retraction EVENTS (`|{ C author-self-retracted }|`), NOT the
//! raw rows removed (D-RF-D5) — the honest, user-meaningful unit ("2 retracted
//! claim(s) hidden" ⇔ two withdrawals). This refines the DISCUSS `len(unfiltered)
//! − len(survivors)` note, which double-counts once the marker row is understood
//! as a separate indexed row.
//!
//! NO I/O. NO async. NO index re-query (I-RF-5). The composition roots (the
//! `cli` search verb; `adapter-http-viewer`) wire the effect shell + the count
//! disclosure around this pure decision.
//!
//! Implementation shape (DELIVER 01-01): the decision is four small, named pure
//! predicates over the RAW `references` graph — [`is_own_retraction_marker`] (the
//! literal D-RF-D3 marker shape), [`is_self_retracted`] (∃ such a same-author
//! marker for this original), [`self_retraction_events`] (the distinct withdrawn
//! originals keyed by `(author_did, cid)` — the `hidden_count` unit, D-RF-D5), and
//! [`is_withdrawn`] (drop the original AND its same-author marker as ONE event,
//! D-RF-D4). The public [`partition_retracted`] wires them: identity when
//! `hide_retracted == false`, else a drop-only, order-preserving filter.

use std::collections::HashSet;

use claim_domain::{Cid, Did, ReferenceType};
use ports::NetworkResultRowRaw;

/// The result of one [`partition_retracted`] pass: the surviving rows (original
/// order, verbatim confidence) + the disclosed retraction-EVENT count.
///
/// `PartialEq` (not `Eq`) because [`NetworkResultRowRaw`] carries an `f64`
/// confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct RetractionPartition {
    /// The rows to render — original relative order preserved, each row's
    /// confidence carried through verbatim (I-RF-2 / D-5). Equals the input
    /// (unchanged) when `hide_retracted == false`.
    pub survivors: Vec<NetworkResultRowRaw>,
    /// The number of AUTHOR-SELF-RETRACTED claims hidden — retraction EVENTS,
    /// NOT raw rows removed (D-RF-D5). `0` when `hide_retracted == false` or when
    /// nothing matched.
    pub hidden_count: u32,
}

/// The PURE retraction-aware view filter (ADR-060 D-RF-D2..D6).
///
/// When `hide_retracted` is `false`, returns the rows unchanged with
/// `hidden_count == 0` (the byte-identical default guard, I-RF-1). When `true`,
/// removes every author-self-retracted claim C AND its same-author retraction
/// marker K (one EVENT), preserving the survivors' original order + verbatim
/// confidence, and reports `hidden_count` as the number of EVENTS. `Counters` and
/// different-author `Retracts` never hide (D-3 / I-RF-4).
///
/// Total + deterministic; no I/O. Operates on the RAW rows (full `references`
/// graph), never on `compose_results`' lossy `counter_annotation`.
pub fn partition_retracted(
    rows: Vec<NetworkResultRowRaw>,
    hide_retracted: bool,
) -> RetractionPartition {
    // I-RF-1 / D-RF-D6: the opt-out path is the byte-identical default guard —
    // rows pass through untouched, nothing disclosed.
    if !hide_retracted {
        return RetractionPartition {
            survivors: rows,
            hidden_count: 0,
        };
    }

    let events = self_retraction_events(&rows);
    // D-RF-D5: the disclosed unit is EVENTS (distinct withdrawn originals), not the
    // raw rows removed (~2× once each event's marker is counted).
    let hidden_count = events.len() as u32;
    // D-RF-D4 + I-RF-2: drop-only, order-preserving — survivors keep their original
    // relative order and verbatim fields/confidence.
    let survivors = rows
        .into_iter()
        .filter(|row| !is_withdrawn(row, &events))
        .collect();

    RetractionPartition {
        survivors,
        hidden_count,
    }
}

/// True when `marker` is the literal D-RF-D3 self-retraction marker for
/// `(target_author, target_cid)`: a SAME-author row carrying a `{ Retracts,
/// target_cid }` reference. A different-author `Retracts` or any `Counters` is NOT
/// a self-retraction marker (no heckler's veto — I-RF-4).
fn is_own_retraction_marker(
    marker: &NetworkResultRowRaw,
    target_author: &Did,
    target_cid: &Cid,
) -> bool {
    &marker.author_did == target_author
        && marker.references.iter().any(|reference| {
            reference.ref_type == ReferenceType::Retracts && &reference.cid == target_cid
        })
}

/// True when `original` is author-self-retracted (D-RF-D3): some row in the set is
/// a same-author `Retracts` marker naming `original`'s OWN cid.
fn is_self_retracted(original: &NetworkResultRowRaw, rows: &[NetworkResultRowRaw]) -> bool {
    rows.iter()
        .any(|marker| is_own_retraction_marker(marker, &original.author_did, &original.cid))
}

/// The set of author-self-retraction EVENTS, keyed by the withdrawn original's
/// `(author_did, cid)` — one entry per distinct withdrawn original C (D-RF-D5).
/// `|events|` is the disclosed `hidden_count`; the event's original + marker rows
/// both drop but count once.
fn self_retraction_events(rows: &[NetworkResultRowRaw]) -> HashSet<(Did, Cid)> {
    rows.iter()
        .filter(|original| is_self_retracted(original, rows))
        .map(|original| (original.author_did.clone(), original.cid.clone()))
        .collect()
}

/// True when `row` must be hidden for a self-retraction event — EITHER it is a
/// withdrawn original C (its own `(author, cid)` is an event) OR it is the
/// same-author marker K naming a withdrawn original (original + marker are ONE
/// event, dropped together — D-RF-D4). Third-party counter/retract rows are never
/// withdrawn (D-3 / I-RF-4).
fn is_withdrawn(row: &NetworkResultRowRaw, events: &HashSet<(Did, Cid)>) -> bool {
    let is_withdrawn_original = events.contains(&(row.author_did.clone(), row.cid.clone()));
    let is_marker_for_event = row.references.iter().any(|reference| {
        reference.ref_type == ReferenceType::Retracts
            && events.contains(&(row.author_did.clone(), reference.cid.clone()))
    });
    is_withdrawn_original || is_marker_for_event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proptest_strategies::arbitrary_raw_rows_with_retractions;
    use chrono::{TimeZone, Utc};
    use claim_domain::{Cid, ClaimReference, Did, KeyId, ReferenceType};
    use proptest::prelude::*;

    const APP: &str = "#org.openlore.application";

    /// Build one raw row for the crafted example corpora. `references` carries the
    /// typed retraction/counter graph the predicate reads off the RAW rows.
    fn row(
        author: &str,
        cid: &str,
        confidence: f64,
        references: Vec<ClaimReference>,
    ) -> NetworkResultRowRaw {
        NetworkResultRowRaw {
            author_did: Did(format!("{author}{APP}")),
            cid: Cid(cid.to_string()),
            subject: "github:denoland/deno".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.dependency-pinning".to_string(),
            confidence,
            composed_at: Utc.with_ymd_and_hms(2026, 5, 26, 12, 0, 0).unwrap(),
            verified_against: KeyId(format!("{author}{APP}")),
            evidence: vec!["https://example.test/evidence".to_string()],
            references,
        }
    }

    fn retracts(cid: &str) -> ClaimReference {
        ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid: Cid(cid.to_string()),
        }
    }

    fn counters(cid: &str) -> ClaimReference {
        ClaimReference {
            ref_type: ReferenceType::Counters,
            cid: Cid(cid.to_string()),
        }
    }

    /// True when `sub` is an order-preserving subsequence of `full`, comparing rows
    /// by full value (so it also witnesses verbatim confidence/field preservation).
    fn is_verbatim_subsequence(sub: &[NetworkResultRowRaw], full: &[NetworkResultRowRaw]) -> bool {
        let mut cursor = full.iter();
        sub.iter().all(|s| cursor.any(|f| f == s))
    }

    /// Recompute the self-retraction predicate directly over a row set (test oracle
    /// for the "nothing self-retracted survives" invariant).
    fn self_retracted_in(c: &NetworkResultRowRaw, set: &[NetworkResultRowRaw]) -> bool {
        set.iter().any(|k| {
            k.author_did == c.author_did
                && k.references
                    .iter()
                    .any(|r| r.ref_type == ReferenceType::Retracts && r.cid == c.cid)
        })
    }

    // -------------------------------------------------------------------------
    // Property tests (structural invariants — no oracle needed)
    // -------------------------------------------------------------------------

    proptest! {
        /// (1) IDENTITY (I-RF-1 / D-RF-D6): `hide_retracted == false` ⇒ survivors
        /// equal the input verbatim (order + values) and `hidden_count == 0`.
        #[test]
        fn identity_when_not_hiding(rows in arbitrary_raw_rows_with_retractions()) {
            let expected = rows.clone();
            let partition = partition_retracted(rows, false);
            prop_assert_eq!(partition.hidden_count, 0);
            prop_assert_eq!(partition.survivors, expected);
        }

        /// (2)+(3)+(4) survivors ⊆ input, order-preserving, fields/confidence
        /// verbatim (I-RF-2): survivors are a value-equal ordered subsequence.
        #[test]
        fn survivors_are_verbatim_ordered_subsequence(rows in arbitrary_raw_rows_with_retractions()) {
            let original = rows.clone();
            let partition = partition_retracted(rows, true);
            prop_assert!(is_verbatim_subsequence(&partition.survivors, &original));
        }

        /// (5) IDEMPOTENT: re-running the filter on the survivors removes nothing
        /// more and discloses `hidden_count == 0`.
        #[test]
        fn hiding_is_idempotent(rows in arbitrary_raw_rows_with_retractions()) {
            let first = partition_retracted(rows, true);
            let second = partition_retracted(first.survivors.clone(), true);
            prop_assert_eq!(&second.survivors, &first.survivors);
            prop_assert_eq!(second.hidden_count, 0);
        }

        /// No author-self-retracted original survives the hide pass (the core
        /// correctness invariant — D-RF-D3/D4).
        #[test]
        fn no_self_retracted_original_survives(rows in arbitrary_raw_rows_with_retractions()) {
            let survivors = partition_retracted(rows, true).survivors;
            for c in &survivors {
                prop_assert!(
                    !self_retracted_in(c, &survivors),
                    "a self-retracted original must not survive --hide-retracted"
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // Example tests (counting semantics + heckler's-veto — crafted corpora)
    // -------------------------------------------------------------------------

    /// (6) `hidden_count` = distinct self-retraction EVENTS (D-RF-D5): ONE
    /// self-retraction (original C + same-author marker K = 2 rows) ⇒
    /// `hidden_count == 1`, NOT 2; both C and K drop; the standing row survives.
    #[test]
    fn hidden_count_is_events_not_rows() {
        let standing = row("did:plc:sven-test", "bafystanding", 0.6, vec![]);
        let original = row("did:plc:priya-test", "bafyorig", 0.8, vec![]);
        let marker = row(
            "did:plc:priya-test",
            "bafymark",
            1.0,
            vec![retracts("bafyorig")],
        );

        let partition = partition_retracted(vec![standing.clone(), original, marker], true);

        assert_eq!(
            partition.hidden_count, 1,
            "one withdrawal event, not two rows"
        );
        assert_eq!(
            partition.survivors,
            vec![standing],
            "only the standing row survives"
        );
    }

    /// Two distinct self-retractions ⇒ `hidden_count == 2` (distinct events).
    #[test]
    fn distinct_events_accumulate_the_count() {
        let rows = vec![
            row("did:plc:priya-test", "bafyA", 0.8, vec![]),
            row("did:plc:priya-test", "bafyAr", 1.0, vec![retracts("bafyA")]),
            row("did:plc:sven-test", "bafyB", 0.7, vec![]),
            row("did:plc:sven-test", "bafyBr", 1.0, vec![retracts("bafyB")]),
        ];

        let partition = partition_retracted(rows, true);

        assert_eq!(partition.hidden_count, 2);
        assert!(partition.survivors.is_empty(), "both events fully hidden");
    }

    /// (7) SELF-RETRACTION DOMINATES a co-present third-party counter (Earned-Trust
    /// #1 / anti-lossy): C is self-retracted by its author AND countered by a third
    /// party. C MUST be hidden (the lossy `counter_annotation` trap would mask it);
    /// the third-party counter row itself stays SHOWN (D-3 — no third-party removal).
    #[test]
    fn self_retraction_dominates_a_co_present_third_party_counter() {
        let original = row("did:plc:priya-test", "bafyorig", 0.8, vec![]);
        let self_marker = row(
            "did:plc:priya-test",
            "bafymark",
            1.0,
            vec![retracts("bafyorig")],
        );
        let third_party_counter = row(
            "did:plc:sven-test",
            "bafycounter",
            0.5,
            vec![counters("bafyorig")],
        );

        let partition = partition_retracted(
            vec![original, self_marker, third_party_counter.clone()],
            true,
        );

        assert_eq!(partition.hidden_count, 1);
        assert_eq!(
            partition.survivors,
            vec![third_party_counter],
            "C + its self-marker are hidden; the third-party counter stays shown"
        );
    }

    /// (8) NO HECKLER'S VETO (D-3 / I-RF-4): a third-party `Counters` AND a
    /// DIFFERENT-author `Retracts` targeting C never hide C — everything survives
    /// and `hidden_count == 0`.
    #[test]
    fn third_party_counter_or_different_author_retract_never_hides() {
        let original = row("did:plc:priya-test", "bafyorig", 0.8, vec![]);
        let foreign_retract = row(
            "did:plc:sven-test",
            "bafyfr",
            1.0,
            vec![retracts("bafyorig")],
        );
        let foreign_counter = row(
            "did:plc:rachel-test",
            "bafyfc",
            0.5,
            vec![counters("bafyorig")],
        );
        let rows = vec![original, foreign_retract, foreign_counter];

        let partition = partition_retracted(rows.clone(), true);

        assert_eq!(
            partition.hidden_count, 0,
            "no self-retraction ⇒ nothing hidden"
        );
        assert_eq!(
            partition.survivors, rows,
            "no heckler's veto: every row survives"
        );
    }
}
