//! contributor-philosophy-inference — slice-01 acceptance: scraping a repo
//! records who builds it (US-CPI-001; D-3, D-4, D-6; DDD-2/3/5/14; UC-1/2/3).
//!
//! Drives the real `openlore scrape github` verb as a subprocess. Real: the
//! scrape pipeline, the pure contributor selection (bot rule, re-rank, top-N),
//! the DuckDB `contribution_links` table. Fake (driven-external only):
//! `FakeGithub`, now serving `GET /repos/{o}/{r}/contributors` from the
//! ADR-063 lie catalogue (`FakeContributorsPosture`).
//!
//! Layer 3 subprocess — example-only (Mandate 9); every sad path is a named
//! example (Mandate 11). All scenarios `#[ignore]`d for one-at-a-time unskip
//! (the feature WS lives in `infer_people_sign.rs`).
//!
//! Covers: every US-CPI-001 AC; KPI-CPI-5 (≤1 extra request, 0 at
//! `--contributors 0`, 100% bot exclusion); UC-1/UC-2/UC-3; the DDD-15
//! contributor-lie gold fixtures.

mod support;

#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

/// CL-1 (happy): Maria scrapes `BurntSushi/ripgrep` for the first time; the
/// top 30 HUMAN contributors by commits are recorded as people linked to the
/// repo (BurntSushi #1), the two bots inside the top 32 are excluded AND
/// named, and no claim is written or published.
///
/// @us-cpi-001 @driving_port @real-io @kpi-cpi-5 @d-3 @happy
#[test]
fn scraping_a_repo_records_its_top_thirty_human_contributors_and_names_the_bots() {
    // GIVEN Maria has not scraped BurntSushi/ripgrep before.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN she scrapes it.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        repo_with("BurntSushi/ripgrep", ripgrep_contributors()),
        Some(T1),
        "",
    );

    // THEN it succeeds and reports 30 recorded, both bots named as excluded.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Contributors recorded: 30"),
        "count line;\n{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("bots excluded"),
        "bot notice;\n{}",
        out.stdout
    );
    for bot in ["dependabot[bot]", "github-actions[bot]"] {
        assert!(
            out.stdout.contains(bot),
            "excluded bot {bot} must be named;\n{}",
            out.stdout
        );
    }

    // AND exactly 30 human links exist, ranked 1..30, BurntSushi first — the
    // ONLY store change (no claim, no publish).
    let links = links_for(&env, "github:BurntSushi/ripgrep");
    assert_eq!(links.len(), 30, "30 human links: {links:#?}");
    assert!(
        links.iter().all(|l| !l.person.ends_with("[bot]")),
        "no bot may be linked"
    );
    assert_eq!(links[0].person, "github:BurntSushi");
    assert_eq!(links[0].github_user_id, BURNTSUSHI_ID as i64);
    assert_eq!(
        links.iter().map(|l| l.rank).collect::<Vec<_>>(),
        (1..=30).collect::<Vec<_>>()
    );
    assert_store_delta(
        &before,
        &env,
        &[(
            "local.contribution_links",
            format!("{:?}", contribution_links(&env)),
        )],
    );
    assert_no_claim_persisted(&env);
}

/// CL-2 (happy): people already linked to OTHER scraped repos are surfaced —
/// after `rust-lang/regex`, scraping ripgrep shows `BurntSushi → rust-lang/regex`.
///
/// @us-cpi-001 @driving_port @real-io @happy
#[test]
fn people_already_linked_to_other_scraped_repos_are_surfaced() {
    // GIVEN Maria already scraped rust-lang/regex (BurntSushi among its people).
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "rust-lang/regex", regex_contributors(), None);

    // WHEN she scrapes BurntSushi/ripgrep.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        repo_with("BurntSushi/ripgrep", ripgrep_contributors()),
        None,
        "",
    );

    // THEN the overlap section names BurntSushi → rust-lang/regex, and only him.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Also contributes to repos you scraped"),
        "overlap heading;\n{}",
        out.stdout
    );
    assert!(
        shows_overlap(&out.stdout, "BurntSushi", "rust-lang/regex"),
        "overlap line;\n{}",
        out.stdout
    );
    assert!(
        !out.stdout.contains("rg-dev-01 →") && !out.stdout.contains("rg-dev-01 ->"),
        "non-overlapping people are not listed"
    );
}

