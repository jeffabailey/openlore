# ADR-053: Viewer-Side `/search` Relationship Resolution Against the LOCAL Active-Subscription Set (batch-once, binary SubscribedPeer-vs-NetworkUnfollowed, `You` deferred) + the Render-Only "Following" Indicator

- **Status**: Accepted
- **Date**: 2026-06-09
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-search-follow-state` (slice-16)
- **Feature**: viewer-search-follow-state (slice-16)
- **Extends**: ADR-052 (the slice-15 active-subscription survey + `list_active_peer_subscriptions` read REUSED here as the active-set source), ADR-037 (the slice-08 `GET /search` view: `SearchState` ADT, `to_indexed_claim`, `compose_results`, the render-only `SEARCH_FOLLOW_GUIDANCE_PREFIX` + `render_follow_guidance` + bare-DID strip — the pattern the "Following" indicator mirrors), ADR-030 (the read-only `StoreReadPort` with NO mutation method), ADR-028 (the loopback bind + the store-readability startup probe), ADR-009 (Hexagonal + Modular Monolith), ADR-007 (functional paradigm — pure render core, effect shell at the I/O edge)

## Context

The slice-08 `GET /search` view renders network-discovery results. The viewer's
`to_indexed_claim` (`crates/adapter-http-viewer/src/lib.rs` ~line 1021-1033)
hardcodes `AuthorRelationship::NetworkUnfollowed` for EVERY result author (the
~line 1017 comment admits "always `NetworkUnfollowed` … per-user-neutral"). The
relationship is threaded end-to-end already — `compose_results` carries
`claim.relationship` verbatim into `NetworkResultRow.relationship`
(`crates/appview-domain/src/compose.rs` ~line 126), and
`render_search_result_row` (`crates/viewer-domain/src/lib.rs` ~line 1719)
branches on it — but because the source is hardcoded, the render layer cannot
tell an already-followed author from an unfollowed one: the `openlore peer add
<did>` follow guidance is offered to EVERYONE, including peers the operator
already subscribes to, and a followed author is never recognized as such.

slice-08's own "Driving Ports for DESIGN" note flagged this as a deferred
decision ("the relationship-label projection — subscribed-peer vs unfollowed —
reads the local subscriptions; DESIGN decides whether the viewer surfaces that
label"). slice-16 IS that deferred decision, now taken. DISCUSS is APPROVED (DoR
9/9); the validated job is **J-005c** (turn a discovery into a follow). The
infrastructure (the `AuthorRelationship` enum, the slice-15
`list_active_peer_subscriptions` read, the `bare_did` SSOT, the
`render_follow_guidance` pattern) all already ship. Three DESIGN-owned questions
remain (the DISCUSS Technical Notes explicitly defer them), each with a real
downside if decided wrong:

1. **Where does relationship resolution land, and how is the N+1 trap closed?**
   The naive shape — for each result author, query "is this DID subscribed?" — is
   N+1 across the result set (R-SF-2, a HIGH risk). The active set must be read
   ONCE and each author resolved in memory. DESIGN must choose the resolution
   seam and structurally foreclose the per-result query.

2. **What relationship states does `/search` resolve, and what about `You` and
   `UnsubscribedCache`?** The enum has four variants; the network corpus is
   per-user-neutral and carries no own-marker. DESIGN must decide the resolution
   surface and the fragment-matching discipline (R-SF-5).

3. **How is a followed author rendered — is there an existing render arm or a new
   one?** The render must show a neutral "Following" indicator for a followed
   author (NO add command) and keep `render_follow_guidance` for an unfollowed
   one (R-SF-3 / R-SF-4). DESIGN must decide whether `render_search_result_row`
   already branches on `SubscribedPeer` or needs a new arm, and where the
   indicator copy lives.

## Decision

### D1 — Resolve in the EFFECT shell, batch-once, in memory (no N+1)

Relationship resolution lands in the **effect shell** (`adapter-http-viewer`),
in `resolve_search_state` (~line 884), AFTER the `IndexQueryPort::search` call
and BEFORE `compose_results`. The shell:

1. Reads the active-subscription set ONCE per `/search` render via the REUSED
   slice-15 `store.list_active_peer_subscriptions()` (itself ONE aggregate query
   — ADR-052). NO new read method, NO new SQL, NO `adapter-duckdb` change.
2. Materializes the bare `peer_did`s into an in-memory `HashSet<String>` (the
   `peer_did` is already bare — `PeerSubscriptionSummary.peer_did`).
3. Resolves each result author by threading the set into `to_indexed_claim`,
   which STOPS hardcoding `NetworkUnfollowed` and instead computes:
   `bare_did(author_did) ∈ set → SubscribedPeer`, else `NetworkUnfollowed`.

The active-set read count is **invariant to the number of result rows** — ONE
read per render, never one per author. The DID comparison strips the
`#org.openlore.application` fragment on BOTH sides via the existing `bare_did`
SSOT before membership (the set's `peer_did` is already bare; the result's
`author_did` may be fragmented — R-SF-5). Resolution sets the per-row
`relationship` ONLY; it never re-groups, re-ranks, or merges (`compose_results`
grouping + order unchanged). **A failed active-set read degrades to an EMPTY set
→ every author resolves to `NetworkUnfollowed` (the slice-08 status quo)** — no
crash, no 5xx, no leaked error (mirrors the slice-15 `/peers`
`Err(_) → NoSubscriptions` graceful-degrade precedent).

