# Acceptance Test Design — review-app-repos-and-accounts (DISTILL)

- **Designer**: Quinn (nw-acceptance-designer). **Date**: 2026-10-19. **Mode**: subagent,
  interactive low (no questions asked; open points in `wave-decisions.md`).
- **Language**: Rust (`[lang-mode] rust`). **Policy**: `[policy-mode] inherit`
  (`docs/architecture/atdd-infrastructure-policy.md`, one section appended). **State-delta port**:
  `[port-mode] inherit` (`tests/common/state_delta.rs`).
- **Hand-off state**: every scenario is `#[ignore = "DELIVER NN-NN: …"]` (01-* R1, 02-* R2,
  03-* R3). The default run is green (all ignored; existing 184 review-app tests still pass with
  the extended GitHub double). R4 / US-RRA-007 is deferred: no test beyond IC-1's row asserting
  today's owned-only rule for an imported org repo.

## 1. Inputs read

| | Artifact |
|---|---|
| + | `discuss/user-stories.md` (9 active stories, 50 active AC + 4 of deferred US-RRA-007, BR-RRA-1..5, NFR-RRA-1..6) |
| + | `discuss/story-map.md`, `discuss/wave-decisions.md` (D-RRA-1..17) |
| + | `discuss/journey-choose-repos-and-accounts.yaml` + `-visual.md` (wording pins, mockups) |
| + | `discuss/outcome-kpis.md` (KPI-RRA-1..7) |
| + | `design/architecture-design.md`, `component-boundaries.md`, `wave-decisions.md` (D-RRA-D1..D12, user decisions OQ-D1..D6), `upstream-changes.md` (UC-RRA-1..8) |
| + | `docs/adrs/ADR-088-*`, `ADR-089-*` |
| + | `docs/product/architecture/brief.md` § For Acceptance Designer / review-app-repos-and-accounts |
| + | `docs/product/journeys/review-and-publish-my-claims.yaml` (BR-RRA-1..5, steps 3-6, 9-12) |
| + | `docs/product/kpi-contracts.yaml` — no KPI-RRA entry registered yet (upstream issue U-RRA-1) |
| + | `docs/architecture/atdd-infrastructure-policy.md` |
| - | `docs/feature/review-app-repos-and-accounts/devops/` (absent — default matrix: clean, with-stale-config) |
| - | `spike/` (absent — no promoted skeleton) |

## 2. Test files

| File | Layer | Crate / run | Content |
|---|---|---|---|
| `tests/acceptance/review_app_repos_and_accounts.rs` | 4 (real HTTP) | `cargo nextest run -p openlore-review-app --test review_app_repos_and_accounts` | R1: WS-RRA-1, RC-1..7, RS-1, RS-2a/b, RS-3..7, MG-1..3 |
| `tests/acceptance/review_app_repos_and_accounts_several.rs` | 4 | `--test review_app_repos_and_accounts_several` | R2: AA-1..3, AS-1, AS-2/2b/2c, AS-3..7, AR-1..3, AR-4a/b, AP-1..4, AV-1 |
| `tests/acceptance/review_app_repos_and_accounts_imports.rs` | 4 | `--test review_app_repos_and_accounts_imports` | R3: IM-1..3, IC-1..5, IC-6a/b, IC-7, IR-1..4, IV-1..3 |
| `tests/acceptance/review_app_repos_and_accounts_core.rs` | 2 (proptest) | `--test review_app_repos_and_accounts_core` | CORE-RRA-1..10 + CORE-RRA-2x over `repo_scope` / `authority` (RED `sut_*` bindings) |
| `tests/acceptance/review_app_repos_and_accounts_fixture.rs` | — | `--test review_app_repos_and_accounts_fixture` (env-gated) | the v1 store regenerator, its own target, never in a scenario run |
| `tests/acceptance/support/review_app/repos_and_accounts.rs` | — | shared | Typed accounts / repos (Mandate-12), wording pins, page contract, step vocabulary, BR-RRA-1 route forger, universe |
| `tests/fixtures/review-app-store-v1/` | — | fixture | Real v1 store written by the shipped app + golden pending queue (README) |
| `crates/test-support/src/fake_github_accounts.rs` | — | double | + repo ids, owner block, org accounts, `transfer_repo` / `rename_repo` (same-host 301), `archive_repo`, per-account `set_profile_unreachable`, `harvested_repos` / `repo_facts_reads_of` oracles (+3 self-tests) |
| `tests/acceptance/support/review_app/app.rs` | — | harness | + `ReviewApp::start_on_store` / `launch_on_store` (state handed back on refusal), `data_dir`, `stop_keeping_state`; launches retry when the app loses the port race (`cannot listen on`) |

