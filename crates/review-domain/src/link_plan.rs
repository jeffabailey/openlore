//! Link plans (slice 3, "Publish this link"; ADR-084, U-21 B): the owner's
//! self-attested identity-link record, previewed before it is written and
//! kept between preview and confirm under kind `publish`.
//!
//! A [`LinkPlan`] is a type apart from [`crate::plans::PublishPlan`]: no
//! claim-publish code can receive one (F-16). It is built only from a
//! [`ProvenLink`] through `LinkDraft` — the link's one encoder — and a
//! stored plan restores only as the owner's own projecting link under its
//! own key.

use ports::claim_domain::identity_link::{project, ClaimFields, LinkDraft, ProvenLink};
use ports::claim_domain::{
    bare_did, claim_record_json, Did, Provenance, RecordOrigin, SelfAttestedClaim, UnsignedClaim,
};
use ports::{PlanKind, RepoRecord, StoredPublishPlan, SuggestionKey};
use serde_json::Value;

use crate::plans::{at_uri, PlanError};
use crate::published::self_attested;

/// The exact link record a confirm will create in the owner's own repo.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkPlan {
    owner_did: String,
    link: SelfAttestedClaim,
}

impl LinkPlan {
    pub fn owner_did(&self) -> &str {
        &self.owner_did
    }

    pub fn claim(&self) -> &UnsignedClaim {
        self.link.unsigned()
    }

    /// The plan's key: the record's own triple (F-03), the rule
    /// `RetractPlan::key` uses.
    pub fn key(&self) -> SuggestionKey {
        triple_of(self.claim())
    }

    /// The record key: the link claim's canonical CID.
    pub fn rkey(&self) -> &str {
        &self.link.cid().0
    }

    /// The exact record JSON to be written (lexicon keys only).
    pub fn record(&self) -> Value {
        claim_record_json(self.claim())
    }

    /// Where the record will live: `at://<did>/org.openlore.claim/<cid>`.
    pub fn at_uri(&self) -> String {
        at_uri(&self.owner_did, self.rkey())
    }

    /// The plan as kept between preview and confirm: kind `publish` (U-21 B).
    pub fn stored(&self) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: self.rkey().to_string(),
            kind: PlanKind::Publish,
            key: self.key(),
            record_json: self.record().to_string(),
        }
    }
}

fn triple_of(claim: &UnsignedClaim) -> SuggestionKey {
    SuggestionKey {
        subject: claim.subject.clone(),
        predicate: claim.predicate.clone(),
        object: claim.object.clone(),
    }
}

/// Build the plan publishing `owner_did`'s proven link, composed at
/// `composed_at` (RFC 3339 UTC). Pure: nothing is written. A link proven for
/// another DID is never this owner's plan.
pub fn link_plan(
    owner_did: &str,
    proven: &ProvenLink,
    composed_at: &str,
) -> Result<LinkPlan, PlanError> {
    let link = LinkDraft::new(proven, composed_at).into_unsigned();
    if link.author_did.0 != owner_did {
        return Err(PlanError::NotTheOwnersRecord);
    }
    Ok(LinkPlan {
        owner_did: owner_did.to_string(),
        link: SelfAttestedClaim::new(link)?,
    })
}

/// Restore a stored link plan for `owner_did`, admitting it only if it is a
/// `publish` plan whose record is self-attested by the owner under its
/// recomputed CID (the plan id), references nothing, projects as a link of
/// the owner's bare DID, and is stored under its own triple.
pub fn restore_link_plan(
    owner_did: &str,
    stored: &StoredPublishPlan,
) -> Result<LinkPlan, PlanError> {
    let link = owners_link(owner_did, stored).ok_or(PlanError::NotTheOwnersRecord)?;
    Ok(LinkPlan {
        owner_did: owner_did.to_string(),
        link: SelfAttestedClaim::new(link)?,
    })
}

