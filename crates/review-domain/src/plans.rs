//! Publish plans (Plan-value pattern, US-BRA-004 / AC-004.4): building a
//! plan is PURE and writes nothing; the preview shows exactly the plan's
//! record, and confirming executes exactly that record, once.
//!
//! The record is a SELF-ATTESTED `org.openlore.claim` (ADR-071): no
//! `signature`, `author` = the owner's bare DID, confidence in integer basis
//! points (ADR-070), and its record key is the claim's canonical CID
//! (claim-domain's one canonicalizer) — the SPIKE-1 wire shape.

use ports::claim_domain::{
    decode_claim_record, Cid, ClaimError, ClaimRecord, ClaimReference, Confidence, Did,
    ReferenceType, SelfAttestedClaim, UnsignedClaim,
};
use ports::{PlanKind, StoredPublishPlan, SuggestionKey};
use serde_json::{json, Value};

use crate::edits::ClaimEdit;

/// The ATProto collection approved claims are written to.
pub const CLAIM_COLLECTION: &str = "org.openlore.claim";

/// What the owner is about to publish: a suggestion's key, evidence and
/// confidence (basis points, 0..=10000).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimDraft {
    pub key: SuggestionKey,
    pub evidence: Vec<String>,
    pub confidence_bp: u16,
}

/// The exact record a confirm will create in the owner's own repo.
#[derive(Debug, Clone, PartialEq)]
pub struct PublishPlan {
    owner_did: String,
    key: SuggestionKey,
    claim: SelfAttestedClaim,
}

/// Why a plan cannot be built or restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// The claim cannot be canonicalized (no CID, so no record key).
    NotCanonical(String),
    /// A stored plan no longer is the self-attested record of its owner
    /// under its own key (tampered or corrupted): it is never executed.
    NotTheOwnersRecord,
}

impl From<ClaimError> for PlanError {
    fn from(err: ClaimError) -> Self {
        Self::NotCanonical(err.to_string())
    }
}

impl PublishPlan {
    pub fn owner_did(&self) -> &str {
        &self.owner_did
    }

    pub fn key(&self) -> &SuggestionKey {
        &self.key
    }

    pub fn claim(&self) -> &UnsignedClaim {
        self.claim.unsigned()
    }

    /// The record key: the claim's canonical CID.
    pub fn rkey(&self) -> &str {
        &self.claim.cid().0
    }

    /// The exact record JSON to be written (lexicon keys only).
    pub fn record(&self) -> Value {
        claim_record_json(self.claim.unsigned())
    }

    /// Where the record will live: `at://<did>/org.openlore.claim/<cid>`.
    pub fn at_uri(&self) -> String {
        at_uri(&self.owner_did, self.rkey())
    }

    /// The plan as kept between preview and confirm.
    pub fn stored(&self) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: self.rkey().to_string(),
            kind: PlanKind::Publish,
            key: self.key.clone(),
            record_json: self.record().to_string(),
        }
    }
}

/// Build the publish plan for `draft`, authored by `owner_did`, composed at
/// `composed_at` (RFC 3339 UTC). Pure: nothing is written.
pub fn publish_plan(
    owner_did: &str,
    draft: &ClaimDraft,
    composed_at: &str,
) -> Result<PublishPlan, PlanError> {
    let unedited = ClaimEdit {
        object: draft.key.object.clone(),
        confidence_bp: draft.confidence_bp,
    };
    publish_edited_plan(owner_did, draft, &unedited, composed_at)
}

/// Build the publish plan for the owner's `edit` of `draft` (US-BRA-005):
/// the record carries exactly the edited philosophy and confidence, so its
/// CID — the record key — is the edited claim's (ADR-071); the plan still
/// names the suggestion it publishes. Pure: nothing is written.
pub fn publish_edited_plan(
    owner_did: &str,
    draft: &ClaimDraft,
    edit: &ClaimEdit,
    composed_at: &str,
) -> Result<PublishPlan, PlanError> {
    let unsigned = UnsignedClaim {
        subject: draft.key.subject.clone(),
        predicate: draft.key.predicate.clone(),
        object: edit.object.clone(),
        evidence: draft.evidence.clone(),
        confidence: Confidence::from_basis_points(i64::from(edit.confidence_bp)),
        author_did: Did(owner_did.to_string()),
        composed_at: composed_at.to_string(),
        references: Vec::new(),
        reason: None,
    };
    Ok(PublishPlan {
        owner_did: owner_did.to_string(),
        key: draft.key.clone(),
        claim: SelfAttestedClaim::new(unsigned)?,
    })
}

