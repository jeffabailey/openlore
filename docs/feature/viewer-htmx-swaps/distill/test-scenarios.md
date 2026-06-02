# DISTILL Test Scenarios: viewer-htmx-swaps (slice-07)

> Every scenario (H-id) in Given-When-Then, the US-HX story it traces to, the route +
> header condition, the swap target, and the invariant it guards. The executable SSOT
> is `tests/acceptance/viewer_htmx.rs` (interaction scenarios) +
> `tests/acceptance/viewer_htmx_invariants.rs` (guardrail/gold). All scenarios are
> authored as `todo!()`-bodied scaffolds that COMPILE + classify RED; DELIVER fills
> them one at a time.

## Driving discipline + harness (Mandate 1, ADR-035)

- Every scenario enters through the CLI driving port — the REAL `openlore ui --port 0`
  subprocess (the `ViewerServer` spawn helper) + in-test HTTP. NO scenario calls
  `viewer-domain` `render_*_fragment` fns directly (those are layer-2 unit tests in
  DELIVER).
- The slice-07 harness seam (ADR-035 / OD-HX-6) adds **two** methods to `ViewerServer`,
  mirroring the existing `get` / `post_form` but setting the `HX-Request: true` header:
  - `get_htmx(path)` → drives the FRAGMENT shape (the htmx swap path).
  - `post_form_htmx(path, fields)` → drives the scrape-results FRAGMENT shape.
  - The existing `get` / `post_form` (no header) REMAIN the no-JS / full-page drivers —
    byte-unchanged, so the slice-06 26-scenario suite stays green (I-HX-4).
- Three `ViewerResponse` discriminators were added (all compile now, classify the body
  DELIVER renders): `is_full_page()` (carries `<!DOCTYPE html>` + `<html>` chrome),
  `is_fragment()` (the inverse), `references_external_cdn()` (off-host htmx host scan).
- External GitHub (only on `POST /scrape`) is the REUSED slice-02 `FakeGithub` via the
  existing `OPENLORE_GITHUB_API_BASE` seam — a NEW double is NOT built. DuckDB is REAL
  (BR-VIEW-4).

## Build-before-run (carry into DELIVER roadmap)

`cargo test` does NOT rebuild a spawned binary. The run MUST `cargo build` the `openlore`
bin first so `ViewerServer` spawns the CURRENT `openlore ui`, not a stale one (mirrors the
slice-06 viewer ATs + the slice-05 indexer ATs).

## Layer placement (Mandate 9/11)

Every H-* scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-ONLY. Sad
paths (zero candidates, network down, unknown CID, missing origin, over-the-end page) are
enumerated explicitly, NEVER PBT-generated at this layer. The `@property` tag on the
guardrails marks universal invariants for the reader; they stay example-pinned here. The
generative exploration of the pure fragment/page renderers is a layer-2 concern (DELIVER
unit tests).

## No-regression GATE (release-relevant, I-HX-4)

The slice-06 26-scenario corpus (`viewer_store.rs` / `viewer_scrape.rs` /
`viewer_invariants.rs`) MUST stay green when the htmx enhancement is layered on. The
slice-07 byte-equivalence side is pinned by `H-INV-NoReg`; DELIVER runs BOTH suites as the
gate. The no-header `get`/`post_form` drivers are byte-unchanged by ADR-035.

---

## US-HX-001 — Pagination swaps the claims table in place (WALKING SKELETON, H-1)

Route: `GET /claims?page=N`. Swap target: `#claims-table`. The thinnest end-to-end htmx
thread — proves header-drives-shape + parity + no-JS fallback on the SAME route.

### H-1a — htmx fragment (`@walking_skeleton`)
- **Given** Maria has 312 signed claims rendered 50 per page
- **When** she requests `/claims?page=2` WITH the `HX-Request` header (`get_htmx`)
- **Then** the response is ONLY the `#claims-table` fragment showing "51–100 of 312" + Prev/Next, NOT a full page, confidence verbatim
- Guards: I-HX-1 (shape selection), NFR-HX-6 (in-place, region only). Fn: `paging_claims_with_htmx_returns_only_the_table_fragment`

