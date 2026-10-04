//! ADR-071 provenance: a claim record is either app-signed (today's path,
//! unchanged) or self-attested by the repo it lives in. The mode is read from
//! the record's own shape plus where it was fetched from; no lexicon field.
//!
//! PURE: no I/O. The effect shell computes the [`RecordOrigin`] by comparing
//! the base URL it actually fetched from with the PDS endpoint it freshly
//! resolved for the repo DID. The AppSigned arm calls [`crate::verify`]
//! verbatim (the AC-009.4 guardrail: app-signed behaviour is byte-identical).

use crate::{canonicalize, compute_cid, verify, Cid, ClaimError, Did, SignedClaim, UnsignedClaim};
use crate::{SignatureBlock, VerifyingKey};

/// A self-attested claim: an unsigned claim paired with its canonical CID.
/// Built only through [`SelfAttestedClaim::new`], so the CID always is the
/// claim's own (no record can carry a CID it does not hash to).
#[derive(Debug, Clone, PartialEq)]
pub struct SelfAttestedClaim {
    unsigned: UnsignedClaim,
    cid: Cid,
}

impl SelfAttestedClaim {
    /// Pair `unsigned` with its canonical CID (the record key it is
    /// published under). Fails only when the claim cannot be canonicalized.
    pub fn new(unsigned: UnsignedClaim) -> Result<Self, ClaimError> {
        let cid = compute_cid(&canonicalize(&unsigned)?);
        Ok(Self { unsigned, cid })
    }

    pub fn unsigned(&self) -> &UnsignedClaim {
        &self.unsigned
    }

    pub fn cid(&self) -> &Cid {
        &self.cid
    }
}

/// A claim record as read from a repo: the two ADR-071 arms.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaimRecord {
    /// Carries an `#org.openlore.application` signature (today's path).
    AppSigned(SignedClaim),
    /// Carries no signature; attested by the repo it is stored in.
    SelfAttested(SelfAttestedClaim),
}

impl ClaimRecord {
    pub fn unsigned(&self) -> &UnsignedClaim {
        match self {
            Self::AppSigned(signed) => &signed.unsigned,
            Self::SelfAttested(claim) => claim.unsigned(),
        }
    }

    /// The claim's CID as carried by the record (recomputed at decode time).
    pub fn cid(&self) -> &Cid {
        match self {
            Self::AppSigned(signed) => &signed.signature.signed_cid,
            Self::SelfAttested(claim) => claim.cid(),
        }
    }
}

/// Where a record was fetched from, as computed by the effect shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOrigin {
    /// The PDS endpoint freshly resolved from the repo DID's document.
    AuthorPds,
    /// Anything else: a relay, a mirror, a cache, an operator-configured URL.
    Relay,
}

impl RecordOrigin {
    /// `AuthorPds` only on an exact match of the fetched base URL with the
    /// resolved PDS endpoint (trailing slashes ignored); never assumed.
    pub fn of(fetched_from: &str, resolved_author_pds: &str) -> Self {
        let base = |url: &str| url.trim_end_matches('/').to_string();
        if base(fetched_from) == base(resolved_author_pds) {
            Self::AuthorPds
        } else {
            Self::Relay
        }
    }
}

/// The admissible modes, before the app signature is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceMode {
    /// A signature is present: the existing verify path decides.
    AppSignedPath,
    SelfAttested {
        repo_did: Did,
    },
}

/// The accepted provenance of a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    AppSigned { key_id: String },
    SelfAttested { repo_did: Did },
}

/// Why a record's provenance is refused (ADR-071 decision table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProvenanceRejection {
    #[error("malformed provenance (claims a key but carries no signature)")]
    MalformedProvenance,
    #[error("foreign repo (self-attested author is not the repo DID)")]
    ForeignRepo,
    #[error("unverifiable provenance (not fetched from the author's own PDS)")]
    UnverifiableProvenance,
    #[error("canonicalization failed")]
    Uncanonicalizable,
    #[error("CID mismatch (possible adversarial input)")]
    IntegrityFailure,
    #[error("no usable verification key")]
    NoAppKey,
    #[error("signature invalid")]
    SignatureInvalid,
}

