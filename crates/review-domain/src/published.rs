//! The owner's published claims, read live from their own PDS (US-BRA-007,
//! US-BRA-011): only records whose ADR-071 provenance verdict is
//! self-attested by the owner, minus retractions and what they retract.
//! Pure: the shell lists the repo and computes the record origin.

use ports::claim_domain::identity_link::is_reserved_predicate;
use ports::claim_domain::{
    decode_claim_record, is_self_retracted, provenance_verdict, ClaimLineage, Did, Provenance,
    RecordOrigin, ReferenceType, UnsignedClaim,
};
use ports::RepoRecord;

use crate::lifecycle::BASIS_POINTS;

/// Who a `/@{segment}` profile URL names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSubject {
    Handle(String),
    Did(String),
}

/// The subject of a profile URL segment: a DID (`did:plc:` / `did:web:`)
/// or a well-formed handle. Anything else names nobody.
pub fn profile_subject(segment: &str) -> Option<ProfileSubject> {
    let did_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_');
    let is_did = ["did:plc:", "did:web:"]
        .iter()
        .any(|method| segment.len() > method.len() && segment.starts_with(method))
        && segment.chars().all(did_char);
    if is_did {
        Some(ProfileSubject::Did(segment.to_string()))
    } else {
        crate::signin::parse_handle(segment).map(|h| ProfileSubject::Handle(h.as_str().to_string()))
    }
}

/// One published claim as the profile shows it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublishedClaim {
    pub subject: String,
    pub object: String,
    pub confidence_bp: u16,
    pub rkey: String,
}

/// The claims a profile shows (pure): records of the owner's own repo whose
/// ADR-071 verdict is self-attested (author is the repo DID, rkey is the
/// claim's CID, fetched from the owner's own PDS), minus self-retracted
/// claims, the retraction records themselves and identity-link records
/// (U-27: links are hidden from the profile and so from the share draft),
/// in a deterministic order.
pub fn published_claims(
    owner_did: &str,
    records: &[RepoRecord],
    origin: RecordOrigin,
) -> Vec<PublishedClaim> {
    let mut shown: Vec<PublishedClaim> = live_claims(owner_did, records, origin)
        .iter()
        .filter(|(_, claim)| !is_reserved_predicate(&claim.predicate))
        .map(|(rkey, claim)| shown_claim(rkey, claim))
        .collect();
    shown.sort();
    shown
}

/// The owner's live published claim under `rkey` — the only claim a
/// retraction may be built for (US-BRA-011), identity links included (03-09). `None` once it is retracted,
/// or if it is not the owner's own self-attested claim.
pub fn live_published_claim(
    owner_did: &str,
    records: &[RepoRecord],
    origin: RecordOrigin,
    rkey: &str,
) -> Option<UnsignedClaim> {
    live_claims(owner_did, records, origin)
        .into_iter()
        .find_map(|(key, claim)| (key == rkey).then_some(claim))
}

/// `(rkey, claim)` of the owner's self-attested claims that are neither
/// retractions nor self-retracted.
fn live_claims<'r>(
    owner_did: &str,
    records: &'r [RepoRecord],
    origin: RecordOrigin,
) -> Vec<(&'r str, UnsignedClaim)> {
    let owner = Did(owner_did.to_string());
    let accepted: Vec<(&str, UnsignedClaim)> = records
        .iter()
        .filter(|record| record.repo_did == owner_did)
        .filter_map(|record| self_attested(record, &owner, origin))
        .collect();
    let retracted: Vec<bool> = {
        let lineages: Vec<ClaimLineage<'_>> = accepted
            .iter()
            .map(|(rkey, claim)| lineage(rkey, claim))
            .collect();
        accepted
            .iter()
            .map(|(rkey, claim)| {
                is_retraction(claim) || is_self_retracted(&lineage(rkey, claim), &lineages)
            })
            .collect()
    };
    accepted
        .into_iter()
        .zip(retracted)
        .filter_map(|(claim, retracted)| (!retracted).then_some(claim))
        .collect()
}

