//! The signed-in review queue, its cards, the edit form and the scan
//! status (US-BRA-003/005/006, ADR-076).

use maud::{html, Markup};
use ports::{
    AccountCheckOutcome, RepoSkipReason, ScanCounts, ScanRun, ScanStatus, SkippedScanRepo,
    Suggestion, SuggestionKey,
};

use super::*;
use crate::edits::CONFIDENCE_GUIDANCE;
use crate::lifecycle::Tally;
use crate::repo_scope::LimitRefusal;

/// Where the signed-in person stands with their GitHub link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubStep<'a> {
    /// No GitHub account linked yet.
    NotLinked,
    /// Linked and verified: a scan may start (after a fresh re-check).
    Verified { login: &'a str },
    /// Linked once, but the last check failed: verify again.
    NeedsReverify { login: &'a str },
}

/// The label of the button that starts a first scan.
pub const SCAN_LABEL: &str = "Scan my repos";

/// The label of the button that scans again after a finished scan.
pub const SCAN_AGAIN_LABEL: &str = "Scan again";

/// The label of the button that resumes a paused or interrupted scan.
pub const RESUME_SCAN_LABEL: &str = "Resume scan";

/// The label of the button that opens the exact-record preview.
pub const APPROVE_LABEL: &str = "Approve";

/// The label of the button that declines a suggestion.
pub const DECLINE_LABEL: &str = "Not me";

/// The queue's privacy statement (AC-003.3), always shown above the cards.
pub const PRIVATE_NOTICE: &str = "Private: only you can see these.";

/// What an empty scan says (AC-003.6).
pub const EMPTY_SCAN_NOTICE: &str = "We didn't find suggestions in your owned, public repos. \
     Forks and archived repos are skipped.";

/// What the queue says right after "Not me" (AC-006.2).
pub const DECLINED_NOTICE: &str = "Declined. Private: never published, won't be suggested again.";

/// The head of a scan's summary when the owner lists no GitHub account and
/// it read only her imports (AC-005.5, D-RRA-8).
pub const NO_ACCOUNT_TO_RECHECK: &str = "No GitHub account to re-check";

/// What an imported repo's card says (US-RRA-006, BR-RRA-1).
pub const NOT_YOURS_TO_PUBLISH: &str = "Not yours to publish.";

/// "Not yours to publish. Only an owner of github.com/rust-lang can publish
/// claims about this repo." for a suggestion about `rust-lang/cargo`.
pub fn not_yours_message(source_repo: &str) -> String {
    let owner = source_repo
        .split_once('/')
        .map_or(source_repo, |(owner, _)| owner);
    format!(
        "{NOT_YOURS_TO_PUBLISH} Only an owner of github.com/{owner} can publish claims about \
         this repo."
    )
}

/// The label of the button that restores a just-declined suggestion.
pub const UNDO_LABEL: &str = "Undo";

/// What the queue says once every suggestion is settled: unlike an empty
/// scan, the scan did find suggestions and the owner reviewed them all.
pub fn all_reviewed_notice(tally: &Tally) -> String {
    format!(
        "You've reviewed every suggestion: {} declined, {} published.",
        tally.declined, tally.published
    )
}

/// Why a scan did not start (ADR-076 §2 budget).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanRefused {
    AlreadyScanning,
    DailyLimitReached,
    AppBusy,
}

/// The sentence explaining a refused scan.
pub fn scan_refused_message(refused: ScanRefused) -> &'static str {
    match refused {
        ScanRefused::AlreadyScanning => "Your scan is already running.",
        ScanRefused::DailyLimitReached => {
            "You have scanned 6 times today, the most a day allows. Please come back \
             tomorrow. Nothing was lost."
        }
        ScanRefused::AppBusy => {
            "OpenLore is busy scanning for other people right now. Please try again in a \
             few minutes. Nothing was lost."
        }
    }
}

