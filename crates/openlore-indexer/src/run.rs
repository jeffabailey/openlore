//! `run` — the indexer's wiring + dispatch (the SECOND composition root body).
//!
//! ADR-009/023: WIRE the four driven adapters (IngestSourcePort,
//! IndexStorePort, IdentityResolvePort, the HTTP query server), run the
//! `capability_boundary_probe` + ALL per-adapter probes BEFORE ingest/serve
//! (wire → probe → use), and REFUSE to start on any probe failure (emit
//! `health.startup.refused` + exit code 2). Then dispatch `serve` / `ingest` /
//! `stats`.
//!
//! The indexer is signing-INCAPABLE + holds NO local store (ADR-023): it wires
//! ONLY the verify-only `IdentityResolvePort` impl + the read-only ingest source
//! + the SEPARATE `index.duckdb` store + the query server. It does NOT wire the
//! signing `IdentityPort`, the user's `StoragePort`/`adapter-duckdb`, or any
//! PDS-write surface — the capability boundary is the ABSENCE of those deps
//! (enforced by `xtask check-arch`'s `indexer_holds_no_signing_or_local_store`).
//!
//! Bootstrap SCAFFOLD (step 01-04): the wiring SHAPE + the wire → probe → use
//! sequence + the refuse-on-probe-failure path are established; the adapter
//! constructors + the serve/ingest/stats bodies are `todo!()` (the real bounded
//! pull-ingest loop + serving land in Phase 03/04).
//
// SCAFFOLD: true

#![allow(dead_code)] // scaffold; the wired adapters' bodies land in Phase 03/04

use adapter_atproto_did::AtProtoDidAdapter;
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::IndexStoreAdapter;
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::XrpcQueryServer;
use ports::{ClockPort, IdentityResolvePort, IndexStorePort, IngestSourcePort};

use crate::probe_gauntlet::{capability_boundary_probe, probe_gauntlet, ProbeRefusal};
use crate::Command;

/// The indexer's wired adapter set, owned by the composition root for the
/// duration of the program (mirrors the CLI's `Wiring`). Holds ONLY the
/// indexer's driven adapters — by construction NO signing identity + NO local
/// store (the capability boundary, ADR-023 / I-AV-5).
pub struct IndexerWiring {
    pub index_store: Box<dyn IndexStorePort>,
    pub ingest_source: Box<dyn IngestSourcePort>,
    /// VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
    pub identity_resolve: Box<dyn IdentityResolvePort>,
    pub query_server: XrpcQueryServer,
    pub clock: Box<dyn ClockPort>,
}

impl IndexerWiring {
    /// Construct the production wiring. Bootstrap SCAFFOLD: the adapter
    /// constructors read the indexer's OWN config (index.duckdb path, listen
    /// addr, PLC endpoint, bounded seed sources) in Phase 03/04. None of the
    /// wired adapters can sign/publish or touch the user's `openlore.duckdb`.
    pub fn production() -> anyhow::Result<Self> {
        // SCAFFOLD: true — wire the indexer's adapters from the indexer config:
        //   let clock           = SystemClockAdapter::new();
        //   let index_store     = IndexStoreAdapter::open(&cfg.index_path)?;     // SEPARATE index.duckdb
        //   let ingest_source   = AtProtoIngestAdapter::new(&cfg.sources, &cfg.relay)?;  // read-only PULL
        //   let identity_resolve= AtProtoDidAdapter::resolve_only(&cfg.plc_endpoint)?;   // VERIFY-ONLY
        //   let query_server    = XrpcQueryServer::bind(cfg.listen_addr)?;
        // The names below are referenced so the (disjoint) adapter set + the
        // verify-only/read-only/no-local-store capability boundary is visible at
        // the wiring seam even while the constructors are scaffolds.
        let _wire = (
            SystemClockAdapter::new,
            IndexStoreAdapter::open,
            AtProtoIngestAdapter::new,
            AtProtoDidAdapter::for_did,
            XrpcQueryServer::bind,
        );
        todo!("IndexerWiring::production — wire the indexer adapters from config (Phase 03/04, ADR-023)")
    }

