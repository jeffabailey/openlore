//! The private queue and its per-suggestion actions (US-BRA-003/004/006).
//! Every read and write is scoped to the signed-in DID (I-BRA-1): a
//! suggestion named by anyone else's form is simply not found.

use hyper::StatusCode;
use std::collections::BTreeMap;

use ports::claim_domain::authority::{
    may_sign, owner_to_read, OwnerBioRead, RepoRead, SigningPermit, SigningRefusal,
};
use ports::{
    GithubError, GithubLink, GithubLinkPort, ImportedReposRead, KpiTally, RepoFacts, RepoSource,
    ReviewStateRead, Suggestion, SuggestionKey, WebSession,
};
use review_domain::edits::{edit_claim, philosophy_choices};
use review_domain::kpi::ActedOrigin;
use review_domain::lifecycle::{
    owner_step, tally, visible_and_approvable, LifecycleEvent, OwnerStep, Tally,
};
use review_domain::plans::{
    plan_expires_at, publish_edited_plan, publish_plan, rfc3339_utc, ClaimDraft, PlanError,
    PublishPlan,
};
use review_domain::repo_scope::{visible, Standing};
use review_domain::selection::{chosen_suggestions, SELECTION_FIELD};
use review_domain::views::{self, AccountChecked, EditView, QueueNotice, QueueView, ScanDetails};

use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::github::github_step_of;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, field, fields, signed_in_post, to_landing,
};
use crate::wiring::{emit, observe, Counted, LogEvent};

/// `GET /review`: the signed-in queue.
pub(crate) fn review(app: &App, request: &PageRequest) -> Reply {
    match current_session(app, request) {
        Some((cookie_value, session)) => {
            queue_page(app, &cookie_value, &session, StatusCode::OK, None)
        }
        None => to_landing(),
    }
}

/// The owner's queue page; pending suggestions show only while the GitHub
/// link is verified (D-12: hidden, never deleted).
pub(crate) fn queue_page(
    app: &App,
    cookie_value: &str,
    session: &WebSession,
    status: StatusCode,
    notice: Option<QueueNotice<'_>>,
) -> Reply {
    let OwnerQueue {
        link,
        pending,
        tally,
    } = owner_queue(
        app.links.as_ref(),
        app.review_read.as_ref(),
        app.imports_read.as_ref(),
        &session.owner_did,
    );
    let latest_scan = app.scans.latest_scan(&session.owner_did).ok().flatten();
    let skipped = app
        .scan_plans
        .latest_skipped(&session.owner_did)
        .unwrap_or_default();
    let accounts = app
        .links
        .github_accounts(&session.owner_did)
        .unwrap_or_default();
    let checks = app
        .scan_plans
        .latest_account_checks(&session.owner_did)
        .unwrap_or_default();
    let account_checks: Vec<AccountChecked<'_>> = checks
        .iter()
        .filter_map(|check| {
            login_of(&accounts, check.github_user_id).map(|login| AccountChecked {
                login,
                outcome: check.outcome,
            })
        })
        .collect();
    let sources = sources_of(app.review_read.as_ref(), &session.owner_did, &accounts);
    let card_imported: Vec<SuggestionKey> = pending
        .iter()
        .filter(|suggestion| matches!(sources.get(&suggestion.key), Some(RepoSource::Imported(_))))
        .map(|suggestion| suggestion.key.clone())
        .collect();
    let imports_listed = app
        .imports_read
        .imports(&session.owner_did)
        .is_ok_and(|imports| !imports.is_empty());
    let card_accounts: Vec<(SuggestionKey, &str)> = pending
        .iter()
        .filter_map(|suggestion| {
            let source = sources.get(&suggestion.key)?;
            match source {
                RepoSource::Account(id) => login_of(&accounts, *id),
                RepoSource::Imported(_) => None,
            }
            .map(|login| (suggestion.key.clone(), login))
        })
        .collect();
    let csrf_token = csrf_token_for(cookie_value);
    Reply::page(
        status,
        views::review_page_of_scan(
            &QueueView {
                handle: &session.handle,
                csrf_token: &csrf_token,
                github: github_step_of(link.as_ref()),
                latest_scan,
                pending: &pending,
                tally,
                notice,
            },
            &ScanDetails {
                skipped: &skipped,
                account_checks: &account_checks,
                card_accounts: &card_accounts,
                card_imported: &card_imported,
                imports_listed,
            },
        ),
    )
}

