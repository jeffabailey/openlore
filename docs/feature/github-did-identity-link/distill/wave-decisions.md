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
