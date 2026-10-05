# Mutation testing — bluesky-claim-review-app (DELIVER Phase 5)

Tool: cargo-mutants 25.3.1, `--timeout-multiplier 3 -j 2`. Gate: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL.
Kill rate = caught / (caught + missed + timeout). Unviable mutants (they do not compile) are left out.
Scope follows htmx-scraper-viewer DV-2: each crate is tested with its own unit, property and integration tests. `-p cli` is not a target (known tooling caveat: cargo-mutants cannot baseline it).

## Summary

| Target | Run | Viable | Caught | Missed | Timeout | Unviable | Kill rate | Gate |
|---|---|---|---|---|---|---|---|---|
| `review-domain` (whole crate) | initial | 492 | 356 | 136 | 0 | 52 | 72.4% | WARN |
| `review-domain` (whole crate) | after new tests | 492 | 492 | 0 | 0 | 52 | **100%** | **PASS** |
| `claim-domain` (feature diff) | initial | 31 | 3 | 28 | 0 | 13 | 9.7% | FAIL |
| `claim-domain` (feature diff) | after new tests | 31 | 30 | 1 (equivalent) | 0 | 13 | **96.8%** (100% without the equivalent mutant) | **PASS** |
| `adapter-review-store` (feature diff, advisory) | single | 157 | 144 | 13 | 0 | 29 | 91.7% | PASS (advisory) |

How the `review-domain` final figure was reached: the full re-run after the new tests gave 489 caught and 3 missed. Two more assertions were then added. A targeted re-run (`-F 'plans.rs:362' -F 'queue.rs:129'`) shows those 3 mutants caught as well.

## 1. `crates/review-domain`

Command: `cargo mutants -p review-domain --timeout-multiplier 3 -j 2`

Why the first run reached only 72.4%: the in-crate tests cover the business branches well. Outside the views, 329 of 386 viable mutants were caught (85.2%). The page renderers (`src/views/*`) are exercised end-to-end by the review-app acceptance tests. Those live in another package, so they are not part of this crate's mutation scope. Inside the crate, 79 of 106 view mutants survived. The new tests check behaviour at the crate's public API. No production code changed.

New tests (commit `4b19b24`):
- `crates/review-domain/tests/domain_contracts.rs` (21 tests):
  - budget window boundary and pruning of a day-old scan
  - `.5` and a signed fraction in confidence parsing
  - the vocabulary
  - the KPI catalogue names and their sums; the cancelled sign-in counts as denied; detection of an edited approval
  - rollup timing
  - DID-shaped tokens, the refusal labels and which refusals disprove ownership
  - plan owner and AT-URI; read-back of the record
  - RFC 3339 output on the civil calendar, including era and leap-day boundaries; 30-minute plan expiry
  - DID profile segments
  - the live published claim
  - reconcile counts
  - the 300-grapheme post limit, the draft's philosophy slugs and the refusal messages
- `crates/review-domain/tests/page_contracts.rs` (18 tests): each page is a whole document that shows what it was given:
  - the CSRF field and plan id on every form
  - reasons, AT-URIs and the links back to the queue or profile
  - headlines with the repo link and philosophy slug
  - confidence buckets
  - `HH:MM` UTC time
  - scan action, summary and notice for each scan status
  - card actions, Undo after a decline, and the edit form's options

Every mutant missed in the initial run was killed by the new tests:

| File | Missed mutants (initial) | Disposition |
|---|---|---|
| budget.rs:18:29 `<`→`<=` (budget_allows) | 1 | killed-by-new-test (`an_event_exactly_one_window_old_no_longer_counts`) |
| budget.rs:64:46 `<`→`<=` (admit_scan) | 1 | killed-by-new-test (`a_scan_a_full_day_old_is_forgotten`) |
| edits.rs:61, 70 guards; 85 `vocabulary` ×3 | 5 | killed-by-new-test |
| kpi.rs:52 `name` ×2, 70 `of_name`, 77 Cancelled arm, 85–86 `approval_was_edited` ×2, 114 `utc_day` ×2, 120 `sum_counters`, 128/131 const arithmetic ×4 | 13 | killed-by-new-test |
| ownership.rs:20, 25, 26, 27 `is_did_shaped`; 102 `disproves_ownership`; 115 `label` ×2 | 7 | killed-by-new-test |
| plans.rs:57/187 `owner_did` ×4, 80/210/321 `at_uri` ×6, 327 `read_back_matches`, 352/354/356/362 `civil_from_days` ×5 | 16 | killed-by-new-test |
| plans.rs:362:35 `PLAN_TTL_SECS` `*`→`+`, `*`→`/` | 2 | killed-by-new-test (`a_preview_stays_confirmable_for_thirty_minutes`; targeted re-run) |
| published.rs:24, 27 `profile_subject` ×5; 71, 73 `live_published_claim` ×2 | 7 | killed-by-new-test |
| reconcile.rs:24 `counts` | 1 | killed-by-new-test |
| share.rs:57 `message` ×2, 108 `>`→`>=`, 206 `philosophy_slug` ×2, 222 `-`→`+` | 6 | killed-by-new-test |
| views/mod.rs: csrf_field, plan_form, back_to_queue, back_to_profile, repo_path ×2, philosophy_slug ×2, claim_headline, suggestion_headline, bucket_label ×4, confidence_with_bucket, not_found_page ×2 | 17 | killed-by-new-test |
| views/github.rs ×4, landing.rs ×4, profile.rs ×2, publish.rs ×8, retract.rs ×6, settings.rs ×6, share.rs ×8 (page bodies and messages) | 38 | killed-by-new-test |
| views/queue.rs: scan_refused_message ×2, utc_clock ×6, scan_action ×3, completed_summary ×4, key_fields, card_action, suggestion_card, edit_page ×2, declined_notice, scan_status_page ×2 | 23 | killed-by-new-test |
| views/queue.rs:129:34 guard `pending.is_empty()`→`true` (scan_notice) | 1 | killed-by-new-test (`a_queue_with_cards_on_screen_never_says_all_were_reviewed`; targeted re-run) |

