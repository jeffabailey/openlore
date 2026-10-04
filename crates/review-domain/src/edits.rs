//! Editing a suggestion before approving it (US-BRA-005): the owner sets
//! her own confidence (BR-4: 0.00–1.00 at two decimals, stored as basis
//! points) and may swap the philosophy across the shared vocabulary (BR-5,
//! the philosophy registry of ADR-059). Pure: an edit is a value; nothing is
//! written until the edited plan is confirmed.

use std::fmt;

use ports::lexicon::philosophy::{object_id, seeds};

/// The field guidance shown for any confidence that is not 0.00–1.00 at
/// two decimals.
pub const CONFIDENCE_GUIDANCE: &str = "Enter a number from 0.00 to 1.00";

/// The highest confidence, in basis points (1.00).
const MAX_BASIS_POINTS: u32 = 10_000;

/// A typed confidence that is not 0.00–1.00 with at most two decimals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidConfidence;

impl fmt::Display for InvalidConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(CONFIDENCE_GUIDANCE)
    }
}

/// The owner's edit of a suggestion: the philosophy she claims and her
/// confidence in basis points (0..=10000).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimEdit {
    pub object: String,
    pub confidence_bp: u16,
}

/// Parse a typed confidence (`0.7`, `0.70`, `.5`, `1`, `1.00`) into basis
/// points: exactly `round(value × 10000)` for every hundredth in range;
/// anything else (a third decimal, a sign, a comma, text, empty, > 1.00)
/// is refused.
pub fn parse_confidence(typed: &str) -> Result<u16, InvalidConfidence> {
    let typed = typed.trim();
    let (whole, hundredths) = match typed.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (typed, None),
    };
    let whole_units = whole_part(whole, hundredths.is_some())?;
    let fraction_bp = hundredths.map_or(Ok(0), fraction_part)?;
    whole_units
        .checked_mul(MAX_BASIS_POINTS)
        .and_then(|bp| bp.checked_add(fraction_bp))
        .filter(|bp| *bp <= MAX_BASIS_POINTS)
        .and_then(|bp| u16::try_from(bp).ok())
        .ok_or(InvalidConfidence)
}

/// The digits before the point; empty only when a fraction follows (`.5`).
fn whole_part(digits: &str, has_fraction: bool) -> Result<u32, InvalidConfidence> {
    match digits {
        "" if has_fraction => Ok(0),
        _ if all_ascii_digits(digits) => digits.parse().map_err(|_| InvalidConfidence),
        _ => Err(InvalidConfidence),
    }
}

/// One or two digits after the point, as basis points (`7` → 7000).
fn fraction_part(digits: &str) -> Result<u32, InvalidConfidence> {
    match digits.len() {
        1 | 2 if all_ascii_digits(digits) => format!("{digits:0<2}")
            .parse::<u32>()
            .map(|hundredths| hundredths * 100)
            .map_err(|_| InvalidConfidence),
        _ => Err(InvalidConfidence),
    }
}

fn all_ascii_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// The philosophy objects of the shared vocabulary (the embedded seed
/// registry, `openlore philosophy list`).
pub fn vocabulary() -> Vec<String> {
    seeds().iter().map(|seed| object_id(&seed.name)).collect()
}

/// What the philosophy swap offers for a suggestion of `current`: the whole
/// vocabulary, with `current` kept first when it is outside it (an unknown
/// object is never rejected, BR-5).
pub fn philosophy_choices(current: &str) -> Vec<String> {
    let known = vocabulary();
    let unknown = (!known.iter().any(|object| object == current)).then(|| current.to_string());
    unknown.into_iter().chain(known).collect()
}

/// The owner's edit from the typed form: the chosen philosophy (blank keeps
/// `suggested_object`) and the parsed confidence.
pub fn edit_claim(
    suggested_object: &str,
    chosen_object: Option<&str>,
    typed_confidence: &str,
) -> Result<ClaimEdit, InvalidConfidence> {
    let object = chosen_object
        .map(str::trim)
        .filter(|chosen| !chosen.is_empty())
        .unwrap_or(suggested_object);
    Ok(ClaimEdit {
        object: object.to_string(),
        confidence_bp: parse_confidence(typed_confidence)?,
    })
}

