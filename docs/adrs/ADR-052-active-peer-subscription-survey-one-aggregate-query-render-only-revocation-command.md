# ADR-052: The Active-Peer-Subscription Survey as ONE Aggregate Query (LEFT JOIN + GROUP BY, active-only, no N+1), Its Read DTO in `ports`, and the Render-Only Revocation Command Reusing the slice-08 Pattern

- **Status**: Accepted
- **Date**: 2026-06-09
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-peer-subscriptions` (slice-15)
- **Feature**: viewer-peer-subscriptions (slice-15)
- **Extends**: ADR-042 (the slice-10 two-read-method survey shape over the read-only `StoreReadPort` + the FLAT row DTO in `ports`), ADR-043 (the `TraversalView` ADT + the pure `viewer-domain` projection + `page = chrome + fragment` parity), ADR-037 (the slice-08 render-only-CLI-command precedent: `SEARCH_FOLLOW_GUIDANCE_PREFIX` + `render_follow_guidance` + the bare-DID strip), ADR-030 (the read-only `StoreReadPort` with NO mutation method), ADR-028 (the loopback bind + the store-readability startup probe), ADR-014 (the slice-03 soft-remove sets `removed_at`), ADR-009 (Hexagonal + Modular Monolith), ADR-007 (functional paradigm)

## Context

slice-15 adds the federation-management VIEWING surface: a net-new read-only
`GET /peers` route that lists the operator's ACTIVE peer subscriptions
(`peer_subscriptions.removed_at IS NULL`), each row showing the peer's DID
verbatim, its PER-PEER local claim count
(`COUNT(*) FROM peer_claims WHERE author_did = <peer>`), and a render-only
`openlore peer remove <bare-did>` revocation command. When the operator follows
no one, a guided empty state points to `openlore peer add <did>`. DISCUSS is
APPROVED (DoR 9/9); J-003c is the validated job (the VIEWING side of
"subscription is revocable without residue"). Three DESIGN-owned questions remain
(WD-PS-5 explicitly defers them), each with a real downside if decided wrong:

1. **The per-peer count is an N+1 trap.** The naive shape — list active
   subscriptions, then call the existing `count_peer_claims(conn, peer_did)` free
   fn once per peer — issues N+1 queries, growing with the subscription count.
   The slice-10/12 single-query-per-render discipline (R-PS-2, a HIGH risk) makes
   ONE aggregate query a HARD product commitment (I-PS-8 / NFR-PS-4). DESIGN must
   choose the aggregate SQL and structurally foreclose the fold.

2. **The DTO has two plausible homes.** `PeerSubscriptionSummary` (the
   active-subscription + count row) could live in `ports` beside the other read
   row DTOs, or in `viewer-domain`. The choice affects which crates can share it
   without a dependency edge.

3. **The revocation command must be render-only.** The viewer is read-only and
   holds no key (I-PS-1, CARDINAL); the `openlore peer remove <did>` affordance
   must be plain TEXT, never an executable control (I-PS-8). slice-08 already
   solved the symmetric problem (the render-only `openlore peer add <did>` follow
   guidance). DESIGN must decide whether to reuse that pattern or invent a new
   one — and where the command text lives (R-PS-4, a MEDIUM risk: the command
   misread as a button).

The cardinals at stake: read-only/no-key (I-PS-1), active-only / residue made
visible (I-PS-2), per-peer never merged (I-PS-3 / J-003a), LOCAL-only (I-PS-4),
no N+1 (I-PS-8), no new crate (I-PS-7).

## Decision

### D1 — ONE aggregate query: `LEFT JOIN peer_claims … GROUP BY`, active-only

`StoreReadPort` gains ONE read-only method:

```rust
fn list_active_peer_subscriptions(
    &self,
) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>;
```

implemented in `adapter-duckdb` over the SAME shared connection (BR-VIEW-4) as
ONE aggregate query:

```sql
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

- **`LEFT JOIN` (not inner JOIN):** keeps a subscribed-but-never-pulled peer in
  the result with `local_claim_count = 0` (US-PS-002 Ex 2). An inner JOIN would
  silently drop it.
- **`COUNT(pc.cid)` (not `COUNT(*)`):** a LEFT-JOIN row with no `peer_claims`
  match has `pc.cid = NULL`; `COUNT` over NULL counts 0, so a never-pulled peer
  reports 0, not 1.
- **`WHERE ps.removed_at IS NULL`:** active-only (I-PS-2) — the exact mirror of
  the write-side `list_active_subscriptions` filter; a soft-removed/purged peer
  never appears (the residue-made-visible promise rendered as an absence).
- **`GROUP BY` per subscription identity + `author_did = peer_did` match:** the
  count is PER-PEER (I-PS-3 / J-003a); two peers → two rows with their own
  counts, never a merged total, never a "consensus" row.
