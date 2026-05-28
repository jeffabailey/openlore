# Wave Decisions — DELIVER — openlore-github-scraper (slice-02)

- **Wave**: DELIVER
- **Date**: 2026-05-28
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 39 steps, 5 phases, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L6 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 39 roadmap steps executed via DES-monitored crafter dispatches; `des-verify-integrity`
reports "All 39 steps have complete DES traces". All 34 slice-02 acceptance scenarios
GREEN (SG-1..9 harvest, SC-1..5 candidates, SS-1..9 sign, SA-1..5 auth, SD-1..6 scraper-domain);
slice-01 + slice-03 suites show zero regression. Two new crates shipped (WD-57): PURE
`scraper-domain` + EFFECT `adapter-github`; crate count 10 → 12.

| Phase | Scope | Result |
|---|---|---|
| 01 Bootstrap (01-01..05) | GithubPort + scraper-domain + adapter-github + cli scrape verb + FakeGithub + 5 test targets | fail-for-right-reason gate; all 34 ATs compile RED |
| 02 scraper-domain pure core (02-01..06) | SD-1..6 (derive_candidates, mapping SSOT, NFC-free properties) | green |
| 03 scrape_github harvest (03-01..09) | SG-1..9 (harvest, public-only refusal, not-found, offline, no-match, no-persist) | green |
| 04 candidates + auth (04-01..09) | SC-1..5 (render/audit) + SA-1..4 (PAT, budget, rate-limit, 401) | green |
| 05 sign (05-01..10) | SS-1..9 (sign-via-slice-01, batch, decline) + SA-5 (token-no-leak on sign) | green |

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES `project_id` header added to execution-log right after `des-init-log` (same hook-defect workaround as slice-03 DV-1). | Stop-hook reads `project_id`; des-init-log writes `feature_id`. |
| DV-2 | Mutation = per-feature ≥80% on the new pure-core `scraper-domain` (Phase 6), matching slice-03 DV-2. | DEVOPS D-D23 adds scraper-domain to the nightly sweep; deliver-time per-feature gate is the immediate check. |
| DV-3 | Workspace rustfmt normalization committed as housekeeping mid/end-of-run (per-file-staging crafters accumulate fmt drift). | Keeps CI fmt gate green; matches slice-03 DV-3. |
| DV-4 | A transient source-write-guard race blocked step 05-01's first GREEN attempt (the PreToolUse(Task) hook did not set `.nwave/des/des-task-active` after the 04-04 rate-limit interruption). Resolved by re-dispatching the crafter normally (the hook set the marker on the fresh dispatch) — NOT by forging the guard marker. | The guard (`session_guard_policy.py`) blocks orchestrator source writes; a legitimately re-dispatched crafter is the correct path. |
| DV-5 | `serde_yaml_ng` chosen as the PURE YAML parser for scraper-domain's embedded signal→predicate mapping (Q-DELIVER-1); reqwest reused for adapter-github (no octocrab). | Minimal new dependency surface; license-clean (MIT/Apache); maintained fork of archived serde_yaml. |

## Demo Evidence — 2026-05-28

Built `target/release/openlore`. The `scrape github <target> [--sign N[,N,...]]` verb
surface is visible via `--help` (the help text documents the human-gate: "Absent →
derive + render only, ZERO writes (WD-49 / I-SCR-1)"). The live-GitHub flows (harvest,
derive, sign-via-slice-01, auth/budget/rate-limit/401, token-no-leak) are exercised
end-to-end by the GREEN acceptance subprocess tests that drive the real `openlore`
binary against the in-process `FakeGithub` double — these ARE the captured demo
evidence per story:

| Story | Demo coverage (green acceptance scenario, real binary + FakeGithub) |
|---|---|
| US-SCR-001 (harvest) | SG-1 harvest→propose; SG-2 public-data banner; SG-3 user target; SG-4/5/6 not-found/private/offline |
| US-SCR-002 (derive candidates) | SC-1..5 (each names source signal, 0.25 default, collapse, footer, disagreed-auditable) |
| US-SCR-003 (review/edit/sign) | SS-1 sign-via-slice-01; SS-2 byte-for-byte; SS-3 provenance CID-stable; SS-4/5 input validation; SS-6 decline-publish |
| US-SCR-004 (optional PAT) | SA-1 budget+no-leak; SA-2 unauth small; SA-3 rate-limit→suggest-token; SA-4 rejected-token 401; SA-5 token-no-leak on sign |
| US-SCR-005 (batch sign) | SS-7 batch walk; SS-8 batch skip-continues; SS-9 invalid-list rejected |
| US-SCR-006 (@infrastructure) | the 2 new crates + GithubPort + FakeGithub bootstrapped (Phase 01) |

