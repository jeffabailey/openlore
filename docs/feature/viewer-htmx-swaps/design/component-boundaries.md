# Component Boundaries: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06 component-boundaries.** ZERO new crates. Three EXTENDED crates
> (`viewer-domain`, `adapter-http-viewer`, the `tests/acceptance/support` harness) + ZERO
> change to `ports`/`adapter-duckdb`/`cli`/`xtask`. Functional Rust (ADR-007): the pure
> rendering core gains fragment renderers (still pure); the effect shell gains a header
> read + an asset route. Governed by ADR-031/032/033/034/035 on top of ADR-028/029/030.

## Crate layout (production crates 21 → 21 — no new crate)

```
crates/
  viewer-domain/          # EXTEND — PURE: public render_*_fragment + ID_* consts + chrome <script src> + tab htmx attrs
  adapter-http-viewer/    # EXTEND — EFFECT: Shape dispatch + GET /static/htmx.min.js (include_str!) + assets/htmx.min.js
  ports/                  # UNCHANGED
  adapter-duckdb/         # UNCHANGED
  cli/                    # UNCHANGED (the `ui` verb wiring is identical)
xtask/                    # UNCHANGED (htmx is a text asset, not a crate dep)
tests/acceptance/support/ # EXTEND — add get_htmx / post_form_htmx (ADR-035)
```

Reuse-over-new justification: every slice-07 capability is a SHAPE of existing slice-06
content over existing routes (I-HX-1 / BR-HX-1). No new domain, no new port, no new store,
no new external system → no new crate earns its existence. The work is an extract-and-compose
refactor in the pure core + a header fork + one asset route in the shell.

---

## `crates/viewer-domain` (EXTEND, PURE)

**slice-07 responsibility delta**: expose each region as a PUBLIC fragment renderer carrying
its swap-target `id`, and re-define each whole-page renderer to COMPOSE that same fragment fn
(ADR-032). Emit the htmx `<script src>` line in the chrome `<head>` and the tab anchors'
htmx attributes (ADR-031/034). Stay PURE + header-unaware.

**New/changed public surface (function-signature ports — fragments ARE the contract):**

- Swap-target id constants (one per region, shared by fragment + page — ADR-032):
  - `ID_CLAIMS_TABLE: &str = "claims-table"`
  - `ID_SCRAPE_RESULTS: &str = "scrape-results"`
  - `ID_CLAIM_DETAIL: &str = "claim-detail"`
  - `ID_VIEW_PANEL: &str = "view-panel"`
- Public fragment renderers (each wraps its region in `div id=(ID_*)`):
  - `render_claims_table_fragment(&PageView<ClaimRowView>) -> Markup`
  - `render_peer_claims_table_fragment(&PageView<PeerClaimRowView>) -> Markup`
  - `render_scrape_results_fragment(&ScrapeState) -> Markup`
  - `render_claim_detail_fragment(&ClaimDetailView) -> Markup`
  - `render_claim_not_found_fragment() -> Markup` (the guided not-found region for `#claim-detail`)
- Re-defined page renderers (SAME signatures as slice-06; now compose the fragment fn):
  - `render_claims_page = chrome + render_claims_table_fragment` (+ pagination INSIDE the fragment)
  - `render_peer_claims_page = chrome + render_peer_claims_table_fragment` (inside `#view-panel`)
  - `render_scrape_page = chrome + form + render_scrape_results_fragment`
  - `render_claim_detail = chrome + render_claim_detail_fragment`
  - `render_error` → composes `render_claim_not_found_fragment` for the full page
- Chrome/layout helper additions (pure markup strings — no I/O, no bytes):
  - the `<head>` emits `<script src="/static/htmx.min.js"></script>` (single site; ADR-031)
  - the tab anchors emit `href` + `hx-get`/`hx-target="#view-panel"`/`hx-swap`/`hx-push-url="true"` (ADR-034)
  - the Prev/Next/detail/scrape triggers emit `href`/`action` + their `hx-*` attrs per
    architecture-design.md §6

