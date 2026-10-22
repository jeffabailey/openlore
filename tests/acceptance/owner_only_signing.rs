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
//! Every scenario is enabled (DELIVER slice 1, WS-OOS-1 first). PS-3 and
//! PS-7 are the guards that held before DELIVER and must keep holding
//! (scaffold policy, DWD-OOS-2).
//!
//! Observation (ruling R1, GDL D-e): PRIMARY Thens read exit status, the
//! refusal text, what Jeff sees through `graph query` / `identity list` and
//! the wire JSON on his PDS (`assert_jeff_sees_no_change`,
//! `assert_signed_and_kept_locally`, `assert_published_marking`); the store
//! universe is the labelled SECONDARY `guardrail_*` (Mandate-8 waiver).
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
/// @oos-d4 @c-oos-1 @s-oos-1 @contract-shape:bounded-change
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
    let seen = what_jeff_sees(&world, rachels);
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
    assert_jeff_sees_no_change("rachelk", &seen, &world, rachels);
    guardrail_store_unchanged("rachelk", &before, &world.env);
}

// =============================================================================
// Refusals say why and who can sign (ADR-086 §10)
// =============================================================================

/// PS-2 (table over why Jeff holds no person authority)
/// ```gherkin
/// @driving_port @real-io @error @security @kpi-oos-1 @c6b @s-oos-1 @contract-shape:unbounded-preservation
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
fn a_person_claim_jeff_cannot_attest_is_refused_says_why_and_who_can_sign() {
    let rows: [RefusalRow; 3] = [
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
        let seen = what_jeff_sees(&world, subject);
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
        assert_jeff_sees_no_change(row, &seen, &world, subject);
        guardrail_store_unchanged(row, &before, &world.env);
    }
}

// =============================================================================
// Dispute and withdrawal stay open (KPI-OOS-3, C-OOS-2)
// =============================================================================

/// PS-3 (table over the two exempt kinds; chained on WS-OOS-1; review H-1)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-3 @c-oos-2 @oos-d11 @guardrail @s-oos-2 @contract-shape:bounded-change
/// Scenario Outline: Jeff can always dispute or withdraw, whatever authority he holds
///   Given <situation>
///   When Jeff <acts>
///   Then his published repo holds <record>
///   Examples:
///     | situation                                                                     | acts                                          | record                                                          |
///     | Rachel published "rachelk adheres to memory-safety" and Jeff pulled it        | disputes Rachel's claim about her own account | his dispute of Rachel's claim, about github:rachelk             |
///     | Jeff signed and published about "jeffbailey", then his bio stopped naming him | withdraws his own claim about "jeffbailey"    | his withdrawal of that claim, about github:jeffbailey           |
/// ```
#[test]
// ACTIVE regression guard (KPI-OOS-3): holds today and must hold after DELIVER.
fn jeff_can_always_dispute_or_withdraw_whatever_authority_he_holds() {
    // Row 1 — a non-owner disputes the owner's claim.
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    let rachels_account = Subject::Person(RACHEL_GH);
    let rachels = given_rachel_published(&mut world, vec![peer_claim_about(rachels_account)]);
    let published_before = world.env.pds.records().len();

    let disputed = when_jeff_counters(&world, &rachels[0], CHECK_PLUS_2_DAYS);

    assert_eq!(
        disputed.status, 0,
        "[counter] {}\n{}",
        disputed.stdout, disputed.stderr
    );
    assert_published_marking(
        &world,
        published_before,
        "counters",
        &rachels[0],
        rachels_account,
        "counter",
    );

    // Row 2 — a former owner withdraws.
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    let own_account = Subject::Person(JEFF_GH);
    let own = given_jeff_signed_and_published(&world, own_account, CHECK_PLUS_2_DAYS);
    bio_drops_the_did(&world, JEFF_GH);
    given_i_verified(&world, Some(CHECK_PLUS_7_DAYS));
    let published_before = world.env.pds.records().len();

    let withdrawn = when_jeff_retracts(&world, &own, CHECK_PLUS_9_DAYS);

    assert_eq!(
        withdrawn.status, 0,
        "[retract] {}\n{}",
        withdrawn.stdout, withdrawn.stderr
    );
    assert_published_marking(
        &world,
        published_before,
        "retracts",
        &own,
        own_account,
        "retract",
    );
}

