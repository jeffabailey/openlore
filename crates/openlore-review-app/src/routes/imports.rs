//! "Add repo" (US-RRA-005): the owner names a public repo she doesn't own;
//! its suggestions stay private (BR-RRA-1). The shape, the verdict and the
//! import are `review_domain::repo_scope`'s decisions; this module only reads
//! GitHub (public reads, the repo's owner is never contacted) and writes the
//! store. A refusal names its cause and changes nothing. "Remove" deletes
//! an import and its pending suggestions at once (US-RRA-009, D-RRA-17).

use hyper::StatusCode;
use ports::claim_domain::authority::RepoRead;
use ports::{GithubError, ImportRemoval, ImportedRepo, KpiTally, RepoFacts, RepoSource};
use review_domain::kpi::{import_refusal_event, import_removed_age_event, utc_day, KpiEvent};
use review_domain::repo_scope::{
    classify_import, import_shape, ImportRefusal, ImportVerdict, SCAN_REPO_CAP,
};
use review_domain::views::{self, ReposNotice, IMPORT_REPO_FIELD};

use crate::http::{App, PageRequest, Reply};
use crate::routes::repos::{
    import_ids, listed_accounts, listed_imports, owner_scope, pending_from, repos_page,
};
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_for, csrf_token_of, field, id_field, signed_in_post};

/// `POST /repos/imports`: read the named repo publicly and list it under
/// "Repos you don't own", counted as `import.added`; or refuse it, counted
/// as `import.refused.<cause>`.
pub(crate) async fn add_import(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let typed = field(&request.form, IMPORT_REPO_FIELD).unwrap_or_default();
    let today = utc_day(crate::limiter::unix_now_secs());
    let scope_guard = app.scope_locks.hold(&session.owner_did).await;
    let verdict = import_verdict(app, &session.owner_did, &typed).await;
    let notice = match verdict {
        ImportVerdict::Admitted(import) => {
            let tally = KpiTally {
                day: &today,
                events: &[KpiEvent::ImportAdded.name()],
            };
            let _ = app
                .imports_write
                .add_import(&session.owner_did, &import, tally);
            None
        }
        ImportVerdict::Refused(refusal) => {
            if let Some(event) = import_refusal_event(refusal) {
                let _ = app.kpi.count(&today, event.name());
            }
            Some(ReposNotice::ImportRefused {
                refusal,
                repo: typed.trim().to_string(),
            })
        }
    };
    drop(scope_guard);
    repos_page(app, &session, &csrf_token_of(request), notice).await
}

/// The verdict on `typed`: a name that is not `owner/name` is refused before
/// anything is asked of GitHub.
async fn import_verdict(app: &App, owner_did: &str, typed: &str) -> ImportVerdict {
    let Some((owner, name)) = import_shape(typed) else {
        return ImportVerdict::Refused(ImportRefusal::Shape);
    };
    let answer = app.github.read_repo(owner, name).await;
    let listed: Vec<u64> = listed_accounts(app, owner_did)
        .iter()
        .map(|account| account.github_user_id)
        .collect();
    classify_import(
        typed,
        repo_read(&answer),
        &listed,
        &owner_scope(app, owner_did).await,
        &import_ids(app, owner_did),
        SCAN_REPO_CAP,
    )
}

/// GitHub's answer as the classification reads it: 404 (missing or
/// private) is one meaning, anything else unanswered is unfinished.
fn repo_read(answer: &Result<RepoFacts, GithubError>) -> RepoRead<'_> {
    match answer {
        Ok(facts) => RepoRead::Found(facts),
        Err(error) if error.is_gone() => RepoRead::Gone,
        Err(_) => RepoRead::Unfinished,
    }
}

/// The session owner's import the form names; another owner's, or none,
/// is not found (I-BRA-1).
fn named_import(app: &App, owner_did: &str, request: &PageRequest) -> Option<ImportedRepo> {
    let named = id_field(&request.form, IMPORT_REPO_FIELD)?;
    listed_imports(app, owner_did)
        .into_iter()
        .find(|import| import.repo_id == named)
}

/// `POST /repos/imports/remove`: the confirm naming how many pending
/// suggestions removing the import deletes. Nothing is deleted here.
pub(crate) fn remove_import_confirm(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let Some(import) = named_import(app, &session.owner_did, request) else {
        return not_found();
    };
    let pending = pending_from(
        app,
        &session.owner_did,
        RepoSource::Imported(import.repo_id),
    );
    Reply::page(
        StatusCode::OK,
        views::remove_import_confirm_page(&import, pending, &csrf_token_for(&cookie_value)),
    )
}

/// `POST /repos/imports/remove/confirm`: delete the import and its pending
/// suggestions at once (ADR-088 §8); its declined keys stay. Counted as
/// `import.removed` with the removal, and by how long it was held.
pub(crate) async fn remove_import(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let scope_guard = app.scope_locks.hold(&session.owner_did).await;
    let Some(import) = named_import(app, &session.owner_did, request) else {
        return not_found();
    };
    let today = utc_day(crate::limiter::unix_now_secs());
    let tally = KpiTally {
        day: &today,
        events: &[KpiEvent::ImportRemoved.name()],
    };
    let removal = app
        .scope_removal
        .remove_import(&session.owner_did, import.repo_id, tally);
    drop(scope_guard);
    let notice = match removal {
        Ok(ImportRemoval::Removed {
            full_name,
            deleted_pending,
            held_secs,
        }) => {
            let _ = app
                .kpi
                .count(&today, import_removed_age_event(held_secs).name());
            Some(ReposNotice::ImportRemoved {
                repo: full_name,
                deleted_pending,
            })
        }
        _ => None,
    };
    repos_page(app, &session, &csrf_token_for(&cookie_value), notice).await
}
