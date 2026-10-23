# fix-indexer-summary-race — evolution

Delivered 2026-10-23. Bug fix through /nw:bugfix. The RCA is in `docs/feature/fix-indexer-summary-race/rca.md`.

## Defect

Under host load, `indexer_deployment_did_list::a_purge_that_fails_ends_the_pass_with_exit_2_and_finishes_on_the_next_pass` failed with a wrong summary `exit_code`. The acceptance harness read serve's last collected pass summary as soon as `trigger` exited. A separate thread collects serve's stdout, so the summary it read was sometimes the previous pass's.

## Fix

| Step | Commit | Change |
|---|---|---|
| 01-01 | d0860bb | `trigger` prints `indexer.trigger.completed` (`pass_id`, `exit_code`) for a completed pass. Exit codes are unchanged. Amended ADR-080 and updated the data-models event table. |
| 01-02 | 3f0493c | The harness reads that `pass_id`, waits up to `TRIGGER_CEILING` for that pass's summary, then asserts exactly one summary with a matching `exit_code`. `timer_fires`, `trigger_with_only` and `PendingPass::finish` wait the same way. Added a test-only lag knob (`Deployment::with_collector_lag`, `OPENLORE_TEST_COLLECTOR_LAG_MS`). Refused startup now joins the collectors instead of sleeping 100 ms. The search-truncated test waits for its event. |

## Evidence

- The regression test `a_lagged_summary_stream_still_checks_the_triggered_pass_s_own_summary` (150 ms lag) failed on the old harness and passes now.
- All 54 indexer acceptance tests pass, both normally and with a 150 ms lag on every test. Before the fix, 16 of the 19 did_list tests failed under that lag.
- Full cli suite: 1192 passed, 9 skipped, 0 failed.
- DES integrity: both steps have complete traces.

## Notes

The lag knob is how to check any new indexer acceptance test for this race: run it with `OPENLORE_TEST_COLLECTOR_LAG_MS=150`. The refactor, review and mutation phases were skipped for this two-step fix. It has a small product change, which a proptest covers, plus test-harness changes.
