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
//! ingest of its own. `stats` is still a `todo!()` scaffold.

#![allow(dead_code)] // some scaffold seams (serve/stats) land in Phase 03/04

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::{self, StreamExt};

use adapter_atproto_did::{AtProtoDidAdapter, IdentityLookup};
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::IndexStoreAdapter;
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::XrpcQueryServer;
use std::collections::BTreeMap;

use appview_domain::ingest_pass::{
    classify_fetch_failure, pass_exit_code, refusal_cause_of, ClassifiedSkip, FetchFailure,
    RefusalCause, SkipReason,
};
use appview_domain::{
    ingest_repo_record, origin_of, plan_listing, records_of, summarize, DidFetch, FallbackUrl,
    IngestOutcome, ListingBudget, ListingPlan, ListingSource, PassSummary, RejectReason,
    ResolutionFailure,
};
use claim_domain::{ClaimRecord, Did, VerificationKey};
use ports::net_policy::TransportPolicy;
use ports::{
    ClockPort, IdentityLookupError, IdentityLookupPort, IdentityResolvePort, IndexStorePort,
    IngestError, IngestSourcePort, RepoListingPort,
};

use crate::config::{
    parse_config, parse_repo_dids, BuildProfile, ConfigError, IndexerConfig, REPO_DIDS_FILE_VAR,
};

use crate::pass_runner::{PassLabel, PassRunner};
use crate::probe_gauntlet::{
    capability_boundary_probe, control_channel_probe, origin_classification_probe, probe_gauntlet,
    ProbeRefusal,
};
use crate::search_handler::{search_handler, SharedIndexReads};
use crate::Command;

/// The one shared index handle: read + write side, safe to share across the
/// serve accept loop's tasks.
pub type SharedIndexStore = Arc<dyn IndexStorePort + Send + Sync>;

/// Refused to start (bad config, wiring, probe) or a fatal runtime/store failure.
const EXIT_FATAL: i32 = 2;
/// `serve` ran to completion.
const EXIT_SERVED: i32 = 0;

/// The indexer's wired adapter set, owned by the composition root for the
/// duration of the program (mirrors the CLI's `Wiring`). Holds ONLY the
/// indexer's driven adapters — by construction NO signing identity + NO local
/// store (the capability boundary, ADR-023 / I-AV-5).
pub struct IndexerWiring {
    /// The process's ONE handle on `index.duckdb` (ADR-080, B1): opened once
    /// here and shared by the probe, the writes and (through its read port
    /// only) the search handler.
    pub index_store: SharedIndexStore,
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
}

