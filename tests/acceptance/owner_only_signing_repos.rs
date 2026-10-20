//! owner-only-signing — slice 2 acceptance (repo authority: the owner of a
//! user-owned repo, or a DID listed in `.github/openlore-authority`; ADR-087
//! §2-§6, ADR-086 §4-§6/§10, C-OOS-1/3/4/9, KPI-OOS-1/4) and slice 5
//! (imported candidates kept locally; OOS-D12 B, C-OOS-7).
//!
//! Drives the REAL `openlore` binary (`scrape github|person [--sign]`,
//! `authority verify`, `claim add/publish`, `candidates list`, `graph query`).
//! Real: the `authority` gate, `adapter-github` over HTTP (incl. the NEW
//! same-host ≤ 1 redirect policy), DuckDB v8 `repo_authority_checks` /
//! `imported_candidates`. Fake (driven-external only): `FakeGithubAccounts`
//! with the ADR-087 lie catalogue (`AuthorityFile`, transfer, rename,
//! cross-host redirect, 403, outage), `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! Layer 3 subprocess — example-only (Mandate 9); sad paths are named rows
//! (Mandate 11). Every scenario is `#[ignore]`d pending DELIVER.
//!
//! State machine (C2a) of the latest local observation of a repo subject
//! (insert-only; latest = max `checked_at`, then max `check_id`):
//!
//! | From | Event | To |
//! |---|---|---|
//! | none | `scrape github <repo>` / `authority verify` / `--sign` read, GitHub answers | found {owner, owner type, file absent / present(dids) / invalid} |
//! | none / any | GitHub answers 404 | repo_not_found (file not read) |
//! | any | rate limit, outage, shape error, cross-host redirect | unchanged (nothing recorded) |
//! | found | transfer / rename / file edited, then re-observed | found with the new facts (older rows kept) |
//!
//! Signing permit (C2b): Attested (Owner: user-owned ∧ owner id is the
//! signer's verified linked id; RepoListed: the file lists the signer's DID
//! as an exact token) ∧ the older of the checks used ≤ 7 days old ∧ none in
//! the future. Everything else is refused with the hand-off.

mod support;

#[allow(unused_imports)]
use support::authority::*;
#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

// =============================================================================
// Owner tier and the import hand-off (scrape --sign)
// =============================================================================

/// RA-1
/// ```gherkin
/// @driving_port @driving_adapter @real-io @kpi-oos-1 @kpi-oos-4 @adr-087-2 @oos-d5 @contract-shape:bounded-change
/// Scenario: Jeff imports his own repo and signs what it shows
///   Given Jeff linked "jeffbailey", the account that owns jeffbailey/tidepool
///   When Jeff imports jeffbailey/tidepool and signs candidate 1
///   Then the claim is signed and reaches his published repo
///   And the repo's authority was observed once, as owned by a user with no authority file
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-1: pending DELIVER"]
fn jeff_imports_his_own_repo_and_signs_what_it_shows() {
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    let checks_before = repo_checks(&world.env);

    let signed = when_jeff_imports_and_signs(&world, TIDEPOOL, CHECK_PLUS_2_DAYS);

    assert_signed_and_published(&world, &signed, Subject::Repo(TIDEPOOL));
    assert_eq!(
        format!("{:?}", repo_checks(&world.env)),
        repo_checks_with(
            &checks_before,
            &[RepoCheckRow::found(TIDEPOOL, false, "absent", &[])]
        ),
        "one observation of tidepool, signer-independent"
    );
    assert_eq!(
        world.github.authority_file_reads_of(&TIDEPOOL.full()),
        1,
        "the authority file is always read (ADR-087 §4)"
    );
}

