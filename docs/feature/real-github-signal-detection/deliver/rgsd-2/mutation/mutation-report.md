# RGSD-2 Mutation Report — pure signal detector

- **Date**: 2026-07-06
- **Tool**: `cargo-mutants 25.3.1` (no `--in-place`)
- **Command**: `cargo mutants -p scraper-domain --file crates/scraper-domain/src/detect.rs`
- **Killed by**: `scraper-domain` in-crate proptests (`detect::tests`)
- **Gate**: ≥ 80 % kill on the pure detector — **PASS**

## Tally

| Outcome | Count |
|---|---|
| Mutants generated | 9 |
| Caught (killed) | 6 |
| Unviable (do not compile) | 3 |
| **Missed / survived** | **0** |
| Timeout | 0 |

**Viable mutants**: 6. **Killed**: 6. **Kill rate on viable mutants: 6/6 = 100 %.**

Unviable mutants are excluded from the kill-rate denominator (standard
cargo-mutants accounting): they never build, so they can never ship a
regression. All three are `…Default::default()` substitutions that fail to
compile because `ports::Signal` deliberately does not derive `Default` — the
type system itself forecloses that mutation class.

## RGSD-2 surface (the slice under test)

The RGSD-2 change is the `DependencyManifestPinned` arm
(`detect_dependency_manifest_pinned`) plus the flattened two-arm
`detect_signals`. Its mutants:

| Location | Mutation | Outcome | Killed by |
|---|---|---|---|
| `detect.rs:102` | `detect_dependency_manifest_pinned -> None` | **caught** | `a_committed_cargo_lock_fires_exactly_one_dependency_pinning_signal` (expects exactly 1 signal; mutant yields 0) |
| `detect.rs:102` | `detect_dependency_manifest_pinned -> Some(Default::default())` | unviable | `Signal` has no `Default` — does not compile |
| `detect.rs:86` | `detect_signals -> vec![]` (flatten) | **caught** | every positive proptest (committed-cargo-lock, memory-safe, both-arms) expects ≥ 1 signal; mutant yields 0 |
| `detect.rs:86` | `detect_signals -> vec![Default::default()]` (flatten) | unviable | `Signal` has no `Default` — does not compile |

**RGSD-2 viable kill rate: 2/2 = 100 % — PASS vs 80 %.**

The over-firing guard proptest
(`an_absent_cargo_lock_fires_no_dependency_pinning_signal`) and the
independence proptest (`a_memory_safe_repo_with_a_cargo_lock_fires_both_signals`)
additionally pin the arm's gating and non-suppression semantics, so the arm is
covered on both the positive and negative edges.

## RGSD-1 arm (pre-covered — not chased)

| Location | Mutation | Outcome |
|---|---|---|
| `detect.rs:61` | `is_memory_safe_language -> false` | caught |
| `detect.rs:61` | `is_memory_safe_language -> true` | caught |
| `detect.rs:117` | `detect_memory_safety_language -> None` | caught |
| `detect.rs:117` | `detect_memory_safety_language -> Some(Default::default())` | unviable |
| `detect.rs:118` | `delete ! in detect_memory_safety_language` | caught |

The RGSD-1 arm's mutants were already fully covered by the RGSD-1 proptests —
no new tests were required.

## Survivor analysis

**No survivors.** No targeted proptest or example was added — the existing
`detect::tests` proptest suite kills every viable mutant on the detector,
including the full RGSD-2 surface. No equivalent-mutant classification was
needed (the only non-killed mutants are unviable, i.e. non-compiling, not
equivalent survivors).

## Verdict

- Pure detector (`detect.rs`) viable kill rate: **100 % (6/6)** — PASS (≥ 80 %).
- RGSD-2 arm viable kill rate: **100 % (2/2)** — PASS.
- Tests added: **none** (no genuine gap).
- Tree left clean: `mutants.out*` removed; `detect.rs` unmodified (no
  `--in-place`); `cargo test -p scraper-domain` green.