/// Restore a stored plan for `owner_did`, admitting it only if its record
/// still is a self-attested claim authored by the owner whose recomputed
/// CID is its plan id.
pub fn restore_plan(owner_did: &str, stored: &StoredPublishPlan) -> Result<PublishPlan, PlanError> {
    let claim = owners_stored_claim(owner_did, stored, PlanKind::Publish)?;
    // A publish creates a fresh claim: a record that references another
    // (a retraction above all) is never published through this confirm.
    if !claim.unsigned().references.is_empty() {
        return Err(PlanError::NotTheOwnersRecord);
    }
    Ok(PublishPlan {
        owner_did: owner_did.to_string(),
        key: stored.key.clone(),
        claim,
    })
}

/// The stored plan's record, admitted only if the plan is of `kind` and its
/// record is a self-attested claim authored by `owner_did` whose recomputed
/// CID is the plan id.
fn owners_stored_claim(
    owner_did: &str,
    stored: &StoredPublishPlan,
    kind: PlanKind,
) -> Result<SelfAttestedClaim, PlanError> {
    let record: Value =
        serde_json::from_str(&stored.record_json).map_err(|_| PlanError::NotTheOwnersRecord)?;
    match decode_claim_record(&record, "") {
        Ok(ClaimRecord::SelfAttested(claim))
            if stored.kind == kind
                && claim.cid().0 == stored.plan_id
                && claim.unsigned().author_did.0 == owner_did =>
        {
            Ok(claim)
        }
        _ => Err(PlanError::NotTheOwnersRecord),
    }
}

/// The retraction a confirm will create (US-BRA-011, ADR-008: retracting
/// ADDS a record, it never deletes or updates one): the retracted claim's
/// own fields, re-composed by its author, with exactly one `retracts`
/// reference to the retracted claim's CID. Self-attested, keyed by its CID.
#[derive(Debug, Clone, PartialEq)]
pub struct RetractPlan {
    owner_did: String,
    retraction: SelfAttestedClaim,
}

impl RetractPlan {
    pub fn owner_did(&self) -> &str {
        &self.owner_did
    }

    /// The CID (record key) of the claim being retracted.
    pub fn retracted_cid(&self) -> &str {
        retracted_cid_of(self.retraction.unsigned()).unwrap_or_default()
    }

    pub fn claim(&self) -> &UnsignedClaim {
        self.retraction.unsigned()
    }

    /// The retraction's record key: its own canonical CID.
    pub fn rkey(&self) -> &str {
        &self.retraction.cid().0
    }

    /// The exact retraction record JSON to be written.
    pub fn record(&self) -> Value {
        claim_record_json(self.retraction.unsigned())
    }

    pub fn at_uri(&self) -> String {
        at_uri(&self.owner_did, self.rkey())
    }

    /// The suggestion the retracted claim was published from.
    pub fn key(&self) -> SuggestionKey {
        let claim = self.claim();
        SuggestionKey {
            subject: claim.subject.clone(),
            predicate: claim.predicate.clone(),
            object: claim.object.clone(),
        }
    }

    /// The plan as kept between preview and confirm.
    pub fn stored(&self) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: self.rkey().to_string(),
            kind: PlanKind::Retract,
            key: self.key(),
            record_json: self.record().to_string(),
        }
    }
}

/// The only reference a retraction carries: `retracts` → the claim's CID.
fn retracted_cid_of(claim: &UnsignedClaim) -> Option<&str> {
    match claim.references.as_slice() {
        [ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid,
        }] => Some(cid.0.as_str()),
        _ => None,
    }
}

/// Build the retraction of the owner's own published claim `original`
/// (record key `original_cid`), composed at `composed_at`. Pure: nothing is
/// written. Someone else's claim is never retracted.
pub fn retract_plan(
    owner_did: &str,
    original_cid: &str,
    original: &UnsignedClaim,
    composed_at: &str,
) -> Result<RetractPlan, PlanError> {
    if original.author_did.0 != owner_did {
        return Err(PlanError::NotTheOwnersRecord);
    }
    let retraction = UnsignedClaim {
        composed_at: composed_at.to_string(),
        references: vec![ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid: Cid(original_cid.to_string()),
        }],
        reason: None,
        ..original.clone()
    };
    Ok(RetractPlan {
        owner_did: owner_did.to_string(),
        retraction: SelfAttestedClaim::new(retraction)?,
    })
}

