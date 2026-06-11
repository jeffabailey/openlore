# Evolution: viewer-peer-counter-aware-counts (slice-19 read-only countered-PEER-claims count on the `GET /` landing peer-claims line + the `/peer-claims` list header — at-a-glance "how many of my cached peer claims have been disputed" on the viewer; the deferred WD-CC-7 PEER sibling of slice-18)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-peer-counter-aware-counts/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-056 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> **slice-18 (`viewer-counter-aware-counts`) — the OWN-claims sibling this slice mirrors
> exactly, and the home of the `render_countered(Option<usize>)` helper, the
> `count_countered_own_claims` SQL pattern, the additive-`Option<usize>`-`LandingSummary`-field
> shape, and the `#[cfg(debug_assertions)]` fault-seam + xtask token-set pattern this slice
> reuses verbatim**, slice-17 (`viewer-landing-dashboard` — the read-only `GET /` landing this
> slice extends, the home of the `LandingSummary` ADT + `render_count` + the `.ok()`-degrade
> count-resolution pattern), the **counter-flag family** slices 11-14
> (`viewer-counter-claim-list-flags`, `viewer-counter-claim-threads`,
> `viewer-counter-flags-graph-surfaces`, `viewer-counter-flags-score-surface` — the source
> of the counter-reference data this slice aggregates, the **slice-13 per-row "Countered" flags**
> a reader drills into on `/peer-claims`, and the slice-14 neutral-framing sensibility this count
> reuses), and slice-16 (`viewer-search-follow-state` — the origin of the **`#[cfg(debug_assertions)]`
> test-only fault-seam + the `xtask check-arch` seam guard** pattern, generalized through
> slices 17-18). Read those parent archives
> (`docs/evolution/viewer-counter-aware-counts-evolution.md`,
> `viewer-landing-dashboard-evolution.md`,
> `viewer-counter-claim-list-flags-evolution.md`,
> `viewer-counter-claim-threads-evolution.md`,
> `viewer-counter-flags-graph-surfaces-evolution.md`,
> `viewer-counter-flags-score-surface-evolution.md`,
> `viewer-search-follow-state-evolution.md`) for the surfaces this slice composes.
> slice-19 realizes the **orientation / at-a-glance facet of J-003b for PEER claims** —
> turning the front-door peer-claims count into a *countered*-aware count, so a reader sees
> not just how many peer claims are cached but how many of them have been disputed (by the
> operator's own counter OR another peer's), and can drill into the **slice-13-flagged
> `/peer-claims` rows**. **Together slice-18 (own) + slice-19 (peer) COMPLETE the
> counter-aware orientation across BOTH own and peer claims.** This was the **deferred
> WD-CC-7 sibling**.

## Summary

