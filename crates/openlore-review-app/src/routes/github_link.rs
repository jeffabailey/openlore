//! "Publish this link" (slice 3, ADR-084, U-26): the verified owner's link
//! preview. A same-origin, signed-in POST spends the verify budget, checks
//! the login's shape, reads the profile itself and keeps the plan only when
//! that fresh read proves the signed-in DID (C-GDL-10, ADR-085 §2). The
//! stored verification is never the proof: this module holds no handle on
//! it. HTTP concerns and port calls only; the decision is
//! `review_domain::link_plan::link_preview`. Nothing is written to any repo.
//!
//! The confirm (03-05) takes the kind-publish plan once, admits it only as
//! the owner's link (`restore_link_plan`), spends the budget, reads the
//! profile again and writes only what `confirm_link` admits from that read.
//! A failed write puts the plan back under its id with its unchanged
//! deadline (03-06); its Retry is a full confirm through this same route.

use hyper::StatusCode;
use ports::claim_domain::identity_link::{check_link, GithubObservation};
use ports::claim_domain::Did;
use ports::{GithubAccountProfile, PlanKind, PublishPlanPort, WebSession};
use review_domain::link_plan::{
    confirm_link, keeps_plan_for_retry, link_preview, link_refusal_of, restore_link_plan, LinkPlan,
    LinkPreviewRefusal, LinkRefusal, BUDGET_SPENT,
};
use review_domain::ownership::{is_github_login, OwnershipRefusal};
use review_domain::plans::{plan_expires_at, rfc3339_utc};
use review_domain::views::{
    self, LinkNotPublished, OwnershipOutcome, PublishRetry, GITHUB_LOGIN_FIELD,
};

use crate::executor::{execute_link, PublishFailure};
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::confirm::{take_confirmed_plan, ConfirmedPlan};
use crate::routes::github::{github_page, refusal_of};
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_for, field, signed_in_post};
use crate::wiring::{emit, LogEvent};

/// `POST /github/link`: the exact-record preview of the owner's link to the
/// account the offer names, or the refusal explaining why there is none.
pub(crate) async fn preview_link(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let named = field(&request.form, GITHUB_LOGIN_FIELD).unwrap_or_default();
    let login = named.trim();
    match kept_link_plan(app, &session, login).await {
        Ok(plan) => Reply::page(
            StatusCode::OK,
            views::link_preview_page(&plan, &csrf_token_for(&cookie_value)),
        ),
        Err(LinkPreviewRefusal::Ownership(refusal)) => {
            let refused = OwnershipOutcome::Refused(&refusal);
            let message =
                views::ownership_message(refused, login, &session.owner_did, &session.handle);
            github_page(app, &session, &cookie_value, Some((refused, &message)))
        }
        Err(LinkPreviewRefusal::NotSaved) => preview_not_saved(login),
    }
}

/// A proven link whose preview could not be kept (U-37): nothing to
/// confirm, nothing published, and GitHub is not blamed.
fn preview_not_saved(login: &str) -> Reply {
    not_published(
        StatusCode::SERVICE_UNAVAILABLE,
        &LinkNotPublished::PreviewNotSaved,
        login,
        "preview_not_saved",
    )
}

/// Spend the budget, check the login, read the profile, decide, and keep a
/// plan only if one was built. Every guard runs before GitHub is asked.
async fn kept_link_plan(
    app: &App,
    session: &WebSession,
    login: &str,
) -> Result<LinkPlan, LinkPreviewRefusal> {
    if !app.verify_attempts.try_spend(&session.owner_did) {
        return Err(OwnershipRefusal::TooManyAttempts.into());
    }
    if !is_github_login(login) {
        return Err(OwnershipRefusal::AccountNotFound.into());
    }
    let profile = app
        .github
        .read_account(login)
        .await
        .map_err(|error| refusal_of(&error))?;
    let now = unix_now_secs();
    let plan = link_preview(
        &session.owner_did,
        &observation_of(&profile),
        &rfc3339_utc(now),
    )?;
    keep_for_confirm(app.plans.as_ref(), &plan, plan_expires_at(now))?;
    Ok(plan)
}