### H-1b — no-JS full page
- **Given** JavaScript is disabled
- **When** she requests `/claims?page=2` WITHOUT the header (`get`)
- **Then** the server returns the COMPLETE slice-06 full page (chrome + the table region)
- Guards: I-HX-1, I-HX-4. Fn: `paging_claims_without_htmx_returns_the_full_page`

### H-1c — parity
- **Given** 312 signed claims
- **When** `/claims?page=2` is fetched both WITH and WITHOUT the header
- **Then** the fragment's rows + indicator + verbatim confidence are contained in the full page's table region
- Guards: I-HX-5 (fragment ⊆ page). Fn: `claims_fragment_equals_the_full_page_table_region`

### H-1d — boundary: over-the-end clamp in both shapes
- **Given** 312 claims (last page 301–312 of 312)
- **When** `?page=99` is fetched WITH and WITHOUT the header
- **Then** both shapes show "301–312 of 312", not a blank result (slice-06 DV-5 clamp preserved)
- Guards: I-HX-4 / I-HX-5. Fn: `over_the_end_page_clamps_in_both_shapes`

## US-HX-002 — Pagination swaps the peer-claims table in place (H-2)

Route: `GET /peer-claims?page=N`. Swap target: `#claims-table` (inside `#view-panel`).
NOTE (DESIGN component-boundaries): DELIVER MUST thread `?page=N` into the peer handler
(slice-06 served only page 1), reusing the existing `parse_page` + `PageView::paged`.

### H-2a — htmx fragment
- **Given** Maria has a federated peer set rendered 50 per page
- **When** she requests `/peer-claims?page=2` WITH the header (`get_htmx`)
- **Then** the response is ONLY the peer-table fragment with the next rows + their origin (peer DID) + "51–100 of N", separable from own claims, NOT a full page
- Guards: I-HX-1, KPI-VIEW-3 (origin preserved). Fn: `paging_peer_claims_with_htmx_returns_only_the_peer_table_fragment`

### H-2b — no-JS full page
- **Given** JavaScript is disabled
- **When** she requests `/peer-claims?page=2` WITHOUT the header
- **Then** the server returns the COMPLETE slice-06 peer-claims page
- Guards: I-HX-1, I-HX-4. Fn: `paging_peer_claims_without_htmx_returns_the_full_page`

### H-2c — parity
- **Given** a federated peer set
- **When** `/peer-claims?page=2` is fetched both shapes
- **Then** the fragment's rows + indicator + peer origin are contained in the full page's peer-table region
- Guards: I-HX-5, KPI-VIEW-3. Fn: `peer_claims_fragment_equals_the_full_page_peer_table_region`

### H-2d — boundary: unknown origin still renders in the fragment
- **Given** a peer claim has no recorded origin
- **When** she pages Peer Claims WITH the header
- **Then** that row still renders in the fragment labeled "unknown", never dropped
- Guards: FR-VIEW-4 defensive render. Fn: `peer_claim_with_unknown_origin_still_renders_in_the_fragment`

## US-HX-003 — Live scrape swaps results below the form (H-3)

Route: `POST /scrape` (`post_form_htmx`, REUSED FakeGithub). Swap target: `#scrape-results`.

### H-3a — htmx fragment (candidates + derived-from, no sign control)
- **Given** network available and a target that would propose candidates
- **When** she submits the target WITH the header (`post_form_htmx`)
- **Then** ONLY the `#scrape-results` region updates with the candidates + their derived-from, NO sign control, response is a fragment
- Guards: I-HX-1, BR-HX-4 (no sign), BR-HX-5 (derived-from). Fn: `submitting_scrape_with_htmx_returns_only_the_results_fragment`

### H-3b — edge: zero candidates
- **Given** a target derives no candidates
- **When** she submits WITH the header
- **Then** the results fragment shows "No candidate claims could be derived" with a suggestion
- Guards: NFR-HX-7 (guided). Fn: `scrape_with_no_candidates_swaps_in_guidance_fragment`

### H-3c — error: network down, no leak
- **Given** GitHub is unreachable
- **When** she submits WITH the header
- **Then** the fragment states GitHub could not be reached, notes the store view still works offline, leaks NO transport/stack internals
- Guards: NFR-HX-7 (no-leak, DV-4 payload-free). Fn: `scrape_network_down_swaps_in_offline_guidance_fragment_without_leaking`

