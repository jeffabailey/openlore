//! Rate-budget arithmetic (ADR-076 §2). PURE: the caller passes the ages of
//! the recent events (the clock stays in the shell).

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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
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
