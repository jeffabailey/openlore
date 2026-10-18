//! Link plans (slice 3, "Publish this link"; ADR-084, U-21 B): the owner's
//! self-attested identity-link record, previewed before it is written and
//! kept between preview and confirm under kind `publish`.
//!
//! A [`LinkPlan`] is a type apart from [`crate::plans::PublishPlan`]: no
//! claim-publish code can receive one (F-16). It is built only from a
//! [`ProvenLink`] through `LinkDraft` — the link's one encoder — and a
//! stored plan restores only as the owner's own projecting link under its
//! own key.

use ports::claim_domain::identity_link::{
    check_link, project, CheckVerdict, ClaimFields, GithubObservation, LinkDraft,
    MalformedObservation, ProofCheck, ProvenLink,
};
use ports::claim_domain::{
    bare_did, claim_record_json, Did, Provenance, RecordOrigin, SelfAttestedClaim, UnsignedClaim,
};
use ports::{GithubError, PlanKind, RepoRecord, StoredPublishPlan, SuggestionKey};
use serde_json::Value;

use crate::ownership::{prove_ownership, GithubAccount, OwnershipRefusal};
use crate::plans::{at_uri, triple_of, PlanError};
use crate::published::self_attested;
use crate::views::LinkNotPublished;

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

    /// The GitHub login the link names (`github:<login>`): the account a
    /// confirm reads again.
    pub fn login(&self) -> &str {
        self.claim()
            .object
            .strip_prefix("github:")
            .unwrap_or_default()
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

/// Why the link preview offers no plan to confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkPreviewRefusal {
    /// The fresh read does not prove the link, GitHub could not be read, or
    /// it was not asked: the GitHub step explains it as Verify does.
    Ownership(OwnershipRefusal),
    /// The fresh read proved the link, but no plan could be built or kept
    /// for the confirm (U-37). Nothing was published; GitHub is not to blame.
    NotSaved,
}

impl From<OwnershipRefusal> for LinkPreviewRefusal {
    fn from(refusal: OwnershipRefusal) -> Self {
        Self::Ownership(refusal)
    }
}

/// The link preview (RA-1a, RA-11): the plan publishing `owner_did`'s link
/// to the account `observation` read just now, or why that read does not
/// prove it. The proof is this fresh read, never a stored verification
/// (C-GDL-10). Once `prove_ownership` passes, the link check and the plan
/// cannot disagree with it; if they ever did, no plan is offered, as
/// [`LinkPreviewRefusal::NotSaved`]. Pure: nothing is written.
pub fn link_preview(
    owner_did: &str,
    observation: &GithubObservation,
    composed_at: &str,
) -> Result<LinkPlan, LinkPreviewRefusal> {
    prove_ownership(owner_did, &account_observed(observation), None)?;
    let Ok(ProofCheck::Verified(proven)) = check_link(&Did(owner_did.to_string()), observation)
    else {
        return Err(LinkPreviewRefusal::NotSaved);
    };
    link_plan(owner_did, &proven, composed_at).map_err(|_| LinkPreviewRefusal::NotSaved)
}

/// Why a link confirm writes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkConfirmRefusal {
    /// The confirm-time read no longer proves the plan's DID.
    ProofGone(CheckVerdict),
    /// GitHub's answer named no account: it proves nothing (fail closed).
    Unreadable,
    /// The confirm-time proof names another DID, login or account than the
    /// previewed plan.
    IdentityChanged,
}

impl LinkConfirmRefusal {
    /// The operator-facing name (logs only).
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProofGone(_) => "proof_gone",
            Self::Unreadable => "unreadable",
            Self::IdentityChanged => "identity_changed",
        }
    }

    /// Why the owner is told the link is not published: a disproved fresh
    /// read says the proof is gone (RA-2); an answer naming no account is
    /// GitHub being unavailable (fail closed); a proof of another identity
    /// says the read did not prove the link.
    pub fn not_published(&self) -> LinkNotPublished {
        match self {
            Self::ProofGone(_) => LinkNotPublished::ProofGone,
            Self::Unreadable => LinkNotPublished::Refused(LinkRefusal::Ownership(
                OwnershipRefusal::GithubUnavailable,
            )),
            Self::IdentityChanged => LinkNotPublished::NotProven,
        }
    }
}

