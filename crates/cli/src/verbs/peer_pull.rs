//! `peer pull` — fetch + verify + cache claims from every subscribed
//! peer (slice-03; US-FED-002 / PP-*).
//!
//! For each active subscription: re-resolve the peer PDS endpoint (fresh
//! per ADR-016), list the peer's `org.openlore.claim` records (walking ALL
//! cursors, Q-DELIVER-5), recompute each record's CID locally and verify
//! its signature against the peer's DID-doc key, and cache the verified
//! records (via `PeerStoragePort::write_peer_claim` + the
//! `peer_claims/<did>/<cid>.json` artifact tree). Fault-isolated: a failed
//! peer or a rejected record never aborts the other pulls; the overall
//! exit code is non-zero if ANY peer was skipped or ANY record rejected.
//!
//! First-pull orientation (data-models.md §OrientationState): the FIRST
//! EVER successful `peer pull` emits the orientation message exactly once
//! (gated via `crate::orientation`), then records
//! `federation.first_pull_completed_at`.
//!
//! ## Pure-vs-effect split (ADR-009 / nw-fp-hexagonal-architecture)
//!
//! The per-record decision (verify → recompute CID → accept/reject) is
//! PURE — `evaluate_record` consumes the parsed `SignedRecord` + the
//! peer's verifying key + the local user's DID and returns a
//! `RecordVerdict` (Stored-eligible / Rejected) with no I/O. The effects —
//! resolve, list, write, artifact, clock, orientation — live in `run`.
//! Rendering is a pure function of the accumulated counts (`render_report`).

use anyhow::{anyhow, Result};
use claim_domain::{canonicalize, compute_cid, verify, Did, SignedClaim, VerifyingKey};
use ports::{PdsError, PeerInfo, PeerSubscription, SignedRecord};

use crate::orientation::{self, OrientationMilestone};
use crate::verbs::claim_publish::build_tokio_runtime;
use crate::wiring::Wiring;

/// Argument struct for the `peer pull` verb. It takes no arguments today
/// (it pulls ALL active subscriptions); the struct exists for uniformity
/// with the other verbs and as the seam for a future `--peer <did>`
/// targeted-pull flag.
#[derive(Debug, Clone, Default)]
pub struct PeerPullArgs {}

/// Outcome of one `peer pull` invocation — exit code + stdout chunk.
pub struct PeerPullOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Per-peer accumulated counts, rendered into the progress block.
struct PeerProgress {
    peer_did: String,
    peer_handle: String,
    fetched: usize,
    stored: usize,
    skipped_existing: usize,
    rejected: usize,
    /// One human-readable reason per rejected record (WD-37 per-record fault
    /// isolation): e.g. "signature invalid", "CID mismatch (possible
    /// adversarial input)", "self attribution". The render surfaces these so
    /// the user sees WHY a record was dropped, not just a count. Always the
    /// same length as `rejected`.
    rejection_reasons: Vec<String>,
    /// `None` ⇔ the peer's PDS was reachable; `Some(reason)` ⇔ the whole
    /// peer was skipped (PP-7 fault isolation).
    peer_skip_reason: Option<String>,
}

impl PeerProgress {
    fn new(peer_did: String, peer_handle: String) -> Self {
        Self {
            peer_did,
            peer_handle,
            fetched: 0,
            stored: 0,
            skipped_existing: 0,
            rejected: 0,
            rejection_reasons: Vec::new(),
            peer_skip_reason: None,
        }
    }

    /// Record one rejected record + the human-readable reason it was
    /// dropped. Keeps `rejected` and `rejection_reasons` in lock-step.
    fn reject(&mut self, reason: String) {
        self.rejected += 1;
        self.rejection_reasons.push(reason);
    }

    /// The number of records this peer presented for verification (fetched
    /// minus the ones already cached). Drives the `verified : N/N` line.
    fn verifiable(&self) -> usize {
        self.fetched.saturating_sub(self.skipped_existing)
    }
}

