# RCA: indexer acceptance tests read a stale pass summary under load

Date: 2026-10-23. Reported as: `indexer_deployment_did_list::a_purge_that_fails_ends_the_pass_with_exit_2_and_finishes_on_the_next_pass` failed once in a full `cargo nextest run -p cli -j 4` run (`the summary's exit_code` assertion) and passed when re-run at `-j 2`.

## Root cause

The defect is in the test harness, not the indexer. `assert_one_summary_matching` (`tests/acceptance/support/indexer_live.rs:810`) reads `live.pass_summaries()` once, as soon as the `trigger` subprocess exits. Serve's stdout is copied into that list by a separate collector thread (`spawn_line_collector`, `indexer_live.rs:285`), and nothing orders that thread against `trigger` exiting. Serve does write the summary before it replies on the socket (`ingest_pass.rs:39-54` → `pass_runner.rs:300` → `control.rs:245`). But when the collector runs late, `summaries.last()` is the previous pass's summary.

`trigger` discards the reply's `pass_id` (`control.rs:121`, `Completed { exit_code, .. }`). Without it, the harness has no way to name the summary it should wait for.

## Evidence

- Original failure: the trigger's exit code was right and the summary's `exit_code` was wrong. This test's passes exit 0, then 2, then 0, so a stale summary always shows a different code.
- Delaying each line of serve's stdout by 150 ms with a wrapper (`CARGO_BIN_EXE_openlore-indexer=<lag wrapper>`) made this test fail 4 of 4 times, and 16 of the 19 tests in the file failed. CPU load alone did not reproduce it.
- The purge fault is a deterministic AtomicBool (`pass_runner.rs:424`). The 30 s per-DID budget and the 1500 s deadline are nowhere near the test's 2 s runtime.

## Exposure

Every caller of `given_a_pass_indexed` or `assert_one_summary_matching` is exposed:

- `indexer_deployment_did_list.rs`
- `indexer_deployment_passes.rs`
- `indexer_deployment_public_surface.rs`
- `indexer_deployment_walking_skeleton.rs`

Some of them can also pass when they should fail. "Exactly one summary per pass_id" and "event X is absent" both still hold on a stale read.

## Fix (user-approved 2026-10-23: match by pass_id)

1. `trigger` keeps the `pass_id` from the reply. For a completed pass it prints `{"event":"indexer.trigger.completed","pass_id":…,"exit_code":…}` to stdout. Its exit code does not change.
2. The harness reads that `pass_id`. It waits, up to `TRIGGER_CEILING`, until a summary with that `pass_id` has been collected. This waits for a named event; it is not a longer sleep. It then asserts there is exactly one such summary and that its `exit_code` matches.
3. A coalesced trigger and an unreachable trigger do not wait.
