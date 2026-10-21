//! owner-only-signing — slice 3 acceptance: how every reader treats a
//! `github:` assertion once authority exists (ADR-086 §2/§3/§7/§8, ADR-087
//! §7/§8, OOS-D9/D10, C-OOS-5/6/8, KPI-OOS-2).
//!
//! The rule under test: every `github:` assertion reads `owner`,
//! `repo-listed`, `third-party` or `authority not checked` (index rows:
//! `authority not checked here`) — labelled, never hidden; the weighted view
//! counts only assertions that are attested (or not about GitHub), names what
//! it skipped, and shows the rest per author only on request; counters,
//! retractions and identity links are never weighted, so a spoofed dispute
//! marker cannot buy weight.
//!
//! Drives the REAL `openlore` binary (`graph query`, `search`, `infer people`,
//! `authority verify`) and the REAL `openlore ui` viewer (`/score`,
//! `/project`), the REAL `openlore-indexer` for network search. Fake
//! (driven-external only): `FakeGithubAccounts`, `FakeGithub`, `PeerPds`,
//! `FakePds`, `FakeIdentity`, the ingest double.
//!
//! Layer 3/4 — example-only (Mandate 9). The unbounded weight invariant over
//! generated feeds is CORE-OOS-5 in `owner_only_signing_core.rs` (layer 2);
//! here RD-4 pins it through both driving ports with one example per kind.
//! Every scenario is `#[ignore]`d pending DELIVER. Every Then reads a
//! driving port (reader output, viewer pages, PDS wire); the store universe
//! is only a labelled SECONDARY guardrail (ruling R1).

mod support;

#[allow(unused_imports)]
use support::authority::*;
#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

/// The store RD-1/2/3/6 share (chained narrative): Jeff linked
/// "jeffbailey" and signed a claim about his repo tidepool (owner); anvil's
/// authority file lists Rachel, who claims anvil (repo-listed), tidepool
/// (third-party) and kelp (never checked). Jeff re-checked tidepool and
/// anvil, not kelp. One followed peer per store (house harness: `peer pull`
/// resolves every subscription).
fn labelled_store() -> LinkWorld {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    the_repo_holds_authority_file(&world, ANVIL, listing(&[RACHEL_DID]));
    given_jeff_verified_authority_of(&world, Subject::Repo(TIDEPOOL), LINKED_AT);
    given_jeff_signed_and_published(&world, Subject::Repo(TIDEPOOL), CHECK_PLUS_2_DAYS);
    given_rachel_published(
        &mut world,
        vec![
            peer_claim_about(Subject::Repo(ANVIL)),
            peer_claim_about(Subject::Repo(KELP)),
            peer_claim_about(Subject::Repo(TIDEPOOL)),
        ],
    );
    given_jeff_verified_authority_of(&world, Subject::Repo(ANVIL), CHECK_PLUS_2_DAYS);
    world
}

// =============================================================================
// Labels (ADR-086 §8)
// =============================================================================

/// RD-1
/// ```gherkin
/// @driving_port @real-io @adr-086-8 @oos-d9 @c-oos-5 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Every claim about a repo is shown with whose word it is
///   Given Jeff signed a claim about his repo tidepool
///   And acme-tools/anvil's authority file lists Rachel, who claims anvil, tidepool and rachelk/kelp
///   And Jeff re-checked tidepool and anvil, but not kelp
///   When Jeff reads every claim about memory-safety
///   Then his own tidepool claim reads "owner", Rachel's anvil claim "repo-listed",
///        Rachel's tidepool claim "third-party" and her kelp claim "authority not checked"
///   And no claim is hidden
/// ```
#[test]
fn every_claim_about_a_repo_is_shown_with_whose_word_it_is() {
    let world = labelled_store();

    let read = when_jeff_reads_philosophy(&world, MEMORY_SAFETY, &[], Some(CHECK_PLUS_2_DAYS));

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    assert_labelled(&read, MY_DID, Subject::Repo(TIDEPOOL), Authority::Owner);
    assert_labelled(
        &read,
        RACHEL_DID,
        Subject::Repo(ANVIL),
        Authority::RepoListed,
    );
    assert_labelled(
        &read,
        RACHEL_DID,
        Subject::Repo(TIDEPOOL),
        Authority::ThirdParty,
    );
    assert_labelled(
        &read,
        RACHEL_DID,
        Subject::Repo(KELP),
        Authority::NotChecked,
    );
}