/// Restore a stored retraction for `owner_did`: only a retract plan whose
/// record is still the owner's self-attested retraction under its own key.
pub fn restore_retract_plan(
    owner_did: &str,
    stored: &StoredPublishPlan,
) -> Result<RetractPlan, PlanError> {
    let retraction = owners_stored_claim(owner_did, stored, PlanKind::Retract)?;
    match retracted_cid_of(retraction.unsigned()) {
        Some(_) => Ok(RetractPlan {
            owner_did: owner_did.to_string(),
            retraction,
        }),
        None => Err(PlanError::NotTheOwnersRecord),
    }
}

/// The self-attested wire shape of a claim (SPIKE-1): `$type`, the claim
/// fields, confidence as basis points, the bare-DID author, `references`.
pub fn claim_record_json(claim: &UnsignedClaim) -> Value {
    let references: Vec<Value> = claim
        .references
        .iter()
        .map(|r| json!({"type": reference_type(r.ref_type), "cid": r.cid.0}))
        .collect();
    json!({
        "$type": CLAIM_COLLECTION,
        "subject": claim.subject,
        "predicate": claim.predicate,
        "object": claim.object,
        "evidence": claim.evidence,
        "confidence": claim.confidence.basis_points(),
        "author": claim.author_did.0,
        "composedAt": claim.composed_at,
        "references": references,
    })
}

fn reference_type(ref_type: ports::claim_domain::ReferenceType) -> &'static str {
    use ports::claim_domain::ReferenceType;
    match ref_type {
        ReferenceType::Retracts => "retracts",
        ReferenceType::Corrects => "corrects",
        ReferenceType::Counters => "counters",
        ReferenceType::Supersedes => "supersedes",
    }
}

/// `at://<did>/org.openlore.claim/<rkey>`.
pub fn at_uri(owner_did: &str, rkey: &str) -> String {
    format!("at://{owner_did}/{CLAIM_COLLECTION}/{rkey}")
}

/// Does a record read back from the repo hash to `rkey`? (DWD-5: the
/// PDS-returned CID is never trusted; the record is recomputed.)
pub fn read_back_matches(record: &Value, rkey: &str) -> bool {
    matches!(
        decode_claim_record(record, ""),
        Ok(ClaimRecord::SelfAttested(claim)) if claim.cid().0 == rkey
    )
}

/// RFC 3339 UTC (`YYYY-MM-DDTHH:MM:SSZ`) of a Unix time (civil calendar).
pub fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let of_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        (of_day % 3600) / 60,
        of_day % 60
    )
}

/// Days since 1970-01-01 → (year, month, day) (H. Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// How long a preview stays confirmable (ADR-074: `expires_at` = +30 min).
pub const PLAN_TTL_SECS: i64 = 30 * 60;

/// When a plan previewed at `previewed_at` (Unix seconds) stops being
/// confirmable.
pub fn plan_expires_at(previewed_at: i64) -> i64 {
    previewed_at.saturating_add(PLAN_TTL_SECS)
}

/// Whether a taken plan may still be executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFreshness {
    /// Confirmed in time: execute it.
    Fresh,
    /// Confirmed too late: refuse it, write nothing.
    Expired,
}

/// A plan is fresh strictly before its `expires_at`; from then on it is
/// expired and is never published.
pub fn plan_freshness(expires_at: i64, now: i64) -> PlanFreshness {
    if now < expires_at {
        PlanFreshness::Fresh
    } else {
        PlanFreshness::Expired
    }
}

#[cfg(test)]
mod tests {
    //! Universe: owner DIDs × suggestion keys × evidence × basis points ×
    //! composition times. State-delta: building a plan touches nothing but
    //! its own value; restoring a stored plan yields the same plan.
    use super::*;
    use proptest::prelude::*;

    fn arb_draft() -> impl Strategy<Value = (String, ClaimDraft)> {
        (
            "[a-z2-7]{24}".prop_map(|s| format!("did:plc:{s}")),
            "[a-z]{1,8}/[a-z]{1,8}",
            "[a-z-]{3,20}",
            prop::collection::vec("https://github\\.com/[a-z]{1,8}/[a-z]{1,8}", 1..4),
            0u16..=10_000,
        )
            .prop_map(|(did, repo, slug, evidence, bp)| {
                (
                    did,
                    ClaimDraft {
                        key: SuggestionKey {
                            subject: format!("github:{repo}"),
                            predicate: "embodiesPhilosophy".into(),
                            object: format!("org.openlore.philosophy.{slug}"),
                        },
                        evidence,
                        confidence_bp: bp,
                    },
                )
            })
    }

