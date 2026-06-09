<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-search-follow-state

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a thin DELTA on the read-only `GET /search` view of `openlore ui`)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-SF-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/08)
> JTBD: YES — every non-`@infrastructure` story traces to **J-005c** (`docs/product/jobs.yaml`, sub-job of J-005); no new job created
> Brownfield DELTA on: `openlore-appview-search` (slice-05 — the network index, per-user-neutral), `htmx-scraper-viewer` (slice-06 — read-only viewer foundation: no key, `StoreReadPort`, `page = chrome + fragment`, I-VIEW invariants), `viewer-htmx-swaps` (slice-07 — `Shape::from_request` fork), `viewer-network-search` (slice-08 — the `GET /search` view, `to_indexed_claim` with the hardcoded `NetworkUnfollowed` THIS slice fixes, `render_search_results_fragment`, `render_follow_guidance` / `SEARCH_FOLLOW_GUIDANCE_PREFIX`, the `AuthorRelationship` handling), `viewer-peer-subscriptions` (slice-15 — `StoreReadPort::list_active_peer_subscriptions` + `PeerSubscriptionSummary`, the active-only subscription read REUSED here to resolve relationships)
> Date: 2026-06-09 · Owner: Luna (nw-product-owner)
> Slice: slice-16

This file is the canonical DISCUSS-wave delta for `viewer-search-follow-state` (slice-16):
closing the **discovery→follow** loop on the existing read-only `GET /search` view. Today the
viewer's `to_indexed_claim` (`crates/adapter-http-viewer/src/lib.rs` ~line 1021-1033) hardcodes
`AuthorRelationship::NetworkUnfollowed` for EVERY network-search result author (the comment at
~line 1017 admits "always NetworkUnfollowed"), so the slice-08 render-only `openlore peer add
<did>` follow affordance is offered even for authors the operator ALREADY follows — and a
followed author is never recognized as such. slice-16 RESOLVES each result author's relationship
in the viewer EFFECT SHELL against the operator's LOCAL active peer subscriptions (the slice-15
`list_active_peer_subscriptions` read, REUSED verbatim):

- author DID ∈ active subscriptions (`peer_subscriptions WHERE removed_at IS NULL`) →
  `SubscribedPeer` → render a neutral **"Following"** indicator (NO follow command).
- otherwise → `NetworkUnfollowed` → keep the slice-08 render-only `openlore peer add <did>`
  follow affordance.

This realizes **J-005c "Turn a discovery into a follow (discovery feeds federation)"** —
discovery becomes the front-door that grows the trusted local graph, and a developer you
already follow is shown as such instead of being re-offered a follow.

This is a DELTA. It REUSES the slice-15 active-subscription read + the existing
`AuthorRelationship` enum (NO new variant) + the slice-08 `render_follow_guidance`
render-only-command pattern. It adds NO new route, NO new read method, NO new crate; it threads
an already-existing LOCAL read into an already-existing render. Tier-1 content is inlined here
(lean); SSOT lives under `docs/product/`; per-slice/registry artifacts under `discuss/`.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-005 family; sub-job **J-005c** "Turn a discovery into a follow", `load_bearing: false`, ~line 516; slice-08 changelog ~line 651 — US-NS-004 realized J-005c as render-only guidance text)
- ✓ `crates/ports/src/federated_row.rs` (`AuthorRelationship { You | SubscribedPeer | UnsubscribedCache | NetworkUnfollowed }` ~line 67 — ALREADY EXISTS; the ~line 58 doc comment confirms the relationship is resolved viewer-side against `peer_subscriptions`; the index is per-user-neutral)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (`to_indexed_claim` ~line 1021-1033 — the hardcoded `NetworkUnfollowed` THIS slice fixes; `resolve_search_state` ~line 884 + the `compose_results` call ~line 912-913 — the resolution seam; `store.list_active_peer_subscriptions()` ALREADY CALLED for `/peers` ~line 717)
- ✓ `crates/ports/src/store_read.rs` (`StoreReadPort::list_active_peer_subscriptions` ~line 445 — slice-15, the active-only read REUSED; `PeerSubscriptionSummary.peer_did` is the bare `did:plc:…` ~line 230)
- ✓ `crates/viewer-domain/src/lib.rs` (`render_search_results_fragment` ~line 1745 branches ONLY `@if NetworkUnfollowed → render_follow_guidance` — there is NO `SubscribedPeer` "Following" branch yet, THIS slice adds it; `render_follow_guidance` ~line 1759; `SEARCH_FOLLOW_GUIDANCE_PREFIX` ~line 1513; `bare_did` SSOT ~line 2566)
- ✓ `docs/feature/viewer-network-search/feature-delta.md` (slice-08 — US-NS-004 render-only follow guidance; WD-NS-3 read-only / follow stays CLI; the "Driving Ports for DESIGN" note already flagged "the relationship-label projection (subscribed-peer vs unfollowed) reads the local subscriptions — DESIGN decides whether the viewer surfaces that label" — slice-16 IS that deferred decision, now taken)
- ✓ `docs/feature/viewer-peer-subscriptions/discuss/` (slice-15 — the lean DISCUSS structure mirrored; the active read REUSED)
- ⊘ `docs/feature/viewer-search-follow-state/diverge/` (no DIVERGE wave — consistent with all prior slices; J-005c validated; non-blocking risk R-SF-1)

