//! Canonical slice-05 ingest fixtures — the adversarial + valid `RawRecord`
//! set, a real-`z6Mk...` PLC DID-document fixture, and the network-search
//! corpora the appview-search acceptance scenarios seed.
//!
//! Symmetric with `fixtures_peer.rs` (slice-03 peer-claim fixtures) +
//! `fixtures_scoring.rs` (slice-04 graph-shape fixtures): each fixture is a
//! free function returning a fresh, immutable value. No shared mutable state.
//! Tests compose by passing values through. Functional paradigm (ADR-007).
//!
//! ## What the bootstrap step provides (DD-AV-13)
//!
//! - [`RawRecordSpec`] — a declarative builder for a `ports::RawRecord`. It
//!   carries the compose-time fields plus an adversarial [`Posture`] knob so a
//!   single recipe yields a VALID signed record, an UNSIGNED one, a
//!   TAMPERED-SIGNATURE one, or a CID-MISMATCH one. Crucially the VALID posture
//!   produces a REAL Ed25519 signature over the canonical claim bytes (via
//!   `claim_domain::sign`) and a `published_cid` that recomputes byte-equal
//!   (via `claim_domain::compute_cid` over `claim_domain::canonicalize`), so the
//!   appview-domain verify-before-index gate runs the REAL pure path on it
//!   (no second verification path; WD-104). The adversarial postures each break
//!   exactly one of the gate's preconditions.
//!
//! - The four NAMED ingest fixtures the AVC-1 generator + the AV-3 release-gate
//!   scenario reference by name: [`fixture_ingest_valid_signed`],
//!   [`fixture_ingest_unsigned`], [`fixture_ingest_tampered_signature`],
//!   [`fixture_ingest_cid_mismatch`].
//!
//! - A real-`z6Mk...` DID-document fixture ([`fixture_real_z6mk_did_doc`]) for a
//!   known deterministic test keypair, so the ADR-026
//!   `claim_domain::decode_ed25519_multibase` decode runs the REAL decode path
//!   (AV-4 gold path) rather than the slice-03 env seam. The `z6Mk...` string is
//!   computed from the keypair's public key by the same multicodec + base58btc +
//!   `z` multibase procedure the production decode inverts (`0xed 0x01` Ed25519
//!   multicodec prefix ++ 32 pubkey bytes).
//!
//! - The network-search corpora ([`corpus_reproducible_builds_nine_authors`],
//!   [`corpus_deno_dependency_pinning_two_authors`],
//!   [`corpus_priya_eight_claims_six_subjects`],
//!   [`corpus_bazel_five_distinct_authors`]) the `seed_network_index` harness
//!   turns into REAL `index.duckdb` rows. Each corpus is a `Vec<RawRecordSpec>`
//!   whose VALID postures verify against the resolvable keypairs the
//!   accompanying DID-doc fixtures expose.
//!
//! ## Expected-value cross-check (DD-AV-13 criterion 1)
//!
//! The builders produce `ports::RawRecord` (re-exported via `appview-domain`)
//! whose `raw_payload` is a `claim_domain::SignedClaim`; the corpora author
//! DIDs (`did:plc:priya-test`, `did:plc:sven-test`, `did:plc:rachel-test`, …),
//! subjects (`github:bazelbuild/bazel`, `github:denoland/deno`, …), objects
//! (`org.openlore.philosophy.reproducible-builds`,
//! `org.openlore.philosophy.dependency-pinning`, …) and confidences (Priya 0.82
//! / 0.70, Sven 0.65, Rachel 0.88, …) are cross-checked against the
//! ports/appview-domain ADTs + the user-stories worked examples.
//!
//! Bootstrap marker: this module is materialized by DELIVER's slice-05
//! bootstrap step (DD-AV-13). The named-fixture + corpus DATA is honest now
//! (the AVC-* generator bodies + the per-scenario seeding land in Phase 02);
//! the point of the bootstrap is that the harness + fixtures COMPILE so every
//! slice-05 acceptance `#[test]` reaches its own `todo!()` (RED, not BROKEN).
//
// SCAFFOLD: true (slice-05)

