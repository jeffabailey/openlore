# RGSD-5 Mutation Report — `detect_test_ratio_or_ci_matrix` (pure detector)

**Date:** 2026-07-07
**Tool:** cargo-mutants 25.3.1 (no `--in-place`)
**Command:** `cargo mutants -p scraper-domain --file crates/scraper-domain/src/detect.rs`
**Gate:** ≥ 80% kill rate on the pure detector — **PASS**

## Tally (whole file)

| Outcome | Count |
|---|---|
| Total mutants | 47 |
| Caught | 39 |
| Timeout (infinite loop → effectively killed) | 1 |
| Missed (survivor) | 1 |
| Unviable (did not compile) | 6 |
| **Viable (denominator)** | **41** |

**Kill rate (whole file):** (39 caught + 1 timeout) / 41 viable = **40/41 = 97.6%** (95.1% even if the timeout is excluded). Both comfortably clear the 80% gate.

## RGSD-5 detector kill rate (the gated surface)

`detect_test_ratio_or_ci_matrix` (detect.rs:171) generates exactly **2 viable mutants**:

| Mutant | Outcome | Killed by |
|---|---|---|
| `171:5 replace … -> Option<Signal> with None` | CAUGHT | `test_ratio_or_ci_matrix_fires_only_on_the_ci_or_tests_disjunction` — when `ci\|tests` present the arm must fire; `None` makes it never fire → `!empty == should_fire` fails. |
| `171:5 replace … -> Option<Signal> with Some(Default::default())` | CAUGHT | Same proptest — a default `Signal` has the wrong `kind`/empty `value`/empty `source_url`; fails both the "must NOT fire when neither present" branch and the CI/tests value+source_url assertions. |

**RGSD-5 detector kill rate: 2/2 = 100% — PASS.** No survivor on the RGSD-5 arm; the `||` disjunction and the CI-precedence value/source_url selection are fully pinned by the existing property test. **No test added.**

## Survivor analysis

### 1 MISSED — `244:43 replace && with || in is_semver_tag` (RGSD-3, out of RGSD-5 scope)

This is the orchestrator-flagged **PROVEN equivalent mutant**. Re-verified equivalent here:

```rust
let starts_component =
    bytes[start].is_ascii_digit() && (start == 0 || !bytes[start - 1].is_ascii_digit());
starts_component && matches_semver_core_at(bytes, start)
```

The mutant weakens the inner `&&` to `||`: `digit(start) || boundary`. It differs from the original `digit(start) && boundary` only when `digit(start) == false && boundary == true`, or when `digit(start) == true && boundary == false`.

- **`digit(start) == false`:** `matches_semver_core_at(bytes, start)` scans zero digits in its first group (`group_start == i`) and returns `false`, so the outer `starts_component && matches_semver_core_at` is `false` in **both** original and mutant. No observable difference.
- **`digit(start) == true && boundary == false`** (a mid-run digit): the mutant additionally evaluates `matches_semver_core_at` at a mid-run offset. But the first digit group's while-loop runs to the same end index regardless of where inside the run it starts, so `matches_semver_core_at` yields the **same** result at a mid-run offset as at that run's boundary offset (which the `.any()` scan also visits). Any positive the mutant "gains" at a mid-run start is already produced at the run boundary; any string it would accept, the original already accepts. **No observable behavior change → genuinely equivalent.**

Not chased (per instruction and proof).

### 1 TIMEOUT — `257:15 replace += with *= in matches_semver_core_at` (RGSD-3, out of scope)

`i *= 1` leaves the digit-scan index unchanged, so `while i < len && bytes[i].is_ascii_digit()` never advances → infinite loop, surfaced as a 20s timeout. This is a real divergence detected by the suite (effectively killed), not a silent survivor. RGSD-3 arm; out of RGSD-5 scope.

## Conclusion

- **RGSD-5 pure detector: 100% (2/2) — PASS vs 80% gate.**
- Whole-file kill rate 97.6% — PASS.
- The single survivor is a proven-equivalent RGSD-3 mutant (not RGSD-5); no genuine gap.
- **No new test required; no new proptest-regressions seed produced.**