/// RA-2 (table over what Jeff imports and tries to sign; Jeff is linked)
/// ```gherkin
/// @driving_port @real-io @error @security @kpi-oos-1 @adr-086-10 @oos-d12 @c-oos-7 @contract-shape:bounded-change
/// Scenario Outline: Jeff imports anything public, but signing what he cannot attest is refused with a hand-off
///   Given Jeff linked "jeffbailey"
///   When Jeff runs <import> and asks to sign candidate 1
///   Then the candidates are still listed and nothing is signed or published
///   And he is told "<reason>", who can sign it, and the command they would run
///   Examples:
///     | import                              | reason                                         | who can sign                                     |
///     | scrape github acme-tools/anvil      | is owned by the organization                   | a DID listed in .github/openlore-authority       |
///     | scrape github rachelk/kelp          | is not owned by your linked GitHub account     | rachelk after linking, or a listed DID           |
///     | scrape person rachelk               | you have no identity link to                   | rachelk after linking                            |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-2: pending DELIVER"]
fn jeff_imports_anything_public_but_signing_what_he_cannot_attest_is_refused_with_a_hand_off() {
    let rows: [(&str, Subject, Refusal, CanSign, Option<String>); 3] = [
        (
            "org repo",
            Subject::Repo(ANVIL),
            Refusal::OrgOwnedNotListed,
            CanSign::ListedDid,
            Some(scrape_handoff_command(ANVIL)),
        ),
        (
            "someone's repo",
            Subject::Repo(KELP),
            Refusal::NotRepoOwner,
            CanSign::LoginOrListedDid(RACHEL_GH.login),
            Some(scrape_handoff_command(KELP)),
        ),
        (
            "someone",
            Subject::Person(RACHEL_GH),
            Refusal::NoLink,
            CanSign::Login(RACHEL_GH.login),
            None,
        ),
    ];
    for (row, subject, reason, who, command) in rows {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        let before = capture_authority_universe(&world.env);

        let outcome = match subject {
            Subject::Repo(repo) => when_jeff_imports_and_signs(&world, repo, CHECK_PLUS_2_DAYS),
            Subject::Person(account) => {
                when_jeff_scrapes_person_and_signs(&world, account, CHECK_PLUS_2_DAYS)
            }
        };

        assert!(
            numbered_count(&outcome.stdout) >= 1,
            "[{row}] the candidates are still listed;\n{}",
            outcome.stdout
        );
        assert_signing_refused(&outcome, subject, reason, Some(who), command.as_deref(), row);
        assert_nothing_signed(row, &before, &world.env);
    }
}

/// RA-3 (table over the authority file on acme-tools/anvil; Jeff has NO
/// identity link — the listed tier needs none, ADR-087 §3)
/// ```gherkin
/// @driving_port @real-io @security @c6a @adr-087-3 @kpi-oos-4 @contract-shape:bounded-change
/// Scenario Outline: An organization repo is signed only by a DID its authority file lists exactly
///   Given acme-tools/anvil is owned by the organization acme-tools
///   And its default branch holds <file>
///   When Jeff imports acme-tools/anvil and signs candidate 1
///   Then <outcome>
///   Examples:
///     | file                                                           | outcome                                          |
///     | an authority file listing Jeff's identity                      | signed and published                             |
///     | an authority file listing Jeff's identity in capitals          | refused: is owned by the organization            |
///     | an authority file listing "did:plc:test-jeffrey"               | refused: is owned by the organization            |
///     | an authority file where Jeff's identity is commented out       | refused: is owned by the organization            |
///     | a symlink named like the authority file                        | refused: is not a valid authority file           |
///     | a submodule named like the authority file                      | refused: is not a valid authority file           |
///     | a directory named like the authority file                      | refused: is not a valid authority file           |
///     | an authority file of exactly 4 KiB that lists Jeff's identity  | signed and published                             |
///     | an authority file over 4 KiB that lists Jeff's identity        | refused: is not a valid authority file           |
///     | an authority file listing Jeff's identity that is not plain text | refused: is not a valid authority file           |
///     | Jeff's identity listed only on another branch                  | refused: is owned by the organization            |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-3: pending DELIVER"]
fn an_organization_repo_is_signed_only_by_a_did_its_authority_file_lists_exactly() {
    let mine = format!("{MY_DID}\n");
    let rows: Vec<(&str, AuthorityFile, Option<Refusal>)> = vec![
        ("listed", listing(&[MY_DID]), None),
        (
            "uppercase",
            listing(&[&MY_DID.to_ascii_uppercase()]),
            Some(Refusal::OrgOwnedNotListed),
        ),
        (
            "prefix collider",
            listing(&[MY_DID_PREFIX_COLLIDER]),
            Some(Refusal::OrgOwnedNotListed),
        ),
        (
            "commented out",
            AuthorityFile::Lists(format!("# {MY_DID}\n")),
            Some(Refusal::OrgOwnedNotListed),
        ),
        ("symlink", AuthorityFile::Symlink, Some(Refusal::FileInvalid)),
        ("submodule", AuthorityFile::Submodule, Some(Refusal::FileInvalid)),
        ("directory", AuthorityFile::Directory, Some(Refusal::FileInvalid)),
        ("exactly 4 KiB", listing_of_exactly(4 * 1024, &[MY_DID]), None),
        ("oversize", AuthorityFile::Oversize(mine.clone()), Some(Refusal::FileInvalid)),
        ("not utf-8", AuthorityFile::NotUtf8(mine.clone()), Some(Refusal::FileInvalid)),
        (
            "other branch",
            AuthorityFile::OnlyOnAnotherBranch(mine),
            Some(Refusal::OrgOwnedNotListed),
        ),
    ];
    let anvil = Subject::Repo(ANVIL);
    for (row, file, refusal) in rows {
        let world = authority_world();
        the_repo_holds_authority_file(&world, ANVIL, file);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_imports_and_signs(&world, ANVIL, CHECK_PLUS_2_DAYS);

        match refusal {
            None => assert_signed_and_published(&world, &outcome, anvil),
            Some(reason) => {
                assert_signing_refused(&outcome, anvil, reason, Some(CanSign::ListedDid), None, row);
                assert_nothing_signed(row, &before, &world.env);
            }
        }
    }
}

