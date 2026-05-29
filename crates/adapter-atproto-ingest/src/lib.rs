//! `adapter-atproto-ingest` — the indexer-side bounded-PULL ingest adapter.
//!
//! EFFECT shell for the `IngestSourcePort` trait (`crates/ports`). Performs a
//! bounded PULL of PUBLIC `org.openlore.claim` records via the ATProto
//! `com.atproto.repo.listRecords` XRPC (seed DIDs → their PDS; an optional
//! configured relay) — ADR-024. The fetched [`RawRecord`]s flow to the pure
//! `appview_domain::ingest_decision` gate; NO verification happens here.
//!
//! ## Read-only by construction (capability boundary I-AV-5)
//!
//! This adapter holds NO `IdentityPort` / signing key and exposes NO write /
//! sign / publish method — the indexer is signing-incapable. The absence is the
//! design: there is structurally no path from this adapter to authoring or
//! mutating a claim (the type-level half of I-AV-5 is `IngestSourcePort` itself
//! having no write method; this adapter simply implements that read-only trait).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! Pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires an [`AtProtoIngestAdapter`] behind the
//! `IngestSourcePort` interface.
//!
//! Bootstrap SCAFFOLD (step 01-03): the port impl exists so the workspace
//! compiles and the wiring seam is present, but every body is `todo!()`. The
//! bounded-PULL + probe behavior is driven by the Phase 03/04 acceptance
//! scenarios (AV-* / the ingest-adapter probe rejecting tampered records).
//
// SCAFFOLD: true  (adapter skeleton; bounded-PULL bodies land in Phase 03/04)

#![allow(dead_code)] // scaffold; real wiring lands in subsequent DELIVER steps
#![forbid(unsafe_code)]

use async_trait::async_trait;
use ports::{IngestError, IngestSourcePort, ProbeOutcome, RawRecord};

/// Bounded read-only PULL `IngestSourcePort` adapter over ATProto XRPC
/// (`listRecords`) — ADR-024.
///
/// READ-ONLY by construction (I-AV-5): holds NO signing identity and no local
/// store handle. Bootstrap SCAFFOLD — the fields land with the real
/// `reqwest`-client + seed-source wiring in Phase 03/04.
pub struct AtProtoIngestAdapter {
    // SCAFFOLD: true — the HTTPS client + bounded-seed source config land in
    // Phase 03/04. No signing/identity field exists by construction (I-AV-5).
    _scaffold: (),
}

impl AtProtoIngestAdapter {
    /// Construct the ingest adapter. Bootstrap SCAFFOLD: the real constructor
    /// (reqwest client + bounded seed-DID/relay config) lands in Phase 03/04.
    pub fn new() -> Self {
        // SCAFFOLD: true
        todo!("AtProtoIngestAdapter::new — wired in Phase 03/04 (ADR-024 bounded PULL)")
    }
}

#[async_trait]
impl IngestSourcePort for AtProtoIngestAdapter {
    fn probe(&self) -> ProbeOutcome {
        // SCAFFOLD: true — the Earned-Trust probe (source reachability +
        // enumeration shape + the network-lies tampered/CID-mismatch rejection
        // check) lands in Phase 03/04.
        todo!("AtProtoIngestAdapter::probe — Earned-Trust ingest probe (Phase 03/04)")
    }

    async fn enumerate(&self, _source: &str) -> Result<Vec<RawRecord>, IngestError> {
        // SCAFFOLD: true — the bounded `listRecords` PULL (ADR-024) lands in
        // Phase 03/04; fetched RawRecords flow to appview_domain::ingest_decision.
        todo!("AtProtoIngestAdapter::enumerate — bounded read-only PULL (Phase 03/04)")
    }
}
