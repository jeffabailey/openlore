# Component Boundaries — viewer-peer-subscriptions (slice-15)

> Companion to architecture-design.md. The 5 existing touchpoints (+ cli), the
> additions to each, and the cross-component invariants. **No new crate; 21
> members. No new dependency edge.**

## Touchpoint map (the established slice-06..14 set)

```
adapter-http-viewer  (EFFECT, driving)  ── reads ──▶  ports::StoreReadPort  ◀── impl ──  adapter-duckdb (EFFECT, driven)
        │  maps to PeersView + renders                                                          │ LOCAL SELECT (ONE aggregate)
        ▼                                                                                       ▼
viewer-domain (PURE)  ── renders PeersView (total fn of the flat DTO; NO new dep) ──▶  HTML    LOCAL DuckDB store
                                                                                        (peer_subscriptions ⟕ peer_claims)
```

Dependency direction is inward (ADR-009). No adapter depends on another adapter.
Only `cli` links the adapters (composition root, unchanged). Unlike slice-10,
**no new pure→pure edge** is introduced: the render consumes the flat
`PeerSubscriptionSummary` directly.

---

## `crates/ports` — the read contract (PURE)

**Adds** to `store_read.rs`:

- `PeerSubscriptionSummary` — the boundary DTO for one active subscription paired
  with its per-peer local claim count. A FLAT DTO (mirrors
  `ClaimRow`/`PeerClaimRow`/`SurveyRow`):

  ```text
  pub struct PeerSubscriptionSummary {
      pub peer_did: String,              // NON-Option (anti-merging, I-PS-3) — bare or fragmented as stored; rendered VERBATIM
      pub peer_handle: String,           // the stored handle (carried; DESIGN does not require it be displayed)
      pub subscribed_at: DateTime<Utc>,  // the active-set order key (mirrors list_active_subscriptions ORDER BY)
      pub local_claim_count: u64,        // COUNT(*) FROM peer_claims WHERE author_did = peer_did — PER-PEER, never merged
  }
  ```

  > `peer_did` is NON-Option (the load-bearing anti-merging attribution): each
  > summary is its own attributed row, never a merged total. `local_claim_count`
  > is `u64` (a `COUNT(*)`; 0 for a subscribed-but-never-pulled peer — US-PS-002
  > Ex 2). `peer_handle` is carried for completeness/parity with
  > `PeerSubscription`; the AC only requires the DID + count + command to render.

- ONE read method on `StoreReadPort` (NO mutation method added):

  ```text
  /// READ-ONLY active-subscription survey for the `/peers` view (slice-15 /
  /// J-003c): every ACTIVE subscription (peer_subscriptions.removed_at IS NULL),
  /// each paired with its PER-PEER local claim count (COUNT(*) FROM peer_claims
  /// WHERE author_did = peer_did). LOCAL only — NO network. The count is
  /// PER-PEER, NEVER a merged total (anti-merging, I-PS-3). A soft-removed /
  /// purged peer is EXCLUDED (active-only, I-PS-2). Returns an EMPTY vec when the
  /// operator follows no one (the viewer renders the guided empty state — never an
  /// error). ONE aggregate query, invariant to peer count (no N+1, I-PS-8).
  /// READ-ONLY by construction: a SELECT over the SAME shared connection the CLI
  /// writes through (BR-VIEW-4) — there is NO mutation method on this trait
  /// (I-VIEW-1 / I-PS-1).
  fn list_active_peer_subscriptions(
      &self,
  ) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>;
  ```

**Boundary:** `ports` stays PURE (no I/O crate). The method returns owned value
types; no lifetimes leak the connection. The existing write-side
`PeerStoragePort::list_active_subscriptions()` is NOT reused (it is on the WRITE
port and carries no counts).

---

## `crates/adapter-duckdb` — the read impl (EFFECT, driven)

**Adds** ONE impl to the `StoreReadPort` adapter (`store_read.rs`), running over
the SAME shared `Arc<Mutex<Connection>>` (BR-VIEW-4) via the existing lock
helper, mapping errors to `StoreReadError` (never panic):

```sql
-- list_active_peer_subscriptions: ONE aggregate query.
-- active-only (removed_at IS NULL) + per-peer COUNT(*) via LEFT JOIN + GROUP BY.
SELECT ps.peer_did,
       ps.peer_handle,
       ps.subscribed_at,
       COUNT(pc.cid) AS local_claim_count
  FROM peer_subscriptions ps
  LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did
 WHERE ps.removed_at IS NULL
 GROUP BY ps.peer_did, ps.peer_handle, ps.subscribed_at
 ORDER BY ps.subscribed_at
```