#![allow(dead_code)]

use claim_domain::{
    canonicalize, compute_cid, sign, Cid, ClaimReference, Confidence, Did, KeyId, ReferenceType,
    SignatureBlock, SignedClaim, SigningKey, UnsignedClaim, VerifyingKey,
};
use ed25519_dalek::SigningKey as DalekSigningKey;
use ports::RawRecord;

// -----------------------------------------------------------------------------
// Well-known fixture identities (deterministic seeded Ed25519 keypairs)
// -----------------------------------------------------------------------------

/// Priya — the headline unfollowed network author (US-AV-001..006 examples).
pub const PRIYA_DID: &str = "did:plc:priya-test";
/// Sven — the second unfollowed author in the AVC-5 / AV-9 anti-merging pairing.
pub const SVEN_DID: &str = "did:plc:sven-test";
/// Rachel — the ALREADY-SUBSCRIBED peer (slice-03 carry-over; subscribed-peer label).
pub const RACHEL_DID: &str = "did:plc:rachel-test";

/// The OpenLore application verification-method fragment appended to every
/// author DID in the signed payload (mirrors `fixtures_peer.rs`).
pub const APP_FRAGMENT: &str = "#org.openlore.application";

/// Deterministic 32-byte Ed25519 seed for a fixture author. Distinct seeds per
/// author so their public keys (and therefore their `z6Mk...` values) differ.
fn seed_for(did: &str) -> [u8; 32] {
    let mut seed = [0u8; 32];
    let bytes = did.as_bytes();
    // Fill the seed deterministically from the DID so a given DID always yields
    // the same keypair across runs/platforms (load-bearing: the z6Mk value the
    // DID-doc fixture exposes must match the key that signs the records).
    for (i, slot) in seed.iter_mut().enumerate() {
        *slot = bytes[i % bytes.len()].wrapping_add(i as u8);
    }
    seed
}

/// A resolvable fixture keypair: the signing key, the verifying key, and the
/// `z6Mk...` `publicKeyMultibase` the PLC DID-document records (ADR-026).
#[derive(Debug, Clone)]
pub struct FixtureKeypair {
    pub did: String,
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    /// The ADR-026 `z6Mk...` base58btc multibase encoding of the public key.
    pub public_key_multibase: String,
    /// The DID-document verification-method id (`<did>#org.openlore.application`).
    pub key_id: KeyId,
}

impl FixtureKeypair {
    /// Derive the deterministic keypair + z6Mk multibase for `did`.
    pub fn for_did(did: &str) -> Self {
        let dalek_sk = DalekSigningKey::from_bytes(&seed_for(did));
        let dalek_vk = dalek_sk.verifying_key();
        let pubkey_bytes = dalek_vk.to_bytes().to_vec();
        let public_key_multibase = encode_ed25519_z6mk(&pubkey_bytes);
        Self {
            did: did.to_string(),
            signing_key: SigningKey(dalek_sk.to_bytes().to_vec()),
            verifying_key: VerifyingKey(pubkey_bytes),
            public_key_multibase,
            key_id: KeyId(format!("{did}{APP_FRAGMENT}")),
        }
    }
}

// -----------------------------------------------------------------------------
// Real-z6Mk DID-document fixture (ADR-026 gold path; AV-4)
// -----------------------------------------------------------------------------

/// A fixture PLC DID-document entry carrying a REAL `z6Mk...` value for a known
/// test keypair. The slice-05 `seed_network_index` harness hosts this on the
/// fixture PLC resolver so the indexer's ADR-026
/// `claim_domain::decode_ed25519_multibase` runs the REAL decode path (NOT the
/// slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam — release-forbidden, AV-4).
#[derive(Debug, Clone)]
pub struct DidDocFixture {
    pub did: String,
    pub key_id: KeyId,
    /// The `z6Mk...` base58btc multibase string the real decode inverts.
    pub public_key_multibase: String,
    /// The decoded verifying key (so a test can cross-check the decode output).
    pub verifying_key: VerifyingKey,
}

