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

#![allow(dead_code)] // some scaffold seams (serve/stats) land in Phase 03/04

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::{self, StreamExt};

use adapter_atproto_did::{AtProtoDidAdapter, IdentityLookup};
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::IndexStoreAdapter;
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::{QueryHandler, XrpcQueryServer};
use std::collections::BTreeMap;

use appview_domain::ingest_pass::{
    classify_fetch_failure, pass_exit_code, refusal_cause_of, ClassifiedSkip, FetchFailure,
    RefusalCause, SkipReason,
};
use appview_domain::{
    compose_results, ingest_repo_record, origin_of, plan_listing, records_of, summarize, DidFetch,
    FallbackUrl, IngestOutcome, ListingPlan, ListingSource, NetworkSearchResult, PassSummary,
    RejectReason, ResolutionFailure,
};
use claim_domain::{ClaimRecord, Did, VerificationKey};
use lexicon::{
    ClaimReferenceDto, SearchDimensionDto, SearchQueryRequest, SearchQueryResponse, SearchResultDto,
};
use ports::net_policy::TransportPolicy;
use ports::{
    ClockPort, IdentityLookupError, IdentityLookupPort, IdentityResolvePort, IndexStorePort,
    IngestError, IngestSourcePort, RepoListingPort, SearchDimension,
};

use crate::config::parse_config;

use crate::probe_gauntlet::{
    capability_boundary_probe, origin_classification_probe, probe_gauntlet, ProbeRefusal,
};
use crate::Command;

/// The indexer's wired adapter set, owned by the composition root for the
/// duration of the program (mirrors the CLI's `Wiring`). Holds ONLY the
/// indexer's driven adapters — by construction NO signing identity + NO local
/// store (the capability boundary, ADR-023 / I-AV-5).
pub struct IndexerWiring {
    pub index_store: Box<dyn IndexStorePort>,
    pub ingest_source: Box<dyn IngestSourcePort>,
    /// Read-only `listRecords` of ONE repo DID, cursor-paged (ADR-071 §4).
    pub repo_listing: Box<dyn RepoListingPort>,
    /// DID → its DID document's PDS, resolved afresh every pass: where each
    /// repo is listed and the only origin a self-attested record may be
    /// indexed from (ADR-071, ADR-077).
    pub pds_lookup: Box<dyn IdentityLookupPort>,
    /// The repo DIDs one ingest pass enumerates (DWD-9).
    pub repo_dids: Vec<Did>,
    /// VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
    pub identity_resolve: Box<dyn IdentityResolvePort>,
    /// The query server is bound only for `serve` (Phase 04); the `ingest`
    /// one-shot pass leaves it `None` (it does not serve).
    pub query_server: Option<XrpcQueryServer>,
    pub clock: Box<dyn ClockPort>,
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
    pub fn production() -> anyhow::Result<Self> {
        let cfg = parse_config(|name| std::env::var(name).ok());
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
        // VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
        let identity_resolve = AtProtoDidAdapter::resolve_only();
        // The query server is bound only for `serve` (Phase 04); the `ingest`
        // one-shot pass leaves it `None` (it never listens). `XrpcQueryServer::bind`
        // itself is still a scaffold (step 04-06) — NOT called on the ingest path.
        let query_server = None;

        Ok(Self {
            index_store: Box::new(index_store),
            ingest_source: Box::new(ingest_source),
            repo_listing: Box::new(AtProtoIngestAdapter::guarded(fallback_base, cfg.policy)),
            pds_lookup: Box::new(IdentityLookup::guarded(
                &cfg.plc_endpoint,
                &cfg.plc_endpoint,
                cfg.policy,
            )),
            repo_dids: cfg.repo_dids,
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

/// `openlore-indexer serve` — serve the `org.openlore.appview.searchClaims`
/// query surface over localhost HTTP (the B1 transport, ADR-027).
///
/// The walking-skeleton serve path (04-01): the index is already populated (the
/// test harness runs a one-shot `ingest` pass FIRST, then `serve` over the same
/// `index.duckdb`). `serve` binds the query server on the configured
/// `listen_addr` (`:0` → an OS-assigned ephemeral port for parallel-safety),
/// prints the bound address as a structured `indexer.serve.listening` event so a
/// supervisor (the test harness) can read the port back, then runs the hyper
/// accept loop until the process is killed.
///
/// The query handler reads the `IndexStorePort` (the SEPARATE `index.duckdb`) and
/// composes per-author via the PURE `appview_domain::compose_results` (the SAME
/// pure core the layer-2 AVC-2 proves) — the wire carries FLAT attributed rows
/// (every `author_did` present; anti-merging across the transport, I-AV-2).
fn serve(wiring: &IndexerWiring) -> i32 {
    // A fresh handle to the SEPARATE index.duckdb for the serve handler. The
    // adapter is Send+Sync (its `Arc<Mutex<Connection>>` substrate is), so it can
    // be shared across the hyper accept loop's per-connection tasks. The wiring's
    // `index_store` already proved (via probe) the store is reachable; this reopen
    // is the long-lived serve handle.
    let store = match IndexStoreAdapter::open(&wiring.index_path) {
        Ok(s) => Arc::new(s),
        Err(err) => {
            eprintln!("openlore-indexer serve: open index store: {err}");
            return 2;
        }
    };

    let handler: QueryHandler = {
        let store = Arc::clone(&store);
        Arc::new(move |request: SearchQueryRequest| handle_search(store.as_ref(), request))
    };

    let listen_addr: std::net::SocketAddr = match wiring.listen_addr.parse() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!(
                "openlore-indexer serve: invalid listen address {:?}: {err}",
                wiring.listen_addr
            );
            return 2;
        }
    };

    // A current-thread runtime suffices for the walking-skeleton serve: hyper's
    // accept loop + the per-connection tasks run concurrently on the single-thread
    // executor (the CLI makes one query at a time). The indexer's `tokio` feature
    // set is `rt` + (via the query-server crate) `net`/`macros` — no multi-thread.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer serve: build async runtime: {err}");
            return 2;
        }
    };

