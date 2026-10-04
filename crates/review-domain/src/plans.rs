//! Publish plans (Plan-value pattern, US-BRA-004 / AC-004.4): building a
//! plan is PURE and writes nothing; the preview shows exactly the plan's
//! record, and confirming executes exactly that record, once.
//!
//! The record is a SELF-ATTESTED `org.openlore.claim` (ADR-071): no
//! `signature`, `author` = the owner's bare DID, confidence in integer basis
//! points (ADR-070), and its record key is the claim's canonical CID
//! (claim-domain's one canonicalizer) — the SPIKE-1 wire shape.

use ports::claim_domain::{
    decode_claim_record, ClaimError, ClaimRecord, Confidence, Did, SelfAttestedClaim, UnsignedClaim,
};
use ports::{StoredPublishPlan, SuggestionKey};
use serde_json::{json, Value};

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
    let unsigned = UnsignedClaim {
        subject: draft.key.subject.clone(),
        predicate: draft.key.predicate.clone(),
        object: draft.key.object.clone(),
        evidence: draft.evidence.clone(),
        confidence: Confidence::from_basis_points(i64::from(draft.confidence_bp)),
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
    let record: Value =
        serde_json::from_str(&stored.record_json).map_err(|_| PlanError::NotTheOwnersRecord)?;
    match decode_claim_record(&record, "") {
        Ok(ClaimRecord::SelfAttested(claim))
            if claim.cid().0 == stored.plan_id && claim.unsigned().author_did.0 == owner_did =>
        {
            Ok(PublishPlan {
                owner_did: owner_did.to_string(),
                key: stored.key.clone(),
                claim,
            })
        }
        _ => Err(PlanError::NotTheOwnersRecord),
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
}