/// Why a link confirm could not read GitHub, or was not allowed to (F-10,
/// U-31): the verify step's refusals, plus a rejected app token. The token
/// itself is never carried (DD-GDL-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRefusal {
    Ownership(OwnershipRefusal),
    /// GitHub did not accept the app's access token.
    TokenRejected,
}

impl LinkRefusal {
    /// The operator-facing name (logs only); never carries the token.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TokenRejected => "token_rejected",
            Self::Ownership(ownership) => ownership.label(),
        }
    }
}

/// This hour's GitHub check budget is spent: the confirm reads nothing.
pub const BUDGET_SPENT: LinkRefusal = LinkRefusal::Ownership(OwnershipRefusal::TooManyAttempts);

/// The refusal a failed confirm-time GitHub read stands for. A rate limit, a
/// rejected token or an outage proves nothing either way; a missing or
/// private account is not found.
pub fn link_refusal_of(error: &GithubError) -> LinkRefusal {
    match error {
        GithubError::RateLimited { .. } => LinkRefusal::Ownership(OwnershipRefusal::RateLimited),
        GithubError::TokenRejected => LinkRefusal::TokenRejected,
        GithubError::NotFound { .. } | GithubError::NotPublic { .. } => {
            LinkRefusal::Ownership(OwnershipRefusal::AccountNotFound)
        }
        GithubError::Network(_)
        | GithubError::ApiShape(_)
        | GithubError::ContributorsUnavailable { .. } => {
            LinkRefusal::Ownership(OwnershipRefusal::GithubUnavailable)
        }
    }
}

/// The link confirm (RA-1b, RR-2a, C-GDL-10): admit `plan` only when the
/// confirm-time check `fresh` proves exactly the plan's DID, login and
/// account. The decision is the confirm-time read alone; a stored
/// verification is never consulted. Pure: nothing is written.
pub fn confirm_link(
    plan: LinkPlan,
    fresh: Result<ProofCheck, MalformedObservation>,
) -> Result<LinkPlan, LinkConfirmRefusal> {
    match fresh {
        Ok(ProofCheck::Verified(proven)) if proves_the_plan(&plan, &proven) => Ok(plan),
        Ok(ProofCheck::Verified(_)) => Err(LinkConfirmRefusal::IdentityChanged),
        Ok(ProofCheck::Disproved { verdict, .. }) => Err(LinkConfirmRefusal::ProofGone(verdict)),
        Err(MalformedObservation) => Err(LinkConfirmRefusal::Unreadable),
    }
}

/// The fresh proof composes exactly the plan's link: the same DID, login and
/// account (the one encoder, at the plan's own composition time).
fn proves_the_plan(plan: &LinkPlan, proven: &ProvenLink) -> bool {
    LinkDraft::new(proven, &plan.claim().composed_at).claim() == plan.claim()
}

/// Whether a link confirm that ended `why` puts its plan back, unchanged,
/// for one Retry (RA-12). Only a write the repo host may have stored
/// (U-42/U-43: a server error or a lost answer): the record key is the
/// link's CID, so a Retry of a create that did land reads "already exists"
/// as success (ADR-071, U-34). A definite refusal, a refusal before the
/// write, an ended session or a read-back mismatch keeps nothing.
pub fn keeps_plan_for_retry(why: &LinkNotPublished) -> bool {
    *why == LinkNotPublished::NotWritten
}

