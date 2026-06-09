# Evolution: viewer-peer-subscriptions (slice-15 read-only `GET /peers` federation-management view on the viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-peer-subscriptions/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-052 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-06 (`htmx-scraper-viewer` — the read-only viewer), slice-08
> (`viewer-network-search` — the source of the **render-only command** pattern
> `render_follow_guidance` this slice reuses), slice-10 (`viewer-graph-traversal` —
> the net-new-route shape this slice mirrors), and the slice-03 federation CLI
> (`openlore peer add` / `peer pull` / `peer remove` — the subscribe/unsubscribe
> MUTATION surface this view deliberately does NOT replace). Read those parent
> archives (`docs/evolution/htmx-scraper-viewer-evolution.md`,
> `viewer-network-search-evolution.md`, `viewer-graph-traversal-evolution.md`) for the
> surfaces this slice composes. slice-15 realizes the **VIEWING side of J-003c**
> ("Subscription is revocable without residue").

## Summary

`viewer-peer-subscriptions` adds one **read-only federation-management view** to the
`openlore ui` read-only viewer: **`GET /peers`**. It lists the operator's currently-active
peer subscriptions — every `peer_subscriptions` row where `removed_at IS NULL` — and for
each peer renders its **DID VERBATIM** + its **per-peer LOCAL claim count** + a **render-only
`openlore peer remove <did>` revocation command TEXT** (not an executable control). When
there are no active subscriptions it renders a guided empty state pointing the operator to
`openlore peer add`. This is the J-003c "see who I'm subscribed to and how to walk it back"
job the node operator reaches in the browser — the **viewing** surface for federation
management; the subscribe/unsubscribe **mutation** stays the slice-03 CLI (the viewer is
read-only and holds no key).

