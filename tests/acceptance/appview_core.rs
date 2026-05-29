//! Slice-05 layer-2 acceptance — the pure `appview-domain` core's two
//! load-bearing trust primitives: the verify-before-index INGEST GATE and the
//! anti-merging-at-network-scale RESULT COMPOSITION.
//!
//! Layer 2 (in-memory acceptance — pure-core direct invocation, NO indexer
//! subprocess) per nw-tdd-methodology Layered Test Discipline matrix +
//! DD-AV-3. Sibling to slice-04's `scoring_core.rs` and slice-02's
//! `scraper_domain.rs`; same shape, same file role. The driving port here is
//! the PURE function signature (`appview_domain::ingest_decision` /
//! `appview_domain::compose_results`) — calling it directly IS port-to-port
//! testing at the domain layer (the function signature IS the public
//! interface).
//!
//! Per Mandate 9 (layer-dependent PBT mode): layers 1-2 may use PBT full.
//! The load-bearing `appview-domain` INVARIANTS are `@property` scenarios
//! runnable via proptest:
//!   - AVC-1 verify-before-index gate: `ingest_decision` returns `Index` IFF
//!     `claim_domain::verify` + CID-recompute BOTH pass; a tampered / unsigned
//!     / CID-mismatch record returns `Reject` and NEVER an `IndexedClaim`
//!     (WD-104 / I-AV-1 / KPI-AV-3; the SAME pure core, no second path).
//!   - AVC-2 attribution preservation: `compose_results` preserves EVERY
//!     author — `distinct_author_count == COUNT(DISTINCT author_did)` over the
//!     input; no row dropped or merged; identical-content-different-author
//!     rows stay separate (WD-103 / I-AV-2 / KPI-AV-2).
//!   - AVC-3 ingest determinism: same `(record, key)` -> byte-identical
//!     `IngestOutcome`; same rows -> byte-identical `NetworkSearchResult`
//!     (the by-construction reproducibility precondition; DESIGN 5.1 inv 4).
//!   - AVC-4 author derived from signed payload: a record that verifies is
//!     indexed with `IndexedClaim.author_did == record.raw_payload.author`
//!     byte-equal (attribution is DERIVED from the signed payload, never
//!     asserted separately; data-models.md "made concrete").
//! Plus example-pinned scenarios for the no-merged-row TYPE contract (AVC-5),
//! the counter-shown-not-applied rule (AVC-6; OD-AV-7 / I-AV-9), and the
//! empty-result near-match suggestion (AVC-7; US-AV-002 Example 4).
//!
//! These are the LOAD-BEARING trust primitives the whole slice-05 thesis rests
//! on (the two cardinal disprovers KPI-AV-2 + KPI-AV-3); pinning them as
//! generative properties at the cheap layer-2 boundary means the example-only
//! layer-3 subprocess tests (`appview_search.rs` + `indexer_ingest.rs`) only
//! need to verify the user-visible RENDERING + the real-I/O wiring of these
//! already-proven invariants.
//!
//! The EXHAUSTIVE per-arm unit coverage (each RejectReason arm, the multibase
//! decode boundary cases, mutation testing of ingest_decision + compose_results
//! + the claim-domain decode helper) is DELIVER's inner TDD loop in
//! `crates/appview-domain/src/`'s `#[cfg(test)] mod tests` block + the
//! `claim-domain` decode tests (out of DISTILL scope per DD-AV-7, symmetric
//! with slice-04 DD-GRAPH-7).
//!
//! Covers:
//! - US-AV-001: verified, attributed ingest (gate + anti-merging at ingest)
//! - US-AV-002: anti-merging at network scale (compose preserves every author)
//! - US-AV-004: verified-marker-is-universal (no unverified IndexedClaim exists)
//! - WD-104 / I-AV-1: verify-before-index reuses the pure core (property)
//! - WD-103 / I-AV-2: anti-merging in network aggregates (property + type)
//! - OD-AV-7 / I-AV-9: counter shown, never applied (example)
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_imports)]

use appview_domain::proptest_strategies::{arbitrary_indexed_claims, arbitrary_raw_records};
use appview_domain::{IngestOutcome, SearchDimension};
use chrono::{TimeZone, Utc};
use claim_domain::{Cid, Did, VerifyingKey};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use std::collections::HashSet;

