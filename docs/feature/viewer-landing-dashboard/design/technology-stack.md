# Technology Stack: viewer-landing-dashboard (slice-17)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-054

## Unchanged — slice-17 introduces NO new technology

slice-17 is a brownfield DELTA on the established viewer stack (slices 06–16). It
adds NO dependency, NO library, NO service, NO build tooling. Every choice below is
inherited and already vendored/locked in the workspace.

| Concern | Technology | License | Status |
|---|---|---|---|
| Language / paradigm | Rust, functional (ADR-007: pure core, effect shell) | — | inherited |
| HTTP server (effect shell) | `hyper` (`adapter-http-viewer`) | MIT | inherited |
| HTML render (pure core) | `maud` (`viewer-domain`) | MIT/Apache-2.0 | inherited |
| LOCAL store | DuckDB via `duckdb` crate (`adapter-duckdb`), shared connection (BR-VIEW-4) | MIT (DuckDB) | inherited |
| Progressive enhancement | vendored `/static/htmx.min.js` (no CDN — ADR-031) | BSD-2-Clause (htmx) | inherited; `/` references it via chrome but does NOT fork by Shape (ADR-054 D5) |
| Architecture rules | `xtask check-arch` (workspace-local) | — | inherited; UNCHANGED for this slice |

## Rationale

- **OSS-first, all permissive licenses** (MIT / Apache-2.0 / BSD): no proprietary
  dependency; nothing added this slice.
- **No new tech is the correct choice**: the slice extends one pure render fn, threads
  an already-held store into one handler, and adds one read-only `COUNT(*)`. Adding a
  templating engine, a client framework, or a metrics library would be resume-driven
  over-engineering for a three-count summary + a link list. Rejected on simplest-
  solution-first.
- **Architectural enforcement tooling** stays the existing `xtask check-arch`
  (language-appropriate, workspace-local; the read-only viewer-capability rule, the
  pure-core-no-io rule, the ports-async-trait rule). No new enforcement tool needed;
  the read-only invariant's three layers (type + xtask + behavioral gold) are already
  in place (ADR-054 Enforcement).

## No external integration

No third-party API, webhook, OAuth provider, or cross-team internal API is consumed
by this slice. No contract-testing tool (Pact, etc.) applies — the only dependency is
the LOCAL read-only DuckDB store.
