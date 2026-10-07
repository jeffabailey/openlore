# RED classification: indexer-deployment (DISTILL, 2026-10-06)

Pre-DELIVER fail-for-the-right-reason gate. Every new scenario was run once with `--ignored` against
the current binaries (`cargo build -p openlore-indexer -p cli -p openlore-review-app` at `7652daf`),
then left `#[ignore]`d. Classification per scenario:

- **MISSING_FUNCTIONALITY**: the assertion fires because the behavior is not implemented (correct RED).
- **RED_SCAFFOLD**: a layer-2 `sut_*` binding panics with `todo!("DELIVER …")` (correct RED, Mandate 7).
- BROKEN (import, fixture, setup) or WRONG_ASSERTION: none remain.

## Thin slice (walking skeleton) — the evidence

```
cargo test -p cli --test indexer_deployment_walking_skeleton -- --ignored
test maria_finds_claims_from_authors_on_different_pdses_after_the_scheduled_pass ... FAILED
  assertion failed: trigger exits with the pass's code
  exit 2 … stderr: error: unrecognized subcommand 'trigger'
  serve stdout: {"event":"indexer.config.loaded",…,"repo_did_count":0,…}
                {"addr":"127.0.0.1:49401","event":"indexer.serve.listening"}
test a_claim_priya_approves_after_a_pass_becomes_searchable_on_the_next_pass ... FAILED
  (same: no `trigger` verb)
test a_fresh_deployment_answers_before_its_first_pass ... FAILED
  assertion failed: healthy before the first pass  left: 404  right: 200
```

Reading: `serve` already starts in production posture (unknown new variables are ignored, the DID
list file is not read: `repo_did_count: 0`). The pass cannot be triggered (B3), and `/healthz` does
not exist (B8). WS-0's empty search already answers 200 with no results (AC-001.4's search half is
already true), but the scenario stays RED on the health half.

## Per-scenario classification

| Scenario | Classification | First failing assertion (observed) |
|---|---|---|
| WS-1, WS-2 | MISSING_FUNCTIONALITY | `trigger` exit 2: unrecognized subcommand (B3) |
| WS-0 | MISSING_FUNCTIONALITY | `GET /healthz` 404 (B8) |
| PS-10, PS-13, PS-23 | MISSING_FUNCTIONALITY | no pass started inside serve (no `config.loaded` with `pass_id`) (B2, B3) |
| PS-11, PS-12, PS-14, PS-16, PS-22, PS-24, PS-25, PS-26 | MISSING_FUNCTIONALITY | `trigger` exits 2, not the pass's code (B3) |
| PS-15 | MISSING_FUNCTIONALITY | `trigger` exit 2, expected 4 (B3) |
| PS-17, PS-18, PS-19 | MISSING_FUNCTIONALITY | no `pass_summary` from serve (B2, B4) |
| PS-20 | MISSING_FUNCTIONALITY | no `indexer.store.unusable` (B12, fault seam) |
| PS-21 | MISSING_FUNCTIONALITY | search answered `200 {"results":[]…}` instead of 500 (B12) |
| DL-30..34, DL-37, PG-36, PG-40..48 | MISSING_FUNCTIONALITY | `trigger` exits 2 at the first pass (B3); the list/purge assertions follow once B3 lands |
| DL-35 | MISSING_FUNCTIONALITY | serve started with both list settings (B5) |
| AS-50, AS-53, AS-54 | MISSING_FUNCTIONALITY | precondition pass: `trigger` exits 2 (B3) |
| AS-52 | MISSING_FUNCTIONALITY | 8 KiB + 1 body not refused with 413 (B8) |
| AS-55 | MISSING_FUNCTIONALITY | `OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB=15` accepted (B9) |
| AS-56 | MISSING_FUNCTIONALITY | `/healthz` 404 (B8) |
| CORE-1..11 (12 tests) | RED_SCAFFOLD | `not yet implemented: DELIVER 05-0x: bind to …` |
| RAC-1 | MISSING_FUNCTIONALITY | `REVIEW_DB_MEMORY_LIMIT_MB (15, 1) must be refused` (B11) |
| XD-1..6 | MISSING_FUNCTIONALITY | rule names absent from `check_arch.rs`; modules (`*search*`, `*runner*`, `*control*`, `purge.rs`) absent |
| XP-1..12 | MISSING_FUNCTIONALITY | the deploy files do not exist yet (`deploy/indexer/**`, `indexer.tf`, `indexer-iam.tf`, Dockerfile, CI jobs) |

One harness defect was found and fixed during this gate: DL-37 first failed with
`PermissionDenied` (setting the mtime of the 0444 list through a write handle). The helper now uses
a read-only handle; the scenario then failed for the right reason (`trigger` exit 2).

## Dependency note for DELIVER

Most layer-3 scenarios fail first at their precondition pass, because they need the `trigger` verb
(B3) before their own assertion can be reached. That is still a correct RED (the precondition is
the missing functionality), but each scenario's OWN assertion is only exercised after WS-1 is green.
When enabling a scenario, re-check that it fails on its own assertion before implementing (DELIVER
RED gate).

## Commit state

All 74 scenarios are `#[ignore = "DELIVER <step>: …"]`. The default runs are green:
`indexer_deployment_{walking_skeleton,passes,did_list,public_surface,core}`,
`review_app_resource_caps`, `xtask` (all tests, including the 2 always-on helper self-tests).
