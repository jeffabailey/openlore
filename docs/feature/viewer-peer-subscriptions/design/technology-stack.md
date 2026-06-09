# Technology Stack — viewer-peer-subscriptions (slice-15)

> **UNCHANGED — the slice reuses the existing viewer stack.** No new technology,
> no new external dependency, no new crate. This document records the
> already-adjudicated choices the slice rides on (OSS-first, licenses pinned by
> the existing `deny.toml`) so the reuse decision is explicit.

## No new technology selected

slice-15 is a brownfield DELTA that adds ONE read method + ONE route + render
functions to the existing read-only viewer. Every tool it uses was selected and
ADR'd in an earlier slice:

| Capability | Technology | License | Where chosen | slice-15 use |
|---|---|---|---|---|
| HTTP server (effect shell) | `hyper` 1.x (hand-rolled, NOT axum) | MIT | ADR-028 (slice-06) | the new `GET /peers` route arm + handler |
| HTML templating (pure core) | `maud` (compile-time macro) | MIT | ADR-029 (slice-06) | `render_peers_fragment` / `render_peers_page` / `render_remove_guidance` |
| Local store (driven adapter) | `duckdb` | MIT | ADR-001 | the active-subscription survey SELECT (read-only, ONE aggregate) |
| Date/time | `chrono` | MIT/Apache-2.0 | (workspace) | `PeerSubscriptionSummary.subscribed_at` (already a `peer_subscriptions` column type) |
| Progressive enhancement | vendored `htmx` 2.0.4 (SHA-256-pinned local asset) | 0BSD | ADR-031 (slice-07) | the `/peers` nav link's optional `hx-get` swap + the `Shape` fork |
| Async runtime | `tokio` | MIT | ADR-004 | (the accept loop; the new handler is sync) |
| Architecture enforcement | `cargo xtask check-arch` (bespoke; `syn` + `cargo_metadata`) | (workspace) | ADR-009 | UNCHANGED (no delta) |

## OSS-first / proprietary check

PASS — no proprietary technology is introduced. Every external crate is already
in the workspace and pinned by `deny.toml`; `cargo deny` needs no change
(`ports` / `maud` are in-workspace; `chrono` / `duckdb` already pinned). No new
license to adjudicate.

## Architecture enforcement tooling (language-appropriate)

The project's standing enforcement tool is **`cargo xtask check-arch`** — a
bespoke Rust ArchUnit-equivalent that combines an import-graph pass (over
`cargo metadata`) with `syn`-AST source rules (the anti-merging SQL scan, the
cfg-gate scans). `import-linter`-style import-graph-only tooling was rejected
project-wide (it cannot express the method-presence / SQL-literal rules this
codebase enforces) — consistent with Earned-Trust principle 12.

slice-15 adds **NO delta** to `check-arch`:
- the viewer capability rule (`VIEWER_FORBIDDEN_DEPS`) is UNCHANGED — the read
  touches no signing/identity/PDS/indexer surface;
- the pure-core no-I/O arm for `viewer-domain` is UNCHANGED — NO new dependency
  edge (the render is a total fn of the flat DTO, unlike slice-10's
  `viewer-domain → claim-domain` allowlist edge);
- the anti-merging SQL rule (`no_cross_table_join_elides_author`) stays GREEN by
  construction — the new SQL names `peer_subscriptions` + `peer_claims` (not the
  standalone `claims` table), so the classifier returns `None`.

## Earned-Trust (principle 12) — probe posture

No new adapter or port with its own substrate dependency is introduced. The new
`list_active_peer_subscriptions` read runs over the EXISTING, already-probed
`StoreReadPort` DuckDB connection (the store-readability probe of ADR-028/030 —
"wire then probe then use" — already gates the viewer's startup). The read is a
pure `SELECT` over tables the migrations already create; there is no new external
dependency that could lie, so no new `probe()` scenario is required. `cargo xtask
check-probes` is UNCHANGED.

## Confirmation

- **No new crate.** Workspace stays **21 members**.
- **No new external dependency.** `cargo deny` unchanged.
- **No new persisted type / schema.** The subscription survey is computed per request.
- **No xtask check-arch delta.** Read-only DB read; no new edge, no new forbidden dep.
