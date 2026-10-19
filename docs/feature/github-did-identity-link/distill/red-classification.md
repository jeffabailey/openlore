# RED classification — github-did-identity-link (pre-DELIVER gate)

Final pass, run **2026-10-14** against `main` (feature unimplemented). Hand-off state and CI
policy: the single statement is in the header of `test-scenarios.md` (D-d).

## Commands and results

| Command | Result |
|---|---|
| `rustfmt --edition 2021 --check` on `identity_link.rs`, `identity_link_readers.rs`, `identity_link_core.rs`, `review_app_identity_link.rs`, `review_app_identity_link_readers.rs`, `crates/test-support/src/fake_github_accounts.rs` (the `mod support` / `review_app` modules are checked through them) | clean |
| `cargo clippy -p cli --test identity_link --test identity_link_readers --test identity_link_core --test review_app_identity_link_readers -- -D warnings` | `Finished`, 0 warnings |
| `cargo clippy -p openlore-review-app --test review_app_identity_link -- -D warnings` | `Finished`, 0 warnings |
| `cargo test -p cli --test identity_link --test identity_link_readers --test identity_link_core --test review_app_identity_link_readers` (default) | ok — identity_link 2 passed (support self-tests) / 33 ignored; core 0 / 10 ignored; readers 4 passed (IR-13, IR-17 + 2 support) / 17 ignored; RR 4 passed (support) / 5 ignored |
| `cargo test -p openlore-review-app --test review_app_identity_link` (default) | ok — 3 passed (RA-3 + 2 support) / 8 ignored |
| the same two commands with `--no-fail-fast -- --include-ignored` | identity_link 32 failed / 3 passed (support ×2, the fixture regenerator, which skips without `OPENLORE_REGEN_PRE_V7`); core 10 failed; readers 17 failed / 4 passed; RR 5 failed / 4 passed; review app 8 failed / 3 passed |
| `OPENLORE_REGEN_PRE_V7=1 cargo test -p cli --test identity_link -- --ignored regenerate` | ok — fixture written from the v6 binary; self-check (schema versions, rows, read-back) passed |

## Classification of every failure

Every GIVEN that uses only shipped verbs succeeded (scrapes, `claim add`, `claim counter`,
`peer add`/`pull`, `infer people`, review-app sign-in and verification, the frozen-store import
with its v6 precondition). No scenario fails in compile, fixture or setup.

