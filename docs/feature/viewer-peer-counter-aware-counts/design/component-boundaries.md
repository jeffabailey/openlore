# Component Boundaries: viewer-peer-counter-aware-counts (slice-19)

> Wave: DESIGN (lean) · Owner: Morgan · 2026-06-10 · ADR: ADR-056
> Functional paradigm (ADR-007): the boundary is pure render core vs effect shell.
> NO new crate · NO new route · workspace stays 21 members.

## 1. Crate touch map (what changes, what does not)

| Crate | Role | Change in slice-19 |
|---|---|---|
| `viewer-domain` (PURE) | The render core | Add a FIFTH `Option<usize>` field `countered_peer_claims` to `LandingSummary`. `render_landing` renders `render_countered(summary.countered_peer_claims)` BESIDE the UNCHANGED peer-claims line (currently `p { (render_count(summary.peer_claims)) " peer claims" }` ~677). `render_peer_claims_page` gains a `countered_peer_claims: Option<usize>` param and renders the SAME helper in its header (currently `h1 { "Peer Claims" }` ~1170). NO new helper — REUSES the slice-18 `render_countered` (~640). The slice-18 own line (~673-676) + the `/claims` header (~389) are UNTOUCHED. |
| `adapter-http-viewer` (EFFECT) | The HTTP shell | `landing_page` adds a 5th `.ok()` resolution (`countered_peer_claims: countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok()`) to the `LandingSummary`. `peer_claims_page` resolves `countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok()` and passes it into `render_peer_claims_page(&page_view, countered_peer)` (the `Shape::FullPage` arm; the `Shape::Fragment` arm is UNTOUCHED). Add ONE new cfg-gated fault seam `countered_peer_count_with_fault_seam` (4th token — see §6). |
| `ports` (TRAIT) | `StoreReadPort` | Add ONE read-only method: `count_countered_peer_claims(&self) -> Result<usize, StoreReadError>` (WD-PC-5 → count-only aggregate, ADR-056 D1 — the 5th count-only sibling). NO mutation method added. |
| `adapter-duckdb` (ADAPTER) | DuckDB impl | Implement `count_countered_peer_claims`: `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')`. One aggregate read; NO bind parameters (literal `'counters'` only). The inner `UNION` IN-set is IDENTICAL to slice-18's `count_countered_own_claims` (~517); ONLY the outer table differs (`claims c` → `peer_claims p`). |
| `xtask` (ARCH RULES) | check-arch | `no_cross_table_join_elides_author`: GREEN by construction (see §5 — R-PC-9 RESOLVED). `VIEWER_FAIL_SEAM_TOKENS`: gains the 4th token `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` (the new fault seam — see §6). `check_viewer_capability_boundary` is a crate-dependency-graph rule (a read-only count touches no dependency edge) — UNCHANGED. |
| All other crates | — | UNCHANGED. No new crate. Workspace stays 21. |

## 2. The pure / effect boundary (ADR-007)

```
                 EFFECT SHELL                              PURE CORE
   adapter-http-viewer::landing_page              viewer-domain::render_landing
   adapter-http-viewer::peer_claims_page          viewer-domain::render_peer_claims_page
   ──────────────────────────────────             ──────────────────────────────────
   reads count_countered_peer_claims (I/O, Result) total fns of Option<usize> (+ the
   maps Result→Option via .ok()         ──►       slice-17 LandingSummary / the PageView)
   builds LandingSummary (5th field) /            Some(n) → "(n countered)"
     passes Option into render_peer_claims_page    None    → "(— countered)"
   wraps html_ok (200)                            render via the REUSED render_countered helper
```

- **The I/O (the countered-peer read + the `Result→Option` mapping) lives ONLY in the shell.**
  The pure core never sees a `Result`, a `StoreReadError`, or the store — it receives an
  already-resolved `Option<usize>` (on `LandingSummary`, or as the `render_peer_claims_page`
  param) and renders totally. This is the slice-17/18 pure-render discipline unchanged.
