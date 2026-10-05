# Test scenarios — indexer-per-did-pds-fetch (DISTILL)

There are 55 executable scenarios, all `#[ignore]`d at hand-off:

- 37 subprocess scenarios at layer 3.
- 14 pure-core contracts at layer 2 (proptest + exhaustive tables).
- 4 structural xtask checks.

Gherkin lives in each test's doc comment. Tags: `@US-*`, `@AC-*`, `@walking_skeleton`,
`@driving_port`, `@real-io`, `@error`, `@adversarial`, `@boundary`, `@regression`,
`@property`, `@contract-shape:*`.

## Suites

| Target | File | Layer | Scenarios |
|---|---|---|---|
| `indexer_per_did_fetch` | `tests/acceptance/indexer_per_did_fetch.rs` | 3 | WS-1, WS-2, IPF-01..09 (11) |
| `indexer_per_did_resilience` | `tests/acceptance/indexer_per_did_resilience.rs` | 3 | IPF-10..20 (11) |
| `indexer_per_did_config` | `tests/acceptance/indexer_per_did_config.rs` | 3 | IPF-21..30 (10) |
| `search_self_attested_label` | `tests/acceptance/search_self_attested_label.rs` | 3 | IPF-31..35 (5) |
| `indexer_pass_core` | `tests/acceptance/indexer_pass_core.rs` | 2 | CORE-1..14 (11 properties + 3 tables) |
| `indexer_per_did_architecture` (xtask) | `xtask/tests/indexer_per_did_architecture.rs` | static | XA-1..4 |

Shared harness: `tests/acceptance/support/indexer_network.rs`. Multi-PDS fake:
`crates/test-support/src/fake_atproto.rs` (indexer postures).

## Scenario list

| ID | Scenario (test fn) | Story | Kind | Tags (abridged) |
|---|---|---|---|---|
| WS-1 | `maria_finds_priyas_self_attested_claim_published_on_her_own_pds` | 001 | WS happy | `@walking_skeleton @driving_port @driving_adapter @real-io` |
| WS-2 | `authors_on_different_pdses_are_all_found_in_one_pass` | 001 | WS happy | `@walking_skeleton @driving_port @real-io @adapter-integration` |
| IPF-01 | `an_author_who_moved_pds_is_followed_to_the_new_one` | 001 | edge | chained on WS-2 |
| IPF-02 | `records_of_another_repo_are_never_attributed_to_the_requested_author` | 001 | error/adversarial | |
| IPF-03 | `a_tampered_self_attested_record_is_still_refused` | 001 | error/adversarial | |
| IPF-04 | `an_unresolvable_authors_app_signed_claims_come_through_the_fallback` | 003 | happy (alt) | |
| IPF-05 | `a_resolvable_author_is_never_read_from_the_fallback` | 003 | invariant | chained on IPF-04 Given |
| IPF-06 | `a_fallback_that_is_somebodys_own_pds_never_vouches_for_self_attested_claims` | 003 | error/adversarial | |
| IPF-07 | `a_failing_fallback_is_isolated_like_any_other_source` | 003 | error | chained on IPF-04 Given |
| IPF-08 | `a_fallback_whose_name_leads_only_to_a_private_address_is_never_contacted` | 003 | error/adversarial | Tripwire, no seam |
| IPF-09 | `a_did_web_author_whose_web_host_is_down_is_read_through_the_fallback` | 003 | error | did:web |
| IPF-10 | `one_pds_being_down_does_not_hide_the_others` | 002 | error | `@driving_port` |
| IPF-11 | `every_way_an_authors_pds_can_fail_skips_only_that_author_with_its_reason` | 002 | error table (6) | down/503/429/404/HTML/redirect |
| IPF-12 | `an_unresolvable_author_is_skipped_with_a_reason` | 002 | error | |
| IPF-13 | `every_way_a_did_document_can_fail_makes_that_author_unresolvable` | 002 | error table (4) | 404/500/id-mismatch/no PDS |
| IPF-14 | `a_skipped_authors_earlier_claims_stay_searchable` | 002 | error | chained, search |
| IPF-15 | `a_hanging_pds_cannot_stall_the_pass` | 002 | error | budget 2 s |
| IPF-16 | `a_did_document_that_never_arrives_counts_against_the_same_budget` | 002 | error | budget 2 s |
| IPF-17 | `skipped_authors_are_retried_on_the_next_pass` | 002 | recovery | chained |
| IPF-18 | `many_authors_never_cause_unbounded_concurrent_requests` | 002 | boundary | 40 authors / 12 hosts / cap 3 |
| IPF-19 | `when_every_authors_source_fails_the_pass_reports_a_total_outage` | 002 | error | exit 3 |
| IPF-20 | `a_did_document_pointing_at_a_private_or_insecure_address_is_never_followed` | 002 | adversarial table (6) | SSRF |
| IPF-21 | `a_malformed_repo_did_is_explained_at_startup` | 004 | error table (4) | |
| IPF-22 | `a_malformed_or_unsafe_fallback_source_is_explained_at_startup` | 004 | error table (7) | |
| IPF-23 | `fan_out_bounds_outside_their_range_are_explained_at_startup` | 004 | boundary table (6) | |
| IPF-24 | `fan_out_bounds_at_the_edge_of_their_range_are_accepted_and_reported` | 004 | boundary table (3) | |
| IPF-25 | `an_empty_did_list_and_no_fallback_are_reported_not_refused` | 004 | boundary | zero DIDs → exit 0 |
| IPF-26 | `a_repo_did_listed_twice_is_read_once_and_counted_once` | 004 | boundary | |
| IPF-27 | `a_single_source_deployment_indexes_exactly_what_it_did_before` | 004 | regression | NFR-3 |
| IPF-28 | `with_the_directory_down_a_single_source_deployment_keeps_its_old_behaviour` | 004 | regression/error | NFR-3 |
| IPF-29 | `without_the_test_seam_the_production_policy_refuses_a_loopback_fallback` | 004 | error | |
| IPF-30 | `a_release_build_refuses_the_loopback_test_seam` | 004 | release gate | CI-only (DWD-8) |
| IPF-31 | `maria_sees_priyas_claim_marked_self_attested_next_to_verified` | 005 | happy | `@driving_port` |
| IPF-32 | `the_label_is_the_same_whichever_way_maria_searches` | 005 | happy table (3) | see upstream issue 1 |
| IPF-33 | `inspecting_a_self_attested_claim_never_claims_a_signature_was_checked` | 005 | happy | `--show` |
| IPF-34 | `results_from_an_indexer_that_does_not_report_provenance_read_exactly_as_before` | 005 | regression | old server |
| IPF-35 | `a_result_with_an_unknown_provenance_is_withheld_not_misstated` | 005 | error/adversarial | |
| CORE-1..14 | pure contracts: origin rule, plan_listing, summary + exit code, address guard, endpoint pre-check, config totality, release refusal, DID dedup, repo binding, provenance decode, failure-class table, range edges, DTO round trip | 001–005 | `@property` / tables | layer 2 |
| XA-1..4 | origin only via ListingSource; guarded clients only; both rules in check-arch; scanner non-vacuity | 002/003 | static | |

