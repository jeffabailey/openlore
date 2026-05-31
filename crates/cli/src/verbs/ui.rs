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
use adapter_http_viewer::{read_only_launch_banner, SharedStore, ViewerServer};
use anyhow::{anyhow, Context, Result};
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
    // separate process; opening the file at the resolved path is the read handle
    // (the writing CLI process has exited). `read_adapter()` exposes ONLY the
    // read-only `StoreReadPort` surface (no write/sign method — I-VIEW-1).
    let db_path = paths.duckdb_file();
    let storage = DuckDbStorageAdapter::open(&db_path)
        .with_context(|| format!("opening your store at {}", db_path.display()))?;
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
        // store + asking if another process holds it (ADR-030; NFR-VIEW-6).
        if let ProbeOutcome::Refused { detail, .. } = server.probe() {
            eprintln!("openlore ui: refusing to serve — {detail}");
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

/// Force-link the `StoreReadPort` trait into this module's import graph so the
/// capability-boundary intent reads clearly at the call site (the viewer holds a
/// read-only store and nothing else). No-op at runtime.
#[allow(dead_code)]
fn _capability_marker(_store: &dyn StoreReadPort) {}
