# Data Models: viewer-counter-aware-counts (slice-18)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-055
> Functional paradigm (ADR-007). NO new persisted type; NO schema change. The countered count is
> computed per-request from the EXISTING slice-12 counter-reference tables; nothing new is stored.

## 1. The extended view-model — `LandingSummary` gains a FOURTH `Option<usize>` field

```rust
// crates/viewer-domain/src/lib.rs (extends the slice-17 struct, ADR-054 D1 → ADR-055 D2)
pub struct LandingSummary {
    pub own_claims: Option<usize>,           // count_claims          — slice-17
    pub peer_claims: Option<usize>,          // count_peer_claims     — slice-17
    pub active_peers: Option<usize>,         // count_active_peer_subscriptions — slice-17
    pub countered_own_claims: Option<usize>, // count_countered_own_claims      — slice-18 (additive)
}
```

- `Some(n)` = a SUCCESSFUL read of `n` (including `Some(0)` — an honest "nothing of mine has drawn
  a counter"). `None` = the countered-count read FAILED → the slice-17 `MISSING_COUNT_MARKER` "—".
  `0 ≠ missing` stays a TYPE-LEVEL distinction; a fabricated 0 on a failed read is unrepresentable
  (C-5 / WD-CC-6).
- The four counts degrade INDEPENDENTLY: a failed countered read leaves the three slice-17 counts
  `Some(_)` (and vice versa) — the per-count `.ok()` model (ADR-054 D2) extended to a fourth field.
- The pure render is a TOTAL function of `LandingSummary` over all 2⁴ `Option` combinations.

## 2. The countered count read — `count_countered_own_claims` (WD-CC-5 RESOLVED → count-only aggregate)

`StoreReadPort` (read-only — NO mutation method):

```rust
fn count_countered_own_claims(&self) -> Result<usize, StoreReadError>;
```

`adapter-duckdb` impl — ONE aggregate, NO bind parameters, injection-safe:

```sql
SELECT COUNT(DISTINCT c.cid)
FROM claims c
WHERE c.cid IN (
    SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters'
    UNION
    SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters'
)
```

| Property | How the SQL guarantees it | Cardinal |
|---|---|---|
| Presence count (a claim countered N times counts ONCE) | the inner `UNION` de-dupes the countered `referenced_cid` set; `c.cid IN (…)` is a MEMBERSHIP test (no per-counter row); `COUNT(DISTINCT c.cid)` double-defends — NO JOIN-fanout (an `IN`-subquery never multiplies outer rows) | C-4 / WD-CC-4 / R-CC-4 |
| Own-only (peer-claims-countered excluded) | the outer table is `claims` (own claims); a countered PEER claim is not in `claims`, so it never contributes — excluded by query shape, not a relaxable filter | WD-CC-7 |
| Read-only / injection-safe | returns `Result<usize, _>`; no mutation method on the trait; NO bind parameters (literal `'counters'` only) | C-1 |
| Invariant to store size / no N+1 | ONE aggregate; both ref columns indexed (slice-12 ADR-048); the landing budget grows 3→4, `claims_page` +1 | C-3 / WD-CC-3 |
| LOCAL only | reads the indexed ref tables over the shared connection; no network seam | C-2 |

The DATA is the slice-12 `counter_presence_for` data (`claim_references ∪ peer_claim_references`,
`ref_type='counters'`) — read as a COUNT rather than a presence SET. The own-via-peer-references
note (ADR-055 Context): own claims are countered by PEERS (the self-counter rule), so a countered
own-claim cid appears in `peer_claim_references` (a peer's counter) OR in `claim_references` (the
operator's own later counter to a DIFFERENT own claim of hers). The `UNION` over both tables
captures both; the DISTINCT IN-set + outer DISTINCT count each such cid once.

## 3. The countered-count render — the SHARED `render_countered` helper (WD-CC-8 single source)

```rust
// crates/viewer-domain/src/lib.rs — PURE total fn, ONE site of the "(N countered)" copy
fn render_countered(countered: Option<usize>) -> String {
    // reuses the slice-17 render_count mapping for the inner number:
    //   Some(n) → format!("({} countered)", n)         e.g. "(3 countered)", "(0 countered)"
    //   None    → format!("({} countered)", MISSING_COUNT_MARKER)   → "(— countered)"
}
```

- **Landing (`render_landing`)** — rendered BESIDE the UNCHANGED own-claims line:
  `(render_count(summary.own_claims)) " own claims " (render_countered(summary.countered_own_claims))`
  → `"12 own claims (3 countered)"`. The own-claims "12" is verbatim (additive awareness, never a
  re-weight — C-4).
- **`/claims` header (`render_claims_page`)** — the fn gains a `countered_own_claims: Option<usize>`
  param; the SAME `render_countered` output renders near the "My Claims" `h1` / read-only notice.
  The list body (slice-06 ordering `composed_at DESC, cid`, paging, total count, every row's
  verbatim confidence, the slice-12 per-row flags) is UNTOUCHED (additive — C-4 / WD-CC-9).
- **Single source (WD-CC-8)**: both surfaces render the SAME number through the SAME helper; the
  copy cannot diverge. A gold test pins landing "(N countered)" == `/claims` header "(N countered)".

### Missing ≠ zero / anti-misread copy table

| `countered_own_claims` | rendered | meaning | guard |
|---|---|---|---|
| `Some(3)` | `(3 countered)` | 3 own claims have ≥1 counter (presence) | C-4 — never "disputed by N" |
| `Some(1)` for a claim countered by 2 peers | `(1 countered)` | counted ONCE | C-4 / R-CC-4 |
| `Some(0)` | `(0 countered)` | honest "nothing of mine disputed" — a SUCCESSFUL read | C-5 — distinct from missing |
| `None` | `(— countered)` | the read FAILED — the missing marker, NOT a fabricated 0 | C-5 / WD-CC-6 |

The copy is NEUTRAL disputed-claim awareness — never "refuted", "false", "disputed by N", a score,
a deduction, or a verdict (C-6 / WD-CC-10, the slice-14 anti-misread sensibility). It lives in ONE
helper, so a copy mutation has exactly one site to attack.

## 4. The effect-shell resolution (`.ok()` per surface — ADR-055 D4)

```rust
// landing_page — adds a 4th independent resolution to the slice-17 three
let summary = LandingSummary {
    own_claims:           store.count_claims().ok(),
    peer_claims:          /* slice-17 fault-seam */ store.count_peer_claims().ok(),
    active_peers:         store.count_active_peer_subscriptions().ok(),
    countered_own_claims: store.count_countered_own_claims().ok(),  // slice-18
};

// claims_page — resolves the SAME read, threads the Option into the full-page render
let countered = store.count_countered_own_claims().ok();
// ... render_claims_page(&page_view, countered)  (full-page path; the htmx fragment path is untouched)
```

A failed countered read → `None` → the missing marker, the sibling counts/rows + nav hub intact,
always 200 (C-2 / NFR-VIEW-6). The slice-17 `#[cfg(debug_assertions)]`-gated peer-claims fault
seam is the precedent for a countered-count fault seam that INDUCES the `Err` the `.ok()` already
handles (used by the missing≠zero behavioral test).

## 5. What is NOT modeled (boundary)

- NO new persisted type / column / table / index — the count is computed per-request from the
  existing slice-12 ref tables (I-VIEW-4 / BR-VIEW-2).
- NO peer-claims-countered field this slice (WD-CC-7 — own-only; the deferred sibling would add a
  parallel `Option` + a `peer_claims`-outer-table count).
- NO ADT richer than `Option<usize>` (ADR-055 D2 — the count has two states: read / unread).
- NO counter content (authors/reasons/threads) in the model — the count is a scalar.
