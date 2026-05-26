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
use adapter_system_clock::SystemClockAdapter;
use anyhow::{anyhow, Context, Result};
use ports::{ClockPort, IdentityPort, PdsPort, ProbeOutcome, StoragePort};

use crate::paths::OpenLorePaths;

/// One adapter wired behind each port trait. Owned by the composition
/// root for the duration of the program; dropped on shutdown.
pub struct Wiring {
    pub identity: Box<dyn IdentityPort>,
    pub storage: Box<dyn StoragePort>,
    pub pds: Box<dyn PdsPort>,
    pub clock: Box<dyn ClockPort>,
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
        let storage: Box<dyn StoragePort> = Box::new(storage);

        let pds_endpoint =
            std::env::var("OPENLORE_PDS_ENDPOINT").unwrap_or_default();
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
            Box::new(AtProtoPdsAdapter::for_endpoint("https://placeholder.invalid"))
        } else {
            Box::new(AtProtoPdsAdapter::with_did(
                pds_endpoint,
                "did:plc:placeholder-host",
                &did,
            ))
        };

        let clock: Box<dyn ClockPort> = Box::new(SystemClockAdapter::new());

        Ok(Self {
            identity,
            storage,
            pds,
            clock,
            paths,
        })
    }

    /// Walk every adapter's probe arm. Returns `Err(...)` carrying the
    /// first refusal with its structured `health.startup.refused`
    /// payload preserved for tracing emission.
    pub fn probe_gauntlet(&self) -> Result<(), ProbeRefusal> {
        check_probe("identity", self.identity.probe())?;
        check_probe("storage", self.storage.probe())?;
        check_probe("pds", self.pds.probe())?;
        check_probe("clock", self.clock.probe())?;
        Ok(())
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
        let seed = decode_hex_seed(&hex)
            .with_context(|| "decoding OPENLORE_KEY_SEED_HEX env var")?;
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

