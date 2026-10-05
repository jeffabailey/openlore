//! The confirm half of every Plan-value flow (publish, retract, share;
//! ADR-074): a signed-in, anti-forgery-checked POST names a previewed plan,
//! which is taken from the owner's own store exactly once, of exactly the
//! confirmed kind. What the plan becomes is each route's business.

use ports::{PlanKind, StoredPublishPlan, TakenPublishPlan};
use review_domain::plans::{plan_freshness, PlanFreshness};

use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_of, field, signed_in_post};

/// A plan of the signed-in owner, taken once for this confirm.
pub(crate) struct ConfirmedPlan {
    pub(crate) owner_did: String,
    /// The plan id the form named (the key a Retry puts it back under).
    pub(crate) plan_id: String,
    pub(crate) stored: StoredPublishPlan,
    /// The preview's deadline; a put-back keeps it unchanged.
    pub(crate) expires_at: i64,
    /// The anti-forgery token a Retry form re-submits.
    pub(crate) csrf_token: String,
}

impl ConfirmedPlan {
    /// Did the confirm arrive after the preview's deadline?
    pub(crate) fn expired(&self) -> bool {
        plan_freshness(self.expires_at, unix_now_secs()) == PlanFreshness::Expired
    }
}

/// Take the `kind` plan the confirm form names, or the reply that refuses
/// the confirm: no session, a forged token, or no such plan of the owner's
/// (already taken, expired and swept, someone else's, another kind).
pub(crate) fn take_confirmed_plan(
    app: &App,
    request: &PageRequest,
    kind: PlanKind,
) -> Result<ConfirmedPlan, Reply> {
    let (_, session) = signed_in_post(app, request)?;
    let plan_id = field(&request.form, "plan").ok_or_else(not_found)?;
    let TakenPublishPlan { plan, expires_at } = app
        .plans
        .take_publish_plan(&session.owner_did, &plan_id, kind)
        .ok()
        .flatten()
        .ok_or_else(not_found)?;
    Ok(ConfirmedPlan {
        owner_did: session.owner_did,
        plan_id,
        stored: plan,
        expires_at,
        csrf_token: csrf_token_of(request),
    })
}
