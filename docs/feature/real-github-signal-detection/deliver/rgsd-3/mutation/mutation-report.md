# RGSD-3 Mutation Report — pure signal detector

**Scope (gated surface):** `crates/scraper-domain/src/detect.rs`
**Tool:** cargo-mutants 25.3.1 (no `--in-place`; mutants built in copied trees)
**Command:** `cargo mutants -p scraper-domain --file crates/scraper-domain/src/detect.rs`
**Killed by:** `scraper-domain` in-crate proptests (`detect::tests`)
**Gate:** ≥ 80% kill rate on the RGSD-3 detector → **PASS**

## Tally

| Outcome | Count |
|---|---|
| Caught | 34 |
| Timeout (detected — infinite loop) | 1 |
| Missed | 1 |
| Unviable (did not compile) | 4 |
| **Total generated** | **40** |

- Unmutated baseline: green (8.2s build + 1.4s test), auto test-timeout 20s.
- The 4 unviable mutants are the `Some(Default::default())` / `vec![Default::default()]` substitutions on `Signal`-producing fns — `Signal` has no `Default` impl, so they never compile and are excluded from the denominator (standard cargo-mutants accounting).

## Kill rate on the RGSD-3 detector

Viable mutants = 40 − 4 unviable = **36**.
Detected = 34 caught + 1 timeout = **35**.

**Kill rate = 35 / 36 = 97.2%** — **PASS vs the 80% gate.**

RGSD-3-specific fns (`is_semver_tag`, `matches_semver_core_at`, `pick_semver_tag`,
`detect_semver_and_changelog`, plus the `SemverAndChangelog` arm inside
`detect_signals`) account for the bulk of the generated mutants; all were caught
except the single equivalent survivor below.

## Survivor analysis

### `detect.rs:124:13` — `replace && with || in is_semver_tag` — **MISSED (equivalent mutant)**

```rust
// original
let starts_component = bytes[start].is_ascii_digit()
    && (start == 0 || !bytes[start - 1].is_ascii_digit());
// mutant
let starts_component = bytes[start].is_ascii_digit()
    || (start == 0 || !bytes[start - 1].is_ascii_digit());
```

`starts_component` is a pure **optimization / clarity filter** deciding which
start offsets `is_semver_tag` bothers to feed to `matches_semver_core_at`. It
cannot change the function's boolean output — this is a genuine **equivalent
mutant**, not a test gap. Proof that no input distinguishes the two versions:

The result is `OR over start of ( starts_component(start) AND core(start) )`,
where `core = matches_semver_core_at`. The `&&→||` change only *adds* start
offsets (makes `starts_component` true more often). The set of newly-included
offsets is `(is_digit ∧ ¬boundary) ∨ (¬is_digit ∧ boundary)`:

1. **`¬is_digit ∧ boundary`** — a non-digit start. `core(start)` scans the first
   group with `while is_ascii_digit`, collects zero digits, and returns `false`.
   So the `AND core` term is false; adds nothing to the OR.
2. **`is_digit ∧ ¬boundary`** — a digit mid-run (previous byte also a digit). If
   `core(start)` were true here (`D+.D+.D+` from `start`), then the run's
   component-boundary offset `start' ≤ start` — already included by the original
   — also satisfies `core(start')` (its first group merely has extra leading
   digits, still `D+.D+.D+`). So the original OR is *already* true; adds nothing.

Neither added offset can flip a `false` result to `true`, and none is ever
removed, so the output is identical for every input. No panic is introduced
either: when `is_digit` is true the mutant short-circuits before touching
`bytes[start - 1]`; when false, `start == 0` guards the only `start - 1` access
exactly as the original does.

**Action:** documented as equivalent — **no test added.** Writing a test to
"kill" it is impossible (no distinguishing input exists) and would only couple a
test to the internal scan-optimization structure. Per methodology, equivalent
mutants are documented, not chased.

### `detect.rs:137:15` — `replace += with *= in matches_semver_core_at` — TIMEOUT (detected)

`i *= 1` leaves the digit-scan index unchanged → infinite loop → cargo-mutants
20s auto-timeout. Counted as detected (the proptests hang and never pass under
the mutant); no action needed.

## Boundary coverage that did the killing

The existing `detect::tests` proptest corpus kills the semver boundary mutants
directly, confirming the RGSD-3 boundary cases the task flagged are covered:

- `v1`, `1.2`, `""` (non-semver, too few `MAJOR.MINOR.PATCH` components) — kill
  the separator / component-count mutants (`144:*`, `142:*`, `139:14`) and the
  `125:26 &&→||` reach-broadening mutant.
- `wincolor-0.1.6` (package-prefixed, core not at offset 0) — kills the
  `124:31 delete !` and `124:44 -→/` guard mutants (both would drop a non-zero
  core start).
- `1.2.3`, `v1.2.3`, `14.1.1`, `v2.0.0-rc1` (bare / `v`-prefixed / multi-digit /
  prerelease) — kill the digit-scan and return-value mutants (`133:*`, `136:*`,
  `119:*`).

## Verdict

- **Kill rate 97.2% (35/36) — PASS vs 80% gate.**
- 1 survivor: proven **equivalent** (documented, no test warranted).
- **No test added, no source change** — the RGSD-3 detector was already
  mutation-tight.
</content>
</invoke>