// =============================================================================
// Weight (ADR-086 §8, KPI-OOS-2)
// =============================================================================

/// RD-2 (chained on RD-1)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @adr-022 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: The weighted view counts only attested claims and says what it left out
///   Given the store of RD-1
///   When Jeff asks for the weighted view of memory-safety, explaining tidepool's weight
///   Then tidepool and anvil are weighted, and kelp is not
///   And tidepool's weight is built from Jeff's claim alone
///   And he is told "2 claims not counted: 1 third-party, 1 authority not checked" with the command to re-check
/// ```
#[test]
fn the_weighted_view_counts_only_attested_claims_and_says_what_it_left_out() {
    let world = labelled_store();
    let tidepool = TIDEPOOL.subject();

    let weighted = when_jeff_reads_philosophy(
        &world,
        MEMORY_SAFETY,
        &["--weighted", "--explain", &tidepool],
        Some(CHECK_PLUS_2_DAYS),
    );

    assert_eq!(
        weighted.status, 0,
        "{}\n{}",
        weighted.stdout, weighted.stderr
    );
    assert!(!weighted_rows_about(&weighted.stdout, Subject::Repo(TIDEPOOL)).is_empty());
    assert!(!weighted_rows_about(&weighted.stdout, Subject::Repo(ANVIL)).is_empty());
    assert!(
        weighted_rows_about(&weighted.stdout, Subject::Repo(KELP)).is_empty(),
        "kelp is not weighted;\n{}",
        weighted.stdout
    );
    assert!(
        weighted.stdout.contains(&format!("Contribution: {MY_DID}"))
            && !weighted
                .stdout
                .contains(&format!("Contribution: {RACHEL_DID}")),
        "tidepool's weight is Jeff's claim alone (the --explain breakdown);\n{}",
        weighted.stdout
    );
    for phrase in [
        "2 claims not counted",
        "1 third-party",
        "1 authority not checked",
        "openlore authority verify",
    ] {
        assert!(
            weighted.stdout.contains(phrase),
            "the footer says `{phrase}`;\n{}",
            weighted.stdout
        );
    }
}

/// RD-3 (chained on RD-2)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Jeff can see the claims left out, per author, without changing the weight
///   Given the store of RD-1, and the weighted view of memory-safety it gives
///   When Jeff asks for the weighted view of memory-safety including unattested claims
///   Then the weighted rows are exactly the same
///   And a separate "Unattested claims, by author" section lists Rachel's tidepool and kelp claims under her name, and not her anvil claim
/// ```
#[test]
fn jeff_can_see_the_claims_left_out_per_author_without_changing_the_weight() {
    let world = labelled_store();
    let plain = when_jeff_reads_philosophy(
        &world,
        MEMORY_SAFETY,
        &["--weighted"],
        Some(CHECK_PLUS_2_DAYS),
    );

    let with_unattested = when_jeff_reads_philosophy(
        &world,
        MEMORY_SAFETY,
        &["--weighted", "--include-unattested"],
        Some(CHECK_PLUS_2_DAYS),
    );

    assert_eq!(
        with_unattested.status, 0,
        "{}\n{}",
        with_unattested.stdout, with_unattested.stderr
    );
    let (weight, section) = with_unattested
        .stdout
        .split_once(UNATTESTED_SECTION)
        .unwrap_or_else(|| {
            panic!(
                "a `{UNATTESTED_SECTION}` section;\n{}",
                with_unattested.stdout
            )
        });
    assert_eq!(
        weighted_rows(weight),
        weighted_rows(&plain.stdout),
        "the weighted rows are unchanged"
    );
    assert!(
        !rows_about(section, RACHEL_DID, &TIDEPOOL.subject()).is_empty(),
        "{section}"
    );
    assert!(
        !rows_about(section, RACHEL_DID, &KELP.subject()).is_empty(),
        "{section}"
    );
    assert!(
        rows_about(section, RACHEL_DID, &ANVIL.subject()).is_empty(),
        "an attested claim is not in the unattested section;\n{section}"
    );
}

