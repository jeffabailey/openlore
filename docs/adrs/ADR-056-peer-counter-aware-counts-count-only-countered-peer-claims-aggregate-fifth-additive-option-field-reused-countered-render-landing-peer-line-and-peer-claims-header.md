# ADR-056: Peer Counter-Aware Counts — a Count-Only `count_countered_peer_claims` Presence Aggregate (the slice-18 SQL with outer table `peer_claims`), a FIFTH Additive `Option<usize>` Field on `LandingSummary`, the REUSED `render_countered` Helper Driving the Same Number on the Landing Peer Line and the `/peer-claims` Header, and a 4th DISTINCT Fault-Seam Token

- **Status**: Accepted
- **Date**: 2026-06-10
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-peer-counter-aware-counts` (slice-19)
- **Feature**: viewer-peer-counter-aware-counts (slice-19)
- **Extends / mirrors**: ADR-055 (slice-18 — the count-only `count_countered_own_claims` aggregate, the additive `Option<usize>` `LandingSummary` field, the shared `render_countered` helper, the missing≠zero per-count `.ok()` degrade, the cfg-gated countered-count fault seam; this ADR is the EXACT peer mirror that slice-18 explicitly DEFERRED at WD-CC-7 — "the count-only query shape makes the deferred sibling clean to add later"), ADR-054 (slice-17 — the Option-shaped `LandingSummary`, `render_count`, `MISSING_COUNT_MARKER`, the per-count independent degrade), ADR-048 (slice-12 — `counter_presence_for` + the indexed `claim_references ∪ peer_claim_references` ref tables, `ref_type='counters'`; this ADR reuses the SAME ref-tables-only data as a COUNT), ADR-046 (slice-11 — peer counters land in `peer_claim_references`), ADR-030 (the read-only `StoreReadPort` with NO mutation method), ADR-031 (offline-first vendored htmx, no CDN), ADR-009 (Hexagonal + Modular Monolith), ADR-007 (functional paradigm — pure render core, effect shell at the I/O edge), and the slice-14/18 anti-misread neutral-copy sensibility.

## Context

slice-18 (ADR-055) answered, in one cheap aggregate, "HOW MANY of my OWN claims have been
countered?" and surfaced it beside the own-claims line on `GET /` ("12 own claims (3 countered)")
and in the `/claims` list header. It explicitly DEFERRED the symmetric peer count (WD-CC-7):
own-claims-countered was the load-bearing orientation signal, and the count-only query shape made
the peer sibling clean to add later. The PEER line on the landing
(`p { (render_count(summary.peer_claims)) " peer claims" }`) and the `/peer-claims` header
(`h1 { "Peer Claims" }`) are still BARE — the operator must leave `/` and scan `/peer-claims` to
learn how much of her CACHED PEER material is disputed.

slice-19 closes that remaining half. DISCUSS is APPROVED (DoR 9/9, J-003b orientation facet). It
handed DESIGN one open question (WD-PC-5) and one constraint to verify (R-PC-9), each resolved
below.

The data: a cached peer claim is countered by the OPERATOR (her counter, recorded in
`claim_references`) OR by ANOTHER PEER (their counter, recorded in `peer_claim_references` —
slice-11). So a countered peer-claim cid is a `peer_claims` cid that appears as a countered
`referenced_cid` across `claim_references ∪ peer_claim_references` where `ref_type='counters'` —
the IDENTICAL inner IN-set as slice-18; only the OUTER table differs (`peer_claims`, not `claims`).

## Decision

### D1 — Add a count-only `count_countered_peer_claims()` read (WD-PC-5 RESOLVED → count-only aggregate)

A new read method is added to the read-only `StoreReadPort` — the 5th count-only sibling:

```
fn count_countered_peer_claims(&self) -> Result<usize, StoreReadError>;
```

`adapter-duckdb` implements it as ONE aggregate, the count of DISTINCT cached PEER-claim CIDs that
appear as a countered `referenced_cid` across the two indexed ref tables — the EXACT slice-18 SQL
with the outer table swapped `claims c → peer_claims p`:

```sql
SELECT COUNT(DISTINCT p.cid)
FROM peer_claims p
WHERE p.cid IN (
    SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters'
    UNION
    SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters'
)
```

- **Presence count via the IN-set + DISTINCT (C-4 / WD-PC-4).** The inner `UNION` (set union —
  de-duplicated, not `UNION ALL`) collapses the countered `referenced_cid`s to a distinct set;
  `p.cid IN (…)` then asks, per cached peer claim, "is this claim countered at all?" — a
  membership test, NOT a per-counter row. A peer claim countered by N counterers (the operator +
  another peer, or two peers) contributes its cid ONCE to the IN-set, so it is counted ONCE.
  `COUNT(DISTINCT p.cid)` is belt-and-braces. There is NO JOIN-fanout: an `IN`-subquery never
  multiplies the outer `peer_claims` rows. The result can NEVER be a "disputed by N" total. The
  inner IN-set is byte-IDENTICAL to slice-18's, so the de-dup/presence-once semantics are
  inherited verbatim.
- **Peer-only by construction (WD-PC-7 RESOLVED → peer-only).** The outer table is `peer_claims`
  (the operator's CACHED peer claims). A countered OWN claim is NOT in `peer_claims`, so it never
  contributes — own-claims-countered (the slice-18 count) is excluded by the query shape, not by a
  filter that could be relaxed. This is the own+peer COMPLETION: own via `claims` outer (slice-18),
  peer via `peer_claims` outer (this slice); the two reads are INDEPENDENT siblings, no third
  dimension.
- **Literal `'counters'` only; parameter-free, injection-safe.** The only WHERE values are the
  literal `ref_type='counters'` constants — no caller input is interpolated. The query takes NO
  bind parameters, so the injection surface is zero.
- **Read-only by construction (C-1 CARDINAL).** It returns `Result<usize, _>`; the trait declares
  no mutation method, so a `Box<dyn StoreReadPort>` stays structurally incapable of mutating. It
  runs over the SAME shared connection the CLI writes through (BR-VIEW-4). LOCAL only — no network
  seam (C-2).
- **Invariant to store size (C-3).** ONE aggregate, both ref columns indexed (slice-12 ADR-048);
  the landing's read budget grows from 4 to 5, `peer_claims_page` adds 1. NO per-claim
  `counter_presence_for` loop.

**Rationale (count-only chosen over `counter_presence_for(peer_cids).len()`):** identical to
slice-18 ADR-055 D1.
- **Symmetry.** The landing's other four counts ARE count-only aggregates (`count_claims`,
  `count_peer_claims`, `count_active_peer_subscriptions`, `count_countered_own_claims`); a fifth
  count-only sibling makes the five-count summary five structurally similar aggregate reads.
- **Cheapness / honest no-N+1.** `counter_presence_for(peer_cids).len()` would first materialize
  every peer cid (`SELECT cid FROM peer_claims` into a `Vec<String>`), bind it twice into an `IN
  (?, …)` presence query, decode the presence SET, then throw it all away to take `.len()`. The
  count-only aggregate decodes ONE scalar and binds NOTHING.
- **Cost is one tiny read method** mirroring the four that already exist, plus one aggregate impl
  in `adapter-duckdb` shaped after slice-18's `count_countered_own_claims` (same tables, same
  filter, outer table `peer_claims`). It adds NO mutation method, NO new crate (workspace stays
  21), NO new route.

### D2 — A FIFTH additive `Option<usize>` field on `LandingSummary` (WD-PC-6 / missing≠zero)

`LandingSummary` gains a FIFTH field, parallel to the slice-17/18 four:

```
pub struct LandingSummary {
    pub own_claims: Option<usize>,
    pub peer_claims: Option<usize>,
    pub active_peers: Option<usize>,
    pub countered_own_claims: Option<usize>,   // slice-18
    pub countered_peer_claims: Option<usize>,  // slice-19 — additive
}
```

- `Some(n)` = a SUCCESSFUL read of `n` (including `Some(0)` — an honest "nothing of my cached peer
  material has drawn a counter"). `None` = the countered-peer-count read FAILED → the slice-17
  `MISSING_COUNT_MARKER` "—". `0 ≠ missing` stays a TYPE-LEVEL distinction; a fabricated 0 on a
  failed read is unrepresentable (C-5 / WD-PC-6).
- The field degrades INDEPENDENTLY: a failed countered-peer read leaves the four siblings
  `Some(_)` (incl. the slice-18 `countered_own_claims`), and vice versa. The per-count `.ok()`
  model (ADR-054 D2 / ADR-055 D4) extended to a fifth field. A transient countered-peer failure
  must NOT blank the slice-18 own-countered count the operator can legitimately see.
- **A fifth parallel `Option<usize>` is chosen over a richer ADT or a separate view-model**
  because the fifth count has IDENTICAL shape and IDENTICAL degrade semantics to the four — a
  fifth parallel field is the total, exhaustive, mutation-resistant model with the least ceremony,
  and keeps the pure render a TOTAL function of `LandingSummary` (now 2⁵ `Option` combinations;
  every one renders, no panic, no I/O). SAME reasoning as ADR-054 D1 / ADR-055 D2.

### D3 — The REUSED `render_countered` helper renders the count on BOTH peer surfaces from the SAME number (WD-PC-8 single source, WD-PC-10 no new helper)

The slice-18 `render_countered` helper (`viewer-domain` ~640) is REUSED verbatim — NO new helper:

```
pub fn render_countered(countered: Option<usize>) -> String  // → "(1 countered)" | "(0 countered)" | "(— countered)"
```

- **`render_landing`** renders it BESIDE the unchanged peer-claims line — the peer-claims
  `render_count(summary.peer_claims)` "4" is UNTOUCHED (additive awareness, never a re-weight —
  C-4): `(render_count(summary.peer_claims)) " peer claims " (render_countered(summary.countered_peer_claims))`
  → "4 peer claims (1 countered)". This EXACTLY mirrors how slice-18 extended the own line; the
  slice-18 own line itself is UNTOUCHED (WD-PC-7).
- **The `/peer-claims` header** (`render_peer_claims_page`) renders the SAME helper output beside
  the "Peer Claims" `h1`. `render_peer_claims_page` gains a `countered_peer_claims: Option<usize>`
  parameter (the SAME `Option` the landing uses), mirroring slice-18's `render_claims_page`
  parameter. The list body — the slice-06/07 ordering (`composed_at DESC, cid`), paging, total
  count, every row's verbatim confidence, the slice-13 per-row flags, the peer origin — is
  UNTOUCHED (the header count is ADDITIVE — C-4 / WD-PC-9). The `Shape::Fragment` htmx swap path is
  UNTOUCHED (the header count is full-page chrome).
- **Single source (WD-PC-8).** Both surfaces resolve the count from the SAME
  `count_countered_peer_claims` read and render through the SAME `render_countered` helper, so the
  landing "(N countered)" and the `/peer-claims` header "(N countered)" cannot diverge in COPY and
  are pinned EQUAL for the same store by a gold test. The two routes resolve the count
  INDEPENDENTLY per render (separate `GET` handlers) — that is correct: the single source is the
  READ METHOD + the RENDER HELPER, not a cached value.

### D4 — The effect shell resolves the count per route via a 4th DISTINCT fault seam + `.ok()`

- `landing_page` adds a FIFTH `.ok()` resolution:
  `countered_peer_claims: countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok()`
  — building the extended `LandingSummary`. A failed read → `None` → the missing marker, the other
  four counts + the nav hub intact, always 200 (C-2 / NFR-VIEW-6).
- `peer_claims_page` resolves the SAME read and passes it into
  `render_peer_claims_page(&page_view, countered_peer)` (the `Shape::FullPage` arm). The
  countered-peer read is INDEPENDENT of the list read + the slice-13 presence read: a failed
  countered-peer read renders the header missing marker while the rows + per-row flags still
  render. Always 200, never a 5xx, never a fabricated "(0 countered)".
- **A 4th DISTINCT fault-seam token** `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` (seam fn
  `countered_peer_count_with_fault_seam`, `#[cfg(debug_assertions)]`-gated, the release identity)
  is added — NOT a reuse of the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`. A distinct token
  lets a behavioral test fail the PEER count INDEPENDENTLY of the own count, so the missing≠zero AT
  can assert "the slice-18 own count still renders while the peer count degrades". The xtask
  `VIEWER_FAIL_SEAM_TOKENS` array gains the 4th entry so the release-build guard covers it.

## Alternatives Considered

### For D1 (read shape, WD-PC-5)

- **`counter_presence_for(all_peer_cids).len()`** — zero new port surface, but materializes every
  peer cid into a `Vec<String>`, binds it twice into an `IN (?, …)` presence query, and decodes
  the presence SET just to count it. REJECTED: asymmetric with the four count-only landing reads,
  and does throwaway materialization. Viable and correct at dogfood scale, but the count-only
  aggregate is cleaner and cheaper — the SAME trade-off ADR-054 D3 / ADR-055 D1 resolved.
- **`UNION ALL` in the inner subquery instead of `UNION`** — the duplicate `referenced_cid` is
  harmless under `IN`, but REJECTED in favour of `UNION` for clarity of intent (the DISTINCT set
  of countered cids), exactly as slice-18 decided. The presence count is doubly defended (the
  de-duped IN-set + the outer `COUNT(DISTINCT p.cid)`).

### For D2 (degrade model)

- **An all-or-nothing `Result<Summary, _>`** — a single failed count would blank ALL five
  numbers. REJECTED: violates the per-count independent degrade and risks the 5xx C-2 forbids.
- **Fabricate 0 on a failed countered-peer read (`unwrap_or(0)`)** — REJECTED: misleads "nothing
  of my cached peer material is disputed" (R-PC-1/C-5). `Option` makes `0 ≠ missing` representable.

### For D3 (render — new helper vs reuse)

- **A new `render_countered_peer` helper** — REJECTED: the peer parenthetical has the IDENTICAL
  copy + missing-marker behaviour as the own parenthetical; a second helper would duplicate the
  single mutation-killable copy site WD-PC-10 mandates be ONE place. The slice-18 helper is reused
  verbatim.

### For D4 (fault seam — reuse vs distinct token)

- **Reuse the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` token for the peer count too** —
  REJECTED: setting it would fail BOTH the own-countered AND the peer-countered reads at once, so
  the missing≠zero AT for the PEER count could not assert independent degrade (it could not
  distinguish a peer-only failure from a both-failure). A 4th distinct token mirrors how each prior
  count got its own seam (slices 16/17/18), keeping the two reads independently fault-injectable.

