//! owner-only-signing step 01-01 (crafter-owned): the person-tier authority
//! evidence is held by `authority_evidence_only_from_adapters` — a CLI source
//! that builds it is a finding; the adapters and test support may build it;
//! the real tree passes. Step 02-01: `repo_authority_checks_insert_only` —
//! any production SQL that updates or deletes a repo observation is a finding;
//! inserts and reads are not; the real tree passes. Step 02-02: the repo
//! observation (`FoundRepo`, `RepoObservation::Found`) is held by the same
//! evidence rule. Step 02-05: `sign_only_with_permit` — an `IdentityPort`
//! sign outside `sign_permitted` is a finding (philosophy records allowlisted);
//! the real tree passes. Step 02-09 deleted the transitional T1 entry and its
//! rule. Step 02-11: the same rule holds `publish_signed_claim` to a
//! `&SigningPermit` parameter. Step 03-05 (C-OOS-8): the indexer reaching
//! `adapter-github` is a capability-boundary violation. Step 05-01 (C-OOS-7):
//! `imported_candidates_not_in_scoring_or_inference` — scoring reading the
//! kept candidates, inference naming their port, rewriting them, or
//! `candidates list` holding GitHub is a finding; the real tree passes.
//! Refactor (U-OOS-D9): the decoded repo evidence (`LatestRepoObservation`,
//! `RepoFinding`, `AuthorityFileSeen`, `ServedRepoObservation`) is held by
//! the same evidence rule; matching it in a pattern builds nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use xtask::check_arch::{
    check_indexer_capability_boundary, classify_authority_evidence,
    classify_repo_authority_checks_literal, classify_sign_only_with_permit,
    scan_authority_evidence_only_from_adapters,
    scan_imported_candidates_not_in_scoring_or_inference, scan_repo_authority_checks_insert_only,
    scan_sign_only_with_permit,
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

const FABRICATED_REPO_EVIDENCE: &str = "
fn decoded(row: &Row) -> ServedRepoObservation {
    let finding = if row.found {
        RepoFinding::Found {
            owner_id: row.owner_id,
            owner_kind: RepoOwnerKind::User,
            authority_file: AuthorityFileSeen::Present(vec![row.did.clone()]),
        }
    } else {
        claim_domain::authority::RepoFinding::NotFound
    };
    let _unlisted = AuthorityFileSeen::Absent;
    ServedRepoObservation {
        latest: LatestRepoObservation {
            finding,
            checked_at: row.checked_at,
        },
        served_as: None,
    }
}

fn judged(latest: &LatestRepoObservation) -> bool {
    match &latest.finding {
        RepoFinding::Found { authority_file: AuthorityFileSeen::Absent, .. } => true,
        RepoFinding::NotFound => false,
    }
}";

