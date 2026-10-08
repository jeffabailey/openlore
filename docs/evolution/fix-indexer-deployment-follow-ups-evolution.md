# Evolution: fix-indexer-deployment-follow-ups

- **Type**: bugfix delivery (3 steps, one phase), brownfield indexer and deploy scripts
- **Dates**: 2026-10-08: RCA and roadmap (`6316916`) through the mutation report (`309acac`)
- **Origin**: the open follow-ups of
  [`indexer-deployment-evolution.md`](indexer-deployment-evolution.md) (L3, L5, tautological
  oracles, review-app `render-secrets.sh`).
- **Records**: `docs/feature/fix-indexer-deployment-follow-ups/` (`rca.md` with the root causes, user
  decisions and known gaps, `deliver/roadmap.json`, `deliver/execution-log.json`,
  `deliver/mutation/mutation-report.md`).
- **State**: code complete, **not deployed**. No new crates, no schema change, no new alarm. ADR-080,
  the indexer-deployment architecture and data models, and the A3 observability and alerting docs
  amended.

## Summary

A search that could not read the index answered 500 and paged nobody. The host health line now
counts a search 500 as not live (A3), and the binary logs `indexer.search.store_error`. The
review app's `render-secrets.sh` replaces each file in place instead of swapping the bind-mounted
directory. Four self-referential test oracles now use literal tables. D1 (`PURGE_UNLISTED=0`
refused) was not a defect; the evolution note that called it one was corrected.

## Defects, root causes and fixes

Order D2 → D4 → D3 (D1 was doc only and went in with D2). Each code step began with a regression
test that failed against the code at the time.

| Defect | Root cause | Regression tests (RED evidence) | Fix | Commit |
|---|---|---|---|---|
| **D1**: `PURGE_UNLISTED=0` refused | Not a defect. ADR-082 and data-models §14 accept only `1` or unset; the code, AS-55 and CORE-8 match. The evolution note misstated the ADR. | None (doc only) | Evolution note corrected | `148e63e` |
| **D2**: search failures never paged | The `not_live` formula used only running / healthz / summary age / DID-list age. The search probe already ran every 2 min, but its result went only into the log line. `/healthz` reports only a poisoned mutex, and every other store error became a 500 with no log event. ADR-080 §7 promised more than the code did. The XP-13 CI stub `curl` always succeeded, so no test could fail the search. | `a_search_500_makes_the_host_not_live`, `a_busy_throttled_or_timed_out_search_does_not_make_the_host_not_live` (503 / 429 / 408 / no answer stay `not_live 0`). `a_search_that_cannot_read_the_index_logs_a_store_error_without_the_query` and the property `a_store_error_is_reported_once_with_the_dimension_and_never_the_value`. | The probe captures the HTTP status (no `curl -f`), and `not_live` adds `search_status == 500`. 503 is the busy reply during a pass or purge, so it does not count; A3's 2-of-2 rule absorbs one blip. The binary emits `indexer.search.store_error` with the dimension only, never the query; it does not page. Formula text updated in the script header, observability design, monitoring-alerting (A3) and ADR-080. | `148e63e` |
| **D4**: tautological oracles | The TEST_FAULT property took generator and oracle from the code under test. CORE-4 `list_oracle` mirrored `entries_of` / `with_new`, and `sut_read_did_list` dropped `problem`. In CORE-5 the harness chose which failure won. The control round-trips only checked that encode and decode agree. | Each new literal oracle was shown failing against a planted wrong implementation: `every_test_fault_has_a_literal_row`, `the_did_list_examples_load_or_refuse_exactly` (keeps `problem`), `a_pass_s_exit_code_puts_local_failures_before_outages_before_success`, `the_wire_text_of_every_request_and_reply_is_fixed` (`{"request":"run_pass"}` etc.). | Literal tables replace the mirrored oracles (test only). CORE-5 was rescoped to one failure per call (see Reviews). | `13bd6de` |
| **D3**: `render-secrets.sh` swapped the directory | Compose bind-mounts `/pds/app/secrets` read-only. The script renamed a new directory into place and deleted the old one, so a running container kept the old, now deleted, inode. Both callers force-recreate right after rendering, but if `docker pull` failed after a render, the old container ran against an empty `/run/secrets`. | `two_renders_replace_secret_files_and_never_the_directory` (inode-based; RED), `a_render_missing_a_required_secret_changes_nothing` | Per-file write-then-rename in the same directory (as `render-dids.sh` does). Stale files are removed, and the directory is never swapped. Rotation stays restart-based. | `18f0e01` |

