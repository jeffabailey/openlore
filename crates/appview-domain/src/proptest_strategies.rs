//! Proptest strategies for the verify-before-index gate (`ingest_decision`).
//!
//! Step 02-01 (AVC-1): bootstraps the cardinal `@property` scenario in slice-05
//! — the verify-before-index gate's iff. Per DD-AV-7 / the nw-distill polyglot
//! matrix, proptest is the canonical Rust PBT crate; the strategy below
//! generates `(RawRecord, VerificationKey)` pairs split ~50% valid / ~50%
//! adversarial across the three reject postures (unsigned / tampered-signature /
//! cid-mismatch), so the gate's `Index` AND `Reject` arms are both exercised on
//! every run.
//!
//! ## Generator placement (avoids a dep cycle — DESIGN_CONTEXT #3)
//!
//! `crates/test-support` depends on `appview-domain`, so `appview-domain` MUST
//! NOT depend on `test-support`. Therefore this generator is SELF-CONTAINED:
//! it builds `RawRecord`s directly from `claim_domain` primitives (a
//! deterministic seeded Ed25519 keypair → `canonicalize` → `compute_cid` →
//! `sign`), NOT via the `fixtures_ingest.rs` builders. The valid posture
//! produces a REAL signature over the canonical claim bytes and a
//! `published_cid` that recomputes byte-equal, so the gate runs the REAL pure
//! `claim_domain::verify` + `compute_cid` path on it (no second verification
//! path; WD-104). Each adversarial posture breaks exactly ONE precondition.
//!
//! ## Generator exposure (mirrors slice-01 claim-domain)
//!
//! `pub` so the layer-2 acceptance test (`tests/acceptance/appview_core.rs`,
//! compiled as a `[[test]]` in `cli`, which already depends on `appview-domain`)
//! reaches `arbitrary_raw_records()` via the pure-core import path
//! `appview_domain::proptest_strategies::arbitrary_raw_records`. `proptest` is a
//! regular dependency of this pure crate exactly as in `claim-domain` (it is a
//! pure-CPU crate, NOT on the `xtask check-arch` banned-I/O list).
//!
//! ## Functional discipline
//!
//! Pure. No I/O. No mutation. Each generator returns a fresh immutable value.
//! The strategies compose via `prop_map` / `prop_oneof` — small, named,
//! single-purpose builders, NEVER a 200-line nested tuple.

use chrono::{TimeZone, Utc};
use claim_domain::{
    canonicalize, compute_cid, sign, Cid, Confidence, Did, KeyId, SignatureBlock, SignedClaim,
    SigningKey, UnsignedClaim, VerificationKey,
};
use ed25519_dalek::SigningKey as DalekSigningKey;
use proptest::prelude::*;

use crate::{AuthorRelationship, IndexedClaim, RawRecord};

/// The OpenLore application verification-method fragment appended to every
/// author DID in the signed payload (mirrors the test-support fixtures).
const APP_FRAGMENT: &str = "#org.openlore.application";

/// The adversarial posture a generated record is materialized under. `Valid` is
/// the gold path (real signature, recomputing CID); each other variant breaks
/// exactly ONE verify-before-index precondition (WD-104). Self-contained mirror
/// of `test-support`'s `Posture` (we cannot import it — cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Posture {
    /// Real Ed25519 signature over the canonical bytes; `published_cid` recomputes.
    Valid,
    /// No usable signature block (empty signature bytes) → `RejectReason::Unsigned`.
    Unsigned,
    /// A real-shaped but tampered signature (last byte flipped) → `BadSignature`.
    TamperedSignature,
    /// A valid signature but a `published_cid` that does NOT recompute → `CidMismatch`.
    CidMismatch,
}

