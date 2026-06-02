# Shared Artifacts Registry: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06.** Every `${variable}` in the journey mockups maps to a single source
> of truth here. The cardinal integration risk for this slice is **one content rendered in
> two shapes** (full page vs fragment) drifting apart, and the **htmx asset** leaking to a
> CDN. Both are tracked below. Tech choices (exact asset mechanism, exact swap ids, history
> strategy) are open decisions for DESIGN (OD-HX-*), but the *artifact contracts* they must
> satisfy are fixed here.

## Registry

```yaml
shared_artifacts:

  hx_request_header:
    source_of_truth: "incoming HTTP request header `HX-Request` (set by the htmx library on swap-driven requests; absent on plain navigation / curl / bookmark / no-JS)"
    consumers:
      - "GET /claims handler (fragment vs full page)"
      - "GET /peer-claims handler (fragment vs full page)"
      - "GET /claims/{cid} handler (fragment vs full page)"
      - "POST /scrape handler (fragment vs full page)"
    owner: "adapter-http-viewer (effect shell route handlers) — the ONE place that reads the header and selects the response shape"
    integration_risk: "HIGH — if any handler ignores HX-Request it returns the wrong shape: a full page to an htmx swap (double chrome) or a bare fragment to a no-JS/curl/bookmark request (broken half-page, breaks I-HX-1)."
    validation: "Drive each route WITH and WITHOUT the header against the real `openlore ui`; assert fragment shape when present, complete page when absent."

  swap_target:
    source_of_truth: "the page-chrome / layout shell in `viewer-domain` — the `id` attributes on the swappable regions: #claims-table, #scrape-results, #claim-detail, #view-panel (exact ids are OD-HX-3)"
    consumers:
      - "full page (defines each region by id)"
      - "fragment response (must target the SAME id so the swap lands)"
    owner: "viewer-domain (pure page chrome + fragment renderers share the id definitions)"
    integration_risk: "HIGH — a fragment that targets an id the full-page chrome does not define (or a renamed id on one side) makes the swap land nowhere. Full page and fragment must agree on every id."
    validation: "Assert each full page contains the swap-target id; assert each fragment's swap targets that same id. Single id constant per region referenced by both."

  fragment:
    source_of_truth: "`viewer-domain` render functions that emit ONLY the changed region — split out of the slice-06 whole-page renderers (e.g. a claims-table renderer factored out of render_claims_page; a results renderer out of render_scrape_page; a detail renderer out of render_claim_detail; a view-panel renderer for the tab). The exact fragment/page rendering split is OD-HX-2."
    consumers:
      - "GET /claims & /peer-claims under HX-Request (table fragment)"
      - "POST /scrape under HX-Request (results fragment)"
      - "GET /claims/{cid} under HX-Request (detail fragment)"
      - "tab switch under HX-Request (view-panel fragment)"
    owner: "viewer-domain (pure)"
    integration_risk: "HIGH — the fragment's content MUST equal the corresponding region of the full page (same rows, same position indicator, verbatim confidence, peer origin). Divergence = the two shapes disagree (breaks I-HX-5)."
    validation: "Parity test: for the same inputs, the fragment renderer output equals the region the full-page renderer produces (e.g. the full page embeds the same fragment)."

  full_page:
    source_of_truth: "slice-06 `viewer-domain` whole-page renderers (render_claims_page, render_peer_claims_page, render_claim_detail, render_scrape_page) — UNCHANGED in content for non-htmx requests"
    consumers:
      - "every route when HX-Request is absent (no-JS, direct URL, bookmark, view-source, curl)"
    owner: "viewer-domain (pure) — slice-06"
    integration_risk: "HIGH — must stay byte-equivalent to slice-06 for non-htmx requests (I-HX-4); the slice-06 26-scenario acceptance suite is the regression gate."
    validation: "slice-06 acceptance corpus stays GREEN; non-htmx responses unchanged."

  htmx_asset:
    source_of_truth: "the htmx library served by THIS process — a vendored asset (e.g. GET /static/htmx.min.js) OR inlined into the page chrome. SINGLE source. The exact mechanism is OD-HX-1."
    consumers:
      - "every page's chrome references the one local source"
    owner: "viewer-domain page chrome (references it) + adapter-http-viewer (serves it, if a static route is chosen)"
    integration_risk: "HIGH — if loaded from a CDN, swaps break offline (breaks I-HX-2 / I-VIEW-6 / KPI-5). If duplicated, the two copies can drift. Loopback-only + offline are inherited hard invariants."
    validation: "Offline test: with the network down, every store view AND every swap works. Property test: no served page references an off-host URL for htmx."

  position_indicator:
    source_of_truth: "slice-06 `viewer-domain` PageView (start/end/total) + render_position_indicator; pagination clamp (slice-06 DV-5) preserved"
    consumers:
      - "full /claims & /peer-claims pages"
      - "the table fragment (must show the SAME 'X–Y of N' and Prev/Next bounds)"
    owner: "viewer-domain (pure) — slice-06"
    integration_risk: "MEDIUM — fragment and full page must show identical 'X–Y of N' and identical clamp behavior for an over-the-end page."
    validation: "Parity test on the indicator string + clamp boundary in both shapes."

  active_view_url:
    source_of_truth: "the browser URL/history after a tab swap — must reflect /claims or /peer-claims so the active view is bookmarkable and Back works. History strategy (e.g. hx-push-url) is OD-HX-4."
    consumers:
      - "tab switch (htmx) updates it"
      - "bookmark / Back / reload re-enter via the full page on that URL"
    owner: "adapter-http-viewer + viewer-domain chrome (whichever drives the history mechanism — OD-HX-4)"
    integration_risk: "MEDIUM — if the tab swap does not update the URL, bookmark/Back/reload break; the no-JS path already uses real URLs, so the htmx path must converge on the same URLs."
    validation: "After a tab swap, the URL is the target route; reloading that URL yields the full page for that view."
```

## Integration checkpoints (consolidated)

1. **Header-drives-shape**: `${hx_request_header}` present → `${fragment}`; absent → `${full_page}`. One decision point per route, in the effect shell. (I-HX-1)
2. **Fragment/full-page parity**: `${fragment}` == the matching region of `${full_page}` for the same inputs (rows, `${position_indicator}`, verbatim confidence, peer origin). (I-HX-5)
3. **Swap targets agree**: every `${fragment}` targets a `${swap_target}` id that `${full_page}` chrome defines. (OD-HX-3 fixes the exact ids.)
4. **Asset is local + single-source**: `${htmx_asset}` has one source, served from loopback, never a CDN; offline test is the gate. (I-HX-2 / I-VIEW-6)
5. **URL reflects view after tab swap**: `${active_view_url}` updates so bookmark/Back/reload converge with the no-JS full-page path. (OD-HX-4 fixes the mechanism.)
6. **Read-only preserved**: no `${fragment}` or swap introduces a write/sign route; no key in the web process. (I-HX-3 / I-VIEW-1/2)

## CLI/UX vocabulary consistency

- Verb unchanged: **`openlore ui [--port <P>]`** (default 8788), loopback only, no auth — slice-06.
- Routes unchanged: `GET /`, `GET /claims?page=N`, `GET /claims/{cid}`, `GET /peer-claims?page=N`, `GET|POST /scrape`. **No new data routes** — only the optional `${htmx_asset}` static route (OD-HX-1).
- "fragment" vs "full page" is the slice-07 ubiquitous language for the two response shapes of the SAME content.
