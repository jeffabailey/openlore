# Wave Decisions — DISTILL — scale-to-zero-hosting

- **Designer**: Quinn (nw-acceptance-designer). **Date**: 2026-10-30 (tests begun 2026-10-27;
  revision cycle 1 of the final four-reviewer gate the same day). **Mode**: subagent, uncommitted
  work. Nothing committed.
- `[lang-mode] rust` · `[policy-mode] inherit` (rows updated in
  `docs/architecture/atdd-infrastructure-policy.md`) · `[port-mode] inherit`
  (`tests/common/state_delta.rs`).
- **Revision cycle 2 of 2** applies the re-review findings and the DESIGN and DEVOPS changes of the same
  day: strict pass events, § Timeouts, A9 13,000, `write_probe_outcome`'s input, ECR keep 10, schedules
  disabled until after the first flip.
- **WARN — DISCUSS artifacts missing**: requirements R-STZ-1..10 come from DESIGN; see DWD-STZ-13.

## Prior-wave reading and reconciliation

Read: `../feature-delta.md` (DESIGN, DEVOPS, both Changed Assumptions sections),
`../design/wave-decisions.md`, `../devops/wave-decisions.md`, `../devops/upstream-changes.md`,
`../environments.yaml`, `../spike/findings.md`, ADR-090, the brief's "For Acceptance Designer" note,
`docs/product/kpi-contracts.yaml`, `upstream-issues.md`.

**Reconciliation passed — 0 contradictions**, reconciled against DEVOPS changes C-1..C-10 and
U-STZ-1..9, and again against the cycle 2 revision. The DEVOPS table runs to C-14; C-11..C-14 are the same
changes as U-STZ-6..9. Each one
reached the suites: U-STZ-1 and U-STZ-2 as OP-7 and the runbook, U-STZ-3 in every pass record,
U-STZ-4 as OP-4, U-STZ-5 as PA-E6 and OP-5, U-STZ-6 as PR-3 and PR-4, U-STZ-7 as PR-E3 and OP-10,
U-STZ-8 as PA-13, U-STZ-9 as OP-15, OP-E18 and OP-E19. C-6 (IAM) and C-7 (budget) have no scenario, as before: DEVOPS checks them with
P-15 and a drill. The architect's rulings on UI-17..20 (D-STZ-39, D-STZ-41, D-STZ-48, D-STZ-50) are
applied. `.nwave/des-config.json` declares no `deliverable_type`, so the application
default applies: only the four-reviewer gate.

## Key Decisions