The load-bearing thesis: **a read-only management view whose ACTIVE-ONLY filter makes the
J-003c residue-free promise VISIBLE, while taking on authority over nothing**. The view reads
the LOCAL store read-only, filtering `removed_at IS NULL`, so a peer that was removed via the
CLI is **ABSENT from `/peers`** even though its cached `peer_claims` remain on disk — that
absence IS the "revocable without residue" guarantee rendered. The CARDINAL concerns are
three: (1) **read-only / no-key** — the port gains only a read method, `/peers` renders the
revocation as render-only command TEXT (mirroring slice-08's `render_follow_guidance`), no
executable control; (2) **active-only / residue-made-visible** — the filter is the J-003c
promise made glanceable; (3) **per-peer not-merged (J-003a)** — the claim count is grouped
per `peer_did`, never a network total, no "all peers" row. Read-only is enforced at **three
layers** (a `StoreReadPort` with no mutation method [TYPE], the `xtask check-arch` viewer
capability rule [STRUCTURAL], and a behavioral GOLD invariant [BEHAVIORAL]).

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is an **additive
render surface, not a re-architecture**: it extends `viewer-domain` (a pure `PeersView` ADT +
the `render_peers_*` parity split + `render_remove_guidance`), `adapter-http-viewer` (the
`GET /peers` handler + the `Shape` fork + nav link), the `adapter-duckdb` read impl (ONE
aggregate survey query), the `ports` (one read seam + the `PeerSubscriptionSummary` DTO), and
the `cli` (`ui` wiring, still no key). It REUSES the slice-08 render-only-command pattern (the
`PEER_REMOVE_GUIDANCE_PREFIX` / `PEER_ADD_GUIDANCE_PREFIX` constants follow `render_follow_guidance`)
and the slice-10 net-new-route shape (page = chrome + fragment, the `Shape` fork). Unlike
slice-10 it adds **NO new pure-core edge** — `render` is a total function of the flat
`PeerSubscriptionSummary` DTO (no `viewer-domain → claim-domain` reachout); `xtask check-arch`
is **UNCHANGED**.

### What shipped (one paragraph)

One `GET` view — **`GET /peers`** — that on request runs a single read-only aggregate survey
query (`peer_subscriptions ps LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did WHERE
ps.removed_at IS NULL GROUP BY ps.peer_did COUNT(pc.cid)`), maps the rows to a pure `PeersView`
ADT (`Subscriptions{peers} | NoSubscriptions`), and projects them into HTML, forking by
`Shape::from_request` (the slice-07/10 `HX-Request` selector) — a full page (chrome + fragment)
without the header, the peers fragment with it. Each peer row carries its **DID verbatim** +
its **per-peer LOCAL claim count** + a **render-only `openlore peer remove <did>` command TEXT**
(via `render_remove_guidance` / `PEER_REMOVE_GUIDANCE_PREFIX`, mirroring slice-08's
`render_follow_guidance` — TEXT, not an executable button). The **LEFT JOIN keeps zero-claims
(never-pulled) peers**, with `COUNT(pc.cid)` yielding **0 (not 1)** for them. The read filters
**`removed_at IS NULL`** (active-only), so a CLI-removed peer is ABSENT even though its cached
`peer_claims` survive on disk — the residue-free promise rendered as that absence. The count is
**grouped per `peer_did`** — never a network total, no "all peers" row (J-003a anti-merging).
When there are no active subscriptions the view renders the guided `NoSubscriptions` empty state
pointing to `openlore peer add` (`PEER_ADD_GUIDANCE_PREFIX`). The `GET /peers` handler **degrades
to `NoSubscriptions` on read error** (no 5xx). The store read is **LOCAL and read-only** (offline,
no network); the bind stays loopback-only; nothing is persisted; the viewer holds no key.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-09 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-09 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-09 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-09 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **9/9 roadmap steps** done across **3 phases** (all COMMIT/PASS in
  `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_peer_subscriptions` corpus (PS-1..PS-7 —
  including the **thick walking skeleton** at 01-01) + the GOLD invariants
  (`viewer_peer_subscriptions_invariants` — read-only / no-write, offline, and N+1-free).
  Plus the `viewer-domain` unit/property tests (the new `PeersView` projection +
  `render_peers_*` parity + `render_remove_guidance`). The `ViewerServer` harness drives the
  REAL `openlore ui` over HTTP; the store is seeded through the REAL `peer add` / `peer pull`
  / `peer remove` verbs.
- **Slices 06/08/10 + slice-03 corpora GREEN — zero regression** (the full workspace
  acceptance suite green across all slices).
- **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT) +
  `adapter-duckdb` (EFFECT, read impl) + `ports` (the read seam + the `PeerSubscriptionSummary`
  DTO) + `cli` (DRIVER) in place; REUSES the slice-08 render-only-command pattern + the
  slice-10 net-new-route shape. Workspace member count stays **21**; `cargo xtask check-arch`
  reports "21 workspace members".
- **NO new pure-core edge** (unlike slice-10): `render` is a total function of the flat
  `PeerSubscriptionSummary` DTO — no `viewer-domain → claim-domain` reachout. `xtask
  check-arch` allowlist is UNCHANGED.
- **NO new production dependency**: `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate** on the new pure `viewer-domain` production functions (**7/7
  viable caught, 0 missed**) on the in-diff scope — exceeds the ≥80% per-feature gate.
- **1 ADR** (ADR-052) Accepted/shipped.
- DES integrity: 9/9 steps have complete DES traces.
- Adversarial review: **APPROVED**, 0 defects, zero Testing Theater.
- Gates: DoR 9/9, DESIGN APPROVED, DISTILL APPROVED 9.8/10.
- `cargo xtask check-arch`: OK (21 workspace members; the anti-merging SQL rule stays green by
  construction — the aggregate query names `peer_subscriptions` + `peer_claims` but not the
  standalone `claims` table).

## Wave-by-wave changelog

### DISCUSS (2026-06-09)

Luna framed the slice as a **brownfield DELTA on slices 06/08/10 + the slice-03 federation
CLI** that realizes the **VIEWING side of J-003c** ("Subscription is revocable without
residue"). Persona is **P-001 (the node operator)**, the viewer's operator wearing the
federation-management hat. The load-bearing DISCUSS decision: **`/peers` is a read-only
management VIEW, not a control surface** — it lists the operator's active subscriptions (each
peer's DID verbatim + per-peer local claim count) and tells the operator how to walk a
subscription back, but the revocation itself stays the slice-03 `openlore peer remove` CLI (the
viewer holds no key). The CARDINAL framing insight: **the active-only filter makes the J-003c
residue-free promise VISIBLE** — a CLI-removed peer disappears from `/peers` even though its
cached claims remain on disk, so the absence IS the promise rendered. The walking skeleton is
the thick `/peers` thread (the route + the `Shape` fork + the `PeersView` ADT + the read seam +
the aggregate query + the parity render + the `ui` wiring), validating the riskiest assumption
first — that a read-only management view can render the active-subscription survey with per-peer
(not-merged) counts and a render-only revocation command while staying key-free.

### DESIGN (2026-06-09)

Morgan locked slice-15 as an **additive render surface, not a re-architecture** — ZERO new
crates, ZERO new binary, ZERO new architectural style, ZERO new persisted type, ZERO new
pure-core edge. The open decisions were resolved adopting the DISCUSS leans, captured in one
ADR:

- **ADR-052** (viewer peer-subscriptions read view — one aggregate query, render-only
  revocation, no new core edge): a **NEW read-only seam** `list_active_peer_subscriptions()` +
  a **`PeerSubscriptionSummary` DTO** on the store read port (the DTO lives in `ports`),
  implemented as **ONE aggregate query** — `peer_subscriptions ps LEFT JOIN peer_claims pc ON
  pc.author_did = ps.peer_did WHERE ps.removed_at IS NULL GROUP BY ... COUNT(pc.cid)`. The
  **LEFT JOIN keeps zero-claims (never-pulled) peers** in the result; `COUNT(pc.cid)` yields
  **0 (not 1)** for them (counting the joined column, never the row). ONE query, **no N+1**.
  The view is a **NEW pure `viewer-domain` projection** — a `PeersView` ADT (`Subscriptions{peers}
  | NoSubscriptions`), the `render_peers_fragment` / `render_peers_page` parity split
  (page = chrome + fragment), `render_remove_guidance`, and the `PEER_REMOVE_GUIDANCE_PREFIX` /
  `PEER_ADD_GUIDANCE_PREFIX` constants — the **render-only command TEXT pattern REUSED from
  slice-08's `render_follow_guidance`** (TEXT, not an executable control). The `GET /peers`
  handler **degrades to `NoSubscriptions` on read error** (no 5xx). **NO new pure-core edge**:
  `render` is a total function of the flat `PeerSubscriptionSummary` DTO (unlike slice-10's
  `viewer-domain → claim-domain` edge), so `xtask check-arch` is UNCHANGED — the anti-merging
  SQL rule stays green by construction (the query names `peer_subscriptions` + `peer_claims`,
  not the standalone `claims` table).

The read-only contract is enforced at THREE layers (a `StoreReadPort` with no mutation method,
the `xtask check-arch` viewer capability rule [unchanged], and a behavioral GOLD invariant).
The C4 views, the `/peers` data-flow, and the I-PS-1..n structural-guarantee table are in the
DESIGN sections of `feature-delta.md` and `design/`. DISTILL closed at **APPROVED 9.8/10**.

### DISTILL (2026-06-09)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_peer_subscriptions.rs`** (Tier A — `PS-` ids PS-1..PS-7): the **thick walking
  skeleton** (PS-1 — the `/peers` active-subscription fragment naming each peer's DID verbatim +
  per-peer local claim count + the render-only `openlore peer remove <did>` command), the
  **fragment/page parity** (PS-2 — page = chrome + fragment), the **anti-merging per-peer count**
  (PS-3 — counts grouped per `peer_did`, never a network total, no "all peers" row, J-003a), the
  **active-only / residue-made-visible** (PS-4 — a CLI-removed peer is ABSENT from `/peers` while
  its cached `peer_claims` remain on disk, fed by a residue seed that retains the cached claims),
  the **zero-claims LEFT JOIN** (PS-5 — a subscribed-without-pulling peer renders with count 0,
  not 1, fed by a subscribe-without-pull seed), the **guided empty state** (PS-6 — `NoSubscriptions`
  pointing to `openlore peer add` when there are no active subscriptions), and **only-removed →
  empty** (PS-7 — when every subscription is removed, `/peers` renders the empty state).
- **`viewer_peer_subscriptions_invariants.rs`** (gold guardrails): **read-only / no-write** (no
  sign/publish/subscribe/remove EXECUTABLE control on any shape — the revocation is render-only
  TEXT; store row counts unchanged across rich/empty × page/fragment), **offline** (the survey
  reads the LOCAL store with no network; the page references only the vendored local htmx asset,
  no CDN; loopback-only), and **N+1-free** (the active-subscription list is one aggregate query,
  not one-per-peer).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL federation verbs (`peer add` / `peer pull` / `peer remove`) — the
residue seed retains cached claims on disk after a `peer remove`, the zero-claims seed subscribes
without pulling. RED classification: both targets COMPILE green, scenarios FAIL via `todo!()` =
MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-09)

