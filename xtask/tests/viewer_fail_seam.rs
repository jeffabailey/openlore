//! Integration tests for the viewer fault-injection seam release-build guard
//! (slice-16 / US-SF-001 / ADR-053 §Earned-Trust — active-set read; slice-17 /
//! US-LD-000/001 / Theme 4 / ADR-054 D2 — peer-claims count) added to / extended
//! in `cargo xtask check-arch`.
//!
//! The viewer fault-injection env seams exist ONLY to let acceptance scenarios
//! INDUCE a mid-request read failure and observe the production graceful-degrade
//! path: `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` (slice-16: `/search` active-set
//! read → empty set → all-NetworkUnfollowed) and `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_
//! COUNT` (slice-17: `GET /` peer-claims count → `.ok() → None →
//! MISSING_COUNT_MARKER "—"`). Mirroring the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_`
//! seam discipline, EACH fault injector is RELEASE-FORBIDDEN: every read of EACH
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

/// slice-17 (ADR-054 D2): the SAFE shape for the peer-claims-count seam — the
/// `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` read sits inside a
/// `#[cfg(debug_assertions)]` function, compiling ONLY in debug/test builds, never
/// release. MUST pass the scan. Same multi-line-signature layout as production.
const PEER_CLAIMS_COUNT_GATED_SEAM: &str = r#"
#[cfg(debug_assertions)]
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
}

#[cfg(not(debug_assertions))]
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    read
}
"#;

/// slice-17: an UNSAFE shape — the `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` read
/// leaks into a function with NO cfg gate, so it WOULD compile into a release
/// binary (a production `GET /` peer-claims count could be forced to degrade to the
/// missing marker). MUST be flagged.
const PEER_CLAIMS_COUNT_UNGATED_LEAK: &str = r#"
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
}
"#;

/// The PRODUCTION layout: BOTH viewer fault seams (active-set + peer-claims-count)
/// gated in the same source. The guard must pass when EVERY token is correctly
/// gated — the realistic post-slice-17 viewer shape.
const BOTH_SEAMS_GATED: &str = r#"
#[cfg(debug_assertions)]
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
}

#[cfg(debug_assertions)]
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT").is_some() {
        return Err(StoreReadError::Unreadable { detail: "fault injected".to_string() });
    }
    read
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
        "viewer source with no fault-seam token must pass"
    );
}

// -----------------------------------------------------------------------------
// slice-17 (ADR-054 D2): the peer-claims-count fault seam is covered by the SAME
// extended guard (the new token appended to the guard's scanned-token set), so an
// UNGATED `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` read also fails check-arch.
// -----------------------------------------------------------------------------

#[test]
fn cfg_gated_peer_claims_count_seam_passes_the_scan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), PEER_CLAIMS_COUNT_GATED_SEAM);

    let findings = scan_viewer_fail_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.is_empty(),
        "a #[cfg(debug_assertions)]-gated OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT read MUST \
         pass the scan (release-forbidden satisfied), got: {findings:?}"
    );
}

#[test]
fn ungated_peer_claims_count_seam_in_release_path_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), PEER_CLAIMS_COUNT_UNGATED_LEAK);

    let findings = scan_viewer_fail_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings
            .iter()
            .any(|f| f.contains("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT")
                && f.contains("adapter-http-viewer")),
        "an OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT read NOT behind a cfg gate would ship a \
         degrade backdoor in a release binary and MUST be flagged, got: {findings:?}"
    );
}

#[test]
fn both_viewer_fault_seams_gated_passes_the_scan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_viewer_src(tmp.path(), BOTH_SEAMS_GATED);

    let findings = scan_viewer_fail_seam_guard(tmp.path()).expect("scan runs");
    assert!(
        findings.is_empty(),
        "the production viewer with BOTH fault seams gated (active-set + peer-claims-count) \
         MUST pass the scan, got: {findings:?}"
    );
}