- **ONE query, invariant to peer count:** the per-peer `count_peer_claims` fold
  (N+1) is REJECTED (I-PS-8).

### D2 — `PeerSubscriptionSummary` lives in `ports`

The flat read DTO lives in `crates/ports/src/store_read.rs` beside `ClaimRow` /
`PeerClaimRow` / `SurveyRow` (the slice-10 precedent, ADR-042):

```rust
pub struct PeerSubscriptionSummary {
    pub peer_did: String,              // NON-Option (anti-merging) — rendered VERBATIM
    pub peer_handle: String,
    pub subscribed_at: DateTime<Utc>,
    pub local_claim_count: u64,
}
```

It is read-side boundary data returned by a `StoreReadPort` method; `peer_did` is
NON-Option (the load-bearing anti-merging attribution). Living in `ports` keeps
the port self-contained and lets `adapter-duckdb` (producer) and
`adapter-http-viewer` (consumer) share it without a `viewer-domain` dependency.

### D3 — Render-only revocation command reuses the slice-08 pattern

A new `PEER_REMOVE_GUIDANCE_PREFIX` const + a `render_remove_guidance(peer_did)`
fn in `viewer-domain` mirror `SEARCH_FOLLOW_GUIDANCE_PREFIX` +
`render_follow_guidance` (ADR-037) VERBATIM in shape: the bare-DID strip
(`peer_did.split('#').next()`) + a render-only `<p>`/`<code>` of TEXT — never an
`<a>` that executes, never a `<form>`, never an `hx-*` mutation. The empty-state
`openlore peer add <did>` command uses a sibling `PEER_ADD_GUIDANCE_PREFIX` (the
slice-08 "openlore peer add" wording is the exact precedent). Each command text
is held in ONE place (single mutation site), keeping it consistent with the
slice-03 verbs.

### D4 — xtask check-arch boundary UNCHANGED