// =============================================================================
// Re-checking authority (authority verify)
// =============================================================================

/// RA-4
/// ```gherkin
/// @driving_port @real-io @adr-087-6 @c-oos-3 @c-oos-4 @kpi-oos-4 @contract-shape:bounded-change
/// Scenario: Re-checking authority labels each peer's repo claims and records one observation per repo
///   Given acme-tools/anvil's authority file lists Rachel's identity
///   And Rachel published claims about acme-tools/anvil and jeffbailey/tidepool, and Jeff pulled both
///   When Jeff re-checks authority
///   Then her anvil claim reads "repo-listed" and her tidepool claim "third-party"
///   And exactly one observation of each repo was recorded, from one repo read and one authority-file read each
///   And nothing else in his store or published repo changed
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-4: pending DELIVER"]
fn rechecking_authority_labels_each_peers_repo_claims_and_records_one_observation_per_repo() {
    let mut world = authority_world();
    the_repo_holds_authority_file(&world, ANVIL, listing(&[RACHEL_DID]));
    given_rachel_published(
        &mut world,
        vec![
            peer_claim_about(Subject::Repo(ANVIL)),
            peer_claim_about(Subject::Repo(TIDEPOOL)),
        ],
    );
    let before = capture_authority_universe(&world.env);
    let checks_before = repo_checks(&world.env);

    let verified = when_jeff_verifies_authority(&world, &[], Some(CHECK_PLUS_2_DAYS));

    assert_eq!(verified.status, 0, "{}\n{}", verified.stdout, verified.stderr);
    assert_labelled(&verified, RACHEL_DID, Subject::Repo(ANVIL), Authority::RepoListed);
    assert_labelled(&verified, RACHEL_DID, Subject::Repo(TIDEPOOL), Authority::ThirdParty);
    for repo in [ANVIL, TIDEPOOL] {
        assert_eq!(world.github.repo_facts_reads_of(&repo.full()), 1, "{}", repo.full());
        assert_eq!(world.github.authority_file_reads_of(&repo.full()), 1, "{}", repo.full());
    }
    assert_authority_store_delta(
        &before,
        &world.env,
        &[(
            "local.repo_authority_checks",
            repo_checks_with(
                &checks_before,
                &[
                    RepoCheckRow::found(ANVIL, true, "present", &[RACHEL_DID]),
                    RepoCheckRow::found(TIDEPOOL, false, "absent", &[]),
                ],
            ),
        )],
    );
}

/// RA-5 (table over how GitHub fails; chained on RA-4's Given)
/// ```gherkin
/// @driving_port @real-io @infrastructure-failure @security @c7a @adr-087-4 @contract-shape:unbounded-preservation
/// Scenario Outline: When GitHub cannot answer, re-checking records nothing and changes nothing
///   Given Rachel published a claim about acme-tools/anvil and Jeff pulled it
///   And <trouble>
///   When Jeff re-checks the authority of acme-tools/anvil
///   Then he is told GitHub could not be asked
///   And nothing in his store or published repo changed
///   Examples:
///     | trouble                                                       |
///     | GitHub says it is too busy to answer                          |
///     | GitHub cannot be reached                                      |
///     | GitHub answers the repo read by sending him to another site   |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-5: pending DELIVER"]
fn when_github_cannot_answer_rechecking_records_nothing_and_changes_nothing() {
    let rows: [(&str, fn(&LinkWorld)); 3] = [
        ("rate limited", |w| github_has(w, GithubTrouble::RateLimitedAnonymously)),
        ("unreachable", |w| github_has(w, GithubTrouble::Unreachable)),
        ("another host", |w| {
            w.github.redirect_repo_to_another_host(ANVIL.owner, ANVIL.name)
        }),
    ];
    let anvil = Subject::Repo(ANVIL);
    for (row, trouble) in rows {
        let mut world = authority_world();
        given_rachel_published(&mut world, vec![peer_claim_about(anvil)]);
        trouble(&world);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_verifies_authority(
            &world,
            &["--subject", &anvil.uri()],
            Some(CHECK_PLUS_2_DAYS),
        );

        assert_refused(&outcome, anvil, Refusal::GithubUnfinished, None, None, row);
        assert_authority_store_unchanged(row, &before, &world.env);
    }
}

