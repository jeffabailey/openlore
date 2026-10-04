//! `GET /retract` and `POST /retract`: retracting a published claim
//! (US-BRA-011, ADR-008). Retracting ADDS a self-attested record whose one
//! reference `retracts` the claim's CID; nothing is ever deleted or updated
//! (the write port cannot express either). The preview builds a pure
//! `RetractPlan` for a claim read LIVE from the owner's own PDS and keeps it
//! in the take-once plan store; only "Confirm retraction" writes, once. A
//! claim that is no longer live (already retracted) is never retracted again.

use hyper::StatusCode;
use ports::claim_domain::{RecordOrigin, UnsignedClaim};
use ports::{PlanKind, ResolvedIdentity, SuggestionState, TakenPublishPlan};
use review_domain::lifecycle::{retraction_step, OwnerStep};
use review_domain::plans::{
    plan_expires_at, plan_freshness, restore_retract_plan, retract_plan, rfc3339_utc,
    PlanFreshness, RetractPlan,
};
use review_domain::views::{self, live_published_claim, PublishRetry, RetractPreview};

use crate::executor::execute_retract;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now;
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_matches, csrf_token_for, current_session, field, forbidden, to_landing, SESSION_COOKIE,
};
use crate::wiring::{emit, LogEvent};

/// The owner's PDS could not be read, so nothing can be decided or written.
struct PdsUnreachable;

/// Open the retract preview for the owner's live claim `?claim=<cid>`.
pub(crate) async fn retract_preview(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie, session)) = current_session(app, request) else {
        return to_landing();
    };
    let Ok(identity) = app.identity.resolve_did(&session.owner_did).await else {
        return unavailable(views::DIRECTORY_UNREACHABLE_NOTICE, "/review");
    };
    let rkey = field(&request.query, "claim").unwrap_or_default();
    let original = match live_claim(app, &identity, &rkey).await {
        Ok(Some(claim)) => claim,
        Ok(None) => return not_found(),
        Err(PdsUnreachable) => {
            return unavailable(views::PDS_UNREACHABLE_NOTICE, &profile_path(&identity))
        }
    };
    let kept = retract_plan(&identity.did, &rkey, &original, &rfc3339_utc(now()))
        .ok()
        .filter(|plan| {
            app.plans
                .put_publish_plan(&identity.did, &plan.stored(), plan_expires_at(now()))
                .is_ok()
        });
    match kept {
        Some(plan) => page(
            StatusCode::OK,
            views::retract_preview_page(&RetractPreview {
                plan: &plan,
                csrf_token: &csrf_token_for(&cookie),
                profile_path: &profile_path(&identity),
            }),
        ),
        None => not_found(),
    }
}

/// "Confirm retraction" (or "Retry"): take the retract plan exactly once and
/// add the retraction — unless the claim already is retracted.
pub(crate) async fn confirm_retract(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let owner_did = session.owner_did.as_str();
    let plan_id = field(&request.form, "plan").unwrap_or_default();
    let Some(TakenPublishPlan { plan, expires_at }) = app
        .plans
        .take_publish_plan(owner_did, &plan_id, PlanKind::Retract)
        .ok()
        .flatten()
    else {
        return not_found();
    };
    let Ok(identity) = app.identity.resolve_did(owner_did).await else {
        return unavailable(views::DIRECTORY_UNREACHABLE_NOTICE, "/review");
    };
    if plan_freshness(expires_at, now()) == PlanFreshness::Expired {
        return unavailable(
            "This preview has expired. Nothing was retracted; press Retract again.",
            &profile_path(&identity),
        );
    }
    let Ok(plan) = restore_retract_plan(owner_did, &plan) else {
        return not_found();
    };
    let confirmed = Confirmed {
        app,
        plan: &plan,
        expires_at,
        csrf_token: &field(&request.cookies, SESSION_COOKIE)
            .map(|cookie| csrf_token_for(&cookie))
            .unwrap_or_default(),
        profile_path: &profile_path(&identity),
    };
    match live_claim(app, &identity, plan.retracted_cid()).await {
        Ok(live) => match retraction_step(live.is_some()) {
            Some(OwnerStep::Move { .. }) => confirmed.retract().await,
            _ => page(
                StatusCode::OK,
                views::retracted_page(None, confirmed.profile_path),
            ),
        },
        Err(PdsUnreachable) => {
            confirmed.not_retracted("We couldn't reach your PDS. Nothing was retracted.")
        }
    }
}

/// A taken, fresh retract plan of a still-live claim.
struct Confirmed<'a> {
    app: &'a App,
    plan: &'a RetractPlan,
    expires_at: i64,
    csrf_token: &'a str,
    profile_path: &'a str,
}

impl Confirmed<'_> {
    async fn retract(&self) -> Reply {
        let app = self.app;
        match execute_retract(app.repo_write.as_ref(), app.repo_read.as_ref(), self.plan).await {
            Ok(at_uri) => {
                emit(LogEvent::RetractPosted);
                let _ = app.review_write.change_state(
                    self.plan.owner_did(),
                    &self.plan.key(),
                    SuggestionState::Published,
                    SuggestionState::Retracted,
                );
                page(
                    StatusCode::OK,
                    views::retracted_page(Some(&at_uri), self.profile_path),
                )
            }
            Err(failure) => self.not_retracted(failure.retract_message()),
        }
    }

    /// Nothing was retracted: the plan goes back (same deadline) so Retry
    /// adds the retraction exactly once.
    fn not_retracted(&self, reason: &str) -> Reply {
        let kept = self
            .app
            .plans
            .put_publish_plan(self.plan.owner_did(), &self.plan.stored(), self.expires_at)
            .is_ok();
        let retry = kept.then_some(PublishRetry {
            plan_id: self.plan.rkey(),
            csrf_token: self.csrf_token,
        });
        page(
            StatusCode::BAD_GATEWAY,
            views::retract_failed_page(reason, retry, self.profile_path),
        )
    }
}

/// The owner's live published claim under `rkey`, read from their own PDS.
async fn live_claim(
    app: &App,
    identity: &ResolvedIdentity,
    rkey: &str,
) -> Result<Option<UnsignedClaim>, PdsUnreachable> {
    let listing = app
        .repo_listing
        .list_repo_claims(&identity.pds_endpoint, &identity.did)
        .await
        .map_err(|_| PdsUnreachable)?;
    let origin = RecordOrigin::of(&listing.fetched_from, &identity.pds_endpoint);
    Ok(live_published_claim(
        &identity.did,
        &listing.records,
        origin,
        rkey,
    ))
}

fn profile_path(identity: &ResolvedIdentity) -> String {
    format!("/@{}", identity.verified_handle)
}

fn unavailable(reason: &str, profile_path: &str) -> Reply {
    page(
        StatusCode::SERVICE_UNAVAILABLE,
        views::retract_failed_page(reason, None, profile_path),
    )
}

fn page(status: StatusCode, html: String) -> Reply {
    Reply::Page {
        status,
        html,
        set_cookie: None,
    }
}

fn now() -> i64 {
    i64::try_from(unix_now()).unwrap_or_default()
}