**Error/edge share** (subprocess scenarios): 28 of 37 are error, adversarial, boundary or
regression scenarios, which is **76%** (target ≥ 40%).

## AC traceability (27 ACs, all covered)

| AC | Scenarios |
|---|---|
| AC-001.1 | WS-1, WS-2 |
| AC-001.2 | WS-1, WS-2, CORE-2 |
| AC-001.3 | WS-1 |
| AC-001.4 | IPF-01 |
| AC-001.5 | IPF-02, CORE-10 |
| AC-001.6 | IPF-03 |
| AC-002.1 | IPF-10, IPF-11, CORE-12 |
| AC-002.2 | IPF-12, IPF-13, IPF-16, CORE-3 |
| AC-002.3 | IPF-10, IPF-11, IPF-12 (and every `assert_skipped`: closed field set, detail ≤ 200 chars, no claim content) |
| AC-002.4 | IPF-14, IPF-19 |
| AC-002.5 | IPF-15, IPF-16, CORE-12 |
| AC-002.6 | IPF-17 |
| AC-002.7 | IPF-18, IPF-26, CORE-4 (and every `assert_pass_completed`: own + fallback + skipped == configured) |
| AC-002.8 *(design)* | IPF-19 (exit 3, summary last), IPF-10 (partial → 0), IPF-25 (none configured → 0), CORE-4 |
| AC-002.9 *(design)* | IPF-20, CORE-3, CORE-5, CORE-6, CORE-13, XA-2 |
| AC-003.1 | IPF-04, IPF-06, IPF-09, CORE-1 |
| AC-003.2 | IPF-05, IPF-20, CORE-3 |
| AC-003.3 | IPF-07, CORE-12 |
| AC-003.4 *(design)* | IPF-08 (runtime, after DNS), IPF-22 (startup: non-https / refused literal) |
| AC-004.1 | IPF-21, CORE-7, CORE-9 |
| AC-004.2 | IPF-22, IPF-23, CORE-7 |
| AC-004.3 | IPF-24, IPF-25 |
| AC-004.4 | IPF-27, IPF-28 (and the existing indexer/search suites after harness migration, upstream issue 2) |
| AC-004.5 *(design)* | IPF-29, IPF-30, CORE-8, IPF-20 row `http://10.0.0.1` (debug + seam still refuses a private address) |
| AC-005.1 | IPF-31, IPF-33 |
| AC-005.2 | IPF-34, IPF-35, CORE-11, CORE-14 |
| AC-005.3 | IPF-32 |

