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

use adapter_atproto_did::AtProtoDidAdapter;
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::IndexStoreAdapter;
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::{QueryHandler, XrpcQueryServer};
use appview_domain::{
    compose_results, ingest_decision, IngestOutcome, NetworkSearchResult, RejectReason,
};
use lexicon::{
    ClaimReferenceDto, SearchDimensionDto, SearchQueryRequest, SearchQueryResponse, SearchResultDto,
};
use ports::{ClockPort, IdentityResolvePort, IndexStorePort, IngestSourcePort, SearchDimension};

use crate::probe_gauntlet::{capability_boundary_probe, probe_gauntlet, ProbeRefusal};
use crate::Command;

/// The indexer's wired adapter set, owned by the composition root for the
/// duration of the program (mirrors the CLI's `Wiring`). Holds ONLY the
/// indexer's driven adapters — by construction NO signing identity + NO local
/// store (the capability boundary, ADR-023 / I-AV-5).
pub struct IndexerWiring {
    pub index_store: Box<dyn IndexStorePort>,
    pub ingest_source: Box<dyn IngestSourcePort>,
    /// VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
    pub identity_resolve: Box<dyn IdentityResolvePort>,
    /// The query server is bound only for `serve` (Phase 04); the `ingest`
    /// one-shot pass leaves it `None` (it does not serve).
    pub query_server: Option<XrpcQueryServer>,
    pub clock: Box<dyn ClockPort>,
    /// The configured bounded ingest source base URL (ADR-024).
    pub source_url: String,
    /// The configured SEPARATE `index.duckdb` path (ADR-023). Threaded into the
    /// `capability_boundary_probe` so it can REFUSE if mis-wired against the
    /// user's `openlore.duckdb` (the capability boundary, I-AV-5).
    pub index_path: PathBuf,
    /// The HTTP/XRPC query surface listen address (ADR-027). `serve` binds it;
    /// `:0` resolves to an OS-assigned ephemeral port read back at runtime.
    pub listen_addr: String,
}

/// The indexer's resolved configuration (the test analog of `config.toml`, read
/// from env-var seams; mirrors the CLI's `OPENLORE_*` seams). Production reads
/// `~/.config/openlore-indexer/config.toml`; the TOML path lands in a later step.
struct IndexerConfig {
    /// The SEPARATE `index.duckdb` path (ADR-023; NEVER the user's openlore.duckdb).
    index_path: PathBuf,
    /// The bounded ingest source base URL hosting public `listRecords` (ADR-024).
    source_url: String,
    /// The HTTP/XRPC query surface listen address (ADR-027). `:0` for an
    /// OS-assigned ephemeral port (the parallel-safe test default; DEVOPS open-q 8).
    listen_addr: String,
}

