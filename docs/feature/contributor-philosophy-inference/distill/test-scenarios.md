# Acceptance Test Design — contributor-philosophy-inference (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-09-27 · **Designer**: Quinn (nw-acceptance-designer)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- **Language**: Rust (`[lang-mode] rust`) · skip marker `#[ignore = "…"]` · PBT lib proptest (layer 1-2 only — DELIVER)
- **Framework**: Rust std `#[test]` + `assert_cmd` subprocess, GWT in doc-comments + `// GIVEN/WHEN/THEN` (house convention, same as `scrape_sign.rs` / `publish_roundtrip.rs`)
- **Inherits**: DISCUSS (D-1..D-9, OD-CPI-1..7 locked) + DESIGN (DDD-1..16, ADR-063, ADR-064, UC-1..UC-8). **DEVOPS: not run (⊘)** — the feature adds no infrastructure; default environment = the existing CI acceptance stage.
- **Policy**: `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` present; 3 rows appended/extended (infer verb, `ContributionLinkPort`, `FakeGithub` `/contributors`). `[port-mode] inherit` (`tests/common/state_delta.rs`).

The `.rs` files are the scenario SSOT; this is the map.

## 1. Inputs read

| Input | Status |
|---|---|
| `feature-delta.md` (DISCUSS + DESIGN) | ✓ |
| `slices/slice-01..05` | ✓ |
| `design/wave-decisions.md`, `design/upstream-changes.md` (UC-1..UC-8) | ✓ |
| ADR-063, ADR-064 | ✓ |
| `docs/product/journeys/infer-contributor-philosophy.yaml` (failure_modes) | ✓ |
| `docs/product/kpi-contracts.yaml` | ✓ (KPI-CPI-* not yet registered — DEVOPS not run; scenarios tag `@kpi-cpi-N` directly) |
| `discuss/`, `devops/` wave-decisions files | ⊘ (DISCUSS lives inline in `feature-delta.md`; no DEVOPS wave) |
| House style: `serverless-philosophy-federation/distill/*` | ✓ |

## 2. Wave-decision reconciliation

**Reconciliation passed — 0 contradictions.** DESIGN changed three DISCUSS assumptions, all
explicitly recorded as *Changed Assumptions* / UC clarifications (not silent overrides):
read port `StoragePort` not `StoreReadPort` (internal, no observable change); request count
"exactly 1" → "1, and 0 at `--contributors 0`" (UC-1, a refinement); too-large 403 / empty
204 = notice + exit 0 (UC-2, a case DISCUSS did not cover). All eight UCs are **encoded as
scenarios** and flagged for PO ratification (`acceptance-review.md`).

## 3. Test files

| File (`tests/acceptance/`) | Slice / story | Scenarios | Active | Ignored (DELIVER) | Ignored (live) |
|---|---|---|---|---|---|
| `infer_people_sign.rs` | 03 / US-CPI-003 + **WS** | 10 | **1 (WS)** | 8 | 1 |
| `contributor_links.rs` | 01 / US-CPI-001 | 15 | 0 | 14 | 1 |
| `infer_people.rs` | 02 / US-CPI-002 | 16 | 0 | 15 | 1 |
| `infer_people_evidence_grows.rs` | 04 / US-CPI-004 | 11 | 0 | 10 | 1 |
| `scrape_person.rs` | 05 / US-CPI-005 | 9 | 0 | 8 | 1 |
| **Total** | | **61** | **1** | **55** | **5** |

Shared step vocabulary: `tests/acceptance/support/people.rs` (registered as `support::people`).
Test-double extension: `crates/test-support/src/fake_github.rs` — `FakeContributor`,
`FakeContributorsPosture`, `with_contributors`, `with_contributors_posture`, route
`/repos/{o}/{r}/contributors` (default 200 `[]`), 3 new unit tests.

**Mix**: happy 10 · error 13 · edge 20 · boundary 5 · guardrail 8 · live production-data 5.
**Error + edge + boundary = 38/61 = 62%** (strict error-only 21%); ≥ 40% target met.

