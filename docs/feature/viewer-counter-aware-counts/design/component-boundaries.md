# Component Boundaries: viewer-counter-aware-counts (slice-18)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-09 · ADR: ADR-055
> Functional paradigm (ADR-007): the boundary is pure render core vs effect shell.
> NO new crate · NO new route · workspace stays 21 members.

## 1. Crate touch map (what changes, what does not)

| Crate | Role | Change in slice-18 |
|---|---|---|
| `viewer-domain` (PURE) | The render core | Add a FOURTH `Option<usize>` field `countered_own_claims` to `LandingSummary`. Add `fn render_countered(Option<usize>) -> String` (`Some(n)→"(n countered)"`, `None→"(— countered)"` via the slice-17 `render_count` mapping). `render_landing` renders it BESIDE the unchanged own-claims line. `render_claims_page` gains a `countered_own_claims: Option<usize>` param and renders the SAME helper in the header. |
| `adapter-http-viewer` (EFFECT) | The HTTP shell | `landing_page` adds a 4th `.ok()` resolution (`countered_own_claims: store.count_countered_own_claims().ok()`) to the `LandingSummary`. `claims_page` resolves `store.count_countered_own_claims().ok()` and passes it into `render_claims_page(&page_view, countered)` (full-page path). |
| `ports` (TRAIT) | `StoreReadPort` | Add ONE read-only method: `count_countered_own_claims(&self) -> Result<usize, StoreReadError>` (WD-CC-5 → count-only aggregate, ADR-055 D1). NO mutation method added. |
| `adapter-duckdb` (ADAPTER) | DuckDB impl | Implement `count_countered_own_claims`: `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')`. One aggregate read; NO bind parameters (literal `'counters'` only). |
| `xtask` (ARCH RULES) | check-arch | UNCHANGED. `check_viewer_capability_boundary` is a crate-dependency-graph rule (a read-only count touches no dependency edge). `no_cross_table_join_elides_author` is GREEN by construction (see §5). |
| All other crates | — | UNCHANGED. No new crate. Workspace stays 21. |

## 2. The pure / effect boundary (ADR-007)

```
                 EFFECT SHELL                              PURE CORE
   adapter-http-viewer::landing_page              viewer-domain::render_landing
   adapter-http-viewer::claims_page               viewer-domain::render_claims_page
   ──────────────────────────────────             ──────────────────────────────────
   reads count_countered_own_claims (I/O, Result) total fns of Option<usize> (+ the
   maps Result→Option via .ok()         ──►       slice-17 LandingSummary / the PageView)
   builds LandingSummary (4th field) /            Some(n) → "(n countered)"
     passes Option into render_claims_page        None    → "(— countered)"
   wraps html_ok (200)                            render via the SHARED render_countered helper
```

- **The I/O (the countered read + the `Result→Option` mapping) lives ONLY in the shell.** The
  pure core never sees a `Result`, a `StoreReadError`, or the store — it receives an
  already-resolved `Option<usize>` (on `LandingSummary`, or as the `render_claims_page` param) and
  renders totally. This is the slice-17 pure-render discipline unchanged (ADR-054 D1/D2).
- **The degrade decision (`.ok()`) is an effect-shell decision** (it is about how to treat an I/O
  failure), mirroring slice-17's per-count `.ok()` and slice-12's `unwrap_or_default()` (ADR-048).
  The pure render only knows "this count is `Some` or `None`" and renders each totally — now over
  2⁴ `Option` combinations on `LandingSummary`, every one a defined render (no panic, no I/O).

## 3. The new read (one count-only aggregate; the data is reused)

| Count | Read | Status | Source |
|---|---|---|---|
| own claims | `count_claims()` | EXISTING | slice-06, `ports` ~296, `adapter-duckdb` (`SELECT COUNT(*) FROM claims`) |
| peer claims | `count_peer_claims()` | EXISTING | slice-06, `ports` ~316, `adapter-duckdb` ~488 |
| active peers | `count_active_peer_subscriptions()` | EXISTING (slice-17) | ADR-054 D3, `adapter-duckdb` ~498 |
| **countered own claims** | **`count_countered_own_claims()`** | **NEW (count-only, read-only)** | ADR-055 D1; the DATA (the `claim_references ∪ peer_claim_references`, `ref_type='counters'` ref tables) is the slice-12 `counter_presence_for` data, read as a COUNT instead of a presence SET |

The countered-own-claims count is the ONLY net-new read surface. The DATA is REUSED (the slice-12
counter-reference tables); only the read SHAPE is new (a single `COUNT(DISTINCT …)` aggregate vs
the slice-12 per-page `IN (?, …)` presence SELECT). It is a READ method: `StoreReadPort` still
declares no mutation method (I-VIEW-1), so the read-only invariant is preserved by construction.