/// CL-3 (boundary): `--contributors 5` records exactly five HUMANS with
/// dtolnay first, even though a bot sits at raw position 2 (N is applied to
/// humans, DDD-2) — and that skipped bot is named.
///
/// @us-cpi-001 @driving_port @real-io @od-cpi-7 @boundary
#[test]
fn the_contributor_count_can_be_overridden_and_counts_only_humans() {
    // GIVEN Tobias wants only the core maintainers of dtolnay/anyhow.
    let env = TestEnv::initialized();

    // WHEN he scrapes with --contributors 5.
    let scrape = scrape_github(
        &env,
        &["dtolnay/anyhow", "--contributors", "5"],
        repo_with("dtolnay/anyhow", anyhow_contributors()),
        None,
        "",
    );

    // THEN exactly five humans are recorded, dtolnay ranked first.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Contributors recorded: 5"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("dependabot[bot]"),
        "the bot skipped inside the top 5 is named;\n{}",
        out.stdout
    );
    let links = links_for(&env, "github:dtolnay/anyhow");
    assert_eq!(links.len(), 5, "{links:#?}");
    assert_eq!(links[0].person, "github:dtolnay");
    assert_eq!(links[0].rank, 1);
}

/// CL-4 (boundary, UC-1): `--contributors 0` records none, says so, and makes
/// NO contributors request at all.
///
/// @us-cpi-001 @driving_port @real-io @uc-1 @kpi-cpi-5 @boundary
#[test]
fn zero_contributors_records_none_and_asks_github_nothing_extra() {
    // GIVEN a repo with contributors.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN Maria scrapes it with --contributors 0.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep", "--contributors", "0"],
        repo_with("BurntSushi/ripgrep", ripgrep_contributors()),
        None,
        "",
    );

    // THEN it succeeds, says none were recorded, never asked for contributors,
    // and changed nothing.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Contributors recorded: 0"),
        "{}",
        out.stdout
    );
    assert_eq!(
        scrape.contributor_requests(),
        0,
        "no contributors request at N=0: {:?}",
        scrape.seen_paths
    );
    assert_store_unchanged(&before, &env);
}

/// CL-5 (boundary, D-4): re-scraping a month later refreshes rank, commits
/// and last-observed for people still in the top 30, KEEPS the link of a
/// person who fell out (with its original last-observed date), never touches
/// first-observed, and never duplicates a row.
///
/// @us-cpi-001 @driving_port @real-io @d-4 @od-cpi-6 @boundary
#[test]
fn re_scraping_never_loses_previously_recorded_people() {
    // GIVEN ripgrep's 30 contributors were recorded on T1.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", ripgrep_contributors(), Some(T1));

    // WHEN Maria scrapes it again on T2, rg-dev-29 has dropped out of the top
    // 30 (a newcomer took the slot) and BurntSushi has more commits.
    let mut later = ripgrep_contributors();
    later.retain(|c| c.login != "rg-dev-29");
    later[0].contributions = 2_100;
    later.push(FakeContributor::human("newcomer", 99_001, 700));
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        repo_with("BurntSushi/ripgrep", later),
        Some(T2),
        "",
    );
    assert_eq!(
        scrape.outcome.status, 0,
        "{}\n{}",
        scrape.outcome.stdout, scrape.outcome.stderr
    );

    // THEN rg-dev-29's earlier link survives, still last observed on T1 …
    let links = links_for(&env, "github:BurntSushi/ripgrep");
    let dropped = links
        .iter()
        .find(|l| l.person == "github:rg-dev-29")
        .expect("earlier link kept");
    assert_eq!(dropped.last_observed, &T1[..10]);
    // … the others show refreshed commits and T2, first-observed unchanged …
    let bs = links
        .iter()
        .find(|l| l.person == "github:BurntSushi")
        .expect("BurntSushi");
    assert_eq!(bs.contributions, 2_100);
    assert_eq!(bs.last_observed, &T2[..10]);
    assert_eq!(bs.first_observed, &T1[..10]);
    // … and the table holds 31 distinct people (30 + newcomer), no duplicates.
    assert_eq!(links.len(), 31, "{links:#?}");
}

