//! Aggregate KPI counting (kpi-instrumentation §2, OD-BRA-11). Pure.
//!
//! A counter is a [`KpiEvent`] — a closed catalogue whose names are
//! `&'static str` — so a counter cannot carry a DID, a handle or claim
//! content by construction. Reading counters back keeps only catalogue
//! names, so nothing else can reach the operator either.

use std::collections::BTreeMap;

use ports::claim_domain::authority::SigningRefusal;
use ports::claim_domain::UnsignedClaim;
use ports::{AccountCheckOutcome, AccountRefusal, KpiCounterRow, OutsideDefault, Suggestion};

use crate::ownership::OwnershipRefusal;
use crate::repo_scope::ImportRefusal;
use crate::signin::SignInFailure;

/// Every counted event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KpiEvent {
    SignInStarted,
    SignInCompleted,
    SignInDenied,
    SignInFailed,
    GithubVerifyOk,
    GithubVerifyFail,
    ScanCompleted,
    SuggestionApproved,
    SuggestionApprovedEdited,
    SuggestionDeclined,
    /// KPI-RRA-1: an action on a suggestion today's top 5 would never have
    /// produced.
    SuggestionActedOutsideDefault,
    /// KPI-RRA-1 restricted to suggestions from an account other than the
    /// owner's first (UC-RRA-7).
    SuggestionActedOutsideDefaultOtherAccount,
    /// KPI-RRA-1 (and KPI-RRA-3) restricted to suggestions from a repo the
    /// owner imported.
    SuggestionActedOutsideDefaultImported,
    /// The choose-repos page was opened.
    ReposChoiceViewed,
    /// A Save stored a repo choice.
    ReposChoiceSaved,
    /// A stored repo choice ticks something other than today's top 5.
    ReposChoiceChangedFromDefault,
    /// A Save was refused: the scan's total would pass the limit.
    ReposChoiceRefusedOverLimit,
    /// A scan was refused before admission: nothing to read (AC-002.5).
    ScanRefusedNothingToRead,
    /// A scan was refused before admission: past the limit (BR-RRA-3).
    ScanRefusedOverLimit,
    /// KPI-RRA-2: an owner who lists an account pressed Verify on another.
    AccountAddStarted,
    /// KPI-RRA-2: that account was added, verified.
    AccountAddVerified,
    AccountAddRefusedDidMissing,
    AccountAddRefusedHeldByAnother,
    AccountAddRefusedAlreadyListed,
    AccountAddRefusedAtLimit,
    AccountAddRefusedUnreachable,
    /// A scan's bio check of one listed account failed (D-RRA-T1).
    ScanAccountFailed,
    /// A scan's bio check of one listed account did not finish (AC-004.8).
    ScanAccountUnfinished,
    /// An owner removed one of her GitHub accounts (US-RRA-008).
    AccountRemoved,
    /// KPI-RRA-3: an owner named a public repo she doesn't own (US-RRA-005).
    ImportAdded,
    /// An owner removed one of her imports (US-RRA-009).
    ImportRemoved,
    /// KPI-RRA-3 retention: a removed import was held under a day,
    ImportRemovedAgeLt1d,
    /// under 14 days,
    ImportRemovedAgeLt14d,
    /// or 14 days or longer.
    ImportRemovedAgeGe14d,
    /// An import moved to the owned list of the account that owns it now
    /// (BR-RRA-5).
    ImportMovedToOwned,
    /// "Add repo" was refused: the name is not `owner/name`.
    ImportRefusedShape,
    /// "Add repo" was refused: no public repo is there.
    ImportRefusedNotFound,
    /// "Add repo" was refused: a listed account owns the repo.
    ImportRefusedOwned,
    /// "Add repo" was refused: the scan's total would pass the limit.
    ImportRefusedOverLimit,
    /// "Add repo" was refused: GitHub could not answer.
    ImportRefusedUnreachable,
    /// KPI-RRA-4: a publish was refused because none of the owner's listed
    /// accounts owns the repo now (another account or an organization does).
    PublishRefusedNotOwned,
    /// KPI-RRA-4: a publish was refused because the owner account's bio,
    /// read in that request, no longer holds the signer's DID.
    PublishRefusedBioFailed,
    /// KPI-RRA-4: a publish was refused because GitHub could not answer.
    PublishRefusedUnreachable,
    SharePosted,
    RetractPosted,
    Disconnect,
}