**Dependencies**: UNCHANGED — `{maud, maud_macros, ports}` only. Fragment renderers are pure
maud; the htmx attributes + `<script src>` are literal attribute/text strings (no new dep,
no bytes embedded here). `xtask check-arch` (check_arch.rs:1208) still passes verbatim.

**Invariants enforced here:**
- **I-HX-5 (parity)**: the page CALLS the fragment fn — there is no second region renderer
  to drift. Structural.
- **FR-VIEW-8 (confidence verbatim)**: `render_confidence` (one site) renders inside the
  fragment, so both shapes show the identical numeric.
- **I-VIEW-5 / WD-62 (derived-from)**: `derived_from` stays ONLY on `CandidateRowView`,
  rendered ONLY by `render_scrape_results_fragment` — the persisted-view fragments have no
  derived-from slot (type-level, unchanged).
- **BR-VIEW-1 / I-SCR-1 (no sign control)**: the scrape results fragment renders NO sign
  control (unchanged — the fragment is the slice-06 results markup, moved into a `div id`).
- **NFR-HX-8 (a11y)**: fragments keep semantic HTML; the no-JS path keeps real anchors/forms.

**Boundary**: registered in `xtask check-arch` as pure-core (I/O ban list applies; unchanged).

---

## `crates/adapter-http-viewer` (EXTEND, EFFECT)

**slice-07 responsibility delta**: read `HX-Request` once per request → `Shape`; fork each
handler's render call between fragment and page (ADR-033); serve the vendored htmx asset
(ADR-031). NO new capability, NO new port, NO new write/sign surface.

**New/changed surface:**
- `assets/htmx.min.js` — the vendored htmx 2.0.4 minified release (0BSD), checked into the
  repo under the crate. Embedded: `const HTMX_MIN_JS: &str = include_str!("../assets/htmx.min.js");`
  + `const HTMX_SHA256: &str = "<sha256 of the bytes>";` (integrity, asserted in a unit test).
- `enum Shape { Fragment, FullPage }` + `Shape::from_request(&Request<_>) -> Shape` (the ONE
  `HX-Request` read site; presence → Fragment).
- `fn serve_htmx_asset() -> Response<Full<Bytes>>` — `200`, `Content-Type:
  application/javascript; charset=utf-8`, a long-lived immutable `Cache-Control` header,
  body = `HTMX_MIN_JS` bytes. Ignores `Shape`. Asset route, NOT a data route.
- `route` gains: derive `shape` once; add the `"/static/htmx.min.js"` GET arm (before the
  data arms); thread `shape` into `claims_page` / `peer_claims_page` / `claim_detail_page` /
  `scrape_get` / `scrape_post`.
- Each handler gains a `shape: Shape` param and a `match shape { Fragment => render_*_fragment(...).into_string(), FullPage => render_*_page(...) }` at its render call. The
  read/project/clamp/error/status logic is slice-06-UNCHANGED.

**Existing handlers — the bounded slice-07 diff** (against `adapter-http-viewer/src/lib.rs`):
- `claims_page(store, query)` → `claims_page(store, query, shape)`: build the SAME
  `PageView`, then fork the render. (Clamp, degrade-to-empty, status unchanged.)
- `peer_claims_page(store)` → `peer_claims_page(store, query, shape)`: SAME read; fork render.
  (slice-07 MAY also thread `?page=N` for peer paging parity with US-HX-002 — same `parse_page`
  + `PageView::paged` shape the claims handler already uses; this is the only behavioral
  extension and it reuses the existing pure pagination machinery.)
- `claim_detail_page(store, cid)` → `(store, cid, shape)`: `get_claim` unchanged; on `Some`
  fork detail fragment vs page; on `None`/`Err` fork not-found fragment vs page — 404 status
  carries through BOTH shapes (the shape fork is AFTER the found/not-found decision).
- `scrape_get` (the `GET /scrape` arm) and `scrape_post(req, github)` → `(…, shape)`: build
  the SAME `ScrapeState`, fork the render (results fragment vs full page). No persist, no
  sign — unchanged.

