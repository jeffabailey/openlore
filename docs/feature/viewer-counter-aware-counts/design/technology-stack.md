# Technology Stack: viewer-counter-aware-counts (slice-18)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-055

## Unchanged — no new technology, no new dependency

slice-18 is a thin brownfield DELTA on the slice-17 orientation surfaces. It introduces NO new
crate, NO new dependency, NO new external integration, and NO new technology. The entire stack is
inherited verbatim from slices 06–17:

| Concern | Technology | License | Status |
|---|---|---|---|
| HTTP server (effect shell) | `hyper` 1.x (hand-rolled minimal server, loopback-only; `axum` banned via `deny.toml`) | MIT | UNCHANGED (slice-06) |
| HTML render (pure core) | `maud` (compile-time templates) | MIT/Apache-2.0 | UNCHANGED (slice-06) |
| Progressive enhancement | vendored `/static/htmx.min.js` (offline-first, NO CDN — ADR-031) | (vendored) | UNCHANGED; the countered count is full-page chrome, no new htmx |
| LOCAL store | DuckDB via `duckdb` crate, read-only `StoreReadPort` over the shared connection | MIT (duckdb) | UNCHANGED (slice-03/06) |
| Async runtime | `tokio` (loopback listener) | MIT | UNCHANGED |
| Arch enforcement | `xtask` check-arch (`no_cross_table_join_elides_author`, `check_viewer_capability_boundary`) | (workspace) | UNCHANGED — both rules GREEN by construction (see component-boundaries §5) |

## Paradigm

Functional Rust (ADR-007): the one new read (`count_countered_own_claims`) is an effect at the
I/O edge (`adapter-duckdb`); the one new render helper (`render_countered`) + the extended
`render_landing`/`render_claims_page` are PURE total functions in `viewer-domain`. The shell
(`adapter-http-viewer`) is the SANDWICH — read (impure) → build the `Option<usize>` (pure) →
render (pure). No new ADT is warranted: the countered count is a fourth parallel `Option<usize>`
(ADR-055 D2), identical in shape and degrade to the slice-17 three.

## Arch-enforcement tooling (recommendation — already in place)

No new enforcement tooling is needed. The slice's invariants are covered by the EXISTING
language-appropriate xtask rules:

- Read-only viewer capability: `check_viewer_capability_boundary` (crate-dependency-graph rule) +
  the `StoreReadPort` no-mutation-method type layer + a behavioral gold (no mutating control on
  `/` or `/claims`). Three orthogonal layers, unchanged.
- Anti-merging SQL: `no_cross_table_join_elides_author` (AST + word-boundary classifier over
  `adapter-duckdb` SQL literals). GREEN by construction for the new count (component-boundaries
  §5).

## OSS posture

All dependencies are mature OSS (MIT / Apache-2.0). No proprietary technology. No new dependency
of any license is added by this slice.
