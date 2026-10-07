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

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::{self, StreamExt};

use adapter_atproto_did::{AtProtoDidAdapter, IdentityLookup};
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::{DuckDbCaps, IndexStoreAdapter, UnusableReason};
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::XrpcQueryServer;
use std::collections::BTreeMap;

use appview_domain::did_list::read_did_list;
use appview_domain::health::StoreHealth;
use appview_domain::ingest_pass::{
    classify_fetch_failure, pass_exit, refusal_cause_of, summarize_outcomes, within_deadline,
    ClassifiedSkip, FetchFailure, PassExit, PassFailure, RefusalCause, SkipReason,
};
use appview_domain::{
    ingest_repo_record, origin_of, plan_listing, records_of, DidFetch, FallbackUrl, IngestOutcome,
    ListingBudget, ListingPlan, ListingSource, PassSummary, RejectReason, ResolutionFailure,
};
use claim_domain::{ClaimRecord, Did, VerificationKey};
use ports::net_policy::TransportPolicy;
use ports::{
    ClockPort, IdentityLookupError, IdentityLookupPort, IdentityResolvePort, IndexPurgePort,
    IndexStorePort, IngestError, IngestSourcePort, RepoListingPort, SearchDimension,
};

use crate::config::{
    parse_config, BuildProfile, ConfigError, IndexerConfig, TestFault, REPO_DIDS_FILE_VAR,
};

use crate::health::{health_handler, StoreUsability};
use crate::pass_runner::{
    failing_first_purge, purge_unlisted_authors, PassLabel, PassRunner, PurgeStep, PurgedAuthor,
    StatusReader,
};
use crate::probe_gauntlet::{
    capability_boundary_probe, check_probe, control_channel_probe, origin_classification_probe,
    probe_gauntlet, ProbeRefusal,
};
use crate::search_handler::{search_handler, unreadable_index, SearchTruncated, SharedIndexReads};
use crate::Command;
use appview_domain::pass_runner::PassId;

/// The one shared index handle: read + write side, safe to share across the
/// serve accept loop's tasks.
pub type SharedIndexStore = Arc<dyn IndexStorePort + Send + Sync>;

