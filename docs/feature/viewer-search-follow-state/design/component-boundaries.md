# Component Boundaries — viewer-search-follow-state (slice-16)

> Companion to architecture-design.md. The 2 touched crates, the additions to
> each, the 5 context crates that stay UNCHANGED, and the cross-component
> invariants. **No new crate; 21 members. No new dependency edge. No new route.
> No new read method. No new `AuthorRelationship` variant.**

## Touchpoint map (the established slice-06..15 set; slice-16 touches 2)

```
adapter-http-viewer (EFFECT, driving)
  │  resolve_search_state:
  │    1. index_query.search(…)  ───────────▶  Network index (per-user-neutral, UNCHANGED)
  │    2. store.list_active_peer_subscriptions()  ──▶  ports::StoreReadPort  ◀── impl ── adapter-duckdb (UNCHANGED)
  │         └─ Ok → HashSet<bare peer_did> ; Err → EMPTY set (degrade)                     │ slice-15 SELECT (reused)
  │    3. raw.results.map(to_indexed_claim(row, &set))  ── resolve per row IN MEMORY      ▼
  │    4. compose_results(claims, dimension)  ──▶  appview-domain (PURE, UNCHANGED)   LOCAL DuckDB store
  ▼  render (Shape fork)                              └─ carries relationship verbatim   (peer_subscriptions, removed_at IS NULL)
viewer-domain (PURE)
  render_search_result_row(&NetworkResultRow) ── match relationship {
     NetworkUnfollowed → render_follow_guidance  (slice-08, UNCHANGED)
     SubscribedPeer    → render_following_indicator  (NEW arm, total fn of existing row)
     _                 → no affordance
  } ──▶ HTML
```

Dependency direction is inward (ADR-009). No adapter depends on another adapter.
Only `cli` links the adapters (composition root, UNCHANGED — the viewer already
holds both `IndexQueryPort` and `StoreReadPort`). **No new pure→pure edge** is
introduced: the new render arm consumes the existing `appview_domain::NetworkResultRow`.

---

## `crates/adapter-http-viewer` — the resolution seam (EFFECT, driving) — CHANGED

**Changes** `resolve_search_state` (~line 884) + `to_indexed_claim` (~line 1021):

```text
// resolve_search_state — the Ok(raw) arm (the ONLY change to the existing flow):
Ok(raw) => {
    // slice-16 (US-SF-001 / ADR-053): read the operator's LOCAL active-subscription
    // set ONCE per render (REUSED slice-15 list_active_peer_subscriptions — NO new
    // read method, NO new SQL, NO network), materialize the bare peer_dids into a
    // HashSet<String>. A read FAILURE degrades to an EMPTY set → every author resolves
    // to NetworkUnfollowed (the slice-08 status quo; C-7 / WD-SF-6) — NO crash, NO 5xx.
    let active: HashSet<String> = match store.list_active_peer_subscriptions() {
        Ok(summaries) => summaries.into_iter().map(|s| s.peer_did).collect(),
        Err(_) => HashSet::new(),
    };
    // Resolve each author IN MEMORY against `active` (no N+1; C-4). The relationship
    // is threaded into to_indexed_claim (which STOPS hardcoding NetworkUnfollowed).
    let claims = raw.results.into_iter()
        .map(|row| to_indexed_claim(row, &active))
        .collect();
    let result: NetworkSearchResult = compose_results(claims, dimension);
    SearchState::Results { result, dimension }
}

// to_indexed_claim — the relationship is now RESOLVED, not hardcoded:
fn to_indexed_claim(row: NetworkResultRowRaw, active: &HashSet<String>) -> IndexedClaim {
    let relationship = if active.contains(bare_did(&row.author_did.0)) {
        AuthorRelationship::SubscribedPeer
    } else {
        AuthorRelationship::NetworkUnfollowed   // binary (C-6); You/UnsubscribedCache not resolved
    };
    IndexedClaim { /* … all other fields byte-equal unchanged … */, relationship }
}
```

