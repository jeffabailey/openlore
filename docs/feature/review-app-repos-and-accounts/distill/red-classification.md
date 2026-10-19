# RED classification — review-app-repos-and-accounts (pre-DELIVER gate)

Refreshed **2026-10-19** after review cycle 2 (wording fixes and the readiness re-check in the harness; every first failure line unchanged from cycle 1), against `main` (feature unimplemented; store v1).
Every scenario is `#[ignore = "…"]`; the default run is green.

## Commands and results

| Command | Result |
|---|---|
| `rustfmt --edition 2021 --check` on this feature's files (not `cargo fmt --all`: another crafter is editing other crates) | clean |
| `cargo clippy -p openlore-review-app -p openlore-test-support --tests -- -D warnings` | 0 warnings |
| `cargo nextest run -p openlore-review-app` (default) | 186 passed — every shipped review-app test, with the extended GitHub double and the port-race retry in the harness; the new scenarios are skipped |
| `cargo nextest run -p openlore-review-app --test review_app_repos_and_accounts --test review_app_repos_and_accounts_several --test review_app_repos_and_accounts_imports --test review_app_repos_and_accounts_core --run-ignored only --no-fail-fast` | **70 failed, 0 passed** |
| fixture regenerator | its own target `review_app_repos_and_accounts_fixture` (never in a scenario run); `duckdb` CLI confirms the committed store is schema v1 |

Every GIVEN built only from shipped behaviour succeeded (sign-in, verify one account, scan the
default top 5, approve, publish, decline, start on an existing store). No scenario fails to
compile or in fixture setup. R2/R3 scenarios that first Save a choice fail at the R1 page: the
release order (R1 → R2 → R3) DELIVER follows; each is RED for the missing feature. MG-1's first
check (pending queue equals the golden list) passes on the v1 app, so the fixture is a faithful
v1 store.

## First failure line per scenario (59)

