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
//! ## Transport delta: a peer's own openlore instance (US-SF-006)
//!
//! The flow above is unchanged whatever hosts the peer's claims; only the
//! TRANSPORT differs. One `GET /manifest` probe of the resolved
//! serviceEndpoint selects it (`publish_domain::select_peer_transport`,
//! pure): a marker-bearing openlore instance is read from that SAME manifest
//! read plus `GET /records/:cid` through the READ-ONLY `InstanceReadPort` (the bytes
//! decoded by the hoisted lexicon decode with the CID recomputed in Rust);
//! anything else answering is the ATProto PDS path, unchanged. Either way
//! every record then goes through the SAME `evaluate_record` verify and the
//! SAME `write_peer_claim` store — there is no second verify path, and this
//! verb holds no write capability toward any instance (ADR-062 §4/§6,
//! enforced by `xtask check-arch`).
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
use claim_domain::{
    provenance_verdict, Cid, ClaimRecord, Did, ProvenanceRejection, RecordOrigin, VerifyingKey,
};
use ports::{
    InstanceManifest, InstanceReadPort, PdsError, PeerInfo, PeerSubscription, SignedRecord,
};
use publish_domain::PeerTransport;

use crate::orientation::{self, OrientationMilestone};
use crate::verbs::claim_publish::build_tokio_runtime;
use crate::wiring::{self, Wiring};

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
    /// Freshly stored records admitted as self-attested (ADR-071): attested
    /// by the author's own repo, carrying no app signature — so they are
    /// reported on their own line, never counted as "signatures valid".
    self_attested: usize,
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
            self_attested: 0,
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

    /// The records presented for APP-SIGNATURE verification: the verifiable
    /// ones minus those admitted as self-attested (which carry no signature).
    fn signature_checked(&self) -> usize {
        self.verifiable().saturating_sub(self.self_attested)
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

    // The peer's app verifying key (from the resolved DID-doc verification
    // methods), if any. ADR-071: a Bluesky user has none — their
    // self-attested records need none; an app-signed record without a key
    // is rejected per record by the verdict.
    let verifying_key = peer_verifying_key(&peer_info);

    // Fetch every record over the transport the peer's endpoint selects. A
    // peer-level failure (unreachable, unreadable listing) skips this peer.
    let (origin, fetched) = match fetch_peer_records(wiring, runtime, &peer_did, &peer_info) {
        Ok(fetched) => fetched,
        Err(skip_reason) => {
            block.peer_skip_reason = Some(skip_reason);
            return block;
        }
    };

    block.fetched = fetched.len();

    for fetched_record in &fetched {
        let record = match fetched_record {
            Ok(record) => record,
            // Unreadable before any trust decision ⇒ reject this record only.
            Err(reason) => {
                block.reject(reason.clone());
                continue;
            }
        };
        match evaluate_record(record, &peer_did, origin, verifying_key.as_ref(), local_did) {
            RecordVerdict::Verified => {
                match store_admitted(wiring, &peer_did, &record.record, &peer_info) {
                    Ok(outcome) if outcome.written => {
                        block.stored += 1;
                        if matches!(record.record, ClaimRecord::SelfAttested(_)) {
                            block.self_attested += 1;
                        }
                    }
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

/// File an admitted record in its ADR-071 mode (app-signed rows exactly as
/// before; self-attested rows marked `self-attested`).
fn store_admitted(
    wiring: &Wiring,
    peer_did: &Did,
    record: &ClaimRecord,
    peer_info: &PeerInfo,
) -> Result<ports::WritePeerClaimOutcome, ports::PeerStorageError> {
    let fetched_at = wiring.clock.now_utc();
    match record {
        ClaimRecord::AppSigned(signed) => wiring.peer_storage.write_peer_claim(
            peer_did,
            signed,
            &peer_info.pds_endpoint,
            fetched_at,
        ),
        ClaimRecord::SelfAttested(claim) => wiring.peer_storage.write_self_attested_peer_claim(
            peer_did,
            claim,
            &peer_info.pds_endpoint,
            fetched_at,
        ),
    }
}

/// One record as fetched, BEFORE any trust decision: parsed into the domain
/// `SignedRecord` (keyed by the CID the peer lists it under), or why it could
/// not even be read — a per-record rejection reason.
type FetchedRecord = Result<SignedRecord, String>;

/// Fetch all of one peer's records over the transport its resolved endpoint
/// selects (US-SF-006): the opaque-instance read, or the shipped PDS XRPC
/// listing, with the computed origin of what was read (ADR-071: only the
/// PDS endpoint just resolved from the peer's DID document is the author's
/// own PDS). `Err` is the reason the whole peer is skipped (PP-7).
fn fetch_peer_records(
    wiring: &Wiring,
    runtime: &tokio::runtime::Runtime,
    peer_did: &Did,
    peer_info: &PeerInfo,
) -> Result<(RecordOrigin, Vec<FetchedRecord>), String> {
    let endpoint = peer_info.pds_endpoint.as_str();
    let observation = wiring::observe_peer_endpoint(endpoint);
    match publish_domain::select_peer_transport(&observation) {
        PeerTransport::OpaqueInstance => {
            // The selecting probe already read the manifest — use that ONE read.
            let manifest = publish_domain::classify_manifest_observation(&observation)
                .map_err(|err| format!("instance read failed ({err})"))?;
            Ok((
                RecordOrigin::Relay,
                read_peer_instance(wiring::instance_reader_for(endpoint).as_ref(), &manifest),
            ))
        }
        PeerTransport::AtprotoPds => {
            let fetched_from = peer_info.pds_endpoint.as_str();
            let origin = RecordOrigin::of(fetched_from, peer_info.pds_endpoint.as_str());
            list_pds_records(wiring, runtime, peer_did, peer_info).map(|fetched| (origin, fetched))
        }
        PeerTransport::Unreachable { detail } => {
            Err(format!("peer endpoint unreachable ({detail})"))
        }
    }
}

/// The shipped J-003 transport: list ALL the peer's records from its PDS,
/// walking every cursor (Q-DELIVER-5). Network failure ⇒ skip (PP-7).
fn list_pds_records(
    wiring: &Wiring,
    runtime: &tokio::runtime::Runtime,
    peer_did: &Did,
    peer_info: &PeerInfo,
) -> Result<Vec<FetchedRecord>, String> {
    match runtime.block_on(
        wiring
            .pds
            .list_peer_records(peer_did, &peer_info.pds_endpoint, None),
    ) {
        Ok(page) => Ok(page.records.into_iter().map(Ok).collect()),
        Err(PdsError::Unreachable { message }) => Err(format!("PDS unreachable ({message})")),
        Err(err) => Err(format!("PDS read failed ({err})")),
    }
}

/// The opaque-instance transport (ADR-062 §4): the verbatim bytes of every
/// record the (already-read) manifest lists, via `GET /records/:cid` through
/// the READ-ONLY port. The manifest's display fields are never trusted — only
/// its CID keys, which each record's recomputed CID must byte-match.
fn read_peer_instance(
    instance: &dyn InstanceReadPort,
    manifest: &InstanceManifest,
) -> Vec<FetchedRecord> {
    publish_domain::manifest_cids(manifest)
        .iter()
        .map(|cid| read_instance_record(instance, cid))
        .collect()
}

/// Read one record's bytes under its manifest key and decode them (the
/// hoisted lexicon decode; the claim carries the CID RECOMPUTED in Rust from
/// these bytes, never one from the wire).
fn read_instance_record(instance: &dyn InstanceReadPort, cid: &Cid) -> FetchedRecord {
    let bytes = instance
        .get_record(cid)
        .map_err(|err| format!("record read failed ({err})"))?;
    let signed_claim = publish_domain::decode_pulled(&bytes)
        .map_err(|unreadable| format!("unreadable record ({})", unreadable.detail))?;
    Ok(SignedRecord {
        rkey: cid.0.clone(),
        record: ClaimRecord::AppSigned(signed_claim),
    })
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

/// PURE per-record decision. A record authored by the LOCAL user is
/// rejected first (SelfAttribution / WD-40). Then the ADR-071 provenance
/// verdict: an app-signed record must match its rkey (WD-24) and verify
/// against the peer's key through the unchanged `verify`; a self-attested
/// record must be unsigned, authored by the bare repo DID, fetched from the
/// author's own PDS, and match its rkey. No I/O.
fn evaluate_record(
    record: &SignedRecord,
    repo_did: &Did,
    origin: RecordOrigin,
    verifying_key: Option<&VerifyingKey>,
    local_did: &Did,
) -> RecordVerdict {
    if crate::verbs::bare_did(&record.record.unsigned().author_did.0) == local_did.0 {
        return RecordVerdict::rejected("self attribution");
    }
    match provenance_verdict(
        &record.record,
        &record.rkey,
        repo_did,
        origin,
        verifying_key,
    ) {
        Ok(_) => RecordVerdict::Verified,
        Err(rejection) => RecordVerdict::rejected(&rejection_reason(rejection, &record.record)),
    }
}

/// The user-facing reason for a refused provenance (ADR-071 closed ADT). The
/// app-signed wording is unchanged ("CID mismatch (possible adversarial
/// input)", "signature invalid", "canonicalization failed"); an unsigned
/// record whose content does not hash to its record key names the failed
/// integrity check.
fn rejection_reason(rejection: ProvenanceRejection, record: &ClaimRecord) -> String {
    match (rejection, record) {
        (ProvenanceRejection::NoAppKey, _) => {
            "signature invalid (no usable verification key)".into()
        }
        (ProvenanceRejection::IntegrityFailure, ClaimRecord::SelfAttested(_)) => {
            "integrity check failed (CID mismatch, possible adversarial input)".into()
        }
        (other, _) => other.to_string(),
    }
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
    // The openlore signing key is the `#org.openlore.application` method; a
    // real DID document also lists the account's `#atproto` (secp256k1) key,
    // which never signs claims. Prefer the openlore method, then any method
    // that decodes as Ed25519.
    let is_openlore =
        |id: &str| id.ends_with(adapter_atproto_did::OPENLORE_VERIFICATION_METHOD_FRAGMENT);
    let (openlore, others): (Vec<_>, Vec<_>) = peer_info
        .verification_methods
        .iter()
        .partition(|method| is_openlore(&method.id));
    openlore
        .into_iter()
        .chain(others)
        .find_map(|method| ed25519_key_from_multibase(&method.public_key_multibase))
}

/// An Ed25519 key from a `publicKeyMultibase`: the `z6Mk...` form a real DID
/// document publishes, or the `hex:` form the test doubles use.
fn ed25519_key_from_multibase(multibase: &str) -> Option<VerifyingKey> {
    if let Some(hex) = multibase.strip_prefix("hex:") {
        return decode_hex_32(hex).map(VerifyingKey);
    }
    claim_domain::decode_ed25519_multibase(multibase)
        .ok()
        .map(|key| VerifyingKey(key.0))
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
                // Self-attested records carry no signature: they are excluded
                // from the signature line (shown only when something was
                // signature-checked, or nothing self-attested — i.e. exactly
                // as before for app-signed peers) and reported on their own.
                let signature_checked = block.signature_checked();
                if signature_checked > 0 || block.self_attested == 0 {
                    let verified = signature_checked.saturating_sub(block.rejected);
                    out.push_str(&format!(
                        "    verified  : {}/{} signatures valid against {}'s DID document\n",
                        verified, signature_checked, block.peer_handle,
                    ));
                }
                if block.self_attested > 0 {
                    out.push_str(&format!(
                        "    self-attested: {} (no app signature; published by {} in their own repo)\n",
                        block.self_attested, block.peer_handle,
                    ));
                }
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
    #[test]
    fn peer_key_is_the_openlore_method_decoded_from_its_z6mk_multibase() {
        let key = claim_domain::VerificationKey((1u8..=32).collect());
        let method = |id: &str, multibase: String| ports::VerificationMethod {
            id: id.to_string(),
            type_: "Multikey".to_string(),
            controller: Did("did:plc:jeff".to_string()),
            public_key_multibase: multibase,
        };
        let peer = PeerInfo {
            did: Did("did:plc:jeff".to_string()),
            handle: "jeff.example".to_string(),
            pds_endpoint: "https://pds.example".parse().expect("url"),
            // A real PLC document lists the account's secp256k1 `#atproto` key
            // first; it must be skipped, not mis-decoded.
            verification_methods: vec![
                method(
                    "did:plc:jeff#atproto",
                    "zQ3shXjHeiBuRCKmM36cuYnm7YEMzhGnCmCyW92sRJ9pribSF".to_string(),
                ),
                method(
                    "did:plc:jeff#org.openlore.application",
                    claim_domain::encode_ed25519_multibase(&key),
                ),
            ],
        };
        let found = peer_verifying_key(&peer).expect("the openlore key is found");
        assert_eq!(found.0, key.0);
    }

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
            self_attested: 0,
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
            self_attested: 0,
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

    mod self_attested {
        use super::super::*;
        use claim_domain::{
            Confidence, SelfAttestedClaim, SignatureBlock, SignedClaim, UnsignedClaim,
        };
        use proptest::prelude::*;

        fn unsigned(author: &str) -> UnsignedClaim {
            UnsignedClaim {
                subject: "github:priyaraman/tidepool".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.memory-safety".into(),
                evidence: vec!["https://github.com/priyaraman/tidepool".into()],
                confidence: Confidence::from_basis_points(2500),
                author_did: Did(author.into()),
                composed_at: "2026-10-04T15:02:11Z".into(),
                references: Vec::new(),
                reason: None,
            }
        }

        /// The universe: one record in each ADR-071 arm.
        fn record(self_attested: bool) -> ClaimRecord {
            if self_attested {
                ClaimRecord::SelfAttested(
                    SelfAttestedClaim::new(unsigned("did:plc:priya")).expect("canonical"),
                )
            } else {
                ClaimRecord::AppSigned(SignedClaim {
                    unsigned: unsigned("did:plc:rachel#org.openlore.application"),
                    signature: SignatureBlock {
                        signed_cid: Cid("bafyrachel".into()),
                        signature_bytes: vec![0u8; 64],
                        verification_method: "did:plc:rachel#org.openlore.application".into(),
                    },
                })
            }
        }

        fn arb_rejection() -> impl Strategy<Value = ProvenanceRejection> {
            prop_oneof![
                Just(ProvenanceRejection::MalformedProvenance),
                Just(ProvenanceRejection::ForeignRepo),
                Just(ProvenanceRejection::UnverifiableProvenance),
                Just(ProvenanceRejection::Uncanonicalizable),
                Just(ProvenanceRejection::IntegrityFailure),
                Just(ProvenanceRejection::NoAppKey),
                Just(ProvenanceRejection::SignatureInvalid),
            ]
        }

        /// The named phrases each rejection must surface (RD-4). Every other
        /// phrase of [`ALL_PHRASES`] must be absent, so a swapped mapping is
        /// caught in either direction.
        fn named_phrases(rejection: ProvenanceRejection, self_attested: bool) -> Vec<&'static str> {
            match rejection {
                ProvenanceRejection::MalformedProvenance => vec!["malformed provenance"],
                ProvenanceRejection::ForeignRepo => vec!["foreign repo"],
                ProvenanceRejection::UnverifiableProvenance => vec!["unverifiable provenance"],
                ProvenanceRejection::Uncanonicalizable => vec!["canonicalization failed"],
                ProvenanceRejection::IntegrityFailure if self_attested => {
                    vec!["integrity check failed", "CID mismatch"]
                }
                ProvenanceRejection::IntegrityFailure => vec!["CID mismatch"],
                ProvenanceRejection::NoAppKey | ProvenanceRejection::SignatureInvalid => {
                    vec!["signature invalid"]
                }
            }
        }

        const ALL_PHRASES: [&str; 7] = [
            "malformed provenance",
            "foreign repo",
            "unverifiable provenance",
            "canonicalization failed",
            "integrity check failed",
            "CID mismatch",
            "signature invalid",
        ];

        proptest! {
            /// RD-4 / AC-009.4: every rejection names exactly its own reason
            /// phrases (and no other's), and the app-signed integrity
            /// wording is unchanged.
            #[test]
            fn each_rejection_names_its_own_reason(
                rejection in arb_rejection(),
                self_attested in any::<bool>(),
            ) {
                let reason = rejection_reason(rejection, &record(self_attested));
                let expected = named_phrases(rejection, self_attested);
                for phrase in ALL_PHRASES {
                    prop_assert_eq!(
                        reason.contains(phrase),
                        expected.contains(&phrase),
                        "{:?} vs phrase {:?}", reason, phrase
                    );
                }
                if !self_attested && rejection == ProvenanceRejection::IntegrityFailure {
                    prop_assert_eq!(reason, "CID mismatch (possible adversarial input)");
                }
            }

            /// AC-009.3 / AC-009.4 state delta over the progress counts: a
            /// peer block reports its self-attested records on their own
            /// line (never "unverified", never as "signatures valid"), and a
            /// block with none renders exactly the app-signed lines.
            #[test]
            fn self_attested_records_are_reported_apart_from_signatures(
                signed in 0usize..5,
                self_attested in 0usize..5,
                rejected in 0usize..3,
                existing in 0usize..3,
            ) {
                let block = PeerProgress {
                    peer_did: "did:plc:priya".into(),
                    peer_handle: "priya.test".into(),
                    fetched: signed + self_attested + rejected + existing,
                    stored: signed + self_attested,
                    skipped_existing: existing,
                    rejected,
                    self_attested,
                    rejection_reasons: vec!["signature invalid".into(); rejected],
                    peer_skip_reason: None,
                };
                let rendered = render_report(&[block], signed + self_attested, "");
                prop_assert!(!rendered.contains("unverified"));
                prop_assert_eq!(rendered.contains("self-attested"), self_attested > 0);
                let checked = signed + rejected;
                let signature_line = format!("{signed}/{checked} signatures valid");
                prop_assert_eq!(
                    rendered.contains(&signature_line),
                    checked > 0 || self_attested == 0,
                    "{}", rendered
                );
                if self_attested > 0 {
                    let marker = format!("self-attested: {self_attested} ");
                    prop_assert!(rendered.contains(&marker), "{}", rendered);
                }
            }
        }
    }
}
