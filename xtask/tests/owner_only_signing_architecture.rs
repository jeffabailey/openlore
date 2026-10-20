//! owner-only-signing step 01-01 (crafter-owned): the person-tier authority
//! evidence is held by `authority_evidence_only_from_adapters` — a CLI source
//! that builds it is a finding; the adapters and test support may build it;
//! the real tree passes. Step 02-01: `repo_authority_checks_insert_only` —
//! any production SQL that updates or deletes a repo observation is a finding;
//! inserts and reads are not; the real tree passes. Step 02-02: the repo
//! observation (`FoundRepo`, `RepoObservation::Found`) is held by the same
//! evidence rule. Step 02-05: `sign_only_with_permit` — an `IdentityPort`
//! sign outside `sign_permitted` is a finding (philosophy records allowlisted);
//! `t1_entry_only_from_sign_batch` — the transitional T1 entry called from any
//! verb but `claim_add` and `sign_batch` is a finding; the real tree passes
//! both.

use std::path::{Path, PathBuf};

use xtask::check_arch::{
    classify_authority_evidence, classify_repo_authority_checks_literal,
    classify_sign_only_with_permit, classify_t1_entry_callers,
    scan_authority_evidence_only_from_adapters, scan_repo_authority_checks_insert_only,
    scan_sign_only_with_permit, scan_t1_entry_only_from_sign_batch,
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

const FABRICATED_REPO_OBSERVATION: &str = "
fn remembered(row: &Row) -> RepoObservation {
    let repo = ports::FoundRepo {
        repo_id: row.repo_id,
        full_name_seen: row.full_name.clone(),
        owner_id: row.owner_id,
        owner_type: RepoOwnerKind::User,
        authority_file: AuthorityFile::Present(vec![row.did.clone()]),
    };
    ports::RepoObservation::Found(repo)
}";

#[test]
fn a_repo_observation_built_in_cli_code_is_a_finding_but_adapters_may_build_it() {
    let findings = classify_authority_evidence(
        "crates/cli/src/verbs/authority_verify.rs",
        FABRICATED_REPO_OBSERVATION,
    );
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(findings[0].contains("FoundRepo"), "{findings:?}");
    assert!(
        findings[1].contains("RepoObservation::Found"),
        "{findings:?}"
    );

    for allowed in [
        "crates/adapter-github/src/client.rs",
        "crates/adapter-duckdb/src/repo_authority_checks.rs",
        "crates/test-support/src/lib.rs",
    ] {
        assert!(
            classify_authority_evidence(allowed, FABRICATED_REPO_OBSERVATION).is_empty(),
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

const SIGNS_DIRECTLY: &str = "
fn sign_and_keep(wiring: &Wiring, cid: &Cid) -> Result<SignatureBlock> {
    let block = wiring.identity.sign(cid)?;
    let again = IdentityPort::sign(wiring.identity.as_ref(), cid)?;
    Ok(block)
}
#[cfg(test)]
mod tests {
    fn signs_in_a_test(identity: &FakeIdentity, cid: &Cid) { identity.sign(cid).unwrap(); }
}";

const SIGNS_THROUGH_THE_PERMIT: &str = "
pub(crate) fn sign_permitted(identity: &dyn IdentityPort, permit: &SigningPermit) {
    identity.sign(&cid);
}
fn helper(identity: &dyn IdentityPort) { identity.sign(&cid); }";

#[test]
fn a_sign_outside_sign_permitted_is_a_finding_but_philosophy_records_may_sign() {
    let findings =
        classify_sign_only_with_permit("crates/cli/src/verbs/claim_counter.rs", SIGNS_DIRECTLY);
    assert_eq!(findings.len(), 2, "both production signs: {findings:?}");
    assert!(
        findings.iter().all(|f| f.contains("sign_only_with_permit")
            && f.contains("crates/cli/src/verbs/claim_counter.rs")),
        "{findings:?}"
    );

    assert!(classify_sign_only_with_permit(
        "crates/cli/src/verbs/philosophy_add.rs",
        SIGNS_DIRECTLY
    )
    .is_empty());

    let in_gate = classify_sign_only_with_permit(
        "crates/cli/src/verbs/sign_permitted.rs",
        SIGNS_THROUGH_THE_PERMIT,
    );
    assert_eq!(
        in_gate.len(),
        1,
        "only the helper outside the gate: {in_gate:?}"
    );
    assert!(
        classify_sign_only_with_permit(
            "crates/cli/src/verbs/claim_add.rs",
            SIGNS_THROUGH_THE_PERMIT
        )
        .len()
            == 2,
        "a `sign_permitted` outside its module is no gate"
    );
}

#[test]
fn the_real_tree_signs_only_through_sign_permitted() {
    let findings = scan_sign_only_with_permit(&root()).expect("scan");
    assert!(findings.is_empty(), "{findings:#?}");
}

const CALLS_THE_T1_ENTRY: &str = "
fn sign_repo_claim(signer: &str, claim: ClaimShape<'_>) -> Option<SigningPermit> {
    claim_domain::authority::permit_ungated_repo_t1(signer, claim)
}
fn later() { let entry = permit_ungated_repo_t1; }";

#[test]
fn the_t1_entry_called_from_claim_counter_is_a_finding_but_claim_add_and_sign_batch_may_call_it() {
    let findings =
        classify_t1_entry_callers("crates/cli/src/verbs/claim_counter.rs", CALLS_THE_T1_ENTRY);
    assert_eq!(
        findings.len(),
        2,
        "the call and the reference: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .all(|f| f.contains("t1_entry_only_from_sign_batch") && f.contains("claim_counter.rs")),
        "{findings:?}"
    );

    for allowed in [
        "crates/cli/src/verbs/claim_add.rs",
        "crates/cli/src/verbs/sign_batch.rs",
    ] {
        assert!(
            classify_t1_entry_callers(allowed, CALLS_THE_T1_ENTRY).is_empty(),
            "{allowed}"
        );
    }
}

#[test]
fn the_real_tree_calls_the_t1_entry_only_from_claim_add_and_sign_batch() {
    let findings = scan_t1_entry_only_from_sign_batch(&root()).expect("scan");
    assert!(findings.is_empty(), "{findings:#?}");
}