Executed **9 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton + parity + anti-merging (01-xx)**: **01-01 is the THICK
  walking skeleton** (PS-1) — the `/peers` route + `Shape::from_request` dispatch + the `PeersView`
  ADT + the read-only `list_active_peer_subscriptions()` seam + the aggregate query + the
  `render_peers_*` parity split + the render-only `render_remove_guidance` + the `ui` wiring. It
  shipped page = chrome + fragment, so **01-02 (parity, PS-2)** and **01-03 (anti-merging per-peer
  count, PS-3)** were confirmatory off the WS structure. **The thick WS drove the whole thread
  green.**
- **Phase 02 — active-only / residue + zero-claims + empty state (02-xx)**: **02-01** the
  **active-only / residue-made-visible** (PS-4 — the `removed_at IS NULL` filter, the residue seed
  that retains cached `peer_claims` after a `peer remove`); **02-02** the **zero-claims LEFT JOIN**
  (PS-5 — `COUNT(pc.cid)` yields 0, fed by the subscribe-without-pull seed); **02-03** the **guided
  empty state** (PS-6 — `NoSubscriptions` + `PEER_ADD_GUIDANCE_PREFIX`); **02-04** **only-removed →
  empty** (PS-7). These carried the honest seeds (real `peer add` / `pull` / `remove` verbs).
