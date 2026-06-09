<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-peer-subscriptions

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a net-new read-only `GET /peers` view on the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-PS-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/08/10)
> JTBD: YES — every non-`@infrastructure` story traces to **J-003c** (`docs/product/jobs.yaml`, sub-job of J-003); no new job created
> Brownfield DELTA on: `openlore-federated-read` (slice-03 — `peer_subscriptions`, `active_subscription_dids`/`list_active_subscriptions`, `PeerSubscription`, `openlore peer remove`), `htmx-scraper-viewer` (slice-06 — read-only viewer foundation: no key, `StoreReadPort`, `page = chrome + fragment`, I-VIEW invariants), `viewer-htmx-swaps` (slice-07 — `Shape::from_request` fork), `viewer-network-search` (slice-08 — `SEARCH_FOLLOW_GUIDANCE_PREFIX` + `render_follow_guidance` + `AuthorRelationship::NetworkUnfollowed`, the render-only CLI-command precedent MIRRORED here), `viewer-graph-traversal` (slice-10 — the most recent net-new-route slice; its DISCUSS shape is mirrored)
> Date: 2026-06-09 · Owner: Luna (nw-product-owner)
> Slice: slice-15

This file is the canonical DISCUSS-wave delta for `viewer-peer-subscriptions`
(slice-15): the federation-management **VIEWING** surface. The actual subscribe /
unsubscribe stays CLI (slice-03 `openlore peer add` / `openlore peer remove`); the
viewer is **read-only, holds no key** — a hard invariant. slice-15 adds a net-new
read-only **`GET /peers`** view that lists the operator's currently-subscribed peers
(active rows in `peer_subscriptions` where `removed_at IS NULL`) — each peer's DID +
its local claim count — plus a **render-only** `openlore peer remove <did>` revocation
command per peer, so the operator sees who they follow AND the clean, residue-free
unsubscribe path. When there are no subscriptions, a guided empty state.

This is a DELTA. It MIRRORS, with the same render-only-CLI-command discipline, the
slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX` / `render_follow_guidance` /
`AuthorRelationship::NetworkUnfollowed` pattern (the `/search` view renders
`openlore peer add <did>` as guidance TEXT, never an executable control); slice-15
renders `openlore peer remove <did>` the same way. It REUSES the slice-06/07/10
`page = chrome + fragment` render pattern and the read-only `StoreReadPort`. It adds
ONE net-new route (`GET /peers`) and ONE net-new read-only `StoreReadPort` method (list
active subscriptions with per-peer claim counts) — the first slice since slice-10 to
add a read method. NO new crate; workspace stays 21. Tier-1 content is inlined here
(lean); SSOT lives under `docs/product/`; per-journey/registry artifacts under
`discuss/`.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003 at ~line 168; sub-job **J-003c** "Subscription is revocable without residue" at ~line 260; J-003a anti-merging at ~line 241; the slice-03 changelog at ~line 591)
- ✓ `docs/product/journeys/subscribe-and-read-federated.yaml` (the slice-03 J-003 journey: `peer add` → `peer pull` → `graph query --federated` → `peer remove`; the `peer remove` step-4 with soft-remove vs `--purge`; the `peer_did` shared artifact + its consumers; the anti-merging guarantee)
- ✓ `crates/ports/src/store_read.rs` (the read-only `StoreReadPort` — `list_claims`/`list_peer_claims`/`count_peer_claims`/`query_*_survey`/`counter_presence_for`; NO method yet lists active subscriptions with per-peer counts — this slice adds ONE; `PeerOrigin`/`PeerClaimRow`)
- ✓ `crates/adapter-duckdb/src/peer_storage.rs` (the `peer_subscriptions` table; `list_active_subscriptions()` = `SELECT … WHERE removed_at IS NULL ORDER BY subscribed_at` on the WRITE `PeerStoragePort`; the `count_peer_claims(conn, peer_did)` free fn = `SELECT count(*) FROM peer_claims WHERE author_did = ?` — the exact per-peer count shape; `soft_remove`/`hard_purge` set `removed_at`)
- ✓ `crates/ports/src/federated_row.rs` (`PeerSubscription { peer_did, peer_handle, peer_pds_endpoint, subscribed_at, removed_at: Option<DateTime<Utc>> }` at ~line 154; `AuthorRelationship` at ~line 67 — `NetworkUnfollowed` is the slice-08 render-only-follow driver)
- ✓ `crates/viewer-domain/src/lib.rs` (`SEARCH_FOLLOW_GUIDANCE_PREFIX = "Follow this author from the CLI: openlore peer add"` ~line 1513; `render_follow_guidance` ~line 1759 — the bare-DID strip + render-only `<p>` TEXT pattern MIRRORED for `peer remove`; the `Shape` fork + `page = chrome + fragment` render)
- ✓ `docs/feature/viewer-network-search/discuss/wave-decisions.md` (the slice-08 render-only-command precedent: WD-NS-3 read-only / follow stays CLI / view may DISPLAY the affordance never execute it; WD-NS-6 progressive enhancement; WD-NS-7 nothing persisted)
- ✓ `docs/feature/viewer-graph-traversal/discuss/wave-decisions.md` (slice-10, the most recent net-new-route + net-new-read slice; its DISCUSS shape — WD-GT-1 new LOCAL GET route, WD-GT-3 read-only 3-layer, WD-GT-4 LOCAL-only/offline, WD-GT-5 anti-merging, WD-GT-8 progressive enhancement, WD-GT-9 nothing persisted/loopback bind — MIRRORED)
- ✓ `docs/feature/viewer-counter-flags-graph-surfaces/discuss/` (the recent lean DISCUSS structure mirrored: feature-delta + user-stories `## System Constraints` + acceptance-criteria + dor-checklist + outcome-kpis + wave-decisions)
- ⊘ `docs/feature/viewer-peer-subscriptions/diverge/` (no DIVERGE wave for this slice — noted as a non-blocking risk; the job J-003c is already validated and the slice-03 journey is grounded)

