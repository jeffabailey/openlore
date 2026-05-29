//! `suggest` — the PURE near-match suggestion for an empty dimension result.
//!
//! [`near_match_suggestion`] computes a near-match (edit distance over the known
//! dimension values) so an empty `--object`/`--contributor`/`--subject` result
//! can offer "did you mean …?" (US-AV-002 Ex 4) instead of a bare zero-result.
//! Pure; deterministic; no I/O.
//!
//! The body is intentionally `todo!()` at the 01-01 bootstrap; the suggestion
//! behavior is driven by the Phase 02+ empty-result scenario. Split into its own
//! module for mutation-test clarity (D-D40).
//
// SCAFFOLD: true

/// The maximum edit distance at which a known object is "close enough" to be
/// offered as a "did you mean …?" suggestion. A single substitution, insertion,
/// deletion, or transposition (the common typo shapes — e.g.
/// `reproducable`→`reproducible`) is at most distance 2, so it is suggested;
/// a genuinely unrelated query is many edits away and yields no suggestion.
/// Kept deliberately tight so an empty result NEVER offers a spurious match.
const SUGGESTION_MAX_DISTANCE: usize = 2;

/// Near-match suggestion for an empty dimension result (edit distance over
/// `known` values). Returns `Some(suggestion)` when a close-enough known value
/// exists, else `None`. Deterministic; no I/O.
///
/// The pipeline is three small, named steps: rank every candidate by its
/// Levenshtein distance to `query`, pick the closest (ties broken by the
/// lexicographically smallest candidate so the result is independent of input
/// order), then keep it only when it is within [`SUGGESTION_MAX_DISTANCE`].
pub fn near_match_suggestion(query: &str, known: &[String]) -> Option<String> {
    closest_candidate(query, known)
        .filter(|(distance, _)| *distance <= SUGGESTION_MAX_DISTANCE)
        .map(|(_, candidate)| candidate)
}

/// Find the `(distance, candidate)` minimising Levenshtein distance to `query`.
/// On a distance tie, the lexicographically smallest candidate wins — a
/// deterministic tiebreak independent of the order candidates appear in `known`.
/// Returns `None` only for an empty candidate set.
fn closest_candidate(query: &str, known: &[String]) -> Option<(usize, String)> {
    known
        .iter()
        .map(|candidate| (levenshtein(query, candidate), candidate.clone()))
        .min_by(|left, right| {
            // Lower distance first; then lower (lexicographically smaller)
            // candidate — the documented deterministic tiebreak.
            left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))
        })
}