### For scope (WD-PC-7 / BR-PC-4)

- **Also add a third dimension or re-touch the slice-18 own count** — REJECTED + surfaced: this
  slice adds JUST the peer count; the slice-18 own surfaces (landing own line + `/claims` header)
  are UNTOUCHED. own+peer is now COMPLETE; there is no third dimension.

## Consequences

### Positive
- `0 ≠ missing` for the countered-peer count is a TYPE-LEVEL invariant (`Option<usize>`); a
  fabricated 0 on failure is unrepresentable (C-5).
- Independent degrade: the countered-peer count can fail without blanking the four siblings (incl.
  the slice-18 own-countered count), the nav hub, or the `/peer-claims` rows/flags, and never
  5xxes (C-2 honored); a distinct 4th fault-seam token makes that independence testable.
- Presence count by construction (de-duped IN-set + DISTINCT, no JOIN-fanout); a peer claim
  countered N times counts ONCE; the peer-claims count is never re-weighted (C-4).
- ONE additional aggregate read per render on each surface (landing 4→5; `peer_claims_page` +1),
  invariant to store size, both ref columns indexed (no N+1, C-3).
- Single source: the landing peer line and `/peer-claims` header render the SAME number through
  the SAME (REUSED) `render_countered` helper; the copy cannot diverge (WD-PC-8); pinned equal by
  a gold test.
