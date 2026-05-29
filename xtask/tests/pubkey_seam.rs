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
use xtask::check_arch::{classify_pubkey_seam_guard, scan_pubkey_seam_guard};

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

// -----------------------------------------------------------------------------
// Broadened-scope (ADR-026): the scan covers `peer_resolve.rs`, not just `lib.rs`.
//
// Step 03-01 added the slice-05 `resolve_verification_key_via_seam` to
// `peer_resolve.rs` and NARROWED the scan to `lib.rs` to dodge the (then-ungated)
// peer_resolve seam. ADR-026 requires the seam to be release-FORBIDDEN: an
// UNGATED `OPENLORE_PEER_PUBKEY_HEX_` read ANYWHERE in `adapter-atproto-did`
// (including `peer_resolve.rs`) MUST fail the rule. These tests drive the
// EFFECT-shell scan (not just the pure classifier) against a temp workspace so
// the GUARDED-SOURCES list — the load-bearing scope axis — is exercised directly.
// -----------------------------------------------------------------------------

/// Lay down a `crates/adapter-atproto-did/src/<name>` file under `root` with the
/// given contents, creating parent dirs. Returns the file path.
fn write_adapter_did_src(root: &std::path::Path, name: &str, contents: &str) -> std::path::PathBuf {
    let dir = root.join("crates/adapter-atproto-did/src");
    std::fs::create_dir_all(&dir).expect("create adapter-atproto-did/src dir");
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write adapter source fixture");
    path
}

#[test]
fn ungated_seam_in_peer_resolve_is_flagged_by_the_scan() {
    // The broadened scope: an UNGATED seam read living in `peer_resolve.rs`
    // (NOT `lib.rs`) must be caught now (ADR-026). Before broadening, the scan
    // only looked at `lib.rs`, so this leak slipped through.
    let tmp = tempfile::tempdir().expect("tempdir");
    // A clean lib.rs so the only possible finding is the peer_resolve seam.
    write_adapter_did_src(tmp.path(), "lib.rs", NO_SEAM_SOURCE);
    write_adapter_did_src(tmp.path(), "peer_resolve.rs", UNGUARDED_SEAM_LEAK);

    let findings = scan_pubkey_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.iter().any(|f| f.contains("peer_resolve.rs")),
        "an ungated OPENLORE_PEER_PUBKEY_HEX_ read in peer_resolve.rs MUST be \
         flagged by the broadened scan (ADR-026), got: {findings:?}"
    );
}

#[test]
fn cfg_gated_seam_in_peer_resolve_passes_the_scan() {
    // The now-gated production shape: a `#[cfg(debug_assertions)]`-gated seam in
    // `peer_resolve.rs` compiles OUT of release, so the broadened scan passes it
    // (no false positive on the legitimate debug-only short-circuit).
    let tmp = tempfile::tempdir().expect("tempdir");
    write_adapter_did_src(tmp.path(), "lib.rs", NO_SEAM_SOURCE);
    write_adapter_did_src(tmp.path(), "peer_resolve.rs", DEBUG_ASSERTIONS_GATED_SEAM);

    let findings = scan_pubkey_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.is_empty(),
        "a #[cfg(debug_assertions)]-gated seam in peer_resolve.rs MUST pass the \
         broadened scan (release-forbidden satisfied), got: {findings:?}"
    );
}

/// The release-gating shape this task lands: the seam read sits inside a
/// `#[cfg(debug_assertions)]` function, so it compiles ONLY in debug/test builds,
/// never release. MUST pass the scan.
const DEBUG_ASSERTIONS_GATED_SEAM: &str = r#"
#[cfg(debug_assertions)]
fn seam_verification_key(did: &str) -> Option<Vec<u8>> {
    let var = format!("OPENLORE_PEER_PUBKEY_HEX_{}", did_to_env(did));
    std::env::var(&var).ok().and_then(|hex| decode_hex(&hex))
}

#[cfg(not(debug_assertions))]
fn seam_verification_key(_did: &str) -> Option<Vec<u8>> {
    None
}
"#;

#[test]
fn debug_assertions_gated_seam_classifier_passes() {
    // The pure classifier accepts `#[cfg(debug_assertions)]` (any cfg gate) —
    // pin it directly so the gate-flavor this task uses is covered alongside the
    // test-feature gates.
    assert!(
        classify_pubkey_seam_guard(DEBUG_ASSERTIONS_GATED_SEAM).is_none(),
        "a #[cfg(debug_assertions)]-gated seam read must pass the classifier"
    );
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
