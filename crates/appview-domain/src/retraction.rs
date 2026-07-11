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
//! DISTILL RED scaffold (Mandate 7): the signature + `RetractionPartition` type
//! land here with a `panic!` body marked `// SCAFFOLD: true`, so the DISTILL
//! acceptance suite classifies as RED (assertion/panic on missing behavior), NOT
//! BROKEN (compile/import error). DELIVER replaces the body one scenario at a
//! time (RED→GREEN→COMMIT) and pins the property set in `crates/appview-domain`.
//
// SCAFFOLD: true

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
    // SCAFFOLD: DELIVER owns the body (RED→GREEN→COMMIT per scenario). Kept as a
    // panic (not `todo!()`/`unimplemented!()`) so the DISTILL RED gate classifies
    // MISSING_FUNCTIONALITY, and the exact input type (owned `Vec` vs borrowed
    // slice) is DELIVER's Q-DELIVER-RF-1 to finalize against the two call sites.
    let _ = (rows, hide_retracted);
    panic!(
        "partition_retracted: not yet implemented — RED scaffold \
         (ADR-060 D-RF-D2/D3/D4/D5/D6; DELIVER owns the body)"
    )
}
