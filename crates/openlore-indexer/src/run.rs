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
use std::time::{Duration, Instant};

use clap::Subcommand;

use adapter_atproto_did::{AtProtoDidAdapter, IdentityLookup};
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::{DuckDbCaps, IndexStoreAdapter, UnusableReason};
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::{
    QueryHandler, RateLimit, ServeLimits, TrustedProxies, XrpcQueryServer,
};

use appview_domain::FallbackUrl;
use chrono::{DateTime, Utc};
use claim_domain::Did;
use ports::net_policy::TransportPolicy;
use ports::{
    ClockPort, IdentityLookupPort, IdentityResolvePort, IndexPurgePort, IndexStorePort,
    IngestSourcePort, RepoListingPort, SearchDimension,
};

use crate::config::{parse_config, BuildProfile, ConfigError, IndexerConfig, TestFault};
use crate::events::{Events, StdoutEvents};
use crate::health::{health_handler, store_usability, LastPublishedAt};
use crate::ingest_pass::{caught_pass_work, in_serve_pass, ingest, EndedPass};
use crate::pass_runner::{PassRunner, StatusReader};
pub use crate::probe_gauntlet::ProbeRefusal;
use crate::probe_gauntlet::{
    capability_boundary_probe, check_probe, control_channel_probe, origin_classification_probe,
    probe_gauntlet,
};
use crate::runner_contract::{failing_first_purge, PassLabel, RunnerPurge};
use crate::search_handler::{
    search_handler, unreadable_index, SearchStoreError, SearchTruncated, SharedIndexReads,
};

/// The indexer subcommands. `serve` is the long-running search server; `ingest`
/// is a one-shot bounded PULL; `stats` reports coverage.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve the search query surface over the index (ADR-027, ADR-080).
    Serve,
    /// Run a one-shot bounded PULL pass (ADR-024).
    Ingest,
    /// Report index coverage (claims indexed, distinct authors, ingest lag).
    Stats,
    /// Ask the running `serve` for one pass; exit with that pass's code
    /// (0 also when it joined a running pass; 4 when `serve` is unreachable).
    Trigger,
}

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

impl Wired {
    /// wire -> PROBE: every probe of the wiring, then the purge capability's
    /// (ADR-009). The only way to the wiring's use.
    pub fn probed(self) -> Result<ProbedWiring, ProbeRefusal> {
        self.wiring.probe_all()?;
        if let Some(purge) = &self.purge_capability {
            check_probe("index_purge", purge.probe())?;
        }
        Ok(ProbedWiring {
            wiring: self.wiring,
            purge_capability: self.purge_capability,
        })
    }
}

/// A wiring that passed its probes, so it can be used.
pub struct ProbedWiring {
    pub wiring: IndexerWiring,
    purge_capability: Option<SharedIndexPurge>,
}

impl ProbedWiring {
    /// One pass's work over this wiring, for a root with no pass runner. It
    /// takes the wiring, so the store is closed when the work ends and the file
    /// can be published (D-STZ-40a); the pass's summary is still to be written.
    #[must_use]
    pub fn pass_work(self, pass: PassLabel, started: Instant) -> EndedPass {
        let purge = self.purge_capability.as_deref().map(RunnerPurge::lent);
        caught_pass_work(&self.wiring, pass, purge, started)
    }
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
    /// Where the pass and the handlers write their event lines (UI-8).
    pub events: Events,
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
    /// authors keep their claims. The root builds `identity_resolve` itself
    /// (D-STZ-50): the verify-only path, never the signing `IdentityPort`.
    pub fn production(
        cfg: IndexerConfig,
        events: Events,
        identity_resolve: Box<dyn IdentityResolvePort>,
    ) -> anyhow::Result<Wired> {
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
            identity_resolve,
            query_server,
            clock: Box::new(clock),
            events,
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
    let events = Events::new(Arc::new(StdoutEvents));

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
    events.emit(&config_loaded_event(&cfg));

    // Step 1: WIRE.
    let wired = match IndexerWiring::production(cfg, events, verify_only_resolver()) {
        Ok(wired) => wired,
        Err(err) => {
            eprintln!("openlore-indexer: failed to construct adapter wiring: {err:#}");
            return EXIT_FATAL;
        }
    };

    // Step 2: PROBE (capability boundary + the per-adapter gauntlet). REFUSE to
    // start on any probe failure — emit `health.startup.refused` + exit 2.
    let ProbedWiring {
        wiring,
        purge_capability,
    } = match wired.probed() {
        Ok(probed) => probed,
        Err(refusal) => {
            emit_health_startup_refused(&refusal);
            return EXIT_FATAL;
        }
    };

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

/// VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
fn verify_only_resolver() -> Box<dyn IdentityResolvePort> {
    Box::new(AtProtoDidAdapter::resolve_only())
}

/// `trigger` reads ONLY the control-socket variable — no config, no probes, no
/// store (M4: the client must never take the index lock from `serve`).
#[cfg(unix)]
pub fn trigger() -> i32 {
    crate::control::trigger(std::env::var(crate::control::CONTROL_SOCKET_VAR).ok())
}

/// The control channel exists only on Unix: elsewhere `serve` is unreachable.
#[cfg(not(unix))]
pub fn trigger() -> i32 {
    eprintln!(
        "{}",
        serde_json::json!({
            "event": "indexer.trigger.unreachable",
            "cause": "unsupported_platform",
        })
    );
    4
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
            |pass, purge| in_serve_pass(wiring, pass, purge, Instant::now()),
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
            wiring.events.emit_flushed(&store_unusable_event(reason));
            std::process::exit(EXIT_FATAL);
        }
        std::thread::sleep(STORE_WATCH_PERIOD);
    }
}

