//! The CLI and the viewer share one store AT THE SAME TIME.
//!
//! DuckDB lets one process hold the store file at a time. Both processes
//! open it per store operation and close it after (`adapter-duckdb`'s
//! `SharedConn`), so a running `openlore ui` no longer locks out CLI verbs,
//! and the viewer shows what the CLI just wrote without a restart.
//!
//! Layer: subprocess acceptance — the REAL `openlore ui` child and REAL
//! `openlore` CLI children over ONE real DuckDB file; GitHub is the
//! `FakeGithub` double.

mod support;

#[allow(unused_imports)]
use support::*;

/// Given the viewer is serving the store;
/// When Maria signs a claim with `openlore claim add`;
/// Then the CLI succeeds AND the running viewer lists the new claim.
///
/// @driving_port @driving_adapter @real-io @happy
#[test]
fn a_claim_added_while_the_viewer_runs_shows_up_in_the_viewer() {
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start(&env);
    assert_eq!(viewer.get("/claims").status, 200, "the viewer serves first");

    // A newline confirms "sign locally"; EOF then declines the publish.
    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            "github:concurrent-org/shared-store",
            "--predicate",
            "embodiesPhilosophy",
            "--object",
            "org.openlore.philosophy.memory-safety",
            "--evidence",
            "https://github.com/concurrent-org/shared-store",
            "--confidence",
            "0.5",
        ],
        "\n",
    );
    assert_eq!(
        outcome.status, 0,
        "claim add must succeed while the viewer runs; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let page = viewer.get("/claims");
    assert_eq!(page.status, 200);
    assert!(
        page.body.contains("concurrent-org/shared-store"),
        "the running viewer must list the claim the CLI just signed; got:\n{}",
        page.body
    );
}

/// Given the viewer is serving the store (with its own live-scrape seam);
/// When Maria runs `openlore scrape github rust-lang/cargo` from the CLI;
/// Then the scrape succeeds and records the repo's contributors, and the
/// viewer keeps serving.
///
/// @driving_port @driving_adapter @real-io @happy
#[test]
fn a_cli_scrape_runs_while_the_viewer_serves() {
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start_with_github(
        &env,
        GithubServer::start(FakeGithub::for_public_repo_with_all_signals(
            "rust-lang/cargo",
        )),
    );
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals(
        "rust-lang/cargo",
    ));

    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );
    assert_eq!(
        outcome.status, 0,
        "a CLI scrape must succeed while the viewer runs; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    assert!(
        outcome.stdout.contains("Contributors recorded"),
        "the scrape must record contributors into the shared store; got:\n{}",
        outcome.stdout
    );
    assert_eq!(
        viewer.get("/claims").status,
        200,
        "the viewer keeps serving after the CLI wrote"
    );
}
