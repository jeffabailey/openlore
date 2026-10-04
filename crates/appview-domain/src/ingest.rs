//! `ingest` — the PURE verify-before-index gate (WD-104 / I-AV-1).
//!
//! [`ingest_decision`] is the single deterministic `RawRecord -> IngestOutcome`
//! decision. It reuses `claim_domain::verify` + `compute_cid` — the SAME pure
//! core the CLI uses, NO second verification path (WD-104). A record is admitted
//! (`Index`) ONLY when:
//!
//! - its signature verifies against the resolved `VerificationKey`, AND
//! - its recomputed CID matches the published CID, AND
//! - its author (from the SIGNED payload) is carried into the `IndexedClaim`.
//!
//! Anything else is `Reject`ed with a structured [`RejectReason`]. The body is
//! intentionally `todo!()` at the 01-01 bootstrap; the gate behavior is driven
//! by the Phase 02+ ingest scenarios (KPI-AV-3 `indexer_rejects_unverified_claim`)
//! split into its own module for mutation-test clarity (D-D40).
//
// SCAFFOLD: true

use chrono::{DateTime, Utc};
use claim_domain::{
    canonicalize, compute_cid, provenance_verdict, verify, Cid, ClaimRecord, Did, KeyId,
    Provenance, RecordOrigin, SelfAttestedClaim, UnsignedClaim, VerificationKey, VerifyingKey,
};
use ports::PeerClaimProvenance;

use crate::{IndexedClaim, IngestOutcome, RawRecord, RejectReason};

/// The PURE verify-before-index gate. Calls `claim_domain::verify` +
/// `compute_cid` (the SAME pure core; NO second verification path, WD-104).
/// Deterministic; no I/O; clock-free.
///
/// Returns [`IngestOutcome::Index`] with the verified, author-attributed claim
/// iff the signature verifies AND the recomputed CID matches the published CID;
/// otherwise [`IngestOutcome::Reject`] with the structured reason.
///
/// The classification order encodes the WD-104 precondition chain so each
/// adversarial posture maps to its distinct [`RejectReason`]:
///
/// 1. no usable signature block       → [`RejectReason::Unsigned`]
/// 2. signature does not verify        → [`RejectReason::BadSignature`]
/// 3. recomputed CID ≠ published CID    → [`RejectReason::CidMismatch`]
/// 4. all three pass                    → [`IngestOutcome::Index`]
pub fn ingest_decision(record: &RawRecord, resolved_key: &VerificationKey) -> IngestOutcome {
    let payload = &record.raw_payload;

    // 1. Unsigned: no usable signature block (the gate never even reaches the
    //    pure verify path for these). A record with no signature bytes can carry
    //    no Ed25519 signature, so it is structurally unsigned.
    if payload.signature.signature_bytes.is_empty() {
        return IngestOutcome::Reject(RejectReason::Unsigned);
    }

    // 2. Signature: reuse the SAME pure `claim_domain::verify` (no second path).
    //    Bridge the ADR-026 `VerificationKey` decode output (32 raw pubkey bytes)
    //    into the lower-level `VerifyingKey` `verify` consumes — `verify`'s
    //    signature stays UNCHANGED (the bridge is wired HERE, at the call site).
    let pubkey = VerifyingKey(resolved_key.0.clone());
    if verify(payload, &pubkey).is_err() {
        return IngestOutcome::Reject(RejectReason::BadSignature);
    }

    // 3. CID match: recompute the CID over the canonical bytes (the SAME pure
    //    `canonicalize` + `compute_cid` core) and compare against the
    //    network-published CID. A mismatch is tamper/inconsistency at the
    //    publish boundary even though the signature itself verified.
    let canonical = match canonicalize(&payload.unsigned) {
        Ok(bytes) => bytes,
        // A claim that verified above already canonicalized inside `verify`; a
        // failure here would be a tampered/mismatched body — classify as such
        // rather than panic (railway-oriented; pure).
        Err(_) => return IngestOutcome::Reject(RejectReason::CidMismatch),
    };
    if compute_cid(&canonical) != record.published_cid {
        return IngestOutcome::Reject(RejectReason::CidMismatch);
    }

    // 4. Admit: build the verified, attributed IndexedClaim. Attribution is
    //    DERIVED byte-equal from the SIGNED payload (never source_pds). The
    //    verified marker is NEVER empty (WD-104).
    IngestOutcome::Index(build_indexed_claim(record))
}

