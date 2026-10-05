# Mutation testing — indexer-per-did-pds-fetch (DELIVER Phase 5)

Tool: cargo-mutants 25.3.1, `--timeout-multiplier 3 -j 2`. Gate: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL.
Kill rate = caught / (caught + missed + timeout). Unviable mutants (they do not compile) are left out.
Scope: only this feature's changes are mutated (`--in-diff` against `3c7a56b`). Each crate is measured with its own in-crate tests, as in the bluesky-claim-review-app precedent. `-p cli` is not a target.

## Summary

| Target | Run | Viable | Caught | Missed | Timeout | Unviable | Kill rate | Gate |
|---|---|---|---|---|---|---|---|---|
| `appview-domain` (ingest_pass.rs) | initial | 34 | 13 | 21 | 0 | 10 | 38.2% | FAIL |
| `appview-domain` (ingest_pass.rs) | after new tests | 34 | 34 | 0 | 0 | 10 | **100%** | **PASS** |
| `ports` (net_policy.rs and other port diffs) | initial | 43 | 19 | 24 | 0 | 1 | 44.2% | FAIL |
| `ports` (net_policy.rs and other port diffs) | after new tests | 43 | 43 | 0 | 0 | 1 | **100%** | **PASS** |
| `openlore-indexer` (config.rs) | initial | 34 | 20 | 14 | 0 | 6 | 58.8% | FAIL |
| `openlore-indexer` (config.rs) | after new tests | 34 | 34 | 0 | 0 | 6 | **100%** | **PASS** |

Both runs were full runs over each in-diff scope; no targeted `-F` re-runs were needed.
The security-critical code in net_policy (address ranges, IPv4-mapped unwrap, https-only, userinfo) is at 100%. No survivor needed an equivalence argument.
No production behaviour changed. The only production-file edit appends a `#[cfg(test)] mod contracts;` declaration to config.rs. The indexer is a binary-only crate, so its tests must live inside it.

New tests (commit `4566a0c`):
- `crates/appview-domain/tests/ingest_pass_contracts.rs`: exit codes 0/3 and the outage rule (examples plus a property over own/fallback/skipped counts); the `SkipReason` and `RefusalCause` tokens; trimming of the `PdsEndpoint`/`FallbackUrl` base URLs and `ListingSource::base`; `fallback_used` for every `FetchFailure` on each source.
- `crates/ports/tests/net_policy_contracts.rs`: every refused range at its edges, including IPv4-mapped forms; public addresses just outside each range; `is_loopback`; under the test seam, loopback is the only refused address admitted; `admitted_addresses` keeps DNS order; URLs with a username and/or password are refused; IP-literal https; http only to loopback under the seam; policy tokens.
- `crates/openlore-indexer/src/config_contracts.rs`: the DID length edge (2048 loads, 2049 does not); each malformed-DID shape reports "not well-formed" rather than "names no repo"; valid identifier characters; the `ConfigError` Display format; the default index path with and without `OPENLORE_HOME`; under the seam, the fallback admits http to loopback but not https to a refused address; a query or fragment in the fallback is refused.

## 1. `crates/appview-domain`

Command: `cargo mutants -p appview-domain --in-diff <(git diff 3c7a56b..HEAD -- crates/appview-domain) --timeout-multiplier 3 -j 2`

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| ingest_pass.rs:390–391 `pass_exit_code` (→0/1/-1; `+`→`-`/`*`; `>=`→`<`; `&&`→`\|\|`; `==`→`!=`) | 8 | killed-by-new-test |
| ingest_pass.rs:95 `SkipReason::token`, :269 `RefusalCause::token` (→""/"xyzzy") | 4 | killed-by-new-test |
| ingest_pass.rs:21 `base_url`, :34 `PdsEndpoint::as_str`, :51 `FallbackUrl::as_str`, :75 `ListingSource::base` | 7 | killed-by-new-test |
| ingest_pass.rs:137 `ClassifiedSkip::fallback_used` (→true/false) | 2 | killed-by-new-test |

## 2. `crates/ports`

Command: `cargo mutants -p ports --in-diff <(git diff 3c7a56b..HEAD -- crates/ports) --timeout-multiplier 3 -j 2`

Why the first run reached only 44.2%: the existing in-file properties compare `admitted_addresses` and `url_admissible` against `address_refused` and `is_loopback`. Those properties used the production functions as their own oracle, so a mutated range predicate went unnoticed. The new tests check concrete addresses instead.

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| net_policy.rs:32 `address_refused` (→true/false) | 2 | killed-by-new-test |
| net_policy.rs:42 `is_loopback` (→true/false) | 2 | killed-by-new-test |
| net_policy.rs:90 `v4_refused` (→true/false; `==`→`!=`; `\|\|`→`&&` ×3) | 6 | killed-by-new-test |
| net_policy.rs:94–98 `v6_refused` (→true/false; `\|\|`→`&&` ×3; `&`→`\|`/`^` ×4; `==`→`!=` ×2) | 11 | killed-by-new-test |
| net_policy.rs:84 `url_admissible` userinfo `&&`→`\|\|` | 1 | killed-by-new-test |
| net_policy.rs:21 `TransportPolicy::token` (→""/"xyzzy") | 2 | killed-by-new-test |

## 3. `crates/openlore-indexer` (config.rs)

Command: `cargo mutants -p openlore-indexer --in-diff <(git diff 3c7a56b..HEAD -- crates/openlore-indexer/src/config.rs) --timeout-multiplier 3 -j 2`

| File:line | Missed mutants (initial) | Disposition |
|---|---|---|
| config.rs:112 `ConfigError` Display → `Ok(Default)` | 1 | killed-by-new-test |
| config.rs:208 `>`→`>=`/`==` (MAX_DID_LENGTH) | 2 | killed-by-new-test |
| config.rs:209–211 `repo_did` `\|\|`→`&&` ×3 | 3 | killed-by-new-test |
| config.rs:223–225 `did_identifier_valid` (→true; `&&`→`\|\|` ×2) | 3 | killed-by-new-test |
| config.rs:234–238 `fallback_url` (`==`→`!=`; `&&`→`\|\|` ×3) | 4 | killed-by-new-test |
| config.rs:279 `default_index_path` → `Default` | 1 | killed-by-new-test |

## Advisory targets

These were not run: `adapter-index-query` (`decode_wire_provenance`) and `adapter-index-store` (contributor `starts_with`). They are advisory, and the DuckDB adapter builds are slow on this host. They are left for the CI nightly run.

## Post-run safety

cargo-mutants mutates a copy of the tree. `git status --short` was clean after the runs, apart from the new test files, which are now committed.