/// What the queue reports about the owner's last action.
#[derive(Debug, Clone, Copy)]
pub enum QueueNotice<'a> {
    /// The scan the owner asked for did not start.
    ScanRefused(ScanRefused),
    /// The owner just declined this suggestion; it can still be undone.
    Declined(&'a SuggestionKey),
    /// The scan did not start: no repo is chosen and none is named (AC-002.5).
    NothingToScan,
    /// The scan did not start: more repos are chosen than one scan reads.
    ScanOverLimit(LimitRefusal),
}

/// AC-002.5: a scan with nothing to read.
pub const NOTHING_TO_SCAN_MESSAGE: &str =
    "Choose at least one repo, or name a public repo, before scanning.";

/// "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to scan."
pub fn scan_over_limit_message(refusal: LimitRefusal) -> String {
    format!(
        "You can choose up to {} repos per scan in total. You have {} chosen. Untick {} to scan.",
        refusal.limit, refusal.total, refusal.excess
    )
}

/// "5 repos read" (`1 repo read`).
pub fn repos_read_message(read: usize) -> String {
    match read {
        1 => "1 repo read".to_string(),
        n => format!("{n} repos read"),
    }
}

/// Everything the signed-in queue shows.
#[derive(Debug, Clone, Copy)]
pub struct QueueView<'a> {
    pub handle: &'a str,
    pub csrf_token: &'a str,
    pub github: GithubStep<'a>,
    pub latest_scan: Option<ScanRun>,
    /// The owner's pending suggestions (shown only while the link is verified).
    pub pending: &'a [Suggestion],
    /// How many of the owner's suggestions stand in each state (AC-004.7).
    pub tally: Tally,
    pub notice: Option<QueueNotice<'a>>,
}

/// `HH:MM` UTC of a Unix time.
pub fn utc_clock(unix_secs: i64) -> String {
    let of_day = unix_secs.rem_euclid(24 * 60 * 60);
    format!("{:02}:{:02}", of_day / 3600, (of_day % 3600) / 60)
}

/// The scan action the queue offers after `latest`.
fn scan_action(latest: Option<ScanRun>) -> Option<&'static str> {
    match latest.map(|run| run.status) {
        None => Some(SCAN_LABEL),
        Some(ScanStatus::Running) => None,
        Some(ScanStatus::RateLimited | ScanStatus::Interrupted) => Some(RESUME_SCAN_LABEL),
        Some(ScanStatus::Completed | ScanStatus::OwnershipFailed) => Some(SCAN_AGAIN_LABEL),
    }
}

/// The queue region: the target htmx swaps when a scan starts and while it
/// runs.
const QUEUE_REGION: &str = "#queue";

/// While a scan runs the queue region re-fetches itself this often.
const SCAN_POLL_TRIGGER: &str = "every 2s";

/// Whether the latest scan is still running, so the queue region should keep
/// re-fetching itself to show what it finds.
fn scan_running(latest: Option<ScanRun>) -> bool {
    latest.is_some_and(|run| run.status == ScanStatus::Running)
}

/// "Scanning your repos…", with what has landed so far once anything has.
fn scanning_notice(found: usize) -> String {
    match found {
        0 => "Scanning your repos…".to_string(),
        1 => "Scanning your repos… 1 suggestion so far".to_string(),
        n => format!("Scanning your repos… {n} suggestions so far"),
    }
}

/// What the latest scan says above the cards, if anything.
fn scan_notice(latest: Option<ScanRun>, pending: &[Suggestion], tally: &Tally) -> Option<String> {
    let run = latest?;
    match run.status {
        ScanStatus::Running => Some(scanning_notice(pending.len())),
        ScanStatus::Completed if pending.is_empty() && tally.all_reviewed() => {
            Some(all_reviewed_notice(tally))
        }
        ScanStatus::Completed if pending.is_empty() => Some(EMPTY_SCAN_NOTICE.to_string()),
        ScanStatus::RateLimited => Some(format!(
            "GitHub is busy, resuming at {} UTC. The suggestions found so far are kept, \
             and the scan picks up again on its own then.",
            run.resume_after
                .map_or_else(|| "a later time".to_string(), utc_clock)
        )),
        ScanStatus::Interrupted => Some(
            "Your last scan was interrupted. The suggestions found so far are kept; press \
             Resume scan to finish it."
                .to_string(),
        ),
        ScanStatus::Completed | ScanStatus::OwnershipFailed => None,
    }
}

