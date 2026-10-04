//! Page renderers (maud). Each page is chrome + body, as in ADR-032.
//! Copy constants here are the single source of truth for the app's text.

use maud::{html, Markup, DOCTYPE};

use ports::claim_domain::{
    decode_claim_record, is_self_retracted, provenance_verdict, ClaimLineage, Did, Provenance,
    RecordOrigin, ReferenceType, UnsignedClaim,
};
use ports::{RepoRecord, ScanRun, ScanStatus, Suggestion, SuggestionKey};

use crate::edits::CONFIDENCE_GUIDANCE;
use crate::lifecycle::Tally;
use crate::ownership::OwnershipRefusal;
use crate::plans::PublishPlan;
use crate::signin::{PermissionMode, SignInFailure};

/// The name Bluesky shows when the app asks for permission.
pub const APP_NAME: &str = "OpenLore review";

/// What the app is for, in one line.
pub const APP_PURPOSE: &str =
    "Review the philosophy claims OpenLore suggests from your public GitHub repos, \
     and publish only the ones you approve to your own Bluesky repo.";

/// The three never-commitments (AC-001.2), shown before the handle field.
pub const NEVER_COMMITMENTS: [&str; 3] = [
    "OpenLore never publishes a suggestion you have not approved.",
    "OpenLore never shows your pending or declined suggestions to anyone.",
    "OpenLore never posts to Bluesky unless you press Post.",
];

/// The public-data banner (NFR-BRA-8).
pub const PUBLIC_DATA_NOTICE: &str =
    "Only public GitHub data is read: your public profile and your public repos.";

/// Why the broad permission is requested in fallback mode (ADR-073).
pub const BROAD_PERMISSION_NOTICE: &str =
    "Your Bluesky server does not yet support asking for claim and post creation alone, \
     so OpenLore asks for the broader permission. It still only creates claims, and posts \
     only when you press Post.";

/// The sign-in form's button label.
pub const SIGN_IN_LABEL: &str = "Sign in with Bluesky";

/// The sign-out button label.
pub const SIGN_OUT_LABEL: &str = "Sign out";

/// What a person sees when a sign-in did not complete.
pub fn sign_in_failure_message(failure: SignInFailure) -> &'static str {
    match failure {
        SignInFailure::HandleNotFound => {
            "We couldn't find that Bluesky handle. Check the spelling."
        }
        SignInFailure::TemporarilyUnavailable => {
            "Signing in with Bluesky is temporarily unavailable. Nothing changed. \
             Please try again in a few minutes."
        }
        SignInFailure::Cancelled => "No access granted. Nothing changed.",
        SignInFailure::ExchangeFailed => {
            "Signing in failed: your Bluesky server did not complete the sign-in. \
             Nothing changed. Please try again."
        }
        SignInFailure::AccountMismatch => {
            "Your Bluesky server answered for a different account than the handle you \
             entered, so you were not signed in. Nothing changed."
        }
        SignInFailure::ReturnNotRecognised => {
            "That sign-in link has expired or was already used. Nothing changed."
        }
    }
}

/// The shared page chrome. `body` is the page-specific fragment.
fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (APP_NAME) }
            }
            body {
                header { strong { (APP_NAME) } }
                main { (body) }
            }
        }
    }
}

/// The public landing page: what the app never does, then the handle field.
/// `failure` explains a sign-in that did not complete.
pub fn landing_page(mode: PermissionMode, failure: Option<SignInFailure>) -> String {
    page(
        "Welcome",
        html! {
            h1 { (APP_NAME) }
            p { (APP_PURPOSE) }
            @if let Some(failure) = failure {
                p role="alert" { (sign_in_failure_message(failure)) }
            }
            ul {
                @for commitment in NEVER_COMMITMENTS {
                    li { (commitment) }
                }
            }
            p { (PUBLIC_DATA_NOTICE) }
            @if mode == PermissionMode::BroadFallback {
                p { (BROAD_PERMISSION_NOTICE) }
            }
            form method="post" action="/signin" {
                label for="handle" { "Your Bluesky handle" }
                input id="handle" name="handle" type="text" autocomplete="username"
                    placeholder="you.bsky.social" required;
                button type="submit" { (SIGN_IN_LABEL) }
            }
        },
    )
    .into_string()
}

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

