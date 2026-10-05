//! The signed-in review queue, its cards, the edit form and the scan
//! status (US-BRA-003/005/006, ADR-076).

use maud::{html, Markup};
use ports::{ScanCounts, ScanRun, ScanStatus, Suggestion, SuggestionKey};

use super::*;
use crate::edits::CONFIDENCE_GUIDANCE;
use crate::lifecycle::Tally;

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

/// What the latest scan says above the cards, if anything.
fn scan_notice(latest: Option<ScanRun>, pending: &[Suggestion], tally: &Tally) -> Option<String> {
    let run = latest?;
    match run.status {
        ScanStatus::Running => Some("Scanning your repos…".to_string()),
        ScanStatus::Completed if pending.is_empty() && tally.all_reviewed() => {
            Some(all_reviewed_notice(tally))
        }
        ScanStatus::Completed if pending.is_empty() => Some(EMPTY_SCAN_NOTICE.to_string()),
        ScanStatus::RateLimited => Some(format!(
            "GitHub is busy, resuming at {} UTC. The suggestions found so far are kept; \
             press Resume scan then.",
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
    let new = match counts.new {
        1 => "1 new suggestion".to_string(),
        n => format!("{n} new suggestions"),
    };
    let published = (counts.already_published > 0)
        .then(|| format!("{} already published", counts.already_published));
    let declined = (counts.declined_hidden > 0)
        .then(|| format!("{} declined (hidden)", counts.declined_hidden));
    ["Ownership re-checked ✓".to_string(), new]
        .into_iter()
        .chain(published)
        .chain(declined)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The summary of the latest scan, once it has completed.
fn completed_summary(latest: Option<ScanRun>) -> Option<String> {
    latest
        .filter(|run| run.status == ScanStatus::Completed)
        .map(|run| scan_summary(run.counts))
}

/// The hidden fields naming a suggestion in its card's forms.
fn key_fields(key: &SuggestionKey) -> Markup {
    html! {
        input type="hidden" name="subject" value=(key.subject);
        input type="hidden" name="predicate" value=(key.predicate);
        input type="hidden" name="object" value=(key.object);
    }
}

/// One action of a card: a form naming the suggestion, its button bound to
/// the triage key `triage_key`.
fn card_action(
    action: &str,
    triage_key: &str,
    label: &str,
    key: &SuggestionKey,
    csrf_token: &str,
) -> Markup {
    html! {
        form method="post" action=(action) {
            (csrf_field(csrf_token))
            (key_fields(key))
            button type="submit" data-key=(triage_key) { (label) }
        }
    }
}

/// One evidence-backed card (AC-003.2): confidence, why, evidence, actions.
fn suggestion_card(suggestion: &Suggestion, csrf_token: &str) -> Markup {
    html! {
        article {
            h2 { (suggestion_headline(&suggestion.key)) }
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
            (card_action("/review/approve", "a", APPROVE_LABEL, &suggestion.key, csrf_token))
            (card_action("/review/edit", "e", EDIT_LABEL, &suggestion.key, csrf_token))
            (card_action("/review/decline", "n", DECLINE_LABEL, &suggestion.key, csrf_token))
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

/// The verified owner's queue: privacy statement, scan state, cards.
fn verified_queue(login: &str, view: &QueueView<'_>) -> Markup {
    html! {
        p { "GitHub: github.com/" (login) " (verified)" }
        h1 { "Your suggestions" }
        p { strong { (PRIVATE_NOTICE) } }
        @match view.notice {
            Some(QueueNotice::ScanRefused(refused)) => {
                p role="alert" { (scan_refused_message(refused)) }
            }
            Some(QueueNotice::Declined(key)) => {
                (declined_notice(key, view.csrf_token))
            }
            None => {}
        }
        @if let Some(summary) = completed_summary(view.latest_scan) {
            p role="status" { (summary) }
        }
        @if let Some(notice) = scan_notice(view.latest_scan, view.pending, &view.tally) {
            p role="status" { (notice) }
        }
        @if view.tally.published > 0 {
            p { (view.pending.len()) " pending · " (view.tally.published) " published" }
        }
        @if let Some(label) = scan_action(view.latest_scan) {
            form method="post" action="/scan" {
                (csrf_field(view.csrf_token))
                button type="submit" { (label) }
            }
        }
        @for suggestion in view.pending {
            (suggestion_card(suggestion, view.csrf_token))
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
    page(
        "Your review queue",
        html! {
            p { "Signed in as @" (view.handle) }
            @match view.github {
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
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::Verified { login } => {
                    (verified_queue(login, view))
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
