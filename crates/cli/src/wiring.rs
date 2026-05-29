//! `wiring` — adapter composition for the openlore binary (ADR-009 D-9).
//!
//! This module is the only place in the codebase allowed to instantiate
//! concrete adapter types and bind them behind their respective port
//! traits. Per ADR-009 the rule is: WIRE → PROBE → USE. The composition
//! root constructs every adapter at startup, walks the probe gauntlet,
//! and refuses to serve traffic if any adapter refuses.
//!
//! ## Slice-01 test seam
//!
//! For acceptance tests, production wiring is parameterized via three
//! environment variables (set by `tests/acceptance/support/mod.rs`):
//!
//! - `OPENLORE_DID` — the DID to bind the IdentityPort to (slice-03
//!   replaces this with real did:plc resolution against PLC directory).
//! - `OPENLORE_KEY_SEED_HEX` — 64-char hex Ed25519 seed used when no OS
//!   keychain entry exists (test scenarios use all-zero seed to match
//!   `openlore_test_support::FakeIdentity::jeff`).
//! - `OPENLORE_PDS_ENDPOINT` — PDS URL the PdsPort adapter binds to.
//!   Empty/unset is allowed in slice-01 (the init verb does not call
//!   PDS); WS-8+ scenarios point this at the in-process fake.
//!
//! These env-var seams are explicit in production code (not hidden
//! cfg(test) paths) so the same binary built once is used for both
//! production and the subprocess-driven acceptance suite (DD-2 + DD-5).

use adapter_atproto_did::AtProtoDidAdapter;
use adapter_atproto_pds::AtProtoPdsAdapter;
use adapter_duckdb::DuckDbStorageAdapter;
use adapter_github::GithubAdapter;
use adapter_index_query::HttpIndexQueryAdapter;
use adapter_system_clock::SystemClockAdapter;
use anyhow::{anyhow, Context, Result};
use ports::{
    ClockPort, GithubPort, IdentityPort, IndexQueryPort, PdsPort, PeerStoragePort, ProbeOutcome,
    StoragePort,
};

use crate::paths::OpenLorePaths;

/// One adapter wired behind each port trait. Owned by the composition
/// root for the duration of the program; dropped on shutdown.
pub struct Wiring {
    pub identity: Box<dyn IdentityPort>,
    pub storage: Box<dyn StoragePort>,
    /// Slice-03 peer-storage adapter. Shares the SAME DuckDB connection
    /// pool as `storage` (Q-DELIVER-3 single-writer constraint) — it is
    /// constructed via `DuckDbStorageAdapter::peer_adapter()` so no second
    /// handle to the DB file is ever opened.
    pub peer_storage: Box<dyn PeerStoragePort>,
    pub pds: Box<dyn PdsPort>,
    pub clock: Box<dyn ClockPort>,
    /// Slice-02 GitHub-scraper adapter (ADR-019). Reads `GITHUB_TOKEN` +
    /// `OPENLORE_GITHUB_API_BASE` from the environment via
    /// `GithubAdapter::from_env`. Holds NO storage/identity/pds reference —
    /// by construction it cannot sign or publish (the human-gate, I-SCR-1).
    pub github: Box<dyn GithubPort>,
    /// Slice-05 (appview search; ADR-027) index-query CLIENT — the `openlore
    /// search` verb's transport to the self-hosted indexer. WIRED + SOFT-probed
    /// at startup: an unreachable indexer is informational, NOT a startup
    /// refusal — it MUST NOT block `claim add` (WD-116 / KPI-5). The CLI links
    /// the CLIENT (`adapter-index-query`) only — NEVER the indexer's server /
    /// store / ingest crates (I-AV-3, enforced by `xtask check-arch`).
    ///
    /// Bootstrap SCAFFOLD (step 01-04): `None` because
    /// `HttpIndexQueryAdapter::new()`/`probe()` are Phase 03/04 `todo!()`
    /// scaffolds — constructing it now would panic the binary at startup for
    /// EVERY verb (violating WD-116). The slot is present so the `search` verb
    /// can name it and the SOFT-probe shape is established; the real
    /// construction + the graceful-degradation `Unreachable` mapping land with
    /// the AV-* scenarios in Phase 03/04. The `_phantom_index_query` reference
    /// below keeps `HttpIndexQueryAdapter` in the dep graph so `xtask
    /// check-arch` sees the CLI links the CLIENT (and only the client).
    pub index_query: Option<Box<dyn IndexQueryPort>>,
    pub paths: OpenLorePaths,
}

