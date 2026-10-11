# Mutation report: fix-pass-failure-precedence

**Gate:** per-feature, kill rate >= 80%. **Result: PASS, 100% (5/5 viable cargo-mutants mutants caught).** Supplementary hand-written shell mutants: 2/3 caught; the one survivor is the documented equivalent mutant.

## Scope

Production Rust changed by this bugfix (`git diff 2c6eb1f..HEAD`, run at `e3282d8`), restricted to the changed lines (`--in-diff`), `#[cfg(test)]` code excluded:

- `crates/appview-domain/src/ingest_pass.rs`: `until_first_failure`, `within_deadline`
- `crates/openlore-indexer/src/ingest_pass.rs`: `listed_pass_work`, `staged_pass_work`, `pass_work` (DID loop), `gated_did`, `finish_pass` (doc only)

`69a8b2d` (landed during the run) changes only `finish_pass`'s doc comment, so it adds no mutants. The tests, ADR and doc changes are out of scope.

## Command

```
git diff 2c6eb1f..HEAD -- crates/appview-domain/src/ingest_pass.rs > domain.patch
git diff 2c6eb1f..HEAD -- crates/openlore-indexer/src/ingest_pass.rs > indexer.patch

cargo mutants -j 2 -p appview-domain --file crates/appview-domain/src/ingest_pass.rs \
  --in-diff domain.patch --timeout 600

CARGO=cargo-wrap.sh cargo-mutants mutants -j 2 \
  --file crates/openlore-indexer/src/ingest_pass.rs --in-diff indexer.patch --timeout 600 \
  -- --bins --test indexer_deployment_passes --test indexer_deployment_core --test indexer_deployment_walking_skeleton
```

The appview-domain run used that crate's own unit tests (41) and integration tests. The indexer run reused the recipe from `fix-indexer-deployment-follow-ups`. cargo-mutants 25.3.1 ignores `--test-package`. The indexer has no lib target, and its acceptance suites live in `cli` and spawn `target/debug/openlore-indexer`. So a throwaway `CARGO` wrapper (kept in the session scratchpad, not the repo) did two things:

- It rewrote `--package=openlore-indexer@0.0.1` to also pass `--package=cli`.
- After the `test --no-run` build phase, it ran `cargo build -p openlore-indexer --bin openlore-indexer`, so the suites spawn the mutated binary.

Three pitfalls, recorded for next time:

- Invoke the `cargo-mutants mutants` binary directly. When run as `cargo mutants`, cargo overwrites `CARGO`, so the wrapper never runs.
- cargo-mutants passes the package as `name@version`.
- The wrapper must call the rustup proxy (`~/.cargo/bin/cargo`), not the toolchain's cargo binary. Otherwise a stray `rustc` on PATH breaks the build.

Test set per indexer mutant: openlore-indexer unit tests (46) + `indexer_deployment_passes` (20) + `indexer_deployment_core` (9) + `indexer_deployment_walking_skeleton` (5). Baseline: 172s build + 70s test, green.

## Results

| Outcome | Count |
|---|---|
| Caught | 5 |
| Missed | 0 |
| Unviable | 12 |
| Timeout | 0 |
| **Kill rate** | **5/5 = 100%** |

### Per file

| File | Caught | Missed | Unviable |
|---|---|---|---|
| appview-domain/src/ingest_pass.rs | 5 | 0 | 5 |
| openlore-indexer/src/ingest_pass.rs | 0 | 0 | 7 |

### Caught

- ingest_pass.rs:539 (domain) `within_deadline` match guard `elapsed > deadline` -> `true`: killed by `past_its_deadline_only_a_pass_that_had_not_failed_becomes_deadline_exceeded`.
- The same guard -> `false`: killed by that property and `an_upsert_failure_past_the_deadline_still_reports_upsert_failed`.
- The same guard with `>` -> `==` / `<`: killed by the same two tests.
- The same guard with `>` -> `>=`: killed by `a_pass_ending_exactly_at_its_deadline_keeps_its_own_exit` (`crates/appview-domain/tests/deployment_contracts.rs`). The two properties rarely draw `elapsed == deadline`, so they would not catch it alone.

`within_deadline` mutants are all killed by appview-domain's own tests, so CORE-7 (cli) was not needed. CORE-7 pins the same rule end to end, but it would not catch the `>=` boundary on its own either: its ms/secs ranges almost never draw equality.

### Unviable (do not compile; not counted)

- domain:499 `until_first_failure` -> `PassProgress::new()` / `::new(Default::default())` / `::from(Default::default())` / `::from_iter([Default::default()])` (`PassProgress` is a plain struct with none of these).
- domain:537 `within_deadline` -> `Default::default()` (`PassExit` has no `Default`).
- indexer:99 `listed_pass_work` -> `Default::default()`, indexer:110 `staged_pass_work` -> `Ok(Default::default())`, indexer:363 `pass_work` -> `Default::default()` (`PassWork` has no `Default`).
- indexer:415 `gated_did` -> `PassStep::new()` / `::new(Default::default())` / `::from(Default::default())` / `::from_iter([Default::default()])` (`PassStep` is a `Result` alias).

### Supplementary hand-written shell mutants (not counted in the gate)

cargo-mutants produced no viable mutant for the indexer shell, so three hand-written mutants were run. Each ran in a throwaway git worktree with the same test set and was reverted after its run.

| Mutant | Result | Killed by |
|---|---|---|
| M1: the timeout arm `Err(_deadline_passed) => Some(Err((None, PassDeadlineExceeded)))` -> `None` (a fetch timeout ends the sequence instead of failing the pass) | survived | (equivalent mutant, see below) |
| M2: `gated_did` drops the failing DID's outcome: `(Some(outcome), ...)` -> `(None, ...)` | caught | `a_pass_that_cannot_store_a_claim_ends_with_exit_2_and_search_keeps_answering` (PS) |
| M3: the timeout arm reports `UpsertFailed` instead of `PassDeadlineExceeded` | caught | `a_pass_that_overruns_its_deadline_ends_with_exit_2_and_keeps_what_it_saved` (PS-19) |

Environment note: in a worktree under the long scratchpad path, PS-19 failed even at baseline. The pass reported `upsert_failed` at about 60s, which looks like a path-length artifact. The same test passed in the repo checkout and in the cargo-mutants temp copy. The worktree was therefore moved to a short `$TMPDIR` path, where the baseline was green (80/80), before M1-M3 were run.

## Survivors

None among the cargo-mutants mutants. No tests added.

The hand-written M1 survives and is an **accepted equivalent mutant**. When `fetches.next()` times out at the pass deadline, M1 ends the DID sequence (`None`) instead of failing it with `PassDeadlineExceeded`. The pass then has no failure, so its natural exit is `Completed` or `TotalOutage`. `finish_pass` then applies `within_deadline`, and because the pass is past its deadline, that natural exit becomes `Failed(PassDeadlineExceeded)`. That is the same code, cause and summary as the unmutated shell. The values gathered before the timeout are identical too, because the timed-out step carries none. The two paths are indistinguishable from outside the process.

## Post-run safety

cargo-mutants mutates a temporary copy of the tree. The hand-written mutants ran in a detached worktree outside the repo, which was removed afterwards (`git worktree list` shows only the main checkout). Afterwards, `git status --short` lists only the untracked `docs/feature/fix-pass-failure-precedence/`.