impl IndexerWiring {
    /// Construct the production wiring from the indexer's OWN config (env-seam now;
    /// `config.toml` later). NONE of the wired adapters can sign/publish or touch
    /// the user's `openlore.duckdb` — the capability boundary (ADR-023 / I-AV-5)
    /// is the ABSENCE of the signing identity / local store from this dep graph
    /// (`xtask check-arch`'s `indexer_holds_no_signing_or_local_store` rule).
    pub fn production(cfg: IndexerConfig) -> anyhow::Result<Self> {
        let fallback_base = cfg
            .fallback
            .as_ref()
            .map(FallbackUrl::as_str)
            .unwrap_or_default();

        let clock = SystemClockAdapter::new();
        // SEPARATE index.duckdb (ADR-023).
        let index_store = IndexStoreAdapter::open(&cfg.index_path)
            .map_err(|err| anyhow::anyhow!("open index store: {err}"))?;
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

        Ok(Self {
            index_store: Arc::new(index_store),
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
    let wiring = match IndexerWiring::production(cfg) {
        Ok(w) => w,
        Err(err) => {
            eprintln!("openlore-indexer: failed to construct adapter wiring: {err:#}");
            return EXIT_FATAL;
        }
    };

    // Step 2: PROBE (capability boundary + the per-adapter gauntlet). REFUSE to
    // start on any probe failure — emit `health.startup.refused` + exit 2.
    if let Err(refusal) = wiring.probe_all() {
        emit_health_startup_refused(&refusal);
        return EXIT_FATAL;
    }

    // Step 3: USE — dispatch the subcommand.
    match command {
        Command::Serve => serve(&wiring),
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
fn serve(wiring: &IndexerWiring) -> i32 {
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
        let runner = Arc::new(PassRunner::spawn_scoped(scope, |pass| {
            in_serve_pass(wiring, pass)
        }));
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
        drop(runner);
        let code = serve_searches(wiring);
        stop_answering.store(true, Ordering::Relaxed);
        code
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

/// One pass inside `serve`: load the DID list afresh (the file when one is
/// configured, ADR-081), announce it under the pass's label, run the pass.
fn in_serve_pass(wiring: &IndexerWiring, pass: PassLabel) -> i32 {
    let started = Instant::now();
    let (repo_dids, source) = match pass_list(wiring) {
        Ok(listed) => listed,
        Err(refusal) => return refuse_pass(pass, &refusal, started),
    };
    emit_pass_config_loaded(wiring, pass, &repo_dids, source);
    run_pass(wiring, &repo_dids, Some(pass))
}

/// Where a pass's DID list came from (`indexer.config.loaded.repo_dids_source`).
#[derive(Debug, Clone, Copy)]
enum ListSource {
    Env,
    File,
}

impl ListSource {
    const fn token(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::File => "file",
        }
    }
}

/// Why a pass refused its DID list: the cause token and what to name.
#[derive(Debug)]
struct ListRefusal {
    cause: &'static str,
    variable: &'static str,
    value: String,
}

/// This pass's DID list: the file read now when configured, else the
/// environment's list.
fn pass_list(wiring: &IndexerWiring) -> Result<(Vec<Did>, ListSource), ListRefusal> {
    let Some(file) = &wiring.repo_dids_file else {
        return Ok((wiring.repo_dids.clone(), ListSource::Env));
    };
    let text = std::fs::read_to_string(file).map_err(|_| ListRefusal {
        cause: "repo_dids_unreadable",
        variable: REPO_DIDS_FILE_VAR,
        value: file.display().to_string(),
    })?;
    parse_repo_dids(&text)
        .map(|dids| (dids, ListSource::File))
        .map_err(|error| ListRefusal {
            cause: "repo_dids_malformed",
            variable: REPO_DIDS_FILE_VAR,
            value: error.value,
        })
}

/// A refused list: no fetch, one `pass_refused`, then the summary (exit 2).
fn refuse_pass(pass: PassLabel, refusal: &ListRefusal, started: Instant) -> i32 {
    emit_in(
        Some(pass),
        serde_json::json!({
            "event": "indexer.ingest.pass_refused",
            "cause": refusal.cause,
            "variable": refusal.variable,
            "value": refusal.value,
        }),
    );
    let mut summary = pass_summary_event(&summarize(&[]), EXIT_FATAL, started);
    summary["cause"] = refusal.cause.into();
    emit_in(Some(pass), summary);
    EXIT_FATAL
}

/// `indexer.config.loaded` at a pass's start, after its list loaded.
fn emit_pass_config_loaded(
    wiring: &IndexerWiring,
    pass: PassLabel,
    repo_dids: &[Did],
    source: ListSource,
) {
    emit_in(
        Some(pass),
        serde_json::json!({
            "event": "indexer.config.loaded",
            "repo_did_count": repo_dids.len(),
            "repo_dids_source": source.token(),
            "fallback_configured": wiring.fallback.is_some(),
            "max_concurrent_fetches": wiring.max_concurrent_fetches,
            "per_did_time_budget_secs": wiring.per_did_time_budget.as_secs(),
            "transport_policy": wiring.policy.token(),
        }),
    );
}

/// Bind the query server and answer searches until the process is killed.
fn serve_searches(wiring: &IndexerWiring) -> i32 {
    let reads: SharedIndexReads = Arc::clone(&wiring.index_store) as SharedIndexReads;
    let handler = search_handler(reads);

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
            Ok(server) => server,
            Err(err) => {
                eprintln!("openlore-indexer serve: bind query server: {err}");
                return EXIT_FATAL;
            }
        };
        // Emit the bound address so the supervisor (the test harness) can read the
        // ephemeral port back. The event is structural (an address; no claim
        // content) — the DevOps observability contract (WD-105).
        emit(serde_json::json!({
            "event": "indexer.serve.listening",
            "addr": server.local_addr().to_string(),
        }));
        // Flush stdout so a line-reading supervisor sees the event immediately.
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match server.serve().await {
            Ok(()) => EXIT_SERVED,
            Err(err) => {
                eprintln!("openlore-indexer serve: serve loop failed: {err}");
                EXIT_FATAL
            }
        }
    })
}

/// `openlore-indexer ingest` — a one-shot bounded PULL pass (ADR-024, ADR-077).
///
/// Fetch phase, per configured repo DID (DWD-9): resolve the DID's document
/// afresh (never cached across passes) → the PURE `plan_listing` decides the
/// listing source (its own PDS, else the fallback, else skip) → `listRecords`
/// with `repo=<DID>`, every cursor followed within the page bound.
///
/// Gate phase, in configured order: the PURE `records_of` keeps the repo's own
/// records → `origin_of` the listing source → resolve the author key for an
/// app-signed record → the PURE `ingest_repo_record` gate → on `Index` upsert
/// the attributed row; on `Reject` count the reason.
///
/// Emits `indexer.ingest.verified`, `indexer.ingest.rejected` and, last,
/// `indexer.ingest.pass_summary` on stdout (structural counts + DIDs only, NO
/// claim-content telemetry, WD-105).
fn ingest(wiring: &IndexerWiring) -> i32 {
    run_pass(wiring, &wiring.repo_dids, None)
}

/// One pass over `repo_dids`; every event carries `pass`'s label when the
/// pass runs inside `serve`.
fn run_pass(wiring: &IndexerWiring, repo_dids: &[Did], pass: Option<PassLabel>) -> i32 {
    let started = Instant::now();
    let runtime = match current_thread_runtime() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer: failed to build async runtime: {err}");
            return EXIT_FATAL;
        }
    };

