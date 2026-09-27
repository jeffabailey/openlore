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
//! - `push` — plan from ONE manifest read (Q-SF-D3: local CIDs minus the
//!   manifest's CIDs; already-present claims are skipped, never re-probed),
//!   then for each claim to push: stage the verbatim lexicon-JSON blob
//!   under its Rust-minted CID, read it back, recompute the CID in Rust, and
//!   ONLY on a match commit the manifest entry (verify before manifest
//!   append; manifest append = commit). The local store is never written.
//!   The write path is owner-authed (DV-4): the `PUT`s carry the owner token
//!   from `OPENLORE_PUBLISH_TOKEN`; without it (or with a wrong one) the
//!   instance refuses and the verb fails with `publish.unauthorized_write`.
//! - `pull` — read the manifest + every record back, re-parse, recompute each
//!   CID in Rust, byte-match it against the key, report `N/M CIDs verified`;
//!   then reconcile the verified records against the local store (Matched /
//!   New / Conflict / Rejected — never an overwrite, D-6) and report the
//!   tally. Verified New records the local identity authored are then
//!   inserted through the SAME `StoragePort` write path `claim add` uses
//!   (DuckDB row + `<cid>.json`), each only after its signature verifies
//!   against the local identity; foreign-author records are reported and
//!   never inserted (anti-merging). Matched / Conflict / Rejected records
//!   never touch the local store.
//! - `status` — READ-ONLY inspection: the registered target, its card URL,
//!   and its current reachability (the same adapter probe; no write path).

use anyhow::{anyhow, Context};
use std::collections::HashMap;

use claim_domain::{Cid, SignedClaim};
use ports::{ClaimRow, InstanceReadPort, PageRequest, PublishPort, RecordBytes, RoundTripVerdict};
use publish_domain::{ForeignRecord, LocalClaim, PulledRecord, Reconciled, RecordIdentity};
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

/// The result of `publish push`: one result per claim the plan sent, plus
/// how many local claims the instance's manifest already listed (skipped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReport {
    pub instance_url: String,
    pub results: Vec<PushResult>,
    pub skipped: usize,
}

impl PushReport {
    /// Claims stored, read back, CID-verified, and committed.
    pub fn committed_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r, PushResult::Committed { .. }))
            .count()
    }

    fn all_committed(&self) -> bool {
        self.committed_count() == self.results.len()
    }
}

/// The per-record result of inserting a selected pulled record locally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertResult {
    /// Signature verified against the local identity; row + artifact written.
    Inserted { cid: Cid },
    /// The signature does not verify against the local identity — NOT
    /// inserted (verify-before-trust: a matching CID alone is not enough).
    SignatureInvalid { cid: Cid },
}

/// The result of `publish pull`: one round-trip verdict and one reconcile
/// outcome per manifest entry, the insert result of every selected New
/// own-author record, and the New foreign-author records NOT inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullReport {
    pub instance_url: String,
    pub verdicts: Vec<RoundTripVerdict>,
    pub reconciled: Vec<Reconciled>,
    pub inserts: Vec<InsertResult>,
    pub foreign: Vec<ForeignRecord>,
}

impl PullReport {
    pub fn verified_count(&self) -> usize {
        self.verdicts
            .iter()
            .filter(|v| matches!(v, RoundTripVerdict::Verified { .. }))
            .count()
    }

    pub fn inserted_count(&self) -> usize {
        self.inserts
            .iter()
            .filter(|r| matches!(r, InsertResult::Inserted { .. }))
            .count()
    }

    fn clean(&self) -> bool {
        self.verified_count() == self.verdicts.len() && self.inserted_count() == self.inserts.len()
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
        stdout: render_publish_init(
            instance_url,
            &wiring.identity.author_did().0,
            &publish_domain::card_url(instance_url),
        ),
    })
}

fn push(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::publish_port_for(&instance_url)?;
    wiring::probe_instance(instance.as_ref()).map_err(PublishVerbError::Refused)?;
    let manifest = instance
        .fetch_manifest()
        .with_context(|| format!("reading the manifest of {instance_url}"))?;
    let plan = publish_domain::plan_push(
        &own_claim_cids(wiring)?,
        &publish_domain::manifest_cids(&manifest),
    );
    let results = plan
        .to_push
        .iter()
        .map(|cid| push_one(instance.as_ref(), &own_signed_claim(wiring, cid)?))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let report = PushReport {
        instance_url,
        results,
        skipped: plan.skipped.len(),
    };
    Ok(PublishOutcome {
        exit_code: if report.all_committed() { 0 } else { 1 },
        stdout: render_publish_push(&report),
    })
}

fn pull(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::instance_reader_for(&instance_url);
    wiring::probe_instance(instance.as_ref()).map_err(PublishVerbError::Refused)?;
    let manifest = instance
        .fetch_manifest()
        .with_context(|| format!("reading the manifest of {instance_url}"))?;
    let fetched = publish_domain::manifest_cids(&manifest)
        .iter()
        .map(|cid| fetch_record(instance.as_ref(), cid))
        .collect::<anyhow::Result<HashMap<_, _>>>()?;
    let pulled = publish_domain::manifest_cids(&manifest)
        .iter()
        .map(|cid| publish_domain::recompute_pulled(cid, &fetched[cid]))
        .collect::<Vec<_>>();
    let reconciled = publish_domain::reconcile(&own_local_claims(wiring)?, &pulled);
    let selection =
        publish_domain::select_inserts(&wiring.identity.author_did().0, &reconciled, &pulled);
    let inserts = selection
        .to_insert
        .iter()
        .map(|cid| insert_pulled(wiring, cid, &fetched[cid]))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let report = PullReport {
        instance_url,
        verdicts: pulled.iter().map(PulledRecord::verdict).collect(),
        reconciled,
        inserts,
        foreign: selection.foreign,
    };
    Ok(PublishOutcome {
        exit_code: if report.clean() { 0 } else { 1 },
        stdout: render_publish_pull(&report),
    })
}

