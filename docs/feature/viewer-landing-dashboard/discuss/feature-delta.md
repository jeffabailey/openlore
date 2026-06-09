<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-landing-dashboard

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (an EXTENSION of the existing read-only `GET /` landing route on the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-LD-001 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07)
> JTBD: YES — the user-visible story traces to **J-002** (orientation facet — "explore the philosophy graph to inform a decision"); the enabling read-wiring story is `infrastructure-only` with rationale; no new job created
> Brownfield DELTA on: `htmx-scraper-viewer` (slice-06 — the read-only viewer foundation: `render_landing` ~549, `READ_ONLY_NOTICE`, `page_head`, the I-VIEW invariants, no key, `StoreReadPort`, `page = chrome + fragment`), `viewer-htmx-swaps` (slice-07 — `Shape::from_request` fork, the `render_tab_nav` real-`<a href>`-enhanced-with-htmx pattern), and all 11 shipped viewer surfaces (slices 06–16 — the routes the hub links to: `/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`)
> Date: 2026-06-09 · Owner: Luna (nw-product-owner)
> Slice: slice-17

This file is the canonical DISCUSS-wave delta for `viewer-landing-dashboard`
(slice-17): turning the viewer's front door `GET /` from a near-empty page (today it
shows only an `<h1>`, the `READ_ONLY_NOTICE`, and a SINGLE `<a href="/claims">` link —
"queries nothing") into a read-only **navigation hub + at-a-glance LOCAL store summary**.

The 11-surface viewer (slices 06–16) is not yet navigable as a coherent app: the other
8 surfaces (`/peers`, `/search`, `/score`, `/project`, `/philosophy`, `/scrape`,
`/peer-claims`) are only cross-linked WITHIN features, so a user who opens `/` cannot
discover them. And the landing surfaces NO store state, despite `count_claims` /
`count_peer_claims` / `list_active_peer_subscriptions` all existing on the read-only
`StoreReadPort`. slice-17 closes the discoverability gap and realizes **KPI-VIEW-1
(time-to-see-store-contents)** as the front door: open `/` and immediately (a) see what's
in your LOCAL store (own claims, peer claims, active peer subscriptions) and (b) navigate
to every surface.

This is a DELTA. It EXTENDS the existing `GET /` route (NO new route). It threads the
read-only store the viewer ALREADY holds into the landing handler (`landing_page` is
currently the only handler that takes no store) and REUSES three reads that already exist
on `StoreReadPort` — NO new read method (or, at most, one tiny count-only variant for
active subscriptions, an open DESIGN question — see below). It REUSES the slice-06/07
`page = chrome + fragment` render pattern and the read-only viewer. Read-only / no-key /
LOCAL / offline, like every viewer surface. NO new crate; workspace stays 21. Tier-1
content is inlined here (lean); SSOT lives under `docs/product/`; per-discuss artifacts
under `discuss/`.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-002 "Explore the philosophy graph to inform a decision" at ~line 63 — the orientation job the viewer realizes; J-001 "undiscoverable" push at ~line 34; the slice-09/10/15 viewer changelog entries at ~lines 672/702/578 confirming the viewer realizes J-002/J-003 VIEWING on P-001's browser surface)
- ✓ `crates/viewer-domain/src/lib.rs` (`render_landing()` ~549 — the CURRENT near-empty front door: `<h1>` + `READ_ONLY_NOTICE` + ONE `/claims` link, queries nothing; `READ_ONLY_NOTICE` ~521 shared with the launch banner; `page_head` ~261; `render_tab_nav` ~284 — the real-`<a href>`-enhanced-with-htmx nav precedent; the route URL CONSTS: `MY_CLAIMS_URL`/`PEER_CLAIMS_URL` ~229/234, `SEARCH_URL` ~1457, `SCORE_URL` ~1833, `PROJECT_URL`/`PHILOSOPHY_URL` ~2137/2142, `PEERS_URL` ~2665 — the single-source-of-truth for every surface link)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (`landing_page()` ~414 — "Pure render — needs no store read … it queries nothing" — the handler this slice threads the store into; the route table ~359–409 showing all shipped routes; how the OTHER handlers thread `store.as_ref()` and fork by `Shape`)
- ✓ `crates/ports/src/store_read.rs` (the read-only `StoreReadPort`: `count_claims()` ~296 and `count_peer_claims()` ~316 are ALREADY count-only methods; `list_active_peer_subscriptions()` ~445 returns `Vec<PeerSubscriptionSummary>` (slice-15) — there is NO count-only variant for active subs, so the dashboard either `.len()`s the existing read or DESIGN adds a tiny `count_active_peer_subscriptions` — OPEN DESIGN QUESTION; NO mutation method on the trait — read-only by construction, I-VIEW-1)
- ✓ `docs/feature/htmx-scraper-viewer/discuss/` (the slice-06 read-only viewer foundation: `render_landing`, `READ_ONLY_NOTICE`, the I-VIEW invariants — read-only/no-key/loopback/local-first, `page = chrome + fragment`)
- ✓ `docs/feature/viewer-peer-subscriptions/discuss/` (slice-15 — the most recent net-new-read slice; its lean DISCUSS structure + render-only-command + parity + single-aggregate-query disciplines MIRRORED; `list_active_peer_subscriptions` is the slice-15 read this slice reuses)
- ⊘ `docs/feature/viewer-landing-dashboard/diverge/` (no DIVERGE wave for this slice — consistent with all prior OpenLore slices; noted as a non-blocking risk; the job J-002 is already validated and the surfaces being linked are all SHIPPED)