/// RD-4 (table over what is added beside an attested claim; KPI-OOS-2, A-13)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-2 @kpi-gdl-2 @a-13 @uc-16 @uc-20 @security @c-oos-6 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario Outline: Nothing but an attested claim moves the weight
///   Given anvil's authority file lists Jeff, and Jeff re-checked anvil and signed and published "anvil is memory-safe"
///   And Jeff noted the weighted view of memory-safety
///   And Rachel published <addition>, Jeff pulled it and re-checked authority
///   When Jeff asks for the weighted view of memory-safety
///   Then it is exactly as Jeff noted it
///   Examples:
///     | addition                                                                 |
///     | a dispute of Jeff's claim                                                |
///     | a claim about anvil marked as disputing a claim nobody has (spoofed)     |
///     | a claim about anvil, and her own withdrawal of it                        |
///     | a link from her identity to her GitHub account                           |
///     | a claim about anvil, which its authority file does not list her for     |
///     | a withdrawal of Jeff's claim (only its author may withdraw it)           |
/// ```
#[test]
fn nothing_but_an_attested_claim_moves_the_weight() {
    use claim_domain::ReferenceType;
    let anvil = Subject::Repo(ANVIL);
    let rows: [&str; 6] = [
        "dispute",
        "spoofed dispute",
        "withdrawn",
        "link",
        "unattested",
        "foreign withdrawal",
    ];
    for row in rows {
        let mut world = authority_world();
        the_repo_holds_authority_file(&world, ANVIL, listing(&[MY_DID]));
        given_jeff_verified_authority_of(&world, anvil, LINKED_AT);
        let jeffs = given_jeff_signed_and_published(&world, anvil, LINKED_AT);
        let noted =
            when_jeff_reads_philosophy(&world, MEMORY_SAFETY, &["--weighted"], Some(LINKED_AT));
        assert!(
            !weighted_rows_about(&noted.stdout, anvil).is_empty(),
            "[{row}] precondition: Jeff's attested claim weighs anvil;\n{}",
            noted.stdout
        );
        let rachels_claim = peer_claim_about(anvil);
        match row {
            "dispute" => {
                given_rachel_published(
                    &mut world,
                    vec![rachels_claim.marking(ReferenceType::Counters, &jeffs)],
                );
            }
            "spoofed dispute" => {
                given_rachel_published(
                    &mut world,
                    vec![rachels_claim.marking(
                        ReferenceType::Counters,
                        "bafyreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
                    )],
                );
            }
            "withdrawn" => {
                let own = given_rachel_published(&mut world, vec![rachels_claim.clone()]);
                given_rachel_published(
                    &mut world,
                    vec![rachels_claim.marking(ReferenceType::Retracts, &own[0])],
                );
            }
            "foreign withdrawal" => {
                given_rachel_published(
                    &mut world,
                    vec![rachels_claim.marking(ReferenceType::Retracts, &jeffs)],
                );
            }
            "link" => {
                given_rachel_published(&mut world, vec![link_spec(RACHEL_DID, RACHEL_GH)]);
            }
            _ => {
                given_rachel_published(&mut world, vec![rachels_claim]);
            }
        }
        given_jeff_verified_authority(&world, CHECK_PLUS_2_DAYS);

        let after = when_jeff_reads_philosophy(
            &world,
            MEMORY_SAFETY,
            &["--weighted"],
            Some(CHECK_PLUS_2_DAYS),
        );

        assert_eq!(
            after.status, 0,
            "[{row}] {}\n{}",
            after.stdout, after.stderr
        );
        assert_eq!(
            weighted_rows(&after.stdout),
            weighted_rows(&noted.stdout),
            "[{row}] the weighted view must not move"
        );
    }
}