| ID | Decision | Rationale |
|---|---|---|
| DWD-STZ-1 | Rust `#[test]` suites with the Gherkin in doc comments, no `.feature` files. | Repo convention (DWD-IXD-1). |
| DWD-STZ-2 | Suites call the REAL Lambda root in-process with real event shapes; only the AWS ports, clock, event sink, PDS network and public edge are doubled. | Pillar 3. Rows in the ATDD policy. |
| DWD-STZ-3 | Waiver: no cargo scenario starts the `openlore-indexer-lambda` binary. | The Runtime API needs RIE. CI `lambda-smoke` (DV-STZ-2) already runs the release image. |
| DWD-STZ-4 | The walking skeletons stay `#[ignore]`d at hand-off. | DWD-IXD precedent; the root is a scaffold. |
| DWD-STZ-5 | PA-E5 follows D-STZ-41: failed pass, exit-2 summary, record written, nothing published, no crash. | The harness needed narrow postures (`DownloadsFail`, `DownloadsRefused`). |
| DWD-STZ-6 | A full outage is its own scenario, PA-E7. | D-STZ-41: the old record stays. |
| DWD-STZ-7 | **Superseded.** Causes are pinned now: DESIGN has named them (D-STZ-41). | The reason not to pin them is gone. |
| DWD-STZ-8 | `xtask/tests/scale_to_zero_platform.rs` covers the Operator driving port. | DEVOPS named these checks, and `indexer_deployment_platform.rs` is the pattern. |
| DWD-STZ-9 | `scale_to_zero_harness.rs` and the stub check, green at hand-off, are not counted as scenarios. | RED stops at the first scaffold panic, so nothing after it has run. |
| DWD-STZ-10 | **Tier B declined**, against both Mandate 10 conditions. (1) A journey of 3 or more chained scenarios: **met**, PA-1 to PA-9. (2) A domain-rich input space: **not met**. A pass's inputs are DID lists and claims from a small cast, and what varies is the store's behaviour, a closed set of `ObjectTrouble` postures. The rich decisions are pure and already generated at layer 1 (CORE-1..8). An `InMemoryComposition` would need a double of DuckDB, so it would test the double. Both conditions are required. | Same call as DWD-IXD-12. |
| DWD-STZ-11 | **Superseded by DWD-STZ-14.** | |
| DWD-STZ-12 | `@contract-shape:` on every scenario; the 132 tags were counted by script. | Project mandate. |
| DWD-STZ-13 | **Waiver: no DISCUSS wave** (user-approved scope, feature-delta header). The R-STZ table is the trace basis, and `@R-STZ-n` tags appear only where a scenario asserts the requirement. | See Requirement coverage. |
| DWD-STZ-14 | SE-E6 poisons a running instance with a new test fault, `store_poisoned_after_first_search`. | The old test set the fault before start, so it was a cold instance, and ADR-090 §3 says init refuses. Init refusal is SE-E5. A store that breaks while running is a different state, and only that one answers 503. |
| DWD-STZ-15 | Requests to the store are counted only where the count is the requirement: SE-4 (D-STZ-10, at most one check per TTL), PA-4 (KPI-IXD-4, read on every pass), and PA-E11 and PR-E3 (a refused event makes no call, counted after init). PA-12 and PA-13 count object writes through `ForbiddenWrites`, which excludes init's write check: it runs whatever the event, so "no publish or record write", not "no write". | Elsewhere a count pins the mechanism. SE-E3 uses a log line instead of a request count. |
| DWD-STZ-16 | Scratch space is a port-level bound: at most 2× the index size after a swap, with two swaps in a row in SE-3 and three failed reloads in both rows of SE-E4, measured against the served copy. A damaged object is as large as the one it replaced, so a leak in that row shows. | One leaked copy still fits 2×. Two do not. |
| DWD-STZ-17 | The fourth concurrent search is OP-6, a tofu assertion, plus the existing `adapter-index-query` tests of a 429. The 429 at cap 3 itself is unverified until the trial (P-18). | Reserved concurrency is a Lambda setting: no in-process test can reach it. SE-6, now "Serial searches from one client are all answered", no longer claims to. `lambda test-alarm throttles` sets concurrency 0, which is a different case. |
| DWD-STZ-18 | `scheduled_event()` is `{}`, not the EventBridge envelope. | Both schedules set `input = "{}"` (U-STZ-7), so that is the whole event. It also makes PA-1 and PR-1 the proof that `{}` runs a normal pass and a normal probe. |
| DWD-STZ-19 | The tofu assertions read whole settings in the right block (`=1` is not `=10`), through a small reader with a green positive control. | Substring checks on squeezed text could not tell them apart. The reader's contract is in the suite's header: literal `function_name`, no `for_each` or `dynamic` on those resources. |
| DWD-STZ-20 | `rust-version = "1.94.1"` is on the two Lambda crates now. | User request; 1.91 still builds the crates checked (see Verification). `../design/wave-decisions.md` still says "applied in DELIVER". |
| DWD-STZ-21 | The four `PassFailure` tokens are added to `appview-domain` now, with `IndexObjectWriteConditionIgnored` in `ProbeRefusalReason`. | `final_exit` works on the domain `PassExit`, so its tests need them. They are vocabulary: nothing produces them yet. |
| DWD-STZ-22 | `decide`'s own `PassExit` is gone. `PublishStep`, `RecordStep`, `PassMode`, `ServedIndex` and `StaleServedIndex` are DISTILL-proposed names. | D-STZ-40 names the function and its three arguments, not these types. |
| DWD-STZ-23 | The moto seeds send a credential scope and read back what they set, and `the_seeds_really_reach_the_moto_store` runs them with no adapter. | moto routes by that scope. An unsigned POST reached its S3 backend and answered 200 with ListBuckets. Verified by hand first. |

| DWD-STZ-24 | WP-1 is a table over a pure adapter function, `write_probe_outcome`, and PA-E10 and the fake see only its verdict. | moto honours conditions, so it cannot play a store that lies, and RIO-6 sees only the honest rows. |
| DWD-STZ-25 | XS-10 calls a scaffold, `scan_indexer_family_holds_no_signing`, over planted trees, and over the real tree last. | A test of a rule that does not exist yet needs a seam to call. The scaffold is not wired into `check-arch`, so check-arch stays green. The real tree is RED until the Lambda root has its `resolve_only` site. |