/// The shape-and-origin half of the verdict: which mode a record may be
/// admitted under, given whether its recomputed CID matches its record key.
/// Shape rejections take precedence; an admitted mode with a CID mismatch is
/// an integrity failure.
pub fn provenance_mode(
    signature_present: bool,
    author: &Did,
    repo_did: &Did,
    origin: RecordOrigin,
    cid_matches_rkey: bool,
) -> Result<ProvenanceMode, ProvenanceRejection> {
    let mode = if signature_present {
        ProvenanceMode::AppSignedPath
    } else if author.0.contains('#') {
        return Err(ProvenanceRejection::MalformedProvenance);
    } else if author != repo_did {
        return Err(ProvenanceRejection::ForeignRepo);
    } else if origin != RecordOrigin::AuthorPds {
        return Err(ProvenanceRejection::UnverifiableProvenance);
    } else {
        ProvenanceMode::SelfAttested {
            repo_did: repo_did.clone(),
        }
    };
    if cid_matches_rkey {
        Ok(mode)
    } else {
        Err(ProvenanceRejection::IntegrityFailure)
    }
}

/// The full ADR-071 verdict over a decoded record and its `rkey` (taken from
/// the `at://` URI, never the view's `cid`). The AppSigned arm runs the
/// unchanged [`verify`] over the recomputed CID with the resolved app key.
pub fn provenance_verdict(
    record: &ClaimRecord,
    rkey: &str,
    repo_did: &Did,
    origin: RecordOrigin,
    app_key: Option<&VerifyingKey>,
) -> Result<Provenance, ProvenanceRejection> {
    let recomputed = canonicalize(record.unsigned())
        .map(|bytes| compute_cid(&bytes))
        .map_err(|_| ProvenanceRejection::Uncanonicalizable)?;
    let signature_present = matches!(record, ClaimRecord::AppSigned(_));
    let mode = provenance_mode(
        signature_present,
        &record.unsigned().author_did,
        repo_did,
        origin,
        recomputed.0 == rkey,
    )?;
    match (mode, record) {
        (ProvenanceMode::AppSignedPath, ClaimRecord::AppSigned(signed)) => {
            verify_app_signature(signed, recomputed, app_key)
        }
        (ProvenanceMode::SelfAttested { repo_did }, _) => Ok(Provenance::SelfAttested { repo_did }),
        (ProvenanceMode::AppSignedPath, ClaimRecord::SelfAttested(_)) => {
            Err(ProvenanceRejection::MalformedProvenance)
        }
    }
}

/// The unchanged app-signed path: `verify` over the recomputed CID.
fn verify_app_signature(
    signed: &SignedClaim,
    recomputed: Cid,
    app_key: Option<&VerifyingKey>,
) -> Result<Provenance, ProvenanceRejection> {
    let key = app_key.ok_or(ProvenanceRejection::NoAppKey)?;
    let to_verify = SignedClaim {
        unsigned: signed.unsigned.clone(),
        signature: SignatureBlock {
            signed_cid: recomputed,
            ..signed.signature.clone()
        },
    };
    verify(&to_verify, key)
        .map(|()| Provenance::AppSigned {
            key_id: signed.signature.verification_method.clone(),
        })
        .map_err(|_| ProvenanceRejection::SignatureInvalid)
}

#[cfg(test)]
mod tests {
    //! Universe: (signature present × author shape × repo DID × origin ×
    //! CID match) and generated claims. The verdict is checked against the
    //! ADR-071 table and against the record-level verdict.
    use super::*;
    use crate::proptest_strategies::arb_unsigned_claim;
    use crate::{sign, Confidence};
    use proptest::prelude::*;

    fn arb_did() -> impl Strategy<Value = Did> {
        "[a-z2-7]{24}".prop_map(|s| Did(format!("did:plc:{s}")))
    }

    fn arb_origin() -> impl Strategy<Value = RecordOrigin> {
        prop_oneof![Just(RecordOrigin::AuthorPds), Just(RecordOrigin::Relay)]
    }

    fn table(
        sig: bool,
        author: &Did,
        repo: &Did,
        origin: RecordOrigin,
        cid_ok: bool,
    ) -> Result<ProvenanceMode, ProvenanceRejection> {
        let admitted = match (sig, author.0.contains('#'), author == repo, origin) {
            (true, ..) => ProvenanceMode::AppSignedPath,
            (false, true, ..) => return Err(ProvenanceRejection::MalformedProvenance),
            (false, false, false, _) => return Err(ProvenanceRejection::ForeignRepo),
            (false, false, true, RecordOrigin::Relay) => {
                return Err(ProvenanceRejection::UnverifiableProvenance)
            }
            (false, false, true, RecordOrigin::AuthorPds) => ProvenanceMode::SelfAttested {
                repo_did: repo.clone(),
            },
        };
        if cid_ok {
            Ok(admitted)
        } else {
            Err(ProvenanceRejection::IntegrityFailure)
        }
    }

