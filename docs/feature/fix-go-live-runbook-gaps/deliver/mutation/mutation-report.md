# Mutation report: fix-go-live-runbook-gaps

**Gate:** per-feature, kill rate >= 80%. **Result: PASS, 100% (4/4 viable mutants caught).**

## Scope

Production Rust changed by 57e0d19 (`git diff f2ba683 57e0d19`), restricted to the changed lines (`--in-diff`):

- `crates/openlore-review-app/src/config.rs`: `scan_concurrency`, `parse_config` (new `scan_concurrency` field)
- `crates/openlore-review-app/src/limiter.rs`: `ScanLimiter::new`, `admit`, `release`
- `crates/openlore-review-app/src/wiring.rs`: `app` (limiter built from the configured concurrency)
- `crates/review-domain/src/budget.rs`: `admit_scan` (`max_running` replaces `CONCURRENT_SCANS`)

Shell scripts, docs and the xtask drift test are out of scope.

## Command

```
git diff f2ba683 57e0d19 -- crates/openlore-review-app/src crates/review-domain/src > diff.patch
cargo mutants --in-diff diff.patch -j 2 --timeout 900
```

No `CARGO` wrapper was needed: the review-app acceptance suites (including `review_app_resource_caps`, which spawns the binary and checks the startup refusal) are test targets of the `openlore-review-app` package itself, so cargo-mutants ran them against every mutated binary. Test set per mutant: `openlore-review-app` and `review-domain` unit tests plus all review-app acceptance targets (220 tests; `the_review_app_refuses_a_scan_concurrency_outside_its_range` ran in the baseline). Baseline: 64s build + 17s test, green.

## Results

| Outcome | Count |
|---|---|
| Caught | 4 |
| Missed | 0 |
| Unviable | 4 |
| Timeout | 0 |
| **Kill rate** | **4/4 = 100%** |

### Per file

| File | Caught | Missed | Unviable |
|---|---|---|---|
| config.rs | 2 | 0 | 1 |
| limiter.rs | 1 | 0 | 1 |
| wiring.rs | 0 | 0 | 1 |
| budget.rs | 1 | 0 | 1 |

### Caught

- config.rs:202 `scan_concurrency` -> `Ok(0)`: killed by `a_scan_concurrency_is_accepted_exactly_within_its_range`, `text_with_a_non_digit_is_never_a_scan_concurrency`
- config.rs:202 `scan_concurrency` -> `Ok(1)`: killed by the same two properties (unset must be 2; junk must be refused)
- limiter.rs:56 `ScanLimiter::release` -> `()`: killed by `ownership_is_re_checked_before_every_scrape` (acceptance)
- budget.rs:78 `>=` -> `<` in `admit_scan`: killed by `running_scans_never_exceed_the_configured_concurrency`, `a_limit_of_two_admits_exactly_as_before`, `the_seventh_scan_in_a_day_is_refused_until_a_day_has_passed`

### Unviable (do not compile; not counted)

- config.rs:129 `parse_config` -> `Ok(Default::default())` (`AppConfig` has no `Default`)
- limiter.rs:41 `ScanLimiter::admit` -> `Default::default()` (`ScanAdmission` has no `Default`)
- wiring.rs:549 `app` -> `Default::default()` (`App` has no `Default`)
- budget.rs:63 `admit_scan` -> `(Default::default(), Default::default())` (`ScanAdmission` has no `Default`)

## Survivors

None. No tests added.

## Note (outside the mutation operators)

cargo-mutants generates no mutant that substitutes a constant for `wired.config.scan_concurrency` in `wiring.rs` or for `self.max_running` in `limiter.rs`, so the threading of the configured value from config to limiter is not exercised by this gate. The domain rule (`admit_scan` honours `max_running`) and the config parse are each covered by literal-oracle properties; no end-to-end test shows a concurrency of 1 making a second owner's scan busy.

## Post-run safety

cargo-mutants mutates a temporary copy of the tree and its output directory was kept outside the repository; the working tree had no mutants artefacts after the run.