    /// Run the wire → PROBE → use gate: the `capability_boundary_probe` FIRST
    /// (ADR-023), then the per-adapter gauntlet + the query-server probe. Returns
    /// the first refusal so the composition root can emit `health.startup.refused`
    /// and exit 2. SCAFFOLD — wired once the probe bodies land.
    pub fn probe_all(&self) -> Result<(), ProbeRefusal> {
        capability_boundary_probe(self.index_store.as_ref(), self.identity_resolve.as_ref())?;
        probe_gauntlet(
            self.index_store.as_ref(),
            self.ingest_source.as_ref(),
            self.identity_resolve.as_ref(),
        )?;
        // The query server's probe is an inherent method (not a `*Port` trait),
        // so it is checked here at the composition root, not in the gauntlet.
        // SCAFFOLD: true — `check the query_server.probe()` once its body lands.
        let _ = (&self.query_server, &self.clock);
        Ok(())
    }
}

/// Dispatch the parsed indexer subcommand through the wire → probe → use gate.
/// Returns the exit code the caller hands back to the OS (mirrors the CLI's
/// `dispatch`):
///
/// 1. Construct the wiring (instantiates every indexer adapter).
/// 2. Run the capability-boundary probe + ALL probes; refuse with
///    `health.startup.refused` + exit 2 on any refusal (REFUSES to start, ADR-023).
/// 3. Dispatch the verb (`serve` runs the query server + ingest loop; `ingest`
///    is a one-shot bounded PULL pass; `stats` reports index coverage).
///
/// Bootstrap SCAFFOLD (step 01-04): the sequence is wired; the verb bodies are
/// `todo!()`.
pub fn run(command: Command) -> i32 {
    // Step 1: WIRE.
    let wiring = match IndexerWiring::production() {
        Ok(w) => w,
        Err(err) => {
            eprintln!("openlore-indexer: failed to construct adapter wiring: {err:#}");
            return 2;
        }
    };

    // Step 2: PROBE (capability boundary + the per-adapter gauntlet). REFUSE to
    // start on any probe failure — emit `health.startup.refused` + exit 2.
    if let Err(refusal) = wiring.probe_all() {
        emit_health_startup_refused(&refusal);
        return 2;
    }

    // Step 3: USE — dispatch the subcommand.
    match command {
        Command::Serve => serve(&wiring),
        Command::Ingest => ingest(&wiring),
        Command::Stats => stats(&wiring),
    }
}

/// `openlore-indexer serve` — run the bounded pull-ingest loop + serve the
/// `org.openlore.appview.searchClaims` query surface (ADR-024/027). SCAFFOLD.
fn serve(_wiring: &IndexerWiring) -> i32 {
    // SCAFFOLD: true — the run loop (bounded PULL → verify-before-index gate →
    // upsert → serve queries) lands in Phase 03/04.
    todo!("openlore-indexer serve — run the pull-ingest loop + query server (Phase 03/04)")
}

/// `openlore-indexer ingest` — a one-shot bounded PULL pass (ADR-024). SCAFFOLD.
fn ingest(_wiring: &IndexerWiring) -> i32 {
    // SCAFFOLD: true — one bounded `enumerate` → `ingest_decision` → `upsert`
    // pass lands in Phase 03/04 (records flow through the pure verify gate).
    todo!("openlore-indexer ingest — one-shot bounded PULL pass (Phase 03/04, ADR-024)")
}

/// `openlore-indexer stats` — report index coverage (claims indexed, distinct
/// authors, ingest lag). SCAFFOLD.
fn stats(_wiring: &IndexerWiring) -> i32 {
    // SCAFFOLD: true — index coverage report lands in Phase 03/04.
    todo!("openlore-indexer stats — index coverage report (Phase 03/04)")
}

/// Emit a `health.startup.refused` event to stderr in the structured shape
/// DevOps consumes — identical to the CLI's, so observability layers route on
/// both binaries uniformly. The pure data comes straight from the refusing
/// adapter's (or the capability-boundary probe's) `ProbeRefusal` payload.
fn emit_health_startup_refused(refusal: &ProbeRefusal) {
    let event = serde_json::json!({
        "event": "health.startup.refused",
        "binary": "openlore-indexer",
        "adapter": refusal.adapter,
        "reason": format!("{:?}", refusal.reason),
        "detail": refusal.detail,
        "structured": refusal.structured,
    });
    eprintln!("{event}");
    eprintln!(
        "openlore-indexer: refusing to start — {} adapter: {}",
        refusal.adapter, refusal.detail
    );
}