Live production-data scenarios (slice briefs require real repos) are `#[ignore = "live-github: …"]`
AND gated on `OPENLORE_LIVE_GITHUB=1` (they early-return otherwise), so `--ignored` DELIVER runs
never touch api.github.com by accident.

## 4. Scenario inventory → AC traceability

### US-CPI-001 — `contributor_links.rs` (slice-01)

| ID | Test fn | Tags | AC / decision |
|---|---|---|---|
| CL-1 | `scraping_a_repo_records_its_top_thirty_human_contributors_and_names_the_bots` | @happy @kpi-cpi-5 | AC1 top-N, AC2 bots named, AC7 zero claims |
| CL-2 | `people_already_linked_to_other_scraped_repos_are_surfaced` | @happy | AC4 overlap |
| CL-3 | `the_contributor_count_can_be_overridden_and_counts_only_humans` | @boundary @od-cpi-7 | AC3 override; DDD-2 N counts humans |
| CL-4 | `zero_contributors_records_none_and_asks_github_nothing_extra` | @boundary @uc-1 @kpi-cpi-5 | AC3 N=0; **UC-1** 0 requests |
| CL-5 | `re_scraping_never_loses_previously_recorded_people` | @boundary @d-4 | AC5 refresh + never delete; first_observed invariant |
| CL-6 | `a_rate_limited_contributor_harvest_records_nothing_and_says_why` | @error | AC6 rate limit + GITHUB_TOKEN, no partial links |
| CL-7 | `a_rejected_token_on_the_contributor_harvest_records_nothing` | @error | AC6 auth; no-token-leak |
| CL-8 | `an_unexpected_contributor_list_records_nothing` | @error @ddd-15 | DDD-14 shape failure, no partial write |
| CL-9 | `a_repo_too_large_to_list_contributors_is_a_notice_not_a_failure` | @edge @uc-2 | **UC-2** 403 too-large, exit 0 |
| CL-10 | `an_empty_repository_is_a_notice_not_a_failure` | @edge @uc-2 | **UC-2** 204 empty, exit 0 |
| CL-11 | `more_than_one_hundred_contributors_is_rejected_before_asking_github` | @error @od-cpi-7 | OD-CPI-7 0..=100; journey failure_mode |
| CL-12 | `asking_for_contributors_of_a_person_is_rejected_before_asking_github` | @error @uc-3 | **UC-3** |
| CL-13 | `the_recorded_people_are_humans_ranked_by_commits_whatever_order_github_uses` | @edge @ddd-3 @ddd-15 | bot rule (Bot type, `[bot]` as User, case-insensitive), re-rank, de-dup |
| CL-14 | `recording_contributors_costs_exactly_one_extra_public_request` | @guardrail @kpi-cpi-5 @d-6 | KPI-CPI-5 ≤1 extra request; public allowlist |
| CL-15 | `live_real_repos_share_burntsushi_and_anyhow_core_is_dtolnay_first` | @live-github | slice-01 production-data ACs; R-1 |

### US-CPI-002 — `infer_people.rs` (slice-02)