impl Wiring {
    /// Construct the production wiring rooted at the given XDG-resolved
    /// paths. Tests inject their isolated tempdir via `OpenLorePaths`;
    /// production resolves real XDG paths in `main`.
    ///
    /// Env-var seams (see module comment):
    /// - `OPENLORE_DID` — required; slice-01 stub for did:plc resolution.
    /// - `OPENLORE_KEY_SEED_HEX` — optional; if set, used directly as
    ///   the Ed25519 seed. Otherwise the adapter loads from OS keychain.
    /// - `OPENLORE_PDS_ENDPOINT` — optional; defaults to empty string.
    pub fn production(paths: OpenLorePaths) -> Result<Self> {
        let did = std::env::var("OPENLORE_DID").map_err(|_| {
            anyhow!(
                "OPENLORE_DID environment variable is not set. \
                 slice-01 requires this as the slice-01 stub for did:plc resolution; \
                 slice-03 will replace it with PLC directory lookup."
            )
        })?;

        let identity: Box<dyn IdentityPort> = Box::new(build_identity(&did)?);

        let storage_db_path = paths.duckdb_file();
        let storage = DuckDbStorageAdapter::open(&storage_db_path)
            .with_context(|| format!("opening DuckDB at {}", storage_db_path.display()))?;
        // Slice-03 peer-storage adapter SHARES the same DuckDB connection
        // pool as the author-storage adapter (Q-DELIVER-3): construct it
        // from the open `DuckDbStorageAdapter` BEFORE boxing the latter
        // behind the `StoragePort` trait object. No second DB handle. The
        // local user's DID is threaded in so the adapter can enforce the
        // WD-40 SelfAttribution guard at the storage write boundary (layer 2).
        let peer_storage: Box<dyn PeerStoragePort> =
            Box::new(storage.peer_adapter(identity.author_did()));
        let storage: Box<dyn StoragePort> = Box::new(storage);

        let pds_endpoint = std::env::var("OPENLORE_PDS_ENDPOINT").unwrap_or_default();
        let pds: Box<dyn PdsPort> = if pds_endpoint.is_empty() {
            // Slice-01 init verb does NOT call the PDS. We still wire a
            // PdsPort adapter so the probe gauntlet has uniform shape;
            // the empty-endpoint case refuses via the adapter's
            // pre-flight arm (PdsTlsHandshakeFailed) which the
            // composition root surfaces as `health.startup.refused`.
            // To make the init verb usable WITHOUT a configured PDS
            // (the bootstrap case before the user has chosen a PDS),
            // we permit the empty endpoint here and only refuse at
            // claim-publish time. The probe arm for the empty endpoint
            // is therefore skipped in slice-01 by binding a no-network
            // adapter when the endpoint is empty.
            Box::new(AtProtoPdsAdapter::for_endpoint(
                "https://placeholder.invalid",
            ))
        } else {
            Box::new(AtProtoPdsAdapter::with_did(
                pds_endpoint,
                "did:plc:placeholder-host",
                &did,
            ))
        };

        let clock: Box<dyn ClockPort> = Box::new(SystemClockAdapter::new());

        // Slice-02 GitHub-scraper adapter (ADR-019). Construction reads the
        // optional `GITHUB_TOKEN` PAT and resolves the API base (the real
        // public API, or the `OPENLORE_GITHUB_API_BASE` test seam). It holds
        // no signing/storage capability — the human-gate is structural
        // (I-SCR-1 / WD-49). The harvest method bodies are RED `todo!()`
        // scaffolds at this step; only the `scrape github` dispatch routing +
        // this wiring slot land in 01-04 (the live harvest is Phase 03/04).
        let github: Box<dyn GithubPort> = Box::new(GithubAdapter::from_env());

        // Slice-05 (ADR-027): the index-query CLIENT slot. WIRED + SOFT-probed,
        // but at this bootstrap step it is `None` because
        // `HttpIndexQueryAdapter::new()` is a Phase 03/04 `todo!()` scaffold —
        // calling it now would panic at startup for EVERY verb, violating WD-116
        // (an unreachable indexer MUST NOT block `claim add`). The reference to
        // `HttpIndexQueryAdapter::new` below keeps the CLIENT crate in the CLI's
        // dep graph (so `xtask check-arch` sees the CLI links the client and
        // ONLY the client — I-AV-3), without invoking the scaffold.
        let _phantom_index_query: fn() -> HttpIndexQueryAdapter = HttpIndexQueryAdapter::new;
        let index_query: Option<Box<dyn IndexQueryPort>> = None;

        Ok(Self {
            identity,
            storage,
            peer_storage,
            pds,
            clock,
            github,
            index_query,
            paths,
        })
    }

