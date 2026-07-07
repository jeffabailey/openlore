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
    /// The committed `Cargo.lock`'s public file URL (RGSD-2). `Some(url)` when
    /// the effect shell's `content_exists(owner, repo, "Cargo.lock")` probe
    /// returned 200 (a committed Cargo.lock — the dependency manifest is
    /// pinned); `None` when it returned 404 (absent). Flows into the
    /// [`SignalKind::DependencyManifestPinned`] signal's `source_url` so the
    /// derived candidate names the Cargo.lock as its evidence (design §2/§3).
    pub cargo_lock_url: Option<String>,
}

/// The curated set of memory-safety languages (design §2) — languages with
/// memory-safety guarantees (ownership-based like Rust, or runtime/GC-managed
/// like Go, Python, Ruby, Java) that embody the memory-safety philosophy.
/// Stored LOWERCASE so the match against GitHub's `language`
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
/// Each detector arm is INDEPENDENT: a repo can fire both the
/// `MemorySafetyLanguage` arm (RGSD-1) and the `DependencyManifestPinned` arm
/// (RGSD-2) — the returned vector is the union of every arm that fired.
///
/// - `MemorySafetyLanguage` (RGSD-1): when the primary `language` is present
///   AND in [`MEMORY_SAFE_LANGUAGES`] (case-insensitive), emit exactly one
///   [`SignalKind::MemorySafetyLanguage`] signal whose `value` is HONEST about
///   what was measured — the primary language only, NOT "no unsafe blocks"
///   (design §3, deferred). Every out-of-set or absent language yields none.
/// - `DependencyManifestPinned` (RGSD-2): when `cargo_lock_url` is `Some(url)`
///   (a committed `Cargo.lock` — the manifest is pinned), emit exactly one
///   [`SignalKind::DependencyManifestPinned`] signal sourced at that URL. A
///   `None` (no committed Cargo.lock) yields none — detection is
///   Cargo.lock-gated, never unconditional (design §2, the over-firing guard).
///
/// Later detectors (semver+changelog, docs, tests) extend the returned vector
/// with their own arms.
pub fn detect_signals(facts: &RepoFacts) -> Vec<Signal> {
    [
        detect_memory_safety_language(facts),
        detect_dependency_manifest_pinned(facts),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The `DependencyManifestPinned` detector arm (RGSD-2): fires when the repo
/// commits a `Cargo.lock` (`cargo_lock_url` is `Some`), meaning its dependency
/// manifest pins exact versions. Returns `Some(signal)` sourced at the
/// committed Cargo.lock's public URL, or `None` when no Cargo.lock is present.
/// PURE — a total predicate over the facts; the effect-shell probe that fills
/// `cargo_lock_url` lives in `adapter-github`.
fn detect_dependency_manifest_pinned(facts: &RepoFacts) -> Option<Signal> {
    let cargo_lock_url = facts.cargo_lock_url.as_deref()?;
    Some(Signal {
        kind: SignalKind::DependencyManifestPinned,
        // Honest semantics (design §3): a committed Cargo.lock pins the
        // dependency manifest to exact versions — that is exactly what the
        // probe measured.
        value: "Cargo.lock committed (pinned dependencies)".to_string(),
        source_url: cargo_lock_url.to_string(),
    })
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
                cargo_lock_url: None,
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
                cargo_lock_url: None,
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
            let facts = RepoFacts { language: None, source_url, cargo_lock_url: None };
            prop_assert!(
                detect_signals(&facts).is_empty(),
                "an absent language must fire no signal"
            );
        }

        /// Property (RGSD-2, design §2/§5): a repo whose `cargo_lock_url` is
        /// `Some(url)` (a committed Cargo.lock) fires EXACTLY ONE
        /// `DependencyManifestPinned` signal sourced at that URL — regardless
        /// of the language (the arms are independent). The `language` is drawn
        /// from the NON-memory-safe set so the memory-safety arm stays quiet,
        /// isolating the dependency-pinning arm as the only thing that fires.
        #[test]
        fn a_committed_cargo_lock_fires_exactly_one_dependency_pinning_signal(
            language in prop::sample::select(vec!["C", "C++", "Assembly", "COBOL"]),
            cargo_lock_url in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language.to_string()),
                source_url,
                cargo_lock_url: Some(cargo_lock_url.clone()),
            };
            let signals = detect_signals(&facts);
            prop_assert_eq!(
                signals.len(), 1,
                "a committed Cargo.lock fires exactly one signal (language {} is non-safe)",
                language
            );
            prop_assert_eq!(signals[0].kind, SignalKind::DependencyManifestPinned);
            prop_assert_eq!(
                &signals[0].source_url, &cargo_lock_url,
                "the dependency-pinning signal must be sourced at the committed Cargo.lock URL"
            );
        }

        /// Property (RGSD-2, over-firing guard, design §2): a repo whose
        /// `cargo_lock_url` is `None` (no committed Cargo.lock) fires NO
        /// `DependencyManifestPinned` signal — detection is Cargo.lock-gated,
        /// never unconditional. Paired with a non-memory-safe language so the
        /// FULL signal set is empty (isolating the gate).
        #[test]
        fn an_absent_cargo_lock_fires_no_dependency_pinning_signal(
            language in prop::sample::select(vec!["C", "C++", "Assembly", "COBOL"]),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language.to_string()),
                source_url,
                cargo_lock_url: None,
            };
            prop_assert!(
                detect_signals(&facts)
                    .iter()
                    .all(|s| s.kind != SignalKind::DependencyManifestPinned),
                "no committed Cargo.lock must fire no dependency-pinning signal (over-firing guard)"
            );
        }

        /// Property (RGSD-2 + RGSD-1 independence, design §2): a repo that is
        /// BOTH memory-safe AND commits a Cargo.lock fires BOTH signals — the
        /// two detector arms are independent, neither suppresses the other.
        #[test]
        fn a_memory_safe_repo_with_a_cargo_lock_fires_both_signals(
            language in arb_memory_safe_language(),
            cargo_lock_url in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language),
                source_url,
                cargo_lock_url: Some(cargo_lock_url),
            };
            let kinds: Vec<SignalKind> = detect_signals(&facts).iter().map(|s| s.kind).collect();
            prop_assert!(
                kinds.contains(&SignalKind::MemorySafetyLanguage),
                "the memory-safety arm must still fire when a Cargo.lock is also present"
            );
            prop_assert!(
                kinds.contains(&SignalKind::DependencyManifestPinned),
                "the dependency-pinning arm must fire alongside the memory-safety arm (independent arms)"
            );
        }
    }
}
