# Requirements: viewer-search-follow-state (slice-16)

> Wave: DISCUSS (lean) · Owner: Luna (nw-product-owner) · 2026-06-09 · Job: **J-005c**
> Brownfield DELTA on slices 05/06/07/08/15. Solution-neutral: DESIGN owns the resolution
> fn shape, the set type, the "Following" indicator markup/copy, and the match arms.

## 1. Context

The read-only `GET /search` view (slice-08) is the browser UI for J-005 network discovery.
It renders verified + attributed network-index results and, per slice-08 US-NS-004, shows a
render-only `openlore peer add <did>` follow affordance for unfollowed authors — realizing
J-005c (discovery feeds federation) as guidance text. BUT the viewer hardcodes
`AuthorRelationship::NetworkUnfollowed` for EVERY result author
(`crates/adapter-http-viewer/src/lib.rs` ~line 1021-1033), so the affordance is shown even
for authors the operator already follows. slice-16 RESOLVES each result author's relationship
against the operator's LOCAL active subscriptions (the slice-15
`list_active_peer_subscriptions` read) so a followed author is shown "Following" and NOT
re-offered a follow.

## 2. Functional Requirements

| ID | Requirement | Trace |
|---|---|---|
| FR-SF-1 | The `/search` effect shell reads the operator's LOCAL active-subscription set ONCE per render (the slice-15 `StoreReadPort::list_active_peer_subscriptions`) and materializes it into an in-memory set of bare DIDs. | US-SF-001, C-4 |
| FR-SF-2 | Each result author's relationship is resolved: bare `author_did` ∈ active set → `SubscribedPeer`; otherwise → `NetworkUnfollowed`. The hardcoded `NetworkUnfollowed` in `to_indexed_claim` is replaced. | US-SF-001, C-2 |
| FR-SF-3 | The DID comparison strips the `#fragment` on BOTH sides via the existing `bare_did` SSOT before set membership. | US-SF-001, R-SF-5 |
| FR-SF-4 | A result author resolved `SubscribedPeer` renders a neutral "Following" indicator and NO `openlore peer add` command. | US-SF-002, C-2 |
| FR-SF-5 | A result author resolved `NetworkUnfollowed` renders the slice-08 render-only `openlore peer add <bare-did>` follow guidance (via `render_follow_guidance`) — unchanged. | US-SF-002, C-2 |
| FR-SF-6 | The resolved follow-state renders identically under the htmx `#search-results` fragment and the no-JS full page (same render fn). | US-SF-002, C-8 |
| FR-SF-7 | A failed active-set read degrades resolution to all-`NetworkUnfollowed` (the slice-08 status quo); the search results still render. | US-SF-001, C-7 |

## 3. Non-Functional Requirements

| ID | Requirement | Measurable criterion |
|---|---|---|
| NFR-SF-1 (Read-only, CARDINAL) | The viewer holds no signing key and exposes no follow/unfollow control; both affordances are render-only TEXT. | Behavioral gold: no `<button>`/`<form>`/mutating `<a>`/`hx-*` follow control on `/search`; no key in the process. (KPI-VIEW-2) |
| NFR-SF-2 (Accuracy) | A followed author is shown "Following" and NOT re-offered a follow; an unfollowed author keeps the `peer add` affordance. | Behavioral: a seeded active subscription → that result row shows "Following" + no add command; an unfollowed result row shows the add command. (J-005c) |
| NFR-SF-3 (No N+1) | The active set is read exactly ONCE per `/search` render, invariant to result count. | Behavioral: active-set read count invariant to the number of result rows. (C-4) |
| NFR-SF-4 (LOCAL / offline resolution) | Relationship resolution uses NO network; the index stays per-user-neutral. | Behavioral: no extra network call for resolution; the index query payload carries no follow-graph state. (C-3, KPI-5) |
| NFR-SF-5 (Attribution + ranking unchanged) | Resolution does not merge, re-group, or re-rank results. | Behavioral: result grouping + order with-and-without an active subscription present are identical; only the per-row affordance differs. (C-5, KPI-AV-2) |
| NFR-SF-6 (Graceful degradation) | A failed active-set read never crashes, blanks, or leaks; it degrades to the slice-08 status quo. | Behavioral: seeded read failure → all rows `peer add`, results still render, no leaked error. (C-7) |
| NFR-SF-7 (Parity) | Both shapes render the resolved follow-state identically. | Behavioral: htmx fragment vs no-JS full page structurally identical for the follow-state. (C-8) |

## 4. Business Rules

- **BR-SF-1**: Binary resolution on `/search` — `SubscribedPeer` (∈ active set) or
  `NetworkUnfollowed` (otherwise). `You` and `UnsubscribedCache` are NOT resolved on this
  surface (WD-SF-2/5). A soft-removed peer (not in the active set) → `NetworkUnfollowed`.
- **BR-SF-2**: The active set is the slice-15 active-only read (`removed_at IS NULL`) — the
  single source of truth for "who the operator currently follows". No second follow-graph
  read path is introduced.
- **BR-SF-3**: The follow path is the slice-03 CLI `openlore peer add`. The viewer never
  executes a follow; it only resolves + displays the relationship + the render-only command.

## 5. Dependencies

| Dependency | Status |
|---|---|
| slice-15 `StoreReadPort::list_active_peer_subscriptions` + `PeerSubscriptionSummary` (the active-set read) | SHIPPED |
| slice-08 `GET /search`, `to_indexed_claim`, `resolve_search_state`, `compose_results`, `render_search_results_fragment`, `render_follow_guidance`, `SEARCH_FOLLOW_GUIDANCE_PREFIX` | SHIPPED |
| `AuthorRelationship` enum (`You`/`SubscribedPeer`/`UnsubscribedCache`/`NetworkUnfollowed`) | SHIPPED — no new variant |
| `bare_did` SSOT (fragment strip) | SHIPPED |

## 6. Out of Scope

See `user-stories.md` §"Out of scope". Key exclusions: follow/unfollow control in the viewer;
`You` own-DID resolution (deferred); `UnsubscribedCache` resolution; any network seam for
resolution; re-grouping/re-ranking/merging; new route/read/variant/crate; N+1.
