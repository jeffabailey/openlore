# ADR-031: htmx Asset Delivery — Vendored, `include_str!`-Embedded, Served from One Cached `GET /static/htmx.min.js` Route

- **Status**: Accepted / shipped (slice-07 viewer-htmx-swaps, DELIVER 2026-06-02)
- **Date**: 2026-06-02
- **Deciders**: Morgan (nw-solution-architect), per OD-HX-1 for viewer-htmx-swaps (slice-07). **The mechanism (vendored static route) is the USER DECISION; this ADR records it, its provenance, and the rejected alternatives.**
- **Feature**: viewer-htmx-swaps (slice-07)
- **Extends**: ADR-028 (viewer verb shape + loopback-only bind), ADR-029 (maud pure-core), ADR-007 (pure/effect split).
- **Resolves**: OD-HX-1 (htmx asset delivery mechanism + pinned version + provenance).

## Context

The slice-07 swaps require the htmx JavaScript library to be present in the browser
(it sets the `HX-Request` header and performs the in-place DOM swaps). The hard
invariant I-HX-2 (inheriting I-VIEW-6 / KPI-VIEW-5) is that the dashboard — including
its swaps — works **fully offline**: the asset must be served by the viewer process
itself, from a SINGLE source, with NO off-host (CDN) reference on any page. The viewer
is read-only and loopback-only (I-VIEW-1/4); whatever delivers the asset must not breach
either property.

The forces:

- **Offline-first (I-HX-2)**: a CDN `<script>` breaks every swap the moment the operator
  is offline — a silent regression of the slice-06 offline promise. Disqualifying.
- **Single source / no drift (BR-HX-6, US-HX-005 boundary)**: exactly one copy of the
  asset; no second copy that can drift to a different htmx version.
- **Pure-core untouched (ADR-007 / ADR-029)**: `viewer-domain` is pinned by
  `xtask check-arch` to deps exactly `{maud, maud_macros, ports}` (check_arch.rs:1208).
  The asset must NOT add a crate dependency to the pure core. The pure chrome only
  *references* the asset by URL (a plain `<script src>` string); it never embeds or
  loads bytes.
- **Cacheability**: the asset is static and immutable for a pinned version; it should be
  served with a long-lived cache header so the operator's browser fetches it once.
- **Read-only + loopback (I-HX-3 / I-VIEW-4)**: the asset route is GET-only, serves a
  fixed in-binary byte string, holds no key, and rides the existing loopback bind.

## Decision

**Vendor a pinned htmx ~2.x minified release as a checked-in repo file at
`crates/adapter-http-viewer/assets/htmx.min.js`, embed it into the binary at compile time
via `include_str!`, and serve it from ONE route `GET /static/htmx.min.js` with
`Content-Type: application/javascript; charset=utf-8` and a long-lived immutable cache
header. Every page's chrome references it via a single `<script src="/static/htmx.min.js"></script>`
tag emitted by the pure `viewer-domain` layout helper. NEVER a CDN.**

### Pinned version + provenance (recorded as required by OD-HX-1)

