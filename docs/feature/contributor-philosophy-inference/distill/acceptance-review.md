# Acceptance Review — contributor-philosophy-inference (DISTILL + consolidated gate)

- **Wave**: DISTILL · **Date**: 2026-09-27 · **Designer**: Quinn (nw-acceptance-designer)
- **Subject**: 61 scenarios / 5 `.rs` files + `support/people.rs` + `FakeGithub` `/contributors`
  extension + `distill/{test-scenarios,walking-skeleton,red-classification}.md` + the DISTILL
  sections of `feature-delta.md`.
- **Gate**: consolidated end-of-DISTILL review across DISCUSS + DESIGN + DISTILL, reviewers
  dispatched in parallel (no DEVOPS wave → no platform reviewer).

## Consolidated gate — 2026-09-27

| Reviewer | Scope | Verdict | Blockers / high / low |
|---|---|---|---|
| Eclipse (nw-product-owner-reviewer) | DISCUSS + UC ratification | conditionally approved → **approved after fix** | 1 / 0 / 0 |
| Architect (nw-solution-architect-reviewer) | DESIGN + ADR-063/064 + tests vs wire contract | **approved** | 0 / 0 / 0 |
| Sentinel (nw-acceptance-designer-reviewer) | DISTILL tests + docs | **approved** (avg 9.3; CM-A/B/C pass) | 0 / 0 / 0 |

**Blocking finding fixed (PO)**: slice-05 AC "exactly one GitHub request is made for a
user-target scrape" vs the shipped user scrape, which GETs `/users/{user}` twice (resolve +
harvest). SP-5 originally asserted only "no repo crawl". **Fix**: SP-5
(`reading_a_person_asks_github_only_for_their_profile_once`) now asserts
`seen_paths == ["/users/BurntSushi"]` — the AC holds as written; DELIVER collapses the
double-fetch (see "Existing tests DELIVER must narrow" #3). Re-review not needed: the fix
adopts the reviewer's option (a) verbatim.

**Architect cross-check (recorded)**: tests stay at the CLI driving port; evidence ordering,
bare-DID AT-URIs, `supersedes` reference, and hundredths arithmetic match ADR-064/DDD-9/10;
reading the DDD-5 `contribution_links` columns directly for slice-01 observations and pinning
the `supersedes` reference as preview-only (not an editable prompt) are acceptable.

### PO ratification of DESIGN-introduced edge cases

All **ratified** by the PO reviewer (consistent with D-1..D-9 / OD-CPI-1..7), encoded as:

| UC | Scenario | UC | Scenario |
|---|---|---|---|
| UC-1 N=0 → no request | CL-4 | UC-5 weakened: unsubscribed / missing | EG-5, EG-6 |
| UC-2 403 too-large / 204 empty → notice, exit 0 | CL-9, CL-10 | UC-6 case-insensitive `github:` join | IP-9 |
| UC-3 `--contributors` rejected on a person | CL-12 | UC-7 AT-URI + commits-URL evidence | IS-2, WS |
| UC-4 hand-authored never STRONGER | EG-9 | UC-8 numbering follows filters | IS-6 |
| DISTILL addition IP-14 (`--person` must be `github:<login>`) | IP-14 | slice-05 "exactly one request" | SP-5 |

**Action for the product owner (Luna)**: fold UC-1..UC-8 + IP-14 into the story ACs in
`feature-delta.md` at DELIVER finalize (they are ratified here; the story text still shows the
DISCUSS wording).

## Designer self-review (critique dimensions 1-9) — fixes applied before the gate

```yaml
review_id: accept_rev_2026-09-27_contributor-philosophy-inference
reviewer: acceptance-designer (self-review)
issues_identified:
  happy_path_bias:
    - {severity: none, note: "error 13 + edge 20 + boundary 5 = 38/61 = 62% (>= 40%)"}
  gwt_format:
    - {severity: none, note: "house Rust convention: GWT doc-comment + // GIVEN/WHEN/THEN; one CLI invocation per When (SP-2 compares two surfaces by design)"}
  business_language:
    - {severity: none, note: "test fn names are domain sentences (person, repo, claim, sign, retract, supersede); no HTTP/SQL/JSON terms"}
  coverage:
    - {severity: none, note: "every AC of US-CPI-001..005 + KPI-CPI-2..5 + UC-1..8 + Q-CPI-D1/D3/D4/D7 traced (test-scenarios.md §4)"}
  walking_skeleton:
    - {severity: none, note: "WS-CPI-1 is a user goal; Then observes the signed claim a peer would read; RED = missing verb after all Givens succeed"}
  observable_behavior:
    - {severity: none, note: "universe = CLI output, FakeGithub request log, claim artifacts, claims/peer_claims counts, DDD-5 link columns; assert_store_unchanged/assert_store_delta are fail-closed (Mandate 8)"}
  fixture_theater:
    - {severity: high, status: FIXED, issue: "canonical inference store used the 30-person ripgrep list, so every rg-dev would also be a candidate and 'exactly 1 candidate' / 'valid range 1..3' would fail for a fixture reason", fix: "canonical store scrapes core-maintainer lists (burntsushi_only); realistic lists stay in slice-01 and the WS"}
    - {severity: high, status: FIXED, issue: "CL-12 passed today only because --contributors does not exist (vacuous GREEN)", fix: "Then also requires the refusal to say the flag is for owner/repo targets"}
    - {severity: high, status: FIXED, issue: "EG-7 'no hint' passed today because no hint exists (vacuous GREEN)", fix: "Then first requires 'Contributors recorded: 1'"}
  red_gate:
    - {severity: none, note: "0 BROKEN; all failures MISSING_FUNCTIONALITY; SP-4 intentionally GREEN-today (regression guard); see red-classification.md"}
approval_status: approved
```

## Existing tests DELIVER must narrow (Q-CPI-D5)

None was edited in DISTILL; all stay green today (verified: `scrape_github`, `scrape_sign`,
`scrape_auth`, `scrape_candidates`, `publish_roundtrip` suites + `openlore-test-support` unit tests).

1. **`tests/acceptance/scrape_github.rs` SG-3**
   `scrape_github_resolves_user_target_and_derives_no_candidates_aggregation_deferred` asserts
   `"No candidate claims could be derived"` and "no numbered list" for a user target. Slice-05
   replaces that path with the person view → when activating SP-3, rewrite SG-3's Then to the
   person-view guidance (`github:torvalds is not linked to any repo you've scraped`).
2. **Gate `scraper_never_persists_unsigned`** — `assert_no_claim_persisted`
   (`tests/acceptance/support/mod.rs`) already checks only the `claims` table + PDS + claim files,
   so its assertions stay valid; narrow its doc comment ("ZERO observable persistence") and the
   titles `scrape_github_harvests_public_repo_proposes_candidates_and_persists_nothing` /
   `scrape_github_is_a_pure_read_persisting_nothing_across_repeated_runs` to "persists no
   claim" (contribution links are now written; `FakeGithub` default `[]` keeps them at 0 rows
   there, so no assertion changes).
