# ADR-038: `GET /search` — Its Own Route, a Bookmarkable GET Form, a Third Nav Link, and the slice-05 Config Reuse (with the `viewer-domain → appview-domain` check-arch Allowlist)

- **Status**: Accepted / shipped (slice-08 viewer-network-search, DELIVER 2026-06-04). The `GET /search` route + bookmarkable GET form (object/contributor/subject dimension selector — completed at step 02-03, DV-NS-3) + third nav link + slice-05 config reuse shipped; the 2 `xtask check-arch` deltas (the `viewer-domain → appview-domain` pure-core allowlist entry + the extended viewer capability rule) landed; `check-arch` reports 21 workspace members.
- **Date**: 2026-06-04
- **Deciders**: Morgan (nw-solution-architect), resolving OD-NS-4 + OD-NS-5 for viewer-network-search (slice-08).
- **Feature**: viewer-network-search (slice-08)
- **Extends**: ADR-028 (the hand-rolled hyper route table), ADR-031 (the vendored htmx asset, offline chrome), ADR-033 (the `Shape` fork — `HX-Request` is the sole selector), ADR-034 (the nav tabs + `hx-push-url` history pattern), ADR-036 (the wired `IndexQueryPort`), ADR-037 (the `SearchState` ADT + the render).
- **Resolves**: OD-NS-4 (own route vs nav tab) + OD-NS-5 (form UI + GET vs POST) + the `xtask check-arch` consequences of slices 08.

## Context

US-NS-002 introduces `/search`: a form (dimension selector
object/contributor/subject + a value input) that, on submit, queries the indexer and
renders verified attributed results. Two route questions:

- **OD-NS-4**: should `/search` be its OWN route, or a third TAB in the existing My
  Claims / Peer Claims nav (which swaps `#view-panel`, ADR-034)?
- **OD-NS-5**: GET-query form (`/search?<dim>=<value>`) vs POST form; and the
  fragment-vs-full-page fork.

The viewer's other network route, `POST /scrape` (slice-06/07), is the precedent for
a network-touching handler that forks by `Shape` and renders an ADT. But `/scrape`
is a POST because it submits a target to harvest (an action). `/search` is a READ:
its result depends only on the dimension+value, and the slice-05 CLI already treats a
search as a deterministic re-runnable query (the `--share`/`openlore://search?…`
link encodes the query, not a snapshot, ADR-027 / I-AV-8). A bookmarkable,
shareable browser search URL is the natural browser analog of that link.

ADR-037 establishes that `viewer-domain` projects the `appview-domain` result types,
which requires a new pure dependency edge `viewer-domain → appview-domain`. The
viewer capability boundary (ADR-030 / I-VIEW-1) and the indexer-internal boundary
(ADR-023 / I-AV-5) must both still hold under `xtask check-arch`.

## Decision

**`/search` is its OWN route — `GET /search` — added to the viewer nav as a THIRD
link alongside My Claims / Peer Claims. The form is a GET form
(`/search?<dimension>=<value>`, dimensions `object` / `contributor` / `subject`) so a
search is a bookmarkable/shareable URL and the no-JS path is plain navigation. The
handler reads `HX-Request` once (ADR-033) and forks by `Shape`: the fragment swaps
the `#search-results` region (with `hx-push-url` so the address bar shows the query,
ADR-034); the no-`HX-Request` request returns the complete `/search` page. The
indexer URL resolves through the slice-05 `[appview] indexer_url` + `OPENLORE_INDEXER_URL`
resolution (ADR-036 / OD-NS-6).**

### Route + handler shape (extends the ADR-028/033 route table)