No DISCUSS decision below contradicts the prior-wave evidence: J-003c is validated; the
viewer is read-only (slices 06–14); subscribe/unsubscribe is the slice-03 CLI; the
render-only-command discipline is the slice-08 precedent.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona
as slices 06–14 (`docs/product/personas/senior-engineer-solo-builder.yaml`). She runs
`openlore ui` to glance at her store, navigate without reloads, read transparent scores
(`/score`, slice-09), traverse the graph (`/project` + `/philosophy`, slice-10),
discover the network (`/search`, slice-08), read counter-claim threads
(`/claims/{cid}`, slice-11), and spot countered claims on her lists (slice-12/13).
slice-15 gives her the **federation-management VIEWING** surface: she opens `/peers` to
see WHO she currently follows and HOW to leave cleanly.

> P-002 (researcher-tech-lead) is the primary persona for the slice-03 CLI federation
> job; but the BROWSER viewer is P-001's surface (slices 06–14). The slice-03 `peer
> add` / `peer remove` verbs are P-001/P-002's CLI surface; the `/peers` view is
> P-001's read-only browser surface over the SAME local subscription set.

### Subscription-manager hat (NEW for slice-15)

P-001 wearing the subscription-manager hat opens `/peers` to answer two questions at a
glance: "Who am I currently following?" and "How do I leave a peer cleanly, without
lingering data?" — WITHOUT the viewer ever executing a subscription change (the
unsubscribe is a render-only CLI command she copies into her terminal).

- **Load-bearing anxieties** (from J-003c four-forces, anxiety (b)): "If I subscribed to
  a DID I later regret — is subscription a one-way commitment? Where do I even see who I
  follow, and how do I leave?" · "If I removed a peer, did it really go, or is there
  lingering residue?"
- **Load-bearing signals of success**: "I can see, in one read-only view, every peer I
  currently follow with its DID and how many of its claims I hold locally." · "Right
  beside each peer is the exact CLI command to unsubscribe cleanly." · "After I run
  `openlore peer remove <did>` in my terminal and reload `/peers`, that peer is GONE
  from the list — its absence IS the residue-free promise made visible." · "When I
  follow nobody, the view tells me so and shows me how to start."

> This DISCUSS wave appends the slice-15 subscription-manager hat to
> `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09,
> slice-15). It MINTS a new hat (federation-management is a distinct scanning behavior
> from the counter-claim-scanner / graph-explorer hats).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003c**: *When I subscribe to a DID I later regret subscribing to, I want to
> unsubscribe cleanly with no lingering data, so subscription isn't a one-way
> commitment.*
> (`docs/product/jobs.yaml`, sub-job of **J-003**, opportunity score 15.)

