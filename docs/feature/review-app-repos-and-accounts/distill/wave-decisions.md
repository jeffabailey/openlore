# Wave Decisions — DISTILL — review-app-repos-and-accounts

- **Designer**: Quinn (nw-acceptance-designer). **Date**: 2026-10-19. **Mode**: subagent,
  interactive low ("just get it done"): no question was asked; every open point is recorded below.
- `[lang-mode] rust` · `[policy-mode] inherit` (one section appended) · `[port-mode] inherit`.

## Prior-wave reading and reconciliation

DISCUSS, DESIGN (incl. "User decisions" OQ-D1..D6), ADR-088/089, the brief, the SSOT journey and
the KPI contracts were read (checklist in `test-scenarios.md` §1). No DEVOPS wave: the default
environment matrix is used (clean; with-stale-config = v1 store, unverified account, GitHub
unreachable; with-pre-commit N/A).

**Reconciliation passed — 0 blocking contradictions.** Tensions found, each already settled
upstream, and how the tests follow them:

| # | DISCUSS | DESIGN | Settled by | Tests follow |
|---|---|---|---|---|
| R-1 | BR-RRA-4 / AC-008.4: pending of a source removed during a scan "deleted after the scan" | UC-RRA-8: deleted at once, later inserts refused | user OQ-D4 | AR-4, IR-4 assert the observable end state (no pending row of the source after the scan; no further read) — true under both wordings |
| R-2 | BR-RRA-3: scan-start refusal "for example after the value is lowered" | `SCAN_REPO_CAP = 10` constant, not configurable | D-RRA-16, I-03 | the scan-start `OverLimit` is unreachable through pages; covered at layer 2 (CORE-RRA-3/4) |
| R-3 | AC-005.2: a repo "owned by a verified account" is pointed to the repo list | §9 R3: classification "owned by a listed account" | orchestrator D-RRA-T2 | listed (verified or not): IM-2 row "priya-tidal is listed but needs re-verifying", CORE-RRA-9 |
| R-4 | OQ-D2: a moved import follows owned-repo rules | ADR-089 §5: eligibility decided only by the fresh read | user OQ-D2 | IC-6 publishes the moved repo's suggestion after the fresh check |

## Key Decisions

### Decisions settled by the orchestrator under the user's "just get it done" (review cycle 1; the user may override)

| ID | Decision | Tests |
|---|---|---|
| D-RRA-T1 | A scan re-checks the bio of EVERY listed account, unverified included; one that passes is read (and so re-verified) in that scan. Supersedes DWD-RRA-9. | AS-2c, CORE-RRA-4 |
| D-RRA-T2 | Importing a repo is refused ("… is yours. Tick it in your repo list instead.") when ANY listed account owns it, verified or not. | IM-2 (row 8), CORE-RRA-9 |
| D-RRA-T3 | Adding a repo that is already on "Repos you don't own" shows "github:<owner>/<repo> is already in your list." and changes nothing (closes the C2b gap, was U-RRA-2). | IM-2 (row 9) |
| D-RRA-T4 | KPI-RRA-1..7 registration in `docs/product/kpi-contracts.yaml` is DELIVER / finalize work (was U-RRA-1). | — |

### Walking-skeleton strategy (Sentinel F1)

**Strategy B — real local, fake costly/external** (as the project's Architecture of Reference
and `atdd-infrastructure-policy.md` fix it for the review app): the REAL `openlore-review-app`
binary over HTTP (driving port), the REAL `review-app.duckdb` (driven internal), and fakes only
for the external systems — `FakeGithubAccounts` (GitHub REST) and `FakeAtprotoNetwork` (the
owner's PDS + OAuth). Rationale: the feature's risk is in the app's own wiring (choice → frozen
plan → scan → permit → write), which only a real app + real store exercises; GitHub and the PDS
are third-party, rate-limited and non-deterministic, and their real shapes are guarded by the
`adapter-github` contract tests (U-RRA-3) and the DEVOPS nightly live smoke. Strategy A (all
in-memory) would hide the store migration and the per-DID lock; C (all real) needs live GitHub
and a live PDS.