/// The label of the button that checks the bio.
pub const VERIFY_LABEL: &str = "Verify";

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

/// `priyaraman/tidepool` of `github:priyaraman/tidepool`.
fn repo_path(subject: &str) -> &str {
    subject.strip_prefix("github:").unwrap_or(subject)
}

/// `dependency-pinning` of `org.openlore.philosophy.dependency-pinning`.
fn philosophy_slug(object: &str) -> &str {
    object.rsplit('.').next().unwrap_or(object)
}

/// `0.25` of 2500 basis points.
pub fn confidence_text(basis_points: u16) -> String {
    format!(
        "{}.{:02}",
        basis_points / 10_000,
        (basis_points % 10_000) / 100
    )
}

/// The display-only bucket label of a confidence (WD-10).
pub fn bucket_label(basis_points: u16) -> &'static str {
    use ports::claim_domain::{confidence_bucket, ConfidenceBucket};
    match confidence_bucket(f64::from(basis_points) / 10_000.0) {
        ConfidenceBucket::Speculative => "speculative",
        ConfidenceBucket::Weighted => "weighted",
        ConfidenceBucket::WellEvidenced => "well-evidenced",
        ConfidenceBucket::Triangulated => "triangulated",
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

/// The suggestion's headline: "<repo> embodies <philosophy>".
fn suggestion_headline(key: &SuggestionKey) -> Markup {
    let repo = repo_path(&key.subject);
    html! {
        a href=(format!("https://github.com/{repo}/")) { (repo) }
        " embodies " (philosophy_slug(&key.object))
    }
}

/// One evidence-backed card (AC-003.2): confidence, why, evidence, actions.
fn suggestion_card(suggestion: &Suggestion, csrf_token: &str) -> Markup {
    html! {
        article {
            h2 { (suggestion_headline(&suggestion.key)) }
            p {
                "Confidence " (confidence_text(suggestion.confidence_bp))
                " (" (bucket_label(suggestion.confidence_bp)) ")"
            }
            p { "Why: " (suggestion.why.join("; ")) }
            p {
                "Evidence: "
                @for url in &suggestion.evidence {
                    a href=(url) { (url) } " "
                }
            }
            form method="post" action="/review/approve" {
                input type="hidden" name="csrf" value=(csrf_token);
                (key_fields(&suggestion.key))
                button type="submit" data-key="a" { (APPROVE_LABEL) }
            }
            form method="post" action="/review/edit" {
                input type="hidden" name="csrf" value=(csrf_token);
                (key_fields(&suggestion.key))
                button type="submit" data-key="e" { (EDIT_LABEL) }
            }
            form method="post" action="/review/decline" {
                input type="hidden" name="csrf" value=(csrf_token);
                (key_fields(&suggestion.key))
                button type="submit" data-key="n" { (DECLINE_LABEL) }
            }
        }
    }
}

/// The card's edit button label (US-BRA-005).
pub const EDIT_LABEL: &str = "Edit";

/// The edit form's button that previews the edited claim.
pub const PREVIEW_LABEL: &str = "Preview";

/// The edit form's way back: the suggestion, unchanged.
pub const CANCEL_LABEL: &str = "Cancel";

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
                input type="hidden" name="csrf" value=(view.csrf_token);
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
    .into_string()
}