/// The gate for one record of one repo, as listed from `origin` under
/// `rkey` (ADR-071). An app-signed record goes through [`ingest_decision`]
/// unchanged, its published CID being the rkey; with no resolved key it is
/// refused as a bad signature, exactly as before. A self-attested record is
/// indexed iff [`provenance_verdict`] accepts it, attributed to the repo DID
/// and marked self-attested; otherwise it is refused with that verdict.
pub fn ingest_repo_record(
    record: &ClaimRecord,
    rkey: &str,
    repo_did: &Did,
    origin: RecordOrigin,
    resolved_key: Option<&VerificationKey>,
) -> IngestOutcome {
    match record {
        ClaimRecord::AppSigned(signed) => match resolved_key {
            Some(key) => ingest_decision(
                &RawRecord {
                    published_cid: Cid(rkey.to_string()),
                    raw_payload: signed.clone(),
                    source_pds: String::new(),
                },
                key,
            ),
            None => IngestOutcome::Reject(RejectReason::BadSignature),
        },
        ClaimRecord::SelfAttested(claim) => {
            match provenance_verdict(record, rkey, repo_did, origin, None) {
                Ok(Provenance::SelfAttested { repo_did }) => {
                    IngestOutcome::Index(self_attested_indexed_claim(claim, repo_did))
                }
                Ok(Provenance::AppSigned { .. }) => {
                    IngestOutcome::Reject(RejectReason::Provenance(
                        claim_domain::ProvenanceRejection::MalformedProvenance,
                    ))
                }
                Err(rejection) => IngestOutcome::Reject(RejectReason::Provenance(rejection)),
            }
        }
    }
}

/// A self-attested claim as indexed: attributed to (and verified against)
/// the bare repo DID it was attested by.
fn self_attested_indexed_claim(claim: &SelfAttestedClaim, repo_did: Did) -> IndexedClaim {
    IndexedClaim {
        cid: claim.cid().clone(),
        verified_against: KeyId(repo_did.0.clone()),
        provenance: PeerClaimProvenance::SelfAttested,
        ..indexed_claim_of(claim.unsigned(), repo_did)
    }
}

/// The attributed row content shared by both provenances. Pure; clock-free.
fn indexed_claim_of(unsigned: &UnsignedClaim, author_did: Did) -> IndexedClaim {
    IndexedClaim {
        author_did,
        cid: Cid(String::new()),
        subject: unsigned.subject.clone(),
        predicate: unsigned.predicate.clone(),
        object: unsigned.object.clone(),
        confidence: confidence_value(&unsigned.confidence),
        composed_at: parse_composed_at(&unsigned.composed_at),
        verified_against: KeyId(String::new()),
        evidence: unsigned.evidence.clone(),
        references: unsigned.references.clone(),
        relationship: ports::AuthorRelationship::NetworkUnfollowed,
        provenance: PeerClaimProvenance::AppSigned,
    }
}

/// Build the verified, attributed [`IndexedClaim`] from a record that passed all
/// three gate preconditions. Pure; clock-free — every field is derived from the
/// signed payload + the verified published CID.
fn build_indexed_claim(record: &RawRecord) -> IndexedClaim {
    let unsigned = &record.raw_payload.unsigned;
    IndexedClaim {
        // The verified network-published CID.
        cid: record.published_cid.clone(),
        // The verified marker — NEVER empty (WD-104): the signature's
        // verification-method, else the author DID (the key-id form).
        verified_against: verified_against_key_id(record),
        // Author DERIVED byte-equal from the signed payload (anti-merging, WD-103).
        ..indexed_claim_of(unsigned, unsigned.author_did.clone())
    }
}

/// Read the numeric `[0.0, 1.0]` confidence out of the domain `Confidence`
/// wrapper. The wrapper's `value()` accessor is still a RED scaffold (lands in a
/// later step), and its inner field is private to `claim-domain`; the
/// transparent serde representation (a plain JSON number) is the available pure
/// read path — the inverse of the `serde_json::from_value(json!(x))` construction
/// the fixtures use. Pure; no I/O.
fn confidence_value(confidence: &claim_domain::Confidence) -> f64 {
    serde_json::to_value(confidence)
        .ok()
        .and_then(|v| v.as_f64())
        .expect("Confidence serializes transparently as a JSON number")
}