impl IndexerConfig {
    /// Resolve config from env-var seams. `OPENLORE_INDEXER_INDEX_PATH` /
    /// `OPENLORE_INDEXER_SOURCE_URL` / `OPENLORE_INDEXER_LISTEN_ADDR` override;
    /// otherwise fall back to the `OPENLORE_HOME`-anchored default path + an empty
    /// source + an ephemeral localhost listen address.
    fn from_env() -> Self {
        let index_path = std::env::var("OPENLORE_INDEXER_INDEX_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_index_path());
        let source_url = std::env::var("OPENLORE_INDEXER_SOURCE_URL").unwrap_or_default();
        let listen_addr = std::env::var("OPENLORE_INDEXER_LISTEN_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:0".to_string());
        Self {
            index_path,
            source_url,
            listen_addr,
        }
    }
}

/// The `OPENLORE_HOME`-anchored default index path:
/// `<home>/.local/share/openlore-indexer/index.duckdb`.
fn default_index_path() -> PathBuf {
    let home = std::env::var("OPENLORE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".local")
        .join("share")
        .join("openlore-indexer")
        .join("index.duckdb")
}

impl IndexerWiring {
    /// Construct the production wiring from the indexer's OWN config (env-seam now;
    /// `config.toml` later). NONE of the wired adapters can sign/publish or touch
    /// the user's `openlore.duckdb` — the capability boundary (ADR-023 / I-AV-5)
    /// is the ABSENCE of the signing identity / local store from this dep graph
    /// (`xtask check-arch`'s `indexer_holds_no_signing_or_local_store` rule).
    pub fn production() -> anyhow::Result<Self> {
        let cfg = IndexerConfig::from_env();

        let clock = SystemClockAdapter::new();
        // SEPARATE index.duckdb (ADR-023).
        let index_store = IndexStoreAdapter::open(&cfg.index_path)
            .map_err(|err| anyhow::anyhow!("open index store: {err}"))?;
        // Read-only bounded PULL (ADR-024).
        let ingest_source = AtProtoIngestAdapter::new(&cfg.source_url);
        // VERIFY-ONLY resolve path (ADR-026) — never the signing `IdentityPort`.
        let identity_resolve = AtProtoDidAdapter::resolve_only();
        // The query server is bound only for `serve` (Phase 04); the `ingest`
        // one-shot pass leaves it `None` (it never listens). `XrpcQueryServer::bind`
        // itself is still a scaffold (step 04-06) — NOT called on the ingest path.
        let query_server = None;

        Ok(Self {
            index_store: Box::new(index_store),
            ingest_source: Box::new(ingest_source),
            identity_resolve: Box::new(identity_resolve),
            query_server,
            clock: Box::new(clock),
            source_url: cfg.source_url,
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

/// `openlore-indexer ingest` — a one-shot bounded PULL pass (ADR-024).
///
/// The pipeline (wire → probe → use already ran upstream): bounded `enumerate`
/// of public `listRecords` → for each record resolve the author's verification
/// key → run the PURE `appview_domain::ingest_decision` verify-before-index gate
/// (the SAME pure core; no second verification path, WD-104) → on `Index` upsert
/// the attributed row + write the JSON artifact + bump `verified`; on `Reject`
/// bump `rejected{reason}` (the adversarial records NEVER enter the index).
///
/// Emits `indexer.ingest.verified` (count) + `indexer.ingest.rejected` (count)
/// as structured stdout events (the DevOps observability contract — structural
/// counts + DIDs only, NO claim-content telemetry, WD-105).
fn ingest(wiring: &IndexerWiring) -> i32 {
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

    // Bounded PULL of the public listRecords surface (read-only, ADR-024).
    let records = match runtime.block_on(wiring.ingest_source.enumerate(&wiring.source_url)) {
        Ok(records) => records,
        Err(err) => {
            eprintln!("openlore-indexer: ingest source enumerate failed: {err}");
            return 2;
        }
    };

    let mut verified: u64 = 0;
    let mut rejected_unsigned: u64 = 0;
    let mut rejected_bad_signature: u64 = 0;
    let mut rejected_cid_mismatch: u64 = 0;
    let mut rejected_schema_unknown: u64 = 0;

    for record in &records {
        // Resolve the author's verification key (ADR-026 resolve-only path). A
        // resolution failure is a REJECT (we never index a claim we cannot
        // verify) — classified as BadSignature (the key authority is absent).
        let author = &record.raw_payload.unsigned.author_did;
        let resolved_key =
            match runtime.block_on(wiring.identity_resolve.resolve_verification_key(author)) {
                Ok(key) => key,
                Err(_) => {
                    rejected_bad_signature += 1;
                    continue;
                }
            };

        // The PURE verify-before-index gate (SAME core; no second path, WD-104).
        match ingest_decision(record, &resolved_key) {
            IngestOutcome::Index(claim) => {
                // Upsert the verified, attributed row + write the JSON artifact.
                if let Err(err) = wiring.index_store.upsert(&claim) {
                    eprintln!("openlore-indexer: index upsert failed: {err}");
                    return 2;
                }
                verified += 1;
            }
            IngestOutcome::Reject(reason) => match reason {
                RejectReason::Unsigned => rejected_unsigned += 1,
                RejectReason::BadSignature => rejected_bad_signature += 1,
                RejectReason::CidMismatch => rejected_cid_mismatch += 1,
                RejectReason::SchemaUnknown => rejected_schema_unknown += 1,
            },
        }
    }

    let rejected_total = rejected_unsigned
        + rejected_bad_signature
        + rejected_cid_mismatch
        + rejected_schema_unknown;
    emit_ingest_counters(
        verified,
        rejected_total,
        rejected_unsigned,
        rejected_bad_signature,
        rejected_cid_mismatch,
        rejected_schema_unknown,
    );
    0
}

/// Emit the structured `indexer.ingest.verified` + `indexer.ingest.rejected`
/// events to stdout (the DevOps observability contract). Structural counts +
/// per-reason breakdown ONLY — NO claim-content telemetry (WD-105 privacy).
#[allow(clippy::too_many_arguments)]
fn emit_ingest_counters(
    verified: u64,
    rejected_total: u64,
    rejected_unsigned: u64,
    rejected_bad_signature: u64,
    rejected_cid_mismatch: u64,
    rejected_schema_unknown: u64,
) {
    let verified_event = serde_json::json!({
        "event": "indexer.ingest.verified",
        "count": verified,
    });
    let rejected_event = serde_json::json!({
        "event": "indexer.ingest.rejected",
        "count": rejected_total,
        "by_reason": {
            "unsigned": rejected_unsigned,
            "bad_signature": rejected_bad_signature,
            "cid_mismatch": rejected_cid_mismatch,
            "schema_unknown": rejected_schema_unknown,
        },
    });
    println!("{verified_event}");
    println!("{rejected_event}");
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