impl KpiEvent {
    pub const ALL: [KpiEvent; 46] = [
        Self::SignInStarted,
        Self::SignInCompleted,
        Self::SignInDenied,
        Self::SignInFailed,
        Self::GithubVerifyOk,
        Self::GithubVerifyFail,
        Self::ScanCompleted,
        Self::SuggestionApproved,
        Self::SuggestionApprovedEdited,
        Self::SuggestionDeclined,
        Self::SuggestionActedOutsideDefault,
        Self::SuggestionActedOutsideDefaultOtherAccount,
        Self::SuggestionActedOutsideDefaultImported,
        Self::ReposChoiceViewed,
        Self::ReposChoiceSaved,
        Self::ReposChoiceChangedFromDefault,
        Self::ReposChoiceRefusedOverLimit,
        Self::ScanRefusedNothingToRead,
        Self::ScanRefusedOverLimit,
        Self::AccountAddStarted,
        Self::AccountAddVerified,
        Self::AccountAddRefusedDidMissing,
        Self::AccountAddRefusedHeldByAnother,
        Self::AccountAddRefusedAlreadyListed,
        Self::AccountAddRefusedAtLimit,
        Self::AccountAddRefusedUnreachable,
        Self::ScanAccountFailed,
        Self::ScanAccountUnfinished,
        Self::AccountRemoved,
        Self::ImportAdded,
        Self::ImportRemoved,
        Self::ImportRemovedAgeLt1d,
        Self::ImportRemovedAgeLt14d,
        Self::ImportRemovedAgeGe14d,
        Self::ImportMovedToOwned,
        Self::ImportRefusedShape,
        Self::ImportRefusedNotFound,
        Self::ImportRefusedOwned,
        Self::ImportRefusedOverLimit,
        Self::ImportRefusedUnreachable,
        Self::PublishRefusedNotOwned,
        Self::PublishRefusedBioFailed,
        Self::PublishRefusedUnreachable,
        Self::SharePosted,
        Self::RetractPosted,
        Self::Disconnect,
    ];

    /// The `kpi_counters.event` name.
    pub fn name(self) -> &'static str {
        match self {
            Self::SignInStarted => "signin.started",
            Self::SignInCompleted => "signin.completed",
            Self::SignInDenied => "signin.denied",
            Self::SignInFailed => "signin.failed",
            Self::GithubVerifyOk => "github.verify.ok",
            Self::GithubVerifyFail => "github.verify.fail",
            Self::ScanCompleted => "scan.completed",
            Self::SuggestionApproved => "suggestion.approved",
            Self::SuggestionApprovedEdited => "suggestion.approved.edited",
            Self::SuggestionDeclined => "suggestion.declined",
            Self::SuggestionActedOutsideDefault => "suggestion.acted.outside_default",
            Self::SuggestionActedOutsideDefaultOtherAccount => {
                "suggestion.acted.outside_default.other_account"
            }
            Self::SuggestionActedOutsideDefaultImported => {
                "suggestion.acted.outside_default.imported"
            }
            Self::ReposChoiceViewed => "repos.choice.viewed",
            Self::ReposChoiceSaved => "repos.choice.saved",
            Self::ReposChoiceChangedFromDefault => "repos.choice.changed_from_default",
            Self::ReposChoiceRefusedOverLimit => "repos.choice.refused.over_limit",
            Self::ScanRefusedNothingToRead => "scan.refused.nothing_to_read",
            Self::ScanRefusedOverLimit => "scan.refused.over_limit",
            Self::AccountAddStarted => "account.add.started",
            Self::AccountAddVerified => "account.add.verified",
            Self::AccountAddRefusedDidMissing => "account.add.refused.did_missing",
            Self::AccountAddRefusedHeldByAnother => "account.add.refused.held_by_another",
            Self::AccountAddRefusedAlreadyListed => "account.add.refused.already_listed",
            Self::AccountAddRefusedAtLimit => "account.add.refused.at_limit",
            Self::AccountAddRefusedUnreachable => "account.add.refused.unreachable",
            Self::ScanAccountFailed => "scan.account.failed",
            Self::ScanAccountUnfinished => "scan.account.unfinished",
            Self::AccountRemoved => "account.removed",
            Self::ImportAdded => "import.added",
            Self::ImportRemoved => "import.removed",
            Self::ImportRemovedAgeLt1d => "import.removed.age.lt_1d",
            Self::ImportRemovedAgeLt14d => "import.removed.age.lt_14d",
            Self::ImportRemovedAgeGe14d => "import.removed.age.ge_14d",
            Self::ImportMovedToOwned => "import.moved_to_owned",
            Self::ImportRefusedShape => "import.refused.shape",
            Self::ImportRefusedNotFound => "import.refused.not_found",
            Self::ImportRefusedOwned => "import.refused.owned",
            Self::ImportRefusedOverLimit => "import.refused.over_limit",
            Self::ImportRefusedUnreachable => "import.refused.unreachable",
            Self::PublishRefusedNotOwned => "publish.refused.not_owned",
            Self::PublishRefusedBioFailed => "publish.refused.bio_failed",
            Self::PublishRefusedUnreachable => "publish.refused.unreachable",
            Self::SharePosted => "share.posted",
            Self::RetractPosted => "retract.posted",
            Self::Disconnect => "disconnect",
        }
    }

    fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.name() == name)
    }
}

