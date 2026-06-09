//! Integration tests for the viewer active-set-read fault-injection seam
//! release-build guard (slice-16 / US-SF-001 / Theme E / C-7 / ADR-053
//! §Earned-Trust) added to `cargo xtask check-arch` in step 02-03.
//!
//! The slice-16 `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` env seam exists ONLY to
//! let the SF-8 acceptance scenario INDUCE a mid-request active-set read failure
//! and observe the production graceful-degrade path (`Err → empty set →
//! all-NetworkUnfollowed`). Mirroring the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_`
//! seam discipline, this fault injector is RELEASE-FORBIDDEN: every read of the
//! token MUST sit inside a `#[cfg(debug_assertions)]`-gated item, else it would
//! compile a degrade backdoor into a release binary.
//!
//! These exercise the EFFECT-shell scan [`xtask::check_arch::scan_viewer_fail_seam_guard`]
//! against a temp workspace so the guard is falsifiable (catches an ungated seam,
//! passes the gated one) — never an always-green check.

use xtask::check_arch::scan_viewer_fail_seam_guard;

/// The SAFE shape this task lands: the `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`
/// read sits inside a `#[cfg(debug_assertions)]` function, so it compiles ONLY in
/// debug/test builds, never release. MUST pass the scan.
/// NOTE the MULTI-LINE signature: rustfmt breaks the `fn` over three depth-0
/// lines between the `#[cfg(debug_assertions)]` attribute and the opening `{`.
/// The guard's cfg-attribute classifier MUST still recognize this item as gated
/// (a depth-0 line that opens no brace must NOT drop the pending cfg gate) — this
/// is the exact production layout in `adapter-http-viewer/src/lib.rs`.
const DEBUG_ASSERTIONS_GATED_SEAM: &str = r#"
#[cfg(debug_assertions)]
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
}

#[cfg(not(debug_assertions))]
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    read
}
"#;

/// An UNSAFE shape: the `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` read leaks into a
/// function with NO cfg gate, so it WOULD compile into a release binary (a
/// production /search could be forced to degrade). MUST be flagged.
const UNGUARDED_SEAM_LEAK: &str = r#"
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
}
"#;

/// A viewer with no fault-seam token at all is trivially clean. MUST pass.
const NO_SEAM_SOURCE: &str = r#"
fn index_query_active_set(store: &dyn StoreReadPort) -> HashSet<String> {
    store.list_active_peer_subscriptions().map(collect).unwrap_or_default()
}
"#;

/// Lay down `crates/adapter-http-viewer/src/lib.rs` under `root` with the given
/// contents, creating parent dirs. Returns the file path.
fn write_viewer_src(root: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let dir = root.join("crates/adapter-http-viewer/src");
    std::fs::create_dir_all(&dir).expect("create adapter-http-viewer/src dir");
    let path = dir.join("lib.rs");
    std::fs::write(&path, contents).expect("write viewer source fixture");
    path
}

#[test]
fn cfg_gated_fault_seam_passes_the_scan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), DEBUG_ASSERTIONS_GATED_SEAM);

    let findings = scan_viewer_fail_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.is_empty(),
        "a #[cfg(debug_assertions)]-gated OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ read MUST \
         pass the scan (release-forbidden satisfied), got: {findings:?}"
    );
}

#[test]
fn ungated_fault_seam_in_release_path_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), UNGUARDED_SEAM_LEAK);

    let findings = scan_viewer_fail_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.iter().any(|f| f.contains("adapter-http-viewer")),
        "an OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ read NOT behind a cfg gate would ship a \
         degrade backdoor in a release binary and MUST be flagged, got: {findings:?}"
    );
}

#[test]
fn viewer_without_the_seam_token_is_clean() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), NO_SEAM_SOURCE);

    assert!(
        scan_viewer_fail_seam_guard(tmp.path())
            .expect("scan runs")
            .is_empty(),
        "viewer source with no OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ token must pass"
    );
}