// The `appview-domain` crate + its public ADTs (RawRecord [re-exported from
// ports], IngestOutcome, RejectReason, IndexedClaim [ports], NetworkResultRow,
// NetworkSearchResult, SearchDimension, CounterRef) and the pure entry points
// (`ingest_decision`, `compose_results`, `near_match_suggestion`) are
// scaffolded by DELIVER's first slice-05 step (the indexer-subsystem bootstrap)
// per component-boundaries.md §`crates/appview-domain`. Until then this file
// does not compile — that is the intended RED-ready state (DD-AV-13): once the
// crate's types exist with `todo!()` bodies, every `#[test]` here reaches its
// own `todo!()` (RED, not BROKEN).
//
// NOTE — unlike the subprocess-driven appview_search/indexer_ingest tests, this
// file invokes `appview_domain` directly (layer 2). It does NOT use the
// `support/mod.rs` TestEnv (no subprocess, no real indexer). Same pattern as
// slice-04's `scoring_core.rs` and slice-02's `scraper_domain.rs`.
//
// The test-support adversarial fixtures (`fixture_ingest_unsigned`,
// `fixture_ingest_tampered_signature`, `fixture_ingest_cid_mismatch`,
// `fixture_ingest_valid_signed`, the real-`z6Mk...` keypair) + the
// `RawRecordSpec` builders are materialized in
// `crates/test-support/src/fixtures_ingest.rs` by the same bootstrap step.

// =============================================================================
// US-AV-001 — the verify-before-index INGEST GATE (PROPERTY; I-AV-1 / WD-104)
// =============================================================================

/// AVC-1 / Property (Mandate 9 layer 2 PBT full): `ingest_decision` returns
/// `Index(_)` IF AND ONLY IF the signature verifies against the resolved key
/// AND the recomputed CID matches the published CID. A record that is unsigned,
/// has a tampered signature, or whose CID does not match returns `Reject(_)` —
/// NEVER an `IndexedClaim`. This IS the cardinal verified-before-index gate
/// (WD-104 / I-AV-1 / KPI-AV-3) and it reuses the SAME pure `claim_domain`
/// verify + compute_cid core (no second verification path).
///
///     forall (record, resolved_key):
///         ingest_decision(record, key) == Index(_)
///             <=>  claim_domain::verify(record.raw_payload, key) == Ok
///              &&  claim_domain::compute_cid(record.raw_payload) == record.published_cid
///
/// @property @us-av-001 @us-av-004 @i-av-1 @kpi-av-3 @release-gate
#[test]
fn appview_ingest_gate_indexes_iff_verified_and_cid_matches_property() {
    // Layer-2 @property (Mandate 9): pure-core direct invocation, NO indexer
    // subprocess. The driving port IS the pure `appview_domain::ingest_decision`
    // signature; we drive it over an arbitrary mix of valid + adversarial
    // records (unsigned / tampered-sig / cid-mismatch) paired with the resolved
    // verification key, and assert the iff:
    //
    //   - a record whose signature verifies AND whose recomputed CID matches
    //     the published CID  =>  Index(claim) with claim.author_did ==
    //     record.raw_payload.author and a NON-EMPTY claim.verified_against.
    //   - ANY adversarial record (unsigned / tampered-sig / cid-mismatch)
    //     =>  Reject(reason) and NO IndexedClaim is produced.
    //
    // This is the SAME pure-core decision the indexer runs at ingest and the
    // CLI renders as `[verified]` (DESIGN 5.1 inv 2: no second verification
    // path). A mutant that lets a tampered record through, or that fabricates a
    // verified_against on a rejected record, fails LOUDLY here at the cheap
    // layer-2 boundary.
    //
    // Universe (port-exposed observable surface of the gate): the IngestOutcome
    // discriminant (Index|Reject) + (on Index) claim.author_did,
    // claim.verified_against, claim.cid. NO internal field of the verify core.
    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_raw_records(), |(record, key)| {
            // ORACLE — the SAME pure core the gate must reuse (no second path):
            // a record may enter the index IFF its signature verifies against
            // the resolved key AND its recomputed CID matches the published CID.
            // `verify` takes the lower-level VerifyingKey; bridge the bytes (the
            // ADR-026 VerificationKey decode output wraps the same 32 pubkey
            // bytes the production gate bridges at its call site).
            let pubkey = VerifyingKey(key.0.clone());
            let verifies = claim_domain::verify(&record.raw_payload, &pubkey).is_ok();
            let canonical = claim_domain::canonicalize(&record.raw_payload.unsigned)
                .expect("a generated claim canonicalizes");
            let cid_matches =
                claim_domain::compute_cid(&canonical) == record.published_cid;
            let should_index = verifies && cid_matches;

            let outcome = appview_domain::ingest_decision(&record, &key);

            match outcome {
                IngestOutcome::Index(claim) => {
                    // The iff (=> direction): the gate indexed, so BOTH the
                    // signature AND the CID must have passed the pure core.
                    prop_assert!(
                        should_index,
                        "ingest_decision returned Index but the pure core \
                         disagrees (verifies={verifies}, cid_matches={cid_matches}) \
                         — the gate must NOT admit an unverified/mismatched record"
                    );
                    // On Index: attribution is DERIVED byte-equal from the
                    // SIGNED payload's author, never supplied out-of-band.
                    prop_assert_eq!(
                        &claim.author_did,
                        &record.raw_payload.unsigned.author_did,
                        "indexed author_did must equal the signed payload author byte-for-byte"
                    );
                    // On Index: the verified-marker key id is NEVER empty
                    // (the universal `[verified]` construction guarantee, WD-104).
                    prop_assert!(
                        !claim.verified_against.0.is_empty(),
                        "indexed claim.verified_against must never be empty (WD-104)"
                    );
                    // On Index: the indexed CID is the verified published CID.
                    prop_assert_eq!(
                        &claim.cid,
                        &record.published_cid,
                        "indexed cid must equal the verified published_cid"
                    );
                }
                IngestOutcome::Reject(_reason) => {
                    // The iff (<= direction): the gate rejected, so the pure
                    // core must agree the record was NOT both verified AND
                    // CID-matched — AND no IndexedClaim is produced (the Reject
                    // arm carries a RejectReason, never a claim, by type).
                    prop_assert!(
                        !should_index,
                        "ingest_decision rejected a record the pure core would \
                         admit (verifies={verifies}, cid_matches={cid_matches}) \
                         — a valid signed+matched record must be indexed"
                    );
                }
            }
            Ok(())
        })
        .unwrap();
}

