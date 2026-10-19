# Wave Decisions — DISTILL — github-did-identity-link

- **Wave**: DISTILL · **Date**: 2026-10-14 (final pass after review iteration 2 and the user's rulings) · **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Inherits**: DESIGN GDL-D1..D14, A-1..A-10, C-GDL-1..10, ADR-084, ADR-085 (ACCEPTED 2026-10-14; revised by owner-only-signing UC-1..8 / ADR-086/087)
- `[lang-mode] rust` · `[policy-mode] inherit` (`docs/architecture/atdd-infrastructure-policy.md`: 2 rows appended, the `FakeGithubAccounts` row extended) · `[port-mode] inherit` (`tests/common/state_delta.rs`)
- Map: `test-scenarios.md` (incl. the one statement of the CI policy, traceability, environment waivers) · WS: `walking-skeleton.md` · RED gate: `red-classification.md`

## Prior-wave reading and reconciliation

- **No DISCUSS wave** (WARN): AC derived from DESIGN; scenarios trace to decisions (`test-scenarios.md` §3).
- **No DEVOPS wave** (WARN): the default matrix is N/A for these verbs (waivers in `test-scenarios.md` §6); the required GitHub REST contract test is owned below (DD-GDL-11).
- **Reconciliation passed — 0 contradictions.** The open questions DESIGN left (Q-GDL-D3/D5/D6) were ruled on by the user on 2026-10-14 (D-a/D-b/D-c); the user added D-g.

## Decisions

D-a, D-b, D-c and D-g are **user-ratified (2026-10-14)**. D-d, D-e and D-f are review-driven DISTILL
decisions the user can override. Each is pinned in the tests.

| # | Decision | Status | Pinned by |
|---|---|---|---|
| D-a (Q-GDL-D3) | Re-linking an account the same DID already has an Active link to is refused with "already linked", pointing to `openlore identity verify`; nothing is signed or published. Closes C2b and C4a. | user-ratified | IL-5 (`assert_already_linked`) |
| D-b (Q-GDL-D5) | `claim counter` on a `sameAs` link is ALLOWED: a counter needs no authority (OOS-D1), never counts in scoring, and — its author not being the subject DID — is never projected as a link (GDL-D3). DESIGN amended (dated "Amended 2026-10-14 (user, DISTILL)"): ADR-084 §4, design `wave-decisions.md` C-GDL-2, `kpi-contracts.yaml` KPI-GDL-1, and the crafter brief line in `architecture/brief.md`. | user-ratified | IL-8c (wire JSON author ≠ subject, `counters` reference, `identity list` shows no link) |
| D-c (Q-GDL-D6) | Search uses ONE label for `github:` rows, links included: "authority not checked here" (owner-only-signing UC-7). | user-ratified | IR-16, `SEARCH_LABEL` |
| D-g (new) | Linking an account again after retracting the link is allowed and publishes a fresh link (new record ID). Lifecycle: Retracted + link → new Active link. | user-ratified | IL-14; lifecycle doc in `identity_link.rs` |
| D-d (CI) | Hand-off ignore policy — stated once, in the header of `test-scenarios.md`. | DISTILL | `test-scenarios.md` |
| D-e (SA C-1, Sentinel B-2) | PRIMARY Thens observe through driving ports (`identity list`/`verify` output and exit, person views, review-app pages, PDS wire JSON); "a check was recorded" via the `identity list` verdict/age line; the store universe is a SECONDARY guardrail (waiver below). | DISTILL | all scenarios |
| D-f (SA C-2) | The signed record is asserted from the PDS wire JSON (real `decode_claim_record`); printed CID == PDS record key; "no link produced" is asserted over the PDS record set. | DISTILL | `assert_link_on_pds`, `assert_no_link_on_pds` |

### Mandate-8 waiver (explicit)

Mandate 8 asks every state-mutating test at layers 1-3 to assert through a port-exposed universe.
These scenarios run at layers 3-4 and observe through driving ports first (D-e). The secondary
guardrail reads `claims` / `peer_claims` row counts, claim files, PDS record count, `contribution_links`
and the DESIGN-pinned `identity_link_checks` columns (`subject_did`, `github_user_id`, `login_seen`,
`verdict`; timestamps excluded). Reading those tables directly is a deliberate waiver following house
precedent: `store_universe_slots` / `capture_store_universe` and `contribution_links` in
`tests/acceptance/support/people.rs` (contributor-philosophy-inference DDD-5). It is fail-closed (any
unexpected change in the universe fails) and never the only evidence for a Then.

## Key Decisions

| # | Decision | Rationale |
|---|---|---|
| DD-GDL-1 | House framework: Rust `#[test]` files under `tests/acceptance/`, Gherkin in doc-comments, `#[ignore = "…"]`. Four targets in `crates/cli/Cargo.toml`, one in `crates/openlore-review-app/Cargo.toml`; all compile today. | CPI / review-app / indexer-deployment precedent. |
| DD-GDL-2 | Walking skeleton WS-GDL-1, **strategy C** (REAL `openlore`, `adapter-github` over HTTP, `adapter-duckdb`, filesystem; FAKE `FakeGithubAccounts`, `FakePds`, `FakeIdentity`, `PeerPds`). Ignored at hand-off per D-d. | Thinnest end-to-end slice of user value. |
| DD-GDL-3 | `FakeGithubAccounts` is the GitHub double for every root; extended (test-support only): `GET /user/{id}`, `OldLogin::{NotFound, Redirects, RedirectsToAnotherHost}`, `delete_account`, `register`, `serve_stale_profile`/`end_stale_profile`, `request_paths`; 4 unit tests. `support::people::Invocation` gains `github_token` (sets `GITHUB_TOKEN`). | ADR-085 Earned Trust lie catalogue; platform review (401, cross-host). |
| DD-GDL-4 | GitHub request paths are observed (precedent `assert_target_record_read_once`). Request counts are bounded (IV-2: 1; IV-4: ≤ 2, no id read); exact order only in the two-request rename case (IV-3a) and the one-request follow-up (IV-3b). | SA M-1. |
| DD-GDL-5 | Layer split (Mandate 9): example-only at layers 3-4, table loops with row labels in every assertion message; proptest only in `identity_link_core.rs` with ADR-text oracles. CORE-4 uses the REAL `decode_claim_record` / `canonicalize` / `compute_cid`; CORE-6 the REAL `scoring::score`. | SA H-2. |
| DD-GDL-6 | Tier B not added: `link_view` is a pure projection; FORALL properties CORE-5/5b cover the space. `sut_link_view` documents the fold of `link_view(links, lineages, checks)` (lineage → lifecycle). | Mandate 10; SA M-3. |
| DD-GDL-7 | Wording is DISTILL-owned, pinned once in the support modules. Scenario text speaks plain language; refusals and verify lines must carry BOTH the verdict token (`Verdict::token()`) and the plain phrase (`Verdict::plainly()`). Refusals exit 1; usage errors exit 2 and name the flag. | PO F10; Sentinel H-2; SA L-3. |
| DD-GDL-8 | One subscribed peer per store (`peer pull` resolves every subscription). | Found while classifying RED. |
| DD-GDL-9 | Pillar 2: multi-When scenarios were split so each has one When; the later scenario's Given replays the earlier Given+When (IL-9a→9b, IV-1→IV-2, IV-3a→3b, IF-1a→1b, IR-1→IR-2, IR-3→IR-4, IR-8a/8b, RA-1a→1b, RR-1a→1b, RR-2a→2b). In IR-5/6/7 the re-check is now a Given. | Sentinel H-4. |
| DD-GDL-10 | Owner-only-signing is out of scope. IL-8b, IL-11, IR-11 and IR-17 seed `github:` repo claims with `claim add` / CPI givens; they join the ADR-086 seeded-check migration list. | Separate later feature. |
| DD-GDL-11 | **Contract test owner — accountable role: the DELIVER crafter for slice 1**, in the `adapter-github` `read_person_by_id` step — a plain `#[test]` over recorded fixtures `crates/test-support/fixtures/github/users_profile.json` and `user_by_id.json` (shape `login`, `id`, `bio`). Not written now (also recorded in the infra-policy row). | Platform review. |
| DD-GDL-12 | **Migration**: v7 is forward-only and idempotent. IL-13a/b run against a REAL pre-v7 store: `tests/fixtures/identity-link-pre-v7-store/` (a DuckDB `EXPORT DATABASE` dump, 68 KB, made 2026-10-14 by the current binary whose schema head is v6, plus the rest of the home and a golden read-back). IL-13a asserts `schema_version` gains v7 exactly once and every earlier row is unchanged; IL-13b asserts a reopen changes nothing. The fixture must stay frozen: its regenerator refuses unless the schema head is v6. **Downgrade**: an older binary refuses a v7 store through its existing startup probe; no AT, because the test build has no older binary to run cheaply — documented only. | Platform review. |
| DD-GDL-13 | The identity verbs ignore GitHub's `github-authentication-token-expiration` header (it only matters to the review app's operator warning). A rejected token (401) is "GitHub unavailable, nothing recorded", token never echoed (IL-6, IV-6, RA-5). | Platform review. |
| DD-GDL-14 | **Core activation order** for DELIVER: CORE-1/1x with the matcher hoist; CORE-2 with `check_link` (slice 1, before IV-3a); CORE-4 with the `ProvenLink → LinkDraft` codec (slice 1, before WS-GDL-1 goes green); CORE-3 with `project`; CORE-5/5b with `link_view`; CORE-7 with the reserved-predicate guard (before IL-8a); CORE-6/6x with the scorer drop (slice 2, before IR-14). | Platform review. |
| DD-GDL-16 | **DELIVER gate for the layer-2 bindings (M-10)**: every `sut_*` in `identity_link_core.rs` is a `todo!()` placeholder except `sut_score`. Before a CORE property is un-ignored, its binding must be replaced by ONE call into the real `claim_domain::identity_link` function (mapping its ADTs onto the view types, no logic in the binding); a CORE property may not go green against a stub. When ADR-086/087 moves the `sameAs` drop into `scoring::score_attested`, re-point `sut_score` to it in the same commit (CORE-6/6x must keep passing). | Sentinel M-10. |
| DD-GDL-15 | Mandate 4 (pure function extraction): the decisions are already extracted into `claim-domain` by DESIGN and tested at layer 2 (CORE-*); layer 3-4 scenarios test only adapters and wiring. | Sentinel M-12. |