/// What a completed scan found (AC-010.5, DWD-11): ownership was re-checked
/// first, how many suggestions are new, and — when any — how many of the
/// keys it derived were already published or declined (declined stay
/// hidden). Every count is of that scan's derived keys only.
pub fn scan_summary(counts: ScanCounts) -> String {
    summary_after("Ownership re-checked ✓".to_string(), counts)
}

/// One listed account's bio check in the latest scan, by the login it is
/// listed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountChecked<'a> {
    pub login: &'a str,
    pub outcome: AccountCheckOutcome,
}

/// "Ownership re-checked: priyaraman ✓ · priya-tidal ✗": each account's
/// check in the latest scan, in account order.
pub fn accounts_rechecked(checks: &[AccountChecked<'_>]) -> String {
    let each: Vec<String> = checks
        .iter()
        .map(|check| match check.outcome {
            AccountCheckOutcome::Passed => format!("{} ✓", check.login),
            AccountCheckOutcome::Failed => format!("{} ✗", check.login),
            AccountCheckOutcome::NotReached => format!("{} not reached", check.login),
        })
        .collect();
    format!("Ownership re-checked: {}", each.join(" · "))
}

/// What a completed scan found, its head naming each account's check when
/// the scan recorded them.
pub fn scan_summary_by_account(counts: ScanCounts, checks: &[AccountChecked<'_>]) -> String {
    if checks.is_empty() {
        scan_summary(counts)
    } else {
        summary_after(accounts_rechecked(checks), counts)
    }
}

/// Why an account's repos were not read in the latest scan, and what to do
/// (journey mockup 4); nothing for an account whose check passed. It names
/// the account, never its bio or the DID.
pub fn account_check_message(check: &AccountChecked<'_>) -> Option<String> {
    match check.outcome {
        AccountCheckOutcome::Passed => None,
        AccountCheckOutcome::Failed => Some(format!(
            "Your DID is not in the bio of github.com/{}, so its repos were not read and its \
             suggestions are hidden. Put your DID back, then verify.",
            check.login
        )),
        AccountCheckOutcome::NotReached => Some(format!(
            "{}: GitHub could not be reached, so its repos were not read. Nothing was lost. \
             Scan again later.",
            check.login
        )),
    }
}

/// `head` then the counts of what a scan found.
fn summary_after(head: String, counts: ScanCounts) -> String {
    let new = match counts.new {
        1 => "1 new suggestion".to_string(),
        n => format!("{n} new suggestions"),
    };
    let published = (counts.already_published > 0)
        .then(|| format!("{} already published", counts.already_published));
    let declined = (counts.declined_hidden > 0)
        .then(|| format!("{} declined (hidden)", counts.declined_hidden));
    [head, new]
        .into_iter()
        .chain(published)
        .chain(declined)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Why a chosen repo was skipped and the fix (AC-002.4): it names the repo,
/// never its owner's bio or DID.
pub fn skipped_repo_message(skipped: &SkippedScanRepo) -> String {
    let (reason, fix) = match skipped.reason {
        RepoSkipReason::Archived => ("archived", "Untick it, or unarchive it on GitHub."),
        RepoSkipReason::Fork => ("a fork", "Untick it."),
        RepoSkipReason::MissingOrTransferred => ("missing or owned by someone else", "Untick it."),
        RepoSkipReason::AccountUnverified => (
            "its account's bio no longer holds your DID",
            "Put your DID back, then verify.",
        ),
    };
    format!(
        "Skipped {}: {reason} since you chose it. {fix}",
        skipped.full_name
    )
}

/// "No GitHub account to re-check · 1 repo read · 3 new suggestions": a
/// scan of an owner without accounts read only her imports.
pub fn imports_only_summary(repos_read: usize, counts: ScanCounts) -> String {
    summary_after(
        format!(
            "{NO_ACCOUNT_TO_RECHECK} · {}",
            repos_read_message(repos_read)
        ),
        counts,
    )
}

/// Whose queue is shown: an owner with a verified account, or one with no
/// account who named public repos (D-RRA-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueHead<'a> {
    Account { login: &'a str },
    NoAccount,
}

/// The summary of the latest scan, once it has completed.
fn completed_summary(
    head: QueueHead<'_>,
    latest: Option<ScanRun>,
    checks: &[AccountChecked<'_>],
) -> Option<String> {
    latest
        .filter(|run| run.status == ScanStatus::Completed)
        .map(|run| match head {
            QueueHead::Account { .. } => scan_summary_by_account(run.counts, checks),
            QueueHead::NoAccount => imports_only_summary(run.repos_read, run.counts),
        })
}

/// Why each account the latest finished scan did not read was not read.
fn account_check_notices(latest: Option<ScanRun>, checks: &[AccountChecked<'_>]) -> Markup {
    let finished = latest.is_some_and(|run| run.status != ScanStatus::Running);
    html! {
        @if finished {
            @for message in checks.iter().filter_map(account_check_message) {
                p role="status" { (message) }
            }
        }
    }
}

/// What the latest scan adds to the queue beyond its counts: the chosen
/// repos it skipped, each account's check, and the account each pending
/// suggestion came from (by the login it is listed under).
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanDetails<'a> {
    pub skipped: &'a [SkippedScanRepo],
    pub account_checks: &'a [AccountChecked<'a>],
    pub card_accounts: &'a [(SuggestionKey, &'a str)],
    /// The pending suggestions about repos she imported: never hers to publish.
    pub card_imported: &'a [SuggestionKey],
    /// Whether she lists any imported repo (then a scan needs no account).
    pub imports_listed: bool,
}

impl<'a> ScanDetails<'a> {
    /// Where the pending suggestion `key` came from, as its card says.
    fn card_source(&self, key: &SuggestionKey) -> CardSource<'a> {
        if self.card_imported.contains(key) {
            CardSource::Imported
        } else {
            CardSource::Account(self.account_of(key))
        }
    }

    /// The account the pending suggestion `key` came from.
    fn account_of(&self, key: &SuggestionKey) -> Option<&'a str> {
        self.card_accounts
            .iter()
            .find(|(card, _)| card == key)
            .map(|(_, login)| *login)
    }
}

