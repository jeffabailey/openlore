# DISTILL wave decisions: indexer-deployment

- **Date**: 2026-10-06. **Agent**: Quinn (nw-acceptance-designer), autonomous. Nothing committed.
- `[lang-mode] rust` · `[policy-mode] inherit` (rows appended to
  `docs/architecture/atdd-infrastructure-policy.md`) · `[port-mode] inherit` (`tests/common/state_delta.rs`).
- **Reconciliation passed — 0 contradictions** across DISCUSS / DESIGN / DEVOPS. The changes between
  waves are explicit amendments, not contradictions: C-4 and I-IXD-2 (binary changes B1-B15 in scope),
  AC-003.2 (purge, user decision, ADR-082), US-IXD-002 "ingest from the timer" → "`trigger`, pass runs
  inside `serve`" (user-visible behavior unchanged), US-IXD-006 t4g.small as an operator decision only,
  NFR-IXD-4 tightened (indexer ≤ 100 MB within the 256 MB budget). DEVOPS user decisions U-1..U-3 are
  consistent with DESIGN §9.
- Missing SSOT: `docs/product/kpi-contracts.yaml` has no IXD entries (soft warning). `@kpi`
  scenarios trace to `discuss/outcome-kpis.md` instead (WS-1, WS-2, PS-14, PS-22, DL-30).

## Decisions

| ID | Decision | Rationale |
|---|---|---|
| DWD-IXD-1 | Repo convention, not `.feature` files: Rust `#[test]` suites with the Gherkin in doc comments, registered as `[[test]]` in `crates/cli` (indexer suites) and `crates/openlore-review-app` (B11). | Matches `indexer_per_did_*`, `review_app_*`. |
| DWD-IXD-2 | New harness `tests/acceptance/support/indexer_live.rs` (`LiveIndex`, `Deployment`, `PassExit`, `FailureCause`, `Fault`, shared `journey` steps) on top of `indexer_network.rs`. `serve`'s stdout/stderr are drained continuously (the existing `ServingIndexer` drops its stdout reader after `listening`, which would break a `serve` that logs passes). | Production posture `serve` + `trigger` through driving ports only. |
| DWD-IXD-3 | `indexer_network.rs` changes (additive): `Author::Tomas`, `IndexerWorld::configured_with_members`, `did_publishes_self_attested`, `configured_dids`, `indexer_env` made `pub`. | Tomás is the US-IXD-003 author; look-alike DIDs for the purge prefix rule. Existing suites re-run green. |
| DWD-IXD-4 | `trigger` runs with `serve`'s full environment (what `docker exec` inherits); PS-16 separately proves it needs only the socket variable. | Production fidelity plus M4. |
| DWD-IXD-5 | Index rows are read directly only after `serve` stops; while it runs, every observation goes through `openlore search` or the public HTTP surface. | DuckDB has one holder per file; also keeps assertions at the driving port. |
| DWD-IXD-6 | TEST-ONLY fault seam `OPENLORE_INDEXER_TEST_FAULT` (`first_pass_panics`, `store_poisoned`, `search_store_read_fails`, `purge_fails`), debug builds only, refused by a release build like the loopback seam. | A panic, a poisoned mutex and a failing purge cannot be provoked from outside the process. Precedent: the viewer's `#[cfg(debug_assertions)]` seams. **Name is DISTILL-proposed.** |
| DWD-IXD-7 | Real faults wherever possible: store-write failure = a file where an author's artifact directory belongs; deadline = a hanging PDS with a 120 s per-DID limit and a 60 s deadline; unreadable list = a directory at the list path (works as root); interruption = SIGKILL. | Earned Trust without seams. |
| DWD-IXD-8 | Layer-2 contracts in `indexer_deployment_core.rs` with RED `sut_*` bindings (`todo!()`), oracles from the ADR/data-model text. CORE-11 (purge universe) binds the real adapter over a temp DuckDB, the same shape as the existing `atomic_upsert_properties`. | Mandate 9 (PBT at layers 1-2), Mandate 7. |
| DWD-IXD-9 | Shell scripts are run for real under `bash` from xtask tests with recording PATH stubs (`aws`, `gh`, `cosign`, `crane`, `curl`, `docker`, `timeout`, `chown`). `render-dids.sh` must honour `INDEXER_CONFIG_DIR`. | CI-checkable H1 and digest-guard behavior without AWS. |
| DWD-IXD-10 | B11 review-app cap variables are DISTILL-proposed as `REVIEW_DB_MEMORY_LIMIT_MB` / `REVIEW_DB_THREADS` (review-app naming convention, indexer ranges). | data-models §1 leaves the names to the crafter; one-line rename in the suite and XP-2. |
| DWD-IXD-11 | Not automated (stay in the runbook / live checks): TLS certificate validity, alarm test-fire (AC-004.5), cost, ≤ 30 s downtime and PDS 200 during deploy, auto-rollback drill, memory gate, IMDS hop limit, reboot resume, `deploy.sh status` timing. | Need the real host, AWS or time. |
| DWD-IXD-12 | Tier B (state-machine PBT on an in-memory composition) not added. The only rich state machine (single-flight) is a pure transition, covered model-based in CORE-6; the journey's input space is not domain-rich enough to justify an `InMemoryComposition` of the whole indexer. | Mandate 10 conditions not both met. |