// =============================================================================
// Offline signing from a fresh local check (I-9 exception, ADR-086 §5)
// =============================================================================

/// PS-4 (boundary table over the age of Jeff's latest link check)
/// ```gherkin
/// @driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @oos-d7 @security @s-oos-3 @contract-shape:bounded-change
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
        let seen = what_jeff_sees(&world, own);
        let asked_before = world.github_paths().len();
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs_offline(&world, own, now);

        assert_github_not_asked(&world, asked_before, now);
        match refusal {
            None => {
                assert_signed_and_kept_locally(now, &outcome, &seen, &world, own);
                guardrail_store_delta(
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
                assert_jeff_sees_no_change(now, &seen, &world, own);
                guardrail_store_unchanged(now, &before, &world.env);
            }
        }
    }
}

/// PS-9 (ruling R3: GitHub cannot answer at signing time; chained on PS-4's Given)
/// ```gherkin
/// @driving_port @real-io @infrastructure-failure @c7a @i-9 @r-3 @security @s-oos-3 @contract-shape:bounded-change
/// Scenario Outline: When GitHub cannot answer as Jeff signs from an import, his latest check decides, as offline
///   Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
///   And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
///   And <trouble> once the import has read his profile
///   When Jeff reads "jeffbailey" as a person and signs his memory-safety candidate at <time>
///   Then <outcome>
///   And his linked identities read as before (the failed re-check recorded nothing)
///   Examples:
///     | trouble                                         | time            | outcome                                                                        |
///     | GitHub says it is too busy to answer            | 3 October 09:00 | the claim is signed and reaches his published repo                             |
///     | GitHub cannot be reached                        | 3 October 09:00 | the claim is signed and reaches his published repo                             |
///     | GitHub says it is too busy to answer            | 8 October 09:01 | nothing is signed; he is told to re-check authority, with the command to run   |
///
/// @driving_port @real-io @security @s-oos-3 @r-3 @adr-087-6 @contract-shape:bounded-change
/// Scenario Outline: When GitHub answers as Jeff signs from an import, his own link is re-checked first, once
///   Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
///   And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
///   And GitHub answers every profile read, BurntSushi's included
///   When Jeff reads "<person>" as a person and signs candidate 1 on 21 October at 09:00
///   Then <outcome>
///   And GitHub was asked for Jeff's own profile <asked>
///   Examples:
///     | person     | outcome                                                                                              | asked                                                     |
///     | jeffbailey | the claim is signed and reaches his published repo; his identities show one new check, dated today   | once for the import and exactly once more for the re-check |
///     | BurntSushi | nothing is signed; he is told BurntSushi can sign it after linking; his identities read as before    | never                                                     |
/// ```
#[test]
fn when_github_cannot_answer_as_jeff_signs_from_an_import_his_latest_check_decides() {
    // The import itself reads the person's profile once (`scrape person`).
    const IMPORT_READS_THE_PROFILE: usize = 1;
    let own = Subject::Person(JEFF_GH);
    let rows: [(ProfileTrouble, &str, Option<Refusal>); 3] = [
        (ProfileTrouble::TooBusy, CHECK_PLUS_2_DAYS, None),
        (ProfileTrouble::Unreachable, CHECK_PLUS_2_DAYS, None),
        (
            ProfileTrouble::TooBusy,
            CHECK_PLUS_7_DAYS_1_MIN,
            Some(Refusal::CheckTooOld),
        ),
    ];
    for (trouble, now, refusal) in rows {
        let row = format!("{trouble:?} at {now}");
        let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
        github_stops_answering_jeffs_profile_after(&world, IMPORT_READS_THE_PROFILE, trouble);
        let seen = what_jeff_sees(&world, own);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_scrapes_person_and_signs(&world, JEFF_GH, now);

        match refusal {
            None => assert_signed_and_published(&world, &outcome, own),
            Some(reason) => {
                assert_signing_refused(
                    &outcome,
                    own,
                    reason,
                    None,
                    Some(&verify_command(own)),
                    &row,
                );
                assert_nothing_signed_or_published(&row, &seen, &world, own);
                guardrail_nothing_signed(&row, &before, &world.env);
            }
        }
        assert_eq!(
            what_jeff_sees(&world, own).identities,
            seen.identities,
            "[{row}] the failed re-check recorded nothing"
        );
    }

    // GitHub answers: a stale (20-day-old) check is refreshed exactly once
    // when Jeff signs about his own account, and never otherwise.
    let rows: [(Account, Option<Refusal>, usize); 2] = [
        (JEFF_GH, None, IMPORT_READS_THE_PROFILE + 1),
        (BURNTSUSHI_GH, Some(Refusal::NoLink), 0),
    ];
    for (person, refusal, jeffs_profile_reads) in rows {
        let row = format!("GitHub answers, {} at {CHECK_PLUS_20_DAYS}", person.login);
        let subject = Subject::Person(person);
        let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
        github_serves_the_profile_of(&world, BURNTSUSHI_GH);
        let seen = what_jeff_sees(&world, subject);
        let checks_before = link_checks(&world.env);
        let before = capture_authority_universe(&world.env);
        let asked_before = world.github_paths().len();

        let outcome = when_jeff_scrapes_person_and_signs(&world, person, CHECK_PLUS_20_DAYS);
        let asked = world.github_paths_since(asked_before);

        match refusal {
            None => {
                assert_signed_and_published(&world, &outcome, subject);
                let listed = when_i_list(&world, Some(CHECK_PLUS_20_DAYS));
                assert_check_age(&listed, MY_DID, JEFF_GH, "today", false);
                let mut one_more = checks_before.clone();
                one_more.push(CheckRow::of(MY_DID, JEFF_GH, Verdict::Verified));
                assert_eq!(
                    link_checks(&world.env),
                    one_more,
                    "[{row}] guardrail: exactly one new check of Jeff's own link"
                );
            }
            Some(reason) => {
                assert_signing_refused(
                    &outcome,
                    subject,
                    reason,
                    Some(CanSign::Login(person.login)),
                    None,
                    &row,
                );
                assert_nothing_signed_or_published(&row, &seen, &world, subject);
                assert_eq!(
                    what_jeff_sees(&world, subject).identities,
                    seen.identities,
                    "[{row}] his linked identities read as before (no check recorded)"
                );
                guardrail_nothing_signed(&row, &before, &world.env);
                assert_eq!(
                    link_checks(&world.env),
                    checks_before,
                    "[{row}] guardrail: no check recorded"
                );
            }
        }
        assert_eq!(
            profile_reads_of(&asked, JEFF_GH),
            jeffs_profile_reads,
            "[{row}] how often GitHub was asked for Jeff's own profile; asked: {asked:#?}"
        );
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
        given_repo_scraped(
            &world.env,
            &repo.full(),
            jeff_and_burntsushi_contribute(),
            None,
        );
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
/// @driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @s-oos-1 @contract-shape:bounded-change
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
///
/// @driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @s-oos-1 @u-oos-12 @contract-shape:unbounded-preservation
/// Scenario: A stronger inference about BurntSushi is not signed, though Jeff signed the earlier one
///   Given Jeff once held a verified link to BurntSushi's account
///   And Jeff signed the inferred memory-safety adherence about BurntSushi from ripgrep alone
///   And Jeff has since withdrawn that link
///   And Jeff recorded that BurntSushi also builds rust-lang/regex, so the inference about him is stronger
///   When Jeff signs the stronger candidate about BurntSushi
///   Then nothing is signed or published; he is told BurntSushi can sign it after linking the account
///   And his earlier adherence about BurntSushi reads as before
/// ```
#[test]
fn jeff_signs_an_inferred_adherence_only_about_himself() {
    let rows: [(Account, Option<Refusal>); 2] =
        [(JEFF_GH, None), (BURNTSUSHI_GH, Some(Refusal::NoLink))];
    for (person, refusal) in rows {
        let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
        let subject = Subject::Person(person);
        let listed = when_jeff_lists_inferred_people(&world);
        let n = candidate_number_naming(&listed, subject);
        let seen = what_jeff_sees(&world, subject);
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
                assert_jeff_sees_no_change(person.login, &seen, &world, subject);
                guardrail_store_unchanged(person.login, &before, &world.env);
            }
        }
    }

    // U-OOS-12: the superseding inference about someone else's account.
    let row = "supersedes BurntSushi";
    let (world, earlier) = store_where_jeff_signed_burntsushis_inference_then_withdrew_the_link();
    let subject = Subject::Person(BURNTSUSHI_GH);
    let listed = when_jeff_lists_inferred_people(&world);
    assert_candidate_would_supersede(&listed, subject, &earlier);
    let n = candidate_number_naming(&listed, subject);
    let seen = what_jeff_sees(&world, subject);
    let before = capture_authority_universe(&world.env);

    let outcome = when_jeff_signs_inferred(&world, n, CHECK_PLUS_2_DAYS);

    assert_signing_refused(
        &outcome,
        subject,
        Refusal::NoLink,
        Some(CanSign::Login(BURNTSUSHI_GH.login)),
        None,
        row,
    );
    assert_jeff_sees_no_change(row, &seen, &world, subject);
    guardrail_store_unchanged(row, &before, &world.env);
}

