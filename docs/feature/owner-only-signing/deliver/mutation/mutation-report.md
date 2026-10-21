# Owner-Only Signing — Mutation Report (feature-changed logic)

**Date:** 2026-10-21
**Tool:** cargo-mutants 25.3.1 (no `--in-place`; `--timeout 180 -j 2`; output dirs in the session scratchpad, never in the repo)
**Scope:** only the lines this feature changed. Each surface runs with `--in-diff`, using the diff from the feature base `6fc64b9` to `cf9994f`.
**Gate:** kill rate = (caught + timeout) / viable, with unviable mutants excluded. ≥ 80% PASS · 70–80% WARN · < 70% FAIL.
**Verdict:** **PASS — 100% (176/176) on the gated surfaces after the killing tests.** The baseline was 76.1% (134/176, WARN), and `claim-domain/src/authority.rs` alone was 72.6% (WARN).

## Commands

```
# scope: the feature diff only
git diff 6fc64b9..HEAD -- <surface files> > <scratch>/<surface>.diff

cargo mutants -p claim-domain   --file crates/claim-domain/src/authority.rs \
  --in-diff authority.diff --timeout 180 -j 2 -o <scratch>/authority                       # gated
cargo mutants -p scoring        --file crates/scoring/src/score.rs --file crates/scoring/src/explain.rs \
  --in-diff scoring.diff --timeout 180 -j 2 -o <scratch>/scoring                           # gated
cargo mutants -p ports          --file crates/ports/src/graph.rs --file crates/ports/src/imported_candidate.rs \
  --in-diff ports.diff --timeout 180 -j 2 -o <scratch>/ports                               # gated
cargo mutants -p adapter-duckdb --file crates/adapter-duckdb/src/imported_candidates.rs \
  --file crates/adapter-duckdb/src/artifact_paths.rs --file crates/adapter-duckdb/src/repo_authority_checks.rs \
  --file crates/adapter-duckdb/src/schema_v9.rs \
  --in-diff adapter-duckdb.diff --timeout 180 -j 2 -o <scratch>/duckdb                     # gated
cargo mutants -p review-domain  --file crates/review-domain/src/share.rs \
  --in-diff review-domain.diff --timeout 180 -j 2 -o <scratch>/review                      # gated
cargo mutants -p adapter-github --file crates/adapter-github/src/client.rs --file crates/adapter-github/src/lib.rs \
  --in-diff adapter-github.diff --timeout 180 -j 2 -o <scratch>/github                     # gated
cargo mutants -p xtask          --file xtask/src/check_arch.rs --in-diff xtask.diff ...     # optional (baseline timed out)
cargo mutants -p cli            --file 'crates/cli/src/render/*.rs' --in-diff cli-render.diff ...  # optional (baseline failed)
```

**After the killing tests:** the same commands were re-run.

- For `authority.rs`, `score.rs` and `explain.rs`, the diff was regenerated as `git diff 6fc64b9 -- <files>`. The new `#[cfg(test)]` code shifted the old diff's trailing context, so cargo-mutants refused the old diff.
- `--list` produced the identical set of mutants: 155 for authority and 16 for scoring.

## Tally (per file)

| File | Total | Caught | Timeout | Missed | Unviable | Viable | Kill rate |
|---|---|---|---|---|---|---|---|
| `claim-domain/src/authority.rs` (baseline) | 155 | 69 | 0 | 26 | 60 | 95 | 72.6% (WARN) |
| `claim-domain/src/authority.rs` (after) | 155 | **95** | 0 | **0** | 60 | 95 | **100%** |
| `scoring/src/score.rs` (baseline) | 4 | 0 | 0 | 4 | 0 | 4 | 0% |
| `scoring/src/explain.rs` (baseline) | 12 | 9 | 0 | 1 | 2 | 10 | 90.0% |
| `scoring/src/{score,explain}.rs` (after) | 16 | **14** | 0 | **0** | 2 | 14 | **100%** |
| `ports/src/graph.rs` (baseline) | 10 | 0 | 0 | 4 | 6 | 4 | 0% in the scoped run |
| `ports/src/imported_candidate.rs` | 2 | 0 | 0 | 0 | 2 | 0 | — (no viable mutant) |
| `ports/src/graph.rs` (after) | 10 | **4** | 0 | **0** | 6 | 4 | **100%** |
| `adapter-duckdb/src/artifact_paths.rs` | 18 | 18 | 0 | 0 | 0 | 18 | **100%** |
| `adapter-duckdb/src/imported_candidates.rs` | 19 | 7 | 0 | 0 | 12 | 7 | **100%** |
| `adapter-duckdb/src/schema_v9.rs` | 6 | 6 | 0 | 0 | 0 | 6 | **100%** |
| `adapter-duckdb/src/repo_authority_checks.rs` (baseline) | 40 | 17 | 0 | 3 | 20 | 20 | 85.0% |
| `adapter-duckdb/src/repo_authority_checks.rs` (after) | 40 | **20** | 0 | **0** | 20 | 20 | **100%** |
| `review-domain/src/share.rs` (baseline) | 8 | 0 | 0 | 4 | 4 | 4 | 0% |
| `review-domain/src/share.rs` (after) | 8 | **4** | 0 | **0** | 4 | 4 | **100%** |
| `adapter-github/src/client.rs` | 8 | 5 | 0 | 0 | 3 | 5 | **100%** |
| `adapter-github/src/lib.rs` | 5 | 3 | 0 | 0 | 2 | 3 | **100%** |
| `xtask/src/check_arch.rs` (optional) | 109 | — | — | — | — | — | not measured (baseline timed out) |
| `cli/src/render/*.rs` (optional) | 83 | — | — | — | — | — | not measured (baseline failed) |