Total: 136 missed, all 136 killed. Remaining survivors: none.

## 2. `crates/claim-domain` (this feature's diff only)

Command: `cargo mutants -p claim-domain --in-diff <(git diff 012c26e..HEAD -- crates/claim-domain) --timeout-multiplier 3 -j 2`

Why the first run reached only 9.7%: ADR-071's shared `decode_claim_record` and `RecordOrigin::of` were tested only from consumer crates (ingest, peer read, review-domain). In-crate tests covered only the multibase key decode and the provenance verdict table.

New tests (commit `683abd2`), in `crates/claim-domain/tests/decode_claim_record.rs` (9 tests):
- a property: a `signature.sig` round-trips any byte string of up to 96 bytes through a reference base64url-no-pad encoder
- a property: every decoded field comes out as it was written
- a record without a signature decodes as self-attested
- every required field must be present and a string
- references are decoded in order with their type; an unknown reference type is refused
- characters outside the base64url alphabet are refused
- the `kid` falls back to the default only when the record has none
- `AuthorPds` requires an exact base-URL match; a trailing slash is ignored

| Missed mutants (initial) | Disposition |
|---|---|
| decode.rs:304–323 `base64url_decode` / `sextet` (22 mutants) | killed-by-new-test (21); 1 equivalent (below) |
| decode.rs:264 `decode_references` → `Ok(vec![])` | killed-by-new-test |
| decode.rs:294 `required_str` ×2 | killed-by-new-test |
| provenance.rs:79:31 `==`→`!=` in `RecordOrigin::of` | killed-by-new-test |
| **decode.rs:318:30 `\|`→`^` in `base64url_decode`** | **equivalent**: `acc << 6` always has its low 6 bits clear, and `sextet` returns at most 63, so the two operands never share a set bit. On disjoint bits, `a \| b == a ^ b`. |

## 3. `crates/adapter-review-store` (advisory, not gating)

Command: `cargo mutants -p adapter-review-store --in-diff <(git diff 012c26e..HEAD -- crates/adapter-review-store/src) --timeout-multiplier 3 -j 2` (1h 8m; each test run against DuckDB takes about 50 s).

Result: 144 caught, 13 missed, 29 unviable, so 91.7%. No tests were added because this target does not gate. The survivors are accepted and recorded here for a later hardening pass:

| Mutant | Disposition |
|---|---|
| secrets.rs:114 `auth_request_aad` → `vec![]` / `vec![0]` / `vec![1]` (3) | accepted. A seal/unseal round-trip passes with any fixed AAD. A test that moves a sealed blob to a different `state` row and expects the unseal to be refused would kill these. |
| secrets.rs:118 `oauth_session_aad` → `vec![]` / `vec![0]` / `vec![1]` (3) | accepted. Same gap, for the owner DID. |
| schema.rs:138:20 guard → `true`; 138:22 `>`→`>=` (2) | accepted. No test records a version older than the current one, which should be `Unknown`, not `TooNew`. The `>=` mutant is only reachable when the version equals the current one, and the `==` arm catches that first, so it is equivalent. |
| lib.rs:125:34 `\|\|`→`&&`; 125:48 `<`→`==` / `<=` in `unseal_with` (3) | accepted. Defence-in-depth checks: a wrong key still fails AEAD with `Seal`. Only a truncated blob (shorter than the nonce) can tell them apart. |
| lib.rs:149 `StoreError` `Display` → `Ok` (1) | accepted. Operator-facing log text that no test asserts. |
| probe.rs:106 `canary_counts_rolled_back` → `Ok((1, 0))` (1) | accepted. The mutant returns exactly the "isolated" answer the probe expects, so the probe cannot fail against a correct store. Killing it would need a store that is broken on purpose. |

## Hygiene

- cargo-mutants mutates a copied tree. After all runs, `git status --short` showed no changes to source files. Only the new test files and this report were added, each committed by explicit path.
- `cargo test -p review-domain -p claim-domain`: all green. `cargo clippy --tests -D warnings` and `cargo fmt` are clean for both crates.
- No production code was changed.
