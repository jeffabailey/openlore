//! `GET /share` and `POST /share`: the opt-in share post (US-BRA-008,
//! I-BRA-6). The preview builds a pure `SharePostPlan` from the owner's
//! claims read LIVE from their own PDS (never from private review state) and
//! keeps it in the take-once plan store; nothing is posted. Only "Post to
//! Bluesky" posts, exactly the confirmed text, once. "Don't post" or
//! leaving simply lets the plan expire. The post text is never logged.

use std::sync::atomic::{AtomicBool, Ordering};

use std::collections::BTreeSet;

use hyper::StatusCode;
use ports::{PlanKind, ResolvedIdentity};
use review_domain::plans::{plan_expires_at, rfc3339_utc};
use review_domain::share::{
    compose_post, quote_permitted, restore_share_plan, SharePostPlan, ShareQuote,
};
use review_domain::views::{self, ProfileContent, PublishedClaim, SharePreview};

use crate::executor::execute_share;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::confirm::{take_confirmed_plan, ConfirmedPlan};
use crate::routes::profile::read_published;
use crate::routes::review::{not_found, AuthorityNow, PublishChecks};
use crate::routes::signin::{csrf_token_for, current_session, field, random_token, to_landing};
use crate::wiring::{observe, LogEvent};

/// What a preview that could not be kept is told.
const NOT_PREPARED_NOTICE: &str =
    "We couldn't prepare your post right now. Nothing was posted; try again shortly.";

/// What a confirm that arrived after its preview's deadline is told.
const EXPIRED_NOTICE: &str =
    "This preview has expired. Nothing was posted; open Share on Bluesky… again.";

/// Open the share preview: the draft, its profile link, and the Post button.
pub(crate) async fn share_preview(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !app.share_allowance.allows(&session.owner_did) {
        return unavailable(StatusCode::TOO_MANY_REQUESTS, views::SHARE_LIMIT_NOTICE);
    }
    let Ok(identity) = app.identity.resolve_did(&session.owner_did).await else {
        return unavailable(
            StatusCode::SERVICE_UNAVAILABLE,
            views::DIRECTORY_UNREACHABLE_NOTICE,
        );
    };
    let ProfileContent::Claims(published) =
        read_published(app.repo_listing.as_ref(), &identity).await
    else {
        return unavailable(
            StatusCode::SERVICE_UNAVAILABLE,
            views::PDS_UNREACHABLE_NOTICE,
        );
    };
    let mut checks = PublishChecks::new(app, &identity.did);
    let quote = quote_now(&published, &mut checks).await;
    let plan = match quote.plan(&profile_url(app, &identity)) {
        Ok(plan) => plan,
        Err(refusal) => return unavailable(StatusCode::OK, &refusal.message()),
    };
    let plan_id = random_token();
    let stored = plan.stored(&identity.did, &plan_id);
    if app
        .plans
        .put_publish_plan(&identity.did, &stored, plan_expires_at(unix_now_secs()))
        .is_err()
    {
        return unavailable(StatusCode::SERVICE_UNAVAILABLE, NOT_PREPARED_NOTICE);
    }
    preview(
        StatusCode::OK,
        &plan,
        &SharePreviewState {
            plan_id: &plan_id,
            csrf_token: &csrf_token_for(&cookie),
            text: plan.draft().text(),
        },
        None,
        quote.not_counted(),
    )
}

/// The share quote (P-OOS-2): each distinct repo of `published` checked
/// once, in this request; a claim is quoted only under a permit, so a read
/// GitHub could not answer leaves it not counted. A preview publishes
/// nothing, so no decision is recorded as a publish check (U-OOS-D34).
async fn quote_now(published: &[PublishedClaim], authority: &mut impl AuthorityNow) -> ShareQuote {
    let subjects: BTreeSet<&str> = published
        .iter()
        .map(|claim| claim.subject.as_str())
        .collect();
    let mut permits = Vec::new();
    for subject in subjects {
        if let Ok(permit) = authority.decide(subject).await {
            permits.push(permit);
        }
    }
    quote_permitted(published, &permits)
}

/// "Post to Bluesky": post exactly the confirmed text of a previewed plan.
pub(crate) async fn confirm_share(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Share) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    if confirmed.expired() {
        return unavailable(StatusCode::GONE, EXPIRED_NOTICE);
    }
    let Ok(plan) = restore_share_plan(&confirmed.owner_did, &confirmed.stored) else {
        return not_found();
    };
    let Some(spent_at) = app.share_allowance.try_spend(&confirmed.owner_did) else {
        return unavailable(StatusCode::TOO_MANY_REQUESTS, views::SHARE_LIMIT_NOTICE);
    };
    let share = Share {
        app,
        plan: &plan,
        confirmed: &confirmed,
        text: &field(&request.form, "text").unwrap_or_default(),
        posted: AtomicBool::new(false),
    };
    let reply = share.post().await;
    if !share.posted() {
        app.share_allowance.refund(&confirmed.owner_did, spent_at);
    }
    reply
}

