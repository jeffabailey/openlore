# Technology Stack: htmx-scraper-viewer (slice-06)

> **DELTA** on the existing OpenLore stack. The viewer adds exactly ONE new
> dependency (`maud`); everything else is reused from slices 01–05. Open-source-first:
> every choice is OSS with a documented license. Proprietary: none.

## New dependency (one)

| Crate | Version | License | Role | Maintenance | Why / alternatives |
|-------|---------|---------|------|-------------|--------------------|
| `maud` | 0.26.x | MIT | Compile-time HTML templating macro in the PURE `viewer-domain` crate (ADR-029) | Mature, widely used, actively maintained | OD-VIEW-1 resolved to maud: compile-time-checked, zero runtime I/O (so it lives in the pure core), auto-escaping, single small dep. Rejected: `askama` (ships template files + build step), `tera`/`handlebars` (runtime template I/O — breaks ADR-007 pure core), hand-rolled `format!` (no structural safety). See ADR-029. |

`maud` is added to `xtask check-arch::PURE_CORE_ALLOWED_CRATES` (ADR-029), mirroring the
slice-02 `serde_yaml_ng` adjudication — the explicit pure-core allowlist entry (WD-35).

## Reused dependencies (no new addition)

| Crate / facility | Source slice | Role in slice-06 | License |
|------------------|--------------|------------------|---------|
| `hyper` 1.x (`hyper`, server + http1) | slice-05 `adapter-xrpc-query-server` | The viewer's HTTP server (hand-rolled, ADR-028 / OD-VIEW-2) | MIT |
| `hyper-util` (`TokioIo`) | slice-05 | tokio adapter for hyper IO | MIT |
| `http-body-util` (`Full`, `BodyExt`) | slice-05 | request/response body handling | MIT |
| `tokio` (current-thread runtime) | slice-01 `verbs::claim_publish::build_tokio_runtime` | Runs the serve loop + the `/scrape` async harvest; reuses the EXACT runtime builder the CLI already uses | MIT |
| `duckdb` | slice-01 `adapter-duckdb` | Read-only paginated reads over the SAME store (ADR-030); no new schema | MIT |
| `adapter-github` (`GithubPort`, `reqwest`/rustls) | slice-02 | Live `/scrape` harvest + pure `scraper-domain::derive_candidates` | MIT/Apache |
| `serde` / `serde_json` | workspace | Boundary value types | MIT/Apache |
| `chrono` | workspace | `composed_at` / `fetched_at` timestamp rendering | MIT/Apache |
| `clap` | slice-01 cli | The `ui` subcommand + `--port` flag | MIT/Apache |
| `thiserror` | workspace | `StoreReadError` typed errors | MIT/Apache |

## Banned / explicitly NOT used

| Tech | Status | Reason |
|------|--------|--------|
| `axum` | **BANNED** (`deny.toml`) | OD-VIEW-2: the HTTP layer is hyper hand-rolled per the slice-05 precedent (ADR-028). |
| `actix-web` | **BANNED** (`deny.toml`) | Same as axum. |
| `tera` / `handlebars` (runtime template engines) | not used | Runtime template I/O breaks the ADR-007 pure-core constraint (ADR-029). |
| Any auth / session / JWT crate | not used | OD-VIEW-7: loopback-only, no auth (ADR-028) — no credential store on purpose. |
| Any TLS termination crate for the listener | not used | Loopback-only plaintext; off-host access (with TLS) is a future slice + ADR. |
| Any second datastore | not used | BR-VIEW-4: the viewer reads the SAME DuckDB the CLI writes; no second store. |
| A client-side JS framework / build toolchain | not used | Server-rendered maud HTML; htmx progressive enhancement (if added) is partial-swap markup, no build step. |

## Dependency-surface impact

- **New crates**: `viewer-domain` (pure; deps: `maud`, `ports`, `serde`, `chrono`),
  `adapter-http-viewer` (effect; deps: `hyper`, `hyper-util`, `http-body-util`, `tokio`,
  `ports`, `viewer-domain`).
- **CLI binary**: gains the `hyper`/`maud` reachable surface (hyper already transitively
  present via the workspace; maud is the one genuinely new MIT crate). Bounded and
  documented.
- **Production crate count**: 19 → 21 (confirmed, ADR-028).

## License posture

All dependencies are MIT or Apache-2.0 (preferred tiers). No GPL/AGPL, no proprietary.
The single new direct dependency (`maud`, MIT) is the lightest viable option that keeps
rendering in the pure core.