/// The store PS-5's supersedes case starts from: Jeff linked "jeffbailey"
/// and once held a verified link to BurntSushi's account; Rachel published
/// memory-safety claims about ripgrep and regex; Jeff recorded ripgrep's
/// contributors, signed the inference about BurntSushi from ripgrep alone,
/// withdrew his BurntSushi link, then recorded regex's contributors (the
/// evidence change). Returns the world and the earlier claim's CID.
fn store_where_jeff_signed_burntsushis_inference_then_withdrew_the_link() -> (LinkWorld, String) {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    let burntsushi_link = given_jeff_held_a_link_to(&world, BURNTSUSHI_GH);
    given_repo_scraped(
        &world.env,
        &RIPGREP.full(),
        jeff_and_burntsushi_contribute(),
        None,
    );
    given_rachel_published(
        &mut world,
        [RIPGREP, REGEX]
            .iter()
            .map(|r| peer_claim_about(Subject::Repo(*r)))
            .collect(),
    );
    let earlier =
        given_jeff_signed_inferred_about(&world, Subject::Person(BURNTSUSHI_GH), CHECK_PLUS_2_DAYS);
    given_jeff_withdrew_his_link(&world, &burntsushi_link, CHECK_PLUS_2_DAYS);
    given_repo_scraped(
        &world.env,
        &REGEX.full(),
        jeff_and_burntsushi_contribute(),
        None,
    );
    (world, earlier)
}