/// Keep `plan` under its own id for its confirm until `expires_at`.
fn keep_for_confirm(
    plans: &dyn PublishPlanPort,
    plan: &LinkPlan,
    expires_at: i64,
) -> Result<(), LinkPreviewRefusal> {
    plans
        .put_publish_plan(plan.owner_did(), &plan.stored(), expires_at)
        .map_err(|_| LinkPreviewRefusal::NotSaved)
}

/// The fresh read as the link proof sees it.
fn observation_of(profile: &GithubAccountProfile) -> GithubObservation {
    GithubObservation {
        login: profile.login.clone(),
        user_id: profile.id,
        bio: profile.bio.clone(),
    }
}

/// `POST /github/link/publish`: confirm the previewed link. The plan is
/// taken exactly once; a plan that is not the owner's link (an ordinary
/// publish plan, a tampered one) is used up and writes nothing.
pub(crate) async fn publish_link(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Publish) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    let Ok(plan) = restore_link_plan(&confirmed.owner_did, &confirmed.stored) else {
        return not_found();
    };
    let login = plan.login().to_string();
    if confirmed.expired() {
        return not_published(
            StatusCode::GONE,
            &LinkNotPublished::Expired,
            &login,
            "expired",
        );
    }
    match proven_again(app, plan).await {
        Ok(plan) => write_link(app, &plan, &confirmed).await,
        Err((why, label)) => not_published(StatusCode::CONFLICT, &why, &login, label),
    }
}

/// Spend the budget, read the profile again and admit the plan only if that
/// confirm-time read proves exactly its link (C-GDL-10). A spent budget or a
/// failed read is refused as `review_domain` maps it; the plan is not kept.
async fn proven_again(
    app: &App,
    plan: LinkPlan,
) -> Result<LinkPlan, (LinkNotPublished, &'static str)> {
    if !app.verify_attempts.try_spend(plan.owner_did()) {
        return Err(refused(BUDGET_SPENT));
    }
    let profile = app
        .github
        .read_account(plan.login())
        .await
        .map_err(|error| refused(link_refusal_of(&error)))?;
    let fresh = check_link(
        &Did(plan.owner_did().to_string()),
        &observation_of(&profile),
    );
    confirm_link(plan, fresh).map_err(|refusal| (refusal.not_published(), refusal.label()))
}

/// A confirm refused before or by its GitHub read, with its log label.
fn refused(refusal: LinkRefusal) -> (LinkNotPublished, &'static str) {
    let label = refusal.label();
    (LinkNotPublished::Refused(refusal), label)
}

/// The create-only write and its read-back; the page names the record's
/// exact address. A write the host did not take keeps the plan for Retry.
async fn write_link(app: &App, plan: &LinkPlan, confirmed: &ConfirmedPlan) -> Reply {
    match execute_link(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            emit(LogEvent::GithubLinkPublished);
            Reply::page(StatusCode::OK, views::link_published_page(&at_uri))
        }
        Err(failure) => {
            let (why, label) = write_failure(&failure);
            let retry = (keeps_plan_for_retry(&why) && put_back(app, plan, confirmed)).then_some(
                PublishRetry {
                    plan_id: plan.rkey(),
                    csrf_token: &confirmed.csrf_token,
                },
            );
            emit(LogEvent::GithubLinkRefused(label));
            Reply::page(
                StatusCode::BAD_GATEWAY,
                views::link_not_published_page(&why, plan.login(), retry),
            )
        }
    }
}

/// Keep the plan under its own id with the preview's unchanged deadline.
fn put_back(app: &App, plan: &LinkPlan, confirmed: &ConfirmedPlan) -> bool {
    keep_for_confirm(app.plans.as_ref(), plan, confirmed.expires_at).is_ok()
}

