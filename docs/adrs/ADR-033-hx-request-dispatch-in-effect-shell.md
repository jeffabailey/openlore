# ADR-033: `HX-Request` Shape Dispatch Lives in the Effect Shell — One Decision Point per Route, Pure Core Stays Header-Unaware

- **Status**: Accepted (DESIGN — slice-07 viewer-htmx-swaps)
- **Date**: 2026-06-02
- **Deciders**: Morgan (nw-solution-architect), per OD-HX-5 for viewer-htmx-swaps (slice-07).
- **Feature**: viewer-htmx-swaps (slice-07)
- **Extends**: ADR-007 (pure/effect split), ADR-028 (the hand-rolled hyper route table in `adapter-http-viewer`), ADR-032 (the pure fragment/page fns the shell chooses between).
- **Resolves**: OD-HX-5 (where the HX-Request branch lives) + the enforcement of I-HX-1 (progressive enhancement) and BR-HX-2 (header is the sole selector).

## Context

For each of the four data routes, the response SHAPE (fragment vs full page) is selected
by the presence of the `HX-Request` header (set by htmx on swap-driven requests; absent on
plain navigation, curl, bookmark, view-source, JS-disabled). BR-HX-2 fixes the header as
the SOLE selector — no query param, no separate endpoint. The pure core (`viewer-domain`,
ADR-007/029) must NOT know about HTTP headers; it exposes both shapes (ADR-032) and stays
unaware of why one is chosen.

The integration risk (shared-artifacts-registry.md, HIGH): if any handler ignores
`HX-Request`, it returns the wrong shape — a full page to a swap (double chrome inside the
target) or a bare fragment to a no-JS/curl request (a broken half-page, breaching I-HX-1).
There must be ONE decision point per route, in ONE place.

The existing handler structure (`adapter-http-viewer/src/lib.rs`) is a hand-rolled hyper
`route(req, store, github)` fn that matches method+path and calls per-route handler fns
(`claims_page`, `peer_claims_page`, `claim_detail_page`, `scrape_post`), each currently
returning a full-page `Response` via `html_ok(render_*_page(...))`.

## Decision

**Read the `HX-Request` header ONCE per request in the effect-shell `route` fn, derive a
single `Shape { Fragment, FullPage }` value, and thread it into each per-route handler. Each
handler builds the SAME `viewer-domain` view-model as today, then calls
`render_*_fragment(...)` (Shape::Fragment) or `render_*_page(...)` (Shape::FullPage) per
ADR-032. The pure core never sees the header. This is the only place the header is read.**

### Dispatch point in the existing handler structure

```text
async fn route(req, store, github) -> Response {
    let method = req.method().clone();
    let path   = req.uri().path().to_string();
    let query  = req.uri().query().map(str::to_string);
    let shape  = Shape::from_request(&req);   // <-- the ONE read of `HX-Request`, here

    if method == POST && path == "/scrape" { return scrape_post(req, github, shape).await; }
    if method != GET { return not_found(); }
    match path {
        "/static/htmx.min.js" => serve_htmx_asset(),     // asset route — ignores shape (ADR-031)
        "/"            => landing_page(),                  // landing — always full (no swap target)
        "/claims"      => claims_page(store, query, shape),
        "/peer-claims" => peer_claims_page(store, query, shape),
        "/scrape"      => scrape_get(shape),
        p if p.starts_with("/claims/") => claim_detail_page(store, cid, shape),
        _ => not_found(),
    }
}
```

`Shape::from_request(req)` is a tiny PURE total fn in the effect shell:
`if req.headers().contains_key("HX-Request") { Fragment } else { FullPage }`. (htmx sends
`HX-Request: true`; presence is sufficient and is what htmx guarantees. Held in ONE place
so the header name is a single site.)

Each handler then forks ONLY at the render call:

```text
fn claims_page(store, query, shape) -> Response {
    let page_view = /* unchanged slice-06 read + project */;
    let body = match shape {
        Shape::Fragment => render_claims_table_fragment(&page_view).into_string(),
        Shape::FullPage => render_claims_page(&page_view),
    };
    html_ok(body)
}
```

The data read, the view-model projection, the clamp, the error degradation, the status
codes — ALL unchanged from slice-06. The header changes ONLY which pure renderer is called.
Notes:
- **No new data routes** (I-HX-1 / BR-HX-1): the routes are identical; only the response
  shape varies by header.
- **Asset route ignores shape** (ADR-031): `/static/htmx.min.js` always serves the script,
  regardless of header — it is an asset route, not a data route.
- **Landing (`/`) is always full**: it has no swap target; htmx requests are not expected
  there, and a full page is the safe default.
- **Status codes carry through both shapes**: unknown-CID detail returns the guided
  not-found body at 404 in BOTH shapes (the fragment is the not-found fragment; the full
  page is the not-found page) — the shape fork is inside the handler, after the
  found/not-found decision.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Pass `is_htmx: bool` into `viewer-domain` renderers** | One renderer entry per view. | **Rejected (ADR-007).** The pure core would know about the HTTP header → purity breach; the renderer's job is markup, not transport policy. The shell owns transport. |
| **A separate set of `/fragment/...` or `?fragment=1` routes** | Explicit. | **Rejected (BR-HX-2 / BR-HX-1).** Duplicates the data surface (two URLs for one content → drift + new routes), and the header — not a URL — is the contractual selector. htmx's whole point is the SAME URL with a header. |
| **Middleware/wrapper that rewrites the response** (render full page, strip chrome for htmx) | DRY at the transport edge. | **Rejected.** String surgery on rendered HTML (fragile, escaping hazards) and it inverts ADR-032 (compose page FROM fragment, not strip fragment FROM page). The clean fork is one `match shape` at the render call. |
| **Decide shape per-handler by re-reading the header inside each handler** | Local. | **Rejected (integration risk).** N read sites = N chances to forget one (the registry's HIGH risk). One read in `route`, threaded as a typed `Shape`, makes "did this route honor the header?" a single, total, type-checked decision. |

## Consequences

### Positive
- ONE header read site (`Shape::from_request` in `route`); the typed `Shape` threaded into
  handlers makes the wrong-shape integration risk a compile-time-shaped concern, not a
  scattered convention. (I-HX-1 / BR-HX-2 enforced at one point.)
- Pure core stays header-unaware (ADR-007 preserved): `viewer-domain` exposes both shapes;
  the shell chooses.
- Minimal diff to slice-06 handlers: the read/project/clamp/error logic is untouched; only
  the final render call forks. The no-regression gate (I-HX-4) is easy to keep green
  because the FullPage arm calls the unchanged `render_*_page`.
- Testable seam: drive each route with/without the header (ADR-035 harness) and assert the
  shape — the Earned-Trust probe for "header drives shape".

### Negative
- Every handler signature gains a `shape: Shape` parameter. Accepted: it is a small, total,
  explicit thread; the alternative (implicit/global) is the integration risk this ADR
  closes.
- The `Shape` fork is repeated in each handler (one `match` each). Accepted: the
  repetition is the point — each route explicitly honors the header at its render call; a
  helper that hides it would re-introduce a single hidden decision the registry warns about.

## Revisit Trigger
- htmx adds a more specific signal we want to honor (e.g. `HX-Target` for partial-of-partial)
  → extend `Shape`/`Shape::from_request`; still one read site.
- A route needs a third shape → widen the `Shape` ADT; the per-handler `match` stays total.