- **The degrade decision (`.ok()`) is an effect-shell decision** (how to treat an I/O failure),
  mirroring slice-18's `countered_own_claims` `.ok()` and slice-13's `unwrap_or_default()`. The
  pure render only knows "this count is `Some` or `None`" and renders each totally — now over 2⁵
  `Option` combinations on `LandingSummary`, every one a defined render (no panic, no I/O).

## 3. The new read (one count-only aggregate; the data is reused) — WD-PC-5 RESOLVED

| Count | Read | Status | Source |
|---|---|---|---|
| own claims | `count_claims()` | EXISTING | slice-06, `ports` ~296 |
| peer claims | `count_peer_claims()` | EXISTING | slice-06, `ports` ~316, `adapter-duckdb` ~488 |
| active peers | `count_active_peer_subscriptions()` | EXISTING (slice-17) | ADR-054 D3, `adapter-duckdb` ~498 |
| countered OWN claims | `count_countered_own_claims()` | EXISTING (slice-18) | ADR-055 D1, `ports` ~354, `adapter-duckdb` ~517 |
| **countered PEER claims** | **`count_countered_peer_claims()`** | **NEW (count-only, read-only)** | ADR-056 D1; the EXACT slice-18 SQL with outer `claims c → peer_claims p`; the DATA (the `claim_references ∪ peer_claim_references`, `ref_type='counters'` ref tables) is the slice-12 `counter_presence_for` data, read as a COUNT |

**WD-PC-5 RESOLVED → count-only aggregate (the 5th sibling), recommended + adopted.** The DISCUSS
open question offered (a) the count-only aggregate `count_countered_peer_claims()` vs (b) reuse
`counter_presence_for(all_peer_cids).len()`. DESIGN chooses (a), for the SAME two reasons
slice-18 ADR-055 D1 chose it for own claims:
- **Symmetry.** The landing's other four counts ARE count-only aggregates (`count_claims`,
  `count_peer_claims`, `count_active_peer_subscriptions`, `count_countered_own_claims`); a fifth
  count-only sibling makes the five-count summary five structurally similar aggregate reads.
- **Cheapness / honest no-N+1.** `counter_presence_for(peer_cids).len()` would first materialize
  every peer cid (`SELECT cid FROM peer_claims` into a `Vec<String>`), bind it twice into an `IN
  (?, …)` presence query, decode the presence SET, then throw it away to take `.len()`. The
  count-only aggregate decodes ONE scalar and binds NOTHING.

The countered-peer-claims count is the ONLY net-new read surface. The DATA is REUSED (the slice-12
counter-reference tables); only the read SHAPE (a single `COUNT(DISTINCT …)` aggregate, outer
table `peer_claims`) is new. It is a READ method: `StoreReadPort` still declares no mutation
method (I-VIEW-1), so the read-only invariant is preserved by construction.

## 4. The new read's effect on `ports` / `adapter-duckdb`

- `ports`: +1 trait method signature (`count_countered_peer_claims`). Sync read posture unchanged
  (a sync read like the other four counts).
- `adapter-duckdb`: +1 impl — `conn.query_row("SELECT COUNT(DISTINCT p.cid) FROM peer_claims p
  WHERE p.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION
  SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')", [], |row|
  row.get(0))` mapping a failure to `StoreReadError::QueryFailed`, mirroring
  `count_countered_own_claims` (~517). NO bind parameters (literal `'counters'` only —
  parameter-free, injection-safe).
- Any OTHER `StoreReadPort` impl (test doubles, the slice-17/18 fakes) gains the same method;
  trivially a count, no mutation.

## 5. xtask boundary — the anti-merging SQL rule is GREEN by construction (R-PC-9 RESOLVED, VERIFIED)