Each step also has a `chore: execution log` commit (`67f6b2b`, `36d5995`, `55c5ffe`).

## User decisions (2026-10-08)

- D1: no behaviour change; correct the evolution note.
- D2: a search 500 (not 503 / 429 / 408) counts toward A3 `not_live`, plus a structured
  store-error log event. No new alarm.
- D3: include the per-file render-secrets hardening in this delivery.
- D4: fix all four oracles.

## Reviews

- **Roadmap (full model)**: NEEDS_REVISION, then approved. CORE-5 could not be implemented as
  written: passing every failure to the code under test needs a multi-fault seam that does not
  exist. The orchestrator rescoped it to one failure per call. Two more findings: `search_ok` was
  undefined without `curl -f`, and the D3 guard had to compare the full directory listing.
- **Implementation (full model)**: NEEDS_REVISION; the code was correct. Fixed in `5f2f5a5`: M1
  (a get-parameter failing on the 2nd SSM parameter leaves the full listing, dotfiles included,
  unchanged and no temp file behind), L1 (A3 docs and tofu text; stale `indexer_live = 0` →
  `not_live = 1`), L2 (partial-rename convergence documented), L3 (health lines parsed with
  `serde_json` and asserted by value), L4 (exactly one store-error event, asserted after the log
  settles). L6 in `ce0c902`: the CORE-5 comment no longer claims precedence coverage. The rescope
  had assumed acceptance tests covered precedence, but none did.

## Quality gates

| Gate | Result |
|---|---|
| Roadmap review (full model) | NEEDS_REVISION → approved |
| DES integrity | All 3 steps have complete DES traces |
| Adversarial review (full model) | NEEDS_REVISION → M1, L1-L4, L6 fixed |
| Mutation (in-diff vs `6316916`, gate 80%) | `search_handler.rs` + `run.rs`: 11/11 viable caught (**100%**), 2 unviable. `config.rs` / `control.rs` changes were test-only; shell is covered by xtask tests |
| CI | Green at `6316916`; later commits being pushed at finalize |

## Lessons

- **A rescope must verify the coverage it relies on.** The orchestrator narrowed CORE-5 on the
  assumption that acceptance tests already pinned "first failure ends the pass". None did, and the
  review caught the false comment. **Lesson:** when a rescope hands coverage elsewhere, name the
  test that provides it, or record the gap.
- **A CI stub that always succeeds hid D2.** The XP-13 `curl` stub returned success for every
  probe, so the search part of the health line could never fail in any test. **Lesson:** a stub on a
  monitored path needs a failing mode, and a test that uses it.
- **Mutation tool flags can be silently ignored.** cargo-mutants 25.3.1 ignored `--test-package` /
  `--test-workspace` and tested only `openlore-indexer`, whose unit tests cannot reach the `run.rs`
  emitters. A temporary `CARGO` wrapper ran the `cli` acceptance suites against the mutated binary.
  **Lesson:** confirm from the baseline which tests actually ran before trusting a kill rate.
- **Tautological oracles survive mutation fixes.** Killing the mutants with concrete tests left the
  self-referential properties in place. **Lesson:** a review checklist item: "does the oracle call or
  mirror the code under test?"

## Open follow-ups (not fixed here)

- **Pass-failure precedence** ("the first failure ends the pass", `ingest_pass.rs`) is pinned by no
  test. It needs a multi-fault seam; `with_fault` takes one fault.
- **No check-arch rule for the indexer's `serde_json` event fields**; the struct type is the only
  guard against logging the query text.
- `OPENLORE_INDEXER_TEST_FAULT` trims, and blank means unset, in dev builds (intended, documented).
- Queued as separate `/nw:refactor` tasks (user choice): remove the redundant `send_idempotent`
  retry; remove stale `SCAFFOLD` headers (indexer-deployment L4); share DuckDB cap parsing between
  the indexer and the review app.
- Ops backlog: narrow `TRUSTED_PROXIES` to the actual `pds_default` subnet; `deploy.sh` `measure`,
  `kpi` and `test-alarm` modes.