/// The login the listed account `github_user_id` is shown under.
fn login_of(accounts: &[GithubLink], github_user_id: u64) -> Option<&str> {
    accounts
        .iter()
        .find(|account| account.github_user_id == github_user_id)
        .map(|account| account.github_login.as_str())
}

/// Where each of the owner's pending suggestions came from. One stored
/// without a source came from her first account (the one link a scan read
/// before several accounts).
fn sources_of(
    review_read: &dyn ReviewStateRead,
    owner_did: &str,
    accounts: &[GithubLink],
) -> BTreeMap<SuggestionKey, RepoSource> {
    let mut sources: BTreeMap<SuggestionKey, RepoSource> = review_read
        .pending_sources(owner_did)
        .unwrap_or_default()
        .into_iter()
        .collect();
    if let Some(first) = accounts.first() {
        for suggestion in review_read
            .pending_suggestions(owner_did)
            .unwrap_or_default()
        {
            sources
                .entry(suggestion.key)
                .or_insert(RepoSource::Account(first.github_user_id));
        }
    }
    sources
}

/// The owner's pending suggestions she sees now (CORE-RRA-7): those whose
/// source account is listed and verified, or whose imported repo is listed.
/// The rest are hidden, not deleted.
fn visible_pending(
    review_read: &dyn ReviewStateRead,
    owner_did: &str,
    accounts: &[GithubLink],
    imports: &[u64],
) -> Vec<Suggestion> {
    let pending = review_read
        .pending_suggestions(owner_did)
        .unwrap_or_default();
    let sources = sources_of(review_read, owner_did, accounts);
    let (kept, origins): (Vec<Suggestion>, Vec<RepoSource>) = pending
        .into_iter()
        .filter_map(|suggestion| {
            let source = *sources.get(&suggestion.key)?;
            Some((suggestion, source))
        })
        .unzip();
    let standings: Vec<(u64, Standing)> = accounts
        .iter()
        .map(|account| {
            let standing = if account.verified {
                Standing::Verified
            } else {
                Standing::Unverified
            };
            (account.github_user_id, standing)
        })
        .collect();
    kept.into_iter()
        .zip(visible(&origins, &standings, imports))
        .filter_map(|(suggestion, shown)| shown.then_some(suggestion))
        .collect()
}

/// What the owner's queue page lists: the GitHub link, the pending
/// suggestions (while that link is verified, or she lists imports) and
/// their tally.
pub(crate) struct OwnerQueue {
    pub(crate) link: Option<GithubLink>,
    pub(crate) pending: Vec<Suggestion>,
    pub(crate) tally: Tally,
}

/// The queue composition `queue_page` renders, read from the owner's links
/// and review state.
pub(crate) fn owner_queue(
    links: &dyn GithubLinkPort,
    review_read: &dyn ReviewStateRead,
    imports_read: &dyn ImportedReposRead,
    owner_did: &str,
) -> OwnerQueue {
    let accounts = links.github_accounts(owner_did).unwrap_or_default();
    let imports: Vec<u64> = imports_read
        .imports(owner_did)
        .unwrap_or_default()
        .iter()
        .map(|import| import.repo_id)
        .collect();
    let link = accounts.first().cloned();
    let shows_queue = link.as_ref().is_some_and(|link| link.verified) || !imports.is_empty();
    let pending = if shows_queue {
        visible_pending(review_read, owner_did, &accounts, &imports)
    } else {
        Vec::new()
    };
    let tally = tally(
        shows_queue
            .then(|| review_read.suggestion_states(owner_did).ok())
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, state)| state),
    );
    OwnerQueue {
        link,
        pending,
        tally,
    }
}