The slice's flagged constraint (R-PC-9) is whether the new SQL trips the
`no_cross_table_join_elides_author` rule now that the OUTER table is `peer_claims` (slice-18's
outer was `claims`). VERDICT: it does NOT trip it. Verified directly against
`xtask/src/check_arch.rs::classify_sql_literal` (lines 319-336), `contains_word` (289-304), and
`is_word_byte` (308-310).

The classifier computes (verbatim from source):

```rust
let mentions_peer_claims = contains_word(literal, "peer_claims");
let mentions_own_claims  = contains_word(literal, "claims");
let is_cross_store = mentions_peer_claims && mentions_own_claims;
if !is_cross_store { return None; }                 // ← the new SQL returns here
if contains_word(literal, "author_did") { return None; }
Some(SqlAntiMergingViolation { … })
```

Trace the new literal
`SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')`:

- **`mentions_peer_claims = contains_word(literal, "peer_claims")` → TRUE.** The outer
  `FROM peer_claims p`: the byte before `peer_claims` is a space (not a word byte), the byte
  after is a space (not a word byte) → both boundaries pass → whole-word match.
  **This is the one new wrinkle vs slice-18** — slice-18's outer was `claims`, so its
  `mentions_peer_claims` was FALSE. Here it is TRUE.
- **`mentions_own_claims = contains_word(literal, "claims")` → FALSE.** Check EVERY occurrence of
  the substring `claims` in the literal:
  1. `FROM peer_claims p` and `peer_claim_references`: in `peer_claims`, the byte BEFORE the
     `claims` substring is `_` (the underscore of `peer_`), and `is_word_byte(b'_')` is TRUE →
     `before_ok` is FALSE → NOT a whole-word `claims`.
  2. `claim_references` / `peer_claim_references`: these contain the token `claim` (no trailing
     `s`) — the substring `claims` does NOT appear in `claim_references` at all (it is
     `c-l-a-i-m-_-r-e-f…`). So no match candidate there.
  3. There is NO standalone `claims` table named anywhere in this query (the outer table is
     `peer_claims`, not `claims`).
  Therefore `contains_word(literal, "claims")` finds no whole-word occurrence → **FALSE**.
- **`is_cross_store = mentions_peer_claims && mentions_own_claims = TRUE && FALSE = FALSE`.**
- `if !is_cross_store { return None; }` → the classifier **returns `None`. No violation. GREEN.**

**Why it is still GREEN despite the new wrinkle.** slice-18 was GREEN with `(peer=F, own=F)`;
slice-19 is GREEN with `(peer=T, own=F)`. Both reach `None` because `is_cross_store` is the
LOGICAL AND of the two flags, and the rule only fires when a literal names BOTH the standalone
`claims` table AND the `peer_claims` table as whole words (a merging JOIN that elides
attribution). The new SQL names `peer_claims` (TRUE) but NOT standalone `claims` (the
`peer_claims`/`claim_references`/`peer_claim_references` tokens all fail the `claims` whole-word
boundary). With only ONE of the two flags set, the AND is FALSE → `None`. Semantically: the rule
guards a JOIN merging `claims`+`peer_claims` attribution; this query is a presence COUNT over ONE
store's peer-claim cids with a subquery membership test over the indexed REF tables — there is no
`claims`+`peer_claims` merging JOIN and no `author_did` to elide (a count has no attribution).

Note: the `peer_claim_references` table name DOES contain `peer_claim`, but `contains_word(…,
"peer_claims")` needs the trailing `s` + a non-word boundary — `peer_claim_references` is
`peer_claim_…` (no `s` after `claim`), so it does NOT contribute the `mentions_peer_claims` match;
the match comes SOLELY from the outer `FROM peer_claims p`. Either way, `mentions_own_claims`
stays FALSE, so the verdict is `None`.

The index-store aggregation variant (`mentions_aggregation` /
`classify_index_store_sql_literal`) scans `crates/adapter-index-store/src` ONLY (not
`adapter-duckdb`, see `scan_adapter_index_store_sql` ~908), so the `COUNT(DISTINCT …)` in this
`adapter-duckdb` impl does not trip it.