/// Derive the never-empty verified-marker key id (WD-104). Uses the signature's
/// `verification_method` when present, else the author DID (the same key-id
/// shape). Pure.
fn verified_against_key_id(record: &RawRecord) -> KeyId {
    let vm = &record.raw_payload.signature.verification_method;
    if vm.is_empty() {
        KeyId(record.raw_payload.unsigned.author_did.0.clone())
    } else {
        KeyId(vm.clone())
    }
}

/// Parse the signed payload's RFC3339 `composed_at` into a `DateTime<Utc>`.
/// Clock-free + deterministic: a verified claim's timestamp is covered by the
/// signature, so it is well-formed by construction; the Unix-epoch fallback
/// keeps the gate total + pure (NEVER `Utc::now()`).
fn parse_composed_at(rfc3339: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is representable"))
}

#[cfg(test)]
mod tests {
    //! DELIVER inner loop (DD-AV-7): EXHAUSTIVE per-`RejectReason`-arm coverage
    //! of the verify-before-index gate. Each adversarial posture must map to its
    //! distinct reject reason; the valid posture must `Index` with the derived
    //! author + a non-empty `verified_against`. Self-contained (no test-support
    //! dep — cycle): records are built directly from `claim_domain` primitives
    //! over a deterministic seeded Ed25519 keypair, the same shape the
    //! `proptest_strategies` generator uses.

    use super::*;
    use crate::RejectReason;
    use claim_domain::{
        canonicalize, compute_cid, sign, Cid, Confidence, Did, SignatureBlock, SignedClaim,
        SigningKey, UnsignedClaim,
    };
    use ed25519_dalek::SigningKey as DalekSigningKey;

    const AUTHOR: &str = "did:plc:priya-test#org.openlore.application";

    /// A deterministic `(SigningKey, VerificationKey)` test keypair. The
    /// `VerificationKey` (ADR-026 decode output) wraps the same pubkey bytes the
    /// gate bridges into the lower-level `VerifyingKey` for `claim_domain::verify`.
    fn keypair() -> (SigningKey, VerificationKey) {
        let dalek_sk = DalekSigningKey::from_bytes(&[7u8; 32]);
        let pubkey = dalek_sk.verifying_key().to_bytes().to_vec();
        (
            SigningKey(dalek_sk.to_bytes().to_vec()),
            VerificationKey(pubkey),
        )
    }

    fn sample_unsigned() -> UnsignedClaim {
        let confidence: Confidence = serde_json::from_value(serde_json::json!(0.82))
            .expect("0.82 is a well-formed confidence");
        UnsignedClaim {
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.reproducible-builds".to_string(),
            evidence: vec!["https://example.test/evidence/bazel".to_string()],
            confidence,
            author_did: Did(AUTHOR.to_string()),
            composed_at: "2026-05-26T12:00:00Z".to_string(),
            references: Vec::new(),
            reason: None,
        }
    }

    /// Build the canonical body CID for the sample claim.
    fn body_cid(unsigned: &UnsignedClaim) -> Cid {
        compute_cid(&canonicalize(unsigned).expect("sample claim canonicalizes"))
    }

    /// A VALID signed record: real signature over the body CID, published CID
    /// recomputes byte-equal.
    fn valid_record(sk: &SigningKey) -> RawRecord {
        let unsigned = sample_unsigned();
        let cid = body_cid(&unsigned);
        let signature = sign(&cid, sk).expect("sign succeeds");
        RawRecord {
            published_cid: cid,
            raw_payload: SignedClaim {
                unsigned,
                signature,
            },
            source_pds: "https://pds.example.test".to_string(),
        }
    }

    /// An UNSIGNED record: empty signature bytes → `RejectReason::Unsigned`.
    fn unsigned_record() -> RawRecord {
        let unsigned = sample_unsigned();
        let cid = body_cid(&unsigned);
        RawRecord {
            published_cid: cid.clone(),
            raw_payload: SignedClaim {
                unsigned,
                signature: SignatureBlock {
                    signed_cid: cid,
                    signature_bytes: Vec::new(),
                    verification_method: String::new(),
                },
            },
            source_pds: "https://pds.example.test".to_string(),
        }
    }

    /// A TAMPERED-SIGNATURE record: a real 64-byte signature with the last byte
    /// flipped → `RejectReason::BadSignature`.
    fn tampered_record(sk: &SigningKey) -> RawRecord {
        let mut record = valid_record(sk);
        let last = record.raw_payload.signature.signature_bytes.len() - 1;
        record.raw_payload.signature.signature_bytes[last] ^= 0x01;
        record
    }