| DWD-STZ-26 | **Waiver (F5).** Walking skeletons are environment-agnostic and in-process, so they carry no environment Givens. Infrastructure environments escalate to the platform checks: `ci-rio-object-store` to RIO-1..9, `ci-image-lambda` to `lambda-smoke`, `ci-image-self-host` to serve-smoke, `prod-trial` to the trial stages. | A Given for `local` or `ci-rio-object-store` would change nothing the skeleton does, so it would be a label. |
| DWD-STZ-27 | **Language exemption (user decision, 2026-10-30; extended the same day).** (1) Guards and operator checks (XS, OP) may use technical terms, because their readers are operators. The RIO tests may too, because they exist to prove the real store adapter. (2) Scenarios may chain steps where the sequence is the behaviour under test: the timed freshness scenarios SE-3, SE-4, SE-8, SE-E3 and WS-2, and OP-3 (stop then start) and OP-E6 (declined then confirmed). RIO-1 and RIO-2's preconditions moved into Given. | Everything else keeps Pillar 1 and one When. |
| DWD-STZ-28 | Mandate 8 at the OP layer (CM-E scope): OP scenarios assert on the ordered call log of a stubbed tool, not through `assert_state_delta`. The universe of `deploy.sh` is the AWS, registry and host state the stubs do not hold. So the call log is the observable, and each scenario states what must not have been called. The mandate's layer table puts operator scripts under real tools at layer 3, and this is the one written exception. | A state-delta over a stub's log would only restate the log. Real state is covered by the trial stages. |
| DWD-STZ-29 | WP-2 tests the pure `location_names_write_probe_key`, with a control row, and one assertion that `connect` uses it. PA-E12 is the same refusal at init of all three functions, with DISTILL-proposed variable names. | The architect ruled (UI-21, D-STZ-39). A constant alone leaves `index_key` free to name it. |
| DWD-STZ-30 | A stalled `HEAD` in the fake takes the adapter's 1 s and then fails, because that timeout is the adapter's port contract. A stalled download outlasts every bound (30 s, then an error), because the 3 s bound covers the open too, so only the root can apply it, and a root with no bound fails SE-E8 instead of hanging it. | D-STZ-10 and § Timeouts place the two bounds in different layers. RIO-9 holds a real client to a bound against a silent store. |
| DWD-STZ-31 | TO-1 tests the one table, `call_budget`, for a `max_attempts = 1` check. | With no SDK client yet, the honest layer is the table every client must be built from. A real client's obedience is RIO-9's half, on the write probe only. UI-22. |

## Test Coverage Summary

| Suite | Scenarios | Error | State at hand-off |
|---|---|---|---|
| Walking skeleton (WS) | 2 | 0 | RED, ignored |
| Search (SE) | 18 | 10 | RED, ignored |
| Pass (PA) | 26 | 14 | RED, ignored |
| Probe (PR) | 9 | 5 | RED, ignored |
| Core (CORE) | 8 | 0 | RED, ignored (27 tests) |
| Real I/O (RIO) | 9 | 5 | RIO-1..8 not run without moto, RED with it; RIO-9 RED, no moto needed |
| Adapter rules (WP, TO) | 3 | 1 | RED, ignored |
| Fingerprint (FP) | 5 | 0 | RED, ignored |
| Architecture (XS) | 13 | 3 | XS-2, XS-2b, XS-3, XS-4, XS-5, XS-7 green; XS-1, XS-6, XS-8..12 RED |
| Platform (OP) | 39 | 22 | RED, ignored |
| **Total** | **132** | **60 (45.5%)** | |

Error-path share is 45.5% by scenario (47.9% without XS) and 56.9% by outline row (148 of 260). An
`@error` tag means the scenario's subject is a failure: PA-5, PA-6 and PR-3 carry it, and plain
boundaries such as SE-1 and PR-B1 do not, and neither does WP-1, a whole table with one refusal row. Layer-1 PBT runs on all eight pure decisions; everything at
layer 2 and above is example-only (Mandates 9 and 11). Every pass scenario asserts over the
port-exposed universe (Mandate 8), and every Delta that writes the record declares
`last_pass.index_published_at`. Search and probe scenarios change only an instance's private copy or
a log line, so they assert on their answers and events.

### Requirement coverage