Invariants: I-IPF-1 (WS-2, IPF-03), I-IPF-2 (IPF-04/05/06, CORE-1, XA-1), I-IPF-3 (IPF-07/10/14),
I-IPF-4 (IPF-15/18), I-IPF-5 (WS-2, IPF-02), I-IPF-6 (assert_skipped content rule; no new
capability). Lies catalogue (architecture §9): each row maps to IPF-02/11/13/15/19/20/06, XA-1/2
and CORE-1. The cursor-loop row is the existing `take_page` bound and is not re-tested.

## DELIVER enablement order (one ignore at a time)

The ignore reason names the step. The order follows the thin slice first:

1. **00 (no AT)**: harness migration of the existing suites (wave-decisions upstream issue 2).
2. **01-01** WS-1 → **01-02** WS-2, CORE-2 → **01-03** IPF-01 → **01-04** IPF-02, CORE-10 → **01-05** IPF-03.
3. **02-01** IPF-10 → **02-02** IPF-11, CORE-12 → **02-03** IPF-12 → **02-04** IPF-13 → **02-05** IPF-14 →
   **02-06** IPF-15 → **02-07** IPF-16 → **02-08** IPF-17 → **02-09** IPF-18 → **02-10** IPF-19, CORE-4 →
   **02-11** IPF-20, CORE-5/6/13, XA-2.
4. **03-01** IPF-04, CORE-1, XA-1, XA-4 → **03-02** IPF-05, CORE-3 → **03-03** IPF-06 → **03-04** IPF-07 →
   **03-05** IPF-08 → **03-06** IPF-09. XA-3 goes with the later of 02-11 and 03-01.
5. **04-01** IPF-21, CORE-7 → **04-02** IPF-22 → **04-03** IPF-23 → **04-04** IPF-24 → **04-05** IPF-25 →
   **04-06** IPF-26, CORE-9 → **04-07** IPF-27 → **04-08** IPF-28 → **04-09** IPF-29 → **04-10** CORE-8
   (+ IPF-30 in the CI release-guard job).
6. **05-01** IPF-31 → **05-02** IPF-32 → **05-03** IPF-33 → **05-04** IPF-34, CORE-11, CORE-14 → **05-05** IPF-35.

US-IPF-005 (05-xx) can ship as its own milestone. WS-1 does not depend on it: it asserts
attribution, not the label.

## Self-completeness audit (nw-at-completeness-check)

| Item | Verdict | Evidence |
|---|---|---|
| C1a empty/min | ✓ | IPF-25 (0 DIDs), CORE-4 (0 outcomes), CORE-10 (0 records) |
| C1b boundaries | ✓ | IPF-23/24 (0/1/16/17, 0/1/600/601), CORE-13 (range edges) |
| C2a state machine documented | ✓ | per-DID unit diagram in the `indexer_per_did_resilience.rs` module doc |
| C2b illegal event per state | ✓ | resolved → fallback (IPF-05, IPF-20, CORE-3); skipped → delete (IPF-14, IPF-19); refused → fallback (IPF-20) |
| C3 0/1/N | ✓ | DIDs 0 / 1 / 40 (IPF-25, IPF-03, IPF-18); records via CORE-10 |
| C4a apply twice | ✓ | second passes in IPF-01/14/17/19 (no duplicate rows) |
| C4b inverse without prerequisite | ✓ (N/A) | the indexer has no inverse operation (no delete, ADR-024). A skip of a never-indexed DID removes nothing (IPF-12) |
| C5a mode-flag combinations | ✓ | resolution × fallback × policy (CORE-3); seam × build profile (CORE-8, IPF-29/30); fallback present/absent (IPF-04/12) |
| C5b flag orthogonality | ✓ | the bounds change the shape, not the result (IPF-24; IPF-18 summary) |
| C6a malformed input per parameter | ✓ | IPF-21/22/23, CORE-7 |
| C6b each declared error | ✓ | all 5 skip reasons, fallback_failure, config refusal, exit 3, foreign_repo, cid_mismatch |
| C6c closed error set | ✓ | CORE-12 (exhaustive), skip-event field whitelist, CORE-7 (refusal names a known variable) |
| C7a degraded resource | ✓ | directory down (IPF-28), host down (IPF-10/19) |
| C7b interruption/timeout | ✓ | IPF-15/16 (hang → deadline). The upsert-failure partial commit is a documented gap (unit level) |
| C7c concurrent actors | ✓ (N/A) | not claimed concurrent-safe across processes. In-pass concurrency is bounded by IPF-18 |

**Verdict: 15/15, COMPLETE.** Gaps are classified as follows. The upsert-failure,
`did:web`-over-TLS and DNS-rebinding gaps are AT_GAP_IN_DELIVERY_SCOPE and go to adapter or unit
tests in DELIVER. The contributor-lift issue is a SPECIFICATION_AMBIGUITY routed to DESIGN
(wave-decisions, upstream issue 1). It is non-blocking because the user-visible AC is asserted.