**Gated kill rate:**

- **Baseline:** (69 + 9 + 0 + 48 + 0 + 8) / (95 + 14 + 4 + 51 + 4 + 8) = 134/176 = **76.1% (WARN)**.
- **After the killing tests:** 176/176 = **100%**.

No mutant timed out on any gated surface.

## Survivor analysis

All 37 survivors were real gaps, and none was equivalent. Many of them, such as the repo-tier classifier, `score_attested`, `index_rows_authority` and `quote_permitted`, are exercised by the workspace acceptance suite and the cli/viewer crates. A per-crate run cannot see those tests, so each crate now pins its own contract.

### claim-domain `authority.rs` — 26 MISSED, all killed

The killing tests are in `crates/claim-domain/src/authority.rs`, in the `gate_from_checks_tests` and `authority_file_tests` modules.

| Mutant(s) | Gap | Killed by |
|---|---|---|
| `owner_link` `954:39 == → !=`, `956:28 guard → true / false`, `956:42 == → !=`; `lists_author → true / false`; `found_repo_authority` `979:33 == → !=`, `981:19 && → \|\|`, `981:27 == → !=` (9) | No in-crate test classified a **repo** claim. The repo tier was only reached through `may_sign_repo_as_served` with a verified link to a user-owned repo. | `a_reader_labels_a_repo_claim_from_the_latest_observation_and_the_authors_links`. One row per tier and reason: owner, a verified link to *another* account, an unverified link, an unchecked link, listed despite an unverified link, an org repo with a verified owner link, org listed, org listing other DIDs (including the prefix `did:plc:jeff2`), an invalid file, not found, and never observed. |
| `is_github_login → true`, `358:9 / 359:9 / 360:9 && → \|\|`; `is_github_repo_name → true`, `367:9 / 370:9 / 371:9 && → \|\|`; `github_requirement` guards `342:20 → true`, `346:26 → true`, `346:49 && → \|\|` (11) | Nothing pinned GitHub's login and repo-name syntax. Only `github:a/b/c` was shown to be malformed. | `a_github_subject_names_an_account_or_repo_only_in_githubs_own_syntax`. Login edges: empty, leading or trailing hyphen, `_` and `.`, 39 versus 40 characters. Repo-name edges: empty, `.`, `..`, `$`, space, 100 versus 101 characters, and a bad owner with a good name. |
| `602:27 guard within_signing_window(...) → true` in `may_sign_repo_from_checks` | The window was tested for the person tier only. | `a_repo_permit_rests_on_an_observation_within_the_signing_window`. A listed DID at exactly 7 days is permitted. At 7 days plus 1 minute, it gets `Recheck { last_checked: Some(observation) }`. |
| `742:9 delete match arm Found{Organization}` in `repo_can_sign` | The org-repo hand-off was never checked. | `only_a_listed_did_can_sign_about_an_organization_repo`. The refusal is `OrgOwnedNotListed` with `can_sign: ListedInAuthorityFile`. |
| `771:5 republish_permit → None` | No in-crate test. | `only_a_claim_already_published_here_gets_a_republish_permit`. A published claim gets an Exempt permit for its own signer and subject, and a never-published claim gets none. |
| `854:9 Authority::label → "" / "xyzzy"` (2) | The reader labels were spelled only through the cli renderers. | `each_authority_reads_under_its_own_label`. It pins the four ADR-086 §8 spellings as literals. |
| `213:47 * → +` in `AUTHORITY_FILE_MAX_BYTES` | The size test is relative to the constant, so changing the constant changes the test with it. | `a_four_kibibyte_file_still_grants`. A literal 4096-byte file still lists its DID (ADR-087 §3). |

### scoring — 5 MISSED, all killed

| Mutant(s) | Gap | Killed by |
|---|---|---|
| `score_attested → Default::default()`; `is_weight_eligible → true / false`; `175:50 == → !=` (4) | `score_attested` had no test in the crate. | `score_attested_tests::only_attested_github_assertions_and_assertions_needing_no_authority_carry_weight` (`score.rs`). The feed holds one row of each kind: attested, third-party, unchecked, an exempt assertion, a dispute, and a malformed subject. The dispute and the malformed row read *attested*, so only their kind and subject keep them out. The result equals `score` of the attested row plus the exempt assertion. |
| `explain.rs:79:5 needs_authority → true` | No test fed a non-`github:` row to `left_out` while the reader's view still returned an authority for it. | `a_claim_needing_no_authority_is_never_left_out` (`explain.rs`). |