## Mandate-12 / step reuse (informational)

Rust has no step decorators; the equivalents are the typed vocabulary and the shared step functions.
Domain types: `Author`, `Host`, `Claim`, `SkipReason`, `Provenance` (existing) + `PassExit`, `FailureCause`,
`Fault`, `Deployment`, `Startup` (new). Step functions: 35 Given/When/Then helpers
(`journey::*`, `LiveIndex::*` steps, suite-local `given_*`/`then_*`), about 260 invocations across
the 43 layer-3/4 scenarios: a ratio of about 7.5× (measured by grep, informational). Step bodies
delegate to the binaries; no business logic.

## Completeness audit (nw-at-completeness-check, 15 items)

| Item | Result | Evidence |
|---|---|---|
| C1a empty / zero | pass | PG-36 empty list, WS-0 empty index, CORE generators include 0 |
| C1b boundaries | pass | AS-52 (8192/8193, 512/513), AS-55, CORE-8, CORE-9b, RAC-1 |
| C2a state machine documented | pass | module docs of `indexer_deployment_passes.rs`, `indexer_deployment_did_list.rs` |
| C2b illegal event per state | pass | busy (PS-13), no serve (PS-15), end while idle (CORE-6), refused list → no purge (PG-43), unusable store (PS-20) |
| C3 0 / 1 / N | pass | lists 0/1/40; results 0/many/1001 |
| C4a apply twice | pass | PS-25, PG-47, CORE-11, XP-5 |
| C4b inverse without prerequisite | pass | `trigger` without `serve` (PS-15); purge of an author never indexed (CORE-11) |
| C5a mode-flag combinations | pass | purge on/off (PG-40/PG-44), list file vs inline (DL-35), every setting (AS-55) |
| C5b flag orthogonality | **gap** | PG-44 does not assert that the purge flag changes nothing but the purge (documented, LOW) |
| C6a malformed input | pass | typo, BOM, CRLF, junk settings, oversized body/value, stale socket |
| C6b each declared error | pass | every `cause` token, `store.unusable`, `trigger.unreachable`, 413/400/500/503, startup refusal |
| C6c closed error set | pass | CORE-5 (exit ∈ {0,2,3}, cause iff 2), CORE-9 (closed route set) |
| C7a degraded resource | pass | unreadable list, SSM failure (XP-6), PDS down, store write failure |
| C7b interruption | pass | SIGKILL mid-pass (PS-23), deadline (PS-19), failed purge resumes (PG-48) |
| C7c concurrent actors | pass | two triggers (PS-13), 100 searches during a pass (PS-11) |

**Verdict: 14/15, COMPLETE.** Gap kind: AT_GAP_IN_DELIVERY_SCOPE (C5b, LOW). No SPECIFICATION_AMBIGUITY
blockers. Telemetry: (indexer-deployment, C5, 1, LOW); all other categories 0 findings.

## Upstream findings (for DELIVER / DEVOPS, not blockers)

1. **`render-dids.sh` as non-root.** DEVOPS §4 counts a `chown` error as a render failure, so the
   script (and its bats test) would always fail on a non-root CI runner. XP-5/6 stub `chown`;
   DELIVER should either stub it in the bats test too or `chown` only when `EUID=0`.
2. **bats vs the Rust-driven test.** DEVOPS lists `deploy/indexer/tests/render-dids.bats`; XP-5/XP-6
   assert the same contract by running the real script. Keep one (decision for DELIVER).
3. **Control-socket refusal rule.** data-models §1 says "a non-path value is refused". AS-55 uses
   `0.0.0.0:9000`, which assumes the rule is "an absolute path is required".
4. **The CLI on a 500.** PS-21 expects `openlore search` to say "Network index unavailable" when the
   index answers 500 (the existing degrade message). If the CLI prints something else, adjust the
   assertion, not the server contract.
5. **B9 read-back.** The probe's DuckDB settings read-back is not observable from outside; it needs
   an adapter-level test in DELIVER (AS-55 covers the range refusals only).

## Orchestrator decisions on the DISTILL hand-off (2026-10-06)
1. The test-only fault seam is named `OPENLORE_INDEXER_TEST_FAULT`, honoured only in debug builds (a release build refuses to start with it set, like the loopback seam).
2. The review-app DuckDB caps are `REVIEW_DB_MEMORY_LIMIT_MB` and `REVIEW_DB_THREADS`, as proposed.
3. `render-dids.sh` takes an `INDEXER_CONFIG_DIR` override for tests and runs `chown` only when run as root. Keep the Rust-driven test in xtask/tests/indexer_deployment_platform.rs and do not add a parallel bats test.
4. The control-socket setting must be an absolute path (start is refused otherwise).
5. When the index answers 500, `openlore search` prints "Network index unavailable" (the existing graceful degradation, ADR-027).
