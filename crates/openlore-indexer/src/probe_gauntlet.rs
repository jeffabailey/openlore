//! `probe_gauntlet` — the indexer's wire → PROBE → use gate (ADR-009 D-9 / 023).
//!
//! Mirrors the slice-01 CLI `wiring::probe_gauntlet` shape: the SECOND
//! composition root constructs every adapter at startup, runs the
//! `capability_boundary_probe` FIRST (ADR-023: assert the store is `index.duckdb`
//! and the identity adapter is resolve-only — refuse on violation), then walks
//! ALL per-adapter probes, and REFUSES to serve on any refusal
//! (`health.startup.refused` + exit code 2). wire → probe → use.
//!
//! Bootstrap SCAFFOLD (step 01-04): the gauntlet's SHAPE is established here —
//! the `ProbeRefusal` payload, the `capability_boundary_probe`, and the
//! per-adapter walk — but the bodies are `todo!()` (the wired adapters' probes
//! are themselves Phase 03/04 scaffolds). The refuse-on-probe-failure + exit-2 +
//! `health.startup.refused` emission lands with the real probe bodies.
//
// SCAFFOLD: true

use ports::{IdentityResolvePort, IndexStorePort, IngestSourcePort, ProbeOutcome};

/// A refusal carried up from the indexer's probe gauntlet — the same shape the
/// CLI uses (`adapter` name + the raw `ProbeOutcome::Refused` payload) so the
/// indexer's composition root can emit `health.startup.refused` with all fields
/// intact (ADR-023 telemetry: `indexer.capability_boundary_violated` etc.).
#[derive(Debug)]
pub struct ProbeRefusal {
    pub adapter: &'static str,
    pub reason: ports::ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

/// Convert one adapter's [`ProbeOutcome`] into the gauntlet's railway result.
/// Shared helper mirroring the CLI's `check_probe`. Bootstrap SCAFFOLD — wired
/// into the gauntlet once the real probe bodies land.
fn check_probe(adapter: &'static str, outcome: ProbeOutcome) -> Result<(), ProbeRefusal> {
    match outcome {
        ProbeOutcome::Ok => Ok(()),
        ProbeOutcome::Refused {
            reason,
            detail,
            structured,
        } => Err(ProbeRefusal {
            adapter,
            reason,
            detail,
            structured,
        }),
    }
}

/// The ADR-023 capability-boundary probe (behavioral layer of I-AV-5): assert
/// the store is the SEPARATE `index.duckdb` (NOT the user's `openlore.duckdb`)
/// and the identity adapter is resolve/verify-only (NO signing), refusing on
/// violation (`indexer.capability_boundary_violated`). Runs FIRST, before the
/// per-adapter probes.
///
/// SCAFFOLD: true — the behavioral assertion lands with the real probe bodies
/// (Phase 03/04). The structural backstop is the `xtask check-arch`
/// `indexer_holds_no_signing_or_local_store` rule (which already passes on the
/// real indexer crate's dep graph).
pub fn capability_boundary_probe(
    index_store: &dyn IndexStorePort,
    resolve: &dyn IdentityResolvePort,
) -> Result<(), ProbeRefusal> {
    // Happy-path arm for the AV-1 walking skeleton (step 03-01): the index store +
    // resolve adapter are wired (their own probes assert readiness; the index
    // store probe confirms the schema is the SEPARATE index.duckdb, the resolve
    // adapter is the verify-only port — the indexer never wires the signing
    // IdentityPort or the user's StoragePort, enforced structurally by the
    // `xtask check-arch` `indexer_holds_no_signing_or_local_store` rule + the
    // crate dep graph). The behavioral substrate-lie refusal (a fsync-lying store
    // / a signing identity wired by mistake) is AV-6/03-06.
    let _ = (index_store, resolve);
    Ok(())
}

/// Walk every indexer adapter's probe arm AFTER the capability-boundary probe.
/// Returns `Err(..)` carrying the first refusal with its structured
/// `health.startup.refused` payload preserved (wire → probe → use; refuse on any
/// refusal). The `query_server` probe is an inherent method (not a `*Port`
/// trait), so it is checked at the composition root, not here.
///
/// SCAFFOLD: true — the walk is wired once the adapters' probe bodies land
/// (Phase 03/04). The `check_probe` helper is the shape it will use.
pub fn probe_gauntlet(
    index_store: &dyn IndexStorePort,
    ingest_src: &dyn IngestSourcePort,
    resolve: &dyn IdentityResolvePort,
) -> Result<(), ProbeRefusal> {
    // The real walk (mirrors the CLI gauntlet): refuse on the FIRST adapter that
    // is not ready (wire → probe → use; ADR-009). The adapters' probe bodies are
    // now real (step 03-01).
    check_probe("index_store", index_store.probe())?;
    check_probe("ingest_source", ingest_src.probe())?;
    check_probe("identity_resolve", resolve.probe())?;
    Ok(())
}
