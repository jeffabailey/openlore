# review-app-repos-and-accounts: mutation report

DELIVER Phase 5, per-feature mutation testing (gate: at least 80% of viable
mutants caught). The mutants were generated against `c088606`, which holds
all 20 steps, the refactor and the review fixes up to R4. Re-review commits
`26a71e7`, `9b7b1d4` and `c17b68f` landed while the run was going. The new
tests were checked against `c17b68f`.

**Result: 767 of 778 viable mutants caught (98.6%). PASS.**
The first pass caught 695 of 778 (89.3%). New crafter-owned tests killed 72
of the 83 survivors. The other 11 are equivalent, reachable only through a
race, or need a capability the GitHub double does not have. Each is
explained below.

## Command

cargo-mutants 25.3.1 ignores tests that live in another package, and most
of this feature's behavior is covered by the `openlore-review-app`
acceptance targets. So, as in github-did-identity-link slices 1 to 4,
cargo-mutants listed the mutants and a manual runner ran them:

```
git diff main..c088606 -- <scope files> > scope.diff
cargo mutants --list --in-diff scope.diff --json --diff > mutants.json
```

Scope: `crates/claim-domain/src/authority.rs`;
`crates/review-domain/src/{repo_scope/*, plans.rs, link_plan.rs, kpi.rs, views/{github,publish,queue,repos,mod}.rs}`;
`crates/adapter-review-store/src/{github_accounts,imports,kpi,lib,probe,purge,removal,repo_choices,scan_runs,schema,suggestions}.rs`;
`crates/adapter-github/src/{client,lib,probe}.rs`;
`crates/ports/src/{github,review,probe,lib}.rs`;
`crates/openlore-review-app/src/{config,wiring,scan,http,main}.rs` and
`routes/{github,imports,publish,repos,review,scan,signin,settings}.rs`;
and the feature's hunks of `xtask/src/check_arch.rs`. The run was
exhaustive: all 981 listed mutants ran, nothing was sampled. No mutants
came from `link_plan.rs`, `views/mod.rs`, `purge.rs`, `kpi.rs` (store),
`adapter-github/src/probe.rs`, `ports/src/{probe,lib}.rs`, `http.rs` or
`main.rs`, because their feature lines are imports, re-exports, constants
or call-site arguments.

The runner applied each mutant's diff with `patch` in one of five scratch
git worktrees of `c088606`. Each worktree had its own `CARGO_TARGET_DIR`.
Neither `openlore-rra` nor the main checkout was mutated. For each mutant,
the runner ran these stages in order and stopped at the first failure.
Each stage is `cargo nextest run --no-run -p <pkg>` (a failure here means
**unviable**), then `cargo nextest run --max-fail 1:immediate -p <pkg>`
(a failure here means **caught**). A mutant that passed every stage is
**missed**. The time limit was 1500 s per command, and no mutant reached
it.

| Owning crate | Stages (packages, all their test targets) |
|---|---|
| claim-domain | claim-domain, review-domain, openlore-review-app |
| review-domain | review-domain, openlore-review-app |
| ports | ports, adapter-review-store, openlore-review-app |
| adapter-github | adapter-github, openlore-review-app |
| adapter-review-store | adapter-review-store, openlore-review-app |
| openlore-review-app | openlore-review-app |
| xtask | xtask |

`-p openlore-review-app` runs every review_app_* suite, including
`review_app_repos_and_accounts{,_several,_imports,_core}`,
`review_app_publish_permit_alone` and `page_github_requests`. Every stage
passed on unmutated `c088606` before the run started. The first attempt
ran nextest at full width with five workers, and the load average rose
above 160. It was stopped, results written after the stop were discarded,
and the run restarted with `NEXTEST_TEST_THREADS=4`.

## Totals