Human-gate invariant (WD-49 / I-SCR-1) is end-to-end verified: `scrape` without `--sign`
makes ZERO writes (SG-8, SG-9, SC-5, scraper_never_persists_unsigned); public-data-only
(SG-5, KPI-SCR-4); token never leaks to stdout/stderr/signed-claim/PDS (SA-1, SA-4, SA-5).

## Post-Merge Integration Gate — PASS

- Full slice-02 acceptance suite GREEN single-threaded (34 scenarios + support self-tests = 42 test fns); slice-01 + slice-03 suites green (no regression) — 2026-05-28.
- Environment matrix: hermetic (subprocess + FakeGithub + tempfile HOME); no per-environment cross-product dependency (DEVOPS graceful-degrade default applies).
- Known harness flake (NOT a slice-02 regression): `adapter-system-clock` `now_utc_*` env-var contention under full-workspace PARALLEL runs; passes single-threaded / in isolation.

## Quality gates

- `cargo xtask check-arch`: OK (12 workspace members) — scraper-domain pure-core allowlist + GitHub public-only enforcement active.
- `cargo xtask check-probes`: OK (GithubAdapter probe is real; the 1 allowlisted-stub warning is the pre-existing slice-03 peer-storage probe, knowingly accepted at slice-03 review — out of slice-02 scope).
- Per-phase L1-L6 refactor / adversarial review / mutation outcomes recorded below.

## Phase 4 — L1-L6 refactoring (commit 6d45612)

RPP applied (honest "already clean" assessment — minimal warranted): the `adapter-github` auth-report side channel was contained (process-global `OnceLock<Mutex>` → thread-local `Cell`; documented as accepted slice-02 tech-debt because `GithubPort` can't carry `AuthReport` without a contract change); `--sign` batch orchestration extracted out of `run()`. All tests green; check-arch + check-probes + clippy clean. Source writes flowed (the 05-01 first-attempt block was a confirmed transient hook race, resolved by normal re-dispatch).

## Phase 5 — Adversarial review: APPROVED (zero blockers)

@nw-software-crafter-reviewer verdict APPROVED. All scrutinized "no-production-change, just-unskip" clusters (SG-2/4/6/7/8/9, SC-1..5, SS-2/3/4/5, SA-2/3/4/5) confirmed GENUINE (load-bearing port-to-port; deletion-test would red them) — zero Testing Theater. Human-gate (no-write-without-sign + single-publish-path), public-data-only, token-no-leak (double-observable: saw_token + assert_token_value_absent), scraper-domain purity + mapping-SSOT + confidence-no-inflate + auditability, provenance CID-stability — all PASS. All 9 TDD gates satisfied. Auth thread-local = accepted documented tech-debt (single-threaded CLI; revisit when GithubPort widens). parse_selection dup+range fix correct. Test seams contained.

## Phase 6 — Mutation testing (per-feature ≥80%): PASS

cargo-mutants 25.3.1 on the new pure-core `scraper-domain`:

| Target | Mutants | Caught | Missed | Unviable | Kill rate |
|---|---|---|---|---|---|
| `scraper-domain` (derive + mapping) | 20 | 12 | 1 | 7 | **92.3%** (12/13 viable) |

Gate SATISFIED (>80%). The 1 surviving mutant replaces `MappingError`'s `Display::fmt` with a default — i.e. the error-message TEXT for a malformed mapping is not assertion-covered. Low-value: `MappingError` cannot fire in practice (the embedded mapping is build-time SSOT-validated by `mapping_matches_ssot`). Logged for a future test-optimizer pass; not a slice-02 deliverable.

## Phase 7 — Deliver integrity verification: PASS

`des-verify-integrity docs/feature/openlore-github-scraper/deliver/` → "All 39 steps have complete DES traces" (exit 0).
