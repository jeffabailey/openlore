<!-- markdownlint-disable MD024 -->
# User Stories: viewer-landing-dashboard (slice-17)

> Combined file (one section per story). Brownfield DELTA on slices 06/07/15.
> The user-visible story traces to **J-002** (the ORIENTATION / FRONT-DOOR facet of
> "explore the graph to inform a decision", `docs/product/jobs.yaml`). The viewer is
> read-only, holds no key. slice-17 EXTENDS the existing `GET /` route — no new route —
> threads the read-only store into the landing handler, and REUSES three existing
> `StoreReadPort` reads for the LOCAL store summary.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: `/` holds `StoreReadPort` only — no mutation
  method, no signing key in the viewer process, no write/compose/sign/subscribe/follow
  control on the rendered surface. Every navigation affordance is a plain `<a href>` (no-JS
  navigable, optionally htmx-enhanced like `render_tab_nav`); none executes a mutation.
  Enforced 3 layers (type: the read port has no mutation method + xtask check-arch
  viewer-capability rule + behavioral gold). [KPI-VIEW-2, slice-06–16]
- **C-2 LOCAL-only / offline + graceful degrade (CARDINAL)**: the three counts are LOCAL
  DuckDB aggregate reads (`count_claims` / `count_peer_claims` / active-subscriptions);
  NO network seam on this route. `/` renders fully with the network down and references
  only the vendored local `/static/htmx.min.js` (no CDN). If any count read FAILS, `/`
  degrades gracefully — the navigation hub still renders WITHOUT that number (never a 5xx,
  never blank, never a raw stack trace). [KPI-5, KPI-VIEW-5, slice-06/07 KPI-HX-G2, NFR-VIEW-6]
- **C-3 Navigation completeness**: the hub links ALL shipped entry-point surfaces —
  `/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`,
  `/peers`. The deep / parameterized routes (`/claims/{cid}`, `/score?contributor`,
  `/project?subject`, `/philosophy?object`) are reached THROUGH those, not top-level. Each
  link uses the route's single-source-of-truth URL CONST from `viewer-domain`
  (`MY_CLAIMS_URL`, `PEER_CLAIMS_URL`, `PROJECT_URL`, `PHILOSOPHY_URL`, `SCORE_URL`,
  `SEARCH_URL`, `PEERS_URL`, the `/scrape` path) — never a hardcoded path string. [the
  discoverability gap]
- **C-4 Cheap / no N+1 / invariant to store size**: the summary is a FIXED 3 aggregate
  count reads per landing render, invariant to store size. `count_claims`/`count_peer_claims`
  are aggregate `COUNT(*)`; the active-subs count is the slice-15 single-aggregate read
  (`.len()` or a count-only variant). NO per-claim / per-peer loop. [slice-15 I-PS-8
  single-aggregate discipline]
- **C-5 Progressive enhancement + parity**: the summary + nav hub live in the SAME render
  the full page and (if DESIGN forks the shape) the htmx fragment both embed, so they
  render identically. The landing is typically a full page; DESIGN confirms the shape
  handling (full-page-only vs `Shape::from_request` fork). A swap is a nicety, never a
  requirement; the no-JS full page is the contract. [slice-07 KPI-HX-G1/G2/G3]
- **C-6 No new crates; no new route; reuse the existing reads**: extend the PURE
  `viewer-domain` (`render_landing` gains a summary input) + EFFECT `adapter-http-viewer`
  (`landing_page` threads the store) + at most `ports` / `adapter-duckdb` IF DESIGN elects a
  count-only active-subs variant. NO new `GET /` route (it already exists). Workspace stays
  21 members. Functional paradigm (ADR-007). [slice-06–16]
- **C-7 Counts are aggregates, never merges; missing ≠ zero**: the three numbers are
  store-level aggregate COUNTS, not a merge of distinct authors' claims into a faceless
  record (per-author CONTENT stays on the existing attributed surfaces). A FAILED read
  displays a missing-number state ("—"/omitted), DISTINCT from a successful read of 0.
  [BR-LD-1, BR-LD-3]

---

