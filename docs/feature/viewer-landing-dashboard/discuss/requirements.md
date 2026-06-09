# Requirements: viewer-landing-dashboard (slice-17)

> Wave: DISCUSS (lean) · Owner: Luna (nw-product-owner) · 2026-06-09
> Job: **J-002** (`docs/product/jobs.yaml`) — "Explore the philosophy graph to inform a
> decision" (the ORIENTATION / FRONT-DOOR facet).
> Brownfield DELTA on slices 06 (htmx-scraper-viewer — `render_landing`, `READ_ONLY_NOTICE`,
> the I-VIEW invariants, `page = chrome + fragment`) / 07 (viewer-htmx-swaps —
> `Shape::from_request` fork, `render_tab_nav`) / 15 (viewer-peer-subscriptions —
> `list_active_peer_subscriptions`). Reuses reads shipped in 06 (`count_claims`,
> `count_peer_claims`) and 15.

## 1. Context

The `openlore ui` viewer has shipped 11 surfaces (slices 06–16): `/claims`,
`/claims/{cid}` (counter threads), `/peer-claims`, `/project`, `/philosophy`, `/score`,
`/search`, `/scrape`, `/peers`. But the landing page `GET /` (`render_landing`) currently
shows ONLY an `<h1>`, the `READ_ONLY_NOTICE`, and a SINGLE `<a href="/claims">` link — it
"queries nothing." The other 8 surfaces are cross-linked only WITHIN features, so a user
who opens `/` cannot discover `/peers`, `/search`, `/score`, `/project`, `/philosophy`,
`/scrape`, `/peer-claims`. The 11-slice viewer is not navigable as a coherent app from its
own entry point, and the landing surfaces NO store state — despite `count_claims` /
`count_peer_claims` / `list_active_peer_subscriptions` all already existing on the
read-only `StoreReadPort`.

slice-17 turns `GET /` into a read-only **navigation hub + at-a-glance LOCAL store
summary**: links to all the viewer surfaces plus a small dashboard of LOCAL counts (own
claims, peer claims, active peer subscriptions). It realizes **KPI-VIEW-1
(time-to-see-store-contents)** as the front door and closes the discoverability gap.
Read-only / no-key / LOCAL / offline, like every viewer surface. It EXTENDS the existing
`GET /` route (no new route), threads the read-only store into the landing handler, and
REUSES the three existing reads (no new read method — or, at most, one tiny count-only
variant for active subscriptions, an open DESIGN question).

## 2. Functional Requirements

| ID | Requirement | Rationale / Source |
|---|---|---|
| FR-LD-1 | `GET /` renders an at-a-glance LOCAL store summary showing THREE counts: own claims, peer claims, active peer subscriptions. | J-002 orientation; KPI-VIEW-1 at the front door |
| FR-LD-2 | The own-claims count is `count_claims()`; the peer-claims count is `count_peer_claims()`; the active-peer count is the count of `list_active_peer_subscriptions()` (`.len()`) OR a count-only variant — each a single LOCAL aggregate read, never a per-row loop. | reuse the existing reads; I-LD-4 no-N+1 |
| FR-LD-3 | `GET /` renders a navigation hub linking ALL shipped entry-point surfaces: `/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`. | the discoverability gap; I-LD-3 navigation completeness |
| FR-LD-4 | Each navigation link uses the route's single-source-of-truth URL CONST from `viewer-domain` (`MY_CLAIMS_URL`, `PEER_CLAIMS_URL`, `PROJECT_URL`, `PHILOSOPHY_URL`, `SCORE_URL`, `SEARCH_URL`, `PEERS_URL`, and the `/scrape` path) — never a hardcoded path string. | route-const single source of truth; prevents link drift |
| FR-LD-5 | The deep / parameterized routes (`/claims/{cid}`, `/score?contributor`, `/project?subject`, `/philosophy?object`) are NOT top-level links; they are reached THROUGH the 8 entry-point surfaces. | I-LD-3 scope of the hub |
| FR-LD-6 | `GET /` retains the existing `<h1>` heading and the `READ_ONLY_NOTICE` (the operator is told, up front, the view is read-only). | slice-06 NFR-VIEW-1 inheritance |
| FR-LD-7 | When a count read FAILS, `/` renders the navigation hub WITHOUT that number (a missing-number state, e.g. "—"), DISTINCT from a successful read of 0 — never a 5xx, never a blank page, never a fabricated 0. | I-LD-2 graceful degrade; NFR-VIEW-6 |
| FR-LD-8 | The summary + nav hub live in the SAME render the full page and (if DESIGN forks the shape) the htmx fragment both embed, so they render identically (parity). The landing is typically a full page; DESIGN confirms the shape handling. | slice-07 `page = chrome + fragment`; I-LD-5 |

## 3. Non-Functional Requirements