`/peers` is a read-only DB read like every slice-06..14 viewer read. No
`check-arch` delta: (a) the viewer capability rule (`VIEWER_FORBIDDEN_DEPS`) is
UNCHANGED (no signing/identity/PDS/indexer surface); (b) the pure-core no-I/O arm
for `viewer-domain` is UNCHANGED — NO new dependency edge (the render is a total
fn of the flat DTO, unlike slice-10's `viewer-domain → claim-domain` edge); (c)
the anti-merging SQL rule (`no_cross_table_join_elides_author`) stays GREEN by
construction — the SQL names `peer_subscriptions` + `peer_claims` but NOT the
standalone `claims` table, so `classify_sql_literal` returns `None` (the
word-boundary classifier excludes `peer_claims` from counting as `claims`). The
SQL projects `peer_did`/`author_did` anyway, so the rule would hold even if it
applied.

## Alternatives Considered

### For D1 (the per-peer count SQL)

- **Per-peer `count_peer_claims(conn, peer_did)` fold (N+1).** REJECTED — issues
  one query per peer, growing with the subscription count, violating the
  single-query discipline (I-PS-8 / R-PS-2). The very fold WD-PS-5 names as the
  thing to avoid.
- **Correlated subquery** (`SELECT ps.*, (SELECT count(*) FROM peer_claims WHERE
  author_did = ps.peer_did) … FROM peer_subscriptions ps WHERE removed_at IS
  NULL`). VIABLE — it is also ONE query and equally correct (and equally
  handles the zero-claims peer, since a scalar `count(*)` of no rows is 0). Not
  chosen because the `LEFT JOIN` + `GROUP BY` expresses the per-peer count as a
  single aggregate over a single scan and reads as the natural whole-set lift of
  the existing `count_peer_claims(conn, peer_did)` shape (one `COUNT`, one
  predicate `author_did = peer_did`). The two are behaviorally equivalent for the
  AC; this ADR fixes the JOIN form and DELIVER may swap to the correlated
  subquery if a query plan favors it — the contract (ONE aggregate query,
  active-only, per-peer, zero-safe) is the fixed point.

### For D2 (the DTO home)

- **`PeerSubscriptionSummary` in `viewer-domain`.** REJECTED — it is the return
  type of a `StoreReadPort` method, so `ports` must reference it regardless; the
  adapter would then need a `viewer-domain` edge to produce it, inverting the
  dependency direction (adapter → pure render core). Placing it in `ports`
  matches every other read row DTO (ADR-042) and keeps dependencies inward.
- **Reuse the write-side `PeerSubscription`** (`{ peer_did, peer_handle,
  peer_pds_endpoint, subscribed_at, removed_at }`). REJECTED — it carries no
  count, carries fields the view does not need (`peer_pds_endpoint`,
  `removed_at`), and lives on the WRITE port (`PeerStoragePort`); the read-only
  viewer must not depend on the write port. A dedicated FLAT read DTO is the
  slice-10 precedent.

### For D3 (the revocation command)

- **An executable unsubscribe control** (a `<form>`/`<button>` POSTing a remove).
  REJECTED — violates the CARDINAL read-only/no-key invariant (I-PS-1); the
  viewer holds no key and performs no mutation. Subscribe/unsubscribe stays
  EXCLUSIVELY the slice-03 CLI.
- **A new, bespoke render-only-command helper.** REJECTED — slice-08
  `render_follow_guidance` already solved the identical problem (render-only CLI
  command TEXT with a bare-DID strip), is already vetted by the slice-08
  read-only gold, and reusing its shape keeps the two surfaces consistent and the
  command text in one place per verb.

### For D4 (the xtask boundary)

- **Add a pure-core allowlist edge (as slice-10 did).** REJECTED as unnecessary —
  the render needs no pure dependency; it is a total fn of the flat DTO.
- **Add a capability-rule entry.** REJECTED — the read touches no
  signing/identity/PDS/indexer surface; the existing read-only boundary covers
  it.

## Consequences

### Positive

- **No N+1 by construction** — ONE aggregate query, invariant to peer count; the
  behavioral gold (query count invariant to N) holds (I-PS-8 / NFR-PS-4).
- **Active-only is the rendering of the residue-free promise** — the SQL `WHERE
  removed_at IS NULL` means a removed peer's absence IS the J-003c guarantee made
  visible (I-PS-2); a behavioral gold seeds a soft-removed peer and asserts its
  absence.
- **Per-peer, never merged** — the count is grouped per peer and matched on
  `author_did`; the two-peers-distinct-counts gold (5 vs 3, never 8) holds
  (I-PS-3 / J-003a).
- **Read-only / no-key preserved** — the new method is a `SELECT` on a port with
  no mutation method; the command is render-only TEXT; the read-only gold holds
  (I-PS-1).
- **Reuse-first** — the DTO mirrors the slice-10 read DTO; the command reuses the
  slice-08 pattern; the route mirrors `/peer-claims`/`/project`; no new crate, no
  new dependency edge, no new external dependency, no xtask delta.
- **Zero-claims peer handled correctly** — the LEFT JOIN + `COUNT(pc.cid)` keeps
  a subscribed-but-never-pulled peer at count 0 (US-PS-002 Ex 2).

### Negative / trade-offs

- **The `LEFT JOIN` over the whole `peer_claims` table is wider than a scalar
  subquery per active peer.** For a dogfood-scale local store (a handful of peers,
  hundreds-to-thousands of claims) this is negligible and still ONE query; at much
  larger scale the correlated-subquery alternative (D1) is a drop-in swap with the
  same contract. Accepted — the local-first scale makes the JOIN the simpler,
  more readable form.
- **`peer_handle` is carried in the DTO but the AC does not require it be
  displayed.** Mild over-fetch (one already-present column). Accepted — it mirrors
  `PeerSubscription` and gives DELIVER the option to show the handle without a
  schema/DTO change.
- **The empty-state `peer add` command shows a `<did>` placeholder** (the operator
  follows no one, so there is no concrete DID to suggest). Accepted — it is
  onboarding guidance, not a per-row command; the placeholder is the slice-08
  follow-guidance precedent for the no-target case.

### Earned-Trust (principle 12)

No new adapter, port, or external dependency with its own substrate is
introduced. The read runs over the EXISTING, already-probed `StoreReadPort`
DuckDB connection (the ADR-028/030 store-readability probe — "wire then probe
then use" — already gates viewer startup). The read is a pure `SELECT` over
tables the migrations already create; nothing new can lie, so no new `probe()`
scenario is required and `cargo xtask check-probes` is UNCHANGED.

## Confirmation

- Read method: `list_active_peer_subscriptions(&self) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>` on `StoreReadPort` (read-only — no mutation method added).
- SQL: `peer_subscriptions LEFT JOIN peer_claims ON author_did = peer_did`, `WHERE removed_at IS NULL`, `GROUP BY` the subscription identity, `COUNT(pc.cid)` — ONE aggregate query (no N+1).
- DTO: `ports::PeerSubscriptionSummary { peer_did (non-Option), peer_handle, subscribed_at, local_claim_count: u64 }`.
- ADT: `viewer_domain::PeersView` — `Subscriptions { peers } | NoSubscriptions`.
- Render-only command: `PEER_REMOVE_GUIDANCE_PREFIX` + `render_remove_guidance(peer_did)` (bare-DID strip); empty-state `PEER_ADD_GUIDANCE_PREFIX`.
- xtask check-arch: UNCHANGED.
- NO new crate; workspace stays 21 members; NO new dependency edge; NO new external dependency.
