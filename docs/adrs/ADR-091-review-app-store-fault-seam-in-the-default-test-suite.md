# ADR-091: Review App Store Fault Seam Runs in the Default Test Suite

- **Status**: Accepted (2026-11-21)
- **Date**: 2026-11-21
- **Deciders**: Jeff Bailey
- **Feature**: fix-repos-read-truthful (derogation asked by its whole-candidate review, finding `test:feedback-loop-budget`)
- **Builds on**: the existing `migrate_before_commit` test fault of the review app

## Context

Two acceptance suites of the review app cause a store write to fail through the
development fault seam `REVIEW_APP_TEST_FAULT`:

- `review_app_scan_store_failure` (`refuse_mark_repo_read`, `refuse_record_account_checks`);
- `review_app_legacy_run_repos_read` (no fault; it shares the same support).

The seam exists only in a debug build (`cfg(debug_assertions)`); a release build
ignores the variable. The behaviour these suites check (a refused store write
ends the scan interrupted, never completed with an untrue count) cannot be
caused from the public `serve` command in any other way.

The test design rules ask for adapter fault injection to live outside the
default suite unless a recorded decision accepts its cost.

## Decision

The store fault seam family (`migrate_before_commit`, `refuse_mark_repo_read`,
`refuse_record_account_checks`, and later faults of the same kind) runs in the
default `cargo test` suite. No separate infrastructure suite is made.

## Consequences

- Measured cost on the development Mac (2026-11-21): 4.6 s for the 4 tests of
  `review_app_scan_store_failure` and 1.7-2.9 s for the 6 tests of
  `review_app_legacy_run_repos_read`. CI runs them on every push.
- A new fault of this family does not need a new ADR if its suite stays within
  a few seconds. A fault suite that adds much more time to the default run
  needs a new decision.
- The release build never reads `REVIEW_APP_TEST_FAULT`; the config property
  `only_a_development_build_takes_the_test_fault_seam` and the xtask guard
  `review_app_test_fault_guard` keep it so.
