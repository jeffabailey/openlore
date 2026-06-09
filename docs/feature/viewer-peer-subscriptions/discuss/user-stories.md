<!-- markdownlint-disable MD024 -->
# User Stories: viewer-peer-subscriptions (slice-15)

> Combined file (one section per story). Brownfield DELTA on slices 03/06/07/08/10.
> Every non-`@infrastructure` story traces to **J-003c** (`docs/product/jobs.yaml`).
> The unsubscribe stays the slice-03 CLI (`openlore peer remove`); the viewer is
> read-only, holds no key. The render-only command MIRRORS the slice-08
> `render_follow_guidance` precedent.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: `/peers` holds `StoreReadPort` only — no
  mutation method, no signing key in the viewer process, no write/subscribe/unsubscribe
  control on the rendered surface. The unsubscribe is render-only `openlore peer remove
  <did>` TEXT, never an executable control. Subscribe/unsubscribe stays EXCLUSIVELY the
  slice-03 CLI. Enforced 3 layers (type: the read port has no mutation method + xtask
  check-arch viewer-capability rule + behavioral gold). [KPI-VIEW-2, slice-06–14, slice-08
  WD-NS-3]
- **C-2 Active-only / residue made visible (CARDINAL)**: `/peers` lists ONLY active
  subscriptions (`peer_subscriptions.removed_at IS NULL`). A peer removed via the CLI
  (soft-remove OR `--purge`) VANISHES from `/peers` — that absence IS the J-003c
  "revocable without residue" promise rendered. The read NEVER shows soft-removed
  (`UnsubscribedCache`) rows. [J-003c, slice-03 ADR-014]
- **C-3 Per-peer, never merged**: each peer is its own attributed row; the claim count
  is PER-PEER (`COUNT(*) FROM peer_claims WHERE author_did = <this peer>`), NEVER a merged
  total across peers, never a "consensus peer" row. The list is keyed by peer DID.
  [J-003a, KPI-AV-2, KPI-FED-1/2, slice-03 I-FED-1]
- **C-4 LOCAL-only / offline**: the subscription list + per-peer counts are a LOCAL
  DuckDB read (`peer_subscriptions` + `peer_claims`); NO network seam on this route.
  `/peers` renders fully with the network down and references only the vendored local
  `/static/htmx.min.js` (no CDN). [KPI-5, KPI-VIEW-5, slice-10 WD-GT-4, slice-06/07 KPI-HX-G2]
- **C-5 Progressive enhancement + parity**: an `HX-Request` returns the `/peers`
  fragment; a no-JS / bookmark / direct-URL request returns the full page = chrome + the
  SAME fragment. The render-only command + the empty state live in the SAME fragment fn
  both shapes embed, so they render identically. A swap is a nicety, never a requirement.
  [slice-07 KPI-HX-G1/G2/G3, slice-08 WD-NS-6, slice-10 WD-GT-8]
- **C-6 No new crates**: extend the PURE `viewer-domain` + EFFECT `adapter-http-viewer`
  + `ports` (ONE new read method) + `adapter-duckdb` (ONE new read impl + SQL) + `cli` +
  `xtask`. Workspace stays 21 members. Functional paradigm (ADR-007). [slice-06–14]