Suites live in `crates/openlore-review-app/Cargo.toml` (DWD-3 precedent: app-only suites) — see
DWD-RRA-1.

## 3. Counts

| Release | HTTP scenario fns | Example rows (tables) | Layer-2 properties | Total test fns |
|---|---|---|---|---|
| R1 (01-*) | 19 | RC-3 ×2, RS-1 ×3, RS-4 ×3 | 4 (CORE-RRA-1, 2, 4, 5) | 23 |
| R2 (02-*) | 22 | AA-2 ×6, AS-3 ×3, AP-2 ×2, AP-4 ×2 | 4 (CORE-RRA-6, 7, 8, 2x) | 26 |
| R3 (03-*) | 18 | IM-1 ×2, IM-2 ×12, IC-1 ×2, IC-3 ×18, IC-5 ×2, IC-6b ×2 | 3 (CORE-RRA-3, 9, 10) | 21 |
| **All** | **59** | | **11** | **70** |

Error / edge / boundary / adversarial / infrastructure scenarios: 39 of 59 HTTP scenarios (66%).
Step reuse (informational): 287 Gherkin step lines over 62 vocabulary functions = 4.6×.

## 4. Story → scenario map

| Story | AC | Scenarios |
|---|---|---|
| US-RRA-001 | 001.1 RC-1, AA-3 · 001.2 RC-2, WS · 001.3 RC-3 · 001.4 RC-1, RC-4 · 001.5 RC-5 · 001.6 RC-6 · 001.7 RC-7 | WS-RRA-1, RC-1..7, CORE-RRA-1/2 |
| US-RRA-002 | 002.1 RS-1, RS-7 · 002.2 RS-2a, RS-2b · 002.3 RS-3 · 002.4 RS-4 · 002.5 RS-5 · 002.6 RS-6 | RS-1..7, CORE-RRA-4/5 (CORE-RRA-3 in R3: it needs import admission) |
| US-RRA-003 | 003.1 AA-1 · 003.2 AA-2 · 003.3 AA-2 · 003.4 AA-2 · 003.5 AA-2 · 003.6 AA-1 · 003.7 AA-3 | AA-1..3, CORE-RRA-8 |
| US-RRA-004 | 004.1 AS-1 · 004.2 AS-2, AS-5 · 004.3 AS-2, AS-2b · 004.4 AS-2b, AS-2c · 004.5 AS-1 · 004.6 AS-4, AS-7, AA-3 · 004.7 AS-6 · 004.8 AS-3 | AS-1..7, CORE-RRA-4/5/7 |
| US-RRA-008 | 008.1 AR-1, AR-2 · 008.2 AR-1 · 008.3 AR-3 · 008.4 AR-4a, AR-4b | AR-1..4b, CORE-RRA-5 |
| US-RRA-010 | 010.1 AP-1, AP-4 · 010.2 AP-3 · 010.3 AP-2 | AP-1..4, CORE-RRA-6 |
| US-RRA-005 | 005.1 IM-1, IM-2 · 005.2 IM-2 · 005.3 IM-1, IM-2 · 005.4 IM-1 · 005.5 IM-3 · 005.6 IM-2 | IM-1..3, CORE-RRA-9 |
| US-RRA-006 | 006.1 IC-1 · 006.2 IC-2, IC-3 · 006.3 IC-4 · 006.4 IC-5 · 006.5 IC-6a, IC-6b, IC-7 | IC-1..7, CORE-RRA-6/10 |
| US-RRA-009 | 009.1 IR-1 · 009.2 IR-2 · 009.3 IR-3 · 009.4 IR-4 | IR-1..4 |
| US-RRA-007 | deferred (D-RRA-12) | IC-1 row `tidal-labs/streamline` asserts today's owned-only rule only |