/// CL-6 (error): the GitHub rate budget runs out on the contributors read —
/// the CLI names the rate limit, suggests GITHUB_TOKEN, records NO links
/// (no partial snapshot), and exits non-zero.
///
/// @us-cpi-001 @driving_port @real-io @error @ddd-14
#[test]
#[ignore = "DELIVER slice-01: unskip one-at-a-time (US-CPI-001 rate-limited harvest records nothing, exit != 0)"]
fn a_rate_limited_contributor_harvest_records_nothing_and_says_why() {
    // GIVEN Aanya's unauthenticated budget is exhausted by the contributors read.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN she scrapes BurntSushi/ripgrep.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        FakeGithub::for_public_repo("BurntSushi/ripgrep")
            .with_contributors_posture(FakeContributorsPosture::RateLimited),
        None,
        "",
    );

    // THEN non-zero, rate limit + GITHUB_TOKEN named, nothing recorded.
    let out = &scrape.outcome;
    assert_ne!(
        out.status, 0,
        "a failed harvest is never read as 'no contributors';\n{}",
        out.stdout
    );
    assert!(out.stderr.contains("rate limit"), "{}", out.stderr);
    assert!(out.stderr.contains("GITHUB_TOKEN"), "{}", out.stderr);
    assert_store_unchanged(&before, &env);
}

/// CL-7 (error): a stale token is rejected on the contributors read — exit
/// non-zero, no links, the token value never echoed.
///
/// @us-cpi-001 @driving_port @real-io @error
#[test]
#[ignore = "DELIVER slice-01: unskip one-at-a-time (US-CPI-001 rejected token records nothing)"]
fn a_rejected_token_on_the_contributor_harvest_records_nothing() {
    // GIVEN the configured token is rejected by the contributors read.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);
    let server = GithubServer::start(
        FakeGithub::for_public_repo("BurntSushi/ripgrep")
            .with_contributors_posture(FakeContributorsPosture::TokenRejected),
    );

    // WHEN Maria scrapes with that token.
    let out = run_openlore_scrape_with_token(
        &env,
        &["scrape", "github", "BurntSushi/ripgrep"],
        server.base_url(),
        FIXTURE_REJECTED_PAT,
    );

    // THEN non-zero, the rejection explained, nothing recorded, token unseen.
    assert_ne!(out.status, 0, "{}", out.stdout);
    assert!(out.stderr.contains("GITHUB_TOKEN"), "{}", out.stderr);
    assert_token_value_absent(&out, FIXTURE_REJECTED_PAT);
    assert_store_unchanged(&before, &env);
}

/// CL-8 (error): GitHub returns a contributors row with no login/id (shape
/// drift) — the scrape fails naming the unexpected response and records NO
/// partial link set (the valid rows beside it are not written either).
///
/// @us-cpi-001 @driving_port @real-io @error @ddd-15
#[test]
#[ignore = "DELIVER slice-01: unskip one-at-a-time (DDD-15 malformed contributors row -> no partial write)"]
fn an_unexpected_contributor_list_records_nothing() {
    // GIVEN the contributors list contains one valid row and one without login/id.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);
    let body = serde_json::json!([
        { "login": "BurntSushi", "id": BURNTSUSHI_ID, "type": "User", "contributions": 2000 },
        { "type": "User", "contributions": 10 }
    ]);

    // WHEN Maria scrapes ripgrep.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        FakeGithub::for_public_repo("BurntSushi/ripgrep")
            .with_contributors_posture(FakeContributorsPosture::Malformed(body)),
        None,
        "",
    );

    // THEN non-zero with the shape cause named; nothing recorded.
    assert_ne!(scrape.outcome.status, 0, "{}", scrape.outcome.stdout);
    assert!(
        scrape.outcome.stderr.contains("unexpected"),
        "{}",
        scrape.outcome.stderr
    );
    assert_store_unchanged(&before, &env);
}

/// CL-9 (edge, UC-2): GitHub says the contributor list is too large to list —
/// a named notice, no links, and the scrape otherwise succeeds (exit 0, repo
/// candidates still proposed).
///
/// @us-cpi-001 @driving_port @real-io @uc-2 @ddd-14 @edge
#[test]
#[ignore = "DELIVER slice-01: unskip one-at-a-time (UC-2 too-large contributor list is a notice, exit 0)"]
fn a_repo_too_large_to_list_contributors_is_a_notice_not_a_failure() {
    // GIVEN a repo whose contributor list GitHub refuses as too large.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN Maria scrapes it.
    let scrape = scrape_github(
        &env,
        &["torvalds/linux"],
        FakeGithub::for_public_repo_with_all_signals("torvalds/linux")
            .with_contributors_posture(FakeContributorsPosture::TooLarge),
        None,
        "",
    );

    // THEN exit 0, the reason named, repo candidates still listed, no links.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Contributors not recorded"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("too large"), "{}", out.stdout);
    assert!(
        out.stdout.contains("[1]"),
        "the repo's own candidates are unaffected;\n{}",
        out.stdout
    );
    assert_store_unchanged(&before, &env);
}