- **Phase 03 — gold (03-xx)**: **03-01** the **GOLD read-only / no-write** (no executable
  write/sign/subscribe/remove control on any shape — the revocation is render-only TEXT) +
  **offline**; **03-02** the **N+1-free** invariant (one aggregate query, not one-per-peer). They
  flipped GREEN off the confirmatory render path.

The 9-step shape: a **thorough WS at 01-01** drove the whole thread green for free (page = chrome
+ fragment makes parity structural; the aggregate query makes the per-peer count and N+1-free
structural); the rest were **confirmatory with honest seeds**. The Phase-3 refactor (50ebaa0)
**unified the bare-DID strip** across `render_follow_guidance` (slice-08) + `render_remove_guidance`
(slice-15) into the existing `bare_did` SSOT — removing a second DID-normalization path.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-PS-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..10 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-PS-2 | Mutation = per-feature 100% on the new PURE `viewer-domain` production functions (the `PeersView` projection + `render_peers_*` + `render_remove_guidance`), matching slice-02..10 DV-2. The killing properties are kept IN-CRATE (the `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. 7/7 in-diff viable caught, 0 missed. |
| DV-PS-3 | **ONE aggregate read query** (`peer_subscriptions ps LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did WHERE ps.removed_at IS NULL GROUP BY ... COUNT(pc.cid)`) — NOT one query per peer (ADR-052). | The per-peer claim count is the join+group, not an N+1 fan-out — one round-trip, mutation-/regression-stable, and the N+1-free gold invariant proves it. The LEFT JOIN keeps zero-claims peers; `COUNT(pc.cid)` (the joined column, never the row) yields 0 not 1 for never-pulled peers. |
| DV-PS-4 | **The read filters `removed_at IS NULL` (active-only); a CLI-removed peer is ABSENT from `/peers` even though its cached `peer_claims` remain on disk** (ADR-052). | This absence IS the J-003c "revocable without residue" promise rendered — the view does not delete the cached claims (the CLI's job), it makes the *subscription* state authoritative on screen. The residue seed (retain cached claims after `peer remove`) proves the absence is intentional, not a data loss. |
| DV-PS-5 | **The claim count is grouped PER `peer_did` — never a network total, no "all peers" row** (J-003a, carry-forward of the anti-merging cardinal). | A network total or "all peers" row is exactly where per-peer attribution would collapse into a consensus figure; grouping per `peer_did` keeps each peer's local count as its own attributed cell (PS-3 anti-merging). |
| DV-PS-6 | **The revocation is RENDER-ONLY command TEXT (`openlore peer remove <did>`), reusing slice-08's `render_follow_guidance` pattern — NOT an executable control** (ADR-052). | The viewer is read-only and holds no key; rendering the revocation as copyable command TEXT tells the operator how to walk a subscription back without giving the web surface a mutation control. The render-only / no-write gold invariant proves no executable control exists on any shape. |
| DV-PS-7 | **Read-only enforced at three layers** (a `StoreReadPort` with no mutation method [TYPE] + the `xtask check-arch` viewer capability rule [STRUCTURAL, UNCHANGED] + the gold read-only / no-write invariant [BEHAVIORAL]). | The read-only guarantee cannot be defeated by any single-layer slip. Unlike slice-10, NO new pure-core edge was added — `render` is a total fn of the flat `PeerSubscriptionSummary` DTO — so `check-arch` stays UNCHANGED (no new reachability). |
| DV-PS-8 | **The `GET /peers` handler degrades to `NoSubscriptions` on read error (no 5xx)** (ADR-052). | A federation-management view that 500s on a transient read error is worse than an honest empty state; degrading to `NoSubscriptions` keeps the surface resilient and the operator oriented. |
| DV-PS-9 | **The bare-DID strip was unified across `render_follow_guidance` + `render_remove_guidance` into the existing `bare_did` SSOT** (Phase-3 refactor, 50ebaa0). | Two render-only-command renderers normalizing DIDs two ways is a divergence waiting to bite; collapsing both onto the existing `bare_did` SSOT keeps DID normalization in ONE place. |

## Cardinal release gates + slice-15 invariants (I-PS-1..n)

The cardinal release gates realized on the peers surface — all release-blocking:

1. **Read-only / no key (CARDINAL, I-PS-1)** — `/peers` is a READ; no write/sign/subscribe/remove
   EXECUTABLE route; the web process holds no signing key; the read seam has NO mutation method
   (type-level); the revocation is render-only command TEXT (mirroring slice-08's
   `render_follow_guidance`), no executable control. Three-layer: TYPE (no write method) +
   STRUCTURAL (`xtask check-arch` viewer capability rule, UNCHANGED) + BEHAVIORAL (gold read-only /
   no-write).
2. **Active-only / residue-made-visible (CARDINAL, I-PS-2)** — the read filters `removed_at IS
   NULL`; a CLI-removed peer is ABSENT from `/peers` even though its cached `peer_claims` remain on
   disk — that absence IS the J-003c residue-free promise rendered (PS-4 + the residue seed).
3. **Per-peer not-merged (CARDINAL, I-PS-3, J-003a)** — the claim count is grouped per `peer_did`,
   never a network total, no "all peers" row (PS-3 anti-merging).
4. **Zero-claims kept (I-PS-4)** — the LEFT JOIN keeps never-pulled (subscribed-without-pulling)
   peers; `COUNT(pc.cid)` yields 0 (not 1) for them (PS-5 + the subscribe-without-pull seed).
5. **Offline / local-only (I-PS-5)** — the survey reads the LOCAL store with no network (fully
   offline); the page references only the vendored local htmx asset (no CDN); loopback-only bind;
   nothing persisted (the offline gold).
6. **N+1-free (I-PS-6)** — the active-subscription list is ONE aggregate query (the LEFT JOIN +
   GROUP BY), not one query per peer (the N+1-free gold).
7. **Fragment/page parity (I-PS-7)** — full page (chrome + fragment) without `HX-Request`, the same
   peers fragment with it; page = chrome + fragment by construction (PS-2).
8. **Guided empty state (I-PS-8)** — when there are no active subscriptions (none, or all removed),
   `/peers` renders the guided `NoSubscriptions` state pointing to `openlore peer add`
   (`PEER_ADD_GUIDANCE_PREFIX`) (PS-6, PS-7).

| # | Invariant | Enforcement |
|---|---|---|
| I-PS-1 | Read-only / no key (`/peers` is a READ; no executable write/sign/subscribe/remove route; no key in the process; the read seam holds no mutation method; the revocation is render-only command TEXT). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule, UNCHANGED) + BEHAVIORAL (gold read-only / no-write, DV-PS-6/7). Cardinal. |
| I-PS-2 | Active-only / residue-made-visible (the read filters `removed_at IS NULL`; a CLI-removed peer is ABSENT from `/peers` while its cached `peer_claims` remain on disk — the J-003c promise rendered as absence). | STRUCTURAL (the `WHERE ps.removed_at IS NULL` filter, DV-PS-4) + BEHAVIORAL (PS-4 + the residue seed that retains cached claims). Cardinal. |
| I-PS-3 | Per-peer not-merged (the claim count grouped per `peer_did`; never a network total; no "all peers" row). | STRUCTURAL (`GROUP BY ps.peer_did`, no network total, DV-PS-5) + BEHAVIORAL (PS-3 anti-merging). Cardinal (J-003a). |
| I-PS-4 | Zero-claims kept (never-pulled peers render with count 0, not 1; the LEFT JOIN keeps them). | STRUCTURAL (LEFT JOIN + `COUNT(pc.cid)` on the joined column, DV-PS-3) + BEHAVIORAL (PS-5 + the subscribe-without-pull seed). |
| I-PS-5 | Offline / local-only (the survey reads the LOCAL store with no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the read-only local aggregate query; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (offline gold + read-only row-count delta). |
| I-PS-6 | N+1-free (the active-subscription list is ONE aggregate query, not one-per-peer). | STRUCTURAL (the single LEFT JOIN + GROUP BY query, DV-PS-3) + BEHAVIORAL (N+1-free gold). |
| I-PS-7 | Fragment/page parity (full page without `HX-Request`, the same peers fragment with it; page = chrome + fragment). | STRUCTURAL (the page renderer embeds the peers fragment) + BEHAVIORAL (PS-2 parity). |
| I-PS-8 | Guided empty state (no active subscriptions — none or all removed — renders the guided `NoSubscriptions` pointing to `openlore peer add`). | STRUCTURAL (`NoSubscriptions` arm + `PEER_ADD_GUIDANCE_PREFIX`) + BEHAVIORAL (PS-6, PS-7). |

All slice-15 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets (read-only /
no key / human gate / offline + loopback / progressive enhancement / structural fragment/page
parity); the per-peer count is shown verbatim in both shapes.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_peer_subscriptions` corpus (PS-1..PS-7, the thick
  walking skeleton at PS-1) + the GOLD `viewer_peer_subscriptions_invariants` (read-only /
  no-write, offline, N+1-free) GREEN + the `viewer-domain` unit/property tests (the new `PeersView`
  projection + `render_peers_*` parity + `render_remove_guidance`); slices 06/08/10 + slice-03
  corpora GREEN — zero regression. The `ViewerServer` harness drives the REAL `openlore ui` over
  HTTP; the store is seeded through the REAL `peer add` / `peer pull` / `peer remove` verbs.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, **no new allowlist
  edge** (unlike slice-10): `render` is a total fn of the flat `PeerSubscriptionSummary` DTO, so the
  viewer adds NO `viewer-domain → claim-domain` reachout. The anti-merging SQL rule stays green by
  construction (the aggregate query names `peer_subscriptions` + `peer_claims`, never the standalone
  `claims` table). The viewer capability rule is unchanged (read-only peer-subscription read; no
  signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; the Phase-3 refactor (50ebaa0) **unified the
  bare-DID strip** across `render_follow_guidance` (slice-08) + `render_remove_guidance` (slice-15)
  into the existing `bare_did` SSOT; `viewer-domain` purity intact (no I/O imports; maud + ports
  only; the `Shape` dispatch lives in the effect shell; NO `claim-domain` reachout).
- **Adversarial review**: **APPROVED**, **0 defects, zero Testing Theater**. The active-only /
  residue confirmed structural (the `removed_at IS NULL` filter + the residue seed, DV-PS-4); the
  per-peer-not-merged confirmed structural (`GROUP BY ps.peer_did`, DV-PS-5); the read-only /
  render-only-command confirmed load-bearing (no executable control + the render-only TEXT pattern,
  DV-PS-6/7); the N+1-free confirmed structural (one aggregate query, DV-PS-3).
- **DES integrity**: PASS — all 9 steps have complete DES traces (9/9).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the `PeersView` projection + the
`render_peers_fragment` / `render_peers_page` parity split + `render_remove_guidance` + the
unified `bare_did` strip). The slice-04/05 cross-package lesson stays applied — the `viewer-domain`
unit/property tests pin the production functions IN/against the crate, so the per-feature mutation
measurement reaches the real killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`PeersView` projection + `render_peers_*` parity + `render_remove_guidance`, in-diff) | 7 | 7 | 0 | **100%** (7/7 in-diff viable) |