## US-LD-000: Thread the read-only store into `GET /` and resolve the three LOCAL counts in fixed aggregate reads, degrading gracefully on read failure (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-LD-000 threads the read-only store the viewer ALREADY holds into the `landing_page`
handler (today the only handler that takes no store — "Pure render — needs no store read")
and resolves the three LOCAL counts (own claims, peer claims, active peer subscriptions)
via the EXISTING `StoreReadPort` reads (`count_claims`, `count_peer_claims`, and the count
of `list_active_peer_subscriptions` — `.len()` OR a tiny count-only variant), each a single
aggregate read, degrading to a missing-number state on read failure (never a 5xx). It
produces no user-visible output on its own (the rendered dashboard + the navigation hub are
US-LD-001), so it enables a user decision only THROUGH that story. The slice contains ONE
non-infrastructure, user-visible story (US-LD-001) with a real decision, so the slice has
release value (Dimension-0 slice-level check passes). This is READ-ONLY by construction:
it adds no mutation method, and if DESIGN elects the count-only variant, that variant is on
`StoreReadPort`, which declares no mutation method.

### Problem

`GET /` (`landing_page` → `render_landing`) is the ONLY viewer handler that takes no store
— it "queries nothing." To surface a LOCAL store summary on the front door, the landing
handler must thread the read-only store the viewer already holds and resolve three counts.
Two reads already exist as count-only methods (`count_claims`, `count_peer_claims`); the
active-peer count needs the count of the slice-15 `list_active_peer_subscriptions`. A naive
implementation could fabricate a 0 when a read fails (misleading "empty store") or 5xx the
whole front door — neither acceptable for the first surface the operator sees.

### Who

- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the
  orientation-hat story (US-LD-001) consumes.

### Solution