| Field | Value |
|-------|-------|
| Library | htmx |
| Pinned version | **2.0.4** (htmx ~2.x; the specific minified release) |
| Vendored file | `crates/adapter-http-viewer/assets/htmx.min.js` |
| Provenance | Upstream release artifact `htmx.min.js` for the pinned version, from the official htmx distribution (`https://github.com/bigskysoftware/htmx`, release `v2.0.4`). License: **0BSD** (BSD Zero Clause — permissive, no attribution requirement; compatible with OpenLore's OSS-first policy). |
| Integrity | A SHA-256 of the vendored bytes is recorded in the crate (a `const HTMX_SHA256: &str` next to the `include_str!`) and asserted in a unit test against the embedded bytes, so a silent swap of the vendored file is caught at build/test time. The exact released minified bytes are committed verbatim; no transform. |
| Embedding | `const HTMX_MIN_JS: &str = include_str!("../assets/htmx.min.js");` — a compile-time text embed of a LOCAL repo file. Adds NO crate dependency (it is not a `Cargo` dep; `xtask check-arch` is unaffected — see ADR-033 §enforcement). |

> The crafter MAY bump the patch within htmx 2.0.x if the pinned release is yanked; any
> bump updates BOTH the vendored file AND `HTMX_SHA256` in the same commit (the integrity
> test enforces they agree). A MAJOR bump (3.x) is a new ADR.

### Where it lives (pure/effect split preserved)

- **Effect shell (`adapter-http-viewer`)** OWNS the bytes (`include_str!`) and the route
  `GET /static/htmx.min.js` → `200 application/javascript` + cache header. This is an
  *asset route*, NOT a data route (it reads no store, touches no network, holds no key)
  — call this out explicitly against BR-HX-1 (which permits exactly this one new route).
- **Pure core (`viewer-domain`)** only emits the reference: the layout/`<head>` helper
  renders `<script src="/static/htmx.min.js"></script>` as ordinary markup. No bytes, no
  I/O, no new dep in the pure core.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **CDN `<script src="https://unpkg.com/htmx.org@2...">`** | Zero vendoring; always current. | **Hard reject (I-HX-2).** Breaks every swap offline; introduces an off-host reference (fails the no-CDN property of US-HX-005); ties the dashboard's behavior to a third party. Disqualified by the offline-first invariant before any other consideration. |
| **Inline the whole minified library into every page's `<head>`** (the USER-REJECTED alternative #1) | No extra route; trivially offline; single source if the constant lives in one place. | Rejected (user decision). Inlining ships the full library bytes in EVERY page response (the operator re-downloads it on every navigation; no browser caching of the script), bloating each full-page response and — critically — changing the **byte content of the slice-06 full pages**, which would breach I-HX-4 (non-htmx responses byte-equivalent to slice-06) unless carefully gated. A separate cached route keeps the script out of the page body, so the page chrome's only delta is a one-line `<script src>` tag, and the browser fetches the asset once. |
| **Hand-rolled / minimal custom swap JS** (the USER-REJECTED alternative #2) | No third-party asset at all; smallest possible bytes. | Rejected (user decision). Re-implements htmx's header-setting + swap + `hx-push-url` semantics by hand — a bug-prone, untested re-derivation of a mature, audited library for no benefit. htmx (0BSD, tiny, no transitive deps, widely used) is exactly the OSS-first choice; hand-rolling trades a vetted dependency for ongoing maintenance of subtle DOM/history behavior. |
| **Serve from a runtime filesystem path** (`tower-http::ServeDir` / read a file at request time) | Conventional static serving. | Rejected. Adds a runtime filesystem-I/O dependency + a shipped file artifact outside the binary (deployment fragility; the single-binary `openlore` packaging, ADR-011, ships no sidecar files). `include_str!` keeps the asset IN the binary — one artifact, no path resolution, no missing-file failure mode, offline by construction. (`tower`/`axum` are also `deny.toml`-banned.) |

## Consequences

### Positive
- Offline by construction: the asset is in the binary; no network, no CDN, no off-host
  reference on any page (US-HX-005 property holds structurally).
- Single source: one `const` + one route + one `<script src>` reference. No drift
  possible; the integrity test pins the exact bytes.
- The slice-06 full pages stay byte-equivalent except for one `<script src>` line in the
  shared chrome (I-HX-4 — see ADR-032 for how the chrome delta is bounded).
- Cacheable: the browser fetches `htmx.min.js` once (immutable cache header); subsequent
  navigations and swaps reuse the cached script.
- Pure core untouched: `viewer-domain` deps stay `{maud, maud_macros, ports}`; the asset
  is an effect-shell concern.
- Read-only + loopback preserved: the asset route is GET-only, serves fixed bytes, holds
  no key, rides the existing loopback bind (I-HX-3 / I-VIEW-4).

### Negative
- A vendored binary-ish text file lives in the repo and must be bumped manually on a
  security advisory. Accepted: the integrity `const` + test make a bump a single auditable
  commit; htmx 2.x is stable and small.
- One new route on the surface. Accepted and bounded: BR-HX-1 explicitly permits exactly
  one new asset route; it is annotated as an asset (not data) route in the route table.

## Revisit Trigger
- htmx security advisory or yank of the pinned release → bump version + SHA in one commit.
- A second asset (CSS file, second script) is needed → generalize the single `/static/...`
  route into a tiny static table (still in-binary, still one source per asset).
- htmx 3.x migration → new ADR (breaking attribute/behavior changes).