/// PS-6 (chained on PS-5's Given)
/// ```gherkin
/// @driving_port @real-io @kpi-oos-1 @adr-086-10 @s-oos-1 @contract-shape:unbounded-preservation
/// Scenario: Jeff's inferred people say who can sign each candidate, and listing writes nothing
///   Given Jeff linked "jeffbailey"
///   And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
///   When Jeff lists his inferred people
///   Then the candidate about jeffbailey says "You can sign"
///   And the candidate about BurntSushi says it can be signed by BurntSushi after "openlore identity link github BurntSushi"
///   And nothing in his store or published repo changed
/// ```
#[test]
fn jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing() {
    let world = store_where_jeff_and_burntsushi_build_memory_safe_repos();
    let seen = what_jeff_sees(&world, Subject::Person(JEFF_GH));
    let before = capture_authority_universe(&world.env);

    let listed = when_jeff_lists_inferred_people(&world);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_candidate_says_who_can_sign(&listed, Subject::Person(JEFF_GH), None);
    assert_candidate_says_who_can_sign(
        &listed,
        Subject::Person(BURNTSUSHI_GH),
        Some(CanSign::Login(BURNTSUSHI_GH.login)),
    );
    assert_jeff_sees_no_change("listing", &seen, &world, Subject::Person(JEFF_GH));
    guardrail_store_unchanged("listing", &before, &world.env);
}

// =============================================================================
// Contested and renamed accounts (UC-2, ADR-087 §1, G-2)
// =============================================================================