/// How a sign-in that did not complete counts.
pub fn sign_in_refusal_event(failure: SignInFailure) -> KpiEvent {
    match failure {
        SignInFailure::Cancelled => KpiEvent::SignInDenied,
        _ => KpiEvent::SignInFailed,
    }
}

/// Why adding another account was refused, as KPI-RRA-2 counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAddRefused {
    DidMissing,
    HeldByAnother,
    AlreadyListed,
    AtLimit,
    Unreachable,
}

impl AccountAddRefused {
    /// The refusal of an account that may not join the list.
    pub fn of_listing(refusal: AccountRefusal) -> Self {
        match refusal {
            AccountRefusal::AlreadyListed => Self::AlreadyListed,
            AccountRefusal::HeldByAnotherOwner => Self::HeldByAnother,
            AccountRefusal::AtLimit => Self::AtLimit,
        }
    }

    /// The refusal of a bio check, when KPI-RRA-2 counts it (a mistyped
    /// login or the hourly budget is not counted).
    pub fn of_check(refusal: &OwnershipRefusal) -> Option<Self> {
        match refusal {
            OwnershipRefusal::DidMissing
            | OwnershipRefusal::NoBio
            | OwnershipRefusal::DifferentDid(_)
            | OwnershipRefusal::IdentityChanged => Some(Self::DidMissing),
            OwnershipRefusal::LinkedToAnotherAccount => Some(Self::HeldByAnother),
            OwnershipRefusal::RateLimited | OwnershipRefusal::GithubUnavailable => {
                Some(Self::Unreachable)
            }
            OwnershipRefusal::AccountNotFound | OwnershipRefusal::TooManyAttempts => None,
        }
    }

    pub fn event(self) -> KpiEvent {
        match self {
            Self::DidMissing => KpiEvent::AccountAddRefusedDidMissing,
            Self::HeldByAnother => KpiEvent::AccountAddRefusedHeldByAnother,
            Self::AlreadyListed => KpiEvent::AccountAddRefusedAlreadyListed,
            Self::AtLimit => KpiEvent::AccountAddRefusedAtLimit,
            Self::Unreachable => KpiEvent::AccountAddRefusedUnreachable,
        }
    }

    /// The operator-facing reason.
    pub fn label(self) -> &'static str {
        match self {
            Self::DidMissing => "did_missing",
            Self::HeldByAnother => "held_by_another",
            Self::AlreadyListed => "already_listed",
            Self::AtLimit => "at_limit",
            Self::Unreachable => "unreachable",
        }
    }
}

/// What a refused publish counts (KPI-RRA-4).
pub fn publish_refusal_events(refusal: &SigningRefusal) -> Vec<KpiEvent> {
    match refusal {
        SigningRefusal::NotOwnedByListedAccount | SigningRefusal::OrgOwned => {
            vec![KpiEvent::PublishRefusedNotOwned]
        }
        SigningRefusal::BioNoLongerHolds(_) => vec![KpiEvent::PublishRefusedBioFailed],
        SigningRefusal::Unfinished => vec![KpiEvent::PublishRefusedUnreachable],
        SigningRefusal::RepoGone | SigningRefusal::RepoRenamed { .. } => Vec::new(),
    }
}