Slice-15 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0 missed).
`adapter-http-viewer` + `adapter-duckdb` are NOT mutated by design (effect shell; covered by the
GOLD invariants through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A thorough walking skeleton drove the whole thread green for free**: the 01-01 WS shipped
  page = chrome + fragment AND the single aggregate LEFT JOIN query on day one, so parity (PS-2),
  per-peer-not-merged (PS-3), and N+1-free became structural, and the rest of the slice (active-only,
  zero-claims, empty state, gold) was confirmatory with honest seeds. **Lesson: when a slice's
  cardinals are SQL-shape decisions (active-only filter, per-peer GROUP BY, single-query N+1-free),
  get the query right inside the walking skeleton and most downstream scenarios become confirmation
  of the structure rather than new work.**
- **The active-only filter is the J-003c promise made VISIBLE — prove it with a residue seed
  (DV-PS-4)**: filtering `removed_at IS NULL` means a CLI-removed peer disappears from `/peers` while
  its cached claims survive on disk; that absence IS "revocable without residue." The honest test is
  a seed that performs a REAL `peer remove` and RETAINS the cached claims, then asserts the peer is
  ABSENT from the view. **Lesson: when a view's value is an ABSENCE (a removed thing not showing up),
  seed the real removal AND keep the residue, so the test proves the absence is the designed
  promise, not accidental data loss.**
- **`COUNT(pc.cid)` over `COUNT(*)` is the difference between 0 and 1 for never-pulled peers
  (DV-PS-3)**: the LEFT JOIN keeps subscribed-without-pulling peers, but `COUNT(*)` would count the
  one all-NULL joined row as 1; `COUNT(pc.cid)` counts the joined column and yields 0. **Lesson: in a
  LEFT JOIN aggregate, count the JOINED column, never the row, when "zero matches" must read as 0 —
  and pin it with a subscribe-without-pull seed so the off-by-one can't regress silently.**
- **Render-only command TEXT keeps a read-only surface honest — reuse the slice-08 pattern
  (DV-PS-6)**: `/peers` tells the operator how to revoke (`openlore peer remove <did>`) without
  becoming a mutation control by rendering the command as copyable TEXT (the slice-08
  `render_follow_guidance` pattern), not a button. **Lesson: a read-only view can still GUIDE the
  operator toward a mutation it cannot itself perform — render the command as text, keep the
  executable control on the keyed CLI, and prove no executable control leaked with a no-write gold.**
- **No new pure-core edge is a feature, not a gap (vs slice-10)**: slice-10 added a `viewer-domain →
  claim-domain` edge to reuse the bucket; slice-15 keeps `render` a total function of the flat
  `PeerSubscriptionSummary` DTO, so `check-arch` is UNCHANGED and the viewer's reachability does not
  widen. **Lesson: when the projection's inputs can be flattened into a DTO at the port boundary,
  prefer the flat DTO over a new pure→pure edge — it keeps the dependency graph (and the
  architectural test surface) smaller for free.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-052 fixed the contracts; field-level shaping (`PeersView` arms, the `PeerSubscriptionSummary` DTO shape, the aggregate query, the `render_peers_*` parity split) left to DELIVER. | All adopted; the `PeersView` arms (`Subscriptions{peers}` / `NoSubscriptions`), the flat DTO, the single LEFT JOIN + GROUP BY query, and the parity split materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-052 fixed the single-aggregate-query read boundary (LEFT JOIN, `COUNT(pc.cid)`, `removed_at IS NULL`). | Shipped exactly — one query, zero-claims kept at 0, active-only filter, N+1-free gold green. | Resolved at DELIVER. |
| 3 | ADR-052 fixed the render-only-command revocation intent (reuse slice-08 `render_follow_guidance`). | `render_remove_guidance` + `PEER_REMOVE_GUIDANCE_PREFIX` / `PEER_ADD_GUIDANCE_PREFIX` landed; no executable control (no-write gold green). | Resolved at DELIVER. |
| 4 | ADR-052 fixed "no new pure-core edge; check-arch unchanged." | `render` is a total fn of the flat DTO; `check-arch` reports 21 members, no new allowlist edge. | Resolved at DELIVER. |
| 5 | The bare-DID strip was expected per-renderer. | Phase-3 refactor (50ebaa0) UNIFIED the strip across `render_follow_guidance` + `render_remove_guidance` into the existing `bare_did` SSOT. | Improved at DELIVER (refactor); DID normalization in one place (DV-PS-9). |
| 6 | Review expected to pass clean. | Review APPROVED, 0 defects, zero Testing Theater. | Confirmed at DELIVER. |
| 7 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-PS-2, 100% in-diff 7/7, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-peer-subscriptions/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer this slice extends):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-08 archive** (the render-only-command pattern `render_follow_guidance` this slice
  reuses): `docs/evolution/viewer-network-search-evolution.md`
