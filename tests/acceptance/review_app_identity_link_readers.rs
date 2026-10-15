//! github-did-identity-link — slice 3 readers: an OpenLore reader pulls a
//! link the review app wrote (self-attested, ADR-071) and proves its GitHub
//! side itself (ADR-084 Earned Trust round trip, ADR-085 §2 staleness window,
//! GDL-D11, KPI-GDL-3/4).
//!
//! RR-1/RR-3 need no app: a PDS (`FakeAtprotoNetwork`) holds a record in
//! EXACTLY the shape the app writes (`self_attested_link_value`). RR-2 runs the
//! REAL app to publish inside GitHub's ~60 s cache window, then the REAL
//! `openlore` CLI re-checks it (build the app first:
//! `cargo build -p openlore-review-app`).
//!
//! Layer 4 (subprocess + real DuckDB + real HTTP): example-only. `#[ignore]`d
//! for one-at-a-time DELIVER (slice 3).

// `support` and `review_app` both pull in tests/common/state_delta.rs.
#![allow(clippy::duplicate_mod)]

mod support;

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::{FakeAtprotoNetwork, FakeGithubAccounts, StoredRecord};
use review_app::*;
use support::identity_links::{assert_shown, assert_verify_reported, Account, Shown, Verdict};
use support::people::{openlore_with, Invocation};
use support::{run_openlore_with_peer_resolver, CliOutcome, FakeIdentity, TestEnv};

fn priya_account() -> Account {
    Account {
        login: "priyaraman",
        id: 4_210_001,
    }
}

/// GIVEN: Maria adds `persona` as a peer and pulls (the unchanged ADR-071 path).
fn maria_pulls(env: &TestEnv, net: &FakeAtprotoNetwork, persona: Persona) -> CliOutcome {
    let pds = net.pds_url_for(persona.did());
    let added =
        run_openlore_with_peer_resolver(env, &["peer", "add", persona.did()], persona.did(), &pds);
    assert_eq!(
        added.status, 0,
        "precondition: peer add;\n{}\n{}",
        added.stdout, added.stderr
    );
    run_openlore_with_peer_resolver(env, &["peer", "pull"], persona.did(), &pds)
}

/// `openlore identity <args…>` as Maria, with GitHub wired to `github`.
fn maria_identity(env: &TestEnv, github: &FakeGithubAccounts, args: &[&str]) -> CliOutcome {
    let mut full = vec!["identity"];
    full.extend_from_slice(args);
    openlore_with(
        env,
        &full,
        Invocation {
            github_base: Some(github.base_url()),
            ..Invocation::default()
        },
    )
}

/// GIVEN: Priya's repo holds the link the app writes, and Maria pulled it.
fn maria_pulled_priyas_app_link() -> (TestEnv, FakeAtprotoNetwork, FakeGithubAccounts) {
    let net = FakeAtprotoNetwork::start(bluesky_accounts());
    seed_claim_record(
        &net,
        Persona::Priya.did(),
        self_attested_link_value(Persona::Priya),
        None,
    );
    let github = FakeGithubAccounts::start(github_fixtures());
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let pulled = maria_pulls(&env, &net, Persona::Priya);
    assert_eq!(
        pulled.status, 0,
        "precondition: pull;\n{}\n{}",
        pulled.stdout, pulled.stderr
    );
    (env, net, github)
}

/// GIVEN: Priya verified in the app, removed her DID from her bio while GitHub
/// still served the old bio, and published her link within that minute.
fn priya_published_inside_the_cache_window(world: &ReviewWorld) -> StoredRecord {
    let mut browser = given_github_verified(world, Persona::Priya);
    let old_bio = format!("Rust, tide models. {}", Persona::Priya.did());
    removes_did_from_bio(world, Persona::Priya);
    world
        .github
        .serve_stale_profile(Persona::Priya.github_login(), Some(&old_bio));
    when_opens_link_preview(&mut browser);
    when_confirms_link(&mut browser);
    then_pds_holds_self_attested_link(world, Persona::Priya)
}

/// RR-1a
/// ```gherkin
/// @driving_port @real-io @adapter-integration @adr-084-earned-trust @gdl-d11 @contract-shape:pure-function
/// Scenario: Priya's app-published link reaches Maria as Priya's unchecked claim
///   Given Priya's repo holds the self-attested link the review app wrote for github "priyaraman"
///   And Maria pulled Priya's claims
///   When Maria lists linked identities
///   Then she sees Priya's claim on github:priyaraman, not checked
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn priyas_app_published_link_reaches_maria_as_priyas_unchecked_claim() {
    let (env, _net, github) = maria_pulled_priyas_app_link();

    let listed = maria_identity(&env, &github, &["list"]);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(
        &listed,
        Persona::Priya.did(),
        priya_account(),
        Shown::NotChecked,
    );
}