**Boundary / invariants:**
- **ONE aggregate query, invariant to peer count** (no N+1; no per-peer
  `count_peer_claims` fold). The LEFT JOIN + `COUNT(pc.cid)` yields 0 for a
  subscribed-but-never-pulled peer (the row is kept; `COUNT` of a NULL-joined row
  is 0). `COUNT(pc.cid)` (not `COUNT(*)`) so the no-match LEFT-JOIN row counts 0,
  not 1.
- **active-only:** `WHERE ps.removed_at IS NULL` — a soft-removed/purged peer is
  excluded (I-PS-2). The exact mirror of the write-side
  `list_active_subscriptions` filter.
- **per-peer, never merged:** the count is grouped by the subscription identity
  and joined on `author_did = peer_did`; two peers yield two rows with their own
  counts. No `AVG`, no global total.
- **anti-merging SQL rule:** the SQL names `peer_subscriptions` + `peer_claims`
  but NOT the standalone `claims` table, so `classify_sql_literal` returns `None`
  (not a cross-store `claims ∪ peer_claims` query) — the rule is N/A and stays
  GREEN. It also projects `author_did` (the JOIN/GROUP key), so the rule would
  hold even if it applied.
- **LOCAL only** — no network crate is reachable from this method.

---

## `crates/viewer-domain` — the view-model + render (PURE)

**Adds:**

- `PeersView` ADT (the pure render input — total match):

  ```text
  pub enum PeersView {
      /// ≥1 active subscription: the attributed peer rows, order-preserved from
      /// the read (subscribed_at). Each row carries its DID, per-peer count, and
      /// the render-only revoke command.
      Subscriptions { peers: Vec<PeerSubscriptionSummary> },
      /// Zero active subscriptions (or a read error): the guided empty state
      /// pointing to `openlore peer add <did>` — never blank, never an error.
      NoSubscriptions,
  }
  ```

  > `Subscriptions` carries the `ports::PeerSubscriptionSummary` flat DTO
  > directly (no second projection type needed — the render reads `peer_did` +
  > `local_claim_count` verbatim). DELIVER may instead carry a thin
  > `viewer-domain` row type if it prefers to keep the ADT crate-local; the
  > load-bearing contract is the two arms + the per-row (DID, count, command).

- The render-only command constants + helper (mirroring slice-08):

  ```text
  /// The render-only revocation command prefix (mirrors SEARCH_FOLLOW_GUIDANCE_PREFIX).
  /// Held in ONE place so the slice-03 `peer remove` verb is a single source of truth.
  pub const PEER_REMOVE_GUIDANCE_PREFIX: &str = "Unsubscribe from the CLI: openlore peer remove";

  /// The render-only starting command prefix for the empty state (mirrors the slice-08
  /// follow-guidance "openlore peer add" wording). Held in ONE place.
  pub const PEER_ADD_GUIDANCE_PREFIX: &str = "Subscribe to a peer from the CLI: openlore peer add";

  /// Render the render-only `openlore peer remove <bare-did>` command as plain TEXT
  /// inside a <p>/<code> — NEVER an <a>/form/hx-* control. The bare DID is derived by
  /// stripping any app-identity `#…` fragment (mirrors render_follow_guidance). PURE.
  fn render_remove_guidance(peer_did: &str) -> Markup;  // bare = peer_did.split('#').next()
  ```

- `render_peers_fragment(&PeersView) -> Markup` / `render_peers_page(&PeersView) -> String`:
  the page EMBEDS the SAME fragment fn (parity, I-PS-5). The `Subscriptions` arm
  renders one row per peer — the DID VERBATIM, the per-peer `local_claim_count`,
  and `render_remove_guidance(peer_did)`. The `NoSubscriptions` arm renders the
  guided empty state ("You are not subscribed to any peers.") + the
  `PEER_ADD_GUIDANCE_PREFIX` command TEXT. Both shapes embed the SAME arms.

- `PEERS_URL` nav const (`/peers`) + a nav link in the chrome (mirrors
  `MY_CLAIMS_URL`/`PEER_CLAIMS_URL` in `render_tab_nav` / `page_head` chrome).

**Boundary / invariants:**
- PURE — no I/O. **NO new `[dependencies]` edge** (the render is a total function
  of the flat DTO; it needs neither `claim-domain` nor any other pure core).
- The DID is rendered VERBATIM (attribution); maud auto-escapes it.
- The render-only command is a `<p>`/`<code>` of TEXT — never an executable
  control (I-PS-1 / I-PS-8). No `<button>`, no `<form>`, no mutating `<a>`, no
  `hx-*` that mutates.
- Each command text lives in ONE place (single mutation site).

---

## `crates/adapter-http-viewer` — the route handler (EFFECT, driving)

**Adds** ONE handler + ONE route-table arm (mirrors `peer_claims_page` /
`project_page`):

```text
// route(): synchronous arm, alongside /claims, /score, /project, /philosophy, /peer-claims
PEERS_URL => Ok(peers_page(store.as_ref(), shape)),