| ID | Test fn | Tags | AC / decision |
|---|---|---|---|
| IP-1 | `a_person_is_proposed_for_the_philosophies_of_signed_repos_they_build` | @happy @kpi-cpi-2 @d-7 | AC1, AC2 provenance (CID + own author + rank), AC4 speculative, AC7 writes nothing |
| IP-2 | `unsigned_scraper_candidates_never_feed_inference` | @guardrail @kpi-cpi-3 @d-2 | AC3 unsigned never contribute + footer |
| IP-3 | `a_soft_retracted_repo_claim_stops_supporting_inferences` | @edge | AC1 non-retracted; confidence 0.15 |
| IP-4 | `no_candidates_is_a_normal_outcome` | @edge | AC6 empty → exit 0 |
| IP-5 | `an_unsubscribed_peers_claims_no_longer_support_inferences` | @edge @d-2 | active peers only |
| IP-6 | `a_counter_claim_is_never_counted_as_support` | @edge @ddd-7 | markers never support; 3rd-party counter doesn't hide |
| IP-7 | `a_superseded_repo_claim_is_replaced_by_its_successor_as_support` | @edge @ddd-7 | superseded never supports |
| IP-8 | `an_already_signed_person_philosophy_is_not_proposed_again` | @edge | AC5 |
| IP-9 | `repos_differing_only_in_letter_case_are_the_same_repo` | @edge @uc-6 | **UC-6** |
| IP-10 | `every_inferred_candidate_carries_complete_provenance` | @property @guardrail @kpi-cpi-2 | KPI-CPI-2 100% |
| IP-11 | `proposed_confidence_is_capped_and_never_exceeds_the_strongest_support` | @property @boundary @ddd-9 | `min(29, 15+5(k−1), floor(100·max))`: cap 0.29, floor 0.17 |
| IP-12 | `inference_runs_offline_and_writes_nothing` | @guardrail @kpi-5 | AC7 offline + writes nothing |
| IP-13 | `a_minimum_number_of_supporting_repos_can_be_required` | @boundary @ddd-13 | `--min-repos` |
| IP-14 | `a_person_named_without_the_github_prefix_is_refused_with_the_expected_form` | @error @d-8 | `--person github:<login>` (DISTILL addition — PO to ratify) |
| IP-15 | `the_new_surface_says_person_and_contributor_still_means_claim_author` | @guardrail @d-8 | vocabulary; DoD #6 |
| IP-16 | `live_burntsushi_memory_safety_is_inferred_from_real_repos` | @live-github | slice-02 production-data AC |

### US-CPI-003 — `infer_people_sign.rs` (slice-03)

| ID | Test fn | Tags | AC / decision |
|---|---|---|---|
| **WS-CPI-1** | `maria_signs_an_evidence_backed_adherence_for_the_person_who_builds_her_signed_repos` | @walking_skeleton @driving_port @driving_adapter @real-io | AC1 same sign path, AC2 provenance in payload (exact ADR-064 evidence list), AC3 confidence edited |
| IS-2 | `signing_an_inference_keeps_each_supporting_claims_own_author_and_re_verifies` | @happy @kpi-cpi-2 @uc-7 @q-cpi-d3 | AC2 CID stable on re-verify; D-7; **UC-7**; published record carries evidence; **Q-CPI-D3** bare DID pinned |
| IS-3 | `listing_inferred_candidates_without_sign_writes_nothing` | @guardrail @kpi-cpi-3 | AC4 without --sign nothing written |
| IS-4 | `selecting_a_candidate_that_does_not_exist_is_rejected_before_composing` | @error | AC4 out-of-range; journey failure_mode |
| IS-5 | `the_candidate_signed_is_the_one_listed_under_that_number` | @edge | AC5 deterministic numbering |
| IS-6 | `sign_numbers_candidates_within_the_same_person_filter_as_the_list` | @edge @uc-8 | **UC-8** |
| IS-7 | `an_out_of_range_confidence_is_refused_and_re_asked_before_signing` | @error | AC3 [0.0, 1.0] |
| IS-8 | `accepting_the_proposed_confidence_signs_the_speculative_value_unchanged` | @edge | AC3 never auto-raised |
| IS-9 | `signing_two_candidates_in_one_pass_produces_one_claim_each` | @happy | AC1 `N[,N…]` |
| IS-10 | `live_signing_burntsushi_memory_safety_carries_both_real_supporting_claims` | @live-github | slice-03 production-data AC |

### US-CPI-004 — `infer_people_evidence_grows.rs` (slice-04)