fn write_failure(failure: &PublishFailure) -> (LinkNotPublished, &'static str) {
    match failure {
        _ if failure.needs_sign_in() => (LinkNotPublished::SessionExpired, "session_expired"),
        PublishFailure::ReadBackMismatch => {
            (LinkNotPublished::NotAsPreviewed, "read_back_mismatch")
        }
        PublishFailure::NotWritten(_) => (LinkNotPublished::NotWritten, "not_written"),
    }
}

fn not_published(
    status: StatusCode,
    why: &LinkNotPublished,
    login: &str,
    label: &'static str,
) -> Reply {
    emit(LogEvent::GithubLinkRefused(label));
    Reply::page(status, views::link_not_published_page(why, login, None))
}

#[cfg(test)]
mod tests {
    //! U-37: a proven link whose preview could not be kept is refused in its
    //! own words, never as GitHub being unreachable.
    use super::{keep_for_confirm, preview_not_saved};
    use crate::http::Reply;
    use adapter_review_store::{DataKey, ReviewStore};
    use hyper::StatusCode;
    use ports::claim_domain::identity_link::GithubObservation;
    use ports::{PlanKind, PublishPlanPort, ReviewStoreError, StoredPublishPlan, TakenPublishPlan};
    use review_domain::link_plan::{link_preview, LinkPlan, LinkPreviewRefusal};
    use review_domain::views::NOTHING_PUBLISHED;

    const OWNER: &str = "did:plc:priyaraman7777777777777777";
    const LOGIN: &str = "priyaraman";
    const EXPIRES_AT: i64 = 1_700_000_000;

    /// A plan store that keeps nothing.
    struct UnwritablePlans;

    impl PublishPlanPort for UnwritablePlans {
        fn put_publish_plan(
            &self,
            _: &str,
            _: &StoredPublishPlan,
            _: i64,
        ) -> Result<(), ReviewStoreError> {
            Err(ReviewStoreError("disk full".to_string()))
        }

        fn take_publish_plan(
            &self,
            _: &str,
            _: &str,
            _: PlanKind,
        ) -> Result<Option<TakenPublishPlan>, ReviewStoreError> {
            Ok(None)
        }
    }

    fn proven_plan() -> LinkPlan {
        let observation = GithubObservation {
            login: LOGIN.to_string(),
            user_id: 4_210_001,
            bio: Some(format!("my DID: {OWNER}")),
        };
        link_preview(OWNER, &observation, "2026-10-17T09:30:00Z").expect("the bio proves it")
    }

    #[test]
    fn a_kept_preview_is_taken_back_exactly_as_previewed() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).expect("store");
        let plan = proven_plan();

        assert_eq!(keep_for_confirm(&store, &plan, EXPIRES_AT), Ok(()));

        let taken = store
            .take_publish_plan(OWNER, plan.rkey(), PlanKind::Publish)
            .expect("readable")
            .expect("kept");
        assert_eq!(taken.plan, plan.stored());
        assert_eq!(taken.expires_at, EXPIRES_AT);
    }

    #[test]
    fn a_preview_that_could_not_be_kept_is_not_saved_and_never_blames_github() {
        let plan = proven_plan();

        assert_eq!(
            keep_for_confirm(&UnwritablePlans, &plan, EXPIRES_AT),
            Err(LinkPreviewRefusal::NotSaved)
        );

        let Reply::Page { status, html, .. } = preview_not_saved(LOGIN) else {
            panic!("a page, not a redirect");
        };
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(html.contains("We could not save this preview."), "{html}");
        assert!(html.contains(NOTHING_PUBLISHED), "{html}");
        assert!(!html.contains("GitHub could not be reached"), "{html}");
        assert!(!html.contains("disk full"), "{html}");
    }
}