/// `POST /review/approve`: the exact-record preview of one of the owner's
/// visible pending suggestions. Builds and keeps the publish plan (the
/// Plan-value: what is shown is what a confirm writes); writes nothing to
/// any repo.
pub(crate) async fn approve(app: &App, request: &PageRequest) -> Reply {
    let (session, suggestion) = match own_suggestion(app, request, "object") {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    let subject = suggestion.key.subject.clone();
    let mut checks = PublishChecks::new(app, &session.owner_did);
    match checks.permit(&subject).await {
        Ok(permit) => {
            let plan = publish_plan(&permit, &draft_of(suggestion), &composed_now());
            keep_and_preview(app, request, &session, plan)
        }
        Err(refusal) => checks.refused(&subject, refusal),
    }
}

/// `POST /review/approve-selected`: the batch preview of every ticked card
/// that names one of the owner's visible pending suggestions (at most
/// `MAX_SELECTED`). One plan is built and kept per suggestion, exactly as a
/// single Approve keeps one; nothing is written to any repo. A selection
/// naming nothing of the owner's goes back to the queue.
pub(crate) async fn approve_selected(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let pending = owner_pending(app, &session.owner_did);
    let values = fields(&request.form, SELECTION_FIELD);
    let composed_at = composed_now();
    let mut checks = PublishChecks::new(app, &session.owner_did);
    let mut kept: Vec<PublishPlan> = Vec::new();
    for suggestion in chosen_suggestions(pending, &values) {
        let permit = match checks.permit(&suggestion.key.subject).await {
            Ok(permit) => permit,
            Err(refusal) => {
                checks.settle(refusal);
                continue;
            }
        };
        let plan = publish_plan(&permit, &draft_of(suggestion), &composed_at).ok();
        kept.extend(plan.filter(|plan| keep_plan(app, &session, plan)));
    }
    if kept.is_empty() {
        return Reply::redirect("/review");
    }
    Reply::page(
        StatusCode::OK,
        views::batch_preview_page(&kept, &csrf_token_of(request)),
    )
}

/// Keep `plan` until its preview deadline; false if it could not be kept.
fn keep_plan(app: &App, session: &WebSession, plan: &PublishPlan) -> bool {
    app.plans
        .put_publish_plan(
            &session.owner_did,
            &plan.stored(),
            plan_expires_at(unix_now_secs()),
        )
        .is_ok()
}

/// `POST /review/edit`: the edit form of one of the owner's visible pending
/// suggestions, prefilled with the suggestion (US-BRA-005). Writes nothing.
pub(crate) fn edit(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, "object", |_, suggestion| {
        let typed = views::confidence_text(suggestion.confidence_bp);
        edit_form(
            request,
            &suggestion,
            &suggestion.key.object,
            &typed,
            None,
            StatusCode::OK,
        )
    })
}

