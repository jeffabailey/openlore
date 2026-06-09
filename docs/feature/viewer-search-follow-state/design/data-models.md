# Data Models — viewer-search-follow-state (slice-16)

> Companion to architecture-design.md / component-boundaries.md. The
> relationship-resolution flow, the in-memory active-DID set, the reused enum +
> read DTO, and the render-only "Following" indicator SSOT. **Zero new persisted
> type** (the active set is read + resolved per request; nothing is written).

## 1. Reused types (NO new type introduced)

slice-16 introduces NO new persisted type, NO new boundary DTO, and NO new enum
variant. It REUSES, verbatim:

| Type | Crate | Origin | slice-16 use |
|---|---|---|---|
| `AuthorRelationship` (`You \| SubscribedPeer \| UnsubscribedCache \| NetworkUnfollowed`) | `ports::federated_row` | slice-03/05 | the per-row `relationship`; slice-16 produces only `SubscribedPeer` / `NetworkUnfollowed` (binary, C-6 / WD-SF-5) |
| `PeerSubscriptionSummary { peer_did: String (bare, non-Option), … }` | `ports::store_read` | slice-15 (ADR-052) | the active-set read row; `peer_did` is the bare DID materialized into the resolution set |
| `IndexedClaim { …, relationship: AuthorRelationship }` | `appview-domain` | slice-05 | `to_indexed_claim` now SETS `relationship` from resolution (was hardcoded) |
| `NetworkResultRow { …, relationship: AuthorRelationship }` | `appview-domain` (~line 126/136) | slice-08 | `compose_results` carries `relationship` verbatim; the render branches on it |
| `SearchState` ADT (`Form \| Results \| NoResults \| Unavailable`) | `viewer-domain` | slice-08 (ADR-037) | UNCHANGED — the render input; the `Results` arm now carries resolved relationships |

> The relationship was ALREADY threaded end-to-end (`to_indexed_claim` →
> `compose_results.to_result_row` ~line 126 → `render_search_result_row`). The
> ONLY hardcode was at the SOURCE (`to_indexed_claim` always
> `NetworkUnfollowed`). slice-16 replaces that source with a resolved value; the
> downstream pipeline is unchanged.

## 2. The in-memory active-DID set (transient, per-request)

```text
// adapter-http-viewer :: resolve_search_state — read ONCE per render, transient:
let active: HashSet<String> =
    store.list_active_peer_subscriptions()        // REUSED slice-15 read (ONE aggregate)
        .map(|summaries| summaries.into_iter().map(|s| s.peer_did).collect())
        .unwrap_or_default();                      // Err → EMPTY set (degrade, C-7)
```

- `HashSet<String>` of **bare** DIDs (the slice-15 `peer_did` is already bare —
  `did:plc:…`, non-Option).
- Built ONCE per `/search` render, invariant to result count (C-4 / no N+1). It
  is `O(1)`-membership; resolution is `O(result_rows)` lookups in memory.
- **Transient** — never persisted, never serialized, dropped at the end of the
  request (zero new persisted type; WD-SF-9).
- A read failure yields an EMPTY set (`unwrap_or_default`), so every author
  resolves to `NetworkUnfollowed` (the slice-08 status quo; C-7 / WD-SF-6).

## 3. Relationship-resolution flow (per result row, PURE in memory)

```text
// the resolution rule (a small total fn; ADR-053 D1):
fn resolve_relationship(author_did: &str, active: &HashSet<String>) -> AuthorRelationship {
    if active.contains(bare_did(author_did)) {     // bare_did strips #org.openlore.application (R-SF-5)
        AuthorRelationship::SubscribedPeer
    } else {
        AuthorRelationship::NetworkUnfollowed       // binary (C-6); You/UnsubscribedCache NOT produced
    }
}
```

- **Fragment strip on BOTH sides** (R-SF-5): the active set's `peer_did` is
  already bare; the result's `author_did` may carry the
  `#org.openlore.application` signing fragment — reduced via the existing
  `bare_did` SSOT (`viewer-domain` ~line 2566 / the adapter mirror) before
  membership. So `did:plc:rachel-test#org.openlore.application` matches the bare
  `did:plc:rachel-test` in the set.
