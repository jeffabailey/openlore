# Mutation testing — fix-indexer-follow-ups (DELIVER Phase 5)

Tool: cargo-mutants 25.3.1, `--timeout-multiplier 3 -j 2`. Gate: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL.
Kill rate = caught / (caught + missed + timeout). Unviable mutants (they do not compile) are left out.
Scope: only this delivery's changes are mutated (`--in-diff` against `0cfdd8e`). Each crate is measured with its own in-crate tests, as in the indexer-per-did-pds-fetch precedent.

## Summary

| Target | Viable | Caught | Missed | Timeout | Unviable | Kill rate | Gate |
|---|---|---|---|---|---|---|---|
| `claim-domain` (`distinct_references`) | 2 | 2 | 0 | 0 | 1 | **100%** | **PASS** |
| `appview-domain` (`ListingBudget`, `ListingSource::budget`, `plan_listing`) | 0 | 0 | 0 | 0 | 1 | n/a (no viable mutant) | **PASS** (see §2) |
| `openlore-indexer` (config.rs) | 9 | 9 | 0 | 0 | 2 | **100%** | **PASS** |
| `adapter-index-store` (advisory) | 2 | 2 | 0 | 0 | 0 | **100%** | **PASS** |

Each target had one full run over its in-diff scope. Nothing survived, so no new tests were added and no targeted `-F` re-runs were needed. No production file was touched.

## 1. `crates/claim-domain`

Command: `cargo mutants -p claim-domain --in-diff <(git diff 0cfdd8e..HEAD -- crates/claim-domain) --timeout-multiplier 3 -j 2`

| File:line | Mutant | Outcome |
|---|---|---|
| decode.rs:300 | `distinct_references` → `vec![]` | caught |
| decode.rs:303 | delete `!` in the first-occurrence filter | caught |
| decode.rs:300 | `distinct_references` → `vec![Default::default()]` | unviable (`ClaimReference` has no `Default`) |

The in-file property `indexed_references_have_no_duplicates_and_lose_none` kills both viable mutants.

## 2. `crates/appview-domain`

Command: `cargo mutants -p appview-domain --in-diff <(git diff 0cfdd8e..HEAD -- crates/appview-domain) --timeout-multiplier 3 -j 2`

| File:line | Mutant | Outcome |
|---|---|---|
| ingest_pass.rs:88 | `ListingSource::budget` → `Default::default()` | unviable (`E0277`: `ListingBudget: Default` is not satisfied; the log confirms this is a real compile error, not a scratch-build fault) |

cargo-mutants 25.3.1 produced only this one mutant for the diff. It does not swap match arms that have no wildcard, and the `plan_listing` change in this delivery is test-only. The gate is therefore vacuous. The behaviour is still pinned: the in-file property `a_resolved_did_is_listed_on_its_own_pds` asserts `RemainingOfShared` for the own PDS and `Fresh` for the fallback. Swapping or collapsing the arms would make it fail.

## 3. `crates/openlore-indexer` (config.rs)

Command: `cargo mutants -p openlore-indexer --in-diff <(git diff 0cfdd8e..HEAD -- crates/openlore-indexer/src/config.rs) --timeout-multiplier 3 -j 2`

| File:line | Mutant | Outcome |
|---|---|---|
| config.rs:239 | `source_url_admissible` → `true` / `false` | caught ×2 |
| config.rs:240, 242, 243:46 | `&&` → `\|\|` | caught ×3 |
| config.rs:243:13 | `\|\|` → `&&` | caught |
| config.rs:241 | delete `!` (query/fragment refusal) | caught |
| config.rs:261 | `plc_endpoint` → `Ok(String::new())` / `Ok("xyzzy".into())` | caught ×2 |
| config.rs:133 | `parse_config` → `Ok(Default::default())` | unviable (`IndexerConfig` has no `Default`) |
| config.rs:251 | `fallback_url` → `Ok(Default::default())` | unviable (`FallbackUrl` has no `Default`) |

The shared admissibility function, the PLC endpoint refusal and the blank-value refusal are fully covered. The covering tests are `a_plc_endpoint_is_accepted_iff_the_policy_admits_it`, the `pass_core_properties` env property and the existing fallback contracts.

## 4. Advisory: `crates/adapter-index-store`

Command: `cargo mutants -p adapter-index-store --in-diff <(git diff 0cfdd8e..HEAD -- crates/adapter-index-store) --timeout-multiplier 3 -j 2`

| File:line | Mutant | Outcome |
|---|---|---|
| lib.rs:300 | `IndexStorePort::upsert` → `Ok(())` | caught |
| lib.rs:354 | `replace_claim_rows` → `Ok(())` | caught |

The transactional upsert is covered. This run completed in about 4 minutes, including the DuckDB baseline build.

## Equivalent mutants

None. Every missed or uncovered case above is either caught or unviable.

## Post-run safety

cargo-mutants mutates a copy of the tree in a scratch directory. `git status --short` was clean after all four runs, apart from this report.
