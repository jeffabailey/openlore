//! owner-only-signing step 01-01 (crafter-owned): the person-tier authority
//! evidence is held by `authority_evidence_only_from_adapters` — a CLI source
//! that builds it is a finding; the adapters and test support may build it;
//! the real tree passes.

use std::path::{Path, PathBuf};

use xtask::check_arch::{classify_authority_evidence, scan_authority_evidence_only_from_adapters};

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
