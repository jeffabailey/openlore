# Slice-23 Mutation Report — philosophy-vocabulary-registry

**Date:** 2026-07-06
**Tool:** cargo-mutants 25.3.1
**Gate:** pure-core kill rate >= 80%
**Result:** **PASS** — pure-core viable kill rate = **100%** (16/16)

## Scope

- **Pure core (GATED surface):** `crates/lexicon/src/philosophy.rs` — the slice-23
  `find(key) -> Option<Philosophy>` resolver plus the `normalize` / `object_id` /
  `seeds` / `validate_philosophy_json` functions it reuses. In-crate unit +
  proptest suite (`#[cfg(test)] mod tests`) does the killing.
- **Effect shell (REPORTED, not gated):** `crates/cli/src/verbs/philosophy_show.rs`
  (show verb) and `crates/cli/src/render/philosophy.rs` (`render_record`). Classified
  by inspection below — see "Effect-shell qualitative assessment".

## Command

```
cargo mutants -p lexicon --file crates/lexicon/src/philosophy.rs
```

(Run WITHOUT `--in-place`; `-- --test-threads=1` was dropped because it broke
cargo-mutants' trailing-arg passing and the lexicon pure tests need no
serialization. Preview taken first with `cargo mutants --list --file ...`.)

## Tally

| Outcome  | Count |
|----------|-------|
| Total mutants | 19 |
| Caught (killed) | 16 |
| Unviable (did not compile) | 3 |
| **Missed / survived** | **0** |

Runtime: 19 mutants in 19s (4.7s baseline build + 0.5s baseline test).

**Pure-core viable kill rate = caught / (caught + missed) = 16 / 16 = 100%.**
(Unviable mutants are excluded from the rate: they never produce a runnable
binary, so no test could observe them.)

### The 3 unviable mutants

All three substitute `Default::default()` for a `Philosophy`-bearing return, but
`Philosophy` deliberately derives only `Debug, Clone, PartialEq, Serialize,
Deserialize` — **no `Default`** — so these do not compile:

- `philosophy.rs:69` `validate_philosophy_json -> ... with Ok(Default::default())`
- `philosophy.rs:169` `seeds -> Vec<Philosophy> with vec![Default::default()]`
- `philosophy.rs:194` `find -> Option<Philosophy> with Some(Default::default())`

The absence of a `Default` impl is itself an illegal-states-unrepresentable
guard (a philosophy has no meaningful empty value), so these are not test gaps.

### Caught mutants (16) — representative kills

- `find -> None` — killed by `find_resolves_every_seed_by_name_and_object_id`
  (every seed must resolve to `Some(itself)`).
- `find`: `||`->`&&`, `==`->`!=` (both arms) — killed by the resolve/round-trip
  and totality-soundness proptests (the name-OR-object contract flips).
- `normalize -> String::new()` / `-> "xyzzy".into()` — killed via `object_id`
  round-trips and the seed object-id invariants.
- `normalize`: `||`->`&&`, `==`->`!=`, `delete !` (separator collapse logic) —
  killed by the kebab-idempotence / object-id-byte-identity checks.
- `object_id -> String::new()` / `-> "xyzzy".into()` — killed by seed object-id
  assertions and `find`-by-object-id round-trip.
- `seeds -> vec![]` — killed (empty vocabulary breaks the resolve proptest).
- `validate_philosophy_json` guard-negations (`delete !` at required-field and
  array-of-string gates) — killed by lexicon validator tests.

## Survivor analysis

**None.** Zero pure-core mutants survived; no targeted test was needed and no
`test(...): kill slice-23 mutation survivor` commit was required.

## Effect-shell qualitative assessment (not gated)

Classified by inspection; the acceptance binary `philosophy_show` (6/6, incl
PS-1..4) exercises both paths:

- `philosophy_show::run` match (`Some -> (0, render_record)`, `None -> (1,
  guidance)`): a `find -> None` or exit-code flip would be caught by
  `philosophy_show_by_name_...` (exit 0 + full render) and
  `philosophy_show_unknown_name_exits_non_zero_with_plain_guidance` (exit 1 +
  guidance substring). Well-covered.
- `unknown_philosophy_guidance` string: the unknown-name AT asserts the miss is
  named verbatim and the recovery-verb hints appear — a stubbed/emptied guidance
  would fail that assertion.
- `render_record`: covered by the by-name and by-object-id show ATs (name,
  full description, aliases, seeAlso all asserted present).

These are effect-shell files driven through the process boundary; per the
project Phase-5 pattern they are reported qualitatively, not gated.

## Post-run hygiene

- Ran without `--in-place`; `git status` shows **no** modification to
  `philosophy.rs` or the cli source files.
- `mutants.out` / `mutants.out.old` scratch removed.
- `cargo test -p lexicon` green afterward: **41 passed, 0 failed**.

## Verdict

**PASS.** Pure-core kill rate 100% (16/16 viable) >= 80% gate. No survivors, no
test added, no source change.