    let fetches = fetch_all(wiring, repo_dids, &runtime);
    fetches
        .iter()
        .for_each(|fetch| emit_fallback_read(pass, fetch));
    fetches
        .iter()
        .for_each(|fetch| emit_source_skipped(pass, fetch));

    let tally = match gate_all(wiring, &runtime, &fetches) {
        Ok(tally) => tally,
        Err(err) => {
            eprintln!("openlore-indexer: index upsert failed: {err}");
            return EXIT_FATAL;
        }
    };
    tally.emit(pass);

    let summary = summarize(&fetches);
    let exit_code = pass_exit_code(&summary);
    emit_in(pass, pass_summary_event(&summary, exit_code, started));
    exit_code
}

/// The single-threaded runtime both `serve` and `ingest` run on.
fn current_thread_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

/// The fetch phase: every configured repo DID, at most
/// `max_concurrent_fetches` at once, results in configured order.
fn fetch_all(
    wiring: &IndexerWiring,
    repo_dids: &[Did],
    runtime: &tokio::runtime::Runtime,
) -> Vec<DidFetch> {
    runtime.block_on(
        stream::iter(repo_dids)
            .map(|repo_did| fetch_repo(wiring, repo_did))
            .buffered(wiring.max_concurrent_fetches)
            .collect(),
    )
}

/// The gate phase: every fetched repo through the verify/provenance gate, in
/// configured order. `Err` only for a store failure (fatal).
fn gate_all(
    wiring: &IndexerWiring,
    runtime: &tokio::runtime::Runtime,
    fetches: &[DidFetch],
) -> Result<IngestTally, String> {
    let mut tally = IngestTally::default();
    for fetch in fetches {
        gate_fetch(wiring, runtime, fetch, &mut tally)?;
    }
    Ok(tally)
}

/// One repo DID's fetch: resolve its PDS, plan where to list it, list it.
/// Resolving and an own-PDS listing share ONE deadline; a fallback listing
/// runs under a fresh one (the pure plan says which — ADR-078 §4 amended).
/// Any failure skips only this DID, with its reason (ADR-078); nothing about
/// the skip is stored, so the DID is retried on the next pass.
async fn fetch_repo(wiring: &IndexerWiring, repo_did: &Did) -> DidFetch {
    let shared_deadline = tokio::time::Instant::now() + wiring.per_did_time_budget;
    let resolution = resolve_pds(wiring, repo_did, shared_deadline).await;
    let pds_url = resolution.as_ref().ok().cloned();
    match plan_listing(resolution, wiring.policy, wiring.fallback.as_ref()) {
        ListingPlan::List(source) => {
            let deadline = match source.budget() {
                ListingBudget::RemainingOfShared => shared_deadline,
                ListingBudget::Fresh => tokio::time::Instant::now() + wiring.per_did_time_budget,
            };
            list_source(wiring, repo_did, source, deadline).await
        }
        ListingPlan::Skip(reason) => DidFetch::Skipped {
            did: repo_did.clone(),
            skip: ClassifiedSkip::planned(reason),
            pds_url,
        },
    }
}