- Additive on `/peer-claims`: the slice-06/07 list order/paging/count/confidence + the slice-13
  per-row flags + the peer origin are byte-identical to the no-header-count baseline (C-4 /
  WD-PC-9); the htmx fragment path is untouched.
- Neutral anti-misread copy in the ONE mutation-killable helper site SHARED with slice-18 (C-6 /
  WD-PC-10).
- own+peer COMPLETION: the slice-18 own surfaces are UNTOUCHED (WD-PC-7); no third dimension.
- Read-only preserved: no mutation method added; workspace stays 21; no new crate; no new route;
  no new render helper; loopback bind unchanged; nothing persisted.

### Negative / trade-offs
- One new read method on `StoreReadPort` (`count_countered_peer_claims`) + one aggregate impl in
  `adapter-duckdb` (vs zero new surface for `counter_presence_for(...).len()`). Accepted: one
  trivial read-only method buys symmetry + cheapness; mirrors ADR-055 D1.
- `render_peer_claims_page` gains a parameter (`countered_peer_claims: Option<usize>`), a signature
  change rippling to its callers/tests. Accepted: the SAME number must drive the header, and a
  parameter is the pure, single-source way to pass it (mirrors slice-18 `render_claims_page`).
- A 4th viewer fault-seam token is added. Accepted: independent per-count fault injection is the
  established pattern (slices 16/17/18) and is required for the missing≠zero AT to assert
  per-count degrade.

