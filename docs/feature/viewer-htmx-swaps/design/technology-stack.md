# Technology Stack: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06 technology-stack.** slice-07 adds exactly ONE new technology — the
> **htmx** client library — delivered as a VENDORED static text asset (not a crate). No new
> Rust crate dependency; no change to the workspace dep graph; `deny.toml` unaffected. The
> server stack (hyper, maud, tokio, DuckDB) is reused verbatim from slice-06. OSS-first
> policy honored: htmx is permissive-licensed, tiny, mature, and served locally (no CDN).

## 1. New technology: htmx (client-side, vendored)

| Field | Value |
|---|---|
| Name / version | **htmx 2.0.4** (pinned; htmx ~2.x minified release) |
| Role | Client-side progressive-enhancement library: sets the `HX-Request` header on swap-driven requests and performs in-place DOM swaps + history (`hx-get`/`hx-post`/`hx-target`/`hx-swap`/`hx-push-url`). |
| License | **0BSD** (BSD Zero Clause License — permissive, no attribution requirement). Compatible with OpenLore's OSS-first policy; sits above MIT/Apache in permissiveness (MIT > Apache 2.0 > BSD … — 0BSD is at least as permissive as MIT). |
| Maintenance | Mature, widely adopted, actively maintained (bigskysoftware/htmx); large community; no transitive runtime deps; single self-contained minified file. |
| Provenance | Upstream official release artifact `htmx.min.js` for v2.0.4 (`https://github.com/bigskysoftware/htmx`, release `v2.0.4`). |
| Delivery | **Vendored** at `crates/adapter-http-viewer/assets/htmx.min.js`; embedded in the binary via `include_str!`; served from ONE cached route `GET /static/htmx.min.js`. **NEVER a CDN** (I-HX-2 / offline-first). |
| Integrity | SHA-256 of the vendored bytes recorded as `const HTMX_SHA256` next to the `include_str!` and asserted against the embedded bytes in a unit test (a silent file swap fails the build/test). |
| Crate dependency? | **NO.** htmx is a static TEXT asset embedded via `include_str!` — it is not a `Cargo.toml` dependency, adds nothing to the workspace dep graph, and does not touch the pure-core allowlist or `deny.toml`. |

Decision recorded in **ADR-031** (vendored asset, alternatives: CDN / inline-in-page /
hand-rolled JS / runtime-filesystem serving — all rejected).

## 2. Reused server stack (UNCHANGED from slice-06)

| Technology | Version (slice-06) | Role | License | slice-07 use |
|---|---|---|---|---|
| `hyper` + `hyper-util` + `http-body-util` | 1.x | Hand-rolled HTTP server (axum/actix banned) | MIT | + reads `HX-Request`; + serves `GET /static/htmx.min.js`. No version change. |
| `maud` | (slice-06 pin) | Compile-time HTML macro, PURE core (ADR-029) | MIT | + public `render_*_fragment` fns + chrome `<script src>` + tab `hx-*` attrs. No version change. |
| `tokio` | (workspace pin) | Reused current-thread runtime (composition root) | MIT | UNCHANGED. |
| `duckdb` via `adapter-duckdb` | (workspace pin) | Read-only store reads | MIT | UNCHANGED (peer paging reuses existing `list_peer_claims`). |
| `reqwest` (via `adapter-github`) | (workspace pin) | Live `/scrape` harvest only | MIT/Apache-2.0 | UNCHANGED. |
| `thiserror`, `serde`/`serde_json` | (workspace pin) | errors / form + event JSON | MIT/Apache-2.0 | UNCHANGED. |
| `proptest` (dev), `reqwest::blocking` (test) | — | pure-core property tests / AT HTTP harness | MIT/Apache-2.0 | + `get_htmx`/`post_form_htmx` (ADR-035). |

## 3. Why htmx (not alternatives) — quality-attribute rationale

The driver is **progressive enhancement with offline-first delivery** (I-HX-1 / I-HX-2) on a
**localhost, single-operator, read-only** surface, in a **functional-Rust, server-rendered**
codebase that bans heavy web frameworks. htmx fits because:

- **Server-rendered fit**: htmx swaps server-rendered HTML fragments — exactly the
  `render_*_fragment` outputs (ADR-032). No client-side data model, no build step, no
  bundler, no SPA framework. The server stays the source of truth (parity is server-side).
- **Tiny + self-contained**: one minified file, no transitive runtime deps — trivially
  vendorable + embeddable via `include_str!` (offline by construction, I-HX-2).
- **Progressive enhancement by design**: htmx ENHANCES real anchors/forms (which carry `href`/
  `action`), so the no-JS path is the slice-06 full-page experience untouched (I-HX-1).
- **Permissive license** (0BSD), mature, widely used — OSS-first satisfied.

Alternatives (recorded in ADR-031): a CDN (breaks offline), inlining into every page (bloats
responses + risks the I-HX-4 byte-equivalence), hand-rolled swap JS (re-derives a mature
library — bug-prone), and a JS framework / SPA (massive complexity for a localhost read-only
dashboard — resume-driven, rejected). A full client framework would also fight the
server-rendered, pure-core architecture.

## 4. Enforcement / governance

- **`deny.toml`**: UNCHANGED. `axum`/`actix-web` stay banned; htmx is not a crate, so it is
  not subject to crate-license review beyond recording the vendored file's 0BSD license + SHA
  here and in ADR-031.
- **`xtask check-arch`**: UNCHANGED. `viewer-domain` deps stay `{maud, maud_macros, ports}`;
  the asset is an `include_str!` text embed in the effect shell (no dep edge). The viewer
  capability boundary (only `cli` links `adapter-http-viewer`; no signing/PDS surface) holds.
- **Integrity test**: `assert_eq!(sha256(HTMX_MIN_JS), HTMX_SHA256)` pins the vendored bytes.
- **No-CDN property** (US-HX-005): a test asserts no served page references an off-host htmx
  URL; the only htmx reference is `src="/static/htmx.min.js"`.