## 4. The new read's effect on `ports` / `adapter-duckdb`

- `ports`: +1 trait method signature (`count_countered_own_claims`). The sync read posture is
  unchanged (this is a sync read like `count_claims`/`count_peer_claims`/
  `count_active_peer_subscriptions`).
- `adapter-duckdb`: +1 impl — `let conn = self.lock_conn()?; conn.query_row("SELECT COUNT(DISTINCT
  c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE
  ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE
  ref_type='counters')", [], |row| row.get(0))` mapping a failure to `StoreReadError::QueryFailed`,
  mirroring `count_active_peer_subscriptions` (~498). NO bind parameters (the only WHERE values are
  the literal `'counters'` constants — parameter-free, injection-safe).
- Any OTHER `StoreReadPort` impl (test doubles, the slice-17 `FakeLandingStore`) gains the same
  method; trivially a count, no mutation.

## 5. xtask boundary — the anti-merging SQL rule is GREEN by construction (VERIFIED)

The slice's anti-merging concern (called out in the brief) is the `no_cross_table_join_elides_author`
rule (`xtask/src/check_arch.rs`, the `classify_sql_literal` classifier ~319). VERDICT: the new SQL
does NOT trip it. Why, mechanically:

- The classifier fires ONLY when a SQL literal mentions BOTH the `claims` table AND the
  `peer_claims` table as WHOLE WORDS but omits `author_did` (a MERGING JOIN that elides
  attribution).
- The new SQL names three identifiers containing the substring `claim`: `claims`,
  `claim_references`, and `peer_claim_references`. The classifier's `contains_word` uses ASCII
  word boundaries (`[A-Za-z0-9_]`):
  - `contains_word(sql, "peer_claims")` → **FALSE**. The literal's closest token is
    `peer_claim_references`; after the `peer_claims` substring comes `_references`, and `_` IS a
    word byte → the trailing boundary fails. So `mentions_peer_claims` is false.
  - With `mentions_peer_claims == false`, `is_cross_store = mentions_peer_claims &&
    mentions_own_claims` is false → the classifier returns `None`. **No violation.**
- This is the EXACT reason the slice-12 `counter_presence_for` SQL (which names `claim_references`
  + `peer_claim_references`, no `author_did`) is GREEN today. The new query is in the SAME
  word-boundary class.
- Semantically, the rule guards a JOIN that merges `claims`+`peer_claims` attribution; this query
  is a presence COUNT over the indexed REF tables — there is NO `peer_claims`/`claims` merging
  JOIN, and a count has no `author_did` to elide (the rule's concern does not arise).
- The index-store aggregation variant of the rule (`mentions_aggregation`, which would flag a
  `COUNT(` over `indexed_claims`) scans `crates/adapter-index-store/src` ONLY — NOT
  `adapter-duckdb` — so the `COUNT(DISTINCT …)` in this `adapter-duckdb` impl does not trip it.

`check_viewer_capability_boundary` (the transitive-crate-dependency rule) also stays GREEN —
adding a read-only count method to `StoreReadPort` changes no crate dependency edge (the viewer
still depends on no write/sign adapter). The read-only invariant's THREE enforcement layers
(ADR-055 Enforcement) hold unchanged: (1) type — the trait has no mutation method; (2) xtask —
this capability rule; (3) behavioral gold — `/` and `/claims` have no mutating control + store
unchanged after N requests.

## 6. What stays out (boundary guards)

- NO new route — `GET /` and `GET /claims` exist; the route arms only gain the countered read +
  render.
- NO new crate — workspace stays 21.
- NO mutation method, NO signing key, NO write/compose/sign/subscribe/follow control. The
  countered count is render-only text, never a sort/filter/mutating control (C-1 CARDINAL).
- NO network seam — one LOCAL aggregate read.
- NO peer-claims-countered count this slice (WD-CC-7 — own-only is the load-bearing signal;
  peer-claims-countered is a recommended DEFERRED sibling, surfaced not dropped).
- NO re-order/filter/re-weight/re-page of the `/claims` list — the header count is ADDITIVE; the
  slice-06 ordering/paging/count + the slice-12 per-row flags are byte-identical to the
  no-header-count baseline (C-4 / WD-CC-9).
- NO re-weight of the own-claims count — the "12" is unchanged (C-4).
- NO counter CONTENT (authors/reasons/threads) in the count — reading WHO countered WHAT stays the
  slice-11 thread + the slice-12 per-row flags.
- NO `Shape` fork added to `/` (ADR-054 D5 unchanged); the `/claims` header count is full-page
  chrome — the htmx fragment path is untouched.
