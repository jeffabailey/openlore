# Wave Decisions: viewer-search-follow-state (slice-16) — DISCUSS

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-09
> Feature type: User-facing · JTBD: YES (J-005c) · UX depth: Lightweight · Walking skeleton: brownfield DELTA (thin)
> Brownfield DELTA on slices 05 (appview-search) / 06 (htmx-scraper-viewer) / 07 (viewer-htmx-swaps) / 08 (viewer-network-search) / 15 (viewer-peer-subscriptions).

This slice closes the discovery→follow loop on the existing read-only **`GET /search`**
view (slice-08). Today the viewer's `to_indexed_claim` (`crates/adapter-http-viewer/src/lib.rs`
~line 1021-1034) hardcodes `AuthorRelationship::NetworkUnfollowed` for EVERY network-search
result author — so the slice-08 render-only `openlore peer add <did>` follow affordance is
offered even for authors the operator ALREADY follows, and a followed author is never
recognized as such. This slice RESOLVES each search-result author's relationship in the
viewer EFFECT SHELL against the operator's LOCAL active peer subscriptions (the slice-15
`StoreReadPort::list_active_peer_subscriptions` read, REUSED verbatim):

- author DID ∈ active subscriptions (`peer_subscriptions WHERE removed_at IS NULL`) →
  `SubscribedPeer` → render a neutral **"Following"** indicator, NO follow command.
- otherwise → `NetworkUnfollowed` → keep the slice-08 render-only `openlore peer add <did>`
  follow affordance.

`You` (own-DID) resolution is **deferred** (scope decision WD-SF-2 below). This realizes
**J-005c "Turn a discovery into a follow (discovery feeds federation)"**: discovery becomes
the front-door that grows the trusted local graph, and a developer you already follow is
shown as such instead of being re-offered a follow.

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-005 family; **J-005c** "Turn a discovery into a follow",
  `load_bearing: false`, at ~line 516; the slice-08 changelog at ~line 651 — US-NS-004
  realized J-005c as render-only guidance text; J-005a/J-005b for boundary)
- ✓ `crates/ports/src/federated_row.rs` (`AuthorRelationship { You | SubscribedPeer |
  UnsubscribedCache | NetworkUnfollowed }` at ~line 67 — ALREADY EXISTS; the ~line 58 doc
  comment confirms the relationship is resolved CLI-side / viewer-side against
  `peer_subscriptions`; the index itself is per-user-neutral)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (`to_indexed_claim` ~line 1021-1033 — the
  hardcoded `NetworkUnfollowed` THIS slice fixes, with the comment admitting the viewer is
  "per-user-neutral … always NetworkUnfollowed"; `resolve_search_state` ~line 884 + the
  `compose_results` call ~line 912-913 — the seam where the active set is read + threaded;
  `store.list_active_peer_subscriptions()` ALREADY CALLED for `/peers` ~line 717)
- ✓ `crates/ports/src/store_read.rs` (`StoreReadPort::list_active_peer_subscriptions(&self)
  -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>` ~line 445 — slice-15, the
  active-only read REUSED; `PeerSubscriptionSummary.peer_did` is the bare `did:plc:…`,
  non-Option, ~line 230)
- ✓ `crates/viewer-domain/src/lib.rs` (`render_search_results_fragment` ~line 1745: the
  render today branches ONLY `@if matches!(row.relationship, NetworkUnfollowed) →
  render_follow_guidance` — there is NO `SubscribedPeer` "Following" branch yet, THIS slice
  adds it; `SEARCH_FOLLOW_GUIDANCE_PREFIX` ~line 1513; `render_follow_guidance` ~line 1759;
  `bare_did` SSOT ~line 2566 for the DID comparison)
