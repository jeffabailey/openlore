//! `ui` — the read-only htmx viewer verb (slice-06; ADR-028/030).
//!
//! `openlore ui [--port <P>]` is a LONG-RUNNING server bound to `127.0.0.1` ONLY
//! that renders the operator's OWN node store as server-rendered HTML over a
//! READ-ONLY `StoreReadPort` (I-VIEW-1). The signing key never enters this
//! process — the viewer holds a `Box<dyn StoreReadPort>` and links NO identity /
//! PDS surface (I-VIEW-3, structural via the `adapter-http-viewer` capability
//! boundary; `cli` is the only crate that links it).
//!
//! Wire → PROBE → use (ADR-009/030): open the store read handle, bind the hyper
//! server on the configured loopback port, run the viewer's store-readability +
//! loopback probe, emit `viewer.serve.listening` (so a supervisor/test harness
//! can read the bound `:0` port back), then run the accept loop until killed.

use adapter_duckdb::DuckDbStorageAdapter;
use adapter_http_viewer::{
    read_only_launch_banner, viewer_store_unreadable_refusal, SharedStore, ViewerServer,
};
use anyhow::{anyhow, Result};
use ports::{ProbeOutcome, StoreReadPort};
use std::sync::Arc;

use crate::paths::OpenLorePaths;
use crate::verbs::claim_publish::build_tokio_runtime;

/// Argument struct for the `ui` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct UiArgs {
    /// The loopback port to bind (`0` = OS-assigned ephemeral, read back via the
    /// `viewer.serve.listening` event).
    pub port: u16,
}

/// Run the `openlore ui` verb. Returns the process exit code. Blocks until the
/// server is killed (it is a long-running serve loop) or refuses to start (the
/// store is unreadable / a non-loopback bind was somehow attempted).
pub fn run(paths: &OpenLorePaths, args: &UiArgs) -> i32 {
    match serve(paths, args) {
        Ok(code) => code,
        Err(err) => {
            // Plain-language refusal — never a raw stack trace (NFR-VIEW-6).
            eprintln!("openlore ui: {err:#}");
            2
        }
    }
}

/// Open the read-only store, bind the viewer server, probe, announce, and serve.
/// Split out from [`run`] so the error path renders a single plain-language line.
fn serve(paths: &OpenLorePaths, args: &UiArgs) -> Result<i32> {
    // Open the SAME store the CLI writes through (BR-VIEW-4). The viewer is a
    // separate process; opening the file at the resolved path is the read handle.
    // `read_adapter()` exposes ONLY the read-only `StoreReadPort` surface (no
    // write/sign method — I-VIEW-1).
    //
    // WIRE→PROBE→USE store-readability fork (ADR-030 §Earned-Trust step 1): if the
    // open FAILS — the common cause is another process holding the DuckDB file
    // lock — the viewer REFUSES to serve with the SAME plain-language refusal the
    // probe would render (naming the store path, asking if another process holds
    // it; NO raw transport error / stack trace, NFR-VIEW-6), and emits the
    // structured `health.startup.refused` event. This open failure happens BEFORE
    // the server can be built, so it cannot flow through `server.probe()` — both
    // surfaces share `viewer_store_unreadable_refusal` for one consistent message.
    let db_path = paths.duckdb_file();
    let store_path = db_path.display().to_string();
    let storage = match DuckDbStorageAdapter::open(&db_path) {
        Ok(storage) => storage,
        Err(err) => {
            let refusal = viewer_store_unreadable_refusal(&store_path, &err.to_string());
            emit_startup_refused(&refusal);
            return Ok(2);
        }
    };
    let store: SharedStore = Arc::new(storage.read_adapter());

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", args.port)
        .parse()
        .map_err(|err| anyhow!("invalid viewer listen address: {err}"))?;

    // The viewer's tokio runtime — reuse the CLI's `build_tokio_runtime` shape
    // (current-thread; the accept loop + per-connection tasks run concurrently on
    // the single-thread executor). The runtime is built OUTSIDE block_on so the
    // bind happens inside it.
    let runtime = build_tokio_runtime();
    let code = runtime.block_on(async move {
        let server = match ViewerServer::bind(addr, Arc::clone(&store)) {
            Ok(server) => server,
            Err(err) => {
                eprintln!("openlore ui: {err}");
                return 2;
            }
        };

        // Wire → PROBE → use: run the store-readability + loopback probe BEFORE
        // serving. A refusal surfaces as a plain-language message naming the
        // store + asking if another process holds it (ADR-030; NFR-VIEW-6), AND a
        // structured `health.startup.refused` event for DevOps.
        let outcome = server.probe(&store_path);
        if let ProbeOutcome::Refused { .. } = &outcome {
            emit_startup_refused(&outcome);
            return 2;
        }

        // Emit the bound address so a supervisor (the test harness) can read the
        // ephemeral `:0` port back (mirrors `indexer.serve.listening`). Structural
        // (an address; no claim content) — the DevOps observability contract.
        let bound_addr = server.local_addr().to_string();
        let listening = serde_json::json!({
            "event": "viewer.serve.listening",
            "addr": bound_addr,
        });
        println!("{listening}");

        // Read-only launch notice (AC-001.2): now that the loopback probe has
        // passed, tell the operator — up front — the loopback listen URL, that the
        // view is read-only, and that no signing key is loaded. The exact strings
        // are a PURE `viewer-domain` formatting fn (unit/property-pinned).
        println!("{}", read_only_launch_banner(&bound_addr));
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match server.serve().await {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("openlore ui: serve loop failed: {err}");
                2
            }
        }
    });
    Ok(code)
}

/// Emit the viewer's startup refusal: a structured `health.startup.refused`
/// event (for DevOps observability — carries the reason + the structured payload
/// with the raw cause) followed by the plain-language operator line on stderr
/// (the `detail` — names the store path, asks if another process holds it; NO
/// stack trace / raw transport error — NFR-VIEW-6). Mirrors the cli composition
/// root's `emit_health_startup_refused`, but consumes a `ProbeOutcome` directly
/// (the viewer is its own composition root and does not build a `Wiring`).
///
/// A no-op for `ProbeOutcome::Ok` (callers only invoke it on a `Refused`).
fn emit_startup_refused(outcome: &ProbeOutcome) {
    let ProbeOutcome::Refused {
        reason,
        detail,
        structured,
    } = outcome
    else {
        return;
    };
    let event = serde_json::json!({
        "event": "health.startup.refused",
        "adapter": "viewer",
        "reason": format!("{reason:?}"),
        "detail": detail,
        "structured": structured,
    });
    eprintln!("{event}");
    eprintln!("openlore ui: refusing to serve — {detail}");
}

/// Force-link the `StoreReadPort` trait into this module's import graph so the
/// capability-boundary intent reads clearly at the call site (the viewer holds a
/// read-only store and nothing else). No-op at runtime.
#[allow(dead_code)]
fn _capability_marker(_store: &dyn StoreReadPort) {}
