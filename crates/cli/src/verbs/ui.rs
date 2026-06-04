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
use adapter_github::GithubAdapter;
use adapter_http_viewer::{
    read_only_launch_banner, viewer_store_unreadable_refusal, SharedGithub, SharedIndexQuery,
    SharedStore, ViewerServer,
};
use adapter_index_query::HttpIndexQueryAdapter;
use anyhow::{anyhow, Result};
use ports::{IndexQueryPort, ProbeOutcome, StoreReadPort};
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

    // Wire the slice-02 `GithubPort` (adapter-github) for the `/scrape` LIVE
    // propose route (US-VIEW-005). `from_env` resolves the GitHub API base (the
    // real public API, or the `OPENLORE_GITHUB_API_BASE` test seam) and the
    // optional `GITHUB_TOKEN`. CAPABILITY (I-VIEW-1/I-VIEW-3): a `GithubPort`
    // reads ONLY public GitHub — it holds NO signing key / IdentityPort / write
    // StoragePort, so the viewer process STILL holds only a read-only store + a
    // public-read GitHub port (no signing surface enters the viewer). The
    // `/scrape` route persists nothing (BR-VIEW-2).
    let github: SharedGithub = Arc::new(GithubAdapter::from_env());

    // Wire the slice-05 READ-ONLY `IndexQueryPort` (adapter-index-query) for the
    // slice-08 `/search` NETWORK-SEARCH route (US-NS-001..004; ADR-036/037). The
    // indexer URL is resolved from the SAME slice-05 seam the `openlore search` CLI
    // verb reads (`OPENLORE_INDEXER_URL` / `[appview] indexer_url`, OD-NS-6); an
    // UNSET/empty seam yields `None` so `/search` renders the fixed Unavailable
    // notice WITHOUT a network call (I-NS-2). CAPABILITY (I-NS-1): an
    // `IndexQueryPort` is read-only by construction — NO signing key / IdentityPort
    // / PdsPort enters the viewer process; the viewer still holds only a read-only
    // store + a public-read GitHub port + this read-only index query. The `/search`
    // route persists NOTHING (WD-NS-7).
    let index_query: Option<SharedIndexQuery> = resolve_index_query();

    // A startup soft-probe of the index is INFORMATIONAL (KPI-5 / WD-116): an
    // unreachable indexer must NOT refuse viewer startup. The probe does no network
    // round-trip; we surface its outcome as an event for DevOps but never gate on it.
    if let Some(index_query) = &index_query {
        let outcome = index_query.probe();
        let probe_event = serde_json::json!({
            "event": "viewer.index_query.probe",
            "refused": matches!(outcome, ProbeOutcome::Refused { .. }),
        });
        println!("{probe_event}");
    }

    let addr: std::net::SocketAddr = format!("127.0.0.1:{}", args.port)
        .parse()
        .map_err(|err| anyhow!("invalid viewer listen address: {err}"))?;

    // The viewer's tokio runtime — reuse the CLI's `build_tokio_runtime` shape
    // (current-thread; the accept loop + per-connection tasks run concurrently on
    // the single-thread executor). The runtime is built OUTSIDE block_on so the
    // bind happens inside it.
    let runtime = build_tokio_runtime();
    let code = runtime.block_on(async move {
        let server = match ViewerServer::bind_with_index_query(
            addr,
            Arc::clone(&store),
            Some(Arc::clone(&github)),
            index_query.clone(),
        ) {
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

/// The env-var seam the viewer composition root reads for the self-hosted indexer
/// URL (ADR-036 / OD-NS-6) — the SAME seam the `openlore search` CLI verb reads.
/// Production resolves `[appview] indexer_url` from the config; the acceptance
/// harness sets this env var to the localhost `openlore-indexer serve` port. An
/// empty/unset value ⇒ the index is UNCONFIGURED (the SOFT `/search` Unavailable
/// degradation WITHOUT a network call, I-NS-2).
const INDEXER_URL_ENV: &str = "OPENLORE_INDEXER_URL";

/// Resolve the READ-ONLY `IndexQueryPort` for the `/search` route from the slice-05
/// indexer-URL seam (OD-NS-6). Returns `Some(adapter)` wired at the configured URL,
/// or `None` when the seam is unset/empty (the UNCONFIGURED case — `/search` then
/// renders the fixed Unavailable notice WITHOUT any network call, I-NS-2). NO
/// signing key / IdentityPort / PdsPort is involved — the index query is read-only
/// by construction (I-NS-1).
fn resolve_index_query() -> Option<SharedIndexQuery> {
    let url = std::env::var(INDEXER_URL_ENV).unwrap_or_default();
    if url.is_empty() {
        return None;
    }
    let adapter: SharedIndexQuery = Arc::new(HttpIndexQueryAdapter::for_url(url));
    Some(adapter)
}

/// Force-link the `StoreReadPort` + read-only `IndexQueryPort` traits into this
/// module's import graph so the capability-boundary intent reads clearly at the
/// call site (the viewer holds a read-only store + a read-only index query and
/// nothing that can sign). No-op at runtime.
#[allow(dead_code)]
fn _capability_marker(_store: &dyn StoreReadPort, _index: &dyn IndexQueryPort) {}
