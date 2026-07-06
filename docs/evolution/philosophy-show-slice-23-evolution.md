<!-- markdownlint-disable MD013 -->
# Evolution: philosophy-show (slice-23 — `openlore philosophy show <name-or-object>`: inspect one philosophy's full record from the embedded vocabulary)

> Part of the `philosophy-vocabulary-registry` feature (slices 22–28). Builds on
> slice-22 (the pure lexicon vocabulary core + `philosophy list`). Paradigm:
> functional Rust (ADR-007). Companion design: ADR-059 §5 (slice-23 row).

## Summary

slice-23 ships US-PV-002 ("Inspect one philosophy"): the read verb `openlore
philosophy show <name-or-object>` prints one philosophy's full record — name,
description, aliases, seeAlso — from the 12 embedded seeds. It accepts EITHER a
bare name (`memory-safety`) OR the full object id
(`org.openlore.philosophy.memory-safety`), both resolving to the same record. An
unknown key exits non-zero with plain guidance ("no such philosophy: <key>" +
hints to `philosophy list` / `philosophy add`) and NEVER a stack trace. Offline
by construction (embedded seeds; no store, no network, dispatched before
`Wiring::production`). No new crate — workspace stays 21 members.

### What shipped (one paragraph)

A pure total seam `lexicon::philosophy::find(key: &str) -> Option<Philosophy>`
(resolves the seed whose `object_id(name) == key` OR whose `normalize(name) ==
normalize(key)` — so name and object id both resolve, case/separator-insensitive
on the bare name) is the natural unit/property boundary. The CLI adds
`PhilosophyCommand::Show { key }` routed through the same early/offline dispatch
as `list`, a thin verb `verbs/philosophy_show.rs` (found → render + exit 0;
unknown → guidance + non-zero, no panic), and a pure single-record renderer
`render/philosophy.rs::render_record`. Text-only (AC-002.1 scope; no `--json`).

### Wave timeline

- DISCUSS / DESIGN — covered at the feature level: `feature-delta.md` US-PV-002
  (AC-002.1 / AC-002.2) + ADR-059 §5 (slice-23 row). No per-slice DISCUSS/DESIGN
  needed — a thin read verb over the settled slice-22 core.
- DISTILL — 2026-07-06, commit `2ef46a9`: RED scaffold `tests/acceptance/philosophy_show.rs`
  (PS-1..4) + `distill/red-classification-slice-23.md`. Gate PASS (4/4 genuine
  RED, 0 BROKEN).
- DELIVER — 2026-07-06 (this slice).

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `2a3f9fc` — pure `find` seam (+ 2 proptests) + `philosophy show`
  happy read (by name AND object id) + offline early-dispatch + `render_record`.
  Greened **PS-1** (walking skeleton), **PS-2** (by object id), **PS-4** (offline).
- **01-02** `d1358ff` — unknown-key guidance: non-zero exit + "no such philosophy"
  + list/add hints, no panic. Greened **PS-3**.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated quality gate PASS (decomposition ratio 0.4 steps/production-files, 4 RED scenarios each owned once, valid DAG 01-01 → 01-02, IDs valid) |
| DISTILL RED | 4/4 genuine MISSING_FUNCTIONALITY (clap `unrecognized subcommand 'show'`), 0 BROKEN — gate PASS |
| Integrity | `verify_deliver_integrity` exit 0 — all 2 steps complete DES traces |
| Phase-3 refactor (L1-L4) | No changes warranted (code already clean; `find`'s two match arms are distinct required strategies, not duplication) |
| Phase-4 adversarial review | APPROVED — 0 defects, 0 testing theater, external validity confirmed (port-to-port subprocess), test budget respected (4/8) |
| Phase-5 mutation | Pure-core `find`/normalize/object_id = **100% (16/16 viable, 3 unviable Default-substitutions, 0 survivors)** — gate ≥80% PASSED; effect-shell covered by the PS-1..4 acceptance binary |
| check-arch | OK — 21 workspace members (no new crate) |

Tests green: `philosophy_show` 6/6 (PS-1..4 + 2 support framework self-tests);
`lexicon` 41; `philosophy_vocabulary` 8/8 (slice-22 no-regression).

## Also fixed this session (pre-existing regression, out of slice scope)

`f252fd7` — the two `viewer_graph_traversal` no-claims tests
(`a_claim_less_{project,philosophy}_renders_the_guided_no_claims_state`) had been
failing on `main` since slice-21 (viewer-persistent-left-nav, ADR-058) added a
persistent `<nav id="viewer-nav">` carrying the fixed landing-hub links
(`/project`, `/philosophy`, `/score`) to every full page. The I-GT-4
"no fabricated traversal edge" helper's whole-page substring scan false-positived
on that site chrome. Fix (test-support only; production markup unchanged): added
`strip_persistent_nav` and scoped the fabricated-edge/cid scan to the content
region — the invariant (empty content invents no drill edge) is preserved exactly;
the HX-Request fragment carries no chrome so it is checked in full.
`viewer_graph_traversal` now 16/16 (was 14/2).

## Deviations: planned vs shipped

- DELIVER workspace lives under `deliver/slice-23/` (its own roadmap +
  execution-log + mutation report) to avoid clobbering slice-22's finalized
  `deliver/` artifacts — the feature spans multiple slices under one feature dir.
  Note: the DES stop-hook validates the deliver-level `execution-log.json`, so a
  step's phases can land there transiently; slice-22's committed log was restored
  and slice-23's phases consolidated into `deliver/slice-23/execution-log.json`.
- `crates/lexicon/Cargo.toml` gained a `proptest` dev-dependency for the `find`
  property tests (dev-only; no runtime/crate-count impact — still 21 members).

## KPI / dogfood

`./cli.sh philosophy show memory-safety` → the full record (description + aliases
+ seeAlso); `./cli.sh philosophy show org.openlore.philosophy.memory-safety` →
the same record; `./cli.sh philosophy show nope` → plain guidance + non-zero.

## Next slices (OUT of scope)

24 mint (`philosophy add` — signed, open), 25 claim-compose advisory, 26 alias
triangulation, 27 viewer `/philosophies` surface, 28 scraper single-source.

## Commit trail

`2ef46a9` (DISTILL RED), `2a3f9fc` (01-01), `d1358ff` (01-02). Plus the
out-of-scope regression fix `f252fd7`. All on `main` (trunk-based, no PR).
