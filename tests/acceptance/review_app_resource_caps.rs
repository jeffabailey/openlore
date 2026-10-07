//! indexer-deployment B11 (DISTILL 2026-10-06; user decision, in scope) — the
//! review app's DuckDB is capped at 48 MB and 1 thread from its own config,
//! read back by its probe, so it shares the 1 GiB PDS host with the indexer
//! (architecture-design.md §3, R-IXD-D4: the documented "64 MB, one thread" was
//! never set anywhere).
//!
//! Driving port: the REAL `openlore-review-app serve` (startup + `/healthz`).
//! Layer 4, example-only. The cap values and their CI posture (compose env,
//! `init: true`, 192m) are asserted against the real deploy files in
//! `xtask/tests/indexer_deployment_platform.rs`.
//!
//! ## Configuration contract (DISTILL-proposed; DELIVER owns the final names)
//!
//! Review-app config names follow its own convention (`REVIEW_DB`): the caps
//! are `REVIEW_DB_MEMORY_LIMIT_MB` (16..=1024) and `REVIEW_DB_THREADS` (1..=4),
//! the same ranges as the indexer's (data-models.md §1). Renaming is a one-line
//! change in [`MEMORY_LIMIT_MB`] / [`THREADS`].
//!
//! `#[ignore]`d at DISTILL hand-off.

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::*;

const MEMORY_LIMIT_MB: &str = "REVIEW_DB_MEMORY_LIMIT_MB";
const THREADS: &str = "REVIEW_DB_THREADS";

fn with_caps(memory: &str, threads: &str) -> impl FnOnce(&mut AppSettings) {
    let (memory, threads) = (memory.to_string(), threads.to_string());
    move |s: &mut AppSettings| {
        s.extra_env.push((MEMORY_LIMIT_MB.to_string(), memory));
        s.extra_env.push((THREADS.to_string(), threads));
    }
}

/// RAC-1
/// ```gherkin
/// @US-IXD-006 @AC-006.4 @AC-006.5 @NFR-IXD-4 @B11 @C1b @C6a @error @real-io @driving_port @contract-shape:unbounded-preservation
/// Scenario Outline: The review app accepts database caps within their range and refuses the rest
///   Given Jeff sets the review app's database memory to <memory> MB and its threads to <threads>
///   When the review app starts
///   Then it <outcome>
///   Examples:
///     | memory | threads | outcome                                   |
///     | 48     | 1       | reports healthy (the production sizes)    |
///     | 16     | 4       | reports healthy (the limits)              |
///     | 1024   | 1       | reports healthy (the limits)              |
///     | 15     | 1       | refuses to start, naming the memory cap   |
///     | 1025   | 1       | refuses to start, naming the memory cap   |
///     | lots   | 1       | refuses to start, naming the memory cap   |
///     | 48     | 0       | refuses to start, naming the threads cap  |
///     | 48     | 5       | refuses to start, naming the threads cap  |
/// ```
#[test]
fn the_review_app_accepts_database_caps_within_their_range_and_refuses_the_rest() {
    for (memory, threads) in [("48", "1"), ("16", "4"), ("1024", "1")] {
        let (_gh, _net, startup) = ReviewWorld::launch(|_, _| {}, with_caps(memory, threads));
        let Startup::Ready(app) = startup else {
            panic!("the review app starts with its database at {memory} MB / {threads} thread(s)");
        };
        assert_eq!(app.get("/healthz").0, 200);
    }
    let cases = [
        (MEMORY_LIMIT_MB, "15", "1"),
        (MEMORY_LIMIT_MB, "1025", "1"),
        (MEMORY_LIMIT_MB, "lots", "1"),
        (THREADS, "48", "0"),
        (THREADS, "48", "5"),
    ];
    for (named, memory, threads) in cases {
        let (_gh, _net, startup) = ReviewWorld::launch(|_, _| {}, with_caps(memory, threads));
        match startup {
            Startup::Ready(_) => panic!("{named} ({memory}, {threads}) must be refused"),
            Startup::Refused { exit_code, logs } => {
                assert!(
                    exit_code.is_some() && exit_code != Some(0),
                    "{named}: non-zero exit"
                );
                assert!(logs.contains("health.startup.refused"), "{named}\n{logs}");
                assert!(logs.contains(named), "the refusal names {named}\n{logs}");
            }
        }
    }
}
