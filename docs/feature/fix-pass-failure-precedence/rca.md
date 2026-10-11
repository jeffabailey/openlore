# RCA — indexer pass-failure precedence is untested and the deadline override breaks it

Source: /nw:bugfix, opus read-only RCA, 2026-10-11. Backlog item from docs/evolution/indexer-deployment-evolution.md ("Still open"). The orchestrator spot-checked the deadline override against the files.

## Rule
"First failure ends the pass": a pass that hits several faults stops at the first one (later stages and DIDs are not attempted) and reports that failure as its `cause`.

## Today
- The precedence is implicit in the shell's control flow in crates/openlore-indexer/src/ingest_pass.rs:
  - early returns for list (:112-115) and purge (:106-109);
  - `break` on a deadline (:377-379) or a gate stop (:385-387);
  - `?` inside gate_fetch (:523-528).
- The pure core only receives the result: `pass_exit(Option<PassFailure>, …)` (crates/appview-domain/src/ingest_pass.rs:470-480).
- **Bug:** `within_deadline` (appview-domain ingest_pass.rs:486-496, applied in finish_pass :332-336) turns ANY exit into `pass_deadline_exceeded` when the elapsed time at summary is past the deadline. The post-failure tail (tally.emit + checkpoint, :390-391) is not time-boxed. So an early UpsertFailed can be reported as pass_deadline_exceeded, which is a later condition.

## Five Whys (root cause)
1. No test drives two faults in one pass; the CORE-5 comment says so (tests/acceptance/indexer_deployment_core.rs:428-432).
2. The acceptance seam is a single fault: `with_fault(Fault)` → `Option<Fault>`, and one TEST_FAULT token (config.rs:185, 226, 336-347).
3. The precedence lives in async shell control flow, not in a pure function, so no unit test can reach it (ingest_pass.rs has one unit test, for the panic fallback).
4. CORE-5 was rescoped to "one failure per call" on the assumption that the acceptance tests covered precedence. They did not.
5. **Root cause:** the decision "which failure wins and when to stop" was never pulled into the pure core (contrary to ADR-007).

## Decisions (user, 2026-10-11)
- Fix direction: a pure first-failure combinator in appview-domain::ingest_pass, with the indexer's stage/DID loop routed through it, plus a property test and a literal two-fault example. No acceptance multi-fault seam.
- Deadline: **the first failure wins.** Override to pass_deadline_exceeded only when the pass had no earlier failure. Pin it with a test.
- Commit locally and hold the push until the user says so.

## Risk
- The combinator and its tests are low risk.
- Rerouting the async loop is medium risk: keep the `timeout_at` fetch deadline and gate-as-fetched streaming outside the combinator.
- The deadline change alters the operator-visible `cause` only in the edge case where an early failure is followed by a slow tail.