/// AVC-3a / Property (Mandate 9 layer 2): `ingest_decision` is deterministic —
/// the same `(record, resolved_key)` yields a byte-identical `IngestOutcome`.
/// The reproducibility precondition for the verify gate (DESIGN 5.1 inv 4).
///
/// @property @us-av-001 @i-av-1
#[test]
fn appview_ingest_decision_is_deterministic_property() {
    // Layer-2 @property: call ingest_decision twice on the same input; the two
    // IngestOutcomes must be byte-identical (same discriminant; same
    // IndexedClaim or same RejectReason). A non-deterministic gate (e.g. a
    // clock read, a hash-map iteration order leaking into the decision) would
    // make the `[verified]` construction guarantee unsound.
    //
    // Universe: the full IngestOutcome value (discriminant + payload).
    //
    //     forall (record, key):
    //         ingest_decision(record, key) == ingest_decision(record, key)
    //
    // `IngestOutcome` derives `PartialEq` + `Debug` over its `Index(IndexedClaim)`
    // / `Reject(RejectReason)` arms, so this equality covers the discriminant AND
    // the full payload (the IndexedClaim's derived author / cid / confidence /
    // composed_at / verified_against, or the structured RejectReason) byte-for-
    // byte. Symmetric-property style (Hebert ch.3 Tier 1): applying the same pure
    // transformation twice yields the same value. The generator drives over the
    // valid AND every adversarial posture, so determinism is pinned across BOTH
    // gate arms. ingest_decision is clock-free / I/O-free / HashMap-order-free by
    // construction (02-01); this property PINS that — a future refactor that reads
    // a wall clock (e.g. `Utc::now()`) or lets a HashMap iteration order leak into
    // the decision fails LOUDLY here. Mirrors slice-04's
    // `scoring_score_is_deterministic_property`.
    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_raw_records(), |(record, key)| {
            let first = appview_domain::ingest_decision(&record, &key);
            let second = appview_domain::ingest_decision(&record, &key);
            prop_assert_eq!(
                first,
                second,
                "ingest_decision must be DETERMINISTIC: the same (record, resolved_key) must yield \
                 a byte-identical IngestOutcome (same discriminant; same IndexedClaim or same \
                 RejectReason) — the reproducibility precondition for the verify gate and the \
                 universal `[verified]` construction guarantee (DESIGN 5.1 inv 4). A clock read or \
                 a HashMap iteration-order leak in the decision would make this unsound."
            );
            Ok(())
        })
        .expect(
            "determinism invariant: ingest_decision(record, key) must equal a second call with the \
             SAME inputs for all generated valid + adversarial records",
        );
}

