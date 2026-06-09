# Data Models: viewer-landing-dashboard (slice-17)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-054
> Functional paradigm (ADR-007): the view-model is an immutable value; the render is
> a total function of it. NO new PERSISTED type (the summary is computed per-request,
> never stored — I-LD-6 / WD-LD-10).

## 1. `LandingSummary` — the front-door view-model (Option-shaped, per-count degrade)

```rust
/// The three at-a-glance LOCAL store counts for GET /. Each count is the OUTCOME of
/// one aggregate read, modelled as Option<usize>:
///   Some(n) = a SUCCESSFUL read of n (Some(0) is an honest empty store)
///   None    = that count's read FAILED (a missing-number state — renders "—")
/// `0 ≠ missing` is therefore a TYPE-LEVEL distinction: a fabricated 0 on failure is
/// UNREPRESENTABLE (the only way to Some(0) is a read that returned 0). The three
/// counts degrade INDEPENDENTLY — one None does not affect the other two.
/// Computed per-request in the effect shell; never persisted.
pub struct LandingSummary {
    pub own_claims: Option<usize>,    // count_claims()
    pub peer_claims: Option<usize>,   // count_peer_claims()
    pub active_peers: Option<usize>,  // count_active_peer_subscriptions()
}
```

- This is a DESIGN-owned shape (the DISCUSS Technical Notes deferred "Option-per-count
  vs a small ADT" to DESIGN). Chosen: a FLAT struct of three identical `Option<usize>`
  fields, because the three counts have identical shape and identical degrade
  semantics — three parallel `Option`s is the total, exhaustive, mutation-resistant
  model with the least ceremony (ADR-054 D1; the richer per-count ADT was rejected as
  adding names without adding states).

### State table (each field, independently)

| Field value | Meaning | Renders as |
|---|---|---|
| `Some(0)` | successful read, store has none | `0` (e.g. "0 own claims") — an honest empty store |
| `Some(n)`, n>0 | successful read | `n` (e.g. "12 own claims") |
| `None` | read FAILED (transient `StoreReadError`) | the missing-number marker `"—"` — DISTINCT from `0` |

The eight (2³) combinations are all valid and all render totally (no panic, no I/O).
Domain examples (US-LD-000/001): `{Some(12), Some(7), Some(2)}` (happy path);
`{Some(0), Some(0), Some(0)}` (honest empty store); `{Some(12), None, Some(2)}`
(peer-claims read failed → "—" for peer claims, the other two still shown).

## 2. The missing-number rendering (missing ≠ zero)

The pure render maps each field totally:

```
Some(n) → render the number n
None    → render MISSING_COUNT_MARKER   (a new const in viewer-domain, e.g. "—")
```

`MISSING_COUNT_MARKER` is held as a ONE-place const (mutation has one site to attack,
mirroring `READ_ONLY_NOTICE` / `CLAIM_NOT_FOUND_NOTICE`). The marker is visually and
semantically distinct from the digit `0`. This realizes BR-LD-3 / WD-LD-8: "—" means
"couldn't read this"; "0" means "your store has none". A fabricated 0 on a failed
read is forbidden — and unrepresentable, since the shell maps a failed read to `None`,
never `Some(0)`.

## 3. The Result → Option mapping (effect shell)

The three reads each return `Result<usize, StoreReadError>`. The shell maps each to
`Option<usize>` via `.ok()` INDEPENDENTLY (ADR-054 D2):

```
own_claims   = store.count_claims().ok()
peer_claims  = store.count_peer_claims().ok()
active_peers = store.count_active_peer_subscriptions().ok()
```

`.ok()` discards the `StoreReadError` cause (never echoed — NFR-VIEW-6), turning
`Err` into `None`. This is the slice-12 `unwrap_or_default()` degrade generalized to
`Option` (so a failure is "missing", not "default 0"). No `Result`, no error, and no
store reach the pure core.

## 4. The navigation hub — URL consts (no new persisted data)

The hub is rendered from the 8 route URL consts in `viewer-domain` — 7 existing + 1
minted this slice (ADR-054 D4):

| Surface | Const | Status |
|---|---|---|
| My Claims | `MY_CLAIMS_URL` (`/claims`) | existing ~229 |
| Peer Claims | `PEER_CLAIMS_URL` (`/peer-claims`) | existing ~234 |
| Network Search | `SEARCH_URL` (`/search`) | existing ~1457 |
| Contributor Score | `SCORE_URL` (`/score`) | existing ~1833 |
| Project Survey | `PROJECT_URL` (`/project`) | existing ~2137 |
| Philosophy Survey | `PHILOSOPHY_URL` (`/philosophy`) | existing ~2142 |
| Peer Subscriptions | `PEERS_URL` (`/peers`) | existing ~2665 |
| Live Scrape | `SCRAPE_URL` (`/scrape`) | **NEW this slice** |

Each hub link is `a href=(CONST) { "<label>" }` — a plain, no-JS-navigable anchor
(the `render_tab_nav` real-`<a href>` precedent; htmx enhancement optional, never
required — the no-JS link is the contract, C-5). No deep/parameterized route
(`/claims/{cid}`, `/score?contributor`, `/project?subject`, `/philosophy?object`) is
a top-level hub link (FR-LD-5). `HTMX_ASSET_URL` and any internal/asset route are NOT
linked (BR-LD-4).

## 5. Persistence / storage

NONE. The `LandingSummary` is a transient per-request value (I-LD-6 / WD-LD-10): no
new table, no new column, no new persisted type. The three reads are SELECTs over the
existing shared connection (BR-VIEW-4); the active-subs `COUNT(*)` reads the existing
`peer_subscriptions` table (`removed_at IS NULL`, the slice-15 active-only definition,
BR-LD-2). The bind stays 127.0.0.1.
