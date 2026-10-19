//! "Add repo" (US-RRA-005): the owner names a public repo she doesn't own;
//! its suggestions stay private (BR-RRA-1). The shape, the verdict and the
//! import are `review_domain::repo_scope`'s decisions; this module only reads
//! GitHub (public reads, the repo's owner is never contacted) and writes the
//! store. A refusal names its cause and changes nothing.

use ports::claim_domain::authority::RepoRead;
use ports::{GithubError, KpiTally, RepoFacts};
use review_domain::kpi::{import_refusal_event, utc_day, KpiEvent};
use review_domain::repo_scope::{
    classify_import, import_shape, ImportRefusal, ImportVerdict, SCAN_REPO_CAP,
};
use review_domain::views::{ReposNotice, IMPORT_REPO_FIELD};

use crate::http::{App, PageRequest, Reply};
use crate::routes::repos::{import_ids, listed_accounts, owner_scope, repos_page};
use crate::routes::signin::{csrf_token_of, field, signed_in_post};

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
    let notice = match import_verdict(app, &session.owner_did, &typed).await {
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
        Err(GithubError::NotFound { .. } | GithubError::NotPublic { .. }) => RepoRead::Gone,
        Err(_) => RepoRead::Unfinished,
    }
}
