# RED classification — owner-only-signing (pre-DELIVER gate)

Run **2026-10-20** against `main` (feature unimplemented: no `authority` verb, no v8, no gate in
the CLI composers). Every new scenario is `#[ignore = "DISTILL owner-only-signing …"]` except
PS-3 (active KPI-OOS-3 guard); the default run is green.

## Commands and results

| Command | Result |
|---|---|
| `cargo check --workspace --tests` | exit 0 |
| `cargo test -p openlore-test-support --lib the_authority_file` | 1 passed (the new `AuthorityFile` double) |
| `cargo test -p cli --test owner_only_signing --test owner_only_signing_repos --test owner_only_signing_readers --test owner_only_signing_core` (default) | green; PS-3 runs and passes |
| same with `-- --ignored` (+ `cargo test -p openlore-review-app --test owner_only_signing_review_app -- --ignored`) | 43 failed, 1 passed (PS-7, see below) |

Every GIVEN built only from shipped behaviour succeeded (identity link + verify, scrape, peer
add/pull of one peer, `claim add`, the frozen pre-v7 store, the review app's scan and publish,
network index seeding). Scenarios whose GIVEN uses the new `authority verify` verb fail there
with `unrecognized subcommand 'authority'` surfaced as a failed precondition: RED for the
missing feature (RRA precedent: a later-slice Given fails at the earlier slice's missing verb).
The layer-2 properties fail at their `todo!()` bindings.

**Holds today (ignored, not a red signal):** PS-7 (Jeff signs about his contested account) —
true today because nothing is gated; it becomes meaningful once the gate lands. **WS-OOS-1** row 1
passes today, row 2 fails: the business reason.

**Two test bugs found and fixed during the gate** (WRONG_ASSERTION / FIXTURE_BROKEN, before
hand-off): (1) `rows_about` read one line per claim, but `graph query --object` and `search`
print multi-line entries — rewritten to entry blocks; (2) two followed peers in one store fail
`peer pull` (the harness resolves only the latest peer) — RD-1/2/3/4/5/6 and RA-4 now use one
peer per store.

## First failure line per scenario

| Scenario (test fn) | First failure | Class |
|---|---|---|
| CORE-OOS-1 `only_disputes_withdrawals_and_links_are_weightless_kinds` | not yet implemented: RED: bind to claim_domain::authority::kind (owner-only-signing slice 1) | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-3 `a_claim_is_attested_exactly_when_the_authority_table_says_so` | not yet implemented: RED: bind to claim_domain::authority::classify (owner-only-signing slices 1-2) | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-2 `only_github_assertions_need_authority` | not yet implemented: RED: bind to claim_domain::authority::requirement (owner-only-signing slice 1) | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-5 `nothing_but_attested_assertions_moves_the_weight` | not yet implemented: RED: bind to scoring::score_attested (owner-only-signing slice 3) | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-4 `the_gate_permits_only_exempt_or_freshly_attested_claims` | not yet implemented: RED: bind to the claim_domain::authority gate over local checks (slice 1) | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-4x `the_signing_window_includes_seven_days_and_excludes_the_minute_after` | not yet implemented: RED: bind to the claim_domain::authority gate over local checks (slice 1) | MISSING_FUNCTIONALITY ✅ |
| IC-1 `jeff_reviews_what_he_imported_offline_one_repo_at_a_time` | assertion `left == right` failed: | MISSING_FUNCTIONALITY ✅ |
| IC-2 `importing_a_repo_keeps_its_candidates_locally_and_nowhere_else` | assertion `left == right` failed: precondition: `authority verify` must succeed; | MISSING_FUNCTIONALITY ✅ |
| IC-3 `with_nothing_imported_the_list_says_so` | assertion `left == right` failed: | MISSING_FUNCTIONALITY ✅ |
| IC-4 `importing_the_same_repo_twice_lists_each_candidate_once` | assertion `left == right` failed: | MISSING_FUNCTIONALITY ✅ |
| PS-2 `a_person_claim_jeff_cannot_attest_is_refused_says_why_and_who_can_sign` | assertion `left != right` failed: [no link] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| PS-4 `with_the_network_off_jeff_signs_about_himself_only_from_a_check_at_most_7_days_old` | assertion `left != right` failed: [2026-10-08T09:01:00Z] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| PS-5 `jeff_signs_an_inferred_adherence_only_about_himself` | assertion `left != right` failed: [BurntSushi] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| PS-6 `jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing` | candidate 2 about github:jeffbailey says `You can sign`: | MISSING_FUNCTIONALITY ✅ |
| PS-8 `after_jeff_renames_his_github_account_he_signs_only_under_the_new_login` | assertion `left != right` failed: [jeffbailey] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| RA-1 `jeff_imports_his_own_repo_and_signs_what_it_shows` | assertion `left == right` failed: one observation of tidepool, signer-independent | MISSING_FUNCTIONALITY ✅ |
| RA-10 `with_the_network_off_jeff_signs_a_repo_claim_only_from_fresh_checks` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RA-11a `a_claim_jeff_signed_about_someone_elses_repo_before_owner_only_signing_is_never_published` | [pre-feature] the refusal names github:BurntSushi/ripgrep; | MISSING_FUNCTIONALITY ✅ |
| RA-11b `publishing_a_stored_claim_later_asks_for_authority_only_if_it_was_never_published` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RA-12 `opening_a_store_made_before_owner_only_signing_keeps_everything_in_it` | assertion `left == right` failed: [first] v8 recorded once: [1, 3, 4, 5, 6, 7] | MISSING_FUNCTIONALITY ✅ |
| RA-13 `signing_from_an_import_rechecks_jeffs_own_link_first` | [bio holds] Jeff's own link was re-checked | MISSING_FUNCTIONALITY ✅ |
| RA-14 `with_no_github_claims_in_his_store_rechecking_authority_asks_github_nothing` | assertion `left == right` failed: | MISSING_FUNCTIONALITY ✅ |
| RA-15 `a_repo_github_does_not_have_is_recorded_as_not_found_and_cannot_be_signed` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/vanished` must s | MISSING_FUNCTIONALITY ✅ |
| RA-2 `jeff_imports_anything_public_but_signing_what_he_cannot_attest_is_refused_with_a_hand_off` | assertion `left != right` failed: [org repo] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| RA-3 `an_organization_repo_is_signed_only_by_a_did_its_authority_file_lists_exactly` | assertion `left != right` failed: [uppercase] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| RA-4 `rechecking_authority_labels_each_peers_repo_claims_and_records_one_observation_per_repo` | assertion `left == right` failed: | MISSING_FUNCTIONALITY ✅ |
| RA-5 `when_github_cannot_answer_rechecking_records_nothing_and_changes_nothing` | [rate limited] the refusal names github:acme-tools/anvil; | MISSING_FUNCTIONALITY ✅ |
| RA-6 `when_githubs_budget_runs_out_mid_import_the_candidates_are_listed_and_signing_waits_for_the_reset` | assertion `left != right` failed: [budget] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
| RA-7 `after_jeff_transfers_his_repo_his_earlier_claim_reads_third_party_and_nothing_is_withdrawn` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RA-8 `after_jeff_renames_his_repo_he_signs_only_under_the_name_github_serves_now` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool-rs` mus | MISSING_FUNCTIONALITY ✅ |
| RA-9 `a_listed_identity_stays_listed_whoever_observed_the_repo_last` | assertion `left == right` failed: precondition: `authority verify` must succeed; | MISSING_FUNCTIONALITY ✅ |
| RD-1 `every_claim_about_a_repo_is_shown_with_whose_word_it_is` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RD-10 `after_jeff_renames_his_account_his_claim_about_the_old_login_reads_third_party` | [third-party] did:plc:test-jeff's claim about github:jeffbailey reads `third-party`: | MISSING_FUNCTIONALITY ✅ |
| RD-2 `the_weighted_view_counts_only_attested_claims_and_says_what_it_left_out` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RD-3 `jeff_can_see_the_claims_left_out_per_author_without_changing_the_weight` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RD-4 `nothing_but_an_attested_claim_moves_the_weight` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/anvil` must succ | MISSING_FUNCTIONALITY ✅ |
| RD-5 `a_claim_marked_as_a_dispute_is_shown_as_a_dispute` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/anvil` must succ | MISSING_FUNCTIONALITY ✅ |
| RD-6 `jeffs_viewer_weighs_and_labels_claims_exactly_as_his_terminal_does` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must s | MISSING_FUNCTIONALITY ✅ |
| RD-7 `network_search_labels_every_github_claim_using_jeffs_own_check_when_he_has_one` | [no local check] the row reads `authority not checked here`; | MISSING_FUNCTIONALITY ✅ |
| RD-8 `claims_made_before_owner_only_signing_are_judged_by_the_same_rule` | assertion `left == right` failed: precondition: `authority verify` must succeed; | MISSING_FUNCTIONALITY ✅ |
| RD-9 `inferred_people_show_the_authority_of_each_repo_claim_they_rest_on` | assertion `left == right` failed: precondition: `authority verify` must succeed; | MISSING_FUNCTIONALITY ✅ |
| SA-1 `priyas_share_post_names_only_claims_about_repos_she_still_owns` | How I build, in claims I've published on OpenLore: dependency-pinning, test-driven. http://127.0.0.1:64757/@pr | MISSING_FUNCTIONALITY ✅ |
| WS-OOS-1 `only_jeff_can_sign_what_is_said_about_jeffs_github_account` | assertion `left != right` failed: [rachelk] a refused signature exits non-zero; | MISSING_FUNCTIONALITY ✅ |
