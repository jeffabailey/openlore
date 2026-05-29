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

use adapter_atproto_did::AtProtoDidAdapter;
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_index_store::IndexStoreAdapter;
use adapter_system_clock::SystemClockAdapter;
use adapter_xrpc_query_server::XrpcQueryServer;
use appview_domain::{ingest_decision, IngestOutcome, RejectReason};
use ports::{ClockPort, IdentityResolvePort, IndexStorePort, IngestSourcePort};

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
}

/// The indexer's resolved configuration (the test analog of `config.toml`, read
/// from env-var seams; mirrors the CLI's `OPENLORE_*` seams). Production reads
/// `~/.config/openlore-indexer/config.toml`; the TOML path lands in a later step.
struct IndexerConfig {
    /// The SEPARATE `index.duckdb` path (ADR-023; NEVER the user's openlore.duckdb).
    index_path: PathBuf,
    /// The bounded ingest source base URL hosting public `listRecords` (ADR-024).
    source_url: String,
}

impl IndexerConfig {
    /// Resolve config from env-var seams. `OPENLORE_INDEXER_INDEX_PATH` /
    /// `OPENLORE_INDEXER_SOURCE_URL` override; otherwise fall back to the
    /// `OPENLORE_HOME`-anchored default path + an empty source.
    fn from_env() -> Self {
        let index_path = std::env::var("OPENLORE_INDEXER_INDEX_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_index_path());
        let source_url = std::env::var("OPENLORE_INDEXER_SOURCE_URL").unwrap_or_default();
        Self {
            index_path,
            source_url,
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

/// `openlore-indexer serve` — run the bounded pull-ingest loop + serve the
/// `org.openlore.appview.searchClaims` query surface (ADR-024/027). SCAFFOLD.
fn serve(_wiring: &IndexerWiring) -> i32 {
    // SCAFFOLD: true — the run loop (bounded PULL → verify-before-index gate →
    // upsert → serve queries) lands in Phase 03/04.
    todo!("openlore-indexer serve — run the pull-ingest loop + query server (Phase 03/04)")
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
        let resolved_key = match runtime
            .block_on(wiring.identity_resolve.resolve_verification_key(author))
        {
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

    let rejected_total =
        rejected_unsigned + rejected_bad_signature + rejected_cid_mismatch + rejected_schema_unknown;
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