```text
async fn route(req, store, github, index_query) -> Response {
    let shape = Shape::from_request(&req);            // ADR-033: the ONE HX-Request read
    ...
    match (method, path) {
        (GET, "/")            => landing_page(),
        (GET, "/claims")      => claims_page(store, query, shape),
        (GET, "/peer-claims") => peer_claims_page(store, query, shape),
        (GET, "/scrape")      => scrape_get(shape),
        (POST,"/scrape")      => scrape_post(req, github, shape).await,
        (GET, "/search")      => search_get(index_query, query, shape).await,   // slice-08 NEW
        (GET, HTMX_ASSET_URL) => htmx_asset(),
        ...
    }
}

async fn search_get(index_query, query, shape) -> Response {
    // 1. Parse the dimension+value from the query string (?object= / ?contributor= /
    //    ?subject=). No dimension => SearchState::Form (the empty form; no network call).
    let state = match parse_search_query(query) {
        None => SearchState::Form,
        Some((dim, value)) => match index_query {
            // No indexer wired => Unavailable (ADR-036/037: same as unconfigured).
            None => SearchState::Unavailable,
            Some(iq) => match iq.search(dim, &value, None).await {
                // SOFT/non-fatal Unreachable => Unavailable (I-NS-2; payload discarded).
                Err(IndexQueryError::Unreachable { .. }) => SearchState::Unavailable,
                Err(_)  => SearchState::Unavailable,         // any other failure: same calm guidance, no leak
                Ok(raw) if raw.results.is_empty()
                        => SearchState::NoResults(no_results_view(dim, &value, raw.suggestion)),
                Ok(raw) => SearchState::Results(
                    // REUSE the pure composition (ADR-025/037): flat attributed rows ->
                    // appview_domain::compose_results -> per-author NetworkSearchResult ->
                    // the viewer view-model projection.
                    project(compose_results(into_indexed(raw.results), dim))
                ),
            },
        },
    };
    match shape {                                            // ADR-032/033 fork at the render call
        Shape::Fragment => html_ok(render_search_results_fragment(&state).into_string()),
        Shape::FullPage => html_ok(render_search_page(&state)),
    }
}
```

The dimension/value parse is a pure total fn over the query string (the `parse_page`
precedent). An unknown/malformed dimension key yields `SearchState::Form` (the empty
form, never a crash). The handler persists NOTHING (I-NS-8) and renders no sign/write
control (I-NS-1).

### Nav: a third link (OD-NS-4) — own route, not a `#view-panel` tab

`render_tab_nav` (ADR-034) gains a third anchor to `/search`. Unlike the My/Peer tabs
(which swap `#view-panel` because they are two views of the SAME local-store corpus),
`/search` is a DISTINCT corpus (the network index, not the local store), so it is its
OWN page/route — the nav link is an ordinary `<a href="/search">` enhanced with
`hx-get`/`hx-push-url` consistent with the other links, but the SEARCH FORM's submit
(not the nav link) is what targets `#search-results`. The form is a GET `<form
method="get" action="/search">` with the dimension selector + value input;
JS-enabled, it carries `hx-get="/search"`, `hx-target="#search-results"`,
`hx-push-url="true"` (the slice-07 pattern) so a submit swaps the results region in
place and the address bar shows `?<dim>=<value>`.

### `xtask check-arch` consequences (the enforcement deltas)

1. **ADD `appview-domain` as an allowed PURE dependency of `viewer-domain`** in the
   pure-core dependency rules: `viewer-domain → appview-domain` is permitted (both are
   pure domain crates; the edge never reverses). Both remain on the pure-core
   allowlist (no I/O); the edge introduces NO I/O crate into `viewer-domain` (ADR-037).
2. **EXTEND the viewer capability rule** (the slice-06 analog of `indexer_holds_no_…`):
   `adapter-http-viewer` and `viewer-domain` MUST NOT depend on any signing/identity/PDS
   crate, the indexer's SERVER/store/ingest crates (`adapter-xrpc-query-server`,
   `adapter-index-store`, `adapter-atproto-ingest`), or any write surface. The viewer
   MAY hold `IndexQueryPort` (read-only) + `adapter-index-query` (wired via `cli`),
   exactly as it MAY hold `GithubPort` (ADR-030/036). Confirm the rule's allowlist
   admits `IndexQueryPort` as a read-only capability for the viewer process.