- ✓ `docs/feature/viewer-network-search/feature-delta.md` (slice-08 — US-NS-004 render-only
  follow guidance; WD-NS-3 read-only / follow stays CLI / view may DISPLAY never execute;
  the "Driving Ports for DESIGN" note already flagged "the relationship-label projection
  (subscribed-peer vs unfollowed) reads the local subscriptions — DESIGN decides whether the
  viewer surfaces that label" — slice-16 IS that deferred decision, now taken)
- ✓ `docs/feature/viewer-peer-subscriptions/discuss/` (slice-15 — the lean DISCUSS structure
  mirrored; `list_active_peer_subscriptions` + `PeerSubscriptionSummary` REUSED as the
  active-set read)
- ⊘ `docs/feature/viewer-search-follow-state/diverge/` (no DIVERGE wave — consistent with all
  prior OpenLore slices; J-005c is validated; noted as a non-blocking risk R-SF-1)

No DISCUSS decision below contradicts the prior-wave evidence: J-005c is validated; the
viewer is read-only (slices 06–15); the follow path stays the slice-03 CLI; the
`AuthorRelationship` enum + the render-only-command discipline already exist.

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`,
`journeys/`). Proceeded without re-running JTBD; J-005c is validated (slice-05 DISCUSS,
changelog 2026-05-28; carried into slice-08, changelog 2026-06-04).

## Scope Assessment: PASS — 2 stories (1 infra + 1 user-visible), 1 bounded context (the viewer's `/search` surface), 3 integration points, estimated ~0.5–1 day

Carpaccio gate, run BEFORE journey-visualization investment (Phase 1.5). Thinner than
slice-15: NO new route, NO new read method, NO new variant, NO new crate — a thin DELTA that
THREADS an already-existing read into an already-existing render.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 2 (1 infra + 1 user-visible) | No (<10) |
| Bounded contexts / modules | 1 — the viewer `/search` surface. Extends `adapter-http-viewer` (effect: thread the active set into `to_indexed_claim`/`resolve_search_state`) + `viewer-domain` (pure: add the `SubscribedPeer` "Following" render branch). NO `ports` change (`list_active_peer_subscriptions` + `AuthorRelationship` already exist); NO `adapter-duckdb` change (the read already exists). NO new crate. | No (single context) |
| Walking-skeleton integration points | 3: (1) READ the slice-15 active-subscription set in `resolve_search_state` (REUSE), (2) RESOLVE each result author's `author_did` against the active-DID set in `to_indexed_claim` (replace the hardcoded `NetworkUnfollowed`), (3) RENDER the `SubscribedPeer` "Following" indicator branch in `render_search_results_fragment` (new pure arm). Within ≤5. | No (≤5) |
| Estimated effort | ~0.5–1 day (one read thread + one resolution fn + one render arm; everything else REUSED) | No (≤2 weeks) |
| Independent user outcomes | 1 (an already-followed author is shown as "Following" and NOT re-offered a follow; a genuinely-unfollowed author keeps the affordance) | No |

**Verdict: RIGHT-SIZED.** The thing that would make it oversized — a follow/unfollow control
in the viewer, a per-result subscription query (N+1), an `UnsubscribedCache` resolution path,
or `You` own-DID resolution requiring an identity surface in the viewer — is explicitly OUT of
scope (WD-SF-1 read-only, WD-SF-3 one batch read, WD-SF-5 no `UnsubscribedCache`, WD-SF-2
`You` deferred). If DESIGN finds the resolution + render exceed 1 day, the infra read-thread
(US-SF-001) and the render (US-SF-002) are already separated — no further split needed.

## Locked decisions (WD-SF-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-SF-1 (CARDINAL) | **READ-ONLY / no key (inherited I-VIEW / WD-NS-3)**: BOTH the "Following" indicator AND the `peer add` affordance stay render-only TEXT. The viewer holds no key, mutates nothing, exposes no follow/unfollow control. The follow path is still the slice-03 CLI. slice-16 changes only WHICH render-only affordance a row shows (a neutral "Following" label vs the `peer add` guidance) — it adds NO executable control. | The read-only / key-less viewer is cardinal across slices 06–15. Resolving a relationship against a LOCAL read is a READ; it adds no write surface. Load-bearing read-only boundary (KPI-VIEW-2). | LOCKED |
| WD-SF-2 (SCOPE) | **`You` (own-DID) resolution is DEFERRED** — slice-16 resolves ONLY `SubscribedPeer` vs `NetworkUnfollowed`. A search result whose author is the operator themselves is treated as `NetworkUnfollowed` (it keeps the `peer add` guidance) UNLESS it is also an active subscription (it never is — you do not subscribe to yourself). | The `/search` corpus is the NETWORK index (per-user-neutral); a network result row carries no `SourceTable`/own-marker. Resolving `You` requires the viewer to cheaply hold the operator's OWN DID on the read-only network-search surface, which it does not today (no identity surface in the read-only viewer; that would risk the key-less boundary's clarity). Deferring keeps the slice thin and the read-only boundary crisp. The cost of deferral is small: re-offering yourself a `peer add` you would simply never run. Revisit if/when the viewer cheaply holds the operator DID (e.g. a future `/me` surface). | LOCKED |
| WD-SF-3 | **ONE batch read of the active set per search render (no N+1)**: the active-subscription set is read ONCE per `/search` render via the slice-15 `list_active_peer_subscriptions` (which is itself ONE aggregate query), materialized into an in-memory set of bare DIDs, and each result author is resolved against that set in memory. NO per-result subscription query. | Mirrors the slice-15 single-aggregate-query discipline. A per-result `is_subscribed(did)` query would be N+1 across the result set. REUSES the already-shipped slice-15 read (no new SQL, no new read method). | LOCKED |
| WD-SF-4 | **LOCAL / offline relationship resolution**: the relationship is resolved against the LOCAL active-subscription set (a LOCAL DuckDB read), NOT the network. The network index stays per-user-neutral (the slice-05/08 boundary). The `/search` route's network seam is UNCHANGED — it still queries the indexer for the result rows; slice-16 adds ONE LOCAL read alongside it for the relationship resolution. | The index must NOT learn who the operator follows (per-user-neutral, slice-05 KPI-AV boundary). Resolution is the operator's LOCAL business. The LOCAL read degrades independently: if it fails, resolution falls back to `NetworkUnfollowed` (the slice-08 status quo — see WD-SF-6), never a crash. | LOCKED |
| WD-SF-5 | **No `UnsubscribedCache` resolution on `/search`**: slice-16 resolves to exactly two states — `SubscribedPeer` (∈ active set) or `NetworkUnfollowed` (otherwise). A soft-removed peer (`removed_at` set) is NOT in the active set, so it resolves to `NetworkUnfollowed` (keeps the `peer add` affordance) — correct, because the operator does NOT currently follow them. `UnsubscribedCache` is a federated-read (slice-03) cache-residue concept, not a network-discovery relationship. | The slice-15 active read already excludes soft-removed rows (`removed_at IS NULL`). A soft-removed author SHOULD be re-offered a follow on `/search` (they are discoverable and currently-unfollowed). This keeps the resolution binary + matches the existing enum's documented network-search usage (the ~line 58 comment: network search produces relationships resolved against `peer_subscriptions`). | LOCKED |
| WD-SF-6 | **Graceful degradation of the active-set read**: if the LOCAL active-subscription read fails, the relationship resolution degrades to the slice-08 status quo — every author resolves to `NetworkUnfollowed` (the `peer add` affordance shown). The `/search` results still render; no crash, no blank region, no leaked error. | Mirrors the slice-08 `Err(_) → PeersView::NoSubscriptions` graceful-degrade pattern on `/peers` (~line 719). The relationship label is an enrichment; its failure must never break discovery. Worst case is the pre-slice-16 behavior, which was acceptable. | LOCKED |
| WD-SF-7 | **Progressive enhancement + parity (inherited slice-07/08 WD-NS-6)**: the resolved relationship renders identically under the htmx `#search-results` fragment and the no-JS full page (same `render_search_results_fragment` both shapes embed). A swap is a nicety; the no-JS full page is the contract. | The resolution happens in the shell BEFORE the render; both shapes consume the SAME `SearchState`/`IndexedClaim.relationship`. Parity is by construction. | LOCKED |
| WD-SF-8 | **Attribution + ranking UNCHANGED (J-003a / slice-08 I-NS-3)**: relationship resolution does NOT merge, re-group, or re-rank results. Every result stays attributed to its own author (`compose_results` per-author grouping unchanged); the relationship label is a per-row enrichment only. | Anti-merging is cardinal. Resolution reads the relationship and chooses a render-only affordance per row; it touches neither the grouping nor the order. The `[verified]` marker + verbatim confidence + counter-annotation are all unchanged. | LOCKED |
| WD-SF-9 | **Zero new persisted types; no new route; no new variant; no new crate; loopback-only bind.** The active set is read + resolved per-request, never persisted. `AuthorRelationship` (4 variants) + `list_active_peer_subscriptions` already exist; slice-16 USES them. Workspace stays 21 members. | The viewer persists nothing from a read (BR-VIEW-2 / I-VIEW-1/4). The enum + the read already ship; this is a thin wiring + render DELTA. | LOCKED |
| WD-SF-10 | **No new KPI ID**: slice-16 REALIZES inherited KPIs on the `/search` follow-state facet — KPI-AV-4 (the discovery→federation funnel, the J-005c north-star this slice directly serves) + guardrails KPI-VIEW-2 (read-only) / KPI-AV-2 (anti-merging) / KPI-5 / KPI-HX-G1/G2/G3 (no-JS/offline/no-CDN). | Matches slice-08–15 (no new KPI per facet slice). slice-16 strengthens the ACCURACY of the discovery→follow funnel: it stops re-offering a follow to an already-followed author, so the `peer add` affordance is shown ONLY where it is actionable. Detail in `outcome-kpis.md`. | LOCKED |
| WD-SF-11 | **Persona: P-001 (Maria), network-discovery hat (slice-08), no new hat.** The same surface (`/search`) and the same scanning behavior (discover the network) as slice-08; slice-16 sharpens the follow affordance's accuracy. No persona-file change. | The browser viewer is P-001's surface (slices 06–15). slice-16 does not introduce a new scanning behavior — it corrects the affordance on the EXISTING discovery hat. No new hat warranted. | LOCKED |

## Risks logged

### R-SF-1 (RISK) — No DIVERGE wave for slice-16

No `diverge/` directory for this feature — consistent with all prior OpenLore slices.
NON-BLOCKING: J-005c is validated in `docs/product/jobs.yaml`; the journey is the slice-08
`discover-the-network-from-the-browser` arc (the follow-guidance step), grounded verbatim;
the design direction is unambiguous (the slice-08 "Driving Ports for DESIGN" note already
named this deferred relationship-label projection). No JTBD re-run required.

### R-SF-2 (RISK) — Relationship resolution becomes N+1 (one subscription query per result author)

Mitigated by WD-SF-3 + US-SF-001 AC: the active set is read ONCE per render (the slice-15
single-aggregate-query read) into an in-memory bare-DID set; each result author is resolved
in memory. A behavioral test asserts the active-set read count is invariant to the number of
result rows. A per-result `is_subscribed` query is explicitly REJECTED.

### R-SF-3 (RISK) — An already-followed author is still re-offered `peer add` (the core bug not fixed)

Mitigated by WD-SF-1/-3/-5 + US-SF-002 AC: a domain example with a followed author
(`did:plc:rachel-test`, active subscription) appears in results and shows the neutral
"Following" indicator with NO `peer add` command; a behavioral test seeds an active
subscription, runs a search that returns that author, and asserts the row shows "Following"
and NO `peer add` text. This is the load-bearing AC of the slice.

### R-SF-4 (RISK) — A genuinely-unfollowed author loses the follow affordance (over-correction)

Mitigated by US-SF-002 AC: a domain example with an unfollowed author
(`did:plc:priya-test`, NOT in the active set) keeps the slice-08 `openlore peer add
did:plc:priya-test` render-only affordance; a behavioral test asserts the unfollowed row
still shows the `peer add` command. The resolution is binary (WD-SF-5): not-in-active-set →
`NetworkUnfollowed` → affordance retained.

### R-SF-5 (RISK) — The `#fragment` mismatch breaks DID comparison (followed author misclassified)

The active set stores BARE DIDs (`PeerSubscriptionSummary.peer_did` is bare); a search
result's `author_did` carries the `#org.openlore.application` signing fragment. Mitigated by
WD-SF-3 + US-SF-001 AC: the comparison strips the fragment via the existing `bare_did` SSOT
(`crates/viewer-domain/src/lib.rs` ~line 2566) on BOTH sides before set membership. A domain
example with a fragmented result DID vs a bare active-set DID pins the byte-equal match.

### R-SF-6 (RISK) — Relationship resolution accidentally re-ranks or merges results

Mitigated by WD-SF-8 + US-SF-002 AC: the resolution sets the per-row `relationship` field
ONLY; the `compose_results` per-author grouping + order is UNCHANGED. A behavioral test
asserts the result grouping/order with-and-without an active subscription present is
identical (only the per-row affordance differs).

## DoR verdict: PASSED (9/9 for both stories; Dimension 0 PASS; JTBD PASS)

See `dor-checklist.md`.

## Handoff readiness

DISCUSS artifacts complete (lean set, mirroring slice-08/15): `feature-delta.md` (DISCUSS
section appended), `requirements.md`, `user-stories.md`, `acceptance-criteria.md`,
`outcome-kpis.md`, `dor-checklist.md`, `wave-decisions.md`. Ready for DESIGN
(solution-architect) once peer review approves. No code written; no DESIGN performed.

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-search-follow-state/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in the
validated J-005c job statement (slice-05/08) and the slice-08
`discover-the-network-from-the-browser` journey (the follow-guidance step on `/search`).