/// The facts of the fresh read the ownership proof explains a refusal by.
fn account_observed(observation: &GithubObservation) -> GithubAccount<'_> {
    GithubAccount {
        login: &observation.login,
        user_id: observation.user_id,
        bio: observation.bio.as_deref(),
    }
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
    let projected = project(ClaimFields::of(&claim), &provenance)?;
    let admitted = stored.kind == PlanKind::Publish
        && claim.references.is_empty()
        && projected.did == bare_did(owner_did)
        && stored.key == triple_of(&claim);
    admitted.then_some(claim)
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

    /// The `github.link.refused` log labels are an operator contract: each
    /// refusal keeps its own literal name (mutation testing, slice 3).
    #[test]
    fn each_link_refusal_logs_under_its_own_label() {
        for (refusal, label) in [
            (
                LinkConfirmRefusal::ProofGone(CheckVerdict::DidMissing),
                "proof_gone",
            ),
            (LinkConfirmRefusal::Unreadable, "unreadable"),
            (LinkConfirmRefusal::IdentityChanged, "identity_changed"),
        ] {
            assert_eq!(refusal.label(), label);
        }
        for (refusal, label) in [
            (LinkRefusal::TokenRejected, "token_rejected"),
            (BUDGET_SPENT, "too_many_attempts"),
            (
                LinkRefusal::Ownership(OwnershipRefusal::RateLimited),
                "rate_limited",
            ),
            (
                LinkRefusal::Ownership(OwnershipRefusal::AccountNotFound),
                "account_not_found",
            ),
        ] {
            assert_eq!(refusal.label(), label);
        }
    }
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

        /// Universe: owner DIDs × another DID × GitHub answers (a real or a
        /// malformed account) × literal bios. `link_preview` builds a plan
        /// exactly when the fresh read names a real account and its bio
        /// names the signed-in DID as a token; the plan links that DID to
        /// that account, is the owner's own link plan, and is never a claim
        /// publish (`restore_plan` refuses it on both confirm paths).
        #[test]
        fn a_link_preview_builds_a_plan_exactly_when_the_fresh_read_proves_the_signed_in_did(
            did in arb_did(), other in arb_did(), account in arb_account(),
            bio_kind in 0usize..6, real_account in any::<bool>(),
        ) {
            prop_assume!(did != other);
            let (login, user_id) = account;
            let (bio, names_did) = match bio_kind {
                0 => (Some(format!("my DID: {did}")), true),
                1 => (Some(format!("Builder. {did}.")), true),
                2 => (Some(format!("Builder. {other}")), false),
                3 => (Some(format!("my DID: {did}x")), false),
                4 => (Some("no DID here".to_string()), false),
                _ => (None, false),
            };
            let observation = GithubObservation {
                login: login.clone(),
                user_id: if real_account { user_id } else { 0 },
                bio,
            };

            let preview = link_preview(&did, &observation, COMPOSED_AT);

            prop_assert_eq!(preview.is_ok(), names_did && real_account, "{:?}", preview);
            match preview {
                Ok(plan) => {
                    prop_assert_eq!(plan.owner_did(), did.as_str());
                    prop_assert_eq!(&plan.claim().subject, &did);
                    prop_assert_eq!(&plan.claim().object, &format!("github:{login}"));
                    prop_assert_eq!(&restore_link_plan(&did, &plan.stored()), &Ok(plan.clone()));
                    prop_assert!(restore_plan(&did, &plan.stored()).is_err());
                }
                // RA-11b: a bio naming another DID is explained with that DID.
                Err(refusal) if real_account && bio_kind == 2 => {
                    prop_assert_eq!(refusal, LinkPreviewRefusal::Ownership(OwnershipRefusal::DifferentDid(other)));
                }
                // A read that does not prove the link is explained as Verify
                // explains it, never as a preview that could not be saved.
                Err(refusal) => prop_assert!(matches!(refusal, LinkPreviewRefusal::Ownership(_)), "{:?}", refusal),
            }
        }

        /// Universe: owner DIDs × another DID × two GitHub accounts × the
        /// confirm-time check (proving the plan's own link, the same account
        /// for another DID, another login, another account number, a
        /// disproving bio, an unreadable answer). The confirm admits the
        /// plan exactly when the fresh proof names its DID, login and
        /// account; any other proof is IdentityChanged; a disproof or an
        /// unreadable answer is never admitted.
        #[test]
        fn a_link_confirm_admits_the_plan_only_when_the_fresh_read_proves_exactly_its_link(
            did in arb_did(), other in arb_did(), account in arb_account(),
            other_account in arb_account(), case in 0usize..6,
        ) {
            prop_assume!(did != other);
            prop_assume!(account.0 != other_account.0 && account.1 != other_account.1);
            let plan = link_plan(&did, &proven(&did, &account), COMPOSED_AT).expect("link plan");
            let (login, user_id) = account.clone();
            let fresh = match case {
                0 => Ok(ProofCheck::Verified(proven(&did, &account))),
                1 => Ok(ProofCheck::Verified(proven(&other, &account))),
                2 => Ok(ProofCheck::Verified(proven(&did, &(other_account.0.clone(), user_id)))),
                3 => Ok(ProofCheck::Verified(proven(&did, &(login.clone(), other_account.1)))),
                4 => check_link(&Did(did.clone()), &GithubObservation {
                    login: login.clone(), user_id, bio: Some("no DID here".into()),
                }),
                _ => check_link(&Did(did.clone()), &GithubObservation {
                    login: login.clone(), user_id: 0, bio: Some(format!("my DID: {did}")),
                }),
            };

            let confirmed = confirm_link(plan.clone(), fresh);

            match case {
                0 => prop_assert_eq!(confirmed, Ok(plan)),
                1..=3 => prop_assert_eq!(confirmed, Err(LinkConfirmRefusal::IdentityChanged)),
                4 => prop_assert_eq!(confirmed, Err(LinkConfirmRefusal::ProofGone(CheckVerdict::DidMissing))),
                _ => prop_assert_eq!(confirmed, Err(LinkConfirmRefusal::Unreadable)),
            }
        }

        /// C-6: a stored link plan is never a publish-queue entry. Its key
        /// carries the reserved link predicate, so no claim draft the queue
        /// could publish shares it, and neither confirm path for claims
        /// restores it.
        #[test]
        fn a_stored_link_plan_never_stands_for_a_publish_queue_entry(
            did in arb_did(), account in arb_account(),
        ) {
            let plan = link_plan(&did, &proven(&did, &account), COMPOSED_AT).expect("link plan");
            let stored = plan.stored();
            let as_queue_entry = ClaimDraft {
                key: stored.key.clone(),
                evidence: plan.claim().evidence.clone(),
                confidence_bp: 10_000,
            };
            prop_assert_eq!(
                publish_plan(&did, &as_queue_entry, COMPOSED_AT).err(),
                Some(PlanError::ReservedPredicate)
            );
            prop_assert!(restore_plan(&did, &stored).is_err());
        }

        /// Universe: every way a link confirm can end without publishing —
        /// one of each variant (`LinkNotPublished::one_of_each`, an
        /// exhaustive match beside the enum) plus more refusals.
        /// The plan is put back for Retry exactly when the repo host did not
        /// take the write; an expired preview, a spent budget, an unproven
        /// read, an ended session and a read-back mismatch keep nothing.
        #[test]
        fn a_link_confirm_keeps_its_plan_only_for_a_failed_write(
            why in prop::sample::select(
                LinkNotPublished::one_of_each()
                    .into_iter()
                    .chain([
                        LinkNotPublished::Refused(LinkRefusal::TokenRejected),
                        LinkNotPublished::Refused(LinkRefusal::Ownership(OwnershipRefusal::RateLimited)),
                    ])
                    .collect::<Vec<_>>(),
            ),
        ) {
            prop_assert_eq!(keeps_plan_for_retry(&why), why == LinkNotPublished::NotWritten, "{:?}", why);
        }

        /// Universe: owner DIDs × another DID × a GitHub account × the
        /// disproving bios (literal oracles: a bio without the DID, a bio
        /// naming another DID, no bio). A confirm whose fresh read disproves
        /// the link is refused as ProofGone with that verdict, writes nothing
        /// (no plan is admitted), keeps no plan for Retry, and its page says
        /// the proof is gone and that nothing was published.
        #[test]
        fn a_disproved_link_confirm_writes_nothing_and_keeps_no_plan(
            did in arb_did(), other in arb_did(), account in arb_account(), bio_kind in 0usize..3,
        ) {
            prop_assume!(did != other);
            let plan = link_plan(&did, &proven(&did, &account), COMPOSED_AT).expect("link plan");
            let (login, user_id) = account;
            let login_of_plan = login.clone();
            let (bio, verdict) = match bio_kind {
                0 => (Some("Builder. Rust, tide models".to_string()), CheckVerdict::DidMissing),
                1 => (Some(format!("my DID: {other}")), CheckVerdict::DifferentDid),
                _ => (None, CheckVerdict::NoBio),
            };
            let fresh = check_link(&Did(did.clone()), &GithubObservation { login, user_id, bio });

            let refusal = confirm_link(plan, fresh).expect_err("a disproved read admits no plan");

            prop_assert_eq!(&refusal, &LinkConfirmRefusal::ProofGone(verdict));
            let why = refusal.not_published();
            prop_assert_eq!(&why, &LinkNotPublished::ProofGone);
            prop_assert!(!keeps_plan_for_retry(&why));
            let page = crate::views::link_not_published_page(&why, &login_of_plan, None);
            prop_assert!(page.contains("proof is gone"), "{}", page);
            prop_assert!(page.contains(crate::views::NOTHING_PUBLISHED), "{}", page);
        }

        /// F-08, F-10. Universe: every GitHub read failure (each variant,
        /// arbitrary detail) and a spent budget × logins. Literal oracle:
        /// a rate limit is RateLimited, a 401 is TokenRejected, a missing or
        /// private account is AccountNotFound, anything else is
        /// GithubUnavailable; a rate limit never reads as a disproof. Each
        /// keeps no plan, says nothing was published and never echoes the
        /// failure's detail.
        #[test]
        fn an_unreadable_link_confirm_publishes_nothing_and_keeps_no_plan(
            case in 0usize..8, detail in "[a-z]{12,20}", login in "[a-z][a-z0-9-]{0,15}",
        ) {
            let (refusal, expected) = match case {
                0 => (link_refusal_of(&GithubError::RateLimited { authenticated: true }),
                      LinkRefusal::Ownership(OwnershipRefusal::RateLimited)),
                1 => (link_refusal_of(&GithubError::RateLimited { authenticated: false }),
                      LinkRefusal::Ownership(OwnershipRefusal::RateLimited)),
                2 => (link_refusal_of(&GithubError::TokenRejected), LinkRefusal::TokenRejected),
                3 => (link_refusal_of(&GithubError::NotFound { target: detail.clone() }),
                      LinkRefusal::Ownership(OwnershipRefusal::AccountNotFound)),
                4 => (link_refusal_of(&GithubError::NotPublic { target: detail.clone() }),
                      LinkRefusal::Ownership(OwnershipRefusal::AccountNotFound)),
                5 => (link_refusal_of(&GithubError::Network(detail.clone())),
                      LinkRefusal::Ownership(OwnershipRefusal::GithubUnavailable)),
                6 => (link_refusal_of(&GithubError::ApiShape(detail.clone())),
                      LinkRefusal::Ownership(OwnershipRefusal::GithubUnavailable)),
                _ => (BUDGET_SPENT, LinkRefusal::Ownership(OwnershipRefusal::TooManyAttempts)),
            };
            prop_assert_eq!(&refusal, &expected);
            if let LinkRefusal::Ownership(ownership) = &refusal {
                prop_assert!(!ownership.disproves_ownership() || case == 3 || case == 4);
            }

            let why = LinkNotPublished::Refused(refusal);

            prop_assert!(!keeps_plan_for_retry(&why));
            let page = crate::views::link_not_published_page(&why, &login, None);
            prop_assert!(page.contains(crate::views::NOTHING_PUBLISHED), "{}", page);
            prop_assert!(!page.contains(&detail), "{}", page);
        }

        /// Universe: owner DIDs × GitHub accounts. A confirm-time answer
        /// naming no account (user id 0) is MalformedObservation: the confirm
        /// admits nothing, it is GitHub being unavailable, keeps no plan and
        /// its page says GitHub could not be reached and nothing was published.
        #[test]
        fn an_answer_naming_no_account_at_confirm_is_github_unavailable(
            did in arb_did(), account in arb_account(),
        ) {
            let plan = link_plan(&did, &proven(&did, &account), COMPOSED_AT).expect("link plan");
            let (login, _) = account;
            let fresh = check_link(&Did(did.clone()), &GithubObservation {
                login: login.clone(), user_id: 0, bio: Some(format!("my DID: {did}")),
            });

            let refusal = confirm_link(plan, fresh).expect_err("an unreadable answer admits no plan");

            prop_assert_eq!(&refusal, &LinkConfirmRefusal::Unreadable);
            let why = refusal.not_published();
            prop_assert_eq!(
                &why,
                &LinkNotPublished::Refused(LinkRefusal::Ownership(OwnershipRefusal::GithubUnavailable))
            );
            prop_assert!(!keeps_plan_for_retry(&why));
            let page = crate::views::link_not_published_page(&why, &login, None);
            prop_assert!(page.contains("GitHub could not be reached"), "{}", page);
            prop_assert!(page.contains(crate::views::NOTHING_PUBLISHED), "{}", page);
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
