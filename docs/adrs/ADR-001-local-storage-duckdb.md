# ADR-001: Local Storage Engine = DuckDB

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect), per WD-8 lock from Luna (nw-product-owner)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

slice-01 needs a single-file, embedded, in-process local store that holds the
user's signed claims, supports indexed lookup by `(subject, predicate, object,
author_did, cid)`, persists across CLI invocations, and is portable across the
three target platforms (macOS, Linux, WSL2). The walking skeleton's query
workload is single-subject lookup with optional predicate filter — no graph
traversal yet. (Graph traversal is the slice-04 workload; that slice is
explicitly out of scope for this feature and re-opens this decision per WD-8.)

P-001 ("Senior Engineer Solo Builder") is comfortable with SQL and lives in a
terminal; they expect "one binary, one file, no daemon."

Locked input: WD-8 in `docs/feature/openlore-foundation/feature-delta.md` fixes
DuckDB for slice-01 and re-opens the choice in slice-04. This ADR records the
binding architectural decision for slice-01 and cites the alternatives analysis
in `docs/feature/openlore-foundation/discuss/alternatives-considered.md`
(Choice 1) without relitigating it.

## Decision

**Use DuckDB as the embedded local store for slice-01**, accessed via the
official Rust `duckdb` crate.

- Single file at `~/.local/share/openlore/openlore.duckdb` (XDG-respecting).
- Schema versioned via a single `schema_version` table; migrations applied at
  `openlore init` and on each subsequent start (idempotent forward-only).
- All claim writes go through a `storage-port` trait so DuckDB stays a swappable
  adapter (see ADR-009 — Hexagonal architecture style).

## Alternatives Considered

| Option | Rejection rationale (slice-01) |
|---|---|
| **Kùzu** — embedded native graph DB | Slice-01 needs only indexed lookup; the graph-traversal advantage is unused. Adopting Kùzu now pays a learning-curve + Rust-client-maturity cost for capability not needed until slice-04. **Re-evaluated in slice-04** under benchmark. |
| **SurrealDB** — multi-model embedded | Embedded story younger than DuckDB's; surface area larger than walking skeleton needs. Loses on "kill or validate the thesis fast." |
| **SQLite** — embedded SQL | Viable but DuckDB's columnar engine handles the future analytical workload (slice-04 scoring) more gracefully without a second migration; DuckDB is already the user's stated choice for the umbrella. |
| **Plain JSON files only** (no DB) | Already mandated as the canonical signed artifact (see ADR-006). The DB is a derived **index**, not the source of truth — but query by subject without a DB requires N file reads. |

Full evaluation in `docs/feature/openlore-foundation/discuss/alternatives-considered.md`
Choice 1.

## Consequences

### Positive

- Single embedded file; zero ops surface for P-001.
- SQL is a known surface for the persona; debuggable via `duckdb` CLI.
- Permissive license (MIT) — no copyleft hazard.
- Columnar engine sets up slice-04 analytical queries gracefully even if Kùzu
  later supplements it for traversal.

### Negative

- DuckDB's Rust client is less mature than its Python client; some surface area
  (async, advanced types) may require workarounds. **Mitigation**: keep DuckDB
  behind the `storage-port` trait so the blast radius of a vendor swap is
  contained to one adapter module.
- Graph traversal via recursive CTEs is awkward; slice-04 may add Kùzu.
- Concurrent writers are not safe; CLI assumes single-process at a time per
  database file. **Mitigation**: take a file lock at adapter open; fail fast.

### Earned Trust (per principle 12)

The `duckdb-adapter` MUST expose a `probe()` method that the composition root
runs at startup (see ADR-009, "Wire then probe then use"). The probe MUST:

1. Open the DB file at the configured path and verify schema version matches
   the embedded migration set.
2. Write a sentinel row with a known CID, read it back, and assert byte-equality
   on all fields (validates DuckDB's serde round-trip behavior — historically a
   source of silent type coercion).
3. Verify `fsync` is honored on the storage medium (tmpfs / Docker overlayfs /
   WSL2 DrvFs all lie about durability — see Earned Trust rule). Probe writes a
   sentinel, `fsync`s, opens a fresh handle, and reads back. On failure,
   refuses to start with `health.startup.refused{reason: storage.fsync_unreliable, ...}`.

## Revisit Trigger

- slice-04 (`openlore-scoring-graph`) re-opens this decision by benchmark.
- A portability blocker on a target platform.
- A new Rust-native embedded graph engine with stable bindings.