/// AVC-4 / Property (Mandate 9 layer 2): a record that verifies is indexed with
/// `IndexedClaim.author_did` EQUAL (byte-equal) to the `author` field of its
/// signed payload — attribution is DERIVED from the signed payload, never
/// asserted separately or supplied out-of-band (data-models.md "Invariant":
/// the row's attribution is derived from the signed payload).
///
/// @property @us-av-001 @i-av-2 @anti-merging
#[test]
fn appview_indexed_author_is_derived_from_signed_payload_property() {
    // Layer-2 @property: for every record that ingest_decision indexes, the
    // resulting IndexedClaim.author_did equals record.raw_payload.author
    // byte-for-byte. The attribution must come from the SIGNED bytes (which the
    // signature covers), never from the unsigned provenance (source_pds) — a
    // forgeable field. This is the type/derivation half of anti-merging at
    // ingest: an author can never be substituted or dropped between the signed
    // payload and the indexed row.
    //
    // Universe: claim.author_did vs record.raw_payload.author (port-exposed
    // boundary values).
    //
    //     forall (record, key) where ingest_decision indexes:
    //         IndexedClaim.author_did == record.raw_payload.unsigned.author_did
    //
    // STRENGTHENING (source_pds independence — criteria 2 & 3): attribution is
    // covered by the signature, so it comes from the SIGNED payload, NEVER from
    // the unsigned/forgeable provenance (`source_pds`). We PIN this by re-running
    // the SAME signed payload with a DIFFERENT `source_pds`: the gate must index
    // the SAME author_did. A mutant that sourced attribution from `source_pds`
    // (or substituted/dropped the author between the signed payload and the row)
    // fails LOUDLY here. Only the Index arm carries an author to assert (the
    // Reject arm carries a RejectReason, never a claim, by type) — so this is a
    // property over `arbitrary_raw_records()` filtered to records that VERIFY.
    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_raw_records(), |(record, key)| {
            match appview_domain::ingest_decision(&record, &key) {
                IngestOutcome::Index(claim) => {
                    // (1) Attribution is DERIVED byte-equal from the SIGNED
                    // payload's author — never asserted out-of-band.
                    prop_assert_eq!(
                        &claim.author_did,
                        &record.raw_payload.unsigned.author_did,
                        "indexed author_did must equal the signed payload author byte-for-byte \
                         (data-models.md Invariant: attribution derived from the signed payload)"
                    );

                    // (2)+(3) source_pds INDEPENDENCE: forge a DIFFERENT
                    // provenance on the SAME signed payload; the gate must still
                    // index the SAME author. The forgeable `source_pds` can NEVER
                    // be the attribution source, and the author can NEVER be
                    // substituted or dropped between the signed payload and the
                    // indexed row.
                    let mut forged = record.clone();
                    forged.source_pds =
                        format!("https://forged-relay.example.test/{}", record.source_pds);
                    prop_assert_ne!(
                        &forged.source_pds,
                        &record.source_pds,
                        "the forged provenance must actually differ (test self-check)"
                    );
                    match appview_domain::ingest_decision(&forged, &key) {
                        IngestOutcome::Index(forged_claim) => {
                            prop_assert_eq!(
                                &forged_claim.author_did,
                                &claim.author_did,
                                "indexed author_did must be INDEPENDENT of source_pds: the same \
                                 signed payload under a forged provenance must index the SAME \
                                 author (attribution comes from the SIGNED bytes, never the \
                                 unsigned/forgeable source_pds)"
                            );
                        }
                        IngestOutcome::Reject(reason) => {
                            // Changing only the unsigned provenance must NEVER
                            // flip a verified record's gate decision.
                            prop_assert!(
                                false,
                                "forging source_pds (an unsigned field) must NOT change the gate \
                                 decision: a record that indexed now Reject(ed) ({reason:?})"
                            );
                        }
                    }
                }
                // The Reject arm carries no author to derive — out of scope for
                // this derivation property (covered by AVC-1's gate iff).
                IngestOutcome::Reject(_reason) => {}
            }
            Ok(())
        })
        .unwrap();
}

// =============================================================================
// US-AV-002 — anti-merging at network scale (PROPERTY; I-AV-2 / WD-103)
// =============================================================================