/// PS-7
/// ```gherkin
/// @driving_port @real-io @edge @security @uc-2 @adr-087-1 @guardrail @s-oos-1 @contract-shape:bounded-change
/// Scenario: Jeff still signs about his account when its bio also names another identity
///   Given the "jeffbailey" bio names both Jeff's identity and Rachel's
///   And Jeff and Rachel both published a link to "jeffbailey", and Jeff re-checked every link in his store (both verify)
///   When Jeff signs and publishes "jeffbailey adheres to memory-safety"
///   Then the claim is signed and reaches his published repo
/// ```
#[test]
// ACTIVE regression guard (review H-3): holds today (nothing is gated) and
// must keep holding once the gate lands — a contested account still signs.
fn jeff_still_signs_about_his_account_when_its_bio_also_names_another_identity() {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, JEFF_GH)],
    );
    bio_names_both(&world, JEFF_GH, MY_DID, RACHEL_DID);
    given_i_verified(&world, Some(LINKED_AT));
    let own = Subject::Person(JEFF_GH);

    let signed = when_jeff_signs(&world, own, Some(CHECK_PLUS_2_DAYS));

    assert_signed_and_published(&world, &signed, own);
}

/// PS-8 (table over the old and the new login)
/// ```gherkin
/// @driving_port @real-io @edge @g-2 @a-14 @adr-087-1 @security @s-oos-1 @contract-shape:bounded-change
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
fn after_jeff_renames_his_github_account_he_signs_only_under_the_new_login() {
    let rows: [(Account, Option<Refusal>); 2] = [
        (JEFF_RENAMED, None),
        (JEFF_GH, Some(Refusal::LoginNotCurrent)),
    ];
    for (login, refusal) in rows {
        let world = authority_world();
        given_jeff_linked_his_account(&world);
        i_renamed_my_account(
            &world,
            JEFF_GH,
            JEFF_RENAMED,
            openlore_test_support::OldLogin::NotFound,
        );
        given_i_verified(&world, Some(LINKED_AT));
        let subject = Subject::Person(login);
        let seen = what_jeff_sees(&world, subject);
        let before = capture_authority_universe(&world.env);

        let outcome = when_jeff_signs(&world, subject, Some(CHECK_PLUS_2_DAYS));

        match refusal {
            None => assert_signed_and_published(&world, &outcome, subject),
            Some(reason) => {
                assert_signing_refused(&outcome, subject, reason, None, None, login.login);
                assert_jeff_sees_no_change(login.login, &seen, &world, subject);
                guardrail_store_unchanged(login.login, &before, &world.env);
            }
        }
    }
}

/// One PS-2 refusal row: label, Given, subject account, expected refusal.
type RefusalRow = (&'static str, fn(&LinkWorld), Account, Refusal);

// =============================================================================
// U-OOS-D30 — `publish push` takes a permit per claim, like `claim publish`
// =============================================================================

/// GIVEN: Jeff registered `instance` as his publish target (`publish init`).
fn given_jeff_registered_his_instance(world: &LinkWorld, instance: &FakeInstance) {
    let init = run_openlore_publish(&world.env, &["init", instance.endpoint_url()], instance);
    assert_eq!(
        init.status,
        0,
        "precondition: Jeff registers his instance;{}",
        shown(&init)
    );
}

/// WHEN: Jeff pushes his whole store to his registered instance at `now`.
fn when_jeff_pushes(world: &LinkWorld, now: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["publish", "push"],
        Invocation {
            now: Some(now),
            ..Default::default()
        },
    )
}

/// The run's exit, stdout and stderr, for a failing assertion.
fn shown(outcome: &CliOutcome) -> String {
    format!(
        "\n--- exit {} ---\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr
    )
}

/// The instance's committed CIDs, sorted.
fn instance_cids(instance: &FakeInstance) -> Vec<String> {
    let mut cids = instance.stored_cids();
    cids.sort();
    cids
}