### ports `graph.rs` — 4 MISSED, all killed

| Mutant(s) | Gap | Killed by (`crates/ports/src/graph.rs`, `feed_authority_tests`) |
|---|---|---|
| `index_rows_authority → Ok(vec![]) / Ok(vec![None])`; `FeedAuthority::of → None` (3) | The tests that catch these live in the cli and the viewer, and the scoped run does not see them. | `each_index_row_is_judged_by_its_position_and_bare_author_did`. A pure stub implements `FeedAuthorityReadPort`; it is a port double, not a mock library. Two rows share a CID and get different labels, because rows are keyed by position. A `#org.openlore.application` author is judged by its bare DID, and an exempt row gets `None`. |
| `FeedAuthority::into_by_cid → BTreeMap::new()` | Same as above: only the cli and viewer tests catch it. | `a_feed_authority_hands_back_every_judgment_by_cid` |

### review-domain `share.rs` — 4 MISSED, all killed

| Mutant(s) | Gap | Killed by (`crates/review-domain/src/share.rs`, `tests::quote_permitted_tests`) |
|---|---|---|
| `ShareQuote::quoted / not_counted → empty`; `229:44 == → !=` in `quote_permitted` (3) | The share split was tested only through the review app's routes. | `only_a_claim_a_permit_of_this_request_covers_is_quoted`. The permit is minted by ADR-089 `may_sign`. The covered claim is quoted, the other is listed as not counted, and the plan names only the quoted claim. |
| `212:35 && → \|\|` in `ShareQuote::plan` | Not tested in the crate. | `the_share_is_offered_while_any_claim_is_live_whether_quoted_or_not`. The share is offered when none or all of the claims are quoted. It is refused with `NothingPublished` only when no claim is live. |

### adapter-duckdb `repo_authority_checks.rs` — 3 MISSED, all killed (real DuckDB)

| Mutant(s) | Gap | Killed by (`crates/adapter-duckdb/tests/repo_authority_checks_probe.rs`) |
|---|---|---|
| `latest_repo_observations → Ok(BTreeMap::new())`; `served_repo_observation → Ok(None)` (2) | The decoders for the pure gate and classifier were exercised only through the cli. | `the_gate_and_the_classifier_read_each_repos_latest_observation_as_github_served_it`. It records into a real DuckDB file and reads back the latest observation of each subject. It checks the name GitHub served (`acme/Tool`), `served_as: None` for a repo that was not found, and that a never-seen subject is absent. |
| `107:9 delete match arm "absent"` in `authority_file_from_columns` | No adapter test read back an observation with an absent file through the decoded port. | The same test: the `AuthorityFile::Absent` observation reads back as `AuthorityFileSeen::Absent`. |

### Surfaces with no survivors

- **`adapter-github`:** the authority-file decode in `client.rs` and `lib.rs` had 8/8 viable mutants caught by `github_authority_contract.rs`. No test added.
- **`adapter-duckdb`:** `artifact_paths.rs`, `imported_candidates.rs` and `schema_v9.rs` had all viable mutants caught.
- **`ports/imported_candidate.rs`:** both mutants were unviable.

## Not measured (optional, not gated)

- **`xtask/src/check_arch.rs`** (109 mutants in the feature diff). cargo-mutants reported `TIMEOUT Unmutated baseline in 4.8s build + 180.1s test`: the unmutated `-p xtask` test run did not finish within 180 s on this heavily loaded host. A longer timeout for 109 mutants would take hours here, so no mutants were tested. The feature rules are covered by `xtask/tests/owner_only_signing_architecture.rs`.
- **`crates/cli/src/render/*.rs`** (83 mutants). cargo-mutants reported `cargo test failed in an unmutated tree`. In the scratch copy, 21 of 27 tests in `-p cli --test appview_search` fail on the baseline. This is the same environment-dependent failure recorded in the serverless-philosophy-federation report, and it is unrelated to this feature. The renderers are exercised by the workspace acceptance suite, which is a separate package from `-p cli`.

## Gates after the tests

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test -j 4 -p claim-domain -p scoring -p ports -p review-domain -p adapter-duckdb`: all green. No DuckDB lock flake, so no `-j 2` re-run was needed.
- `git status` after every run: no mutated source files and no `mutants.out*` in the repo.

## Conclusion

- **claim-domain `authority.rs`:** 72.6% → **100%** (95/95). 7 tests added.
- **scoring:** 64.3% (9/14) → **100%** (14/14). 2 tests added.
- **ports `graph.rs`:** 0% scoped → **100%** (4/4). 2 tests added.
- **adapter-duckdb:** 94.1% (48/51) → **100%** (51/51). 1 real-DuckDB test added.
- **review-domain `share.rs`:** 0% → **100%** (4/4). 2 tests added.
- **adapter-github:** **100%** (8/8). No change.
- **Gated overall:** 76.1% → **100%** (176/176). **PASS** against the 80% gate.
- No equivalent mutants and no production behaviour changed. No survivor revealed a production bug.
