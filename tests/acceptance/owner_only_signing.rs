//! owner-only-signing — slice 1 acceptance + the feature's WALKING SKELETON:
//! only the owner of a GitHub account can sign what is said about it; anyone
//! can still dispute or withdraw (ADR-086 §1-5/§9/§10, ADR-087 §1, C-OOS-1/2/9,
//! KPI-OOS-1/3).
//!
//! Drives the REAL `openlore` binary (`claim add/counter/retract`, `infer
//! people [--sign]`, `identity link/verify`) — the production composition
//! root. Real: claim-domain (incl. the NEW `authority` gate), the shared sign
//! path (`sign_permitted`), DuckDB (v7 link checks), the single publish path.
//! Fake (driven-external only): `FakeGithubAccounts`, `FakeGithub` (repo
//! contributors), `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! Layer 3 subprocess — example-only (Mandate 9); sad paths are named example
//! rows (Mandate 11). Refusal families are tables. Vocabulary:
//! `support/authority.rs`.
//!
//! Every scenario is `#[ignore]`d (house rule D-d: a red test on trunk blocks
//! every image job). DELIVER enables WS-OOS-1 first; it fails today for the
//! business reason (row 2: `claim add` signs Rachel's account).
//!
//! State machine (C2a) of "may Jeff sign an assertion about github:<login>?"
//! (person tier, ADR-087 §1), from this reader's local checks:
//!
//! | State | Event | Outcome |
//! |---|---|---|
//! | no link | sign | refused (no identity link), hand-off names the link command |
//! | linked, latest check verified ≤ 7 days, login current | sign (online or offline) | permit → signed |
//! | linked, latest check verified > 7 days old or dated in the future | sign offline | refused, names `authority verify` |
//! | linked, latest check disproved | sign | refused (link not verified) |
//! | linked, renamed since (login_seen ≠ subject login) | sign about the old login | refused (login not current) |
//! | any | counter / retract | permitted (exempt, KPI-OOS-3) |

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
// WALKING SKELETON — the first scenario DELIVER enables
// =============================================================================

/// WS-OOS-1
/// ```gherkin
/// @walking_skeleton @driving_port @driving_adapter @real-io @security @kpi-oos-1
/// @oos-d4 @c-oos-1 @contract-shape:bounded-change
/// Scenario Outline: Only Jeff can sign what is said about Jeff's GitHub account
///   Given Jeff has publicly linked his GitHub account "jeffbailey" to his identity
///   When Jeff signs and publishes "<account> adheres to memory-safety"
///   Then <outcome>
///   Examples:
///     | account    | outcome                                                                                       |
///     | jeffbailey | the claim is signed and reaches his published repo                                            |
///     | rachelk    | nothing is signed or published, and he is told rachelk can sign it after linking her account |
/// ```
///
/// Demo value (stakeholder litmus): "OpenLore will sign what I say about
/// myself, and tells me who to ask when I try to speak for someone else."
#[test]
#[ignore = "DISTILL owner-only-signing WS-OOS-1: pending DELIVER (enable FIRST)"]
fn only_jeff_can_sign_what_is_said_about_jeffs_github_account() {
    // Row 1 — his own account: signed and published (the permit path).
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    let own = Subject::Person(JEFF_GH);

    let signed = when_jeff_signs(&world, own, Some(CHECK_PLUS_2_DAYS));

    assert_signed_and_published(&world, &signed, own);

    // Row 2 — Rachel's account: refused with the hand-off, nothing written.
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    let rachels = Subject::Person(RACHEL_GH);
    let before = capture_authority_universe(&world.env);

    let refused = when_jeff_signs(&world, rachels, Some(CHECK_PLUS_2_DAYS));

    assert_signing_refused(
        &refused,
        rachels,
        Refusal::NoLink,
        Some(CanSign::Login(RACHEL_GH.login)),
        None,
        "rachelk",
    );
    assert_authority_store_unchanged("rachelk", &before, &world.env);
}

// =============================================================================
// Refusals say why and who can sign (ADR-086 §10)
// =============================================================================

