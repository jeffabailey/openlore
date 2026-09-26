//! `openlore publish {init,push,pull,status}` — publish to the user's OWN
//! serverless instance over the opaque content-addressed transport (ADR-062;
//! serverless-philosophy-federation, US-SF-001/002).
//!
//! Shape of every verb: resolve the target → WIRE the instance port →
//! PROBE it (a refusal is surfaced as `health.startup.refused` with a
//! `publish.*` reason) → USE it. The pure decisions (record blob, display
//! projection, round-trip verdict, manifest parse) live in `publish-domain`;
//! this module only sequences effects.
//!
//! - `init <url>` — probe, then record `<url>` as the publish target.
//! - `push` — for each own signed claim: stage the verbatim lexicon-JSON blob
//!   under its Rust-minted CID, read it back, recompute the CID in Rust, and
//!   ONLY on a match commit the manifest entry (verify before manifest
//!   append; manifest append = commit). The local store is never written.
//! - `pull` — read the manifest + every record back, re-parse, recompute each
//!   CID in Rust, byte-match it against the key, report `N/M CIDs verified`.
//! - `status` — the registered target + its reachability (minimal).

use anyhow::{anyhow, Context};
use claim_domain::{Cid, SignedClaim};
use ports::{InstanceReadPort, PageRequest, PublishPort, RoundTripVerdict};
use serde::{Deserialize, Serialize};

use crate::render::{
    render_publish_init, render_publish_pull, render_publish_push, render_publish_status,
};
use crate::wiring::{self, ProbeRefusal, Wiring};

/// Env seam: the publish target used when none has been registered via
/// `publish init` (acceptance tests aim sad-path postures through it).
pub const PUBLISH_ENDPOINT_ENV: &str = "OPENLORE_PUBLISH_ENDPOINT";

/// Which publish verb to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishVerb {
    Init { instance_url: String },
    Push,
    Pull,
    Status,
}

/// A verb's captured result: exit code + the stdout block to print.
#[derive(Debug)]
pub struct PublishOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Why a publish verb stopped early.
#[derive(Debug)]
pub enum PublishVerbError {
    /// WIRE-then-PROBE refusal — emitted as `health.startup.refused`.
    Refused(ProbeRefusal),
    /// Any other failure (no target, store read, transport error mid-verb).
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for PublishVerbError {
    fn from(err: anyhow::Error) -> Self {
        Self::Failed(err)
    }
}

/// The per-claim result of `publish push`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    /// Stored, read back, CID-verified, and committed to the manifest.
    Committed { cid: Cid },
    /// The read-back recomputed a different CID (or was unreadable) — NOT
    /// committed (`publish.cid_roundtrip_failed`).
    Rejected { cid: Cid, detail: String },
}

/// The result of `publish pull`: one verdict per manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullReport {
    pub instance_url: String,
    pub verdicts: Vec<RoundTripVerdict>,
}

impl PullReport {
    pub fn verified_count(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| matches!(v, RoundTripVerdict::Verified { .. }))
            .count()
    }
}

/// `<config>/publish.toml` — the registered publish target.
#[derive(Debug, Serialize, Deserialize)]
struct PublishConfig {
    instance_url: String,
}

/// Run one publish verb.
pub fn run(wiring: &Wiring, verb: &PublishVerb) -> Result<PublishOutcome, PublishVerbError> {
    match verb {
        PublishVerb::Init { instance_url } => init(wiring, instance_url),
        PublishVerb::Push => push(wiring),
        PublishVerb::Pull => pull(wiring),
        PublishVerb::Status => status(wiring),
    }
}

// -----------------------------------------------------------------------------
// Verbs
// -----------------------------------------------------------------------------

fn init(wiring: &Wiring, instance_url: &str) -> Result<PublishOutcome, PublishVerbError> {
    let instance = wiring::instance_reader_for(instance_url);
    wiring::probe_instance(instance.as_ref()).map_err(PublishVerbError::Refused)?;
    register_target(wiring, instance_url)?;
    Ok(PublishOutcome {
        exit_code: 0,
        stdout: render_publish_init(instance_url, &wiring.identity.author_did().0),
    })
}

fn push(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::publish_port_for(&instance_url);
    wiring::probe_instance(instance.as_ref()).map_err(PublishVerbError::Refused)?;
    let results = own_signed_claims(wiring)?
        .iter()
        .map(|signed| push_one(instance.as_ref(), signed))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let all_committed = results
        .iter()
        .all(|r| matches!(r, PushResult::Committed { .. }));
    Ok(PublishOutcome {
        exit_code: if all_committed { 0 } else { 1 },
        stdout: render_publish_push(&instance_url, &results),
    })
}

