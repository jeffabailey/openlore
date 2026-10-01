//! `openlore scrape person <user | profile URL>` — read a GitHub person:
//! their public profile, their top owned repos, a scrape of each of those
//! repos (signals, proposed claims, contributors recorded), then the person
//! view from the local store.
//!
//! Layer: subprocess acceptance — the REAL `openlore` binary over a REAL
//! DuckDB store, with `FakeGithub` as the only double.

mod support;

#[allow(unused_imports)]
use support::*;

fn jeff() -> FakePerson {
    FakePerson::new("jeffabailey")
        .named("Jeff Bailey")
        .with_bio("Builds open source tools")
        .located("Portland, OR")
        .with_followers(42)
        .owns("openlore", 12, Some("Rust"), "2026-09-29T00:00:00Z")
        .owns("blog", 3, Some("Go"), "2026-08-01T00:00:00Z")
        .owns("dotfiles", 1, Some("Shell"), "2025-01-01T00:00:00Z")
        .forked("rust", 900)
}

fn jeff_github() -> GithubServer {
    GithubServer::start(
        FakeGithub::for_public_person(jeff()).with_contributors(vec![
            FakeContributor::human("jeffabailey", 1, 500),
            FakeContributor::human("helper", 2, 20),
        ]),
    )
}

/// SP-1: a profile URL shows the profile, lists the top owned repos (forks
/// skipped), scrapes each one into the store, and ends with the person view
/// that now links the person to those repos.
///
/// @driving_port @real-io @happy
#[test]
fn scrape_person_shows_profile_scrapes_top_repos_and_links_the_person() {
    let env = TestEnv::initialized();
    let github = jeff_github();

    let outcome = run_openlore_scrape(
        &env,
        &[
            "scrape",
            "person",
            "https://github.com/jeffabailey/",
            "--repos",
            "2",
        ],
        github.base_url(),
    );

    assert_eq!(
        outcome.status, 0,
        "scrape person must succeed; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    let out = &outcome.stdout;
    for expected in [
        "Jeff Bailey (@jeffabailey)",
        "Builds open source tools",
        "Portland, OR",
        "followers : 42",
        "Top repos (2 of 3 owned; 1 fork skipped)",
        "jeffabailey/openlore",
        "jeffabailey/blog",
    ] {
        assert!(out.contains(expected), "expected {expected:?} in:\n{out}");
    }
    assert!(
        !out.contains("jeffabailey/dotfiles"),
        "only the top 2 repos are listed and scraped; got:\n{out}"
    );
    assert!(
        !out.contains("jeffabailey/rust"),
        "forks are skipped; got:\n{out}"
    );
    // Each top repo was scraped: candidates proposed, contributors recorded.
    assert_eq!(
        out.matches("contributors recorded: 2").count(),
        2,
        "both repos record their contributors; got:\n{out}"
    );
    assert!(
        out.contains("openlore scrape github jeffabailey/openlore --sign N"),
        "each scraped repo says how to sign its candidates; got:\n{out}"
    );
    // The person view now links them to both scraped repos.
    assert!(
        out.contains("github:jeffabailey is linked to 2 scraped repos"),
        "the person view must show the newly recorded links; got:\n{out}"
    );
    assert_no_claim_persisted(&env);
}

/// SP-2: `--repos 0` shows the profile and the person view, and scrapes
/// nothing.
///
/// @driving_port @real-io @edge
#[test]
fn scrape_person_with_zero_repos_scrapes_nothing() {
    let env = TestEnv::initialized();
    let github = jeff_github();

    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "person", "jeffabailey", "--repos", "0"],
        github.base_url(),
    );

    assert_eq!(outcome.status, 0, "stderr:\n{}", outcome.stderr);
    assert!(outcome.stdout.contains("Jeff Bailey (@jeffabailey)"));
    assert!(
        !outcome.stdout.contains("contributors recorded"),
        "--repos 0 scrapes no repo; got:\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("github:jeffabailey is not linked to any repo you've scraped"),
        "the person view still renders; got:\n{}",
        outcome.stdout
    );
}

/// SP-3: an `owner/repo` target is refused before any GitHub request, with a
/// pointer at `scrape github`.
///
/// @driving_port @real-io @sad
#[test]
fn scrape_person_refuses_a_repo_target() {
    let env = TestEnv::initialized();
    let github = jeff_github();

    let outcome = run_openlore_scrape(
        &env,
        &[
            "scrape",
            "person",
            "https://github.com/jeffabailey/openlore",
        ],
        github.base_url(),
    );

    assert_ne!(outcome.status, 0, "a repo target must be refused");
    assert!(
        outcome
            .stderr
            .contains("scrape github jeffabailey/openlore"),
        "the refusal points at `scrape github`; stderr:\n{}",
        outcome.stderr
    );
}

/// SP-4: without a token the user is told how many requests the scrape will
/// make against the unauthenticated hourly limit.
///
/// @driving_port @real-io @us-scr-004
#[test]
fn scrape_person_warns_about_the_unauthenticated_rate_limit() {
    let env = TestEnv::initialized();
    let github = jeff_github();

    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "person", "jeffabailey", "--repos", "3"],
        github.base_url(),
    );

    assert_eq!(outcome.status, 0, "stderr:\n{}", outcome.stderr);
    assert!(
        outcome.stdout.contains("GITHUB_TOKEN"),
        "an unauthenticated multi-repo scrape mentions GITHUB_TOKEN; got:\n{}",
        outcome.stdout
    );
}
