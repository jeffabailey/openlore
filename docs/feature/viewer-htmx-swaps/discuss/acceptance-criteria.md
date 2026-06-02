<!-- markdownlint-disable MD024 -->
# Acceptance Criteria: viewer-htmx-swaps (slice-07)

> Given-When-Then, derived from the journey + user stories. **Every story carries three
> guardrail layers**: (a) the **htmx-request fragment** behavior, (b) the **no-JS full-page
> fallback**, and (c) the **read-only / offline** guarantees. All criteria are testable by
> **sending or withholding the `HX-Request` header** against the real `openlore ui` process
> (the slice-06 `ViewerServer` harness convention, extended per OD-HX-6). Non-htmx responses
> must stay **byte-equivalent to slice-06** (the 26-scenario slice-06 corpus is the regression
> gate).

## Cross-cutting guardrails (apply to every story)

### @property — Every non-htmx response is the complete slice-06 page
Given a request WITHOUT the `HX-Request` header (curl, bookmark, no-JS, view-source)
When it hits any viewer route
Then the response is the complete slice-06 full page for that route, byte-equivalent

### @property — htmx swaps add no write or sign surface
Given the viewer is serving the htmx-enhanced routes
Then no swap introduces a write or sign route
And the web process still holds no signing key (key-access audit stays zero)
And the bind stays loopback-only (127.0.0.1)

### @property — The slice-06 acceptance suite stays green
Given the slice-06 viewer acceptance corpus (26 scenarios)
When the htmx enhancement is layered on
Then all 26 slice-06 scenarios still pass with zero regression

### @property — Fragment/full-page parity
Given identical inputs to a route
Then the fragment returned under `HX-Request` equals the corresponding region of the full page
(same rows, same "X–Y of N", verbatim confidence, peer origin)

---

## US-HX-001 — Pagination swaps the claims table in place (Walking Skeleton)

### Scenario: htmx-request fragment behavior
Given Maria has 312 signed claims rendered 50 per page and is scrolled partway down page 1
When her browser requests page 2 WITH the `HX-Request` header
Then the response is only the claims-table fragment showing 51–100 of 312 (rows + Prev/Next)
And the chrome, navigation, and scroll position are unchanged
And no full-page reload occurs

### Scenario: no-JS full-page fallback
Given JavaScript is disabled
When Maria requests `/claims?page=2` WITHOUT the `HX-Request` header (plain Next link)
Then the server returns the complete `/claims?page=2` page, byte-equivalent to slice-06

### Scenario: boundary — over-the-end page clamps in both shapes
Given 312 claims at page size 50 (last page is 301–312 of 312)
When a request asks for a page beyond the last
Then both the htmx fragment and the full page show 301–312 of 312, not a blank result

### Scenario: read-only / offline guardrail
Given the `/claims` swap is serving
Then confidence renders verbatim in the fragment
And no write/sign route is added
And the table fragment renders with no network access

---

## US-HX-002 — Pagination swaps the peer-claims table in place

### Scenario: htmx-request fragment behavior
Given Maria has 1,840 federated peer claims from 4 peers rendered 50 per page
When her browser requests the next page WITH the `HX-Request` header
Then the response is only the peer-claims-table fragment with the next 50 rows and their origin
And the peer rows are separable from her own claims
And the chrome and navigation are unchanged

### Scenario: no-JS full-page fallback
Given JavaScript is disabled
When Maria requests `/peer-claims?page=2` WITHOUT the header (plain Next link)
Then the server returns the complete slice-06 `/peer-claims?page=2` page

### Scenario: boundary — unknown origin still renders in the fragment
Given a federated peer claim has no recorded origin
When Maria pages the Peer Claims list WITH the `HX-Request` header
Then that row still renders in the fragment with origin shown as "unknown", never dropped

### Scenario: read-only / offline guardrail
Given the `/peer-claims` swap is serving
Then the peer-claims fragment renders with no network access
And no write/sign route is added

---

## US-HX-003 — Live scrape swaps results below the form

### Scenario: htmx-request fragment behavior
Given Maria is on the Live Scrape view with network available
And a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
When she submits the target WITH the `HX-Request` header
Then only the results region updates to show the 7 candidates with their derived-from
And the form and its target value remain in place