/// Resolve `repo_did`'s PDS afresh before `deadline` (never cached).
async fn resolve_pds(
    wiring: &IndexerWiring,
    repo_did: &Did,
    deadline: tokio::time::Instant,
) -> Result<String, ResolutionFailure> {
    match tokio::time::timeout_at(deadline, wiring.pds_lookup.resolve_pds(&repo_did.0)).await {
        Ok(resolved) => resolved.map_err(resolution_failure_of),
        Err(_elapsed) => Err(ResolutionFailure::TimedOut),
    }
}

/// List `repo_did` from its planned source before `deadline`; a failed or
/// timed-out listing becomes a classified skip naming the source that failed.
async fn list_source(
    wiring: &IndexerWiring,
    repo_did: &Did,
    source: ListingSource,
    deadline: tokio::time::Instant,
) -> DidFetch {
    let listing = wiring
        .repo_listing
        .list_repo_claims(source.base(), &repo_did.0);
    let failure = match tokio::time::timeout_at(deadline, listing).await {
        Ok(Ok(listing)) => {
            return DidFetch::Read {
                did: repo_did.clone(),
                source,
                listing,
            }
        }
        Ok(Err(err)) => {
            eprintln!("openlore-indexer: listing {} failed: {err}", repo_did.0);
            fetch_failure_of(&err)
        }
        Err(_elapsed) => {
            eprintln!("openlore-indexer: listing {} timed out", repo_did.0);
            FetchFailure::TimedOut
        }
    };
    DidFetch::Skipped {
        did: repo_did.clone(),
        skip: classify_fetch_failure(&source, failure),
        pds_url: Some(source.base().to_string()),
    }
}

/// How a listing error is classified by the pure core.
fn fetch_failure_of(error: &IngestError) -> FetchFailure {
    match error {
        IngestError::Unreachable { .. } => FetchFailure::Unreachable,
        IngestError::BadResponse { .. } | IngestError::ProbeRefused { .. } => {
            FetchFailure::BadResponse
        }
        IngestError::AddressRefused { .. } => FetchFailure::AddressRefused,
    }
}

/// Why a DID's PDS could not be resolved, as the pure planner sees it.
fn resolution_failure_of(error: IdentityLookupError) -> ResolutionFailure {
    match error {
        IdentityLookupError::NotFound => ResolutionFailure::NotFound,
        IdentityLookupError::Unavailable { .. } => ResolutionFailure::Unavailable,
    }
}

/// Run one fetched repo's records through the verify/provenance gate and
/// upsert the admitted ones. `Err` only for a store failure (fatal, exit 2).
fn gate_fetch(
    wiring: &IndexerWiring,
    runtime: &tokio::runtime::Runtime,
    fetch: &DidFetch,
    tally: &mut IngestTally,
) -> Result<(), String> {
    let DidFetch::Read {
        did: repo_did,
        source,
        listing,
    } = fetch
    else {
        return Ok(());
    };
    let origin = origin_of(source, &listing.fetched_from);
    let bound = records_of(repo_did, &listing.records);
    tally.count(RefusalCause::ForeignRepo, bound.foreign);
    for (rkey, decoded) in bound.own {
        let Ok(record) = decoded else {
            tally.reject(&RejectReason::SchemaUnknown);
            continue;
        };
        let key = author_key(wiring, runtime, &record);
        let outcome = ingest_repo_record(&record, &rkey, repo_did, origin, key.as_ref());
        record_outcome(wiring, repo_did, &rkey, outcome, tally)?;
    }
    Ok(())
}

/// Act on one record's gate decision: upsert an admitted claim, count a
/// refusal (naming a provenance refusal on stderr). `Err` only for a store
/// failure.
fn record_outcome(
    wiring: &IndexerWiring,
    repo_did: &Did,
    rkey: &str,
    outcome: IngestOutcome,
    tally: &mut IngestTally,
) -> Result<(), String> {
    match outcome {
        IngestOutcome::Index(claim) => {
            wiring
                .index_store
                .upsert(&claim)
                .map_err(|err| err.to_string())?;
            tally.verified += 1;
        }
        IngestOutcome::Reject(reason) => {
            if let RejectReason::Provenance(rejection) = &reason {
                eprintln!(
                    "openlore-indexer: refused at://{}/org.openlore.claim/{rkey}: {rejection}",
                    repo_did.0
                );
            }
            tally.reject(&reason);
        }
    }
    Ok(())
}