/// AVC-2 / Property (Mandate 9 layer 2 PBT full): `compose_results` preserves
/// EVERY author. For an arbitrary non-empty `Vec<IndexedClaim>`,
/// `distinct_author_count` equals the number of distinct `author_did`s in the
/// input; no row is dropped; identical-content-different-author rows stay
/// separate; and there is NO API that returns a merged multi-author row. This
/// IS the cardinal anti-merging-at-network-scale invariant (WD-103 / I-AV-2 /
/// KPI-AV-2), the network-scale descendant of slice-03 I-FED-1 + slice-04
/// I-GRAPH-1/2.
///
///     forall rows:
///         let r = compose_results(rows, dimension);
///         r.distinct_author_count == rows.iter().map(|c| c.author_did).distinct().count()
///         && r.total_claims == rows.len()
///         && flatten(r.by_author).len() == rows.len()   // no row dropped/merged
///         && every input (author_did, cid) appears exactly once in the output
///
/// @property @us-av-002 @us-av-003 @us-av-006 @i-av-2 @kpi-av-2 @anti-merging @release-gate
#[test]
fn appview_compose_preserves_every_author_property() {
    // Layer-2 @property (Mandate 9): pure-core direct invocation. The driving
    // port IS the pure `appview_domain::compose_results` signature; we drive it
    // over an arbitrary non-empty IndexedClaim set spanning a small universe of
    // {subject in 3, object in 2, author in 3} (so the generated sets exercise
    // single-author, multi-author, and identical-(subject,object)-distinct-
    // author pairings) and assert the anti-merging invariant:
    //
    //   - distinct_author_count == COUNT(DISTINCT author_did) over the input
    //     (a COUNT over attributed rows, NEVER a merged aggregate).
    //   - total_claims == rows.len() (no claim dropped).
    //   - the flattened by_author rows == the input rows as a multiset (no row
    //     dropped, none invented, none merged) — every input (author_did, cid)
    //     appears EXACTLY once in exactly one author group.
    //   - two input rows with identical (subject, object) but distinct
    //     author_did land in DIFFERENT author groups (never collapsed).
    //
    // A mutant that GROUP-BYs object/subject (dropping author_did into a faceless
    // consensus count) fails LOUDLY here. There is no `NetworkSearchResult` API
    // that exposes a merged multi-author row to assert against — the absence is
    // the type-level guarantee (AVC-5).
    //
    // Universe (port-exposed observable surface of the composition):
    // result.distinct_author_count, result.total_claims, and the multiset of
    // (author_did, cid) flattened from result.by_author. NEVER an internal
    // grouping-map field.
    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_indexed_claims(), |rows| {
            // ORACLE — the obviously-correct reference for the anti-merging
            // invariants, computed DIRECTLY over the input rows (a COUNT over
            // attributed rows, never a merge). Hebert ch.3 Tier-1 "Modeling":
            // SUT (compose_results) vs simpler-but-correct reference (these
            // direct over-the-input computations).
            let expected_total = rows.len() as u32;
            let expected_distinct_authors: HashSet<Did> =
                rows.iter().map(|c| c.author_did.clone()).collect();
            // The input as a multiset of (author_did, cid): every generated row
            // carries a DISTINCT cid, so this is the exact set the output must
            // reproduce (no row dropped, none invented, none merged).
            let input_pairs: HashSet<(Did, Cid)> = rows
                .iter()
                .map(|c| (c.author_did.clone(), c.cid.clone()))
                .collect();
            prop_assert_eq!(
                input_pairs.len(),
                rows.len(),
                "test self-check: the generator must emit DISTINCT (author_did, cid) per row \
                 so the multiset equivalence below is well-defined"
            );

            let result = appview_domain::compose_results(rows.clone(), SearchDimension::Object);

            // (Criterion 1) distinct_author_count == COUNT(DISTINCT author_did)
            // over the input — a COUNT over attributed rows, NEVER a merged
            // aggregate. A mutant that GROUP-BYs object/subject into a faceless
            // count fails LOUDLY here.
            prop_assert_eq!(
                result.distinct_author_count,
                expected_distinct_authors.len() as u32,
                "distinct_author_count must equal COUNT(DISTINCT author_did) over the input \
                 (a COUNT over attributed rows, never a merged aggregate) — WD-103 / I-AV-2"
            );

            // (Criterion 2a) total_claims == rows.len() (no claim dropped).
            prop_assert_eq!(
                result.total_claims,
                expected_total,
                "total_claims must equal rows.len() — no claim may be dropped or merged away"
            );

            // (Criterion 2b) the flattened by_author rows == the input rows as a
            // MULTISET: every input (author_did, cid) appears EXACTLY once, in
            // exactly one author group. We assert (i) the flattened count equals
            // the input count (none dropped, none invented) and (ii) the set of
            // (author_did, cid) pairs is identical (none merged, none renamed).
            let flattened: Vec<(Did, Cid)> = result
                .by_author
                .iter()
                .flat_map(|(_did, group)| {
                    group
                        .iter()
                        .map(|row| (row.author_did.clone(), row.cid.clone()))
                })
                .collect();
            prop_assert_eq!(
                flattened.len(),
                rows.len(),
                "the flattened by_author rows must equal the input rows in COUNT (no row \
                 dropped, none invented, none merged across authors)"
            );
            let output_pairs: HashSet<(Did, Cid)> = flattened.iter().cloned().collect();
            prop_assert_eq!(
                output_pairs.len(),
                flattened.len(),
                "each (author_did, cid) must appear EXACTLY once across the whole result \
                 (no duplication; the multiset is in fact a set of distinct rows)"
            );
            prop_assert_eq!(
                &output_pairs,
                &input_pairs,
                "the flattened by_author (author_did, cid) multiset must EQUAL the input rows \
                 — every input row preserved, none merged, none invented"
            );

            // (Criterion 2c) every row lands in the group keyed by ITS OWN
            // author_did — never collapsed under a foreign / faceless key. This is
            // what makes the by_author grouping attribution-preserving rather than
            // a relabeling.
            for (group_did, group) in &result.by_author {
                for row in group {
                    prop_assert_eq!(
                        &row.author_did,
                        group_did,
                        "every row must live under its OWN author_did group key — never \
                         collapsed under a foreign or merged key (anti-merging, WD-103)"
                    );
                }
            }

            // (Criterion 3) two input rows with identical (subject, object) but
            // DISTINCT author_did must land in DIFFERENT author groups (never
            // collapsed into a merged multi-author row). We verify the structural
            // guarantee directly: the by_author group KEYS are exactly the distinct
            // authors of the input and each appears ONCE — so no two distinct
            // authors can ever share a group, regardless of identical content.
            let group_keys: Vec<Did> = result
                .by_author
                .iter()
                .map(|(did, _)| did.clone())
                .collect();
            let unique_group_keys: HashSet<Did> = group_keys.iter().cloned().collect();
            prop_assert_eq!(
                unique_group_keys.len(),
                group_keys.len(),
                "each author_did must key AT MOST ONE group — distinct authors with \
                 identical (subject, object) content can never be collapsed into one \
                 multi-author row (no merged-row API exists to violate)"
            );
            prop_assert_eq!(
                &unique_group_keys,
                &expected_distinct_authors,
                "the by_author group keys must be EXACTLY the distinct authors of the input"
            );

            Ok(())
        })
        .unwrap();
}

