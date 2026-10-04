//! Rate-budget arithmetic (ADR-076 §2). PURE: the caller passes the ages of
//! the recent events or the current time (the clock stays in the shell), and
//! the scan budget is state in, state out (the root holds the state).

use std::collections::{BTreeMap, BTreeSet};

/// Verify attempts allowed per DID per hour.
pub const VERIFY_ATTEMPTS_PER_HOUR: usize = 20;

/// One hour, in seconds.
pub const HOUR_SECS: u64 = 60 * 60;

/// An action is allowed iff fewer than `limit` actions happened inside the
/// window; actions older than the window never count.
pub fn budget_allows(recent_event_ages_secs: &[u64], limit: usize, window_secs: u64) -> bool {
    recent_event_ages_secs
        .iter()
        .filter(|age| **age < window_secs)
        .count()
        < limit
}

/// Scans allowed per DID per day.
pub const SCANS_PER_DAY: usize = 6;

/// One day, in seconds.
pub const DAY_SECS: u64 = 24 * HOUR_SECS;

/// Scans that may run at once, across everyone.
pub const CONCURRENT_SCANS: usize = 2;

/// Below this many remaining GitHub requests a scan pauses before its next repo.
pub const GITHUB_REMAINING_FLOOR: u64 = 300;

/// Who is scanning now, and when each DID started its recent scans.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanBudget {
    running: BTreeSet<String>,
    started: BTreeMap<String, Vec<u64>>,
}

/// Whether a scan may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanAdmission {
    Admitted,
    /// This DID already has a scan running (1 at a time per DID).
    AlreadyScanning,
    /// This DID started its daily allowance of scans in the last day.
    DailyLimitReached,
    /// The app is already running its maximum of concurrent scans.
    AppBusy,
}

/// Admit (or refuse) a scan by `owner_did` at `now` (Unix seconds).
pub fn admit_scan(budget: ScanBudget, owner_did: &str, now: u64) -> (ScanBudget, ScanAdmission) {
    let ScanBudget {
        mut running,
        mut started,
    } = budget;
    let recent: Vec<u64> = started
        .remove(owner_did)
        .unwrap_or_default()
        .into_iter()
        .filter(|at| now.saturating_sub(*at) < DAY_SECS)
        .collect();
    let ages: Vec<u64> = recent.iter().map(|at| now.saturating_sub(*at)).collect();
    let admission = if running.contains(owner_did) {
        ScanAdmission::AlreadyScanning
    } else if !budget_allows(&ages, SCANS_PER_DAY, DAY_SECS) {
        ScanAdmission::DailyLimitReached
    } else if running.len() >= CONCURRENT_SCANS {
        ScanAdmission::AppBusy
    } else {
        ScanAdmission::Admitted
    };
    let recent = match admission {
        ScanAdmission::Admitted => {
            running.insert(owner_did.to_string());
            [recent, vec![now]].concat()
        }
        _ => recent,
    };
    if !recent.is_empty() {
        started.insert(owner_did.to_string(), recent);
    }
    (ScanBudget { running, started }, admission)
}

/// The scan by `owner_did` ended (however it ended).
pub fn release_scan(budget: ScanBudget, owner_did: &str) -> ScanBudget {
    let ScanBudget {
        mut running,
        started,
    } = budget;
    running.remove(owner_did);
    ScanBudget { running, started }
}