    /// A CID-MISMATCH record: a valid signature but a published CID that does
    /// NOT recompute → `RejectReason::CidMismatch`.
    fn cid_mismatch_record(sk: &SigningKey) -> RawRecord {
        let mut record = valid_record(sk);
        record.published_cid = Cid(format!("{}tampered", record.published_cid.0));
        record
    }

    #[test]
    fn valid_record_indexes_with_derived_author_and_nonempty_verified_against() {
        let (sk, vk) = keypair();
        let record = valid_record(&sk);

        match ingest_decision(&record, &vk) {
            IngestOutcome::Index(claim) => {
                // Author DERIVED byte-equal from the signed payload.
                assert_eq!(
                    claim.author_did, record.raw_payload.unsigned.author_did,
                    "indexed author must equal the signed payload author"
                );
                // verified_against NEVER empty (WD-104 universal verified marker).
                assert!(
                    !claim.verified_against.0.is_empty(),
                    "verified_against must never be empty on Index"
                );
                // CID is the verified published CID.
                assert_eq!(claim.cid, record.published_cid);
            }
            other => panic!("a valid signed record must Index, got {other:?}"),
        }
    }

    #[test]
    fn indexed_claim_carries_the_signed_payloads_exact_confidence() {
        // The signed payload's confidence is a SPECIFIC non-boundary value (0.82
        // — distinct from the 0.0/1.0/-1.0 a constant-replacement mutant of
        // `confidence_value` would yield). The indexed claim must carry that exact
        // value through from the SIGNED payload, so any constant mutant diverges.
        let (sk, vk) = keypair();
        let record = valid_record(&sk);

        match ingest_decision(&record, &vk) {
            IngestOutcome::Index(claim) => {
                assert_eq!(
                    claim.confidence, 0.82,
                    "the indexed claim must carry the signed payload's EXACT confidence"
                );
            }
            other => panic!("a valid signed record must Index, got {other:?}"),
        }
    }

    #[test]
    fn unsigned_record_rejects_with_unsigned_reason() {
        let (_sk, vk) = keypair();
        assert_eq!(
            ingest_decision(&unsigned_record(), &vk),
            IngestOutcome::Reject(RejectReason::Unsigned),
            "a record with no signature bytes must Reject(Unsigned)"
        );
    }

    #[test]
    fn tampered_signature_record_rejects_with_bad_signature_reason() {
        let (sk, vk) = keypair();
        assert_eq!(
            ingest_decision(&tampered_record(&sk), &vk),
            IngestOutcome::Reject(RejectReason::BadSignature),
            "a tampered signature must Reject(BadSignature)"
        );
    }

    #[test]
    fn cid_mismatch_record_rejects_with_cid_mismatch_reason() {
        let (sk, vk) = keypair();
        assert_eq!(
            ingest_decision(&cid_mismatch_record(&sk), &vk),
            IngestOutcome::Reject(RejectReason::CidMismatch),
            "a published CID that does not recompute must Reject(CidMismatch)"
        );
    }

    // -------------------------------------------------------------------------
    // ingest_repo_record (ADR-071): one record of one listed repo
    // -------------------------------------------------------------------------

    use claim_domain::{ClaimRecord, ProvenanceRejection, RecordOrigin, SelfAttestedClaim};
    use proptest::prelude::*;

    const REPO: &str = "did:plc:priya-test";

    /// Universe of a self-attested record as listed: who it names as author,
    /// which rkey it is listed under, and where it was fetched from.
    #[derive(Debug, Clone, Copy)]
    enum Author {
        Repo,
        OtherDid,
        RepoAppKey,
    }

    fn arb_author() -> impl Strategy<Value = Author> {
        prop_oneof![
            Just(Author::Repo),
            Just(Author::OtherDid),
            Just(Author::RepoAppKey)
        ]
    }

    fn arb_origin() -> impl Strategy<Value = RecordOrigin> {
        prop_oneof![Just(RecordOrigin::AuthorPds), Just(RecordOrigin::Relay)]
    }