/// The delete side of the same handle, handed to the pass runner only (ADR-082).
pub type SharedIndexPurge = Arc<dyn IndexPurgePort + Send + Sync>;

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
    /// The same handle's purge capability when `OPENLORE_INDEXER_PURGE_UNLISTED=1`
    /// (ADR-082); `None` = removed authors keep their claims.
    pub index_purge: Option<SharedIndexPurge>,
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
    /// The TEST-ONLY fault to provoke (never set in a release build).
    pub test_fault: Option<TestFault>,
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
        let caps = DuckDbCaps {
            memory_limit_mb: cfg.duckdb_memory_limit_mb,
            threads: cfg.duckdb_threads,
        };
        let index_store = Arc::new(
            IndexStoreAdapter::open_capped(&cfg.index_path, caps)
                .map_err(|err| anyhow::anyhow!("open index store: {err}"))?,
        );
        let index_purge = cfg.purge_unlisted.then(|| {
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

        Ok(Self {
            store_condition: Arc::clone(&index_store),
            index_reads,
            index_store,
            index_purge,
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
            test_fault: cfg.test_fault,
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
        if let Some(purge) = &self.index_purge {
            check_probe("index_purge", purge.probe())?;
        }
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
    emit(serde_json::json!({
        "event": "indexer.store.unusable",
        "adapter": "index_store",
        "reason": reason.token(),
    }));
    use std::io::Write;
    let _ = std::io::stdout().flush();
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

/// One pass inside `serve`, ending with exactly one summary whatever happens
/// (B4): a refused list, a store failure and a panic are summarised like a
/// completed pass, each with its cause.
fn in_serve_pass(wiring: &IndexerWiring, pass: PassLabel) -> i32 {
    let started = Instant::now();
    let work = catch_unwind(AssertUnwindSafe(|| {
        provoke_test_fault(wiring, pass);
        listed_pass_work(wiring, pass, started)
    }))
    .unwrap_or_else(|_panic| PassWork::failed(PassFailure::PassPanicked));
    finish_pass(Some(pass), &work, started, wiring.pass_deadline)
}

/// The TEST-ONLY pass faults (debug builds only; config refuses the seam in a
/// release build): the first pass panics, or the store is poisoned.
fn provoke_test_fault(wiring: &IndexerWiring, pass: PassLabel) {
    match wiring.test_fault {
        Some(TestFault::FirstPassPanics) if pass.pass_id() == PassId(1) => {
            panic!("test fault: the first pass panics")
        }
        Some(TestFault::StorePoisoned) => wiring.store_condition.poison_for_test_fault(),
        _ => {}
    }
}

/// Load the DID list afresh (the file when one is configured, ADR-081),
/// announce it under the pass's label, do the pass's work. A refused list
/// fetches nothing.
fn listed_pass_work(wiring: &IndexerWiring, pass: PassLabel, started: Instant) -> PassWork {
    match pass_list(wiring) {
        Ok((repo_dids, source)) => {
            emit_pass_config_loaded(wiring, pass, &repo_dids, source);
            match purge_removed_authors(wiring, &repo_dids, pass) {
                Ok(purged_authors) => PassWork {
                    purged_authors,
                    ..pass_work(wiring, &repo_dids, Some(pass), started)
                },
                Err(purged_authors) => PassWork {
                    purged_authors,
                    ..PassWork::failed(PassFailure::PurgeFailed)
                },
            }
        }
        Err(refusal) => {
            emit_pass_refused(pass, &refusal);
            PassWork::failed(refusal.cause)
        }
    }
}

/// At pass start, before any fetch: when purging is enabled, purge the
/// authors this pass's loaded list no longer names (ADR-082), announcing
/// each. `Ok`/`Err` carry how many authors were purged; `Err` = the store
/// failed (exit 2, nothing is fetched; the next pass finishes the purge).
fn purge_removed_authors(
    wiring: &IndexerWiring,
    repo_dids: &[Did],
    pass: PassLabel,
) -> Result<u64, u64> {
    let Some(purge) = &wiring.index_purge else {
        return Ok(0);
    };
    match purge_unlisted_authors(purge.as_ref(), repo_dids) {
        PurgeStep::Suppressed(reason) => {
            emit_in(
                Some(pass),
                serde_json::json!({
                    "event": "indexer.ingest.purge_suppressed",
                    "reason": reason.token(),
                }),
            );
            Ok(0)
        }
        PurgeStep::Purged(purged) => Ok(emit_purged(pass, &purged)),
        PurgeStep::Failed { purged, error } => {
            eprintln!("openlore-indexer: purge failed: {error}");
            Err(emit_purged(pass, &purged))
        }
    }
}

/// `indexer.ingest.author_purged` per purged author (bare DID and count
/// only, no claim content); how many there were.
fn emit_purged(pass: PassLabel, purged: &[PurgedAuthor]) -> u64 {
    for author in purged {
        emit_in(
            Some(pass),
            serde_json::json!({
                "event": "indexer.ingest.author_purged",
                "did": author.did.0,
                "claims_removed": author.claims_removed,
            }),
        );
    }
    purged.len() as u64
}

/// Where a pass's DID list came from (`indexer.config.loaded.repo_dids_source`).
#[derive(Debug, Clone, Copy)]
enum ListSource {
    Env,
    /// The list file, last rendered `age_secs` ago (its mtime, B15); `None`
    /// when the platform keeps no modification time.
    File {
        age_secs: Option<u64>,
    },
}

impl ListSource {
    const fn token(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::File { .. } => "file",
        }
    }

    /// `repo_dids_age_secs` — reported for the file only.
    const fn age_secs(self) -> Option<u64> {
        match self {
            Self::Env => None,
            Self::File { age_secs } => age_secs,
        }
    }
}

/// Why a pass refused its DID list: the cause token and what to name.
#[derive(Debug)]
struct ListRefusal {
    cause: PassFailure,
    variable: &'static str,
    value: String,
}

/// This pass's DID list: the file read now when configured, else the
/// environment's list.
fn pass_list(wiring: &IndexerWiring) -> Result<(Vec<Did>, ListSource), ListRefusal> {
    let Some(file) = &wiring.repo_dids_file else {
        return Ok((wiring.repo_dids.clone(), ListSource::Env));
    };
    let (text, age_secs) = read_list_file(file).map_err(|_| ListRefusal {
        cause: PassFailure::ListUnreadable,
        variable: REPO_DIDS_FILE_VAR,
        value: file.display().to_string(),
    })?;
    read_did_list(&text)
        .map(|dids| (dids, ListSource::File { age_secs }))
        .map_err(|bad| ListRefusal {
            cause: PassFailure::ListMalformed,
            variable: REPO_DIDS_FILE_VAR,
            value: bad.entry,
        })
}

/// The list file's text and age, opened by PATH now (never a cached handle:
/// the host replaces the file by rename, H1) and aged from the same handle.
fn read_list_file(file: &Path) -> std::io::Result<(String, Option<u64>)> {
    use std::io::Read;
    let mut handle = std::fs::File::open(file)?;
    let mut text = String::new();
    handle.read_to_string(&mut text)?;
    let age_secs = handle
        .metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .map(|rendered| {
            std::time::SystemTime::now()
                .duration_since(rendered)
                .unwrap_or_default()
                .as_secs()
        });
    Ok((text, age_secs))
}

/// `indexer.ingest.pass_refused`: the list was refused; nothing is fetched.
fn emit_pass_refused(pass: PassLabel, refusal: &ListRefusal) {
    emit_in(
        Some(pass),
        serde_json::json!({
            "event": "indexer.ingest.pass_refused",
            "cause": refusal.cause.token(),
            "variable": refusal.variable,
            "value": refusal.value,
        }),
    );
}

/// `indexer.config.loaded` at a pass's start, after its list loaded.
fn emit_pass_config_loaded(
    wiring: &IndexerWiring,
    pass: PassLabel,
    repo_dids: &[Did],
    source: ListSource,
) {
    let mut loaded = serde_json::json!({
        "event": "indexer.config.loaded",
        "repo_did_count": repo_dids.len(),
        "repo_dids_source": source.token(),
        "fallback_configured": wiring.fallback.is_some(),
        "max_concurrent_fetches": wiring.max_concurrent_fetches,
        "per_did_time_budget_secs": wiring.per_did_time_budget.as_secs(),
        "transport_policy": wiring.policy.token(),
    });
    if let Some(age_secs) = source.age_secs() {
        loaded["repo_dids_age_secs"] = age_secs.into();
    }
    emit_in(Some(pass), loaded);
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
            Ok(server) => server.with_health(health_handler(status, store_usability(wiring))),
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

/// One pass over `repo_dids`, ending with its one summary; every event
/// carries `pass`'s label when the pass runs inside `serve`.
fn run_pass(wiring: &IndexerWiring, repo_dids: &[Did], pass: Option<PassLabel>) -> i32 {
    let started = Instant::now();
    let work = pass_work(wiring, repo_dids, pass, started);
    finish_pass(pass, &work, started, wiring.pass_deadline)
}

/// What a pass's work came to, before its summary is written: the per-DID
/// accounting and the local failure that ended it, if one did.
struct PassWork {
    summary: PassSummary,
    failure: Option<PassFailure>,
    /// Authors purged at the pass's start (ADR-082).
    purged_authors: u64,
}

impl PassWork {
    /// A pass that ended on `failure` before it fetched anything.
    fn failed(failure: PassFailure) -> Self {
        Self {
            summary: PassSummary::default(),
            failure: Some(failure),
            purged_authors: 0,
        }
    }
}

/// Write the pass's ONE summary (last of its events) and return its exit code;
/// a pass that ran past `deadline` failed, whatever it came to (B13).
fn finish_pass(
    pass: Option<PassLabel>,
    work: &PassWork,
    started: Instant,
    deadline: Duration,
) -> i32 {
    let exit = within_deadline(
        pass_exit(work.failure, &work.summary),
        started.elapsed(),
        deadline,
    );
    emit_in(pass, pass_summary_event(work, exit, started));
    exit.code()
}

/// The pass's work. Each DID is gated as soon as its fetch is in (configured
/// order), so an author's claims are searchable while slower authors are
/// still being fetched; the first store failure ends the pass (ADR-078 §2).
/// The pass ends with an explicit checkpoint (ADR-080 §6). Whatever is still
/// outstanding when the pass's deadline (from `started`) passes is abandoned:
/// the pass ends there, keeping every upsert already committed (B13).
fn pass_work(
    wiring: &IndexerWiring,
    repo_dids: &[Did],
    pass: Option<PassLabel>,
    started: Instant,
) -> PassWork {
    let deadline = tokio::time::Instant::from_std(started + wiring.pass_deadline);
    let runtime = match current_thread_runtime() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer: failed to build async runtime: {err}");
            return PassWork::failed(PassFailure::PassPanicked);
        }
    };

    let mut fetches = Box::pin(
        stream::iter(repo_dids)
            .map(|repo_did| fetch_repo(wiring, repo_did))
            .buffered(wiring.max_concurrent_fetches),
    );
    let mut outcomes = Vec::with_capacity(repo_dids.len());
    let mut tally = IngestTally::default();
    let mut author_keys = AuthorKeys::new(deadline);
    let mut failure = None;
    loop {
        let fetch = match runtime
            .block_on(async { tokio::time::timeout_at(deadline, fetches.next()).await })
        {
            Ok(Some(fetch)) => fetch,
            Ok(None) => break,
            Err(_deadline_passed) => {
                failure = Some(PassFailure::PassDeadlineExceeded);
                break;
            }
        };
        emit_fallback_read(pass, &fetch);
        emit_source_skipped(pass, &fetch);
        outcomes.push(fetch.outcome());
        if let Err(stop) = gate_fetch(wiring, &runtime, &fetch, &mut author_keys, &mut tally) {
            failure = Some(stop.failure());
            break;
        }
    }
    tally.emit(pass);
    checkpoint(wiring);
    PassWork {
        summary: summarize_outcomes(outcomes),
        failure,
        purged_authors: 0,
    }
}

/// The end-of-pass checkpoint. Everything it folds in is already committed,
/// so a failure is reported and changes nothing the pass reports.
fn checkpoint(wiring: &IndexerWiring) {
    if let Err(err) = wiring.index_store.checkpoint() {
        eprintln!("openlore-indexer: end-of-pass checkpoint failed: {err}");
    }
}

/// The single-threaded runtime both `serve` and `ingest` run on.
fn current_thread_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
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
    author_keys: &mut AuthorKeys,
    tally: &mut IngestTally,
) -> Result<(), GateStop> {
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
        let key = author_keys.of(wiring, runtime, &record)?;
        let outcome = ingest_repo_record(&record, &rkey, repo_did, origin, key.as_ref());
        record_outcome(wiring, repo_did, &rkey, outcome, tally).map_err(|err| {
            eprintln!("openlore-indexer: index upsert failed: {err}");
            GateStop::UpsertFailed
        })?;
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

/// `indexer.ingest.pass_summary` — the pass's LAST stdout event, naming the
/// cause on exit 2, with how many authors it purged.
fn pass_summary_event(work: &PassWork, exit: PassExit, started: Instant) -> serde_json::Value {
    let summary = &work.summary;
    let mut event = serde_json::json!({
        "event": "indexer.ingest.pass_summary",
        "configured": summary.configured,
        "own_pds": summary.own_pds,
        "fallback": summary.fallback,
        "skipped": summary.skipped,
        "duration_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "exit_code": exit.code(),
        "purged_authors": work.purged_authors,
    });
    if let Some(cause) = exit.cause() {
        event["cause"] = cause.token().into();
    }
    event
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

/// Why the gate stopped a pass early.
#[derive(Debug, Clone, Copy)]
enum GateStop {
    /// A store write failed (exit 2 `upsert_failed`).
    UpsertFailed,
    /// The pass's deadline passed while an author key was being resolved.
    DeadlineExceeded,
}

impl GateStop {
    const fn failure(self) -> PassFailure {
        match self {
            Self::UpsertFailed => PassFailure::UpsertFailed,
            Self::DeadlineExceeded => PassFailure::PassDeadlineExceeded,
        }
    }
}

/// The author keys one pass resolved (ADR-080 §8): each author DID is
/// resolved at most once per pass, and never past the pass's deadline.
struct AuthorKeys {
    resolved: BTreeMap<String, Option<VerificationKey>>,
    deadline: tokio::time::Instant,
}

impl AuthorKeys {
    fn new(deadline: tokio::time::Instant) -> Self {
        Self {
            resolved: BTreeMap::new(),
            deadline,
        }
    }

    /// The resolved verification key of an app-signed record's author
    /// (ADR-026 resolve-only path); `None` when it cannot be resolved (the
    /// gate then refuses the record). A self-attested record has no key.
    fn of(
        &mut self,
        wiring: &IndexerWiring,
        runtime: &tokio::runtime::Runtime,
        record: &ClaimRecord,
    ) -> Result<Option<VerificationKey>, GateStop> {
        let ClaimRecord::AppSigned(signed) = record else {
            return Ok(None);
        };
        let author_did = &signed.unsigned.author_did;
        if let Some(key) = self.resolved.get(&author_did.0) {
            return Ok(key.clone());
        }
        let resolving = wiring.identity_resolve.resolve_verification_key(author_did);
        let key = runtime
            .block_on(async { tokio::time::timeout_at(self.deadline, resolving).await })
            .map_err(|_deadline_passed| GateStop::DeadlineExceeded)?
            .ok();
        self.resolved.insert(author_did.0.clone(), key.clone());
        Ok(key)
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
