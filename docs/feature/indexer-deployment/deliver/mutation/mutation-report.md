# Mutation testing — indexer-deployment (DELIVER Phase 5)

Tool: cargo-mutants 25.3.1, `--timeout-multiplier 3 -j 2`. Gate: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL.
Kill rate = caught / (caught + missed + timeout). Unviable mutants (they do not compile) are left out.
Scope: only this feature's changes are mutated (`--in-diff` against `30cc3f7`). Each crate is measured with its own in-crate tests. The workspace acceptance tests (`tests/acceptance/*`) are not run by `-p <crate>`, as in the indexer-per-did-pds-fetch precedent.

## Summary

| Target | Run | Viable | Caught | Missed | Timeout | Unviable | Kill rate | Gate |
|---|---|---|---|---|---|---|---|---|
| `appview-domain` (did_list, pass_runner, purge_plan, health, ingest_pass) | initial | 55 | 16 | 39 | 0 | 12 | 29.1% | FAIL |
| `appview-domain` | after new tests | 55 | 55 | 0 | 0 | 12 | **100%** | **PASS** |
| `adapter-xrpc-query-server` (lib.rs, rate_limit.rs) | initial | 83 | 66 | 17 | 0 | 110 | 79.5% | WARN |
| `adapter-xrpc-query-server` | after new tests | 83 | 81 | 2 | 0 | 110 | **97.6%** | **PASS** |
| `openlore-indexer` (config.rs, control.rs) | initial | 54 | 29 | 25 | 0 | 11 | 53.7% | FAIL |
| `openlore-indexer` | after new tests | 54 | 53 | 1 | 0 | 11 | **98.1%** | **PASS** |
| `openlore-review-app` (config.rs caps), advisory | initial | 3 | 3 | 0 | 0 | 1 | 100% | PASS |

All re-runs were full runs over each in-diff scope; no targeted `-F` re-runs were needed. The 3 surviving mutants are equivalent or not observable in-process (see below).
No production behaviour changed. The only production-file edit appends a `#[cfg(test)] #[path = "control_contracts.rs"] mod contracts;` declaration to control.rs. The indexer is a binary-only crate, so its tests must live inside it.

New tests:
- `crates/appview-domain/tests/deployment_contracts.rs` (commit `d237fd3`): the DID list read on concrete lists (separators, CRLF, de-duplication in first-seen order, empty and blank lists, the 2048/2049-byte edge, each malformed shape and its `EntryProblem`, the BOM, the first bad entry winning); the `describe` texts; the purge plan by bare DID, including a look-alike prefix; the empty-list suppression and its token; `PassId::next`; the single-flight walk through every reply; `within_deadline` exactly at the deadline; every `PassFailure` token; the `/healthz` status and body for each store state.
- `crates/adapter-xrpc-query-server/tests/public_answers.rs` and `rate_limit_contracts.rs` (commit `84789aa`): `/healthz` body over a socket; a body over 8192 bytes is 413; a body cut short is 400, not 413; an unreadable index is 500 `index_unavailable`; `admit` at its exact edges; `is_full_at` before and after one second; Retry-After of a half-refilled bucket; the limiter forgets nobody under capacity, and when full forgets the refilled client but keeps the active one.
- `crates/openlore-indexer/src/control_contracts.rs` and additions to `config_contracts.rs` (commit `03bfaff`): `Unreachable` tokens; blank or relative socket → `socket_not_configured`, missing absolute socket → `connect_failed`; `trigger(None)` exits 4; `bind` replaces a stale socket or a leftover file and sets 0600; a directory is never removed, a missing path is fine, and other errors are reported; a probe answered by a mute socket fails; a running `serve` answers its own probe and a trigger receives the pass's own exit code (3); each `TestFault` token parses to its fault; a list file alone is the list source.

## Tautological / self-referential oracles

- **Found: `openlore-indexer` config.rs `tests`, the `OPENLORE_INDEXER_TEST_FAULT` property (around config.rs:753–762).** It decides the expected outcome with `TestFault::from_token(&value)`, which is the function under test. So `from_token → None` and `== → !=` survived (both sides of the comparison change together). The new test names each fault's token as a literal.
- **CORE-4 (`tests/acceptance/indexer_deployment_core.rs`, `list_oracle`)** is not tautological: it is an independent re-implementation and never calls `read_did_list`. However, it (a) maps every refusal to `Malformed(entry)`, so a mutant that swaps `NotADid`, `NotWellFormed` and `NamesNoRepo` passes it; (b) mirrors the implementation's structure line for line, so a shared misreading of the spec would pass both; and (c) lives outside the crate, so `-p appview-domain` never ran it. `did_list.rs` had no in-crate test, which explains 25 of the 39 initial survivors. The new tests check concrete lists with hand-written expected values, including the problem kind.
- The rate-limit trusted-proxy property uses `peer.is_loopback()` (std) as part of its oracle, not production code. It is acceptable.

## 1. `crates/appview-domain`