/// Whether a scan may read its next repo, given GitHub's last reported
/// remaining budget (unknown = not yet observed, so go on).
pub fn github_budget_allows(remaining: Option<u64>) -> bool {
    remaining.is_none_or(|remaining| remaining >= GITHUB_REMAINING_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// One step of a day of scanning: someone starts, or someone finishes.
    #[derive(Debug, Clone)]
    enum Step {
        Start { did: usize, after_secs: u64 },
        Finish { did: usize },
    }

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![
            (0usize..4, 0u64..20_000).prop_map(|(did, after_secs)| Step::Start { did, after_secs }),
            (0usize..4).prop_map(|did| Step::Finish { did }),
        ]
    }

    const DIDS: [&str; 4] = [
        "did:plc:priya",
        "did:plc:dmitri",
        "did:plc:aisha",
        "did:plc:sam",
    ];

    proptest! {
        /// Universe: any interleaving of starts and finishes by four DIDs.
        /// At most 2 scans run at once, at most 1 per DID, and no DID is
        /// admitted more than 6 times within any one day.
        #[test]
        fn scans_never_exceed_the_concurrency_or_the_daily_allowance(
            steps in prop::collection::vec(step(), 0..60),
        ) {
            let mut budget = ScanBudget::default();
            let mut now = 1_000_000u64;
            let mut running: BTreeSet<usize> = BTreeSet::new();
            let mut admitted: Vec<(usize, u64)> = Vec::new();
            for step in steps {
                match step {
                    Step::Start { did, after_secs } => {
                        now += after_secs;
                        let (next, admission) = admit_scan(budget, DIDS[did], now);
                        budget = next;
                        if admission == ScanAdmission::Admitted {
                            prop_assert!(running.insert(did), "one scan per DID at a time");
                            admitted.push((did, now));
                        }
                    }
                    Step::Finish { did } => {
                        budget = release_scan(budget, DIDS[did]);
                        running.remove(&did);
                    }
                }
                prop_assert!(running.len() <= CONCURRENT_SCANS);
                for (did, at) in &admitted {
                    let in_day = admitted.iter()
                        .filter(|(d, t)| d == did && *t >= *at && *t - *at < DAY_SECS)
                        .count();
                    prop_assert!(in_day <= SCANS_PER_DAY);
                }
            }
        }

        /// Universe: n back-to-back finished scans by one DID within a day.
        /// Exactly the first 6 are admitted; the 7th onward is refused for
        /// the day, and a day later one is admitted again.
        #[test]
        fn the_seventh_scan_in_a_day_is_refused_until_a_day_has_passed(n in 0usize..12, gap in 1u64..600) {
            let mut budget = ScanBudget::default();
            let start = 5_000_000u64;
            let mut admissions = Vec::new();
            for i in 0..n {
                let (next, admission) = admit_scan(budget, DIDS[0], start + i as u64 * gap);
                budget = release_scan(next, DIDS[0]);
                admissions.push(admission);
            }
            let admitted = admissions.iter().filter(|a| **a == ScanAdmission::Admitted).count();
            prop_assert_eq!(admitted, n.min(SCANS_PER_DAY));
            prop_assert!(admissions.iter().skip(SCANS_PER_DAY).all(|a| *a == ScanAdmission::DailyLimitReached));
            let (_, tomorrow) = admit_scan(budget, DIDS[0], start + DAY_SECS + n as u64 * gap);
            prop_assert_eq!(tomorrow, ScanAdmission::Admitted);
        }

        /// Universe: every remaining count. A scan reads on iff GitHub has at
        /// least the floor left (or has not reported yet).
        #[test]
        fn a_scan_reads_on_only_at_or_above_the_github_floor(remaining in proptest::option::of(0u64..1_000)) {
            prop_assert_eq!(
                github_budget_allows(remaining),
                remaining.is_none_or(|r| r >= GITHUB_REMAINING_FLOOR)
            );
            prop_assert!(!github_budget_allows(Some(GITHUB_REMAINING_FLOOR - 1)));
            prop_assert!(github_budget_allows(Some(GITHUB_REMAINING_FLOOR)));
        }

        /// Universe: (event ages, limit, window). Allowed exactly while the
        /// events inside the window number fewer than the limit; adding an
        /// event outside the window never changes the answer.
        #[test]
        fn a_budget_allows_exactly_while_under_its_limit_within_the_window(
            ages in prop::collection::vec(0u64..200_000, 0..15),
            limit in 1usize..10, window in 1u64..100_000, stale in 0u64..100_000,
        ) {
            let inside = ages.iter().filter(|a| **a < window).count();
            prop_assert_eq!(budget_allows(&ages, limit, window), inside < limit);
            let with_stale = [ages.clone(), vec![window + stale]].concat();
            prop_assert_eq!(budget_allows(&with_stale, limit, window), budget_allows(&ages, limit, window));
        }
    }
}