`viewer-peer-counter-aware-counts` enriches the `openlore ui` read-only viewer's **`GET /` landing
peer-claims line** and the **`/peer-claims` list header** with a **countered-PEER-claims count** —
rendering "4 peer claims (1 countered)". It is the **PEER sibling of slice-18 (which did
own-claims)**: together they **COMPLETE the counter-aware orientation across BOTH own and peer
claims**. Before this slice the landing told the reader *how many* peer claims were cached; now it
also tells them *how many of those cached peer claims have been disputed* (by the operator's own
counter OR another peer's) — and the **slice-13-flagged `/peer-claims` rows** are the drill-in
target. The slice realizes the **orientation / at-a-glance facet of J-003b for peer claims**, and
discharges the **deferred WD-CC-7 sibling**. It REUSES the slice-17 `LandingSummary` + the slice-18
`render_countered` helper / SQL pattern / fault-seam pattern and the slice-12/13 counter-reference
data, and adds **ONE new read-only count**, `count_countered_peer_claims`.

The load-bearing thesis: **a reader's front door surfaces not just cached peer-claim volume but
peer-claim contestation — read-only, LOCAL, and at exactly one aggregate read of cost.** The
countered-peer count is render-only text beside the unchanged peer-claims line; it never re-weights
or re-orders anything, and **the slice-18 own surfaces are UNTOUCHED**. The CARDINAL concerns are
six, **all mirroring slice-18 applied to peer claims**: (1) **read-only / no-key** — a render-only
count text; no write/sort/filter control, the count NEVER re-weights or re-orders the peer claims,
**and the slice-18 own surfaces are UNTOUCHED**; (2) **LOCAL / offline** — a LOCAL aggregate over the
counter-reference tables, no network; (3) **no-N+1** — ONE aggregate read per render, invariant to
store size (the landing's read budget grows to **5 counts**, by exactly 1; proven by a
**120-plain-peer-claim bulk seed**); (4) **missing≠zero** — a **5th** additive `Option<usize>` field
on `LandingSummary`, `Some(0)` → "(0 countered)" honest zero, `None` → "(— countered)" missing
marker, a fabricated `0` unrepresentable; independent per-count `.ok()` degrade, **never 5xx, never
blanks the 4 sibling counts incl. the slice-18 own-countered**; (5) **presence-count** — a peer claim
countered by N counterers counts ONCE, via `COUNT(DISTINCT)` + an IN-set (either ref-table arm
contributes once); (6) **anti-misread** — neutral "(N countered)", reusing the slice-18/14
`render_countered` helper — no penalty / deduction / "disputed by N" / verdict.

The slice ships **ZERO new crates** (workspace stays at **21 members**) and **ZERO new routes**
(it thickens the existing `GET /` landing peer-claims line and the existing `/peer-claims` header).
It is an **additive enrichment of two existing surfaces, not a re-architecture**: it extends
`viewer-domain` (the **REUSED** `render_countered(Option<usize>)` helper — NO new helper — now also
used by the landing peer line + `render_peer_claims_page`'s header, and the 5th `LandingSummary`
field), `adapter-http-viewer` (resolving the new count into both surfaces via `.ok()` degrade + a
**distinct 4th fault seam**), the `adapter-duckdb` read impl (ONE new count query), the `ports` (one
read seam + the 5th `LandingSummary` field), and the `cli` (`ui` wiring, still no key). The one new
read method is **read-only on the existing `StoreReadPort`**. The defining design choice is the
**5th additive `Option<usize>` field on `LandingSummary`** — which keeps a fabricated `0`
unrepresentable — together with the **REUSED `render_countered` helper** that is the single SSOT copy
site for both the landing peer line and the `/peer-claims` header.

### What shipped (one paragraph)

Two enriched surfaces — the **`GET /` landing peer-claims line** and the **`/peer-claims` list
header** — each now render a **countered-PEER-claims count** ("4 peer claims (1 countered)"). On
request each surface resolves the NEW LOCAL count `count_countered_peer_claims` (independently, per
render) via **`.ok()` degrade** and projects it through the **REUSED `render_countered(Option<usize>)`**
helper — `Some(n)` → "(n countered)" (a genuine zero renders "(0 countered)"), `None` → "(—
countered)" (the missing marker) — so a fabricated `0` is unrepresentable. The count is added as the
**5th additive `Option<usize>` field on `LandingSummary`**, beside the unchanged
own-claims/peer-claims/active-peers/**slice-18 own-countered** fields; its degrade is **independent**
(a failed countered-peer count never sinks the page, never 5xx, and never blanks the 4 sibling counts
— including the slice-18 own-countered). The new `count_countered_peer_claims = SELECT
COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM claim_references
WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE
ref_type='counters')` is **the EXACT slice-18 `count_countered_own_claims` SQL with the outer table
swapped `claims`→`peer_claims`** — **count-only**, **presence-once** by `COUNT(DISTINCT)` + the IN-set
(a peer claim countered by N counterers counts once; either ref-table arm contributes once),
**peer-only** by the outer `peer_claims` table (a peer claim is countered when its cid appears as a
countered `referenced_cid`, by the operator's own counter OR another peer's). The SQL is
**parameter-free / injection-safe** and **xtask R-PC-9 GREEN by construction**: it names `peer_claims`
(a whole word — the **one wrinkle** vs slice-18 whose outer was `claims`) but NOT the standalone
`claims`, so the `no_cross_table_join_elides_author` classifier returns `None` (`is_cross_store =
mentions_peer_claims AND mentions_own_claims = TRUE AND FALSE = FALSE`). The **REUSED `render_countered`
helper** (NO new helper — single SSOT across the landing peer line + the `/peer-claims` header,
WD-PC-8/10) is used by BOTH `render_landing` (beside the unchanged peer-claims line) AND
`render_peer_claims_page`'s `/peer-claims` header — both routes resolve the count INDEPENDENTLY per
render and render via the SAME helper, so the two surfaces **cannot diverge**. The store read is
**LOCAL and read-only** (offline, no network); the count is render-only and **never re-weights or
re-orders** the peer claims; the **slice-18 own surfaces are UNTOUCHED**; nothing is persisted; the
viewer holds no key; the framing is the **neutral "(N countered)"** reusing the slice-18/14
sensibility (no penalty / "disputed by N" / verdict). A **distinct 4th
`OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` cfg-gated fault seam** lets the peer count fail
independently of slice-18's own count.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-10 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-10 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-10 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-10 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **7/7 roadmap steps** done across **3 phases** (all COMMIT/PASS in
  `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_peer_counter_aware_counts` corpus (PC- ids —
  including the **thick walking skeleton** at 01-01 driving the landing peer-claims-line countered
  count + the `/peer-claims` header) + the GOLD invariants
  (`viewer_peer_counter_aware_counts_invariants` — read-only / no-reweight-no-reorder /
  **own-untouched**, LOCAL / offline, N+1-free, anti-misread). Plus the new `adapter-http-viewer`
  in-crate unit tests (the countered-peer-seam pass-through, the Err-injection, and a
  `peer_claims_page` header-render test over a fake store) and the `viewer-domain` unit/property
  tests (the `render_countered` projection over the peer count + the 5th `LandingSummary` field).
  The `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through
  the REAL verbs.
- **Slices 06/11-14/16/17/18 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices; the slice-18 own surfaces UNTOUCHED).
- **NO new crate, NO new route**: extends `viewer-domain` (PURE — the REUSED `render_countered`
  helper + the 5th field) + `adapter-http-viewer` (EFFECT — the count resolution into both surfaces
  + the 4th fault seam) + `adapter-duckdb` (EFFECT, read impl) + `ports` (the read seam + the 5th
  `LandingSummary` field) + `cli` (DRIVER) in place. Workspace member count stays **21**;
  `cargo xtask check-arch` reports "21 workspace members".
- **NO new production dependency**: `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate on the genuinely-viable in-diff** (`viewer-domain` 4/4 caught;
  `adapter-http-viewer` 4/4 viable caught after adding 3 in-crate unit tests) — exceeds the ≥80%
  per-feature gate. The 2 remaining cargo-mutants "missed" are the `#[cfg(not(debug_assertions))]`
  release identity sibling of the peer fault seam (a cfg-dead-branch artifact, same class as
  slices 16/17/18's lone survivors), independently guarded.
- **1 ADR** (ADR-056) Accepted/shipped.
- DES integrity: **7/7** steps have complete DES traces.
- Adversarial review: **APPROVED**, 0 defects, zero Testing Theater (presence-once / single-source /
  additive / own-untouched / fault-seam-gating all verified).
- Gates: DoR 9/9, DESIGN APPROVED (R-PC-9 xtask-green verified line-by-line), DISTILL APPROVED.
- **Release build verified seam-free**: all **4** viewer fault tokens are ABSENT from the release
  rlib.
- `cargo xtask check-arch`: OK (21 workspace members; the `scan_viewer_fail_seam_guard`
  `VIEWER_FAIL_SEAM_TOKENS` set was extended to a **4th token** — an ungated read of ANY of the four
  fails check-arch).

## Wave-by-wave changelog

### DISCUSS (2026-06-10)

Luna framed the slice as a **brownfield DELTA on slice-18 (the own-claims counter-aware count) +
slice-17 (the landing) + the counter-flag family (slices 11-14)** that realizes **the orientation /
at-a-glance facet of J-003b for PEER claims**, discharging the **deferred WD-CC-7 sibling**. Persona
is **P-001 (the node operator)** opening the viewer to answer not just "how many peer claims are
cached?" but "how many of my cached peer claims have been disputed?". The load-bearing DISCUSS
decision: **mirror slice-18 exactly, applied to peer claims — enrich the existing peer-claims count
with a countered-aware count on BOTH the landing peer-claims line AND the `/peer-claims` header —
render-only, never re-weighting or re-ordering anything, and the slice-18 own surfaces UNTOUCHED**.
The CARDINAL framing insight: **slice-18 oriented the reader to OWN-claim contestation; slice-19 is
its PEER sibling, and together they COMPLETE the counter-aware orientation across BOTH own and peer
claims** — a reader at the front door now sees both how many of their own claims AND how many of
their cached peer claims have been disputed, and can drill into the slice-13-flagged `/peer-claims`
rows. Two scoping calls: **peer-claims-countered only — no third dimension; own+peer now complete**;
and **a count, not a list — `count_countered_peer_claims`, not a re-materialization**. The walking
skeleton is the thick thread (the new count read + the 5th `LandingSummary` field + the REUSED
`render_countered` helper + BOTH the landing peer-claims line and the `/peer-claims` header
threaded), validating the riskiest assumption first — that the slice-18 SQL pattern transplants to
peer claims presence-once and peer-only while staying read-only, never re-ordering, and leaving the
own surfaces untouched.

### DESIGN (2026-06-10)

Morgan locked slice-19 as an **additive enrichment of two existing surfaces, not a
re-architecture** — ZERO new crates, ZERO new routes, ZERO new binary, ZERO new architectural
style, ZERO new persisted type. The open decisions were resolved adopting the DISCUSS leans,
captured in one ADR:

- **ADR-056** (peer counter-aware counts — count-only countered-peer-claims read, 5th additive
  `LandingSummary` field, REUSED `render_countered` helper across both peer surfaces, presence-once
  aggregate, the slice-18 SQL with the outer table swapped `claims`→`peer_claims`): the NEW read
  **`count_countered_peer_claims = SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN
  (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid
  FROM peer_claim_references WHERE ref_type='counters')`** is **the EXACT slice-18
  `count_countered_own_claims` SQL with the outer table swapped `claims`→`peer_claims`** —
  **count-only**, **presence-once** by `COUNT(DISTINCT)` + the IN-set (a peer claim countered by N
  counterers counts once; either ref-table arm contributes once), **peer-only** by the outer
  `peer_claims` table (a peer claim is countered when its cid appears as a countered `referenced_cid`,
  by the operator's own counter OR another peer's), **parameter-free / injection-safe**, and
  **xtask R-PC-9 GREEN BY CONSTRUCTION**: it names `peer_claims` (a whole word — the **one wrinkle**
  vs slice-18 whose outer was `claims`) but NOT the standalone `claims`, so the
  `no_cross_table_join_elides_author` classifier returns `None` (`is_cross_store = mentions_peer_claims
  AND mentions_own_claims = TRUE AND FALSE = FALSE`). The count is the **5th additive `Option<usize>`
  field on `LandingSummary`**, keeping a fabricated `0` unrepresentable (`Some(0)` → "(0 countered)"
  honest zero, `None` → "(— countered)" missing marker). The view delta is the **REUSED
  `render_countered(Option<usize>)` helper** — NO new helper, ONE SSOT copy site delegating to
  slice-17's `render_count` — now also used by BOTH `render_landing` (beside the unchanged peer-claims
  line) AND `render_peer_claims_page`'s `/peer-claims` header; **both routes resolve the count
  INDEPENDENTLY per render and render via the SAME helper, so they cannot diverge (WD-PC-8/10)**. The
  framing is the **neutral "(N countered)"** reusing the slice-18/14 sensibility (no penalty /
  "disputed by N" / verdict — anti-misread). The **4th fault seam — PC-DEGRADE** (a failed
  countered-peer-count read → "(— countered)" on BOTH peer surfaces, no 5xx, never blanks the 4
  sibling counts) is exercised via a **distinct TEST-ONLY**
  `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` env seam honored ONLY by a `#[cfg(debug_assertions)]`
  function (the `#[cfg(not(debug_assertions))]` release sibling is the identity function, no env read
  compiled in), mirroring slice-16/17/18; the seam is **DISTINCT** from slice-18's own seam so the
  peer count fails independently. The slice-16/17/18 `scan_viewer_fail_seam_guard`
  `VIEWER_FAIL_SEAM_TOKENS` set is **extended to a 4th token**.

The read-only / no-reweight-no-reorder / own-untouched contract is enforced at THREE layers (a
`StoreReadPort` with no mutation method, the `xtask check-arch` viewer capability rule, and a
behavioral GOLD invariant — the count is render-only and never re-weights/re-orders, and the slice-18
own surfaces are untouched). The C4 views, the dual-surface data-flow, and the I-PC-1..n
structural-guarantee table are in the DESIGN sections of `feature-delta.md` and `design/`. DISTILL
closed at **APPROVED**.

### DISTILL (2026-06-10)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_peer_counter_aware_counts.rs`** (Tier A — `PC-` ids): the **thick walking skeleton**
  (**PC-WS** — the landing peer-claims line rendering the countered-peer count via the 5th
  `LandingSummary` field + the REUSED `render_countered`, driven by the new
  `count_countered_peer_claims` read), the **`/peer-claims` header + presence-once / either-table**
  (**PC-HEADER / PC-PRESENCE** — the `/peer-claims` header renders the SAME countered-peer count via
  the SAME helper, and a peer claim countered by N counterers counts ONCE, either ref-table arm
  contributing once), the **honest-zero** (**PC-ZERO** — a store with no countered peer claims renders
  "(0 countered)", not the missing marker), the **missing≠zero degrade** (**PC-DEGRADE** — a failed
  countered-peer-count read renders "(— countered)" on BOTH peer surfaces, NEVER a fabricated "0", NO
  5xx, never blanks the 4 sibling counts — driven by the distinct TEST-ONLY `#[cfg(debug_assertions)]`
  fault seam), the **no-reweight / no-reorder / own-untouched** (**PC-NO-REWEIGHT / PC-NO-REORDER /
  PC-INV-OwnUntouched** — the count never re-weights or re-orders the peer claims, and the slice-18
  own surfaces are untouched), and the **read-only / offline / N+1-free / anti-misread**
  (**PC-READONLY / PC-OFFLINE / PC-N+1 / PC-ANTI-MISREAD** — render-only count, LOCAL aggregate, ONE
  read per render proven by the 120-plain-peer-claim bulk seed, neutral framing).
- **`viewer_peer_counter_aware_counts_invariants.rs`** (gold guardrails — 10 GOLD invariants):
  **read-only / no-reweight-no-reorder / own-untouched** (the count is render-only; the peer-claims
  list order + weighting are unchanged across rich/empty/degraded renders, and the slice-18 own
  surfaces are untouched — **PC-INV-OwnUntouched**), **LOCAL / offline** (the countered-peer count
  reads the LOCAL counter-reference tables with no network), **N+1-free** (ONE FIXED aggregate read
  per render, invariant to store size — proven by the **120-plain-peer-claim bulk seed**; the
  landing's read budget grows to 5 counts, by exactly 1), and **anti-misread** (the neutral "(N
  countered)" framing — no penalty / deduction / "disputed by N" / verdict).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is seeded
through the REAL verbs (peer claims cached, then countered by the operator's own counter OR another
peer). The crafter caught a **real seed bug** at DISTILL: the distinct-peer counter inflated the
peer total — fixed via the operator-counter arm + a unified single-pull; and the no-N+1 bulk seed
needed **Rachel's PDS rebuilt** for the all-peers re-pull. The PC-DEGRADE fault is driven by the
distinct TEST-ONLY `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` env seam (honored only under
`debug_assertions`). RED classification: both targets COMPILE green, scenarios FAIL via `todo!()` /
unimplemented seam = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-10)

Executed **7 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton + `/peer-claims` header + presence/either-table + honest-zero
  (01-xx)**: **01-01 is the THICK walking skeleton** (**PC-WS**) — the new
  `count_countered_peer_claims` read + the 5th `LandingSummary` field + the REUSED `render_countered`
  helper + the landing peer-claims line threaded. **The thick WS drove the landing peer line into
  existence.** **01-02 (PC-HEADER / PC-PRESENCE)** threaded the `/peer-claims` header to render the
  SAME countered-peer count via the SAME helper (a real `peer_claims_page` `Option` param), and
  pinned presence-once + either-table (a peer claim countered by N counterers counts once, either
  ref-table arm contributing once) — **the `/peer-claims` header was real work, not confirmatory**;
  **01-03 (PC-ZERO)** the honest empty "(0 countered)".
- **Phase 02 — the genuinely-new 4th fault seam + no-reweight/own-untouched + no-reorder +
  offline/N+1/anti-misread (02-xx)**: **02-01** the **PC-DEGRADE** fault seam — **the real
  implementation work of the slice** (the distinct `#[cfg(debug_assertions)]`
  `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` env seam + the release identity sibling + the
  `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` set extended to a 4th token; a failed
  countered-peer-count read → "(— countered)" on BOTH peer surfaces, no 5xx, never blanks the 4
  sibling counts); **02-02** the **PC-NO-REWEIGHT / PC-INV-OwnUntouched / PC-NO-REORDER** (the count
  never re-weights or re-orders the peer claims; the slice-18 own surfaces UNTOUCHED) — **02-02 also
  fixed the no-N+1 bulk seed** (the all-peers re-pull needed Rachel's PDS rebuilt); **02-03** the
  **PC-READONLY / PC-OFFLINE / PC-N+1 / PC-ANTI-MISREAD** (render-only count, LOCAL aggregate, ONE
  read per render, neutral framing).
- **Phase 03 — gold (03-xx)**: **03-01** the **10 GOLD invariants** (read-only /
  no-reweight-no-reorder / **PC-INV-OwnUntouched**, LOCAL / offline, N+1-free, anti-misread). They
  flipped GREEN off the confirmatory render path.

The 7-step shape: a **thorough WS at 01-01** drove the landing peer line into existence (the 5th
additive field + the REUSED `render_countered` helper + the count read), but **01-02 (the
`/peer-claims` header threading — a real `peer_claims_page` `Option` param) was real work** (a second
surface had to resolve the count independently and render via the shared helper), and **PC-DEGRADE
(the distinct 4th fault seam + the guard token) was the real new implementation work**. The rest were
**unskipped per step** (the scaffolds were `#[ignore]`d) and confirmatory; 02-03 also fixed the
no-N+1 bulk seed. **Phase-3 refactor: none needed** — `render_countered` already SSOT (REUSED, no
new helper), and **two refactors were correctly DECLINED**: (1) the **two-near-identical-count-aggregate
SQL-helper extraction** — the xtask classifier word-scans the literal SQL strings, so templating the
outer table would defeat its legibility; and (2) the **4-fault-seam unification** — each token must
stay a distinct literal at its own cfg-gated site, the repetition IS the guard's enforcement surface.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-PC-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..18 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-PC-2 | Mutation = per-feature 100% on the genuinely-viable in-diff (`viewer-domain` 4/4 caught; `adapter-http-viewer` 4/4 viable caught after adding 3 in-crate unit tests), matching slice-02..18 DV-2. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. The 2 remaining cargo-mutants "missed" are cfg-dead-branch artifacts (see Mutation note), not viable survivors; ≥80%-of-viable gate MET. |
| DV-PC-3 | **`count_countered_peer_claims` is COUNT-ONLY, not a list re-materialization** (ADR-056). | A count-only aggregate keeps the new read symmetric with slice-18's `count_countered_own_claims` + slice-17's `count_active_peer_subscriptions` and cheap (no row materialization); the landing's read budget grows by exactly ONE fixed aggregate to 5 counts, invariant to store size (the 120-plain-peer-claim bulk seed proves it). |
| DV-PC-4 | **The 5th field on `LandingSummary` is ADDITIVE `Option<usize>` — a fabricated `0` stays UNREPRESENTABLE** (`Some(0)` → "(0 countered)", `None` → "(— countered)") (ADR-056). | Adding the countered-peer count as a 5th `Option<usize>` field (not a flag or a sentinel) inherits the slice-17/18 missing≠zero TYPE guarantee for free — there is no way to type a failed read as "0" — and the addition disturbs none of the existing four counts (incl. the slice-18 own-countered). |
| DV-PC-5 | **Per-count INDEPENDENT `.ok()` degrade — a failed countered-peer count never sinks the page, never disturbs the other four counts, and `GET /` (and `/peer-claims`) never 5xx, never blanks the slice-18 own-countered** (ADR-056). | A front door that 500s (or blanks the own-countered count) because the peer count failed is worse than an honest partial summary; degrading the new count independently to "(— countered)" keeps both peer surfaces resilient, the reader oriented, and the slice-18 own count intact (PC-DEGRADE). |
| DV-PC-6 | **Presence-once via `COUNT(DISTINCT p.cid)` + an IN-set over both reference tables — a peer claim countered by N counterers counts ONCE, either ref-table arm contributing once** (ADR-056). | Without `DISTINCT` + the IN-set, a peer claim countered by N counterers (operator's own counter OR other peers) would inflate the count N-fold; counting the DISTINCT countered peer-claim cids makes the count a true "how many of my cached peer claims are contested" presence measure, not a counter-event tally (PC-PRESENCE). |
| DV-PC-7 | **Peer-only by the OUTER `peer_claims` table — a peer claim is countered when its cid appears as a countered `referenced_cid` (by the operator's own counter OR another peer's)** (ADR-056). | The count must be "MY cached PEER claims that are contested", not own claims; anchoring the outer table on `peer_claims` and matching against the union of `referenced_cid`s gets peer-only by swapping ONLY the outer table from the slice-18 SQL (`claims`→`peer_claims`) — both counter arms (own + peer) contribute the same countered `referenced_cid`. |
| DV-PC-8 | **REUSED `render_countered(Option<usize>)` helper — NO new helper, ONE SSOT copy site delegating to slice-17's `render_count` — now also used by BOTH `render_landing`'s peer line AND `render_peer_claims_page`'s header; both routes resolve the count INDEPENDENTLY per render** (ADR-056, WD-PC-8/10). | Two surfaces showing the same count must not be able to diverge; routing both through the SAME slice-18 helper (NO new helper) means the missing≠zero distinction and the "(N countered)" framing have a single source — the surfaces cannot drift apart in framing or marker, and the slice-18 own surfaces stay on the identical helper. |
| DV-PC-9 | **The countered-peer count is RENDER-ONLY — it never re-weights or re-orders the peer claims, and the slice-18 own surfaces are UNTOUCHED** (ADR-056). | A count that silently re-ranked the peer claims would be a control surface, not an orientation aid; keeping it render-only (a text suffix beside the peer-claims line / in the header) preserves the read-only contract, the existing peer-claim ordering, AND the untouched slice-18 own surfaces (PC-NO-REWEIGHT / PC-NO-REORDER / PC-INV-OwnUntouched). |
| DV-PC-10 | **The SQL is xtask R-PC-9 GREEN BY CONSTRUCTION — it names `peer_claims` (a whole word, the one wrinkle vs slice-18) but NOT the standalone `claims`, so `no_cross_table_join_elides_author` returns `None` (`is_cross_store = mentions_peer_claims AND mentions_own_claims = TRUE AND FALSE = FALSE`)** (ADR-056). | The viewer anti-merging xtask rule fires on a query that pairs the standalone `claims` and `peer_claims` tables; the countered-peer count names `peer_claims` + the two *reference* tables but NOT standalone `claims`, so the classifier short-circuits FALSE — verified line-by-line against the real classifier at DESIGN, not assumed. |
| DV-PC-11 | **The 4th fault seam (PC-DEGRADE) is TEST-ONLY and DISTINCT: `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` honored ONLY by a `#[cfg(debug_assertions)]` function; the release sibling is the identity function (NO env read compiled in)** — release build verified seam-free, all 4 viewer fault tokens ABSENT from the rlib (ADR-056, mirroring slice-16/17/18). | PC-DEGRADE needs a deterministic mid-request count failure on BOTH peer surfaces that fails INDEPENDENTLY of slice-18's own count, but a fault hook compiled into release is a production liability; gating a DISTINCT env read behind `debug_assertions` keeps the release binary seam-free while the debug profile drives the peer degrade scenario without disturbing the own count. |
| DV-PC-12 | **The slice-16/17/18 `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` set was EXTENDED to a 4th token** — an ungated read of ANY of the four fails check-arch (ADR-056). | The generalized token-set guard from slice-17 paid off again: adding the 4th seam was ONE set entry, and an ungated read of the new token is caught structurally — the cfg-gate enforcement stays in ONE place and extends to every future seam by one entry. |
| DV-PC-13 | **The fault-seam degrade is independently pinned**: the debug seam's pass-through + Err-injection is pinned by the new `adapter-http-viewer` unit tests; the release identity sibling is pinned by the xtask seam guard + the release-build seam-free check (ADR-056). | The 2 cargo-mutants "missed" land on the release identity sibling (not compiled under the debug test profile, so neither reachable nor genuinely viable); the debug twin is killed by the in-crate tests, and the release sibling is structurally pinned — so the cfg-dead-branch artifact is covered without theatre. |
| DV-PC-14 | **Phase-3 refactor: none needed — `render_countered` REUSED (no new helper); TWO refactors DECLINED: the two-near-identical-count-aggregate SQL-helper extraction (xtask word-scans the literal SQL) AND the 4-fault-seam unification (each token a distinct cfg-gated literal).** | Templating the outer table behind one SQL helper would move the literal table names off the strings the xtask classifier word-scans, defeating R-PC-9's legibility; unifying the four fault-seam tokens would move the literals off their cfg-gated sites and defeat the per-token guard. Both non-refactors recorded as deliberate calls — the repetition IS the guard's enforcement surface. |
| DV-PC-15 | **Closed the package-scoped mutation harness gap with 3 in-crate `adapter-http-viewer` unit tests** (the countered-peer-seam pass-through + Err-injection + a `peer_claims_page` header-render test over a fake store), mirroring the slice-16/17/18 precedent. | The slice-19 adapter logic is acceptance-covered in the `cli` package and invisible to `cargo mutants -p adapter-http-viewer`; the 3 in-crate tests reach the 4 viable adapter mutants locally (including the second-surface peer header render) so the per-feature gate is met without a cross-package harness. |
| DV-PC-16 | **A real seed bug was caught at DISTILL: the distinct-peer counter inflated the peer total — fixed via the operator-counter arm + a unified single-pull; and the no-N+1 bulk seed needed Rachel's PDS rebuilt for the all-peers re-pull.** | Position-aware asserts + oracle-pinned seeds prevented fixture theater — the inflated peer total would have masked a presence-once miscount; pinning the seed to a unified single-pull and rebuilding Rachel's PDS for the bulk re-pull made PC-PRESENCE and the N+1-free gold honest. |

## Cardinal release gates + slice-19 invariants (I-PC-1..n)

The cardinal release gates realized on the landing peer line + `/peer-claims` surfaces — all
release-blocking:

1. **Read-only / no-reweight-no-reorder / own-untouched (CARDINAL, I-PC-1)** — the countered-peer
   count is render-only text; no write/sort/filter control; the count NEVER re-weights or re-orders
   the peer claims; **the slice-18 own surfaces are UNTOUCHED**; the web process holds no signing key;
   the read seam has NO mutation method (type-level). Three-layer: TYPE (no write method) + STRUCTURAL
   (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-reweight-no-reorder /
   PC-INV-OwnUntouched).
2. **Missing≠zero (CARDINAL, I-PC-2)** — the 5th additive `Option<usize>` `LandingSummary` field
   makes a fabricated `0` unrepresentable: `Some(0)` → "(0 countered)" (honest zero), `None` → "(—
   countered)" (the missing marker); the count degrades independently via `.ok()`, the surfaces never
   5xx and never blank the 4 sibling counts incl. the slice-18 own-countered (PC-DEGRADE + PC-ZERO).
3. **Presence-once (CARDINAL, I-PC-3)** — a peer claim countered by N counterers counts ONCE, via
   `COUNT(DISTINCT p.cid)` + the IN-set over both reference tables; either ref-table arm contributes
   once (PC-PRESENCE).
4. **No-N+1 (CARDINAL, I-PC-4)** — ONE FIXED aggregate read per render, invariant to store size; the
   landing's read budget grows to 5 counts, by EXACTLY 1, never one-per-row — proven by the
   120-plain-peer-claim bulk seed (the N+1-free gold).
5. **Single-source / both-surfaces (CARDINAL, I-PC-5)** — BOTH `render_landing`'s peer line and
   `render_peer_claims_page`'s `/peer-claims` header render the count via the SAME (REUSED)
   `render_countered` helper (one SSOT copy site delegating to `render_count`); both routes resolve
   the count INDEPENDENTLY per render — the surfaces cannot diverge (PC-HEADER, WD-PC-8/10).
6. **Peer-only (I-PC-6)** — `count_countered_peer_claims` is anchored on the outer `peer_claims`
   table (cached peer claims, countered by the operator's own counter OR another peer's); it counts MY
   contested cached peer claims, the EXACT slice-18 SQL with the outer table swapped `claims`→`peer_claims`.
7. **LOCAL / offline (I-PC-7)** — the countered-peer count reads the LOCAL counter-reference tables
   with no network (fully offline); nothing persisted (the offline gold).
8. **Anti-misread (I-PC-8)** — the neutral "(N countered)" framing reusing the slice-18/14 sensibility
   — no penalty / deduction / "disputed by N" / verdict (the anti-misread gold).
9. **Fault seam release-gated (I-PC-9)** — the PC-DEGRADE fault trigger
   (`OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`, DISTINCT from slice-18's own seam) is honored ONLY by
   a `#[cfg(debug_assertions)]` function; the release sibling is the identity function (no env read
   compiled in); release build verified seam-free (all 4 viewer fault tokens absent from the rlib);
   the `VIEWER_FAIL_SEAM_TOKENS` xtask guard (now 4 tokens) fails any ungated read.
10. **Own-untouched (CARDINAL, I-PC-10)** — the slice-18 own-claims surfaces (the landing own-claims
    line + the `/claims` header + the slice-18 own-countered count) are UNTOUCHED; own + peer now
    complete the counter-aware orientation across both (PC-INV-OwnUntouched).

| # | Invariant | Enforcement |
|---|---|---|
| I-PC-1 | Read-only / no-reweight-no-reorder / own-untouched (the countered-peer count is render-only text; no executable write/sort/filter control; the count never re-weights or re-orders the peer claims; the slice-18 own surfaces untouched; no key in the process; the read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-reweight-no-reorder / PC-INV-OwnUntouched, DV-PC-9). Cardinal. |
| I-PC-2 | Missing≠zero (the 5th additive `Option<usize>` `LandingSummary` field makes a fabricated `0` unrepresentable; `Some(0)` → "(0 countered)", `None` → "(— countered)"; per-count independent `.ok()` degrade; never 5xx; never blanks the 4 sibling counts). | TYPE (the 5th `Option<usize>` field, DV-PC-4) + STRUCTURAL (`render_countered` → `render_count` `Some`/`None` split + the `MISSING_COUNT_MARKER` SSOT, DV-PC-5/8) + BEHAVIORAL (PC-DEGRADE + PC-ZERO). Cardinal. |
| I-PC-3 | Presence-once (a peer claim countered by N counterers counts once; `COUNT(DISTINCT p.cid)` + the IN-set over both reference tables; either ref-table arm contributes once). | STRUCTURAL (the `COUNT(DISTINCT)` + IN-set aggregate, DV-PC-6) + BEHAVIORAL (PC-PRESENCE). Cardinal. |
| I-PC-4 | No-N+1 (ONE FIXED aggregate read per render, invariant to store size; the landing's read budget grows to 5 counts, by exactly 1, never one-per-row; proven by the 120-plain-peer-claim bulk seed). | STRUCTURAL (the single fixed count read, DV-PC-3) + BEHAVIORAL (N+1-free gold over the bulk seed). Cardinal. |
| I-PC-5 | Single-source / both-surfaces (BOTH `render_landing`'s peer line and the `/peer-claims` header render the count via the SAME REUSED `render_countered` helper; both routes resolve the count independently per render — the surfaces cannot diverge). | STRUCTURAL (the REUSED `render_countered` SSOT copy site delegating to `render_count`, DV-PC-8) + BEHAVIORAL (PC-HEADER). Cardinal. |
| I-PC-6 | Peer-only (`count_countered_peer_claims` anchored on the outer `peer_claims` table — cached peer claims, countered by the operator's own counter OR another peer's; the EXACT slice-18 SQL with the outer table swapped `claims`→`peer_claims`). | STRUCTURAL (the outer `peer_claims` table + the union-of-reference-tables IN-set, DV-PC-7) + BEHAVIORAL (PC-WS / PC-PRESENCE). |
| I-PC-7 | LOCAL / offline (the countered-peer count reads the LOCAL counter-reference tables with no network; nothing persisted). | STRUCTURAL (the read-only local aggregate count) + BEHAVIORAL (PC-OFFLINE gold). |
| I-PC-8 | Anti-misread (the neutral "(N countered)" framing reusing the slice-18/14 sensibility — no penalty / deduction / "disputed by N" / verdict). | STRUCTURAL (the neutral text in the REUSED `render_countered`, reusing the slice-18/14 framing) + BEHAVIORAL (PC-ANTI-MISREAD gold). |
| I-PC-9 | Fault seam release-gated (PC-DEGRADE trigger `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`, DISTINCT from slice-18's seam, honored ONLY by `#[cfg(debug_assertions)]`; release sibling = identity; release build seam-free, all 4 tokens absent; the `VIEWER_FAIL_SEAM_TOKENS` xtask guard — now 4 tokens — fails any ungated read). | TYPE/COMPILE (the `#[cfg(debug_assertions)]` gate; the release identity sibling, DV-PC-11) + STRUCTURAL (the `scan_viewer_fail_seam_guard` over the 4-token set, DV-PC-12; release-build seam-free check) + BEHAVIORAL (the in-crate seam pass-through + Err-injection tests, DV-PC-13). Cardinal. |
| I-PC-10 | Own-untouched (the slice-18 own-claims surfaces — the landing own-claims line, the `/claims` header, and the slice-18 own-countered count — are UNTOUCHED; own + peer now complete the counter-aware orientation across both). | STRUCTURAL (no edit to the slice-18 own render paths or SQL; the distinct 5th field + 4th fault seam) + BEHAVIORAL (PC-INV-OwnUntouched gold, DV-PC-9). Cardinal. |

All slice-19 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 + slice-17 I-LD-1..8 +
slice-18 I-CC-1..9 sets (read-only / no key / human gate / offline + loopback / progressive
enhancement / structural fragment/page parity / the missing≠zero `LandingSummary` shape / the
slice-18 own-countered count); the countered-peer count is shown verbatim through the REUSED
`render_countered` helper on both peer surfaces.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_peer_counter_aware_counts` corpus (PC-WS, PC-HEADER /
  PC-PRESENCE, PC-ZERO, PC-DEGRADE, PC-NO-REWEIGHT / PC-NO-REORDER / PC-INV-OwnUntouched, PC-READONLY /
  PC-OFFLINE / PC-N+1 / PC-ANTI-MISREAD — the thick walking skeleton at PC-WS) + the GOLD
  `viewer_peer_counter_aware_counts_invariants` (read-only / no-reweight-no-reorder / own-untouched,
  LOCAL / offline, N+1-free, anti-misread) GREEN + the `viewer-domain` unit/property tests (the
  `render_countered` projection over the peer count + the 5th `LandingSummary` field) + the new
  `adapter-http-viewer` in-crate unit tests (the countered-peer-seam pass-through + Err-injection + the
  `peer_claims_page` header-render test over a fake store); slices 06/11-14/16/17/18 corpora GREEN —
  zero regression (the slice-18 own surfaces UNTOUCHED). The `ViewerServer` harness drives the REAL
  `openlore ui` over HTTP; the PC-DEGRADE fault is driven by the distinct TEST-ONLY
  `#[cfg(debug_assertions)]` env seam.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, no new
  allowlist edge: the REUSED `render_countered` is a total fn of `Option<usize>` delegating to
  `render_count` (no `viewer-domain → claim-domain` reachout). The viewer capability rule is unchanged
  (read-only counts; no signing/identity/PDS, no store-write; the count never re-weights/re-orders;
  the own surfaces untouched). The SQL is R-PC-9 GREEN BY CONSTRUCTION (it names `peer_claims` — a
  whole word — + the two *reference* tables, but NOT the standalone `claims`, so
  `no_cross_table_join_elides_author` returns `None`). The `scan_viewer_fail_seam_guard` now iterates
  `VIEWER_FAIL_SEAM_TOKENS` (slice-16 active-set + slice-17 peer-claims-count + slice-18
  countered-own-count + slice-19 countered-peer-count); an ungated read of ANY of the four fails
  check-arch.
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor: none needed**
  (`render_countered` REUSED — no new helper; the two-near-identical-count-aggregate SQL-helper
  extraction was DECLINED — the xtask classifier word-scans the literal SQL, so templating the outer
  table would defeat R-PC-9's legibility; the 4-fault-seam token unification was DECLINED — a
  security/guard invariant overrides Rule-of-Three, DV-PC-14); `viewer-domain` purity intact (no I/O
  imports; maud + ports only; the store threading + `.ok()` degrade live in the effect shell; NO
  `claim-domain` reachout).
- **Release-build seam check**: the `#[cfg(not(debug_assertions))]` release build was verified
  **seam-free** — all **4** viewer fault tokens (slice-16 active-set, slice-17 peer-claims-count,
  slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`, slice-19 `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`)
  are NOT compiled into the release rlib (each release sibling is the identity function, no env read).
- **Adversarial review**: **APPROVED**, **0 defects, zero Testing Theater**. Presence-once verified
  (a peer claim countered by N counterers counts once — the `COUNT(DISTINCT)` + IN-set confirmed, either
  ref-table arm contributing once); single-source verified (both peer surfaces render via the SAME REUSED
  `render_countered` helper — they cannot diverge); additive verified (the 5th `Option<usize>` field
  disturbs none of the existing four counts); own-untouched verified (the slice-18 own surfaces unchanged);
  the fault-seam confirmed release-gated in 3 layers (the `#[cfg(debug_assertions)]` gate + the xtask
  4-token-set guard + the release-build seam-free check). The real seed bug (distinct-peer counter
  inflation) was caught and fixed — no fixture theater.
- **DES integrity**: PASS — all 7 steps have complete DES traces (7/7).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the `render_countered` projection over
the peer count + the 5th `LandingSummary` field render) AND the `adapter-http-viewer` slice-19 logic
(the countered-peer count resolution into both surfaces + the fault-seam pass-through + the
`/peer-claims` header render). The slice-04/05 cross-package lesson stays applied — the killing
properties are kept IN-CRATE.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`render_countered` projection over the peer count + the 5th `LandingSummary` field render, in-diff) | 4 | 4 | 0 | **100%** (4/4 in-diff viable) |
| `adapter-http-viewer` slice-19 logic (countered-peer-count resolution into both surfaces + fault-seam pass-through + `/peer-claims` header render, in-diff) | 4 | 4 | 0 | **100%** (4/4 viable, after the 3 in-crate unit tests) |

Slice-19 per-feature gate SATISFIED (≥80%; actual **100% on the genuinely-viable in-diff**, 0 viable
missed). **The 2 remaining cargo-mutants "missed" are both the `#[cfg(not(debug_assertions))]` release
identity sibling of the peer fault seam** — a **cfg-dead-branch artifact** NOT compiled under the
debug test profile (neither reachable nor genuinely viable), the **same class as slices 16/17/18's
lone survivors**. They are **independently pinned** by the `VIEWER_FAIL_SEAM_TOKENS` xtask guard (now
4 tokens) + the release-build seam-free check (DV-PC-13) — covered without theatre.

**Closing the package-scoped harness gap**: the slice-19 `adapter-http-viewer` logic is
acceptance-covered in the `cli` package and therefore invisible to `cargo mutants -p
adapter-http-viewer` (the package-scoped harness cannot see cross-package acceptance tests). Three
**in-crate `adapter-http-viewer` unit tests** were added (the countered-peer-seam pass-through, the
Err-injection, and a `peer_claims_page` header-render test over a fake store) to close that gap —
**mirroring the slice-16/17/18 adapter-unit-test precedent** — so the 4 viable adapter mutants
(including the second-surface peer header render) are caught locally. `adapter-duckdb` is NOT mutated
by design (effect shell; covered by the GOLD invariants through the real binary). DEVOPS sweep is the
ongoing backstop.

## Lessons learned / issues

- **A sibling slice is a SHAPE transplant, not new architecture — get the swap surgical**: slice-19
  is slice-18 applied to peer claims, and the entire delta was the OUTER table swap
  (`claims`→`peer_claims`), a 5th additive field, a distinct 4th fault seam, and the SAME REUSED
  render helper. **Lesson: when a slice is "do the sibling of an already-shipped slice", the work is
  almost entirely a SURGICAL swap — change exactly what differs (the outer SQL table, a new additive
  field, a new distinct guard token) and REUSE everything that doesn't (the render helper, the SQL
  pattern, the fault-seam shape); the smaller the diff, the cleaner the own-untouched guarantee.**
- **Own-untouched is a CARDINAL that must be a behavioral invariant, not an assumption (DV-PC-9,
  I-PC-10)**: slice-18 + slice-19 together complete the orientation across own + peer, but only if
  slice-19 leaves the slice-18 own surfaces UNTOUCHED — a 5th field + a distinct fault seam, no edit
  to the own render path or SQL. **Lesson: when a sibling slice completes a pair, pin "the prior
  sibling's surfaces are untouched" as an explicit gold invariant (PC-INV-OwnUntouched) — the
  completion claim is only true if the addition is provably additive, and a behavioral assert is how
  you prove it.**
- **The xtask classifier word-scans the literal SQL — so the SQL-helper extraction is the WRONG
  refactor (DV-PC-14, DV-PC-10)**: with two near-identical count-aggregate SQL strings differing only
  in the outer table, the DRY instinct is to template the outer table behind one helper — but the
  R-PC-9 classifier word-scans the literal table names in the strings, so templating would move the
  names off the scanned surface and defeat the rule's legibility. The extraction was correctly
  DECLINED. **Lesson: when a structural rule classifies a query by word-scanning its literal SQL,
  templating the SQL defeats the rule — keep the literals legible at their call sites and record the
  non-refactor as a deliberate call.**
- **A distinct fault seam per count, even when the shape is identical (DV-PC-11, DV-PC-12)**: the peer
  count needed its OWN `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` token (distinct from slice-18's own
  seam) so the peer count fails INDEPENDENTLY — and adding it to the generalized
  `VIEWER_FAIL_SEAM_TOKENS` set was ONE entry. **Lesson: independent degrade requires an independent
  seam — when two counts must fail independently, give each its own cfg-gated token even if the seam
  shape is identical; the token-set guard makes the Nth seam one entry, and the per-token literal is
  the enforcement surface (do NOT unify them).**
- **Position-aware asserts + oracle-pinned seeds catch the seed bug the count would hide (DV-PC-16)**:
  the distinct-peer counter inflated the peer total at DISTILL, and a presence-once miscount would
  have sailed through a less precise fixture — the inflation was caught by oracle-pinned seeds + the
  operator-counter-arm fix + a unified single-pull, and the no-N+1 bulk seed needed Rachel's PDS
  rebuilt for the all-peers re-pull. **Lesson: a count test is only as honest as its seed — pin the
  seed to an oracle and assert by position, because an inflated seed silently turns a presence-once
  bug into a passing test.**
- **xtask R-PC-9 GREEN BY CONSTRUCTION beats green by luck — verify the ONE wrinkle (DV-PC-10)**: the
  peer SQL names `peer_claims` (a whole word — the one wrinkle vs slice-18 whose outer was `claims`)
  but NOT the standalone `claims`, so the classifier short-circuits FALSE
  (`mentions_peer_claims AND mentions_own_claims = TRUE AND FALSE`). This was verified line-by-line
  against the real classifier at DESIGN. **Lesson: when a sibling SQL introduces exactly one new
  table name the classifier scans, trace that wrinkle through the real rule by hand at DESIGN —
  "the sibling passed, so this will too" is not a gate when the sibling's outer table differs.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-056 fixed the contracts; field-level shaping (the 5th `LandingSummary` field, the REUSED `render_countered` helper, the dual-surface threading) left to DELIVER. | All adopted; the 5th `Option<usize>` field, the REUSED `render_countered` (`Some`→"(n countered)" / `None`→"(— countered)"), and BOTH the landing peer line and the `/peer-claims` header materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-056 fixed the new count-only read `count_countered_peer_claims` (the slice-18 COUNT(DISTINCT) + IN-set SQL with the outer table swapped `claims`→`peer_claims`). | Shipped exactly — count-only, presence-once, peer-only, read-only on the existing port, injection-safe; a peer claim countered by N counterers counts once, either ref-table arm contributing once (PC-PRESENCE green). | Resolved at DELIVER. |
| 3 | ADR-056 fixed the 4th fault seam as TEST-ONLY (`#[cfg(debug_assertions)]`) and DISTINCT, the release sibling = identity, and the xtask guard extended to a 4th token. | The `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` seam landed `#[cfg(debug_assertions)]`-only and distinct; `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` extended to 4 tokens; release build verified seam-free (all 4 tokens absent from the rlib). | Resolved at DELIVER. |
| 4 | ADR-056 fixed "no new pure-core edge; check-arch unchanged (member count, allowlist); SQL R-PC-9 GREEN by construction." | The REUSED `render_countered` is a total fn delegating to `render_count`; `check-arch` reports 21 members, no new allowlist edge, no new route; the SQL is R-PC-9 green by construction (names `peer_claims`, not standalone `claims`). | Resolved at DELIVER. |
| 5 | The `/peer-claims` header threading expected to be real work (a second surface resolving the count independently — a real `peer_claims_page` `Option` param — and rendering via the shared helper). | The `/peer-claims` header (01-02) was real work — it added a `peer_claims_page` `Option` param, resolved the count independently, and rendered via the SAME REUSED `render_countered` helper (PC-HEADER / PC-PRESENCE green; single-source held). | Resolved at DELIVER. |
| 6 | Phase-3 refactor anticipated. | NONE needed — `render_countered` REUSED (no new helper); the SQL-helper extraction was DECLINED (xtask word-scans the literal SQL) and the 4-fault-seam token unification was DECLINED (each literal individually cfg-gated, DV-PC-14). | No refactor at DELIVER (a deliberate non-refactor). |
| 7 | `adapter-http-viewer` mutation coverage expected via the cli-package acceptance suite. | The package-scoped harness gap was closed with 3 in-crate `adapter-http-viewer` unit tests (mirroring slice-16/17/18); 4/4 viable adapter mutants caught (including the second-surface peer header render). | Closed at DELIVER (DV-PC-15). |
| 8 | Peer-claims-countered scope (the deferred WD-CC-7 sibling). | SHIPPED — slice-19 discharges WD-CC-7; peer-claims-countered now realized; own (slice-18) + peer (slice-19) complete the counter-aware orientation across both. No third dimension. | The deferred WD-CC-7 sibling, shipped at DELIVER. |
| 9 | Review expected to pass clean. | Review APPROVED, 0 defects, zero Testing Theater; presence-once / single-source / additive / own-untouched / fault-seam-gating all verified; the real seed bug caught and fixed. | Confirmed at DELIVER. |
| 10 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-PC-2, 100% viable in-diff, 0 viable missed; the 2 "missed" are cfg-dead-branch artifacts). | Recorded. |
| 11 | The no-N+1 bulk seed expected to prove a fixed read budget. | The 120-plain-peer-claim bulk seed proved ONE fixed aggregate read per render (the budget grows to 5 counts, by exactly 1); the all-peers re-pull needed Rachel's PDS rebuilt (DV-PC-16). | Resolved at DELIVER (DISTILL seed fix). |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-peer-counter-aware-counts/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, requirements, user-stories,
  acceptance-criteria, outcome-kpis, dor-checklist), `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-18 archive** (the OWN-claims sibling this slice mirrors exactly — the home of the
  `render_countered` helper, the `count_countered_own_claims` SQL pattern, the additive-`Option`-field
  shape, and the `#[cfg(debug_assertions)]` fault-seam + xtask token-set pattern this slice reuses):
  `docs/evolution/viewer-counter-aware-counts-evolution.md`
- **Parent slice-17 archive** (the read-only `GET /` landing this slice extends, home of the
  `LandingSummary` ADT + `render_count` + the `.ok()`-degrade count-resolution pattern):
  `docs/evolution/viewer-landing-dashboard-evolution.md`
- **Parent counter-flag family archives** (the counter-reference data this slice aggregates, the
  slice-13-flagged `/peer-claims` rows, and the slice-14 neutral-framing sensibility):
  `docs/evolution/viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`
- **Parent slice-16 archive** (the `#[cfg(debug_assertions)]` test-only fault-seam + the
  `scan_viewer_fail_seam_guard` xtask guard pattern this slice reuses + extends to a 4th token):
  `docs/evolution/viewer-search-follow-state-evolution.md`
- **Slice-19 ADR**:
  `docs/adrs/ADR-056-peer-counter-aware-counts-count-only-countered-peer-claims-read-5th-additive-landingsummary-field-reused-render-countered-helper-across-both-peer-surfaces.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-peer-counter-aware-counts/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-peer-counter-aware-counts/deliver/execution-log.json`,
  `docs/feature/viewer-peer-counter-aware-counts/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_peer_counter_aware_counts.rs` (PC-WS, PC-HEADER / PC-PRESENCE, PC-ZERO,
  PC-DEGRADE, PC-NO-REWEIGHT / PC-NO-REORDER / PC-INV-OwnUntouched, PC-READONLY / PC-OFFLINE / PC-N+1 /
  PC-ANTI-MISREAD — the thick walking skeleton at PC-WS),
  `tests/acceptance/viewer_peer_counter_aware_counts_invariants.rs` (the 10 gold invariants —
  read-only / no-reweight-no-reorder / own-untouched, LOCAL / offline, N+1-free, anti-misread)
- **Reused fault-seam + xtask-guard pattern**: `xtask` (`scan_viewer_fail_seam_guard`,
  `VIEWER_FAIL_SEAM_TOKENS` — slice-16 active-set + slice-17 peer-claims-count + slice-18
  countered-own-count + slice-19 countered-peer-count); the ADR-026 pubkey-seam release-gate pattern
  (`classify_cfg_gated_token`); the R-PC-9 anti-merging classifier (`no_cross_table_join_elides_author`)
- **Extended viewer crates**: `crates/viewer-domain` (the REUSED `render_countered` helper delegating
  to `render_count` + the 5th `LandingSummary` field), `crates/adapter-http-viewer` (the countered-peer
  count resolved into BOTH the landing peer-claims line and the `/peer-claims` header via `.ok()`
  degrade + the distinct `#[cfg(debug_assertions)]` fault seam + the release identity sibling + the 3
  in-crate unit tests), `crates/adapter-duckdb` (the read-only `count_countered_peer_claims` impl —
  `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM
  claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references
  WHERE ref_type='counters')`), `crates/ports` (the read seam + the 5th `LandingSummary` field)
- **Reused count reads + data (NOT re-implemented)**: the slice-17 `LandingSummary` / `render_count`;
  the slice-18 `render_countered` helper + `count_countered_own_claims` SQL pattern + fault-seam shape;
  the slice-12/13 counter-reference tables (`claim_references` + `peer_claim_references`,
  `ref_type='counters'`)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml` — J-003b (the
  orientation / at-a-glance facet — now COMPLETE across own (slice-18) + peer (slice-19))
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-claim-threads-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`,
  `viewer-search-follow-state-evolution.md`, `viewer-landing-dashboard-evolution.md`,
  `viewer-counter-aware-counts-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS adc1cf6 → DESIGN 00ad026 → DISTILL 0cf8521 → roadmap (post-0cf8521) → 01-01 4a2a5ae →
01-02 6272d74 → 01-03 80f901d → 02-01 f876873 → 02-02 e04e45b → 02-03 4f0cd32 → 03-01 8214cef →
mutation-gate unit tests 7be2180.