/// `indexer.store.unusable`: the store cannot be used; `serve` exits 2 next.
fn store_unusable_event(reason: UnusableReason) -> serde_json::Value {
    serde_json::json!({
        "event": "indexer.store.unusable",
        "adapter": "index_store",
        "reason": reason.token(),
    })
}

/// The self-host's last publication, as `/healthz` reports it: the end of the
/// runner's last exit-0 pass.
fn last_successful_pass(status: StatusReader) -> LastPublishedAt {
    Arc::new(move || {
        status
            .status()
            .last_successful_pass_at()
            .map(DateTime::<Utc>::from)
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
    let handler = search_answers(Arc::clone(&wiring.index_reads), &wiring.events);

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
                .with_health(health_handler(
                    last_successful_pass(status),
                    store_usability(Arc::clone(&wiring.store_condition)),
                ))
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
        wiring.events.emit_flushed(&serde_json::json!({
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

/// Searches answered from `reads`, their truncations and store errors logged
/// through `events`: the query handler of both roots.
#[must_use]
pub fn search_answers(reads: SharedIndexReads, events: &Events) -> QueryHandler {
    let truncations = events.clone();
    let store_errors = events.clone();
    search_handler(
        reads,
        Arc::new(move |cut| truncations.emit(&search_truncated_event(cut))),
        Arc::new(move |failed| store_errors.emit(&search_store_error_event(failed))),
    )
}

/// The single-threaded runtime both `serve` and `ingest` run on.
pub(crate) fn current_thread_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

/// The log of a search cut at the row cap: the dimension and the cap, never
/// the searched value (ADR-083 §3).
fn search_truncated_event(cut: SearchTruncated) -> serde_json::Value {
    serde_json::json!({
        "event": "indexer.search.truncated",
        "dimension": dimension_token(cut.dimension),
        "cap": cut.cap,
    })
}

/// The log of a search the index could not answer (it is answered 500): the
/// dimension only, never the searched value or the store's error text
/// (ADR-080 §7, WD-105). For diagnosis; it pages nothing.
fn search_store_error_event(failed: SearchStoreError) -> serde_json::Value {
    serde_json::json!({
        "event": "indexer.search.store_error",
        "dimension": dimension_token(failed.dimension),
    })
}

/// The log token of a search dimension.
fn dimension_token(dimension: SearchDimension) -> &'static str {
    match dimension {
        SearchDimension::Object => "object",
        SearchDimension::Subject => "subject",
        SearchDimension::Contributor => "contributor",
    }
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
fn config_loaded_event(cfg: &IndexerConfig) -> serde_json::Value {
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
    event
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use ports::{IndexStoreError, ProbeOutcome, ProbeRefusalReason, PurgeReport};

    use super::*;

    /// A purge handle that fails its probe and may not be used.
    struct RefusingPurge;

    impl IndexPurgePort for RefusingPurge {
        fn probe(&self) -> ProbeOutcome {
            ProbeOutcome::Refused {
                reason: ProbeRefusalReason::StorageSchemaMismatch,
                detail: "the purge does not know this schema".to_string(),
                structured: serde_json::json!({}),
            }
        }

        fn indexed_authors(&self) -> Result<BTreeSet<String>, IndexStoreError> {
            panic!("a refused purge was used")
        }

        fn purge_author(&self, _bare_did: &str) -> Result<PurgeReport, IndexStoreError> {
            panic!("a refused purge was used")
        }
    }

    // bypass: one fixed wiring; the contract is which handle gets asked.
    #[test]
    fn a_purge_handle_that_fails_its_probe_stops_the_wiring_being_used() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let index_path = scratch.path().join("index.duckdb").display().to_string();
        let env = HashMap::from([("OPENLORE_INDEXER_INDEX_PATH", index_path)]);
        let config = parse_config(|name| env.get(name).cloned(), BuildProfile::Development)
            .expect("a configuration");
        let events = Events::new(Arc::new(StdoutEvents));
        let wired =
            IndexerWiring::production(config, events, verify_only_resolver()).expect("a wiring");
        let refusing = Wired {
            wiring: wired.wiring,
            purge_capability: Some(Arc::new(RefusingPurge)),
        };

        let refusal = refusing.probed().err().expect("the purge's refusal");

        assert_eq!(refusal.adapter, "index_purge");
    }
}