## Test Coverage Summary

- **75 logical scenarios** (a table counts once): `identity_link.rs` 32 (+1 fixture regenerator, not a scenario), `identity_link_readers.rs` 19, `identity_link_core.rs` 10, `review_app_identity_link.rs` 9, `review_app_identity_link_readers.rs` 5.
- **Mix — informational only, not a gate**: WS 1 · happy 15 · error 20 · edge 18 · boundary 1 · guardrail 10 (65 example scenarios, 75 % non-happy) + 10 layer-2 properties/catalogues.
- **Coverage claim**: every decision, constraint and assumption with behaviour observable through a driving port maps to a scenario; structural ones (A-3 sequencing, A-8 non-editability, C-GDL-9, GDL-D7, GDL-D13) are named as such — `test-scenarios.md` §3 "Constraints and assumptions → scenarios".
- **KPI**: KPI-GDL-1 (WS, IL-3/5/8a/8b/8c, IR-3/4/6/7, RA-2, RR-3, CORE-1/3/5/7) · KPI-GDL-2 (IR-14, IR-15, CORE-6/6x) · KPI-GDL-3 (IV-1, IR-12, RR-2b) · KPI-GDL-4 analog (IR-2, RR-1b).
- **Active at hand-off**: IR-13, IR-17, RA-3 (all pass). Everything else is ignored and RED for a business reason (`red-classification.md`).