/// A taken, fresh share plan with the owner's confirmed text.
struct Share<'a> {
    app: &'a App,
    plan: &'a SharePostPlan,
    confirmed: &'a ConfirmedPlan,
    text: &'a str,
    /// Did the post land? (A share that did not is refunded.)
    posted: AtomicBool,
}

impl Share<'_> {
    fn posted(&self) -> bool {
        self.posted.load(Ordering::Relaxed)
    }

    async fn post(&self) -> Reply {
        let post = match compose_post(self.text, self.plan.profile_url()) {
            Ok(post) => post,
            Err(refusal) => return self.not_posted(StatusCode::OK, &refusal.message()),
        };
        let created_at = rfc3339_utc(unix_now_secs());
        let owner_did = &self.confirmed.owner_did;
        match execute_share(self.app.repo_write.as_ref(), owner_did, &post, &created_at).await {
            Ok(created) => {
                self.posted.store(true, Ordering::Relaxed);
                observe(self.app, LogEvent::SharePosted);
                Reply::page(
                    StatusCode::OK,
                    views::share_posted_page(&created.uri, &profile_path(self.plan)),
                )
            }
            Err(failure) => self.not_posted(
                StatusCode::BAD_GATEWAY,
                &views::share_failed_notice(failure.message(), failure.failed_write()),
            ),
        }
    }

    /// Nothing was posted: the plan goes back (same deadline) and the
    /// preview is shown again with the owner's text and why.
    fn not_posted(&self, status: StatusCode, notice: &str) -> Reply {
        let confirmed = self.confirmed;
        let stored = self.plan.stored(&confirmed.owner_did, &confirmed.plan_id);
        let _ =
            self.app
                .plans
                .put_publish_plan(&confirmed.owner_did, &stored, confirmed.expires_at);
        preview(
            status,
            self.plan,
            &SharePreviewState {
                plan_id: &confirmed.plan_id,
                csrf_token: &confirmed.csrf_token,
                text: self.text,
            },
            Some(notice),
            &[],
        )
    }
}

/// What the preview form re-submits: the plan, the token and the text.
struct SharePreviewState<'a> {
    plan_id: &'a str,
    csrf_token: &'a str,
    text: &'a str,
}

fn preview(
    status: StatusCode,
    plan: &SharePostPlan,
    state: &SharePreviewState<'_>,
    notice: Option<&str>,
    not_counted: &[PublishedClaim],
) -> Reply {
    Reply::page(
        status,
        views::share_preview_page_listing(
            &SharePreview {
                plan_id: state.plan_id,
                csrf_token: state.csrf_token,
                text: state.text,
                profile_url: plan.profile_url(),
                profile_path: &profile_path(plan),
                notice,
            },
            not_counted,
        ),
    )
}

fn unavailable(status: StatusCode, reason: &str) -> Reply {
    Reply::page(status, views::share_unavailable_page(reason))
}

/// The owner's public profile on this app: `<origin>/@<handle>`.
fn profile_url(app: &App, identity: &ResolvedIdentity) -> String {
    format!(
        "{}/@{}",
        app.origin.trim_end_matches('/'),
        identity.verified_handle
    )
}

/// The same-origin path of the plan's profile link.
fn profile_path(plan: &SharePostPlan) -> String {
    plan.profile_url()
        .rfind("/@")
        .map(|at| plan.profile_url()[at..].to_string())
        .unwrap_or_else(|| "/review".to_string())
}

#[cfg(test)]
mod tests {
    //! Ruling L1 (P-OOS-2): a quoted repo whose repo or bio read GitHub
    //! could not finish is listed "not counted", never quoted, and nothing
    //! is posted (the preview holds no write).
    use std::collections::BTreeMap;

    use super::{quote_now, AuthorityNow};
    use ports::claim_domain::authority::{
        may_sign, OwnerBioRead, RepoRead, SigningPermit, SigningRefusal,
    };
    use ports::{RepoFacts, RepoOwnerKind};
    use review_domain::share::NOT_COUNTED;
    use review_domain::views::{self, PublishedClaim, SharePreview};

    const OWNER: &str = "did:plc:priyaraman7777777777777777";
    const OWNER_ACCOUNT: u64 = 4_210_001;
    const PROFILE: &str = "https://app.example/@priyaraman.bsky.social";

