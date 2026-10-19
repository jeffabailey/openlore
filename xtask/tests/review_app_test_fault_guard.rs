//! `cargo xtask check-arch` rule `review_app_test_fault_seam` (DWD-RRA-16):
//! the review app's `REVIEW_APP_TEST_FAULT` fault names compile only into
//! development builds. Exercised against a temp workspace so the guard is
//! falsifiable: it flags a synthetic ungated read and passes the gated shape
//! production uses.

use std::path::Path;

use xtask::check_arch::scan_review_app_test_fault_guard;

/// The production shape (config.rs): the fault name sits inside a
/// `#[cfg(debug_assertions)]` function; the release twin knows no name.
const GATED: &str = r#"
#[cfg(debug_assertions)]
fn test_fault(value: &str) -> Result<TestFault, ConfigError> {
    match value {
        "migrate_before_commit" => Ok(TestFault::MigrateBeforeCommit),
        _ => Err(ConfigError::Invalid { name: TEST_FAULT_VAR, reason: "unknown" }),
    }
}

#[cfg(not(debug_assertions))]
fn test_fault(_value: &str) -> Result<TestFault, ConfigError> {
    Err(ConfigError::TestFaultInRelease)
}
"#;

/// A synthetic violation: the fault name read with no cfg gate, so a release
/// binary would honour it.
const UNGATED: &str = r#"
fn test_fault(value: &str) -> Option<TestFault> {
    (value == "migrate_before_commit").then_some(TestFault::MigrateBeforeCommit)
}
"#;

/// Review R7: the store's fault variant compiled into every build.
const UNGATED_VARIANT: &str = r#"
pub enum MigrationFault {
    StopBeforeCommit,
}
"#;

/// The production shape (adapter-review-store lib.rs): the variant exists
/// only in a development build; the release twin has no values.
const GATED_VARIANT: &str = r#"
#[cfg(debug_assertions)]
pub enum MigrationFault {
    StopBeforeCommit,
}

#[cfg(not(debug_assertions))]
pub enum MigrationFault {}
"#;

fn workspace_with(rel: &str, source: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temp workspace");
    let path = root.path().join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, source).expect("write source");
    root
}

fn findings(root: &Path) -> Vec<String> {
    scan_review_app_test_fault_guard(root).expect("scan")
}

#[test]
fn a_gated_fault_name_passes() {
    let root = workspace_with("crates/openlore-review-app/src/config.rs", GATED);
    assert_eq!(findings(root.path()), Vec::<String>::new());
}

#[test]
fn an_ungated_fault_name_fails_in_the_app_or_the_store() {
    for rel in [
        "crates/openlore-review-app/src/config.rs",
        "crates/adapter-review-store/src/lib.rs",
    ] {
        let root = workspace_with(rel, UNGATED);
        let found = findings(root.path());
        assert_eq!(found.len(), 1, "{rel}: {found:?}");
        assert!(found[0].contains("migrate_before_commit"), "{}", found[0]);
    }
}

#[test]
fn an_ungated_store_fault_variant_fails() {
    let root = workspace_with("crates/adapter-review-store/src/lib.rs", UNGATED_VARIANT);
    let found = findings(root.path());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("StopBeforeCommit"), "{}", found[0]);
}

#[test]
fn a_gated_store_fault_variant_passes() {
    let root = workspace_with("crates/adapter-review-store/src/lib.rs", GATED_VARIANT);
    assert_eq!(findings(root.path()), Vec::<String>::new());
}

#[test]
fn the_real_tree_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    assert_eq!(findings(&root), Vec::<String>::new());
}
