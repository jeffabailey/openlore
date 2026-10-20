//! owner-only-signing step 01-01 (crafter-owned): the person-tier authority
//! evidence is held by `authority_evidence_only_from_adapters` — a CLI source
//! that builds it is a finding; the adapters and test support may build it;
//! the real tree passes. Step 02-01: `repo_authority_checks_insert_only` —
//! any production SQL that updates or deletes a repo observation is a finding;
//! inserts and reads are not; the real tree passes.

use std::path::{Path, PathBuf};

use xtask::check_arch::{
    classify_authority_evidence, classify_repo_authority_checks_literal,
    scan_authority_evidence_only_from_adapters, scan_repo_authority_checks_insert_only,
};

const FABRICATED_PERSON_EVIDENCE: &str = "
fn stored(row: &Row) -> LinkedAccountEvidence {
    LinkedAccountEvidence {
        github_id: row.id,
        linked_login: row.login.clone(),
        latest_check: Some(claim_domain::authority::LatestLinkCheck {
            login_seen: row.login.clone(),
            verdict: CheckVerdict::Verified,
        }),
    }
}";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn person_evidence_built_in_cli_code_is_a_finding_but_adapters_may_build_it() {
    let findings = classify_authority_evidence(
        "crates/cli/src/verbs/claim_add.rs",
        FABRICATED_PERSON_EVIDENCE,
    );
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(
        findings[0].contains("LinkedAccountEvidence"),
        "{findings:?}"
    );
    assert!(findings[1].contains("LatestLinkCheck"), "{findings:?}");

    for allowed in [
        "crates/adapter-duckdb/src/identity_link_checks.rs",
        "crates/test-support/src/lib.rs",
    ] {
        assert!(
            classify_authority_evidence(allowed, FABRICATED_PERSON_EVIDENCE).is_empty(),
            "{allowed}"
        );
    }
}

#[test]
fn the_real_tree_builds_person_evidence_only_in_adapters() {
    let findings = scan_authority_evidence_only_from_adapters(&root()).expect("scan");
    assert!(findings.is_empty(), "{findings:#?}");
}

#[test]
fn rewriting_a_repo_observation_is_a_finding_but_inserting_and_reading_are_not() {
    for synthetic in [
        "UPDATE repo_authority_checks SET outcome = 'found'",
        "DELETE FROM repo_authority_checks WHERE subject_key = ?",
        "truncate repo_authority_checks",
        "INSERT INTO repo_authority_checks (subject_key) VALUES (?) \
         ON CONFLICT (subject_key) DO UPDATE SET outcome = excluded.outcome",
    ] {
        assert!(
            classify_repo_authority_checks_literal(synthetic).is_some(),
            "must fire on: {synthetic}"
        );
    }
    for allowed in [
        "INSERT INTO repo_authority_checks (subject_key, outcome) VALUES (?, ?)",
        "SELECT outcome FROM repo_authority_checks WHERE subject_key = ?",
        "DELETE FROM identity_link_checks WHERE subject_did = ?",
    ] {
        assert_eq!(
            classify_repo_authority_checks_literal(allowed),
            None,
            "must allow: {allowed}"
        );
    }
}

#[test]
fn a_production_update_or_delete_of_repo_observations_is_reported_anywhere_and_the_real_tree_passes(
) {
    let violating = tempfile::tempdir().expect("tempdir");
    let adapter = violating.path().join("crates/adapter-duckdb/src");
    let verbs = violating.path().join("crates/cli/src/verbs");
    std::fs::create_dir_all(&adapter).expect("mkdir");
    std::fs::create_dir_all(&verbs).expect("mkdir");
    std::fs::write(
        adapter.join("repo_authority_checks.rs"),
        "const FORGET: &str = \"DELETE FROM repo_authority_checks WHERE subject_key = ?\";\n",
    )
    .expect("write");
    // Split across literals: the table name in one, the rewrite in another.
    std::fs::write(
        verbs.join("authority_verify.rs"),
        "const TABLE: &str = \"repo_authority_checks\";\n\
         const REFRESH: &str = \"UPDATE {} SET checked_at = now()\";\n",
    )
    .expect("write");
    std::fs::write(
        verbs.join("other.rs"),
        "#[cfg(test)]\nmod tests {\n    const RESET: &str = \"DELETE FROM repo_authority_checks\";\n}\n",
    )
    .expect("write");

    let findings = scan_repo_authority_checks_insert_only(violating.path()).expect("scan");
    assert_eq!(findings.len(), 2, "both production rewrites: {findings:?}");
    assert!(
        findings
            .iter()
            .any(|f| f.contains("repo_authority_checks.rs") && f.contains("DELETE")),
        "{findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.contains("authority_verify.rs") && f.contains("UPDATE")),
        "{findings:?}"
    );

    assert_eq!(
        scan_repo_authority_checks_insert_only(&root()).expect("scan real tree"),
        Vec::<String>::new(),
        "the real tree only inserts and reads repo_authority_checks"
    );
}