/// `POST /review/edit/preview`: the exact-record preview of the owner's
/// edit. A refused confidence re-shows the form with guidance and no way to
/// publish; a valid edit becomes a new plan whose record is exactly the
/// edit (its CID is the record key). Writes nothing to any repo.
pub(crate) async fn preview_edit(app: &App, request: &PageRequest) -> Reply {
    let (session, suggestion) = match own_suggestion(app, request, "suggested_object") {
        Ok(found) => found,
        Err(refused) => return refused,
    };
    let chosen = field(&request.form, "object");
    let typed = field(&request.form, "confidence").unwrap_or_default();
    match edit_claim(&suggestion.key.object, chosen.as_deref(), &typed) {
        Ok(edit) => {
            let subject = suggestion.key.subject.clone();
            let mut checks = PublishChecks::new(app, &session.owner_did);
            match checks.permit(&subject).await {
                Ok(permit) => {
                    let draft = draft_of(suggestion);
                    let plan = publish_edited_plan(&permit, &draft, &edit, &composed_now());
                    keep_and_preview(app, request, &session, plan)
                }
                Err(refusal) => checks.refused(&subject, refusal),
            }
        }
        Err(invalid) => {
            let chosen = chosen.unwrap_or_else(|| suggestion.key.object.clone());
            let guidance = invalid.to_string();
            edit_form(
                request,
                &suggestion,
                &chosen,
                &typed,
                Some(&guidance),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
    }
}

fn edit_form(
    request: &PageRequest,
    suggestion: &Suggestion,
    chosen: &str,
    typed_confidence: &str,
    guidance: Option<&str>,
    status: StatusCode,
) -> Reply {
    let choices = philosophy_choices(&suggestion.key.object);
    Reply::page(
        status,
        views::edit_page(&EditView {
            suggestion,
            csrf_token: &csrf_token_of(request),
            choices: &choices,
            chosen,
            typed_confidence,
            guidance,
        }),
    )
}

fn draft_of(suggestion: Suggestion) -> ClaimDraft {
    ClaimDraft {
        key: suggestion.key,
        evidence: suggestion.evidence,
        confidence_bp: suggestion.confidence_bp,
    }
}

fn composed_now() -> String {
    rfc3339_utc(unix_now_secs())
}

/// Keep a built plan until its deadline and show its exact-record preview;
/// a plan that cannot be built or kept is not found.
fn keep_and_preview(
    app: &App,
    request: &PageRequest,
    session: &WebSession,
    plan: Result<PublishPlan, PlanError>,
) -> Reply {
    let kept = plan.ok().filter(|plan| keep_plan(app, session, plan));
    match kept {
        Some(plan) => Reply::page(
            StatusCode::OK,
            views::approval_preview_page(&plan, &csrf_token_of(request)),
        ),
        None => not_found(),
    }
}

/// The last verdict stored when a publish found the DID gone from the bio.
const BIO_NO_LONGER_HOLDS_VERDICT: &str = "did_missing";

/// The fresh reads one request makes to mint signing permits (ADR-089 §4):
/// the repo as GitHub serves it now and its owner's bio now. Shared between
/// suggestions of the same repo or owner account within the request; never
/// kept across requests.
pub(crate) struct PublishChecks<'a> {
    app: &'a App,
    owner_did: &'a str,
    accounts: Vec<GithubLink>,
    listed: Vec<u64>,
    repos: BTreeMap<String, RepoAnswer>,
    bios: BTreeMap<u64, BioAnswer>,
}

#[derive(Clone)]
enum RepoAnswer {
    Found(RepoFacts),
    Gone,
    Unfinished,
}

impl RepoAnswer {
    fn as_read(&self) -> RepoRead<'_> {
        match self {
            Self::Found(facts) => RepoRead::Found(facts),
            Self::Gone => RepoRead::Gone,
            Self::Unfinished => RepoRead::Unfinished,
        }
    }
}

#[derive(Clone)]
enum BioAnswer {
    Bio(Option<String>),
    Unfinished,
}

impl BioAnswer {
    fn as_read(&self) -> OwnerBioRead<'_> {
        match self {
            Self::Bio(bio) => OwnerBioRead::Bio(bio.as_deref()),
            Self::Unfinished => OwnerBioRead::Unfinished,
        }
    }
}

impl<'a> PublishChecks<'a> {
    pub(crate) fn new(app: &'a App, owner_did: &'a str) -> Self {
        let accounts = app.links.github_accounts(owner_did).unwrap_or_default();
        let listed = accounts
            .iter()
            .map(|account| account.github_user_id)
            .collect();
        Self {
            app,
            owner_did,
            accounts,
            listed,
            repos: BTreeMap::new(),
            bios: BTreeMap::new(),
        }
    }