| Scenario (test fn) | Ignore id | First failure | Class |
|---|---|---|---|
| WS-RRA-1 `priya_publishes_a_claim_about_a_repo_her_old_top_5_never_read` | DELIVER 01-01 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RC-1 `first_visit_shows_todays_top_5_already_ticked` | DELIVER 01-02 | assertion left == right failed | MISSING_FUNCTIONALITY ✅ |
| RC-2 `priya_saves_her_own_choice` | DELIVER 01-03 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RC-3 `choosing_more_than_the_limit_is_refused_with_the_fix` | DELIVER 01-04 | no "Save choice" list for github.com/dvolkov on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RC-4 `an_owner_with_only_forks_learns_why_nothing_is_listed` | DELIVER 01-05 | not found | MISSING_FUNCTIONALITY ✅ |
| RC-5 `a_failed_listing_leaves_the_saved_choice_alone` | DELIVER 01-06 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RC-6 `priya_chooses_and_saves_with_the_keyboard_only` | DELIVER 01-07 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RC-7 `an_account_that_needs_re_verifying_shows_no_repo_list` | DELIVER 01-08 | not found | MISSING_FUNCTIONALITY ✅ |
| RS-1 `the_scan_reads_only_the_repos_priya_chose_or_todays_top_5` | DELIVER 01-09 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-2a `a_paused_scan_finishes_the_choice_it_started_with` | DELIVER 01-10 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-3 `a_repo_created_after_the_choice_is_not_read_until_ticked` | DELIVER 01-11 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-4 `a_chosen_repo_that_can_no_longer_be_read_is_skipped_and_named` | DELIVER 01-12 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-5 `a_scan_with_nothing_to_read_does_not_start` | DELIVER 01-13 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-6 `unticking_a_repo_keeps_its_pending_suggestions` | DELIVER 01-14 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| RS-7 `a_repo_renamed_on_github_stays_chosen` | DELIVER 01-15 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| MG-1 `priyas_suggestions_are_all_still_there_after_the_update` | DELIVER 01-16 | Select priyaraman/driftwood embodies documentation-first Confidence 0.25 (speculative) Why: substantial README (12000 by | MISSING_FUNCTIONALITY ✅ |
| MG-2 `restarting_after_the_update_changes_nothing_priya_sees` | DELIVER 01-17 | assertion left == right failed: precondition: the first start updated the data once | MISSING_FUNCTIONALITY ✅ |
| RS-2b `the_next_scan_uses_the_choice_saved_during_the_last_one` | DELIVER 01-18 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| MG-3 `an_update_cut_off_before_it_finished_runs_again_cleanly` | DELIVER 01-19 | MISSING_FUNCTIONALITY: precondition: the update was not cut off — the app started normally with REVIEW_APP_TEST_FAULT=mi | MISSING_FUNCTIONALITY ✅ |
| AA-1 `priya_verifies_her_work_account_without_losing_her_personal_one` | DELIVER 02-01 | state-delta assertion failed: | MISSING_FUNCTIONALITY ✅ |
| AA-2 `an_account_that_cannot_be_added_changes_nothing` | DELIVER 02-02 | HeldByAnotherSignIn: expected ["github.com/dvolkov is already verified for another Bluesky account. Each GitHub account  | MISSING_FUNCTIONALITY ✅ |
| AA-3 `a_new_accounts_default_ticks_fit_the_limit` | DELIVER 02-03 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-1 `one_scan_covers_both_accounts_and_each_card_names_its_account` | DELIVER 02-04 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-2 `an_account_that_fails_its_check_is_skipped_named_and_hidden` | DELIVER 02-05 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-3 `github_failing_during_a_scan_hides_nothing` | DELIVER 02-06 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-4 `two_accounts_together_cannot_exceed_the_limit` | DELIVER 02-07 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-5 `when_every_account_fails_nothing_is_read` | DELIVER 02-08 | no card containing ["priyaraman/tidepool", "test-driven"] on http://127.0.0.1:57794/review | MISSING_FUNCTIONALITY ✅ |
| AS-6 `a_paused_scan_resumes_with_the_same_accounts_and_choices` | DELIVER 02-09 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AR-1 `priya_removes_an_account_she_no_longer_wants_scanned` | DELIVER 02-10 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AR-2 `cancelling_the_removal_changes_nothing` | DELIVER 02-11 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AR-3 `re_adding_offers_suggestions_again_but_never_declined_or_published_ones` | DELIVER 02-12 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AR-4a `removing_an_account_during_a_scan_deletes_its_pending_suggestions_at_once` | DELIVER 02-13 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AP-1 `a_repo_transferred_away_after_the_scan_can_no_longer_be_published` | DELIVER 02-14 | Preview before publishing · OpenLore review OpenLore review Preview before publishing priyaraman/kelp embodies dependenc | MISSING_FUNCTIONALITY ✅ |
| AP-2 `github_failing_at_preview_shows_no_preview_and_publishes_nothing` | DELIVER 02-15 | rate limited false: Preview before publishing · OpenLore review OpenLore review Preview before publishing priyaraman/tid | MISSING_FUNCTIONALITY ✅ |
| AP-3 `a_bio_edited_between_preview_and_confirm_stops_the_publish` | DELIVER 02-16 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AP-4 `a_claim_about_her_own_repo_is_published_only_if_she_still_owns_it_when_she_confirms` | DELIVER 02-17 | assertion left == right failed: KPI-RRA-4 audit: one passed check per request (preview, confirm) | MISSING_FUNCTIONALITY ✅ |
| AV-1 `one_owner_never_sees_or_changes_another_owners_accounts_or_choices` | DELIVER 02-18 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-7 `an_unverified_accounts_saved_repos_still_count_toward_the_limit` | DELIVER 02-19 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-2b `hidden_suggestions_come_back_after_re_verifying` | DELIVER 02-20 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AS-2c `a_scan_re_checks_an_unverified_account_and_reads_it_once_its_did_is_back` | DELIVER 02-21 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| AR-4b `a_resumed_scan_neither_reads_nor_adds_anything_for_a_removed_account` | DELIVER 02-22 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IM-1 `priya_names_a_public_repo_she_doesnt_own_and_gets_private_suggestions` | DELIVER 03-01 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IM-2 `a_repo_that_cannot_be_imported_is_refused_with_the_cause_and_the_fix` | DELIVER 03-02 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IM-3 `someone_with_no_verified_github_account_can_still_look_at_a_repo` | DELIVER 03-03 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IC-1 `an_imported_suggestion_offers_no_way_to_publish_and_says_who_can` | DELIVER 03-04 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IC-2 `a_forged_publish_request_for_a_repo_he_doesnt_own_writes_nothing` | DELIVER 03-05 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IC-3 `no_request_publishes_a_suggestion_about_a_repo_the_owner_doesnt_own` | DELIVER 03-06 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IC-4 `profile_and_share_preview_never_show_imported_suggestions` | DELIVER 03-07 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IC-5 `github_failing_at_confirm_publishes_nothing` | DELIVER 03-08 | rate limited false: Published · OpenLore review OpenLore review Published to your repo Your claim is in your own repo at | MISSING_FUNCTIONALITY ✅ |
| IC-6a `an_imported_repo_moves_to_the_owned_list_when_its_owner_account_is_verified` | DELIVER 03-09 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IC-7 `a_scan_moves_an_imported_repo_that_one_of_her_accounts_now_owns` | DELIVER 03-10 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IR-1 `removing_an_imported_repo_stops_reading_it` | DELIVER 03-11 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IR-2 `re_adding_an_imported_repo_offers_suggestions_again_but_never_declined_ones` | DELIVER 03-12 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IR-3 `removing_frees_room_under_the_limit` | DELIVER 03-13 | no "Save choice" list for github.com/dvolkov on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IR-4 `removing_an_imported_repo_during_a_scan_stops_its_read` | DELIVER 03-14 | no "Save choice" list for github.com/priyaraman on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IV-1 `one_owner_never_sees_or_acts_on_another_owners_imports` | DELIVER 03-15 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |
| IV-2 `disconnect_and_forget_me_forgets_her_accounts_choices_and_imports` | DELIVER 03-16 | no "Save choice" list for github.com/priya-tidal on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IV-3 `two_changes_at_once_never_take_the_total_past_the_limit` | DELIVER 03-17 | no "Save choice" list for github.com/dvolkov on the choose-repos page: | MISSING_FUNCTIONALITY ✅ |
| IC-6b `a_moved_suggestion_is_published_only_while_its_owner_accounts_bio_holds_her_did` | DELIVER 03-18 | no button or link labelled "Add repo" | MISSING_FUNCTIONALITY ✅ |

## Layer-2 properties (11; ids CORE-RRA-*, never a DELIVER scenario id)

| Property (test fn) | Ignore id | First failure | Class |
|---|---|---|---|
| `core_rra1_default_ticks_fill_only_the_room_left` | CORE-RRA-1 (bind with DELIVER step 01-02) | not yet implemented: RED: bind to review_domain::repo_scope::default_ticks (DELIVER 01-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra10_a_move_keeps_the_total_and_ticks_the_repo` | CORE-RRA-10 (bind with DELIVER step 03-09) | not yet implemented: RED: bind to review_domain::repo_scope::moves (DELIVER 03-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra2_a_save_is_admitted_iff_the_total_fits` | CORE-RRA-2 (bind with DELIVER step 01-04) | not yet implemented: RED: bind to review_domain::repo_scope::admit_save (DELIVER 01-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra2x_an_unverified_accounts_saved_ticks_still_count` | CORE-RRA-2x (bind with DELIVER step 02-19) | not yet implemented: RED: bind to review_domain::repo_scope::admit_save (DELIVER 01-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra3_admitted_changes_never_overfill_a_scan` | CORE-RRA-3 (bind with DELIVER step 03-02) | not yet implemented: RED: bind to review_domain::repo_scope::admit_import (DELIVER 03-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra4_a_scan_plans_only_what_was_chosen_from_passing_accounts` | CORE-RRA-4 (bind with DELIVER step 01-09) | not yet implemented: RED: bind to review_domain::repo_scope::{scan_intent, plan_account} (DELIVER 01-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra5_resumes_read_only_the_frozen_plan_each_repo_once` | CORE-RRA-5 (bind with DELIVER step 01-10) | not yet implemented: RED: bind to review_domain::repo_scope::resume_plan (DELIVER 01-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra6_only_a_listed_user_owner_whose_bio_holds_the_did_may_publish` | CORE-RRA-6 (bind with DELIVER step 02-14) | not yet implemented: RED: bind to claim_domain::authority::may_sign (DELIVER 02-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra7_visibility_follows_the_source_never_the_tick` | CORE-RRA-7 (bind with DELIVER step 02-05) | not yet implemented: RED: bind to review_domain::repo_scope::visible (DELIVER 02-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra8_at_most_three_accounts_one_owner_each` | CORE-RRA-8 (bind with DELIVER step 02-02) | not yet implemented: RED: bind to the pure review_domain::repo_scope::admit_account decision (DELIVER 02-*) | MISSING_FUNCTIONALITY ✅ |
| `core_rra9_an_import_verdict_is_one_of_a_closed_set_in_a_fixed_order` | CORE-RRA-9 (bind with DELIVER step 03-02) | not yet implemented: RED: bind to review_domain::repo_scope::classify_import (DELIVER 03-*) | MISSING_FUNCTIONALITY ✅ |

Result: **70 RED for a business reason · 0 BROKEN · 0 WRONG_ASSERTION.**