> The `resolve_search_state` signature gains the `store: &dyn StoreReadPort`
> already available to the handler (the viewer holds it; `search_page` passes it
> through). The exact plumbing (param vs a captured-set closure) is a DELIVER
> shape choice with identical observable behavior (ADR-053 Q2). `bare_did` is the
> existing adapter-side mirror of the `viewer-domain` SSOT.

**Boundary / invariants:**
- **ONE active-set read per render, invariant to result count** (C-4 / no N+1).
  The read is the slice-15 single-aggregate query; resolution is in memory.
- **Fragment-strip on both sides** (R-SF-5): the active set's `peer_did` is
  already bare; the result's `author_did` may carry `#org.openlore.application` —
  stripped via `bare_did` before `HashSet::contains`.
- **Binary resolution** (C-6): `SubscribedPeer` ∨ `NetworkUnfollowed`; `You` /
  `UnsubscribedCache` never produced here.
- **Graceful degradation** (C-7): a read `Err` → empty set → all
  `NetworkUnfollowed`; the existing index-query outcomes
  (`Unavailable`/`NoResults`/`Form`) are UNCHANGED.
- **No re-grouping / re-ranking** (C-5): only the per-row `relationship` is set;
  `compose_results` is called exactly as in slice-08.
- **Read-only**: reads `IndexQueryPort` + `StoreReadPort`, neither with a
  mutation method; holds no key. `async` (the `.await` on the index query is
  UNCHANGED; the active-set read is a synchronous LOCAL call).

---

## `crates/viewer-domain` — the render arm (PURE core) — CHANGED

**Adds** the indicator const + helper, and ONE arm to `render_search_result_row`
(~line 1719):

```text
/// The render-only neutral "Following" indicator a SubscribedPeer search-result row
/// carries (slice-16 / US-SF-002 / ADR-053): the sibling of SEARCH_FOLLOW_GUIDANCE_PREFIX.
/// It is a NEUTRAL LABEL — no command, no verb-phrase, no DID — distinguishing an
/// already-followed author from an unfollowed one. NO executable control; the read-only
/// viewer holds no key and the follow stays the slice-03 CLI. Held in ONE place (single
/// mutation site).
pub const SEARCH_FOLLOWING_INDICATOR: &str = "Following";

/// Render the render-only "Following" indicator for an already-followed network author
/// (slice-16 / US-SF-002) as plain TEXT inside a <p>/<span> — NEVER an <a>/<button>/
/// <form>/hx-* control. PURE total function. The mirror of render_follow_guidance for
/// the SubscribedPeer arm.
fn render_following_indicator() -> Markup {
    html! { " " p { (SEARCH_FOLLOWING_INDICATOR) } }
}

// render_search_result_row — the relationship branch becomes two-armed:
@if matches!(row.relationship, AuthorRelationship::NetworkUnfollowed) {
    (render_follow_guidance(&row.author_did.0))   // slice-08, UNCHANGED
} @else if matches!(row.relationship, AuthorRelationship::SubscribedPeer) {
    (render_following_indicator())                 // NEW arm — neutral "Following" TEXT
}
// You / UnsubscribedCache: no affordance (never arise on /search, ADR-053 D2)
```

> The branch is equivalently a total `match row.relationship { … }` (DELIVER may
> write it either way); the load-bearing contract is: `SubscribedPeer` → neutral
> "Following" + NO `peer add`; `NetworkUnfollowed` → the unchanged slice-08
> command. The render input is `appview_domain::NetworkResultRow`, which already
> carries `relationship` (no new field).

**Boundary / invariants:**
- PURE — no I/O. **NO new `[dependencies]` edge** (the arm is a total fn of the
  existing `NetworkResultRow`; it needs no new pure-core import).
- Both affordances are `<p>`/`<span>`/`<code>` of TEXT — never an executable
  control (C-1 / I-NS-1). No `<button>`, no `<form>`, no mutating `<a>`, no
  mutating `hx-*`.
- The "Following" copy lives in ONE const; `render_follow_guidance` +
  `SEARCH_FOLLOW_GUIDANCE_PREFIX` + `bare_did` are REUSED verbatim (unchanged).