**Dependencies**: UNCHANGED — `hyper`/`hyper-util`/`http-body-util`, `tokio`, `ports`,
`viewer-domain`, `scraper-domain`, `serde`/`serde_json`, `thiserror`. The htmx asset is a
text file embedded via `include_str!` — NOT a crate dependency; the dep graph is unchanged,
so `xtask check-arch` (no `adapter-*`→`adapter-*` edge; only `cli` links this crate) still
passes. NO `IdentityPort`/`PdsPort`/write `StoragePort` — the read-only/no-key guarantee is
untouched by slice-07.

**Earned-Trust `probe()`**: UNCHANGED (store-read + read-only-capability + loopback). The
asset route + shape dispatch are pure in-memory operations over already-probed capabilities;
they add no new substrate. The PARITY + header→shape contracts are probed empirically by the
acceptance tests (ADR-032/035) — the Earned-Trust check that the two shapes agree in the real
running viewer.

---

## `crates/ports`, `crates/adapter-duckdb`, `crates/cli`, `xtask` (UNCHANGED)

- **`ports`**: no new boundary type, no new port. `StoreReadPort` + `ClaimRow`/`PageRequest`/
  `Page` etc. are reused verbatim.
- **`adapter-duckdb`**: no new query, no new table. The peer-paging extension (US-HX-002)
  reuses the existing `list_peer_claims(PageRequest)` (already paginated) + `count_peer_claims`
  — the slice-06 handler simply called the first page; slice-07 threads `?page=N` through the
  SAME port methods. No adapter change.
- **`cli`**: the `ui` verb wiring (WIRE→PROBE→USE) is identical; no new dep, no new flag.
- **`xtask`**: no rule change (see architecture-design.md §11) — htmx is a text asset, the
  pure core deps are unchanged, the capability boundary holds.

---

## `tests/acceptance/support` (EXTEND, TEST — ADR-035)

`ViewerServer` gains two methods mirroring `get` / `post_form` but setting `HX-Request: true`:
- `get_htmx(&self, path: &str) -> ViewerResponse` — drives the FRAGMENT shape.
- `post_form_htmx(&self, path, fields) -> ViewerResponse` — drives the scrape results fragment.

The existing `get` / `post_form` (no header) REMAIN the full-page/no-JS-path drivers
(unchanged → slice-06 suite stays green). `ViewerResponse` (status + body + `body_contains`)
is reused verbatim. TEST convention only; zero production impact.

---

## Cross-component invariants (slice-07, referencing inherited I-VIEW-*)

| Invariant | Structural enforcement (slice-07) |
|-----------|-----------------------------------|
| **I-HX-1 progressive enhancement** | One header read (`Shape::from_request`, ADR-033); FullPage arm calls the unchanged `render_*_page`; only `/static` is a new route. Drive with/without header (ADR-035). |
| **I-HX-2 offline htmx** | `include_str!` in-binary asset; chrome references one local `/static/htmx.min.js`; no off-host URL on any page (ADR-031). Offline gold test. |
| **I-HX-3 read-only** | `adapter-http-viewer` capability boundary UNCHANGED (StoreReadPort + GithubPort only; no key); asset route GET-only fixed bytes. `xtask check-arch` viewer rule passes. |
| **I-HX-4 no regression** | FullPage arm = slice-06 `render_*_page` (now composed from the same fragment markup); the page body delta is bounded to the `div id` wrapper + the `<script src>` line; slice-06 26-scenario suite is the gate. |
| **I-HX-5 parity** | `render_*_page` EMBEDS `render_*_fragment` (ADR-032) — no second renderer to drift. Parity test asserts page⊇fragment. |
| **I-VIEW-1/2/3/4 (read-only/no-key/CLI-gate/loopback)** | UNCHANGED — slice-07 adds no port, no key, no bind change; the asset route is loopback-only + GET-only. |
| **I-VIEW-5 derived-from / FR-VIEW-8 confidence** | UNCHANGED — type-level (CandidateRowView only) + one-site `render_confidence`, both inside the fragment. |
| **No new persisted types / no new CID** | slice-07 adds ZERO persisted types, ZERO schema, ZERO CID path (read + render only). |
