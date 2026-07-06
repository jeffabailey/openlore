# Mutation Report — philosophy-vocabulary-registry (slice-22, Phase 5)

**Tool**: cargo-mutants 25.3.1 (Rust workspace)
**Date**: 2026-07-05
**Paradigm**: functional (ADR-007) — pure claim/vocabulary core, effect shell at I/O edges
**Method**: feature-scoped, following the established Phase-5 pattern
(`docs/evolution/viewer-persistent-left-nav-evolution.md` DV-NAV-2): mutate the
**pure domain core**, kill with in-crate feature tests; treat cross-crate
effect-shell mutants as coverage artifacts killable only through the acceptance
binary — reported, not gated.

## Pre-flight (green baselines)

| Suite | Result |
|-------|--------|
| `cargo test -p lexicon` | 38 passed (pre-existing) / 39 after the added test |
| `cargo build --bin openlore` | ok |
| `cargo test -p cli --test philosophy_vocabulary` | 8 passed |

The 2 pre-existing `viewer_graph_traversal` failures are out of scope (not
slice-22 code) and were ignored per the task boundary.

## Gated surface — PURE CORE (`crates/lexicon/src/philosophy.rs`)

Test tool: the lexicon crate's own `#[cfg(test)]` suite (validator accept/reject
arms + seed invariants + `normalize`/`object_id` property loops in
`crates/lexicon/src/lib.rs`).

### Tally

| Run | Total | Caught | Missed | Unviable | Timeout |
|-----|-------|--------|--------|----------|---------|
| Initial | 14 | 10 | 2 | 2 | 0 |
| After survivor kill | 14 | **12** | **0** | 2 | 0 |

**Pure-core kill rate** = killed / (killed + survived), excluding unviable
= **12 / 12 = 100%** (initial run was 10/12 = 83.3%).

### Gate

**>= 80% pure-core kill rate: PASSED** (100%, up from an already-passing 83.3%).

### Per-survivor analysis (initial run)

Both initial survivors were in `normalize` line 128
(`ch.is_whitespace() || ch == '_' || ch == '-'`):

1. `128:38: replace || with && in normalize` — precedence makes this
   `(ch.is_whitespace() && ch == '_') || ch == '-'`; the conjunct is
   unsatisfiable, so only `'-'` remains a separator. Whitespace/underscore get
   dropped: `normalize("memory_safety")` → `"memorysafety"`.
2. `128:44: replace == with != in normalize` — flips `'_'` classification so
   arbitrary punctuation becomes a separator while `'_'` is dropped:
   `normalize("test.driven")` → `"test-driven"`, `normalize("memory_safety")`
   → `"memorysafety"`.

**Classification: genuine test gap (NOT equivalent, NOT cross-crate artifact).**
The existing `normalize` tests pinned only *structural* invariants — output
charset `[a-z0-9-]`, no boundary/doubled dash, idempotence, NSID prefix — plus
one exact assertion on an already-kebab input (`"memory-safety"`). None pinned
the exact normalized *value* for an input containing whitespace, `'_'`, or other
punctuation, so a mutant that mis-classified separators still produced
kebab-shaped output and slipped through every property.

### Test added

`crates/lexicon/src/lib.rs::seeds_tests::normalize_maps_separators_and_punctuation_to_exact_kebab`
— a Modeling-style reference-mapping property pinning exact
`(input → expected)` pairs (`"Memory Safety"`/`"memory_safety"`/
`"  Memory   Safety  "` → `"memory-safety"`; `"test.driven"` → `"testdriven"`;
`"C++ style"` → `"c-style"`). Stays within the pure-crate dependency envelope
(hand-rolled table, the documented ADR-059 fallback where a proptest
dev-dependency is unavailable). Re-run confirmed both survivors killed → 12/12.

This is one distinct behavior (value-level separator/punctuation mapping),
complementing the existing structural-invariant tests — within the
behavior-first test budget.

## Reported surface — EFFECT SHELL (cli driving adapter, NOT gated)

Files: `crates/cli/src/render/philosophy.rs`,
`crates/cli/src/verbs/philosophy_list.rs`.
Test command: the acceptance binary (`cargo test -p cli -- --test philosophy_vocabulary`).

| Total | Caught | Missed | Unviable | Timeout |
|-------|--------|--------|----------|---------|
| 10 | 10 | 0 | 0 | 0 |

All 10 effect-shell mutants (function-replacement mutants on
`render_philosophy_list`, `render_seed_block`, and the verb `run`'s
`(i32, String)` result) were **caught via the cross-crate acceptance layer** —
the real `openlore` subprocess binary exercised by the 8-test
`philosophy_vocabulary` acceptance suite. Classified as coverage artifacts per
the established pattern; they happen to be 100% killed through the real binary,
so the effect shell carries no genuine survivor.

## Result

- **Pure-core kill rate: 100% (12/12 viable) — PASSES the >= 80% gate.**
- Effect shell: 10/10 caught via the acceptance binary (reported, not gated).
- Survivor classification: 2 initial survivors = one genuine test gap (exact
  `normalize` value mapping), closed with one added in-crate Modeling property;
  0 equivalent mutants; 0 unresolved cross-crate artifacts.
- Working tree restored clean (cargo-mutants used its scratch copy; no
  `--in-place`); `mutants.out*` scratch dirs removed.
- `cargo test -p lexicon` green (39 passed) after the added test.
</content>
</invoke>