Command: `cargo mutants -p appview-domain --in-diff <(git diff 30cc3f7..HEAD -- crates/appview-domain) --timeout-multiplier 3 -j 2`

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| did_list.rs:52 `read_did_list` → `Ok(vec![])` | 1 | killed-by-new-test |
| did_list.rs:61–62 `entries_of` (→empty/once ""/once "xyzzy"; `==`→`!=`; `\|\|`→`&&`; delete `!`) | 6 | killed-by-new-test |
| did_list.rs:67 `with_new` (→`vec![]`; delete `!`) | 2 | killed-by-new-test |
| did_list.rs:87–99 `entry_problem` (→None; `>`→`<`/`==`/`>=`; `\|\|`→`&&` ×3; delete `!` ×3) | 10 | killed-by-new-test |
| did_list.rs:108–112 `did_identifier_valid` (→true/false; delete `!` ×2; `&&`→`\|\|` ×2; `\|\|`→`&&`) | 7 | killed-by-new-test |
| did_list.rs:35 `EntryProblem::describe` (→""/"xyzzy") | 2 | killed-by-new-test |
| health.rs:45 `status_code` (→0/1), :53 `body` → `Default` | 3 | killed-by-new-test |
| ingest_pass.rs:431 `PassFailure::token` (→""/"xyzzy") | 2 | killed-by-new-test |
| ingest_pass.rs:491 `within_deadline` `>`→`>=` (the property rarely hits equality) | 1 | killed-by-new-test |
| pass_runner.rs:20 `PassId::next` `+`→`-`/`*` | 2 | killed-by-new-test |
| purge_plan.rs:47 `PurgeSuppressed::token` (→""/"xyzzy"), :76 delete `!` in `plan_purge` | 3 | killed-by-new-test |

`single_flight` was fully killed by the existing in-file tests.

## 2. `crates/adapter-xrpc-query-server`

Command: `cargo mutants -p adapter-xrpc-query-server --in-diff <(git diff 30cc3f7..HEAD -- crates/adapter-xrpc-query-server) --timeout-multiplier 3 -j 2`

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| lib.rs:166, :168 `admit` `>`→`>=` (body and value edges) | 2 | killed-by-new-test |
| lib.rs:402 `search` LengthLimitError guard (→false: 413 becomes 400; →true: a short body becomes 413) | 2 | killed-by-new-test |
| lib.rs:437 `health_answer`, :452 `too_large`, :469 `index_unavailable` → empty 200 | 3 | killed-by-new-test |
| rate_limit.rs:114 `is_full_at` (→true/false; `>=`→`<`) | 3 | killed-by-new-test |
| rate_limit.rs:140 `take` `-`→`+` (Retry-After of a partly refilled bucket) | 1 | killed-by-new-test |
| rate_limit.rs:271 `check` `&&`→`\|\|`; :286 `tracked_clients` (→0/1); :301 `make_room` delete `!` | 4 | killed-by-new-test |
| rate_limit.rs:141 `take` `*`→`+` in `per_sec * TOKEN` | 1 | **equivalent**: `missing ≤ TOKEN` (1000) and both `per_sec·1000` and `per_sec+1000` are ≥ 1000, so `missing.div_ceil(divisor)` is 1 either way, and `.max(1)` gives 1 s. Retry-After is always 1 s. |
| rate_limit.rs:156 `TrustedProxies::loopback_only` → `Default::default()` | 1 | **equivalent**: the body is `Self::default()`. |

## 3. `crates/openlore-indexer` (config.rs, control.rs)

Command: `cargo mutants -p openlore-indexer --in-diff <(git diff 30cc3f7..HEAD -- crates/openlore-indexer/src/config.rs crates/openlore-indexer/src/control.rs) --timeout-multiplier 3 -j 2`

The pure protocol (encode/decode, `trigger_outcome_of`, `trigger_exit_code`), the settings ranges, trusted proxies and the socket-path rule were already fully killed. The survivors were tokens and the socket effect shell.

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| config.rs:118 `TestFault::token` → "xyzzy"; :127 `from_token` (→None; `==`→`!=`) | 3 | killed-by-new-test (tautological property, see above) |
| config.rs:402 `list_file` → `Ok(Default)` | 1 | killed-by-new-test |
| control.rs:86 `Unreachable::token` (→""/"xyzzy") | 2 | killed-by-new-test |
| control.rs:201–204 `remove_stale_socket` (→Ok; both guards →true/false; `\|\|`→`&&`; `==`→`!=`) | 7 | killed-by-new-test |
| control.rs:212 `answer_triggers` (→(); delete `!`), :227 `answer` → () , :246 `run_pass` → None | 4 | killed-by-new-test |
| control.rs:265 `probe_round_trip` → Ok, :278 `exchange` → Ok(None) | 2 | killed-by-new-test |
| control.rs:292 `trigger` (→-1/0/1) | 3 | killed-by-new-test |
| control.rs:301 `request_one_pass` (`\|\|`→`&&`; delete `!`) | 2 | killed-by-new-test |
| control.rs:315 `report` → () | 1 | **not observable in-process**: it only writes the `indexer.trigger.coalesced`/`unreachable` event lines to stdout/stderr. The subprocess acceptance test `tests/acceptance/indexer_deployment_passes.rs` covers it, but `-p openlore-indexer` does not run that test. |

## Advisory targets

- `openlore-review-app` config.rs caps: 3/3 caught (1 unviable), 100%.
- `adapter-index-store` purge.rs: not run. The DuckDB build is slow on this host, so it is left for the CI nightly run.

## Post-run safety

cargo-mutants mutates a copy of the tree. `git status --short` was clean after the runs, apart from the new test files, which are now committed.
