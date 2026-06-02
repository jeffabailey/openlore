<!-- markdownlint-disable MD024 -->
# User Stories: viewer-htmx-swaps (slice-07)

> **DELTA** on slice-06 (`htmx-scraper-viewer`). htmx partial-swaps as a progressive
> enhancement over the SAME read-only `openlore ui` routes. Every value-producing story traces
> to the single job **navigate-without-reloads** ("navigate the local read/view surface
> without full-page reloads"). `job_id` references `docs/product/jobs.yaml` job **J-002**
> ("Read another developer's / my own claims and SEE who claims what" — the read/orient/inspect
> job). The viewer is a **read-only view surface over existing signed claims**, so it serves
> J-002 (orienting/inspecting), NOT J-001 (authoring/signing, which the viewer explicitly does
> NOT do). slice-07 narrows J-002 to *navigating* that view smoothly. No JTBD re-run (single
> clear job; configured JTBD=No). Signing stays in the CLI (I-SCR-1 / I-VIEW-3).

## System Constraints (cross-cutting — apply to every story)

- **Progressive enhancement** (I-HX-1, NFR-HX-1): every route serves a complete slice-06 full
  page when `HX-Request` is absent (no-JS, direct URL, bookmark, view-source, curl); htmx
  requests get the fragment of the SAME content. No new data routes; only the response shape
  varies by header.
- **htmx served locally** (I-HX-2, NFR-HX-2): vendored/inlined, never a CDN; works offline
  (inherits I-VIEW-6 / KPI-VIEW-5).
- **Read-only preserved** (I-HX-3, NFR-HX-3): no swap adds a write/sign route; no key in the
  web process (inherits I-VIEW-1/2 / I-SCR-1).
- **No regression** (I-HX-4, NFR-HX-4): non-htmx responses byte-equivalent to slice-06; the
  slice-06 26-scenario acceptance suite stays green.
- **Fragment/full-page parity** (I-HX-5, NFR-HX-5): the fragment equals the full page's
  corresponding region (same rows, "X–Y of N", verbatim confidence, peer origin).
- **HTTP surface unchanged** (BR-HX-1): existing GET routes + existing `POST /scrape`; only
  the optional local htmx asset route is new.
- Tech choices deferred to DESIGN (OD-HX-1..6). All AC are testable by sending/withholding the
  `HX-Request` header against the real `openlore ui`.

---

## US-HX-001: Pagination swaps the claims table in place (Walking Skeleton)

- **job_id**: J-002 (navigate-without-reloads — page the read/view surface smoothly)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: When Maria clicks Next on `http://127.0.0.1:8788/claims`, the whole page
  reloads — the document repaints, her scroll jumps to the top, and there is a visible flash.
- **After**: Maria clicks Next and only the claims table updates in place (rows + "51–100 of
  312" + Prev/Next); the page holds still and keeps her scroll position. With JavaScript off,
  the same Next link returns the complete slice-06 `/claims?page=2` page.
- **Decision enabled**: Maria can scan through a real-sized store page by page without losing
  her place or waiting on a repaint, and decide where in her claims to look next.

### Problem

Maria Santos is a node operator who, since slice-06, can finally see her store in a browser.
But paging is heavy: every Next/Prev on `/claims` triggers a full-page reload that jumps her
scroll to the top and flashes. On a real-sized store this makes browsing feel like reloading
a form, not navigating a place — undercutting the slice-06 "see at a glance" win at scale.

### Who

- Node operator | on localhost, paging her own `claims` list | motivated to browse a
  real-sized store without losing her place or waiting on full repaints.

### Solution

On `GET /claims?page=N`, branch on the `HX-Request` header in the effect-shell handler: present
→ return just the **claims-table fragment** (rows + position indicator + Prev/Next) swapped
into the table region; absent → return the **full slice-06 page**, byte-equivalent. This is
the thinnest end-to-end htmx thread — it proves header-drives-shape + fragment/full-page
parity + no-JS fallback on the SAME route. The skeleton carries a minimal local htmx
reference so the swap can fire (full asset hardening is US-HX-005).

### Domain Examples

#### 1: Happy path — htmx swap
Maria has 312 claims at page size 50, viewing page 1 scrolled partway down. She clicks Next;
only the table swaps to show "51–100 of 312" starting with `("tokio-rs/tokio","has-license",
"MIT")` at 0.95; her scroll and the read-only banner are untouched; no flash.

#### 2: Edge — no JavaScript / direct URL
With JS disabled, Maria clicks Next (a plain link to `/claims?page=2`); the server returns the
complete `/claims?page=2` page, byte-equivalent to slice-06.

#### 3: Boundary — over-the-end page clamps
A request asks for a page past the last (e.g. `?page=99` on 312 claims); both the fragment and
the full page show the last page "301–312 of 312", not a blank result (slice-06 DV-5 clamp
preserved in both shapes).

### UAT Scenarios (BDD)

#### Scenario: Paging the claims list updates only the table, in place
Given Maria has 312 signed claims rendered 50 per page and is scrolled partway down page 1
When her browser requests page 2 as an htmx request with the HX-Request header
Then the response is just the claims-table fragment showing 51–100 of 312
And the page chrome, navigation, and scroll position are unchanged
And no full-page reload occurs

#### Scenario: The claims page works as a full page without JavaScript
Given JavaScript is disabled in Maria's browser
When she clicks Next as a plain link to /claims?page=2
Then the server returns the complete /claims?page=2 page
And it is byte-equivalent to the slice-06 full page

#### Scenario: An over-the-end page clamps to the last page in both shapes
Given Maria has 312 claims at page size 50 (last page is 301–312 of 312)
When a request asks for a page beyond the last page
Then the response shows 301–312 of 312, not a blank result
And this holds for both the htmx fragment and the full page

#### Scenario: The swap adds no write or sign surface
Given the htmx-enhanced /claims route is serving swaps
Then no swap introduces a write or sign route
And the web process still holds no signing key

### Acceptance Criteria

- [ ] `GET /claims?page=N` with `HX-Request` returns only the claims-table fragment (rows + "X–Y of N" + Prev/Next).
- [ ] `GET /claims?page=N` without `HX-Request` returns the complete slice-06 full page, byte-equivalent.
- [ ] The htmx swap replaces only the table region; chrome, nav, and scroll position are unchanged.
- [ ] Over-the-end `?page` clamps to the last page in BOTH shapes (no blank result).
- [ ] Confidence renders verbatim in the fragment (FR-VIEW-8 parity).
- [ ] No write/sign route is added; key-access audit stays zero.

### Outcome KPIs

- **Who**: node operator paging her claims | **Does what**: pages the claims list with an in-place swap instead of a full reload | **By how much**: Next/Prev on `/claims` is a partial swap (table-only) for htmx requests, full page for non-htmx, in 100% of cases | **Measured by**: KPI-HX-1 (swap vs reload, via HX-Request presence/absence against the real `openlore ui`) | **Baseline**: slice-06 — every Next/Prev is a full-page reload.

### Technical Notes

- Read-only `GET`; reuses slice-06 `StoreReadPort` list/count + PageView/clamp. No new data route.
- Rendering split (fragment vs page) is OD-HX-2; HX-Request branch location is OD-HX-5; swap ids OD-HX-3; asset OD-HX-1.
- Depends on: slice-06 `/claims` + PageView (exist); a minimal local htmx reference.

---

## US-HX-002: Pagination swaps the peer-claims table in place

- **job_id**: J-002 (navigate-without-reloads — page the federated list smoothly)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: Maria's `/peer-claims` view holds 1,840 federated rows; every Next reloads the
  whole page and jumps her scroll — the most painful paging in the viewer.
- **After**: Maria pages her federated peer claims with an in-place table swap; each row still
  shows its origin and stays separable from her own claims. JS off → full slice-06 page.
- **Decision enabled**: Maria can browse a large federated set page by page to judge whether
  her federation set looks right, without losing her place.

### Problem

Maria is a node operator on a federated node with 1,840 peer claims from 4 peers. Paging this
large list is where the full-reload jolt hurts most: every Next repaints the document and
resets her scroll, making the federated set tedious to scan.

### Who

- Federated node operator | paging `/peer-claims` on localhost | motivated to scan a large
  federated set without performance pain or losing position.

### Solution

Apply the US-HX-001 pattern to `GET /peer-claims?page=N`: `HX-Request` present → peer-claims-
table fragment (rows with origin + position indicator + Prev/Next) swapped in place; absent →
full slice-06 page. Peer origin and separability from own claims are preserved in the fragment
(KPI-VIEW-3 carried).

### Domain Examples

#### 1: Happy path — htmx swap
Maria's 1,840 peer claims page 50 at a time; she clicks Next and the peer table swaps to the
next 50, each row showing origin (e.g. `axum/axum has-license MIT 0.88 origin: peer-A`);
chrome unchanged.

#### 2: Edge — no JavaScript
With JS off, Next is a plain link to `/peer-claims?page=2`; the full slice-06 page returns.

#### 3: Boundary — missing origin still renders
A peer row with no recorded origin still renders in the fragment labeled origin "unknown",
never dropped (carries slice-06 behavior into the fragment shape).

### UAT Scenarios (BDD)

#### Scenario: Paging the peer-claims list updates only the table, in place
Given Maria has 1,840 federated peer claims from 4 peers rendered 50 per page
When her browser requests the next page as an htmx request with the HX-Request header
Then the response is just the peer-claims-table fragment with the next 50 rows and their origin
And the peer rows are separable from her own claims
And the page chrome and navigation are unchanged

#### Scenario: The peer-claims page works as a full page without JavaScript
Given JavaScript is disabled
When Maria clicks Next as a plain link to /peer-claims?page=2
Then the server returns the complete slice-06 /peer-claims?page=2 page

#### Scenario: A peer row with unknown origin still renders in the fragment
Given a federated peer claim has no recorded origin
When Maria pages the Peer Claims list as an htmx request
Then that row still renders in the fragment with origin shown as "unknown"

### Acceptance Criteria

- [ ] `GET /peer-claims?page=N` with `HX-Request` returns only the peer-claims-table fragment (rows + origin + "X–Y of N" + Prev/Next).
- [ ] Without `HX-Request`, returns the complete slice-06 full page, byte-equivalent.
- [ ] Peer rows keep origin and remain separable from own claims in the fragment.
- [ ] Missing origin renders as "unknown" in the fragment, never dropped.
- [ ] Over-the-end `?page` clamps to the last page in both shapes.

### Outcome KPIs

- **Who**: federated node operator | **Does what**: pages the federated peer list with an in-place swap | **By how much**: Next/Prev on `/peer-claims` is a table-only swap for htmx requests, full page for non-htmx, 100% of cases, origin preserved | **Measured by**: KPI-HX-1 | **Baseline**: slice-06 — full-page reload on every peer-list page.

### Technical Notes

- Reuses slice-06 `/peer-claims` + PageView; same rendering split (OD-HX-2) and swap pattern as US-HX-001.
- Depends on: US-HX-001 (pattern), slice-06 `/peer-claims` (exists).

---

## US-HX-003: Live scrape swaps results below the form

- **job_id**: J-002 (navigate-without-reloads — triage proposals in place)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: When Maria submits a scrape target on `/scrape`, the whole page reloads to show
  the proposed candidates — the form clears its place and the document flashes.
- **After**: Maria submits a target and only the results region updates below the form to show
  the candidates (or zero-candidate / network-down guidance); the form and its value stay; no
  sign control; nothing is saved. JS off → full slice-06 `/scrape` page.
- **Decision enabled**: Maria can triage which scrape candidates are worth signing in a
  scannable list that appears in place, then run the CLI sign command for the ones she picks.

### Problem

Maria is a node operator who, before signing scraped claims, triages candidates in the browser
(slice-06). But submitting a target reloads the whole `/scrape` page, flashing the document and
losing the form's place — friction on a view she uses iteratively to compare targets.

### Who

- Node operator triaging scrape candidates | localhost, network available | motivated to
  compare proposals iteratively without a reload each submit, then sign in the CLI.

### Solution

On `POST /scrape`, branch on `HX-Request`: present → return just the **results fragment**
(candidates with derived-from, OR the zero-candidate guidance, OR the network-down guidance
that notes the store view still works offline) swapped into the results region below the form;
absent → full slice-06 `/scrape` page. NO sign control in either shape; nothing persisted;
re-harvests on each submit (carries BR-VIEW-1/2, I-SCR-1).

### Domain Examples

#### 1: Happy path — htmx swap
Maria enters `tokio-rs/tokio` and submits; 7 candidates swap in below the form, e.g.
`("tokio-rs/tokio","has-license","MIT")` at 0.95 with derived-from "LICENSE @ HEAD"; the
results say nothing is signed/saved and direct her to the CLI; the form keeps `tokio-rs/tokio`.

#### 2: Edge — no candidates derived
She submits `some-org/empty-repo`; the results region swaps to "No candidate claims could be
derived" with a suggestion — no reload.

#### 3: Error — network unavailable
Offline, she submits `tokio-rs/tokio`; the results region swaps to "GitHub could not be
reached" and notes her store view still works offline (no transport/stack internals leaked).

### UAT Scenarios (BDD)

#### Scenario: Submitting a scrape target swaps in the proposals without reloading
Given Maria is on the Live Scrape view with network available
And a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
When she submits the target as an htmx request with the HX-Request header
Then only the results region updates to show the 7 candidates with their derived-from
And the form and its target value remain in place
And the results state that nothing is signed or saved and direct her to the CLI
And no candidate is persisted
And no sign control is rendered

#### Scenario: A target that yields no candidates swaps in guidance
Given a live scrape of "some-org/empty-repo" derives no candidates
When Maria submits that target as an htmx request
Then the results region shows "No candidate claims could be derived" with a suggestion

#### Scenario: Network failure swaps in guidance that the store view still works offline
Given Maria cannot reach GitHub
When she submits "tokio-rs/tokio" as an htmx request
Then the results region shows that GitHub could not be reached
And it states her store view still works offline

#### Scenario: Scrape submit works as a full page without JavaScript
Given JavaScript is disabled
When Maria submits the scrape form as a plain POST /scrape
Then the server returns the complete /scrape page with the candidates below the form

### Acceptance Criteria

- [ ] `POST /scrape` with `HX-Request` returns only the results fragment (candidates / zero-candidate / network-down) swapped below the form.
- [ ] The form and its target value remain after the swap.
- [ ] NO sign control is rendered in the fragment; nothing is persisted; refresh re-harvests.
- [ ] Candidates show derived-from; persisted-view fragments never show derived-from (BR-HX-5).
- [ ] Network-down fragment notes the store view still works offline and leaks no internals.
- [ ] Without `HX-Request`, returns the complete slice-06 `/scrape` page.

### Outcome KPIs

- **Who**: node operator triaging scrape candidates | **Does what**: submits a target and reviews proposals via an in-place swap | **By how much**: `POST /scrape` results appear as a partial swap (results-only) for htmx requests, full page for non-htmx, 100% of cases, with no sign control and nothing persisted | **Measured by**: KPI-HX-2 + guardrail KPI-HX-G3 | **Baseline**: slice-06 — every submit reloads the whole `/scrape` page.

### Technical Notes

- Reuses slice-06 `POST /scrape` → `GithubPort` + `derive_candidates`; no persist, no sign (I-SCR-1).
- Network-down render carries the slice-06 DV-4 payload-free error pattern (no-leak by type).
- Depends on: US-HX-001 (pattern), slice-06 `/scrape` (exists). Rendering split OD-HX-2; ids OD-HX-3.

---

## US-HX-004: Claim detail loads inline

- **job_id**: J-002 (navigate-without-reloads — inspect a claim in place)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: When Maria clicks a claim on `/claims`, the browser navigates away to
  `/claims/{cid}` — she leaves the list and loses her scroll position.
- **After**: Maria clicks a claim and its detail (all fields + evidence) loads into an inline
  panel without leaving the list. JS off / direct URL → the full slice-06 detail page.
- **Decision enabled**: Maria can verify a claim's evidence and decide whether it is
  well-supported, without losing her place in the list she was scanning.

### Problem

Maria is a node operator verifying the evidence behind specific claims. In slice-06, opening a
claim navigates away to its detail page; to check several claims she repeatedly leaves and
returns to the list, losing her scroll position each time.

### Who

- Node operator reviewing several claims in a sitting | localhost | motivated to inspect a
  claim's evidence without leaving the list she is scanning.

### Solution

On `GET /claims/{cid}`, branch on `HX-Request`: present → return just the **claim-detail
fragment** (all fields + complete evidence[], confidence verbatim) swapped into an inline
panel; absent → full slice-06 detail page. Unknown CID returns the guided not-found fragment;
a claim with no evidence shows "no evidence attached" — both in fragment and full page.

### Domain Examples

#### 1: Happy path — htmx swap
Maria clicks her claim `bafyrei...1` (two evidence URLs); the detail panel swaps in showing
subject `rust-lang/rust`, confidence verbatim `0.90`, author `did:plc:maria...`, composed
`2026-04-18T09:12:03Z`, and both evidence URLs; the list stays in place.

#### 2: Edge — no JavaScript / direct URL
With JS off (or opening `/claims/bafyrei...1` directly), the full slice-06 detail page returns.

#### 3: Error — unknown CID
Maria opens `/claims/bafyrei...zzz` (not in the store); both the fragment and the full page
show "No claim with that identifier in your store" with a link back to the list.

### UAT Scenarios (BDD)

#### Scenario: Opening a claim loads its detail inline without leaving the list
Given Maria is on the My Claims view and her claim bafyrei...1 has two evidence URLs
When she opens that claim as an htmx request with the HX-Request header
Then the detail region updates to show all claim fields and both evidence URLs
And the claims list remains in place
And the confidence is shown verbatim as 0.90

#### Scenario: Opening a claim works as a full page without JavaScript
Given JavaScript is disabled
When Maria clicks a claim row as a plain link to /claims/bafyrei...1
Then the server returns the complete slice-06 claim detail page

#### Scenario: An unknown claim id guides the operator in both shapes
Given no claim with CID bafyrei...zzz exists in the store
When Maria opens that claim as an htmx request or as a full page
Then she sees "No claim with that identifier in your store" with a link back to the list

### Acceptance Criteria

- [ ] `GET /claims/{cid}` with `HX-Request` returns only the claim-detail fragment (all fields + complete evidence[], confidence verbatim).
- [ ] The claims list remains in place during the inline swap.
- [ ] Unknown CID returns the guided not-found fragment with a back link (and the full page does the same).
- [ ] A claim with no evidence shows "no evidence attached" in both shapes.
- [ ] Without `HX-Request`, returns the complete slice-06 detail page.

### Outcome KPIs

- **Who**: node operator inspecting claims | **Does what**: opens a claim's detail via an inline swap | **By how much**: `GET /claims/{cid}` is a detail-panel swap for htmx requests (list preserved), full page for non-htmx, 100% of cases | **Measured by**: KPI-HX-3 | **Baseline**: slice-06 — opening a claim navigates away from the list.

### Technical Notes

- Reuses slice-06 `GET /claims/{cid}` + `get_claim` + evidence. Confidence verbatim (FR-VIEW-8).
- Depends on: US-HX-001 (pattern), slice-06 detail route (exists). Rendering split OD-HX-2; inline panel id OD-HX-3.

---

## US-HX-005: htmx served locally so swaps work offline

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story builds no new user-visible behavior; it hardens the
  delivery of the htmx asset (vendored/inlined, single-source, no CDN) so that the user-facing
  swaps in US-HX-001..004 and US-HX-006 keep working fully offline. It exists solely to make
  the offline guardrail (I-HX-2 / I-VIEW-6 / KPI-VIEW-5) structural for the value-producing
  stories. It enables no new operator decision on its own and therefore carries no Elevator
  Pitch (per the @infrastructure exemption). Its value is realized through the stories it
  supports; per slice-level check, the slice's value comes from US-HX-001..004/006, all of
  which ARE user-visible — so the slice is not infrastructure-only.
- **Release**: R4 (hardening; DESIGN may fold forward) | **MoSCoW**: Must | **Priority**: P5
- **Tags**: @infrastructure

### Problem

The slice-06 viewer promises the dashboard works fully offline (I-VIEW-6 / KPI-VIEW-5). htmx
swaps require the htmx JavaScript library; if that library were loaded from a CDN, every swap
would break the moment Maria's machine is offline — silently breaking the offline guarantee
the operator relies on. The asset must be served by the viewer itself, from a single source,
with no off-host reference.

### Who

- Node operator on an offline / air-gapped machine | localhost | relying on the dashboard
  (including its swaps) working with no network, as slice-06 promised.

### Solution

Serve the htmx library from the viewer process itself — a vendored static asset (e.g.
`GET /static/htmx.min.js`) or inlined into the page chrome (OD-HX-1) — from a single source
that every page references; no page references a CDN. The offline test is the gate: with the
network down, every store view AND every swap still works.

### Domain Examples

#### 1: Happy path — offline swap
Maria's laptop has no network. She loads `/claims`, clicks Next, opens a claim, and switches
tabs; htmx loads from the viewer process itself and every swap works.

#### 2: Edge — view-source audit
Maria (or a reviewer) views source on every route; no page references an off-host URL to load
htmx; the library comes from loopback or is inlined.

#### 3: Boundary — single source
The asset has exactly one source; there is no second copy that could drift to a different
htmx version.

### UAT Scenarios (BDD)

#### Scenario: htmx-powered swaps keep working with no network
Given Maria's machine has no network access
When she loads any viewer route and triggers a swap (page, open a claim, switch tabs)
Then the htmx library loads from the viewer process itself, not a CDN
And every store view and every swap still works fully offline

#### Scenario: No viewer page references an external CDN for htmx
Given the viewer is serving any route
When the served HTML is inspected
Then no page references an off-host URL to load the htmx library

#### Scenario: Serving the asset adds no write or sign surface
Given the local htmx asset is served (e.g. via a static route or inlined)
Then no write or sign route is introduced
And the web process still holds no signing key

### Acceptance Criteria

- [ ] htmx is served by the viewer process (vendored asset route or inlined), never from a CDN.
- [ ] With the network down, every store view AND every swap still works.
- [ ] No served page references an off-host URL for htmx (property-checkable).
- [ ] The asset has a single source (no drifting second copy).
- [ ] Serving the asset adds no write/sign route; key-access audit stays zero; bind stays loopback-only.

### Outcome KPIs

- **Who**: offline node operator | **Does what**: uses the viewer (including swaps) with no network | **By how much**: 100% of swaps and store views work offline; zero pages reference a CDN | **Measured by**: KPI-HX-G2 (offline, hardened to a property) + KPI-HX-G3 (no new write surface) | **Baseline**: slice-06 store views already work offline; this extends that guarantee to swaps.

### Technical Notes

- Asset mechanism is OD-HX-1 (static route vs inline; pinned version/integrity). Loopback-only (I-VIEW-4).
- Used by US-HX-001..004 and US-HX-006 (all swaps depend on the asset). The skeleton (US-HX-001) carries a minimal local reference; this story hardens it.

---

## US-HX-006: Switch My Claims ↔ Peer Claims in place

- **job_id**: J-002 (navigate-without-reloads — switch views in place)
- **Release**: R3 | **MoSCoW**: Should | **Priority**: P4

### Elevator Pitch

- **Before**: Switching between My Claims and Peer Claims reloads the whole page each time, so
  comparing "mine vs federated" means a full repaint on every toggle.
- **After**: Maria clicks a tab and only the active view panel swaps in place; the browser URL
  updates to `/claims` or `/peer-claims` so the view is bookmarkable and Back works. JS off →
  the tab is a plain link to the full slice-06 page.
- **Decision enabled**: Maria can flip between her own and federated claims as one continuous
  place to judge whether her federation set looks right — and bookmark or Back to the view she
  was on.

### Problem

Maria is a node operator who compares her own claims against her federated peer claims. In
slice-06, switching tabs reloads the whole page each toggle, flashing the document — friction
on a comparison she makes repeatedly.

### Who

- Federated node operator comparing own vs federated claims | localhost | motivated to flip
  between the two views as one place, with the URL still reflecting where she is.

### Solution

Make the My Claims ↔ Peer Claims tabs swap the **active view-panel fragment** in place under
`HX-Request` (`GET /claims` / `GET /peer-claims`), preserving peer origin and separability;
update the browser URL/history (mechanism is OD-HX-4, e.g. `hx-push-url`) so the active view
is bookmarkable and Back works, converging with the no-JS real-URL path. Without `HX-Request`,
the tab is a plain link to the full slice-06 page.

### Domain Examples

#### 1: Happy path — htmx swap
Maria is on My Claims; she clicks the Peer Claims tab; only the view panel swaps to the Peer
Claims list (rows with origin, separable from her own); the URL becomes `/peer-claims`.

#### 2: Edge — bookmark / Back
After switching to Peer Claims, Maria bookmarks the page and later opens the bookmark; she
lands on the full `/peer-claims` page. Pressing Back after a swap returns her to My Claims.

#### 3: Edge — no JavaScript
With JS off, the Peer Claims tab is a plain link to `/peer-claims`; the full slice-06 page
returns.

### UAT Scenarios (BDD)

#### Scenario: Switching to Peer Claims swaps the view panel in place
Given Maria is on the My Claims view with 1,840 federated peer claims from 4 peers
When she switches to Peer Claims as an htmx request with the HX-Request header
Then only the view panel updates to the Peer Claims list showing each row's origin
And the peer rows are separable from her own claims
And the browser URL reflects /peer-claims so the view is bookmarkable and Back works

#### Scenario: The tabs work as full-page navigation without JavaScript
Given JavaScript is disabled
When Maria clicks the Peer Claims tab as a plain link to /peer-claims
Then the server returns the complete slice-06 /peer-claims page

#### Scenario: A bookmark of the switched view re-enters via the full page
Given Maria switched to Peer Claims and bookmarked the page
When she later opens that bookmark
Then she lands on the complete /peer-claims page

### Acceptance Criteria

- [ ] Tab switch with `HX-Request` swaps only the active view-panel fragment in place.
- [ ] Peer rows keep origin and remain separable from own claims in the swapped panel.
- [ ] The browser URL updates to `/claims` or `/peer-claims` so the view is bookmarkable and Back works.
- [ ] Without `HX-Request`, the tab is a plain link returning the complete slice-06 page.
- [ ] Reloading the switched-to URL yields the full slice-06 page for that view.

### Outcome KPIs

- **Who**: federated node operator | **Does what**: switches between own and federated views via an in-place swap | **By how much**: a tab switch is a view-panel swap for htmx requests (URL updated, bookmarkable), full page for non-htmx, 100% of cases | **Measured by**: KPI-HX-4 | **Baseline**: slice-06 — every tab switch is a full-page reload.

### Technical Notes

- URL/history strategy is OD-HX-4 (e.g. `hx-push-url`), converging with the no-JS real-URL path.
- Depends on: US-HX-001/002 (list rendering + pattern), slice-06 `/claims` & `/peer-claims` (exist). View-panel id OD-HX-3.