`check_viewer_capability_boundary` (the transitive-crate-dependency rule) also stays GREEN —
adding a read-only count method to `StoreReadPort` changes no crate dependency edge (the viewer
still depends on no write/sign adapter). The read-only invariant's THREE enforcement layers
(ADR-056 Enforcement) hold unchanged: (1) type — the trait has no mutation method; (2) xtask —
this capability rule; (3) behavioral gold — `/` and `/peer-claims` have no mutating control +
store unchanged after N requests.

## 6. The fault seam — a 4th DISTINCT token (DECISION: distinct token, NOT reuse)

DECISION (the brief's open sub-question): add a 4th DISTINCT viewer fault-seam token
`OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` with its own seam fn
`countered_peer_count_with_fault_seam`, rather than REUSE the slice-18
`OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` / `countered_count_with_fault_seam` for the peer count
too.

**Rationale.** Each viewer count has its OWN fault-seam token so a behavioral test can fail ONE
count's read INDEPENDENTLY and assert the missing≠zero degrade is per-count, not all-or-nothing.
The existing token set in `VIEWER_FAIL_SEAM_TOKENS` (`xtask/src/check_arch.rs` ~1042) is exactly
this pattern: `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` (slice-16),
`OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` (slice-17),
`OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` (slice-18). If slice-19 REUSED the slice-18 token, setting
it would fail BOTH the own-countered AND the peer-countered reads at once, so the missing≠zero AT
for the PEER count could not distinguish a peer-only failure (and a test asserting "the own count
still renders while the peer count degrades" would be impossible to write). A distinct 4th token
keeps the two reads independently fault-injectable, exactly mirroring how each prior count got its
own seam.

- New seam fn `countered_peer_count_with_fault_seam` in `adapter-http-viewer`, `#[cfg(debug_assertions)]`-gated
  (the release build is the identity, NO env-var read compiled in), reading
  `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` and substituting `Err(StoreReadError::Unreadable
  { … })`. EXACT structural mirror of `countered_count_with_fault_seam` (~514).
- `xtask` `VIEWER_FAIL_SEAM_TOKENS` (~1042) gains a 4th entry
  `"OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT"`, so the release-build guard
  (`scan_viewer_fail_seam_guard`) covers it — any UNGATED read of the new token is a guard
  violation, identical to the three existing tokens.
- Wired around the countered-peer read in BOTH `landing_page` and `peer_claims_page` so a single
  injected failure exercises both surfaces (mirroring slice-18 D4).

## 7. What stays out (boundary guards)

- NO new route — `GET /` and `GET /peer-claims` exist; the route arms only gain the countered-peer
  read + render.
- NO new crate — workspace stays 21.
- NO new render helper — `render_countered` is REUSED (slice-18, single SSOT copy site).
- NO mutation method, NO signing key, NO write/compose/sign/subscribe/follow control. The
  countered-peer count is render-only text, never a sort/filter/mutating control (C-1 CARDINAL).
- NO network seam — one LOCAL aggregate read.
- NO re-touching the slice-18 OWN surfaces — the landing own line + the `/claims` header still
  render "(N countered)" unchanged (WD-PC-7 / BR-PC-4); no third dimension.
- NO re-order/filter/re-weight/re-page of the `/peer-claims` list — the header count is ADDITIVE;
  the slice-06/07 ordering/paging/count + the slice-13 per-row flags + the peer origin are
  byte-identical to the no-header-count baseline (C-4 / WD-PC-9). The `Shape::Fragment` htmx swap
  path is UNTOUCHED (the header count is full-page chrome).
- NO re-weight of the peer-claims count — the "4" is unchanged (C-4).
- NO counter CONTENT (authors/reasons/threads) in the count — reading WHO countered WHAT stays the
  slice-11 thread + the slice-13 per-row flags.
</content>