/// Run the `peer pull` verb.
pub fn run(wiring: &Wiring, _args: &PeerPullArgs) -> Result<PeerPullOutcome> {
    let subscriptions = wiring
        .peer_storage
        .list_active_subscriptions()
        .map_err(|err| anyhow!("could not list active subscriptions: {err}"))?;

    // PP-8: no subscriptions ⇒ a clean no-op (exit 0, nothing written).
    if subscriptions.is_empty() {
        return Ok(PeerPullOutcome {
            exit_code: 0,
            stdout: "No peers subscribed. Run `openlore peer add <did>` first.\n".to_string(),
        });
    }

    let runtime = build_tokio_runtime();
    let local_did = wiring.identity.author_did().clone();

    let mut progress: Vec<PeerProgress> = Vec::new();
    let mut any_failure = false;

    for subscription in &subscriptions {
        let block = pull_one_peer(wiring, &runtime, subscription, &local_did);
        if block.peer_skip_reason.is_some() || block.rejected > 0 {
            any_failure = true;
        }
        progress.push(block);
    }

    // First-pull orientation (WD-39): the FIRST EVER successful pull emits
    // the orientation marker exactly once. A failed orientation write is
    // logged, never fatal (data-models.md §OrientationState).
    let orientation_block = maybe_emit_first_pull_orientation(wiring);

    let total_stored: usize = progress.iter().map(|p| p.stored).sum();
    let stdout = render_report(&progress, total_stored, &orientation_block);

    // Exit non-zero on ANY peer skip or record rejection (WD-37 / ADR-013).
    let exit_code = if any_failure { 1 } else { 0 };
    Ok(PeerPullOutcome { exit_code, stdout })
}

/// Pull + verify + cache one peer's claims. Fault-isolated: an unreachable
/// peer is recorded as a skip (never an error that aborts the loop); a
/// rejected record is counted, the rest proceed.
fn pull_one_peer(
    wiring: &Wiring,
    runtime: &tokio::runtime::Runtime,
    subscription: &PeerSubscription,
    local_did: &Did,
) -> PeerProgress {
    let peer_did = subscription.peer_did.clone();
    let mut block = PeerProgress::new(peer_did.0.clone(), subscription.peer_handle.clone());

    // Re-resolve the peer DID FRESH per ADR-016 (do not trust the cached
    // endpoint as authoritative). A resolution failure skips this peer.
    let peer_info = match wiring.identity.resolve_peer(&peer_did) {
        Ok(info) => info,
        Err(err) => {
            block.peer_skip_reason = Some(format!("DID resolution failed: {err}"));
            return block;
        }
    };
    block.peer_handle = peer_info.handle.clone();

    // The peer's verifying key (from the resolved DID-doc verification
    // methods). Absent / undecodable key ⇒ skip the whole peer (we cannot
    // verify any of its records).
    let verifying_key = match peer_verifying_key(&peer_info) {
        Some(key) => key,
        None => {
            block.peer_skip_reason =
                Some("no usable verification key in the peer's DID document".to_string());
            return block;
        }
    };

    // List ALL records, walking every cursor (Q-DELIVER-5). Network failure
    // ⇒ skip this peer (PP-7).
    let page = match runtime.block_on(wiring.pds.list_peer_records(
        &peer_did,
        &peer_info.pds_endpoint,
        None,
    )) {
        Ok(page) => page,
        Err(PdsError::Unreachable { message }) => {
            block.peer_skip_reason = Some(format!("PDS unreachable ({message})"));
            return block;
        }
        Err(err) => {
            block.peer_skip_reason = Some(format!("PDS read failed ({err})"));
            return block;
        }
    };

    block.fetched = page.records.len();

    for record in &page.records {
        match evaluate_record(record, &verifying_key, local_did) {
            RecordVerdict::Verified => {
                match wiring.peer_storage.write_peer_claim(
                    &peer_did,
                    &record.signed_claim,
                    &peer_info.pds_endpoint,
                    wiring.clock.now_utc(),
                ) {
                    Ok(outcome) if outcome.written => block.stored += 1,
                    // Idempotent re-pull: the CID was already cached.
                    Ok(_) => block.skipped_existing += 1,
                    // Anti-merging rejection (Self/Cross) or storage error
                    // ⇒ reject this record only, continue with others.
                    Err(err) => block.reject(write_rejection_reason(&err)),
                }
            }
            // Per-record fault isolation (WD-37): a rejected record never
            // aborts the others — record the reason + continue.
            RecordVerdict::Rejected { reason } => block.reject(reason),
        }
    }

    block
}

/// Map a `PeerStorageError` from `write_peer_claim` into the user-facing
/// rejection reason surfaced in the per-peer progress block. Self/Cross
/// attribution (WD-40 / WD-41) carry their own message; any other storage
/// error is surfaced verbatim.
fn write_rejection_reason(err: &ports::PeerStorageError) -> String {
    match err {
        ports::PeerStorageError::SelfAttribution => "self attribution".to_string(),
        ports::PeerStorageError::CrossAttribution { .. } => "cross attribution".to_string(),
        other => format!("storage rejected ({other})"),
    }
}