/// The record's claim, if its provenance verdict is self-attested by `owner`.
pub(crate) fn self_attested<'r>(
    record: &'r RepoRecord,
    owner: &Did,
    origin: RecordOrigin,
) -> Option<(&'r str, UnsignedClaim)> {
    let claim = decode_claim_record(&record.value, "").ok()?;
    match provenance_verdict(&claim, &record.rkey, owner, origin, None) {
        Ok(Provenance::SelfAttested { .. }) => Some((&record.rkey, claim.unsigned().clone())),
        _ => None,
    }
}

fn lineage<'c>(rkey: &'c str, claim: &'c UnsignedClaim) -> ClaimLineage<'c> {
    ClaimLineage {
        author_did: &claim.author_did.0,
        cid: rkey,
        references: &claim.references,
    }
}

fn is_retraction(claim: &UnsignedClaim) -> bool {
    claim
        .references
        .iter()
        .any(|reference| reference.ref_type == ReferenceType::Retracts)
}

fn shown_claim(rkey: &str, claim: &UnsignedClaim) -> PublishedClaim {
    let basis_points = claim
        .confidence
        .basis_points()
        .clamp(0, i64::from(BASIS_POINTS));
    PublishedClaim {
        subject: claim.subject.clone(),
        object: claim.object.clone(),
        confidence_bp: u16::try_from(basis_points).unwrap_or_default(),
        rkey: rkey.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const OWNER: &str = "did:plc:owner";

    /// How one generated record departs (or not) from a self-attested claim
    /// of the owner's own repo.
    #[derive(Debug, Clone, Copy)]
    struct RecordShape {
        in_owner_repo: bool,
        author: u8,
        signed: bool,
        rkey_is_cid: bool,
        confidence_bp: u16,
    }

    fn record_shape() -> impl Strategy<Value = RecordShape> {
        (
            any::<bool>(),
            0u8..3,
            any::<bool>(),
            any::<bool>(),
            0u16..=10_000,
        )
            .prop_map(
                |(in_owner_repo, author, signed, rkey_is_cid, confidence_bp)| RecordShape {
                    in_owner_repo,
                    author,
                    signed,
                    rkey_is_cid,
                    confidence_bp,
                },
            )
    }

    fn claim_body(index: usize, shape: RecordShape, retracts: Option<&str>) -> serde_json::Value {
        let author = match shape.author {
            0 => OWNER.to_string(),
            1 => "did:plc:someone-else".to_string(),
            _ => format!("{OWNER}#org.openlore.application"),
        };
        let references: Vec<serde_json::Value> = retracts
            .map(|cid| serde_json::json!({"type": "retracts", "cid": cid}))
            .into_iter()
            .collect();
        let mut body = serde_json::json!({
            "subject": format!("github:priyaraman/repo{index}"),
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.memory-safety",
            "evidence": [],
            "confidence": shape.confidence_bp,
            "author": author,
            "composedAt": "2026-10-04T15:02:11Z",
            "references": references,
        });
        if shape.signed {
            body["signature"] = serde_json::json!({"kid": format!("{OWNER}#org.openlore.application"), "alg": "EdDSA", "sig": "AAAA"});
        }
        body
    }

    fn cid_of(body: &serde_json::Value) -> String {
        decode_claim_record(body, "")
            .expect("decodable")
            .cid()
            .0
            .clone()
    }

    fn repo_record(body: serde_json::Value, shape: RecordShape) -> RepoRecord {
        let cid = cid_of(&body);
        RepoRecord {
            repo_did: if shape.in_owner_repo {
                OWNER.to_string()
            } else {
                "did:plc:elsewhere".to_string()
            },
            rkey: if shape.rkey_is_cid {
                cid
            } else {
                format!("{cid}x")
            },
            value: body,
        }
    }

    fn admissible(shape: RecordShape) -> bool {
        shape.in_owner_repo && shape.author == 0 && !shape.signed && shape.rkey_is_cid
    }

    proptest! {
        /// Universe: up to 8 records varying in repo, author, signature, rkey
        /// and confidence, read from the owner's PDS or elsewhere, with an
        /// optional owner retraction of one of them. Exactly the
        /// self-attested claims of the owner's own repo, fetched from the
        /// owner's PDS and not retracted, appear, sorted, whatever the
        /// listing order.
        #[test]
        fn only_verdict_accepted_unretracted_claims_of_the_owner_appear(
            shapes in proptest::collection::vec(record_shape(), 0..8),
            from_author_pds in any::<bool>(),
            retracted in proptest::option::of(0usize..8),
        ) {
            let bodies: Vec<serde_json::Value> =
                shapes.iter().enumerate().map(|(i, s)| claim_body(i, *s, None)).collect();
            let mut records: Vec<RepoRecord> =
                bodies.iter().zip(&shapes).map(|(b, s)| repo_record(b.clone(), *s)).collect();
            let retracted = retracted.filter(|i| *i < shapes.len());
            let marker_shape = RecordShape { in_owner_repo: true, author: 0, signed: false, rkey_is_cid: true, confidence_bp: 0 };
            if let Some(i) = retracted {
                let original_cid = records[i].rkey.clone();
                records.push(repo_record(claim_body(100, marker_shape, Some(&original_cid)), marker_shape));
            }
            let origin = if from_author_pds { RecordOrigin::AuthorPds } else { RecordOrigin::Relay };

            let mut expected: Vec<PublishedClaim> = shapes
                .iter()
                .enumerate()
                .filter(|(i, s)| from_author_pds && admissible(**s) && Some(*i) != retracted)
                .map(|(i, s)| PublishedClaim {
                    subject: format!("github:priyaraman/repo{i}"),
                    object: "org.openlore.philosophy.memory-safety".to_string(),
                    confidence_bp: s.confidence_bp,
                    rkey: records[i].rkey.clone(),
                })
                .collect();
            expected.sort();

            prop_assert_eq!(&published_claims(OWNER, &records, origin), &expected);
            records.reverse();
            prop_assert_eq!(&published_claims(OWNER, &records, origin), &expected);
        }

        /// Universe: arbitrary URL segments. A segment names a DID only with
        /// a `did:plc:`/`did:web:` method; otherwise it is a handle or nobody.
        #[test]
        fn a_profile_segment_names_a_did_a_handle_or_nobody(segment in "(did:(plc|web|key):)?[a-z0-9.:-]{0,24}") {
            match profile_subject(&segment) {
                Some(ProfileSubject::Did(did)) => {
                    prop_assert!(did.starts_with("did:plc:") || did.starts_with("did:web:"));
                }
                Some(ProfileSubject::Handle(handle)) => prop_assert!(handle.contains('.')),
                None => {}
            }
        }
    }

    /// Universe for the link properties: the owner's own repo read from the
    /// owner's PDS, holding ordinary claims (one per philosophy slot, some
    /// under a case variant of the reserved predicate) interleaved with the
    /// owner's link records, built by the production link encoder.
    const PHILOSOPHIES: [&str; 3] = ["memory-safety", "test-first", "small-commits"];
    const SHARE_PROFILE_URL: &str = "https://review.example/@priya.example";

    fn arb_account() -> impl Strategy<Value = (String, u64)> {
        ("[a-z][a-z0-9-]{0,15}", 1u64..=u64::from(u32::MAX))
    }

    /// The owner's link record for the GitHub `account`, from the production
    /// encoder (the bio names the owner's DID, so the link is proven).
    fn link_record((login, user_id): &(String, u64)) -> RepoRecord {
        let observation = ports::claim_domain::identity_link::GithubObservation {
            login: login.clone(),
            user_id: *user_id,
            bio: Some(format!("my DID: {OWNER}")),
        };
        let body = ports::claim_domain::identity_link::self_attested_link_record(
            OWNER,
            &observation,
            "2026-10-17T09:30:00Z",
        )
        .expect("the bio names the DID, so the link is proven");
        repo_record(body, owner_shape(10_000))
    }

    fn owner_shape(confidence_bp: u16) -> RecordShape {
        RecordShape {
            in_owner_repo: true,
            author: 0,
            signed: false,
            rkey_is_cid: true,
            confidence_bp,
        }
    }

    /// An ordinary claim of the owner naming `PHILOSOPHIES[philosophy]`,
    /// under the reserved predicate's upper-cased spelling when `case_variant`.
    fn ordinary_record(
        index: usize,
        philosophy: usize,
        case_variant: bool,
        confidence_bp: u16,
    ) -> RepoRecord {
        let mut body = claim_body(index, owner_shape(confidence_bp), None);
        body["object"] = serde_json::json!(format!(
            "org.openlore.philosophy.{}",
            PHILOSOPHIES[philosophy]
        ));
        if case_variant {
            body["predicate"] = serde_json::json!(
                ports::claim_domain::identity_link::link_predicate().to_uppercase()
            );
        }
        repo_record(body, owner_shape(confidence_bp))
    }

    fn arb_ordinary() -> impl Strategy<Value = (usize, bool, u16)> {
        (0usize..PHILOSOPHIES.len(), any::<bool>(), 0u16..=10_000)
    }

    proptest! {
        /// For any mix of ordinary claims and link records, the profile list
        /// is the list of the ordinary claims alone, and the share draft
        /// built from it never names a linked account yet names every
        /// ordinary philosophy (U-27).
        #[test]
        fn a_link_record_never_reaches_the_profile_list_or_the_share_draft(
            ordinary in proptest::collection::vec(arb_ordinary(), 0..5),
            accounts in proptest::collection::vec(arb_account(), 1..4),
            link_slots in proptest::collection::vec(0usize..6, 1..4),
        ) {
            let mut records: Vec<RepoRecord> = ordinary
                .iter()
                .enumerate()
                .map(|(i, (philosophy, variant, bp))| ordinary_record(i, *philosophy, *variant, *bp))
                .collect();
            let mut expected: Vec<PublishedClaim> = ordinary
                .iter()
                .zip(&records)
                .enumerate()
                .map(|(i, ((philosophy, _, bp), record))| PublishedClaim {
                    subject: format!("github:priyaraman/repo{i}"),
                    object: format!("org.openlore.philosophy.{}", PHILOSOPHIES[*philosophy]),
                    confidence_bp: *bp,
                    rkey: record.rkey.clone(),
                })
                .collect();
            expected.sort();
            for (account, slot) in accounts.iter().zip(link_slots.iter().cycle()) {
                records.insert((*slot).min(records.len()), link_record(account));
            }

            let published = published_claims(OWNER, &records, RecordOrigin::AuthorPds);
            prop_assert_eq!(&published, &expected);

            let draft = match crate::share::share_post_plan(SHARE_PROFILE_URL, &published) {
                Ok(plan) => plan.draft().text().to_string(),
                Err(_) => {
                    prop_assert!(ordinary.is_empty(), "only an empty profile has nothing to share");
                    crate::share::share_draft(SHARE_PROFILE_URL, &published)
                        .expect("the bare draft")
                        .draft()
                        .text()
                        .to_string()
                }
            };
            for (login, _) in &accounts {
                let linked = format!("github:{login}");
                prop_assert!(!draft.contains(&linked), "draft {:?} names {}", draft, linked);
            }
            for (philosophy, _, _) in &ordinary {
                prop_assert!(draft.contains(PHILOSOPHIES[*philosophy]), "draft {:?} omits {}", draft, PHILOSOPHIES[*philosophy]);
            }
        }

        /// A live link record stays reachable under its own key, so /retract
        /// can retract it (03-09), whatever ordinary claims surround it.
        #[test]
        fn a_live_link_record_can_still_be_retracted(
            ordinary in proptest::collection::vec(arb_ordinary(), 0..4),
            account in arb_account(),
            slot in 0usize..5,
        ) {
            let mut records: Vec<RepoRecord> = ordinary
                .iter()
                .enumerate()
                .map(|(i, (philosophy, variant, bp))| ordinary_record(i, *philosophy, *variant, *bp))
                .collect();
            let link = link_record(&account);
            let rkey = link.rkey.clone();
            records.insert(slot.min(records.len()), link);

            let live = live_published_claim(OWNER, &records, RecordOrigin::AuthorPds, &rkey);
            let live = live.expect("the live link is found under its key");
            prop_assert_eq!(&live.subject, &OWNER.to_string());
            prop_assert_eq!(&live.object, &format!("github:{}", account.0));
            prop_assert!(ports::claim_domain::identity_link::is_reserved_predicate(&live.predicate));
        }
    }
}
