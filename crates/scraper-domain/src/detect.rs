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
    /// A semver-shaped tag name the repo publishes (RGSD-3). `Some(tag)` when
    /// the effect shell's `list_tags(owner, repo)` probe found at least one tag
    /// matching [`is_semver_tag`] (via [`pick_semver_tag`]) — the repo follows
    /// semantic versioning in its release tags; `None` when it has no
    /// semver-shaped tag. One HALF of the [`SignalKind::SemverAndChangelog`]
    /// CONJUNCTION (design §2).
    pub semver_tag: Option<String>,
    /// The committed `CHANGELOG.md`'s public file URL (RGSD-3). `Some(url)` when
    /// the effect shell's `content_exists(owner, repo, "CHANGELOG.md")` probe
    /// returned 200 (a committed CHANGELOG); `None` when it returned 404
    /// (absent). The OTHER half of the [`SignalKind::SemverAndChangelog`]
    /// CONJUNCTION, and the emitted signal's `source_url` so the derived
    /// candidate names the CHANGELOG as its evidence (design §2/§3).
    pub changelog_url: Option<String>,
    /// The repo's README size in bytes (RGSD-4). `Some(bytes)` when the effect
    /// shell's `fetch_readme(owner, repo)` probe found a README (`GET
    /// /repos/{o}/{r}/readme` -> 200 carries the file `size`); `None` when the
    /// repo has no README (404). One disjunct of the
    /// [`SignalKind::DocsPresentAndSubstantial`] DISJUNCTION: a README fires the
    /// signal only when it is SUBSTANTIAL (`bytes >= README_SUBSTANTIAL_BYTES`).
    pub readme_bytes: Option<u64>,
    /// The README's public file URL (RGSD-4). `Some(url)` alongside
    /// `readme_bytes` when a README was found; flows into the
    /// [`SignalKind::DocsPresentAndSubstantial`] signal's `source_url` when the
    /// README disjunct fires, so the derived candidate names the README as its
    /// evidence (design section 3).
    pub readme_url: Option<String>,
    /// The `docs/` directory's public URL (RGSD-4). `Some(url)` when the effect
    /// shell's `content_exists(owner, repo, "docs")` probe returned 200 (a
    /// `docs/` directory is present); `None` when it returned 404 (absent). The
    /// OTHER disjunct of the [`SignalKind::DocsPresentAndSubstantial`]
    /// DISJUNCTION -- a `docs/` dir alone fires the signal even without a
    /// substantial README (design section 2/5).
    pub docs_url: Option<String>,
    /// The `.github/workflows` CI directory's public URL (RGSD-5). `Some(url)`
    /// when the effect shell's `content_exists(owner, repo, ".github/workflows")`
    /// probe returned 200 (the repo runs CI workflows); `None` when it returned
    /// 404 (absent). One disjunct of the [`SignalKind::TestRatioOrCiMatrix`]
    /// DISJUNCTION -- CI workflows alone fire the signal (design section 2/5).
    /// Reuses the RGSD-2 `content_exists` probe (no new effect method).
    pub ci_workflows_url: Option<String>,
    /// The `tests/` directory's public URL (RGSD-5). `Some(url)` when the effect
    /// shell's `content_exists(owner, repo, "tests")` probe returned 200 (the
    /// repo invests in a test suite); `None` when it returned 404 (absent). The
    /// OTHER disjunct of the [`SignalKind::TestRatioOrCiMatrix`] DISJUNCTION -- a
    /// `tests/` dir alone fires the signal even without CI workflows (design
    /// section 2/5). The deferred "test/source ratio > 0.5" precision (a full
    /// recursive tree walk) is NOT claimed; the cheap directory-presence proxy is
    /// used.
    pub tests_dir_url: Option<String>,
}

/// The README byte-size floor at or above which a README counts as SUBSTANTIAL
/// (RGSD-4, design section 5). An honest heuristic: a README of at least this
/// many bytes is thorough enough to evidence the documentation-first
/// philosophy. SPIKE-verified -- real substantial READMEs (ripgrep's `size`
/// 21615) clear it comfortably, while a stub README (octocat/Hello-World's
/// `size` 13) does not.
pub const README_SUBSTANTIAL_BYTES: u64 = 3000;

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
        detect_semver_and_changelog(facts),
        detect_docs_present_and_substantial(facts),
        detect_test_ratio_or_ci_matrix(facts),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The `TestRatioOrCiMatrix` detector arm (RGSD-5): fires when EITHER the repo