/// PURE per-record decision: a record is `Verified` iff BOTH (a) its
/// signature verifies against the peer's key AND (b) its locally
/// recomputed CID byte-matches the peer-published rkey (WD-24). A record
/// authored by the LOCAL user is rejected here too (SelfAttribution /
/// WD-40 — even if its signature verified, which would indicate key
/// compromise). No I/O.
fn evaluate_record(
    record: &SignedRecord,
    verifying_key: &VerifyingKey,
    local_did: &Did,
) -> RecordVerdict {
    // SelfAttribution (WD-40): a peer record claiming the LOCAL user's DID
    // is rejected before any storage write.
    if bare_did(&record.signed_claim.unsigned.author_did.0) == local_did.0 {
        return RecordVerdict::rejected("self attribution");
    }

    // CID round-trip (WD-24): recompute locally; reject on mismatch with
    // the peer-published rkey (canonicalization disagreement → "possible
    // adversarial input").
    let Ok(canonical) = canonicalize(&record.signed_claim.unsigned) else {
        return RecordVerdict::rejected("canonicalization failed");
    };
    let recomputed = compute_cid(&canonical);
    if recomputed.0 != record.rkey {
        return RecordVerdict::rejected("CID mismatch (possible adversarial input)");
    }

    // Signature verify (WD-24) against the peer's DID-doc key. The parsed
    // SignedClaim already carries `signed_cid = recomputed`, so `verify`
    // checks the signature over the recomputed CID. A failure here is the
    // KPI-FED-6 path — a tampered or wrong-key signature is dropped with a
    // "signature invalid" reason, never stored.
    let to_verify = SignedClaim {
        unsigned: record.signed_claim.unsigned.clone(),
        signature: claim_domain::SignatureBlock {
            signed_cid: recomputed,
            ..record.signed_claim.signature.clone()
        },
    };
    if verify(&to_verify, verifying_key).is_err() {
        return RecordVerdict::rejected("signature invalid");
    }

    RecordVerdict::Verified
}

/// Outcome of the pure per-record evaluation. A `Rejected` verdict carries
/// the human-readable reason so the render surfaces WHY (WD-37 + ADR-013).
enum RecordVerdict {
    Verified,
    Rejected { reason: String },
}

impl RecordVerdict {
    /// Construct a `Rejected` verdict with a borrowed reason.
    fn rejected(reason: &str) -> Self {
        RecordVerdict::Rejected {
            reason: reason.to_string(),
        }
    }
}

/// Decode the peer's Ed25519 verifying key from the resolved DID-doc
/// verification methods. Supports the acceptance pubkey seam encoding
/// (`hex:<64-char-hex>`) the resolver injects; a production multibase key
/// (`z6Mk…`) decode lands when real PLC resolution ships. Returns `None`
/// if no method carries a decodable key.
fn peer_verifying_key(peer_info: &PeerInfo) -> Option<VerifyingKey> {
    for method in &peer_info.verification_methods {
        if let Some(hex) = method.public_key_multibase.strip_prefix("hex:") {
            if let Some(bytes) = decode_hex_32(hex) {
                return Some(VerifyingKey(bytes));
            }
        }
    }
    None
}