/// The canonical real-`z6Mk` DID-document fixture for Priya — the keypair the
/// walking-skeleton beat-1 (AV-1) + the AV-4 gold path resolve against.
pub fn fixture_real_z6mk_did_doc() -> DidDocFixture {
    did_doc_for(PRIYA_DID)
}

/// A real-`z6Mk` DID-document fixture for an arbitrary fixture DID.
pub fn did_doc_for(did: &str) -> DidDocFixture {
    let kp = FixtureKeypair::for_did(did);
    DidDocFixture {
        did: kp.did,
        key_id: kp.key_id,
        public_key_multibase: kp.public_key_multibase,
        verifying_key: kp.verifying_key,
    }
}

// -----------------------------------------------------------------------------
// RawRecordSpec — the declarative ingest-record builder
// -----------------------------------------------------------------------------

/// The adversarial posture a [`RawRecordSpec`] is materialized under. `Valid` is
/// the gold path (real signature, recomputing CID); each other variant breaks
/// exactly ONE verify-before-index precondition (WD-104 / AV-3 adversarial set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    /// Real Ed25519 signature over the canonical bytes; `published_cid` recomputes.
    Valid,
    /// No usable signature block (empty signature bytes) → `RejectReason::Unsigned`.
    Unsigned,
    /// A real-shaped but tampered signature (last byte flipped) → `BadSignature`.
    TamperedSignature,
    /// A valid signature but a `published_cid` that does NOT recompute → `CidMismatch`.
    CidMismatch,
}

/// A declarative recipe for one `ports::RawRecord`. Builders set the compose-time
/// claim fields + the adversarial [`Posture`]; [`RawRecordSpec::into_raw_record`]
/// runs the REAL crypto to produce the wire `RawRecord` the ingest gate consumes.
#[derive(Debug, Clone)]
pub struct RawRecordSpec {
    pub author_did: String,
    pub subject: String,
    pub object: String,
    pub predicate: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub composed_at: String,
    pub references: Vec<ClaimReference>,
    pub source_pds: String,
    pub posture: Posture,
}

impl RawRecordSpec {
    /// A valid signed claim recipe with sensible defaults. Customize via the
    /// `with_*` builders; flip to an adversarial posture via [`Self::posture`].
    pub fn valid(author_did: &str, subject: &str, object: &str, confidence: f64) -> Self {
        Self {
            author_did: author_did.to_string(),
            subject: subject.to_string(),
            object: object.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            confidence,
            evidence: vec![format!("https://example.test/evidence/{}", short_tag(subject))],
            composed_at: "2026-05-26T12:00:00Z".to_string(),
            references: Vec::new(),
            source_pds: "https://pds.example.test".to_string(),
            posture: Posture::Valid,
        }
    }

    /// Set the adversarial posture (defaults to `Valid`).
    pub fn posture(mut self, posture: Posture) -> Self {
        self.posture = posture;
        self
    }

    /// Override the evidence URLs.
    pub fn with_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Override the compose timestamp (RFC3339 UTC).
    pub fn with_composed_at(mut self, composed_at: &str) -> Self {
        self.composed_at = composed_at.to_string();
        self
    }

    /// Add a typed reference (e.g. a `Counters` pointer for the OD-AV-7 fixture).
    pub fn with_reference(mut self, ref_type: ReferenceType, cid: &str) -> Self {
        self.references.push(ClaimReference {
            ref_type,
            cid: Cid(cid.to_string()),
        });
        self
    }