/// Derive a deterministic 32-byte Ed25519 seed for a fixture author. Distinct
/// seeds per author so their public keys differ across the generated universe.
fn seed_for(did: &str) -> [u8; 32] {
    let mut seed = [0u8; 32];
    let bytes = did.as_bytes();
    for (i, slot) in seed.iter_mut().enumerate() {
        *slot = bytes[i % bytes.len()].wrapping_add(i as u8);
    }
    seed
}

/// The deterministic `(SigningKey, VerificationKey)` keypair for an author DID.
/// `VerificationKey` is the ADR-026 decode output the gate consumes; it wraps the
/// same 32 raw public-key bytes as the lower-level `VerifyingKey`.
fn keypair_for(did: &str) -> (SigningKey, VerificationKey) {
    let dalek_sk = DalekSigningKey::from_bytes(&seed_for(did));
    let pubkey_bytes = dalek_sk.verifying_key().to_bytes().to_vec();
    (
        SigningKey(dalek_sk.to_bytes().to_vec()),
        VerificationKey(pubkey_bytes),
    )
}

/// Build an `UnsignedClaim` from generated components. `Confidence` is built via
/// serde (its smart constructor is still a RED scaffold — the same trick
/// `fixtures_ingest.rs` uses); the value is in `[0.0, 1.0]` by construction.
fn unsigned_claim(
    author_did: &str,
    subject: &str,
    object: &str,
    confidence: f64,
    composed_at: &str,
) -> UnsignedClaim {
    let confidence: Confidence = serde_json::from_value(serde_json::json!(confidence))
        .expect("generated confidence value is a well-formed JSON number in [0.0, 1.0]");
    UnsignedClaim {
        subject: subject.to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: object.to_string(),
        evidence: vec![format!("https://example.test/evidence/{subject}")],
        confidence,
        // Attribution is carried byte-equal into the IndexedClaim from HERE (the
        // signed payload), never from the unsigned provenance (source_pds).
        author_did: Did(format!("{author_did}{APP_FRAGMENT}")),
        composed_at: composed_at.to_string(),
        references: Vec::new(),
        reason: None,
    }
}

/// Produce the signature block under the chosen posture. `Valid` + `CidMismatch`
/// carry a REAL signature over the real body CID; `Unsigned` carries none;
/// `TamperedSignature` flips the last byte of a real signature.
fn signature_for(body_cid: &Cid, signing_key: &SigningKey, posture: Posture) -> SignatureBlock {
    match posture {
        Posture::Unsigned => SignatureBlock {
            signed_cid: body_cid.clone(),
            signature_bytes: Vec::new(),
            verification_method: String::new(),
        },
        Posture::TamperedSignature => {
            let mut block = sign(body_cid, signing_key).expect("generated signing succeeds");
            if let Some(last) = block.signature_bytes.last_mut() {
                *last ^= 0x01;
            } else {
                block.signature_bytes.push(0x01);
            }
            block
        }
        Posture::Valid | Posture::CidMismatch => {
            sign(body_cid, signing_key).expect("generated signing succeeds")
        }
    }
}

/// Materialize one `(RawRecord, VerificationKey)` pair under `posture`, running
/// the REAL crypto so the gate exercises the REAL pure path (no second
/// verification path; WD-104). The keypair is derived deterministically from the
/// author DID so the paired `VerificationKey` resolves the record's signature.
fn raw_record(
    author_did: &str,
    subject: &str,
    object: &str,
    confidence: f64,
    composed_at: &str,
    posture: Posture,
) -> (RawRecord, VerificationKey) {
    let (signing_key, verification_key) = keypair_for(author_did);
    let unsigned = unsigned_claim(author_did, subject, object, confidence, composed_at);
    let canonical = canonicalize(&unsigned).expect("generated claim canonicalizes");
    let body_cid = compute_cid(&canonical);

    let signature = signature_for(&body_cid, &signing_key, posture);
    let published_cid = match posture {
        // The cid-mismatch posture publishes a CID that does NOT recompute from
        // the canonical bytes — the recompute-vs-published gate fails.
        Posture::CidMismatch => Cid(format!("{}tampered", body_cid.0)),
        _ => body_cid,
    };

    let record = RawRecord {
        published_cid,
        raw_payload: SignedClaim {
            unsigned,
            signature,
        },
        source_pds: "https://pds.example.test".to_string(),
    };
    (record, verification_key)
}