/// runs CI workflows (`ci_workflows_url` is `Some`) OR has a `tests/` directory
/// (`tests_dir_url` is `Some`) — the DISJUNCTION (design section 2/5). Either
/// disjunct alone fires it; returns `None` when NEITHER holds. PURE — a total
/// predicate over the facts; the effect-shell `content_exists` probes that fill
/// `ci_workflows_url` / `tests_dir_url` live in `adapter-github` (reusing the
/// RGSD-2 probe, no new effect method).
///
/// The emitted signal is HONEST about which evidence was measured (design
/// section 3): when the CI disjunct fires (CI takes precedence when both are
/// present) it names the CI workflows and is sourced at the workflows URL;
/// otherwise it names the `tests/` directory and is sourced there. The deferred
/// "test/source ratio > 0.5" precision (a full recursive tree walk) is NEVER
/// claimed.
fn detect_test_ratio_or_ci_matrix(facts: &RepoFacts) -> Option<Signal> {
    if let Some(ci_workflows_url) = facts.ci_workflows_url.as_deref() {
        return Some(Signal {
            kind: SignalKind::TestRatioOrCiMatrix,
            value: "CI workflows present (.github/workflows)".to_string(),
            source_url: ci_workflows_url.to_string(),
        });
    }
    let tests_dir_url = facts.tests_dir_url.as_deref()?;
    Some(Signal {
        kind: SignalKind::TestRatioOrCiMatrix,
        value: "tests/ directory present".to_string(),
        source_url: tests_dir_url.to_string(),
    })
}

/// Whether a README of the given byte size counts as SUBSTANTIAL (RGSD-4): its
/// `size` is present AND at or above [`README_SUBSTANTIAL_BYTES`]. PURE + total.
/// A `None` (no README) or a below-threshold size is NOT substantial — the
/// under-firing guard the RGSD-4 negative scenario pins.
fn is_substantial_readme(readme_bytes: Option<u64>) -> bool {
    readme_bytes.is_some_and(|bytes| bytes >= README_SUBSTANTIAL_BYTES)
}

/// The `DocsPresentAndSubstantial` detector arm (RGSD-4): fires when EITHER the
/// repo has a SUBSTANTIAL README (`readme_bytes >= README_SUBSTANTIAL_BYTES`) OR
/// a `docs/` directory is present (`docs_url` is `Some`) — the DISJUNCTION
/// (design section 2/5). Either disjunct alone fires it. Returns `None` when
/// NEITHER holds (a tiny README with no docs dir never fires). PURE — a total
/// predicate over the facts; the effect-shell probes that fill `readme_bytes` /
/// `readme_url` / `docs_url` live in `adapter-github`.
///
/// The emitted signal is HONEST about which evidence was measured (design
/// section 3): when the README disjunct fires it names the actual README byte
/// count and is sourced at the README's URL; otherwise it names the `docs/`
/// directory and is sourced there. The deferred "doc-comment density" refinement
/// is NEVER claimed.
fn detect_docs_present_and_substantial(facts: &RepoFacts) -> Option<Signal> {
    if is_substantial_readme(facts.readme_bytes) {
        let bytes = facts.readme_bytes.unwrap_or_default();
        return Some(Signal {
            kind: SignalKind::DocsPresentAndSubstantial,
            value: format!("substantial README ({bytes} bytes)"),
            // Source at the README the harvest read so the derived candidate
            // names its evidence (design section 3, KPI-SCR-3); fall back to the
            // repo URL if the README URL is somehow absent.
            source_url: facts
                .readme_url
                .clone()
                .unwrap_or_else(|| facts.source_url.clone()),
        });
    }
    let docs_url = facts.docs_url.as_deref()?;
    Some(Signal {
        kind: SignalKind::DocsPresentAndSubstantial,
        value: "docs/ directory present".to_string(),
        source_url: docs_url.to_string(),
    })
}