    /// Materialize the wire `ports::RawRecord` by running the REAL crypto under
    /// the chosen posture. The keypair is derived deterministically from the
    /// author DID (so the resolvable DID-doc fixture exposes the matching key).
    pub fn into_raw_record(self) -> RawRecord {
        let kp = FixtureKeypair::for_did(&self.author_did);
        let unsigned = self.unsigned_claim();
        let canonical = canonicalize(&unsigned).expect("fixture claim canonicalizes");
        let body_cid = compute_cid(&canonical);

        let signature = self.signature_for(&body_cid, &kp);
        let published_cid = match self.posture {
            // The cid-mismatch posture publishes a CID that does NOT recompute
            // from the canonical bytes — the recompute-vs-published gate fails.
            Posture::CidMismatch => Cid(format!("{}tampered", body_cid.0)),
            _ => body_cid,
        };

        RawRecord {
            published_cid,
            raw_payload: SignedClaim {
                unsigned,
                signature,
            },
            source_pds: self.source_pds.clone(),
        }
    }

    /// The unsigned claim assembled from the spec fields. `Confidence` is built
    /// via serde (its smart constructor is still a RED scaffold — the same trick
    /// `fixtures.rs` / `identity.rs` use).
    fn unsigned_claim(&self) -> UnsignedClaim {
        let confidence: Confidence = serde_json::from_value(serde_json::json!(self.confidence))
            .expect("fixture confidence value is well-formed JSON number");
        UnsignedClaim {
            subject: self.subject.clone(),
            predicate: self.predicate.clone(),
            object: self.object.clone(),
            evidence: self.evidence.clone(),
            confidence,
            author_did: Did(format!("{}{APP_FRAGMENT}", self.author_did)),
            composed_at: self.composed_at.clone(),
            references: self.references.clone(),
            reason: None,
        }
    }

    /// Produce the signature block under the chosen posture.
    fn signature_for(&self, body_cid: &Cid, kp: &FixtureKeypair) -> SignatureBlock {
        let verification_method = format!("{}{APP_FRAGMENT}", self.author_did);
        match self.posture {
            Posture::Unsigned => SignatureBlock {
                signed_cid: body_cid.clone(),
                // No usable signature → the gate classifies `Unsigned`.
                signature_bytes: Vec::new(),
                verification_method,
            },
            Posture::TamperedSignature => {
                let mut block = real_signature(body_cid, &kp.signing_key, &verification_method);
                // Flip the last byte so the signature no longer verifies.
                if let Some(last) = block.signature_bytes.last_mut() {
                    *last ^= 0x01;
                } else {
                    block.signature_bytes.push(0x01);
                }
                block
            }
            // Valid + CidMismatch both carry a REAL signature over the real body
            // CID; CidMismatch breaks the recompute gate at the `published_cid`
            // level, not the signature.
            Posture::Valid | Posture::CidMismatch => {
                real_signature(body_cid, &kp.signing_key, &verification_method)
            }
        }
    }
}

/// Sign `body_cid` with `signing_key` via the REAL pure `claim_domain::sign`.
fn real_signature(body_cid: &Cid, signing_key: &SigningKey, vm: &str) -> SignatureBlock {
    let mut block = sign(body_cid, signing_key).expect("fixture signing succeeds");
    block.verification_method = vm.to_string();
    block
}

// -----------------------------------------------------------------------------
// The four NAMED ingest fixtures (AVC-1 generator + AV-3 release gate)
// -----------------------------------------------------------------------------

/// A VALID signed public claim by Priya on bazel embodying reproducible-builds
/// (0.82) — the walking-skeleton beat-1 record (AV-1) + the valid member of the
/// AV-3 adversarial set. Verifies against [`fixture_real_z6mk_did_doc`].
pub fn fixture_ingest_valid_signed() -> RawRecordSpec {
    RawRecordSpec::valid(
        PRIYA_DID,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        0.82,
    )
}

