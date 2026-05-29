//! Integration tests for the `no_pubkey_seam_in_release_build` rule (I-AV-6 /
//! ADR-026) added to `cargo xtask check-arch` in step 01-04.
//!
//! The slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam is RETAINED for tests
//! but RELEASE-FORBIDDEN: a release build that reads it would resolve a pubkey
//! from the environment instead of the REAL ADR-026 PLC `z6Mk...` decode (AV-4,
//! step 03-04, runs the real decode with the seam UNSET). This rule mirrors the
//! slice-03 `no_autoconfirm_in_release_build` (D-D20) shape: every occurrence of
//! the `OPENLORE_PEER_PUBKEY_HEX_` token MUST sit inside a `#[cfg(...)]`-gated
//! item, else it would compile into a release binary.
//!
//! These exercise the PURE classifier
//! [`xtask::check_arch::classify_pubkey_seam_guard`] against in-memory source
//! snippets (no filesystem) PLUS a proptest covering the equivalence classes,
//! symmetric with the slice-03 autoconfirm_guard / slice-02 I-SCR-1 tests.

use proptest::prelude::*;
use xtask::check_arch::classify_pubkey_seam_guard;

// -----------------------------------------------------------------------------
// Hand-written fixtures (the canonical safe + leak shapes).
// -----------------------------------------------------------------------------

/// The SAFE shape: the `OPENLORE_PEER_PUBKEY_HEX_<did>` read lives ONLY inside a
/// `#[cfg(any(test, feature = "test-pubkey-seam"))]` function; the release-build
/// sibling does the real decode with NO env read. D-D20-equivalent satisfied —
/// MUST pass.
const SAFE_GUARDED_SEAM: &str = r#"
#[cfg(any(test, feature = "test-pubkey-seam"))]
fn resolve_seam(did: &str) -> Option<Vec<u8>> {
    let var = format!("OPENLORE_PEER_PUBKEY_HEX_{}", did_to_env(did));
    std::env::var(&var).ok().and_then(|hex| decode_hex(&hex))
}

#[cfg(not(any(test, feature = "test-pubkey-seam")))]
fn resolve_seam(_did: &str) -> Option<Vec<u8>> {
    None
}
"#;

/// An UNSAFE shape: the `OPENLORE_PEER_PUBKEY_HEX_` read leaks into a function
/// that has NO cfg gate, so it WOULD compile into a release binary (the
/// production path could short-circuit the real ADR-026 decode). MUST be flagged.
const UNGUARDED_SEAM_LEAK: &str = r#"
fn resolve_verification_key(did: &str) -> Option<Vec<u8>> {
    let var = format!("OPENLORE_PEER_PUBKEY_HEX_{}", did_to_env(did));
    std::env::var(&var).ok().and_then(|hex| decode_hex(&hex))
}
"#;

/// A source with no pubkey-seam token at all is trivially clean. MUST pass.
const NO_SEAM_SOURCE: &str = r#"
fn resolve_verification_key(did: &str) -> Option<Vec<u8>> {
    decode_ed25519_multibase(&resolve_plc_pubkey(did)?).ok()
}
"#;

#[test]
fn cfg_gated_pubkey_seam_passes() {
    let verdict = classify_pubkey_seam_guard(SAFE_GUARDED_SEAM);
    assert!(
        verdict.is_none(),
        "cfg-gated OPENLORE_PEER_PUBKEY_HEX_ read must pass (I-AV-6 satisfied); \
         classifier flagged: {verdict:?}"
    );
}

#[test]
fn ungated_pubkey_seam_in_release_path_is_rejected() {
    let verdict = classify_pubkey_seam_guard(UNGUARDED_SEAM_LEAK);
    assert!(
        verdict.is_some(),
        "an OPENLORE_PEER_PUBKEY_HEX_ read NOT behind a cfg gate would ship in a \
         release binary (I-AV-6 violation) and MUST be flagged"
    );
}

#[test]
fn source_without_the_seam_token_is_clean() {
    assert!(
        classify_pubkey_seam_guard(NO_SEAM_SOURCE).is_none(),
        "source with no OPENLORE_PEER_PUBKEY_HEX_ token must pass"
    );
}

// -----------------------------------------------------------------------------
// Property-based: the gated vs ungated equivalence classes (PBT mandate;
// symmetric with autoconfirm_guard's release-build guard).
// -----------------------------------------------------------------------------

/// Build a function that reads the pubkey seam, wrapped in a chosen cfg-gate
/// attribute. The DID-suffix and the function name vary; the gate is the
/// load-bearing axis.
fn gated_seam_source(gate: &str, fn_name: &str) -> String {
    format!(
        "{gate}\nfn {fn_name}(did: &str) -> Option<Vec<u8>> {{\n    \
         let v = format!(\"OPENLORE_PEER_PUBKEY_HEX_{{}}\", did);\n    \
         std::env::var(&v).ok().map(|h| h.into_bytes())\n}}\n"
    )
}

/// A cfg gate that compiles OUT of a release build (the seam read is test-only).
fn release_excluding_gate_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("#[cfg(test)]".to_string()),
        Just("#[cfg(any(test, feature = \"test-pubkey-seam\"))]".to_string()),
        Just("#[cfg(feature = \"test-pubkey-seam\")]".to_string()),
    ]
}

proptest! {
    /// Property: ANY cfg-gated pubkey-seam read passes (no false positives over
    /// the safe equivalence class — every test-only gate compiles out of release).
    #[test]
    fn all_cfg_gated_seam_reads_pass(
        gate in release_excluding_gate_strategy(),
        fn_name in "resolve_seam[a-z]{0,6}",
    ) {
        let src = gated_seam_source(&gate, &fn_name);
        prop_assert!(
            classify_pubkey_seam_guard(&src).is_none(),
            "a cfg-gated OPENLORE_PEER_PUBKEY_HEX_ read MUST pass, but the \
             classifier flagged it:\n{src}"
        );
    }

    /// Property: ANY ungated pubkey-seam read is flagged (the whole leak
    /// equivalence class fires — an ungated read ships in release).
    #[test]
    fn all_ungated_seam_reads_are_rejected(
        fn_name in "resolve[a-z]{0,6}",
    ) {
        let src = gated_seam_source("", &fn_name);
        prop_assert!(
            classify_pubkey_seam_guard(&src).is_some(),
            "an ungated OPENLORE_PEER_PUBKEY_HEX_ read MUST be flagged, but the \
             classifier passed it:\n{src}"
        );
    }
}