| Scenario(s) | Failing step | Cause | Class |
|---|---|---|---|
| WS-GDL-1, IL-2, IL-3, IL-4, IL-6, IL-12, IV-8 | WHEN `identity link` / `verify` (IL-3/IL-6 first hit the pinned exit 1; an unknown verb exits 2) | `unrecognized subcommand 'identity'` | MISSING_FUNCTIONALITY ✅ |
| IL-7 | THEN exit 1 + "not a GitHub username" | same | MISSING_FUNCTIONALITY ✅ |
| IL-9a | THEN stderr names the "repo host" | same | MISSING_FUNCTIONALITY ✅ |
| IL-13a | WHEN the first `identity list` on the frozen v6 store (precondition `schema_version` head 6 passed) | same | MISSING_FUNCTIONALITY ✅ |
| IL-13b | GIVEN the first open (chained) | same | MISSING_FUNCTIONALITY ✅ |
| IL-5, IL-10, IL-11 (its before-view precondition passed), IL-14, IV-1..IV-7, IV-9, IV-10, IF-1a, IF-2, IR-3, IR-4, IR-5, IR-8a/b, IR-9, IR-18 | GIVEN "Jeff linked …" (chained) | same | MISSING_FUNCTIONALITY ✅ |
| IF-1b, IR-6, IR-7, IR-10, IR-11, IR-12, RR-3 | GIVEN "Jeff re-checked …" after a successful `peer pull` | same | MISSING_FUNCTIONALITY ✅ |
| IL-9b | GIVEN "the link is signed locally" (chained on IL-9a) | no link signed | MISSING_FUNCTIONALITY ✅ |
| IR-1, IR-2, IL-8c, RR-1a, RR-1b | `identity list` / `verify` (IL-8c: the counter published, author ≠ subject on the wire, nothing projectable) | same | MISSING_FUNCTIONALITY ✅ |
| IL-8a | THEN `claim add --predicate sameAs` exits 1 | today it signs and publishes | MISSING_FUNCTIONALITY ✅ |
| IL-8b | THEN the inference batch exits 1 | today it signs the edited claim (exit 0) | MISSING_FUNCTIONALITY ✅ |
| IR-14 | THEN `github:rachelk` has no weighted row | `did:plc:rachel-test weight 1.00 [SPARSE]` | MISSING_FUNCTIONALITY ✅ |
| IR-15 | THEN the `/score` region is identical with / without the link | the link is scored | MISSING_FUNCTIONALITY ✅ |
| IR-16 | THEN "authority not checked here" | label absent (the link is listed and attributed) | MISSING_FUNCTIONALITY ✅ |
| RA-1a, RA-1b, RA-2, RA-4..RA-8, RR-2a, RR-2b | WHEN pressing "Publish this link" | `no button or link labelled "Publish this link"` | MISSING_FUNCTIONALITY ✅ |
| CORE-1, 1x, 2, 3, 4, 5, 5b, 7 | `sut_*` binding | `not yet implemented: RED: bind to claim_domain::identity_link::…` | MISSING_FUNCTIONALITY ✅ (Mandate 7 seam; gate DD-GDL-16) |
| CORE-6, CORE-6x | REAL `scoring::score` | a `sameAs` row is ranked as a pairing | MISSING_FUNCTIONALITY ✅ |
| **IR-13**, **IR-17**, **RA-3** | — | pass | ACTIVE regression guards (D-d) |

Result: **0 BROKEN, 0 WRONG_ASSERTION.**

## Slice-2 review RED scenarios (2026-10-17)

Run: `cargo test -p cli --test identity_link_readers -- --include-ignored`, after the slice-2
DELIVER, against `main` at `c07bfe9`. Every GIVEN (link, `peer pull` of the link and of the
retraction/supersession marker, `identity verify`) succeeded; each failure is the reviewer's
failure scenario.

| Scenario | Failing step | Actual failure | Class |
|---|---|---|---|
| IR-18b (B1) | THEN `--did did:plc:test-jeff` shows github:jeffbailey as contested | `expected "github:jeffbailey is contested" naming did:plc:test-jeff` — stdout `did:plc:test-jeff is github:jeffbailey — verified today (record …)` | MISSING_FUNCTIONALITY ✅ |
| IR-8c (B2) | THEN nothing links Rachel to github:rachelk after her retraction was pulled | `did:plc:rachel-test and github:rachelk must not be shown as a link at all` — stdout `did:plc:rachel-test claims github:rachelk — not checked (record …)` | MISSING_FUNCTIONALITY ✅ |
| IR-8d (B2) | THEN the line names only the successor record | `the line names only the new record " (record <successor>)"` — line ends `(records <successor>, <superseded>)` | MISSING_FUNCTIONALITY ✅ |
| IR-12b (H2-1) | — | pass: a re-registered login decides nothing (`check_on_profile` gates on the account number) | ACTIVE regression guard |
| IR-12c (H2-2, U-15 scope) | — | pass | ACTIVE regression guard |
| IR-15 (H1), IR-16 (L1) hardened; `link_checks` (M1) | — | pass | — |

Default run: readers 25 passed / 3 ignored; identity_link 39 passed / 1 ignored.

## Slice 4 — the viewer's linked-identity line (2026-10-19, re-run after review cycle 2 — current)