    fn self_attested(author: Author, subject: &str, confidence: f64) -> SelfAttestedClaim {
        let author_did = match author {
            Author::Repo => REPO.to_string(),
            Author::OtherDid => "did:plc:sam-test".to_string(),
            Author::RepoAppKey => format!("{REPO}#org.openlore.application"),
        };
        let unsigned = UnsignedClaim {
            subject: subject.to_string(),
            author_did: Did(author_did),
            confidence: serde_json::from_value(serde_json::json!(confidence))
                .expect("confidence in range"),
            ..sample_unsigned()
        };
        SelfAttestedClaim::new(unsigned).expect("canonicalizes")
    }

    proptest! {
        /// The self-attested arm indexes iff the ADR-071 verdict accepts; an
        /// indexed row is attributed to the repo DID, keyed by its rkey and
        /// marked self-attested; a refusal carries the verdict's reason. A
        /// record fetched through a relay is therefore never indexed.
        #[test]
        fn a_self_attested_record_is_indexed_iff_its_provenance_verdict_accepts(
            author in arb_author(),
            listed_under_own_cid in any::<bool>(),
            origin in arb_origin(),
            subject in "github:[a-z]{1,8}/[a-z]{1,8}",
            confidence in 0.0f64..=1.0,
        ) {
            let claim = self_attested(author, &subject, confidence);
            let rkey = if listed_under_own_cid { claim.cid().0.clone() } else { "bafyother".to_string() };
            let record = ClaimRecord::SelfAttested(claim.clone());
            let repo = Did(REPO.to_string());
            let verdict = claim_domain::provenance_verdict(&record, &rkey, &repo, origin, None);

            match (ingest_repo_record(&record, &rkey, &repo, origin, None), verdict) {
                (IngestOutcome::Index(indexed), Ok(_)) => {
                    prop_assert_eq!(&indexed.author_did, &repo);
                    prop_assert_eq!(&indexed.verified_against.0, REPO);
                    prop_assert_eq!(&indexed.cid.0, &rkey);
                    prop_assert_eq!(indexed.provenance, PeerClaimProvenance::SelfAttested);
                    prop_assert_eq!(&indexed.subject, &subject);
                    prop_assert_eq!(origin, RecordOrigin::AuthorPds);
                }
                (IngestOutcome::Reject(reason), Err(rejection)) => {
                    prop_assert_eq!(reason, RejectReason::Provenance(rejection));
                }
                (outcome, verdict) => prop_assert!(false, "gate {outcome:?} disagrees with verdict {verdict:?}"),
            }
        }

        /// An app-signed record decides exactly as the unchanged gate does, with
        /// its rkey as the published CID, whatever repo or origin it was listed
        /// from; with no resolved key it is a bad signature, as before.
        #[test]
        fn an_app_signed_record_decides_exactly_as_before(
            posture in 0u8..4,
            origin in arb_origin(),
            listed_in_author_repo in any::<bool>(),
        ) {
            let (sk, vk) = keypair();
            let raw = match posture {
                0 => valid_record(&sk),
                1 => unsigned_record(),
                2 => tampered_record(&sk),
                _ => cid_mismatch_record(&sk),
            };
            let repo = Did(if listed_in_author_repo { REPO } else { "did:plc:elsewhere" }.to_string());
            let record = ClaimRecord::AppSigned(raw.raw_payload.clone());
            let rkey = raw.published_cid.0.clone();
            prop_assert_eq!(
                ingest_repo_record(&record, &rkey, &repo, origin, Some(&vk)),
                ingest_decision(&raw, &vk)
            );
            prop_assert_eq!(
                ingest_repo_record(&record, &rkey, &repo, origin, None),
                IngestOutcome::Reject(RejectReason::BadSignature)
            );
        }
    }

    /// The KPI-BRA-6 guard: an honest self-attested claim read through a relay
    /// is refused as unverifiable provenance, never indexed.
    // bypass: the single named example the mutation note requires killing.
    #[test]
    fn an_honest_self_attested_claim_fetched_through_a_relay_is_refused() {
        let claim = self_attested(Author::Repo, "github:priyaraman/tidepool", 0.25);
        let rkey = claim.cid().0.clone();
        let repo = Did(REPO.to_string());
        assert_eq!(
            ingest_repo_record(
                &ClaimRecord::SelfAttested(claim),
                &rkey,
                &repo,
                RecordOrigin::Relay,
                None
            ),
            IngestOutcome::Reject(RejectReason::Provenance(
                ProvenanceRejection::UnverifiableProvenance
            ))
        );
    }
}