No DISCUSS decision below contradicts prior-wave evidence: J-005c is validated; the viewer is
read-only (slices 06–15); the follow path stays the slice-03 CLI; the `AuthorRelationship` enum
+ the active read + the render-only-command discipline already exist.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona as
slices 06–15 (`docs/product/personas/senior-engineer-solo-builder.yaml`), wearing the
**network-discovery hat** minted in slice-08. slice-16 does NOT introduce a new scanning
behavior; it sharpens the follow affordance's accuracy on the EXISTING `/search` discovery
surface. No new persona hat is minted (WD-SF-11).

> P-002 (researcher/tech-lead) is the slice-05 CLI discovery persona; the BROWSER viewer is
> P-001's surface (slices 06–15). The follow verb is the slice-03/05 `openlore peer add` CLI;
> `/search` is P-001's read-only browser surface over the network index + her LOCAL follow graph.

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-005c**: *When a network search surfaces a claim by a developer I do not yet follow, I
> want a one-step path to subscribe to them via the slice-03 federation flow, so discovery
> becomes the front-door that grows my trusted local graph rather than a dead-end read.*
> (`docs/product/jobs.yaml`, sub-job of **J-005**, opportunity score 15.)

slice-08 realized J-005c as render-only `peer add` guidance text — but indiscriminately, on
EVERY result. slice-16 makes the front-door ACCURATE: it shows the `peer add` affordance ONLY
for genuinely-unfollowed authors, and a neutral "Following" indicator for authors already in
the trusted local graph. The follow GUARANTEE itself stays the slice-03 CLI; slice-16 sharpens
WHEN the discovery surface invites it.

No new job. No new sub-job. The one user-visible story (US-SF-002) traces to J-005c.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | The user-visible story → J-005c. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-005a (search by dimension) untouched — grouping/ranking unchanged. J-005b (verified-before-index) untouched — the `[verified]` marker is preserved. J-003a (anti-merging) HONORED — relationship is a per-row enrichment; no merge/re-rank. |
| No contradiction with cardinal invariants? | PASS | Read-only / no-key (KPI-VIEW-2) HONORED — both affordances are render-only TEXT; no follow/unfollow control; no key. Index per-user-neutral (slice-05/08) HONORED — resolution reads the LOCAL active set, never tells the index who you follow. Local-first (KPI-5) HONORED — LOCAL read. |
| Follow NOT re-introduced as an executable control on the viewer? | PASS | slice-16 adds ZERO follow/unfollow controls. The follow is render-only `openlore peer add <did>` TEXT (the slice-08 pattern); the "Following" indicator is a neutral render-only label; the viewer holds no key and executes nothing. |
| Accuracy honored (the load-bearing new behavior)? | PASS (the defining AC) | A followed author shows "Following" + NO add command; an unfollowed author keeps the add command. An explicit AC + domain examples (followed, all-followed, none-followed) pin it. |
| Job already fully served? | NO (gap is real) | slice-08 offers `peer add` to EVERY result author (hardcoded `NetworkUnfollowed`), including already-followed peers. There is NO recognition of a followed author on `/search` today. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting thin DELTA on the
existing `/search` view.

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments (inherited, not re-litigated). Full text in `user-stories.md`
§"System Constraints" (C-1..C-9). Summary:

| ID | Commitment | Source |
|---|---|---|
| C-1 (CARDINAL) | Read-only / no key: BOTH the "Following" indicator AND the `peer add` affordance stay render-only TEXT; no follow/unfollow control; no key. The follow stays the slice-03 CLI. | KPI-VIEW-2, slice-06–15, slice-08 WD-NS-3 |
| C-2 | Accuracy (load-bearing): a followed author shows "Following" + no add; an unfollowed author keeps `peer add`. | J-005c, WD-SF-2/5 |
| C-3 | LOCAL / offline relationship resolution; the index stays per-user-neutral. | KPI-5, slice-05/08 boundary, slice-15 WD-PS-4 |
| C-4 | ONE batch read of the active set per render (no N+1). | WD-SF-3, slice-15 I-PS-3/4 |
| C-5 | Attribution + ranking UNCHANGED (relationship is a per-row enrichment). | J-003a, slice-08 I-NS-3 |
| C-6 | Binary resolution; `You`/`UnsubscribedCache` not resolved on `/search`. | WD-SF-2/5 |
| C-7 | Graceful degradation of the active-set read (degrade to slice-08 status quo). | WD-SF-6, slice-08 I-NS-2 |
| C-8 | Progressive enhancement + parity (same fragment both shapes). | slice-07/08 WD-NS-6 |
| C-9 | No new crates / route / variant / persisted type; loopback-only bind; workspace 21. | slice-06–15 |

---

## Wave: DISCUSS / [REF] Proposed change (no new route, no new read method)

- **Route**: `GET /search` — UNCHANGED (the slice-08 route). slice-16 adds no new route.
- **Read**: REUSES the slice-15 `StoreReadPort::list_active_peer_subscriptions` — UNCHANGED
  (no new read method, no new SQL, no `adapter-duckdb` change). The shell calls it ONCE per
  `/search` render to get the active-subscription set, materialized into an in-memory bare-DID
  set.
- **Resolution (NEW, in the EFFECT shell `adapter-http-viewer`)**: in `resolve_search_state`,
  after the indexer query + BEFORE `compose_results`, read the active set ONCE and resolve each
  result author's `author_did` (fragment-stripped via `bare_did`): ∈ set → `SubscribedPeer`,
  else → `NetworkUnfollowed`. Thread the resolved relationship into `to_indexed_claim` (which
  stops hardcoding `NetworkUnfollowed`). A failed read degrades to all-`NetworkUnfollowed`.
- **Render (NEW arm, in the PURE `viewer-domain`)**: extend `render_search_results_fragment`
  with a `SubscribedPeer → "Following"` neutral render-only indicator arm (the sibling of the
  existing `NetworkUnfollowed → render_follow_guidance` arm, which is unchanged).

> DESIGN owns: the resolution fn shape (param vs closure), the in-memory set type, the
> "Following" indicator markup + copy, and whether the render is a total `match` over the 4
> `AuthorRelationship` variants. The PRODUCT contract is the AC in `user-stories.md`.

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-005c, with boundaries)

| Story | Title | job_id | Boundary note |
|---|---|---|---|
| US-SF-001 | Resolve each `/search` result author's relationship against the LOCAL active subscriptions (REUSE slice-15 read; replace the hardcoded `NetworkUnfollowed`) | `infrastructure-only` | `infrastructure_rationale` below. Enables US-SF-002. READ-ONLY by construction. |
| US-SF-002 | On `/search`, show "Following" for a followed author; keep `peer add` only for unfollowed | J-005c | The accuracy fix to the slice-08 discovery→follow guidance. NOT the follow itself (slice-03 CLI). |

### Infrastructure rationale (US-SF-001)