Run against `main` at `9a67df6` (slice 4 unimplemented), after `cargo build -p openlore-indexer -p cli --bins`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy -p cli --tests -- -D warnings` | fails ONLY in `review_app_self_attested_readers`, on another feature's uncommitted `support/review_app/repos_and_accounts.rs` (review-app-repos-and-accounts, in progress in parallel; `GithubRepo::pushed_at`, `GithubAccount::organization` not yet in test-support). Not slice-4 code. |
| `cargo clippy -p cli --test viewer_linked_identity --test viewer_linked_identity_invariants --test identity_link_readers -- -D warnings` | `Finished`, 0 warnings |
| `cargo clippy -p xtask --tests -- -D warnings` | `Finished`, 0 warnings |
| `cargo nextest run -p cli --test viewer_linked_identity --test viewer_linked_identity_invariants --test identity_link_readers --run-ignored all --no-fail-fast` | 54 run: 36 passed (every `identity_link_readers` test, IR-15 rewritten included), 18 failed (all 18 slice-4 fns, VL-17a and VL-17b separately; clocks pinned in VL-1, 2, 15, 17a, 17b, 20) |
| `cargo nextest run -p xtask --test viewer_fail_seam --run-ignored all --no-fail-fast` | 13 run: 12 passed, 1 failed (the new VL-G case) |
| default run (ignored skipped) | slice-4 targets: 0 run / 18 ignored; xtask: 12 passed / 1 ignored |

Every GIVEN reached succeeded (link, `peer add`/`pull`/`remove`, re-follow, `identity verify` incl.
`--github jeff-at-work` for the contested account, rename + re-registration, the future-dated seeded
check, `claim add`, the network index ingest + serve, the frozen pre-v7 store); every failure is the
missing region or the unbound seam.

| Scenario | Test fn | First failure line | Class |
|---|---|---|---|
| VL-1 (WS) | `rachels_contributor_page_shows_her_github_account_as_verified` | `[Rachel's page] expected a \`Linked identities\` region;` | MISSING_FUNCTIONALITY ✅ |
| VL-2 + VL-18 | `every_link_standing_reads_in_the_viewer_exactly_as_identity_list_prints_it` | `assertion \`left == right\` failed: [jeff, same day] the viewer reads exactly as \`identity list\` prints it;` | MISSING_FUNCTIONALITY ✅ |
| VL-2p | `for_every_listing_the_viewers_lines_are_the_identity_list_lines_without_backticks` | `not yet implemented: RED: bind to viewer_domain::linked_identity — the LinkedIdentities::Lines render (slice 4, D-VL-5)` | MISSING_FUNCTIONALITY ✅ (Mandate 7 seam, DD-GDL-16 practice) |
| VL-3 | `a_github_persons_page_shows_the_did_that_linked_the_account_in_any_spelling` | `[github:jeffbailey] expected a \`Linked identities\` region;` | MISSING_FUNCTIONALITY ✅ |
| VL-17a | `until_jeff_rechecks_a_renamed_accounts_line_stays_on_its_old_logins_page` | `expected \`did:plc:test-jeff is github:jeffbailey (verified …)\`; [row: github:jeffbailey]` | MISSING_FUNCTIONALITY ✅ |
| VL-17b | `when_jeff_rechecks_a_renamed_accounts_line_moves_to_its_new_logins_page` | `expected \`did:plc:test-jeff is github:jeff-b (verified …)\`; [row: after re-check, github:jeff-b]` (after the real re-check) | MISSING_FUNCTIONALITY ✅ |
| VL-4 | `a_contested_account_reads_contested_on_every_page_about_it` | `expected \`github:jeffbailey is contested\` naming did:plc:test-jeff;` | MISSING_FUNCTIONALITY ✅ |
| VL-5 | `a_strangers_link_about_tobias_and_flawed_link_records_never_show_as_anyones_linked_identity` | `assertion \`left == right\` failed: [Rachel's page] exactly one line: []` (Tobias's pages passed: no region) | MISSING_FUNCTIONALITY ✅ |
| VL-6 | `a_retracted_or_superseded_link_and_the_retraction_itself_never_show` | `[Rachel's page] exactly one line: []` | MISSING_FUNCTIONALITY ✅ |
| VL-7 | `a_link_from_a_peer_jeff_removed_does_not_show_until_he_follows_her_again` | `expected \`did:plc:rachel-test is github:rachelk (verified …)\`;` (row "followed") | MISSING_FUNCTIONALITY ✅ |
| VL-10 | `a_claims_detail_page_shows_its_authors_linked_identity` | `[claim detail] expected a \`Linked identities\` region;` | MISSING_FUNCTIONALITY ✅ |
| VL-11 + VL-19 | `network_search_shows_each_authors_linked_identity_and_labels_link_rows` | `expected \`did:plc:rachel-test is github:rachelk (verified …)\`; [row: Rachel's group]` | MISSING_FUNCTIONALITY ✅ |
| VL-12 | `a_person_with_no_link_shows_no_linked_identities_region` | `[Jeff's page] expected a \`Linked identities\` region;` (positive anchor) | MISSING_FUNCTIONALITY ✅ |
| VL-15 | `a_check_dated_in_the_future_is_ignored` | `assertion \`left == right\` failed: [future check] the viewer reads exactly as \`identity list\` prints it;` | MISSING_FUNCTIONALITY ✅ |
| VL-20 | `a_store_made_before_identity_links_shows_every_page_and_no_linked_identity_it_does_not_hold` | `expected \`did:plc:test-jeff is github:jeffbailey (verified …)\`; [row: after Jeff linked, his page]` (the "as made" rows passed: the viewer opens the v6 store, 200, no region, no refused probe) | MISSING_FUNCTIONALITY ✅ |
| VL-8 + VL-16 | `showing_linked_identities_asks_github_nothing_changes_nothing_and_reads_the_same_with_github_down` | `[GitHub Up, Jeff's page] expected a \`Linked identities\` region;` | MISSING_FUNCTIONALITY ✅ |
| VL-9 + VL-14 | `when_the_link_store_cannot_be_read_every_page_still_loads_and_says_so` | `[checks fail per request, /score?contributor=did:plc:test-jeff] expected a region saying \`Linked identities could not be read from your local store.\`;` | MISSING_FUNCTIONALITY ✅ |
| VL-13 | `the_results_swap_and_the_full_page_carry_the_same_linked_identities` | `[Jeff's page, full page] expected a \`Linked identities\` region;` | MISSING_FUNCTIONALITY ✅ |
| VL-G | `xtask::viewer_fail_seam::ungated_identity_links_seam_in_release_path_is_rejected` | `an ungated OPENLORE_VIEWER_FAIL_IDENTITY_LINKS read would ship a link-read backdoor in a release binary and MUST be flagged, got: []` | MISSING_FUNCTIONALITY ✅ (token not yet in `VIEWER_FAIL_SEAM_TOKENS`) |

Result: **0 BROKEN, 0 WRONG_ASSERTION.** Not yet reached (an earlier row fails first), each using
only shipped behaviour: VL-2's proof-gone rows (`identity verify --github rachelk` after a bio or
account change on the GitHub double), VL-9's `/search` rows and the VL-14 damaged-store row
(`DROP TABLE`, then `viewer.identity_links.probe`), VL-19's unconfigured-index row. The DELIVER step
that turns each scenario green must confirm its later rows reach their Thens.

**DELIVER note (review VL-H2):** step 04-07 must append `OPENLORE_VIEWER_FAIL_IDENTITY_LINKS` to
`VIEWER_FAIL_SEAM_TOKENS` in `xtask/src/check_arch.rs` and un-ignore VL-G in the same step that adds
the seam, gated behind `#[cfg(debug_assertions)]`.
