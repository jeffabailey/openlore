# ADR-055: Counter-Aware Counts — a Count-Only `count_countered_own_claims` Presence Aggregate (DISTINCT, ref-tables-only), an Additive `Option<usize>` Field on `LandingSummary`, and a Shared `render_countered` Helper Driving the Same Number on the Landing and the `/claims` Header

- **Status**: Accepted
- **Date**: 2026-06-09
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-counter-aware-counts` (slice-18)
- **Feature**: viewer-counter-aware-counts (slice-18)
- **Extends**: ADR-054 (slice-17 — the Option-shaped `LandingSummary`, `render_count`, `MISSING_COUNT_MARKER`, the per-count `.ok()` independent degrade, the count-only-read decision; this ADR adds a FOURTH count-only sibling read + a FOURTH `Option<usize>` field on the SAME summary, and shares the same missing≠zero render discipline), ADR-048 (slice-12 — `counter_presence_for` + the indexed `claim_references ∪ peer_claim_references` ref tables, `ref_type='counters'`, the `unwrap_or_default()` graceful-degrade; this ADR reuses the SAME ref-tables-only data shape as a COUNT rather than a presence SET), ADR-046 (slice-11 — the UNION-ALL counter ref lookup), ADR-030 (the read-only `StoreReadPort` with NO mutation method — the count-only countered variant is a read), ADR-031 (offline-first vendored htmx, no CDN), ADR-009 (Hexagonal + Modular Monolith), ADR-007 (functional paradigm — pure render core, effect shell at the I/O edge), and the slice-14 anti-misread neutral-copy sensibility (the count is disputed-claim awareness, never a verdict/penalty).

## Context

The `openlore ui` viewer can answer "is THIS ONE claim countered?" (slice-11
`query_counter_claims`) and "which of THESE page CIDs are countered?" (slice-12
`counter_presence_for`, a presence SUBSET over `claim_references ∪
peer_claim_references` where `ref_type='counters'`). slice-17 turned `GET /` into a
read-only orientation front door with a three-count `LandingSummary` (own claims,
peer claims, active peers), each an `Option<usize>` degrading independently via
`.ok()`. But NO read answers, in one cheap aggregate, "HOW MANY of my own claims have
been countered?" — the at-a-glance disputed-claim awareness this slice surfaces on the
landing ("12 own claims (3 countered)") and in the `/claims` list header.

DISCUSS is APPROVED (DoR 9/9, J-003b orientation facet). It handed DESIGN two open
questions, each with a real downside if decided wrong:

1. **WD-CC-5 — the countered-count read shape.** (a) a count-only aggregate
   `count_countered_own_claims()` (mirrors slice-17's `count_active_peer_subscriptions`
   count-only decision, ADR-054 D3) vs (b) reuse the slice-12
   `counter_presence_for(all_own_cids).len()` (zero new port surface, but materializes
   every own cid + the presence set just to count). PRODUCT contract: a SINGLE
   aggregate read, invariant to store size (C-3), either way.
2. **WD-CC-7 — own-claims-only vs also adding peer-claims-countered.** Recommend
   own-claims-only as the load-bearing orientation signal; peer-claims-countered
   deferred.

The data: own claims are countered by PEERS (the self-counter rule blocks countering
one's own claim directly), so a countered own-claim is an own-claim cid (`SELECT cid
FROM claims`) that appears as a countered `referenced_cid` in `claim_references` (the
operator's own later counter to a DIFFERENT own claim of hers) ∪ `peer_claim_references`
(a peer's counter) where `ref_type='counters'`.

## Decision

### D1 — Add a count-only `count_countered_own_claims()` read (WD-CC-5 RESOLVED → count-only aggregate)

A new read method is added to the read-only `StoreReadPort`:

```
fn count_countered_own_claims(&self) -> Result<usize, StoreReadError>;
```

`adapter-duckdb` implements it as ONE aggregate, the count of DISTINCT own-claim CIDs
that appear as a countered `referenced_cid` across the two indexed ref tables:

```sql
SELECT COUNT(DISTINCT c.cid)
FROM claims c
WHERE c.cid IN (
    SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters'
    UNION
    SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters'
)
```

- **Presence count via the IN-set + DISTINCT (C-4 / BR-CC-1).** The inner `UNION`
  (set union — de-duplicated, not `UNION ALL`) collapses the countered
  `referenced_cid`s to a distinct set; `c.cid IN (…)` then asks, per own claim, "is
  this claim countered at all?" — a membership test, NOT a per-counter row. A claim
  countered by N peers contributes its cid ONCE to the IN-set, so it is counted ONCE.
  `COUNT(DISTINCT c.cid)` is belt-and-braces against any duplicate own cid. There is
  NO JOIN-fanout: an `IN`-subquery never multiplies the outer `claims` rows the way a
  `JOIN claim_references` would. The result can NEVER be a "disputed by N" total.
- **Own-only by construction (WD-CC-7 RESOLVED → own-only).** The outer table is
  `claims` (the operator's OWN claims). A peer claim that is countered is NOT in
  `claims`, so it never contributes — peer-claims-countered is excluded by the query
  shape, not by a filter that could be relaxed. Adding peer-claims-countered would be a
  SECOND read + a SECOND field (deferred per WD-CC-7; see Alternatives).
- **Literal `'counters'` only; parameter-free, injection-safe.** The only WHERE values
  are the literal `ref_type='counters'` constants — no caller input is interpolated.
  The query takes NO bind parameters (unlike slice-12's per-page `IN (?, ?, …)`), so
  the injection surface is zero.
- **Read-only by construction (C-1 CARDINAL).** It returns `Result<usize, _>`; the
  trait declares no mutation method, so a `Box<dyn StoreReadPort>` stays structurally
  incapable of mutating. It runs over the SAME shared connection the CLI writes through
  (BR-VIEW-4). LOCAL only — no network seam (C-2).
- **Invariant to store size (C-3).** ONE aggregate, both ref columns indexed (slice-12
  ADR-048); the landing's read budget grows from 3 to 4, `claims_page` adds 1. NO
  per-claim `counter_presence_for` loop.

**Rationale (count-only chosen over `counter_presence_for(own_cids).len()`):**
- **Symmetry.** The landing's other three counts ARE count-only aggregates
  (`count_claims`, `count_peer_claims`, `count_active_peer_subscriptions`); a fourth
  count-only sibling makes the four-count summary four structurally similar aggregate
  reads — the cleanest read shape for the slice's "fixed aggregate reads, invariant to
  store size" contract (C-3). This MIRRORS the identical slice-17 ADR-054 D3 choice.
- **Cheapness / honest no-N+1.** `counter_presence_for(own_cids).len()` would first
  materialize every own cid (`SELECT cid FROM claims` decoded into a `Vec<String>`),
  bind it twice into an `IN (?, …)` presence query, decode the presence SET, then throw
  it all away to take `.len()`. The count-only aggregate decodes ONE scalar and binds
  NOTHING. Both are invariant to store size, but the count-only read does the minimum
  work for the number the orientation surfaces actually need.
- **Cost is one tiny read method.** The downside (a new port surface) is one read-only
  method mirroring three that already exist, plus one aggregate impl in `adapter-duckdb`
  shaped after the slice-12 `counter_presence_for` SQL (same tables, same `ref_type`
  filter, as a COUNT instead of a presence SELECT). It adds NO mutation method, NO new
  crate (workspace stays 21), NO new route.

### D2 — Additive `Option<usize>` field on the slice-17 `LandingSummary` (WD-CC-6 / missing≠zero, ADR-054 D1 generalized)

`LandingSummary` gains a FOURTH field, parallel to the slice-17 three:

```
pub struct LandingSummary {
    pub own_claims: Option<usize>,
    pub peer_claims: Option<usize>,
    pub active_peers: Option<usize>,
    pub countered_own_claims: Option<usize>,   // slice-18 — additive
}
```

- `Some(n)` = a SUCCESSFUL read of `n` (including `Some(0)` — an honest "nothing of
  mine has drawn a counter"). `None` = the countered-count read FAILED → the slice-17
  `MISSING_COUNT_MARKER` "—". `0 ≠ missing` stays a TYPE-LEVEL distinction; a fabricated
  0 on a failed read is unrepresentable (C-5 / WD-CC-6).
- The field degrades INDEPENDENTLY: a failed countered read leaves `own_claims`,
  `peer_claims`, `active_peers` `Some(_)` (and vice versa). There is NO all-or-nothing
  summary state — the slice-17 per-count `.ok()` independent degrade (ADR-054 D2)
  extended to a fourth count. A transient countered-count failure must NOT blank the
  own-claims number the operator can legitimately see (US-CC-000 domain example 3).
- **A fourth parallel `Option<usize>` is chosen over a richer ADT or a separate
  view-model** because the fourth count has IDENTICAL shape and IDENTICAL degrade
  semantics to the three slice-17 counts — a fourth parallel field is the total,
  exhaustive, mutation-resistant model with the least ceremony, and keeps the pure
  render a TOTAL function of `LandingSummary` (now 2⁴ `Option` combinations; every one
  renders, no panic, no I/O). This is the SAME reasoning ADR-054 D1 used to reject a
  bespoke per-count enum.

### D3 — A shared `render_countered` helper renders the countered count on BOTH surfaces from the SAME number (WD-CC-8 single source)

The pure render of the parenthetical "(N countered)" lives in ONE helper in
`viewer-domain`, consumed by BOTH `render_landing` and the `/claims` header:

```
fn render_countered(countered: Option<usize>) -> String   // → "(3 countered)" | "(0 countered)" | "(— countered)"
```

- It maps `Some(n) → "(n countered)"`, `None → "(— countered)"` (reusing the slice-17
  `render_count` mapping for the inner number: `Some(n)→n`, `None→MISSING_COUNT_MARKER`).
  The "(N countered)" copy is NEUTRAL disputed-claim awareness — never "refuted",
  "false", "disputed by N", a score, a deduction, or a verdict (C-6 / WD-CC-10, the
  slice-14 anti-misread sensibility). Held in ONE place so the exact copy + the
  missing-marker behaviour is a single mutation-killable site.
- **`render_landing`** renders it BESIDE the unchanged own-claims line — the own-claims
  `render_count(summary.own_claims)` "12" is UNTOUCHED (additive awareness, never a
  re-weight — C-4): `(render_count(summary.own_claims)) " own claims " (render_countered(summary.countered_own_claims))`
  → "12 own claims (3 countered)".
- **The `/claims` header** (`render_claims_page`) renders the SAME helper output near
  the "My Claims" `h1` / read-only notice. `render_claims_page` gains a
  `countered_own_claims: Option<usize>` parameter (the SAME `Option` the landing uses).
  The list body — the slice-06 ordering (`composed_at DESC, cid`), paging, total count,
  every row's verbatim confidence, and the slice-12 per-row flags — is UNTOUCHED (the
  header count is ADDITIVE, never a re-order/filter/re-weight — C-4 / WD-CC-9).
- **Single source (WD-CC-8).** Both surfaces resolve the count from the SAME
  `count_countered_own_claims` read and render through the SAME `render_countered`
  helper, so the landing "(N countered)" and the `/claims` header "(N countered)" cannot
  diverge in COPY (the helper is one fn) and are pinned EQUAL for the same store by a
  gold test. The two routes resolve the count INDEPENDENTLY per render (they are
  separate `GET` handlers, each reading the current store) — that is correct: the single
  source is the READ METHOD + the RENDER HELPER, not a cached value (caching would
  introduce staleness and a shared-state seam the read-only viewer deliberately avoids).

### D4 — The effect shell resolves the countered count per route via `.ok()` (ADR-054 D2 extended)

- `landing_page` adds a FOURTH `.ok()` resolution:
  `countered_own_claims: store.count_countered_own_claims().ok()` — building the
  extended `LandingSummary`. A failed read → `None` → the missing marker, the other
  three counts + the nav hub intact, always 200 (C-2 / NFR-VIEW-6).
- `claims_page` resolves `store.count_countered_own_claims().ok()` and passes it into
  `render_claims_page(&page_view, countered_own_claims)`. The countered-count read is
  INDEPENDENT of the list read: a failed countered read renders the header missing
  marker while the rows still render; a failed list read degrades the list as today
  (slice-06) while the header count renders if its read succeeded. Always 200, never a
  5xx, never a fabricated 0.

## Alternatives Considered

### For D1 (read shape, WD-CC-5)

- **`counter_presence_for(all_own_cids).len()`** — zero new port surface, but
  materializes every own cid into a `Vec<String>`, binds it twice into an `IN (?, …)`
  presence query, and decodes the presence SET just to count it. REJECTED: asymmetric
  with the three count-only landing reads, and does throwaway materialization. Viable
  and correct at dogfood scale, but the count-only aggregate is cleaner and cheaper for
  one trivial read method — the SAME trade-off ADR-054 D3 resolved for the active-peer
  count.
- **`UNION ALL` in the inner subquery instead of `UNION`** — would let the SAME
  `referenced_cid` appear twice in the IN-set, but because `IN` is a membership test
  the duplicate is harmless to correctness. REJECTED in favour of `UNION` for clarity
  of intent (we want the DISTINCT set of countered cids), even though `IN` + the outer
  `COUNT(DISTINCT c.cid)` would produce the right number either way. The presence count
  is doubly defended (the de-duped IN-set + the outer DISTINCT).

### For D2 (degrade model)

- **An all-or-nothing `Result<Summary, _>`** — a single failed count would blank ALL
  four numbers. REJECTED: violates the per-count independent degrade (US-CC-000 domain
  example 3) and risks the 5xx C-2 forbids.
- **Fabricate 0 on a failed countered read (`unwrap_or(0)`)** — REJECTED: misleads
  "nothing of mine is disputed" (R-CC-1/C-5). `Option` makes `0 ≠ missing` representable.
- **A separate parallel `Option<usize>` NOT on `LandingSummary`** (a second view-model
  threaded alongside) — REJECTED: the countered count has identical shape + degrade to
  the slice-17 counts and is rendered IN the same summary; a fourth field on the
  existing struct is the lowest-ceremony single source. (The `/claims` header takes the
  bare `Option<usize>` directly, since `render_claims_page` does not take a
  `LandingSummary` — see D3.)

### For D3 (scope, WD-CC-7)

- **Also add a peer-claims-countered count this slice** — a SECOND count-only aggregate
  (`claims` → `peer_claims` as the outer table) + a SECOND `Option` field + a SECOND
  parenthetical on the peer-claims line. REJECTED for this slice: own-claims-countered
  is the load-bearing "how much of MY work drew pushback" orientation signal;
  peer-claims-countered widens the read + render + copy for a less-central signal and
  breaks the single-count clarity. Surfaced as an explicit DEFERRED decision (WD-CC-7),
  not silently dropped — a recommended follow-up slice if dogfood shows demand. The
  count-only query shape makes adding it later a clean, additive sibling.

## Consequences

### Positive
- `0 ≠ missing` for the countered count is a TYPE-LEVEL invariant (`Option<usize>`); a
  fabricated 0 on failure is unrepresentable (C-5).
- Independent degrade: the countered count can fail without blanking the three slice-17
  counts, the nav hub, or the `/claims` rows, and never 5xxes (C-2 honored).
- The countered count is a presence count by construction (de-duped IN-set + DISTINCT,
  no JOIN-fanout); a claim countered N times counts ONCE; the own-claims count is never
  re-weighted (C-4).
- ONE additional aggregate read per render on each surface (landing 3→4; `claims_page`
  +1), invariant to store size, both ref columns indexed (no N+1, C-3).
- Single source: the landing and `/claims` header render the SAME number through the
  SAME `render_countered` helper; the copy cannot diverge (WD-CC-8); pinned equal by a
  gold test.
- Additive on `/claims`: the slice-06 list order/paging/count/confidence + the slice-12
  per-row flags are byte-identical to the no-header-count baseline (C-4 / WD-CC-9).
- Neutral anti-misread copy in one mutation-killable helper site (C-6 / WD-CC-10).
- Read-only preserved: no mutation method added (the count-only read is a read);
  workspace stays 21; no new crate; no new route; loopback bind unchanged; nothing
  persisted.

### Negative / trade-offs
- One new read method on `StoreReadPort` (`count_countered_own_claims`) + one aggregate
  impl in `adapter-duckdb` (vs zero new surface for `counter_presence_for(...).len()`).
  Accepted: one trivial read-only method buys symmetry + cheapness; mirrors ADR-054 D3.
- `render_claims_page` gains a parameter (`countered_own_claims: Option<usize>`), a
  signature change rippling to its callers/tests. Accepted: the SAME number must drive
  the header, and a parameter is the pure, single-source way to pass it (no global, no
  hidden state).
- Peer-claims-countered is NOT shown this slice (WD-CC-7). Accepted + surfaced: a
  recommended deferred sibling, not a silent omission.

## Enforcement (Earned Trust — how the design proves it honors its contract)

- **Read-only (3 layers, cardinal C-1):** (a) TYPE — `StoreReadPort` declares no
  mutation method; the added `count_countered_own_claims` returns `Result<usize, _>`, so
  a `Box<dyn StoreReadPort>` remains structurally incapable of mutating. (b) xtask
  `check_viewer_capability_boundary` (crate-dependency-graph rule) stays GREEN — the
  viewer depends on no write adapter; a read-only count method changes no dependency
  edge. (c) BEHAVIORAL gold — port-to-port tests assert `/` and `/claims` contain no
  `<form>`/`<button>`/mutating control (the countered count is render-only text, never a
  sort/filter control), and that store contents are byte-identical before/after N
  requests (acceptance-criteria.md Theme 6).
- **Anti-merging SQL rule (`no_cross_table_join_elides_author`, xtask layer 2 of 3):**
  the new `count_countered_own_claims` SQL is GREEN by construction. The
  `classify_sql_literal` classifier (xtask/src/check_arch.rs ~319) fires only when a
  literal mentions BOTH the `claims` table AND the `peer_claims` table as WHOLE WORDS
  but omits `author_did`. The new SQL names `claims`, `claim_references`, and
  `peer_claim_references` — and `contains_word("…peer_claim_references…", "peer_claims")`
  is FALSE (the `_` after `peer_claims` is a word byte, so the word boundary fails), so
  `mentions_peer_claims` is false, so `is_cross_store` is false, so the classifier
  returns `None` (no violation). This is the EXACT reason the slice-12
  `counter_presence_for` SQL (which names the same two ref tables, no `author_did`) is
  GREEN. The rule guards a MERGING JOIN that elides attribution; this query is a
  presence COUNT over indexed ref tables with no attribution to merge — it neither joins
  `claims`+`peer_claims` nor drops an author projection (there is no author in a count).
  A behavioral note: the index-store aggregation variant of the rule
  (`mentions_aggregation`) scans `crates/adapter-index-store/src` ONLY, not
  `adapter-duckdb`, so the `COUNT(DISTINCT …)` does not trip it. The xtask boundary is
  UNCHANGED by this slice.
- **Missing ≠ zero (C-5 / WD-CC-6):** behavioral tests seed an unreadable countered
  count and assert `/` and `/claims` render the missing marker for the countered count
  (distinct from a seeded real 0), the sibling counts/rows still render, and the route
  returns 200 — never a 5xx, never a raw stack trace (Theme 4). Earned Trust applied to
  the store-read dependency: the design exercises the substrate LYING (a transient
  `StoreReadError`) and proves both orientation surfaces survive it. The slice-17
  peer-claims fault seam (`#[cfg(debug_assertions)]`-gated) is the precedent for the
  countered-count fault seam.
- **Presence count, counted once (C-4):** a behavioral test seeds a claim countered by
  TWO peers and asserts the landing + header render "(1 countered)", not "(2 countered)"
  (Theme 2). Earned Trust applied to the de-dup contract: the design exercises the
  multi-counter case the DISTINCT defends against.
- **Single source (WD-CC-8):** a gold test asserts the landing "(N countered)" ==
  the `/claims` header "(N countered)" for the same store (Theme 1).
- **No regression on `/claims` (C-4 / WD-CC-9):** a gold test asserts the `/claims` list
  order/paging/count/confidence is byte-identical to the no-header-count baseline (Theme
  5).
- **No N+1 (C-3):** a `@property`-shaped behavioral test asserts the countered-count
  read count is invariant to store size (Theme 8).
- **No network (C-2):** a behavioral test asserts `/` and `/claims` render fully
  network-down, make no outbound request, and reference only the vendored
  `/static/htmx.min.js` (Theme 7).
