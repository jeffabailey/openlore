# RED classification — owner-only-signing (pre-DELIVER gate)

Re-run **2026-10-20, review iteration 1** against `main` (feature unimplemented: no `authority`
or `candidates` verb, no v8, no gate in the CLI composers). Every new scenario is
`#[ignore = "DISTILL owner-only-signing …"]` except the ACTIVE guards PS-3, PS-7 and IC-5; the
default run is green.

## Commands and results

| Command | Result |
|---|---|
| `cargo check --workspace --tests` | exit 0 |
| `cargo test -p openlore-test-support --lib` | green (incl. `the_authority_file_is_served_per_shape…` and the new `a_profile_stops_answering_after_the_reads_it_serves`) |
| `cargo test -p cli --test owner_only_signing --test owner_only_signing_repos --test owner_only_signing_readers --test owner_only_signing_core` (default) | green; PS-3, PS-7, IC-5 run and pass |
| `cargo test -p openlore-review-app --test owner_only_signing_review_app` (default) | green (SA-1 ignored) |
| same with `--no-fail-fast -- --ignored --skip regenerate` (+ review app `-- --ignored`) | 47 failed, 0 passed — every one MISSING_FUNCTIONALITY (table) |
| `OPENLORE_REGEN_V7=1 cargo test -p cli --test owner_only_signing_repos -- --ignored regenerate` | 1 passed — wrote and self-checked `tests/fixtures/owner-only-signing-v7-store/` |
| `cargo test -p cli --test identity_link_readers a_link_its_peer_author_retracted` | 1 passed — a peer publishing twice into one store works (review L-2; RD-4's "withdrawn" row relies on it) |

## Active guards (scaffold policy, DWD-OOS-2)

A scenario that holds today AND must hold after DELIVER runs active; one that holds today only
vacuously but becomes meaningful under the gate is still active when it would catch a regression
the moment the gate lands (review H-3).

| Scenario | Why active |
|---|---|
| PS-3 | KPI-OOS-3: dispute and withdrawal must never be gated; now asserts the published wire body (review H-1) |
| PS-7 | contested account still signs — the first gate commit that over-refuses fails it (review H-3; verified green today) |
| IC-5 | imported candidates never feed `infer people` — fails the moment slice 5 wires them into inference (C-OOS-7) |
| RD-5 | **not** active: its Given needs `authority verify` (GIVEN-blocked); re-checked in iteration 1 |

## Wrong-reason failures found and fixed in iteration 1 (before hand-off)

1. `jeffs_entries` counted every line naming Jeff's DID in an entry (3 per claim in
   `graph query --subject` output) — now counts one `cid:` line per claim (WRONG_ASSERTION).
2. PS-9 and RA-2's person row used `scrape person <login> --sign 1` in a world with no recorded
   contributions, so no person candidate existed ("valid range 1..0", FIXTURE_BROKEN). PS-9 now
   uses PS-5's store; RA-2 records Rachel's signed kelp claim and kelp's contributors. Both
   re-run: they now reach the business assertion (person row signs today; PS-9 rows 1-2 pass
   today, row 3 fails for the business reason).
3. IC-5 matched the philosophy's full id against reader output that prints the short name —
   now `short_name(…)` (also applied to IC-1/2/4).
4. RA-12b ran the permitted row first, so the first failure was the v8 guardrail; the refusal
   row now runs first.

## First failure per scenario (review M-1: full lines, GIVEN-blocked noted)

"GIVEN-blocked" = the scenario fails in a Given that uses the new `authority verify` verb
(`unrecognized subcommand 'authority'` surfaced as a failed precondition) — RED for the missing
slice-2 verb, RRA precedent. IC-1/3/4, RA-4 and RA-14 (re-run per M-1) fail in their WHEN on the
missing verb itself.