### H-3d — no-JS full page
- **Given** JavaScript is disabled
- **When** she submits the form WITHOUT the header (`post_form`)
- **Then** the server returns the COMPLETE slice-06 `/scrape` page with the candidates below the form
- Guards: I-HX-1, I-HX-4. Fn: `submitting_scrape_without_htmx_returns_the_full_page`

### H-3e — parity
- **Given** a target that would propose candidates
- **When** `POST /scrape` is submitted both shapes
- **Then** the fragment's candidates + derived-from + verbatim confidence are contained in the full page's results region
- Guards: I-HX-5. Fn: `scrape_results_fragment_equals_the_full_page_results_region`

## US-HX-004 — Claim detail loads inline (H-4)

Route: `GET /claims/{cid}`. Swap target: `#claim-detail`. The shape fork is AFTER the
found/not-found decision — the 404 carries through BOTH shapes.

### H-4a — htmx fragment
- **Given** Maria's claim has two evidence URLs
- **When** she opens it WITH the header (`get_htmx`)
- **Then** ONLY the `#claim-detail` region updates with all fields + both evidence URLs + verbatim 0.90, response is a fragment
- Guards: I-HX-1, FR-VIEW-8 (verbatim). Fn: `opening_a_claim_with_htmx_returns_only_the_detail_fragment`

### H-4b — no-JS full page
- **Given** JavaScript is disabled (or direct URL)
- **When** she opens `/claims/{cid}` WITHOUT the header
- **Then** the server returns the COMPLETE slice-06 detail page
- Guards: I-HX-1, I-HX-4. Fn: `opening_a_claim_without_htmx_returns_the_full_detail_page`

### H-4c — error: unknown CID guided in both shapes
- **Given** no claim with the requested CID exists
- **When** she opens it WITH or WITHOUT the header
- **Then** both shapes show "No claim with that identifier in your store" + a `/claims` back link (404 carried through both)
- Guards: NFR-HX-7. Fn: `unknown_cid_guides_the_operator_in_both_shapes`

### H-4d — edge: no evidence in both shapes
- **Given** a claim signed without evidence
- **When** she opens its detail WITH and WITHOUT the header
- **Then** both shapes show "no evidence attached", never a blank section
- Guards: NFR-HX-7. Fn: `claim_with_no_evidence_renders_clearly_in_both_shapes`

### H-4e — parity
- **Given** the claim has two evidence URLs
- **When** `/claims/{cid}` is fetched both shapes
- **Then** the fields + evidence + verbatim confidence are contained in the full page's detail region
- Guards: I-HX-5, FR-VIEW-8. Fn: `claim_detail_fragment_equals_the_full_page_detail_region`

## US-HX-006 — Switch My Claims ↔ Peer Claims in place (H-6)