    fn self_attested_by(author: &Did, mut unsigned: UnsignedClaim) -> ClaimRecord {
        unsigned.author_did = author.clone();
        unsigned.confidence = Confidence::from_basis_points(2500);
        ClaimRecord::SelfAttested(SelfAttestedClaim::new(unsigned).expect("canonical"))
    }

    proptest! {
        #[test]
        fn the_mode_follows_the_adr_071_table(
            repo in arb_did(), other in arb_did(), sig in any::<bool>(),
            shape in 0u8..3, origin in arb_origin(), cid_ok in any::<bool>()
        ) {
            prop_assume!(repo != other);
            let author = match shape {
                0 => Did(format!("{}#org.openlore.application", repo.0)),
                1 => repo.clone(),
                _ => other,
            };
            prop_assert_eq!(
                provenance_mode(sig, &author, &repo, origin, cid_ok),
                table(sig, &author, &repo, origin, cid_ok)
            );
        }

        /// A self-attested record in its author's repo, fetched from the
        /// author's PDS under its own CID, is admitted; moved to another
        /// repo, fetched from a relay, or keyed by anything else, it is not.
        #[test]
        fn a_self_attested_record_is_admitted_only_in_its_own_repo_from_its_own_pds(
            author in arb_did(), other in arb_did(), unsigned in arb_unsigned_claim()
        ) {
            prop_assume!(author != other);
            let record = self_attested_by(&author, unsigned);
            let rkey = record.cid().0.clone();
            prop_assert_eq!(
                provenance_verdict(&record, &rkey, &author, RecordOrigin::AuthorPds, None),
                Ok(Provenance::SelfAttested { repo_did: author.clone() })
            );
            prop_assert_eq!(
                provenance_verdict(&record, &rkey, &other, RecordOrigin::AuthorPds, None),
                Err(ProvenanceRejection::ForeignRepo)
            );
            prop_assert_eq!(
                provenance_verdict(&record, &rkey, &author, RecordOrigin::Relay, None),
                Err(ProvenanceRejection::UnverifiableProvenance)
            );
            prop_assert_eq!(
                provenance_verdict(&record, &format!("{rkey}x"), &author, RecordOrigin::AuthorPds, None),
                Err(ProvenanceRejection::IntegrityFailure)
            );
        }

        /// Stripping the signature off an app-signed record (author keeps its
        /// `#fragment`) never yields a self-attested claim.
        #[test]
        fn a_signature_stripped_app_record_is_malformed_not_self_attested(
            repo in arb_did(), unsigned in arb_unsigned_claim()
        ) {
            let mut unsigned = unsigned;
            unsigned.author_did = Did(format!("{}#org.openlore.application", repo.0));
            unsigned.confidence = Confidence::from_basis_points(5000);
            let record = ClaimRecord::SelfAttested(SelfAttestedClaim::new(unsigned).expect("canonical"));
            let rkey = record.cid().0.clone();
            prop_assert_eq!(
                provenance_verdict(&record, &rkey, &repo, RecordOrigin::AuthorPds, None),
                Err(ProvenanceRejection::MalformedProvenance)
            );
        }
    }

    // bypass: one fixed Ed25519 key pair is enough to show the AppSigned arm
    // delegates to the unchanged `verify` (accept own key, refuse no key).
    #[test]
    fn the_app_signed_arm_runs_the_unchanged_verify() {
        let seed = [7u8; 32];
        let key = VerifyingKey(
            ed25519_dalek::SigningKey::from_bytes(&seed)
                .verifying_key()
                .to_bytes()
                .to_vec(),
        );
        let repo = Did("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        let unsigned = UnsignedClaim {
            subject: "github:a/b".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.test-driven".into(),
            evidence: vec!["https://github.com/a/b".into()],
            confidence: Confidence::from_basis_points(2500),
            author_did: Did(format!("{}#org.openlore.application", repo.0)),
            composed_at: "2026-10-04T15:02:11Z".into(),
            references: vec![],
            reason: None,
        };
        let cid = compute_cid(&canonicalize(&unsigned).expect("canonical"));
        let signature = sign(&cid, &crate::SigningKey(seed.to_vec())).expect("sign");
        let rkey = cid.0.clone();
        let record = ClaimRecord::AppSigned(SignedClaim {
            unsigned,
            signature,
        });
        assert!(matches!(
            provenance_verdict(&record, &rkey, &repo, RecordOrigin::Relay, Some(&key)),
            Ok(Provenance::AppSigned { .. })
        ));
        assert_eq!(
            provenance_verdict(&record, &rkey, &repo, RecordOrigin::AuthorPds, None),
            Err(ProvenanceRejection::NoAppKey)
        );
    }
}