| Scenario (test fn) | First failure | Where | Class |
|---|---|---|---|
| CORE-OOS-1 `only_disputes_withdrawals_and_links_are_weightless_kinds` | not yet implemented: RED: bind to claim_domain::authority::kind (owner-only-signing slice 1) thread 'only_disputes_withdrawals_and_links_are_weightless_kinds' (72055064) panicked at crates/cli/../../t | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-2 `only_github_assertions_need_authority` | not yet implemented: RED: bind to claim_domain::authority::requirement (owner-only-signing slice 1) thread 'only_github_assertions_need_authority' (72055065) panicked at crates/cli/../../tests/accepta | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-3 `a_claim_is_attested_exactly_when_the_authority_table_says_so` | not yet implemented: RED: bind to claim_domain::authority::classify (owner-only-signing slices 1-2) thread 'a_claim_is_attested_exactly_when_the_authority_table_says_so' (72055062) panicked at crates/ | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-4 `the_gate_permits_only_exempt_or_freshly_attested_claims` | not yet implemented: RED: bind to the claim_domain::authority gate over local checks (slice 1) thread 'the_gate_permits_only_exempt_or_freshly_attested_claims' (72055066) panicked at crates/cli/../../ | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-4x `the_signing_window_includes_seven_days_and_excludes_the_minute_after` | not yet implemented: RED: bind to the claim_domain::authority gate over local checks (slice 1) note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| CORE-OOS-5 `nothing_but_attested_assertions_moves_the_weight` | not yet implemented: RED: bind to scoring::score_attested (owner-only-signing slice 3) thread 'nothing_but_attested_assertions_moves_the_weight' (72055063) panicked at crates/cli/../../tests/acceptanc | layer-2 `todo!()` | MISSING_FUNCTIONALITY ✅ |
| IC-1 `jeff_reviews_what_he_imported_offline_one_repo_at_a_time` | assertion `left == right` failed: error: unrecognized subcommand 'candidates' | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| IC-2 `importing_a_repo_keeps_its_candidates_locally_and_nowhere_else` | assertion `left == right` failed: precondition: `authority verify` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| IC-3 `with_nothing_imported_the_list_says_so` | assertion `left == right` failed: error: unrecognized subcommand 'candidates' | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| IC-4 `importing_the_same_repo_twice_lists_each_candidate_once` | assertion `left == right` failed: error: unrecognized subcommand 'candidates' | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-2 `a_person_claim_jeff_cannot_attest_is_refused_says_why_and_who_can_sign` | assertion `left != right` failed: [no link] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-4 `with_the_network_off_jeff_signs_about_himself_only_from_a_check_at_most_7_days_old` | assertion `left != right` failed: [2026-10-08T09:01:00Z] a refused signature exits non-zero (rows 1-2 pass today) | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-5 `jeff_signs_an_inferred_adherence_only_about_himself` | assertion `left != right` failed: [BurntSushi] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-6 `jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing` | candidate 2 about github:jeffbailey says `You can sign`: [2] NEW github:jeffbailey adheres to memory-safety | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-8 `after_jeff_renames_his_github_account_he_signs_only_under_the_new_login` | assertion `left != right` failed: [jeffbailey] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| PS-9 `when_github_cannot_answer_as_jeff_signs_from_an_import_his_latest_check_decides` | assertion `left != right` failed: [TooBusy at 2026-10-08T09:01:00Z] a refused signature exits non-zero (rows 1-2 pass today: no refresh, no gate) | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-1 `jeff_imports_his_own_repo_and_signs_what_it_shows` | [owner] did:plc:test-jeff's claim about github:jeffbailey/tidepool reads `owner`: author: did:plc:test-jeff (you) | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-10 `with_the_network_off_jeff_signs_a_repo_claim_only_from_fresh_checks` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RA-11a `a_claim_jeff_signed_about_someone_elses_repo_before_owner_only_signing_is_never_published` | [network on] the refusal names github:BurntSushi/ripgrep; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-11b `publishing_a_stored_claim_later_asks_for_authority_only_if_it_was_never_published` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RA-12 `opening_a_store_made_before_owner_only_signing_keeps_everything_in_it` | assertion `left == right` failed: guardrail: v8 recorded once: [1, 3, 4, 5, 6, 7] left: 0 | guardrail (upgrade v8 missing; primary Thens hold today) | MISSING_FUNCTIONALITY ✅ |
| RA-12b `jeffs_identity_link_checks_carry_over_when_his_store_gains_owner_only_signing` | assertion `left != right` failed: [2026-10-08T09:01:00Z] a refused signature exits non-zero (row 1 golden `identity list` matches today) | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-13 `signing_from_an_import_rechecks_jeffs_own_link_first` | [BioHolds at 2026-10-21T09:00:00Z] Jeff's own link was re-checked | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-14 `with_no_github_claims_in_his_store_rechecking_authority_asks_github_nothing` | assertion `left == right` failed: error: unrecognized subcommand 'authority' | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-15 `a_repo_github_does_not_have_is_recorded_as_not_found_and_cannot_be_signed` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/vanished` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RA-16 `when_githubs_budget_is_gone_rechecking_says_so_records_nothing_and_never_shows_his_token` | [AnonymousExhausted] the refusal names github:acme-tools/anvil; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-2 `jeff_imports_anything_public_but_signing_what_he_cannot_attest_is_refused_with_a_hand_off` | assertion `left != right` failed: [org repo] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-3 `an_organization_repo_is_signed_only_by_a_did_its_authority_file_lists_exactly` | assertion `left != right` failed: [uppercase] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-4 `rechecking_authority_labels_each_peers_repo_claims_and_records_one_observation_per_repo` | assertion `left == right` failed: error: unrecognized subcommand 'authority' | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-5 `when_github_cannot_answer_rechecking_records_nothing_and_changes_nothing` | [rate limited] the refusal names github:acme-tools/anvil; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-6 `when_githubs_budget_runs_out_mid_import_the_candidates_are_listed_and_his_latest_check_decides` | assertion `left != right` failed: [never checked] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RA-7 `after_jeff_transfers_his_repo_his_earlier_claim_reads_third_party_and_nothing_is_withdrawn` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RA-8 `after_jeff_renames_his_repo_he_signs_only_under_the_name_github_serves_now` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool-rs` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RA-9 `a_listed_identity_stays_listed_whoever_observed_the_repo_last` | assertion `left == right` failed: precondition: `authority verify` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-1 `every_claim_about_a_repo_is_shown_with_whose_word_it_is` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-10 `after_jeff_renames_his_account_his_claim_about_the_old_login_reads_third_party` | [third-party] did:plc:test-jeff's claim about github:jeffbailey reads `third-party`: author_did: did:plc:test-jeff (you) | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RD-11 `asking_for_unattested_claims_without_the_weighted_view_is_a_usage_mistake` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-2 `the_weighted_view_counts_only_attested_claims_and_says_what_it_left_out` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-3 `jeff_can_see_the_claims_left_out_per_author_without_changing_the_weight` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-4 `nothing_but_an_attested_claim_moves_the_weight` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/anvil` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-5 `a_claim_marked_as_a_dispute_is_shown_as_a_dispute` | assertion `left == right` failed: precondition: `authority verify --subject github:acme-tools/anvil` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-6 `jeffs_viewer_weighs_and_labels_claims_exactly_as_his_terminal_does` | assertion `left == right` failed: precondition: `authority verify --subject github:jeffbailey/tidepool` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-7 `network_search_labels_every_github_claim_using_jeffs_own_check_when_he_has_one` | [no local check] the row reads `authority not checked here`; Discovery indexes ONLY public, signed claims, verified before indexing. Nothing private is read or aggregated. | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| RD-8 `claims_made_before_owner_only_signing_are_judged_by_the_same_rule` | assertion `left == right` failed: precondition: `authority verify` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| RD-9 `inferred_people_show_the_authority_of_each_repo_claim_they_rest_on` | assertion `left == right` failed: precondition: `authority verify` must succeed; | GIVEN-blocked (`authority verify` missing) | MISSING_FUNCTIONALITY ✅ |
| SA-1 `priyas_share_post_names_only_claims_about_repos_she_still_owns` | How I build, in claims I've published on OpenLore: dependency-pinning, test-driven. http://127.0.0.1:60449/@priyaraman.bsky.social note: run with `RUST_BACKTRACE=1` environment variable to display a b | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
| WS-OOS-1 `only_jeff_can_sign_what_is_said_about_jeffs_github_account` | assertion `left != right` failed: [rachelk] a refused signature exits non-zero; | WHEN/THEN | MISSING_FUNCTIONALITY ✅ |