No DISCUSS decision below contradicts the prior-wave evidence: the viewer is read-only
(slices 06–16); `GET /` already exists; the three reads already exist on `StoreReadPort`;
the route URL constants are the single source of truth for each surface link.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona
as slices 06–16 (`docs/product/personas/senior-engineer-solo-builder.yaml`). She runs
`openlore ui` to glance at her store, read transparent scores (`/score`, slice-09),
traverse the graph (`/project` + `/philosophy`, slice-10), discover the network
(`/search`, slice-08), read counter-claim threads (`/claims/{cid}`, slice-11), and manage
her federation subscriptions (`/peers`, slice-15). slice-17 gives her the **orientation /
front-door** surface: she opens `/` and immediately sees what's in her store and where
she can go.

> P-002 (researcher-tech-lead) is the primary persona for several CLI jobs; but the
> BROWSER viewer is P-001's surface (slices 06–16). `/` is P-001's first-touch surface
> every time she opens the viewer.

### Orientation hat (NEW for slice-17)

P-001 wearing the orientation hat opens `/` to answer two questions in the first second:
"What's in my store right now?" and "Where can I go?" — WITHOUT having to already know
the route names, and WITHOUT the viewer ever holding a key or mutating anything.

- **Load-bearing anxieties** (from J-002 four-forces — "What if the graph is sparse … and
  I make a bad call?"; and the discoverability push): "I opened the viewer — is there
  anything in here? Where do I even start? I only ever see a link to claims; are the other
  views I read about actually here?"
- **Load-bearing signals of success**: "The moment I open `/`, I see how many own claims,
  peer claims, and active peer subscriptions are in my LOCAL store." · "From `/` I can
  reach every surface — claims, peer-claims, project, philosophy, score, search, scrape,
  peers — by a plain link, no JS required." · "If a count can't be read, the page still
  shows me the navigation and never throws a 5xx." · "It loads with the network down."

> This DISCUSS wave appends the slice-17 orientation hat to
> `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09,
> slice-17). It MINTS a new hat (front-door orientation is a distinct first-touch behavior
> from the graph-explorer / counter-claim-scanner / subscription-manager hats).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-002**: *When I'm choosing a tech stack or evaluating a community, I want to see what
> philosophies a project embodies and who else holds those philosophies, so I can pick
> projects aligned with how I want to build.*
> (`docs/product/jobs.yaml`, opportunity score 14, underserved-primary-for-slice for the
> scoring-graph; the viewer realizes the VIEWING/orientation side across slices 06–16.)

slice-17 realizes the **ORIENTATION / FRONT-DOOR** facet of J-002. Every exploration the
viewer supports (query a project, a philosophy, a contributor; read counter threads;
review the network; manage peers) STARTS at `/`. Today `/` is a dead end (one link,
queries nothing), so the operator cannot orient: she cannot see whether her store has
anything in it, and she cannot discover the surfaces that answer J-002. slice-17 makes `/`
the orienting front door:

1. It shows the operator, at a glance, what's in her LOCAL store — own claims, peer
   claims, active peer subscriptions (three counts; the "is the graph sparse / is there
   anything here?" question answered immediately — the J-002 sparse-vs-dense honesty
   applied at the front door).
2. It links to every shipped surface so she can navigate the coherent app (closing the
   discoverability gap: today only `/claims` is reachable from `/`).

The COUNTS themselves are LOCAL aggregate reads that already exist
(`count_claims` / `count_peer_claims` / `list_active_peer_subscriptions`). No new job. No
new sub-job. The user-visible story traces to J-002; the read-wiring story is
`infrastructure-only` with rationale.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | The user-visible story (US-LD-001) → J-002 (orientation facet). The infra story enables it. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | The dashboard shows three LOCAL COUNTS only — no per-author content, no scores, no merging. It NEVER collapses per-author attribution (the counts are aggregates BY DESIGN — "how many own claims / peer claims / active peers", not a "consensus" anything). Drilling into who-said-what stays the existing surfaces (`/claims`, `/peer-claims`, `/score`). |
| No contradiction with cardinal invariants? | PASS | Read-only / no-key (I-VIEW-1/2/3, KPI-VIEW-2) HONORED — `/` reads three counts + renders nav links only; it never mutates, holds no key, adds no write control. Local-first (KPI-5) HONORED — the counts are LOCAL DB reads, no network. |
| Counts are aggregates, not merges that lose attribution? | PASS | The three numbers are store-level aggregates ("how many", a legitimate count), NOT a merge of distinct authors' claims into one faceless record. The anti-merging invariant protects per-author CONTENT rendering; a store-wide count is not content. The dashboard links OUT to the attributed surfaces. |
| New route introduced? | NO (extends `GET /`) | slice-17 adds ZERO new routes. It extends the existing `render_landing` / `landing_page`. |
| New read method introduced? | NO (reuses existing) — or at most ONE tiny count-only variant | `count_claims` + `count_peer_claims` already exist; active-subs uses `.len()` of the existing `list_active_peer_subscriptions` OR a tiny `count_active_peer_subscriptions` (OPEN DESIGN QUESTION). No new orientation read is invented. |
| Job already fully served? | NO (the gap is real) | Today `/` shows one link and queries nothing; the other 8 surfaces are undiscoverable from the front door and no store state is surfaced. The 11-slice viewer is not navigable as a coherent app from its own entry point. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting extension of the
existing read-only front-door route.

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-17 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-7). Summary table:

| ID | Commitment | Source |
|---|---|---|
| I-LD-1 (= I-VIEW-1/2/3) — **CARDINAL** | **Read-only / no key**: `/` holds `StoreReadPort` only — no mutation method, no signing key in the viewer process, no write control on the rendered surface. All navigation links are plain `<a href>` (no-JS navigable, optionally htmx-enhanced like `render_tab_nav`); none executes a mutation. 3-layer (type: the read port has no mutation method + xtask check-arch viewer-capability rule + behavioral gold). | KPI-VIEW-2, slice-06–16 |
| I-LD-2 (= KPI-5 / KPI-VIEW-5) — **CARDINAL** | **LOCAL-only / offline + graceful degrade**: the three counts are LOCAL aggregate reads (`count_claims` / `count_peer_claims` / active-subscriptions); NO network seam on this route. `/` renders fully with the network down and references only the vendored local `/static/htmx.min.js` (no CDN). If any count read FAILS, `/` degrades gracefully — it shows the navigation hub WITHOUT that number (never a 5xx, never a blank page, never a raw stack trace). | KPI-5, KPI-VIEW-5, slice-06/07 KPI-HX-G2, NFR-VIEW-6 |
| I-LD-3 | **Navigation completeness**: the hub links to ALL shipped surfaces that make sense as entry points — `/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape`, `/peers`. `/claims/{cid}` and dimension-parameterized routes (`/score?contributor`, `/project?subject`, `/philosophy?object`) are reached THROUGH those, not top-level. Each link uses the route's single-source-of-truth URL CONST from `viewer-domain` (`MY_CLAIMS_URL`, `PEERS_URL`, …) — never a hardcoded path string. | the discoverability gap; the route-const single-source-of-truth |
| I-LD-4 | **Cheap / no N+1 / invariant to store size**: the summary is a SMALL FIXED number of aggregate count reads per landing render (3 — own claims, peer claims, active peer subscriptions), invariant to store size. `count_*` are aggregate `COUNT(*)`; active-subscriptions is the slice-15 single-aggregate read (used via `.len()` or a count-only variant). NO per-claim / per-peer loop. | slice-15 single-aggregate discipline; I-PS-8 |
| I-LD-5 (= I-HX-1/4/5) | **Progressive enhancement + parity**: `/` serves a full page (chrome + landing fragment) WITHOUT `HX-Request` and the SAME fragment WITH it (slice-07 `Shape::from_request` fork) IF DESIGN forks it; otherwise `/` stays a full-page render (the landing is typically the entry full page). The dashboard counts + the nav links live in the SAME render both shapes embed, so they render identically. A swap is a nicety, never a requirement; the no-JS full page is the contract. DESIGN confirms the shape handling. | slice-07 KPI-HX-G1/G2/G3 |
| I-LD-6 | **Zero new persisted types; loopback-only bind**: the counts are computed per-request and never persisted; the bind stays 127.0.0.1-only. | BR-VIEW-2 / I-VIEW-4, slice-15 I-PS-6 |
| I-LD-7 | **No new crates; no new route; reuse the existing reads**: extend the PURE `viewer-domain` (`render_landing` gains a `LandingSummary` input) + EFFECT `adapter-http-viewer` (`landing_page` threads the store) + at most `ports` / `adapter-duckdb` IF DESIGN elects a count-only active-subs variant (OPEN QUESTION). Workspace stays 21 members. Functional paradigm (ADR-007). NO new `GET /` route — it already exists. | slice-06–16 precedent |

---

## Wave: DISCUSS / [REF] Proposed change + count-read approach

- **Route (EXTENDED — NO new route)**: `GET /` already exists (`landing_page` →
  `render_landing`). slice-17 threads the read-only store the viewer ALREADY holds into
  `landing_page` (today it is the only handler that takes no store) and passes a
  `LandingSummary` (the three counts) into the pure `render_landing`.
- **Reads (REUSED — NO new read method, with ONE open DESIGN question)**: the three counts
  the dashboard needs ALL already exist on the read-only `StoreReadPort`:
  - own claims → `count_claims() -> Result<usize, StoreReadError>` (slice-06).
  - peer claims → `count_peer_claims() -> Result<usize, StoreReadError>` (slice-06).
  - active peer subscriptions → `list_active_peer_subscriptions() -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>` (slice-15). The dashboard needs only the COUNT, so it can `.len()` this existing read, OR DESIGN may add a tiny count-only `count_active_peer_subscriptions()` variant (a `COUNT(*) … WHERE removed_at IS NULL`).

  > **OPEN DESIGN QUESTION (DD owns it)**: use `.len()` of the existing
  > `list_active_peer_subscriptions` (zero new port surface; materializes the full active
  > set just to count it — cheap at dogfood scale: the active set is tiny, and the read is
  > already ONE aggregate query) OR add a count-only `count_active_peer_subscriptions()`
  > (mirrors `count_claims`/`count_peer_claims`; avoids materializing rows just to count).
  > The PRODUCT contract is: the active-subscription COUNT is a single aggregate read,
  > invariant to store size (I-LD-4) — DESIGN picks the cheaper/cleaner of the two. If
  > DESIGN adds the count-only variant, it is a read-only method on `StoreReadPort` (no
  > mutation added) and `adapter-duckdb` gains ONE `COUNT(*)` impl; workspace stays 21.

- **Pure render (EXTENDED, in `viewer-domain`)**: `render_landing()` becomes
  `render_landing(summary)` (a proposed `LandingSummary { own_claims, peer_claims,
  active_peers }` flat input where each count is an `Option<usize>` / total ADT so a failed
  read degrades to "—" rather than a fabricated 0). It renders: the existing `<h1>` +
  `READ_ONLY_NOTICE`; a small dashboard of the three LOCAL counts; and a navigation hub of
  plain `<a href>` links to all 8 shipped entry-point surfaces (each from its route URL
  CONST). When a count is absent (read failed), the hub renders without that number.
  DESIGN owns the exact `LandingSummary` shape (Option-per-count vs a small ADT) and the
  markup.

---

## Wave: DISCUSS / [REF] JTBD trace (story → J-002, with boundaries)

| Story | Title | job_id | Boundary note |
|---|---|---|---|
| US-LD-000 | Thread the read-only store into `GET /` and resolve the three LOCAL counts (own claims, peer claims, active peer subscriptions) in fixed aggregate reads, degrading gracefully on read failure | `infrastructure-only` | `infrastructure_rationale` below. Enables US-LD-001. NOT a mutation; read-only by construction. Reuses existing reads. |
| US-LD-001 | Open the viewer and see, at a glance, what's in my LOCAL store + navigate to every surface | J-002 | The ORIENTATION / FRONT-DOOR facet of J-002. The counts are LOCAL aggregates; the hub links to all 8 shipped entry-point surfaces. Drilling into who-said-what stays the existing surfaces. |

### Infrastructure rationale (US-LD-000)

US-LD-000 carries `job_id: infrastructure-only` with this rationale: it threads the
read-only store the viewer ALREADY holds into the `landing_page` handler (the only handler
that takes no store today) and resolves the three LOCAL counts via the EXISTING reads
(`count_claims` / `count_peer_claims` / active-subscriptions count), each a single
aggregate read, degrading to a missing-number state on read failure (never a 5xx). It
produces no user-visible output on its own (the rendered dashboard + the navigation hub
are US-LD-001), so it enables a user decision only THROUGH that story. The slice contains
ONE non-infrastructure, user-visible story (US-LD-001) with a real decision (orient: what's
here + where to go), so the slice has release value (Dimension-0 slice-level check passes).
This is READ-ONLY by construction: it adds no mutation method (and if DESIGN elects a
count-only variant, that variant is on `StoreReadPort`, which declares no mutation method).

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-17 does NOT, under any circumstance:

- **Add any write / compose / sign / subscribe / follow control to `/`** (I-LD-1, CARDINAL
  — inherits the slice-06 key-less viewer). The front door reads counts + renders links;
  it executes nothing.
- **Hold a signing key or any mutation capability in the viewer process** (I-LD-1).
- **Add a new route.** `GET /` already exists; slice-17 extends it (I-LD-7).
- **Render claim CONTENT, scores, counter threads, or any per-author rows on `/`.** The
  dashboard shows three LOCAL COUNTS only; reading who-said-what is the existing surfaces
  (`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/peers`).
- **Merge anything / show a "consensus" anything.** The three numbers are store-level
  aggregate counts, not a merge of distinct authors' claims into a faceless record.
- **Add any network seam to this route.** The three counts are LOCAL DB reads (I-LD-2). No
  PDS fetch, no DID re-resolution, no `peer pull`, no network search.
- **Top-level-link the deep / parameterized routes.** `/claims/{cid}`,
  `/score?contributor`, `/project?subject`, `/philosophy?object` are reached THROUGH the
  8 entry-point links, not from the hub directly (I-LD-3).
- **Persist anything** (I-LD-6) or bind anything but 127.0.0.1 (I-LD-6).
- **Add a new crate** (I-LD-7 — workspace stays 21).
- **Issue a per-claim / per-peer read loop (N+1).** The summary is a FIXED 3 aggregate
  reads per render, invariant to store size (I-LD-4).
- **Hardcode any surface path string.** Each link uses the route's URL CONST from
  `viewer-domain` (I-LD-3).

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey visualization investment (Phase 1.5). This is the THINNEST delta in the
viewer series — it extends an existing route, reuses existing reads, and adds a render +
the store-threading. Expect PASS.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 2 (1 infra + 1 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure — `render_landing` gains a summary input) + `adapter-http-viewer` (effect — `landing_page` threads the store) + at most `ports` / `adapter-duckdb` IF DESIGN elects a count-only active-subs variant — all existing; NO new crate | No (single context) |
| Walking-skeleton integration points | 3: (1) thread the read-only store into `landing_page`, (2) resolve the three counts via the EXISTING reads (own + peer via the existing count methods; active-subs via `.len()` or a tiny count-only variant), (3) the extended `render_landing(summary)` + the nav hub. Well within ≤5. | No (≤5) |
| Estimated effort | ~0.5–1 day (thread the store + the three count reads + extend one pure render with a small input + the nav hub from existing URL consts) | No (≤2 weeks) |
| Independent user outcomes | 1 (open the viewer and orient: see what's in my store + reach every surface) | No |

**## Scope Assessment: PASS — 2 stories (1 infra + 1 user-visible), 1 context, 3 integration points (extend `GET /` handler + reuse 3 existing reads + extend the landing render with the nav hub), estimated ~0.5–1 day. No new route; reuses existing reads; no new crate; workspace stays 21.**

The thing that would make it oversized — rendering claim content / scores / threads on the
front door, adding a write/compose affordance, or a network seam — is explicitly OUT of
scope (I-LD-1 read-only, I-LD-2 LOCAL-only, the content-stays-on-existing-surfaces
boundary). This is the thinnest slice in the series: it touches no SQL (reusing existing
reads) unless DESIGN elects the optional count-only variant.

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-LD-000 | Thread the read-only store into `GET /` and resolve the three LOCAL counts (own claims, peer claims, active peer subscriptions) in fixed aggregate reads (reusing existing `StoreReadPort` reads), degrading gracefully on read failure | infrastructure-only |
| US-LD-001 | Open the viewer and see, at a glance, what's in my LOCAL store (own claims, peer claims, active peers) + navigate to every shipped surface from the front door | J-002 |

---

## Wave: DISCUSS / [REF] User story with elevator pitch + AC

<!-- Full story bodies live in user-stories.md; elevator pitch + key AC themes summarized here for the single-narrative reader. Each AC names its driving route. -->

### US-LD-000 — Thread the store + resolve the three counts (`@infrastructure`)

`@infrastructure` — no Elevator Pitch (produces no user-visible output; enables
US-LD-001). It threads the read-only store the viewer ALREADY holds into `landing_page`
(today the only storeless handler) and resolves the three LOCAL counts via the EXISTING
reads (`count_claims`, `count_peer_claims`, active-subscriptions count), each a single
aggregate read, degrading to a missing-number state on read failure (never a 5xx).

**Key AC themes**: the landing resolves exactly three LOCAL aggregate counts per render
(own claims, peer claims, active peer subscriptions), invariant to store size (no N+1, no
per-claim/per-peer loop); each count uses an EXISTING read (`count_claims` /
`count_peer_claims` / `.len()` of `list_active_peer_subscriptions` OR a tiny count-only
variant — DESIGN's choice); a failed count read degrades to a missing-number state (the
hub still renders, never a 5xx); the change adds NO mutation method to `StoreReadPort`
(read-only by construction); LOCAL only (no network).

### US-LD-001 — Open the viewer and orient: store summary + navigate everywhere

**Elevator Pitch**
- Before: when Maria opens the viewer at `http://127.0.0.1:<port>/`, she sees only a heading, the read-only notice, and a single "View my claims" link — she cannot tell what's in her store, and she cannot discover the 8 other surfaces (peers, search, score, project, philosophy, scrape, peer-claims) the viewer ships.
- After: open `http://127.0.0.1:<port>/` → an at-a-glance LOCAL store summary (own claims, peer claims, active peer subscriptions, each a number) plus a navigation hub of plain links to every shipped surface; if a count can't be read, the hub still renders without that number; it loads with the network down.
- Decision enabled: Maria decides WHERE to go next (and knows whether her store has anything in it to explore) — orienting her whole session from the front door, the realization of KPI-VIEW-1 as the front door.

**Key AC themes** (driving route `GET /`): the landing shows the three LOCAL counts (own
claims, peer claims, active peers); the landing links to ALL 8 shipped entry-point
surfaces (`/claims`, `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`,
`/scrape`, `/peers`) — each link via its route URL CONST, never a hardcoded path; the
route is read-only (no write control, no key); a failed count read degrades gracefully
(hub renders without that number, never a 5xx); LOCAL only (renders offline, no CDN); the
read-only notice and the page render identically under htmx + no-JS (parity, if DESIGN
forks the shape).

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-17 mints **NO new KPI ID**. Like slice-08–16 it REALIZES inherited KPIs on a new
facet (the `/` front-door dashboard + nav hub). Full detail in `outcome-kpis.md`. The
relevant inherited KPIs:

- **KPI-VIEW-1** (`Time-to-see-store-contents` — legibility north-star): slice-17 realizes
  this AS THE FRONT DOOR. The moment the operator opens `/` she sees what's in her LOCAL
  store (three counts) — the minimal-time-to-orient outcome, at the very first surface.
- **KPI-VIEW-2** (read-only, guardrail): MET — no write control, no key; the front door
  reads counts + renders links only. Release-blocking.
- **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS
  no-regression / read-only, guardrails): MET — the counts are LOCAL DB reads; the route
  renders offline; references only the vendored htmx asset; serves a full page; adds no
  write surface. Graceful degrade on a failed count read (no 5xx). Release-blocking.