/// AVC-3b / Property (Mandate 9 layer 2): `compose_results` is deterministic —
/// the same `(rows, dimension)` yields a byte-identical `NetworkSearchResult`
/// (the by-construction reproducibility precondition; DESIGN 5.1 inv 4). The
/// share-link contract (US-AV-006) rests on this: re-running the same query over
/// the same rows yields the same per-author result.
///
/// @property @us-av-002 @us-av-006 @i-av-8
#[test]
fn appview_compose_results_is_deterministic_property() {
    // Layer-2 @property: call compose_results twice on the same (rows,
    // dimension); the two NetworkSearchResults must be byte-identical
    // (same by_author ordering, same counts, same suggestion). This underpins
    // the `--share` "re-running the query yields the same result" contract
    // (the link encodes the query, not a snapshot — I-AV-8) AND the
    // determinism the wire-shape relies on.
    //
    // Universe: the full NetworkSearchResult value.
    //
    //     forall (rows, dim):
    //         compose_results(rows, dim) == compose_results(rows, dim)
    //
    // `NetworkSearchResult` derives `PartialEq` + `Debug` over its `by_author`
    // (the per-author groups + their ordering), `distinct_author_count`,
    // `total_claims`, and `suggestion`, so this equality covers the FULL value
    // byte-for-byte (same by_author ordering, same counts, same suggestion).
    // Symmetric-property style (Hebert ch.3 Tier 1): applying the same pure
    // transformation twice yields the same value. We drive over the arbitrary
    // IndexedClaim set (`arbitrary_indexed_claims()`, 02-04) crossed with an
    // arbitrary `SearchDimension`, so determinism is pinned across the whole
    // {single-author, multi-author, identical-content-distinct-author} × {Object,
    // Contributor, Subject} space. compose_results is clock-free / I/O-free by
    // construction (02-04: BTreeMap-keyed author grouping → stable author
    // ordering, no HashMap-iteration-order leak; rows stable-sorted by cid within
    // each group; constant `suggestion`). This property PINS that — a future
    // refactor that swapped the BTreeMap for a HashMap (leaking iteration order
    // into `by_author`) or read a wall clock would fail LOUDLY here. Mirrors the
    // sibling AVC-3a (`appview_ingest_decision_is_deterministic_property`).
    let dimension = prop_oneof![
        Just(SearchDimension::Object),
        Just(SearchDimension::Contributor),
        Just(SearchDimension::Subject),
    ];
    let mut runner = TestRunner::default();
    runner
        .run(&(arbitrary_indexed_claims(), dimension), |(rows, dim)| {
            let first = appview_domain::compose_results(rows.clone(), dim);
            let second = appview_domain::compose_results(rows.clone(), dim);
            prop_assert_eq!(
                first,
                second,
                "compose_results must be DETERMINISTIC: the same (rows, dimension) must yield a \
                 byte-identical NetworkSearchResult (same by_author ordering, same counts, same \
                 suggestion) — the by-construction reproducibility precondition (DESIGN 5.1 inv 4). \
                 The `--share` contract (US-AV-006 / I-AV-8) rests on this: the share link encodes \
                 the QUERY, not a snapshot, so re-running the same query over the same rows must \
                 yield the same per-author result. A HashMap-iteration-order leak in the by_author \
                 grouping, or a wall-clock read, would make this unsound."
            );
            Ok(())
        })
        .expect(
            "determinism invariant: compose_results(rows, dim) must equal a second call with the \
             SAME inputs for all generated IndexedClaim sets and search dimensions",
        );
}

