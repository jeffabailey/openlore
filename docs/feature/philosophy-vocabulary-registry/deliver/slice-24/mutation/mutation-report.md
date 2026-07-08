<!-- markdownlint-disable MD013 -->
# Slice-24 Mutation Report — philosophy-vocabulary-registry

**Date:** 2026-07-08
**Tool:** cargo-mutants 25.3.1
**Gate:** pure-core kill rate >= 80%
**Result:** **PASS** — pure-core viable kill rate = **100%** (16/16)

## Scope

- **Pure core (GATED surface):** `crates/lexicon/src/philosophy.rs` — the slice-24
  addition is Gate 1b in `validate_philosophy_json` (present-but-blank required
  string → `LexiconError::MissingField`), plus the `normalize` / `object_id` /
  `seeds` / `find` functions the mint verb reuses. Killed by the in-crate unit +
  proptest suite (`#[cfg(test)] mod tests`), including the new
  `blank_required_string_rejects_naming_the_field` and
  `well_formed_record_still_validates` properties.
- **Effect shell (REPORTED, not gated):** `crates/adapter-duckdb/src/schema_v4.rs`
  (migration), `crates/adapter-duckdb/src/lib.rs::write_signed_philosophy` (atomic
  persist), and `crates/cli/src/verbs/philosophy_add.rs` (the mint verb). Covered
  by the real-temp-store adapter integration tests and the PA-1..5 subprocess
  acceptance binary; classified qualitatively below (mirrors slice-23's precedent
  of gating the pure core and reporting the effect shell).

## Command

```
cargo mutants -p lexicon --file crates/lexicon/src/philosophy.rs
```

(Preview taken first with `cargo mutants --list --file …`. Run without
`--in-place`; cargo-mutants builds each mutant in its own scratch tree, so it ran
concurrently with the full-workspace regression suite without target-lock
contention.)

## Tally

| Outcome  | Count |
|----------|-------|
| Total mutants | 19 |
| Caught (killed) | 16 |
| Unviable (did not compile) | 3 |
| **Missed / survived** | **0** |

Runtime: 19 mutants in 22s (6.3s baseline build + 0.5s baseline test; auto test
timeout 20s).

**Pure-core viable kill rate = caught / (caught + missed) = 16 / 16 = 100%.**
(The 3 unviable mutants are the `Default::default()` substitutions —
`validate_philosophy_json -> Ok(Default::default())`, `find -> Some(Default::default())`,
`seeds -> vec![Default::default()]` — which do not compile because `Philosophy`
derives no `Default`; they never produce a runnable binary and are excluded from
the rate, per cargo-mutants convention and the slice-23 report.)

## Coverage of the slice-24 blank gate

The mutants over the required-field control flow in `validate_philosophy_json`
(the presence `!contains_key` negation and the object/root guards at lines 76 and
109) are all caught by the validator's reject/accept properties. The new blank
gate (`text.trim().is_empty()`) is exercised by
`blank_required_string_rejects_naming_the_field` (proptest over the empty +
whitespace-run equivalence class for both `name` and `description`) and guarded on
the accept side by `well_formed_record_still_validates`, so a mutant that weakens
the required-field logic reddens the suite. 0 survivors on this surface.

## Effect-shell qualitative assessment (reported, not gated)

- **`schema_v4::run_migration`** — idempotent (`CREATE TABLE IF NOT EXISTS` +
  `schema_version` guard) and forward-only; a mutant dropping the DDL or the
  version bump reddens the adapter probe test (asserts v4) and the
  `write_signed_philosophy` integration test (which needs the `philosophies`
  table). Covered.
- **`write_signed_philosophy`** — the atomic artifact-then-row write and the
  `object_id UNIQUE` duplicate → typed `WriteFailed` mapping are pinned by the
  real-temp-store integration tests (write, duplicate, v4-version). A mutant that
  skips the INSERT or the artifact write reddens those.
- **`verbs/philosophy_add.rs`** — the compose→validate→collision→prompt→sign→persist
  ordering and the seed-collision / cancel / object-id-print branches are pinned by
  PA-1..5 (subprocess acceptance). The pure `seed_collision_guidance` helper has
  its own unit test.

## Gate verdict

**PASS** — pure-core viable kill rate 100% (16/16) ≥ 80%. 0 survivors. Effect
shell covered by adapter integration + PA-1..5 acceptance. Closes the deferred
Phase-5 mutation gate for slice-24.