/// The hidden fields naming a suggestion in its card's forms.
fn key_fields(key: &SuggestionKey) -> Markup {
    html! {
        input type="hidden" name="subject" value=(key.subject);
        input type="hidden" name="predicate" value=(key.predicate);
        input type="hidden" name="object" value=(key.object);
    }
}

/// The form that previews every ticked card at once.
const APPROVE_SELECTED_FORM: &str = "approve-selected";

/// The button that previews every ticked card at once.
pub const APPROVE_SELECTED_LABEL: &str = "Approve selected";

/// The card's tick box, submitted by the "Approve selected" form.
fn selection_box(key: &SuggestionKey) -> Markup {
    let label = format!(
        "Select {} embodies {}",
        repo_path(&key.subject),
        philosophy_slug(&key.object)
    );
    html! {
        label class="select" {
            input type="checkbox" name=(SELECTION_FIELD) value=(selection_value(key))
                form=(APPROVE_SELECTED_FORM) aria-label=(label);
            " Select"
        }
    }
}

/// "Approve selected": the ticked cards' boxes belong to this form.
fn approve_selected_form(csrf_token: &str) -> Markup {
    html! {
        form id=(APPROVE_SELECTED_FORM) method="post" action="/review/approve-selected" {
            (csrf_field(csrf_token))
            button type="submit" { (APPROVE_SELECTED_LABEL) }
        }
    }
}

/// How a card action is sent: Approve opens its preview inside the card
/// with htmx (a plain post without JavaScript); the others load a page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CardSwap {
    InCard,
    Page,
}