/// An UNSIGNED record (no usable signature block) → `RejectReason::Unsigned`.
/// Member of the AV-3 adversarial set; NEVER enters the index.
pub fn fixture_ingest_unsigned() -> RawRecordSpec {
    fixture_ingest_valid_signed().posture(Posture::Unsigned)
}

/// A TAMPERED-SIGNATURE record (last signature byte flipped) →
/// `RejectReason::BadSignature`. Member of the AV-3 adversarial set.
pub fn fixture_ingest_tampered_signature() -> RawRecordSpec {
    fixture_ingest_valid_signed().posture(Posture::TamperedSignature)
}

/// A CID-MISMATCH record (recomputed CID != published CID) →
/// `RejectReason::CidMismatch`. Member of the AV-3 adversarial set.
pub fn fixture_ingest_cid_mismatch() -> RawRecordSpec {
    fixture_ingest_valid_signed().posture(Posture::CidMismatch)
}

/// The canonical AV-3 release-gate adversarial set: the three adversarial
/// postures PLUS one valid record (the four-record `listRecords` surface the
/// `FakeIngestSource` hosts for `indexer_rejects_unverified_claim`).
pub fn fixture_ingest_adversarial_set_plus_one_valid() -> Vec<RawRecordSpec> {
    vec![
        fixture_ingest_unsigned(),
        fixture_ingest_tampered_signature(),
        fixture_ingest_cid_mismatch(),
        fixture_ingest_valid_signed(),
    ]
}

// -----------------------------------------------------------------------------
// Network-search corpora (seed_network_index inputs)
// -----------------------------------------------------------------------------

/// US-AV-002 Example 1: verified reproducible-builds claims by 9 DISTINCT
/// authors across 7 subjects, including Priya (UNFOLLOWED, bazel 0.82) + Rachel
/// (SUBSCRIBED peer, nixpkgs 0.88). The headline `--object` discovery corpus.
pub fn corpus_reproducible_builds_nine_authors() -> Vec<RawRecordSpec> {
    let object = "org.openlore.philosophy.reproducible-builds";
    let authors_subjects = [
        (PRIYA_DID, "github:bazelbuild/bazel", 0.82),
        (RACHEL_DID, "github:NixOS/nixpkgs", 0.88),
        ("did:plc:author3-test", "github:reproducible-builds/repro", 0.74),
        ("did:plc:author4-test", "github:gentoo/gentoo", 0.66),
        ("did:plc:author5-test", "github:guix/guix", 0.91),
        ("did:plc:author6-test", "github:debian/debian", 0.58),
        ("did:plc:author7-test", "github:archlinux/arch", 0.63),
        ("did:plc:author8-test", "github:fedora/fedora", 0.70),
        ("did:plc:author9-test", "github:openSUSE/opensuse", 0.77),
    ];
    authors_subjects
        .iter()
        .map(|(did, subject, conf)| RawRecordSpec::valid(did, subject, object, *conf))
        .collect()
}

/// US-AV-002 Example 2 / AVC-5: github:denoland/deno + dependency-pinning by two
/// UNFOLLOWED authors (Priya 0.70, Sven 0.65) — the identical-(subject,object)
/// zero-merge fixture. The two records MUST stay two distinct attributed rows.
pub fn corpus_deno_dependency_pinning_two_authors() -> Vec<RawRecordSpec> {
    let subject = "github:denoland/deno";
    let object = "org.openlore.philosophy.dependency-pinning";
    vec![
        RawRecordSpec::valid(PRIYA_DID, subject, object, 0.70),
        RawRecordSpec::valid(SVEN_DID, subject, object, 0.65),
    ]
}