slice-15 realizes the **VIEWING / LEGIBILITY** side of J-003c. The "without residue"
GUARANTEE itself is the slice-03 CLI's `openlore peer remove` (which sets `removed_at`,
and with `--purge` deletes the cached `peer_claims`). slice-15 makes the subscription
set LEGIBLE and surfaces the revocation PATH:

1. It shows the operator WHO they currently follow — the active subscription set.
2. It shows, per peer, HOW many of that peer's claims they hold locally (per-peer count,
   never a merged total — J-003a inheritance).
3. It surfaces the render-only `openlore peer remove <did>` revocation command per peer.
4. The read shows ONLY active subscriptions (`removed_at IS NULL`), so a peer removed via
   the CLI VANISHES from `/peers` — that absence IS the "revocable without residue"
   promise rendered.

No new job. No new sub-job. Every non-`@infrastructure` story traces to J-003c.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Every user-visible story → J-003c. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-003a (attribute every peer claim without merging) is HONORED — each peer is its own row, the claim count is PER-PEER (`WHERE author_did = ?`), NEVER a merged total across peers; no "consensus peer" row. J-003b (counter-claim authoring) is untouched. |
| No contradiction with cardinal invariants? | PASS | Read-only / no-key (I-VIEW-1/2/3, KPI-VIEW-2) HONORED — `/peers` renders the subscription set + the revocation COMMAND TEXT only; it never mutates, holds no key, adds no write/subscribe/unsubscribe control. Anti-merging (KPI-AV-2 / KPI-FED-1/2) HONORED — per-peer rows, per-peer counts. Local-first (KPI-5) HONORED — the read is a LOCAL DB read, no network. |
| Subscribe/unsubscribe NOT re-introduced on the viewer? | PASS | This slice adds ZERO write/subscribe/unsubscribe controls. The unsubscribe is render-only `openlore peer remove <did>` TEXT (mirroring the slice-08 render-only `peer add` follow guidance); the viewer holds no key and executes nothing (I-VIEW-3). |
| Active-only honored (residue made visible)? | PASS (the defining AC) | `/peers` reads ONLY active subscriptions (`removed_at IS NULL`). A peer soft-removed or purged via the CLI is absent from `/peers` — the absence IS the residue-free promise. An explicit AC + domain example pin this. |
| Job already fully served? | NO (gap is real) | The CLI `openlore peer add`/`remove` mutate the subscription set, but there is NO read-only browser surface that LISTS the current subscriptions with per-peer counts + the revocation path. The operator cannot, today, glance at who they follow in the viewer. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting net-new
read-only view.

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-15 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-8). Summary table:

| ID | Commitment | Source |
|---|---|---|
| I-PS-1 (= I-VIEW-1/2/3) — **CARDINAL** | **Read-only / no key**: `/peers` holds `StoreReadPort` only — no mutation method, no signing key in the viewer process, no write/subscribe/unsubscribe control on the rendered surface. The unsubscribe is render-only `openlore peer remove <did>` TEXT, never an executable control. Subscribe/unsubscribe stays EXCLUSIVELY the slice-03 CLI. 3-layer (type: the read port has no mutation method + xtask check-arch viewer capability rule + behavioral gold). | KPI-VIEW-2, slice-06–14, slice-08 WD-NS-3 |
| I-PS-2 — **CARDINAL** | **Active-only / residue made visible**: `/peers` lists ONLY active subscriptions (`removed_at IS NULL`). A peer removed via the CLI (soft-remove OR `--purge`) VANISHES from `/peers` — that absence IS the J-003c "revocable without residue" promise rendered. The read NEVER shows soft-removed (`UnsubscribedCache`) rows. | J-003c, slice-03 ADR-014 soft-remove |
| I-PS-3 (= J-003a / KPI-AV-2 / KPI-FED-1/2) | **Per-peer, never merged**: each peer is its own attributed row; the claim count is PER-PEER (`COUNT(*) FROM peer_claims WHERE author_did = <this peer>`), NEVER a merged total across peers, never a "consensus" row. The list is grouped/keyed by peer DID. | J-003a, slice-03 I-FED-1 |
| I-PS-4 (= KPI-5 / KPI-VIEW-5) | **LOCAL-only / offline**: the subscription list + per-peer claim counts are a LOCAL DuckDB read (`peer_subscriptions` + `peer_claims`); NO network seam on this route. `/peers` renders fully with the network down and references only the vendored local `/static/htmx.min.js` (no CDN). | KPI-5, slice-10 WD-GT-4, slice-06/07 KPI-HX-G2 |
| I-PS-5 (= I-HX-1/4/5) | **Progressive enhancement + parity**: an `HX-Request` returns the `/peers` fragment; a no-JS / bookmark / direct-URL request returns the full page = chrome + the SAME fragment (slice-07 `Shape::from_request` fork). The render-only command text + the empty state live in the SAME fragment fn both shapes embed, so they render identically. A swap is a nicety, never a requirement. | slice-07 KPI-HX-G1/G2/G3, slice-08 WD-NS-6, slice-10 WD-GT-8 |
| I-PS-6 | **Zero new persisted types; loopback-only bind**: the subscription list is computed per-request and never persisted; the bind stays 127.0.0.1-only. | BR-VIEW-2 / I-VIEW-4, slice-10 WD-GT-9, slice-08 WD-NS-7 |
| I-PS-7 | **No new crates**: extend the PURE `viewer-domain` + EFFECT `adapter-http-viewer` + `ports` (ONE new read method) + `adapter-duckdb` (ONE new read impl + new SQL) + `cli` + `xtask`. Workspace stays 21 members. Functional paradigm (ADR-007). | slice-06–14 precedent |
| I-PS-8 | **Render-only revocation command (single source of truth)**: the `openlore peer remove <did>` text is held in ONE place (a `PEER_REMOVE_GUIDANCE_PREFIX` const, mirroring the slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX`), rendered with the bare-DID strip (`render_remove_guidance`, mirroring `render_follow_guidance`) as a render-only `<p>`/`<code>` — never an `<a>` that executes, never a form. | slice-08 `render_follow_guidance` precedent |

---

## Wave: DISCUSS / [REF] Proposed route + read method

- **Route (NEW)**: `GET /peers` — a net-new LOCAL read-only route on the `openlore ui`
  viewer. Renders the active subscription set as HTML; serves a full page without
  `HX-Request` and a fragment of the same peers region with it (slice-07
  `page = chrome + fragment`). This is the first net-new route since slice-10 (`/project`
  + `/philosophy`).
- **Read method (NEW — the first new `StoreReadPort` method since slice-10)**: a
  read-only method on `StoreReadPort` that returns the ACTIVE subscriptions
  (`removed_at IS NULL`) each paired with its PER-PEER local claim count. A proposed
  shape (DESIGN owns the exact signature):

  ```rust
  /// READ-ONLY active-subscription survey for the `/peers` view (slice-15 / J-003c):
  /// every ACTIVE subscription (`peer_subscriptions.removed_at IS NULL`), each paired
  /// with its PER-PEER local claim count (`COUNT(*) FROM peer_claims WHERE author_did
  /// = <peer_did>`). LOCAL only — NO network. The count is PER-PEER, NEVER a merged
  /// total (anti-merging, J-003a). A soft-removed / purged peer is EXCLUDED (active-
  /// only, I-PS-2). Returns an EMPTY vec when the operator follows no one (the viewer
  /// renders the guided empty state — never an error). READ-ONLY by construction: a
  /// SELECT over the SAME shared connection the CLI writes through (BR-VIEW-4) — there
  /// is NO mutation method on this trait (I-VIEW-1).
  fn list_active_peer_subscriptions(
      &self,
  ) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>;
  ```

  where `PeerSubscriptionSummary` is a FLAT read DTO (proposed):
  `{ peer_did: String, peer_handle: String, subscribed_at: DateTime<Utc>,
  local_claim_count: u64 }`. The `peer_did` is rendered VERBATIM (attribution
  discipline). The new SQL is a `peer_subscriptions WHERE removed_at IS NULL` join /
  correlated `COUNT(*)` against `peer_claims` grouped by `author_did` — ONE aggregate
  query (no N+1; mirrors the slice-10/12 single-query-per-render discipline). The
  existing write-side `PeerStoragePort::list_active_subscriptions()` is NOT reused (it
  is on the write port and carries no counts); the existing adapter free fn
  `count_peer_claims(conn, peer_did)` confirms the per-peer count SQL shape.

  > DESIGN owns whether the count is a correlated subquery, a `LEFT JOIN … GROUP BY`,
  > or a per-peer `count_peer_claims` fold (the latter is N+1 — AVOID; the product
  > contract is ONE aggregate query per render). DESIGN also owns whether the DTO lives
  > in `ports` beside `PeerClaimRow` or in `viewer-domain`. The PRODUCT contract is the
  > AC in `user-stories.md`.

- **Pure render (NEW, in `viewer-domain`)**: a `render_peers_fragment` that maps the
  `Vec<PeerSubscriptionSummary>` into a list — each peer row shows its DID (verbatim),
  its local claim count, and the render-only `openlore peer remove <did>` command
  (`render_remove_guidance`, mirroring slice-08 `render_follow_guidance`). When the vec
  is EMPTY, a guided empty state ("You are not subscribed to any peers. Subscribe from
  the CLI: `openlore peer add <did>`."). The fragment is embedded by both the full page
  (chrome + fragment) and the htmx response (fragment only).

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-003c, with boundaries)

| Story | Title | job_id | Boundary note |
|---|---|---|---|
| US-PS-001 | Read-only viewer capability to list active subscriptions with per-peer claim counts (the new `StoreReadPort` method + adapter read + `/peers` handler wiring) | `infrastructure-only` | `infrastructure_rationale` below. Enables US-PS-002/003. NOT a mutation; read-only by construction. |
| US-PS-002 | See, on `/peers`, every peer I currently follow — its DID + local claim count — and the render-only `openlore peer remove <did>` revocation command per peer | J-003c | The VIEWING side of J-003c. NOT the unsubscribe itself (slice-03 CLI). Active-only (I-PS-2). Per-peer counts (J-003a). |
| US-PS-003 | When I follow no peers, see a guided empty state on `/peers` telling me so and how to start | J-003c | The empty-state facet of the VIEWING side of J-003c. Guides to the slice-03 `openlore peer add` CLI. |

**J-003a / J-003b / J-003c boundary statement (explicit per the brief):**

- **J-003a** (attribute every peer claim without merging) is HONORED: each peer is its
  OWN attributed row; the claim count is PER-PEER (`WHERE author_did = <this peer>`),
  NEVER a merged total across peers and NEVER a "consensus peer" aggregate. slice-15
  mints no J-003a story; it carries the invariant structurally (per-peer rows, per-peer
  counts).
- **J-003b** (counter-claim as first-class disagreement) is untouched. `/peers` shows no
  claims, no counters — only the subscription set + per-peer counts.
- **J-003c** (subscription revocable without residue) is THIS slice's job — the VIEWING /
  LEGIBILITY half. The "without residue" GUARANTEE itself is the slice-03 CLI `peer
  remove` (sets `removed_at`; `--purge` deletes cached `peer_claims`). slice-15 makes the
  subscription set legible, surfaces the revocation command, and shows ONLY active
  subscriptions so a removed peer vanishes (the residue-free promise rendered).

### Infrastructure rationale (US-PS-001)

US-PS-001 carries `job_id: infrastructure-only` with this rationale: it adds the new
read-only `StoreReadPort` method (list active subscriptions with per-peer claim counts),
its `adapter-duckdb` read impl (the new active-only + per-peer-count SQL), and the
`GET /peers` handler wiring that calls the read and hands the result to the pure
projection. It produces no user-visible output on its own (the rendered list, the
revocation command, and the empty state are US-PS-002/003), so it enables a user
decision only THROUGH those stories. The slice contains TWO non-infrastructure,
user-visible stories (US-PS-002, US-PS-003), so the slice has release value (Dimension-0
slice-level check passes).

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-15 does NOT, under any circumstance:

- **Subscribe or unsubscribe from the viewer.** No "follow / unfollow / remove / purge"
  button, form, or control. Subscribe/unsubscribe stays EXCLUSIVELY the slice-03 CLI
  (`openlore peer add` / `openlore peer remove`). The unsubscribe is render-only
  `openlore peer remove <did>` TEXT (I-PS-1 / I-PS-8).
- **Hold a signing key or any mutation capability in the viewer process** (I-PS-1,
  CARDINAL — inherits the slice-06 key-less viewer).
- **Show soft-removed / unsubscribed-cache peers.** `/peers` lists ONLY active
  subscriptions (`removed_at IS NULL`). A removed peer vanishes (I-PS-2, the residue-made-
  visible promise).
- **Merge peers or claim counts.** Each peer is its own row; the claim count is per-peer,
  never a merged total or "consensus" row (I-PS-3 / J-003a).
- **Add any network seam to this route.** The subscription list + counts are a LOCAL DB
  read (I-PS-4). No PDS fetch, no DID re-resolution, no `peer pull` — `/peers` shows the
  LOCAL state as-is.
- **Re-render the per-peer claims, the counter threads, or the graph.** `/peers` shows
  the subscription set + per-peer COUNT only; drilling into a peer's claims is the
  existing `/peer-claims` / `/claims` surfaces, out of this slice.
- **Persist anything** (I-PS-6) or bind anything but 127.0.0.1 (I-PS-6).
- **Add a new crate** (I-PS-7 — workspace stays 21).
- **Issue one query per peer (N+1).** The active-subscription + per-peer-count read is
  ONE aggregate query per render (mirrors the slice-10/12 single-query discipline).

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey visualization investment (Phase 1.5). Slightly bigger than the flag
slices (slice-12/13) — a NEW route + a NEW read + a NEW render — but still a thin
~1-day vertical slice.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure), `adapter-http-viewer` (effect), `ports` (one new read method), `adapter-duckdb` (one new read impl + SQL), `cli`, `xtask` — all existing; NO new crate | No (single context) |
| Walking-skeleton integration points | 4: (1) the new `GET /peers` route, (2) the new `StoreReadPort` method, (3) the new `adapter-duckdb` read impl + SQL, (4) the new `render_peers_fragment` + `render_remove_guidance` (mirrors slice-08). Within ≤5. | No (≤5) |
| Estimated effort | ~1 day (one read method + one SQL + one render fragment mirroring the slice-08 render-only-command pattern + the slice-10 net-new-route shape) | No (≤2 weeks) |
| Independent user outcomes | 1 (see who I follow + the clean revocation path in the browser) — the empty state is a facet of the same outcome, not a separate one | No |

**## Scope Assessment: PASS — 3 stories, 1 context, 4 integration points (1 new route + 1 new read method + 1 new adapter read + 1 new render), estimated ~1 day. No new crate; workspace stays 21.**

The thing that would make it oversized — building a write/subscribe/unsubscribe
affordance into the viewer, or a `peer pull` / DID-re-resolution network seam, or a
per-peer claims drill-in — is explicitly OUT of scope (I-PS-1 read-only, I-PS-4
LOCAL-only). If DESIGN finds the read method + the render exceed 1 day, US-PS-003 (the
empty state) can split from US-PS-002 — but each remains a thin, independently-
demonstrable end-to-end slice on the `/peers` surface.

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-PS-001 | Read-only viewer capability: list active subscriptions (`removed_at IS NULL`) with per-peer local claim counts in ONE aggregate query; wire it into `GET /peers` | infrastructure-only |
| US-PS-002 | See on `/peers` every peer I currently follow — DID (verbatim) + local claim count + the render-only `openlore peer remove <did>` command; a removed peer is absent | J-003c |
| US-PS-003 | When I follow no peers, see a guided empty state on `/peers` telling me so and pointing me to `openlore peer add` | J-003c |

---

## Wave: DISCUSS / [REF] User stories with elevator pitches + AC

<!-- Full story bodies live in user-stories.md; elevator pitches + key AC themes summarized here for the single-narrative reader. Each AC names its driving route. -->

### US-PS-001 — Read-only active-subscription read + `/peers` wiring (`@infrastructure`)

`@infrastructure` — no Elevator Pitch (produces no user-visible output; enables
US-PS-002/003). It adds the new read-only `StoreReadPort` method (list active
subscriptions with per-peer claim counts), its `adapter-duckdb` read impl (active-only
+ per-peer-count SQL, ONE aggregate query), and the `GET /peers` handler wiring.

**Key AC themes**: the read returns ONLY active subscriptions (`removed_at IS NULL`);
each carries its PER-PEER local claim count (`COUNT(*) … WHERE author_did = <peer>`),
never a merged total; the read is ONE aggregate query per render (no N+1, invariant to
peer count); a soft-removed / purged peer is EXCLUDED; no-subscriptions returns an empty
vec (no error); the read adds NO mutation method to `StoreReadPort` (read-only by
construction); LOCAL only (no network).

### US-PS-002 — See who I follow + the render-only revocation command

**Elevator Pitch**
- Before: Maria has no browser surface that shows who she currently follows; to recall her subscriptions she greps the CLI / inspects the store, and to leave a peer she has to remember the exact `peer remove` syntax.
- After: open `http://127.0.0.1:<port>/peers` → a list of every peer she currently follows, each row showing the peer's DID and how many of its claims she holds locally, with the render-only command `openlore peer remove <did>` beside it; a peer she removed via the CLI is absent on reload.
- Decision enabled: Maria decides WHICH peer to unsubscribe from — and sees the exact, clean revocation command to run — confident that a removed peer vanishes from the list (no residue).

**Key AC themes** (driving route `GET /peers`, both shapes): each active subscription
renders one row showing the peer DID VERBATIM + its local claim count; beside each peer
is the render-only `openlore peer remove <did>` command (the `PEER_REMOVE_GUIDANCE_PREFIX`
+ bare DID, mirroring slice-08 `render_follow_guidance`) as TEXT, never an executable
control; the count is PER-PEER, never a merged total; a peer removed via the CLI is
ABSENT from `/peers` (active-only, the residue-made-visible AC); the view renders
identically under htmx fragment + no-JS full page (parity); the route is read-only (no
write/subscribe/unsubscribe control, no key); LOCAL only (renders offline).

### US-PS-003 — Guided empty state when following no one

**Elevator Pitch**
- Before: when Maria follows no peers, she has no way in the browser to confirm "I follow no one" vs "the view is broken," and no in-context pointer to how to start following.
- After: open `http://127.0.0.1:<port>/peers` with no active subscriptions → a guided empty state: "You are not subscribed to any peers." plus the render-only `openlore peer add <did>` command to start.
- Decision enabled: Maria confidently concludes she follows no one (not a bug) and learns, in-context, the CLI command to start subscribing.

**Key AC themes** (driving route `GET /peers`, both shapes): when the active-subscription
read returns empty, the view renders a guided empty state naming "no peers" + the
render-only `openlore peer add <did>` starting command; the empty state renders
identically under htmx fragment + no-JS full page; the empty state is never an error and
never a blank page; a peer that EXISTS only as a soft-removed / purged row still yields
the empty state (active-only — those rows are not subscriptions).

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-15 mints **NO new KPI ID**. Like slice-08–14 it REALIZES inherited KPIs on a new
facet (the `/peers` federation-management view). Full detail in `outcome-kpis.md`. The
relevant inherited KPIs:

- **KPI-FED-4** (`Zero purge residue` — the J-003c sovereignty signal): slice-15
  STRENGTHENS the READ side of the J-003c loop. The CLI `peer remove` guarantees zero
  residue; `/peers` makes that guarantee VISIBLE — a removed peer is absent from the
  list, so the operator can SEE the residue-free outcome rather than trust it blind.
- **KPI-VIEW-1** (`Time-to-see-store-contents` — legibility north-star): EXTENDED into the
  federation-management dimension (who do I follow, at a glance, zero CLI).
- **KPI-VIEW-2** (read-only, guardrail): MET — no write/subscribe/unsubscribe route, no
  key read. Release-blocking.
- **KPI-AV-2 / KPI-FED-1/2** (anti-merging, guardrails): MET — per-peer rows, per-peer
  counts, never a merged total. Release-blocking.
- **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS
  no-regression / read-only, guardrails): MET — the read is a LOCAL DB read, the route
  renders offline, references only the vendored htmx asset, serves a full page without
  HX-Request, and adds no write surface. Release-blocking.