/// Decode a 64-char lowercase-hex string into 32 bytes; `None` on any
/// malformed input.
fn decode_hex_32(s: &str) -> Option<Vec<u8>> {
    let trimmed = s.trim();
    if trimmed.len() != 64 {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut out = Vec::with_capacity(32);
    for i in 0..32 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Strip a `#fragment` from a DID, returning the bare DID.
fn bare_did(did: &str) -> String {
    did.split('#').next().unwrap_or(did).to_string()
}

/// Emit the first-pull orientation block exactly once per install (WD-39).
/// Returns the rendered marker text to append to stdout, or empty string if
/// it has already fired. The orientation state lives in `identity.toml`;
/// a write failure is logged-and-ignored (never fatal).
fn maybe_emit_first_pull_orientation(wiring: &Wiring) -> String {
    let identity_path = wiring.paths.identity_toml();
    let state = orientation::load(&identity_path).unwrap_or_default();
    if !state.should_fire(OrientationMilestone::FirstPull) {
        return String::new();
    }

    let now = wiring.clock.now_utc().to_rfc3339();
    if let Err(err) =
        orientation::mark_completed(&identity_path, OrientationMilestone::FirstPull, now)
    {
        // Non-fatal: the orientation may re-fire on the next pull, but the
        // pull itself succeeded. Log to stderr, do not abort.
        eprintln!("openlore peer pull: could not record first-pull orientation: {err:#}");
    }

    let mut out = String::new();
    out.push('\n');
    out.push_str(
        "First federated pull complete. Peer claims live in a SEPARATE layer from your own.\n",
    );
    out.push_str(
        "  Query them with `openlore graph query --federated <subject>` to see them \
         attributed per author.\n",
    );
    out
}

/// PURE render of the full pull report (journey YAML tui_mockup /
/// Q-DELIVER-6 + ADR-013 output convention). The per-peer progress block,
/// the total summary, the content-frozen anti-merging line, and the
/// first-pull orientation (when present).
fn render_report(
    progress: &[PeerProgress],
    total_stored: usize,
    orientation_block: &str,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Pulling claims from {} subscribed peer{}...\n\n",
        progress.len(),
        if progress.len() == 1 { "" } else { "s" }
    ));

    for block in progress {
        out.push_str(&format!("  {} ({})\n", block.peer_did, block.peer_handle));
        match &block.peer_skip_reason {
            Some(reason) => {
                out.push_str(&format!("    skipped   : {reason}\n"));
            }
            None => {
                out.push_str(&format!("    fetched   : {} records\n", block.fetched));
                out.push_str(&format!(
                    "    new       : {} ({} already in peer_claims, skipped)\n",
                    block.stored, block.skipped_existing
                ));
                // Verified = records that passed verify (freshly stored OR
                // already cached) over the verifiable count (fetched minus
                // already-cached). Rejected records are the complement.
                let verified = block.verifiable().saturating_sub(block.rejected);
                out.push_str(&format!(
                    "    verified  : {}/{} signatures valid against {}'s DID document\n",
                    verified,
                    block.verifiable(),
                    block.peer_handle,
                ));
                if block.rejected > 0 {
                    out.push_str(&format!("    rejected  : {}\n", block.rejected));
                    // Surface the per-record reason (WD-37 + ADR-013): the
                    // user sees WHY each record was dropped, not just a count.
                    for reason in &block.rejection_reasons {
                        out.push_str(&format!("      - {reason}\n"));
                    }
                }
                out.push_str("    stored    : peer_claims (attribution preserved per record)\n");
            }
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "Pulled {total_stored} new peer claim{}.\n",
        if total_stored == 1 { "" } else { "s" }
    ));
    out.push_str("None merged with your own claims; query with --federated to see them.\n");
    out.push_str(orientation_block);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `verified : N/N` line in the progress block must report the
    /// verified count over the verifiable count, naming the peer's DID and
    /// the content-frozen anti-merging line. Pins the Q-DELIVER-6 / ADR-013
    /// render contract without spawning a runtime.
    #[test]
    fn render_report_emits_progress_block_and_anti_merging_line() {
        let block = PeerProgress {
            peer_did: "did:plc:rachel-test".to_string(),
            peer_handle: "rachel.test".to_string(),
            fetched: 3,
            stored: 3,
            skipped_existing: 0,
            rejected: 0,
            rejection_reasons: Vec::new(),
            peer_skip_reason: None,
        };
        let rendered = render_report(&[block], 3, "");
        assert!(
            rendered.contains("did:plc:rachel-test"),
            "names the peer DID"
        );
        assert!(rendered.contains("fetched   : 3 records"), "fetched line");
        assert!(rendered.contains("3/3"), "verified N/N line");
        assert!(rendered.contains("stored    : peer_claims"), "stored line");
        assert!(
            rendered.contains("None merged with your own claims"),
            "content-frozen anti-merging line (ADR-013)"
        );
    }

    /// PP-3: a peer with one rejected record renders a `rejected : 1` line
    /// plus the per-record reason verbatim, while still reporting the stored
    /// honest records. Pins the WD-37 + ADR-013 reject-reason render contract
    /// (the KPI-FED-6 "signature invalid" wording) without spawning a runtime.
    #[test]
    fn render_report_emits_rejected_count_and_reason_for_tampered_record() {
        let block = PeerProgress {
            peer_did: "did:plc:rachel-test".to_string(),
            peer_handle: "rachel.test".to_string(),
            fetched: 5,
            stored: 4,
            skipped_existing: 0,
            rejected: 1,
            rejection_reasons: vec!["signature invalid".to_string()],
            peer_skip_reason: None,
        };
        let rendered = render_report(&[block], 4, "");
        assert!(
            rendered.contains("rejected  : 1"),
            "must report the rejected count;\n{rendered}"
        );
        assert!(
            rendered.contains("signature invalid"),
            "must surface the per-record reject reason verbatim (KPI-FED-6);\n{rendered}"
        );
        assert!(
            rendered.contains("4/5"),
            "4 of 5 fetched records verify (1 rejected); verified/verifiable = 4/5;\n{rendered}"
        );
    }

    /// A skipped peer (PP-7 fault isolation) renders a `skipped` line, not
    /// a fetched/verified block.
    #[test]
    fn render_report_emits_skip_line_for_unreachable_peer() {
        let mut block = PeerProgress::new("did:plc:down-test".to_string(), "down".to_string());
        block.peer_skip_reason = Some("PDS unreachable (connection refused)".to_string());
        let rendered = render_report(&[block], 0, "");
        assert!(rendered.contains("did:plc:down-test"));
        assert!(rendered.contains("skipped"));
        assert!(rendered.contains("PDS unreachable"));
    }
}