    proptest! {
        #[test]
        fn a_stored_plan_restores_to_the_same_plan_and_reads_back_under_its_key(
            (did, draft) in arb_draft(), secs in 0i64..4_102_444_800
        ) {
            let plan = publish_plan(&did, &draft, &rfc3339_utc(secs)).expect("plan");
            prop_assert_eq!(&restore_plan(&did, &plan.stored()), &Ok(plan.clone()));
            prop_assert!(read_back_matches(&plan.record(), plan.rkey()));
            prop_assert_eq!(&plan.record()["author"], &json!(did));
            prop_assert_eq!(&plan.record()["confidence"], &json!(draft.confidence_bp));
        }

        /// A stored plan is never executed for someone else, nor after its
        /// record was altered.
        #[test]
        fn a_stored_plan_is_refused_for_another_owner_or_when_altered(
            (did, draft) in arb_draft(), (other, _) in arb_draft()
        ) {
            prop_assume!(did != other);
            let stored = publish_plan(&did, &draft, "2026-10-04T15:02:11Z").expect("plan").stored();
            prop_assert_eq!(restore_plan(&other, &stored), Err(PlanError::NotTheOwnersRecord));
            let altered = StoredPublishPlan {
                record_json: stored.record_json.replace("embodiesPhilosophy", "rejectsPhilosophy"),
                ..stored.clone()
            };
            prop_assert_eq!(restore_plan(&did, &altered), Err(PlanError::NotTheOwnersRecord));
        }

        #[test]
        fn rfc3339_round_trips_through_the_civil_calendar(secs in 0i64..4_102_444_800) {
            let text = rfc3339_utc(secs);
            prop_assert_eq!(text.len(), 20);
            let (date, time) = text.trim_end_matches('Z').split_once('T').expect("T");
            let parts: Vec<i64> = date.split('-').chain(time.split(':')).map(|p| p.parse().expect("digits")).collect();
            let (y, m, d) = (parts[0], parts[1], parts[2]);
            // Days-from-civil (inverse), checked against the input.
            let y2 = if m <= 2 { y - 1 } else { y };
            let era = y2.div_euclid(400);
            let yoe = y2 - era * 400;
            let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            let days = era * 146_097 + doe - 719_468;
            prop_assert_eq!(days * 86_400 + parts[3] * 3600 + parts[4] * 60 + parts[5], secs);
        }
    }

    proptest! {
        /// Universe: the record's lexicon keys. An edited plan differs from
        /// the unedited one in exactly `object` and `confidence` (set to the
        /// edit); every other key is unchanged, its key is its recomputed
        /// CID, and it still names the suggestion it publishes.
        #[test]
        fn an_edited_plan_record_is_exactly_the_edit(
            (did, draft) in arb_draft(),
            slug in "[a-z-]{3,20}",
            bp in 0u16..=10_000,
        ) {
            let edit = ClaimEdit { object: format!("org.openlore.philosophy.{slug}"), confidence_bp: bp };
            let composed_at = "2026-10-04T15:02:11Z";
            let before = publish_plan(&did, &draft, composed_at).expect("plan").record();
            let plan = publish_edited_plan(&did, &draft, &edit, composed_at).expect("edited plan");
            let after = plan.record();
            for (field, value) in after.as_object().expect("object") {
                match field.as_str() {
                    "object" => prop_assert_eq!(value, &json!(edit.object)),
                    "confidence" => prop_assert_eq!(value, &json!(edit.confidence_bp)),
                    _ => prop_assert_eq!(value, &before[field]),
                }
            }
            prop_assert_eq!(after.as_object().map(|o| o.len()), before.as_object().map(|o| o.len()));
            prop_assert!(read_back_matches(&after, plan.rkey()));
            prop_assert_eq!(plan.key(), &draft.key);
            prop_assert_eq!(&restore_plan(&did, &plan.stored()), &Ok(plan.clone()));
        }
    }