- Parity: `render_search_page` EMBEDS `render_search_results_fragment` (the
  resolved `relationship` renders identically in both shapes, C-8) — UNCHANGED.
- Attribution + grouping + the `[verified]` marker + verbatim confidence +
  counter-annotation are UNCHANGED (the arm is additive per-row, C-5).

---

## `crates/appview-domain` — PURE — UNCHANGED

`compose_results` (`compose.rs` ~line 33) + `to_result_row` (~line 116) already
carry `claim.relationship` verbatim into `NetworkResultRow.relationship`
(~line 126) and group per author without merge/re-rank. slice-16 needs NO change
here — the resolved relationship flows through the EXISTING pipeline.

## `crates/ports` — PURE port traits — UNCHANGED

`AuthorRelationship` (`federated_row.rs` ~line 67 — 4 variants) and
`StoreReadPort::list_active_peer_subscriptions` (slice-15) BOTH already exist. No
new variant, no new method, no new DTO. The ~line 58 doc comment already states
the relationship is resolved viewer-side against `peer_subscriptions` — slice-16
realizes exactly that.

## `crates/adapter-duckdb` — EFFECT (driven) — UNCHANGED

The slice-15 active-subscription SELECT (`removed_at IS NULL`, ONE aggregate) is
REUSED verbatim. No new SQL, no new read impl, no schema/migration change.

## `crates/xtask` — enforcement (tooling) — UNCHANGED

NO change. The resolution reuses the `StoreReadPort` the viewer already holds and
the new render arm is a total fn of the existing `NetworkResultRow`:
- The viewer capability rule (`VIEWER_FORBIDDEN_DEPS`) is UNCHANGED — no
  signing/identity/PDS/indexer-mutation surface touched.
- The pure-core no-I/O arm for `viewer-domain` is UNCHANGED — no new dependency
  edge (the arm consumes the existing `NetworkResultRow`).
- The anti-merging SQL rule (`no_cross_table_join_elides_author`) is N/A —
  slice-16 adds NO SQL (it reuses the slice-15 query).

## `crates/cli` — composition root — UNCHANGED

No change. The viewer already holds both `IndexQueryPort` and `StoreReadPort`
(the latter wired since slice-15 for `/peers`); `resolve_search_state` reads the
store the handler already passes.

---

## Cross-component invariants (DELIVER must keep green)

1. **Read-only / no key (C-1, CARDINAL):** the viewer holds `StoreReadPort` +
   `IndexQueryPort`, neither with a mutation method; capability rule unchanged;
   behavioral gold: neither affordance is an executable control.
2. **Accuracy (C-2, load-bearing):** seeded-followed → "Following" + no add;
   unfollowed → keeps add; all-followed → no add anywhere; none-followed →
   status quo. Behavioral gold per case.
3. **LOCAL/offline (C-3):** resolution reads only `StoreReadPort`; the index
   query is per-user-neutral + unchanged; no-network-for-resolution scenario.
4. **ONE batch read / no N+1 (C-4):** the active set is read ONCE into a
   `HashSet`; resolution in memory; read-count-invariant-to-result-count gold.
5. **Attribution + ranking unchanged (C-5 / J-003a):** only `relationship` set;
   `compose_results` unchanged; grouping/order-identical-with-and-without-an-
   active-subscription gold.
6. **Binary; `You`/`UnsubscribedCache` not resolved (C-6):** only
   `SubscribedPeer`/`NetworkUnfollowed`; soft-removed-peer→`peer add` gold.
7. **Graceful degradation (C-7):** read `Err` → empty set → all
   `NetworkUnfollowed`; failed-read-degrades-without-crash gold.
8. **Parity (C-8):** `render_search_page` embeds the SAME
   `render_search_results_fragment`; htmx-vs-no-JS-parity gold.
9. **Fragment-strip match (R-SF-5):** `bare_did` on both sides before membership;
   fragmented-result-DID-matches-bare-active-DID gold.
10. **No new crate; 21 members; no new dependency edge; no new route; no new read
    method; no new `AuthorRelationship` variant.**
