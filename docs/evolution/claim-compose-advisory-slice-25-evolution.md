<!-- markdownlint-disable MD013 -->
# Evolution: claim-compose-advisory (slice-25 — `claim add` suggests the shared philosophy vocabulary, advisory-only)

> Part of the `philosophy-vocabulary-registry` feature (slices 22–28). Builds on
> slice-22 (pure lexicon vocabulary core + seeds) and slice-24 (mint). Paradigm:
> functional Rust (ADR-007). Companion design: ADR-059 §5 (slice-25 row).

## Summary

slice-25 ships US-PV-004 ("Compose a claim against the vocabulary, advisory"): the
`claim add` compose preview now shows a **non-blocking** advisory line for the
`--object`. A known philosophy object resolves to its canonical name; an object that
matched via a seed **alias** resolves to the canonical philosophy (marked as an
alias); an object in the `org.openlore.philosophy.*` namespace that matches no seed
gets a plain "not a known philosophy — will be signed as-is" warning; an ordinary
non-philosophy object gets **no** advisory (the preview is byte-identical to before).
Crucially, the advisory is **display-only** — it never rejects, and never changes the
signed bytes ("claims not truth", D3 / AC-004.3). No new crate — workspace stays 21.

### What shipped (one paragraph)

A pure, total lexicon classifier `resolve_object_advisory(object) -> ObjectAdvisory`
(an ADT: `Canonical { name } | Alias { canonical } | UnknownInNamespace |
NotPhilosophy`) over the embedded seeds — reusing `seeds()`, `normalize()`, and the
`NSID` object-id prefix (no second hardcoded prefix). Canonical name-match takes
precedence over alias-match. The CLI appends one advisory line in
`render_compose_preview` via a pure helper (`compose_object_advisory_line`) that
delegates to the resolver and formats the verdict; `build_unsigned_claim` and the
signed payload path are untouched, so the object the user typed is signed verbatim.

### Wave timeline

- DISCUSS / DESIGN — feature-level: `feature-delta.md` US-PV-004 (AC-004.1..3) +
  ADR-059 §5 (slice-25 row). No per-slice DISCUSS/DESIGN.
- DISTILL — 2026-07-08, commit `5b7c919`: RED scaffold
  `tests/acceptance/claim_compose_advisory.rs` (CA-1..5) +
  `distill/red-classification-slice-25.md`. Gate PASS (4/4 genuine RED, CA-5
  green-today, 0 BROKEN, 80% error/edge).
- DELIVER — 2026-07-08 (this slice).

## DELIVER steps (5-phase TDD, functional crafter)

- **01-01** `0c18747` — pure `ObjectAdvisory` ADT + `resolve_object_advisory`
  (canonical-before-alias, `normalize`-based, namespace-prefix-gated, total) +
  proptest over every seed's object-id/aliases and the two negative arms. Exported
  from `lexicon`.
- **02-01** `994f616` — advisory line in `render_compose_preview` (display-only;
  Canonical → `↳ resolves to <name>`, Alias → `↳ resolves to <canonical> (alias)`,
  UnknownInNamespace → `⚠ not a known philosophy — will be signed as-is`,
  NotPhilosophy → nothing). Greened **CA-1** (WS known), **CA-2** (alias), **CA-3**
  (unknown-still-signs), **CA-4** (byte-parity); kept **CA-5** green (no over-firing).
- **Phase-3 refactor** `1e3119f` — L2: parse the seed set once in the resolver
  (the sole adversarial-review finding, STYLE-001).

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated quality gate PASS (2 steps / 2 production files, CA-1..4 owned by 02-01 + resolver unit-owned by 01-01, valid DAG 01-01 → 02-01, DISTILL linkage present) |
| DISTILL RED | 4/4 genuine MISSING_FUNCTIONALITY (advisory line absent from today's preview) + CA-5 GREEN-today, 0 BROKEN — gate PASS; 80% error/edge |
| Phase-3 refactor (L1-L4) | One L2 applied (`1e3119f`, parse seeds once); code otherwise clean |
| Phase-4 adversarial review | APPROVED — 0 blockers, 0 high/medium, 1 low (STYLE-001, fixed). Independently traced the AC-004.3 byte-parity invariant (advisory never reaches `build_unsigned_claim`/signed bytes), the non-blocking guarantee, canonical-before-alias precedence, resolver totality, CA-5 no-regression, pure/effect boundary, and genuine (non-theater) property test |
| Phase-5 mutation | Pure-core `crates/lexicon/src/philosophy.rs` (incl. `resolve_object_advisory`) = **100% viable kill (18/18, 4 unviable, 0 survivors)** — gate ≥80% PASSED. Report: `deliver/slice-25/mutation/mutation-report.md` |
| Full regression | `cargo test --workspace` → all 105 result blocks green, 0 failed (incl. `walking_skeleton` + the `not as truth`/bucket-label preview invariants — no regression) |
| check-arch | OK — 21 workspace members (no new crate); pure-core import ban intact |

Tests green: `claim_compose_advisory` 7/7 (CA-1..5 + 2 support self-tests); `lexicon`
44; full workspace 0 failed.

## The load-bearing invariant (AC-004.3 / D3)

"Claims not truth": the advisory is a **preview-string-only** concern. `CA-4` is the
guard — it types an alias object (`org.openlore.philosophy.mem-safety`), confirms the
sign, and asserts the persisted signed `<cid>.json` contains that alias **verbatim**
and does NOT contain the canonical rewrite (`…memory-safety`). The resolver's result
never flows into `build_unsigned_claim`, the CID, or the signed bytes. Verified by
both CA-4 and the adversarial review's call-site trace.

## Deviations: planned vs shipped

- No new files — all three edits extend existing files (`lexicon/philosophy.rs`,
  `lexicon/lib.rs`, `cli/verbs/claim_add.rs`). No new crate; 21 members.
- Advisory trigger keys on the `org.openlore.philosophy.*` object namespace (an
  out-of-namespace object gets no advisory — CA-5). Alias resolution here is the
  small pure display-only classifier; the harder **read-time aggregation** (grouping
  stored claims under canonical in `graph query`/`score`) is slice-26 — OUT of scope.
- `// SCAFFOLD: true` left in `tests/acceptance/claim_compose_advisory.rs` — repo
  convention.
- Outcome registry: skipped — `docs/product/outcomes/registry.yaml` does not exist
  and prior slices registered none (precedent).

## KPI / dogfood

`./cli.sh claim add --subject github:rust-lang/rust --predicate embodiesPhilosophy
--object org.openlore.philosophy.mem-safety --confidence 0.8` → the preview shows
`↳ resolves to memory-safety (alias)`; typing an unknown
`…philosophy.not-a-real-one` shows `⚠ not a known philosophy — will be signed as-is`
and still signs on confirm; a non-philosophy `--object github:rust-lang/rust` shows
no advisory. In every case the object signed is exactly what was typed.

## Next slices (OUT of scope)

26 alias triangulation (read-time aggregation — the payoff), 27 viewer
`/philosophies` surface. (22 seed+list, 23 show, 24 mint, 25 advisory, 28 scraper
single-source shipped.)

## Commit trail

`5b7c919` (DISTILL RED), `0c18747` (01-01), `994f616` (02-01), `1e3119f` (L2
refactor). All on `main` (trunk-based, no PR).
