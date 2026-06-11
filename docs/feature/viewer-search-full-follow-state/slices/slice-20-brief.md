# Slice Brief: slice-20 — viewer-search-full-follow-state

> The peer-DELTA completing the `/search` follow-state ADT to its full four-arm
> resolution. Resolves the slice-16 explicit deferral ("Binary resolution
> (You/UnsubscribedCache deferred)").

## One-line

On the read-only `GET /search` view, resolve the two remaining `AuthorRelationship`
arms — `You` (the result is the operator's OWN claim → a neutral self-attribution
indicator) and `UnsubscribedCache` (the result is a cached claim from a peer the
operator has SINCE removed → a neutral residue indicator) — completing the follow-state
ADT begun in slice-16 (which resolved only `SubscribedPeer` / `NetworkUnfollowed`).

## Why now (the resolved deferral)

slice-16 (`viewer-search-follow-state`) added per-result follow-state on `/search`
but resolved it BINARY:

- `SubscribedPeer` — author ∈ LOCAL active set → "Following" indicator, no add command.
- `NetworkUnfollowed` — author ∉ active set → keeps the `openlore peer add` affordance.

slice-16 EXPLICITLY DEFERRED (CONTEXT.md slice-16 entry, ADR-053 DV-SF-3):

- `You` — the result author is the operator themselves (own DID); neither "Following"
  nor "add" is meaningful (you cannot follow yourself). slice-16 resolved this to
  `NetworkUnfollowed` (re-offered a self-follow it would never run).
- `UnsubscribedCache` — the result author is a peer the operator soft-removed
  (`openlore peer remove`, no `--purge`): present in the LOCAL `peer_claims` cache but
  NOT in the active set (the slice-15 PS-4 retained-cache nuance). slice-16 resolved
  this to `NetworkUnfollowed` too (indistinguishable from a never-subscribed author).

The `AuthorRelationship` enum (`crates/ports/src/federated_row.rs` ~line 67) ALREADY
carries all four variants; the LOCAL-graph read (`crates/adapter-duckdb/src/graph_query.rs`
`attributed_claim_from` ~line 186) ALREADY resolves all four for the federated-read
surfaces. The render `@match` in `viewer-domain` (~line 1924) ALREADY has the two
deferred arms wired to render NOTHING (`You | UnsubscribedCache => {}`). slice-20 makes
the `/search` resolution PRODUCE those arms and gives each a neutral render-only
indicator.

## Scope (thin, additive, render-only)

- Resolve `You`: result author's bare DID ∈ the operator's OWN-claim author DIDs
  (a NEW LOCAL presence read — distinct own author DIDs from the `claims` table).
- Resolve `UnsubscribedCache`: result author's bare DID ∈ the LOCAL cached-peer DIDs
  (present in `peer_claims`) AND ∉ the active set (a NEW LOCAL presence read — distinct
  cached peer author DIDs, including soft-removed).
- Resolution precedence (on `/search`): `You` > `SubscribedPeer` > `UnsubscribedCache`
  > `NetworkUnfollowed`.
- Render two NEW neutral arms: `You → render_self_indicator()`, `UnsubscribedCache →
  render_cached_unsubscribed_indicator()` (siblings of `render_following_indicator`).

## Cardinals (inherited, restated)

- Read-only / no key: the two new indicators are render-only TEXT; no control; no key.
- Additive / no-regression: the slice-16 `SubscribedPeer` "Following" + `NetworkUnfollowed`
  `peer add` rendering is byte-stable; the original search ranking/attribution unchanged.
- LOCAL / offline: the two new presence reads are LOCAL DuckDB; the network index query
  is unchanged + per-user-neutral.
- Graceful degrade: a failed own-DID or cache-presence read degrades to the slice-16
  status quo (the row resolves to `SubscribedPeer`/`NetworkUnfollowed` as before), never
  a 5xx / blank / leak.
- Neutral framing: the `You` + `UnsubscribedCache` indicators are neutral, never
  pejorative.

## Stories

| ID | Title | job_id |
|---|---|---|
| US-FS-001 | Resolve `You` + `UnsubscribedCache` on `/search` against LOCAL own-DID + cached-peer presence (two new batch reads; complete the four-arm resolution) | infrastructure-only |
| US-FS-002 | On `/search`, a developer's own claim shows a neutral self indicator and a soft-removed peer's cached claim shows a neutral residue indicator | J-005c |

## Walking skeleton

Brownfield DELTA — no Feature 0. The thinnest end-to-end slice is US-FS-002 (the two
new indicators rendered from the resolution), backed by US-FS-001 (the two presence
reads + the precedence resolution). Delivery: US-FS-001 → US-FS-002.

## Out of scope

A follow/unfollow control; a signing key; an own-identity surface beyond reading the
operator's own-claim author DIDs; re-rank/merge; a new route or crate; per-result
queries (N+1); resolving `You`/`UnsubscribedCache` on any surface other than `/search`
(the LOCAL graph already resolves them on `/project`, `/philosophy`, etc.).

## Effort

~0.5–1 day. Two new LOCAL presence reads + a four-arm resolution + two new render arms;
everything else REUSED (the slice-16 resolution seam, the existing enum, the render
`@match` whose two empty arms already exist).