/// RR-1b (KPI-GDL-4 analog; chained on RR-1a)
/// ```gherkin
/// @driving_port @real-io @adr-084-earned-trust @gdl-d11 @kpi-gdl-4 @contract-shape:bounded-change
/// Scenario: Once Maria checks it herself, Priya's app-published link shows as Priya's GitHub account
///   Given Maria pulled Priya's app-published link
///   When Maria re-checks Priya's links
///   Then she is told github:priyaraman is verified
///   And her linked identities show Priya as github:priyaraman, verified
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn once_maria_checks_it_herself_priyas_link_shows_as_priyas_github_account() {
    let (env, _net, github) = maria_pulled_priyas_app_link();

    let checked = maria_identity(&env, &github, &["verify", "--did", Persona::Priya.did()]);

    assert_verify_reported(&checked, priya_account(), Verdict::Verified);
    let listed = maria_identity(&env, &github, &["list"]);
    assert_shown(
        &listed,
        Persona::Priya.did(),
        priya_account(),
        Shown::Associated,
    );
}

/// RR-2a (edge, accepted limitation)
/// ```gherkin
/// @driving_port @real-io @edge @adr-085-2 @adr-076-6 @contract-shape:bounded-change
/// Scenario: Priya can publish her link in the minute after removing her DID, before GitHub shows the change
///   Given Priya verified github "priyaraman" in the app
///   And she removed her DID from her bio, but GitHub still serves her old bio for a minute
///   When she publishes her link within that minute
///   Then her repo holds the link
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn priya_can_publish_her_link_in_the_minute_before_github_shows_her_bio_change() {
    let world = ReviewWorld::new();

    let link = priya_published_inside_the_cache_window(&world);

    assert_eq!(link.value["object"], "github:priyaraman");
}

/// RR-2b (KPI-GDL-3; chained on RR-2a)
/// ```gherkin
/// @driving_port @real-io @edge @kpi-gdl-3 @c-gdl-6 @contract-shape:bounded-change
/// Scenario: Maria sees that a link published just before GitHub showed Priya's bio change has lost its proof
///   Given Priya published her link in the minute before GitHub showed her bio change, and that minute has passed
///   And Maria pulled Priya's claims
///   When Maria re-checks her links
///   Then she is told Priya's bio names no DID
///   And her linked identities show Priya's claim on github:priyaraman as "proof gone"
///   And Priya's link is still in her repo, unretracted
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn maria_sees_that_a_link_published_before_github_showed_priyas_bio_change_has_lost_its_proof() {
    let world = ReviewWorld::new();
    let link = priya_published_inside_the_cache_window(&world);
    world
        .github
        .end_stale_profile(Persona::Priya.github_login());
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let pulled = maria_pulls(&env, &world.atproto, Persona::Priya);
    assert_eq!(
        pulled.status, 0,
        "precondition: pull;\n{}\n{}",
        pulled.stdout, pulled.stderr
    );

    let checked = maria_identity(&env, &world.github, &["verify"]);

    assert_verify_reported(&checked, priya_account(), Verdict::DidMissing);
    let listed = maria_identity(&env, &world.github, &["list"]);
    assert_shown(
        &listed,
        Persona::Priya.did(),
        priya_account(),
        Shown::ProofGone(Verdict::DidMissing),
    );
    assert_eq!(
        then_pds_holds_self_attested_link(&world, Persona::Priya),
        link,
        "never auto-retracted (C-GDL-6)"
    );
}

/// RR-3 (error, M-4)
/// ```gherkin
/// @driving_port @real-io @error @m-2 @adr-071 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario: A link of Priya's found in Sam's repository never links Priya
///   Given Sam's repo holds a self-attested link claiming to be Priya's, for github "priyaraman"
///   And Maria pulled Sam's claims and re-checked her links
///   When Maria lists linked identities
///   Then nothing links Priya to github:priyaraman, and GitHub was never asked
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_link_of_priyas_found_in_sams_repository_never_links_priya() {
    let net = FakeAtprotoNetwork::start(bluesky_accounts());
    seed_claim_record(
        &net,
        Persona::Sam.did(),
        self_attested_link_value(Persona::Priya),
        None,
    );
    let github = FakeGithubAccounts::start(github_fixtures());
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    maria_pulls(&env, &net, Persona::Sam);
    let verified = maria_identity(&env, &github, &["verify"]);
    assert_eq!(
        verified.status, 0,
        "precondition: verify;\n{}\n{}",
        verified.stdout, verified.stderr
    );

    let listed = maria_identity(&env, &github, &["list"]);

    assert_shown(
        &listed,
        Persona::Priya.did(),
        priya_account(),
        Shown::Nothing,
    );
    assert!(
        github.request_paths().is_empty(),
        "{:?}",
        github.request_paths()
    );
}
