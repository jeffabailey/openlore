# RED classification (pre-DELIVER fail-for-the-right-reason gate): slice-14

> Acceptance Designer: Quinn · 2026-06-08 · Per `nw-distill` Pre-DELIVER gate + ADR-025.
> DELIVER reads this at PREPARE/RED to confirm RED is genuine before unskipping.

## Build

`cargo build -p cli --test viewer_counter_flags_score_surface --test
viewer_counter_flags_score_surface_invariants` → **compiles, zero errors** (only
pre-existing `support/mod.rs` warnings — unused-var / unreachable-pattern on inherited
seams). All slice-14 seed/assert signatures resolve, so the AT files build.

## Per-scenario classification (all MISSING_FUNCTIONALITY — genuine RED)

| Scenario | Panics at | Class |
|---|---|---|
| SF-1 `open_the_score_breakdown_with_htmx_flags_only_the_countered_contribution` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-2 `a_contribution_with_two_counters_shows_one_neutral_presence_marker_on_the_score` | `seed_score_breakdown_target_two_counters_distinct_authors` `todo!()` | MISSING_FUNCTIONALITY |
| SF-3 `the_per_contribution_subtotals_still_sum_to_the_pairing_weight_with_the_flag` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-4 `adding_the_score_flag_changes_no_weight_ranking_or_row_order_versus_slice09` | `seed_score_breakdown_many_pairings_known_countered_subset` `todo!()` | MISSING_FUNCTIONALITY |
| SF-5 `two_identical_subtotal_contributions_render_identically_only_one_flagged` | `seed_score_breakdown_identical_subtotals_one_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-6 `a_contributor_with_no_countered_contributions_renders_score_with_no_markers` | `seed_score_breakdown_none_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-7 `the_score_flag_renders_identically_under_htmx_and_no_js` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-N1 `a_large_multi_pairing_breakdown_flags_every_countered_contribution_in_one_request` | `seed_score_breakdown_many_pairings_known_countered_subset` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-ReadOnly `every_flagged_score_render_leaves_the_store_read_only` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-NoWrite `no_flagged_score_render_adds_a_write_or_sign_control` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-OfflineChrome `the_flagged_score_chrome_stays_offline_no_cdn` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-Offline `the_flagged_score_surface_renders_fully_offline` | `seed_score_breakdown_one_contribution_countered` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-ByteId (CARDINAL) `the_score_render_is_byte_identical_with_and_without_the_flag` | `seed_score_breakdown_many_pairings_known_countered_subset` `todo!()` | MISSING_FUNCTIONALITY |
| SF-INV-N1 `a_large_multi_pairing_breakdown_resolves_presence_in_one_request` | `seed_score_breakdown_many_pairings_known_countered_subset` `todo!()` | MISSING_FUNCTIONALITY |

## Verdict

**14/14 RED for the right reason.** Every scenario reaches its slice-14 `todo!()`-stubbed
seam and panics with `not yet implemented: slice-14 RED scaffold: …` — implementation is
missing, the test is correct. Zero IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE; zero
WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT (every assertion scans the port-exposed rendered
HTML or the universe-bound store-row-count delta, never an internal struct field). No scenario
is BROKEN. Handoff to DELIVER is unblocked.

(The 2 "passed" tests in each binary are the bundled `support::state_delta::tests` unit
tests, not slice-14 acceptance scenarios.)