A product hypothesis specific to this slice (a leading indicator OF KPI-VIEW-1, not a new
KPI ID):

> **Hypothesis**: We believe that turning `/` into a LOCAL store summary (own claims,
> peer claims, active peers) + a navigation hub to all 8 shipped surfaces (P-001,
> orientation hat) will increase the share of dogfood users who, on opening the viewer,
> immediately know what's in their store and reach a second surface — because the front
> door now answers "what's here?" and "where can I go?" in one read-only view. We will know
> this is true when, post-slice-17, users report (and opt-in telemetry shows) they open `/`
> and navigate to a NON-`/claims` surface (peers, search, score, project, philosophy) from
> the hub in the same session — closing the discoverability gap.

> Detail rationale is in `outcome-kpis.md`. The cross-feature SSOT is
> `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the
read-only `StoreReadPort` with all three reads (`count_claims`, `count_peer_claims`,
`list_active_peer_subscriptions`), the `GET /` route + `render_landing`, the
`page = chrome + fragment` render pattern (slice-06/07), and the route URL constants all
already exist. The thinnest end-to-end slice IS US-LD-001 (the front-door dashboard +
nav hub render), backed by US-LD-000 (threading the store + resolving the three counts).
Delivery sequence: US-LD-000 → US-LD-001. Each is demonstrable in a single session against
the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Requirements (functional + NFR + business rules): `requirements.md`
- User stories (combined, `## System Constraints` at top): `user-stories.md`
- Acceptance criteria (BDD, per theme): `acceptance-criteria.md`
- Outcome KPIs: `outcome-kpis.md`
- Definition of Ready: `dor-checklist.md`
- Wave decisions (WD-LD-*): `wave-decisions.md`