- **C-7 Render-only revocation command (single source of truth)**: the `openlore peer
  remove <did>` text is held in ONE place (a `PEER_REMOVE_GUIDANCE_PREFIX` const,
  mirroring the slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX`), rendered with the bare-DID
  strip (`render_remove_guidance`, mirroring `render_follow_guidance`) as a render-only
  `<p>`/`<code>` — never an `<a>` that executes, never a form. [slice-08 precedent]
- **C-8 ONE aggregate query per render (no N+1)**: the active-subscription + per-peer-
  count read is ONE aggregate query, invariant to the number of active subscriptions. A
  per-peer `count_peer_claims` fold (N+1) is REJECTED. [I-PS-3/4, slice-10/12 discipline]

---

## US-PS-001: Read-only viewer capability to list active subscriptions with per-peer claim counts, wired into `GET /peers` (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-PS-001 adds the new read-only `StoreReadPort` method
(`list_active_peer_subscriptions` — proposed) that returns every ACTIVE subscription
(`removed_at IS NULL`) paired with its PER-PEER local claim count, its `adapter-duckdb`
read impl (the new active-only + per-peer-count SQL, ONE aggregate query), and the
`GET /peers` handler wiring that calls the read and hands the result to the pure
projection. It produces no user-visible output on its own (the rendered list, the
revocation command, and the empty state are US-PS-002/003), so it enables a user
decision only THROUGH those stories. The slice contains TWO non-infrastructure,
user-visible stories (US-PS-002, US-PS-003), so the slice has release value (Dimension-0
slice-level check passes). This is a READ-ONLY capability by construction: the new method
is on `StoreReadPort`, which declares no mutation method.

### Problem

The `peer_subscriptions` table records who the operator follows and the `peer_claims`
table holds each peer's cached claims, but the read-only viewer has no method to list the
ACTIVE subscriptions with their per-peer claim counts. Without it, `/peers` cannot be
rendered; and a naive per-peer count loop would reintroduce an N+1 the single-query
discipline (slice-10/12) exists to avoid.

### Who

- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the
  subscription-manager-hat stories (US-PS-002/003) consume.

### Solution

Add a read-only `StoreReadPort` method returning `Vec<PeerSubscriptionSummary>` where
each summary is `{ peer_did, peer_handle, subscribed_at, local_claim_count }`, computed
from `peer_subscriptions WHERE removed_at IS NULL` joined to a PER-PEER `COUNT(*)` over
`peer_claims` (grouped by `author_did`) — ONE aggregate query. Implement it in
`adapter-duckdb` over the SAME shared connection the CLI writes through (BR-VIEW-4). Wire
`GET /peers` to call it and pass the result into the pure `viewer-domain` projection. NO
mutation method; NO network; NO per-peer query loop.

### Domain Examples

#### 1: Happy path — two active peers with distinct counts in one query

Maria follows `did:plc:rachel-test` (5 cached claims) and `did:plc:tobias-test` (3 cached
claims). The read returns TWO summaries — Rachel with `local_claim_count = 5`, Tobias with
`3` — from ONE aggregate query, ordered by `subscribed_at`.

#### 2: Edge case — a soft-removed peer is excluded

Maria followed `did:plc:rachel-test` then ran `openlore peer remove did:plc:rachel-test`
(no `--purge`), so its row now has `removed_at` set and its 5 `peer_claims` are retained.
The read returns ONLY `did:plc:tobias-test` — Rachel is excluded by the `removed_at IS
NULL` filter, even though her cached claims still exist.

#### 3: Boundary — no active subscriptions returns an empty vec

Maria follows no peers (or has only soft-removed rows). The read returns an EMPTY vec —
NOT an error — so the viewer renders the guided empty state (US-PS-003).

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (port-to-port via the real `openlore ui`
> subprocess). No scenario calls the read method directly.

#### Scenario: The peers read returns active subscriptions with per-peer counts in one aggregate query

Given Maria's store has active subscriptions to `did:plc:rachel-test` (5 cached claims) and `did:plc:tobias-test` (3 cached claims)
When she opens `GET /peers` in the `openlore ui` viewer
Then the page lists exactly two peers
And Rachel's row shows a local claim count of 5 and Tobias's row shows 3
And the subscription set + counts are resolved in exactly ONE aggregate query (invariant to peer count)

#### Scenario: A soft-removed peer is excluded from the peers read

Given Maria subscribed to `did:plc:rachel-test` then ran `openlore peer remove did:plc:rachel-test` (no --purge), leaving her cached claims
And Maria still actively follows `did:plc:tobias-test`
When she opens `GET /peers`
Then the page lists only `did:plc:tobias-test`
And `did:plc:rachel-test` is absent (active-only filter, removed_at IS NULL)

#### Scenario: No active subscriptions resolves to an empty result without error

Given Maria has no active peer subscriptions
When she opens `GET /peers`
Then the peers read returns an empty result (not an error)
And the viewer renders the guided empty state (US-PS-003)

### Acceptance Criteria

- [ ] A new read-only method on `StoreReadPort` returns every ACTIVE subscription (`removed_at IS NULL`) each paired with its PER-PEER local claim count
- [ ] The per-peer count is `COUNT(*) FROM peer_claims WHERE author_did = <peer>` — never a merged/global total
- [ ] The read is ONE aggregate query per render, invariant to the number of active subscriptions (no N+1)
- [ ] A soft-removed / purged peer is EXCLUDED from the result
- [ ] No-active-subscriptions returns an empty result, never an error
- [ ] The new method is on `StoreReadPort` (read-only by construction — no mutation method added to the trait)
- [ ] The read is LOCAL only (no network seam) and runs over the SAME shared connection the CLI writes through (BR-VIEW-4)
- [ ] `GET /peers` is wired to call the read and pass the result into the pure `viewer-domain` projection

### Outcome KPIs

- **Who**: the viewer process serving `GET /peers`
- **Does what**: resolves the active subscription set + per-peer claim counts for the whole page in one aggregate query
- **By how much**: exactly 1 aggregate query per render, invariant to peer count (0 N+1)
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (query count invariant to peer count)
- **Baseline**: today the viewer has no peers read; slice-15 adds exactly 1 aggregate query, never N

### Technical Notes

- Proposed method `fn list_active_peer_subscriptions(&self) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>` on `StoreReadPort` (`crates/ports/src/store_read.rs`). DESIGN owns the exact signature + whether `PeerSubscriptionSummary` lives in `ports` beside `PeerClaimRow` or in `viewer-domain`.
- The new SQL: `peer_subscriptions WHERE removed_at IS NULL` joined to a per-`author_did` `COUNT(*)` over `peer_claims`. DESIGN owns correlated-subquery vs `LEFT JOIN … GROUP BY`; a per-peer `count_peer_claims` fold is REJECTED (N+1).
- The existing write-side `PeerStoragePort::list_active_subscriptions()` (`crates/adapter-duckdb/src/peer_storage.rs`, `SELECT … WHERE removed_at IS NULL ORDER BY subscribed_at`) is NOT reused (write port, no counts); the free fn `count_peer_claims(conn, peer_did)` (same file) confirms the per-peer count SQL shape.
- READ-ONLY: shares the CLI's connection (BR-VIEW-4); the trait declares no mutation method (I-VIEW-1).

---

## US-PS-002: See every peer I currently follow — DID + local claim count + the render-only `openlore peer remove <did>` command

`job_id: J-003c`

### Problem

Maria subscribed to several peers over time. To recall WHO she currently follows she has
to grep the CLI or inspect the store, and to leave a peer she has to remember the exact
`openlore peer remove` syntax. There is no browser surface that shows her current
subscriptions, how many of each peer's claims she holds locally, and the clean revocation
path — and no way to SEE that a removed peer actually left without residue.

### Who

- P-001 (the viewer operator, "Maria"), subscription-manager hat | reviewing her
  federation subscriptions in the browser | wants to see who she follows and how to leave
  a peer cleanly, confident that a removed peer vanishes (no residue).

### Solution

On `/peers`, each ACTIVE subscription renders one row showing the peer's DID (VERBATIM),
its local claim count (`peer_claims WHERE author_did = <peer>`), and the render-only
revocation command `openlore peer remove <bare-did>` (the `PEER_REMOVE_GUIDANCE_PREFIX` +
bare DID, via `render_remove_guidance` — mirroring slice-08 `render_follow_guidance`) as
TEXT, never an executable control. A peer removed via the CLI is ABSENT on the next render
(active-only). The list renders identically under htmx fragment + no-JS full page.

### Elevator Pitch

- **Before**: Maria has no browser surface that shows who she currently follows; to recall her subscriptions she greps the CLI / inspects the store, and to leave a peer she has to remember the exact `openlore peer remove` syntax.
- **After**: open `http://127.0.0.1:<port>/peers` → a list of every peer she currently follows, each row showing the peer's DID and how many of its claims she holds locally, with the render-only command `openlore peer remove <did>` beside it; a peer she removed via the CLI is absent on reload.
- **Decision enabled**: Maria decides WHICH peer to unsubscribe from — and sees the exact, clean revocation command to run — confident that a removed peer vanishes from the list (no residue).

### Domain Examples

#### 1: Happy path — two followed peers, each with its count and revoke command

Maria follows `did:plc:rachel-test` (5 cached claims) and `did:plc:tobias-test` (3 cached
claims). `/peers` shows two rows: Rachel — DID `did:plc:rachel-test`, "5 claims",
`openlore peer remove did:plc:rachel-test`; Tobias — DID `did:plc:tobias-test`, "3
claims", `openlore peer remove did:plc:tobias-test`. Each command is plain TEXT she can
copy; neither is a button.

#### 2: Edge case — a peer with zero cached claims still appears with its revoke command

Maria subscribed to `did:plc:newpeer-test` but never ran `openlore peer pull`, so she
holds 0 of its claims. `/peers` still lists it — DID `did:plc:newpeer-test`, "0 claims",
`openlore peer remove did:plc:newpeer-test`. (Following is recorded independently of
pulling; the row shows the subscription exists.)

#### 3: Boundary / residue-made-visible — a removed peer vanishes on reload

Maria sees `did:plc:rachel-test` on `/peers`, copies `openlore peer remove
did:plc:rachel-test`, runs it in her terminal, and reloads `/peers`. Rachel's row is GONE
— even if her cached `peer_claims` are still on disk (no `--purge`) — because `/peers`
reads only active subscriptions. The absence IS the residue-free promise rendered.

### UAT Scenarios (BDD)

> Driving route: `GET /peers` (the real `openlore ui` subprocess), both shapes.

#### Scenario: The peers list shows each followed peer's DID, local claim count, and render-only revoke command

Given Maria actively follows `did:plc:rachel-test` (5 cached claims) and `did:plc:tobias-test` (3 cached claims)
When she opens `GET /peers` in the viewer
Then she sees two rows, one per peer
And Rachel's row shows the DID `did:plc:rachel-test`, a local claim count of 5, and the render-only command `openlore peer remove did:plc:rachel-test`
And Tobias's row shows the DID `did:plc:tobias-test`, a local claim count of 3, and the render-only command `openlore peer remove did:plc:tobias-test`
And neither command is an executable control (no button, no form, no mutating link)

#### Scenario: A peer removed via the CLI is absent from the peers list (residue made visible)

Given Maria actively follows `did:plc:rachel-test` and it appears on `/peers`
When she runs `openlore peer remove did:plc:rachel-test` in her terminal and reopens `GET /peers`
Then `did:plc:rachel-test` is absent from the list
And its absence holds even though its cached peer claims remain on disk (no --purge)

#### Scenario: The peers list renders identically under htmx and no-JS

Given Maria actively follows one peer
When she requests `GET /peers` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the peers view-panel fragment with the peer row + its render-only revoke command
And the no-JS response is the full page = chrome + the SAME fragment, rendered identically

#### Scenario: The per-peer count is never a merged total

Given Maria follows `did:plc:rachel-test` (5 cached claims) and `did:plc:tobias-test` (3 cached claims)
When she opens `GET /peers`
Then Rachel's row shows 5 and Tobias's row shows 3 (per-peer)
And no row shows a combined total of 8 and there is no merged "all peers" row

### Acceptance Criteria

- [ ] Each active subscription renders one row showing the peer DID VERBATIM (bare `did:plc:…`)
- [ ] Each row shows that peer's LOCAL claim count (`peer_claims WHERE author_did = <peer>`) — a per-peer count, never a merged total
- [ ] Beside each peer is the render-only `openlore peer remove <bare-did>` command (the `PEER_REMOVE_GUIDANCE_PREFIX` + bare DID), as TEXT — never a button, form, or mutating link
- [ ] A peer removed via the CLI is ABSENT from `/peers` on the next render (active-only — the residue-made-visible AC), even when its cached claims remain on disk
- [ ] The list renders identically under the htmx fragment and the no-JS full page (parity by construction — same fragment fn)
- [ ] The route is read-only: no write/subscribe/unsubscribe control, no signing key
- [ ] The read is LOCAL only (renders offline)

### Outcome KPIs

- **Who**: P-001 dogfood operators managing federation subscriptions
- **Does what**: reviews their active subscriptions in the browser and copies the render-only `peer remove` command to leave a peer cleanly
- **By how much**: leading indicator OF KPI-FED-4 (zero purge residue) — a measurable share open `/peers` to review subscriptions and observe a removed peer vanish on reload
- **Measured by**: per-feature GREEN (the list renders active peers + counts + the render-only command; a removed peer is absent); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today there is no browser surface for the subscription set; recalling who you follow requires CLI/store inspection

### Technical Notes

- Render: a new `render_peers_fragment` in `viewer-domain` mapping `Vec<PeerSubscriptionSummary>` to a list. The revoke command REUSES the slice-08 pattern: a `PEER_REMOVE_GUIDANCE_PREFIX` const + a `render_remove_guidance(bare_did)` fn (mirror `SEARCH_FOLLOW_GUIDANCE_PREFIX` / `render_follow_guidance` at `crates/viewer-domain/src/lib.rs` ~1513/1759).
- The bare-DID strip mirrors `render_follow_guidance` (`author_did.split('#').next()`).
- DESIGN owns whether the row is a `<tr>`/`<li>` and the exact markup; the PRODUCT contract is the AC.

---

## US-PS-003: When I follow no peers, see a guided empty state on `/peers`

`job_id: J-003c`

### Problem

When Maria follows no peers, opening `/peers` to an empty or blank page leaves her unsure
whether she truly follows no one or whether the view is broken — and gives her no
in-context pointer to how to start following.

### Who

- P-001 (the viewer operator, "Maria"), subscription-manager hat | opening `/peers` with
  no active subscriptions | wants to confirm she follows no one (not a bug) and learn how
  to start.

### Solution

When the active-subscription read returns empty, `/peers` renders a guided empty state:
"You are not subscribed to any peers." plus the render-only starting command `openlore
peer add <did>`. The empty state lives in the SAME fragment fn both shapes embed, so it
renders identically under htmx + no-JS. A soft-removed-only store (rows exist but all
have `removed_at` set) still yields the empty state (active-only — those are not
subscriptions).

### Elevator Pitch

- **Before**: when Maria follows no peers, she has no way in the browser to confirm "I follow no one" vs "the view is broken," and no in-context pointer to how to start following.
- **After**: open `http://127.0.0.1:<port>/peers` with no active subscriptions → a guided empty state: "You are not subscribed to any peers." plus the render-only `openlore peer add <did>` command to start.
- **Decision enabled**: Maria confidently concludes she follows no one (not a bug) and learns, in-context, the CLI command to start subscribing.

### Domain Examples

#### 1: Happy path — fresh store, no subscriptions

Maria has never subscribed to anyone. `/peers` shows "You are not subscribed to any
peers." and the render-only command `openlore peer add <did>`. No empty table headers, no
blank panel.

#### 2: Edge case — only soft-removed peers remain

Maria subscribed to `did:plc:rachel-test`, then removed her (no `--purge`), and follows
no one else. `/peers` shows the SAME empty state — the soft-removed row is residue, not an
active subscription.

#### 3: Boundary — empty state renders offline and under both shapes

With the network down and no subscriptions, `/peers` still renders the guided empty state
(it is a LOCAL read), identically whether requested with or without `HX-Request`.

### UAT Scenarios (BDD)

> Driving route: `GET /peers` (the real `openlore ui` subprocess), both shapes.

#### Scenario: No active subscriptions shows the guided empty state

Given Maria has no active peer subscriptions
When she opens `GET /peers`
Then she sees the message "You are not subscribed to any peers."
And she sees the render-only starting command `openlore peer add <did>`
And the page is not blank and is not an error

#### Scenario: A store with only soft-removed peers still shows the empty state

Given Maria's only `peer_subscriptions` row is soft-removed (removed_at set) and she follows no one else
When she opens `GET /peers`
Then she sees the guided empty state (the soft-removed row is residue, not an active subscription)

#### Scenario: The empty state renders identically under htmx and no-JS

Given Maria has no active peer subscriptions
When she requests `GET /peers` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the peers fragment containing the empty state
And the no-JS response is the full page = chrome + the SAME fragment, rendered identically

### Acceptance Criteria

- [ ] When the active-subscription read returns empty, `/peers` renders a guided empty state naming "no peers" (e.g. "You are not subscribed to any peers.")
- [ ] The empty state includes the render-only starting command `openlore peer add <did>`
- [ ] The empty state is never an error and never a blank page
- [ ] A store whose only `peer_subscriptions` rows are soft-removed still yields the empty state (active-only)
- [ ] The empty state renders identically under the htmx fragment and the no-JS full page
- [ ] The empty state renders offline (LOCAL read)

### Outcome KPIs

- **Who**: P-001 dogfood operators who follow no peers (e.g. fresh installs, or after removing everyone)
- **Does what**: confirms in the browser that they follow no one (not a bug) and learns the CLI command to start
- **By how much**: leading indicator OF KPI-VIEW-1 — first-run operators reach an unambiguous "no peers + how to start" state with zero CLI inspection
- **Measured by**: per-feature GREEN (the empty state renders with the starting command; never blank/error); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today there is no `/peers` surface; a no-subscriptions operator has no in-browser confirmation or onboarding pointer

### Technical Notes

- The empty state is the `Vec::is_empty()` arm of `render_peers_fragment` (same fn as US-PS-002), so parity is by construction.
- The starting command REUSES the render-only-command pattern with a `peer add` prefix (the slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX = "Follow this author from the CLI: openlore peer add"` is the exact precedent for `peer add` guidance text).

---

## Out of scope (explicit — restated from feature-delta)

- **Subscribing / unsubscribing from the viewer** — no follow/unfollow/remove/purge
  button, form, or control. Stays the slice-03 CLI; the unsubscribe is render-only
  `openlore peer remove <did>` TEXT (C-1 / C-7).
- **Holding a signing key or any mutation capability in the viewer** (C-1, CARDINAL).
- **Showing soft-removed / unsubscribed-cache peers** — `/peers` is active-only (C-2).
- **Merging peers or claim counts** — per-peer rows, per-peer counts (C-3).
- **Any network seam on this route** — LOCAL DB read only; no PDS fetch, no DID
  re-resolution, no `peer pull` (C-4).
- **Re-rendering a peer's claims / counter threads / the graph** — `/peers` shows the
  subscription set + per-peer COUNT only; drilling in is `/peer-claims` / `/claims`
  (existing surfaces).
- **Persisting anything; binding anything but 127.0.0.1; adding a new crate** (C-6).
- **N+1 (one count query per peer)** — ONE aggregate query per render (C-8).
