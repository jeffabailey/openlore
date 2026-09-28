//! The DDD-9 inferred confidence, in whole hundredths (integer arithmetic,
//! so no float noise ever reaches a signed payload).

/// Ceiling of an inferred confidence, in hundredths: inference never leaves
/// the speculative bucket on its own (OD-CPI-3 / DDD-9).
const CONFIDENCE_CAP: u32 = 29;

/// Base confidence for a single supporting repo, in hundredths.
const CONFIDENCE_BASE: u32 = 15;

/// Confidence gained per additional supporting repo, in hundredths.
const CONFIDENCE_STEP: u32 = 5;

/// A confidence in whole hundredths (DDD-9) — integer arithmetic, so no float
/// noise ever reaches a signed payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hundredths(u32);

impl Hundredths {
    /// A confidence of `value` hundredths, clamped to `[0, 100]`.
    pub fn new(value: u32) -> Self {
        Self(value.min(100))
    }

    /// `floor(100 × confidence)` of a `[0.0, 1.0]` claim confidence. A tiny
    /// epsilon absorbs binary representation error (`0.29 × 100 = 28.999…`).
    pub fn floor_of(confidence: f64) -> Self {
        let scaled = (confidence.clamp(0.0, 1.0) * 100.0 + 1e-6).floor();
        Self::new(scaled as u32)
    }

    /// The whole-hundredths value.
    pub fn value(self) -> u32 {
        self.0
    }

    /// The `[0.0, 1.0]` decimal this confidence denotes.
    pub fn as_decimal(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

impl std::fmt::Display for Hundredths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

/// DDD-9: `min(29, 15 + 5·(k − 1), floor(100·max))` for `k` supporting repos.
pub fn inferred_confidence(supporting_repos: usize, max_supporting: Hundredths) -> Hundredths {
    let extra_repos = u32::try_from(supporting_repos.saturating_sub(1)).unwrap_or(u32::MAX);
    let by_breadth = CONFIDENCE_BASE.saturating_add(CONFIDENCE_STEP.saturating_mul(extra_repos));
    Hundredths::new(CONFIDENCE_CAP.min(by_breadth).min(max_supporting.value()))
}

/// The arithmetic behind [`inferred_confidence`], reproducible by hand
/// (J-002c), e.g. `min(0.29, 0.15 + 0.05 × (2 − 1), 0.60) = 0.20`.
pub fn confidence_arithmetic(supporting_repos: usize, max_supporting: Hundredths) -> String {
    format!(
        "min({}, {} + {} × ({supporting_repos} − 1), {max_supporting}) = {}",
        Hundredths::new(CONFIDENCE_CAP),
        Hundredths::new(CONFIDENCE_BASE),
        Hundredths::new(CONFIDENCE_STEP),
        inferred_confidence(supporting_repos, max_supporting)
    )
}
