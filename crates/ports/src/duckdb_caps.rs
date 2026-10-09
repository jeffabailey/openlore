//! `duckdb_caps` — the DuckDB resource caps a long-running app accepts from
//! its environment (indexer B9, review app B11; data-models §1). Pure, so both
//! composition roots share ONE admissible range per cap and ONE parse; each
//! app keeps its own variable names, defaults and refusal type.

use std::fmt;
use std::ops::RangeInclusive;

/// A DuckDB setting an operator may cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuckDbCap {
    /// `memory_limit`, in MiB.
    MemoryLimitMb,
    /// `threads`.
    Threads,
}

impl DuckDbCap {
    /// The admissible values (production: 48 MiB, one thread).
    pub const fn range(self) -> RangeInclusive<u64> {
        match self {
            Self::MemoryLimitMb => 16..=1024,
            Self::Threads => 1..=4,
        }
    }

    /// `text` as a whole number within [`range`](Self::range), else refused
    /// (a blank or signed-negative text is not a number; no trimming).
    pub fn parse(self, text: &str) -> Result<u64, CapRefused> {
        let range = self.range();
        text.parse::<u64>()
            .ok()
            .filter(|number| range.contains(number))
            .ok_or(CapRefused { range })
    }
}

/// A cap value refused: not a whole number within `range`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapRefused {
    pub range: RangeInclusive<u64>,
}

impl fmt::Display for CapRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "must be a whole number from {} to {}",
            self.range.start(),
            self.range.end()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn boundary_and_malformed_texts_have_fixed_answers() {
        let memory_refused = Err(CapRefused { range: 16..=1024 });
        let threads_refused = Err(CapRefused { range: 1..=4 });
        let table = [
            (DuckDbCap::MemoryLimitMb, "15", memory_refused.clone()),
            (DuckDbCap::MemoryLimitMb, "16", Ok(16)),
            (DuckDbCap::MemoryLimitMb, "48", Ok(48)),
            (DuckDbCap::MemoryLimitMb, "1024", Ok(1024)),
            (DuckDbCap::MemoryLimitMb, "1025", memory_refused.clone()),
            (DuckDbCap::MemoryLimitMb, "", memory_refused.clone()),
            (DuckDbCap::MemoryLimitMb, " 48", memory_refused.clone()),
            (DuckDbCap::MemoryLimitMb, "48.0", memory_refused.clone()),
            (DuckDbCap::MemoryLimitMb, "48MiB", memory_refused),
            (DuckDbCap::Threads, "0", threads_refused.clone()),
            (DuckDbCap::Threads, "1", Ok(1)),
            (DuckDbCap::Threads, "4", Ok(4)),
            (DuckDbCap::Threads, "5", threads_refused.clone()),
            (DuckDbCap::Threads, "-1", threads_refused.clone()),
            (DuckDbCap::Threads, "18446744073709551616", threads_refused),
        ];
        for (cap, text, expected) in table {
            assert_eq!(cap.parse(text), expected, "{cap:?} {text:?}");
        }
    }

    #[test]
    fn a_refusal_names_the_admissible_range() {
        assert_eq!(
            DuckDbCap::MemoryLimitMb.parse("0").unwrap_err().to_string(),
            "must be a whole number from 16 to 1024"
        );
        assert_eq!(
            DuckDbCap::Threads.parse("x").unwrap_err().to_string(),
            "must be a whole number from 1 to 4"
        );
    }

    proptest! {
        /// Universe: every cap x every u64 written in decimal. Accepted (as
        /// itself) exactly when within the cap's literal bounds.
        #[test]
        fn a_decimal_number_is_accepted_exactly_within_the_caps_bounds(
            cap_and_bounds in prop_oneof![
                Just((DuckDbCap::MemoryLimitMb, 16u64, 1024u64)),
                Just((DuckDbCap::Threads, 1u64, 4u64)),
            ],
            number in prop_oneof![0u64..1100, any::<u64>()],
        ) {
            let (cap, low, high) = cap_and_bounds;
            let parsed = cap.parse(&number.to_string());
            if low <= number && number <= high {
                prop_assert_eq!(parsed, Ok(number));
            } else {
                prop_assert_eq!(parsed, Err(CapRefused { range: low..=high }));
            }
        }

        /// Universe: every cap x texts containing a character that is neither
        /// a digit nor `+`. Always refused.
        #[test]
        fn text_with_a_non_digit_is_always_refused(
            cap in prop_oneof![Just(DuckDbCap::MemoryLimitMb), Just(DuckDbCap::Threads)],
            prefix in "[0-9]{0,3}",
            junk in "[^0-9+]",
            suffix in "[0-9]{0,3}",
        ) {
            let text = format!("{prefix}{junk}{suffix}");
            prop_assert!(cap.parse(&text).is_err(), "{:?} accepted {:?}", cap, text);
        }
    }
}