/// The four postures, weighted ~50% valid / ~50% adversarial (the adversarial
/// half split evenly across the three reject postures). Forcing the valid arm to
/// ~half the universe keeps BOTH the `Index` and `Reject` gate arms exercised
/// every run, so a mutant that always-rejects (or always-indexes) fails loudly.
fn arb_posture() -> impl Strategy<Value = Posture> {
    prop_oneof![
        3 => Just(Posture::Valid),
        1 => Just(Posture::Unsigned),
        1 => Just(Posture::TamperedSignature),
        1 => Just(Posture::CidMismatch),
    ]
}

/// Generator for an arbitrary mix of valid + adversarial `RawRecord`s paired
/// with their resolved [`VerificationKey`], over a small bounded universe of
/// {author in 3, subject in 3, object in 2, confidence in `[0.0, 1.0]`}.
///
/// Distribution: ~50% valid signed records, ~50% adversarial split across
/// unsigned / tampered-signature / cid-mismatch (see [`arb_posture`]) — so the
/// gate's `Index` AND `Reject` arms are both exercised on every run.
///
/// Used by AVC-1 (gate iff) + AVC-3a (determinism) + AVC-4 (author derivation)
/// as those properties activate.
pub fn arbitrary_raw_records() -> impl Strategy<Value = (RawRecord, VerificationKey)> {
    let author = prop_oneof![
        Just("did:plc:priya-test"),
        Just("did:plc:sven-test"),
        Just("did:plc:rachel-test"),
    ];
    let subject = prop_oneof![
        Just("github:bazelbuild/bazel"),
        Just("github:denoland/deno"),
        Just("github:NixOS/nixpkgs"),
    ];
    let object = prop_oneof![
        Just("org.openlore.philosophy.reproducible-builds"),
        Just("org.openlore.philosophy.dependency-pinning"),
    ];
    // A pinned compose timestamp keeps the gate clock-free: the timestamp is
    // derived from the SIGNED payload, never read from a wall clock (AVC-3a).
    let composed_at = "2026-05-26T12:00:00Z";

    (author, subject, object, 0.0_f64..=1.0, arb_posture()).prop_map(
        move |(author, subject, object, confidence, posture)| {
            raw_record(author, subject, object, confidence, composed_at, posture)
        },
    )
}

// =============================================================================
// AVC-2 — anti-merging-at-network-scale generator (arbitrary IndexedClaim set)
// =============================================================================

/// The bounded `author_did` universe (3) for [`arbitrary_indexed_claims`]. Three
/// distinct authors so generated sets exercise single-author, multi-author, and
/// identical-content-distinct-author groupings.
const AUTHORS: [&str; 3] = [
    "did:plc:priya-test",
    "did:plc:sven-test",
    "did:plc:rachel-test",
];

/// The bounded `subject` universe (3) for [`arbitrary_indexed_claims`].
const SUBJECTS: [&str; 3] = [
    "github:bazelbuild/bazel",
    "github:denoland/deno",
    "github:NixOS/nixpkgs",
];

/// The bounded `object` universe (2) for [`arbitrary_indexed_claims`] — only TWO
/// objects so the {3 subject × 2 object} space (6) is smaller than the row count
/// upper bound, GUARANTEEING (subject, object) collisions across DISTINCT authors
/// in larger generated sets (the no-merge property's load-bearing case).
const OBJECTS: [&str; 2] = [
    "org.openlore.philosophy.reproducible-builds",
    "org.openlore.philosophy.dependency-pinning",
];