/// RA-6
/// ```gherkin
/// @driving_port @real-io @infrastructure-failure @c7a @adr-087-6 @contract-shape:bounded-change
/// Scenario: When GitHub's budget runs out mid-import, the candidates are listed and signing waits for the reset
///   Given Jeff linked "jeffbailey"
///   And GitHub's budget runs out after the harvest of jeffbailey/tidepool, before its authority is read
///   When Jeff imports jeffbailey/tidepool and asks to sign candidate 1
///   Then the candidates are listed
///   And nothing is signed or published, and no observation of tidepool is recorded
///   And he is told GitHub could not be asked and when to retry
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-6: pending DELIVER"]
fn when_githubs_budget_runs_out_mid_import_the_candidates_are_listed_and_signing_waits_for_the_reset() {
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    world.github.exhaust_budget_at_authority_reads(true);
    let before = capture_authority_universe(&world.env);

    let outcome = when_jeff_imports_and_signs(&world, TIDEPOOL, CHECK_PLUS_2_DAYS);

    assert!(numbered_count(&outcome.stdout) >= 1, "candidates listed;\n{}", outcome.stdout);
    assert_signing_refused(
        &outcome,
        Subject::Repo(TIDEPOOL),
        Refusal::GithubUnfinished,
        None,
        Some("retry after"),
        "budget",
    );
    assert_nothing_signed("budget", &before, &world.env);
    assert_eq!(
        repo_checks(&world.env),
        Vec::<RepoCheckRow>::new(),
        "an unfinished read records nothing"
    );
}

/// RA-7 (chained on RA-1)
/// ```gherkin
/// @driving_port @real-io @adr-086-6 @oos-d6 @c-oos-5 @contract-shape:bounded-change
/// Scenario: After Jeff transfers his repo, his earlier claim reads third-party and nothing is withdrawn
///   Given Jeff signed and published a claim about jeffbailey/tidepool while he owned it
///   And Jeff then transferred tidepool to the organization acme-tools
///   When Jeff re-checks authority
///   Then his claim reads "third-party"
///   And his claim is not retracted, deleted or re-signed, and nothing is published
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-7: pending DELIVER"]
fn after_jeff_transfers_his_repo_his_earlier_claim_reads_third_party_and_nothing_is_withdrawn() {
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    given_jeff_verified_authority_of(&world, Subject::Repo(TIDEPOOL), LINKED_AT);
    given_jeff_signed_and_published(&world, Subject::Repo(TIDEPOOL), CHECK_PLUS_2_DAYS);
    jeff_transfers_tidepool_to_acme(&world);
    let before = capture_authority_universe(&world.env);

    let verified = when_jeff_verifies_authority(&world, &[], Some(CHECK_PLUS_7_DAYS));

    assert_eq!(verified.status, 0, "{}\n{}", verified.stdout, verified.stderr);
    assert_labelled(&verified, MY_DID, Subject::Repo(TIDEPOOL), Authority::ThirdParty);
    assert_nothing_signed("transfer", &before, &world.env);
}

/// RA-8 (table over the old and the new repo name)
/// ```gherkin
/// @driving_port @real-io @edge @security @a-10 @adr-087-4 @adr-089-2 @contract-shape:bounded-change
/// Scenario Outline: After Jeff renames his repo, he signs only under the name GitHub serves now
///   Given Jeff linked "jeffbailey" and renamed jeffbailey/tidepool to jeffbailey/tidepool-rs
///   And Jeff re-checked the authority of <name>
///   When Jeff signs and publishes a claim about <name>
///   Then <outcome>
///   Examples:
///     | name                    | outcome                                                                 |
///     | jeffbailey/tidepool-rs  | the claim is signed and reaches his published repo                      |
///     | jeffbailey/tidepool     | nothing is signed; he is told GitHub serves this repo as jeffbailey/tidepool-rs |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-8: pending DELIVER"]
fn after_jeff_renames_his_repo_he_signs_only_under_the_name_github_serves_now() {
    let rows: [(RepoRef, Option<Refusal>); 2] = [
        (TIDEPOOL_RENAMED, None),
        (TIDEPOOL, Some(Refusal::Renamed)),
    ];
    for (repo, refusal) in rows {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        jeff_renames_tidepool(&world);
        let subject = Subject::Repo(repo);
        given_jeff_verified_authority_of(&world, subject, LINKED_AT);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs(&world, subject, Some(CHECK_PLUS_2_DAYS));

        match refusal {
            None => assert_signed_and_published(&world, &outcome, subject),
            Some(reason) => {
                assert_signing_refused(
                    &outcome,
                    subject,
                    reason,
                    None,
                    Some(&TIDEPOOL_RENAMED.full()),
                    "old name",
                );
                assert_authority_store_unchanged("old name", &before, &world.env);
            }
        }
    }
}