| Requirement | Covered by | In a cargo scenario? |
|---|---|---|
| R-STZ-1 cold start, R-STZ-2 warm | **`lambda measure`, a gate**: all 5 cold calls < 3 s and all 20 warm calls < 200 ms on both paths, or Stage 2 is blocked | No. In-process times prove nothing about either. The tags on WS-1 and SE-4 are gone. |
| R-STZ-3 cost | Budget at $2 and $10, 30-day baseline. Recorded cost: about $0.72/month on the bill, about $1.60 at list, about $1.93 for a run held just under the alarms, $1.97 in a 31-day month (DEVOPS) | No |
| R-STZ-4 ship and roll back | OP-1, OP-2, OP-7, OP-11, OP-12, OP-E1..E5, OP-E7..E11, OP-E13..E20 | Yes |
| R-STZ-5 freshness | `lambda kpi` (`freshness_ratio`), A1, A3 | No. The tag on WS-1 is gone. PR-E1 and PR-B1 pin the 45-minute rule, which is A3's and not KPI-IXD-3's. |
| R-STZ-6 DID edits | WS-2, PA-4, RIO-4 | Yes |
| R-STZ-7 paging | PA-5, PA-6, PA-11, PA-E1..E10, PR-1, PR-2, PR-E1..E3 | Yes |
| R-STZ-8 capped in rate | OP-3, OP-4, OP-6 | Yes, as settings. The alarms' test-fire covers the notification path only. The 429 at cap 3 is P-18 |
| R-STZ-9 self-host AWS-free | XS-1, XS-2, XS-3 | Yes |
| R-STZ-10 lag | SE-3, SE-8, SE-E7..E9, PR-3 | Yes |
| NFR-IXD-7 | SE-6, OP-6 | Yes |

### Mandate compliance

- **CM-A**: suites import `openlore_indexer_lambda` entry points and `decide`, plus `ports` types. No
  internal module of the root is imported. `decide` is the pure seam the brief names.
- **CM-B**: a grep of titles and steps for HTTP methods and codes, JSON, S3, SSM, ECR, DuckDB, crate
  names and the like finds XS-1..5 and XS-7..11, all under the exemption in DWD-STZ-27. The earlier claim
  that only XS-2 and XS-3 matched was wrong: its word list had no crate names or `check-arch`. Nothing
  outside XS matches. OP scenarios use operator words such as "live alias", also under the exemption.
  The HTTP detail of SE-B1 is in its test body.
- **CM-C**: 2 skeletons, 130 focused.
- **CM-D**: all eight decisions live in `decide`; `write_probe_outcome` and `call_budget` are the pure
  rules of the adapter.
- **CM-E**: pass scenarios use `assert_state_delta` over `pass_universe` or, for no-publish,
  `bucket_universe`. The OP layer's scope is DWD-STZ-28.
- **CM-F, CM-H**: `proptest` appears only in `scale_to_zero_core.rs`.
- **CM-G**: Tier B declined, DWD-STZ-10.
- **CM-I (Mandate-12)**: typed vocabulary (`Author`, `Claim`, `Exit`, `Fault`, `ObjectTrouble`,
  `BucketRequest`) in `support/indexer_lambda.rs`; the Python-shaped AST check does not apply.
  Step-reuse ratio about 8.7× (723 calls of 83 steps), informational, not a gate.

## Completeness audit (nw-at-completeness-check, 15 items)

| Item | Result | Evidence |
|---|---|---|
| C1a empty / zero | pass | SE-1, PA-9, PA-10, FP-1 |
| C1b boundaries | pass | SE-B1 (8192/8193, 512/513), PR-B1, PR-E1, PR-4, CORE-1, CORE-4, CORE-5, CORE-8 |
| C2a state machine documented | pass | Module docs of the pass and search suites |
| C2b illegal event per state | pass | Refused (SE-E5, PA-E10), Serving (SE-E3, SE-E4, SE-E6), ServingEmpty (CORE-2), pass stages (PA-E3, PA-E5, PA-E8) |
| C3 0 / 1 / N | pass | DID lists PA-9, PA-1, PA-8; claims FP-1; results SE-1, SE-3; instances SE-8 |
| C4a apply twice | pass | PA-2, PA-14, SE-4, FP-2, RIO-6, OP-13 |
| C4b inverse without prerequisite | pass | OP-E10 (rollback with nothing recorded), SE-5 |
| C5a mode combinations | pass | CORE-3, CORE-6 (3 × 3 × 2 exhaustive), CORE-7, PA-12, OP-E4, OP-E9 |
| C5b flag orthogonality | pass | OP-E8 (accepting exit 3 accepts nothing else), OP-E9 row 3, OP-E7 row 2. DEVOPS re-rated this HIGH. |
| C6a malformed input | pass | SE-B1, PA-E4, PA-E11, CORE-7, PR-E1 garbled row, PR-E3, PR-E4, RIO-8, WP-2, OP-E1, OP-E6, OP-E22 |
| C6b each declared error | pass | `IndexObjectError` and `RepoDidListError` variants (PA-E5, PA-E7, SE-E4, RIO-3, RIO-5, RIO-7), `Conflict` (PA-E1), each cause (PA-E1, E5, E8, E9), 400/404/413/500/503 |
| C6c closed error set | pass | CORE-4, CORE-6 (first failure wins, table exhausted), `assert_exit` (exit, payload and summary agree) |
| C7a degraded resource | pass | PA-7, PA-E5, PA-E7, SE-E3, SE-E4, SE-E5, PR-E1 |
| C7b interruption | pass | PA-E6, SE-E1, SE-E7, SE-E8, PR-E2, RIO-9, OP-E15, OP-E19, OP-E20. A real crash or timeout is the documented gap UI-13 |
| C7c concurrent actors | pass | PA-E1, SE-8, OP-E12, OP-E13. Reserved concurrency itself is OP-6 |