    runtime.block_on(async move {
        let server = match XrpcQueryServer::bind(listen_addr, handler) {
            Ok(server) => server,
            Err(err) => {
                eprintln!("openlore-indexer serve: bind query server: {err}");
                return 2;
            }
        };
        // Emit the bound address so the supervisor (the test harness) can read the
        // ephemeral port back. The event is structural (an address; no claim
        // content) — the DevOps observability contract (WD-105).
        let listening = serde_json::json!({
            "event": "indexer.serve.listening",
            "addr": server.local_addr().to_string(),
        });
        println!("{listening}");
        // Flush stdout so a line-reading supervisor sees the event immediately.
        use std::io::Write;
        let _ = std::io::stdout().flush();

        match server.serve().await {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("openlore-indexer serve: serve loop failed: {err}");
                2
            }
        }
    })
}

/// The serve query handler: read the index store along `request.dimension`,
/// compose per-author via the PURE `appview_domain::compose_results`, and project
/// the per-author structure back to a FLAT attributed wire response (every
/// `author_did` present; the `distinct_author_count` is the pure COUNT, never a
/// merge). A store error degrades to an empty result (serve never panics on a
/// read failure; the CLI sees an empty-but-attributed response).
fn handle_search(store: &dyn IndexStorePort, request: SearchQueryRequest) -> SearchQueryResponse {
    let dimension = from_dto_dimension(request.dimension);
    let rows = match dimension {
        SearchDimension::Object => store.query_by_object(&request.value),
        SearchDimension::Subject => store.query_by_subject(&request.value),
        SearchDimension::Contributor => {
            store.query_by_contributor(&claim_domain::Did(request.value.clone()))
        }
    };
    let rows = rows.unwrap_or_default();

    // The per-author grouping + the distinct-author COUNT come from the PURE
    // composition (the SAME core proven at layer 2 by AVC-2). The author ORDER on
    // the wire follows that stable composition; the per-row payload is projected
    // from the original `IndexedClaim` rows (which carry composed_at + evidence the
    // composed `NetworkResultRow` does not). The wire stays FLAT + attributed.
    let composed = compose_results(rows.clone(), dimension);
    let results = flat_attributed_rows(&composed, &rows);
    SearchQueryResponse {
        results,
        distinct_author_count: composed.distinct_author_count,
        total_claims: composed.total_claims,
        suggestion: composed.suggestion,
    }
}