/// PS-2 (table over why Jeff holds no person authority)
/// ```gherkin
/// @driving_port @real-io @error @security @kpi-oos-1 @c6b @contract-shape:unbounded-preservation
/// Scenario Outline: A person claim Jeff cannot attest is refused, says why and who can sign
///   Given <situation>
///   When Jeff signs and publishes "<account> adheres to memory-safety"
///   Then nothing is signed, published or recorded
///   And he is told "<reason>" and who can sign it
///   Examples:
///     | situation                                                         | account    | reason                              | who can sign            |
///     | Jeff has not linked any GitHub account                            | jeffbailey | you have no identity link to        | jeffbailey, after link  |
///     | Jeff's bio stopped naming his identity and he re-checked his link | jeffbailey | your identity link is not verified  | jeffbailey, after link  |
///     | Jeff linked "jeffbailey"                                          | tobiasw    | you have no identity link to        | tobiasw, after link     |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-2: pending DELIVER"]
fn a_person_claim_jeff_cannot_attest_is_refused_says_why_and_who_can_sign() {
    let rows: [(&str, fn(&LinkWorld), Account, Refusal); 3] = [
        ("no link", |_| {}, JEFF_GH, Refusal::NoLink),
        (
            "proof gone",
            |w| {
                given_jeff_linked_his_account(w);
                bio_drops_the_did(w, JEFF_GH);
                given_i_verified(w, Some(CHECK_PLUS_2_DAYS));
            },
            JEFF_GH,
            Refusal::LinkNotVerified,
        ),
        (
            "someone else's account",
            |w| {
                given_jeff_linked_his_account(w);
            },
            TOBIAS_GH,
            Refusal::NoLink,
        ),
    ];
    for (row, situation, account, reason) in rows {
        let world = authority_world();
        situation(&world);
        let subject = Subject::Person(account);
        let before = capture_authority_universe(&world.env);

        let refused = when_jeff_signs(&world, subject, Some(CHECK_PLUS_2_DAYS));

        assert_signing_refused(
            &refused,
            subject,
            reason,
            Some(CanSign::Login(account.login)),
            None,
            row,
        );
        assert_authority_store_unchanged(row, &before, &world.env);
    }
}

// =============================================================================
// Dispute and withdrawal stay open (KPI-OOS-3, C-OOS-2)
// =============================================================================

/// PS-3 (table over the two exempt kinds; chained on WS-OOS-1)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-3 @c-oos-2 @oos-d11 @guardrail @contract-shape:bounded-change
/// Scenario Outline: Jeff can always dispute or withdraw, whatever authority he holds
///   Given <situation>
///   When Jeff <acts>
///   Then the dispute or withdrawal is signed and reaches his published repo
///   Examples:
///     | situation                                                                       | acts                                                     |
///     | Rachel published "rachelk adheres to memory-safety" and Jeff pulled it          | disputes Rachel's claim about her own account            |
///     | Jeff signed and published about "jeffbailey", then his bio stopped naming him   | withdraws his own claim about "jeffbailey"               |
/// ```
#[test]
// ACTIVE regression guard (KPI-OOS-3): holds today and must hold after DELIVER.
fn jeff_can_always_dispute_or_withdraw_whatever_authority_he_holds() {
    // Row 1 — a non-owner disputes the owner's claim.
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    let rachels = given_rachel_published(
        &mut world,
        vec![peer_claim_about(Subject::Person(RACHEL_GH))],
    );
    let pds_before = world.env.pds.records().len();

    let disputed = when_jeff_counters(&world, &rachels[0], CHECK_PLUS_2_DAYS);

    assert_eq!(disputed.status, 0, "[counter] {}\n{}", disputed.stdout, disputed.stderr);
    assert_eq!(
        world.env.pds.records().len(),
        pds_before + 1,
        "[counter] the dispute reached Jeff's published repo"
    );

    // Row 2 — a former owner withdraws.
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    let own = given_jeff_signed_and_published(&world, Subject::Person(JEFF_GH), CHECK_PLUS_2_DAYS);
    bio_drops_the_did(&world, JEFF_GH);
    given_i_verified(&world, Some(CHECK_PLUS_7_DAYS));
    let pds_before = world.env.pds.records().len();

    let withdrawn = when_jeff_retracts(&world, &own, CHECK_PLUS_9_DAYS);

    assert_eq!(withdrawn.status, 0, "[retract] {}\n{}", withdrawn.stdout, withdrawn.stderr);
    assert_eq!(
        world.env.pds.records().len(),
        pds_before + 1,
        "[retract] the withdrawal reached Jeff's published repo"
    );
}