Route: `GET /claims` ↔ `GET /peer-claims`. Swap target: `#view-panel`. `hx-push-url` is
client-side (the HTTP harness can't run JS, ADR-035) — so we assert the SERVER contract:
each URL serves the correct fragment under the header and the correct full page without it.

### H-6a — htmx fragment into `#view-panel`
- **Given** Maria is on My Claims with a federated peer set
- **When** she switches to Peer Claims WITH the header (`get_htmx("/peer-claims")`)
- **Then** ONLY the view-panel updates to the Peer Claims list with each row's origin, separable from own claims, response is a fragment
- Guards: I-HX-1, KPI-VIEW-3. Fn: `switching_to_peer_claims_with_htmx_returns_only_the_view_panel_fragment`

### H-6b — no-JS full page per URL
- **Given** JavaScript is disabled
- **When** each tab URL (`/claims`, `/peer-claims`) is fetched WITHOUT the header
- **Then** each returns the COMPLETE slice-06 full page for that view (converging with the no-JS real-URL path)
- Guards: I-HX-1, ADR-034. Fn: `tab_switch_without_htmx_returns_the_full_page_per_url`

### H-6c — edge: bookmark/reload re-enters via the full page
- **Given** Maria switched to Peer Claims and bookmarked the page
- **When** she opens the bookmark (a plain GET, no header)
- **Then** she lands on the COMPLETE slice-06 `/peer-claims` page
- Guards: ADR-034 (URL convergence). Fn: `bookmark_of_the_switched_view_re_enters_via_the_full_page`

### H-6d — parity
- **Given** a federated peer set
- **When** `/peer-claims` is fetched both shapes
- **Then** the fragment's rows + peer origin are contained in the full page's view-panel region
- Guards: I-HX-5, KPI-VIEW-3. Fn: `view_panel_fragment_equals_the_full_page_view_panel_region`

---

## Guardrail / gold scenarios (`viewer_htmx_invariants.rs`)

### US-HX-005 — htmx served locally so swaps work offline (@infrastructure, H-5)

#### H-5a — `htmx_asset_served_locally`
- **Given** the viewer is running
- **When** `GET /static/htmx.min.js` is fetched
- **Then** it returns 200 with the vendored htmx JS (non-empty, looks like htmx)
- Guards: I-HX-2, FR-HX-6, ADR-031. `@infrastructure @offline @asset @gold`

#### H-5b — `no_viewer_page_references_an_external_cdn` (`@property`)
- **Given** the viewer is serving its routes
- **When** the served HTML of every page-bearing route is inspected
- **Then** no page references an external CDN; every page references the local `/static/htmx.min.js`
- Guards: I-HX-2, BR-HX-6 (offline guarantee, structural). `@property @no-cdn @gold`

#### H-5c — `serving_the_asset_adds_no_write_surface` (`@property`)
- **Given** the local htmx asset is served (loopback bind)
- **When** the asset route is fetched
- **Then** the store row counts are unchanged and no write/sign route is introduced
- Guards: I-HX-3, I-VIEW-1/2/4. `@property @i-hx-3 @gold`

### No-regression — `non_htmx_responses_are_byte_equivalent_to_slice_06` (H-INV-NoReg, `@property`)
- **Given** the htmx enhancement is layered on
- **When** each enhanced route is requested WITHOUT the header
- **Then** each returns the complete slice-06 full page (chrome + content), no CDN reference, no behavioral change
- Guards: I-HX-4, NFR-HX-4. Companion: the slice-06 26-scenario suite stays green (release gate).

### Read-only — `htmx_fragment_routes_leave_the_store_read_only` (H-INV-ReadOnly, `@property`)
- **Given** a store seeded with own + peer claims and a reachable scrape target
- **When** EVERY htmx fragment route (incl. `POST /scrape` via `post_form_htmx`) is exercised
- **Then** the `claims` + `peer_claims` row counts are UNCHANGED (universe-bound `assert_store_read_only`, Mandate 8)
- Guards: I-HX-3, NFR-HX-3, BR-HX-4.

### No new write surface — `no_swap_route_adds_a_write_or_sign_surface` (H-INV-NoWrite, `@property`)
- **Given** the viewer serving the htmx-enhanced routes over a populated store
- **When** every htmx fragment route is requested
- **Then** no fragment renders a sign control (the human gate stays in the CLI)
- Guards: I-HX-3, I-SCR-1.

---

## Scenario inventory (28 total — 22 interaction + 6 guardrail)

| File | Count | Tags / driver |
|---|---|---|
| `viewer_htmx.rs` | 22 | `get`/`get_htmx`/`post_form`/`post_form_htmx`; H-1..H-6 (htmx + no-JS + parity per story) |
| `viewer_htmx_invariants.rs` | 6 | `@property`/`@gold`; US-HX-005 asset/offline + read-only + no-regression + no-write |

`[[test]]` registrations added to `crates/cli/Cargo.toml`: `viewer_htmx`, `viewer_htmx_invariants`.

## RED classification (pre-DELIVER fail-for-right-reason gate)

Both targets COMPILE (`cargo test --no-run` Finished). The walking-skeleton scenario was run
and classified **RED / MISSING_FUNCTIONALITY** — it panics at `not yet implemented` (the
`todo!()` macro) inside the test body, reaching the assertions, NOT an ImportError /
FixtureBroken / SetupFailure. The `get_htmx` / `post_form_htmx` seam compiles (it only adds a
header to the existing reqwest call), so every scenario fails at RUNTIME for a business reason
(the fragment shape / asset route is unimplemented) — the whole-corpus RED gate holds. DELIVER
unskips + fills one scenario at a time (start with H-1a, the walking skeleton).