/// Whether a tag name follows semantic versioning (RGSD-3). PURE + total. A
/// LOOSE match: the name is semver iff it carries a `MAJOR.MINOR.PATCH` numeric
/// core (each component ≥ 1 ASCII digit) somewhere within it, tolerating an
/// optional leading `v` (`v1.2.3`), an optional `<pkgname>-` prefix
/// (`wincolor-0.1.6`), and an optional `-prerelease` / `+build` suffix
/// (`v2.0.0-rc1`). Hand-rolled — NO regex dependency (the pure core stays
/// dependency-light). A name with fewer than three dot-separated numeric
/// components (`v1`, `1.2`) or none at all (`nightly`, `latest`) is NOT semver.
fn is_semver_tag(name: &str) -> bool {
    let bytes = name.as_bytes();
    (0..bytes.len()).any(|start| {
        // A core may only START at a component boundary — a digit not preceded
        // by another digit — so `1.2.3` is found once, not at every digit.
        let starts_component =
            bytes[start].is_ascii_digit() && (start == 0 || !bytes[start - 1].is_ascii_digit());
        starts_component && matches_semver_core_at(bytes, start)
    })
}

/// Whether `bytes[start..]` opens with three dot-separated ASCII-digit groups
/// (`D+.D+.D+`) — the `MAJOR.MINOR.PATCH` core. Any suffix after the third
/// group (a `-rc1` prerelease, a `+build`, or nothing) is tolerated. PURE.
fn matches_semver_core_at(bytes: &[u8], start: usize) -> bool {
    let mut i = start;
    for group in 0..3 {
        let group_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == group_start {
            return false; // each component needs at least one digit
        }
        if group < 2 {
            // the first two components must be followed by a `.` separator
            if i >= bytes.len() || bytes[i] != b'.' {
                return false;
            }
            i += 1;
        }
    }
    true
}

/// The FIRST semver-shaped tag name in `names` (per [`is_semver_tag`]), or
/// `None` when none follow semver (RGSD-3). PURE. Called by the effect shell
/// after `list_tags` to fill [`RepoFacts::semver_tag`].
pub fn pick_semver_tag(names: &[String]) -> Option<String> {
    names.iter().find(|name| is_semver_tag(name)).cloned()
}