fn pull(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::instance_reader_for(&instance_url);
    wiring::probe_instance(instance.as_ref()).map_err(PublishVerbError::Refused)?;
    let manifest = instance
        .fetch_manifest()
        .with_context(|| format!("reading the manifest of {instance_url}"))?;
    let verdicts = manifest
        .entries
        .iter()
        .map(|entry| verify_record(instance.as_ref(), &Cid(entry.cid.clone())))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let report = PullReport {
        instance_url,
        verdicts,
    };
    let all_verified = report.verified_count() == report.verdicts.len();
    Ok(PublishOutcome {
        exit_code: if all_verified { 0 } else { 1 },
        stdout: render_publish_pull(&report),
    })
}

fn status(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::instance_reader_for(&instance_url);
    let reachable = wiring::probe_instance(instance.as_ref()).is_ok();
    Ok(PublishOutcome {
        exit_code: 0,
        stdout: render_publish_status(&instance_url, reachable),
    })
}

// -----------------------------------------------------------------------------
// Steps
// -----------------------------------------------------------------------------

/// Stage → read back → recompute → commit only on a verified CID.
fn push_one(instance: &dyn PublishPort, signed: &SignedClaim) -> anyhow::Result<PushResult> {
    let cid = signed.signature.signed_cid.clone();
    let record = publish_domain::record_bytes_of(signed);
    instance
        .put_record(&cid, &record)
        .with_context(|| format!("storing {} on the instance", cid.0))?;
    match verify_record(instance, &cid)? {
        RoundTripVerdict::Verified { .. } => {
            instance
                .commit_manifest_entry(&cid, &record, &publish_domain::display_projection(signed))
                .with_context(|| format!("committing {} to the manifest", cid.0))?;
            Ok(PushResult::Committed { cid })
        }
        RoundTripVerdict::CidMismatch { recomputed, .. } => Ok(PushResult::Rejected {
            detail: format!("read back as {}", recomputed.0),
            cid,
        }),
    }
}

/// Read one record back and judge its round trip in Rust. Unreadable bytes
/// are a mismatch against an unrecoverable CID, never a pass.
fn verify_record(instance: &dyn InstanceReadPort, cid: &Cid) -> anyhow::Result<RoundTripVerdict> {
    let returned = instance
        .get_record(cid)
        .with_context(|| format!("reading {} back from the instance", cid.0))?;
    Ok(
        publish_domain::verify_round_trip(cid, &returned).unwrap_or_else(|unreadable| {
            RoundTripVerdict::CidMismatch {
                pushed: cid.clone(),
                recomputed: Cid(format!("<unreadable: {}>", unreadable.detail)),
            }
        }),
    )
}

/// Every signed claim in the user's OWN local store (read-only).
fn own_signed_claims(wiring: &Wiring) -> anyhow::Result<Vec<SignedClaim>> {
    let total = wiring
        .store_read
        .count_claims()
        .context("counting own claims")?;
    let page = wiring
        .store_read
        .list_claims(PageRequest {
            offset: 0,
            limit: total as u64,
        })
        .context("listing own claims")?;
    page.rows
        .iter()
        .map(|row| {
            let cid = Cid(row.cid.clone());
            wiring
                .storage
                .read_signed_claim(&cid)
                .with_context(|| format!("reading signed claim {}", row.cid))?
                .ok_or_else(|| anyhow!("signed claim {} is listed but missing", row.cid))
        })
        .collect()
}

/// The registered target, else the env-seam fallback, else a hint to init.
fn resolve_target(wiring: &Wiring) -> anyhow::Result<String> {
    let registered = read_registered_target(wiring)?;
    let from_env = std::env::var(PUBLISH_ENDPOINT_ENV)
        .ok()
        .filter(|url| !url.is_empty());
    registered.or(from_env).ok_or_else(|| {
        anyhow!("no publish target registered. Run `openlore publish init <instance-url>` first.")
    })
}

fn read_registered_target(wiring: &Wiring) -> anyhow::Result<Option<String>> {
    let path = wiring.paths.publish_toml();
    if !path.exists() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let config: PublishConfig =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(config.instance_url))
}

fn register_target(wiring: &Wiring, instance_url: &str) -> anyhow::Result<()> {
    let path = wiring.paths.publish_toml();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let config = PublishConfig {
        instance_url: instance_url.to_string(),
    };
    let text = toml::to_string(&config).context("serializing the publish target")?;
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
}