fn peers_page(store: &dyn StoreReadPort, shape: Shape) -> Response<Full<Bytes>> {
    let view = match store.list_active_peer_subscriptions() {
        Ok(peers) if peers.is_empty() => PeersView::NoSubscriptions,
        Ok(peers) => PeersView::Subscriptions { peers },
        Err(_)    => PeersView::NoSubscriptions,  // degrade gracefully — never a 5xx / stack trace
    };
    match shape {
        Shape::Fragment => html_ok(render_peers_fragment(&view).into_string()),
        Shape::FullPage => html_ok(render_peers_page(&view)),
    }
}
```

**Boundary / invariants:**
- Read → map-to-view (pure) → render sandwich (ADR-007). NO `.await` (LOCAL,
  sync). NO query param to parse (the route takes no argument — it lists the
  whole active set).
- The `ViewerServer` needs NO new field — the route reads the store it ALREADY
  holds (mirrors `/peer-claims`; NOT `/search`'s `IndexQueryPort` wiring).
- A read failure → `NoSubscriptions` (graceful degradation, NFR-PS-6). DELIVER
  MAY choose a distinct "could not read subscriptions" guided message instead of
  reusing the empty state, as long as it is plain-language and never a stack
  trace; the no-blank / no-5xx contract is the fixed point.
- Persists nothing; renders no write/subscribe/unsubscribe control; loopback-only.
- Adds the `/peers` nav link entry so the route is reachable from the chrome.

---

## `crates/xtask` — enforcement (tooling) — UNCHANGED

NO change. The `/peers` read is a read-only DB read like every slice-06..14
viewer read:
- The viewer capability rule (`VIEWER_FORBIDDEN_DEPS`) is UNCHANGED — the read
  touches no signing/identity/PDS/indexer surface.
- The pure-core no-I/O arm for `viewer-domain` is UNCHANGED — no new dependency
  edge (the render is a total fn of the flat DTO).
- The anti-merging SQL rule (`no_cross_table_join_elides_author`) stays GREEN by
  construction — the new SQL names `peer_subscriptions` + `peer_claims` (not the
  standalone `claims` table), so `classify_sql_literal` returns `None`.

(Contrast slice-10, which added ONE pure-core allowlist edge
`viewer-domain → claim-domain`. slice-15 adds none.)

---

## `crates/cli` — composition root — UNCHANGED

No change beyond it already wiring the viewer over the shared read handle
(BR-VIEW-4). The new route reads the store the `ViewerServer` already holds.

---

## Cross-component invariants (DELIVER must keep green)

1. **Read-only (I-PS-1, CARDINAL):** `StoreReadPort` has no mutation method; the
   new read is a `SELECT`; capability rule unchanged; behavioral read-only gold
   green (no form/button/mutating-link; active set identical before/after GETs).
2. **Active-only (I-PS-2, CARDINAL):** `WHERE removed_at IS NULL`; behavioral
   seed-a-soft-removed-peer-assert-absent gold.
3. **Per-peer, never merged (I-PS-3 / J-003a):** `PeerSubscriptionSummary.peer_did`
   non-Option; the count grouped per peer; two-peers-distinct-counts gold (5 vs 3).
4. **No N+1 (I-PS-8):** ONE aggregate query; query-count-invariant-to-peer-count
   behavioral gold.
5. **LOCAL/offline (I-PS-4):** the read touches only `peer_subscriptions`/
   `peer_claims`; the handler holds only `StoreReadPort`; network-disabled
   scenario passes; only the vendored htmx asset referenced.
6. **Render-only revocation command (I-PS-8):** `PEER_REMOVE_GUIDANCE_PREFIX` in
   ONE place; `render_remove_guidance` emits TEXT only.
7. **Parity (I-PS-5):** `render_peers_page` embeds the SAME `render_peers_fragment`;
   both shapes embed the SAME `NoSubscriptions` arm.
8. **No new crate; 21 members; no new dependency edge.**