/// The `SemverAndChangelog` detector arm (RGSD-3): fires when the repo BOTH
/// publishes a semver-shaped tag (`semver_tag` is `Some`) AND commits a
/// CHANGELOG (`changelog_url` is `Some`) — the CONJUNCTION (design §2). Returns
/// `Some(signal)` sourced at the committed CHANGELOG's public URL, or `None`
/// when EITHER half is absent (semver tags alone, or a CHANGELOG alone, never
/// fires it). PURE — a total predicate over the facts; the effect-shell probes
/// that fill `semver_tag` / `changelog_url` live in `adapter-github`.
fn detect_semver_and_changelog(facts: &RepoFacts) -> Option<Signal> {
    let _semver_tag = facts.semver_tag.as_deref()?;
    let changelog_url = facts.changelog_url.as_deref()?;
    Some(Signal {
        kind: SignalKind::SemverAndChangelog,
        // Honest semantics (design §3): the repo follows semver in its tags AND
        // commits a CHANGELOG — that is exactly what the two probes measured.
        value: "semver tags + CHANGELOG present".to_string(),
        // Source the signal at the committed CHANGELOG so the derived candidate
        // names it as evidence the user can audit (design §3, KPI-SCR-3).
        source_url: changelog_url.to_string(),
    })
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
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
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
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
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
            let facts = RepoFacts {
                language: None,
                source_url,
                cargo_lock_url: None,
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
            };
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
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
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
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
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
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
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

        /// Property (RGSD-3): every tag in the SEMVER corpus — bare, `v`-prefixed,
        /// package-prefixed (`wincolor-0.1.6`), or prerelease-suffixed
        /// (`v2.0.0-rc1`) — is recognized by `is_semver_tag`; every tag in the
        /// NON-semver corpus (release-channel names, too-few components, empty) is
        /// rejected. The LOOSE `MAJOR.MINOR.PATCH` match, hand-rolled (no regex).
        #[test]
        fn is_semver_tag_accepts_the_semver_corpus_and_rejects_the_rest(
            semver in prop::sample::select(vec![
                "1.2.3", "v1.2.3", "wincolor-0.1.6", "14.1.1", "v2.0.0-rc1",
            ]),
            non_semver in prop::sample::select(vec![
                "nightly", "latest", "release", "v1", "1.2", "",
            ]),
        ) {
            prop_assert!(
                is_semver_tag(semver),
                "{semver:?} follows MAJOR.MINOR.PATCH and must be recognized as semver"
            );
            prop_assert!(
                !is_semver_tag(non_semver),
                "{non_semver:?} is not MAJOR.MINOR.PATCH and must NOT be recognized as semver"
            );
        }

        /// Property (RGSD-3): `pick_semver_tag` returns the FIRST semver-shaped
        /// tag in the list, or `None` when the list has none — regardless of how
        /// many non-semver channel names precede it.
        #[test]
        fn pick_semver_tag_finds_the_first_semver_name_or_none(
            channels in prop::collection::vec(
                prop::sample::select(vec!["nightly", "latest", "release"]),
                0..4,
            ),
            semver in prop::sample::select(vec!["1.2.3", "v9.9.9", "wincolor-0.1.6"]),
        ) {
            // A list of only non-semver channel names yields None.
            let channels_only: Vec<String> = channels.iter().map(|s| s.to_string()).collect();
            prop_assert_eq!(pick_semver_tag(&channels_only), None);

            // With a semver tag appended after the channel names, pick finds it
            // (it is the first — and only — semver-shaped name in the list).
            let mut with_semver = channels_only.clone();
            with_semver.push(semver.to_string());
            prop_assert_eq!(pick_semver_tag(&with_semver), Some(semver.to_string()));
        }

        /// Property (RGSD-3, design §2): the `SemverAndChangelog` arm fires IFF
        /// BOTH `semver_tag` AND `changelog_url` are `Some` — the CONJUNCTION.
        /// Neither half alone fires it; when it fires there is EXACTLY ONE such
        /// signal, sourced at the committed CHANGELOG's URL (design §3). The
        /// `language`/`cargo_lock_url` are absent so this arm is isolated.
        #[test]
        fn semver_and_changelog_fires_only_on_the_conjunction(
            semver_present in any::<bool>(),
            changelog_present in any::<bool>(),
            tag in arb_source_url(),
            changelog in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: None,
                source_url,
                cargo_lock_url: None,
                semver_tag: semver_present.then(|| tag.clone()),
                changelog_url: changelog_present.then(|| changelog.clone()),
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: None,
                tests_dir_url: None,
            };
            let signals = detect_signals(&facts);
            let semver_signals: Vec<&Signal> = signals
                .iter()
                .filter(|s| s.kind == SignalKind::SemverAndChangelog)
                .collect();
            prop_assert_eq!(
                !semver_signals.is_empty(),
                semver_present && changelog_present,
                "SemverAndChangelog must fire IFF BOTH halves present (semver={}, changelog={})",
                semver_present, changelog_present
            );
            if semver_present && changelog_present {
                prop_assert_eq!(
                    semver_signals.len(), 1,
                    "the conjunction fires exactly one SemverAndChangelog signal"
                );
                prop_assert_eq!(
                    &semver_signals[0].source_url, &changelog,
                    "the signal must be sourced at the committed CHANGELOG's URL (design §3)"
                );
            }
        }

        /// Property (RGSD-3 + RGSD-4 independence, design section 2): a repo that
        /// is memory-safe AND commits a Cargo.lock AND follows semver with a
        /// CHANGELOG AND has a substantial README + docs dir fires ALL FOUR arms
        /// — the detectors are independent, none suppresses another.
        #[test]
        fn all_four_facts_fire_all_four_arms(
            language in arb_memory_safe_language(),
            cargo_lock_url in arb_source_url(),
            semver_tag in arb_source_url(),
            changelog_url in arb_source_url(),
            readme_url in arb_source_url(),
            docs_url in arb_source_url(),
            readme_bytes in README_SUBSTANTIAL_BYTES..1_000_000,
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: Some(language),
                source_url,
                cargo_lock_url: Some(cargo_lock_url),
                semver_tag: Some(semver_tag),
                changelog_url: Some(changelog_url),
                readme_bytes: Some(readme_bytes),
                readme_url: Some(readme_url),
                docs_url: Some(docs_url),
                ci_workflows_url: None,
                tests_dir_url: None,
            };
            let kinds: Vec<SignalKind> = detect_signals(&facts).iter().map(|s| s.kind).collect();
            prop_assert!(kinds.contains(&SignalKind::MemorySafetyLanguage));
            prop_assert!(kinds.contains(&SignalKind::DependencyManifestPinned));
            prop_assert!(kinds.contains(&SignalKind::SemverAndChangelog));
            prop_assert!(kinds.contains(&SignalKind::DocsPresentAndSubstantial));
        }

        /// Property (RGSD-4, design section 2/5): the `DocsPresentAndSubstantial`
        /// arm fires IFF EITHER the README is SUBSTANTIAL
        /// (`readme_bytes >= README_SUBSTANTIAL_BYTES`) OR a `docs/` dir is
        /// present (`docs_url` is `Some`) — the DISJUNCTION. When it fires there
        /// is EXACTLY ONE such signal; when the README disjunct is the one that
        /// fires the signal is HONEST — it names the README and is sourced at the
        /// README URL (design section 3); otherwise it names the docs dir and is
        /// sourced there. `language`/`cargo_lock`/semver are absent so this arm
        /// is isolated. `readme_bytes` spans both sides of the threshold.
        #[test]
        fn docs_present_fires_only_on_the_readme_or_docs_disjunction(
            readme_bytes in prop::option::of(0u64..2 * README_SUBSTANTIAL_BYTES),
            docs_present in any::<bool>(),
            readme_url in arb_source_url(),
            docs_url in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: None,
                source_url,
                cargo_lock_url: None,
                semver_tag: None,
                changelog_url: None,
                readme_bytes,
                readme_url: readme_bytes.map(|_| readme_url.clone()),
                docs_url: docs_present.then(|| docs_url.clone()),
                ci_workflows_url: None,
                tests_dir_url: None,
            };
            let signals = detect_signals(&facts);
            let docs_signals: Vec<&Signal> = signals
                .iter()
                .filter(|s| s.kind == SignalKind::DocsPresentAndSubstantial)
                .collect();
            let substantial = readme_bytes.is_some_and(|b| b >= README_SUBSTANTIAL_BYTES);
            let should_fire = substantial || docs_present;
            prop_assert_eq!(
                !docs_signals.is_empty(),
                should_fire,
                "DocsPresentAndSubstantial must fire IFF a substantial README OR a docs dir \
                 (readme_bytes={:?}, docs_present={})",
                readme_bytes, docs_present
            );
            if should_fire {
                prop_assert_eq!(
                    docs_signals.len(), 1,
                    "the disjunction fires exactly one DocsPresentAndSubstantial signal"
                );
                if substantial {
                    prop_assert!(
                        docs_signals[0].value.contains("README"),
                        "a substantial README must name the README in its value (design section 3); got {:?}",
                        docs_signals[0].value
                    );
                    prop_assert_eq!(
                        &docs_signals[0].source_url, &readme_url,
                        "the README disjunct must source the signal at the README URL (design section 3)"
                    );
                } else {
                    prop_assert_eq!(
                        &docs_signals[0].source_url, &docs_url,
                        "the docs-dir disjunct must source the signal at the docs/ dir URL (design section 3)"
                    );
                }
            }
        }

        /// Property (RGSD-4 boundary + under-firing guard, design section 5): a
        /// README EXACTLY at [`README_SUBSTANTIAL_BYTES`] with NO docs dir fires
        /// the signal (>= is inclusive), while a README ONE byte below the
        /// threshold with NO docs dir does NOT — a tiny README alone never
        /// counts (the guardrail the RGSD-4 negative scenario pins).
        #[test]
        fn readme_at_threshold_fires_but_one_below_does_not(
            readme_url in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let fires = |bytes: u64| {
                let facts = RepoFacts {
                    language: None,
                    source_url: source_url.clone(),
                    cargo_lock_url: None,
                    semver_tag: None,
                    changelog_url: None,
                    readme_bytes: Some(bytes),
                    readme_url: Some(readme_url.clone()),
                    docs_url: None,
                    ci_workflows_url: None,
                    tests_dir_url: None,
                };
                detect_signals(&facts)
                    .iter()
                    .any(|s| s.kind == SignalKind::DocsPresentAndSubstantial)
            };
            prop_assert!(
                fires(README_SUBSTANTIAL_BYTES),
                "a README exactly at the threshold must fire (>= is inclusive)"
            );
            prop_assert!(
                !fires(README_SUBSTANTIAL_BYTES - 1),
                "a README one byte below the threshold with no docs dir must NOT fire"
            );
        }

        /// Property (RGSD-5, design section 2/5): the `TestRatioOrCiMatrix` arm
        /// fires IFF EITHER CI workflows are present (`ci_workflows_url` is
        /// `Some`) OR a `tests/` dir is present (`tests_dir_url` is `Some`) — the
        /// DISJUNCTION. Neither disjunct alone is required; when it fires there is
        /// EXACTLY ONE such signal, and it is HONEST about which evidence was
        /// measured (design section 3): when the CI disjunct fires it names the CI
        /// workflows and is sourced at the workflows URL (CI takes precedence when
        /// both are present); otherwise it names the `tests/` dir and is sourced
        /// there. `language`/`cargo_lock`/semver/docs are absent so this arm is
        /// isolated. Both booleans span present/absent so all four quadrants of
        /// the disjunction are exercised.
        #[test]
        fn test_ratio_or_ci_matrix_fires_only_on_the_ci_or_tests_disjunction(
            ci_present in any::<bool>(),
            tests_present in any::<bool>(),
            ci_url in arb_source_url(),
            tests_url in arb_source_url(),
            source_url in arb_source_url(),
        ) {
            let facts = RepoFacts {
                language: None,
                source_url,
                cargo_lock_url: None,
                semver_tag: None,
                changelog_url: None,
                readme_bytes: None,
                readme_url: None,
                docs_url: None,
                ci_workflows_url: ci_present.then(|| ci_url.clone()),
                tests_dir_url: tests_present.then(|| tests_url.clone()),
            };
            let signals = detect_signals(&facts);
            let test_signals: Vec<&Signal> = signals
                .iter()
                .filter(|s| s.kind == SignalKind::TestRatioOrCiMatrix)
                .collect();
            let should_fire = ci_present || tests_present;
            prop_assert_eq!(
                !test_signals.is_empty(),
                should_fire,
                "TestRatioOrCiMatrix must fire IFF CI workflows OR a tests/ dir \
                 (ci_present={}, tests_present={})",
                ci_present, tests_present
            );
            if should_fire {
                prop_assert_eq!(
                    test_signals.len(), 1,
                    "the disjunction fires exactly one TestRatioOrCiMatrix signal"
                );
                // The deferred "test/source ratio > 0.5" precision is NEVER
                // claimed (design section 3): the value must not claim a ratio.
                prop_assert!(
                    !test_signals[0].value.to_lowercase().contains("ratio"),
                    "RGSD-5 must not claim a test/source ratio was computed (design section 3); got {:?}",
                    test_signals[0].value
                );
                if ci_present {
                    // CI takes precedence when both are present: sourced at the
                    // workflows URL and names the CI workflows (design section 3).
                    prop_assert!(
                        test_signals[0].value.contains("workflows"),
                        "a present CI directory must name the CI workflows in its value; got {:?}",
                        test_signals[0].value
                    );
                    prop_assert_eq!(
                        &test_signals[0].source_url, &ci_url,
                        "the CI disjunct must source the signal at the .github/workflows URL"
                    );
                } else {
                    // tests-only: sourced at the tests/ dir URL and names it.
                    prop_assert!(
                        test_signals[0].value.contains("tests/"),
                        "the tests-dir disjunct must name the tests/ dir in its value; got {:?}",
                        test_signals[0].value
                    );
                    prop_assert_eq!(
                        &test_signals[0].source_url, &tests_url,
                        "the tests-dir disjunct must source the signal at the tests/ dir URL"
                    );
                }
            }
        }
    }

    /// Boundary behavior of the hand-rolled `is_semver_tag` scan (RGSD-3): a core
    /// is found starting at ANY component boundary (a digit not preceded by a
    /// digit), so a `MAJOR.MINOR.PATCH` run that immediately follows non-digit
    /// characters (`abc234.5.6` — the digits open a fresh run after `c`) IS
    /// recognized. This pins the `starts_component` boundary predicate: because
    /// `matches_semver_core_at` depends only on the digit-run's END and the char
    /// after it — never on how far into the run the scan started — a match at any
    /// mid-run offset always coincides with a match at that run's boundary, so
    /// the boundary scan loses no real semver tag (and the boundary check is not
    /// an equivalent no-op: it stops `is_semver_tag` from re-scanning every digit
    /// of a run). Example-based (Mandate 11): these are the exact boundary points.
    #[test]
    fn is_semver_tag_matches_at_a_component_boundary_after_non_digits() {
        // A digit run that opens a valid core right after letters matches at that
        // run's boundary (the digits are a fresh component, not mid-run).
        assert!(is_semver_tag("abc234.5.6"));
        assert!(is_semver_tag("12.34.56"));
        assert!(is_semver_tag("release-1.2.3"));
        // Fewer than three numeric components is never semver, whatever the
        // surrounding characters.
        assert!(!is_semver_tag("1.2"));
        assert!(!is_semver_tag("v1"));
        assert!(!is_semver_tag("abc.def.ghi"));
        assert!(!is_semver_tag(""));
    }
}