/// RA-9 (observation order, review H-2)
/// ```gherkin
/// @driving_port @real-io @security @h-2 @adr-087-4 @contract-shape:pure-function
/// Scenario: A listed identity stays listed whoever observed the repo last
///   Given acme-tools/anvil's authority file lists Rachel's identity, and Rachel's claim about it was pulled
///   And Jeff re-checked authority, then imported acme-tools/anvil himself
///   When Jeff re-checks authority again
///   Then Rachel's claim still reads "repo-listed"
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-9: pending DELIVER"]
fn a_listed_identity_stays_listed_whoever_observed_the_repo_last() {
    let mut world = authority_world();
    the_repo_holds_authority_file(&world, ANVIL, listing(&[RACHEL_DID]));
    given_rachel_published(&mut world, vec![peer_claim_about(Subject::Repo(ANVIL))]);
    given_jeff_verified_authority(&world, LINKED_AT);
    given_jeff_imported(&world, ANVIL, CHECK_PLUS_2_DAYS);

    let verified = when_jeff_verifies_authority(&world, &[], Some(CHECK_PLUS_7_DAYS));

    assert_eq!(verified.status, 0, "{}\n{}", verified.stdout, verified.stderr);
    assert_labelled(&verified, RACHEL_DID, Subject::Repo(ANVIL), Authority::RepoListed);
}

/// RA-14 (C4b: re-check with nothing to re-check)
/// ```gherkin
/// @driving_port @real-io @edge @c3 @c4b @adr-087-6 @contract-shape:unbounded-preservation
/// Scenario: With no GitHub claims in his store, re-checking authority asks GitHub nothing
///   Given Jeff's store holds no claim about a GitHub account or repo
///   When Jeff re-checks authority
///   Then he is told there is nothing to re-check
///   And GitHub is not asked anything, and nothing in his store changed
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-14: pending DELIVER"]
fn with_no_github_claims_in_his_store_rechecking_authority_asks_github_nothing() {
    let world = authority_world();
    let asked_before = world.github_paths().len();
    let before = capture_authority_universe(&world.env);

    let verified = when_jeff_verifies_authority(&world, &[], Some(LINKED_AT));

    assert_eq!(verified.status, 0, "{}\n{}", verified.stdout, verified.stderr);
    assert!(
        verified.stdout.contains(NOTHING_TO_RECHECK),
        "says `{NOTHING_TO_RECHECK}`;\n{}",
        verified.stdout
    );
    assert_github_not_asked(&world, asked_before, "nothing to re-check");
    assert_authority_store_unchanged("nothing to re-check", &before, &world.env);
}

/// RA-15 (C6b: the repo GitHub does not have)
/// ```gherkin
/// @driving_port @real-io @error @adr-087-4 @c6b @contract-shape:bounded-change
/// Scenario: A repo GitHub does not have is recorded as not found and cannot be signed
///   Given Jeff re-checked the authority of acme-tools/vanished, which GitHub does not have
///   And the network is off
///   When Jeff signs a claim about acme-tools/vanished
///   Then nothing is signed; he is told it was not found on GitHub
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-15: pending DELIVER"]
fn a_repo_github_does_not_have_is_recorded_as_not_found_and_cannot_be_signed() {
    let world = authority_world();
    let vanished = Subject::Repo(VANISHED);
    given_jeff_verified_authority_of(&world, vanished, LINKED_AT);
    let before = capture_authority_universe(&world.env);

    let outcome = when_jeff_signs_offline(&world, vanished, CHECK_PLUS_2_DAYS);

    assert_signing_refused(&outcome, vanished, Refusal::RepoNotFound, None, None, "vanished");
    assert_authority_store_unchanged("vanished", &before, &world.env);
}

// =============================================================================
// Signing a repo claim offline from local checks (I-9 exception)
// =============================================================================