    /// A permit for `subject` from this request's reads, or why not; each
    /// check is audited before anything can be written (KPI-RRA-4).
    pub(crate) async fn permit(&mut self, subject: &str) -> Result<SigningPermit, SigningRefusal> {
        let repo = self.repo(subject).await;
        let bio = match owner_to_read(&self.listed, repo.as_read()) {
            Some(owner_id) => self.bio(owner_id).await,
            None => BioAnswer::Unfinished,
        };
        let checked = may_sign(
            self.owner_did,
            subject,
            &self.listed,
            repo.as_read(),
            bio.as_read(),
        );
        emit(match &checked {
            Ok(_) => LogEvent::PublishCheckPassed,
            Err(refusal) => LogEvent::PublishCheckRefused(*refusal),
        });
        checked
    }

    /// A refusal's consequences, nothing written: counted, and a listed
    /// account whose bio no longer holds the DID is marked unverified (an
    /// unfinished check changes no status).
    pub(crate) fn settle(&self, refusal: SigningRefusal) {
        observe(self.app, LogEvent::PublishRefused(refusal));
        if let SigningRefusal::BioNoLongerHolds(github_user_id) = refusal {
            let _ = self.app.account_status.mark_account_unverified(
                self.owner_did,
                github_user_id,
                BIO_NO_LONGER_HOLDS_VERDICT,
            );
        }
    }

    /// A publish refused for want of a permit, settled, and explained with
    /// its next step; a repo gone (missing or private) is not found.
    pub(crate) fn refused(&self, subject: &str, refusal: SigningRefusal) -> Reply {
        self.settle(refusal);
        match refusal {
            SigningRefusal::NotOwnedByListedAccount | SigningRefusal::OrgOwned
                if self.imported(subject) =>
            {
                Reply::page(StatusCode::CONFLICT, views::not_yours_page(subject))
            }
            SigningRefusal::NotOwnedByListedAccount | SigningRefusal::OrgOwned => {
                Reply::page(StatusCode::CONFLICT, views::not_owned_page(subject))
            }
            SigningRefusal::Unfinished => Reply::page(
                StatusCode::SERVICE_UNAVAILABLE,
                views::github_unreachable_page(subject),
            ),
            SigningRefusal::BioNoLongerHolds(github_user_id) => {
                match self.login_of(github_user_id) {
                    Some(login) => {
                        Reply::page(StatusCode::CONFLICT, views::bio_no_longer_holds_page(login))
                    }
                    None => not_found(),
                }
            }
            SigningRefusal::RepoGone => not_found(),
        }
    }

    fn login_of(&self, github_user_id: u64) -> Option<&str> {
        login_of(&self.accounts, github_user_id)
    }

    /// Did the owner's pending suggestions about `subject` come from a repo
    /// she imported?
    fn imported(&self, subject: &str) -> bool {
        sources_of(
            self.app.review_read.as_ref(),
            self.owner_did,
            &self.accounts,
        )
        .iter()
        .any(|(key, source)| key.subject == subject && matches!(source, RepoSource::Imported(_)))
    }

    async fn repo(&mut self, subject: &str) -> RepoAnswer {
        if let Some(known) = self.repos.get(subject) {
            return known.clone();
        }
        let named = subject
            .strip_prefix("github:")
            .and_then(|repo| repo.split_once('/'));
        let answer = match named {
            None => RepoAnswer::Gone,
            Some((owner, name)) => match self.app.github.read_repo(owner, name).await {
                Ok(facts) => RepoAnswer::Found(facts),
                Err(GithubError::NotFound { .. } | GithubError::NotPublic { .. }) => {
                    RepoAnswer::Gone
                }
                Err(_) => RepoAnswer::Unfinished,
            },
        };
        self.repos.insert(subject.to_string(), answer.clone());
        answer
    }

    async fn bio(&mut self, owner_id: u64) -> BioAnswer {
        if let Some(known) = self.bios.get(&owner_id) {
            return known.clone();
        }
        let answer = match self.app.github.read_person_by_id(owner_id).await {
            Ok(profile) => BioAnswer::Bio(profile.bio.clone()),
            Err(_) => BioAnswer::Unfinished,
        };
        self.bios.insert(owner_id, answer.clone());
        answer
    }
}