/// CL-10 (edge, UC-2): an empty repository (no contributors yet) is a named
/// notice, no links, exit 0.
///
/// @us-cpi-001 @driving_port @real-io @uc-2 @edge
#[test]
#[ignore = "DELIVER slice-01: unskip one-at-a-time (UC-2 empty repo is a notice, exit 0)"]
fn an_empty_repository_is_a_notice_not_a_failure() {
    // GIVEN a freshly created repo with no commits.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN Maria scrapes it.
    let scrape = scrape_github(
        &env,
        &["some-org/empty-repo"],
        FakeGithub::for_public_repo("some-org/empty-repo")
            .with_contributors_posture(FakeContributorsPosture::EmptyRepo),
        None,
        "",
    );

    // THEN exit 0, the notice names the empty repo, nothing recorded.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("Contributors not recorded"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("empty"), "{}", out.stdout);
    assert_store_unchanged(&before, &env);
}

/// CL-11 (error, OD-CPI-7): `--contributors 101` is rejected before ANY
/// GitHub request (one page is at most 100).
///
/// @us-cpi-001 @driving_port @real-io @od-cpi-7 @error
#[test]
fn more_than_one_hundred_contributors_is_rejected_before_asking_github() {
    // GIVEN any repo.
    let env = TestEnv::initialized();

    // WHEN Maria asks for 101.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep", "--contributors", "101"],
        repo_with("BurntSushi/ripgrep", ripgrep_contributors()),
        None,
        "",
    );

    // THEN a usage error naming the flag + bound, and GitHub was never asked.
    assert_ne!(scrape.outcome.status, 0);
    assert!(
        scrape.outcome.stderr.contains("--contributors"),
        "{}",
        scrape.outcome.stderr
    );
    assert!(
        scrape.outcome.stderr.contains("100"),
        "{}",
        scrape.outcome.stderr
    );
    assert!(
        scrape.seen_paths.is_empty(),
        "no request may be made: {:?}",
        scrape.seen_paths
    );
}

/// CL-12 (error, UC-3): `--contributors` on a PERSON target is a usage error
/// before any request (a person scrape never crawls, D-4).
///
/// @us-cpi-001 @us-cpi-005 @driving_port @real-io @uc-3 @error
#[test]
fn asking_for_contributors_of_a_person_is_rejected_before_asking_github() {
    // GIVEN a public GitHub user.
    let env = TestEnv::initialized();

    // WHEN Maria passes --contributors to a person scrape.
    let scrape = scrape_github(
        &env,
        &["BurntSushi", "--contributors", "5"],
        FakeGithub::for_public_user("BurntSushi"),
        None,
        "",
    );

    // THEN usage error explaining the flag applies to `owner/repo` targets
    // only (not merely "unknown flag"), and no request made.
    assert_ne!(scrape.outcome.status, 0);
    assert!(
        scrape.outcome.stderr.contains("--contributors")
            && scrape.outcome.stderr.contains("owner/repo"),
        "the refusal must say --contributors applies to owner/repo targets;\n{}",
        scrape.outcome.stderr
    );
    assert!(
        scrape.seen_paths.is_empty(),
        "no request may be made: {:?}",
        scrape.seen_paths
    );
}

/// CL-13 (edge, DDD-3/DDD-15 gold fixture): GitHub's lies are corrected —
/// rows arrive out of commit order, a `renovate[bot]` login is typed "User",
/// a `Some-Tool[BOT]` login is upper-case, and one person appears twice.
/// Recorded: humans only, ranked by commits (ties by login), one row each.
///
/// @us-cpi-001 @driving_port @real-io @ddd-3 @ddd-15 @kpi-cpi-5 @edge
#[test]
fn the_recorded_people_are_humans_ranked_by_commits_whatever_order_github_uses() {
    // GIVEN GitHub serves an unsorted list with disguised bots and a duplicate.
    let env = TestEnv::initialized();
    let rows = vec![
        FakeContributor::human("zed", 3, 50),
        FakeContributor::bot_typed_as_user("renovate[bot]", 900, 400),
        FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 300),
        FakeContributor::bot("Some-Tool[BOT]", 901, 250),
        FakeContributor::human("amy", 2, 50),
        FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 300),
    ];

    // WHEN Maria scrapes the repo.
    let scrape = scrape_github(&env, &["acme/lies"], repo_with("acme/lies", rows), None, "");

    // THEN BurntSushi #1, amy #2 (tie broken by login), zed #3; both bots named.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    let people: Vec<(String, i64)> = links_for(&env, "github:acme/lies")
        .into_iter()
        .map(|l| (l.person, l.rank))
        .collect();
    assert_eq!(
        people,
        vec![
            ("github:BurntSushi".to_string(), 1),
            ("github:amy".to_string(), 2),
            ("github:zed".to_string(), 3)
        ]
    );
    assert!(
        out.stdout.contains("renovate[bot]") && out.stdout.contains("Some-Tool[BOT]"),
        "{}",
        out.stdout
    );
}