/// The age bucket a removed import held for `held_secs` counts (KPI-RRA-3:
/// imports kept 14 days or longer); never which repo or whose.
pub fn import_removed_age_event(held_secs: u64) -> KpiEvent {
    const DAY_SECS: u64 = 24 * 60 * 60;
    match held_secs / DAY_SECS {
        0 => KpiEvent::ImportRemovedAgeLt1d,
        1..=13 => KpiEvent::ImportRemovedAgeLt14d,
        _ => KpiEvent::ImportRemovedAgeGe14d,
    }
}

/// What a refused "Add repo" counts (an already-imported repo is not counted).
pub fn import_refusal_event(refusal: ImportRefusal) -> Option<KpiEvent> {
    match refusal {
        ImportRefusal::Shape => Some(KpiEvent::ImportRefusedShape),
        ImportRefusal::NotFound => Some(KpiEvent::ImportRefusedNotFound),
        ImportRefusal::Unreachable => Some(KpiEvent::ImportRefusedUnreachable),
        ImportRefusal::OwnedByListedAccount => Some(KpiEvent::ImportRefusedOwned),
        ImportRefusal::OverLimit(_) => Some(KpiEvent::ImportRefusedOverLimit),
        ImportRefusal::AlreadyImported => None,
    }
}

/// The operator-facing reason of a refused publish (never whose or which).
pub fn publish_refusal_label(refusal: &SigningRefusal) -> &'static str {
    match refusal {
        SigningRefusal::NotOwnedByListedAccount => "not_owned",
        SigningRefusal::OrgOwned => "org_owned",
        SigningRefusal::RepoGone => "repo_gone",
        SigningRefusal::RepoRenamed { .. } => "repo_renamed",
        SigningRefusal::BioNoLongerHolds(_) => "bio_failed",
        SigningRefusal::Unfinished => "unreachable",
    }
}

/// The published claim is not the suggestion as offered: its philosophy or
/// its confidence was changed before approval (KPI-BRA-3).
pub fn approval_was_edited(published: &UnsignedClaim, offered: Option<&Suggestion>) -> bool {
    offered.is_some_and(|offered| {
        published.object != offered.key.object
            || published.confidence.basis_points() != i64::from(offered.confidence_bp)
    })
}

/// The counters one confirmed approval adds: always `suggestion.approved`,
/// plus `suggestion.approved.edited` when it was edited.
pub fn approval_events(edited: bool) -> Vec<KpiEvent> {
    std::iter::once(KpiEvent::SuggestionApproved)
        .chain(edited.then_some(KpiEvent::SuggestionApprovedEdited))
        .collect()
}

/// The counters an owner's action on a suggestion adds for where it came
/// from: one `suggestion.acted.outside_default` when today's top 5 would
/// never have produced it (KPI-RRA-1), plus `.other_account` when it came
/// from an account other than her first.
pub fn outside_default_events(outside_default: Option<OutsideDefault>) -> Vec<KpiEvent> {
    match outside_default {
        None => Vec::new(),
        Some(OutsideDefault::OwnedChoice) => vec![KpiEvent::SuggestionActedOutsideDefault],
        Some(OutsideDefault::OtherAccount) => vec![
            KpiEvent::SuggestionActedOutsideDefault,
            KpiEvent::SuggestionActedOutsideDefaultOtherAccount,
        ],
    }
}

/// Where a suggestion the owner acts on came from, as KPI-RRA-1 counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActedOrigin {
    /// A repo today's default top 5 of her first account would read.
    Default,
    /// A repo of one of her accounts outside that default.
    Outside(OutsideDefault),
    /// A repo she imported.
    Imported,
}