- **Binary** (C-6 / WD-SF-5): exactly `SubscribedPeer` ∨ `NetworkUnfollowed`. A
  soft-removed peer is absent from the active set (the slice-15 read filters
  `removed_at IS NULL`) → `NetworkUnfollowed` (correctly re-offered). `You` is
  DEFERRED (an own-author result → `NetworkUnfollowed`, ADR-053 D2);
  `UnsubscribedCache` is never produced on `/search`.
- **Per-row enrichment only** (C-5): resolution sets `IndexedClaim.relationship`;
  `compose_results` then groups per author + carries it verbatim — NO merge, NO
  re-group, NO re-rank, NO change to the `[verified]` marker / confidence /
  counter-annotation.

### Worked examples (from the journey)

**US-SF-001 Ex 1 / US-SF-002 Ex 1 — one followed, one unfollowed, one batch read:**
Maria follows `did:plc:rachel-test`. A search returns rows by
`did:plc:rachel-test#org.openlore.application` and
`did:plc:priya-test#org.openlore.application`. The shell reads the active set
ONCE → `{ "did:plc:rachel-test" }`. Rachel's `author_did` strips to
`did:plc:rachel-test` ∈ set → `SubscribedPeer` → "Following", no command.
Priya's strips to `did:plc:priya-test` ∉ set → `NetworkUnfollowed` →
`openlore peer add did:plc:priya-test`. ONE read, both resolved in memory.

**US-SF-001 Ex 2 — fragmented result DID vs bare active DID:**
Active set holds bare `did:plc:rachel-test`; the result row's `author_did` is
`did:plc:rachel-test#org.openlore.application`. `bare_did` strips the fragment →
byte-equal match → `SubscribedPeer` (never misclassified as `NetworkUnfollowed`).

**US-SF-001 Ex 3 / US-SF-002 Ex 3 — read failure / none followed:**
The LOCAL active-set read errors (or the operator follows nobody in the results)
→ EMPTY set → every author `NetworkUnfollowed` → every row keeps the
`openlore peer add <did>` guidance (exactly the slice-08 behavior). No crash, no
blank, no leak.

**US-SF-002 Ex 2 — all followed:**
Maria follows both `did:plc:rachel-test` and `did:plc:tobias-test`; the results
are only by those two. Both strip-match the set → both `SubscribedPeer` → every
row shows "Following"; NO `peer add` appears anywhere.

## 4. The "Following" indicator SSOT (render-only TEXT, single source of truth)

```text
// viewer-domain — held in ONE place (mirrors SEARCH_FOLLOW_GUIDANCE_PREFIX):
SEARCH_FOLLOWING_INDICATOR = "Following"
```

`render_following_indicator()` emits `<p>{SEARCH_FOLLOWING_INDICATOR}</p>` (or a
`<span>`) — a NEUTRAL render-only LABEL: no command, no verb-phrase, no DID,
distinct from the follow guidance. It is plain TEXT, never an
`<a>`/`<button>`/`<form>`/`hx-*` control (C-1). The `NetworkUnfollowed` arm
REUSES `render_follow_guidance` + `SEARCH_FOLLOW_GUIDANCE_PREFIX` ("Follow this
author from the CLI: openlore peer add") + `bare_did` verbatim (unchanged). The
exact markup element is a DELIVER detail; the load-bearing contract: the
"Following" copy is the single source of truth, the indicator is render-only
TEXT carrying NO command.

### Render mapping (per row, by resolved relationship)

| Resolved `relationship` | Render | Source |
|---|---|---|
| `SubscribedPeer` | `render_following_indicator()` → "Following" (neutral TEXT, no command) | NEW arm (slice-16) |
| `NetworkUnfollowed` | `render_follow_guidance(author_did)` → "… openlore peer add <bare-did>" (TEXT) | slice-08, UNCHANGED |
| `You` / `UnsubscribedCache` | (no affordance — never arises on `/search`, ADR-053 D2) | n/a |

## 5. Persistence

NONE. The active-DID set, the resolved `relationship`, the `IndexedClaim`s, the
`NetworkResultRow`s, and the `SearchState` are all computed per request and never
written (WD-SF-9). No schema change, no migration, no new table/column, no new
DTO, no new enum variant. The bind stays loopback 127.0.0.1.