> DESIGN owns whether `to_indexed_claim` takes the set as a parameter (chosen:
> the resolution becomes a small total fn `resolve_relationship(author_did,
> &active_set) -> AuthorRelationship` and `to_indexed_claim` takes `&HashSet` —
> or, equivalently, the shell maps over the raw rows with the set in a closure).
> The load-bearing contract is the AC: one batch read, in-memory resolution,
> fragment-stripped membership, graceful degradation. The closure-vs-parameter
> shape is a DELIVER detail with the same observable behavior.

### D2 — Binary resolution: `SubscribedPeer` vs `NetworkUnfollowed`; `You` deferred; `UnsubscribedCache` N/A

slice-16 resolves to exactly TWO states:

- `bare_did(author_did) ∈ active set` → **`SubscribedPeer`**.
- otherwise → **`NetworkUnfollowed`**.

A soft-removed peer (`removed_at` set) is NOT in the active set (the slice-15
read filters `removed_at IS NULL`), so it resolves to `NetworkUnfollowed` —
correct, because the operator does NOT currently follow them; they are
re-offered the follow. `UnsubscribedCache` is a slice-03 federated-read
cache-residue concept, not a network-discovery relationship — it is NOT resolved
on `/search`.

**`You` (own-DID) resolution is DEFERRED.** The `/search` corpus is the NETWORK
index (per-user-neutral); a network row carries no `SourceTable`/own-marker, and
the read-only network-search surface does not cheaply hold the operator's OWN
DID (there is no identity surface in the read-only viewer; adding one would blur
the key-less boundary). An own-author result therefore falls to
`NetworkUnfollowed` (re-offered a `peer add` it would never run — acceptable;
the operator never subscribes to themselves, so they are never wrongly shown
"Following"). Revisit if/when the viewer cheaply holds the operator DID (e.g. a
future `/me` surface). No new `AuthorRelationship` variant is introduced.

### D3 — The render: add ONE `SubscribedPeer` arm to the EXISTING branch; new render-only "Following" indicator const + helper

`render_search_result_row` (`crates/viewer-domain/src/lib.rs` ~line 1719) today
branches ONLY `@if matches!(row.relationship, NetworkUnfollowed) →
render_follow_guidance`. There is **NO `SubscribedPeer` arm yet** — slice-16
ADDS it. The render becomes a two-arm branch over `row.relationship`:

- `NetworkUnfollowed` → `render_follow_guidance(&row.author_did.0)` (UNCHANGED —
  the slice-08 render-only `openlore peer add <bare-did>` TEXT).