/// One action of a card: a form naming the suggestion, its button bound to
/// the triage key `triage_key`.
fn card_action(
    action: &str,
    triage_key: &str,
    label: &str,
    key: &SuggestionKey,
    csrf_token: &str,
    swap: CardSwap,
) -> Markup {
    let in_card = swap == CardSwap::InCard;
    html! {
        form method="post" action=(action) hx-post=[in_card.then_some(action)]
            hx-target=[in_card.then_some("closest article")]
            hx-select=[in_card.then_some(OUTCOME_REGION)]
            hx-swap=[in_card.then_some("innerHTML")] {
            (csrf_field(csrf_token))
            (key_fields(key))
            button type="submit" data-key=(triage_key) { (label) }
        }
    }
}

/// Where a card's suggestion came from: one of her accounts (by the login
/// it is listed under, when known), or a repo she imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardSource<'a> {
    Account(Option<&'a str>),
    Imported,
}

/// One evidence-backed card (AC-003.2): confidence, why, evidence, actions.
/// Its stable id and `hx-morph-skip-children` let the 2 s refresh during a
/// scan add and remove cards without touching one's contents, so a preview
/// opened inside a card stays open.
fn suggestion_card(suggestion: &Suggestion, source: CardSource<'_>, csrf_token: &str) -> Markup {
    html! {
        article id=(card_id(&suggestion.key)) hx-morph-skip-children {
            (selection_box(&suggestion.key))
            h2 { (suggestion_headline(&suggestion.key)) }
            @match source {
                CardSource::Account(Some(login)) => {
                    p { "from github.com/" (login) }
                }
                CardSource::Account(None) => {}
                CardSource::Imported => {
                    p { strong { (not_yours_message(&suggestion.source_repo)) } }
                }
            }
            p {
                "Confidence " (confidence_with_bucket(suggestion.confidence_bp))
            }
            p { "Why: " (suggestion.why.join("; ")) }
            p {
                "Evidence: "
                @for url in &suggestion.evidence {
                    a href=(url) { (url) } " "
                }
            }
            (card_action("/review/approve", "a", APPROVE_LABEL, &suggestion.key, csrf_token, CardSwap::InCard))
            (card_action("/review/edit", "e", EDIT_LABEL, &suggestion.key, csrf_token, CardSwap::Page))
            (card_action("/review/decline", "n", DECLINE_LABEL, &suggestion.key, csrf_token, CardSwap::Page))
        }
    }
}

/// The card's edit button label (US-BRA-005).
pub const EDIT_LABEL: &str = "Edit";

/// The edit form's button that previews the edited claim.
pub const PREVIEW_LABEL: &str = "Preview";

/// The edit form of one suggestion: what the owner typed so far and, when
/// her confidence was refused, the guidance (approval stays blocked).
pub struct EditView<'a> {
    pub suggestion: &'a Suggestion,
    pub csrf_token: &'a str,
    /// The philosophies offered (the whole vocabulary).
    pub choices: &'a [String],
    /// The philosophy currently chosen.
    pub chosen: &'a str,
    /// The confidence as typed.
    pub typed_confidence: &'a str,
    /// Why the typed confidence was refused, if it was.
    pub guidance: Option<&'a str>,
}

/// Edit a suggestion before approving it (AC-005.1–005.5): swap the
/// philosophy, set the confidence, preview. Nothing is written here; Cancel
/// returns to the queue with the suggestion unchanged.
pub fn edit_page(view: &EditView<'_>) -> String {
    let key = &view.suggestion.key;
    page(
        "Edit before approving",
        html! {
            h1 { "Edit before approving" }
            p { (suggestion_headline(key)) }
            p {
                "Suggested confidence " (confidence_text(view.suggestion.confidence_bp))
                ". Your edit stays private until you publish it."
            }
            form method="post" action="/review/edit/preview" {
                (csrf_field(view.csrf_token))
                input type="hidden" name="subject" value=(key.subject);
                input type="hidden" name="predicate" value=(key.predicate);
                input type="hidden" name="suggested_object" value=(key.object);
                label {
                    "Philosophy "
                    select name="object" {
                        @for object in view.choices {
                            option value=(object) selected[object == view.chosen] {
                                (philosophy_slug(object))
                            }
                        }
                    }
                }
                label {
                    "Confidence (0.00 to 1.00) "
                    input type="text" name="confidence" inputmode="decimal"
                        value=(view.typed_confidence) title=(CONFIDENCE_GUIDANCE);
                }
                @if let Some(guidance) = view.guidance {
                    p role="alert" { (guidance) }
                }
                button type="submit" { (PREVIEW_LABEL) }
            }
            p { a href="/review" { (CANCEL_LABEL) } }
        },
    )
}