### Scenario: read-only guardrail — no sign control, nothing persisted
Given the scrape results have swapped in
Then no sign control is rendered in the fragment
And no candidate is persisted (a re-submit re-harvests)
And derived-from appears only on these candidate rows, never on `/claims` or `/peer-claims`

### Scenario: error — network down, store still works offline (no leak)
Given Maria cannot reach GitHub
When she submits "tokio-rs/tokio" WITH the `HX-Request` header
Then the results region shows that GitHub could not be reached
And it states her store view still works offline
And no transport or stack internals are leaked

### Scenario: edge — zero candidates
Given a live scrape of "some-org/empty-repo" derives no candidates
When Maria submits that target WITH the header
Then the results region shows "No candidate claims could be derived" with a suggestion

### Scenario: no-JS full-page fallback
Given JavaScript is disabled
When Maria submits the scrape form WITHOUT the header (plain `POST /scrape`)
Then the server returns the complete slice-06 `/scrape` page with candidates below the form

---

## US-HX-004 — Claim detail loads inline

### Scenario: htmx-request fragment behavior
Given Maria is on My Claims and her claim bafyrei...1 has two evidence URLs
When she opens that claim WITH the `HX-Request` header
Then the detail region updates to show all claim fields and both evidence URLs
And the claims list remains in place
And the confidence is shown verbatim as 0.90

### Scenario: no-JS full-page fallback
Given JavaScript is disabled (or the URL is opened directly)
When Maria opens `/claims/bafyrei...1` WITHOUT the header
Then the server returns the complete slice-06 claim detail page

### Scenario: error — unknown CID guided in both shapes
Given no claim with CID bafyrei...zzz exists in the store
When Maria opens that claim WITH or WITHOUT the header
Then she sees "No claim with that identifier in your store" with a link back to the list

### Scenario: edge — no evidence
Given Maria has a claim signed without evidence
When she opens its detail (either shape)
Then she sees "no evidence attached" rather than a blank section

### Scenario: read-only / offline guardrail
Given the detail swap is serving
Then the detail fragment renders with no network access
And no write/sign route is added

---

## US-HX-005 — htmx served locally so swaps work offline (@infrastructure)

### Scenario: htmx-request fragment behavior (offline)
Given Maria's machine has no network access
When she loads any route and triggers a swap (page, open a claim, switch tabs)
Then the htmx library loads from the viewer process itself, not a CDN
And every store view and every swap still works fully offline

### Scenario: read-only / offline guardrail — no CDN reference
Given the viewer is serving any route
When the served HTML is inspected (view-source)
Then no page references an off-host URL to load the htmx library
And the asset has a single source (no drifting second copy)

### Scenario: read-only guardrail — asset adds no write surface
Given the local htmx asset is served (static route or inlined)
Then no write or sign route is introduced
And the web process still holds no signing key
And the bind stays loopback-only

### Scenario: no-JS full-page fallback (asset absent path)
Given JavaScript is disabled (the asset never executes)
When Maria uses any route
Then she gets the complete slice-06 full pages and navigation, unaffected by the asset

---

## US-HX-006 — Switch My Claims ↔ Peer Claims in place

### Scenario: htmx-request fragment behavior
Given Maria is on My Claims with 1,840 federated peer claims from 4 peers
When she switches to Peer Claims WITH the `HX-Request` header
Then only the view panel updates to the Peer Claims list showing each row's origin
And the peer rows are separable from her own claims
And the browser URL reflects `/peer-claims` so the view is bookmarkable and Back works

### Scenario: edge — bookmark / Back re-enter via the full page
Given Maria switched to Peer Claims and bookmarked the page
When she later opens that bookmark (or reloads the URL)
Then she lands on the complete slice-06 `/peer-claims` page

### Scenario: no-JS full-page fallback
Given JavaScript is disabled
When Maria clicks the Peer Claims tab WITHOUT the header (plain link)
Then the server returns the complete slice-06 `/peer-claims` page

### Scenario: read-only / offline guardrail
Given the tab swap is serving
Then the view-panel fragment renders with no network access
And no write/sign route is added