// =============================================================================
// Offline signing from a fresh local check (I-9 exception, ADR-086 §5)
// =============================================================================

/// PS-4 (boundary table over the age of Jeff's latest link check)
/// ```gherkin
/// @driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @oos-d7 @security @contract-shape:bounded-change
/// Scenario Outline: With the network off, Jeff signs about himself only from a check at most 7 days old
///   Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
///   And the network is off
///   When Jeff signs "jeffbailey adheres to memory-safety" at <time>
///   Then <outcome>
///   And GitHub is not asked anything
///   Examples:
///     | time                    | outcome                                                                       |
///     | 3 October 09:00         | the claim is signed and kept locally                                          |
///     | 8 October 09:00         | the claim is signed and kept locally (exactly 7 days)                         |
///     | 8 October 09:01         | nothing is signed; he is told to run "openlore authority verify --subject github:jeffbailey" |
///     | 20 September 09:00      | nothing is signed (a check dated after now counts as missing); same command  |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-4: pending DELIVER"]
fn with_the_network_off_jeff_signs_about_himself_only_from_a_check_at_most_7_days_old() {
    let own = Subject::Person(JEFF_GH);
    let command = verify_command(own);
    let rows: [(&str, Option<Refusal>); 4] = [
        (CHECK_PLUS_2_DAYS, None),
        (CHECK_PLUS_7_DAYS, None),
        (CHECK_PLUS_7_DAYS_1_MIN, Some(Refusal::CheckTooOld)),
        (BEFORE_THE_CHECK, Some(Refusal::NotChecked)),
    ];
    for (now, refusal) in rows {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        let asked_before = world.github_paths().len();
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs_offline(&world, own, now);

        assert_github_not_asked(&world, asked_before, now);
        match refusal {
            None => {
                assert_eq!(outcome.status, 0, "[{now}] {}\n{}", outcome.stdout, outcome.stderr);
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
                assert_signing_refused(&outcome, own, reason, None, Some(&command), now);
                assert_authority_store_unchanged(now, &before, &world.env);
            }
        }
    }
}

// =============================================================================
// Inference is candidates-only for everyone but the person (UC-9, A-8)
// =============================================================================

/// The store PS-5 / PS-6 share: Jeff linked "jeffbailey"; Rachel published
/// memory-safety claims about BurntSushi/ripgrep and rust-lang/regex; Jeff
/// recorded both repos' contributors (BurntSushi #1, jeffbailey #2).
fn store_where_jeff_and_burntsushi_build_memory_safe_repos() -> LinkWorld {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    let repos = [
        RepoRef::new("BurntSushi", "ripgrep"),
        RepoRef::new("rust-lang", "regex"),
    ];
    for repo in repos {
        given_repo_scraped(&world.env, &repo.full(), jeff_and_burntsushi_contribute(), None);
    }
    given_rachel_published(
        &mut world,
        repos
            .iter()
            .map(|r| peer_claim_about(Subject::Repo(*r)))
            .collect(),
    );
    world
}

/// PS-5 (table over whose adherence Jeff signs)
/// ```gherkin
/// @driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @contract-shape:bounded-change
/// Scenario Outline: Jeff signs an inferred adherence only about himself
///   Given Jeff linked "jeffbailey"
///   And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
///   And Jeff's inferred people list proposes memory-safety for both people
///   When Jeff signs the inferred candidate about <person>
///   Then <outcome>
///   Examples:
///     | person     | outcome                                                                                        |
///     | jeffbailey | the adherence is signed and reaches his published repo                                         |
///     | BurntSushi | nothing is signed or published; he is told BurntSushi can sign it after linking the account   |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-5: pending DELIVER"]
fn jeff_signs_an_inferred_adherence_only_about_himself() {
    let rows: [(Account, Option<Refusal>); 2] = [
        (JEFF_GH, None),
        (BURNTSUSHI_GH, Some(Refusal::NoLink)),
    ];
    for (person, refusal) in rows {
        let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
        let subject = Subject::Person(person);
        let listed = when_jeff_lists_inferred_people(&world);
        let n = candidate_number_naming(&listed, subject);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs_inferred(&world, n, CHECK_PLUS_2_DAYS);

        match refusal {
            None => assert_signed_and_published(&world, &outcome, subject),
            Some(reason) => {
                assert_signing_refused(
                    &outcome,
                    subject,
                    reason,
                    Some(CanSign::Login(person.login)),
                    None,
                    person.login,
                );
                assert_authority_store_unchanged(person.login, &before, &world.env);
            }
        }
    }
}

