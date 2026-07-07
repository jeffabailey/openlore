//! `detect` — the PURE signal-detection core (RGSD-1 walking skeleton).
//!
//! Sibling of [`derive_candidates`](crate::derive_candidates): where
//! `derive_candidates` maps already-detected [`Signal`](ports::Signal)s onto
//! philosophy candidates, `detect_signals` DECIDES which of the bounded
//! [`SignalKind`](ports::SignalKind)s a REAL public repo exhibits from the
//! structured facts the effect shell reshaped out of the GitHub API response.
//! PURE: `RepoFacts` in, `Vec<Signal>` out; NO I/O.
//!
//! RGSD-1 (design §2/§5) implements ONLY the `MemorySafetyLanguage` arm — the
//! primary-language half of that signal — reading the repo's real `language`
//! field. Detectors 2–5 (dependency-pinning, semver+changelog, docs, tests)
//! land in later slices, each adding one pure predicate arm over an extended
//! `RepoFacts`.

use ports::{Signal, SignalKind};

/// The structured facts signal detection reads out of a real public repo.
///
/// The effect shell (`adapter-github::client::parse_repo_facts`) reshapes the
/// live `/repos/{owner}/{repo}` JSON into this pure value; `detect_signals`
/// then decides which signals fire — no I/O reaches this crate (WD-56).
///
/// RGSD-1 walking skeleton carries only what the `MemorySafetyLanguage` arm
/// needs: the primary `language` (absent when the API reports `null`) and the
/// `source_url` a language-derived signal names as its public evidence. Later
/// slices extend this with `has_cargo_lock`, `readme_bytes`, `tags`, … (design
/// §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    /// The repo's primary language as GitHub reports it (`Some("Rust")`), or
    /// `None` when the API returns `language: null` (e.g. an empty repo).
    pub language: Option<String>,
    /// The public GitHub URL of the repo — flows into a detected signal's
    /// `source_url` (and thus the candidate's evidence).
    pub source_url: String,
}

/// The curated set of memory-safety languages (design §2) — the
/// garbage-collected / ownership-safe languages that embody the memory-safety
/// philosophy. Stored LOWERCASE so the match against GitHub's `language`
/// string is case-insensitive. Deliberately EXCLUDES C, C++, and assembly
/// (unsafe-by-default) — the over-firing guard the RGSD-1 negative scenario
/// pins.
pub const MEMORY_SAFE_LANGUAGES: [&str; 14] = [
    "rust", "go", "swift", "kotlin", "java", "c#", "python", "ruby", "scala", "haskell", "elixir",
    "erlang", "ocaml", "clojure",
];

/// Whether a language string names a memory-safety language (case-insensitive
/// membership in [`MEMORY_SAFE_LANGUAGES`]). PURE.
fn is_memory_safe_language(language: &str) -> bool {
    let normalized = language.to_lowercase();
    MEMORY_SAFE_LANGUAGES.contains(&normalized.as_str())
}

/// Detect the bounded public signals a real repo exhibits from its
/// [`RepoFacts`]. PURE + total.
///
/// RGSD-1 implements ONLY the `MemorySafetyLanguage` arm: when the primary
/// `language` is present AND in [`MEMORY_SAFE_LANGUAGES`] (case-insensitive),
/// emit exactly one [`SignalKind::MemorySafetyLanguage`] signal whose `value`
/// is HONEST about what was measured — the primary language only, NOT "no
/// unsafe blocks" (design §3, deferred). Every out-of-set or absent language
/// yields no signal (the over-firing guard). Later detectors extend the
/// returned vector with their own arms.
pub fn detect_signals(facts: &RepoFacts) -> Vec<Signal> {
    detect_memory_safety_language(facts).into_iter().collect()
}

/// The `MemorySafetyLanguage` detector arm (RGSD-1): fires when the primary
/// language is present AND memory-safe. Returns `Some(signal)` or `None` — the
/// caller flattens each arm's option into the returned signal vector.
fn detect_memory_safety_language(facts: &RepoFacts) -> Option<Signal> {
    let language = facts.language.as_deref()?;
    if !is_memory_safe_language(language) {
        return None;
    }
    Some(Signal {
        kind: SignalKind::MemorySafetyLanguage,
        // Honest semantics (design §3): name ONLY the primary language that was
        // measured — the deferred "no unsafe blocks" refinement is NOT claimed.
        value: format!("primary language: {language}"),
        source_url: facts.source_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A GitHub-shaped public repo URL strategy (mirrors the derive strategies'
    /// URL shape) so a detected signal's `source_url` is realistic.
    fn arb_source_url() -> impl Strategy<Value = String> {
        "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}"
    }

    /// A memory-safety language in an ARBITRARY case (upper/lower/mixed) — the
    /// detection must match case-insensitively (design §2).
    fn arb_memory_safe_language() -> impl Strategy<Value = String> {
        let base = prop::sample::select(MEMORY_SAFE_LANGUAGES.to_vec());
        (base, any::<u64>()).prop_map(|(lang, seed)| recase(lang, seed))
    }

    /// Re-case a lowercase string deterministically from a seed so the strategy
    /// explores mixed-case spellings (`Rust`, `RUST`, `rUsT`, …).
    fn recase(lower: &str, seed: u64) -> String {
        lower
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if (seed >> (i % 64)) & 1 == 1 {
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
            .collect()
    }

    proptest! {
        /// Property (design §7): EVERY memory-safety language, in ANY case,
        /// yields EXACTLY ONE `MemorySafetyLanguage` signal whose `value` names
        /// the language verbatim and whose `source_url` is the repo's URL — and
        /// nothing else.
        #[test]
        fn every_memory_safe_language_any_case_yields_exactly_one_signal(
            language in arb_memory_safe_language(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language.clone()),
                source_url: source_url.clone(),
            };
            let signals = detect_signals(&facts);
            prop_assert_eq!(signals.len(), 1, "a memory-safety language fires exactly one signal");
            prop_assert_eq!(signals[0].kind, SignalKind::MemorySafetyLanguage);
            prop_assert_eq!(&signals[0].source_url, &source_url);
            prop_assert!(
                signals[0].value.contains(&language),
                "the signal value must name the primary language honestly (design §3); got {:?}",
                signals[0].value
            );
            // Honest semantics (design §3): the value must NOT claim the
            // deferred "no unsafe" refinement was verified.
            prop_assert!(
                !signals[0].value.to_lowercase().contains("unsafe"),
                "RGSD-1 must not claim `no unsafe` was verified (design §3); got {:?}",
                signals[0].value
            );
        }

        /// Property: an out-of-set language (NOT memory-safe) — including C/C++,
        /// which are deliberately excluded — yields NO signal (the over-firing
        /// guard the RGSD-1 negative scenario pins).
        #[test]
        fn a_non_memory_safe_language_yields_no_signal(
            language in prop::sample::select(vec!["C", "C++", "Assembly", "COBOL", "Fortran"]),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language.to_string()),
                source_url,
            };
            prop_assert!(
                detect_signals(&facts).is_empty(),
                "a non-memory-safe language ({language}) must fire no signal"
            );
        }

        /// Property: an ABSENT language (`None` — the API reported `null`)
        /// yields NO signal.
        #[test]
        fn an_absent_language_yields_no_signal(source_url in arb_source_url()) {
            let facts = RepoFacts { language: None, source_url };
            prop_assert!(
                detect_signals(&facts).is_empty(),
                "an absent language must fire no signal"
            );
        }
    }
}