- `SubscribedPeer` → a new `render_following_indicator()` emitting a neutral
  render-only label `<p>{SEARCH_FOLLOWING_INDICATOR}</p>` of plain TEXT.
- `You` / `UnsubscribedCache` → render NOTHING extra (they never arise on
  `/search` per D2; the branch is total over the enum by treating them as the
  no-affordance default).

The "Following" indicator is the SIBLING of `render_follow_guidance`, mirroring
the slice-08 / slice-15 single-source-of-truth pattern: a NEW
`SEARCH_FOLLOWING_INDICATOR` const (the sibling of `SEARCH_FOLLOW_GUIDANCE_PREFIX`)
held in ONE place. **Indicator copy: `"Following"`** (a neutral render-only
label; NO command, NO verb-phrase, NO DID — distinct from the follow guidance's
"Follow this author from the CLI: openlore peer add …"). It is a `<p>`/`<span>`
of TEXT — never an `<a>`/`<button>`/`<form>`/`hx-*` control.

> DESIGN owns the exact markup element + copy; the load-bearing contract is the
> AC: a `SubscribedPeer` row shows a neutral "Following" label and NO `peer add`
> command; a `NetworkUnfollowed` row keeps the slice-08 command. The copy
> `"Following"` is the single source of truth; DELIVER may refine the surrounding
> markup so long as the indicator is render-only TEXT and carries no command.

## Alternatives Considered

### A1 — Per-result subscription query (`is_subscribed(did)` per author). REJECTED.

The naive shape: for each result author, query "is this DID an active
subscription?". Correct in outcome but N+1 across the result set (R-SF-2) — the
read count grows with the result count, exactly the trap the slice-15
single-aggregate discipline forbids. The slice-15 read already returns the WHOLE
active set in ONE query; reading it once and resolving in memory is strictly
cheaper and bounds the read count at 1. **Rejected** for the batch-once
in-memory resolution (D1).

### A2 — Resolve `You` via an own-DID identity surface in the viewer. REJECTED (deferred).

Resolving `You` requires the read-only network-search surface to cheaply hold
the operator's OWN DID. The viewer holds no key and exposes no identity surface;
adding one to resolve a cosmetic own-author label would blur the cardinal
key-less / read-only boundary for a small payoff (an own-author result merely
keeps a `peer add` the operator would never run). **Deferred** (D2); revisit
behind a future `/me` surface. This keeps the slice thin and the boundary crisp.

### A3 — Add a new `AuthorRelationship` variant or a viewer-local relationship type. REJECTED.

The existing enum (`You | SubscribedPeer | UnsubscribedCache | NetworkUnfollowed`)
already models exactly the two states slice-16 resolves. A new variant (or a
parallel viewer-local enum) would duplicate the model, ripple through
`compose_results` + the render, and add a workspace-wide type for no behavioral
gain. **Rejected** — slice-16 USES the two existing variants verbatim (C-9 / no
new variant; workspace stays 21).

### A4 — Resolve the relationship against the NETWORK index (ask the indexer "do I follow this author?"). REJECTED.

This would tell the per-user-neutral index who the operator follows, violating
the slice-05/08 index-neutrality boundary (KPI-AV per-user-neutral) and
introducing a network seam on a relationship that is purely LOCAL business.
**Rejected** — resolution is a LOCAL DuckDB read (D1 / C-3); the index query is
unchanged and per-user-neutral.

## Consequences

### Positive

- **Accuracy (the load-bearing fix):** a followed author shows "Following" + NO
  add; an unfollowed author keeps the `peer add` affordance. The discovery→follow
  funnel (KPI-AV-4) shows the `peer add` command ONLY where it is actionable
  (0% re-offered to already-followed authors).
- **No N+1:** ONE active-set read per render, invariant to result count (D1).
- **Reuses everything:** the slice-15 read (no new read method/SQL), the
  `AuthorRelationship` enum (no new variant), the slice-08 `render_follow_guidance`
  + `bare_did` SSOT, the end-to-end `relationship` thread through
  `compose_results`. The ONLY new code is: the in-shell resolution (replacing one
  hardcoded line), the `SubscribedPeer` render arm, and one indicator const.
