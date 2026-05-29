//! `FakeIngestSource` — a bounded fixture `IngestSourcePort` test double.
//!
//! WHY-NEW-FILE: crates/test-support/src/fake_ingest_source.rs
//!   CLOSEST-EXISTING: crates/test-support/src/fake_peer_pds.rs
//!   EXTENSION-COST: fake_peer_pds.rs implements the peer-PDS read traits over an
//!     HTTP XRPC server (listRecords/getRecord/resolveDid); FakeIngestSource
//!     implements the DISTINCT `ports::IngestSourcePort` (async `enumerate` +
//!     `probe`) and validates ingest inputs — folding it in would mix two
//!     unrelated port contracts in one module.
//!   PARALLEL-RATIONALE: IngestSourcePort is a different trait with a different
//!     method surface and a different validation contract (DD-AV-2 reject-gate);
//!     the established convention is one fake module per external system
//!     (fake_pds / fake_peer_pds / fake_github), so the ingest source gets its own.
//!
//! Slice-05 introduces the FIRST adversarial-input EXTERNAL boundary (the network
//! ingest source). Per the Architecture of Reference, a driven-external /
//! non-deterministic dependency gets a FAKE. This fake hosts a bounded set of
//! fixture `RawRecord`s — INCLUDING the adversarial set (unsigned /
//! tampered-signature / cid-mismatch) — so the indexer's verify-before-index gate
//! (`appview_domain::ingest_decision`) runs the REAL pure verification path on
//! every fetched record and REJECTS the adversarial ones (AV-3 / KPI-AV-3).
//!
//! ## DD-AV-2: a test double MUST validate its inputs like the real adapter
//!
//! Per the nw-tdd-methodology Integration Test Contract: a test double that
//! accepts inputs the real adapter would reject creates invisible wiring bugs.
//! `FakeIngestSource::enumerate` therefore VALIDATES like the real
//! `AtProtoIngestAdapter` would:
//!
//!   - an empty `source` string is a `BadResponse` (the real adapter cannot pull
//!     from an empty seed/relay URL);
//!   - the `unreachable` posture returns `IngestError::Unreachable` (the AV-6
//!     probe-failure / startup-refusal driver);
//!   - each HOSTED record's WIRE SHAPE is validated (non-empty author DID,
//!     non-empty subject + object) — the real adapter's `listRecords` parse
//!     rejects a malformed record before it ever reaches the gate.
//!
//! What the fake does NOT do: it does NOT verify signatures or recompute CIDs.
//! That is the GATE's job (the cardinal verify-before-index decision). A
//! permissive fake that "verified" records here — silently dropping the
//! adversarial ones — would HIDE the AV-3 reject-gate wiring (the very thing the
//! release gate proves). So the adversarial records are hosted VERBATIM and
//! flow, unverified, to the real gate.
//
// SCAFFOLD: false  (the fake is a real, honest test double; the bodies are not todo!())

#![allow(dead_code)]

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use ports::{IngestError, IngestSourcePort, ProbeOutcome, RawRecord};

use crate::fixtures_ingest::RawRecordSpec;

/// A bounded fixture ingest source hosting a `listRecords`-style enumeration of
/// `RawRecord`s (valid + adversarial), satisfying `ports::IngestSourcePort`.
///
/// Owns its records by value (no I/O, no network). The `enumerate` call records
/// how many times it was invoked + which sources it was asked for, so the
/// public-data-only invariant (AV-7: NO auth-scoped/private read) can be asserted
/// against the call log. Cloneable (`Arc`-shared call log) so the indexer
/// composition root can hold one handle while a test asserts on another.
#[derive(Clone)]
pub struct FakeIngestSource {
    records: Arc<Vec<RawRecord>>,
    /// Sources `enumerate` was asked for (public-data-only assertion surface).
    requested_sources: Arc<std::sync::Mutex<Vec<String>>>,
    enumerate_calls: Arc<AtomicUsize>,
    /// When set, `enumerate` returns `IngestError::Unreachable` (AV-6 driver).
    unreachable: Arc<AtomicBool>,
}

impl FakeIngestSource {
    /// Host a set of `RawRecordSpec`s — each materialized to its wire `RawRecord`
    /// via the REAL crypto (`RawRecordSpec::into_raw_record`). The adversarial
    /// postures are hosted VERBATIM (the gate, not the fake, rejects them).
    pub fn with_specs(specs: Vec<RawRecordSpec>) -> Self {
        let records = specs
            .into_iter()
            .map(RawRecordSpec::into_raw_record)
            .collect();
        Self::with_records(records)
    }

