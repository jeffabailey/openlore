<!-- markdownlint-disable MD013 -->
# Evolution: philosophy-alias-triangulation (slice-26 — near-synonym philosophies connect at read time)

> Part of the `philosophy-vocabulary-registry` feature (slices 22–28). Builds on
> slice-22 (seeds + aliases), slice-24 (mint), and slice-25 (the pure advisory
> resolver). Paradigm: functional Rust (ADR-007). Companion design: ADR-059 §5
> (slice-26 row). This is the feature's **payoff** slice.

## Summary

slice-26 ships US-PV-005 ("Triangulate via aliases in exploration"): at READ time,
a claim authored against a philosophy's **alias** object aggregates under the
**canonical** philosophy in `graph query --object` (and `--score`/`--weighted`) —
so `mem-safety` and `memory-safety` count as one philosophy and near-synonyms
connect. It is a **derived read-time view**: the stored claim objects are never
rewritten (the signed bytes are immutable, AC-005.2), and each claim stays
attributed to its own author (anti-merging — no consensus merge, AC-005.1). No new
crate — workspace stays 21.

### What shipped (one paragraph)

A pure, total `lexicon::equivalence_class(object) -> Vec<String>` maps any object in
a seed philosophy's class (canonical object-id OR an alias object-id) to the FULL
class — canonical first, then each alias object-id (deduped) — and returns the
singleton `[object]` for an unknown/non-philosophy object (no over-widening). The
CLI object read (`adapter-duckdb::graph_query::query_by_object`) now filters on that
class: `DimensionFilter::Object` carries `Vec<String>` and emits `object = ?` for a
singleton (byte-identical to the pre-slice-26 read) or `object IN (?, …)` for a
multi-member class, on BOTH the own `claims` and `peer_claims` UNION-ALL arms. The
weighted/score path funnels through the same `ScoringFilter::ByObject →
DimensionFilter::Object`, so one change covers `graph query --object` and
`--weighted` with no `scoring` edit — `scoring` already groups by subject, so the
widened row set aggregates under the queried canonical.

### Wave timeline

- DISCUSS / DESIGN — feature-level: `feature-delta.md` US-PV-005 (AC-005.1/2) +
  ADR-059 §5 (slice-26 row). No per-slice DISCUSS/DESIGN.
- DISTILL — 2026-07-08, commit `ba7d1fe`: RED scaffold
  `tests/acceptance/philosophy_alias_triangulation.rs` (AT-1..5) +
  `distill/red-classification-slice-26.md`. Gate PASS (3/3 genuine RED, AT-4/5
  green-today, 0 BROKEN, 80% error/edge). **DISTILL also corrected the read-path**:
  the CLI reads via `graph_query::query_by_object` (not the viewer's
  `query_philosophy_survey` the design named) — the roadmap targeted the real seam.
- DELIVER — 2026-07-08 (this slice).

## DELIVER steps (5-phase TDD, functional crafter)

- **01-01** `39b2fc5` — pure `equivalence_class` over the seeds (canonical-first,
  deduped; singleton for unknown/non-philosophy) reusing `resolve_object_advisory`
  / `seeds` / `object_id` / `normalize`; exported from `lexicon`. proptest: every
  seed's name-id and each alias-id map to the SAME class; non-namespaced → singleton.