A product hypothesis specific to this slice (a leading indicator OF KPI-FED-4, not a new
KPI ID):

> **Hypothesis**: We believe that surfacing the active subscription set + per-peer claim
> counts + the render-only `openlore peer remove <did>` command on `/peers` (P-001,
> subscription-manager hat) will increase the share of dogfood users who can answer "who
> do I follow and how do I leave cleanly?" without leaving the browser, because the
> read-only view makes the subscription set legible and the revocation path visible in
> one place. We will know this is true when, post-slice-15, users report (and opt-in
> telemetry shows) they open `/peers` to review subscriptions and copy the render-only
> `peer remove` command, then observe the removed peer vanish on reload.

> Detail rationale is in `outcome-kpis.md`. The cross-feature SSOT is
> `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the
read-only `StoreReadPort`, the `peer_subscriptions` table + `removed_at` soft-remove
(slice-03), the `page = chrome + fragment` render pattern (slice-06/07), and the
render-only-CLI-command pattern (`SEARCH_FOLLOW_GUIDANCE_PREFIX` / `render_follow_guidance`,
slice-08) all already exist. The thinnest end-to-end slice IS US-PS-002 (the `/peers`
list render with per-peer counts + the render-only `peer remove` command), backed by
US-PS-001 (the new read + wiring). US-PS-003 (the empty state) is a parallel thin slice
on the same fragment. Delivery sequence: US-PS-001 → US-PS-002 → US-PS-003. Each is
demonstrable in a single session against the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Requirements (functional + NFR + business rules): `requirements.md`
- User stories (combined, `## System Constraints` at top): `user-stories.md`
- Acceptance criteria (BDD, per theme): `acceptance-criteria.md`
- Outcome KPIs: `outcome-kpis.md`
- Definition of Ready: `dor-checklist.md`
- Wave decisions (WD-PS-*): `wave-decisions.md`