**Environment waiver for the skeleton (Sentinel F7).** WS-RRA-1 runs on the clean environment
only. Each stale-config environment is covered by focused scenarios instead of skeleton
variants: v1 store to migrate — MG-1, MG-2, MG-3; unverified account — RC-7, AS-2, AS-2c, AS-7,
AP-3; GitHub unreachable / rate-limited / dropping connections — RC-5, AA-2, AS-3, AP-2, IM-2,
IC-5. A skeleton per environment would repeat those flows end to end without a new seam.

| ID | Decision |
|---|---|
| DWD-RRA-1 | The four suites are registered in `crates/openlore-review-app/Cargo.toml`, not `crates/cli` as briefed: they drive only the app binary, and the project keeps app-only suites there (bluesky-claim-review-app DWD-3; `review_app_queue_and_publish`, `review_app_identity_link` live there). Run: `cargo nextest run -p openlore-review-app --test review_app_repos_and_accounts[_several\|_imports\|_core]`. `crates/cli/Cargo.toml` is untouched. |
| DWD-RRA-2 | Page contract pins (vocabulary header): `GET /repos`; one form per verified account naming `github.com/<login>` with "Save choice"; repo boxes are checkboxes whose `aria-label` names `<owner>/<name>` (NFR-RRA-5); import field `repo` + "Add repo"; "Remove" forms whose markup names the login / `owner/name`; account removal confirm with "Remove" / "Cancel"; total line "N of 10 repos chosen". Wording pins are the DISCUSS stories verbatim. DELIVER may rename a markup pin with a one-line change in the vocabulary. |
| DWD-RRA-3 | Every malformed `owner/name` (`cargo`, `rust-lang/`, `/cargo`, `a/b/c`, `rust lang/cargo`) gets the one shape message "Enter the repo as owner/name, for example rust-lang/cargo." (IM-2, CORE-RRA-9). |
| DWD-RRA-4 | Import verdict precedence: shape → `read_repo` (not found / unreachable) → owned by a listed account → over limit → admitted (CORE-RRA-9): the shape check costs no GitHub request, the limit refusal is only given for a real, importable repo. |
| DWD-RRA-5 | "A scan is running" is modelled as a scan paused for GitHub's budget (`set_remaining(120)`), the existing deterministic posture; by BR-RRA-4 the resume is the same scan. Used by RS-2, AS-6, AR-4, IR-4. |
| DWD-RRA-6 | The migration fixture is the real v1 `review-app.duckdb` + `.wal` written by the shipped app (29 KB), not an `EXPORT DATABASE` dump: the review-app crate has no DuckDB dev-dependency, and the store's encrypted fields need the harness's fixed data key. Env-gated regenerator in the R1 suite. |
| DWD-RRA-7 | Layer-2 properties live in `tests/acceptance/review_app_repos_and_accounts_core.rs` with `todo!()` `sut_*` bindings (identity_link_core precedent; RED, not BROKEN, with zero production scaffolding). DELIVER may move each into `crates/review-domain/tests` / `crates/claim-domain/tests` when binding it. |
| DWD-RRA-8 | Scan-intent refusal order when both hold (nothing to read AND over the limit): either refusal is accepted (CORE-RRA-4). |
| DWD-RRA-9 | *Superseded by D-RRA-T1* (a scan re-checks unverified accounts too). |
| DWD-RRA-10 | No Tier B state machine: the op-sequence properties CORE-RRA-3/5 drive the pure core; the journeys are example-chained at layer 4 (Mandate 10 conditions only partly met: rich chains, but the input space is small and typed). |
| DWD-RRA-11 | Mandate 8 at layer 4: preservation and bounded-change scenarios assert through `capture_rra` (universe: PDS claims, PDS write attempts, pending queue, "Not yours" cards, accounts, imports) with `assert_state_delta`; other layer-4 Thens use traditional assertions (allowed at layer 4). |
| DWD-RRA-12 | `FakeGithubAccounts` extended additively (repo ids, owner block, organizations, transfer/rename 301, archive, per-account profile 503, harvest/facts oracles). All 184 existing review-app tests pass with it. "Repos a scan read" = repos harvested past their root (`/repos/{o}/{r}/…`); a bare `GET /repos/{o}/{r}` is the ownership read. |
| DWD-RRA-13 | The harness's closed log catalogue (`world.rs`) gains the `ownership.` prefix for the D-RRA-D11 audit events, so shipped suites keep passing once DELIVER logs `ownership.publish_check.passed`. |
| DWD-RRA-15 | Port race (Platform #1): the app must know its origin (`APP_ORIGIN`, OAuth client id) before it starts, so it cannot bind `:0` and report back without a production change. The harness instead retries a launch up to 6 times when the app refuses with `cannot listen on` (fresh ports; the same ports, after a pause, on a restart, since the origin must not change). All 186 shipped review-app tests pass with it. |
| DWD-RRA-16 | MG-3 uses a development-build fault seam `REVIEW_APP_TEST_FAULT=migrate_before_commit` (the v1 → v2 transaction stops just before COMMIT and the process exits). DELIVER: honour it only in debug builds and add it to the xtask release-build env guard (like `REVIEW_APP_CLOCK_SKEW_SECS`), so a release build refuses to start with it. |
| DWD-RRA-17 | One When per scenario (Sentinel F2): setup moved into Givens; RS-2, AS-2, AR-4 and IC-6 split into a/b(/c) scenarios with their own ignore ids (01-18, 02-20..22, 03-18). RC-6: the harness cannot press keys, so the When is the request a keyboard activation submits, and the Thens assert keyboard operability (accessible names, no focus reordering, button after the boxes). |
| DWD-RRA-18 | Wording pins added where DISCUSS gives content but not a sentence: `LISTING_RATE_LIMITED` (RC-5), the RS-4 skip lines ("a fork since you chose it. Untick it." / "missing or owned by someone else since you chose it. Untick it."), "Forks and archived repos are skipped. Skipped: 3 forks." (RC-4), the failed-check summary line (journey mockup 4); AA-2 row 1 reuses the shipped "We couldn't find your DID in the bio of …" sentence. |
| DWD-RRA-14 | BR-RRA-1 "@property" at layer 4 is an exhaustive table (3 kinds of not-owned suggestion × 6 routes, forms captured from the owner's own card and re-aimed), not proptest (Mandate 9/11); the generated rule table is CORE-RRA-6. |

## Test Coverage Summary

| Release | HTTP scenario fns | Layer-2 properties | Total |
|---|---|---|---|
| R1 (01-01..01-19) | 19 | 4 (CORE-RRA-1, 2, 4, 5) | 23 |
| R2 (02-01..02-22) | 22 | 4 (CORE-RRA-6, 7, 8, 2x) | 26 |
| R3 (03-01..03-18) | 18 | 3 (CORE-RRA-3, 9, 10) | 21 |
| **All** | **59** | **11** | **70** (+ the fixture regenerator, its own target) |

- Story coverage: 9/9 active stories, all 50 active AC (54 incl. the 4 of deferred US-RRA-007;
  map in `test-scenarios.md` §4).
- Error / edge / boundary / adversarial / infrastructure: 39/59 HTTP scenarios (66%).
- Tables (parametrize density): RC-3 ×2, RS-1 ×3, RS-4 ×3, AA-2 ×6, AS-3 ×3, AP-2 ×2, AP-4 ×2,
  IM-1 ×2, IM-2 ×12, IC-1 ×2, IC-3 ×18, IC-5 ×2, IC-6b ×2. Proptest on every unbounded pure rule (CORE-RRA-1..10, 256 cases each).
- `@kpi`: WS-RRA-1 (KPI-RRA-1), RC-2/RC-3, AA-1 (KPI-RRA-2), IM-1 (KPI-RRA-3), AP-1/AP-4, IC-2
  (KPI-RRA-4), AS-2 (KPI-RRA-5), RS-5, IR-1, IC-6, AR-1 — via the D-RRA-D11 counters at
  `/admin/kpi` and the `ownership.publish_check.passed` log event.

### AT-completeness (15 items): **15/15 — COMPLETE**

| Item | | Evidence |
|---|---|---|
| C1a empty/zero | ✓ | RS-5 (empty choice), RC-4 (no eligible repo), IM-3 (no account) |
| C1b boundaries | ✓ | RC-3 (10/11/12), IM-2 (10 + 1), AA-2 (3 + 1 accounts), AA-3 / AS-4 (room), IV-3, CORE-RRA-1/2 |
| C2a state machine documented | ✓ | module docs: choice (R1), account (R2), import (R3) |
| C2b illegal event per state | ✓ | re-add a listed account (AA-2), add an already-imported repo (IM-2, D-RRA-T3), import a repo a listed account owns (IM-2), remove/save foreign rows (AV-1, IV-1), list for an unverified account (RC-7) |
| C3 0/1/many | ✓ | accounts 0/1/2/3, imports 0/1/2, choice 0/5/10 |
| C4a apply twice | ✓ | AA-2 already listed, MG-2 idempotent upgrade, RC-3 repeated Save, IR-2 / AR-3 re-add |
| C4b inverse without prerequisite | ✓ | AV-1 / IV-1 remove of rows the session does not own, RS-5 scan with nothing |
| C5a mode combinations | ✓ | IC-3 (kind × route), RS-1 (saved vs default), CORE-RRA-4 (check × listing) |
| C5b flag orthogonality | ✓ | RS-6 + CORE-RRA-7 (unticking changes reads, never visibility) |
| C6a malformed input per param | ✓ | IM-2 (5 malformed names), AV-1 (foreign account and repo ids), IC-2 (forged route) |
| C6b each declared error | ✓ | every add / import / Save / scan / publish refusal; `RepoGone` and scan `OverLimit` at layer 2 |
| C6c closed error set | ✓ | CORE-RRA-6 (permit), CORE-RRA-8 (add), CORE-RRA-9 (import) |
| C7a degraded resource | ✓ | GitHub 503 / dropped connection / 403 rate limit at list, add, import, scan, preview, confirm (AA-2, AS-3, AP-2, IM-2, IC-5 rows) |
| C7b interruption | ✓ | RS-2a, AS-6, AR-4a/b, IR-4 (pause + resume), MG-2 (restart), MG-3 (update cut off before commit) |
| C7c concurrency | ✓ | IV-3 (two sessions of one DID at once, D-RRA-D8) |

Completeness audit log: all categories 0 findings (the cycle-0 C2 finding closed by D-RRA-T3).
No `at-completeness-gap-log.md` (verdict ≥ 13).

### Mandate-12 (Rust adaptation)

- CM-I-1: typed domain module = `support/review_app/repos_and_accounts.rs` (`Account` enum with
  ids, `Repo`, `repos::*` constants, `PublishRoute`, wording pins) + the shared `domain.rs`.
- CM-I-2: vocabulary functions take `Account` / `Repo` / `Suggestion` / `PublishRoute`, never raw
  logins (raw `&str` only for typed-by-the-user input: `when_imports`, `when_adds_login`).
- CM-I-3: scenario bodies call vocabulary steps; the only control flow is table iteration.
- CM-I-4 (informational, re-measured after cycle 1): see `test-scenarios.md` §3; the ratio rose
  slightly (more Givens per scenario, the same vocabulary).

### Pure functions DELIVER binds (SA medium; indicative, the crafter owns exact signatures)

| Property | Binding (one call) | Shape |
|---|---|---|
| CORE-RRA-1 | `review_domain::repo_scope::default_ticks(accounts, imports, cap) -> Vec<(AccountId, Vec<RepoId>)>` | pure |
| CORE-RRA-2 / 2x | `repo_scope::admit_save(scope, account, ticked, cap) -> Result<(), LimitRefusal { limit, total, excess }>` | pure |
| CORE-RRA-3 / 9 | `repo_scope::admit_import(scope, cap)`, `repo_scope::classify_import(typed, RepoFacts read, listed ids, scope, cap) -> ImportVerdict` | pure |
| CORE-RRA-4 | `repo_scope::scan_intent(scope, cap) -> ScanIntent` + `repo_scope::plan_account(account, check, listing) -> AccountPlan` | pure |
| CORE-RRA-5 | `repo_scope::resume_plan(plan rows, read, checks, removed sources) -> (to_read, skipped)` | pure |
| CORE-RRA-6 | `claim_domain::authority::may_sign(signer, subject, listed ids, &RepoFacts, &profile) -> Result<SigningPermit { signer, subject, basis: Owner(id) }, SigningRefusal>` | pure (ADR-089 §2) |
| CORE-RRA-7 | `repo_scope::visible(rows, accounts, imports) -> Vec<bool>` | pure |
| CORE-RRA-8 | `repo_scope::admit_account(listed, id, held_by_other_did, listed_unverified, max) -> AddAccount` — the pure decision the `GithubAccountsWrite` adapter then applies (SA medium: no longer bound to the write port) | pure |
| CORE-RRA-10 | `repo_scope::moves(scope, repo, account) -> Scope` | pure |

These are listed here for DELIVER rather than in `design/component-boundaries.md` §2, which
already names the module and its outputs.

## Review Gate Result

cycle 2: SA (cycle 1), PO and PA approved; Sentinel's wording residuals fixed without a third review (2-cycle limit; user: "just get it done")

## Review gate

### Cycle 1 (2026-10-19)

SA approved; PO, PA and Sentinel rejected. Every blocker and high is fixed; every medium is
fixed (no waiver needed); lows fixed.

| Reviewer | Finding | Sev | Resolution |
|---|---|---|---|
| PO | F1 removal during a scan must delete at once (OQ-D4) | high | AR-4a asserts no pending row of the removed account right after removal while the scan is still paused; AR-4b asserts the resumed scan neither reads nor inserts for it. `discuss/user-stories.md` BR-RRA-4 and AC-008.4 reworded to "at once" (citing OQ-D4) |
| PO | F2 03-09 must prove the fresh check by outcomes | high | IC-6a (the move) + IC-6b table: owner + bio hold → published; bio removed after the move → refused, nothing written |
| PO | F3 unverified account's saved ticks count | medium | AS-7 (HTTP) + CORE-RRA-2x (example); `arb_scope` documented to generate unverified accounts with saved choices |
| PO | F4/F5 AC count, outcome titles | low | "50 active + 4 deferred AC"; MG-1/MG-2 and CORE-RRA-6 renamed to outcomes |
| PA | 1 port race in `free_port` | high | harness retries on `cannot listen on` (DWD-RRA-15); 186 shipped tests pass |
| PA | 2 interrupted upgrade | high | MG-3 with the dev-only fault seam (DWD-RRA-16, U-RRA-8) |
| PA | 3 backup name / content / never overwritten | medium | MG-1 asserts `review-app.v1.<secs>.bak` and that the copy, started alone, shows the golden queue; MG-2 and MG-3 assert the first copy's bytes unchanged |
| PA | 4 regenerator safety | medium | own test target, non-DELIVER ignore reason, scratch dir, `duckdb` CLI schema-v1 check, writes only `*.regenerated` |
| PA | 5 drop / 403 rows mid-scan and mid-publish | medium | AS-3 ×3, AP-2 ×2, IC-5 ×2, AA-2 and IM-2 rows |
| Sentinel | F1 WS strategy | blocker | Strategy B declared above with rationale |
| Sentinel | F2 one When per scenario | high | DWD-RRA-17 (splits and Given moves at the listed lines) |
| Sentinel | F3 RC-3 unchanged step on refused rows only | high | RC-3 now refused rows only; the 10-repo save is its Given; "scan reads 10" moved to RS-1 row 3 |
| Sentinel | F4 RC-6 keyboard | high | When = the keyboard-submitted request; harness limit stated |
| Sentinel | F5 outcomes, not read counts | high | AP-1/AP-4 assert pages, writes and audit events; read counts kept only as labelled "secondary" checks; AP-4 adds the transferred-between-preview-and-confirm row |
| Sentinel | F6 unique property ids | high | ignore reasons now `CORE-RRA-N (bind with DELIVER step …)` |
| Sentinel | F7 WS stale-config Givens | high | waiver above, pointing to the covering scenarios |
| Sentinel | F8–F12 | medium/low | full messages in Gherkin (AA-2, RS-4, RC-4, RC-5, AS-2, AS-5, IM-1, IM-2, IC-5); WS KPI and "nothing written before confirm" asserted; RS-5 phases in Gherkin; "PDS"/"session"/"replays" replaced by business words; vocabulary header states the backup observation; IM-1 `kind` column used; `@property` only on CORE-RRA-*, IC-3 tagged `@exhaustive-table`; page-render scenarios tagged unbounded-preservation |
| SA | CORE-RRA-8 bound to a write port | medium | rebound to the pure `repo_scope::admit_account` decision |
| SA | PermitView lacks signer and basis | medium | `Permit { signer, subject, basis: Owner(id) }` in view and oracle |
| SA | list bound signatures | medium | table "Pure functions DELIVER binds" above |

## Upstream Issues

| ID | Issue | routes_to |
|---|---|---|
| U-RRA-1 | KPI-RRA-1..7 are not registered in `docs/product/kpi-contracts.yaml`; the @kpi scenarios bind to the D-RRA-D11 counter names. Settled: DELIVER / finalize registers them (D-RRA-T4). | DELIVER |
| U-RRA-2 | *Resolved* by D-RRA-T3 (message for an already-imported repo). | — |
| U-RRA-3 | The DESIGN-required recorded-fixture contract tests for `read_repo` (200 user / 200 org / 301 rename / 301 transfer / 404 / 403) and the paged listing are adapter tests, not acceptance: owned by DELIVER in `adapter-github`. | DELIVER |
| U-RRA-4 | ADR-088 §5.2 item 6 migration case "a DID with suggestions and no `github_links` row" cannot be produced through the v1 app's pages; it belongs in DELIVER's `adapter-review-store` migration test (SQL-seeded v1 file). | DELIVER |
| U-RRA-5 | *Resolved* by D-RRA-T1 (a scan re-checks unverified accounts). DESIGN §6.2 step 3 should say so. | DESIGN (text) |
| U-RRA-6 | *Resolved* by D-RRA-T2 ("listed"); AC-005.2's "verified" wording should follow. | DISCUSS (text) |
| U-RRA-8 | MG-3's fault seam `REVIEW_APP_TEST_FAULT=migrate_before_commit` must be debug-only and refused by a release build (xtask guard). | DELIVER |
| U-RRA-7 | The listing's 3-page cap (300 repos) and the truncated-listing `read_repo` fallback have no acceptance scenario (the double serves one page); a layer-2 / adapter test in DELIVER covers it. | DELIVER |

### Cycle 2 (2026-10-19)

SA (cycle 1), PO and PA approved. Sentinel: needs_revision on wording only; fixed as below
without a third review (two-cycle limit, the user's "just get it done").

| Reviewer | Finding | Resolution |
|---|---|---|
| Sentinel F2 | one action per When (IR-2, IR-3, IV-1, IV-2, IV-3, MG-1, AA-1, AV-1) | setup moved into Givens; IV-1's Dmitri observation is a Given; IV-3's When is one action (submit both tabs at once); AV-1 is an Outline, one request per row, Priya's state checked after each |
| Sentinel F4 | RC-6 When wording | "submits Save choice with … ticked, as her space-bar toggles and Enter would" — matches the body |
| Sentinel F9 | technical words in Examples / Gherkin | "API rate limit exceeded" → "GitHub says it is too busy to answer"; MG-1 says "one dated copy of her data", the file-name pattern stays in the body |
| Sentinel F10 | backup observation claim | stated in the `support/review_app/mod.rs` and vocabulary headers |
| PO lows | stray AS-7 mapping; AS-7 tidal choice; IC-6a Approve | AS-7 removed from AC-001.3, AA-3 tagged @AC-001.1; AS-7 re-verifies priya-tidal and checks its 5 ticks unchanged; IC-6a asserts the card offers Approve and no longer says "Not yours to publish" |
| PA notes | readiness, env table, restart retry | ready only if our process is still running after `/healthz` answers; `REVIEW_APP_TEST_FAULT` and `REVIEW_APP_CLOCK_SKEW_SECS` in the env contract table; `restart()` documents its same-port retry |

Red run after cycle 2: 70 failed / 0 passed, every failure MISSING_FUNCTIONALITY, first lines
unchanged; default run 186 passed.