- **Read-only / LOCAL / offline preserved:** resolution is a LOCAL read; both
  affordances are render-only TEXT; the index stays per-user-neutral; the viewer
  holds no key and executes nothing.
- **Anti-merging preserved:** resolution sets the per-row `relationship` only;
  grouping, order, the `[verified]` marker, and verbatim confidence are
  unchanged.
- **Parity by construction:** the resolution happens in the shell BEFORE the
  render; both the htmx `#search-results` fragment and the no-JS full page consume
  the SAME `SearchState`, so they agree.

### Negative / trade-offs

- **An own-author result is re-offered a `peer add`** (the `You` deferral, D2/A2)
  — accepted; the operator never runs it. Bounded, documented, revisitable.
- **One extra LOCAL read per `/search` render** (the active set) — a single
  aggregate query (ADR-052), invariant to result count; negligible vs the network
  index query already on the path. Degrades to the status quo on failure.
- **The render branch is now two-armed** — a slightly larger total match in
  `render_search_result_row`; mitigated by one mutation site per affordance
  (`render_follow_guidance` + `render_following_indicator`).

## Enforcement (three orthogonal layers — ADR-009 / `cargo xtask check-arch`)

UNCHANGED from the established viewer posture; no new edge, no new forbidden dep,
no new SQL literal:

1. **Subtype (type):** `StoreReadPort` still has NO mutation method (the REUSED
   read is a `SELECT`); the viewer holds `StoreReadPort` + `IndexQueryPort`,
   neither with a mutation method; `viewer-domain` keeps its existing pure-core
   no-I/O dependency allowlist (`{maud, ports, appview-domain, scoring,
   claim-domain}`) — the render of the new arm is a total fn of the existing
   `NetworkResultRow`, so NO new dependency edge is added.
2. **Structural (xtask import-graph + syn-AST):** `VIEWER_FORBIDDEN_DEPS`
   UNCHANGED — resolution touches no signing/identity/PDS surface and reuses the
   already-allowed `StoreReadPort`. The anti-merging SQL rule
   (`no_cross_table_join_elides_author`) is N/A — slice-16 adds NO SQL (it reuses
   the slice-15 query). `import-linter` remains rejected project-wide (import-graph
   only; cannot express the method-presence / SQL-literal / render-only rules).
3. **Behavioral (CI gold, port-to-port through the real `openlore ui`
   subprocess):** (a) a seeded followed author resolves to `SubscribedPeer` and
   shows "Following" with NO `peer add` command; (b) an unfollowed author keeps
   the `peer add` command; (c) the active-set read count is invariant to the
   result count (no N+1); (d) a failed active-set read degrades to all-
   `NetworkUnfollowed` without crash/blank/leak; (e) a fragmented result DID
   matches a bare active-set DID; (f) grouping/order identical with-and-without an
   active subscription; (g) neither affordance is an executable control.

### Earned-Trust (principle 12) — probe posture

No new adapter or port with its own substrate dependency is introduced. The
REUSED `list_active_peer_subscriptions` read runs over the EXISTING,
already-probed `StoreReadPort` DuckDB connection (the store-readability startup
probe of ADR-028/030 — "wire then probe then use" — already gates the viewer's
startup). The substrate "lie" this slice must survive is **a mid-request read
FAILURE**, which is explicitly exercised by behavioral gold (d) above (degrade to
all-`NetworkUnfollowed`, no crash). No new external dependency that could lie is
added, so no new `probe()` scenario is required; `cargo xtask check-probes` is
UNCHANGED.

**Confirmation: NO new crate, NO new route, NO new read method, NO new
`AuthorRelationship` variant, NO new persisted type, NO `adapter-duckdb` change,
NO xtask `check-arch` delta. Workspace stays 21 members. Functional paradigm
(ADR-007) honored — pure render core, effect-shell resolution.**