/// RA-10 (table over the checks behind an offline repo signature)
/// ```gherkin
/// @driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @security @contract-shape:bounded-change
/// Scenario Outline: With the network off, Jeff signs a repo claim only from fresh checks
///   Given <checks>
///   And the network is off
///   When Jeff signs a claim about <repo> at <time>
///   Then <outcome>
///   And GitHub is not asked anything
///   Examples:
///     | checks                                                                                      | repo                  | time        | outcome                                         |
///     | Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October                   | jeffbailey/tidepool   | 3 October   | signed and kept locally                         |
///     | anvil's authority file lists Jeff; Jeff re-checked anvil on 1 October (no identity link)    | acme-tools/anvil      | 3 October   | signed and kept locally                         |
///     | Jeff's link was checked on 1 October; he imported tidepool on 3 October                     | jeffbailey/tidepool   | 8 Oct 09:01 | refused: the older check is over 7 days old     |
///     | Jeff linked "jeffbailey" but tidepool was never observed                                    | jeffbailey/tidepool   | 3 October   | refused: authority has not been checked         |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-10: pending DELIVER"]
fn with_the_network_off_jeff_signs_a_repo_claim_only_from_fresh_checks() {
    let rows: [(&str, fn(&LinkWorld), RepoRef, &str, Option<Refusal>); 4] = [
        (
            "owner, fresh",
            |w| {
                given_jeff_linked_his_account(w);
                given_jeff_verified_authority_of(w, Subject::Repo(TIDEPOOL), LINKED_AT);
            },
            TIDEPOOL,
            CHECK_PLUS_2_DAYS,
            None,
        ),
        (
            "listed, fresh",
            |w| {
                the_repo_holds_authority_file(w, ANVIL, listing(&[MY_DID]));
                given_jeff_verified_authority_of(w, Subject::Repo(ANVIL), LINKED_AT);
            },
            ANVIL,
            CHECK_PLUS_2_DAYS,
            None,
        ),
        (
            "older check stale",
            |w| {
                given_jeff_linked_his_account(w);
                given_jeff_imported(w, TIDEPOOL, CHECK_PLUS_2_DAYS);
            },
            TIDEPOOL,
            CHECK_PLUS_7_DAYS_1_MIN,
            Some(Refusal::CheckTooOld),
        ),
        (
            "never observed",
            |w| {
                given_jeff_linked_his_account(w);
            },
            TIDEPOOL,
            CHECK_PLUS_2_DAYS,
            Some(Refusal::NotChecked),
        ),
    ];
    for (row, checks, repo, now, refusal) in rows {
        let world = authority_world();
        checks(&world);
        let subject = Subject::Repo(repo);
        let asked_before = world.github_paths().len();
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs_offline(&world, subject, now);

        assert_github_not_asked(&world, asked_before, row);
        match refusal {
            None => {
                assert_eq!(outcome.status, 0, "[{row}] {}\n{}", outcome.stdout, outcome.stderr);
                assert_authority_store_delta(
                    &before,
                    &world.env,
                    &[
                        ("local.claims.row_count", claims_plus(&before, 1)),
                        ("local.claim_files", claim_file_slot(&world.env)),
                    ],
                );
            }
            Some(reason) => {
                assert_signing_refused(
                    &outcome,
                    subject,
                    reason,
                    None,
                    Some(&verify_command(subject)),
                    row,
                );
                assert_authority_store_unchanged(row, &before, &world.env);
            }
        }
    }
}

// =============================================================================
// Publishing a stored claim (claim publish <cid>)
// =============================================================================

/// RA-11a
/// ```gherkin
/// @driving_port @real-io @error @security @kpi-oos-1 @oos-d3 @adr-086-4 @contract-shape:unbounded-preservation
/// Scenario: A claim Jeff signed about someone else's repo before owner-only signing is never published
///   Given Jeff's store was made before owner-only signing and holds his unpublished claim about BurntSushi/ripgrep
///   When Jeff publishes that claim
///   Then nothing reaches his published repo
///   And he is told the authority of BurntSushi/ripgrep has not been checked, with the command to check it
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-11a: pending DELIVER"]
fn a_claim_jeff_signed_about_someone_elses_repo_before_owner_only_signing_is_never_published() {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH, RACHEL_GH]);
    let ripgrep = Subject::Repo(RIPGREP);
    let before = capture_authority_universe(&world.env);

    let outcome = when_jeff_publishes(&world, PRE_FEATURE_UNPUBLISHED_CID, CHECK_PLUS_2_DAYS);

    assert_signing_refused(
        &outcome,
        ripgrep,
        Refusal::NotChecked,
        None,
        Some(&verify_command(ripgrep)),
        "pre-feature",
    );
    assert_nothing_signed("pre-feature", &before, &world.env);
}

/// RA-11b (table over a stored claim's history; chained on RA-1)
/// ```gherkin
/// @driving_port @real-io @c4a @kpi-oos-1 @adr-086-4 @contract-shape:bounded-change
/// Scenario Outline: Publishing a stored claim later asks for authority only if it was never published
///   Given Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October
///   And <history>
///   When Jeff publishes that claim on 8 October
///   Then <outcome>
///   Examples:
///     | history                                                                        | outcome                                                |
///     | he signed a claim about tidepool without publishing it                         | it reaches his published repo                          |
///     | he signed it without publishing, then transferred tidepool and re-checked      | refused: is owned by the organization; nothing published |
///     | he signed and published it, then transferred tidepool and re-checked           | succeeds again, and his published repo is unchanged    |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-11b: pending DELIVER"]
fn publishing_a_stored_claim_later_asks_for_authority_only_if_it_was_never_published() {
    #[derive(Clone, Copy)]
    enum History {
        Unpublished,
        UnpublishedThenTransferred,
        PublishedThenTransferred,
    }
    let tidepool = Subject::Repo(TIDEPOOL);
    for history in [
        History::Unpublished,
        History::UnpublishedThenTransferred,
        History::PublishedThenTransferred,
    ] {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        given_jeff_verified_authority_of(&world, tidepool, LINKED_AT);
        let cid = match history {
            History::PublishedThenTransferred => {
                given_jeff_signed_and_published(&world, tidepool, CHECK_PLUS_2_DAYS)
            }
            _ => given_jeff_signed_unpublished(&world, tidepool, CHECK_PLUS_2_DAYS),
        };
        if !matches!(history, History::Unpublished) {
            jeff_transfers_tidepool_to_acme(&world);
            given_jeff_verified_authority_of(&world, tidepool, CHECK_PLUS_2_DAYS);
        }
        let pds_before = world.env.pds.records().len();

        let outcome = when_jeff_publishes(&world, &cid, CHECK_PLUS_7_DAYS);

        match history {
            History::Unpublished => {
                assert_eq!(outcome.status, 0, "{}\n{}", outcome.stdout, outcome.stderr);
                assert_eq!(world.env.pds.records().len(), pds_before + 1, "published");
            }
            History::UnpublishedThenTransferred => {
                assert_signing_refused(
                    &outcome,
                    tidepool,
                    Refusal::OrgOwnedNotListed,
                    Some(CanSign::ListedDid),
                    None,
                    "transferred",
                );
                assert_eq!(world.env.pds.records().len(), pds_before, "nothing published");
            }
            History::PublishedThenTransferred => {
                assert_eq!(outcome.status, 0, "{}\n{}", outcome.stdout, outcome.stderr);
                assert_eq!(world.env.pds.records().len(), pds_before, "idempotent re-publish");
            }
        }
    }
}