/// The just-declined notice and its Undo (AC-006.2 / AC-006.5).
fn declined_notice(key: &SuggestionKey, csrf_token: &str) -> Markup {
    html! {
        p role="status" { (DECLINED_NOTICE) }
        form method="post" action="/review/undo" {
            input type="hidden" name="csrf" value=(csrf_token);
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
        @if let Some(notice) = scan_notice(view.latest_scan, view.pending, &view.tally) {
            p role="status" { (notice) }
        }
        @if view.tally.published > 0 {
            p { (view.pending.len()) " pending · " (view.tally.published) " published" }
        }
        @if let Some(label) = scan_action(view.latest_scan) {
            form method="post" action="/scan" {
                input type="hidden" name="csrf" value=(view.csrf_token);
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
                        "Your DID is no longer in github.com/" (login) "'s bio, or that account \
                         changed hands, so nothing was scanned. Your pending suggestions are \
                         hidden until you verify again: put your DID back in the bio, then verify."
                    }
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::Verified { login } => {
                    (verified_queue(login, view))
                }
            }
            form method="post" action="/signout" {
                input type="hidden" name="csrf" value=(view.csrf_token);
                button type="submit" { (SIGN_OUT_LABEL) }
            }
        },
    )
    .into_string()
}

/// The confirm button of the exact-record preview.
pub const PUBLISH_LABEL: &str = "Publish to my repo";

/// What the preview says a published claim is (and is not).
pub const NOT_AS_TRUTH_NOTICE: &str =
    "Publishing records this as your own claim, signed by your repo: your view, not as truth.";

/// The exact-record preview opened by Approve (AC-004.4): the destination and
/// every field to be written, confidence as shown and as stored, provenance
/// "self-attested". Nothing is written here; only the confirm form writes.
pub fn approval_preview_page(plan: &PublishPlan, csrf_token: &str) -> String {
    let claim = plan.claim();
    let basis_points = u16::try_from(claim.confidence.basis_points()).unwrap_or(u16::MAX);
    page(
        "Preview before publishing",
        html! {
            h1 { "Preview before publishing" }
            p { (suggestion_headline(plan.key())) }
            p { "Destination: your repo " (plan.owner_did()) " on your own PDS, at " (plan.at_uri()) }
            dl {
                dt { "subject" } dd { (claim.subject) }
                dt { "predicate" } dd { (claim.predicate) }
                dt { "object" } dd { (claim.object) }
                dt { "evidence" }
                dd {
                    @for url in &claim.evidence {
                        a href=(url) { (url) } " "
                    }
                }
                dt { "confidence" }
                dd {
                    (confidence_text(basis_points)) " (" (bucket_label(basis_points)) ")"
                    " — stored as " (basis_points)
                }
                dt { "author" } dd { (claim.author_did.0) }
                dt { "composedAt" } dd { (claim.composed_at) }
                dt { "provenance" } dd { "self-attested (no app signature; your repo attests it)" }
            }
            p { strong { (NOT_AS_TRUTH_NOTICE) } }
            p { "Nothing has been published yet." }
            form method="post" action="/review/publish" {
                input type="hidden" name="csrf" value=(csrf_token);
                input type="hidden" name="plan" value=(plan.rkey());
                button type="submit" { (PUBLISH_LABEL) }
            }
            p { a href="/review" { "Back" } }
        },
    )
    .into_string()
}