Thread the read-only store into `landing_page` (mirroring how `claims_page` /
`peers_page` take `store.as_ref()`). Resolve three LOCAL aggregate counts: own claims via
`count_claims()`, peer claims via `count_peer_claims()`, active peers via the count of
`list_active_peer_subscriptions()` (`.len()`) OR a count-only `count_active_peer_subscriptions()`
variant (DESIGN's choice — both are a single aggregate read). Each count read that FAILS
degrades to a missing-number state (the `LandingSummary` models each count as Option / a
total ADT), so a failed read yields "—" rather than a fabricated 0 and never a 5xx. Pass
the `LandingSummary` into the pure `render_landing`. NO mutation method; NO network; NO
per-row loop.

### Domain Examples

#### 1: Happy path — a populated store resolves three counts in three aggregate reads

Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions
(`did:plc:rachel-test`, `did:plc:tobias-test`). Opening `GET /` resolves
`count_claims() = 12`, `count_peer_claims() = 7`, and the active-peer count `= 2` — three
aggregate reads, invariant to store size — and passes `LandingSummary { own_claims:
Some(12), peer_claims: Some(7), active_peers: Some(2) }` to the render.

#### 2: Edge case — an empty store resolves three successful zero counts (not missing)

Maria has a fresh store: 0 own claims, 0 peer claims, 0 active subscriptions. The three
reads SUCCEED and return 0. `LandingSummary { own_claims: Some(0), peer_claims: Some(0),
active_peers: Some(0) }` — three real zeros, NOT a missing-number state. (The view will
say "0 own claims", an honest empty store — distinct from "couldn't read".)

#### 3: Boundary — a failed count read degrades to a missing-number state, no 5xx

Maria's `count_peer_claims()` read fails (e.g. a transient `StoreReadError`). The own-claims
and active-peer counts still resolve. `LandingSummary { own_claims: Some(12), peer_claims:
None, active_peers: Some(2) }` — the peer-claims number renders as "—" and the front door
still renders the whole navigation hub; the route returns 200, never a 5xx, never a raw
stack trace.

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (`GET /`, port-to-port via the real `openlore ui`
> subprocess). No scenario calls a read method directly.

#### Scenario: The front door resolves three LOCAL aggregate counts in fixed reads

Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions
When she opens `GET /` in the `openlore ui` viewer
Then the landing summary shows 12 own claims, 7 peer claims, and 2 active peers
And the three counts are resolved in a FIXED set of aggregate reads, invariant to store size (no per-claim or per-peer loop)

#### Scenario: An empty store shows three real zero counts

Given Maria has a fresh store with 0 own claims, 0 peer claims, and 0 active subscriptions
When she opens `GET /`
Then the landing summary shows 0 own claims, 0 peer claims, and 0 active peers
And each is a successful read of zero, not a missing-number state

#### Scenario: A failed count read degrades to a missing-number state without a 5xx

Given Maria's peer-claims count read fails transiently while the other two reads succeed
When she opens `GET /`
Then the landing renders the navigation hub and the own-claims and active-peer counts
And the peer-claims number renders as a missing-number state (e.g. "—"), not a fabricated 0
And the route returns a 200 page, never a 5xx and never a raw stack trace

### Acceptance Criteria

- [ ] `GET /` resolves exactly three LOCAL aggregate counts per render: own claims (`count_claims`), peer claims (`count_peer_claims`), active peer subscriptions (count of `list_active_peer_subscriptions` or a count-only variant)
- [ ] The reads are a FIXED set per render, invariant to store size (no N+1, no per-claim/per-peer loop)
- [ ] Each count read uses an EXISTING `StoreReadPort` read (no new orientation read invented); the active-subs count is `.len()` of `list_active_peer_subscriptions` OR a tiny count-only variant (DESIGN's choice)
- [ ] A successful read of 0 is DISTINCT from a failed read (0 ≠ missing); the `LandingSummary` models each count as Option / a total ADT
- [ ] A failed count read degrades to a missing-number state — the hub still renders, the route returns 200, never a 5xx, never a raw stack trace
- [ ] The change adds NO mutation method to `StoreReadPort` (read-only by construction); if a count-only variant is added it is a read-only method
- [ ] The reads are LOCAL only (no network seam) and run over the SAME shared connection the CLI writes through (BR-VIEW-4)
- [ ] `landing_page` threads the read-only store and passes the `LandingSummary` into the pure `render_landing`

### Outcome KPIs

- **Who**: the viewer process serving `GET /`
- **Does what**: resolves the LOCAL store summary (three counts) for the front door in a fixed set of aggregate reads, degrading gracefully on failure
- **By how much**: exactly 3 aggregate reads per render, invariant to store size (0 N+1); 0 of N read failures produce a 5xx
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (read count invariant to store size; seeded read failure → 200 with missing-number state)
- **Baseline**: today `landing_page` takes no store and queries nothing; slice-17 adds exactly 3 aggregate reads, never N, and never a 5xx on a count failure

### Technical Notes

- Thread the store: `landing_page(store.as_ref(), shape)` (mirror `claims_page` / `peers_page` at `crates/adapter-http-viewer/src/lib.rs` ~393/440); the route arm `"/" => Ok(landing_page(store.as_ref(), shape))`. The viewer ALREADY holds the store (the other handlers use it).
- Reads (ALL existing on `StoreReadPort`, `crates/ports/src/store_read.rs`): `count_claims()` ~296, `count_peer_claims()` ~316, `list_active_peer_subscriptions()` ~445.
- **OPEN DESIGN QUESTION (WD-LD-5)**: active-subs count via `.len()` of `list_active_peer_subscriptions` (zero new port surface; materializes the tiny active set to count it) vs a count-only `count_active_peer_subscriptions()` (`COUNT(*) … WHERE removed_at IS NULL`, mirrors `count_claims`/`count_peer_claims`). PRODUCT contract: a single aggregate read for the active-subs count either way. If DESIGN adds the variant, it is read-only and workspace stays 21.
- Graceful degrade: model the precedent of `counter_presence_for(...).unwrap_or_default()` (slice-12) — a failed read degrades, never propagates a 5xx. DESIGN owns the `LandingSummary` shape (Option-per-count vs a small ADT).
- READ-ONLY: shares the CLI's connection (BR-VIEW-4); the trait declares no mutation method (I-VIEW-1).

---

## US-LD-001: Open the viewer and see, at a glance, what's in my LOCAL store + navigate to every surface

`job_id: J-002`

### Problem

When Maria opens the viewer at `http://127.0.0.1:<port>/`, she sees only a heading, the
read-only notice, and a single "View my claims" link. She cannot tell what's in her store
(is there anything to explore? is the graph sparse?), and she cannot discover the 8 other
surfaces the viewer ships (peers, search, score, project, philosophy, scrape, peer-claims)
— they are only cross-linked within features. The 11-surface viewer is not navigable as a
coherent app from its own front door, and the front door surfaces no store state despite
the reads existing.

### Who

- P-001 (the viewer operator, "Maria"), orientation hat | opening the viewer for the first
  time in a session | wants to know what's in her store and where she can go, without
  already knowing the route names — confident the front door changes nothing.

### Solution

On `GET /`, render (alongside the existing `<h1>` + `READ_ONLY_NOTICE`): an at-a-glance
LOCAL store summary of three counts (own claims, peer claims, active peer subscriptions),
and a navigation hub of plain `<a href>` links to all 8 shipped entry-point surfaces
(`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`,
`/peers`), each link via its route URL CONST. When a count read failed, the corresponding
number renders as a missing-number state ("—") and the hub still renders. The page is
read-only (no key, no write control), LOCAL (renders offline), and renders identically
under htmx + no-JS (if DESIGN forks the shape).

### Elevator Pitch

- **Before**: when Maria opens the viewer at `http://127.0.0.1:<port>/`, she sees only a heading, the read-only notice, and a single "View my claims" link — she cannot tell what's in her store, and she cannot discover the 8 other surfaces (peers, search, score, project, philosophy, scrape, peer-claims) the viewer ships.
- **After**: open `http://127.0.0.1:<port>/` → an at-a-glance LOCAL store summary (own claims, peer claims, active peer subscriptions, each a number) plus a navigation hub of plain links to every shipped surface; if a count can't be read, the hub still renders without that number; it loads with the network down.
- **Decision enabled**: Maria decides WHERE to go next — and knows whether her store has anything in it to explore — orienting her whole session from the front door (the realization of KPI-VIEW-1 as the front door).

### Domain Examples

#### 1: Happy path — populated store, full summary + full nav hub

Maria has 12 own claims, 7 peer claims, and 2 active peer subscriptions. `/` shows the
read-only notice, a summary ("12 own claims · 7 peer claims · 2 active peers"), and a
navigation hub linking My Claims (`/claims`), Peer Claims (`/peer-claims`), Project Survey
(`/project`), Philosophy Survey (`/philosophy`), Contributor Score (`/score`), Network
Search (`/search`), Live Scrape (`/scrape`), and Peer Subscriptions (`/peers`). She clicks
"Peer Subscriptions" and lands on `/peers` — a surface she could not reach from `/` before.

#### 2: Edge case — fresh empty store, honest zeros + full nav hub

Maria just installed and has 0 own claims, 0 peer claims, 0 active subscriptions. `/` shows
"0 own claims · 0 peer claims · 0 active peers" (honest zeros, not "—") and the SAME full
navigation hub — so she can still navigate to `/scrape` or `/search` to start populating
her store. The empty store is legible, not a dead end.

#### 3: Boundary / graceful degrade — a count read failed, hub still renders

Maria's peer-claims count read fails transiently. `/` shows "12 own claims · — peer
claims · 2 active peers" (the "—" tells her that one number couldn't be read, distinct from
"0") and the full navigation hub still renders. The page is a normal 200, not a 5xx — the
front door never breaks just because one count couldn't be read.

### UAT Scenarios (BDD)

> Driving route: `GET /` (the real `openlore ui` subprocess).

#### Scenario: The front door shows the LOCAL store summary

Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions
When she opens `GET /` in the viewer
Then she sees a store summary showing 12 own claims, 7 peer claims, and 2 active peers
And she sees the read-only notice telling her nothing here can change her store

#### Scenario: The front door links to every shipped surface (discoverability)

Given Maria opens `GET /`
When she looks at the navigation hub
Then she sees a link to each shipped surface: My Claims (/claims), Peer Claims (/peer-claims), Project Survey (/project), Philosophy Survey (/philosophy), Contributor Score (/score), Network Search (/search), Live Scrape (/scrape), and Peer Subscriptions (/peers)
And each link navigates to that surface with no JavaScript required (a plain link)
And no deep or parameterized route (/claims/{cid}, /score?contributor) is a top-level link

#### Scenario: The front door is read-only with no write control

Given Maria opens `GET /`
When she inspects the rendered page
Then it contains no form, no button, and no control to compose, sign, subscribe, or follow
And every navigation affordance is a plain link, not a mutating control
And the viewer process holds no signing key

#### Scenario: A failed count read degrades gracefully on the front door

Given Maria's peer-claims count read fails while the other two counts succeed
When she opens `GET /`
Then the navigation hub renders in full
And the own-claims and active-peer counts show their numbers (12 and 2)
And the peer-claims number renders as a missing-number state (e.g. "—"), not a fabricated 0
And the page is a normal 200, not a 5xx and not a blank page

#### Scenario: The front door renders fully with the network down

Given Maria's store has claims and peers and the network is unavailable
When she opens `GET /`
Then the store summary and the full navigation hub render
And no outbound network request is made by the route
And the page references only the vendored local /static/htmx.min.js (no CDN)

### Acceptance Criteria

- [ ] `/` shows a LOCAL store summary with three counts: own claims, peer claims, active peer subscriptions
- [ ] `/` retains the `<h1>` heading and the `READ_ONLY_NOTICE`
- [ ] `/` links to ALL 8 shipped entry-point surfaces (`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`), each via its route URL CONST, each a plain navigable `<a href>` (no JS required)
- [ ] No deep / parameterized route (`/claims/{cid}`, `/score?contributor`, `/project?subject`, `/philosophy?object`) is a top-level link on `/`
- [ ] The route is read-only: no form/button/mutating control; no write/compose/sign/subscribe/follow affordance; no signing key
- [ ] A failed count read renders a missing-number state for that count (distinct from a successful 0); the hub still renders; the route returns 200, never a 5xx
- [ ] The summary + nav hub render LOCAL/offline, referencing only the vendored `/static/htmx.min.js` (no CDN)
- [ ] The summary + nav hub render identically under htmx fragment + no-JS full page (parity, if DESIGN forks the shape; otherwise `/` is full-page-only)

### Outcome KPIs

- **Who**: P-001 dogfood operators opening the viewer
- **Does what**: on opening `/`, immediately sees what's in their LOCAL store and navigates to a surface from the hub
- **By how much**: leading indicator OF KPI-VIEW-1 (time-to-see-store-contents at the front door) — a measurable share open `/` and reach a NON-`/claims` surface (peers, search, score, project, philosophy) from the hub in the same session
- **Measured by**: per-feature GREEN (the summary + the 8-surface hub render; a failed count degrades, never 5xx); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today `/` shows one link and queries nothing; only `/claims` is reachable from the front door and no store state is surfaced

### Technical Notes

- Render: extend `render_landing()` (`crates/viewer-domain/src/lib.rs` ~554) to `render_landing(summary)`. It keeps the `<h1>` + `READ_ONLY_NOTICE` (~521) and adds the three-count summary + the nav hub. The nav hub follows the `render_tab_nav` precedent (~284): each link a real `<a href=(CONST)>`, optionally htmx-enhanced (`hx-get`/`hx-target`/`hx-push-url`) — but plain navigation is the contract.
- Surface links use the existing URL CONSTS: `MY_CLAIMS_URL` (~229), `PEER_CLAIMS_URL` (~234), `SEARCH_URL` (~1457), `SCORE_URL` (~1833), `PROJECT_URL` (~2137), `PHILOSOPHY_URL` (~2142), `PEERS_URL` (~2665), and the `/scrape` path (no const yet — DESIGN decides whether to add a `SCRAPE_URL` const for parity; never a duplicated literal).
- DESIGN owns: the `LandingSummary` shape (Option-per-count vs a small ADT); the exact markup (list vs cards); whether `/` forks by `Shape` or stays full-page-only; whether to mint a `SCRAPE_URL` const. The PRODUCT contract is the AC.

---

## Out of scope (explicit — restated from feature-delta)

- **Any write / compose / sign / subscribe / follow control on `/`** — read-only front
  door (C-1, CARDINAL). No key.
- **A new route** — `GET /` already exists; slice-17 extends it (C-6).
- **Rendering claim content, scores, counter threads, or per-author rows on `/`** — the
  dashboard shows three LOCAL COUNTS only; who-said-what stays the existing surfaces (C-7 /
  BR-LD-1).
- **Merging anything / a "consensus" anything** — the three numbers are store-level
  aggregate counts, not a merge that loses attribution (C-7 / BR-LD-1).
- **Any network seam on this route** — LOCAL DB reads only; no PDS fetch, no DID
  re-resolution, no `peer pull`, no network search (C-2).
- **Top-level-linking deep / parameterized routes** (`/claims/{cid}`, `/score?contributor`,
  …) — reached THROUGH the 8 entry-point links (C-3 / FR-LD-5).
- **Hardcoding any surface path string** — each link via its route URL CONST (C-3 / FR-LD-4).
- **Fabricating a 0 when a read failed** — a failed read is a missing-number state, distinct
  from a real 0 (C-7 / BR-LD-3).
- **Persisting anything; binding anything but 127.0.0.1; adding a new crate** (C-6).
- **N+1 (a per-claim or per-peer read loop)** — a FIXED 3 aggregate reads per render (C-4).