| ID | Test fn | Tags | AC / decision |
|---|---|---|---|
| EG-1 | `a_newly_signed_repo_philosophy_proposes_a_new_person_inference` | @happy | AC1 hint, AC2 NEW |
| EG-2 | `stronger_evidence_is_offered_as_a_superseding_candidate` | @happy @ddd-8 | AC2 STRONGER naming earlier CID |
| EG-3 | `signing_stronger_evidence_supersedes_and_never_edits_the_earlier_claim` | @happy @kpi-cpi-4 @d-5 | AC2 `supersedes`; AC4 byte-identical |
| EG-4 | `retracted_support_is_flagged_not_acted_on` | @error @kpi-cpi-4 | AC3 SUPPORT WEAKENED (retracted); AC4 |
| EG-5 | `support_from_an_unsubscribed_peer_is_flagged_with_its_reason` | @error @uc-5 | **UC-5** peer no longer subscribed |
| EG-6 | `support_missing_from_the_local_store_is_flagged_with_its_reason` | @error @uc-5 | **UC-5** not in local store |
| EG-7 | `a_scrape_that_changes_nothing_prints_no_hint` | @edge | AC1 silent when unchanged |
| EG-8 | `newly_recorded_people_alone_can_create_new_inferences` | @edge @ddd-14 | AC1 hint counts link-driven change |
| EG-9 | `a_hand_authored_adherence_is_already_signed_and_never_superseded` | @edge @uc-4 | **UC-4** |
| EG-10 | `with_two_current_inferred_claims_the_latest_is_the_one_superseded` | @edge @q-cpi-d4 | **Q-CPI-D4** |
| EG-11 | `live_scraping_regex_offers_burntsushi_memory_safety_as_stronger` | @live-github | slice-04 production-data AC |

### US-CPI-005 — `scrape_person.rs` (slice-05)

| ID | Test fn | Tags | AC / decision |
|---|---|---|---|
| SP-1 | `a_user_scrape_shows_the_persons_accumulated_picture` | @happy | AC1 links + rank, signed, candidates |
| SP-2 | `signing_from_the_person_view_is_the_same_as_signing_from_infer_people` | @guardrail @ddd-13 | AC2 identical CID from both surfaces |
| SP-3 | `a_person_with_no_links_gets_guidance_not_an_error` | @edge | AC4 guidance, exit 0 |
| SP-4 | `a_non_existent_user_still_fails_clearly` | @error | AC4 not-found (GREEN-today regression guard) |
| SP-5 | `reading_a_person_asks_github_only_for_their_profile_once` | @guardrail @d-4 | AC3 exactly one `/users/{user}` request, no crawl (DELIVER collapses the shipped resolve+harvest double-fetch) |
| SP-6 | `a_renamed_login_is_flagged_as_a_possible_rename_never_merged` | @edge @q-cpi-d7 | **Q-CPI-D7** |
| SP-7 | `a_linked_person_without_signed_repo_claims_shows_links_and_how_to_enable_inference` | @edge | AC1 empty candidates |
| SP-8 | `selecting_a_missing_candidate_from_the_person_view_is_rejected` | @error | AC2 same selection rules |
| SP-9 | `live_reading_burntsushi_lists_his_real_scraped_repos` | @live-github | slice-05 production-data AC |

### KPI guardrails → observable assertions

| KPI | Asserted by |
|---|---|
| KPI-CPI-2 100% complete provenance | WS (exact evidence list), IS-2 (own-author at-uris, re-verify, federated record), IP-1, IP-10 |
| KPI-CPI-3 signed-only / non-retracted / nothing without `--sign` | IP-2, IP-3, IP-5, IP-6, IP-7, IP-12, IS-3, IS-4 |
| KPI-CPI-4 no signed claim mutated; growth surfaced | EG-3 (byte-compare), EG-4/5/6 (files identical, no marker authored), EG-1/2 |
| KPI-CPI-5 ≤1 extra request (0 at N=0); bots excluded | CL-14, CL-4, CL-1, CL-3, CL-13 |
| KPI-5 local-first | IP-12 |
| KPI-CPI-1 (North Star) | not automatable (dogfood log); WS demonstrates the path |

## 5. Q-CPI-D1 — output wording pinned (load-bearing substrings)