/// Project the per-author `NetworkSearchResult` (the pure composition's stable
/// author order + within-group cid order) into FLAT attributed wire rows, looking
/// each row's full payload (composed_at, evidence) up from the original
/// `IndexedClaim` rows by cid. The wire carries one row per attributed claim (NO
/// merged/consensus object — I-AV-2).
fn flat_attributed_rows(
    composed: &NetworkSearchResult,
    rows: &[ports::IndexedClaim],
) -> Vec<SearchResultDto> {
    let mut out = Vec::new();
    for (_author, group) in &composed.by_author {
        for composed_row in group {
            let source = rows.iter().find(|r| r.cid == composed_row.cid);
            let composed_at = source
                .map(|r| r.composed_at.to_rfc3339())
                .unwrap_or_default();
            let evidence = source.map(|r| r.evidence.clone()).unwrap_or_default();
            // Carry the row's typed references over the wire (OD-AV-7): a countering
            // claim K's `counters` reference to the countered claim C's CID lets the
            // CLI render reconstruct C's `countered-by <K.cid> (by <K.author>)`
            // annotation (shown, never applied — I-AV-9). The reference rows carry no
            // author (anti-merging preserved); K's author is K's own `author_did`.
            let references = source
                .map(|r| r.references.iter().map(reference_to_dto).collect())
                .unwrap_or_default();
            out.push(SearchResultDto {
                author_did: composed_row.author_did.0.clone(),
                cid: composed_row.cid.0.clone(),
                subject: composed_row.subject.clone(),
                predicate: composed_row.predicate.clone(),
                object: composed_row.object.clone(),
                confidence: composed_row.confidence,
                composed_at,
                verified_against: composed_row.verified_against.0.clone(),
                evidence,
                references,
            });
        }
    }
    out
}

/// Map a typed `claim_domain::ClaimReference` to its wire DTO, using the lowercase
/// `ref_type` token the `indexed_claim_references` CHECK domain + the on-disk
/// artifact use (so the wire, the store, and the artifact agree without drift).
fn reference_to_dto(reference: &claim_domain::ClaimReference) -> ClaimReferenceDto {
    let ref_type = match reference.ref_type {
        claim_domain::ReferenceType::Retracts => "retracts",
        claim_domain::ReferenceType::Corrects => "corrects",
        claim_domain::ReferenceType::Counters => "counters",
        claim_domain::ReferenceType::Supersedes => "supersedes",
    };
    ClaimReferenceDto {
        ref_type: ref_type.to_string(),
        cid: reference.cid.0.clone(),
    }
}

/// Map a wire DTO dimension to the domain `SearchDimension`.
fn from_dto_dimension(dim: SearchDimensionDto) -> SearchDimension {
    match dim {
        SearchDimensionDto::Object => SearchDimension::Object,
        SearchDimensionDto::Contributor => SearchDimension::Contributor,
        SearchDimensionDto::Subject => SearchDimension::Subject,
    }
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
    let started = Instant::now();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer: failed to build async runtime: {err}");
            return 2;
        }
    };

    let fetches: Vec<DidFetch> = runtime.block_on(
        stream::iter(&wiring.repo_dids)
            .map(|repo_did| fetch_repo(wiring, repo_did))
            .buffered(wiring.max_concurrent_fetches)
            .collect(),
    );

    fetches.iter().for_each(emit_fallback_read);
    fetches.iter().for_each(emit_source_skipped);

    let mut tally = IngestTally::default();
    for fetch in &fetches {
        if let Err(err) = gate_fetch(wiring, &runtime, fetch, &mut tally) {
            eprintln!("openlore-indexer: index upsert failed: {err}");
            return 2;
        }
    }

    tally.emit();
    let summary = summarize(&fetches);
    let exit_code = pass_exit_code(&summary);
    emit_pass_summary(&summary, exit_code, started);
    exit_code
}