// =============================================================================
// US-AV-002 — anti-merging TYPE contract (example; I-AV-2 type layer)
// =============================================================================

/// AVC-5 (example; Gate type-level / WD-103): two `IndexedClaim`s with identical
/// (subject, object, predicate, confidence) but DISTINCT non-empty `author_did`s
/// compose to a `NetworkSearchResult` with `by_author` holding BOTH authors as
/// SEPARATE groups and `distinct_author_count == 2` — the type-level half of the
/// three-layer anti-merging enforcement (DESIGN §10). There is NO struct field
/// or method that represents the two claims combined; the absence is the design.
///
/// @us-av-002 @i-av-2 @anti-merging @gate-type
#[test]
fn appview_two_identical_content_distinct_author_claims_compose_to_two_groups() {
    // Example-pinned (the canonical US-AV-002 Example 2 deno/dependency-pinning
    // pairing): Priya@0.70 and Sven@0.65 both assert github:denoland/deno
    // embodies dependency-pinning. compose_results yields by_author with TWO
    // entries (did:plc:priya-test, did:plc:sven-test), each a single-row group;
    // distinct_author_count == 2; total_claims == 2. Asserts that the
    // NetworkSearchResult TYPE has no merged-author row to produce (the
    // load-bearing absence, WD-103) — the per-author structure is the only shape.
    //
    // Universe: result.by_author author-set {priya, sven},
    // result.distinct_author_count (2), result.total_claims (2).
    todo!(
        "DELIVER (slice-05): compose two identical-(subject,object) claims by \
         did:plc:priya-test (0.70) and did:plc:sven-test (0.65); assert \
         by_author has both as separate single-row groups, \
         distinct_author_count==2, total_claims==2; no merged-row shape exists."
    );
}

// =============================================================================
// US-AV-004 — verified marker is universal (example; I-AV-1)
// =============================================================================

/// AVC-7 (example; US-AV-004 boundary): there is NO `IndexedClaim` /
/// `NetworkResultRow` construction with an empty `verified_against`. Because
/// `ingest_decision` only ever produces an `IndexedClaim` on the `Index` arm
/// (which always sets a non-empty `verified_against`), every row that reaches
/// `compose_results` carries the verified marker by construction — there is no
/// `[unverified]` state to render (US-AV-004 Example 3).
///
/// @us-av-004 @i-av-1 @verified-marker
#[test]
fn appview_every_composed_row_carries_a_nonempty_verified_against() {
    // Example-pinned: build a NetworkSearchResult only from IndexedClaims that
    // came out of ingest_decision's Index arm; assert EVERY NetworkResultRow has
    // verified_against != "". The construction guarantee (DESIGN 5.2 #3): the
    // store/result has no `verified BOOLEAN`; verified_against NOT NULL records
    // that every row WAS verified, and the renderer reads it as a universal
    // `[verified]` marker. There is no mixed-trust list to reason about.
    //
    // Universe: the set { row.verified_against for row in flatten(by_author) };
    // assert none is empty.
    todo!(
        "DELIVER (slice-05): build NetworkSearchResult from Index-arm \
         IndexedClaims only; assert every NetworkResultRow.verified_against != \
         \"\". No [unverified] state exists by construction (US-AV-004 Ex 3)."
    );
}

