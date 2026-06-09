# Technology Stack — viewer-search-follow-state (slice-16)

> **UNCHANGED — the slice reuses the existing viewer stack.** No new technology,
> no new external dependency, no new crate. This document records the
> already-adjudicated choices the slice rides on (OSS-first, licenses pinned by
> the existing `deny.toml`) so the reuse decision is explicit.

## No new technology selected

slice-16 is a brownfield DELTA that threads ONE already-existing LOCAL read into
the existing `/search` resolution and adds ONE render arm. Every tool it uses was
selected and ADR'd in an earlier slice:

| Capability | Technology | License | Where chosen | slice-16 use |
|---|---|---|---|---|
| HTTP server (effect shell) | `hyper` 1.x (hand-rolled, NOT axum) | MIT | ADR-028 (slice-06) | the existing `GET /search` handler (UNCHANGED route) |
| HTML templating (pure core) | `maud` (compile-time macro) | MIT | ADR-029 (slice-06) | the new `render_following_indicator` arm + the unchanged `render_follow_guidance` |
| Local store (driven adapter) | `duckdb` | MIT | ADR-001 | the REUSED slice-15 active-subscription SELECT (read-only, ONE aggregate) |
| Network index query | `IndexQueryPort` (reqwest-backed adapter) | MIT/Apache-2.0 | ADR-037 (slice-08) | UNCHANGED — the result rows; per-user-neutral |
| In-memory set | `std::collections::HashSet` | (std, no license to adjudicate) | (std) | the active-DID set the resolution checks membership against |
| Progressive enhancement | vendored `htmx` 2.0.4 (SHA-256-pinned local asset) | 0BSD | ADR-031 (slice-07) | UNCHANGED — the `#search-results` swap + the `Shape` fork |
| Async runtime | `tokio` | MIT | ADR-004 | UNCHANGED — `resolve_search_state` is `async` (the index `.await`) |
| Architecture enforcement | `cargo xtask check-arch` (bespoke; `syn` + `cargo_metadata`) | (workspace) | ADR-009 | UNCHANGED (no delta) |

## OSS-first / proprietary check

PASS — no proprietary technology is introduced. The only "new" type is
`std::collections::HashSet` (the standard library — no external crate, no license
to adjudicate). Every other tool is already in the workspace and pinned by
`deny.toml`; `cargo deny` needs no change. No new license to adjudicate.

## Architecture enforcement tooling (language-appropriate)

The project's standing enforcement tool is **`cargo xtask check-arch`** — a
bespoke Rust ArchUnit-equivalent that combines an import-graph pass (over
`cargo metadata`) with `syn`-AST source rules (the anti-merging SQL scan, the
cfg-gate scans). `import-linter`-style import-graph-only tooling was rejected
project-wide (it cannot express the method-presence / SQL-literal / render-only
rules this codebase enforces) — consistent with Earned-Trust principle 12.

slice-16 adds **NO delta** to `check-arch`:
- the viewer capability rule (`VIEWER_FORBIDDEN_DEPS`) is UNCHANGED — resolution
  reuses the `StoreReadPort` the viewer already holds; no
  signing/identity/PDS/indexer-mutation surface is touched;
- the pure-core no-I/O arm for `viewer-domain` is UNCHANGED — NO new dependency
  edge (the new `SubscribedPeer` render arm is a total fn of the existing
  `appview_domain::NetworkResultRow`);
- the anti-merging SQL rule (`no_cross_table_join_elides_author`) is N/A —
  slice-16 adds NO SQL (it reuses the slice-15 active-subscription query
  verbatim).

## Earned-Trust (principle 12) — probe posture

No new adapter or port with its own substrate dependency is introduced. The
REUSED `list_active_peer_subscriptions` read runs over the EXISTING,
already-probed `StoreReadPort` DuckDB connection (the store-readability startup
probe of ADR-028/030 — "wire then probe then use" — already gates the viewer's
startup). The substrate "lie" slice-16 must survive is a **mid-request read
FAILURE** — exercised by the behavioral degrade-gracefully gold (a failed
active-set read → all-`NetworkUnfollowed`, no crash). No new external dependency
that could lie is added, so no new `probe()` scenario is required; `cargo xtask
check-probes` is UNCHANGED.

## Confirmation

- **No new crate.** Workspace stays **21 members**.
- **No new external dependency.** `HashSet` is std; `cargo deny` unchanged.
- **No new persisted type / schema.** The active set is read + resolved per request.
- **No new route, no new read method, no new `AuthorRelationship` variant.**
- **No xtask check-arch delta.** Reused read; no new edge, no new forbidden dep,
  no new SQL.
