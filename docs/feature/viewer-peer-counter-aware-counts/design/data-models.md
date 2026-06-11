# Data Models: viewer-peer-counter-aware-counts (slice-19)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-10 · ADR: ADR-056
> Functional paradigm (ADR-007). NO new persisted type; NO schema change. The countered-peer
> count is computed per-request from the EXISTING slice-12 counter-reference tables + the slice-03
> `peer_claims` table; nothing new is stored.

## 1. The extended view-model — `LandingSummary` gains a FIFTH `Option<usize>` field

```rust
// crates/viewer-domain/src/lib.rs (extends the slice-17/18 struct, ADR-054 D1 → ADR-055 D2 → ADR-056 D2)
pub struct LandingSummary {
    pub own_claims: Option<usize>,             // count_claims                   — slice-17
    pub peer_claims: Option<usize>,            // count_peer_claims              — slice-17
    pub active_peers: Option<usize>,           // count_active_peer_subscriptions — slice-17
    pub countered_own_claims: Option<usize>,   // count_countered_own_claims     — slice-18
    pub countered_peer_claims: Option<usize>,  // count_countered_peer_claims    — slice-19 (additive)
}
```

- `Some(n)` = a SUCCESSFUL read of `n` (including `Some(0)` — an honest "nothing of my cached
  peer material has drawn a counter"). `None` = the countered-peer-count read FAILED → the
  slice-17 `MISSING_COUNT_MARKER` "—". `0 ≠ missing` stays a TYPE-LEVEL distinction; a fabricated
  0 on a failed read is unrepresentable (C-5 / WD-PC-6).
- The five counts degrade INDEPENDENTLY: a failed countered-peer read leaves the four siblings
  `Some(_)` (incl. the slice-18 `countered_own_claims`), and vice versa — the per-count `.ok()`
  model (ADR-054 D2 / ADR-055 D4) extended to a fifth field.
- The pure render is a TOTAL function of `LandingSummary` over all 2⁵ `Option` combinations.

## 2. The countered-peer count read — `count_countered_peer_claims` (WD-PC-5 RESOLVED → count-only aggregate)

`StoreReadPort` (read-only — NO mutation method):

```rust
fn count_countered_peer_claims(&self) -> Result<usize, StoreReadError>;
```

`adapter-duckdb` impl — ONE aggregate, NO bind parameters, injection-safe (the EXACT slice-18
`count_countered_own_claims` SQL with the OUTER table swapped `claims c → peer_claims p`):

```sql
SELECT COUNT(DISTINCT p.cid)
FROM peer_claims p
WHERE p.cid IN (
    SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters'
    UNION
    SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters'
)
```

| Property | How the SQL guarantees it | Cardinal |
|---|---|---|
| Presence count (a peer claim countered N times counts ONCE) | the inner `UNION` de-dupes the countered `referenced_cid` set; `p.cid IN (…)` is a MEMBERSHIP test (no per-counter row); `COUNT(DISTINCT p.cid)` double-defends — NO JOIN-fanout (an `IN`-subquery never multiplies outer rows). The inner IN-set is IDENTICAL to slice-18's. | C-4 / WD-PC-4 / R-PC-4 |
| Peer-only (own-claims-countered excluded) | the outer table is `peer_claims` (cached peer claims); a countered OWN claim is not in `peer_claims`, so it never contributes — excluded by query shape, not a relaxable filter | WD-PC-7 |
| Read-only / injection-safe | returns `Result<usize, _>`; no mutation method on the trait; NO bind parameters (literal `'counters'` only) | C-1 |
| Invariant to store size / no N+1 | ONE aggregate; both ref columns indexed (slice-12 ADR-048); the landing budget grows 4→5, `peer_claims_page` +1 | C-3 / WD-PC-3 |
| LOCAL only | reads the indexed ref tables + `peer_claims` over the shared connection; no network seam | C-2 |
| xtask anti-merging GREEN | names `peer_claims` (whole word, outer FROM) but NOT standalone `claims` → `is_cross_store` FALSE → classifier `None` (component-boundaries §5, R-PC-9 RESOLVED) | I-FED-1 |

The DATA is the slice-12 `counter_presence_for` data (`claim_references ∪ peer_claim_references`,
`ref_type='counters'`) — read as a COUNT rather than a presence SET. A cached peer claim is
countered by the OPERATOR (her counter, in `claim_references`) OR by ANOTHER PEER (their counter,
in `peer_claim_references` — slice-11). The `UNION` over both tables captures both origins; the
DISTINCT IN-set + outer DISTINCT count each such peer cid ONCE.

## 3. The countered-peer-count render — the REUSED `render_countered` helper (WD-PC-8 single source, WD-PC-10 no new helper)

```rust
// crates/viewer-domain/src/lib.rs ~640 — EXISTING slice-18 helper, REUSED verbatim (NO new helper)
pub fn render_countered(countered: Option<usize>) -> String {
    format!("({} countered)", render_count(countered))
    //   Some(n) → "(n countered)"   e.g. "(1 countered)", "(0 countered)"
    //   None    → "(— countered)"   (MISSING_COUNT_MARKER inside the parenthetical)
}
```

- **Landing (`render_landing`)** — rendered BESIDE the UNCHANGED peer-claims line. The current
  site is `p { (render_count(summary.peer_claims)) " peer claims" }` (~677); slice-19 extends it,
  EXACTLY mirroring how slice-18 extended the own line (~673-676):
  `(render_count(summary.peer_claims)) " peer claims " (render_countered(summary.countered_peer_claims))`
  → `"4 peer claims (1 countered)"`. The peer-claims "4" is verbatim (additive awareness, never a
  re-weight — C-4). The slice-18 own line is UNTOUCHED (WD-PC-7).
- **`/peer-claims` header (`render_peer_claims_page`)** — the fn gains a `countered_peer_claims:
  Option<usize>` param; the SAME `render_countered` output renders beside the "Peer Claims" `h1`
  (currently `h1 { "Peer Claims" }` ~1170 → `h1 { "Peer Claims " (render_countered(countered_peer_claims)) }`),
  EXACTLY mirroring the slice-18 `/claims` header (`h1 { "My Claims " (render_countered(countered_own_claims)) }`
  ~389). The list body (slice-06/07 ordering `composed_at DESC, cid`, paging, total count, every
  row's verbatim confidence, the slice-13 per-row flags, the peer origin) is UNTOUCHED (additive —
  C-4 / WD-PC-9). The `Shape::Fragment` htmx swap path
  (`render_peer_claims_view_panel_fragment`) is UNTOUCHED — the header count is full-page chrome.
- **Single source (WD-PC-8)**: both surfaces render the SAME number through the SAME helper; the
  copy cannot diverge. A gold test pins landing "(N countered)" == `/peer-claims` header "(N countered)".

### Missing ≠ zero / anti-misread copy table

| `countered_peer_claims` | rendered | meaning | guard |
|---|---|---|---|
| `Some(1)` | `(1 countered)` | 1 cached peer claim has ≥1 counter (presence) | C-4 — never "disputed by N" |
| `Some(1)` for a peer claim countered by 2 counterers (e.g. Maria + Rachel) | `(1 countered)` | counted ONCE | C-4 / R-PC-4 |
| `Some(0)` | `(0 countered)` | honest "nothing of my cached peer material disputed" — a SUCCESSFUL read | C-5 — distinct from missing |
| `None` | `(— countered)` | the read FAILED — the missing marker, NOT a fabricated 0 | C-5 / WD-PC-6 |

The copy is NEUTRAL disputed-claim awareness — never "refuted", "false", "disputed by N", a score,
a deduction, or a verdict (C-6 / WD-PC-10, the slice-14/18 anti-misread sensibility). It lives in
ONE helper SHARED with slice-18, so a copy mutation has exactly one site to attack.

## 4. The effect-shell resolution (`.ok()` per surface — ADR-056 D4)

```rust
// landing_page — adds a 5th independent resolution to the slice-17/18 four
let summary = LandingSummary {
    own_claims:            store.count_claims().ok(),
    peer_claims:           peer_claims_count_with_fault_seam(store.count_peer_claims()).ok(),     // slice-17 seam
    active_peers:          store.count_active_peer_subscriptions().ok(),
    countered_own_claims:  countered_count_with_fault_seam(store.count_countered_own_claims()).ok(),  // slice-18 seam
    countered_peer_claims: countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok(),  // slice-19 seam (4th token)
};

// peer_claims_page — resolves the SAME read, threads the Option into the full-page render
let countered_peer = countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok();
// ... Shape::FullPage  => render_peer_claims_page(&page_view, countered_peer)
//     Shape::Fragment  => render_peer_claims_view_panel_fragment(&page_view)  // UNTOUCHED
```

A failed countered-peer read → `None` → the missing marker, the sibling counts (incl. the
slice-18 own-countered) / rows / flags + nav hub intact, always 200 (C-2 / NFR-VIEW-6). The new
`#[cfg(debug_assertions)]`-gated `countered_peer_count_with_fault_seam` (reading the 4th token
`OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`, component-boundaries §6) INDUCES the `Err` the
`.ok()` already handles — used by the missing≠zero behavioral test, and DISTINCT from the slice-18
own-countered seam so the PEER count fails INDEPENDENTLY of the own count.

## 5. What is NOT modeled (boundary)

- NO new persisted type / column / table / index — the count is computed per-request from the
  existing slice-12 ref tables + slice-03 `peer_claims` (I-VIEW-4 / BR-VIEW-2).
- NO third dimension / no re-touching the slice-18 `countered_own_claims` field or its surfaces
  (WD-PC-7 / BR-PC-4) — this is the own+peer COMPLETION, JUST the peer field.
- NO new render helper (`render_countered` is REUSED — WD-PC-10).
- NO ADT richer than `Option<usize>` (ADR-056 D2 — the count has two states: read / unread).
- NO counter content (authors/reasons/threads) in the model — the count is a scalar.
</content>