/// RD-5 (chained on RD-4 row 1)
/// ```gherkin
/// @driving_port @real-io @security @uc-16 @adr-086-2 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: A claim marked as a dispute is shown as a dispute, not as someone's word about the repo
///   Given anvil's authority file lists Jeff, and Jeff re-checked anvil and signed and published "anvil is memory-safe"
///   And Rachel published a claim about anvil marked as disputing Jeff's claim, and Jeff pulled it
///   When Jeff reads every claim about acme-tools/anvil
///   Then Rachel's claim is shown as disputing Jeff's claim
///   And it carries no authority label
/// ```
#[test]
fn a_claim_marked_as_a_dispute_is_shown_as_a_dispute() {
    let anvil = Subject::Repo(ANVIL);
    let mut world = authority_world();
    the_repo_holds_authority_file(&world, ANVIL, listing(&[MY_DID]));
    given_jeff_verified_authority_of(&world, anvil, LINKED_AT);
    let jeffs = given_jeff_signed_and_published(&world, anvil, LINKED_AT);
    given_rachel_published(
        &mut world,
        vec![peer_claim_about(anvil).marking(claim_domain::ReferenceType::Counters, &jeffs)],
    );

    let read = when_jeff_reads_subject(&world, anvil);

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    assert!(
        read.stdout
            .contains(&format!("counters {jeffs} by {MY_DID}")),
        "shown as a dispute;\n{}",
        read.stdout
    );
    let rachels = rows_about(&read.stdout, RACHEL_DID, &anvil.uri()).join("\n");
    for label in [
        Authority::Owner,
        Authority::RepoListed,
        Authority::ThirdParty,
    ] {
        assert!(
            !rachels.contains(label.label()),
            "a dispute carries no `{}` label:\n{rachels}",
            label.label()
        );
    }
}

// =============================================================================
// Viewer parity (KPI-OOS-2, viewer)
// =============================================================================

/// RD-6 (chained on RD-1)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-2 @adr-086-8 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Jeff's viewer weighs and labels claims exactly as his terminal does
///   Given the store of RD-1
///   When Jeff opens Rachel's score page and tidepool's project page in his viewer
///   Then Rachel's page scores anvil and neither kelp nor tidepool, as the terminal's weighted view of her claims does
///   And tidepool's page labels Jeff's claim "owner" and Rachel's "third-party"
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RD-6: pending DELIVER"]
fn jeffs_viewer_weighs_and_labels_claims_exactly_as_his_terminal_does() {
    let world = labelled_store();
    let viewer = ViewerServer::start(&world.env);

    let rachel_page = viewer.get(&format!("/score?contributor={RACHEL_DID}"));
    let project = viewer.get(&format!("/project?subject={}", TIDEPOOL.subject()));

    let rachel_scored = scored_pairings(&rachel_page.body);
    assert!(
        rachel_scored.iter().any(|p| p.contains(&ANVIL.subject())),
        "{rachel_scored:#?}"
    );
    assert!(
        rachel_scored
            .iter()
            .all(|p| !p.contains(&KELP.subject()) && !p.contains(&TIDEPOOL.subject())),
        "{rachel_scored:#?}"
    );
    let terminal = when_jeff_weighs_contributor(&world, RACHEL_DID);
    assert!(!weighted_rows_about(&terminal.stdout, Subject::Repo(ANVIL)).is_empty());
    assert!(weighted_rows_about(&terminal.stdout, Subject::Repo(KELP)).is_empty());
    for label in [Authority::Owner, Authority::ThirdParty] {
        assert!(
            project.body.contains(label.label()),
            "`{}`:\n{}",
            label.label(),
            project.body
        );
    }
}

// =============================================================================
// Network search (ADR-087 §8, UC-7, C-OOS-8)
// =============================================================================

