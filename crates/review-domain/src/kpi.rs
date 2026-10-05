//! Aggregate KPI counting (kpi-instrumentation §2, OD-BRA-11). Pure.
//!
//! A counter is a [`KpiEvent`] — a closed catalogue whose names are
//! `&'static str` — so a counter cannot carry a DID, a handle or claim
//! content by construction. Reading counters back keeps only catalogue
//! names, so nothing else can reach the operator either.

use std::collections::BTreeMap;

use ports::claim_domain::UnsignedClaim;
use ports::{KpiCounterRow, Suggestion};

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
    SharePosted,
    RetractPosted,
    Disconnect,
}

impl KpiEvent {
    pub const ALL: [KpiEvent; 13] = [
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