/// May the owner publish the suggestion `key` right now? Only while it is
/// pending and their GitHub link is verified (CORE-9).
pub(crate) fn owner_can_publish(app: &App, owner_did: &str, key: &SuggestionKey) -> bool {
    let verified = link_verified(app, owner_did);
    app.review_read
        .suggestion_states(owner_did)
        .is_ok_and(|states| {
            states
                .iter()
                .any(|(k, state)| k == key && visible_and_approvable(*state, verified))
        })
}

/// `POST /review/decline`: "Not me" — the suggestion leaves the queue,
/// declined, and is never offered again. Private app state only: this path
/// holds no repo-write capability (I-BRA-2), and declining twice is the
/// same as declining once.
pub(crate) fn decline_suggestion(app: &App, request: &PageRequest) -> Reply {
    with_own_step(
        app,
        request,
        LifecycleEvent::Decline,
        |cookie_value, session, key, _| {
            queue_page(
                app,
                cookie_value,
                session,
                StatusCode::OK,
                Some(QueueNotice::Declined(key)),
            )
        },
    )
}

/// `POST /review/undo`: a declined suggestion is pending again.
pub(crate) fn undo_decline(app: &App, request: &PageRequest) -> Reply {
    with_own_step(app, request, LifecycleEvent::Undo, |_, _, _, _| {
        Reply::redirect("/review")
    })
}

/// Take the owner's `event` on the suggestion the form names, if it is the
/// signed-in owner's and their GitHub link is verified; `done` gets whether
/// the suggestion moved (`false`: it already stood where `event` leaves it).
/// A decline that moves is logged and counted in the move's transaction.
/// A refused event, or anyone else's suggestion, is not found.
fn with_own_step(
    app: &App,
    request: &PageRequest,
    event: LifecycleEvent,
    done: impl FnOnce(&str, &WebSession, &SuggestionKey, bool) -> Reply,
) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let Some(key) = key_of(request, "object").filter(|_| queue_shown(app, &session.owner_did))
    else {
        return not_found();
    };
    let current = app
        .review_read
        .suggestion_states(&session.owner_did)
        .ok()
        .and_then(|states| states.into_iter().find(|(k, _)| *k == key))
        .map(|(_, state)| state);
    let logged = (event == LifecycleEvent::Decline).then(|| LogEvent::SuggestionDeclined {
        origin: acted_origin(app.review_read.as_ref(), &session.owner_did, &key),
    });
    let counted = logged.as_ref().map(Counted::of);
    let tally = counted.as_ref().map_or(KpiTally::NONE, Counted::tally);
    let moved = match current.and_then(|state| owner_step(state, event)) {
        Some(OwnerStep::Move { from, to }) => app
            .review_write
            .change_state(&session.owner_did, &key, from, to, tally)
            .ok(),
        Some(OwnerStep::AlreadyDone) => Some(false),
        None => None,
    };
    if let (Some(true), Some(event)) = (moved, logged) {
        emit(event);
    }
    match moved {
        Some(moved) => done(&cookie_value, &session, &key, moved),
        None => not_found(),
    }
}

/// Where the owner's suggestion `key` came from, as KPI-RRA-1 counts an
/// action on it: an imported repo, or her accounts' repos inside or outside
/// today's default (an unanswered read counts as the default).
fn acted_origin(
    review_read: &dyn ReviewStateRead,
    owner_did: &str,
    key: &SuggestionKey,
) -> ActedOrigin {
    let imported = review_read
        .pending_sources(owner_did)
        .unwrap_or_default()
        .iter()
        .any(|(pending, source)| pending == key && matches!(source, RepoSource::Imported(_)));
    if imported {
        return ActedOrigin::Imported;
    }
    match review_read.outside_default(owner_did, key).ok().flatten() {
        Some(outside_default) => ActedOrigin::Outside(outside_default),
        None => ActedOrigin::Default,
    }
}