- **Parent slice-10 archive** (the net-new-route shape this slice mirrors):
  `docs/evolution/viewer-graph-traversal-evolution.md`
- **Slice-15 ADR**:
  `docs/adrs/ADR-052-viewer-peer-subscriptions-read-view-one-aggregate-query-render-only-revocation.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-peer-subscriptions/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-peer-subscriptions/deliver/execution-log.json`,
  `docs/feature/viewer-peer-subscriptions/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_peer_subscriptions.rs` (PS-1..PS-7, the thick walking skeleton at PS-1),
  `tests/acceptance/viewer_peer_subscriptions_invariants.rs` (the gold invariants — read-only /
  no-write, offline, N+1-free)
- **Reused render-only-command pattern**: `crates/viewer-domain` (`render_follow_guidance`,
  slice-08) — `render_remove_guidance` follows it; both share the unified `bare_did` SSOT
- **Extended viewer crates**: `crates/viewer-domain` (`PeersView` + `render_peers_fragment` /
  `render_peers_page` + `render_remove_guidance` + the `PEER_REMOVE_GUIDANCE_PREFIX` /
  `PEER_ADD_GUIDANCE_PREFIX` constants), `crates/adapter-http-viewer` (`GET /peers` handler +
  `Shape` fork + nav link + the degrade-to-`NoSubscriptions`-on-error path),
  `crates/adapter-duckdb` (the read-only `list_active_peer_subscriptions` impl — the single LEFT
  JOIN + GROUP BY aggregate query), `crates/ports` (the read seam + the `PeerSubscriptionSummary`
  DTO)
- **Federation mutation surface (NOT replaced)**: the slice-03 CLI (`openlore peer add` /
  `peer pull` / `peer remove`) — the keyed subscribe/unsubscribe path the read-only viewer points to
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS 5126540 → DESIGN dc14aef → DISTILL 1e71872 → roadmap (post-1e71872) → 01-01 3fa9e49 →
01-02 8715fe3 → 01-03 2b9440d → 02-01 4dd252c → 02-02 4649ffb → 02-03 798e12f → 02-04 872784d →
03-01 3e658cc → 03-02 e1cf9f3 → refactor 50ebaa0.