/// US-AV-003 Example 1: did:plc:priya-test authors 8 verified claims across 6
/// subjects (bazel x2, buck2, nixpkgs, pants, please, ninja). Maria unfollowed.
/// The contributor-trail corpus (one author, the "reasoning trail not consensus"
/// framing is the load-bearing assertion).
pub fn corpus_priya_eight_claims_six_subjects() -> Vec<RawRecordSpec> {
    let entries = [
        ("github:bazelbuild/bazel", "org.openlore.philosophy.reproducible-builds", 0.82),
        ("github:bazelbuild/bazel", "org.openlore.philosophy.hermetic-builds", 0.79),
        ("github:facebook/buck2", "org.openlore.philosophy.dependency-pinning", 0.68),
        ("github:NixOS/nixpkgs", "org.openlore.philosophy.reproducible-builds", 0.85),
        ("github:pantsbuild/pants", "org.openlore.philosophy.incremental-builds", 0.61),
        ("github:please-build/please", "org.openlore.philosophy.hermetic-builds", 0.57),
        ("github:ninja-build/ninja", "org.openlore.philosophy.minimal-tooling", 0.73),
        ("github:NixOS/nixpkgs", "org.openlore.philosophy.dependency-pinning", 0.64),
    ];
    entries
        .iter()
        .map(|(subject, object, conf)| RawRecordSpec::valid(PRIYA_DID, subject, object, *conf))
        .collect()
}

/// US-AV-003 Example 2: github:bazelbuild/bazel with verified claims from 5
/// DISTINCT network authors (the subject-survey anti-merging fixture). Grouped
/// by author; NO "bazel: the network thinks X" merged row.
pub fn corpus_bazel_five_distinct_authors() -> Vec<RawRecordSpec> {
    let subject = "github:bazelbuild/bazel";
    let entries = [
        (PRIYA_DID, "org.openlore.philosophy.reproducible-builds", 0.82),
        (SVEN_DID, "org.openlore.philosophy.hermetic-builds", 0.71),
        ("did:plc:tobias-test", "org.openlore.philosophy.dependency-pinning", 0.66),
        ("did:plc:aanya-test", "org.openlore.philosophy.incremental-builds", 0.58),
        ("did:plc:lena-test", "org.openlore.philosophy.minimal-tooling", 0.74),
    ];
    entries
        .iter()
        .map(|(did, object, conf)| RawRecordSpec::valid(did, subject, object, *conf))
        .collect()
}

// -----------------------------------------------------------------------------
// z6Mk multibase encoding (ADR-026 — the inverse of decode_ed25519_multibase)
// -----------------------------------------------------------------------------

/// Encode 32 Ed25519 public-key bytes as a `z6Mk...` multibase string: the
/// Ed25519 multicodec prefix (`0xed 0x01`) ++ the key bytes, base58btc-encoded,
/// with the `z` multibase prefix (ADR-026). The exact inverse of
/// `claim_domain::decode_ed25519_multibase` — so the real decode runs on this.
fn encode_ed25519_z6mk(pubkey_bytes: &[u8]) -> String {
    let mut payload = Vec::with_capacity(2 + pubkey_bytes.len());
    payload.push(0xed); // Ed25519 multicodec, low byte
    payload.push(0x01); // varint continuation
    payload.extend_from_slice(pubkey_bytes);
    format!("z{}", base58btc_encode(&payload))
}

/// Pure base58btc (Bitcoin alphabet) encoder. No external dependency — the
/// algorithm is short + well-known, and a fixture builder is exactly the right
/// place for a self-contained deterministic encoder.
fn base58btc_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 58] =
        b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Count leading zero bytes → encoded as leading '1's.
    let leading_zeros = input.iter().take_while(|&&b| b == 0).count();

    // Convert the big-endian byte string to base58 via repeated division.
    let mut digits: Vec<u8> = Vec::new();
    for &byte in input {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let mut out = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        out.push('1');
    }
    for &d in digits.iter().rev() {
        out.push(ALPHABET[d as usize] as char);
    }
    out
}