    proptest! {
        /// Universe: the retraction record's lexicon keys. A retraction is
        /// the original claim's record re-composed by its author, differing
        /// in exactly `composedAt` and `references` (one `retracts` → the
        /// original's CID); it is self-attested under its own CID, never the
        /// original's key, and restores to the same plan.
        #[test]
        fn a_retraction_is_the_originals_record_plus_one_retracts_reference(
            (did, draft) in arb_draft(), published_at in 0i64..2_000_000_000, later in 1i64..1_000_000,
        ) {
            let original = publish_plan(&did, &draft, &rfc3339_utc(published_at)).expect("plan");
            let composed_at = rfc3339_utc(published_at + later);
            let plan = retract_plan(&did, original.rkey(), original.claim(), &composed_at).expect("retraction");
            let (before, after) = (original.record(), plan.record());
            for (field, value) in after.as_object().expect("object") {
                match field.as_str() {
                    "composedAt" => prop_assert_eq!(value, &json!(composed_at)),
                    "references" => prop_assert_eq!(value, &json!([{"type": "retracts", "cid": original.rkey()}])),
                    _ => prop_assert_eq!(value, &before[field]),
                }
            }
            prop_assert_eq!(plan.retracted_cid(), original.rkey());
            prop_assert_ne!(plan.rkey(), original.rkey());
            prop_assert!(read_back_matches(&after, plan.rkey()));
            prop_assert_eq!(&restore_retract_plan(&did, &plan.stored()), &Ok(plan.clone()));
            prop_assert_eq!(restore_retract_plan(&did, &original.stored()), Err(PlanError::NotTheOwnersRecord));
        }

        /// Universe: owners × claims. Only the claim's author can retract it.
        #[test]
        fn nobody_retracts_someone_elses_claim((did, draft) in arb_draft(), (other, _) in arb_draft()) {
            prop_assume!(did != other);
            let original = publish_plan(&did, &draft, "2026-10-04T15:02:11Z").expect("plan");
            prop_assert_eq!(
                retract_plan(&other, original.rkey(), original.claim(), "2026-10-04T16:00:00Z"),
                Err(PlanError::NotTheOwnersRecord)
            );
            let stored = retract_plan(&did, original.rkey(), original.claim(), "2026-10-04T16:00:00Z")
                .expect("retraction")
                .stored();
            prop_assert_eq!(restore_retract_plan(&other, &stored), Err(PlanError::NotTheOwnersRecord));
        }

        /// Universe: (the plan a preview kept, the kind its row is labelled
        /// with, the confirm that restores it). A stored plan is admitted
        /// by exactly the confirm of its own kind and only under its true
        /// label: a share draft is never published or retracted, a publish
        /// plan never posted or taken as a retraction, and so on.
        #[test]
        fn a_stored_plan_is_restored_only_by_the_confirm_of_its_own_kind(
            (did, draft) in arb_draft(),
            label in proptest::sample::select(PlanKind::ALL.to_vec()),
            confirm in proptest::sample::select(PlanKind::ALL.to_vec()),
        ) {
            let publish = publish_plan(&did, &draft, "2026-10-04T15:02:11Z").expect("plan");
            let retract = retract_plan(&did, publish.rkey(), publish.claim(), "2026-10-04T16:00:00Z")
                .expect("retraction");
            let share = crate::share::share_post_plan(
                "https://review.example/@priya.example",
                &[crate::views::PublishedClaim {
                    subject: draft.key.subject.clone(),
                    object: draft.key.object.clone(),
                    confidence_bp: draft.confidence_bp,
                    rkey: publish.rkey().to_string(),
                }],
            )
            .expect("share plan");
            for (kind, stored) in [
                (PlanKind::Publish, publish.stored()),
                (PlanKind::Retract, retract.stored()),
                (PlanKind::Share, share.stored(&did, "random-plan-id")),
            ] {
                let relabelled = StoredPublishPlan { kind: label, ..stored };
                let admitted = match confirm {
                    PlanKind::Publish => restore_plan(&did, &relabelled).is_ok(),
                    PlanKind::Retract => restore_retract_plan(&did, &relabelled).is_ok(),
                    PlanKind::Share => crate::share::restore_share_plan(&did, &relabelled).is_ok(),
                };
                prop_assert_eq!(admitted, kind == label && label == confirm, "{:?} labelled {:?} confirmed as {:?}", kind, label, confirm);
            }
        }
    }

    proptest! {
        /// Universe: (preview time, confirm time). Fresh exactly within the
        /// TTL after the preview; expired from the deadline on, forever.
        #[test]
        fn a_plan_is_confirmable_only_within_thirty_minutes_of_its_preview(
            previewed_at in 0i64..4_000_000_000,
            elapsed in -10i64..10_000,
        ) {
            let expires_at = plan_expires_at(previewed_at);
            let freshness = plan_freshness(expires_at, previewed_at + elapsed);
            let expected = if elapsed < PLAN_TTL_SECS { PlanFreshness::Fresh } else { PlanFreshness::Expired };
            prop_assert_eq!(freshness, expected);
            prop_assert_eq!(plan_freshness(expires_at, expires_at), PlanFreshness::Expired);
        }
    }
}