/// PURE Levenshtein edit distance (insertions, deletions, substitutions) over
/// Unicode scalar values. Classic two-row dynamic-programming computation —
/// O(len(a) * len(b)) time, O(len(b)) space; no allocation per char beyond the
/// two rolling rows. Deterministic; no I/O.
fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    // `previous[j]` = edit distance between the empty prefix of `a` and the
    // first `j` chars of `b` — i.e. `j` insertions.
    let mut previous: Vec<usize> = (0..=b_chars.len()).collect();
    let mut current: Vec<usize> = vec![0; b_chars.len() + 1];

    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1; // distance from `i+1`-char prefix of `a` to empty `b`.
        for (j, &b_char) in b_chars.iter().enumerate() {
            let substitution_cost = usize::from(a_char != b_char);
            current[j + 1] = (previous[j + 1] + 1) // deletion
                .min(current[j] + 1) // insertion
                .min(previous[j] + substitution_cost); // substitution / match
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A small known-object set (a slice of the canonical philosophy URIs) used
    /// across the example cases.
    fn known() -> Vec<String> {
        vec![
            "org.openlore.philosophy.reproducible-builds".to_string(),
            "org.openlore.philosophy.dependency-pinning".to_string(),
            "org.openlore.philosophy.minimalism".to_string(),
            "org.openlore.philosophy.fail-fast".to_string(),
        ]
    }

    /// An exact-match query suggests itself (distance 0 — trivially within
    /// threshold; the closest known object IS the query).
    #[test]
    fn exact_match_suggests_itself() {
        let candidates = known();
        assert_eq!(
            near_match_suggestion("org.openlore.philosophy.minimalism", &candidates),
            Some("org.openlore.philosophy.minimalism".to_string()),
        );
    }

    /// A single-substitution typo (`reproducable` for `reproducible`) suggests
    /// the closest known object by edit distance (the US-AV-002 Ex4 case).
    #[test]
    fn single_typo_suggests_closest_known_object() {
        let candidates = known();
        assert_eq!(
            near_match_suggestion("org.openlore.philosophy.reproducable-builds", &candidates),
            Some("org.openlore.philosophy.reproducible-builds".to_string()),
        );
    }

    /// A query whose nearest known object is beyond the threshold returns None —
    /// no spurious suggestion (the bare empty-result path, exit 0).
    #[test]
    fn below_threshold_returns_none() {
        let candidates = known();
        assert_eq!(
            near_match_suggestion("com.example.totally.unrelated.gibberish-xyzzy", &candidates),
            None,
        );
    }

    /// An empty known set can never offer a suggestion.
    #[test]
    fn empty_known_set_returns_none() {
        assert_eq!(near_match_suggestion("anything", &[]), None);
    }

    /// On a distance TIE, the suggestion is the lexicographically smallest
    /// candidate — deterministic, independent of input order. Here both `"axc"`
    /// and `"ayc"` are at edit distance 1 from `"abc"`; the lower one (`"axc"`)
    /// wins regardless of the order they appear in `known`.
    #[test]
    fn distance_tie_breaks_lexicographically_smallest() {
        let high_first = vec!["ayc".to_string(), "axc".to_string()];
        let low_first = vec!["axc".to_string(), "ayc".to_string()];
        assert_eq!(
            near_match_suggestion("abc", &high_first),
            Some("axc".to_string()),
            "the lexicographically smallest of the tied candidates wins (high-first order)"
        );
        assert_eq!(
            near_match_suggestion("abc", &low_first),
            near_match_suggestion("abc", &high_first),
            "the tiebreak is independent of input order (deterministic)"
        );
    }

    /// Pin KNOWN multi-edit Levenshtein distances so the DP arithmetic (`+ 1`
    /// deletion / `+ 1` insertion / `+ substitution_cost`, plus the `i + 1`
    /// first-column seed) is asserted EXACTLY. A mutant that replaces any `+`
    /// with `*` diverges on these cases because at least one operand exceeds 1
    /// (e.g. an interior cell whose neighbour distance is ≥ 2), so `n * 1 != n + 1`
    /// and `n * cost != n + cost`. The classic `kitten`→`sitting` (3) plus two
    /// further multi-edit pairs cover the deletion, insertion, and substitution
    /// recurrence sites; the `i + 1` seed is exercised by the pure-deletion pair
    /// (`"flaw"`→`""` is 4, forcing `current[0] = i + 1` up to 4).
    #[test]
    fn levenshtein_pins_known_multi_edit_distances() {
        // Classic 3-edit textbook case (substitution k→s, substitution e→i,
        // insertion g): the interior DP cells hold values ≥ 2, so a `+`→`*`
        // mutant computes a different number.
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        // A 3-edit pair driving the deletion + substitution arms with operands ≥ 2.
        assert_eq!(levenshtein("sunday", "saturday"), 3);
        // A pure-deletion pair: distance equals the length, forcing the
        // `current[0] = i + 1` seed to grow to 4 (an `i * 1` mutant would stay 0).
        assert_eq!(levenshtein("flaw", ""), 4);
        // A 2-edit pair (two substitutions) — interior cell reaches 2, so the
        // substitution `+ substitution_cost` site diverges under `*`.
        assert_eq!(levenshtein("abcde", "axcye"), 2);
        // Pure-DELETION pair on the optimal path: `a` longer than `b`, so the
        // `previous[j + 1] + 1` deletion term is the selected minimum at cells
        // whose neighbour distance is ≥ 1. A `+`→`*` mutant on the deletion site
        // makes deletion free (`x * 1 == x`), collapsing the distance.
        assert_eq!(levenshtein("abc", "a"), 2);
        // Pure-INSERTION pair (the mirror): `b` longer than `a`, so the
        // `current[j] + 1` insertion term is on the optimal path. A `+`→`*` mutant
        // on the insertion site makes insertion free, collapsing the distance.
        assert_eq!(levenshtein("a", "abc"), 2);
    }

    proptest! {
        /// Inner-loop ranker property (Hebert ch.3 Tier-1 "Modeling"): whenever
        /// `near_match_suggestion` returns `Some(s)`, `s` is a member of `known`
        /// AND its edit distance to the query is the MINIMUM over all candidates
        /// (a reference min-distance computed directly over the input). A mutant
        /// that returned a non-minimal candidate, or one not in `known`, fails
        /// LOUDLY here. The query and candidates are drawn from a small alphabet
        /// so ties + near-misses are exercised densely.
        #[test]
        fn suggestion_is_a_minimum_edit_distance_member_of_known(
            query in "[a-c]{0,6}",
            known in prop::collection::vec("[a-c]{0,6}", 0..6),
        ) {
            if let Some(suggested) = near_match_suggestion(&query, &known) {
                // The suggestion must be one of the known candidates.
                prop_assert!(
                    known.contains(&suggested),
                    "the suggested string must be a member of `known`"
                );
                // Its distance must be the minimum over all candidates.
                let suggested_distance = levenshtein(&query, &suggested);
                let reference_min = known
                    .iter()
                    .map(|candidate| levenshtein(&query, candidate))
                    .min()
                    .expect("Some(_) implies known is non-empty");
                prop_assert_eq!(
                    suggested_distance,
                    reference_min,
                    "the suggested candidate must be at the MINIMUM edit distance over `known`"
                );
            }
        }
    }
}