/// Emit `indexer.ingest.source_fallback` for a DID read through the fallback
/// (it was unresolvable — the only way onto that arm, ADR-077).
fn emit_fallback_read(pass: Option<PassLabel>, fetch: &DidFetch) {
    if let DidFetch::Read {
        did,
        source: ListingSource::Fallback(fallback),
        ..
    } = fetch
    {
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.source_fallback",
                "did": did.0,
                "reason": SkipReason::DidUnresolvable.token(),
                "fallback_url": fallback.as_str(),
            }),
        );
    }
}

/// Emit `indexer.ingest.source_skipped` for a DID that contributed nothing:
/// its DID and reason, the PDS only when one was resolved — NO claim content.
fn emit_source_skipped(pass: Option<PassLabel>, fetch: &DidFetch) {
    if let DidFetch::Skipped { did, skip, pds_url } = fetch {
        let mut event = serde_json::json!({
            "event": "indexer.ingest.source_skipped",
            "did": did.0,
            "reason": skip.reason.token(),
            "fallback_used": skip.fallback_used(),
        });
        if let Some(pds_url) = pds_url {
            event["pds_url"] = serde_json::Value::from(pds_url.as_str());
        }
        if let Some(fallback_failure) = skip.fallback_failure {
            event["fallback_failure"] = serde_json::Value::from(fallback_failure.token());
        }
        emit_in(pass, event);
    }
}

/// `indexer.ingest.pass_summary` — the pass's LAST stdout event. No pass
/// purges yet, so `purged_authors` is 0.
fn pass_summary_event(
    summary: &PassSummary,
    exit_code: i32,
    started: Instant,
) -> serde_json::Value {
    serde_json::json!({
        "event": "indexer.ingest.pass_summary",
        "configured": summary.configured,
        "own_pds": summary.own_pds,
        "fallback": summary.fallback,
        "skipped": summary.skipped,
        "duration_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "exit_code": exit_code,
        "purged_authors": 0,
    })
}

/// Print one structured event as a stdout line.
fn emit(event: serde_json::Value) {
    println!("{event}");
}

/// Print one pass event, stamped with the pass's label when it has one.
fn emit_in(pass: Option<PassLabel>, mut event: serde_json::Value) {
    if let Some(pass) = pass {
        event["pass_id"] = pass.to_string().into();
    }
    emit(event);
}

/// The resolved verification key of an app-signed record's author (ADR-026
/// resolve-only path); `None` when it cannot be resolved (the gate then
/// refuses the record). A self-attested record has no key to resolve.
fn author_key(
    wiring: &IndexerWiring,
    runtime: &tokio::runtime::Runtime,
    record: &ClaimRecord,
) -> Option<VerificationKey> {
    match record {
        ClaimRecord::AppSigned(signed) => runtime
            .block_on(
                wiring
                    .identity_resolve
                    .resolve_verification_key(&signed.unsigned.author_did),
            )
            .ok(),
        ClaimRecord::SelfAttested(_) => None,
    }
}

/// The counts one ingest pass reports.
#[derive(Debug, Default)]
struct IngestTally {
    verified: u64,
    refused: BTreeMap<RefusalCause, u64>,
}

impl IngestTally {
    fn count(&mut self, cause: RefusalCause, times: u64) {
        *self.refused.entry(cause).or_default() += times;
    }

    fn reject(&mut self, reason: &RejectReason) {
        self.count(refusal_cause_of(reason), 1);
    }

    /// Emit the structured `indexer.ingest.verified` + `indexer.ingest.rejected`
    /// events to stdout (the DevOps observability contract). Structural counts +
    /// per-reason breakdown ONLY — NO claim-content telemetry (WD-105 privacy).
    fn emit(&self, pass: Option<PassLabel>) {
        let by_reason: serde_json::Map<String, serde_json::Value> = RefusalCause::ALL
            .iter()
            .map(|cause| {
                let count = self.refused.get(cause).copied().unwrap_or_default();
                (cause.token().to_string(), count.into())
            })
            .collect();
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.verified",
                "count": self.verified,
            }),
        );
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.rejected",
                "count": self.refused.values().sum::<u64>(),
                "by_reason": by_reason,
            }),
        );
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