### AT-completeness (15 items)

| Item | Pass | Evidence |
|---|---|---|
| C1a empty/zero/min | ✓ | no / blank bio (IL-3); zero links (IV-8); empty feed (CORE-6); forget with no history (IF-2) |
| C1b boundaries | ✓ | 30 / 31 days, future check (IR-9); 40-char login (IL-7) |
| C2a state machine documented | ✓ | `identity_link.rs` module doc |
| C2b illegal event per state | ✓ | NoLink+verify (IV-8), NoLink+forget (IF-2), Active+link again (IL-5, D-a), Retracted+verify (IR-8b); Retracted+link is legal (IL-14, D-g) |
| C3 0/1/N | ✓ | 0/1/2 links (IV-8, WS, IV-7), two accounts per DID (IL-10, IV-9), duplicates (IR-10) |
| C4a apply twice | ✓ | link twice (IL-5), link after retract (IL-14), verify twice (IV-2), forget twice (IF-1b), reopen (IL-13b), CORE-4 |
| C4b inverse without prerequisite | ✓ | forget with no history (IF-2) |
| C5a mode flags | ✓ | `--yes`/declined (WS/IL-2); `--did`/`--github`/both/none (IV-7); `list --did` (IR-18); offline (IR-11) |
| C5b orthogonality | ✓ | IV-7: filters never change a verdict |
| C6a malformed input | ✓ | login (IL-7), `--did` (IF-2), flawed records (IR-7, CORE-3) |
| C6b each declared error | ✓ | five verdicts, rate limit, 401, outage, bad redirects, reserved predicate, already linked, CSRF, expired sessions |
| C6c closed error set | ✓ | CORE-2; IV-5 asserts only the recorded verdicts |
| C7a degraded resource | ✓ | IL-6, IV-6, IV-10, RA-5, IR-11 |
| C7b interruption | ✓ | IL-9a/b (PDS down after signing), RA-8 (session ends at confirm) |
| C7c concurrent actors | ✓ (N/A) | single-actor local CLI; insert-only checks |

**Score: 15 / 15 → COMPLETE.** Audit log: all categories 0 findings after revision.

### Mandate-12 (Rust adaptation)

Typed values in `support/identity_links.rs` (`Account`, `Verdict`, `Shown`, `Peer`, `LinkFlaw`, `AccountFate`, `GithubTrouble`, `CheckRow`) and `Persona` for the review app; scenario bodies hold no business logic. **Step/helper reuse ratio (informational): 5.4×** (369 helper calls in scenario files / 68 helpers used; 74 defined).

## Review Gate Result

**Iteration 1 (2026-10-14): all four reviewers — PO, solution architect, platform architect, Sentinel — returned `rejected_pending_revisions`.** Changes made in response:

