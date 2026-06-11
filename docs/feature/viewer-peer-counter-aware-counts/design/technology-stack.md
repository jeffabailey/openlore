# Technology Stack: viewer-peer-counter-aware-counts (slice-19)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-10 · ADR: ADR-056

## Unchanged — no new technology, no new dependency

slice-19 is a thin brownfield DELTA on the slice-17/18 orientation surfaces — the deferred peer
sibling of slice-18. It introduces NO new crate, NO new dependency, NO new external integration,
and NO new technology. The entire stack is inherited verbatim from slices 06–18:

| Concern | Technology | License | Status |
|---|---|---|---|
| HTTP server (effect shell) | `hyper` 1.x (hand-rolled minimal server, loopback-only; `axum` banned via `deny.toml`) | MIT | UNCHANGED (slice-06) |
| HTML render (pure core) | `maud` (compile-time templates) | MIT/Apache-2.0 | UNCHANGED (slice-06) |
| Progressive enhancement | vendored `/static/htmx.min.js` (offline-first, NO CDN — ADR-031) | (vendored) | UNCHANGED; the countered-peer count is full-page chrome, no new htmx |
| LOCAL store | DuckDB via `duckdb` crate, read-only `StoreReadPort` over the shared connection | MIT (duckdb) | UNCHANGED (slice-03/06) |
| Async runtime | `tokio` (loopback listener) | MIT | UNCHANGED |
| Arch enforcement | `xtask` check-arch (`no_cross_table_join_elides_author`, `check_viewer_capability_boundary`, the `VIEWER_FAIL_SEAM_TOKENS` guard) | (workspace) | UNCHANGED rules; the fault-seam guard gains ONE token entry (a data-only addition to the existing `VIEWER_FAIL_SEAM_TOKENS` array — see component-boundaries §6) |

## Paradigm

Functional Rust (ADR-007): the one new read (`count_countered_peer_claims`) is an effect at the
I/O edge (`adapter-duckdb`); the render is the EXISTING pure `render_countered` helper +
the extended `render_landing` / `render_peer_claims_page` PURE total functions in `viewer-domain`.
The shell (`adapter-http-viewer`) is the SANDWICH — read (impure) → build the `Option<usize>`
(pure) → render (pure). No new ADT is warranted: the countered-peer count is a fifth parallel
`Option<usize>` (ADR-056 D2), identical in shape and degrade to the slice-17/18 four.

## Arch-enforcement tooling (recommendation — already in place)

No new enforcement tooling is needed. The slice's invariants are covered by the EXISTING
language-appropriate xtask rules:

- Read-only viewer capability: `check_viewer_capability_boundary` (crate-dependency-graph rule) +
  the `StoreReadPort` no-mutation-method type layer + a behavioral gold (no mutating control on
  `/` or `/peer-claims`). Three orthogonal layers, unchanged.
- Anti-merging SQL: `no_cross_table_join_elides_author` (AST + word-boundary classifier over
  `adapter-duckdb` SQL literals). GREEN by construction for the new count, VERIFIED against the
  classifier source (component-boundaries §5 — R-PC-9 RESOLVED).
- Fault-seam release-build guard: `scan_viewer_fail_seam_guard` + the `VIEWER_FAIL_SEAM_TOKENS`
  token set — the new `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` token is APPENDED to the
  existing array so the ONE guard covers it (the 4th token, component-boundaries §6).

## OSS posture

All dependencies are mature OSS (MIT / Apache-2.0). No proprietary technology. No new dependency
of any license is added by this slice.
</content>