| ID | NFR | Measurable criterion |
|---|---|---|
| NFR-LD-1 (read-only, CARDINAL) | `/` holds `StoreReadPort` only; no mutation method, no signing key, no write/compose/sign/subscribe/follow control. | Type: the read port declares no mutation method. xtask check-arch viewer-capability rule green. Behavioral gold: no form/`<button>`/mutating control on `/`; only navigation `<a href>`. |
| NFR-LD-2 (local-first / offline + graceful degrade, CARDINAL) | The three counts are LOCAL DuckDB aggregate reads; no network seam; `/` references only the vendored `/static/htmx.min.js` (no CDN). A failed count read degrades to a missing-number state, never a 5xx. | Behavioral test: `/` renders fully with the network down + no outbound request from the route; a seeded unreadable count → `/` still renders the nav hub, no 5xx. |
| NFR-LD-3 (cheap / no N+1) | A FIXED 3 aggregate reads per render, invariant to store size. | Behavioral test: read count invariant to the number of own claims / peer claims / active peers. |
| NFR-LD-4 (loopback bind / no persistence) | Bind stays 127.0.0.1; the counts are computed per-request, never persisted. | Inherited I-VIEW-4 / BR-VIEW-2; no new persisted type. |
| NFR-LD-5 (plain-language errors) | A store-read failure renders a plain-language missing-number state, never a raw stack trace. | Inherited NFR-VIEW-6; `StoreReadError` surfaced cleanly (degrade, not echo). |
| NFR-LD-6 (parity, if shape forked) | If `GET /` forks by `Shape`, the htmx fragment and the no-JS full page render the SAME summary + nav hub. | Behavioral test: `/` WITH and WITHOUT `HX-Request` render the same summary + hub region (or `/` is full-page-only; DESIGN confirms). |

## 4. Business Rules

| ID | Rule | Exception / precedence |
|---|---|---|
| BR-LD-1 | The three numbers are store-level AGGREGATE COUNTS ("how many own claims / peer claims / active peers"), not a merge of distinct authors' claims into a faceless record. Drilling into who-said-what is the existing attributed surfaces. | The anti-merging invariant protects per-author CONTENT rendering; a store-wide count is a legitimate aggregate, not a merge that loses attribution. |
| BR-LD-2 | An active peer subscription counted on `/` is an ACTIVE row (`peer_subscriptions.removed_at IS NULL`) — a soft-removed peer is residue and is NOT counted (inherits the slice-15 active-only definition). | The active-subs count reuses the slice-15 `list_active_peer_subscriptions` (already active-only) or a `WHERE removed_at IS NULL` count-only variant. |
| BR-LD-3 | A FAILED count read displays a missing-number state, DISTINCT from a SUCCESSFUL read of 0. "—" (or omitted) means "couldn't read this"; "0" means "your store has none." | I-LD-2 graceful degrade; the `LandingSummary` models each count as Option / total ADT. |
| BR-LD-4 | The navigation hub links ONLY shipped, user-facing entry-point surfaces. `HTMX_ASSET_URL` (`/static/htmx.min.js`) and any internal/asset route are NOT linked. | I-LD-3; the hub is for human navigation. |

## 5. Requirements Completeness Check

- **Functional**: FR-LD-1..8 — the three-count summary, the count sources (reused reads),
  the nav hub to all 8 surfaces, the URL-const links, the deep-routes-not-top-level scope,
  the retained read-only notice, the graceful-degrade missing-number state, the parity. ✓
- **NFR**: NFR-LD-1..6 — read-only, local/offline+degrade, no-N+1, loopback/no-persist,
  plain-language errors, parity — each with a measurable criterion. ✓
- **Business rules**: BR-LD-1..4 — counts-are-aggregates-not-merges, active-only count,
  missing-vs-zero distinction, entry-points-only hub. ✓

All three requirement categories present; no completeness gap.

## 6. Domain Language (ubiquitous terms)

| Term | Definition |
|---|---|
| Front door / landing | `GET /` — the first surface the operator sees on opening the viewer. Today near-empty; slice-17 makes it the orientation hub. |
| LOCAL store summary | The three at-a-glance aggregate counts on `/`: own claims (`count_claims`), peer claims (`count_peer_claims`), active peer subscriptions (active-only count). |
| Navigation hub | The set of plain `<a href>` links on `/` to all 8 shipped entry-point surfaces, each via its route URL CONST. |
| Entry-point surface | A top-level route a user navigates to directly: `/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`. (Distinct from deep/parameterized routes reached through them.) |
| Missing-number state | The rendered state for a count whose read FAILED ("—" or omitted), distinct from a successful read of 0 (graceful degrade, BR-LD-3). |
| Active peer subscription | A `peer_subscriptions` row with `removed_at IS NULL` (the slice-15 definition) — counted on `/`; a soft-removed row is residue, not counted. |