3. **User-target request count (SP-5)** — the shipped user scrape GETs `/users/{user}` twice
   (`resolve_target` + `harvest_user`). SP-5 requires exactly one; the auth/rate report the
   `scrape_auth.rs` user-target scenarios read (`scrape_auth_authenticated_harvest_reports_budget_and_never_leaks_token`,
   `…unauthenticated_small_target…`) must then come from the single response — keep them green.
4. **Request-count assertions** — none exist beyond SG-5's public-allowlist check
   (`scrape_github_refuses_private_target_and_calls_no_private_endpoint`), which
   `/repos/{o}/{r}/contributors` satisfies. Nothing to narrow for the +1 request.
5. **`[n]` numbering** — `scrape_github_is_a_pure_read_persisting_nothing_across_repeated_runs`
   counts `[n]` markers and `scrape_candidates.rs` slices candidate blocks on `"[3]"`: the
   contributors block, overlap lines and hint must not print `[n] ` markers.
6. **Viewer** — `viewer_invariants.rs::viewer_is_read_only` counts `claims` + `peer_claims`
   only. The viewer `/scrape` must NOT record contribution links (the viewer holds no write
   port); if DELIVER shares the harvest function, add `contribution_links` to that universe.

## Conditions carried to DELIVER

1. Activate one scenario at a time in the order in `feature-delta.md` § Activation order; WS first.
2. Land the unit-level properties listed in `test-scenarios.md` §7 (pure `scraper-domain::people`).
3. Items 1-6 above.
4. Outcome registry: `docs/product/outcomes/registry.yaml` exists only as an untracked, empty
   file not created by this wave; registration was skipped. Candidate rows: OUT `infer people`
   (operation), `scrape github --contributors` (operation), contribution-link append-only
   (invariant), person-adherence provenance in `evidence[]` (invariant).

**Gate result: PASSED** — handoff to DELIVER.