    /// What GitHub answers in this request, per repo: the repo read and
    /// its owner's bio read.
    #[derive(Clone, Copy)]
    enum Answer {
        Holds,
        RepoUnfinished,
        BioUnfinished,
    }

    struct GithubNow {
        answers: BTreeMap<&'static str, Answer>,
        asked: Vec<String>,
        recorded: std::cell::Cell<usize>,
    }

    impl AuthorityNow for GithubNow {
        fn record(&self, _checked: &Result<SigningPermit, SigningRefusal>) {
            self.recorded.set(self.recorded.get() + 1);
        }

        async fn decide(&mut self, subject: &str) -> Result<SigningPermit, SigningRefusal> {
            self.asked.push(subject.to_string());
            let facts = RepoFacts {
                repo_id: 1,
                full_name: subject.trim_start_matches("github:").to_string(),
                owner_id: OWNER_ACCOUNT,
                owner_login: "priyaraman".to_string(),
                owner_kind: RepoOwnerKind::User,
                fork: false,
                archived: false,
            };
            let (repo, bio) = match self.answers[subject] {
                Answer::Holds => (RepoRead::Found(&facts), OwnerBioRead::Bio(Some(OWNER))),
                Answer::RepoUnfinished => (RepoRead::Unfinished, OwnerBioRead::Bio(Some(OWNER))),
                Answer::BioUnfinished => (RepoRead::Found(&facts), OwnerBioRead::Unfinished),
            };
            may_sign(OWNER, subject, &[OWNER_ACCOUNT], repo, bio)
        }
    }

    fn claim(repo: &str, slug: &str) -> PublishedClaim {
        PublishedClaim {
            subject: format!("github:{repo}"),
            object: format!("org.openlore.philosophy.{slug}"),
            confidence_bp: 2500,
            rkey: format!("baf{slug}"),
        }
    }

    #[tokio::test]
    async fn share_preview_lists_a_claim_as_not_counted_when_github_cannot_answer() {
        let tidepool = claim("priyaraman/tidepool", "test-driven");
        let kelp = claim("priyaraman/kelp", "dependency-pinning");
        let reef = claim("priyaraman/reef", "memory-safety");
        let published = vec![
            tidepool.clone(),
            kelp.clone(),
            reef.clone(),
            tidepool.clone(),
        ];
        let mut github = GithubNow {
            answers: BTreeMap::from([
                ("github:priyaraman/tidepool", Answer::Holds),
                ("github:priyaraman/kelp", Answer::RepoUnfinished),
                ("github:priyaraman/reef", Answer::BioUnfinished),
            ]),
            asked: Vec::new(),
            recorded: std::cell::Cell::new(0),
        };

        let quote = quote_now(&published, &mut github).await;

        assert_eq!(github.asked.len(), 3, "one check per distinct repo");
        assert_eq!(quote.quoted(), [tidepool.clone(), tidepool]);
        assert_eq!(quote.not_counted(), [kelp, reef]);
        let plan = quote.plan(PROFILE).expect("a draft is offered");
        let draft = plan.draft().text();
        assert!(draft.contains("test-driven"), "{draft}");
        assert!(!draft.contains("dependency-pinning"), "{draft}");
        assert!(!draft.contains("memory-safety"), "{draft}");
        let page = views::share_preview_page_listing(
            &SharePreview {
                plan_id: "p1",
                csrf_token: "csrf",
                text: draft,
                profile_url: plan.profile_url(),
                profile_path: "/@priyaraman.bsky.social",
                notice: None,
            },
            quote.not_counted(),
        );
        for repo in ["priyaraman/kelp", "priyaraman/reef"] {
            assert!(page.contains(&format!("{repo} — ")), "{repo}: {page}");
        }
        assert_eq!(page.matches(NOT_COUNTED).count(), 2, "{page}");
    }

    /// U-OOS-D34: the share preview decides each repo's permit but records
    /// no publish check: nothing is published, so nothing is audited as one.
    #[tokio::test]
    async fn share_preview_records_no_publish_check() {
        let published = vec![
            claim("priyaraman/tidepool", "test-driven"),
            claim("priyaraman/kelp", "dependency-pinning"),
        ];
        let mut github = GithubNow {
            answers: BTreeMap::from([
                ("github:priyaraman/tidepool", Answer::Holds),
                ("github:priyaraman/kelp", Answer::RepoUnfinished),
            ]),
            asked: Vec::new(),
            recorded: std::cell::Cell::new(0),
        };

        let quote = quote_now(&published, &mut github).await;

        assert_eq!(github.asked.len(), 2, "each repo is decided");
        assert_eq!(quote.quoted().len(), 1, "the permitted claim is quoted");
        assert_eq!(github.recorded.get(), 0, "no publish check is recorded");
    }
}
