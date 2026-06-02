# Requirements: viewer-htmx-swaps (slice-07)

> **Brownfield DELTA** on slice-06 (`htmx-scraper-viewer`). The shipped `openlore ui` viewer
> already serves server-rendered **maud** HTML over six read-only routes (built "htmx-ready,
> progressive enhancement") — but **today every route returns a FULL page**. This slice
> layers real **htmx partial-swaps** on the SAME routes so the node operator gets in-place
> updates (no full-page reload, no scroll reset, no flash), as a **purely additive
> progressive enhancement**. The HTTP surface (URLs, methods) is UNCHANGED — only the
> response *shape* varies by the `HX-Request` header. Identifier prefix: **`HX-`**.
> Completeness target: > 0.95. Tech choices are deferred to DESIGN as OD-HX-*.

## Domain glossary (slice-07 additions; slice-06 glossary inherited)

| Term | Meaning |
|------|---------|
| **Full page** | The complete, navigable HTML document a route serves today (slice-06): chrome + nav + the content region. Served whenever `HX-Request` is absent. |
| **Fragment** | Just the changed region of the SAME content (the table / results / detail / view panel), with no surrounding chrome. Served whenever `HX-Request` is present. |
| **`HX-Request` header** | The request header the htmx library sets on swap-driven requests. Its presence/absence selects fragment vs full page. Absent for plain navigation, curl, bookmark, view-source, JS-disabled. |
| **Swap target** | The `id`-addressed region of the page a fragment replaces in place (e.g. the claims table, the scrape results, the claim-detail panel, the active view panel). |
| **Progressive enhancement** | htmx swaps are layered ON TOP of fully-working server-rendered pages; with JS off / offline / direct URL the operator gets the exact slice-06 experience. |
| **htmx asset** | The htmx JavaScript library, served by the viewer itself (vendored or inlined), never from a CDN, so the dashboard works fully offline. |

---

## Functional Requirements (FR)

| ID | Requirement | Job | Source |
|----|-------------|-----|--------|
| FR-HX-1 | On `GET /claims?page=N` and `GET /peer-claims?page=N`, an htmx request (HX-Request present) returns just the list-table fragment (rows + position indicator "X–Y of N" + Prev/Next) swapped into the table region; the chrome, nav, and scroll position are unchanged. | navigate-without-reloads | Journey step 1 |
| FR-HX-2 | On `POST /scrape`, an htmx request returns just the results fragment (candidates, OR zero-candidates guidance, OR network-down guidance) swapped below the form; the form and its target value remain; NO sign control is rendered; nothing is persisted. | navigate-without-reloads | Journey step 2 |
| FR-HX-3 | On `GET /claims/{cid}`, an htmx request returns just the claim-detail fragment (all fields + complete evidence[], confidence verbatim) swapped into an inline panel; the list remains in place; unknown CID returns the guided not-found fragment. | navigate-without-reloads | Journey step 3 |
| FR-HX-4 | Switching My Claims ↔ Peer Claims as an htmx request returns just the active view-panel fragment (peer rows retain origin, separable from own claims) swapped in place; the browser URL/history updates so the active view is bookmarkable and Back works. | navigate-without-reloads | Journey step 4 |
| FR-HX-5 | For ANY route, when `HX-Request` is absent (no-JS, direct URL, bookmark, view-source, curl), the viewer returns the complete slice-06 full page for that route. No new data routes are introduced; only the response shape varies by header. | navigate-without-reloads | Journey all steps / I-HX-1 |
| FR-HX-6 | The htmx library is served by the viewer process itself (vendored static asset or inlined script), never from a CDN; the page chrome references that single local source. | navigate-without-reloads | Journey step 5 |

## Non-Functional Requirements (NFR) — including the hard guardrails

| ID | Requirement | Measurable criterion |
|----|-------------|----------------------|
| NFR-HX-1 (**progressive enhancement**) | Every route serves a complete, navigable full page when HX-Request is absent. | Each route, requested WITHOUT the header, returns a complete page; requested WITH it, returns the fragment of the SAME content. Verified by sending/withholding HX-Request against the real `openlore ui`. (I-HX-1) |
| NFR-HX-2 (**offline-first asset**) | htmx is served locally; the dashboard works fully offline. | With the network down, every store view AND every swap still works. No served page references an off-host URL for htmx. (I-HX-2, inherits I-VIEW-6 / KPI-VIEW-5) |
| NFR-HX-3 (**read-only preserved**) | Swaps add no write/sign surface; the web process holds no signing key. | Zero new write/sign route reachable; key-access audit unchanged (zero key reads). (I-HX-3, inherits I-VIEW-1/2 / I-SCR-1) |
| NFR-HX-4 (**no regression**) | Non-htmx responses are byte-equivalent to slice-06; the slice-06 acceptance suite stays green. | slice-06 26-scenario corpus GREEN; non-htmx response bytes unchanged per route. (I-HX-4) |
| NFR-HX-5 (**fragment/full-page parity**) | A fragment is the SAME content as the corresponding region of the full page. | For identical inputs, the fragment renderer output equals the full page's region (same rows, "X–Y of N", verbatim confidence, peer origin). (I-HX-5) |
| NFR-HX-6 (**in-place feel**) | A swap updates only the targeted region: no full-document reload, no scroll reset, no flash. | The swapped request replaces only the swap-target region; surrounding chrome and scroll position are unchanged. |
| NFR-HX-7 (**no-leak on swap errors**) | An error rendered into a fragment (network-down, unknown CID) states cause + next step and leaks no transport/stack internals. | Guided message only; carries forward slice-06 NFR-VIEW-6 and the DV-4 payload-free error pattern. |
| NFR-HX-8 (**accessibility preserved**) | Swapped fragments keep semantic HTML and keyboard operability; the no-JS path remains fully keyboard-navigable via real links/forms. | WCAG 2.2 AA minimums hold in both shapes (inherits NFR-VIEW-8). |

## Business Rules (BR)

| ID | Rule |
|----|------|
| BR-HX-1 | The HTTP surface (URLs + methods) is unchanged: swaps ride the existing GET routes + the existing `POST /scrape`. The ONLY permissible new route is the optional local htmx static asset (OD-HX-1). |
| BR-HX-2 | The `HX-Request` header is the sole selector of fragment vs full page; no query param or separate endpoint duplicates the data. |
| BR-HX-3 | The fragment of a route is the SAME content as the full page's corresponding region (no fragment-only data, no full-page-only data). |
| BR-HX-4 | `/scrape` renders NO sign control in either shape, persists nothing, and re-harvests on each submit (carries BR-VIEW-1/BR-VIEW-2). |
| BR-HX-5 | derived-from appears only on `/scrape` candidate fragments, never on `/claims` or `/peer-claims` fragments (carries BR-VIEW-3 / WD-62). |
| BR-HX-6 | htmx is served from a single local source; no page references a CDN (carries the offline-first promise). |

## New Invariants (slice-07) — carried into DESIGN as I-HX-*

| ID | Invariant | Origin |
|----|-----------|--------|
| I-HX-1 | **Progressive enhancement**: every route serves a complete navigable full page when HX-Request is absent; htmx requests get the fragment of the SAME content; no new data routes. | new (this slice) |
| I-HX-2 | **htmx served locally**: the library is vendored/inlined, never from a CDN; the dashboard works fully offline. | new; inherits **I-VIEW-6** / **KPI-VIEW-5** |
| I-HX-3 | **Read-only preserved**: swaps add no write/sign surface; no key in the web process. | inherits **I-VIEW-1/2**, **I-SCR-1** |
| I-HX-4 | **No regression**: non-htmx responses byte-equivalent to slice-06; slice-06 acceptance suite stays green. | new (this slice) |
| I-HX-5 | **Fragment/full-page parity**: the fragment equals the full page's corresponding region. | new (this slice) |

## Inherited Invariants (carried verbatim from slice-06, must still hold)

| ID | Invariant | Still enforced because |
|----|-----------|------------------------|
| I-VIEW-1 | Read-only — no route writes or signs. | Swaps ride existing GET + existing POST /scrape; no new write/sign route (I-HX-3). |
| I-VIEW-2 | No signing key in the web process. | Unchanged wiring; key-access audit stays zero. |
| I-VIEW-3 | Human signing gate stays exclusively in the CLI (I-SCR-1). | `/scrape` fragment renders no sign control (BR-HX-4). |
| I-VIEW-4 | Loopback-only bind (127.0.0.1). | Unchanged `openlore ui` bind; htmx asset also served loopback-only. |
| I-VIEW-5 | derived-from display-only, only on /scrape candidates (WD-62). | Carried as BR-HX-5; persisted-view fragments have no derived-from slot (type-level). |
| I-VIEW-6 | Store views work fully offline (KPI-5 local-first). | Hardened by I-HX-2 (htmx served locally; offline test is the gate). |
| FR-VIEW-8 | Confidence shown verbatim. | Fragment renders confidence from the same f64, verbatim (parity, NFR-HX-5). |

## Open Decisions for DESIGN (OD-HX-*) — handed to Morgan (solution-architect)

> DISCUSS is solution-neutral. These are explicit questions for DESIGN. Feasibility context
> (maud renderers in `viewer-domain`; hand-rolled hyper handlers in `adapter-http-viewer`;
> `deny.toml` bans axum/actix; the slice-06 PageView/clamp) is **context, not decisions**.

| ID | Open question for DESIGN |
|----|--------------------------|
| OD-HX-1 | **htmx asset delivery mechanism**: vendored static asset behind a new `GET /static/htmx.min.js` route, vs inlining the script into the page chrome — and pinned htmx version + integrity. Must be loopback-only, single-source, offline-proven (I-HX-2). |
| OD-HX-2 | **Fragment-vs-page rendering split in `viewer-domain`**: how to factor the slice-06 whole-page renderers so a fragment renderer and the full page emit the SAME content with structural parity (e.g. the full page embeds the fragment), keeping the pure core pure (ADR-007/ADR-029) and parity (I-HX-5) checkable. |
| OD-HX-3 | **Swap targets / element ids + swap semantics**: the exact ids (#claims-table, #scrape-results, #claim-detail, #view-panel are the proposed names) and the htmx swap attributes/targets/triggers per interaction. Both full page and fragment must agree on each id. |
| OD-HX-4 | **Tab-switch URL/history strategy**: how the htmx tab swap updates the browser URL/history so the active view is bookmarkable and Back works (e.g. `hx-push-url`), converging with the no-JS real-URL path. |
| OD-HX-5 | **Where the HX-Request branch lives**: how the `adapter-http-viewer` handlers detect HX-Request and select fragment vs full page within the effect shell, keeping the selection out of the pure core (the pure core renders both shapes; the shell picks). |
| OD-HX-6 | **Acceptance harness extension**: how the slice-06 `ViewerServer` HTTP harness sends/withholds HX-Request to drive both shapes against the real `openlore ui` (test convention only; no production impact). |

## Requirements Completeness Self-Assessment

| Category | Captured? | Evidence |
|----------|-----------|----------|
| Functional | Yes | FR-HX-1..6 cover all four interactions + the no-JS fallback + the local asset. |
| Non-functional | Yes | NFR-HX-1..8 incl. progressive enhancement, offline asset, read-only, no-regression, parity, in-place feel, no-leak, a11y. |
| Business rules | Yes | BR-HX-1..6 incl. unchanged HTTP surface, header-as-sole-selector, parity, no-sign-control, derived-from honesty, no-CDN. |
| Error / sad paths | Yes | no-JS/curl/bookmark full page; network-down + zero-candidate scrape fragments; unknown-CID detail fragment; offline asset; over-the-end page clamp. |
| New + inherited invariants | Yes | I-HX-1..5 + I-VIEW-1..6 + FR-VIEW-8 + I-SCR-1 all reaffirmed with enforcement rationale. |
| Open decisions tracked | Yes | OD-HX-1..6 handed to DESIGN. |

**Estimated completeness: ~0.96** (> 0.95 target). Residual gaps are intentional DESIGN
decisions: the asset mechanism (OD-HX-1), the rendering split (OD-HX-2), exact swap ids
(OD-HX-3), and the history strategy (OD-HX-4) — deferred rather than guessed.