/// The stored plan's record, if it is the owner's own link under its own key.
fn owners_link(owner_did: &str, stored: &StoredPublishPlan) -> Option<UnsignedClaim> {
    let record = RepoRecord {
        repo_did: owner_did.to_string(),
        rkey: stored.plan_id.clone(),
        value: serde_json::from_str(&stored.record_json).ok()?,
    };
    let owner = Did(owner_did.to_string());
    let (_, claim) = self_attested(&record, &owner, RecordOrigin::AuthorPds)?;
    let provenance = Provenance::SelfAttested { repo_did: owner };
    let projected = project(claim_fields(&claim), &provenance)?;
    let admitted = stored.kind == PlanKind::Publish
        && claim.references.is_empty()
        && projected.did == bare_did(owner_did)
        && stored.key == triple_of(&claim);
    admitted.then_some(claim)
}

fn claim_fields(claim: &UnsignedClaim) -> ClaimFields<'_> {
    ClaimFields {
        author: &claim.author_did.0,
        subject: &claim.subject,
        predicate: &claim.predicate,
        object: &claim.object,
        evidence: &claim.evidence,
    }
}

#[cfg(test)]
mod tests {
    //! Universe: owner DIDs × GitHub accounts (login, positive id) ×
    //! composition times, and the stored plans a confirm may be handed: the
    //! owner's link plan, its relabellings, its tamperings, another DID's
    //! link plan, an ordinary publish plan, a retraction and a share draft.
    use super::*;
    use crate::plans::{publish_plan, restore_plan, retract_plan, ClaimDraft};
    use ports::claim_domain::identity_link::{
        check_link, link_predicate, self_attested_link_record, GithubObservation, LinkDraft,
        ProofCheck,
    };
    use ports::claim_domain::{claim_record_json, decode_claim_record, Did};
    use ports::PlanKind;
    use proptest::prelude::*;

    const COMPOSED_AT: &str = "2026-10-17T09:30:00Z";

    fn arb_did() -> impl Strategy<Value = String> {
        "[a-z2-7]{24}".prop_map(|s| format!("did:plc:{s}"))
    }

    fn arb_account() -> impl Strategy<Value = (String, u64)> {
        ("[a-z][a-z0-9-]{0,15}", 1u64..=u64::from(u32::MAX))
    }

    /// The observation proving `did`'s link: its bio names the DID.
    fn proving_observation(did: &str, (login, user_id): &(String, u64)) -> GithubObservation {
        GithubObservation {
            login: login.clone(),
            user_id: *user_id,
            bio: Some(format!("my DID: {did}")),
        }
    }

    fn proven(did: &str, account: &(String, u64)) -> ProvenLink {
        match check_link(&Did(did.to_string()), &proving_observation(did, account)) {
            Ok(ProofCheck::Verified(proven)) => proven,
            other => panic!("the bio names the DID, so the link is proven: {other:?}"),
        }
    }