> Lean mode: the standalone journey-visual + journey-yaml + shared-artifacts-registry
> are NOT produced for this thin DELTA (mirroring the slice-08/12/13/15 lean set). The
> shared artifacts are the route URL CONSTS (`MY_CLAIMS_URL`, `PEERS_URL`, …, the
> single-source-of-truth for each surface link, already registered in `viewer-domain`) and
> the three count reads (already on `StoreReadPort`).

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)** for all 2 stories.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-17 | Low | Low | The job (J-002 orientation facet) is already validated in `docs/product/jobs.yaml`; the surfaces being linked are all SHIPPED (slices 06–16); the reads already exist on `StoreReadPort`. No design-direction ambiguity — the view extends the existing front door and reuses the slice-06/07 patterns. Noted as non-blocking risk (R-LD-1). |
| A count read failure 5xxes the whole front door | Medium | High | I-LD-2 + US-LD-000/001 AC make graceful degrade a HARD product commitment: a failed count read renders the hub WITHOUT that number (never a 5xx, never blank, never a raw stack trace — NFR-VIEW-6). A behavioral test seeds an unreadable count and asserts `/` still renders the nav hub. The other handlers' degrade-to-empty precedent (`counter_presence_for(...).unwrap_or_default()`) is the model. |
| The summary becomes an N+1 (per-claim or per-peer loop) | Low | Medium | I-LD-4 + US-LD-000 AC require a FIXED 3 aggregate reads per render, invariant to store size. `count_claims`/`count_peer_claims` are `COUNT(*)`; active-subs is the slice-15 single-aggregate read (`.len()` or count-only variant). A per-row loop is REJECTED. A behavioral test asserts the read count is invariant to store size. |
| A surface link is hardcoded / drifts from the route | Low | Medium | I-LD-3 requires each link to use the route's URL CONST from `viewer-domain` (`MY_CLAIMS_URL`, `PEERS_URL`, …). DESIGN reuses the consts; a hardcoded path string is REJECTED. |
| A count is fabricated as 0 when the read failed (misleads "empty store") | Medium | Medium | The `LandingSummary` models each count as Option / a total ADT so a FAILED read renders "—" (or omits the number), DISTINCT from a SUCCESSFUL read of 0 ("0 own claims"). The AC pins the distinction: a read failure never displays a fabricated 0. |
| Open count-read approach (`.len()` vs count-only variant) under-specified | Low | Low | Surfaced explicitly as an OPEN DESIGN QUESTION (WD-LD-5); the PRODUCT contract (a single aggregate read for the active-subs count, invariant to store size) holds either way. DESIGN picks the cheaper/cleaner; if it adds the count-only variant, it is read-only and workspace stays 21. |

---

## Changelog

- 2026-06-09 — slice-17 (`viewer-landing-dashboard`) DISCUSS. Traces to J-002 (the
  ORIENTATION / FRONT-DOOR facet of "explore the graph to inform a decision"). 2 stories
  (1 infra + 1 user-visible). EXTENDS the existing read-only `GET /` route (NO new route);
  threads the read-only store the viewer ALREADY holds into `landing_page`; REUSES three
  existing `StoreReadPort` reads (`count_claims` / `count_peer_claims` /
  `list_active_peer_subscriptions`) for the LOCAL store summary — NO new read method (OPEN
  DESIGN QUESTION: `.len()` the active-subs read vs a tiny count-only variant); adds a
  navigation hub linking all 8 shipped entry-point surfaces (each via its route URL CONST).
  CARDINAL decisions: read-only / no-key (I-LD-1); LOCAL-only / offline + graceful degrade
  on a failed count read (I-LD-2). NO new crate (workspace stays 21), no new KPI ID. Scope
  PASS (~0.5–1 day). DoR PASS (9/9).