// =============================================================================
// OD-AV-7 — counter shown, NOT applied (example; I-AV-9)
// =============================================================================

/// AVC-6 (example; OD-AV-7 / I-AV-9): a claim that is COUNTERED by another
/// indexed claim is STILL present in `compose_results`' output; the counter
/// relationship is added as a `NetworkResultRow.counter_annotation`, and the
/// countered row is NEVER removed, filtered, or down-weighted. The counter is
/// SHOWN, never applied (WD-119 default; mirrors slice-04 WD-85).
///
/// @us-av-002 @od-av-7 @i-av-9
#[test]
fn appview_countered_claim_still_appears_with_annotation() {
    // Example-pinned: a claim C is referenced by a later claim K with
    // ref_type=counters (both indexed). compose_results returns C as a row whose
    // counter_annotation is Some(CounterRef { by: K.author_did, cid: K.cid }),
    // and C is STILL in the result (total_claims unchanged, C present). A code
    // path that filtered or down-weighted C would fail here — the counter is an
    // annotation, never a filter (the load-bearing OD-AV-7 default).
    //
    // Universe: presence of C in flatten(by_author), C.counter_annotation
    // (Some), result.total_claims (unchanged by the counter relationship).
    todo!(
        "DELIVER (slice-05): given claim C countered by indexed claim K, \
         compose_results returns C present with counter_annotation == \
         Some(CounterRef{{by: K.author, cid: K.cid}}); C is NOT removed/filtered/ \
         down-weighted (OD-AV-7 shown-not-applied)."
    );
}

// =============================================================================
// US-AV-002 — empty-result near-match suggestion (example; US-AV-002 Ex 4)
// =============================================================================

/// AVC-8 (example; US-AV-002 Example 4): `near_match_suggestion` over the set of
/// known object values returns the closest match (edit distance) for a typo'd
/// query, and `None` when nothing is within the threshold. Drives the empty-
/// result "Did you mean ...?" line the CLI renders for an unknown philosophy URI
/// (exit code 0, a valid empty result, not an error).
///
/// @us-av-002 @suggestion @edge
#[test]
fn appview_near_match_suggestion_finds_closest_known_object() {
    // Example-pinned: known = [..., "org.openlore.philosophy.reproducible-builds",
    // ...]; a typo'd query "org.openlore.philosophy.reproducable-builds" returns
    // Some("org.openlore.philosophy.reproducible-builds"); a query with no close
    // match returns None. The pure helper feeds the CLI's empty-result line; the
    // CLI then exits 0 (a valid empty result, US-AV-002 Ex 4) — NOT a usage error.
    //
    // Universe: the Option<String> suggestion returned.
    todo!(
        "DELIVER (slice-05): near_match_suggestion(typo, known) returns the \
         closest known object by edit distance (the US-AV-002 Ex4 'Did you \
         mean...?'); returns None when nothing is within threshold."
    );
}

// =============================================================================
// Generators (proptest) — materialized by DELIVER's bootstrap step
// =============================================================================

// Generator for an arbitrary mix of valid + adversarial `RawRecord`s paired
// with the resolved `VerificationKey`, over the real-`z6Mk...` test keypair.
// Used by AVC-1 (gate iff) + AVC-3a (determinism) + AVC-4 (author derivation).
// Bodies materialized by DELIVER from `fixtures_ingest.rs`.
//
// Distribution: ~50% valid signed records, ~50% adversarial split across
// unsigned / tampered-signature / cid-mismatch — so the gate's Index AND
// Reject arms are both exercised on every run.
//
// (signature scaffolded; DELIVER fills the proptest Strategy body)

// Generator for an arbitrary NON-EMPTY `Vec<IndexedClaim>` over a small bounded
// universe (3 subjects x 2 objects x 3 authors, confidence in `[0.0, 1.0]`,
// every `verified_against` non-empty) so generated sets exercise single-author,
// multi-author, and identical-content-distinct-author pairings. Used by AVC-2
// (preserve every author) + AVC-3b (compose determinism). Bodies materialized
// by DELIVER.
//
// (signature scaffolded; DELIVER fills the proptest Strategy body)