/// A short alphanumeric tag derived from a string (for distinct evidence URLs).
fn short_tag(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// base58btc round-trips the well-known multicodec prefix shape: a `z6Mk`
    /// string ALWAYS results from the Ed25519 multicodec prefix on any 32-byte
    /// key (the `6Mk` is the base58 image of the `0xed 0x01` + high key bits).
    #[test]
    fn encode_yields_z6mk_prefix() {
        let kp = FixtureKeypair::for_did(PRIYA_DID);
        assert!(
            kp.public_key_multibase.starts_with("z6Mk"),
            "Ed25519 multibase must start with z6Mk; got {}",
            kp.public_key_multibase
        );
    }

    /// base58btc of the well-known sequence [0,0,1] encodes leading zeros as
    /// '1' chars (Bitcoin alphabet) — pins the encoder against a known vector.
    #[test]
    fn base58_encodes_leading_zeros_as_ones() {
        assert_eq!(base58btc_encode(&[0, 0, 0]), "111");
        // 0x00 0x01 → leading '1' + base58("1") = "1" + "2" = "12".
        assert_eq!(base58btc_encode(&[0, 1]), "12");
    }

    /// The valid fixture's `published_cid` recomputes byte-equal from the
    /// canonical payload (the gold-path verify-before-index precondition).
    #[test]
    fn valid_fixture_published_cid_recomputes() {
        let rr = fixture_ingest_valid_signed().into_raw_record();
        let canonical = canonicalize(&rr.raw_payload.unsigned).expect("canonicalize");
        let recomputed = compute_cid(&canonical);
        assert_eq!(
            rr.published_cid, recomputed,
            "valid fixture published_cid must recompute byte-equal"
        );
    }

    /// The cid-mismatch fixture's `published_cid` does NOT recompute (so the
    /// recompute gate rejects it).
    #[test]
    fn cid_mismatch_fixture_published_cid_does_not_recompute() {
        let rr = fixture_ingest_cid_mismatch().into_raw_record();
        let canonical = canonicalize(&rr.raw_payload.unsigned).expect("canonicalize");
        let recomputed = compute_cid(&canonical);
        assert_ne!(
            rr.published_cid, recomputed,
            "cid-mismatch fixture published_cid must NOT recompute"
        );
    }

    /// The unsigned fixture carries no signature bytes (the `Unsigned` reject).
    #[test]
    fn unsigned_fixture_has_empty_signature() {
        let rr = fixture_ingest_unsigned().into_raw_record();
        assert!(
            rr.raw_payload.signature.signature_bytes.is_empty(),
            "unsigned fixture must carry no signature bytes"
        );
    }

    /// The corpora carry the expected DISTINCT author counts (cross-checked
    /// against the user-stories worked examples).
    #[test]
    fn corpora_have_expected_author_cardinality() {
        let nine: std::collections::HashSet<_> = corpus_reproducible_builds_nine_authors()
            .iter()
            .map(|s| s.author_did.clone())
            .collect();
        assert_eq!(nine.len(), 9, "reproducible-builds corpus = 9 distinct authors");

        let deno = corpus_deno_dependency_pinning_two_authors();
        assert_eq!(deno.len(), 2, "deno corpus = 2 records (priya + sven)");
        assert!(deno.iter().all(|s| s.subject == "github:denoland/deno"));
        assert!(deno
            .iter()
            .all(|s| s.object == "org.openlore.philosophy.dependency-pinning"));

        let priya_trail = corpus_priya_eight_claims_six_subjects();
        assert_eq!(priya_trail.len(), 8, "priya trail = 8 claims");
        let subjects: std::collections::HashSet<_> =
            priya_trail.iter().map(|s| s.subject.clone()).collect();
        assert_eq!(subjects.len(), 6, "priya trail spans 6 distinct subjects");
        assert!(priya_trail.iter().all(|s| s.author_did == PRIYA_DID));

        let bazel: std::collections::HashSet<_> = corpus_bazel_five_distinct_authors()
            .iter()
            .map(|s| s.author_did.clone())
            .collect();
        assert_eq!(bazel.len(), 5, "bazel survey = 5 distinct authors");
    }
}
