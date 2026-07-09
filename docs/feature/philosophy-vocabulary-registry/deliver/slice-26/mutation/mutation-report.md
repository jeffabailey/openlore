<!-- markdownlint-disable MD013 -->
# Slice-26 Mutation Report — philosophy-vocabulary-registry

**Date:** 2026-07-08
**Tool:** cargo-mutants 25.3.1
**Gate:** pure-core kill rate >= 80%
**Result:** **PASS** — pure-core viable kill rate = **100%** (22/22, 0 survivors)

## Scope

- **Pure core (GATED surface):** `crates/lexicon/src/philosophy.rs` — the slice-26
  addition is `equivalence_class(object) -> Vec<String>` (the read-time
  canonical+alias class), plus the `resolve_object_advisory` / `seeds` / `normalize`
  / `object_id` functions it reuses. Killed by the in-crate proptest
  `equivalence_class_returns_canonical_plus_alias_object_ids_or_singleton` (every
  seed's object_id(name) and each alias object-id map to the SAME full class;
  non-namespaced input → singleton) plus the pre-existing lexicon property suite.
- **Effect shell (REPORTED, not gated):** `crates/adapter-duckdb/src/graph_query.rs`
  — the `DimensionFilter::Object(Vec<String>)` widening (`object = ?` singleton /
  `object IN (?, …)` multi-member). Covered by the AT-1..5 subprocess acceptance
  binary + `xtask check-arch::no_cross_table_join_elides_author` (anti-merging).
  Classified qualitatively below.

## Command

```
cargo mutants -p lexicon --file crates/lexicon/src/philosophy.rs
```

## Tally

| Outcome  | Count |
|----------|-------|
| Total mutants | 26 |
| Caught (killed) | 20 |
| Killed via timeout | 2 |
| Unviable (did not compile) | 4 |
| **Missed / survived** | **0** |

Runtime: 26 mutants in 2m21s (74.2s baseline build + 0.6s baseline test; auto test
timeout 20s).

**Pure-core viable kill rate = (caught + timeout) / (caught + timeout + missed) =
22 / 22 = 100%.** The 2 timeouts are loop mutants (e.g. flipping the `normalize`
separator-collapse / trailing-`-` trim condition into a non-terminating loop) —
the mutation is detected as a hang and killed by the timeout mechanism, not a
survivor. The 4 unviable are `Default::default()` substitutions over
`Philosophy`/`ObjectAdvisory` (no derived `Default`), excluded from the rate.

## Coverage of the slice-26 resolver

Mutants over `equivalence_class` — returning the wrong class, dropping the alias
arm, mis-ordering canonical-vs-alias, or short-circuiting the singleton fallback —
are caught by the proptest (every seed's name-id and alias-ids resolve to the SAME
class; a non-namespaced string → its own singleton). 0 survivors. The reused
`resolve_object_advisory` / `normalize` / `object_id` mutants are killed by their
existing property suites (the 2 timeouts sit in `normalize`'s loop).

## Effect-shell qualitative assessment (reported, not gated)

- **`DimensionFilter::Object` widening (`graph_query.rs`)** — the singleton
  `object = ?` vs multi-member `object IN (?, …)` branch is pinned by AT-5 (a
  no-alias/other-class object returns only exact matches — a mutant forcing the
  `IN` branch for singletons would still be equivalent, but a mutant dropping the
  `IN` branch reddens AT-1/2/3) and AT-1/2/3 (the alias claim must appear). The
  per-arm parameter binding is exercised by the real cross-store UNION-ALL reads.
- **Anti-merging** — AT-2 + `check-arch::no_cross_table_join_elides_author` guard
  that the widened read still projects `author_did` per row; a mutant collapsing to
  a merging aggregate reddens both.
- **AC-005.2 immutability (AT-4)** — the widening is read-filter-only; no write
  path exists, so no mutant in this surface can rewrite a stored object. AT-4 reads
  the persisted `<cid>.json` alias object as text and would catch any rewrite.

## Gate verdict

**PASS** — pure-core viable kill rate 100% (22/22, 0 survivors) ≥ 80%. Effect-shell
widening covered by AT-1..5 + the anti-merging arch rule.