#[cfg(test)]
mod tests {
    //! Universe: typed confidences (every hundredth in range, its short
    //! forms, and junk) × philosophy choices. Parsing is total: each input
    //! maps to exactly round(v × 10000) or the guidance, nothing else.
    use super::*;
    use crate::views::confidence_text;
    use proptest::prelude::*;

    proptest! {
        /// Every hundredth from 0.00 to 1.00, as displayed, parses back to
        /// its basis points; the short forms agree.
        #[test]
        fn every_displayed_hundredth_parses_back_to_its_basis_points(hundredths in 0u16..=100) {
            let bp = hundredths * 100;
            prop_assert_eq!(parse_confidence(&confidence_text(bp)), Ok(bp));
            let short = format!("{}", f64::from(hundredths) / 100.0);
            prop_assert_eq!(parse_confidence(&short), Ok(bp));
            prop_assert_eq!(parse_confidence(&format!(" {} ", confidence_text(bp))), Ok(bp));
        }

        /// A third decimal is refused even when it is zero (0.700), and so
        /// is anything above 1.00 by a single hundredth or more.
        #[test]
        fn a_third_decimal_or_anything_above_one_is_refused(
            hundredths in 0u16..=100, extra in 0u8..=9, above in 1u32..1_000_000
        ) {
            let three = format!("{}{extra}", confidence_text(hundredths * 100));
            prop_assert_eq!(parse_confidence(&three), Err(InvalidConfidence));
            let over = 100 + above;
            let typed = format!("{}.{:02}", over / 100, over % 100);
            prop_assert_eq!(parse_confidence(&typed), Err(InvalidConfidence));
        }

        /// Signs, commas, exponents, blanks and text never parse.
        #[test]
        fn non_numeric_input_is_refused(
            junk in prop_oneof![
                "-[0-9]?\\.?[0-9]{0,2}",
                "\\+[0-9]\\.[0-9]{1,2}",
                "[0-9],[0-9]{1,2}",
                "[0-9]e-?[0-9]",
                "[a-zA-Z ]{0,6}",
                "[0-9]?\\.",
            ]
        ) {
            prop_assert_eq!(parse_confidence(&junk), Err(InvalidConfidence));
        }

        /// The edit carries exactly the chosen philosophy (blank keeps the
        /// suggestion's) and exactly the parsed confidence.
        #[test]
        fn an_edit_carries_exactly_the_chosen_philosophy_and_confidence(
            suggested in "org\\.openlore\\.philosophy\\.[a-z-]{3,20}",
            chosen in proptest::option::of("org\\.openlore\\.philosophy\\.[a-z-]{3,20}|[ ]{0,2}"),
            hundredths in 0u16..=100,
        ) {
            let edit = edit_claim(&suggested, chosen.as_deref(), &confidence_text(hundredths * 100));
            let expected_object = chosen.as_deref().map(str::trim).filter(|c| !c.is_empty()).unwrap_or(&suggested);
            prop_assert_eq!(edit, Ok(ClaimEdit { object: expected_object.to_string(), confidence_bp: hundredths * 100 }));
            prop_assert_eq!(edit_claim(&suggested, chosen.as_deref(), "1.5"), Err(InvalidConfidence));
        }

        /// The swap offers every vocabulary philosophy exactly once, plus
        /// an unknown current one (never rejected).
        #[test]
        fn the_swap_offers_the_whole_vocabulary_and_the_current_philosophy(
            current in "org\\.openlore\\.philosophy\\.[a-z-]{3,20}"
        ) {
            let choices = philosophy_choices(&current);
            let known = vocabulary();
            prop_assert!(known.iter().all(|object| choices.contains(object)));
            prop_assert!(choices.contains(&current));
            let mut unique = choices.clone();
            unique.sort();
            unique.dedup();
            prop_assert_eq!(unique.len(), choices.len());
            prop_assert_eq!(choices.len(), known.len() + usize::from(!known.contains(&current)));
        }
    }

    #[test]
    fn the_guidance_is_the_refusal_text() {
        // bypass: a single fixed string; there is no input space to explore.
        assert_eq!(InvalidConfidence.to_string(), CONFIDENCE_GUIDANCE);
    }
}