    /// `record` stored under its own recomputed CID with `key`.
    fn stored_under_own_cid(record: &Value, key: SuggestionKey) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: decode_claim_record(record, "")
                .expect("decodable")
                .cid()
                .0
                .clone(),
            kind: PlanKind::Publish,
            key,
            record_json: record.to_string(),
        }
    }

    proptest! {
        #[test]
        fn a_link_plan_restores_only_as_the_owners_link_under_its_own_key(
            did in arb_did(), other in arb_did(), account in arb_account(),
        ) {
            prop_assume!(did != other);
            let (login, user_id) = account.clone();
            let plan = link_plan(&did, &proven(&did, &account), COMPOSED_AT).expect("link plan");
            let stored = plan.stored();
            let triple = SuggestionKey {
                subject: did.clone(),
                predicate: link_predicate().to_string(),
                object: format!("github:{login}"),
            };

            // F-03 / U-21 B: kind publish, keyed by the record's own triple.
            prop_assert_eq!(stored.kind, PlanKind::Publish);
            prop_assert_eq!(&stored.key, &triple);
            prop_assert_eq!(&stored.plan_id, &plan.rkey().to_string());
            prop_assert_eq!(&restore_link_plan(&did, &stored), &Ok(plan.clone()));

            // Never another owner's, never under another label.
            prop_assert!(restore_link_plan(&other, &stored).is_err());
            for label in [PlanKind::Retract, PlanKind::Share] {
                let relabelled = StoredPublishPlan { kind: label, ..stored.clone() };
                prop_assert!(restore_link_plan(&did, &relabelled).is_err(), "labelled {:?}", label);
            }

            // A proven link of another DID is never this owner's plan.
            prop_assert!(link_plan(&did, &proven(&other, &account), COMPOSED_AT).is_err());
            let others = link_plan(&other, &proven(&other, &account), COMPOSED_AT).expect("other's plan");
            prop_assert!(restore_link_plan(&did, &others.stored()).is_err());

            // An ordinary claim publish is not a link.
            let ordinary = publish_plan(&did, &ClaimDraft {
                key: SuggestionKey {
                    subject: format!("github:{login}/repo"),
                    predicate: "embodiesPhilosophy".into(),
                    object: "org.openlore.philosophy.memory-safety".into(),
                },
                evidence: vec![format!("https://github.com/{login}/repo")],
                confidence_bp: 8_000,
            }, COMPOSED_AT).expect("ordinary plan");
            prop_assert!(restore_link_plan(&did, &ordinary.stored()).is_err());

            // The link's retraction, under either label, is not a link publish.
            let retraction = retract_plan(&did, plan.rkey(), plan.claim(), "2026-10-17T10:00:00Z")
                .expect("retraction")
                .stored();
            prop_assert!(restore_link_plan(&did, &retraction).is_err());
            prop_assert!(restore_link_plan(&did, &StoredPublishPlan { kind: PlanKind::Publish, ..retraction }).is_err(), "refused");

            // A share draft is not a link publish.
            let share = crate::share::share_post_plan(
                "https://review.example/@priya.example",
                &[crate::views::PublishedClaim {
                    subject: ordinary.key().subject.clone(),
                    object: ordinary.key().object.clone(),
                    confidence_bp: 8_000,
                    rkey: ordinary.rkey().to_string(),
                }],
            )
            .expect("share plan");
            prop_assert!(restore_link_plan(&did, &share.stored(&did, "random-plan-id")).is_err());

            // Edited evidence: under the stale plan id, and re-keyed so the
            // record no longer projects (no id URI) or names another account.
            let id_uri = format!("https://api.github.com/user/{user_id}");
            let edited_id = plan.record().to_string().replace(&id_uri, &format!("{id_uri}0"));
            prop_assert!(restore_link_plan(&did, &StoredPublishPlan { record_json: edited_id, ..stored.clone() }).is_err(), "refused");
            let mut without_id = plan.record();
            without_id["evidence"] = serde_json::json!([format!("https://github.com/{login}")]);
            prop_assert!(restore_link_plan(&did, &stored_under_own_cid(&without_id, triple.clone())).is_err());
            let mut user_zero = plan.record();
            user_zero["evidence"][1] = serde_json::json!("https://api.github.com/user/0");
            prop_assert!(restore_link_plan(&did, &stored_under_own_cid(&user_zero, triple.clone())).is_err());

            // Edited key: the stored key must be the record's own triple.
            for edited_key in [
                SuggestionKey { object: format!("github:{login}x"), ..triple.clone() },
                SuggestionKey { subject: other.clone(), ..triple.clone() },
                SuggestionKey { predicate: "embodiesPhilosophy".into(), ..triple.clone() },
            ] {
                prop_assert!(restore_link_plan(&did, &StoredPublishPlan { key: edited_key, ..stored.clone() }).is_err(), "refused");
            }

            // The link plan is never a claim publish (F-02).
            prop_assert!(restore_plan(&did, &stored).is_err());
        }

        /// One encoder, no copy: the plan's record is exactly the record
        /// `LinkDraft` composes and the review app's create-only write uses.
        #[test]
        fn a_link_plans_record_is_the_link_drafts_record(
            did in arb_did(), account in arb_account(), secs in 0i64..4_102_444_800,
        ) {
            let composed_at = crate::plans::rfc3339_utc(secs);
            let proven = proven(&did, &account);
            let plan = link_plan(&did, &proven, &composed_at).expect("link plan");
            prop_assert_eq!(
                Some(plan.record()),
                self_attested_link_record(&did, &proving_observation(&did, &account), &composed_at)
            );
            prop_assert_eq!(plan.record(), claim_record_json(LinkDraft::new(&proven, &composed_at).claim()));
            prop_assert_eq!(plan.at_uri(), crate::plans::at_uri(&did, plan.rkey()));
        }
    }
}
