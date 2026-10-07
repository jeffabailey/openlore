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
//! The index store is opened ONCE per process here (ADR-080, B1) and shared:
//! the probe and the ingest pass use it whole, `serve`'s search handler sees it
//! through `IndexReadPort` only (B7). `serve` answers searches; it runs no
//! ingest of its own; the pass body lives in `ingest_pass`. `stats` is still
//! a `todo!()` scaffold.

#![allow(dead_code)] // some scaffold seams (serve/stats) land in Phase 03/04

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use adapter_atproto_did::{AtProtoDidAdapter, IdentityLookup};
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::{DuckDbCaps, IndexStoreAdapter, UnusableReason};
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::{RateLimit, ServeLimits, TrustedProxies, XrpcQueryServer};

use appview_domain::health::StoreHealth;
use appview_domain::FallbackUrl;
use claim_domain::Did;
use ports::net_policy::TransportPolicy;
use ports::{
    ClockPort, IdentityLookupPort, IdentityResolvePort, IndexPurgePort, IndexStorePort,
    IngestSourcePort, RepoListingPort, SearchDimension,
};

use crate::config::{parse_config, BuildProfile, ConfigError, IndexerConfig, TestFault};
use crate::events::{emit, emit_flushed};
use crate::health::{health_handler, StoreUsability};
use crate::ingest_pass::{in_serve_pass, ingest};
use crate::pass_runner::{failing_first_purge, PassRunner, StatusReader};
use crate::probe_gauntlet::{
    capability_boundary_probe, check_probe, control_channel_probe, origin_classification_probe,
    probe_gauntlet, ProbeRefusal,
};
use crate::search_handler::{search_handler, unreadable_index, SearchTruncated, SharedIndexReads};
use crate::Command;

/// The one shared index handle: read + write side, safe to share across the
/// serve accept loop's tasks.
pub type SharedIndexStore = Arc<dyn IndexStorePort + Send + Sync>;

/// The delete side of the same handle, handed to the pass runner only (ADR-082).
pub type SharedIndexPurge = Arc<dyn IndexPurgePort + Send + Sync>;

/// The wiring, and the purge capability that goes to the pass runner and
/// nowhere else (ADR-082): it is not a field of [`IndexerWiring`], so no
/// holder of the wiring can reach it.
pub struct Wired {
    pub wiring: IndexerWiring,
    purge_capability: Option<SharedIndexPurge>,
}

/// Refused to start (bad config, wiring, probe) or a fatal runtime/store failure.
const EXIT_FATAL: i32 = 2;
/// `serve` ran to completion.
const EXIT_SERVED: i32 = 0;

/// How often `serve` checks that its store is still usable (ADR-080 §7).
const STORE_WATCH_PERIOD: Duration = Duration::from_millis(50);