/// The just-declined notice and its Undo (AC-006.2 / AC-006.5).
fn declined_notice(key: &SuggestionKey, csrf_token: &str) -> Markup {
    html! {
        p role="status" { (DECLINED_NOTICE) }
        form method="post" action="/review/undo" {
            (csrf_field(csrf_token))
            (key_fields(key))
            button type="submit" { (UNDO_LABEL) }
        }
    }
}

/// The verified owner's queue: privacy statement, scan state (naming the
/// chosen repos its latest scan skipped), cards.
fn verified_queue(head: QueueHead<'_>, view: &QueueView<'_>, details: &ScanDetails<'_>) -> Markup {
    html! {
        @if let QueueHead::Account { login } = head {
            p {
                "GitHub: github.com/" (login) " (verified) · "
                a href="/github" { "Manage your GitHub link" }
            }
        }
        h1 { "Your suggestions" }
        p { strong { (PRIVATE_NOTICE) } }
        @let polling = scan_running(view.latest_scan);
        div id="queue" hx-get=[polling.then_some("/review")]
            hx-trigger=[polling.then_some(SCAN_POLL_TRIGGER)]
            hx-select=[polling.then_some(QUEUE_REGION)]
            hx-swap=[polling.then_some("outerMorph")] {
            @match view.notice {
                Some(QueueNotice::ScanRefused(refused)) => {
                    p role="alert" { (scan_refused_message(refused)) }
                }
                Some(QueueNotice::Declined(key)) => {
                    (declined_notice(key, view.csrf_token))
                }
                Some(QueueNotice::NothingToScan) => {
                    p role="alert" { (NOTHING_TO_SCAN_MESSAGE) }
                }
                Some(QueueNotice::ScanOverLimit(refusal)) => {
                    p role="alert" { (scan_over_limit_message(refusal)) }
                }
                None => {}
            }
            @if let Some(summary) = completed_summary(head, view.latest_scan, details.account_checks) {
                p role="status" { (summary) }
            }
            (account_check_notices(view.latest_scan, details.account_checks))
            @if let Some(run) = view.latest_scan.filter(|run| run.status == ScanStatus::Completed) {
                p role="status" { (repos_read_message(run.repos_read)) }
                @for skip in details.skipped {
                    p role="status" { (skipped_repo_message(skip)) }
                }
            }
            @if let Some(notice) = scan_notice(view.latest_scan, view.pending, &view.tally) {
                p role="status" { (notice) }
            }
            @if view.tally.published > 0 {
                p { (view.pending.len()) " pending · " (view.tally.published) " published" }
            }
            @if let Some(label) = scan_action(view.latest_scan) {
                form method="post" action="/scan" hx-post="/scan" hx-target=(QUEUE_REGION)
                    hx-select=(QUEUE_REGION) hx-swap="outerMorph" {
                    (csrf_field(view.csrf_token))
                    button type="submit" { (label) }
                }
            }
            @if !view.pending.is_empty() {
                (approve_selected_form(view.csrf_token))
            }
            @for suggestion in view.pending {
                (suggestion_card(suggestion, details.card_source(&suggestion.key), view.csrf_token))
            }
        }
        p data-triage-keys="a e n j k" {
            "Keyboard: A approve · E edit · N not me · J/K next/previous suggestion"
        }
        script src="/assets/triage.js" {}
    }
}

/// The signed-in review queue: the GitHub step until ownership is proven,
/// then the private queue.
pub fn review_page(view: &QueueView<'_>) -> String {
    review_page_naming_skips(view, &[])
}

/// The review queue, its completed latest scan's summary naming each chosen
/// repo that scan `skipped`, with why and the fix.
pub fn review_page_naming_skips(view: &QueueView<'_>, skipped: &[SkippedScanRepo]) -> String {
    review_page_of_scan(
        view,
        &ScanDetails {
            skipped,
            ..ScanDetails::default()
        },
    )
}