/// The confirmation after a publish (AC-004.5): the record's address and
/// the way back.
pub fn published_page(at_uri: &str) -> String {
    page(
        "Published",
        html! {
            h1 { "Published to your repo" }
            p { "Your claim is in your own repo at " code { (at_uri) } }
            p {
                "To retract it, open your profile and press Retract: a retraction is a                  new record in your repo that points at this one."
            }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

/// The button that tries a failed publish again.
pub const RETRY_LABEL: &str = "Retry";

/// What a Retry re-submits: the same plan, under the page's anti-forgery
/// token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishRetry<'a> {
    pub plan_id: &'a str,
    pub csrf_token: &'a str,
}

/// What the owner can do after a publish did not land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishNextStep<'a> {
    /// The plan is kept: Retry publishes it (exactly once).
    Retry(PublishRetry<'a>),
    /// The PDS session ended: sign in again, then publish from the queue.
    SignInAgain,
    /// The plan could not be kept: preview again from the queue.
    BackToQueue,
}

/// A publish that did not land (AC-004.6): nothing was written, the
/// suggestion stays pending, and the owner can try again.
pub fn publish_failed_page(reason: &str, next_step: PublishNextStep<'_>) -> String {
    page(
        "Nothing was published",
        html! {
            h1 { "Nothing was published" }
            p role="alert" { (reason) }
            p { "Your suggestion is still pending." }
            @match next_step {
                PublishNextStep::Retry(retry) => {
                    form method="post" action="/review/publish" {
                        input type="hidden" name="csrf" value=(retry.csrf_token);
                        input type="hidden" name="plan" value=(retry.plan_id);
                        button type="submit" { (RETRY_LABEL) }
                    }
                }
                PublishNextStep::SignInAgain => {
                    p { a href="/" { (SIGN_IN_LABEL) } }
                }
                PublishNextStep::BackToQueue => {}
            }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

/// A confirm that arrived after its preview expired (ADR-074): refused,
/// nothing written.
pub fn plan_expired_page() -> String {
    page(
        "This preview has expired",
        html! {
            h1 { "This preview has expired" }
            p role="alert" {
                "Previews can be published for 30 minutes. Nothing was published; \
                 your suggestion is still pending. Press Approve again to see a fresh preview."
            }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

/// A page that does not exist for this person (never says whose it is).
pub fn not_found_page() -> String {
    page(
        "Not found",
        html! {
            h1 { "Not found" }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

// =============================================================================
// The public profile (US-BRA-007): published claims only, read live
// =============================================================================

/// Who a `/@{segment}` profile URL names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSubject {
    Handle(String),
    Did(String),
}

/// The subject of a profile URL segment: a DID (`did:plc:` / `did:web:`)
/// or a well-formed handle. Anything else names nobody.
pub fn profile_subject(segment: &str) -> Option<ProfileSubject> {
    let did_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_');
    let is_did = ["did:plc:", "did:web:"]
        .iter()
        .any(|method| segment.len() > method.len() && segment.starts_with(method))
        && segment.chars().all(did_char);
    if is_did {
        Some(ProfileSubject::Did(segment.to_string()))
    } else {
        crate::signin::parse_handle(segment).map(|h| ProfileSubject::Handle(h.as_str().to_string()))
    }
}

/// One published claim as the profile shows it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublishedClaim {
    pub subject: String,
    pub object: String,
    pub confidence_bp: u16,
    pub rkey: String,
}

/// The claims a profile shows (pure): records of the owner's own repo whose
/// ADR-071 verdict is self-attested (author is the repo DID, rkey is the
/// claim's CID, fetched from the owner's own PDS), minus self-retracted
/// claims and the retraction records themselves, in a deterministic order.
pub fn published_claims(
    owner_did: &str,
    records: &[RepoRecord],
    origin: RecordOrigin,
) -> Vec<PublishedClaim> {
    let owner = Did(owner_did.to_string());
    let accepted: Vec<(&str, UnsignedClaim)> = records
        .iter()
        .filter(|record| record.repo_did == owner_did)
        .filter_map(|record| self_attested(record, &owner, origin))
        .collect();
    let lineages: Vec<ClaimLineage<'_>> = accepted
        .iter()
        .map(|(rkey, claim)| lineage(rkey, claim))
        .collect();
    let mut shown: Vec<PublishedClaim> = accepted
        .iter()
        .filter(|(rkey, claim)| {
            !is_retraction(claim) && !is_self_retracted(&lineage(rkey, claim), &lineages)
        })
        .map(|(rkey, claim)| shown_claim(rkey, claim))
        .collect();
    shown.sort();
    shown
}

/// The record's claim, if its provenance verdict is self-attested by `owner`.
fn self_attested<'r>(
    record: &'r RepoRecord,
    owner: &Did,
    origin: RecordOrigin,
) -> Option<(&'r str, UnsignedClaim)> {
    let claim = decode_claim_record(&record.value, "").ok()?;
    match provenance_verdict(&claim, &record.rkey, owner, origin, None) {
        Ok(Provenance::SelfAttested { .. }) => Some((&record.rkey, claim.unsigned().clone())),
        _ => None,
    }
}

fn lineage<'c>(rkey: &'c str, claim: &'c UnsignedClaim) -> ClaimLineage<'c> {
    ClaimLineage {
        author_did: &claim.author_did.0,
        cid: rkey,
        references: &claim.references,
    }
}

fn is_retraction(claim: &UnsignedClaim) -> bool {
    claim
        .references
        .iter()
        .any(|reference| reference.ref_type == ReferenceType::Retracts)
}

fn shown_claim(rkey: &str, claim: &UnsignedClaim) -> PublishedClaim {
    let basis_points = claim.confidence.basis_points().clamp(0, 10_000);
    PublishedClaim {
        subject: claim.subject.clone(),
        object: claim.object.clone(),
        confidence_bp: u16::try_from(basis_points).unwrap_or_default(),
        rkey: rkey.to_string(),
    }
}

/// The provenance label every profile claim carries (I-BRA-5).
pub const SELF_ATTESTED_LABEL: &str = "self-attested";

/// The owner's way from an empty profile to their queue.
pub const REVIEW_SUGGESTIONS_LABEL: &str = "Review suggestions →";

/// An unreachable PDS, stated plainly; nothing stale is shown instead.
pub const PDS_UNREACHABLE_NOTICE: &str =
    "We can't reach this person's PDS right now, so no claims are shown. Try again shortly.";

/// The identity directory could not be reached.
pub const DIRECTORY_UNREACHABLE_NOTICE: &str =
    "We can't look this person up right now. Try again shortly.";

/// What a profile can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileContent {
    Claims(Vec<PublishedClaim>),
    PdsUnreachable,
    DirectoryUnreachable,
}

/// A profile page: whose it is, whether its owner is looking, and what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileView<'a> {
    pub handle: &'a str,
    pub viewer_is_owner: bool,
    pub content: &'a ProfileContent,
}

fn profile_card(claim: &PublishedClaim) -> Markup {
    let repo = repo_path(&claim.subject);
    html! {
        article {
            h2 {
                a href=(format!("https://github.com/{repo}/")) { (repo) }
                " embodies " (philosophy_slug(&claim.object))
            }
            p {
                (SELF_ATTESTED_LABEL) " · Confidence " (confidence_text(claim.confidence_bp))
                " (" (bucket_label(claim.confidence_bp)) ")"
            }
        }
    }
}

/// The public profile (US-BRA-007): only what the PDS holds as published.
pub fn profile_page(view: &ProfileView<'_>) -> String {
    page(
        &format!("@{}", view.handle),
        html! {
            h1 { "@" (view.handle) }
            @match view.content {
                ProfileContent::PdsUnreachable => p role="alert" { (PDS_UNREACHABLE_NOTICE) },
                ProfileContent::DirectoryUnreachable => p role="alert" { (DIRECTORY_UNREACHABLE_NOTICE) },
                ProfileContent::Claims(claims) if claims.is_empty() => {
                    p { (view.handle) " hasn't published any claims yet." }
                    @if view.viewer_is_owner {
                        p { a href="/review" { (REVIEW_SUGGESTIONS_LABEL) } }
                    }
                },
                ProfileContent::Claims(claims) => {
                    @for claim in claims { (profile_card(claim)) }
                },
            }
        },
    )
    .into_string()
}

/// A profile URL that names nobody (PR-4): plainly not found.
pub fn profile_not_found_page() -> String {
    page(
        "Profile not found",
        html! {
            h1 { "Profile not found" }
            p { "No Bluesky account answers to that handle or DID." }
            p { a href="/" { (APP_NAME) } }
        },
    )
    .into_string()
}

/// What happened when the person pressed Verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipOutcome<'a> {
    Verified,
    Refused(&'a OwnershipRefusal),
}

/// The sentence explaining a verify outcome (AC-002.3: each names the fix).
pub fn ownership_message(
    outcome: OwnershipOutcome<'_>,
    login: &str,
    session_did: &str,
    handle: &str,
) -> String {
    let refusal = match outcome {
        OwnershipOutcome::Verified => {
            return format!("Verified: github.com/{login} belongs to @{handle}")
        }
        OwnershipOutcome::Refused(refusal) => refusal,
    };
    match refusal {
        OwnershipRefusal::DidMissing => format!(
            "We couldn't find your DID in the bio of github.com/{login}. Add your DID to the \
             bio of your own GitHub account, then press Verify again."
        ),
        OwnershipRefusal::NoBio => format!(
            "We couldn't find a public bio on github.com/{login}. Add your DID to the bio of \
             your own GitHub account, then press Verify again."
        ),
        OwnershipRefusal::DifferentDid(found) => format!(
            "The bio of github.com/{login} holds a different DID ({found}). It must match your \
             signed-in account ({session_did}): replace it with your DID, then press Verify again."
        ),
        OwnershipRefusal::IdentityChanged => format!(
            "github.com/{login} now belongs to a different GitHub account than the one you \
             verified. Please verify again."
        ),
        OwnershipRefusal::AccountNotFound => format!(
            "We couldn't find a GitHub account at github.com/{login}. Check the spelling, then \
             press Verify again."
        ),
        OwnershipRefusal::RateLimited => "GitHub is rate-limiting us right now. Please wait a \
             few minutes, then press Verify again. Nothing was lost."
            .to_string(),
        OwnershipRefusal::GithubUnavailable => "GitHub could not be reached. Please wait a few \
             minutes, then press Verify again. Nothing was lost."
            .to_string(),
        OwnershipRefusal::LinkedToAnotherAccount => format!(
            "github.com/{login} is already verified for another Bluesky account. Each GitHub \
             account can belong to one Bluesky account only."
        ),
        OwnershipRefusal::TooManyAttempts => "Too many verification attempts in the last hour. \
             Please try again later. Nothing was lost."
            .to_string(),
    }
}

/// The GitHub step: the exact DID to copy, where to put it, and the form
/// that checks it. `message` explains the last attempt.
pub fn github_page(session_did: &str, csrf_token: &str, message: Option<&str>) -> String {
    page(
        "Prove your GitHub account",
        html! {
            h1 { "Prove your GitHub account is yours" }
            @if let Some(message) = message {
                p role="status" { (message) }
            }
            p { "Your DID:" }
            p { code id="your-did" { (session_did) } " " a href="#your-did" data-copy=(session_did) { "Copy" } }
            p {
                "Add this exact DID anywhere in the bio of your GitHub profile \
                 (github.com/settings/profile). It shows that you, the person signed in \
                 here, control that GitHub account. Then enter your GitHub username and \
                 press Verify."
            }
            form method="post" action="/github" {
                input type="hidden" name="csrf" value=(csrf_token);
                label for="github_login" { "GitHub username" }
                input id="github_login" name="github_login" type="text" required;
                button type="submit" { (VERIFY_LABEL) }
            }
            p { a href="/review" { "Back to your review queue" } }
            script src="/assets/copy.js" {}
        },
    )
    .into_string()
}

/// The scan-status fragment the queue polls (`data-scan-status`).
pub fn scan_status_page(status: &str) -> String {
    page(
        "Scan status",
        html! {
            div data-scan-status=(status) { "Scan status: " (status) }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

/// The page script: copy a `data-copy` value to the clipboard.
pub const COPY_SCRIPT: &str = "document.querySelectorAll('[data-copy]').forEach(function (el) {\n\
  el.addEventListener('click', function (event) {\n\
    event.preventDefault();\n\
    navigator.clipboard.writeText(el.getAttribute('data-copy'));\n\
  });\n\
});\n";

/// The queue's keyboard triage (A approve, E edit, N not me, J/K move).
pub const TRIAGE_SCRIPT: &str = "(function () {\n\
  var cards = function () { return Array.prototype.slice.call(document.querySelectorAll('article')); };\n\
  var current = 0;\n\
  var focus = function (i) { var c = cards(); if (!c.length) return; current = Math.max(0, Math.min(i, c.length - 1)); var b = c[current].querySelector('button'); if (b) b.focus(); };\n\
  document.addEventListener('keydown', function (event) {\n\
    if (event.target && event.target.tagName === 'INPUT') return;\n\
    var key = event.key.toLowerCase();\n\
    if (key === 'j') focus(current + 1);\n\
    else if (key === 'k') focus(current - 1);\n\
    else { var c = cards()[current]; var b = c && c.querySelector('[data-key=\"' + key + '\"]'); if (b) b.click(); }\n\
  });\n\
})();\n";

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
    }

    const OWNER: &str = "did:plc:owner";

    /// How one generated record departs (or not) from a self-attested claim
    /// of the owner's own repo.
    #[derive(Debug, Clone, Copy)]
    struct RecordShape {
        in_owner_repo: bool,
        author: u8,
        signed: bool,
        rkey_is_cid: bool,
        confidence_bp: u16,
    }

    fn record_shape() -> impl Strategy<Value = RecordShape> {
        (
            any::<bool>(),
            0u8..3,
            any::<bool>(),
            any::<bool>(),
            0u16..=10_000,
        )
            .prop_map(
                |(in_owner_repo, author, signed, rkey_is_cid, confidence_bp)| RecordShape {
                    in_owner_repo,
                    author,
                    signed,
                    rkey_is_cid,
                    confidence_bp,
                },
            )
    }

    fn claim_body(index: usize, shape: RecordShape, retracts: Option<&str>) -> serde_json::Value {
        let author = match shape.author {
            0 => OWNER.to_string(),
            1 => "did:plc:someone-else".to_string(),
            _ => format!("{OWNER}#org.openlore.application"),
        };
        let references: Vec<serde_json::Value> = retracts
            .map(|cid| serde_json::json!({"type": "retracts", "cid": cid}))
            .into_iter()
            .collect();
        let mut body = serde_json::json!({
            "subject": format!("github:priyaraman/repo{index}"),
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.memory-safety",
            "evidence": [],
            "confidence": shape.confidence_bp,
            "author": author,
            "composedAt": "2026-10-04T15:02:11Z",
            "references": references,
        });
        if shape.signed {
            body["signature"] = serde_json::json!({"kid": format!("{OWNER}#org.openlore.application"), "alg": "EdDSA", "sig": "AAAA"});
        }
        body
    }

    fn cid_of(body: &serde_json::Value) -> String {
        decode_claim_record(body, "")
            .expect("decodable")
            .cid()
            .0
            .clone()
    }

    fn repo_record(body: serde_json::Value, shape: RecordShape) -> RepoRecord {
        let cid = cid_of(&body);
        RepoRecord {
            repo_did: if shape.in_owner_repo {
                OWNER.to_string()
            } else {
                "did:plc:elsewhere".to_string()
            },
            rkey: if shape.rkey_is_cid {
                cid
            } else {
                format!("{cid}x")
            },
            value: body,
        }
    }

    fn admissible(shape: RecordShape) -> bool {
        shape.in_owner_repo && shape.author == 0 && !shape.signed && shape.rkey_is_cid
    }

    proptest! {
        /// Universe: up to 8 records varying in repo, author, signature, rkey
        /// and confidence, read from the owner's PDS or elsewhere, with an
        /// optional owner retraction of one of them. Exactly the
        /// self-attested claims of the owner's own repo, fetched from the
        /// owner's PDS and not retracted, appear, sorted, whatever the
        /// listing order.
        #[test]
        fn only_verdict_accepted_unretracted_claims_of_the_owner_appear(
            shapes in proptest::collection::vec(record_shape(), 0..8),
            from_author_pds in any::<bool>(),
            retracted in proptest::option::of(0usize..8),
        ) {
            let bodies: Vec<serde_json::Value> =
                shapes.iter().enumerate().map(|(i, s)| claim_body(i, *s, None)).collect();
            let mut records: Vec<RepoRecord> =
                bodies.iter().zip(&shapes).map(|(b, s)| repo_record(b.clone(), *s)).collect();
            let retracted = retracted.filter(|i| *i < shapes.len());
            let marker_shape = RecordShape { in_owner_repo: true, author: 0, signed: false, rkey_is_cid: true, confidence_bp: 0 };
            if let Some(i) = retracted {
                let original_cid = records[i].rkey.clone();
                records.push(repo_record(claim_body(100, marker_shape, Some(&original_cid)), marker_shape));
            }
            let origin = if from_author_pds { RecordOrigin::AuthorPds } else { RecordOrigin::Relay };

            let mut expected: Vec<PublishedClaim> = shapes
                .iter()
                .enumerate()
                .filter(|(i, s)| from_author_pds && admissible(**s) && Some(*i) != retracted)
                .map(|(i, s)| PublishedClaim {
                    subject: format!("github:priyaraman/repo{i}"),
                    object: "org.openlore.philosophy.memory-safety".to_string(),
                    confidence_bp: s.confidence_bp,
                    rkey: records[i].rkey.clone(),
                })
                .collect();
            expected.sort();

            prop_assert_eq!(&published_claims(OWNER, &records, origin), &expected);
            records.reverse();
            prop_assert_eq!(&published_claims(OWNER, &records, origin), &expected);
        }

        /// Universe: profile contents and viewers. Every shown claim is
        /// labelled self-attested, never unverified; the owner's call to
        /// action appears only to the owner of an empty profile.
        #[test]
        fn every_shown_claim_is_self_attested_and_only_an_empty_owner_profile_offers_review(
            confidences in proptest::collection::vec(0u16..=10_000, 0..4),
            viewer_is_owner in any::<bool>(),
        ) {
            let claims: Vec<PublishedClaim> = confidences
                .iter()
                .enumerate()
                .map(|(i, bp)| PublishedClaim {
                    subject: format!("github:priyaraman/repo{i}"),
                    object: "org.openlore.philosophy.test-driven".to_string(),
                    confidence_bp: *bp,
                    rkey: format!("bafy{i}"),
                })
                .collect();
            let content = ProfileContent::Claims(claims.clone());
            let html = profile_page(&ProfileView { handle: "priyaraman.bsky.social", viewer_is_owner, content: &content });
            prop_assert_eq!(html.matches("<article>").count(), claims.len());
            prop_assert_eq!(html.matches(SELF_ATTESTED_LABEL).count(), claims.len());
            prop_assert!(!html.contains("unverified"));
            prop_assert_eq!(html.contains(REVIEW_SUGGESTIONS_LABEL), viewer_is_owner && claims.is_empty());
        }

        /// Universe: arbitrary URL segments. A segment names a DID only with
        /// a `did:plc:`/`did:web:` method; otherwise it is a handle or nobody.
        #[test]
        fn a_profile_segment_names_a_did_a_handle_or_nobody(segment in "(did:(plc|web|key):)?[a-z0-9.:-]{0,24}") {
            match profile_subject(&segment) {
                Some(ProfileSubject::Did(did)) => {
                    prop_assert!(did.starts_with("did:plc:") || did.starts_with("did:web:"));
                }
                Some(ProfileSubject::Handle(handle)) => prop_assert!(handle.contains('.')),
                None => {}
            }
        }
    }

    /// An unreachable PDS is stated and shows no claims.
    // bypass: a closed two-variant outcome; one example per arm is the universe.
    #[test]
    fn an_unreachable_pds_or_directory_is_stated_with_no_claims() {
        for (content, notice) in [
            (ProfileContent::PdsUnreachable, PDS_UNREACHABLE_NOTICE),
            (
                ProfileContent::DirectoryUnreachable,
                DIRECTORY_UNREACHABLE_NOTICE,
            ),
        ] {
            let html = profile_page(&ProfileView {
                handle: "priyaraman.bsky.social",
                viewer_is_owner: false,
                content: &content,
            });
            assert!(html.contains(notice));
            assert!(!html.contains("<article>"));
        }
    }
}