    /// Host pre-materialized `RawRecord`s directly.
    pub fn with_records(records: Vec<RawRecord>) -> Self {
        Self {
            records: Arc::new(records),
            requested_sources: Arc::new(std::sync::Mutex::new(Vec::new())),
            enumerate_calls: Arc::new(AtomicUsize::new(0)),
            unreachable: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Engage the "unreachable" failure mode (AV-6 startup-refusal driver):
    /// subsequent `enumerate` calls return `IngestError::Unreachable`.
    pub fn simulate_unreachable(&self) {
        self.unreachable.store(true, Ordering::SeqCst);
    }

    /// Inverse of [`simulate_unreachable`](Self::simulate_unreachable).
    pub fn restore(&self) {
        self.unreachable.store(false, Ordering::SeqCst);
    }

    /// How many times `enumerate` was called (public-data-only assertion).
    pub fn enumerate_call_count(&self) -> usize {
        self.enumerate_calls.load(Ordering::SeqCst)
    }

    /// The sources `enumerate` was asked for — the indexer reads ONLY the public
    /// `listRecords` surface; this log proves it made NO auth-scoped call (AV-7).
    pub fn requested_sources(&self) -> Vec<String> {
        self.requested_sources
            .lock()
            .expect("requested_sources mutex not poisoned")
            .clone()
    }

    /// The records hosted (for cross-checking the indexer stored only the
    /// verified subset).
    pub fn hosted_records(&self) -> &[RawRecord] {
        &self.records
    }

    /// DD-AV-2 input validation, applied per hosted record. Mirrors the real
    /// adapter's `listRecords` parse: a record with an empty author DID or an
    /// empty subject/object is a malformed wire shape the real adapter rejects
    /// BEFORE the gate. (Signature/CID validity is intentionally NOT checked
    /// here — that is the gate's job.)
    fn validate_wire_shape(record: &RawRecord) -> Result<(), IngestError> {
        let claim = &record.raw_payload.unsigned;
        if claim.author_did.0.trim().is_empty() {
            return Err(IngestError::BadResponse {
                message: "ingest record carries an empty author DID".to_string(),
            });
        }
        if claim.subject.trim().is_empty() || claim.object.trim().is_empty() {
            return Err(IngestError::BadResponse {
                message: "ingest record carries an empty subject or object".to_string(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl IngestSourcePort for FakeIngestSource {
    fn probe(&self) -> ProbeOutcome {
        // An honest fixture source probes Ok unless the unreachable posture is
        // engaged (the real adapter's probe checks reachability + shape; the
        // refusal path is driven through `simulate_unreachable` + `enumerate`).
        ProbeOutcome::Ok
    }

    async fn enumerate(&self, source: &str) -> Result<Vec<RawRecord>, IngestError> {
        self.enumerate_calls.fetch_add(1, Ordering::SeqCst);
        self.requested_sources
            .lock()
            .expect("requested_sources mutex not poisoned")
            .push(source.to_string());

        // DD-AV-2: the real adapter cannot pull from an empty seed/relay URL.
        if source.trim().is_empty() {
            return Err(IngestError::BadResponse {
                message: "ingest source URL is empty".to_string(),
            });
        }

        if self.unreachable.load(Ordering::SeqCst) {
            return Err(IngestError::Unreachable {
                message: format!("fake ingest source {source} is unreachable"),
            });
        }

        // DD-AV-2: validate every hosted record's wire shape (NOT its
        // signature). The adversarial unsigned/tampered/cid-mismatch records are
        // wire-WELL-FORMED — they pass shape validation and flow to the gate,
        // which is what rejects them (AV-3). Only genuinely malformed shapes
        // (empty author/subject/object) are rejected here.
        for record in self.records.iter() {
            Self::validate_wire_shape(record)?;
        }

        Ok((*self.records).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures_ingest::fixture_ingest_adversarial_set_plus_one_valid;

    /// The fake hosts the adversarial set VERBATIM (it does NOT pre-filter the
    /// adversarial records — that would hide the AV-3 reject-gate wiring).
    #[tokio::test]
    async fn hosts_adversarial_set_verbatim_without_pre_filtering() {
        let source = FakeIngestSource::with_specs(fixture_ingest_adversarial_set_plus_one_valid());
        let records = source
            .enumerate("https://relay.example.test")
            .await
            .expect("enumerate succeeds for a well-shaped source");
        // All FOUR records (3 adversarial + 1 valid) flow through — the gate, not
        // the fake, is responsible for rejecting the 3 adversarial ones.
        assert_eq!(
            records.len(),
            4,
            "the fake must host all 4 records verbatim (no pre-filtering)"
        );
    }

    /// DD-AV-2: an empty source URL is rejected like the real adapter.
    #[tokio::test]
    async fn rejects_empty_source_url() {
        let source = FakeIngestSource::with_specs(fixture_ingest_adversarial_set_plus_one_valid());
        let result = source.enumerate("").await;
        assert!(
            matches!(result, Err(IngestError::BadResponse { .. })),
            "empty source URL must be a BadResponse (DD-AV-2)"
        );
    }

    /// DD-AV-2: the unreachable posture surfaces `IngestError::Unreachable`
    /// (the AV-6 startup-refusal driver).
    #[tokio::test]
    async fn unreachable_posture_surfaces_unreachable_error() {
        let source = FakeIngestSource::with_specs(fixture_ingest_adversarial_set_plus_one_valid());
        source.simulate_unreachable();
        let result = source.enumerate("https://relay.example.test").await;
        assert!(matches!(result, Err(IngestError::Unreachable { .. })));
    }

    /// The call log records which sources were requested (AV-7 public-data-only).
    #[tokio::test]
    async fn records_requested_sources_for_public_data_assertion() {
        let source = FakeIngestSource::with_specs(fixture_ingest_adversarial_set_plus_one_valid());
        let _ = source.enumerate("https://relay.example.test").await;
        assert_eq!(source.enumerate_call_count(), 1);
        assert_eq!(
            source.requested_sources(),
            vec!["https://relay.example.test"]
        );
    }
}
