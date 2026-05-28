# Technology Stack — openlore-scoring-graph (slice-04) — DELTA from slice-03

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Extends**: `docs/feature/openlore-federated-read/design/technology-stack.md`

**Slice-04 introduces ZERO new external crates as production dependencies.**
The WD-8 store revisit resolves to AUGMENT DuckDB with recursive CTEs (a
built-in SQL feature; ADR-021), and the new `scoring` core is pure arithmetic
over `std` + existing pure value types. This is even more conservative than
slice-03 (which added one pure crate, `unicode-normalization`): slice-04 adds
**no new crate of any kind**.

## Production crates — slice-04 surface

| Crate (already in slice-01/03) | New use in slice-04 | License | Justification |
|---|---|---|---|
| `duckdb` (Rust crate) | Recursive CTEs (`WITH RECURSIVE`) for bounded graph traversal; `UNION ALL` attributed projections for the dimension + scoring-feed reads. ALL over the SAME single-file store; NO new tables. | MIT | Already used per ADR-001. Recursive CTEs are a built-in DuckDB SQL feature — no version bump, no extension, no new dependency. The WD-8 revisit chose AUGMENT precisely to avoid a new store (ADR-021). |
| `clap` | The 6 new explorer flags on `graph query`: `--object`, `--contributor`, `--traverse`, `--depth K`, `--weighted`, `--explain <subject>` (ADR-020). | MIT / Apache-2.0 | Already used. Flags on an existing verb; no derive-macro version bump. |
| `serde` / `serde_json` | Deserialize `claims`/`peer_claims` rows into `AttributedClaim` for scoring (read path only; nothing new is serialized OUT). | MIT / Apache-2.0 | Already used. |
| `chrono` | `AttributedClaim.composed_at` / `Contribution` timestamps (pure value types in the scoring core). | MIT / Apache-2.0 | Already used. Pure dependency — permitted in the `scoring` pure-core allowlist. |
| `thiserror` | No new error enum needed; the new `StorageError` variants (scoring-feed / traversal) extend the existing enum. | MIT / Apache-2.0 | Already used. |
| `tracing` | New `health.startup.refused{reason: storage.traversal_*}` probe-failure variants; the `graph.connection.surfaced` + `graph.query.duration_seconds` KPI events (DEVOPS owns wiring). | MIT | Already used per ADR-010. |
| `tokio` | UNCHANGED — no async in slice-04 (all new `StoragePort` methods are sync local reads; no network). | MIT | Already used; not exercised by new slice-04 code paths. |

## NEW production crate — `scoring` (workspace member; NO new external dependency)

| Crate | Kind | External deps | License | Purpose |
|---|---|---|---|---|
| `crates/scoring` | PURE workspace member | NONE beyond `std` + workspace `chrono` + the pure `Did`/`Cid` value types from `ports`/`claim-domain` | (workspace-internal; inherits the MIT/Apache-2.0 workspace license) | The transparent, closed-form, no-ML adherence-weight core. Holds the formula constants as SSOT. Pure; no I/O; in the `xtask check-arch` pure-core allowlist. |

`scoring` is a NEW crate but adds NO new EXTERNAL dependency — it is pure Rust
arithmetic over types the workspace already has. It is the symmetric
counterpart to slice-02's `scraper-domain` (a pure derivation core). The
production crate count goes from 10 to 11; the external-dependency count is
unchanged.

## NO new store / NO graph-DB crate (the WD-8 revisit resolution)

The headline DESIGN decision (OD-GRAPH-1, WD-8) resolves to AUGMENT DuckDB,
NOT swap/add a graph store. Consequently:

- **No Kùzu / KuzuDB embedded crate.** Considered as the "right long-term
  home" for deep unbounded traversal; rejected for slice-04 because the
  traversal workload (low-thousands of claims, bounded default depth 2) is
  handled by recursive CTEs, and a second store would add a new dependency, a
  new license review, a new `cargo deny` entry, a new adapter crate with its
  own `probe()`, a second backup target, and a claims↔graph sync problem. Full
  trade-off table in `architecture-design.md` §9 + ADR-021.