All 50 active AC are covered (the 4 AC of deferred US-RRA-007 are not tested beyond IC-1's row). Must-cover invariants beyond the AC:

| Invariant | Where |
|---|---|
| BR-RRA-1 over every publish route (fresh owner + bio at preview and confirm) | IC-3 (3 kinds × Approve, Approve selected, Edit preview, Confirm, Confirm selected, Retry), IC-2, AP-1..4, CORE-RRA-6 |
| Private: no owner sees another's suggestions, choices, accounts, imports | AV-1, IV-1 (cross-owner replay with the attacker's own session), IC-4 |
| 10-repo cap incl. default and unverified accounts' ticks; 3-account cap | RC-3, AA-3, AS-4, IM-2, IR-3, IV-3; AA-2 (fourth account); CORE-RRA-1/2/3/8 (OQ-D3: `S` counts unverified accounts' saved ticks) |
| Scan admission with several accounts; resumed scan reads only the frozen plan's rest | AS-1 (one of 6), IM-3, RS-5; RS-2, AS-6, AR-4, IR-4; CORE-RRA-5 |
| Removal deletes pending, keeps declined/published keys, never re-offered | AR-1..4, IR-1..2, IV-2 (purge) |
| Store v1 → v2 tags existing suggestions to their account; idempotent; interrupted update re-runs; backup kept and never overwritten | MG-1, MG-2, MG-3 |
| Rename-safe ids (repo rename keeps the tick) | RS-7 |
| Purge covers the new tables (C-RRA-4) | IV-2 |
| Per-DID scope lock (D-RRA-D8) | IV-3 (two sessions at once) |

## 5. Driving-adapter coverage

| Driving port (brief) | Scenarios |
|---|---|
| GitHub step: add / verify / remove with confirm | AA-1..3, AS-2, AR-1..4, AV-1 |
| `GET/POST` choose repos | RC-1..7, RS-*, AA-3, AS-4, IV-3 |
| `POST` import / remove import | IM-*, IR-*, IV-1, IV-3 |
| Scan again / resume | RS-*, AS-*, IM-3, IR-4 |
| queue, approve, approve-selected, edit-preview, decline | IC-1..3, RS-6, AR-3, IR-2 |
| confirm, confirm-selected, retry | IC-3, IC-5, AP-3, AP-4 |
| profile, share preview | IC-4 |
| Disconnect and forget me | IV-2 |
| startup on an existing store (migration) | MG-1, MG-2 |

## 6. Adapter coverage (Mandate 6)

| Adapter | @real-io scenario | Covered by |
|---|---|---|
| `adapter-review-store` v2 (accounts, choices, imports, plan rows, conditional insert, removal) | YES | every HTTP scenario (real DuckDB); MG-1/2 on a real v1 file |
| `adapter-github` `read_repo` + listing ids | YES (fake HTTP, real adapter) | AP-1 (301 transfer), RS-4, RS-7 (301 rename), IM-2 (404), IC-3 |
| `adapter-atproto-oauth` create-only write | YES | WS-RRA-1, AP-4, IC-3 (forbidden writes = 0) |
| GitHub recorded-fixture contract tests (`read_repo` 200/org/301/404/403) | NO — DESIGN-required CI gate, owned by DELIVER in `adapter-github` | U-RRA-3 |

## 7. Environments (default matrix — no DEVOPS wave)

| Env | Scenarios |
|---|---|
| clean | every scenario starts on a fresh store unless stated |
| with-stale-config: v1 store to migrate | MG-1, MG-2, MG-3 |
| with-stale-config: unverified account | RC-7, AS-2, AS-2c, AS-5, AS-7, AP-3, IM-2 |
| with-stale-config: GitHub 503 / dropped connection / 403 rate limit | RC-5, AA-2, AS-3, AP-2, IM-2, IC-5 |
| with-pre-commit | N/A (no hook surface in this feature) |