/// RD-7 (table: no local check vs a local check)
/// ```gherkin
/// @driving_port @real-io @security @uc-7 @oos-d10 @c-oos-8 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario Outline: Network search labels every GitHub claim, using Jeff's own check when he has one
///   Given the network index holds <claim>
///   And <local>
///   When Jeff searches the network for <subject>
///   Then the row reads "<label>"
///   Examples:
///     | claim                                     | local                                                        | subject                 | label                       |
///     | Sven's claim about rust-lang/regex        | Jeff has no check of rust-lang/regex                         | rust-lang/regex         | authority not checked here  |
///     | Priya's claim about acme-tools/anvil      | anvil's file lists Priya and Jeff re-checked anvil           | acme-tools/anvil        | repo-listed                 |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RD-7: pending DELIVER (supersedes GDL IR-16b)"]
fn network_search_labels_every_github_claim_using_jeffs_own_check_when_he_has_one() {
    let regex = Subject::Repo(REGEX);
    let anvil = Subject::Repo(ANVIL);
    let rows: [(&str, &str, Subject, bool, &str); 2] = [
        (
            "no local check",
            openlore_test_support::SVEN_DID,
            regex,
            false,
            NOT_CHECKED_HERE,
        ),
        (
            "local check",
            openlore_test_support::PRIYA_DID,
            anvil,
            true,
            Authority::RepoListed.label(),
        ),
    ];
    for (row, author, subject, checked_locally, label) in rows {
        let world = authority_world();
        if checked_locally {
            the_repo_holds_authority_file(&world, ANVIL, listing(&[author]));
            given_jeff_verified_authority_of(&world, subject, LINKED_AT);
        }
        let indexer =
            seed_network_index_from_specs(&world.env, vec![index_claim_about(author, subject)]);
        let uri = subject.uri();

        let found = run_openlore_search(&world.env, &["search", "--subject", &uri], &indexer);

        assert_eq!(
            found.status, 0,
            "[{row}] {}\n{}",
            found.stdout, found.stderr
        );
        let lines = rows_about(&found.stdout, author, &uri);
        assert!(
            lines.iter().any(|l| l.contains(label)),
            "[{row}] the row reads `{label}`;\n{}",
            found.stdout
        );
    }
}

// =============================================================================
// Grandfathering (ADR-086 §7) and inference sources
// =============================================================================

/// RD-8
/// ```gherkin
/// @driving_port @real-io @oos-d3 @adr-086-7 @a-7 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Claims made before owner-only signing are judged by the same rule, with no cut-off date
///   Given Jeff's store was made before owner-only signing, with his claim about BurntSushi/ripgrep and Rachel's pulled claim about rust-lang/regex
///   And Jeff re-checked authority
///   When Jeff reads every claim about memory-safety
///   Then both claims are still shown, and both read "third-party"
/// ```
#[test]
fn claims_made_before_owner_only_signing_are_judged_by_the_same_rule() {
    let world = frozen_store_world();
    given_jeff_verified_authority(&world, CHECK_PLUS_2_DAYS);

    let read = when_jeff_reads_philosophy(&world, MEMORY_SAFETY, &[], Some(CHECK_PLUS_2_DAYS));

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    assert_labelled(&read, MY_DID, Subject::Repo(RIPGREP), Authority::ThirdParty);
    assert_labelled(
        &read,
        RACHEL_DID,
        Subject::Repo(REGEX),
        Authority::ThirdParty,
    );
}

/// RD-9
/// ```gherkin
/// @driving_port @real-io @uc-10 @adr-086-8 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Inferred people show the authority of each repo claim they rest on
///   Given anvil's authority file lists Rachel, Rachel claims anvil is memory-safe, and Jeff re-checked anvil
///   And Jeff recorded anvil's contributors, BurntSushi first
///   When Jeff lists his inferred people
///   Then BurntSushi's candidate names Rachel's anvil claim as "repo-listed"
///   And nothing in his store or published repo changed
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing RD-9: pending DELIVER"]
fn inferred_people_show_the_authority_of_each_repo_claim_they_rest_on() {
    let anvil = Subject::Repo(ANVIL);
    let mut world = authority_world();
    the_repo_holds_authority_file(&world, ANVIL, listing(&[RACHEL_DID]));
    given_rachel_published(&mut world, vec![peer_claim_about(anvil)]);
    given_jeff_verified_authority(&world, LINKED_AT);
    given_repo_scraped(
        &world.env,
        &ANVIL.full(),
        jeff_and_burntsushi_contribute(),
        None,
    );
    let seen = what_jeff_sees(&world, anvil);
    let before = capture_authority_universe(&world.env);

    let listed = when_jeff_lists_inferred_people(&world);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    let n = candidate_number_naming(&listed, Subject::Person(BURNTSUSHI_GH));
    let block = candidate_block(&listed.stdout, n).unwrap_or_default();
    assert!(
        !rows_about(&block, RACHEL_DID, &anvil.uri()).is_empty()
            && rows_about(&block, RACHEL_DID, &anvil.uri())
                .join("\n")
                .contains(Authority::RepoListed.label()),
        "the source claim reads `repo-listed`:\n{block}"
    );
    assert_jeff_sees_no_change("infer", &seen, &world, anvil);
    guardrail_store_unchanged("infer", &before, &world.env);
}

