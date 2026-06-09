# Walking Skeleton — viewer-search-follow-state (slice-16)

> Wave: **DISTILL** · Owner: Quinn (nw-acceptance-designer) · 2026-06-09

## Brownfield DELTA — no Feature-0 skeleton

slice-16 is a thin brownfield DELTA on the EXISTING read-only `GET /search` view
(slice-08). The viewer, the `/search` route, `to_indexed_claim`,
`render_search_results_fragment`, `render_follow_guidance`, the `AuthorRelationship` enum
(slice-03/05), and the active-subscription read (`list_active_peer_subscriptions`,
slice-15) all already exist and are GREEN. There is NO new walking-skeleton Feature-0;
the thinnest end-to-end slice IS the accuracy behavior itself.

## The walking-skeleton scenario

**SF-1 — `a_followed_author_shows_following_while_an_unfollowed_author_keeps_peer_add`**
(`@walking_skeleton @driving_port @driving_adapter @real-io`).

This is the thinnest COMPLETE thread the slice can demo end-to-end, and it exercises every
slice-16 integration point in one scenario:

1. **READ** the operator's LOCAL active-subscription set (the REUSED slice-15
   `list_active_peer_subscriptions`) — seeded here by a REAL `peer add did:plc:rachel-test`.
2. **RESOLVE** each result author's `author_did` against the active-DID set in the EFFECT
   shell (`resolve_search_state` → `to_indexed_claim`), fragment-stripped via `bare_did`
   (Rachel ∈ set → `SubscribedPeer`; Priya ∉ set → `NetworkUnfollowed`).
3. **RENDER** the differentiated per-row affordance in the PURE `viewer-domain`
   (`render_search_result_row`): Rachel → the NEW `render_following_indicator()` "Following"
   arm; Priya → the unchanged slice-08 `render_follow_guidance` `openlore peer add` arm.

### Litmus test (Dimension 5 — user-centric, not technical-flow)

- **Title** describes the user goal ("a followed author shows Following while an unfollowed
  author keeps peer add"), not a technical flow ("the active set threads through
  to_indexed_claim").
- **Given/When** describe Maria's context + action ("Maria actively follows
  did:plc:rachel-test but not did:plc:priya-test … she opens GET /search"), not internal
  state setup.
- **Then** describes Maria's observations ("Rachel's row shows the neutral Following
  indicator and NO peer add command; Priya's row keeps the render-only openlore peer add
  did:plc:priya-test"), not internal side effects.
- A non-technical stakeholder confirms: "yes — a developer Maria already follows should be
  shown as such, not re-offered a follow; a stranger should still get the one-step follow
  hint." This is exactly J-005c's accuracy fix.

### Demo-ability

Demo-able from the acceptance test directly: spawn the real `openlore ui`, seed Rachel as
an active subscription, search the reproducible-builds object over a reachable index, and
observe Rachel's row reading "Following" (no command) while Priya's reads
`openlore peer add did:plc:priya-test`. One screen, one search, the accuracy fix visible.

## Infrastructure strategy (Architecture of Reference + Project Policy)

Per `docs/architecture/atdd-infrastructure-policy.md` (inherited, `--policy=inherit`):

- **Driving port** `openlore ui` `GET /search` → REAL long-running subprocess
  (`ViewerServer::start_with_indexer`, bound ephemeral `:0`), driven over HTTP. The
  production composition root (Pillar 3).
- **Driven internal** `StoragePort` (the user's `openlore.duckdb`) → REAL DuckDB under
  `OPENLORE_HOME`, seeded via the REAL slice-03 `peer add` verb. The active-subscription set
  the resolution reads.
- **Driven internal** `IndexQueryPort` (CLI↔indexer XRPC) → REAL `openlore-indexer serve`
  over a localhost ephemeral port (the slice-05 reuse). The ONLY mocked boundary — a REAL
  slice-05 binary over a seeded corpus, not a hand-rolled HTTP double.

No new port → **no policy row appended**; a one-line slice-16 note is added to the viewer
row (the `/search` follow-state facet REUSES the slice-08 + slice-15 wiring).

## Build-before-run

`cargo build` BOTH the `openlore` (viewer) and `openlore-indexer` (seeded serve) bins before
running the ATs (`cargo test` does not rebuild spawned binaries automatically). Carry into the
DELIVER roadmap (mirrors slice-05/08/15).