/// The owner's pending suggestions she sees now, exactly as her queue lists
/// them (hidden ones are not hers to act on until their account is
/// verified again).
fn owner_pending(app: &App, owner_did: &str) -> Vec<Suggestion> {
    owner_queue(
        app.links.as_ref(),
        app.review_read.as_ref(),
        app.imports_read.as_ref(),
        owner_did,
    )
    .pending
}

/// Does the owner see a queue: her GitHub link is verified, or she lists
/// imported repos (whose cards she can still decline)?
fn queue_shown(app: &App, owner_did: &str) -> bool {
    link_verified(app, owner_did)
        || app
            .imports_read
            .imports(owner_did)
            .is_ok_and(|imports| !imports.is_empty())
}

/// Is the owner's GitHub link currently verified?
fn link_verified(app: &App, owner_did: &str) -> bool {
    app.links
        .github_link(owner_did)
        .ok()
        .flatten()
        .is_some_and(|link| link.verified)
}

/// Run `act` on the visible pending suggestion the form names (its object
/// in `object_field`), if it is the signed-in owner's; anything else is not
/// found.
fn with_own_suggestion(
    app: &App,
    request: &PageRequest,
    object_field: &str,
    act: impl FnOnce(&WebSession, Suggestion) -> Reply,
) -> Reply {
    match own_suggestion(app, request, object_field) {
        Ok((session, suggestion)) => act(&session, suggestion),
        Err(refused) => refused,
    }
}

/// The signed-in owner's visible pending suggestion the form names (its
/// object in `object_field`); anything else is not found.
fn own_suggestion(
    app: &App,
    request: &PageRequest,
    object_field: &str,
) -> Result<(WebSession, Suggestion), Reply> {
    let (_, session) = signed_in_post(app, request)?;
    let named = key_of(request, object_field);
    let found = named.and_then(|key| {
        owner_pending(app, &session.owner_did)
            .into_iter()
            .find(|suggestion| suggestion.key == key)
    });
    found
        .map(|suggestion| (session, suggestion))
        .ok_or_else(not_found)
}

/// The suggestion key a form names, its object in `object_field`.
fn key_of(request: &PageRequest, object_field: &str) -> Option<SuggestionKey> {
    Some(SuggestionKey {
        subject: field(&request.form, "subject")?,
        predicate: field(&request.form, "predicate")?,
        object: field(&request.form, object_field)?,
    })
}

pub(crate) fn not_found() -> Reply {
    Reply::page(StatusCode::NOT_FOUND, views::not_found_page())
}

#[cfg(test)]
mod tests {
    //! C-6 (U-37): a stored link plan never appears in the review queue. The
    //! queue is composed by `owner_queue`, the function `queue_page` renders.
    use super::{acted_origin, owner_queue, OwnerQueue};
    use crate::routes::github::github_step_of;
    use adapter_review_store::{DataKey, ReviewStore};
    use ports::claim_domain::identity_link::{link_predicate, GithubObservation};
    use ports::{
        AccountAdmission, AccountAdmitted, GithubAccountsWrite, ImportedRepo, ImportedReposWrite,
        KpiTally, OutsideDefault, PublishPlanPort, ReviewStateWrite, Suggestion, SuggestionKey,
    };
    use review_domain::kpi::ActedOrigin;
    use review_domain::lifecycle::tally;
    use review_domain::link_plan::link_preview;
    use review_domain::views::{self, QueueView};

    const OWNER: &str = "did:plc:priyaraman7777777777777777";
    const LINKED_LOGIN: &str = "priyaraman";
    const LINKED_ID: u64 = 4_210_001;