/// The indexer's wired adapter set, owned by the composition root for the
/// duration of the program (mirrors the CLI's `Wiring`). Holds ONLY the
/// indexer's driven adapters — by construction NO signing identity + NO local
/// store (the capability boundary, ADR-023 / I-AV-5).
pub struct IndexerWiring {
    /// The process's ONE handle on `index.duckdb` (ADR-080, B1): opened once
    /// here and shared by the probe, the writes and (through its read port
    /// only) the search handler.
    pub index_store: SharedIndexStore,
    /// The same handle, for the store's own condition (is it still usable?)
    /// and the TEST-ONLY poison fault — never handed to search or the pass.
    pub store_condition: Arc<IndexStoreAdapter>,
    /// What search reads: the same handle through its read port only (B7),
    /// or, under the `search_store_read_fails` test fault, an unreadable one.
    pub index_reads: SharedIndexReads,
    pub ingest_source: Box<dyn IngestSourcePort>,
    /// Read-only `listRecords` of ONE repo DID, cursor-paged (ADR-071 §4).
    pub repo_listing: Box<dyn RepoListingPort>,
    /// DID → its DID document's PDS, resolved afresh every pass: where each
    /// repo is listed and the only origin a self-attested record may be
    /// indexed from (ADR-071, ADR-077).
    pub pds_lookup: Box<dyn IdentityLookupPort>,
    /// The repo DIDs one ingest pass enumerates (DWD-9) when no list file is
    /// configured.
    pub repo_dids: Vec<Did>,
    /// The DID list file each in-`serve` pass reads afresh (ADR-081).
    pub repo_dids_file: Option<PathBuf>,
    /// `serve`'s Unix control socket (ADR-080 §3); `None` = no control channel.
    pub control_socket: Option<PathBuf>,
    /// VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
    pub identity_resolve: Box<dyn IdentityResolvePort>,
    /// The query server is bound only for `serve` (Phase 04); the `ingest`
    /// one-shot pass leaves it `None` (it does not serve).
    pub query_server: Option<XrpcQueryServer>,
    pub clock: Box<dyn ClockPort + Send + Sync>,
    /// Where a DID whose document cannot be resolved is listed (relay
    /// origin, ADR-077); `None` = such a DID is skipped.
    pub fallback: Option<FallbackUrl>,
    /// Which resolved PDS addresses may be listed (DD-IPF-5).
    pub policy: TransportPolicy,
    /// How many DIDs are fetched at once (ADR-078).
    pub max_concurrent_fetches: usize,
    /// The single deadline each DID's fetch (resolve + listing) runs under.
    pub per_did_time_budget: Duration,
    /// The configured SEPARATE `index.duckdb` path (ADR-023). Threaded into the
    /// `capability_boundary_probe` so it can REFUSE if mis-wired against the
    /// user's `openlore.duckdb` (the capability boundary, I-AV-5).
    pub index_path: PathBuf,
    /// The HTTP/XRPC query surface listen address (ADR-027). `serve` binds it;
    /// `:0` resolves to an OS-assigned ephemeral port read back at runtime.
    pub listen_addr: String,
    /// How long one pass may run before it ends with exit 2 (B13).
    pub pass_deadline: Duration,
    /// How often one client may search (review H3).
    pub rate_limit: RateLimit,
    /// Whose `X-Forwarded-For` names the client.
    pub trusted_proxies: TrustedProxies,
    /// The TEST-ONLY fault to provoke (never set in a release build).
    pub test_fault: Option<TestFault>,
}

impl IndexerWiring {
    /// Construct the production wiring from the indexer's OWN config (env-seam now;
    /// `config.toml` later). NONE of the wired adapters can sign/publish or touch
    /// the user's `openlore.duckdb` — the capability boundary (ADR-023 / I-AV-5)
    /// is the ABSENCE of the signing identity / local store from this dep graph
    /// (`xtask check-arch`'s `indexer_holds_no_signing_or_local_store` rule).
    ///
    /// The purge capability (`OPENLORE_INDEXER_PURGE_UNLISTED=1`, ADR-082) is
    /// returned beside the wiring, for the pass runner; `None` = removed
    /// authors keep their claims.
    pub fn production(cfg: IndexerConfig) -> anyhow::Result<Wired> {
        let fallback_base = cfg
            .fallback
            .as_ref()
            .map(FallbackUrl::as_str)
            .unwrap_or_default();

        let clock = SystemClockAdapter::new();
        // SEPARATE index.duckdb (ADR-023).
        let caps = DuckDbCaps {
            memory_limit_mb: cfg.duckdb_memory_limit_mb,
            threads: cfg.duckdb_threads,
        };
        let index_store = Arc::new(
            IndexStoreAdapter::open_capped(&cfg.index_path, caps)
                .map_err(|err| anyhow::anyhow!("open index store: {err}"))?,
        );
        let purge_capability = cfg.purge_unlisted.then(|| {
            let purge = Arc::clone(&index_store) as SharedIndexPurge;
            if cfg.test_fault == Some(TestFault::PurgeFails) {
                failing_first_purge(purge)
            } else {
                purge
            }
        });
        let index_reads = if cfg.test_fault == Some(TestFault::SearchStoreReadFails) {
            unreadable_index()
        } else {
            Arc::clone(&index_store) as SharedIndexReads
        };
        // Read-only bounded PULL (ADR-024), SSRF-guarded (ADR-077 §4): every
        // outbound request obeys the transport policy, after DNS.
        let ingest_source = AtProtoIngestAdapter::guarded(fallback_base, cfg.policy);
        let repo_listing = AtProtoIngestAdapter::guarded(fallback_base, cfg.policy);
        // DID → PDS, SSRF-guarded like every other outbound request.
        let pds_lookup = IdentityLookup::guarded(&cfg.plc_endpoint, &cfg.plc_endpoint, cfg.policy);
        // VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
        let identity_resolve = AtProtoDidAdapter::resolve_only();
        // The query server is bound only for `serve` (Phase 04); the `ingest`
        // one-shot pass leaves it `None` (it never listens). `XrpcQueryServer::bind`
        // itself is still a scaffold (step 04-06) — NOT called on the ingest path.
        let query_server = None;

        let wiring = Self {
            store_condition: Arc::clone(&index_store),
            index_reads,
            index_store,
            ingest_source: Box::new(ingest_source),
            repo_listing: Box::new(repo_listing),
            pds_lookup: Box::new(pds_lookup),
            repo_dids: cfg.repo_dids,
            repo_dids_file: cfg.repo_dids_file,
            control_socket: cfg.control_socket,
            identity_resolve: Box::new(identity_resolve),
            query_server,
            clock: Box::new(clock),
            fallback: cfg.fallback,
            policy: cfg.policy,
            max_concurrent_fetches: cfg.max_concurrent_fetches,
            per_did_time_budget: cfg.per_did_time_budget,
            index_path: cfg.index_path,
            listen_addr: cfg.listen_addr,
            pass_deadline: cfg.pass_deadline,
            rate_limit: cfg.rate_limit,
            trusted_proxies: cfg.trusted_proxies,
            test_fault: cfg.test_fault,
        };
        Ok(Wired {
            wiring,
            purge_capability,
        })
    }