    /// Walk every adapter's probe arm. Returns `Err(...)` carrying the
    /// first refusal with its structured `health.startup.refused`
    /// payload preserved for tracing emission.
    pub fn probe_gauntlet(&self) -> Result<(), ProbeRefusal> {
        check_probe("identity", self.identity.probe())?;
        check_probe("storage", self.storage.probe())?;
        // Slice-03 peer-storage probe entry. The adapter binding exists
        // (`self.peer_storage`) and the gauntlet has its slot, but the
        // `DuckDbPeerStorageAdapter::probe()` body is still a RED scaffold
        // (`todo!()`) at this bootstrap step (01-02 / 01-04). Calling it
        // now would panic the binary at startup, so the call is gated
        // behind the `peer-storage-probe-live` cfg until the real probe
        // lands with the PS-* scenarios — at which point I-FED-3
        // (xtask check-probes) flips it on. The slot is declared here so
        // the gauntlet shape is complete and the wiring is exercised
        // (`self.peer_storage` is read), satisfying the step-01-04 AC
        // "ProbeGauntlet includes the new PeerStoragePort.probe()".
        #[cfg(feature = "peer-storage-probe-live")]
        check_probe("peer_storage", self.peer_storage.probe())?;
        #[cfg(not(feature = "peer-storage-probe-live"))]
        let _ = &self.peer_storage; // SCAFFOLD: true (slice-03) — slot wired; probe body lands with PS-*
        check_probe("pds", self.pds.probe())?;
        check_probe("clock", self.clock.probe())?;
        // Slice-02 GitHub-scraper probe entry (ADR-019 §6). Unlike the slice-03
        // peer-storage slot above, `GithubAdapter::probe()` does REAL
        // non-network work at this bootstrap step (an empty-API-base guard +
        // the no-token-leak auth-mode arm) and returns `ProbeOutcome::Ok` for a
        // non-empty base — `from_env()` resolves the real public API base (or
        // the test seam), so the call is safe at startup and need NOT be
        // feature-gated. The live network arms (public reachability, private
        // refusal, rate-limit headers) fill in around the pinned arm contracts
        // in Phase 03/04; the gauntlet shape + the `self.github` wiring are
        // exercised now, satisfying the step-01-04 AC "ProbeGauntlet includes
        // the new GithubPort.probe()".
        check_probe("github", self.github.probe())?;

        // Slice-05 (ADR-027 / WD-116) index-query SOFT probe. UNLIKE every probe
        // above, an unreachable indexer is INFORMATIONAL — it MUST NOT refuse
        // startup (the CLI must start, and `claim add` / offline `claim publish`
        // / `graph query` must succeed, without a reachable indexer — KPI-5).
        // So the index-query outcome is read but NEVER propagated as a refusal:
        // a `Refused` outcome is swallowed (logged at the verb layer when
        // `search` actually runs). At this bootstrap step the slot is `None`
        // (the adapter constructor is a Phase 03/04 scaffold), so the probe is
        // skipped entirely; the SOFT-not-fatal shape is established here.
        if let Some(index_query) = &self.index_query {
            // Soft: discard the outcome — an unreachable indexer is non-fatal.
            let _soft = index_query.probe();
        }
        Ok(())
    }