/// One repo DID's fetch, under ONE deadline covering resolving and every
/// listing page: resolve its PDS, plan where to list it, list it. Any failure
/// skips only this DID, with its reason (ADR-078); nothing about the skip is
/// stored, so the DID is retried on the next pass.
async fn fetch_repo(wiring: &IndexerWiring, repo_did: &Did) -> DidFetch {
    let deadline = tokio::time::Instant::now() + wiring.per_did_time_budget;
    let resolution =
        match tokio::time::timeout_at(deadline, wiring.pds_lookup.resolve_pds(&repo_did.0)).await {
            Ok(resolved) => resolved.map_err(resolution_failure_of),
            Err(_elapsed) => Err(ResolutionFailure::TimedOut),
        };
    let pds_url = resolution.as_ref().ok().cloned();
    match plan_listing(resolution, wiring.policy, wiring.fallback.as_ref()) {
        ListingPlan::List(source) => list_source(wiring, repo_did, source, deadline).await,
        ListingPlan::Skip(reason) => DidFetch::Skipped {
            did: repo_did.clone(),
            skip: ClassifiedSkip::planned(reason),
            pds_url,
        },
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
        match ingest_repo_record(&record, &rkey, repo_did, origin, key.as_ref()) {
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
    }
    Ok(())
}

/// Emit `indexer.ingest.source_fallback` for a DID read through the fallback
/// (it was unresolvable — the only way onto that arm, ADR-077).
fn emit_fallback_read(fetch: &DidFetch) {
    if let DidFetch::Read {
        did,
        source: ListingSource::Fallback(fallback),
        ..
    } = fetch
    {
        let event = serde_json::json!({
            "event": "indexer.ingest.source_fallback",
            "did": did.0,
            "reason": SkipReason::DidUnresolvable.token(),
            "fallback_url": fallback.as_str(),
        });
        println!("{event}");
    }
}

/// Emit `indexer.ingest.source_skipped` for a DID that contributed nothing:
/// its DID and reason, the PDS only when one was resolved — NO claim content.
fn emit_source_skipped(fetch: &DidFetch) {
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
        println!("{event}");
    }
}

/// Emit `indexer.ingest.pass_summary` — the pass's LAST stdout event.
fn emit_pass_summary(summary: &PassSummary, exit_code: i32, started: Instant) {
    let event = serde_json::json!({
        "event": "indexer.ingest.pass_summary",
        "configured": summary.configured,
        "own_pds": summary.own_pds,
        "fallback": summary.fallback,
        "skipped": summary.skipped,
        "duration_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "exit_code": exit_code,
    });
    println!("{event}");
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
    fn emit(&self) {
        let by_reason: serde_json::Map<String, serde_json::Value> = RefusalCause::ALL
            .iter()
            .map(|cause| {
                let count = self.refused.get(cause).copied().unwrap_or_default();
                (cause.token().to_string(), count.into())
            })
            .collect();
        let verified_event = serde_json::json!({
            "event": "indexer.ingest.verified",
            "count": self.verified,
        });
        let rejected_event = serde_json::json!({
            "event": "indexer.ingest.rejected",
            "count": self.refused.values().sum::<u64>(),
            "by_reason": by_reason,
        });
        println!("{verified_event}");
        println!("{rejected_event}");
    }
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
