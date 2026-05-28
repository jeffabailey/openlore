//! Integration tests for the autoconfirm release-build guard rule (D-D20)
//! added to `cargo xtask check-arch` in step 01-06.
//!
//! WD-21 forbids a `--yes` flag in production; the test escape hatch
//! `OPENLORE_TEST_AUTOCONFIRM` (in `crates/cli/src/verbs/peer_remove.rs`)
//! MUST be compiled out of release builds. This rule asserts that the
//! `OPENLORE_TEST_AUTOCONFIRM` token only appears within a `#[cfg(...)]`-gated
//! region (the env-var read and the `autoconfirm_purge()` fn are gated on
//! `cfg(any(test, feature = "test-autoconfirm"))`).
//!
//! These exercise the PURE classifier
//! [`xtask::check_arch::classify_autoconfirm_guard`] against in-memory source
//! snippets. The release-build code path must read NO env var; the test path
//! reads it only under the cfg gate.

use xtask::check_arch::classify_autoconfirm_guard;

/// A faithful (trimmed) mirror of the real `peer_remove.rs` guard: the env-var
/// read lives ONLY inside the `#[cfg(any(test, feature = "test-autoconfirm"))]`
/// function; the release-build sibling is a `const false` with no env read.
/// This is the SAFE shape (D-D20 satisfied) — MUST pass.
const SAFE_GUARDED_SOURCE: &str = r#"
#[cfg(any(test, feature = "test-autoconfirm"))]
pub fn autoconfirm_purge() -> bool {
    std::env::var("OPENLORE_TEST_AUTOCONFIRM")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[cfg(not(any(test, feature = "test-autoconfirm")))]
pub fn autoconfirm_purge() -> bool {
    false
}
"#;

/// An UNSAFE shape: the `OPENLORE_TEST_AUTOCONFIRM` read leaks into a function
/// that has NO cfg gate, so it WOULD compile into a release binary. MUST be
/// flagged.
const UNGUARDED_LEAK_SOURCE: &str = r#"
pub fn autoconfirm_purge() -> bool {
    std::env::var("OPENLORE_TEST_AUTOCONFIRM")
        .map(|v| v == "1")
        .unwrap_or(false)
}
"#;

/// A source with no autoconfirm token at all is trivially clean (no token to
/// gate). MUST pass.
const NO_TOKEN_SOURCE: &str = r#"
pub fn run() -> bool {
    true
}
"#;

#[test]
fn cfg_gated_autoconfirm_token_passes() {
    let verdict = classify_autoconfirm_guard(SAFE_GUARDED_SOURCE);
    assert!(
        verdict.is_none(),
        "cfg-gated OPENLORE_TEST_AUTOCONFIRM read must pass (D-D20 satisfied); \
         classifier flagged: {verdict:?}"
    );
}

#[test]
fn ungated_autoconfirm_token_in_release_path_is_rejected() {
    let verdict = classify_autoconfirm_guard(UNGUARDED_LEAK_SOURCE);
    assert!(
        verdict.is_some(),
        "an OPENLORE_TEST_AUTOCONFIRM read NOT behind a cfg gate would ship in \
         a release binary (D-D20 violation) and MUST be flagged"
    );
}

#[test]
fn source_without_the_token_is_clean() {
    assert!(
        classify_autoconfirm_guard(NO_TOKEN_SOURCE).is_none(),
        "source with no OPENLORE_TEST_AUTOCONFIRM token must pass"
    );
}

#[test]
fn the_real_peer_remove_source_satisfies_the_guard() {
    // End-to-end against the ACTUAL production file the rule guards. This is
    // the load-bearing assertion: the shipped guard in peer_remove.rs must be
    // cfg-gated. (Resolves the path relative to the workspace root; xtask
    // tests run from the xtask crate dir, so climb one level.)
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/cli/src/verbs/peer_remove.rs"
    ))
    .expect("read crates/cli/src/verbs/peer_remove.rs");
    let verdict = classify_autoconfirm_guard(&src);
    assert!(
        verdict.is_none(),
        "the real peer_remove.rs autoconfirm guard MUST be cfg-gated (D-D20); \
         classifier flagged: {verdict:?}"
    );
}