| Outcome | Count |
|---|---|
| caught | 767 (695 in the first pass, plus 72 after the new tests) |
| missed | 11 (83 in the first pass) |
| unviable | 203 (201, plus #189 and #469 reclassified, see below) |
| timeout | 0 |
| **total listed** | **981** |

**Kill rate = 767 / (767 + 11) = 98.6%.** First pass: 695 / 778 = 89.3%.

Most unviable mutants replace a return value with `Default::default()` for
a type that has no `Default` (`Reply`, `RepoFacts`, `ScanPlan`,
`LimitRefusal`, `Markup` built in a function returning `String`, and
others). They do not compile, so they are not test gaps.

## Per-file breakdown (final)

| File | Caught (first pass) | Caught (new tests) | Missed | Unviable |
|---|---|---|---|---|
| crates/adapter-github/src/client.rs | 15 | 1 | 0 | 2 |
| crates/adapter-github/src/lib.rs | 9 | 0 | 0 | 2 |
| crates/adapter-review-store/src/github_accounts.rs | 22 | 0 | 0 | 4 |
| crates/adapter-review-store/src/imports.rs | 7 | 1 | 0 | 1 |
| crates/adapter-review-store/src/lib.rs | 23 | 1 | 4 | 5 |
| crates/adapter-review-store/src/probe.rs | 2 | 0 | 0 | 8 |
| crates/adapter-review-store/src/removal.rs | 1 | 0 | 0 | 4 |
| crates/adapter-review-store/src/repo_choices.rs | 4 | 0 | 0 | 1 |
| crates/adapter-review-store/src/scan_runs.rs | 23 | 1 | 0 | 6 |
| crates/adapter-review-store/src/schema.rs | 0 | 0 | 0 | 1 |
| crates/adapter-review-store/src/suggestions.rs | 11 | 0 | 0 | 3 |
| crates/claim-domain/src/authority.rs | 20 | 0 | 0 | 3 |
| crates/openlore-review-app/src/config.rs | 2 | 0 | 0 | 3 |
| crates/openlore-review-app/src/routes/github.rs | 19 | 2 | 2 | 14 |
| crates/openlore-review-app/src/routes/imports.rs | 6 | 0 | 0 | 8 |
| crates/openlore-review-app/src/routes/publish.rs | 10 | 1 | 0 | 5 |
| crates/openlore-review-app/src/routes/repos.rs | 33 | 6 | 0 | 13 |
| crates/openlore-review-app/src/routes/review.rs | 36 | 4 | 1 | 19 |
| crates/openlore-review-app/src/routes/scan.rs | 0 | 0 | 0 | 1 |
| crates/openlore-review-app/src/routes/settings.rs | 0 | 0 | 0 | 1 |
| crates/openlore-review-app/src/routes/signin.rs | 23 | 12 | 0 | 2 |
| crates/openlore-review-app/src/scan.rs | 25 | 0 | 1 | 16 |
| crates/openlore-review-app/src/wiring.rs | 15 | 3 | 1 | 14 |
| crates/ports/src/github.rs | 2 | 0 | 0 | 0 |
| crates/ports/src/review.rs | 15 | 2 | 0 | 5 |
| crates/review-domain/src/kpi.rs | 7 | 14 | 0 | 12 |
| crates/review-domain/src/plans.rs | 2 | 0 | 0 | 3 |
| crates/review-domain/src/repo_scope/choose.rs | 26 | 6 | 0 | 8 |
| crates/review-domain/src/repo_scope/imports.rs | 9 | 0 | 0 | 5 |
| crates/review-domain/src/repo_scope/limits.rs | 53 | 0 | 0 | 4 |
| crates/review-domain/src/repo_scope/moves.rs | 15 | 0 | 0 | 4 |
| crates/review-domain/src/repo_scope/plan.rs | 31 | 1 | 0 | 19 |
| crates/review-domain/src/repo_scope/resume.rs | 19 | 0 | 0 | 6 |
| crates/review-domain/src/repo_scope/visibility.rs | 8 | 0 | 0 | 0 |
| crates/review-domain/src/views/github.rs | 18 | 0 | 0 | 0 |
| crates/review-domain/src/views/publish.rs | 18 | 1 | 0 | 0 |
| crates/review-domain/src/views/queue.rs | 39 | 3 | 0 | 1 |
| crates/review-domain/src/views/repos.rs | 39 | 2 | 0 | 0 |
| xtask/src/check_arch.rs | 88 | 11 | 2 | 0 |
| **Total** | **695** | **72** | **11** | **203** |

The security paths have no surviving mutant: `authority.rs`, `repo_scope/*`,
`plans.rs`, `removal.rs`, `routes/publish.rs`, the `routes/review.rs`
publish checks (#329 is equivalent, see below), and the wiring
`ScopeLocks` and page allowance (#481 is equivalent).

## U-RRA-D9 and U-RRA-D10

- **U-RRA-D9** (the per-repo presence re-read and the store's conditional
  insert): none survived. `insert_if_listed`, `insert_if_imported`,
  `source_of`, `add_pending_from_listed_source`, `add_pending_from_import`
  (#148-161) and the scan's `listed_sources` (#442) were all caught by the
  existing store and acceptance tests.
- **U-RRA-D10** (a move in one transaction): `move_import_to_account` and
  `move_rows` produced 7 mutants. Six were caught. #96 (`saved == 0` changed
  to `!=`, so the shown choice is written only where one was already saved)
  survived. It is now killed by a new store test (below). Mutation cannot
  split the move across two transactions, so it says nothing about
  atomicity. The one-transaction design stays as accepted in
  `upstream-issues.md`.

## Missed in the first pass: resolved by new tests (72)

Every kill below was checked by applying the mutant to `c17b68f` plus the
new tests and running only the named test (verified: it fails). Three
mutants no longer applied to `c17b68f`, because re-review changes edited
their lines. For each, the same mutation was made by hand on the current
line (marked "@HEAD").

### Pure core and ports

| Mutants | Function | New test |
|---|---|---|
| #510, #513, #514 | `kpi::AccountAddRefused::{of_check, label}` | `kpi::tests::each_account_add_refusal_counts_and_names_its_kind` |
| #518-#520, #522-#525 | `kpi::import_removed_age_event` (day constant, the `/`, both bucket arms) | `kpi::tests::a_removed_import_counts_the_day_bucket_it_was_held_for` |
| #526 | `kpi::import_refusal_event` | `kpi::tests::each_import_refusal_but_a_repeat_counts_its_kind` |
| #528, #529 | `kpi::publish_refusal_label` | `kpi::tests::a_refused_publish_is_named_by_its_reason` |
| #536 | `kpi::account_check_event` | `kpi::tests::a_failed_or_unfinished_account_check_counts_and_a_passed_one_does_not` |
| #546, #560, #561, #572, #576, #582 | `repo_scope::choose` (`is_new`, `eligible_count`, the `TicksRoom` guard, `differs_from_default`) | `repo_scope::tests::the_choose_list_marks_new_repos_and_the_room_its_default_ticks_leave` |
| #722 | `FrozenScope::plan` (`position == 0`: only a later account's repos are tagged `OtherAccount`, KPI-RRA-1) | `repo_scope::tests::only_repos_of_a_later_account_are_tagged_other_account` |
| #781 | `views::publish::confirmed_suggestion` (the hidden field that binds a confirm to its suggestion) | `page_contracts::each_confirm_names_the_suggestion_its_plan_publishes` |
| #798, #799 | `views::queue::scan_over_limit_message` | `views::queue::tests::an_over_limit_scan_says_how_many_to_untick` |
| #827 | `ScanDetails::account_of` (each card names its own account) | `views::queue::tests::each_card_names_the_account_its_suggestion_came_from` |
| #839, #840 | `ReposNotice::is_refusal` | `views::repos::tests::only_a_refused_change_is_a_refusal` |
| #493, #494 | `ports::PlanRowStatus::parse` ("read", "skipped" arms) | `ports review::tests::each_stored_plan_row_status_reads_back` |

### Adapters

| Mutants | Function | New test |
|---|---|---|
| #19 | `adapter-github client::positive_id` (`> 0` changed to `>= 0`) | `client::tests::a_zero_repo_or_owner_id_is_not_a_positive_id` |
| #59 | store `keep_v1_copy` (any create error treated as a name collision) | `migration_v1_to_v2::a_copy_the_folder_refuses_fails_with_the_folders_reason` (read-only folder; skips itself under root) |
| #96 | store `move_rows` (U-RRA-D10, above) | `imports::tests::a_move_writes_the_shown_choice_only_where_none_was_saved` |
| #140 | store `stored_source` ("imported" arm) | `scan_runs::tests::a_plan_row_reads_back_with_its_source` |

### openlore-review-app

These are in the new crafter-owned HTTP target
`crates/openlore-review-app/tests/page_mutation_pins.rs`, which reuses
the acceptance support read-only, as `page_github_requests.rs` does, plus
two `wiring.rs` unit tests.

| Mutants | Function | New test |
|---|---|---|
| #381, #383, #385, #387, #389, #391, #393, #395, #397, #399, #401, #403 | `signin::handle`: the `same_origin` guard on RemoveAccountConfirm, RemoveAccount, StartScan, Approve, Edit, PreviewEdit, ConfirmPublish, ApproveSelected, SaveRepoChoice, AddImport, RemoveImportConfirm, RemoveImport | `a_page_action_from_another_origin_is_forbidden` (session cookie and valid CSRF token, foreign `Origin`, so every page POST must answer 403) |
| #291, #293 | `repos::is_verified` | `a_save_for_an_account_she_does_not_list_verified_is_not_found` |
| #307, #308 | `repos::repos_page` (422 only for a refusal) | `the_repos_page_answers_422_only_for_a_refused_change` |
| #273 (@HEAD), #280 (@HEAD) | `repos::page_reads_admitted` changed to `true`; `read_listings` admission `+` changed to `*` (now `verified.len() * LISTING_PAGE_LIMIT + also`, re-review N2) | `the_choose_page_stops_listing_once_her_hourly_allowance_is_spent` |
| #359, #362, #363 | `review::queue_shown`, `any_account_verified` | `a_decline_without_a_verified_account_or_an_import_is_not_found` |
| #196 | `github::count_refusal` (KPI-RRA-2) | `a_refused_add_of_another_account_is_counted` |
| #205 | `github::listings_for_moves` (her other verified accounts are listed for the move's limit) | `a_verify_that_moves_an_import_lists_her_other_verified_account` |
| #251 | `publish::confirms_suggestion` (a confirm re-aimed at another suggestion) | `a_confirm_re_aimed_at_another_suggestion_publishes_nothing` |
| #345 (@HEAD) | `PublishChecks::repo` (`is_gone` guard) | `a_repo_gone_at_confirm_is_refused_as_gone` |
| #452, #457 | `wiring::counters_of` (AccountAddStarted, ScanAccountChecked arms) | `wiring::tests::an_add_start_and_a_scans_account_checks_are_counted` |
| #476 | `ScopeLocks::hold_within` (a free DID lock is dropped) | `wiring::tests::a_free_scope_lock_is_dropped` |

### xtask (`xtask/tests/review_app_architecture_pins.rs`)

| Mutants | Function | New test |
|---|---|---|
| #886, #921, #937, #960 | the four review-app scan shells changed to `Ok(vec![])` | `the_review_app_scans_report_an_empty_or_violating_tree` |
| #905, #906, #925 | `visit_item_mod` of the publish-plan and evidence collectors | `rules_see_into_non_test_inline_modules` |
| #952 | `PermitSerdeCollector::visit_item_impl` (`== "Deserialize"` changed to `!=`) | `a_non_serde_impl_for_the_permit_is_allowed` |
| #965 | `scan_signing_permit_not_serializable` (`&&` changed to `\|\|`) | `a_permit_defined_outside_its_module_leaves_the_rule_vacuous` |
| #979, #980 | `check_arch::run` changed to `Ok(1)` / `Ok(-1)` | `check_arch_passes_on_the_real_tree` |

## Still missed (11)

1. **#41, `adapter-review-store/src/lib.rs`, the release-only `stop_at`
   changed to `Ok(())`. Equivalent.** `MigrationFault` has no values in a
   release build, so the only reachable arm is `None => Ok(())`. With the
   mutant applied, `cargo check --release` builds.
2. **#58, `keep_v1_copy`: `now..now + ATTEMPTS` changed to
   `now..now * ATTEMPTS`. Equivalent in practice.** The range still starts
   at `now`. It reaches a second name only after a collision, and see 3
   for when a collision can happen.
3. **#60, #61, `keep_v1_copy`: the `AlreadyExists` guard changed to
   `false` / `!=`. Reachable only through a race.** Any existing
   `<stem>.v1.<secs>.bak` is found and reused before a name is created, so
   `create_new` meets `AlreadyExists` only if another process creates that
   name between the folder listing and the create. No deterministic test
   can produce that without a filesystem seam.
4. **#214, `routes/github.rs` `store_move`: `listed.repo_id == repo_id`
   changed to `!=`. Real, low impact.** The move looks the repo up in a
   listing that always holds it, so the mutant only stores another listed
   repo's `full_name` with the right `repo_id`. Pages name repos from the
   live listing. Only the stored `full_name_seen` of the moved row is
   wrong, and it would show only in a later "skipped" line. No crafter
   test can see it without reading the store file. Acceptance gap, low
   priority: "a moved import keeps its own name in the saved choice."
5. **#217, `routes/github.rs` `unverify_if_listed`: `&&` changed to `||`.
   Equivalent.** It is called only after the profile has been read and
   parsed. For a listed account, `prove_ownership` can then return only
   refusals that disprove ownership: the adapter rejects a profile with no
   login or no positive id before this point, and the linked id is `None`.
6. **#329, `routes/review.rs` `PublishChecks::proven`: `&&` changed to
   `||`. Equivalent in effect.** With the mutant, an already-verified
   permit account is marked verified again under the login on the repo
   read. That changes the stored login only after a rename, and a renamed
   repo is refused at confirm (`RepoRenamed`) before `proven` runs.
7. **#424, `scan.rs` `halt_of`: the `RateLimited` arm deleted. Not
   reachable with the current GitHub double.** `halt_of` maps only a
   listing failure after a passed bio check. The double's `rate_limit_for`
   limits every request, so the check is the step that halts (scan.rs
   line 340, covered). Test-double gap: the double needs a "rate-limit
   only `/users/<login>/repos`" switch, which is test-support territory.
8. **#481, `wiring.rs` `PageReads::admit`: `< hour` changed to `<= hour`.
   Equivalent in practice.** The two differ only when a request is exactly
   3600 s old to the nanosecond on `Instant`. The boundary logic is pinned
   in `budget::admit_page_reads`.
9. **#964, `scan_signing_permit_not_serializable`: `|=` changed to `^=`.
   Equivalent.** Only one path can equal `SIGNING_PERMIT_SOURCE`, so the
   flag is set at most once.
10. **#978, `check_arch::run` changed to `Ok(0)`. Equivalent on a healthy
    tree.** It is pinned only through the individual rule tests.

That is 11 mutants: #41, #58, #60, #61, #214, #217, #329, #424, #481,
#964, #978.

## Reclassified as unviable (2)

- **#189, `config.rs` release-only `test_fault` changed to
  `Ok(Default::default())`**, and **#469, `wiring.rs` release-only
  `migration_fault` changed to `Default::default()`**. These functions are
  `#[cfg(not(debug_assertions))]`, so the debug test build never compiles
  them. In the release build, where they exist, the mutants do not
  compile: `cargo check --release -p openlore-review-app` fails with
  `E0277: TestFault: Default` / `MigrationFault: Default is not satisfied`
  (checked on `c17b68f`).

## Acceptance gaps for the orchestrator

- #214: a moved import should keep its own `full_name_seen` (low priority,
  see above).
- #424 (test double): the GitHub double cannot rate-limit a listing after a
  passed bio check, so the "listing rate-limited, so the scan pauses" path
  is unobserved.
- Not a gap, but worth noting: the cross-origin guard on the page POSTs
  (12 mutants, older routes included) had no test anywhere. It is now
  pinned at crate level. The orchestrator may want an @security AT for it.

No file under `tests/acceptance/` was changed.

## Post-mutation safety

- All six scratch worktrees (`rra-mut-1` to `rra-mut-6`) were removed with
  `git worktree remove --force`. `openlore-rra` was never mutated. Its only
  changes are the new tests, the Cargo `[[test]]` entry for
  `page_mutation_pins`, and this report.
- On `c17b68f` plus the new tests, `cargo fmt --check` and `cargo clippy
  --tests` are clean for the touched packages, and
  `cargo nextest run -p claim-domain -p review-domain -p ports -p adapter-github -p adapter-review-store -p openlore-review-app -p xtask`
  passes (see the commit that adds this report).

## Re-review fixes re-run

The main run mutated `c088606`. The re-review fixes `c088606..c17b68f`
changed production lines that were never mutated, so this run mutates only
those lines.

**Result: 54 of 55 viable mutants caught (98.2%). PASS.**
The first pass caught 52 of 55 (94.5%). Two new tests killed 2 of the 3
survivors. The last one is equivalent.

### Command

```
git diff c088606..c17b68f -- <scope files> > scope.diff
cargo mutants --list --in-diff scope.diff --json --diff > mutants.json
```

Scope: `crates/review-domain/src/{budget.rs, views/settings.rs}`,
`crates/ports/src/github.rs`, `crates/adapter-review-store/src/lib.rs`,
`crates/openlore-review-app/src/{wiring,admin}.rs` and
`routes/{review,repos,github,publish,settings}.rs`. The mutants were
listed on `9c7a0a2` (`c17b68f` plus the main run's tests). Those tests only
add lines below the mutated ones, so the diff's line numbers still match.
All 90 listed mutants ran. Nothing was sampled.

The runner is the same one the main run used. It applied each mutant with
`patch` in one of three scratch worktrees of `9c7a0a2`, each with its own
`CARGO_TARGET_DIR`, and ran the same stages per owning crate
(`--no-run` failing means unviable, a test failing means caught), with
`NEXTEST_TEST_THREADS=4`. Every stage passed on unmutated `9c7a0a2` first
(539 tests). No mutant timed out.

### Totals

| Outcome | Count |
|---|---|
| caught | 54 (52 in the first pass, plus 2 after the new tests) |
| missed | 1 (3 in the first pass) |
| unviable | 35 |
| timeout | 0 |
| **total listed** | **90** |

**Kill rate = 54 / (54 + 1) = 98.2%.**

| File | Caught (first pass) | Caught (new tests) | Missed | Unviable |
|---|---|---|---|---|
| crates/adapter-review-store/src/lib.rs | 3 | 0 | 0 | 1 |
| crates/openlore-review-app/src/admin.rs | 1 | 0 | 0 | 12 |
| crates/openlore-review-app/src/routes/github.rs | 1 | 0 | 0 | 0 |
| crates/openlore-review-app/src/routes/publish.rs | 0 | 0 | 0 | 3 |
| crates/openlore-review-app/src/routes/repos.rs | 9 | 0 | 0 | 1 |
| crates/openlore-review-app/src/routes/review.rs | 9 | 0 | 0 | 6 |
| crates/openlore-review-app/src/routes/settings.rs | 0 | 0 | 0 | 2 |
| crates/openlore-review-app/src/wiring.rs | 10 | 1 | 1 | 8 |
| crates/ports/src/github.rs | 2 | 0 | 0 | 0 |
| crates/review-domain/src/budget.rs | 13 | 1 | 0 | 2 |
| crates/review-domain/src/views/settings.rs | 2 | 0 | 0 | 0 |
| **Total** | **52** | **2** | **1** | **35** |

The N1 permit-read admission (`github_read_admitted`, the `repo` guard and
the `bio` check), the N2 listing admission and refund in `read_listings`,
the N6 `under_scope` status writes, and `pages_read` have no surviving
mutant. The unviable mutants replace a return value with
`Default::default()` for a type that has no `Default` (`Reply`,
`Response<Full<Bytes>>`, `StoreError`, `LogEvent`, `ForgetFailure`). Because
of this, the N7 `put_back` lines in `publish.rs` and the N8 `ScopeBusy`
arms in `settings.rs` and `admin.rs` produced no viable mutant, so this run
says nothing about them either way.

### Survivors resolved by new tests (2)

Each kill was checked by applying the mutant together with the new test
and running only that test (verified: it fails).

| Mutant | Function | New test |
|---|---|---|
| `budget.rs:176` `spent_in_hour`: `< HOUR_SECS` changed to `<=` | a request exactly an hour old still counted | `budget::tests::a_page_request_exactly_an_hour_old_has_expired` |
| `wiring.rs:878` `PageReads::refund`: `times.len() - given_back` changed to `+` | the DID's own requests never given back (only the app's) | `wiring::tests::a_refund_gives_the_did_back_its_unused_requests` |

### Still missed (1)

- **`wiring.rs:853` `PageReads::admit`: `in_hour` `< hour` changed to
  `<= hour`. Equivalent in practice.** This is the same boundary as #481
  in the main run. The two differ only when a request is exactly 3600 s
  old to the nanosecond on `Instant`. The boundary is pinned in
  `budget::spent_in_hour` by the new test above.

### Post-mutation safety

- The three scratch worktrees (`rra-rerun-1` to `rra-rerun-3`) and their
  target folders were removed. `openlore-rra` was never mutated.
- On `9c7a0a2` plus the two new tests, `cargo fmt --all` made no changes,
  `cargo clippy -p review-domain -p ports -p adapter-review-store -p openlore-review-app --tests -- -D warnings`
  is clean, and `cargo nextest run` for the same four packages passes
  (541 tests).
- No file under `tests/acceptance/` was changed.
