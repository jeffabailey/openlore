# Contributor Philosophy Inference — Mutation Report (pure people core)

**Date:** 2026-09-28
**Tool:** cargo-mutants 25.3.1 (no `--in-place`; `--timeout 120 -j 2`, output outside the repo)
**Gate:** kill rate = (caught + timeout) / viable (unviable excluded). ≥ 80% PASS · 70–80% WARN · < 70% FAIL.
**Verdict:** **PASS — 98.4% (186/189) on the gated surfaces after the killing tests** (baseline 82.0%). The 3 remaining survivors are proven equivalent. With them excluded, the rate is 100% (186/186).

## Commands

```
cargo test -p scraper-domain -p claim-domain                                                     # pre-check: green
cargo mutants -p scraper-domain --file 'crates/scraper-domain/src/people/*.rs' --timeout 120 -j 2  # gated
cargo mutants -p claim-domain   --file crates/claim-domain/src/retraction.rs  --timeout 120 -j 2   # gated
cargo mutants -p cli --file crates/cli/src/verbs/sign_batch.rs \
    -F 'parse_confidence|parse_selection|selected_candidates' --timeout 300 -j 2 -- --lib        # secondary (baseline clean → gated)
cargo mutants -p adapter-github --file crates/adapter-github/src/lib.rs \
    -F classify_contributors --timeout 180 -j 2                                                  # secondary (report only)
```

adapter-duckdb, xtask and test-support were not mutated.

## Tally (per file)

Baseline is the run at `f9f1683`. "After" is the run with the new properties in `crates/scraper-domain/src/people/tests.rs`. There were no timeouts in any run.

| File | Total | Unviable | Viable | Caught (before → after) | Missed (before → after) | Kill rate (before → after) |
|---|---|---|---|---|---|---|
| `people/candidate.rs` | 15 | 7 | 8 | 6 → **8** | 2 → **0** | 75.0% → **100%** |
| `people/classify.rs` | 10 | 2 | 8 | 8 → 8 | 0 → 0 | 100% → 100% |
| `people/confidence.rs` | 16 | 2 | 14 | 1 → **14** | 13 → **0** | 7.1% → **100%** |
| `people/inference.rs` | 118 | 84 | 34 | 29 → **32** | 5 → **2** (equivalent) | 85.3% → **94.1%** |
| `people/mod.rs` | 5 | 0 | 5 | 3 → **5** | 2 → **0** | 60.0% → **100%** |
| `people/overlap.rs` | 10 | 1 | 9 | 9 → 9 | 0 → 0 | 100% → 100% |
| `people/provenance.rs` | 19 | 2 | 17 | 15 → **17** | 2 → **0** | 88.2% → **100%** |
| `people/report.rs` | 23 | 5 | 18 | 18 → 18 | 0 → 0 | 100% → 100% |
| `people/selection.rs` | 142 | 116 | 26 | 21 → **25** | 5 → **1** (equivalent) | 80.8% → **96.2%** |
| `people/subject.rs` | 14 | 1 | 13 | 9 → **13** | 4 → **0** | 69.2% → **100%** |
| `people/weakened.rs` | 12 | 3 | 9 | 8 → **9** | 1 → **0** | 88.9% → **100%** |
| **people/ subtotal** | 384 | 223 | 161 | 127 → **158** | 34 → **3** | 78.9% → **98.1%** |
| `claim-domain/src/retraction.rs` | 13 | 0 | 13 | 13 | 0 | **100%** |
| `cli/src/verbs/sign_batch.rs` (3 pure fns, `--lib`) | 17 | 2 | 15 | 15 | 0 | **100%** |
| `adapter-github/src/lib.rs` `classify_contributors` (report only) | 6 | 1 | 5 | 5 | 0 | **100%** |

**Gated kill rate** (people + retraction + cli `sign_batch`, since its baseline ran cleanly):

- Baseline: (127 + 13 + 15) / (161 + 13 + 15) = 155/189 = **82.0%**.
- After: (158 + 13 + 15) / 189 = 186/189 = **98.4%**. Excluding the 3 equivalent mutants, it is 186/186 = **100%**.