/// One generated cell of the bounded universe: indices into the AUTHOR / SUBJECT
/// / OBJECT tables plus a confidence in `[0.0, 1.0]`. The `cid` is NOT generated
/// here — it is derived from the row's ordinal in [`arbitrary_indexed_claims`] so
/// every generated row carries a DISTINCT CID (multiset uniqueness) even when two
/// rows share identical (author, subject, object) content.
#[derive(Debug, Clone)]
struct ClaimCell {
    author_idx: usize,
    subject_idx: usize,
    object_idx: usize,
    confidence: f64,
}

/// Strategy for a single [`ClaimCell`] over the bounded universe.
fn arb_claim_cell() -> impl Strategy<Value = ClaimCell> {
    (
        0..AUTHORS.len(),
        0..SUBJECTS.len(),
        0..OBJECTS.len(),
        0.0_f64..=1.0,
    )
        .prop_map(
            |(author_idx, subject_idx, object_idx, confidence)| ClaimCell {
                author_idx,
                subject_idx,
                object_idx,
                confidence,
            },
        )
}

/// Materialize one [`IndexedClaim`] from a [`ClaimCell`] and a row ordinal. The
/// `cid` encodes the ordinal so it is DISTINCT for every generated row (the
/// multiset-uniqueness precondition AVC-2 relies on); `author_did` is carried
/// byte-equal from the bounded author table (anti-merging attribution, WD-103);
/// `verified_against` is NON-empty by construction (verified-before-index, WD-104,
/// AVC-7). Pure: no I/O, no clock — `composed_at` is a pinned deterministic value.
fn indexed_claim_from(cell: &ClaimCell, ordinal: usize) -> IndexedClaim {
    let author = AUTHORS[cell.author_idx];
    let subject = SUBJECTS[cell.subject_idx];
    let object = OBJECTS[cell.object_idx];
    IndexedClaim {
        author_did: Did(format!("{author}{APP_FRAGMENT}")),
        // Ordinal-encoded so every row is a DISTINCT multiset member, even when two
        // rows share identical (author, subject, object). The author segment keeps
        // identical-content-distinct-author rows trivially distinguishable too.
        cid: Cid(format!("bafyclaim{ordinal:04}{author}")),
        subject: subject.to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: object.to_string(),
        confidence: cell.confidence,
        composed_at: Utc.with_ymd_and_hms(2026, 5, 26, 12, 0, 0).unwrap(),
        verified_against: KeyId(format!("{author}{APP_FRAGMENT}")),
        evidence: vec![format!("https://example.test/evidence/{subject}")],
        references: Vec::new(),
        relationship: AuthorRelationship::NetworkUnfollowed,
    }
}

/// Generator for an arbitrary NON-EMPTY `Vec<IndexedClaim>` over the bounded
/// universe (3 authors × 3 subjects × 2 objects, confidence in `[0.0, 1.0]`, every
/// `verified_against` non-empty). Drives AVC-2 (preserve every author) + AVC-3b
/// (compose determinism).
///
/// Because the (subject, object) space is small (3 × 2 = 6) relative to the row
/// count (1..=12), larger generated sets are GUARANTEED to contain
/// identical-(subject, object) rows under DISTINCT authors — the load-bearing
/// no-merge case (two authors asserting the same thing must stay in two groups,
/// never collapse to a faceless consensus row). Every generated row carries a
/// DISTINCT ordinal-encoded `cid`, so the flattened `by_author` rows form a clean
/// multiset against the input (every `(author_did, cid)` appears exactly once).
pub fn arbitrary_indexed_claims() -> impl Strategy<Value = Vec<IndexedClaim>> {
    prop::collection::vec(arb_claim_cell(), 1..=12).prop_map(|cells| {
        cells
            .iter()
            .enumerate()
            .map(|(ordinal, cell)| indexed_claim_from(cell, ordinal))
            .collect()
    })
}