3. **`check-probes` is unchanged**: `IndexQueryPort` already carries a required
   non-stub `probe()` (slice-05); the viewer reuses the existing
   `HttpIndexQueryAdapter` impl + its probe. `viewer-domain` is pure (no `probe()`).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **A third TAB swapping `#view-panel`** (reuse the My/Peer tab machinery) | Maximal nav reuse. | **Rejected (OD-NS-4 / distinct corpus).** The My/Peer tabs are two views of the LOCAL store under one `#view-panel`; search is the NETWORK index — a different corpus with a form, a public-data banner, and its own degradation surface. Forcing it into `#view-panel` would conflate local and network views and complicate the swap targets. Its own route keeps "local store vs network index" unambiguous (the BR-VIEW-5 spirit). |
| **POST form** (`POST /search`, body-encoded) | Matches `/scrape`. | **Rejected (OD-NS-5 / bookmarkability + no-JS).** `/scrape` is an action (harvest a target); search is a READ whose result is a pure function of dimension+value. A GET URL is bookmarkable/shareable (the browser analog of the slice-05 `openlore://search?…` query-encoding link, I-AV-8) and the no-JS path is plain navigation — no body to re-POST on reload, no "resubmit form?" prompt. |
| **A separate `/search/fragment` route for htmx** | Explicit. | **Rejected (ADR-033 / BR-HX-2).** The header, not a URL, is the sole shape selector. Two URLs for one content duplicates the surface and drifts. One `GET /search`, forked by `HX-Request`. |
| **Persist recent searches / a search history table** | Convenience. | **Rejected (I-NS-8 / WD-NS-7).** Zero new persisted types; results computed per query. A history is out of scope (and would be a new store surface the read-only viewer must not own). |
| **Resolve the indexer URL from a NEW viewer config key** | Viewer autonomy. | **Rejected (OD-NS-6 / ADR-036).** One source of truth — `[appview] indexer_url` + `OPENLORE_INDEXER_URL`. (Covered in ADR-036; restated here for the route's config dependency.) |

## Consequences

### Positive
- A browser search is a shareable/bookmarkable URL (`/search?object=…`) — the browser
  analog of the slice-05 share link, with no extra mechanism (I-AV-8 spirit on the
  browser surface).
- The no-JS path is plain navigation (a GET link/submit → full page); the htmx path
  swaps `#search-results` + pushes the URL — progressive enhancement by the exact
  slice-07 pattern (I-NS-6 / I-HX-1..5). Chrome (incl. htmx) stays offline/vendored
  (ADR-031 / I-NS-7); only the SEARCH itself needs the network (like `/scrape`).
- One handler, one `Shape` fork at the render call (ADR-033); no new data routes keyed
  on the header.
- Read-only + graceful-degradation are structural: no write/sign route added; the only
  failure mapping is to the payload-free `Unavailable` state (ADR-037).

### Negative
- `render_tab_nav` now renders three links (a one-line nav change shared by every page
  that embeds it). Accepted: it is the established nav extension point (ADR-034).
- `route` and the handler signatures thread an `Option<SharedIndexQuery>`. Accepted:
  it mirrors the existing `Option<SharedGithub>` thread exactly (ADR-036).
- A new `check-arch` allowlist entry (`viewer-domain → appview-domain`). Accepted: it
  is a pure→pure edge; the rule explicitly admits it (this ADR §enforcement).

## Revisit Trigger
- A future browser `--share`-equivalent (a copyable link / QR) → the GET URL already
  IS the shareable artifact; a copy affordance is additive (deferred, feature-delta
  Out of Scope).
- A route needs server-side push of fresh results → out of scope (pull-per-query is
  the contract; mirrors ADR-024's pull stance).