| Surface | Pinned substring(s) |
|---|---|
| contributors block | `Contributors recorded: <n>`, `bots excluded`, each bot login |
| overlap | `Also contributes to repos you scraped`, `<login> → <owner/repo>` (ASCII `->` also accepted) |
| no links recorded (UC-2) | `Contributors not recorded` + `too large` / `empty` |
| candidate | `[n] ` headline containing `github:<login>` and the philosophy short name; provenance lines INDENTED under it, one per supporting claim: repo subject + `#<rank>` + claim CID + bare author DID |
| confidence | `<0.xx> (speculative)` + arithmetic containing `min(` |
| empty | `No inferred candidates`; footer `no signed philosophy claims` naming the repo |
| already signed | `already signed` + the CID |
| labels | `NEW`, `STRONGER` (+ earlier CID, `2 repos`, `unchanged`), `support weakened` + `1 of 2 supporting claims retracted` / `no longer subscribed` / `not in local store` |
| sign preview | `derived-from: <k> signed repo claims` + each AT-URI |
| out of range | `candidate <n> does not exist; valid range 1..<m>` |
| hint | `new inferred candidate` + `openlore infer people`; silent when unchanged |
| person view | `linked to <n> scraped repos`, repo + `#<rank>`, `github:<login> is not linked to any repo you've scraped`, `possible rename` |
| `--person` form | `github:<login>` in the refusal |
| help | `infer people --help` has `--person`, never "contributor"; `graph query --help` says `claim author` |

Non-pins (layout DELIVER owns): ordering of blocks, colours, column widths. Constraint: the
contributors/overlap block and the hint MUST NOT use `[n] ` numbering (shipped helpers count
`[n]` markers as candidates).

## 6. Domain-language step vocabulary (soft gate — `support/people.rs`)

| Fact / observation | Step |
|---|---|
| a repo was scraped and its people recorded | `given_repo_scraped` |
| Maria signed a repo philosophy claim | `given_i_signed_repo_claim` / `given_i_signed` |
| Maria retracted her claim | `given_i_retracted` |
| a subscribed peer published claims / markers | `SubscribedPeer::subscribe_and_pull` / `publish_and_pull` / `unsubscribe` / `unsubscribe_and_purge` |
| the canonical BurntSushi store | `given_burntsushi_builds_two_signed_memory_safe_repos` |
| Maria scrapes / infers / signs | `scrape_github`, `infer_people`, `infer_people_with`, `infer_people_offline`, `sign_stdin` |
| candidate n proposes person→philosophy | `assert_candidate` |
| provenance names claim + author + rank | `assert_provenance` / `assert_provenance_omits` |
| confidence speculative with arithmetic | `assert_speculative_confidence` |
| nothing was written | `assert_store_unchanged` / `assert_store_delta` (Mandate 8 universe) |

## 7. Recommended unit-level properties (DELIVER — layer 1, proptest, pure `scraper-domain::people`)

1. Selection: for any raw rows and N∈0..=100 → exactly `min(N, #distinct humans)` people, no login matching the bot rule, ranks `1..=k` contiguous, order = contributions desc then login asc, independent of input order (permutation invariance), de-dup by id.
2. Confidence: ∀ k≥1, max∈[0,1] → value ≤ 29, ≤ ⌊100·max⌋, = `min(29, 15+5(k−1), ⌊100·max⌋)`; monotone non-decreasing in k.
3. Provenance codec: encode → parse round-trips exactly the supporting (author, CID) set for arbitrary DIDs/CIDs/logins; no entry contains `,`; `#fragment` stripped.
4. Eligibility: no candidate ever cites an unsigned, marker (`retracts`/`counters`), self-retracted, same-author-superseded, or non-{You,SubscribedPeer} claim.
5. Candidate smart constructor: non-empty supporting repos, each non-empty claims (KPI-CPI-2 by construction).
6. Numbering: deterministic under input permutation; filters applied before numbering (UC-8).
7. Classification: a claim citing no at-uri is never STRONGER (UC-4); STRONGER supersedes the latest `composed_at` (Q-CPI-D4).
8. Change summary: identical before/after reports → 0 (hint silent).
9. `github:` subject key: case-folding is idempotent and equal for case variants (UC-6).

Adapter-level (DELIVER, layer 3): `DuckDbContributionLinkAdapter` upsert-twice-in-tx probe; `record_snapshot` never lowers the row set nor touches `first_observed_at`.
