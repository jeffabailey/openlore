# Data Models — viewer-peer-subscriptions (slice-15)

> Companion to architecture-design.md / component-boundaries.md. The boundary
> DTO, the pure view-model ADT, the active-subscription survey SQL shape, and the
> render-only command text. **Zero new persisted type** (the subscription list is
> computed per request, WD-PS-8 / I-PS-6).

## 1. Read-side boundary DTO — `ports::PeerSubscriptionSummary`

The flat row the active-subscription survey returns (one per ACTIVE
subscription). Mirrors `ClaimRow`/`PeerClaimRow`/`SurveyRow` (FLAT DTO, not the
rich `PeerSubscription`).

| Field | Type | Source | Load-bearing? |
|---|---|---|---|
| `peer_did` | `String` (NON-Option) | `peer_subscriptions.peer_did` | **YES** — anti-merging (I-PS-3); never elided; rendered VERBATIM (FR-PS-2) |
| `peer_handle` | `String` | `peer_subscriptions.peer_handle` | carried for parity with `PeerSubscription`; not required by the AC to render |
| `subscribed_at` | `DateTime<Utc>` | `peer_subscriptions.subscribed_at` | the active-set order key (mirrors the write-side `ORDER BY subscribed_at`) |
| `local_claim_count` | `u64` | `COUNT(pc.cid)` over `peer_claims WHERE author_did = peer_did` | **YES** — PER-PEER count (FR-PS-3 / I-PS-3); 0 for a subscribed-but-never-pulled peer (US-PS-002 Ex 2) |

> The DTO lives in `crates/ports/src/store_read.rs` beside the other read row
> DTOs (Q2, ADR-052). It is READ-ONLY boundary data — returned by a
> `StoreReadPort` method, consumed by `adapter-http-viewer` and rendered by
> `viewer-domain`. It is NEVER persisted (computed per request).

## 2. Pure view-model ADT — `viewer_domain::PeersView`

```text
/// The pure render input for the `/peers` view. Total match in the renderer
/// (nw-fp-domain-modeling §1): the operator either follows ≥1 active peer, or
/// none (the guided empty state). A read error degrades to NoSubscriptions in the
/// effect shell BEFORE this ADT is built (so the renderer stays total + I/O-free).
pub enum PeersView {
    /// ≥1 active subscription: the attributed peer rows in the read's order
    /// (subscribed_at). Each row renders its DID VERBATIM, its per-peer
    /// local_claim_count, and the render-only `openlore peer remove <bare-did>`
    /// command. Grouping/merging is NEVER done — each peer is its own row keyed
    /// by its DID (anti-merging, I-PS-3).
    Subscriptions { peers: Vec<PeerSubscriptionSummary> },

    /// Zero active subscriptions (an empty read, OR a store-read failure the shell
    /// degraded): the guided empty state ("You are not subscribed to any peers.")
    /// + the render-only `openlore peer add <did>` starting command. NEVER a blank
    /// page, NEVER an error (US-PS-003). A store whose only rows are soft-removed
    /// also yields this arm (active-only — those rows never reach the read).
    NoSubscriptions,
}
```

> `Subscriptions` carries the `ports::PeerSubscriptionSummary` flat DTO directly
> — the renderer reads `peer_did` + `local_claim_count` verbatim, so no second
> projection type is required and `viewer-domain` needs NO new dependency edge.
> DELIVER MAY introduce a thin crate-local row type if it prefers the ADT not
> reference `ports`; the load-bearing contract is the two arms + per-row (DID,
> count, command).

### Projection rules (pure, anti-merging)

- Non-empty read `Vec<PeerSubscriptionSummary>` → `Subscriptions { peers }` (order
  preserved from the SQL `ORDER BY subscribed_at`).
- Empty read → `NoSubscriptions`.
- Each peer row is its own attributed row keyed by `peer_did`; the count is the
  peer's own `local_claim_count`. NO merge, NO "all peers" total, NO "consensus
  peer" row.

### Worked examples (from the journey)

**US-PS-002 Ex 1 — two followed peers, distinct counts:**
`list_active_peer_subscriptions()` → 2 summaries — Rachel
(`did:plc:rachel-test`, `local_claim_count = 5`), Tobias
(`did:plc:tobias-test`, `3`). → `Subscriptions { peers: [rachel(5), tobias(3)] }`.
Renders two rows: each its DID VERBATIM, "5" / "3", and
`openlore peer remove did:plc:rachel-test` / `… tobias-test`. Never a combined
"8", never a merged row.