/// The KPI-RRA-1 counters an action on a suggestion from `origin` adds.
pub fn acted_origin_events(origin: ActedOrigin) -> Vec<KpiEvent> {
    match origin {
        ActedOrigin::Default => Vec::new(),
        ActedOrigin::Outside(outside_default) => outside_default_events(Some(outside_default)),
        ActedOrigin::Imported => vec![
            KpiEvent::SuggestionActedOutsideDefault,
            KpiEvent::SuggestionActedOutsideDefaultImported,
        ],
    }
}

/// The counters a decline adds: `suggestion.declined`, plus KPI-RRA-1's
/// when the suggestion came from outside the default ("act on = approve,
/// edit or decline").
pub fn decline_events(origin: ActedOrigin) -> Vec<KpiEvent> {
    std::iter::once(KpiEvent::SuggestionDeclined)
        .chain(acted_origin_events(origin))
        .collect()
}

/// The counter one scan's bio check of an account adds: a failed or an
/// unfinished check counts; a passed one does not.
pub fn account_check_event(outcome: AccountCheckOutcome) -> Option<KpiEvent> {
    match outcome {
        AccountCheckOutcome::Passed => None,
        AccountCheckOutcome::Failed => Some(KpiEvent::ScanAccountFailed),
        AccountCheckOutcome::NotReached => Some(KpiEvent::ScanAccountUnfinished),
    }
}

/// The counters a stored Save adds: always `repos.choice.saved`, plus
/// `repos.choice.changed_from_default` when it differs from today's top 5.
pub fn choice_saved_events(changed_from_default: bool) -> Vec<KpiEvent> {
    std::iter::once(KpiEvent::ReposChoiceSaved)
        .chain(changed_from_default.then_some(KpiEvent::ReposChoiceChangedFromDefault))
        .collect()
}

/// A validated `YYYY-MM-DD` day.
pub fn parse_day(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && digits(0..4)
        && digits(5..7)
        && digits(8..10);
    shaped.then_some(text)
}

/// The UTC day (`YYYY-MM-DD`) of a unix time.
pub fn utc_day(unix_secs: i64) -> String {
    crate::plans::rfc3339_utc(unix_secs)[..10].to_string()
}

/// Sum the counters of a window, per catalogue event. Rows naming anything
/// outside the catalogue are dropped, so the answer names only events.
pub fn sum_counters(rows: &[KpiCounterRow]) -> BTreeMap<&'static str, i64> {
    rows.iter()
        .filter_map(|row| KpiEvent::of_name(&row.event).map(|event| (event.name(), row.count)))
        .fold(BTreeMap::new(), |mut sums, (name, count)| {
            *sums.entry(name).or_insert(0) += count;
            sums
        })
}

const DAY_SECS: i64 = 24 * 60 * 60;

/// The daily signals fire at 00:05 UTC (kpi-instrumentation §4).
const ROLLUP_OFFSET_SECS: i64 = 5 * 60;

/// Seconds from `now` until the next 00:05 UTC (never zero: a tick that
/// lands exactly on 00:05 waits for the next day's).
pub fn seconds_until_next_rollup(now: i64) -> i64 {
    let since = (now - ROLLUP_OFFSET_SECS).rem_euclid(DAY_SECS);
    DAY_SECS - since
}