Without cli, the 1+2 core is 140/174 = 80.5% at baseline and 171/174 = 98.3% after. The people subtotal alone was 78.9% at baseline, which is WARN. confidence, mod and subject were below 70%, which is FAIL.

The high unviable counts in `selection.rs` and `inference.rs` come from `Default::default()` and collection-constructor replacements on types that have no `Default` impl. They do not compile, so they are excluded as the gate defines.

## Survivor analysis (gated surfaces)

Every one of the 34 baseline survivors is either killed by a new test (31) or proven equivalent (3).

### Killed — 31 mutants

| Mutant(s) | Gap | Killed by (`crates/scraper-domain/src/people/tests.rs`) |
|---|---|---|
| `selection.rs:89:5 is_bot -> true` | **Circular oracle.** `distinct_human_ids` filtered with `is_bot` itself. When every row counted as a bot, the oracle expected zero humans and the selection recorded zero, so the tests passed. | The oracle now uses `matches_bot_rule`, the DDD-3 rule text written independently of `is_bot`. The existing selection properties now kill it. |
| `selection.rs:117:52 < → >` in `collapse_by_user_id` | Nothing checked *which* duplicate row a person was recorded from. | `each_recorded_person_carries_their_accounts_best_row`: contributions equal the account's maximum, and the login is the smallest among those rows. |
| `selection.rs:68:38 > → >=` in `contributor_count_for` | The boundary N = 100 was rarely sampled from `0..=250`. | `exactly_one_page_of_contributors_is_accepted_and_one_more_is_refused` (boundary example). |
| `selection.rs:33:9 Display -> Ok(Default)` | The refusal text was never read. | `a_contributor_count_refusal_names_what_it_refused`: the message names the request, the limit, and the person target. |
| `confidence.rs:28` × 4 (`floor_of`: `* → / +`, `+ → - *`) | `floor_of` (FederatedRow → hundredths) was never exercised in-crate. Its epsilon was unpinned. | `a_confidence_round_trips_through_its_decimal_and_display`: `floor_of(h.as_decimal()) == h` for every 0..=100, including the 0.29 × 100 = 28.999… noise, and flooring of midpoints. |
| `confidence.rs:39` × 5 (`as_decimal -> 0.0 / 1.0 / -1.0`, `/ → * %`) | The decimal form was never checked. | Same property: the decimal lies in [0, 1] and rounds back to the value. |
| `confidence.rs:34:9 value -> 0` | **Circular oracle.** The DDD-9 property computed `expected` with `max.value()`, so the mutation collapsed both sides to 0. | Same property: `Hundredths::new(v).value() == min(v, 100)`. |
| `confidence.rs:45:9 Display -> Ok(Default)` | The `d.dd` display was never checked. | Same property: the display is 4 chars `d.dd` and parses back to the decimal. |
| `confidence.rs:59:5 confidence_arithmetic -> "" / "xyzzy"` (2) | The J-002c explanation was never checked in this crate. | `the_confidence_arithmetic_reads_as_the_ddd9_formula` (the documented worked example). |
| `candidate.rs:140:9 login -> "" / "xyzzy"`, `mod.rs:51:5 strip_github_prefix -> "" / "xyzzy"` (4) | **Circular oracle.** The provenance round-trip built its expected suffix from `candidate.login()`, and it only checked `starts_with`/`ends_with` on the commits URL. | `each_commits_url_names_the_repo_and_the_login`: `github:<login>() == person_subject`, and each commits URL is exactly `https://github.com/<owner/repo>/commits?author=<login>`, derived from the subjects rather than from `login()`. |
| `provenance.rs:74:46 && → \|\|`, `74:65 && → \|\|` (2) | Only well-formed AT-URIs were ever parsed. | `only_a_well_formed_claim_at_uri_is_cited`: an empty DID, an empty CID, and a CID with an extra path segment are all refused. |
| `subject.rs:31:28 guard -> true`, `45:5 is_github_login -> true`, `46:9 && → \|\|`, `47:9 && → \|\|` (4) | The existing D-8 property drew `other` from an alphabet that almost never yields the `github:` prefix. The body after `github:` was therefore never an invalid login. | `only_a_well_formed_login_follows_the_github_scheme`: `github:` + {empty, leading `-`, `_ . / space`} are refused and valid logins are accepted. |
| `inference.rs:149:50 < → >` in `collapse_links` | Arbitrary links all carried the same default timestamp, and nothing checked which duplicate link was kept. | `a_candidate_shows_its_preferred_links_rank_and_smallest_spelling`: over links re-observed across days, each supporting repo shows the rank and spelling of the DDD-10 preferred link (latest observation, then best rank, then smallest spelling). |
| `inference.rs:198:36 < → ==`, `< → >` in `group_support` | Nothing checked which spelling names the person. | Same property: the candidate's person subject is the smallest spelling among its preferred links. |
| `weakened.rs:50:81 == → !=` | The weakened property ran only with the default filter, so person scoping was never exercised. | `a_person_scoped_report_flags_only_that_persons_weakened_claims`: the scoped `weakened` list equals the unscoped one filtered to that person, in the same order. |