#[test]
fn decoded_repo_evidence_built_in_cli_code_is_a_finding_but_matching_it_is_not() {
    let findings = classify_authority_evidence(
        "crates/cli/src/verbs/authority_verify.rs",
        FABRICATED_REPO_EVIDENCE,
    );
    for built in [
        "`RepoFinding::Found { .. }`",
        "`AuthorityFileSeen::Present(..)`",
        "`RepoFinding::NotFound`",
        "`AuthorityFileSeen::Absent`",
        "`ServedRepoObservation { .. }`",
        "`LatestRepoObservation { .. }`",
    ] {
        assert!(
            findings.iter().any(|finding| finding.contains(built)),
            "{built} must be a finding: {findings:#?}"
        );
    }
    assert_eq!(
        findings.len(),
        6,
        "the patterns in `judged` build nothing: {findings:#?}"
    );

    for allowed in [
        "crates/adapter-duckdb/src/repo_authority_checks.rs",
        "crates/test-support/src/lib.rs",
    ] {
        assert!(
            classify_authority_evidence(allowed, FABRICATED_REPO_EVIDENCE).is_empty(),
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

const PUBLISHES_UNDER_A_PERMIT: &str = "
pub fn publish_signed_claim(
    wiring: &Wiring,
    permit: &claim_domain::authority::SigningPermit,
    signed: &SignedClaim,
) {}";

const PUBLISHES_WITHOUT_A_PERMIT: &str = "
pub fn publish_signed_claim(wiring: &Wiring, signed: &SignedClaim) {}
fn helper(permit: &SigningPermit) {}";

#[test]
fn the_single_publish_path_without_a_permit_parameter_is_a_finding() {
    let publish = "crates/cli/src/verbs/claim_publish.rs";
    assert!(
        classify_sign_only_with_permit(publish, PUBLISHES_UNDER_A_PERMIT).is_empty(),
        "publishing under `&SigningPermit` passes"
    );
    for (source, why) in [
        (PUBLISHES_WITHOUT_A_PERMIT, "no permit parameter"),
        ("fn helper() {}", "no publish path"),
    ] {
        let findings = classify_sign_only_with_permit(publish, source);
        assert_eq!(findings.len(), 1, "{why}: {findings:?}");
        assert!(
            findings[0].contains("sign_only_with_permit")
                && findings[0].contains("publish_signed_claim"),
            "{why}: {findings:?}"
        );
    }
    assert!(
        classify_sign_only_with_permit(
            "crates/cli/src/verbs/claim_add.rs",
            PUBLISHES_WITHOUT_A_PERMIT
        )
        .is_empty(),
        "only the publish module is held to it"
    );
}

#[test]
fn the_real_tree_signs_only_through_sign_permitted() {
    let findings = scan_sign_only_with_permit(&root()).expect("scan");
    assert!(findings.is_empty(), "{findings:#?}");
}

/// A workspace where the indexer reaches GitHub only through `via`.
fn indexer_reaching_github_via(via: &[&str]) -> xtask::check_arch::Workspace {
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut chain = vec!["openlore-indexer"];
    chain.extend_from_slice(via);
    chain.push("adapter-github");
    for pair in chain.windows(2) {
        deps.entry(pair[0].to_string())
            .or_default()
            .insert(pair[1].to_string());
    }
    deps.entry("adapter-github".to_string()).or_default();
    xtask::check_arch::Workspace {
        members: deps.keys().cloned().collect(),
        deps,
    }
}

#[test]
fn the_indexer_reaching_github_directly_or_transitively_is_a_violation() {
    for via in [&[][..], &["appview-domain"][..]] {
        let findings = check_indexer_capability_boundary(&indexer_reaching_github_via(via));
        assert!(
            findings
                .iter()
                .any(|v| v.package == "openlore-indexer" && v.forbidden == "adapter-github"),
            "openlore-indexer -> {via:?} -> adapter-github is a violation (C-OOS-8): {findings:?}"
        );
    }
}

fn write_source(root: &Path, rel_path: &str, source: &str) {
    let path = root.join(rel_path);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, source).expect("write");
}

#[test]
fn scoring_or_inference_reading_imported_candidates_is_a_finding_and_the_real_tree_passes() {
    let violating = tempfile::tempdir().expect("tempdir");
    let tree = violating.path();
    // Synthetic violation: scoring reads the kept candidates.
    write_source(
        tree,
        "crates/scoring/src/feed.rs",
        "const FEED: &str = \"SELECT object FROM imported_candidates\";\n",
    );
    // Inference holding the read port.
    write_source(
        tree,
        "crates/cli/src/verbs/infer_people.rs",
        "fn people(kept: &dyn ImportedCandidateReadPort) {}\n",
    );
    // The adapter may read and insert, never rewrite.
    write_source(
        tree,
        "crates/adapter-duckdb/src/imported_candidates.rs",
        "const LIST: &str = \"SELECT object FROM imported_candidates\";\n\
         const KEEP: &str = \"INSERT INTO imported_candidates (object) VALUES (?)\";\n\
         const FORGET: &str = \"DELETE FROM imported_candidates WHERE import_id = ?\";\n",
    );
    // A probe label naming the table is no SQL.
    write_source(
        tree,
        "crates/cli/src/wiring.rs",
        "fn gauntlet() { check_probe(\"imported_candidates\", probe()); }\n",
    );
    // The offline list asking GitHub.
    write_source(
        tree,
        "crates/cli/src/verbs/candidates.rs",
        "fn run(github: &dyn GithubPort) {}\n",
    );

    let findings = scan_imported_candidates_not_in_scoring_or_inference(tree).expect("scan");
    let reported = |file: &str, needle: &str| {
        findings
            .iter()
            .any(|f| f.contains(file) && f.contains(needle))
    };
    assert!(
        reported(
            "scoring/src/feed.rs",
            "SELECT object FROM imported_candidates"
        ),
        "{findings:#?}"
    );
    assert!(
        reported("infer_people.rs", "ImportedCandidateReadPort"),
        "{findings:#?}"
    );
    assert!(
        reported("adapter-duckdb/src/imported_candidates.rs", "DELETE"),
        "{findings:#?}"
    );
    assert!(reported("candidates.rs", "GithubPort"), "{findings:#?}");
    assert!(
        !findings
            .iter()
            .any(|f| f.contains("wiring.rs")
                || (f.contains("adapter-duckdb") && !f.contains("DELETE"))),
        "the adapter's reads and inserts and a probe label are allowed: {findings:#?}"
    );

    assert_eq!(
        scan_imported_candidates_not_in_scoring_or_inference(&root()).expect("scan real tree"),
        Vec::<String>::new(),
        "imported candidates never reach scoring or inference in the real tree"
    );
}