/// RD-10 (reader side of G-2; review F-02; chained on PS-8)
/// ```gherkin
/// @driving_port @real-io @edge @security @g-2 @a-14 @kpi-oos-2 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: After Jeff renames his GitHub account, his claim about the old login reads third-party and stays published
///   Given Jeff linked "jeffbailey" and signed and published "jeffbailey adheres to memory-safety"
///   And Jeff then renamed the account to "jeff-b" and re-checked his link
///   When Jeff reads every claim about memory-safety
///   Then his claim about github:jeffbailey is still shown and reads "third-party"
///   And nothing was withdrawn from his published repo
/// ```
#[test]
fn after_jeff_renames_his_account_his_claim_about_the_old_login_reads_third_party() {
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    given_jeff_signed_and_published(&world, Subject::Person(JEFF_GH), CHECK_PLUS_2_DAYS);
    i_renamed_my_account(
        &world,
        JEFF_GH,
        JEFF_RENAMED,
        openlore_test_support::OldLogin::NotFound,
    );
    given_i_verified(&world, Some(CHECK_PLUS_7_DAYS));
    let pds_before = world.env.pds.records().len();

    let read = when_jeff_reads_philosophy(&world, MEMORY_SAFETY, &[], Some(CHECK_PLUS_7_DAYS));

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    assert_labelled(
        &read,
        MY_DID,
        Subject::Person(JEFF_GH),
        Authority::ThirdParty,
    );
    assert_eq!(
        world.env.pds.records().len(),
        pds_before,
        "nothing withdrawn"
    );
}

// =============================================================================
// `--include-unattested` needs `--weighted` (ruling P-OOS-1, resolves U-OOS-7)
// =============================================================================

/// RD-11 (C5a mode combination; ruling P-OOS-1 accepted 2026-10-20)
/// ```gherkin
/// @driving_port @real-io @error @c5a @p-oos-1 @s-oos-7 @contract-shape:unbounded-preservation
/// Scenario: Asking for unattested claims without the weighted view is a usage mistake
///   Given the store of RD-1
///   When Jeff reads every claim about memory-safety including unattested claims, without asking for the weighted view
///   Then he is told the unattested section needs the weighted view, and the command fails as a usage mistake
///   And no claim is listed
/// ```
#[test]
fn asking_for_unattested_claims_without_the_weighted_view_is_a_usage_mistake() {
    let world = labelled_store();

    let read = when_jeff_reads_philosophy(
        &world,
        MEMORY_SAFETY,
        &["--include-unattested"],
        Some(CHECK_PLUS_2_DAYS),
    );

    assert_eq!(
        read.status, 2,
        "a usage mistake exits 2;\n{}\n{}",
        read.stdout, read.stderr
    );
    assert!(
        format!("{}\n{}", read.stdout, read.stderr).contains(INCLUDE_UNATTESTED_NEEDS_WEIGHTED),
        "says `{INCLUDE_UNATTESTED_NEEDS_WEIGHTED}`;\n{}\n{}",
        read.stdout,
        read.stderr
    );
    for author in [MY_DID, RACHEL_DID] {
        assert!(
            !read.stdout.contains(author),
            "no claim is listed;\n{}",
            read.stdout
        );
    }
}