## Enforcement (Earned Trust — how the design proves it honors its contract)

- **Read-only (3 layers, cardinal C-1):** (a) TYPE — `StoreReadPort` declares no mutation method;
  the added `count_countered_peer_claims` returns `Result<usize, _>`, so a `Box<dyn StoreReadPort>`
  remains structurally incapable of mutating. (b) xtask `check_viewer_capability_boundary`
  (crate-dependency-graph rule) stays GREEN — a read-only count method changes no dependency edge.
  (c) BEHAVIORAL gold — port-to-port tests assert `/` and `/peer-claims` contain no
  `<form>`/`<button>`/mutating control (the countered-peer count is render-only text), and that
  store contents are byte-identical before/after N requests.
- **Anti-merging SQL rule (`no_cross_table_join_elides_author`, xtask) — R-PC-9 RESOLVED, GREEN by
  construction.** The new SQL is verified against the classifier source
  (`xtask/src/check_arch.rs::classify_sql_literal` ~319, `contains_word` ~289, `is_word_byte`
  ~308). `mentions_peer_claims = contains_word(literal, "peer_claims")` is TRUE (the outer
  `FROM peer_claims p` is a whole-word match — the NEW wrinkle vs slice-18, whose outer was
  `claims`). `mentions_own_claims = contains_word(literal, "claims")` is FALSE: the only `claims`
  substrings are inside `peer_claims` (preceded by the word byte `_` → boundary fails) and there is
  NO standalone `claims` table (`claim_references` / `peer_claim_references` are `claim_…`, no
  `claims` substring at all). `is_cross_store = mentions_peer_claims && mentions_own_claims = TRUE
  && FALSE = FALSE`, so the classifier returns `None`. **No violation.** The rule fires ONLY when a
  literal names BOTH the standalone `claims` table AND `peer_claims` as whole words (a merging JOIN
  that elides attribution); this query names `peer_claims` but NOT standalone `claims` — it is a
  presence COUNT over ONE store's peer-claim cids with a subquery membership test over the indexed
  REF tables, no merging JOIN, no `author_did` to elide. The index-store aggregation variant
  (`mentions_aggregation`) scans `crates/adapter-index-store/src` ONLY, not `adapter-duckdb`, so
  the `COUNT(DISTINCT …)` does not trip it. The xtask SQL boundary is UNCHANGED by this slice.
