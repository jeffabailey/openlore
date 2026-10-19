//! "Add repo" (US-RRA-005): the owner names a public repo she doesn't own;
//! its suggestions stay private (BR-RRA-1). The shape and the import are
//! `review_domain::repo_scope`'s decisions; this module only reads GitHub
//! (public reads, the repo's owner is never contacted) and writes the store.

use ports::KpiTally;
use review_domain::kpi::{utc_day, KpiEvent};
use review_domain::repo_scope::{import_of, import_shape};
use review_domain::views::IMPORT_REPO_FIELD;

use crate::http::{App, PageRequest, Reply};
use crate::routes::repos::repos_page;
use crate::routes::signin::{csrf_token_of, field, signed_in_post};

/// `POST /repos/imports`: read the named repo publicly and list it under
/// "Repos you don't own", counted as `import.added`.
pub(crate) async fn add_import(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let typed = field(&request.form, IMPORT_REPO_FIELD).unwrap_or_default();
    if let Some((owner, name)) = import_shape(&typed) {
        if let Ok(facts) = app.github.read_repo(owner, name).await {
            let today = utc_day(crate::limiter::unix_now_secs());
            let tally = KpiTally {
                day: &today,
                events: &[KpiEvent::ImportAdded.name()],
            };
            let _ = app
                .imports_write
                .add_import(&session.owner_did, &import_of(&facts), tally);
        }
    }
    repos_page(app, &session, &csrf_token_of(request), None).await
}
