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

use std::path::Path;

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

/// The user's local-store filename (`adapter-duckdb`, the CLI's store). The
/// indexer must NEVER be wired against this file — it holds NO handle to the
/// user's local store (ADR-023 / I-AV-5). The capability-boundary probe refuses
/// if its configured store path resolves to this filename.
const USER_LOCAL_STORE_FILENAME: &str = "openlore.duckdb";

/// The indexer's OWN store filename (the SEPARATE re-buildable index, ADR-023/025).
/// The capability-boundary probe asserts the configured store path resolves to
/// exactly this filename.
const INDEXER_STORE_FILENAME: &str = "index.duckdb";

/// The ADR-023 capability-boundary probe (behavioral layer of I-AV-5): assert the
/// configured store is the SEPARATE `index.duckdb` (NOT the user's
/// `openlore.duckdb`) and the identity adapter is resolve/verify-only (NO
/// signing), REFUSING on violation (`indexer.capability_boundary_violated`). Runs
/// FIRST, before the per-adapter gauntlet.
///
/// The check is REAL (not a scaffold): it inspects the configured `index_path`
/// observable the composition root passes in — refusing if the path's filename is
/// the user's `openlore.duckdb` OR is anything other than `index.duckdb`. The
/// resolve-only capability is enforced at the TYPE level (the `IdentityResolvePort`
/// trait exposes NO sign/publish method, so a signing identity cannot even be
/// passed here); this probe also walks the resolve adapter's own readiness arm so
/// a mis-wired resolver refuses at startup. The structural backstop is the `xtask
/// check-arch` `indexer_holds_no_signing_or_local_store` rule (the indexer's dep
/// graph excludes `adapter-duckdb`/the signing `IdentityPort` by construction).
pub fn capability_boundary_probe(
    index_path: &Path,
    index_store: &dyn IndexStorePort,
    resolve: &dyn IdentityResolvePort,
) -> Result<(), ProbeRefusal> {
    // Arm 1 (load-bearing): the configured store is the SEPARATE index.duckdb and
    // is NOT the user's openlore.duckdb. The indexer holds no handle to the user
    // store — wiring it against `openlore.duckdb` is a capability-boundary breach.
    // The decision is a PURE function so it is unit-testable without trait objects.
    if let Err(detail) = classify_store_path(index_path) {
        return Err(capability_boundary_refusal(detail));
    }

    // Arm 2: the resolve adapter must be ready (verify-only by type; this walks
    // its readiness arm so a mis-wired resolver refuses BEFORE the first ingest).
    // The index store's own schema/fsync/attribution probe runs in the per-adapter
    // gauntlet that follows.
    check_probe("identity_resolve", resolve.probe())?;

    // The index store handle is threaded so a future arm can cross-check the
    // open store against the configured path; its readiness probe runs next in
    // the gauntlet.
    let _ = index_store;
    Ok(())
}

/// Build the `indexer.capability_boundary_violated` refusal carried up to the
/// composition root (emitted as `health.startup.refused` + exit 2). Uses the
/// existing `StorageSchemaMismatch` reason (the configured store is not the
/// expected index.duckdb) with a `capability_boundary` discriminator in the
/// structured payload so the DevOps layer routes it as the I-AV-5 breach.
fn capability_boundary_refusal(detail: String) -> ProbeRefusal {
    ProbeRefusal {
        adapter: "capability_boundary",
        reason: ports::ProbeRefusalReason::StorageSchemaMismatch,
        structured: serde_json::json!({
            "event": "indexer.capability_boundary_violated",
            "detail": detail,
        }),
        detail,
    }
}

/// PURE decision (no I/O): does `index_path` resolve to the indexer's OWN
/// SEPARATE store (`index.duckdb`)? Returns `Ok(())` when the filename is exactly
/// `index.duckdb`; `Err(detail)` when it is the user's `openlore.duckdb` (the
/// capability-boundary breach) or any other filename (the indexer must index ONLY
/// into its own re-buildable store, ADR-023/025).
///
/// Extracted from [`capability_boundary_probe`] so the load-bearing refusal is
/// unit-testable as a pure function (no trait objects / no async resolve arm).
fn classify_store_path(index_path: &Path) -> Result<(), String> {
    let filename = index_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if filename == USER_LOCAL_STORE_FILENAME {
        return Err(format!(
            "indexer store path {} resolves to the user's local store \
             ({USER_LOCAL_STORE_FILENAME}); the indexer holds NO handle to the user's \
             local store (ADR-023 / I-AV-5)",
            index_path.display()
        ));
    }
    if filename != INDEXER_STORE_FILENAME {
        return Err(format!(
            "indexer store path {} does not resolve to the SEPARATE \
             {INDEXER_STORE_FILENAME} (got filename {filename:?}); the indexer must index \
             ONLY into its own re-buildable store (ADR-023/025)",
            index_path.display()
        ));
    }
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

// -----------------------------------------------------------------------------
// Unit tests — the PURE capability-boundary store-path decision (ADR-023 / I-AV-5).
//
// Port-to-port at domain scope: `classify_store_path` IS the decision's public
// signature, exercised directly (no trait objects, no I/O). Pins the load-bearing
// refusal: the indexer ACCEPTS only its own SEPARATE `index.duckdb` and REFUSES
// the user's `openlore.duckdb` (the capability boundary) or any other filename.
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn accepts_the_separate_index_duckdb() {
        let path = PathBuf::from("/home/u/.local/share/openlore-indexer/index.duckdb");
        assert!(
            classify_store_path(&path).is_ok(),
            "the indexer's OWN index.duckdb must pass the capability-boundary store-path check"
        );
    }

    #[test]
    fn refuses_the_users_openlore_duckdb() {
        // The capability-boundary breach: wired against the user's local store.
        let path = PathBuf::from("/home/u/.local/share/openlore/openlore.duckdb");
        let err = classify_store_path(&path)
            .expect_err("the user's openlore.duckdb must REFUSE (ADR-023 / I-AV-5)");
        assert!(
            err.contains("openlore.duckdb") && err.contains("local store"),
            "the refusal must name the user-local-store breach; got {err:?}"
        );
    }

    #[test]
    fn refuses_any_other_filename() {
        // Anything that is not the SEPARATE index.duckdb is refused — the indexer
        // indexes ONLY into its own re-buildable store (ADR-023/025).
        let path = PathBuf::from("/var/tmp/scratch.duckdb");
        let err = classify_store_path(&path)
            .expect_err("a non-index.duckdb store must REFUSE (ADR-023/025)");
        assert!(
            err.contains("index.duckdb"),
            "the refusal must name the expected SEPARATE index.duckdb; got {err:?}"
        );
    }
}