> Lean mode: the standalone journey-visual + journey-yaml + shared-artifacts-registry
> are NOT produced for this thin DELTA (mirroring the slice-08/12/13 lean set). The
> journey is the slice-03 `subscribe-and-read-federated.yaml` (step 4, `peer remove`),
> grounded verbatim; the single shared artifact (`peer_did`) is already registered there.

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)** for all 3 stories.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-15 | Low | Low | The job (J-003c) is already validated in `docs/product/jobs.yaml`; the journey is the slice-03 `subscribe-and-read-federated.yaml`. No design-direction ambiguity — the view mirrors the slice-08 render-only-command + slice-10 net-new-route patterns. Noted as non-blocking risk. |
| New read method becomes N+1 (one count query per peer) | Medium | High | I-PS-3/I-PS-4 + US-PS-001 AC make the single-aggregate-query-per-render a HARD product commitment (active-subscriptions joined to a per-peer `COUNT(*)`, ONE query). A behavioral test asserts query count invariant to peer count. DESIGN owns the exact SQL (correlated subquery vs `GROUP BY`); a per-peer `count_peer_claims` fold is explicitly REJECTED. |
| Render-only command misread as a button / executed | Low | Medium | The command MIRRORS the slice-08 `render_follow_guidance` precedent (already vetted): a render-only `<p>`/`<code>` of TEXT, never an `<a>` that executes, never a form. The viewer holds no key (I-PS-1). |
| A soft-removed peer leaks into `/peers` (residue made visible broken) | Medium | High | I-PS-2 + US-PS-002/003 AC require the read filter `removed_at IS NULL`; a behavioral test seeds a soft-removed peer and asserts it is ABSENT from `/peers`. The slice-03 `soft_remove` sets `removed_at`, the precondition for the filter. |
| Per-peer count merged into a total (anti-merging broken) | Low | High | I-PS-3 + US-PS-002 AC require the count be PER-PEER (`WHERE author_did = <this peer>`); the existing `count_peer_claims(conn, peer_did)` confirms the per-peer shape; a domain example with two peers of distinct counts pins it. |

---

## Changelog

- 2026-06-09 — slice-15 (`viewer-peer-subscriptions`) DISCUSS. Traces to J-003c (the
  VIEWING / LEGIBILITY side of "subscription is revocable without residue"; the
  without-residue GUARANTEE itself stays the slice-03 CLI `peer remove`). 3 stories (1
  infra + 2 user-visible). Net-new read-only `GET /peers` route + a NEW read-only
  `StoreReadPort` method (list active subscriptions w/ per-peer counts; new SQL in
  adapter-duckdb) — the first slice since slice-10 to add a read method. REUSES the
  slice-08 render-only-CLI-command pattern (`render_follow_guidance` → `render_remove_guidance`,
  `peer remove`). NO new crate (workspace stays 21), no new KPI ID. CARDINAL decisions:
  read-only / no-key (I-PS-1); active-only / residue-made-visible (I-PS-2). Scope PASS
  (~1 day). DoR PASS (9/9).
