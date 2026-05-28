# Wave Decisions — DELIVER — openlore-federated-read (slice-03)

- **Wave**: DELIVER
- **Date**: 2026-05-28
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (per ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 40 steps, 5 phases, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD (PREPARE→RED_ACCEPTANCE→RED_UNIT→GREEN→COMMIT); review + L1-L6 refactor + per-feature mutation all enabled; models inherit.

This file records DELIVER-wave decisions, demo evidence, and quality-gate
outcomes for slice-03. Implementation detail lives in the code + git history
(every step carries a `Step-ID: NN-NN` trailer).

## Execution summary

All 40 roadmap steps executed via DES-monitored crafter dispatches; `des-verify-integrity`
reports "All 40 steps have complete DES traces". All 35 slice-03 acceptance scenarios
GREEN (PS-1..8, PP-1..8, CC-1..6, FQ-1..8, LCC-1..5); slice-01 suites (walking_skeleton 19,
lexicon_conformance 10, federation_roundtrip 6) show zero regression.

| Phase | Scope | Result |
|---|---|---|
| 01 Bootstrap (01-01..01-07) | ports + adapters + cli + test-support + xtask + lexicon | fail-for-right-reason gate established; all 35 ATs compile RED |
| 02 Lexicon + pure core (02-01..02-05) | LCC-1..5 + normalize_reason + validate_counter_claim | green |
| 03 peer_subscribe (03-01..03-07) | PS-1..8 (US-FED-001 + US-FED-005) | green |
| 04 peer_pull (04-01..04-08) | PP-1..8 (US-FED-002; KPI-FED-6 release gate) | green |
| 05 counter_claim + federated_query (05-01..05-13) | CC-1..6 (US-FED-004) + FQ-1..8 (US-FED-003) | green |

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES stop-hook key-mismatch defect (hook reads `project_id`; `des-init-log` writes `feature_id`) resolved by adding a `project_id` header key to `execution-log.json`; `des-log-phase` preserves it on append. | Unblocks every step's stop-hook without touching the append-only event trail. For future slices: add `project_id` header right after `des-init-log`. |
| DV-2 | Mutation strategy = per-feature ≥80% (Phase 6), matching slice-01 precedent, despite DEVOPS D-D8's nightly-only CI scheduling. | Per-feature gate at deliver-time + nightly delta sweep as backstop. |
| DV-3 | Workspace rustfmt normalization committed as housekeeping (commit ca0ba95) mid-run to clear accumulated multi-file fmt drift and keep the CI fmt gate green. | Each crafter staged only its own files, leaving fmt churn uncommitted; a single chore commit prevents accumulation across 40 steps. |
| DV-4 | Test-only peer-pubkey seam `OPENLORE_PEER_PUBKEY_HEX_<did>` added in adapter-atproto-did (mirrors the endpoint seam); production multibase (`z6Mk…`) key decode is a documented TODO for real PLC resolution. | FakePeerPds's resolveDid DID-doc carries a placeholder key; the seam keeps acceptance hermetic. Real PLC key decode lands when production PLC resolution ships (slice-04+). |

## Demo Evidence — 2026-05-28

Built `target/release/openlore`. Verb surface (`peer add|pull|remove`, `claim counter`,
`graph query --federated`) confirmed visible via `--help`. Runtime golden/edge paths
executed standalone in a tempdir (slice-01 stub env: OPENLORE_HOME, OPENLORE_DID,
OPENLORE_KEY_SEED_HEX):

| Story | Command | stdout (captured) | exit |
|---|---|---|---|
| US-FED-006 | `openlore init --handle jeff.test --app-password fake` | `OpenLore initialized for did:plc:test-jeff` | 0 |
| US-FED-002 (PP-8) | `openlore peer pull` (0 subs) | `No peers subscribed. Run \`openlore peer add <did>\` first.` | 0 |
| US-FED-003 (FQ-4/FQ-6) | `openlore graph query --subject github:rust-lang/rust --federated` (0 peers) | first-federated-query orientation + `No peers subscribed. Use \`openlore peer add <did>\` ...` | 0 |
| US-FED-005 (PS-8 / WD-36) | `openlore peer remove did:plc:rachel-test --purge --no-tty` | `refusing to --purge ... in --no-tty mode ... wait for slice-04's --yes flag.` | 1 |
| US-FED-004 (CC-2 / WD-20) | `openlore claim counter bafyreitest` (no --reason) | `error: the following required arguments were not provided: --reason <REASON>` | 2 |

Live-peer happy paths (US-FED-001 subscribe, US-FED-002 pull-with-records,
US-FED-004 counter compose+sign+publish, US-FED-003 multi-author grouped query) are
captured by the GREEN acceptance subprocess tests that drive the real `openlore`
binary against `FakePeerPds` (PS-1, PP-1, CC-1, FQ-1, FQ-8). No story's Elevator
Pitch is fictional — every pitch maps to demonstrable visible output.

## Post-Merge Integration Gate — PASS

- Full acceptance suite GREEN single-threaded (slice-03 43 scenarios incl. state-delta
  bootstraps + slice-01 35) — 2026-05-28.
- Environment matrix: slice-03 acceptance is hermetic (subprocess + `FakePeerPds` +
  `tempfile` HOME) and does NOT depend on a per-environment fixture cross-product
  (per DISTILL acceptance-tests.md §1 graceful-degradation). The default matrix
  (clean | with-pre-commit | with-stale-config) is satisfied by the hermetic design;
  no env-specific divergence exists.
- Known harness flake (NOT a slice-03 regression): `adapter-system-clock`
  `now_utc_honors_openlore_test_now_env_var` intermittently fails under full-workspace
  PARALLEL runs due to two sibling tests racing on the process-global `OPENLORE_TEST_NOW`
  env var; passes deterministically single-threaded / in isolation. Tracked for a
  future test-isolation fix; the clock crate is untouched by slice-03.

## Quality gates

- `cargo xtask check-arch`: OK (10 workspace members) — anti-merging SQL rule + autoconfirm-release-build guard active.
- `cargo xtask check-probes`: OK (one bootstrap-allowlisted stub warning for the not-yet-live PeerStoragePort gauntlet probe; exit unaffected).
- `cargo deny check`: clean (unicode-normalization MIT/Apache-2.0 covered by existing allowlist).
- Per-phase L1-L6 refactor / adversarial review / mutation outcomes recorded below as those phases run.