/// PS-D30a (crafter-owned regression, user decision D30)
/// ```gherkin
/// @driving_port @real-io @security @u-oos-d30 @adr-086-4 @contract-shape:bounded-change
/// Scenario: Pushing my store sends what I may publish and refuses the rest, like claim publish
///   Given Jeff linked "jeffbailey" and signed a claim about himself without publishing it
///   And his store holds a claim about rachelk signed before owner-only signing
///   And he disputed Rachel's claim about rachelk/kelp
///   When Jeff pushes his store to his instance
///   Then his own claim, his link and his dispute reach the instance
///   And the claim about rachelk does not, naming rachelk and why
///   And the push exits 1
/// ```
#[test]
fn pushing_my_store_sends_what_i_may_publish_and_refuses_a_claim_about_someone_else() {
    let mut world = authority_world();
    given_jeff_linked_his_account(&world);
    given_jeff_signed_unpublished(&world, Subject::Person(JEFF_GH), CHECK_PLUS_2_DAYS);
    let rachels = given_rachel_published(&mut world, vec![peer_claim_about(Subject::Repo(KELP))]);
    let about_rachel = Subject::Person(RACHEL_GH);
    let pre_gate = seed_local_claim_about(
        &world.env,
        &about_rachel.uri(),
        "org.openlore.philosophy.memory-safety",
    );
    let disputed = when_jeff_counters(&world, &rachels[0], CHECK_PLUS_2_DAYS);
    assert_eq!(
        disputed.status,
        0,
        "precondition: Jeff may dispute Rachel's claim;{}",
        shown(&disputed)
    );
    let instance = FakeInstance::fresh();
    given_jeff_registered_his_instance(&world, &instance);

    let outcome = when_jeff_pushes(&world, CHECK_PLUS_2_DAYS);

    assert_eq!(
        outcome.status,
        1,
        "a refused claim fails the push;{}",
        shown(&outcome)
    );
    assert_refused(
        &outcome,
        about_rachel,
        Refusal::NoLink,
        Some(CanSign::Login(RACHEL_GH.login)),
        None,
        "push about rachelk",
    );
    assert!(
        outcome
            .stdout
            .contains(&format!("Not pushed (authority): {}", about_rachel.uri())),
        "the push report lists the refused claim;{}",
        shown(&outcome)
    );
    let expected: Vec<String> = local_claim_cids(&world.env)
        .into_iter()
        .filter(|cid| *cid != pre_gate)
        .collect();
    assert_eq!(expected.len(), 3, "his link, his own claim and his dispute");
    assert_eq!(
        instance_cids(&instance),
        expected,
        "the instance holds exactly what Jeff may publish"
    );
}

/// PS-D30b (crafter-owned regression, user decision D30, ADR-087 §2)
/// ```gherkin
/// @driving_port @real-io @edge @security @u-oos-d30 @adr-087-2 @contract-shape:bounded-change
/// Scenario: A claim signed under a repo's old name is not pushed after the rename
///   Given Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October
///   And he signed a claim about jeffbailey/tidepool without publishing it
///   And then he renamed it to jeffbailey/tidepool-rs and re-checked jeffbailey/tidepool
///   When Jeff pushes his store on 8 October
///   Then the claim is refused, naming jeffbailey/tidepool-rs, and the instance does not hold it
/// ```
#[test]
fn a_claim_signed_under_a_repos_old_name_is_not_pushed_after_the_rename() {
    let tidepool = Subject::Repo(TIDEPOOL);
    let world = authority_world();
    given_jeff_linked_his_account(&world);
    given_jeff_verified_authority_of(&world, tidepool, LINKED_AT);
    let cid = given_jeff_signed_unpublished(&world, tidepool, CHECK_PLUS_2_DAYS);
    jeff_renames_tidepool(&world);
    given_jeff_verified_authority_of(&world, tidepool, CHECK_PLUS_2_DAYS);
    let instance = FakeInstance::fresh();
    given_jeff_registered_his_instance(&world, &instance);

    let outcome = when_jeff_pushes(&world, CHECK_PLUS_7_DAYS);

    assert_signing_refused(
        &outcome,
        tidepool,
        Refusal::Renamed,
        None,
        Some(&TIDEPOOL_RENAMED.full()),
        "renamed before push",
    );
    assert!(
        !instance_cids(&instance).contains(&cid),
        "the claim under the old name is not on the instance;{}",
        shown(&outcome)
    );
}
