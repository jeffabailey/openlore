//! `ingest` — the PURE verify-before-index gate (WD-104 / I-AV-1).
//!
//! [`ingest_decision`] is the single deterministic `RawRecord -> IngestOutcome`
//! decision. It reuses `claim_domain::verify` + `compute_cid` — the SAME pure
//! core the CLI uses, NO second verification path (WD-104). A record is admitted
//! (`Index`) ONLY when:
//!
//! - its signature verifies against the resolved `VerificationKey`, AND
//! - its recomputed CID matches the published CID, AND
//! - its author (from the SIGNED payload) is carried into the `IndexedClaim`.
//!
//! Anything else is `Reject`ed with a structured [`RejectReason`]. The body is
//! intentionally `todo!()` at the 01-01 bootstrap; the gate behavior is driven
//! by the Phase 02+ ingest scenarios (KPI-AV-3 `indexer_rejects_unverified_claim`)
//! split into its own module for mutation-test clarity (D-D40).
//
// SCAFFOLD: true

use claim_domain::VerificationKey;

use crate::{IngestOutcome, RawRecord};

/// The PURE verify-before-index gate. Calls `claim_domain::verify` +
/// `compute_cid` (the SAME pure core; NO second verification path, WD-104).
/// Deterministic; no I/O.
///
/// Returns [`IngestOutcome::Index`] with the verified, author-attributed claim
/// iff the signature verifies AND the recomputed CID matches the published CID;
/// otherwise [`IngestOutcome::Reject`] with the structured reason.
pub fn ingest_decision(_record: &RawRecord, _resolved_key: &VerificationKey) -> IngestOutcome {
    // SCAFFOLD: true — behavior driven by the Phase 02+ ingest-gate scenarios.
    todo!("ingest_decision — driven by the verify-before-index acceptance scenarios (WD-104)")
}