US-SF-001 carries `job_id: infrastructure-only` with this rationale: it replaces the hardcoded
`AuthorRelationship::NetworkUnfollowed` in `to_indexed_claim` with a resolution against the
operator's LOCAL active-subscription set (REUSING the slice-15 `list_active_peer_subscriptions`
read — no new read method, no new SQL), reading the set ONCE per render and resolving each
result author in memory. It produces no NEW user-visible output on its own (the "Following"
indicator vs the `peer add` affordance is rendered by US-SF-002), so it enables a user decision
only THROUGH that story. The slice contains ONE non-infrastructure, user-visible story
(US-SF-002), so the slice has release value (Dimension-0 slice-level check passes). READ-ONLY:
reads `StoreReadPort` + `IndexQueryPort`, neither with a mutation method; holds no key.

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey-visualization investment (Phase 1.5). Thinner than slice-15: NO new route,
NO new read method, NO new variant, NO new crate.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 2 (1 infra + 1 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer `/search` surface) extending `adapter-http-viewer` (effect: thread the active set into resolution) + `viewer-domain` (pure: add the `SubscribedPeer` "Following" render arm). NO `ports` change, NO `adapter-duckdb` change, NO new crate. | No (single context) |
| Walking-skeleton integration points | 3: (1) READ the slice-15 active set in `resolve_search_state` (REUSE), (2) RESOLVE each author against the set in `to_indexed_claim` (replace hardcoded `NetworkUnfollowed`), (3) RENDER the `SubscribedPeer` "Following" arm in `render_search_results_fragment` (new pure arm). | No (≤5) |
| Estimated effort | ~0.5–1 day (one read thread + one resolution fn + one render arm; everything else REUSED) | No (≤2 weeks) |
| Independent user outcomes | 1 (a followed author shown "Following" + not re-offered a follow; an unfollowed author keeps the affordance) | No |

**## Scope Assessment: PASS — 2 stories (1 infra + 1 user-visible), 1 context, 3 integration points (1 reused read + 1 resolution + 1 new render arm), estimated ~0.5–1 day. No new route, no new read method, no new `AuthorRelationship` variant, no new crate; workspace stays 21.**

The thing that would make it oversized — a follow/unfollow control in the viewer, a per-result
subscription query (N+1), an `UnsubscribedCache` path, or `You` own-DID resolution requiring an
identity surface — is explicitly OUT of scope (WD-SF-1/2/3/5). The infra read-thread (US-SF-001)
and the render (US-SF-002) are already separated, so no further split is needed.

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-SF-001 | Resolve each `/search` result author's relationship against the LOCAL active subscriptions (REUSE slice-15 read; ONE batch read; fragment-strip match; replace the hardcoded `NetworkUnfollowed`; degrade gracefully) | infrastructure-only |
| US-SF-002 | On `/search`, show a neutral "Following" indicator for an already-followed author (no add command) and keep the render-only `openlore peer add <did>` affordance only for genuinely-unfollowed authors | J-005c |

---

## Wave: DISCUSS / [REF] You-scope decision (WD-SF-2)

**`You` (own-DID) resolution is DEFERRED.** slice-16 resolves ONLY `SubscribedPeer` vs
`NetworkUnfollowed`. A result whose author is the operator themselves resolves to
`NetworkUnfollowed` (re-offered a `peer add` it would never run). Rationale: the `/search`
corpus is the NETWORK index (per-user-neutral); a network row carries no `SourceTable`/own
marker, and the read-only network-search surface does not cheaply hold the operator's OWN DID
(there is no identity surface in the read-only viewer; adding one would blur the key-less
boundary). The cost of deferral is small (a self-follow affordance the operator simply ignores).
Revisit if/when the viewer cheaply holds the operator DID (e.g. a future `/me` surface).

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-16 mints **NO new KPI ID**. Like slice-08–15 it REALIZES inherited KPIs on a new facet
(the `/search` follow-state accuracy). Full detail in `outcome-kpis.md`. Relevant inherited
KPIs:

- **KPI-AV-4** (discovery→federation funnel — the J-005c north-star): slice-16 STRENGTHENS its
  accuracy — the `peer add` affordance is shown ONLY where it is actionable (0% re-offered to
  already-followed authors).
- **KPI-VIEW-2** (read-only, guardrail): MET — no follow/unfollow control, no key.
- **KPI-AV-2 / KPI-FED-1/2** (anti-merging, guardrails): MET — relationship is a per-row
  enrichment; grouping + order unchanged.
- **KPI-5 / index per-user-neutral** (local-first / index neutrality, guardrails): MET —
  resolution is a LOCAL read; the index query is unchanged + per-user-neutral.
- **KPI-HX-G1/G2/G3** (no-JS / offline / no-CDN, guardrails): MET — LOCAL resolution; full page
  without HX-Request; same fragment both shapes; vendored htmx.