    /// Run the wire → PROBE → use gate: the `capability_boundary_probe` FIRST
    /// (ADR-023), then the per-adapter gauntlet + the query-server probe. Returns
    /// the first refusal so the composition root can emit `health.startup.refused`
    /// and exit 2. SCAFFOLD — wired once the probe bodies land.
    pub fn probe_all(&self) -> Result<(), ProbeRefusal> {
        capability_boundary_probe(
            &self.index_path,
            self.index_store.as_ref(),
            self.identity_resolve.as_ref(),
        )?;
        probe_gauntlet(
            self.index_store.as_ref(),
            self.ingest_source.as_ref(),
            self.identity_resolve.as_ref(),
        )?;
        origin_classification_probe()?;
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
/// 3. Dispatch the verb (`serve` answers searches over the index; `ingest` is a
///    one-shot bounded PULL pass; `stats` reports index coverage).
///
/// Bootstrap SCAFFOLD (step 01-04): the sequence is wired; the verb bodies are
/// `todo!()`.
pub fn run(command: Command) -> i32 {
    // Step 0: CONFIG — refuse a bad configuration before anything is wired or
    // contacted (ADR-077/078).
    let cfg = match parse_config(
        |name| std::env::var(name).ok(),
        BuildProfile::of_this_build(),
    ) {
        Ok(cfg) => cfg,
        Err(error) => {
            emit_health_startup_refused(&config_refusal(&error));
            return EXIT_FATAL;
        }
    };
    emit_config_loaded(&cfg);

    // Step 1: WIRE.
    let Wired {
        wiring,
        purge_capability,
    } = match IndexerWiring::production(cfg) {
        Ok(wired) => wired,
        Err(err) => {
            eprintln!("openlore-indexer: failed to construct adapter wiring: {err:#}");
            return EXIT_FATAL;
        }
    };

    // Step 2: PROBE (capability boundary + the per-adapter gauntlet). REFUSE to
    // start on any probe failure — emit `health.startup.refused` + exit 2.
    let probed = wiring.probe_all().and_then(|()| {
        purge_capability
            .as_ref()
            .map_or(Ok(()), |purge| check_probe("index_purge", purge.probe()))
    });
    if let Err(refusal) = probed {
        emit_health_startup_refused(&refusal);
        return EXIT_FATAL;
    }

    // Step 3: USE — dispatch the subcommand. Only `serve` runs passes that
    // purge, so only `serve` receives the purge capability.
    match command {
        Command::Serve => serve(&wiring, purge_capability),
        Command::Ingest => ingest(&wiring),
        Command::Stats => stats(&wiring),
        // `trigger` is dispatched by `main` before any of the above (M4).
        Command::Trigger => EXIT_FATAL,
    }
}

/// `openlore-indexer serve` — serve the `org.openlore.appview.searchClaims`
/// query surface over localhost HTTP (the B1 transport, ADR-027).
///
/// `serve` does not ingest: it answers searches over whatever `index.duckdb`
/// already holds (populated by `ingest` passes). It binds the query server on
/// the configured `listen_addr` (`:0` → an OS-assigned ephemeral port for
/// parallel-safety), prints the bound address as a structured
/// `indexer.serve.listening` event so a supervisor (the test harness) can read
/// the port back, then runs the hyper accept loop until the process is killed.
///
/// The query handler reuses the wiring's ONE index handle (ADR-080: the store is
/// opened exactly once per process) and sees it through `IndexReadPort` only
/// (B7) — see `search_handler`.
fn serve(wiring: &IndexerWiring, purge_capability: Option<SharedIndexPurge>) -> i32 {
    let control = match open_control_channel(wiring.control_socket.as_deref()) {
        Ok(control) => control,
        Err(refusal) => {
            emit_health_startup_refused(&refusal);
            return EXIT_FATAL;
        }
    };
    let stop_answering = AtomicBool::new(false);
    // The pass runner's dedicated thread lives for the whole of `serve`; it
    // runs the same pass over the same store handle (no second open), off the
    // HTTP executor (ADR-080 §2/§6). The control channel holds its start handle.
    std::thread::scope(|scope| {
        let runner = Arc::new(PassRunner::spawn_scoped_purging(
            scope,
            purge_capability,
            |pass, purge| in_serve_pass(wiring, pass, purge),
        ));
        if let Some((socket, listener)) = control {
            let answering = Arc::clone(&runner);
            let stop = &stop_answering;
            scope.spawn(move || control_answer(&listener, &answering, stop));
            if let Err(refusal) = control_channel_probe(&socket, control_round_trip(&socket)) {
                stop_answering.store(true, Ordering::Relaxed);
                emit_health_startup_refused(&refusal);
                return EXIT_FATAL;
            }
        }
        let stop = &stop_answering;
        scope.spawn(move || exit_when_the_store_is_unusable(wiring, stop));
        let status = runner.status_reader();
        drop(runner);
        let code = serve_searches(wiring, status);
        stop_answering.store(true, Ordering::Relaxed);
        code
    })
}

/// Watch the store for as long as `serve` runs: once it is unusable (a
/// poisoned connection), emit `indexer.store.unusable` and exit 2, so the
/// runtime restarts `serve` on a freshly opened store (ADR-080 §7). A process
/// that keeps answering on a dead handle is not allowed.
fn exit_when_the_store_is_unusable(wiring: &IndexerWiring, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if let Some(reason) = wiring.store_condition.unusable_reason() {
            emit_store_unusable(reason);
            std::process::exit(EXIT_FATAL);
        }
        std::thread::sleep(STORE_WATCH_PERIOD);
    }
}

/// `indexer.store.unusable`: the store cannot be used; `serve` exits 2 next.
fn emit_store_unusable(reason: UnusableReason) {
    emit_flushed(serde_json::json!({
        "event": "indexer.store.unusable",
        "adapter": "index_store",
        "reason": reason.token(),
    }));
}

/// Whether the store is usable, as `/healthz` reports it.
fn store_usability(wiring: &IndexerWiring) -> StoreUsability {
    let store = Arc::clone(&wiring.store_condition);
    Arc::new(move || match store.unusable_reason() {
        None => StoreHealth::Usable,
        Some(_) => StoreHealth::Unusable,
    })
}

/// The bound control socket, when one is configured.
#[cfg(unix)]
type ControlChannel = (PathBuf, std::os::unix::net::UnixListener);
#[cfg(not(unix))]
type ControlChannel = (PathBuf, std::convert::Infallible);

/// Bind the configured control socket (replacing a stale file); a bind
/// failure refuses the start.
#[cfg(unix)]
fn open_control_channel(socket: Option<&Path>) -> Result<Option<ControlChannel>, ProbeRefusal> {
    socket
        .map(|socket| {
            crate::control::bind(socket)
                .map(|listener| (socket.to_path_buf(), listener))
                .map_err(|err| control_channel_probe(socket, Err(err)).unwrap_err())
        })
        .transpose()
}

/// The control channel exists only on Unix: configuring one elsewhere refuses.
#[cfg(not(unix))]
fn open_control_channel(socket: Option<&Path>) -> Result<Option<ControlChannel>, ProbeRefusal> {
    match socket {
        None => Ok(None),
        Some(socket) => Err(control_channel_probe(
            socket,
            Err(std::io::Error::other("a control socket needs a Unix host")),
        )
        .unwrap_err()),
    }
}

#[cfg(unix)]
fn control_answer(
    listener: &std::os::unix::net::UnixListener,
    runner: &Arc<PassRunner>,
    stop: &AtomicBool,
) {
    crate::control::answer_triggers(listener, runner, stop);
}

#[cfg(not(unix))]
fn control_answer(never: &std::convert::Infallible, _: &Arc<PassRunner>, _: &AtomicBool) {
    match *never {}
}

#[cfg(unix)]
fn control_round_trip(socket: &Path) -> std::io::Result<()> {
    crate::control::probe_round_trip(socket)
}

#[cfg(not(unix))]
fn control_round_trip(_: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Bind the query server and answer searches (and `/healthz`, from the
/// runner's `status`) until the process is killed.
fn serve_searches(wiring: &IndexerWiring, status: StatusReader) -> i32 {
    let handler = search_handler(
        Arc::clone(&wiring.index_reads),
        Arc::new(emit_search_truncated),
    );

    let listen_addr: std::net::SocketAddr = match wiring.listen_addr.parse() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!(
                "openlore-indexer serve: invalid listen address {:?}: {err}",
                wiring.listen_addr
            );
            return EXIT_FATAL;
        }
    };

    // A current-thread runtime suffices for the walking-skeleton serve: hyper's
    // accept loop + the per-connection tasks run concurrently on the single-thread
    // executor (the CLI makes one query at a time). The indexer's `tokio` feature
    // set is `rt` + (via the query-server crate) `net`/`macros` — no multi-thread.
    let runtime = match current_thread_runtime() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer serve: build async runtime: {err}");
            return EXIT_FATAL;
        }
    };

    runtime.block_on(async move {
        let server = match XrpcQueryServer::bind(listen_addr, handler) {
            Ok(server) => server
                .with_health(health_handler(status, store_usability(wiring)))
                .with_limits(ServeLimits {
                    rate: wiring.rate_limit,
                    trusted_proxies: wiring.trusted_proxies.clone(),
                    ..ServeLimits::default()
                }),
            Err(err) => {
                eprintln!("openlore-indexer serve: bind query server: {err}");
                return EXIT_FATAL;
            }
        };
        // Emit the bound address so the supervisor (the test harness) can read the
        // ephemeral port back. The event is structural (an address; no claim
        // content) — the DevOps observability contract (WD-105). Flushed so a
        // line-reading supervisor sees it immediately.
        emit_flushed(serde_json::json!({
            "event": "indexer.serve.listening",
            "addr": server.local_addr().to_string(),
        }));

        match server.serve().await {
            Ok(()) => EXIT_SERVED,
            Err(err) => {
                eprintln!("openlore-indexer serve: serve loop failed: {err}");
                EXIT_FATAL
            }
        }
    })
}