/// The review queue with what its latest scan adds ([`ScanDetails`]).
pub fn review_page_of_scan(view: &QueueView<'_>, details: &ScanDetails<'_>) -> String {
    page(
        "Your review queue",
        html! {
            p { "Signed in as @" (view.handle) }
            @match view.github {
                GithubStep::NotLinked if details.imports_listed => {
                    p {
                        "No GitHub account linked: your scans read only the repos you named. "
                        a href="/github" { "Verify GitHub ownership" }
                    }
                    (verified_queue(QueueHead::NoAccount, view, details))
                }
                GithubStep::NotLinked => {
                    p { "Before anything is scanned, verify that your GitHub account is yours." }
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::NeedsReverify { login } => {
                    p role="alert" {
                        "Your DID is no longer in github.com/" (login) "'s bio, so we didn't scan. \
                         Your published claims are untouched; your pending suggestions are \
                         hidden until you re-verify: put your DID back in the bio (or, if that \
                         account changed hands, link your own), then verify."
                    }
                    (account_check_notices(view.latest_scan, details.account_checks))
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::Verified { login } => {
                    (verified_queue(QueueHead::Account { login }, view, details))
                }
            }
            form method="post" action="/signout" {
                (csrf_field(view.csrf_token))
                button type="submit" { (SIGN_OUT_LABEL) }
            }
            p { a href="/settings" { "Settings" } }
        },
    )
}

/// The scan-status fragment the queue polls (`data-scan-status`).
pub fn scan_status_page(status: &str) -> String {
    page(
        "Scan status",
        html! {
            div data-scan-status=(status) { "Scan status: " (status) }
            (back_to_queue())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn completed_queue(tally: Tally) -> String {
        review_page(&QueueView {
            handle: "priyaraman.bsky.social",
            csrf_token: "csrf",
            github: GithubStep::Verified {
                login: "priyaraman",
            },
            latest_scan: Some(ScanRun {
                status: ScanStatus::Completed,
                resume_after: None,
                counts: ScanCounts::default(),
                repos_read: 0,
            }),
            pending: &[],
            tally,
            notice: None,
        })
    }

    proptest! {
        /// Universe: settled tallies with nothing pending, after a completed
        /// scan. A queue the owner has fully reviewed says so with its
        /// declined and published counts; only a queue with nothing ever
        /// suggested says the scan found nothing.
        #[test]
        fn a_fully_reviewed_queue_is_not_mistaken_for_an_empty_scan(
            declined in 0usize..5, published in 0usize..5, retracted in 0usize..3,
        ) {
            let tally = Tally { pending: 0, declined, published, retracted };
            let page = completed_queue(tally);
            let settled = declined + published + retracted > 0;
            prop_assert_eq!(page.contains(EMPTY_SCAN_NOTICE), !settled);
            prop_assert_eq!(page.contains(&all_reviewed_notice(&tally)), settled);
            if settled {
                let counts = format!("{declined} declined, {published} published");
                prop_assert!(page.contains(&counts), "{}", page);
            }
        }

        /// Universe: a completed scan's counts. The summary always says
        /// ownership was re-checked and how many are new (so "0 new" when
        /// nothing is), and names each settled count exactly when nonzero.
        #[test]
        fn a_scan_summary_names_exactly_that_scans_counts(
            new in 0usize..20, already_published in 0usize..20, declined_hidden in 0usize..20,
        ) {
            let summary = scan_summary(ScanCounts { new, already_published, declined_hidden });
            prop_assert!(summary.starts_with("Ownership re-checked ✓ · "), "{}", summary);
            let new_part = format!(" · {new} new suggestion");
            prop_assert!(summary.contains(&new_part), "{}", summary);
            let published = format!(" · {already_published} already published");
            prop_assert_eq!(summary.contains(&published), already_published > 0);
            let declined = format!(" · {declined_hidden} declined (hidden)");
            prop_assert_eq!(summary.contains(&declined), declined_hidden > 0);
        }
    }
}