/// RA-12 (upgrade)
/// ```gherkin
/// @driving_port @real-io @c4a @adr-087-5 @contract-shape:unbounded-preservation
/// Scenario: Opening a store made before owner-only signing keeps everything in it
///   Given Jeff's store was made before identity links and owner-only signing
///   When Jeff reads every claim about memory-safety, twice
///   Then the store records the identity-link and repo-authority upgrades, once each
///   And every claim, peer claim and contributor it held is unchanged
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-12: pending DELIVER"]
fn opening_a_store_made_before_owner_only_signing_keeps_everything_in_it() {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH]);
    let rows_before = prior_rows(&world.env);

    for read in ["first", "second"] {
        let out = when_jeff_reads_philosophy(&world, MEMORY_SAFETY, &[], None);
        assert_eq!(out.status, 0, "[{read}] {}\n{}", out.stdout, out.stderr);
        let versions = schema_versions(&world.env);
        for v in [7, 8] {
            assert_eq!(
                versions.iter().filter(|x| **x == v).count(),
                1,
                "[{read}] v{v} recorded once: {versions:?}"
            );
        }
        assert_eq!(prior_rows(&world.env), rows_before, "[{read}] prior rows unchanged");
    }
}

/// RA-13 (table over Jeff's own link at signing time)
/// ```gherkin
/// @driving_port @real-io @n-2 @adr-087-6 @security @contract-shape:bounded-change
/// Scenario Outline: Signing from an import re-checks Jeff's own link first
///   Given Jeff linked "jeffbailey" on 1 October and has not re-checked it since
///   And <bio>
///   When Jeff imports jeffbailey/tidepool and signs candidate 1 on 21 October
///   Then GitHub was asked for jeffbailey's profile before signing
///   And <outcome>
///   Examples:
///     | bio                                     | outcome                                                       |
///     | the bio still names Jeff's identity     | the claim is signed and published                             |
///     | the bio no longer names Jeff's identity | nothing is signed; he is told his identity link is not verified |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RA-13: pending DELIVER"]
fn signing_from_an_import_rechecks_jeffs_own_link_first() {
    let tidepool = Subject::Repo(TIDEPOOL);
    for still_named in [true, false] {
        let row = if still_named { "bio holds" } else { "bio dropped" };
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        if !still_named {
            bio_drops_the_did(&world, JEFF_GH);
        }
        let asked_before = world.github_paths().len();
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_imports_and_signs(&world, TIDEPOOL, CHECK_PLUS_20_DAYS);

        assert!(
            world
                .github_paths_since(asked_before)
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&JEFF_GH.login_path())
                    || p == &JEFF_GH.id_path()),
            "[{row}] Jeff's own link was re-checked"
        );
        if still_named {
            assert_signed_and_published(&world, &outcome, tidepool);
        } else {
            assert_signing_refused(&outcome, tidepool, Refusal::LinkNotVerified, None, None, row);
            assert_nothing_signed(row, &before, &world.env);
        }
    }
}

// =============================================================================
// Slice 5 — imported candidates kept locally (OOS-D12 B, C-OOS-7)
// =============================================================================