**Verdict: 15/15, COMPLETE**, and the same caveat as before: most Lambda scenarios stop at the first
scaffold panic, so their later assertions have not run. No SPECIFICATION_AMBIGUITY blockers: the architect
confirmed UI-17 and UI-19 and ruled on UI-18 and UI-20.
Telemetry: (scale-to-zero-hosting, C4, 0, none), (scale-to-zero-hosting, C5, 0, none), down from one
LOW each; the final review re-rated C5b HIGH, and OP-E8 closes it.

## Verification (2026-10-30)

| Command | Result |
|---|---|
| `cargo nextest run` on the new binaries, ignored skipped | 37 passed, 146 skipped, 0 failed: 29 distinct tests, because 2 `state_delta` self-tests are compiled into 5 binaries. The earlier "33 passed" counted those repeats too |
| `… --run-ignored only`, no moto | 146 run: 137 RED, 9 not run, 0 passed, 0 BROKEN (`red-classification.md`) |
| `… --run-ignored only`, the RIO binary, against pip `moto==5.2.5` | 10 run: the seed check passed, RIO-1..9 stopped at the adapter scaffold. RIO-1..8 and the seed check need moto; RIO-9 does not |
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets -- -D warnings` on `ports`, `appview-domain`, `adapter-index-query`, `adapter-s3-index-object`, `openlore-indexer-lambda`, `xtask` | clean |
| `cargo xtask check-arch` | OK (30 members), with `adapter-atproto-oauth` now forbidden |
| `cargo xtask check-probes` | OK, 20 sites, 4 allowlisted stub warnings |
| `cargo nextest run -p xtask -p ports` | 306 passed, 46 skipped (the ignored scale-to-zero tests) |
| `cargo check --workspace --all-targets` | clean |

**XS-7 note.** No change: it holds today only because Cargo drops a dev-dependency on a crate with no
library, and XS-8 is the test that moves first.

## Upstream Issues

`upstream-issues.md` is the record. UI-1..4, UI-6, UI-7 (for design), UI-9, UI-10, UI-12 and UI-14..20
are closed: the architect ruled on UI-17..20, and the tests carry those rulings. UI-5, UI-8, UI-11 and
UI-13 stay open for DELIVER, with UI-22, new in cycle 2. UI-21 is resolved (D-STZ-39). The stale text this file listed earlier has since been corrected in
DESIGN and DEVOPS.

## Handoff

DELIVER starts with **UI-7** (XS-8), then UI-5, UI-8, UI-11, UI-13 and UI-22. The other action items are
the deadline passed to `dispatch` after the gate (D-STZ-10), shim arming for smoke assertion 9, measuring
the post-pass steps and the query at 140 MB, RIO on the pinned moto image, and the P-15, P-17 and P-18
evidence. Order and pre-requisites: `../feature-delta.md` § Handoff to DELIVER.

## Review Gate Result

Closed. Four reviewers, two revision cycles. All four conditionally approved at closing, with no open
blockers. The language exemption (DWD-STZ-27) is user-approved, the waivers (DWD-STZ-3, DWD-STZ-10,
DWD-STZ-13, DWD-STZ-26, DWD-STZ-28) are recorded, and the reviewers' conditions are applied in this
revision. Reported to DISTILL by the coordinator; the verdicts themselves are not in this file.
