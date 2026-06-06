# Technology Stack — viewer-graph-traversal (slice-10)

> **UNCHANGED — the slice reuses the existing viewer stack.** No new technology,
> no new external dependency, no new crate. This document records the
> already-adjudicated choices the slice rides on (OSS-first, licenses pinned by
> the existing `deny.toml`) so the reuse decision is explicit.

## No new technology selected

slice-10 is a brownfield DELTA that adds two read methods + two routes + render
functions to the existing read-only viewer. Every tool it uses was selected and
ADR'd in an earlier slice:

| Capability | Technology | License | Where chosen | slice-10 use |
|---|---|---|---|---|
| HTTP server (effect shell) | `hyper` 1.x (hand-rolled, NOT axum) | MIT | ADR-028 (slice-06) | the two new GET route arms |
| HTML templating (pure core) | `maud` (compile-time macro) | MIT | ADR-029 (slice-06) | `render_project_*` / `render_philosophy_*` |
| Local store (driven adapter) | `duckdb` | MIT | ADR-001 | the two survey SELECTs (read-only) |
| Progressive enhancement | vendored `htmx` 2.0.4 (SHA-256-pinned local asset) | 0BSD | ADR-031 (slice-07) | the cross-links' optional `hx-get` swap |
| Async runtime | `tokio` | MIT | ADR-004 | (the accept loop; the new handlers are sync) |
| Display-only bucket | `claim-domain::confidence_bucket` (in-workspace, PURE) | (workspace) | WD-10 / D-12 | REUSED for the edge bucket label |
| Scoring (link-out target) | `scoring` (in-workspace, PURE) | (workspace) | ADR-022 | `/score` is the contributor traversal target (REUSED, not rebuilt) |
| Architecture enforcement | `cargo xtask check-arch` (bespoke; `syn` + `cargo_metadata`) | (workspace) | ADR-009 | one allowlist edge added |

## OSS-first / proprietary check

PASS — no proprietary technology is introduced. Every external crate is already
in the workspace and pinned by `deny.toml`; `cargo deny` needs no change
(claim-domain / ports / maud are all in-workspace). No new license to adjudicate.

## Architecture enforcement tooling (language-appropriate)

The project's standing enforcement tool is **`cargo xtask check-arch`** — a
bespoke Rust ArchUnit-equivalent that combines an import-graph pass (over
`cargo metadata`) with `syn`-AST source rules (the anti-merging SQL scan, the
cfg-gate scans). `import-linter`-style import-graph-only tooling was rejected
project-wide (it cannot express the method-presence / SQL-literal rules this
codebase enforces) — consistent with Earned-Trust principle 12. slice-10 adds
exactly ONE allowlist edge (`viewer-domain → claim-domain`); the capability and
anti-merging rules are unchanged (ADR-045).

## Confirmation

- **No new crate.** Workspace stays **21 members**.
- **No new external dependency.** `cargo deny` unchanged.
- **No new persisted type / schema.** Surveys computed per query.
