//! review-app-repos-and-accounts Phase 5 (crafter-owned): the review-app
//! check-arch shells find what is in the tree they are given (not only that
//! the real tree passes), rules see into non-test inline modules, and the
//! whole check passes on the real tree.

use std::path::Path;

use xtask::check_arch::{
    classify_authority_evidence, classify_publish_plan_needs_permit, classify_signing_permit_serde,
    run, scan_authority_evidence_only_from_adapters,
    scan_review_accounts_write_only_in_github_route, scan_review_publish_plan_needs_permit,
    scan_signing_permit_not_serializable,
};

const FABRICATED: &str =
    "fn stored(r: &Row) -> RepoFacts { RepoFacts { repo_id: r.id, owner_id: r.owner, ..r.facts() } }";
const PERMIT_SOURCE: &str = "crates/claim-domain/src/authority.rs";

/// A workspace tree holding only `files`.
fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tree");
    for (rel_path, body) in files {
        let path = root.path().join(rel_path);
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).expect("dirs");
        std::fs::write(path, body).expect("file");
    }
    root
}

#[test]
fn the_review_app_scans_report_an_empty_or_violating_tree() {
    let empty = tree(&[]);
    assert!(
        !scan_review_accounts_write_only_in_github_route(empty.path())
            .expect("scan")
            .is_empty()
    );
    assert!(!scan_review_publish_plan_needs_permit(empty.path())
        .expect("scan")
        .is_empty());
    assert!(!scan_signing_permit_not_serializable(empty.path())
        .expect("scan")
        .is_empty());

    let fabricated = tree(&[("crates/review-domain/src/plans.rs", FABRICATED)]);
    assert!(
        !scan_authority_evidence_only_from_adapters(fabricated.path())
            .expect("scan")
            .is_empty()
    );
}

/// Only the permit's own module defining it keeps the rule from passing
/// vacuously: a `SigningPermit` defined elsewhere does not.
#[test]
fn a_permit_defined_outside_its_module_leaves_the_rule_vacuous() {
    let decoy = tree(&[(
        "crates/elsewhere/src/lib.rs",
        "pub struct SigningPermit { signer: String }",
    )]);
    let findings = scan_signing_permit_not_serializable(decoy.path()).expect("scan");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].contains("would pass vacuously"), "{findings:?}");
}

/// A trait other than serde implemented for the permit is no finding.
#[test]
fn a_non_serde_impl_for_the_permit_is_allowed() {
    let (defined, findings) = classify_signing_permit_serde(
        PERMIT_SOURCE,
        "pub struct SigningPermit { signer: String }\nimpl std::fmt::Debug for SigningPermit { }",
    );
    assert!(defined && findings.is_empty(), "{findings:?}");
}

/// A violation inside a non-test inline module is still found.
#[test]
fn rules_see_into_non_test_inline_modules() {
    let route = "crates/openlore-review-app/src/routes/review.rs";
    assert!(!classify_publish_plan_needs_permit(
        route,
        "mod inner { fn approve(d: &str, x: &D) { publish_plan(d, x, \"t\"); } }"
    )
    .is_empty());
    assert!(
        !classify_authority_evidence(route, &format!("mod inner {{ {FABRICATED} }}")).is_empty()
    );
}

/// The whole check-arch passes on the real tree (exit code 0).
#[test]
fn check_arch_passes_on_the_real_tree() {
    assert_eq!(run().expect("check-arch runs"), 0);
}
