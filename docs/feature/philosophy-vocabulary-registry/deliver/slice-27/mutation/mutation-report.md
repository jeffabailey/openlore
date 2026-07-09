<!-- markdownlint-disable MD013 -->
# Slice-27 Mutation Report — philosophy-vocabulary-registry

**Date:** 2026-07-09
**Tool:** cargo-mutants 25.3.1
**Gate:** pure-core kill rate >= 80%
**Result:** **PASS** — pure-core viable kill rate = **100%** (4/4, 0 survivors)

## Scope

- **Pure core (GATED surface):** `crates/viewer-domain/src/philosophies.rs` — the
  slice-27 addition is `render_philosophies_page()` + `render_philosophy_vocabulary`
  + `render_philosophy_entry` (the pure read-only `/philosophies` view-model over
  `lexicon::philosophy::seeds()`). Killed by the pre-authored VP-1..5 + VP-INV-*
  HTTP acceptance scenarios (which exercise the rendered document) plus the
  viewer-domain in-crate unit/property tests.
- **Effect shell (REPORTED, not gated):** the `PHILOSOPHIES_URL => html_ok(
  render_philosophies_page())` route arm in `adapter-http-viewer/src/lib.rs` (a
  store-free arm mirroring `/scrape`). Covered by the VP-* HTTP subprocess tests +
  the adapter route dispatch test.

## Command

```
cargo mutants -p viewer-domain --file crates/viewer-domain/src/philosophies.rs
```

## Tally

| Outcome  | Count |
|----------|-------|
| Total mutants | 4 |
| Caught (killed) | 4 |
| Unviable (did not compile) | 0 |
| **Missed / survived** | **0** |

Runtime: 4 mutants in 15s (10.3s baseline build + 0.5s baseline test; auto test
timeout 20s).

**Pure-core viable kill rate = 4 / 4 = 100%.**

## Coverage

The renderer is a thin pure projection over the embedded seeds, so its mutation
surface is small (4 mutants — function-replacement / loop-body substitutions). Every
mutant is caught: VP-1 pins the memory-safety name + description fragment + the
`/philosophy?object=org.openlore.philosophy.memory-safety` href; VP-4 / VP-INV
pins full-seed completeness and the read-only (no form/button/mutating-hx) shape; a
mutant that empties the vocabulary loop, drops the entry render, or replaces the
page body reddens those. 0 survivors.

## Effect-shell qualitative assessment (reported, not gated)

- **`PHILOSOPHIES_URL` route arm** — the store-free `html_ok(render_philosophies_page())`
  dispatch is pinned by VP-1 (GET /philosophies → 200 + vocabulary) and VP-3 /
  VP-INV-NoControl (read-only); a mutant dropping or mis-routing the arm reddens
  them. No store handle / signing key in the arm (I-VIEW-3) — the viewer capability
  boundary (`check_viewer_capability_boundary`) stays green.
- **Nav single-source** — the `("Philosophies", PHILOSOPHIES_URL)` `LANDING_HUB_SURFACES`
  entry is pinned by VP-2 / VP-5 / VP-INV-SingleSource + the bumped (8→9)
  nav-item-count unit/property tests.

## Gate verdict

**PASS** — pure-core viable kill rate 100% (4/4, 0 survivors) ≥ 80%. Effect-shell
route + nav covered by VP-1..5 + VP-INV-* acceptance.