- **No `petgraph` / in-memory graph crate.** Considered for an in-memory
  adjacency layer; rejected because the edges ARE the claim rows — materializing
  a separate in-memory graph would duplicate them and risk the
  invented-edge failure mode (Gate 5). The recursive CTE derives edges on
  demand from the authoritative rows.
- **No ML / inference crate.** Forbidden by WD-71. The weight is closed-form
  arithmetic; `scoring` MUST NOT depend on any ML/inference crate (enforced by
  the pure-core allowlist).

## Test-only / dev-dependency additions (slice-04)

| Crate | License | Purpose |
|---|---|---|
| (none new) | — | Slice-04 reuses the existing `test-support` doubles. New fixtures (deterministic scoring set, cyclic-graph traversal fixture, sparse fixture) are DATA in `test-support`, not new crates. Mutation testing (already in the nightly CI per slice-01) extends to `crates/scoring`; the mutation-test runner is already a dev tool, no new crate. |

## License compliance

No new external dependency -> no new license consideration. The slice-01
`cargo deny check licenses` allowlist (MIT OR Apache-2.0 OR BSD-3-Clause OR
Unicode-DFS-2016) is unchanged and satisfied (I-11).

## Versioning policy

Per slice-01: pin MAJOR.MINOR in `Cargo.toml`; `Cargo.lock` resolves PATCH.
Slice-04 bumps NO dependency's MAJOR.MINOR (recursive CTEs are already
available in the pinned DuckDB line). The new `scoring` crate is pinned at the
workspace version.

## Supply chain (inherited)

- `cargo deny check advisories | bans | sources | licenses` runs in CI on
  every commit (I-11). No changes for slice-04 (zero new external deps).
- Reproducible builds via committed `Cargo.lock`.
- No prebuilt binary dependencies.

## Rejected alternatives

| Alternative | Rejected because |
|---|---|
| Swap `adapter-duckdb` for an embedded graph store (Kùzu / KuzuDB) | WD-8 revisit -> AUGMENT (ADR-021). Premature at slice-04 scale; new dependency + adapter + second store + sync problem + new anti-merging enforcement substrate. Revisit trigger documented (dogfeed P95 breach OR >100k peer_claims OR unbounded-deep-traversal JTBD). |
| Add `petgraph` for an in-memory traversal graph | Edges ARE the claim rows; a separate graph would duplicate them and risk invented edges (Gate 5). Recursive CTE derives edges from authoritative rows on demand. |
| Put the scoring formula in `claim-domain` instead of a new crate | The formula is a distinct pure-domain concept (its own ADTs, constants SSOT, mutation-test surface). A module in `claim-domain` would muddy that crate's signing/CID focus. A new PURE crate adds no external dep, no operational boundary (WD-82). |
| Any ML / learned scoring model (e.g., a small embedding/regression crate) | Locked rejected by WD-71. An ML score is unauditable and re-triggers the aggregator distrust the product exists to avoid. The weight MUST be reproducible by hand. |
| Persist/cache computed weights in a new table for query speed | Locked rejected by WD-72. A persisted score goes stale and tempts federation of a derived value. Weights are computed at query time; bounded depth + DuckDB columnar scan keep it fast enough (KPI-GRAPH-6). A future need requires a WD + ADR. |
| Config-file scoring constants instead of compile-time `const` | A constant change is a code change, never a learned/config weight (WD-71). A config path would require a WD + ADR; deferred (Q-DELIVER #4). |

## Summary

Slice-04's technology stack is the slice-03 stack PLUS one pure workspace
crate (`scoring`) and MINUS any new external dependency. The WD-8 store
revisit chose AUGMENT (recursive CTEs over the existing DuckDB file) precisely
to keep the dependency surface at zero growth. The scoring-graph thesis is
validated on the same technology surface that proved the walking skeleton —
the most conservative slice yet on the dependency axis.
