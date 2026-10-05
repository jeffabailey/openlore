//! `GET /retract` and `POST /retract`: retracting a published claim
//! (US-BRA-011, ADR-008). Retracting ADDS a self-attested record whose one
//! reference `retracts` the claim's CID; nothing is ever deleted or updated
//! (the write port cannot express either). The preview builds a pure
//! `RetractPlan` for a claim read LIVE from the owner's own PDS and keeps it
//! in the take-once plan store; only "Confirm retraction" writes, once. A
//! claim that is no longer live (already retracted) is never retracted again.

use hyper::StatusCode;
use ports::claim_domain::{RecordOrigin, UnsignedClaim};
use ports::{PlanKind, ResolvedIdentity, SuggestionState};
use review_domain::lifecycle::{retraction_step, OwnerStep};
use review_domain::plans::{
    plan_expires_at, restore_retract_plan, retract_plan, rfc3339_utc, RetractPlan,
};
use review_domain::views::{self, live_published_claim, PublishRetry, RetractPreview};

use crate::executor::execute_retract;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::confirm::{take_confirmed_plan, ConfirmedPlan};
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_for, current_session, field, to_landing};
use crate::wiring::{observe, LogEvent};

/// What a confirm that arrived after its preview's deadline is told.
const EXPIRED_NOTICE: &str =
    "This preview has expired. Nothing was retracted; press Retract again.";

/// What a confirm whose PDS could not be read is told.
const PDS_UNREACHABLE_ON_CONFIRM: &str = "We couldn't reach your PDS. Nothing was retracted.";

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
    let now = unix_now_secs();
    let kept = retract_plan(&identity.did, &rkey, &original, &rfc3339_utc(now))
        .ok()
        .filter(|plan| {
            app.plans
                .put_publish_plan(&identity.did, &plan.stored(), plan_expires_at(now))
                .is_ok()
        });
    match kept {
        Some(plan) => Reply::page(
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
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Retract) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    let Ok(identity) = app.identity.resolve_did(&confirmed.owner_did).await else {
        return unavailable(views::DIRECTORY_UNREACHABLE_NOTICE, "/review");
    };
    let profile_path = profile_path(&identity);
    if confirmed.expired() {
        return unavailable(EXPIRED_NOTICE, &profile_path);
    }
    let Ok(plan) = restore_retract_plan(&confirmed.owner_did, &confirmed.stored) else {
        return not_found();
    };
    let retraction = Retraction {
        app,
        plan: &plan,
        confirmed: &confirmed,
        profile_path: &profile_path,
    };
    match live_claim(app, &identity, plan.retracted_cid()).await {
        Ok(live) => match retraction_step(live.is_some()) {
            Some(OwnerStep::Move { .. }) => retraction.add().await,
            _ => Reply::page(StatusCode::OK, views::retracted_page(None, &profile_path)),
        },
        Err(PdsUnreachable) => retraction.not_added(PDS_UNREACHABLE_ON_CONFIRM),
    }
}

/// A taken, fresh retract plan of a still-live claim.
struct Retraction<'a> {
    app: &'a App,
    plan: &'a RetractPlan,
    confirmed: &'a ConfirmedPlan,
    profile_path: &'a str,
}

impl Retraction<'_> {
    async fn add(&self) -> Reply {
        let app = self.app;
        match execute_retract(app.repo_write.as_ref(), app.repo_read.as_ref(), self.plan).await {
            Ok(at_uri) => {
                observe(app, LogEvent::RetractPosted);
                let _ = app.review_write.change_state(
                    self.plan.owner_did(),
                    &self.plan.key(),
                    SuggestionState::Published,
                    SuggestionState::Retracted,
                );
                Reply::page(
                    StatusCode::OK,
                    views::retracted_page(Some(&at_uri), self.profile_path),
                )
            }
            Err(failure) => self.not_added(failure.retract_message()),
        }
    }

    /// Nothing was retracted: the plan goes back (same deadline) so Retry
    /// adds the retraction exactly once.
    fn not_added(&self, reason: &str) -> Reply {
        let kept = self
            .app
            .plans
            .put_publish_plan(
                self.plan.owner_did(),
                &self.plan.stored(),
                self.confirmed.expires_at,
            )
            .is_ok();
        let retry = kept.then_some(PublishRetry {
            plan_id: self.plan.rkey(),
            csrf_token: &self.confirmed.csrf_token,
        });
        Reply::page(
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
    Reply::page(
        StatusCode::SERVICE_UNAVAILABLE,
        views::retract_failed_page(reason, None, profile_path),
    )
}