### Equivalent — 3 mutants

| Mutant | Why it is equivalent |
|---|---|
| `selection.rs:117:52 < → <=` in `collapse_by_user_id` | `contribution_order_key` is `(Reverse(contributions), login.lower, login, user_id, account_type)`. That covers **every** field of `RawContributor`. A tie (`==`) therefore means the two rows are identical, and replacing the kept row with an equal one changes nothing. |
| `inference.rs:149:50 < → <=` in `collapse_links` | On a tie of `link_preference = (Reverse(last_observed micros), rank, repo_subject, person_subject)`, the replacement link has the same `repo_subject`, `person_subject` and `rank`. Those are the only fields `group_support` reads from a collapsed link, and the observation time is not read after collapse. The output is identical. |
| `inference.rs:198:36 < → <=` in `group_support` | `if link.person_subject <= group.person_subject { group.person_subject = link.person_subject.clone() }` differs from `<` only when the strings are equal, and then it assigns an equal string. |

### Properties the adversarial review flagged

- `possible_renames_are_mutual` and `an_unchanged_inference_reports_no_new_candidates` are weak alone: one is symmetric by construction, and the other compares a report with itself. Neither area has survivors, though. `overlap.rs` (9/9) and `report.rs` (18/18) are fully killed by the oracle-based properties next to them: `possible_renames_flags_exactly_the_logins_sharing_a_user_id` and `only_candidates_absent_before_are_counted_as_new`. No strengthening was needed for the gate.
- The review's broader concern was properties that "pass against stubs." That concern is confirmed and fixed in three places where the oracle reused the function under test: `is_bot` in `distinct_human_ids`, `Hundredths::value` in the DDD-9 expected value, and `candidate.login()` in the commits-URL suffix.

## Secondary surfaces

- **cli `verbs/sign_batch.rs`** (`parse_confidence`, `parse_selection`, `selected_candidates`): 17 mutants, 15 caught, 2 unviable, **100%**. With the test run limited to the lib (`-- --lib`), the unmutated baseline passed (77.6 s build + 1.5 s test). This avoids the 21 `appview_search` acceptance tests that fail in cargo-mutants' copied tree. Because the baseline ran cleanly, this surface is counted in the gated rate.
- **adapter-github `classify_contributors`** (report only): 6 mutants, 5 caught, 1 unviable, **100%**.

## Conclusion

- **people/**: 78.9% → **98.1%** (158/161). 8 properties and 2 examples were added, and 1 circular oracle was fixed. The 3 survivors are equivalent.
- **claim-domain `retraction.rs`**: **100%** (13/13). No change.
- **cli `sign_batch.rs`**: **100%** (15/15). The lib-only baseline is clean.
- **Gated overall:** 82.0% → **98.4%** (186/189), or 100% with the equivalents excluded. **PASS** against the 80% gate.
- **adapter-github `classify_contributors`** (report only): **100%** (5/5).
- No production code changed. No `mutants.out*` artifacts are in the repo.