- **02-01** `b121a84` — widen the object-dimension read: `DimensionFilter::Object`
  → `Vec<String>` (the class); `where_clause` emits `object = ?` (singleton) or
  `object IN (?, …)` (multi); params bound per UNION-ALL arm. Greened **AT-1** (WS:
  alias claim under the canonical query, attributed), **AT-2** (two attributed rows,
  unmerged), **AT-3** (weighted aggregates the alias claim); kept **AT-4**
  (stored bytes immutable) + **AT-5** (singleton = no over-widening) green. No
  `scoring` change (RED_UNIT skipped — the class is proptest-covered in 01-01).

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — quality gate PASS (2 steps, AT-1..3 owned by 02-01 + `equivalence_class` unit-owned by 01-01, valid DAG, DISTILL linkage present); read-path corrected per the DISTILL finding |
| DISTILL RED | 3/3 genuine MISSING_FUNCTIONALITY (alias claim excluded by today's exact-match read) + AT-4/5 GREEN-today, 0 BROKEN — gate PASS; 80% error/edge |
| Phase-3 refactor (L1-L4) | No changes warranted — the widening is minimal and the code documents the invariants inline |
| Phase-4 adversarial review | APPROVED — 0 blockers, 0 defects across all 7 angles. Independently verified AC-005.2 immutability (read-filter-only, no write path; AT-4 catch in place), anti-merging (UNION-ALL per-row `author_did`, no merging JOIN/GROUP BY), SQL parameterization (no injection; placeholder counts match per arm), `equivalence_class` correctness (canonical-first, dedup, singleton fallback, total), scope limited to the OBJECT dimension, grouping-under-canonical (scoring groups by subject), and genuine tests |
| Phase-5 mutation | Pure-core `crates/lexicon/src/philosophy.rs` (incl. `equivalence_class`) = **100% viable kill (22/22, 0 survivors; 20 caught + 2 killed-via-timeout loop mutants, 4 unviable)** — gate ≥80% PASSED. Report: `deliver/slice-26/mutation/mutation-report.md` |
| Full regression | `cargo test --workspace` → all green except ONE parallel-load flake — `viewer_htmx::claim_with_no_evidence_renders_clearly_in_both_shapes` panicked in the harness `get_htmx` HTTP `.send()` (in-process viewer server transport error under contention, `support/mod.rs:7604`), on a viewer surface UNTOUCHED by slice-26; **isolated re-run of `viewer_htmx` = 24 passed, 0 failed**. Confirmed environment flake per the WS-determinism contract, not a regression. `graph_query_explore` 29 exact-match --object/--subject/--contributor/--score + the anti-merging survey test all green |
| check-arch | OK — 21 workspace members (no new crate); anti-merging + pure-core rules intact |

Tests green: `philosophy_alias_triangulation` 7/7 (AT-1..5 + 2 support self-tests);
`graph_query_explore` 29; `lexicon` (incl. the `equivalence_class` proptest); full
workspace 0 failed.

## Load-bearing invariants

- **AC-005.2 immutability (AT-4)**: resolution is a read-time derivation over an
  `IN`-filter — there is NO write path. The stored claim `object` (DB column +
  signed `<cid>.json`) stays the typed alias verbatim. AT-4 reads the persisted
  artifact as text and would catch any rewrite.
- **Anti-merging (AT-2)**: the widened read keeps the per-row `author_did` UNION-ALL
  projection — two triangulated claims stay two attributed rows, never merged into
  one consensus. `check-arch::no_cross_table_join_elides_author` enforces it.
- **No over-widening (AT-5)**: a no-alias / unknown / non-philosophy object resolves
  to a singleton class → today's exact-match read, byte-for-byte.

## Deviations: planned vs shipped

- No new files — edits extend `lexicon/philosophy.rs`, `lexicon/lib.rs`,
  `adapter-duckdb/graph_query.rs`. No new crate; 21 members.
- `scoring` NOT modified — widening at the read boundary flows the alias claim into
  the existing subject-grouped `ByObject` aggregation (per DESIGN row 26 split).
- **Scope**: SEED-alias equivalence only (pure over `lexicon::seeds()`).
  **Minted-philosophy aliases** (which would need a `philosophies`-table read) are a
  documented FOLLOW-UP. The **viewer's `store_read::query_philosophy_survey`** route
  is also a FOLLOW-UP — the US-PV-005 CLI ACs are satisfied by the `graph_query`
  widening; a shared widened object filter for the viewer can fold into slice-27.
- `// SCAFFOLD: true` left in the acceptance file — repo convention.
- Outcome registry: skipped — no `registry.yaml`; prior slices registered none.

## KPI / dogfood

Author a claim on `org.openlore.philosophy.mem-safety` and another (different
author) on `org.openlore.philosophy.memory-safety`, then
`./cli.sh graph query --object org.openlore.philosophy.memory-safety` → BOTH appear,
each attributed to its author, grouped under `memory-safety`; `--weighted` includes
both in the adherence aggregation. The `mem-safety` claim's stored object is
unchanged on disk.

## Next slices (OUT of scope)

27 viewer `/philosophies` surface (US-PV-006) — the last unstarted slice. (22
seed+list, 23 show, 24 mint, 25 advisory, 26 triangulation, 28 scraper shipped.)
Follow-ups: minted-philosophy alias triangulation; the viewer survey widening.

## Commit trail

`ba7d1fe` (DISTILL RED), `39b2fc5` (01-01), `b121a84` (02-01). All on `main`
(trunk-based, no PR).