/// PS-6 (chained on PS-5's Given)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-1 @adr-086-10 @contract-shape:unbounded-preservation
/// Scenario: Jeff's inferred people say who can sign each candidate, and listing writes nothing
///   Given Jeff linked "jeffbailey"
///   And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
///   When Jeff lists his inferred people
///   Then the candidate about jeffbailey says "You can sign"
///   And the candidate about BurntSushi says it can be signed by BurntSushi after "openlore identity link github BurntSushi"
///   And nothing in his store or published repo changed
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-6: pending DELIVER"]
fn jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing() {
    let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
    let before = capture_authority_universe(&world.env);

    let listed = when_jeff_lists_inferred_people(&world);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_candidate_says_who_can_sign(&listed, Subject::Person(JEFF_GH), None);
    assert_candidate_says_who_can_sign(
        &listed,
        Subject::Person(BURNTSUSHI_GH),
        Some(CanSign::Login(BURNTSUSHI_GH.login)),
    );
    assert_authority_store_unchanged("listing", &before, &world.env);
}

// =============================================================================
// Contested and renamed accounts (UC-2, ADR-087 §1, G-2)
// =============================================================================

/// PS-7
/// ```gherkin
/// @driving_port @real-io @edge @security @uc-2 @adr-087-1 @contract-shape:bounded-change
/// Scenario: Jeff still signs about his account when its bio also names another identity
///   Given the "jeffbailey" bio names both Jeff's identity and Rachel's
///   And Jeff and Rachel both published a link to "jeffbailey", and Jeff re-checked every link in his store (both verify)
///   When Jeff signs and publishes "jeffbailey adheres to memory-safety"
///   Then the claim is signed and reaches his published repo
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-7: pending DELIVER"]
fn jeff_still_signs_about_his_account_when_its_bio_also_names_another_identity() {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    given_peer_published(&mut world, Peer::Rachel, vec![link_spec(RACHEL_DID, JEFF_GH)]);
    bio_names_both(&world, JEFF_GH, MY_DID, RACHEL_DID);
    given_i_verified(&world, Some(LINKED_AT));
    let own = Subject::Person(JEFF_GH);

    let signed = when_jeff_signs(&world, own, Some(CHECK_PLUS_2_DAYS));

    assert_signed_and_published(&world, &signed, own);
}

/// PS-8 (table over the old and the new login)
/// ```gherkin
/// @driving_port @real-io @edge @g-2 @a-14 @adr-087-1 @security @contract-shape:bounded-change
/// Scenario Outline: After Jeff renames his GitHub account, he signs only under the new login
///   Given Jeff linked "jeffbailey", renamed the account to "jeff-b" and re-checked his link
///   When Jeff signs and publishes "<login> adheres to memory-safety"
///   Then <outcome>
///   Examples:
///     | login      | outcome                                                                    |
///     | jeff-b     | the claim is signed and reaches his published repo                         |
///     | jeffbailey | nothing is signed; he is told it is no longer the login of his linked account |
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing PS-8: pending DELIVER"]
fn after_jeff_renames_his_github_account_he_signs_only_under_the_new_login() {
    let rows: [(Account, Option<Refusal>); 2] = [
        (JEFF_RENAMED, None),
        (JEFF_GH, Some(Refusal::LoginNotCurrent)),
    ];
    for (login, refusal) in rows {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        i_renamed_my_account(&world, JEFF_GH, JEFF_RENAMED, openlore_test_support::OldLogin::NotFound);
        given_i_verified(&world, Some(LINKED_AT));
        let subject = Subject::Person(login);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs(&world, subject, Some(CHECK_PLUS_2_DAYS));

        match refusal {
            None => assert_signed_and_published(&world, &outcome, subject),
            Some(reason) => {
                assert_signing_refused(&outcome, subject, reason, None, None, login.login);
                assert_authority_store_unchanged(login.login, &before, &world.env);
            }
        }
    }
}
