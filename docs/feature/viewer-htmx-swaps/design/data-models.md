# Data Models: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06 data-models.** slice-07 adds **ZERO persisted types**, **ZERO schema**,
> **ZERO new CID path**, and **ZERO new boundary (`ports`) type**. It is a *response-shape*
> slice: the SAME slice-06 view-models render in two SHAPES (fragment vs full page) over the
> SAME data. The only NEW in-memory types are an effect-shell `Shape` enum and a vendored
> static asset constant. Every displayed datum still traces to an existing slice-06 column.

## 0. What slice-07 does NOT add to the data model

- No new table, no new column, no migration (the schema delta is EMPTY — as slice-06).
- No new persisted type, no new CID path (CID stability invariant; read + render only).
- No new `ports` boundary type — `ClaimRow`/`ClaimDetail`/`PeerClaimRow`/`PageRequest`/
  `Page`/`StoreReadError` are reused verbatim.
- No new view-model — `ClaimRowView`/`ClaimDetailView`/`PeerClaimRowView`/`CandidateRowView`/
  `PageView<T>`/`ScrapeState`/`PeerOrigin` are reused verbatim (slice-06 §3).
- No second store, no second handle (BR-VIEW-4 / Q-DELIVER-3 — unchanged).

The slice-07 model delta is purely about RENDERING SHAPE and ASSET DELIVERY, both in-memory:

---

## 1. New in-memory types (NOT persisted)

### `Shape` — the response-shape selector (effect shell, ADR-033)

```text
enum Shape { Fragment, FullPage }          // adapter-http-viewer, pure value

Shape::from_request(req) -> Shape =
    if req.headers().contains_key("HX-Request") { Fragment } else { FullPage }
```

- Derived ONCE per request from the `HX-Request` header (BR-HX-2: header is the sole
  selector). Threaded into each handler; never persisted, never crosses into the pure core
  (the pure core stays header-unaware — ADR-007/033).
- Total: every request maps to exactly one shape; absence → `FullPage` (the safe
  progressive-enhancement default, I-HX-1).

### Swap-target id constants (pure core, ADR-032)

```text
const ID_CLAIMS_TABLE:   &str = "claims-table";
const ID_SCRAPE_RESULTS: &str = "scrape-results";
const ID_CLAIM_DETAIL:   &str = "claim-detail";
const ID_VIEW_PANEL:     &str = "view-panel";
```

- One `const` per swappable region, in `viewer-domain`. Referenced by BOTH the fragment
  renderer (which wraps its region in `div id=(ID_*)`) and the full-page chrome (which
  defines the same slot by composing the fragment) — so the swap-target id agrees on both
  sides by construction (the `swap_target` shared-artifact contract, OD-HX-3).

### Vendored htmx asset (effect shell, ADR-031)

```text
const HTMX_MIN_JS: &str = include_str!("../assets/htmx.min.js");  // htmx 2.0.4, 0BSD
const HTMX_SHA256: &str = "<sha256 of the vendored bytes>";       // integrity, test-asserted
```

- A compile-time TEXT embed of a local repo file (`crates/adapter-http-viewer/assets/
  htmx.min.js`). NOT a persisted datum, NOT a crate dependency. Served verbatim at
  `GET /static/htmx.min.js`. The SHA pins the exact bytes (a unit test asserts
  `sha256(HTMX_MIN_JS) == HTMX_SHA256`), so a silent file swap is caught at build/test.

---

## 2. The two response shapes of the SAME view-model (the parity model — I-HX-5)

For each region, ONE view-model renders in two shapes; the page is the chrome wrapped around
the fragment (ADR-032), so the region content is byte-identical across shapes:

| View-model (reused, slice-06) | Fragment shape (`HX-Request` present) | Full-page shape (absent) |
|---|---|---|
| `PageView<ClaimRowView>` | `render_claims_table_fragment` → `<div id="claims-table">` rows + `start–end of total` + Prev/Next | `render_claims_page` → chrome + the SAME `<div id="claims-table">…</div>` |
| `PageView<PeerClaimRowView>` | `render_peer_claims_table_fragment` → `<div id="claims-table">` rows w/ origin + indicator | `render_peer_claims_page` → chrome (inside `#view-panel`) + the SAME div |
| `ScrapeState` | `render_scrape_results_fragment` → `<div id="scrape-results">` candidates / zero / network-down | `render_scrape_page` → chrome + form + the SAME `<div id="scrape-results">…</div>` |
| `ClaimDetailView` | `render_claim_detail_fragment` → `<div id="claim-detail">` all fields + evidence[] | `render_claim_detail` → chrome + the SAME div |
| (unknown CID) | `render_claim_not_found_fragment` → `<div id="claim-detail">` guided not-found + back link | `render_error` → chrome + the SAME not-found region |

**Parity is structural**: the full page renders the SAME fragment fn output, so for identical
inputs the region (rows, `X–Y of N`, verbatim confidence via the one-site `render_confidence`,
peer origin via the one-site `render_peer_origin`, derived-from on candidates only) is
IDENTICAL in both shapes. There is no second renderer to diverge (the `fragment` ↔ `full_page`
shared-artifact contract, I-HX-5).

---

## 3. Displayed-field provenance (UNCHANGED from slice-06 — re-affirmed for the fragment shape)

Every displayed datum in EITHER shape traces to the SAME slice-06 column (data-models.md
slice-06 §1). The fragment shape changes the WRAPPER, never the data:

- Claims fragment: `subject`/`predicate`/`object` (verbatim, escaped), `confidence`
  (VERBATIM numeric, FR-VIEW-8), `cid`; detail adds `author_did`, `composed_at` (RFC-3339),
  `evidence[]` (ordinal order; empty → "no evidence attached").
- Peer fragment: + `PeerOrigin` (`Known{author_did, fetched_from_pds}` → verbatim; `Unknown`
  → "unknown", never dropped). NO derived-from slot (type-level, I-VIEW-5).
- Scrape candidate fragment: + `derived_from` (display-only; ONLY on `CandidateRowView`,
  WD-62) + the "nothing signed/saved — use the CLI" notice (BR-VIEW-1 / I-SCR-1); NO sign
  control.
- Pagination (`PageView<T>`): `start`/`end`/`total` + `prev`/`next` bounds + the clamp
  (slice-06 DV-5) — all INSIDE the fragment, so the indicator string + clamp behavior are
  identical in both shapes (the `position_indicator` shared-artifact contract).

---

## 4. `active_view_url` — the tab history datum (ADR-034, not persisted)

The "current view" is the browser URL/history entry, kept as the REAL route path (`/claims`
or `/peer-claims`) by `hx-push-url="true"` on the tab anchors. It is browser state, not a
server datum: after an htmx tab swap the address bar holds the same path the no-JS link
would navigate to; reload/bookmark of that path hits the server with NO `HX-Request` →
`Shape::FullPage` → the full page for that view. One source of truth (the path), converging
the htmx and no-JS paths (the `active_view_url` shared-artifact contract, FR-HX-4).

---

## 5. Summary: the slice-07 model delta is in-memory and shape-only

| Concern | slice-07 delta |
|---|---|
| Persisted types / schema / CID | ZERO |
| `ports` boundary types | ZERO (reused) |
| `viewer-domain` view-models | ZERO new (reused; new fragment RENDERERS, not new models) |
| New in-memory types | `Shape` (shell), `ID_*` consts (pure), `HTMX_MIN_JS`/`HTMX_SHA256` (shell asset) |
| Data shown | UNCHANGED columns; only the wrapping shape varies |