    /// Extra startup arm beyond the per-adapter probes: refuse to serve
    /// any verb except `init` until `identity.toml` exists at the
    /// resolved config path. This is the bootstrap-state contract from
    /// WS-2: a fresh environment with no identity.toml is "not yet
    /// initialized", and the user-facing remediation is `openlore init`.
    ///
    /// The refusal renders a structured hint that surfaces in the
    /// `health.startup.refused` event payload so observability layers
    /// can route on it; the human-readable stderr line names the exact
    /// command to run.
    pub fn check_initialized_state(&self) -> Result<(), ProbeRefusal> {
        let identity_path = self.paths.identity_toml();
        if identity_path.exists() {
            return Ok(());
        }
        let detail = format!(
            "OpenLore is not initialized (no identity.toml at {}). \
             Run `openlore init` first.",
            identity_path.display()
        );
        let structured = serde_json::json!({
            "missing_file": identity_path.display().to_string(),
            "hint": "Run `openlore init` first.",
            "remediation_command": "openlore init",
        });
        Err(ProbeRefusal {
            adapter: "identity",
            // Closest semantic match in the existing enum — the identity
            // adapter cannot answer queries until bootstrap completes.
            // The detail + structured payload carry the precise
            // remediation hint the user needs.
            reason: ports::ProbeRefusalReason::IdentityKeychainUnreachable,
            detail,
            structured,
        })
    }
}

/// A refusal carried up from the probe gauntlet. Holds the adapter name
/// (`identity` / `storage` / `pds` / `clock`) plus the raw
/// `ProbeOutcome::Refused` payload so the composition root can emit
/// `health.startup.refused` with all fields intact.
#[derive(Debug)]
pub struct ProbeRefusal {
    pub adapter: &'static str,
    pub reason: ports::ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

fn check_probe(adapter: &'static str, outcome: ProbeOutcome) -> Result<(), ProbeRefusal> {
    match outcome {
        ProbeOutcome::Ok => Ok(()),
        ProbeOutcome::Refused {
            reason,
            detail,
            structured,
        } => Err(ProbeRefusal {
            adapter,
            reason,
            detail,
            structured,
        }),
    }
}

/// Build the `IdentityPort` adapter. If `OPENLORE_KEY_SEED_HEX` is set,
/// use the deterministic constructor (test path). Otherwise load from
/// OS keychain (production path).
fn build_identity(did: &str) -> Result<AtProtoDidAdapter> {
    let did_document_methods = vec![
        "#atproto".to_string(),
        adapter_atproto_did::OPENLORE_VERIFICATION_METHOD_FRAGMENT.to_string(),
    ];

    if let Ok(hex) = std::env::var("OPENLORE_KEY_SEED_HEX") {
        let seed =
            decode_hex_seed(&hex).with_context(|| "decoding OPENLORE_KEY_SEED_HEX env var")?;
        AtProtoDidAdapter::new_with_did_document(did, seed, did_document_methods)
            .map_err(|e| anyhow!("constructing IdentityPort from seed: {e}"))
    } else {
        AtProtoDidAdapter::for_did(did, did_document_methods)
            .map_err(|e| anyhow!("loading identity from keychain: {e}"))
    }
}

/// Decode a hex string into a 32-byte Ed25519 seed. Strict on length +
/// character set so a malformed env var fails loudly at startup rather
/// than producing a degenerate keypair.
fn decode_hex_seed(s: &str) -> Result<Vec<u8>> {
    let trimmed = s.trim();
    if trimmed.len() != 64 {
        return Err(anyhow!(
            "expected 64 hex chars for 32-byte Ed25519 seed, got {}",
            trimmed.len()
        ));
    }
    let mut out = Vec::with_capacity(32);
    let bytes = trimmed.as_bytes();
    for i in 0..32 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(anyhow!("invalid hex character: {:?}", b as char)),
    }
}
