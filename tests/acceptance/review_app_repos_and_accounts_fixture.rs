//! review-app-repos-and-accounts — the frozen v1 store regenerator. NOT a
//! scenario and NOT part of any scenario run: it lives in its own test target
//! so `--run-ignored only` on the scenario suites never reaches it.
//!
//! It writes `tests/fixtures/review-app-store-v1/store.regenerated/` and
//! `pending.regenerated.txt` from the CURRENT app binary — it never touches
//! the committed `store/` and `pending.txt`; move the regenerated files in by
//! hand after reviewing them. It refuses unless the store it made is schema
//! v1 (checked with the `duckdb` CLI, which it requires) and the app's cards
//! name no source account, so it only works on a commit before DELIVER 01-16.
//!
//! `OPENLORE_REGEN_REVIEW_STORE_V1=1 cargo test -p openlore-review-app --test review_app_repos_and_accounts_fixture -- --ignored`

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

#[test]
#[ignore = "fixture regenerator, not a scenario: OPENLORE_REGEN_REVIEW_STORE_V1=1 on a binary whose store is v1"]
fn regenerate_the_frozen_v1_store_fixture() {
    if std::env::var("OPENLORE_REGEN_REVIEW_STORE_V1").is_err() {
        eprintln!("skipped: set OPENLORE_REGEN_REVIEW_STORE_V1=1 on a pre-v2 commit");
        return;
    }
    let declined = DRIFTWOOD.embodies(Philosophy::DependencyPinning);
    let published = KELP.embodies(Philosophy::DependencyPinning);
    let world = rra_world();
    let mut browser = given_signed_in(&world, Persona::Priya);
    when_verifies_github(&mut browser, Account::Priyaraman.login());
    then_sees_account_verified(&browser.page, Account::Priyaraman, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    when_declines(&mut browser, declined);
    when_previews_approval(&mut browser, published);
    when_confirms_publish(&mut browser);
    assert!(browser.page.shows("at://"), "{}", browser.page.text());
    let pending = pending_on_queue(&mut browser);
    assert!(
        !pending.is_empty()
            && queue_cards(&mut browser)
                .iter()
                .all(|c| !c.text.contains("from github.com/")),
        "the fixture must be made by a v1 app (its cards name no source account)"
    );

    let ReviewWorld { app, .. } = world;
    let state = app.stop_keeping_state();
    let scratch = tempfile::tempdir().expect("scratch dir");
    for entry in std::fs::read_dir(state.path().join("data"))
        .expect("data dir")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("review-app.duckdb") {
            std::fs::copy(entry.path(), scratch.path().join(&name)).expect("copy store file");
        }
    }
    let version = std::process::Command::new("duckdb")
        .args(["-readonly", "-csv", "-noheader"])
        .arg(scratch.path().join("review-app.duckdb"))
        .arg("SELECT max(version) FROM schema_version")
        .output()
        .expect("the regenerator needs the duckdb CLI on PATH");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "1",
        "the regenerated store must be schema v1: {}",
        String::from_utf8_lossy(&version.stderr)
    );

    let target = v1_store_fixture_dir();
    let out = target.join("store.regenerated");
    std::fs::create_dir_all(&out).expect("store.regenerated dir");
    for entry in std::fs::read_dir(scratch.path())
        .expect("scratch")
        .flatten()
    {
        std::fs::copy(entry.path(), out.join(entry.file_name())).expect("copy out");
    }
    std::fs::write(
        target.join("pending.regenerated.txt"),
        pending.join("\n") + "\n",
    )
    .expect("pending.regenerated.txt");
    eprintln!(
        "wrote {} and pending.regenerated.txt — review, then move them over store/ and pending.txt",
        out.display()
    );
}
