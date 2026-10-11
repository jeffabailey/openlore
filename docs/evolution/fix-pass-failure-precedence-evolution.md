# Evolution: fix-pass-failure-precedence

- **Type**: bugfix delivery (2 steps, one phase), brownfield indexer pass loop and pure core
- **Dates**: 2026-10-11: RCA and roadmap through the last doc fix (`69a8b2d`)
- **Origin**: the "still open" item in `docs/evolution/indexer-deployment-evolution.md`:
  pass-failure precedence was unpinned (CORE-5 had been rescoped to one failure per call).
- **Records**: `docs/feature/fix-pass-failure-precedence/` (`rca.md` with the root cause and user
  decisions, `deliver/roadmap.json`, `deliver/execution-log.json`,
  `deliver/mutation/mutation-report.md`).
- **State**: code complete, committed locally, **not pushed** (user push hold), not deployed. No new
  crate, setting, event or exit code.

## Summary

"First failure ends the pass" lived only in the async shell's control flow (early returns, `break`,
`?`), so no unit test could reach it. The decision now sits in a pure combinator,
`until_first_failure` in `appview-domain::ingest_pass`, and the indexer's per-DID loop runs
through it. The RCA also found a real bug: `within_deadline` turned **any** exit into
`pass_deadline_exceeded` when the summary was past the deadline. Because the post-failure tail
(tally emit + checkpoint) is not time-boxed, an early `upsert_failed` could be reported as a
deadline overrun. Now a pass that already failed keeps its own cause; only a pass that had not
failed becomes `pass_deadline_exceeded`.

## Root cause

The decision "which failure wins and when to stop" was never pulled into the pure core (contrary
to ADR-007). The acceptance seam injects one fault, so no test drove two faults in one pass, and
CORE-5 assumed the acceptance tests covered precedence. They did not.

## Steps and commits

| Step | Change | Commit |
|---|---|---|
| **01-01** First failure ends the pass | `until_first_failure` over a lazy sequence of fallible steps: stops at the first failure, returns the values in order plus that failure. A failing step can carry a value, so a gate-stopped DID is still counted in the summary. The DID loop routes through it; the `timeout_at` fetch deadline and gate-as-fetched streaming stay in the shell. Four properties plus a literal two-fault example; PS-17 count assertions. Behaviour unchanged. | `1230b56` |
| **01-02** First failure outranks the deadline override | `within_deadline` overrides only a natural `Completed` / `TotalOutage` past the deadline; a natural `Failed(f)` stands. Intentional spec change to the unit property and CORE-7; ADR-080, data-models, test-scenarios and the indexer-deployment evolution doc amended. | `0439c05` |
| Refactor | L1-L4 pass, `cargo fmt` | `e3282d8` |
| Review fix | `finish_pass` doc states the first-failure-wins deadline rule up front | `69a8b2d` |

## User decisions (2026-10-11)

- A pure first-failure combinator with property tests and a literal example; **no** multi-fault
  acceptance seam.
- Deadline: **the first failure wins**; pin it with a test.
- Commit locally and hold the push.

## Reviews

- **Roadmap**: rejected once. HIGH: a gate-stopped DID would drop out of the summary counts
  (today its outcome is pushed before the gate). Fixed by carrying the outcome on the failing step
  and adding PS-17 count assertions. Then approved.
- **Implementation**: APPROVED with 3 lows. (1) `finish_pass` doc: fixed in `69a8b2d`. (2) No
  shell-level test that a DID after a gate-stopped one goes ungated: covered by the pure
  property, kept. (3) Two properties partly overlap: kept.

## Quality gates

| Gate | Result |
|---|---|
| Roadmap review | rejected → approved |
| DES integrity | Both steps have complete RED / GREEN / COMMIT traces |
| Implementation review | APPROVED; 1 of 3 lows fixed, 2 accepted |
| Mutation (in-diff vs `2c6eb1f`, gate 80%) | 5/5 viable caught (**100%**), 12 unviable. Indexer shell had no viable mutant; 3 hand-written mutants: 2 caught, 1 accepted equivalent (a fetch timeout ending the sequence is re-labelled `pass_deadline_exceeded` by `within_deadline` anyway) |
| Clippy | Pre-existing `double_must_use` in `crates/ports` under local rustc 1.99 only; unrelated |

## Lessons

- **Control flow is not a tested rule.** Precedence spread over `return`, `break` and `?` looked
  obvious and was never pinned. **Lesson:** when a rule decides an operator-visible outcome, put it
  in a pure function and test it there (ADR-007).
- **An override applied at the end must know what came before.** A summary-time deadline check
  rewrote an earlier, truer cause. **Lesson:** overrides should only fill an absent failure, never
  replace one.
- **Moving a loop into a combinator can drop side effects.** The roadmap review caught that the
  failing DID's outcome was counted before the gate. **Lesson:** list what the old loop did on the
  failure path, and let the failing step carry its value.

## Open follow-ups (not fixed here)

Backlog: a check-arch rule for indexer event fields; the local-only clippy `double_must_use`.