> Detail rationale in `outcome-kpis.md`. Cross-feature SSOT: `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the `/search`
view + `to_indexed_claim` + `render_search_results_fragment` + `render_follow_guidance`
(slice-08), the `AuthorRelationship` enum (slice-03/05), and the active-subscription read
(`list_active_peer_subscriptions`, slice-15) all already exist. The thinnest end-to-end slice IS
US-SF-002 (the accurate "Following" vs `peer add` render), backed by US-SF-001 (the resolution).
Delivery sequence: US-SF-001 → US-SF-002. Each is demonstrable in a single session against the
real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Requirements (functional + NFR + business rules): `requirements.md`
- User stories (combined, `## System Constraints` at top): `user-stories.md`
- Acceptance criteria (BDD, by theme): `acceptance-criteria.md`
- Outcome KPIs: `outcome-kpis.md`
- Definition of Ready: `dor-checklist.md`
- Wave decisions (WD-SF-*): `wave-decisions.md`

> Lean mode: the standalone journey-visual + journey-yaml + shared-artifacts-registry are NOT
> produced for this thin DELTA (mirroring the slice-08/15 lean set). The journey is the slice-08
> `discover-the-network-from-the-browser` arc (the follow-guidance step on `/search`), grounded
> verbatim; the single shared artifact (`author_did` ↔ `peer_subscriptions.peer_did`, matched
> via the `bare_did` SSOT) is already tracked across slices 08/15.

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)** for both stories.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-16 | Low | Low | J-005c validated in `docs/product/jobs.yaml`; the journey is the slice-08 `/search` discovery arc; the slice-08 "Driving Ports for DESIGN" note already named this deferred relationship-label projection. Non-blocking. |
| Resolution becomes N+1 (one subscription query per result author) | Medium | High | C-4 + US-SF-001 AC make the single-batch-read-per-render a HARD commitment (REUSE the slice-15 single-aggregate-query read; resolve in memory). A behavioral test asserts the active-set read count invariant to result count; a per-result query is REJECTED. |
| An already-followed author still re-offered `peer add` (core bug unfixed) | Medium | High | C-1/-3/-5 + US-SF-002 AC: a seeded active subscription → that row shows "Following" + no add; a behavioral test pins it. The load-bearing AC. |
| A genuinely-unfollowed author loses the affordance (over-correction) | Low | High | US-SF-002 AC + the none-followed domain example: an unfollowed author keeps the `peer add` command; binary resolution (not-in-set → `NetworkUnfollowed`). A behavioral test asserts the unfollowed row keeps the command. |
| `#fragment` mismatch misclassifies a followed author | Medium | Medium | C-4 + US-SF-001 AC: strip the fragment via the existing `bare_did` SSOT on both sides before set membership. A domain example with a fragmented result DID vs a bare active DID pins the match. |
| Resolution accidentally re-ranks/merges results | Low | High | C-5 + US-SF-002 AC: resolution sets the per-row `relationship` ONLY; `compose_results` grouping + order unchanged. A behavioral test asserts identical grouping/order with-and-without an active subscription. |

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-16 does NOT, under any circumstance: add a follow/unfollow control to the viewer; hold a
signing key; resolve `You` (own-DID, deferred) or `UnsubscribedCache` on `/search`; add a
network seam for resolution (the index stays per-user-neutral); re-group / re-rank / merge
results; add a new route, read method, `AuthorRelationship` variant, or crate (workspace stays
21); persist anything; bind anything but 127.0.0.1; or issue one subscription query per result
author (N+1). Full list in `user-stories.md` §"Out of scope".

---

## Changelog

- 2026-06-09 — Luna — slice-16 (`viewer-search-follow-state`) DISCUSS. Traces to J-005c (turn a
  discovery into a follow). 2 stories (1 infra + 1 user-visible). EXTENDS the existing read-only
  `GET /search` view (NO new route). REUSES the slice-15 active-subscription read
  (`list_active_peer_subscriptions`) + the existing `AuthorRelationship` enum (NO new variant) +
  the slice-08 `render_follow_guidance` render-only-command pattern. Resolves each result
  author's relationship in the viewer EFFECT shell against the LOCAL active set; renders a
  neutral "Following" indicator for `SubscribedPeer` (NO add command) and keeps the slice-08
  `openlore peer add <did>` affordance for `NetworkUnfollowed`. CARDINAL decisions: read-only /
  no-key (WD-SF-1); accuracy (WD-SF-2/5); LOCAL/offline resolution (WD-SF-4); ONE batch read /
  no N+1 (WD-SF-3). `You`-scope DEFERRED (WD-SF-2). NO new crate (workspace stays 21), no new
  KPI ID. Scope PASS (~0.5–1 day). DoR PASS (9/9).