| Finding | Resolution |
|---|---|
| D-a … D-f (orchestrator) | Fixed — see the decisions table above. |
| SA C-1 / Sentinel B-2 (internal state as primary evidence) | Fixed — D-e; Mandate-8 waiver recorded. |
| SA C-2 (signed record read from local file) | Fixed — D-f. |
| SA H-1 (IV-5 vacuous `identity_changed` scan) | Fixed — IV-5 asserts `identity verify` output and the `identity list` line. |
| SA H-2 (CORE-4 not through the real codec) | Fixed — real decode / canonicalize / compute_cid, CID determinism, project round trip. |
| SA M-1 (exact request order) | Fixed — bounded counts; exact order only in IV-3a/IV-3b. |
| SA M-2 (`used_id_read`) | Fixed — removed; read counts are observed at layer 3. |
| SA M-3 (`link_view` signature) | Documented the fold in `sut_link_view`. |
| SA M-4 (non-author origin) | Fixed — RR-3 (link in Sam's repo: not projected, GitHub never asked). |
| SA M-5 ("verified" matching "unverified") | Fixed — `says_verified`. |
| SA L-1 (split IL-8 infer row) | Fixed — IL-8a / IL-8b / IL-8c. |
| SA L-2 (`let _ = try_press`) | Fixed — removed; the confirm is pressed (the fresh read is pinned to the publish handler, Q-GDL-D8). |
| SA L-3 (one status per refusal) | Fixed — refusals exit 1, usage 2, CSRF 403. |
| SA L-4 (fake doc comment) | Fixed — `set_user_id` doc. |
| SA L-5 (IR-13 vague negative check) | Fixed — `Shown::Nothing` for the bio DID + no `openlore identity link` suggestion. |
| Platform: v6→v7 upgrade AT | Fixed — IL-13. |
| Platform: downgrade | Documented (DD-GDL-12); no AT (older binary not cheaply reachable). |
| Platform: 401 + anonymous rate-limit rows | Fixed — IL-6, IV-6 (3 rows), RA-5 (401 row). |
| Platform: RA-8 expired sessions | Fixed — RA-8 (app session, PDS session). |
| Platform: contribution_links preservation | Fixed — IL-11. |
| Platform: contract test owner, CORE activation, token-expiration header | Documented — DD-GDL-11, DD-GDL-14, DD-GDL-13. |
| PO F1 (second account) | Fixed — IL-10, IV-9. |
| PO F2 (no link data in logs) | Fixed — IL-12 (CLI, 3 verbs); RA-1b (review-app logs: no DID, login or bio). Waived: the CLI has no telemetry channel to check. |
| PO F3 (rename flag never links) | Fixed — IR-17 (active). |
| PO F7 (`list --did`) | Fixed — IR-18. |
| PO F8 (chained / cross-host redirect) | Fixed — IV-10. |
| PO F10 (plain-language verdicts) | Fixed — DD-GDL-7. |
| PO F11 (outcome titles) | Fixed — titles rewritten as outcomes. The reviewer's exact suggested wordings were not in the revision brief, so the rewrites are DISTILL's own. |
| PO F12 (IL-8 infer exit unspecified) | Fixed — closed by the 8a/8b/8c split; IL-8b now states and asserts exit 1 in text and code. |
| PO F13 (RA-5 message not quoted) | Fixed — the scenario quotes "GitHub is rate-limiting us" and "Nothing was published.". |
| PO F14 (IV-7 trailing line outside the outline) | Fixed — already its own scenario, IV-8 (Given/When/Then). |
| Sentinel B-1 (WS strategy) | Fixed — strategy C declared (DD-GDL-2, walking-skeleton.md). |
| Sentinel H-2 (usage errors) | Fixed — exit 2 + `--did` named (IV-7, IF-2); IF-2 row 1 asserts "Forgot 0". |
| Sentinel H-3 (IR-14) | Fixed — asserts success and non-empty weighted rows, compares only ranked rows. |
| Sentinel H-4 (multi-When) | Fixed — DD-GDL-9. |
| Sentinel H-5 (traceability) | Fixed — `test-scenarios.md` §3. |
| Sentinel H-6 (environment waivers) | Fixed — `test-scenarios.md` §6. |
| Sentinel M-9 (tag recount), M-12 (Mandate 4), row labels in table loops | Fixed. |
| Sentinel M-2 (asymmetric exit in IL-8) | Fixed — IL-8a and IL-8b both pin exit 1; IL-8c (allowed counter) exits 0. |
| Sentinel M-3 (hidden Givens in the composer When) | Fixed — `when_i_compose_same_as_via` is gone; seeding (Rachel's link, BurntSushi's repos) is in each scenario's Given; each When is one composer action. |
| Sentinel M-4 (weak review-app assertions) | Fixed — (a) the exact record address `at://<DID>/org.openlore.claim/<record key>` (`then_sees_record_address`, RA-1b and `given_link_published`); (b)/(d) already done; (c) RA-5 also asserts "Nothing was published.". |
| Sentinel M-5 (row labels) | Fixed — `assert_shown_in` / `assert_link_store_unchanged_in` carry the row label; used in IL-3, IL-6, IL-7, IV-5, IV-6, IV-10, IF-2, IR-7, IR-9. |
| Sentinel M-6 (missing Givens) | Fixed — IL-7 and IL-8a have Givens (IL-8a now seeds the bio); IR-4's doc matches its code (Jeff's link, then Rachel's). |
| Sentinel M-7 (jargon) | Fixed — Gherkin text says "published repo" / "repo host" and "record ID"; tags and CLI flags unchanged. |
| Sentinel M-8 (untagged constraints/assumptions) | Fixed — mapping table in `test-scenarios.md` §3; structural items named. |
| Sentinel M-10 (`todo!()` bindings) | Fixed — DELIVER gate DD-GDL-16. |
| Sentinel M-11 (sizing) | Fixed — slices 1 and 2 tagged `@sizing-review-needed` in `test-scenarios.md` (informational). |

Two iteration-1 rows above were NOT true in code when first claimed and were corrected only in the
final pass: RA-5 "Nothing was published." (Sentinel M-4c / PO F13) and IL-13 "v6 upgrade" (it used a
store made by the current binary, not a frozen pre-v7 store). Iteration-1 departures accepted by the
orchestrator: IR-17 un-ignored as a passing guard; the fresh profile read pinned to the confirm request.

**Iteration 2 (2026-10-14)**: platform architect — *conditionally approved*; PO, solution architect
and Sentinel — *rejected / needs_revision*. Escalated to the user, who ruled on 2026-10-14: D-a
confirmed, D-b ratified (with the DESIGN amendments above), D-c confirmed, D-g added.

**Final pass (2026-10-14), to be verified by the orchestrator against code:**

| Item | Resolution |
|---|---|
| User D-b | ADR-084 §4, design C-GDL-2, KPI-GDL-1 (and the crafter brief) amended with dated notes; IL-8c asserts the counter's wire JSON (author ≠ subject, `counters` → link) and that `identity list` shows no link for it. |
| User D-g | IL-14 + lifecycle line in the `identity_link.rs` module doc. |
| 1 IL-11 | Primary Then = BurntSushi's person view (`scrape github BurntSushi`) repo lines before/after; verify exit asserted; `contribution_links` only as guardrail. |
| 2 IL-8b | `read_signed` loop replaced by `identity list` showing no link lines + `assert_no_link_on_pds`. |
| 3 IL-13 | Real frozen v6 store (DD-GDL-12); IL-13a (v7 recorded once, prior rows unchanged, golden read-back) and IL-13b (reopen). |
| 4 RA-5 | Both rows assert "Nothing was published."; the token row pins "GitHub did not accept the app's access"; the app's token is asserted absent from the page HTML. |
| 5 RA-8 | New row: server-side expiry via `restart_app_later(31 days)` (clock-skew seam) with the cookie kept; cookie-cleared and repo-host-session rows kept. |
| 6 CORE-4 | `sut_encode_link` returns the production record JSON; it is fed unmodified to `decode_claim_record`; header updated. |
| 7 | `assert_verify_reported` uses `says_verified` for Verified; IV-7 asserts Verified positively per account. |
| 8 IR-17 | Precondition: BurntSushi's candidate block flags `possible rename: github:burnt-old`; no-link check uses `link_lines` (line shape `did:… is|claims github:…`); "never asked" narrowed to the GitHub accounts double. |
| 9 | IL-8a/8b text: "it is refused and he is told to use openlore identity link" (exit 1 in code). IL-9a: "repo host" in text and in the assertion (Q-GDL-D1 pin). |
| 10 | `assert_link_refused` and `assert_verify_reported` also require `Verdict::plainly()`. |
| 11 RA-3 | Doc comment records that verify-then-scan is one user flow. |
| 12 | Titles rewritten: IV-4, IV-10, RA-6, RR-2a, RR-2b (test names follow). |
| 13 IL-12 | Text and helper state stderr is the CLI log channel; stdout is checked for the bio and for log events. |
| 14 Docs | D-d stated once (`test-scenarios.md`); commands, date and results in `red-classification.md`; DD-GDL-11 names the accountable role; mix marked informational; this section. |

## Open questions (remaining DISTILL defaults)

| ID | Question | Default pinned |
|---|---|---|
| Q-GDL-D1 | Output wording | in `support/identity_links.rs` / `support/review_app/identity_link.rs`; incl. "already linked", "Forgot N", "No linked identities", "authority not checked here", the `Verdict::plainly()` phrases, "repo host" on the identity verb's publish failure, and the review app's "GitHub did not accept the app's access" |
| Q-GDL-D2 | Confirmation UX | `--yes`, or interactive `y`; anything else declines |
| Q-GDL-D4 | A future-dated check beside an older one | the future check is ignored |
| Q-GDL-D7 | Reserved predicate match | exactly `sameAs` |
| Q-GDL-D8 | Review app fresh read | in the publish handler (confirm request), now pinned (RA-1b, RA-2, RA-5) |
| Q-GDL-D9 | `account_not_found` login seen | the login that was read |

## Upstream Issues

1. **DELIVER** — the GitHub REST recorded-fixture contract test (DD-GDL-11); the fixtures do not exist yet. A-10 stays partly unverified until then.
2. **Owner-only-signing migration list** — add IL-8b, IL-11, IR-11, IR-17 (DD-GDL-10). The frozen v6 fixture (IL-13a/b) holds a `github:` repo claim signed before owner-only-signing; it must keep reading as before (it is a prior row).
3. **CI** — `review_app_identity_link_readers` needs `cargo build -p openlore-review-app` first (as `review_app_walking_skeleton`).

## Slice 4

DISTILL 2026-10-19 · Quinn · `[lang-mode] rust` · `[policy-mode] inherit` (one row appended to
`docs/architecture/atdd-infrastructure-policy.md`: the viewer's three link read handles, the fault
seam and the damaged-store probe) · `[port-mode] inherit` (`tests/common/state_delta.rs`).
Map: `test-scenarios.md` § Slice 4 · WS: `walking-skeleton.md` § Slice 4 · RED:
`red-classification.md` § Slice 4.

**Reconciliation passed — 0 contradictions.** DISCUSS absent (WARN; US-VL-1..3 in UC-11 stand in);
DEVOPS absent (WARN; default matrix, with-pre-commit N/A). DESIGN slice 4 status PROPOSED; the user
started the slice, open questions defaulted as the orchestrator stated (no `/peers` line, no
`/identities` page, legend and notice as proposed). Checked against slices 1–3: U-19/U-20 (VL-7 can
re-follow: IR-20c shows `peer add` re-follows), U-39 (VL-6), U-6 (VL-3/17), U-7 (VL-11). One existing
test needed narrowing, not a contradiction (D-VL-T5).

### Key Decisions

| # | Decision | Rationale |
|---|---|---|
| D-VL-T1 | Same house framework: Rust `#[test]` files, Gherkin in doc comments, two targets (`viewer_linked_identity`, `viewer_linked_identity_invariants`), each test `#[ignore = "DELIVER 04-NN: …"]`, walking skeleton included (D-d). The `04-NN` numbers are a proposed order (04-01 WS + VL-12 … 04-08 guardrails). | Slices 1–3 precedent; roadmap may renumber. |
| D-VL-T2 | **Density**: 19 VL scenarios + 1 property in 16 test fns. Same-shape scenarios are table loops with the row label in every message: VL-2 (9 standings, + VL-18), VL-3 (spellings), VL-4 (pages), VL-5 (stranger + 5 flaws × 4 pages), VL-7 (lifecycle rows), VL-9 + VL-14 (3 reads + probe × pages), VL-11 + VL-19, VL-8 + VL-16, VL-12 (5 pages incl. malformed params), VL-13 (5 pages). The unbounded wording space is one layer-2 proptest (VL-2p, 256 cases). | MAX-PBT + parametrize; Mandate 9 (proptest only at layer 2). |
| D-VL-T3 | **Parity by construction**: a page is read back as an `identity list` listing (`region_listing`), so the slice-2 `assert_shown`/`assert_check_age`/`Shown` vocabulary is reused verbatim; VL-2 and VL-15 also compare the page's lines with a real `identity list --did` run at the same pinned instant, backticks removed. | D-VL-5 "character for character"; Mandate-12 reuse. |
| D-VL-T4 | **Markup the tests read (DISTILL-owned, one place: `support/viewer_identities.rs`), selected by role, not class (review VL-M1)**: the region is the `<section>` labelled (`aria-labelledby`) by a heading reading "Linked identities"; its lines are the `<li>` of a `<ul>`; the legend is text inside the section; an unreadable region keeps its heading and shows only the notice; line text = element text with tags removed; DID → `href="/score?contributor=<%-encoded>"`, account → `href="/project?subject=<%-encoded github:login>"` (ADR-044); on `/search` a region sits inside each author group; the link-row label sits inside its row. | The design fixes wording and accessibility (labelled section, list), not class names; one helper keeps DELIVER free to adjust in one place. |
| D-VL-T5 | **IR-15 rewritten** (`identity_link_readers.rs`, review VL-M2): it compared the whole `/score` results fragment; slice 4 puts Rachel's line on that page by design (D-VL-1). It now selects the SCORED pairings by their own selector (`scored_pairings`: the `<section>`s carrying a `Weight:` line), asserts they are identical with and without the link, that `github:rachelk` is not scored, and that the linked-identities region holds no scored pairing. Passes today. | KPI-GDL-2 is about scoring, not about the page carrying a line. |
| D-VL-T6 | **Clock**: the viewer reads `now` through the same clock seam as the CLI identity verbs (`OPENLORE_TEST_NOW`, development builds; `adapter-system-clock`). Tests pin it for VL-2, VL-15. | Design: "now = wall clock at the request edge (as the CLI's identity verbs)". |
| D-VL-T7 | **Fault seam (VL-9)**: one debug-only seam with a target value, `OPENLORE_VIEWER_FAIL_IDENTITY_LINKS=checks\|candidates\|lineage`, failing that read on every page request and **never** the startup probe (VL-9 asserts no refused-probe event). Pinned by VL-G, an ignored case in `xtask/tests/viewer_fail_seam.rs` that fails until DELIVER 04-07 appends the token to `VIEWER_FAIL_SEAM_TOKENS` (`xtask/src/check_arch.rs`). Documented limit (review F-7): real per-request damage is not reproducible in the subprocess harness. | Design allows "one seam with a target value"; release-forbidden like every viewer seam (D-VL-11). |
| D-VL-T8 | **Probe refusal (VL-14) is a real condition**, not a seam: the check table is dropped while its schema version stays (`given_my_proof_history_cannot_be_read`), so the read-only probe (`read_listed_links`) fails at startup. Needs at least one stored link (the read skips checks when no link is stored). Observed through the `viewer.identity_links.probe {refused:true}` startup event (`ViewerServer::startup_lines`) and the notice on all four pages, incl. a person with no link and `/search`. | Earned Trust over a real damaged store (C7a). |
| D-VL-T9 | **GitHub never asked**: every viewer is started with `OPENLORE_GITHUB_API_BASE` = the `FakeGithubAccounts` double, so any request the viewer made would be in its log; VL-1 and VL-8 assert the log gains nothing. VL-16 sets the double offline and asserts identical lines. | C-VL-2 observable through a port, not by absence of wiring. |
| D-VL-T10 | **Every negative carries a positive anchor** so it is RED today for a business reason, never vacuously green: VL-5/6 (Rachel's one sound line), VL-7 (followed rows), VL-12 (Jeff's line), VL-17 (old-login line before re-check), VL-19 (folded with VL-11). | Pre-DELIVER gate; Fixture-Theater guard. |
| D-VL-T11 | **VL-2p binding and oracle waiver (review F4)**: `sut_render_region` is `todo!("RED: bind to viewer_domain::linked_identity …")` (DD-GDL-16 practice). Its oracle is the internal `cli::render::render_linked_identities` — waived because a layer-2 property cannot spawn a process per case; that function is the text the `identity list` driving port prints (pinned through the binary by IR-1..IR-18b), and the subprocess cross-check of the same parity is VL-2 (10 rows) and VL-15, which compare the page with a real `openlore identity list --did` run at the same instant. | Mandate 7 RED-not-BROKEN; one property per wording space. |
| D-VL-T12 | Tier B not added: the region is a pure projection already explored by VL-2p and CORE-5; no command protocol. | Mandate 10. |
| D-VL-T13 | **"Changes nothing" through driving ports (review VL-B1)**: VL-8's PRIMARY Thens are the pages, the GitHub double's request log and Jeff's `identity list` output before vs after browsing. The fail-closed store universe (`assert_link_store_unchanged_in`) stays only as a labelled SECONDARY guardrail under the D-e waiver, never the Then. | D-e; read-only surface observed where the user observes it. |
| D-VL-T14 | **Checks come from the real verb (review VL-M3)**: VL-2's contested and proof-gone rows are recorded by `identity verify --github <login>` against the GitHub double (bio names both DIDs; bio drops the DID; names a stranger's DID; no bio; account deleted). Direct SQL remains only where no verb can produce the state: VL-15 (a check dated in the future) and VL-14 (a damaged store). | Pillar 3: state through production verbs. |
| D-VL-T15 | **Clock pinned for "today" (review F-5)**: every scenario that asserts an age pins both ends — link/verify at `LINKED_AT`, viewer at `SAME_DAY` (VL-1, VL-2, VL-15, VL-17a, VL-17b, VL-20). Viewer stdout AND stderr are drained on their own threads (stderr kept for the startup-failure message) with a 20 s startup deadline (review F-6 and cycle-2 stderr finding), so neither pipe can fill or close under the viewer and a hung start fails fast. | Determinism. |
| D-VL-T16 | **Gherkin hygiene (Sentinel F1–F3, F6–F9)**: placeholders match Examples headers exactly; record IDs are named R1..R3 in the Given; expected lines are written in full; VL-4 carries a DID-valued column; `@property` removed from VL-8 (example-only); VL-17 split into VL-17a (Then: the line stays) and VL-17b (Given = 17a's Given + When; When = re-check); every page-list When is one action on one `<page>` from the Examples (VL-5, VL-6, VL-7, VL-17a, VL-20; VL-8/VL-9 keep "browses the pages about …" as one action over a stated page set); VL-17b's Given is a state ("has not re-checked since, and has the old login's page open"), not 17a's Then; VL-2's Given is conditional-free ("Jeff's latest check of rachelk is <latest check of rachelk>", `none` = never checked); VL-5/VL-6 state exact expected lines with named records R1/R2; VL-4 is one DID per row; "identity list" is kept as a product-surface term (the CLI command the reader knows; `test-scenarios.md` § Slice 4). | Sentinel review. |

### Test Coverage Summary

- **Total**: 21 VL scenarios (VL-1..20, VL-17 split into a/b) + 1 layer-2 property (VL-2p) + 1
  xtask guard case (VL-G) in **19 test fns** (13 in `viewer_linked_identity.rs`, 5 in
  `viewer_linked_identity_invariants.rs`, 1 in `xtask/tests/viewer_fail_seam.rs`); 1 walking
  skeleton (VL-1).
- **Framework**: Rust `#[test]` + Gherkin doc comments; `proptest` (256 cases) for VL-2p only.
- **Integration approach**: REAL `openlore ui` subprocess over the REAL DuckDB store seeded by the
  REAL CLI verbs; REAL `openlore-indexer` for `/search`; fakes only at the external edge
  (`FakeGithubAccounts`, `PeerPds`, `FakePds`, `FakeIdentity`, `FakeIngestServer`).
- **Error/edge share**: 17 / 21 scenarios (81 %).
- **KPI**: KPI-GDL-1 (VL-1, VL-4, VL-5); KPI-GDL-2 unchanged (IR-15 narrowed, D-VL-T5). No new KPI.
- **Helper reuse (Mandate-12, informational)**: the two scenario files call 64 distinct support
  helpers 245 times (≈ 3.8×, a natural ceiling for 18 table-driven fns); 36 of them come from the
  slice-1/2 and house vocabulary (`given_i_linked`, `given_peer_published`, `assert_shown_in`,
  `assert_check_age`, `records_named`, `capture_link_universe`, …). Typed values: `PersonPage`, `LinkRead`,
  `GithubStanding`, `ViewerOpts` plus the slice-1/2 `Account`, `Verdict`, `Shown`, `Peer`, `LinkFlaw`.
- **AT-completeness: 15 / 15 → COMPLETE** (two items N/A with rationale):

| Item | Pass | Evidence |
|---|---|---|
| C1a empty/min | ✓ | no link → no region, empty store (VL-12); 0 records (VL-2p) |
| C1b boundaries | ✓ | 31 days → hint, contested past 30 days → no hint (VL-2/18); future-dated check (VL-15); ages 0–90 days (VL-2p) |
| C2a state machine documented | ✓ | `viewer_linked_identity.rs` module doc |
| C2b illegal event per state | ✓ | hidden states shown nowhere: retracted/superseded (VL-6), removed peer (VL-7), third-party/flawed (VL-5); contested never "is" (VL-4); stale line after rename (VL-17); reads failing in any state (VL-9) |
| C3 0/1/N | ✓ | 0 (VL-12), 1 (VL-1), N lines and two DIDs (VL-2, VL-4), one record and two records (VL-2), 0–3 records and 1–5 links (VL-2p) |
| C4a apply twice | ✓ | pages opened twice with an unchanged store read the same (VL-8/16); remove → re-follow (VL-7) |
| C4b inverse without prerequisite | ✓ (N/A) | the viewer has no inverse operation; nearest: a page for someone who never linked (VL-12) |
| C5a mode flags | ✓ | page shape full/fragment (VL-13); index configured or not (VL-11/19); GitHub up/down (VL-16); each failing read (VL-9) |
| C5b orthogonality | ✓ | shape (VL-13), GitHub standing (VL-16) and login spelling (VL-3) never change the lines |
| C6a malformed input | ✓ | five flawed records (VL-5); a non-username login and a non-DID contributor (VL-12) |
| C6b each declared error | ✓ | each of the three reads (VL-9), probe refused (VL-14), index unavailable (VL-19) |
| C6c closed error set | ✓ | VL-9/14: only the notice; no line, no standing word |
| C7a degraded resource | ✓ | damaged store (VL-14), store made before identity links (VL-20), GitHub unreachable (VL-16), no index (VL-19) |
| C7b interruption | ✓ | a read failing after a passed startup probe, mid-request (VL-9) |
| C7c concurrent actors | ✓ (N/A) | slice 4 adds no concurrency claim; the viewer holds the store alone (one process at a time, as the slice-6 harness) |

Audit log (falsifier telemetry): `(github-did-identity-link/slice-4, C1..C7, findings 0 after
authoring — C6a found at self-audit and closed by two VL-12 rows, severity_max LOW)`.

### Review Gate Result

**Cycle 1 (2026-10-19): all four reviewers rejected.** Every finding addressed; no waivers taken.

| Finding | Resolution |
|---|---|
| PO F-1 (high) VL-9 rows omit `/search` | `/search` added to every per-request row (an index per row) |
| PO F-2 (med) no single-record not-checked line | VL-2 row: Rachel's `rachel-old` link reads `… — not checked (record R3)` |
| PO F-3 (low) VL-13/14 titles | retitled as outcomes (`test-scenarios.md`, Gherkin) |
| Architect VL-B1 (blocker) store snapshot as the Then | D-VL-T13 |
| Architect VL-H2 / Platform F-1 (high) seam token not guarded | VL-G + DELIVER note (D-VL-T7, `red-classification.md`) |
| Architect VL-M1 (med) class pins | D-VL-T4 (heading/role, `aria-labelledby`, `ul`/`li`) |
| Architect VL-M2 (med) IR-15 cut by `rfind` | D-VL-T5 |
| Architect VL-M3 (med) seeded proof-gone rows | D-VL-T14 |
| Platform F-2 (high) pre-v7 store | VL-20 |
| Platform F-3 (med) empty store | VL-12 row group 2 |
| Platform F-4 (med) per-request rows and the probe | VL-9 asserts no refused-probe event per row |
| Platform F-5 (med) clock for "today" | D-VL-T15 |
| Platform F-6 (med) stdout pipe and deadline | D-VL-T15 |
| Platform F-7 (low) per-request real damage | documented limit (D-VL-T7, VL-9 doc) |
| Sentinel F1–F3, F6–F9 | D-VL-T16 |
| Sentinel F4 (high) VL-2p internal oracle | D-VL-T11 |
| Sentinel F5 (high) WS and with-stale-config | documented N/A for the skeleton (`test-scenarios.md`, `walking-skeleton.md`) |
| Sentinel F10 (low) regression seed from a `todo!()` | deleted (`viewer_linked_identity_invariants.proptest-regressions`); proptest re-writes it on every `--run-ignored` run until VL-2p is bound — do not commit it |
| Sentinel F12 (low) mix count | recounted per scenario ID |

Cycle 2: see "Review gate (slice 4)" below.

### Upstream Issues

1. **DESIGN** — the `cli`/`viewer-domain` dependency line was corrected in the design by the
   orchestrator; DELIVER chooses the VL-2p binding route (D-VL-T11).
2. **DESIGN (SPECIFICATION_AMBIGUITY, defaulted)** — the region's markup and where it sits on
   `/search` when reads fail are not specified. Defaulted: one region per author group, each showing
   the notice when unreadable (VL-14 search row); markup per D-VL-T4. Route to DESIGN if a single
   page-level notice is preferred.
3. **DESIGN (SPECIFICATION_AMBIGUITY, defaulted)** — the fault-seam value set and its scope (never
   the startup probe) are DISTILL's choice (D-VL-T7); DELIVER 04-07 adds the token to the xtask
   guard and un-ignores VL-G. Also defaulted: an unreadable region keeps its heading (D-VL-T4).
4. **DELIVER** — whichever step lands the region must keep IR-15 green (it now cuts the region out,
   D-VL-T5) and re-run the viewer suites that compare whole `/score` or `/project` fragments.
5. **CI** — the `/search` rows (VL-11, VL-13, VL-14) need `cargo build -p openlore-indexer` first,
   as the other `/search` suites.

## Review gate (slice 4)

| Cycle | PO (Eclipse) | Solution architect | Platform architect (Forge) | Sentinel | Outcome |
|---|---|---|---|---|---|
| 1 | rejected | rejected | rejected | rejected | all findings fixed (table under § Slice 4 › Review Gate Result) |
| 2 | **approved** | **approved** — VL-H2 carried to DELIVER 04-07 (append `OPENLORE_VIEWER_FAIL_IDENTITY_LINKS` to `VIEWER_FAIL_SEAM_TOKENS`, un-ignore VL-G) | residuals: F-5 clock in VL-17b/VL-20; viewer stderr not drained | residuals: F1/F2 placeholders (VL-4, VL-7); F3 exact lines (VL-5, VL-6); F4 waiver cites the oracle line; F7 VL-17b Given; F8 one action per When; N1 conditional Given in VL-2 | all residuals fixed in this pass (D-VL-T11, T15, T16) |

No third review: the process allows two cycles and the user asked to proceed. Hand-off to DELIVER
with PO and SA approval and the PA / Sentinel cycle-2 residuals closed as listed; the current red
run is in `red-classification.md` § Slice 4.