/// CL-14 (guardrail, KPI-CPI-5 / D-6): recording contributors costs exactly
/// ONE extra public request — the contributors list — compared with the same
/// scrape at `--contributors 0`; every request stays on the public allowlist.
///
/// @us-cpi-001 @driving_port @real-io @kpi-cpi-5 @d-6 @guardrail
#[test]
fn recording_contributors_costs_exactly_one_extra_public_request() {
    // GIVEN two fresh stores and the same repo.
    let with_people = TestEnv::initialized();
    let without_people = TestEnv::initialized();

    // WHEN Maria scrapes it with the default and with --contributors 0.
    let default = scrape_github(
        &with_people,
        &["rust-lang/cargo"],
        repo_with_all_signals("rust-lang/cargo", regex_contributors()),
        None,
        "",
    );
    let zero = scrape_github(
        &without_people,
        &["rust-lang/cargo", "--contributors", "0"],
        repo_with_all_signals("rust-lang/cargo", regex_contributors()),
        None,
        "",
    );

    // THEN the only difference is one contributors read.
    assert_eq!(default.outcome.status, 0, "{}", default.outcome.stderr);
    assert_eq!(zero.outcome.status, 0, "{}", zero.outcome.stderr);
    assert_eq!(
        default.seen_paths.len(),
        zero.seen_paths.len() + 1,
        "{:?} vs {:?}",
        default.seen_paths,
        zero.seen_paths
    );
    assert_eq!(default.contributor_requests(), 1);
    assert!(
        default.seen_paths.iter().all(|p| p.starts_with("/repos/")),
        "{:?}",
        default.seen_paths
    );
}

/// CL-15 (production data, live GitHub — slice-01 brief, R-1 riskiest
/// assumption): scraping the real `rust-lang/regex` then `BurntSushi/ripgrep`
/// shows BurntSushi bridging them; `dtolnay/anyhow --contributors 5` records
/// exactly 5 with dtolnay first.
///
/// @us-cpi-001 @real-io @requires_external @live-github @kpi-cpi-5
#[test]
#[ignore = "live-github: network-dependent production-data check; run with OPENLORE_LIVE_GITHUB=1 cargo test -p cli --test contributor_links -- --ignored live"]
fn live_real_repos_share_burntsushi_and_anyhow_core_is_dtolnay_first() {
    if std::env::var("OPENLORE_LIVE_GITHUB").is_err() {
        eprintln!("skipped: set OPENLORE_LIVE_GITHUB=1 to run against api.github.com");
        return;
    }
    let env = TestEnv::initialized();
    let regex = openlore_with(
        &env,
        &["scrape", "github", "rust-lang/regex"],
        Invocation::default(),
    );
    assert_eq!(regex.status, 0, "{}\n{}", regex.stdout, regex.stderr);
    let ripgrep = openlore_with(
        &env,
        &["scrape", "github", "BurntSushi/ripgrep"],
        Invocation::default(),
    );
    assert_eq!(ripgrep.status, 0, "{}\n{}", ripgrep.stdout, ripgrep.stderr);
    assert!(
        shows_overlap(&ripgrep.stdout, "BurntSushi", "rust-lang/regex"),
        "R-1: real overlap expected;\n{}",
        ripgrep.stdout
    );
    assert!(links_for(&env, "github:BurntSushi/ripgrep")
        .iter()
        .all(|l| !l.person.to_ascii_lowercase().ends_with("[bot]")));

    let anyhow = openlore_with(
        &env,
        &["scrape", "github", "dtolnay/anyhow", "--contributors", "5"],
        Invocation::default(),
    );
    assert_eq!(anyhow.status, 0, "{}\n{}", anyhow.stdout, anyhow.stderr);
    let links = links_for(&env, "github:dtolnay/anyhow");
    assert_eq!(links.len(), 5);
    assert_eq!(links[0].person, "github:dtolnay");
}