/// The single-threaded runtime both `serve` and `ingest` run on.
pub(crate) fn current_thread_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

/// Log a search cut at the row cap: the dimension and the cap, never the
/// searched value (ADR-083 §3).
fn emit_search_truncated(cut: SearchTruncated) {
    let dimension = match cut.dimension {
        SearchDimension::Object => "object",
        SearchDimension::Subject => "subject",
        SearchDimension::Contributor => "contributor",
    };
    emit(serde_json::json!({
        "event": "indexer.search.truncated",
        "dimension": dimension,
        "cap": cut.cap,
    }));
}

/// `openlore-indexer stats` — report index coverage (claims indexed, distinct
/// authors, ingest lag). SCAFFOLD.
fn stats(_wiring: &IndexerWiring) -> i32 {
    // SCAFFOLD: true — index coverage report lands in Phase 03/04.
    todo!("openlore-indexer stats — index coverage report (Phase 03/04)")
}

/// A refused configuration as a startup refusal naming the variable and value.
fn config_refusal(error: &ConfigError) -> ProbeRefusal {
    ProbeRefusal {
        adapter: "config",
        reason: ports::ProbeRefusalReason::IndexerConfigInvalid,
        detail: error.to_string(),
        structured: serde_json::json!({
            "variable": error.variable,
            "value": error.value,
        }),
    }
}

/// `indexer.config.loaded` (stdout): what this run was configured with.
fn emit_config_loaded(cfg: &IndexerConfig) {
    let mut event = serde_json::json!({
        "event": "indexer.config.loaded",
        "repo_did_count": cfg.repo_dids.len(),
        "fallback_configured": cfg.fallback.is_some(),
        "max_concurrent_fetches": cfg.max_concurrent_fetches,
        "per_did_time_budget_secs": cfg.per_did_time_budget.as_secs(),
        "plc_endpoint": cfg.plc_endpoint,
        "transport_policy": cfg.policy.token(),
    });
    if let Some(fallback) = &cfg.fallback {
        event["fallback_url"] = fallback.as_str().into();
    }
    emit(event);
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