**US-PS-002 Ex 2 — followed peer, zero cached claims:**
`did:plc:newpeer-test` subscribed but never pulled → `local_claim_count = 0` (the
LEFT JOIN keeps the row; `COUNT(pc.cid)` of the NULL-joined row is 0). →
`Subscriptions { peers: [newpeer(0)] }`. Renders the row with "0 claims" and its
revoke command.

**US-PS-002 Ex 3 / US-PS-003 Ex 2 — removed/soft-removed peer absent:**
A peer with `removed_at` set is excluded by the SQL `WHERE removed_at IS NULL` —
it never enters the `Vec`. If it was the only row → empty `Vec` →
`NoSubscriptions` → the guided empty state. The absence IS the residue-free
promise rendered.

**US-PS-003 Ex 1 — no subscriptions:**
empty `Vec` → `NoSubscriptions` → "You are not subscribed to any peers." +
`Subscribe to a peer from the CLI: openlore peer add <did>`. Exit 200, never
blank, never an error.

## 3. Active-subscription survey SQL shape (LOCAL, read-only, ONE aggregate)

```sql
-- list_active_peer_subscriptions(): one row per ACTIVE subscription + its
-- per-peer local claim count, in ONE aggregate query (no N+1, I-PS-8).
SELECT ps.peer_did,
       ps.peer_handle,
       ps.subscribed_at,
       COUNT(pc.cid) AS local_claim_count
  FROM peer_subscriptions ps
  LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did
 WHERE ps.removed_at IS NULL
 GROUP BY ps.peer_did, ps.peer_handle, ps.subscribed_at
 ORDER BY ps.subscribed_at;
```

Notes:
- **`LEFT JOIN` (not inner JOIN):** keeps a peer with ZERO cached claims in the
  result (US-PS-002 Ex 2). An inner JOIN would silently drop never-pulled peers.
- **`COUNT(pc.cid)` (not `COUNT(*)`):** a LEFT-JOIN row with no `peer_claims`
  match has `pc.cid = NULL`; `COUNT` over a NULL column counts 0, so a
  never-pulled peer reports 0, not 1.
- **`WHERE ps.removed_at IS NULL`:** active-only (I-PS-2) — the exact mirror of
  the write-side `list_active_subscriptions` filter; a soft-removed/purged peer
  never appears.
- **per-peer, never merged:** the `COUNT` is grouped by the subscription
  identity and matched on `author_did = peer_did`; two peers → two rows with
  their own counts. NO `AVG`, NO global total, NO "consensus" row.
- **anti-merging SQL rule (xtask):** the literal names `peer_subscriptions` +
  `peer_claims` but NOT the standalone `claims` table — `classify_sql_literal`
  returns `None` (not cross-store), so the rule is N/A and stays GREEN. It also
  projects `peer_did` / `author_did` (the JOIN/GROUP key), so it would hold even
  if applied.
- **LOCAL only; empty → empty `Vec`** (Ok, not Err — the viewer renders the
  guided empty state).

> A **correlated subquery** form (`SELECT ps.*, (SELECT count(*) FROM peer_claims
> WHERE author_did = ps.peer_did) FROM peer_subscriptions ps WHERE removed_at IS
> NULL`) is equally ONE query and equally correct; the LEFT JOIN + GROUP BY is
> chosen (ADR-052 Q1) as the single-scan aggregate that reads as the natural
> whole-set lift of the existing `count_peer_claims(conn, peer_did)` shape.

## 4. Render-only command text (single source of truth, mirrors slice-08)

```text
PEER_REMOVE_GUIDANCE_PREFIX = "Unsubscribe from the CLI: openlore peer remove"
PEER_ADD_GUIDANCE_PREFIX    = "Subscribe to a peer from the CLI: openlore peer add"
```

`render_remove_guidance(peer_did)` strips any app-identity `#…` fragment
(`peer_did.split('#').next()`, mirroring `render_follow_guidance`) and emits
`<p>{PEER_REMOVE_GUIDANCE_PREFIX} {bare_did}</p>` — render-only TEXT, never an
`<a>`/`<form>`/`hx-*` control. The empty state emits
`<p>{PEER_ADD_GUIDANCE_PREFIX} <did></p>` (a placeholder `<did>`, since the
operator follows no one yet). The exact wording (the leading verb phrase) is a
DELIVER detail; the load-bearing contract is: the slice-03 verb (`peer remove` /
`peer add`) appears as plain TEXT, with the bare DID for the remove command, in
ONE place each.

## 5. Persistence

NONE. The active-subscription survey + `PeerSubscriptionSummary` + `PeersView`
are computed per request and never written (WD-PS-8 / I-PS-6). No schema change,
no migration, no new table/column. The bind stays loopback 127.0.0.1.
