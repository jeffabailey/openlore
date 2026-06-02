# Walking Skeleton: viewer-htmx-swaps (slice-07)

## The skeleton: H-1 (US-HX-001 — pagination swaps the claims table in place)

The walking skeleton is the **H-1 trio** on `GET /claims?page=N`:

- **H-1a** (`@walking_skeleton`): `get_htmx("/claims?page=2")` returns ONLY the
  `#claims-table` fragment ("51–100 of 312" + rows + Prev/Next), NOT a full page.
- **H-1b**: `get("/claims?page=2")` (no header) returns the COMPLETE slice-06 full page.
- **H-1c**: parity — the fragment content equals the full page's table region.

H-1a carries the single `@walking_skeleton` tag; H-1b/H-1c are its no-JS and parity
companions that complete the progressive-enhancement contract on the same route.

## Why this is the thinnest thread proving the WHOLE progressive-enhancement contract

The slice-07 value proposition is one structural claim: **the `HX-Request` header selects
the response shape over the SAME route — fragment when present, complete slice-06 full page
when absent — and the two shapes are the same content.** H-1 exercises every load-bearing
mechanism of that claim end-to-end, with the least machinery:

1. **Header-drives-shape (ADR-033 / I-HX-1)** — H-1a sends `HX-Request` and gets a fragment;
   H-1b withholds it and gets a full page. The ONE header-read seam (`Shape::from_request`)
   is proven by the observable difference between the two responses on the identical URL.
2. **Fragment/full-page parity (ADR-032 / I-HX-5)** — H-1c proves the full page EMBEDS the
   same fragment fn: the "51–100 of 312" indicator + the rows + verbatim confidence appear in
   BOTH shapes. There is no second renderer to drift.
3. **No-JS fallback + no regression (I-HX-1 / I-HX-4)** — H-1b proves the no-header path is
   the complete slice-06 page (`is_full_page()`), the curl/bookmark/JS-off experience.
4. **Reuse, zero new data route (BR-HX-1 / I-HX-1)** — `/claims?page=N` is the slice-06 route;
   the only change is the SHAPE. The skeleton drives the REAL `openlore ui` subprocess over a
   REAL DuckDB seeded through the production `claim add` path (Pillar 3, BR-VIEW-4).

It is **demo-able to a non-technical stakeholder**: "Maria clicks Next and only the table
updates in place; with JavaScript off, the same Next link returns the whole page." That is the
user-goal framing (Mandate 3 / Dim 5), not a layer-connectivity framing.

## What the skeleton drives DELIVER to build (outside-in)

Implementing H-1a..c forces the whole slice-07 spine into existence:

- `viewer-domain`: promote the claims table region to a public `render_claims_table_fragment`
  wrapped in `div id=ID_CLAIMS_TABLE`; re-define `render_claims_page` to COMPOSE that same
  fragment fn + the chrome (incl. the `<script src="/static/htmx.min.js">` line). (ADR-032)
- `adapter-http-viewer`: read `HX-Request` ONCE → `Shape::from_request`; fork the
  `claims_page` handler's render call between fragment and page. (ADR-033)
- The asset `<script src>` line in the chrome wires US-HX-005's local asset reference into the
  page the skeleton renders (the skeleton carries a minimal local reference; US-HX-005 hardens
  the asset route + the no-CDN/offline gold).

Once H-1 is green, H-2 (peer paging) reuses the SAME pattern; H-3/H-4/H-6 add their own swap
targets but ride the identical `Shape` fork. Subsequent steps that find "the fragment fn +
shape fork already exist from H-1, just add the handler arm" confirm the skeleton was
well-chosen.

## Strategy (Architecture of Reference + Project Infrastructure Policy)

- **Driving port**: `openlore` CLI `ui` verb — REAL subprocess via
  `assert_cmd::cargo_bin("openlore")`, bound to ephemeral `:0` (read back, parallel-safe).
  (Project Infrastructure Policy → Driving.)
- **Driven internal (real)**: `StoragePort` (`adapter-duckdb`) — REAL DuckDB under
  `OPENLORE_HOME`, seeded via the production `claim add` write path. (Policy → Driven internal.)
- **Driven external (fake)**: `GithubPort` — `FakeGithub` (only on `POST /scrape`; not touched
  by the H-1 paging skeleton, which is offline by construction). (Policy → Driven external.)

No policy row needed beyond the existing slice-01..05 entries — slice-07 introduces NO new
port (it is a response-SHAPE delta over the slice-06 routes). The `tests/acceptance/support`
harness gains `get_htmx` / `post_form_htmx` (ADR-035), a TEST-only convention, zero production
impact.

## Litmus test (Mandate 5 / Dim 5)

| Check | H-1 |
|---|---|
| Title describes a user goal? | "Paging the claims list updates only the table, in place" — yes, not "header dispatch through the shell". |
| Given/When are user actions/context? | Given 312 signed claims; When she requests page 2 (with/without header) — yes. |
| Then are user observations? | Then she sees the table fragment / the full page with "51–100 of 312" — observable rendered text, not "a `div` was emitted". |
| Stakeholder confirms "that's what users need"? | Yes — smooth paging with a no-JS fallback is the slice-07 elevator pitch. |