/// The day a rollup at `now` reports: the UTC day that just ended.
pub fn rollup_day(now: i64) -> String {
    utc_day(now - DAY_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn event_name() -> impl Strategy<Value = String> {
        prop_oneof![
            proptest::sample::select(KpiEvent::ALL.to_vec()).prop_map(|e| e.name().to_string()),
            "did:plc:[a-z2-7]{24}",
            "[a-z.@-]{1,20}",
        ]
    }

    proptest! {
        /// Universe: arbitrary counter rows, including names that are a DID
        /// or free text. The sums name only catalogue events, each the sum
        /// of its own rows — identity never comes out.
        #[test]
        fn counters_never_carry_identity(
            rows in prop::collection::vec((event_name(), 0i64..100), 0..20)
        ) {
            let rows: Vec<KpiCounterRow> = rows
                .into_iter()
                .map(|(event, count)| KpiCounterRow { day: "2026-10-04".into(), event, count })
                .collect();
            let sums = sum_counters(&rows);
            for (name, sum) in &sums {
                prop_assert!(KpiEvent::of_name(name).is_some(), "{} is not a catalogue event", name);
                let own: i64 = rows.iter().filter(|r| r.event == *name).map(|r| r.count).sum();
                prop_assert_eq!(*sum, own);
            }
            prop_assert!(sums.keys().all(|name| !name.contains("did:")));
        }

        /// Universe: every instant. The next rollup is within a day, lands
        /// on 00:05 UTC, and reports the day before it.
        #[test]
        fn the_daily_signals_fire_once_a_day_at_five_past_midnight(now in 0i64..4_102_444_800) {
            let wait = seconds_until_next_rollup(now);
            prop_assert!(wait > 0 && wait <= DAY_SECS);
            let at = now + wait;
            prop_assert_eq!(at.rem_euclid(DAY_SECS), ROLLUP_OFFSET_SECS);
            prop_assert_eq!(rollup_day(at), utc_day(at - DAY_SECS));
            prop_assert_eq!(seconds_until_next_rollup(at), DAY_SECS);
        }

        /// Universe: offered suggestions and published claims. An approval
        /// always counts once, and counts as edited exactly when the
        /// philosophy or the confidence differ from what was offered.
        #[test]
        fn an_approval_counts_as_edited_exactly_when_it_was_changed(
            offered_bp in 0u16..=10_000, published_bp in 0u16..=10_000, same_object in any::<bool>()
        ) {
            use ports::claim_domain::{Confidence, Did};
            use ports::SuggestionKey;
            let offered = Suggestion {
                key: SuggestionKey { subject: "s".into(), predicate: "embodies".into(), object: "test-driven".into() },
                confidence_bp: offered_bp,
                evidence: vec![],
                why: vec![],
                source_repo: "o/r".into(),
            };
            let published = UnsignedClaim {
                subject: "s".into(),
                predicate: "embodies".into(),
                object: if same_object { "test-driven".into() } else { "memory-safety".into() },
                evidence: vec![],
                confidence: Confidence::from_basis_points(i64::from(published_bp)),
                author_did: Did("did:plc:x".into()),
                composed_at: "2026-10-04T00:00:00Z".into(),
                references: vec![],
                reason: None,
            };
            let events = approval_events(approval_was_edited(&published, Some(&offered)));
            let edited = !same_object
                || Confidence::from_basis_points(i64::from(published_bp)).basis_points() != i64::from(offered_bp);
            prop_assert_eq!(events[0], KpiEvent::SuggestionApproved);
            prop_assert_eq!(events.contains(&KpiEvent::SuggestionApprovedEdited), edited);
        }

        /// Universe: every origin of a declined suggestion (U-RRA-D5). A
        /// decline always counts once, and counts toward KPI-RRA-1 exactly
        /// when the suggestion came from outside the default, with the same
        /// KPI-RRA-1 counters an approval of it adds.
        #[test]
        fn a_decline_counts_toward_kpi_rra_1_exactly_when_it_came_from_outside_the_default(
            origin in prop_oneof![
                Just(ActedOrigin::Default),
                Just(ActedOrigin::Imported),
                proptest::sample::select(OutsideDefault::ALL.to_vec()).prop_map(ActedOrigin::Outside),
            ]
        ) {
            let events = decline_events(origin);
            prop_assert_eq!(events[0], KpiEvent::SuggestionDeclined);
            prop_assert_eq!(
                events.contains(&KpiEvent::SuggestionActedOutsideDefault),
                origin != ActedOrigin::Default
            );
            prop_assert_eq!(
                events.contains(&KpiEvent::SuggestionActedOutsideDefaultImported),
                origin == ActedOrigin::Imported
            );
            if let ActedOrigin::Outside(outside_default) = origin {
                let approval_counts = outside_default_events(Some(outside_default));
                prop_assert_eq!(&events[1..], approval_counts.as_slice());
            }
        }

        /// Universe: arbitrary strings. Only `YYYY-MM-DD`-shaped text is a day.
        #[test]
        fn only_iso_days_are_days(text in ".{0,12}", y in 1970u32..2100, m in 1u32..13, d in 1u32..29) {
            let day = format!("{y:04}-{m:02}-{d:02}");
            prop_assert_eq!(parse_day(&day), Some(day.as_str()));
            if parse_day(&text).is_some() {
                prop_assert_eq!(text.len(), 10);
            }
        }
    }
}