- **Fault-seam release-build guard (xtask `VIEWER_FAIL_SEAM_TOKENS`, ~1042):** the new
  `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` token is APPENDED to the guarded set; the
  `scan_viewer_fail_seam_guard` `classify_cfg_gated_token` pass then requires every read of it to
  sit behind `#[cfg(debug_assertions)]` — an ungated read (a degrade backdoor in a release binary)
  fails the guard, identical to the three existing tokens. Earned Trust applied to the seam itself.
- **Missing ≠ zero (C-5 / WD-PC-6):** behavioral tests seed an unreadable countered-peer count
  (via the new token) and assert `/` and `/peer-claims` render the missing marker for the
  countered-peer count (distinct from a seeded real 0), the sibling counts/rows/flags still render,
  and the route returns 200 — never a 5xx, never a raw stack trace. Earned Trust applied to the
  store-read dependency: the design exercises the substrate LYING (a transient `StoreReadError`)
  and proves both peer orientation surfaces survive it.
- **Presence count, counted once (C-4):** a behavioral test seeds a peer claim countered by TWO
  counterers (Maria + Rachel) and asserts the landing + header render "(1 countered)", not
  "(2 countered)". Earned Trust applied to the de-dup contract.
- **Single source (WD-PC-8):** a gold test asserts the landing peer "(N countered)" == the
  `/peer-claims` header "(N countered)" for the same store.
- **No regression on `/peer-claims` (C-4 / WD-PC-9):** a gold test asserts the `/peer-claims` list
  order/paging/count/confidence/per-row-flags/origin is byte-identical to the no-header-count
  baseline.
- **No regression on the slice-18 OWN surfaces (WD-PC-7 / BR-PC-4):** a gold test asserts the
  landing own line + the `/claims` header still render "(N countered)" unchanged.
- **No N+1 (C-3):** a `@property`-shaped behavioral test asserts the countered-peer-count read
  count is invariant to store size.
- **No network (C-2):** a behavioral test asserts `/` and `/peer-claims` render fully network-down,
  make no outbound request, and reference only the vendored `/static/htmx.min.js`.
</content>