/// IC-1
/// ```gherkin
/// @driving_port @real-io @oos-d12 @c-oos-7 @a-11 @contract-shape:pure-function
/// Scenario: Jeff reviews what he imported, offline, one repo at a time
///   Given Jeff imported acme-tools/anvil and rachelk/kelp
///   And the network is off
///   When Jeff lists his imported candidates for acme-tools/anvil
///   Then he sees anvil's dependency-pinning candidate and nothing about kelp
///   And GitHub is not asked anything
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing IC-1: pending DELIVER (slice 5)"]
fn jeff_reviews_what_he_imported_offline_one_repo_at_a_time() {
    let world = authority_world();
    given_jeff_imported(&world, ANVIL, LINKED_AT);
    given_jeff_imported(&world, KELP, LINKED_AT);
    let asked_before = world.github_paths().len();

    let listed = when_jeff_lists_imported_candidates(&world, &["--subject", &ANVIL.subject()]);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert!(
        listed.stdout.contains(&ANVIL.subject()) && listed.stdout.contains(DEPENDENCY_PINNING),
        "anvil's candidate is listed;\n{}",
        listed.stdout
    );
    assert!(
        !listed.stdout.contains(&KELP.subject()),
        "the filter keeps kelp out;\n{}",
        listed.stdout
    );
    assert_github_not_asked(&world, asked_before, "offline list");
}

/// IC-2 (guardrail; review F-01)
/// ```gherkin
/// @driving_port @real-io @guardrail @c-oos-7 @kpi-oos-2 @s-oos-9 @contract-shape:bounded-change
/// Scenario: Importing a repo keeps its candidates locally and nowhere else
///   Given anvil's authority file lists Rachel, Rachel claims anvil pins its dependencies, and Jeff re-checked anvil
///   And Jeff's weighted view of dependency-pinning weighs anvil
///   When Jeff imports acme-tools/anvil
///   Then his imported candidates grew
///   And nothing reached his published repo, nothing was signed
///   And his weighted view of dependency-pinning is unchanged
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing IC-2: pending DELIVER (slice 5)"]
fn importing_a_repo_keeps_its_candidates_locally_and_nowhere_else() {
    let anvil = Subject::Repo(ANVIL);
    let mut world = authority_world();
    the_repo_holds_authority_file(&world, ANVIL, listing(&[RACHEL_DID]));
    given_rachel_published(&mut world, vec![peer_claim_about_object(anvil, DEPENDENCY_PINNING)]);
    given_jeff_verified_authority(&world, LINKED_AT);
    let weighted_before = when_jeff_reads_philosophy(&world, DEPENDENCY_PINNING, &["--weighted"], Some(LINKED_AT));
    assert!(
        !weighted_rows_about(&weighted_before.stdout, anvil).is_empty(),
        "precondition: anvil is weighted;\n{}",
        weighted_before.stdout
    );
    let imported_before = imported_candidate_count(&world.env);
    let before = capture_authority_universe(&world.env);

    let imported = when_jeff_imports(&world, ANVIL, Some(LINKED_AT));

    assert_eq!(imported.status, 0, "{}\n{}", imported.stdout, imported.stderr);
    assert!(
        imported_candidate_count(&world.env) > imported_before,
        "the imported candidates are kept"
    );
    assert_nothing_signed("import", &before, &world.env);
    let weighted_after = when_jeff_reads_philosophy(&world, DEPENDENCY_PINNING, &["--weighted"], Some(LINKED_AT));
    assert_eq!(
        weighted_rows(&weighted_after.stdout),
        weighted_rows(&weighted_before.stdout),
        "imported candidates never weigh"
    );
}

/// IC-3 (empty)
/// ```gherkin
/// @driving_port @real-io @edge @c1a @contract-shape:unbounded-preservation
/// Scenario: With nothing imported, the list says so
///   Given Jeff has imported nothing
///   When Jeff lists his imported candidates
///   Then he is told there are no imported candidates, and the command succeeds
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing IC-3: pending DELIVER (slice 5)"]
fn with_nothing_imported_the_list_says_so() {
    let world = authority_world();
    let before = capture_authority_universe(&world.env);

    let listed = when_jeff_lists_imported_candidates(&world, &[]);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert!(
        listed.stdout.contains(NO_IMPORTED_CANDIDATES),
        "{}",
        listed.stdout
    );
    assert_authority_store_unchanged("empty list", &before, &world.env);
}

/// IC-4 (apply twice — DISTILL proposal, upstream U-OOS-6)
/// ```gherkin
/// @driving_port @real-io @c4a @spec-ambiguity @contract-shape:pure-function
/// Scenario: Importing the same repo twice lists each candidate once
///   Given Jeff imported acme-tools/anvil on 1 October and again on 3 October
///   When Jeff lists his imported candidates
///   Then anvil's dependency-pinning candidate is listed once
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing IC-4: pending DELIVER (slice 5; U-OOS-6 ruling)"]
fn importing_the_same_repo_twice_lists_each_candidate_once() {
    let world = authority_world();
    given_jeff_imported(&world, ANVIL, LINKED_AT);
    given_jeff_imported(&world, ANVIL, CHECK_PLUS_2_DAYS);

    let listed = when_jeff_lists_imported_candidates(&world, &[]);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    let lines = listed
        .stdout
        .lines()
        .filter(|l| l.contains(&ANVIL.subject()) && l.contains(DEPENDENCY_PINNING))
        .count();
    assert_eq!(lines, 1, "listed once;\n{}", listed.stdout);
}