    fn pending_suggestion() -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: "github:tideworks/tidepool".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.memory-safety".into(),
            },
            confidence_bp: 2500,
            evidence: vec![],
            why: vec![],
            source_repo: "tideworks/tidepool".into(),
        }
    }

    #[test]
    fn a_stored_link_plan_never_appears_in_the_review_queue() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).expect("store");
        assert_eq!(
            store
                .add_or_reverify(OWNER, LINKED_LOGIN, LINKED_ID, 3, KpiTally::NONE)
                .expect("linked"),
            AccountAdmission::Admitted(AccountAdmitted::Added)
        );
        store
            .add_pending_from_listed_source(
                OWNER,
                &ports::SuggestionSource {
                    account_id: LINKED_ID,
                    repo_id: 1,
                    outside_default: None,
                },
                &[pending_suggestion()],
            )
            .expect("queued");
        let observation = GithubObservation {
            login: LINKED_LOGIN.to_string(),
            user_id: LINKED_ID,
            bio: Some(format!("my DID: {OWNER}")),
        };
        let plan = link_preview(OWNER, &observation, "2026-10-17T09:30:00Z").expect("proven");

        store
            .put_publish_plan(OWNER, &plan.stored(), 4_102_444_800)
            .expect("kept");

        let OwnerQueue {
            link,
            pending,
            tally: queue_tally,
        } = owner_queue(&store, &store, &store, OWNER);
        assert_eq!(pending, vec![pending_suggestion()]);
        assert_eq!(queue_tally, tally([ports::SuggestionState::Pending]));
        let html = views::review_page(&QueueView {
            handle: "priya.example",
            csrf_token: "csrf",
            github: github_step_of(link.as_ref()),
            latest_scan: None,
            pending: &pending,
            tally: queue_tally,
            notice: None,
        });
        assert!(html.contains("tideworks/tidepool"), "{html}");
        for link_part in [plan.claim().object.as_str(), link_predicate(), plan.rkey()] {
            assert!(!html.contains(link_part), "{link_part}: {html}");
        }
    }

    fn suggestion_about(repo: &str) -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: format!("github:{repo}"),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.memory-safety".into(),
            },
            confidence_bp: 2500,
            evidence: vec![],
            why: vec![],
            source_repo: repo.into(),
        }
    }

    /// U-RRA-D5: a declined suggestion counts toward KPI-RRA-1 by where it
    /// came from: an import, an owned repo outside the default, or the
    /// default itself.
    #[test]
    fn a_suggestions_origin_is_read_as_kpi_rra_1_counts_it() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).expect("store");
        store
            .add_or_reverify(OWNER, LINKED_LOGIN, LINKED_ID, 3, KpiTally::NONE)
            .expect("linked");
        let listed = [
            ("priyaraman/default", 1, None),
            ("priyaraman/chosen", 2, Some(OutsideDefault::OwnedChoice)),
            ("priyaraman/other", 3, Some(OutsideDefault::OtherAccount)),
        ];
        for (repo, repo_id, outside_default) in listed {
            store
                .add_pending_from_listed_source(
                    OWNER,
                    &ports::SuggestionSource {
                        account_id: LINKED_ID,
                        repo_id,
                        outside_default,
                    },
                    &[suggestion_about(repo)],
                )
                .expect("queued");
        }
        let imported = ImportedRepo {
            repo_id: 9,
            full_name: "rust-lang/cargo".into(),
        };
        store
            .add_import(OWNER, &imported, KpiTally::NONE)
            .expect("imported");
        store
            .add_pending_from_import(OWNER, 9, &[suggestion_about("rust-lang/cargo")])
            .expect("queued");

        let origin_of = |repo: &str| acted_origin(&store, OWNER, &suggestion_about(repo).key);
        assert_eq!(origin_of("priyaraman/default"), ActedOrigin::Default);
        assert_eq!(
            origin_of("priyaraman/chosen"),
            ActedOrigin::Outside(OutsideDefault::OwnedChoice)
        );
        assert_eq!(
            origin_of("priyaraman/other"),
            ActedOrigin::Outside(OutsideDefault::OtherAccount)
        );
        assert_eq!(origin_of("rust-lang/cargo"), ActedOrigin::Imported);
    }
}