fn status(wiring: &Wiring) -> Result<PublishOutcome, PublishVerbError> {
    let instance_url = resolve_target(wiring)?;
    let instance = wiring::instance_reader_for(&instance_url);
    let reachable = wiring::probe_instance(instance.as_ref()).is_ok();
    Ok(PublishOutcome {
        exit_code: 0,
        stdout: render_publish_status(
            &instance_url,
            &publish_domain::card_url(&instance_url),
            reachable,
        ),
    })
}

// -----------------------------------------------------------------------------
// Steps
// -----------------------------------------------------------------------------

/// Stage → read back → recompute → commit only on a verified, verbatim
/// read-back. A mismatch never reaches the manifest (the store has no
/// DELETE; manifest append = commit), so nothing is silently stored.
fn push_one(instance: &dyn PublishPort, signed: &SignedClaim) -> anyhow::Result<PushResult> {
    let cid = signed.signature.signed_cid.clone();
    let record = publish_domain::record_bytes_of(signed);
    instance
        .put_record(&cid, &record)
        .with_context(|| format!("storing {} on the instance", cid.0))?;
    let returned = instance
        .get_record(&cid)
        .with_context(|| format!("reading {} back from the instance", cid.0))?;
    match publish_domain::judge_readback(&cid, &record, &returned) {
        Ok(_) => {
            instance
                .commit_manifest_entry(&cid, &record, &publish_domain::display_projection(signed))
                .with_context(|| format!("committing {} to the manifest", cid.0))?;
            Ok(PushResult::Committed { cid })
        }
        Err(mismatch) => Ok(PushResult::Rejected {
            detail: mismatch.describe(),
            cid,
        }),
    }
}

/// Read one record's bytes back from the instance, keyed by the CID it is
/// listed under. Nothing is trusted yet: every record is recomputed in Rust
/// (verify-before-trust, J-003) before it is classified or inserted.
fn fetch_record(instance: &dyn InstanceReadPort, cid: &Cid) -> anyhow::Result<(Cid, RecordBytes)> {
    let returned = instance
        .get_record(cid)
        .with_context(|| format!("reading {} back from the instance", cid.0))?;
    Ok((cid.clone(), returned))
}

/// Insert one selected (verified New, own-author) record into the local
/// store through the `claim add` write path — only once its signature
/// verifies against the local identity. The decoded claim carries the
/// Rust-recomputed CID, which `select_inserts` already matched to `cid`.
fn insert_pulled(wiring: &Wiring, cid: &Cid, bytes: &RecordBytes) -> anyhow::Result<InsertResult> {
    let signed = publish_domain::decode_pulled(bytes)
        .map_err(|unreadable| anyhow!("re-parsing {}: {}", cid.0, unreadable.detail))?;
    if signed.signature.signed_cid != *cid || wiring.identity.verify(&signed).is_err() {
        return Ok(InsertResult::SignatureInvalid { cid: cid.clone() });
    }
    wiring
        .storage
        .write_signed_claim(&signed)
        .with_context(|| format!("storing pulled claim {} locally", cid.0))?;
    Ok(InsertResult::Inserted { cid: cid.clone() })
}

/// Every claim row in the user's OWN local store, in the store's stable
/// listing order (read-only port).
fn own_claim_rows(wiring: &Wiring) -> anyhow::Result<Vec<ClaimRow>> {
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
    Ok(page.rows)
}

/// The CIDs of every claim in the user's OWN local store.
fn own_claim_cids(wiring: &Wiring) -> anyhow::Result<Vec<Cid>> {
    Ok(own_claim_rows(wiring)?
        .into_iter()
        .map(|row| Cid(row.cid))
        .collect())
}

/// Every local claim with its logical identity — what a pull reconciles
/// against.
fn own_local_claims(wiring: &Wiring) -> anyhow::Result<Vec<LocalClaim>> {
    Ok(own_claim_rows(wiring)?
        .into_iter()
        .map(local_claim_of)
        .collect())
}

fn local_claim_of(row: ClaimRow) -> LocalClaim {
    LocalClaim {
        cid: Cid(row.cid),
        identity: RecordIdentity {
            author_did: row.author_did,
            subject: row.subject,
            predicate: row.predicate,
            object: row.object,
        },
    }
}

/// One of the user's own signed claims, read (never written) from its local
/// artifact.
fn own_signed_claim(wiring: &Wiring, cid: &Cid) -> anyhow::Result<SignedClaim> {
    wiring
        .storage
        .read_signed_claim(cid)
        .with_context(|| format!("reading signed claim {}", cid.0))?
        .ok_or_else(|| anyhow!("signed claim {} is listed but missing", cid.0))
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
