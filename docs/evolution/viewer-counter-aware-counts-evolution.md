# Evolution: viewer-counter-aware-counts (slice-18 read-only countered-own-claims count on the `GET /` landing + the `/claims` list header — at-a-glance "how much of my store has been disputed" on the viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-counter-aware-counts/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-055 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-17 (`viewer-landing-dashboard` — the read-only `GET /` landing this slice
> extends, and the home of the `LandingSummary` ADT + `render_count` + the `.ok()`-degrade
> count-resolution pattern this slice reuses), the **counter-flag family** slices 11-14
> (`viewer-counter-claim-list-flags`, `viewer-counter-claim-threads`,
> `viewer-counter-flags-graph-surfaces`, `viewer-counter-flags-score-surface` — the source
> of the counter-reference data this slice aggregates, the slice-12-flagged `/claims` rows
> a reader drills into, and the slice-14 neutral-framing sensibility this count reuses),
> and slice-16 (`viewer-search-follow-state` — the origin of the **`#[cfg(debug_assertions)]`
> test-only fault-seam + the `xtask check-arch` seam guard** pattern, generalized through
> slice-17). Read those parent archives
> (`docs/evolution/viewer-landing-dashboard-evolution.md`,
> `viewer-counter-claim-list-flags-evolution.md`,
> `viewer-counter-claim-threads-evolution.md`,
> `viewer-counter-flags-graph-surfaces-evolution.md`,
> `viewer-counter-flags-score-surface-evolution.md`,
> `viewer-search-follow-state-evolution.md`) for the surfaces this slice composes.
> slice-18 realizes the **orientation / at-a-glance facet of J-003b** — turning the
> front-door own-claims count into a *countered*-aware count, so a reader sees not just
> how much is in their store but how much of it has been disputed, and can drill into the
> slice-12-flagged rows.

## Summary

`viewer-counter-aware-counts` enriches the `openlore ui` read-only viewer's **`GET /` landing
own-claims line** and the **`/claims` list header** with a **countered-own-claims count** —
rendering "12 own claims (3 countered)". It ties the shipped **counter-flag family (slices
11-14)** into **front-door orientation (slice-17)**: before this slice the landing told the
reader *how much* was in their store; now it also tells them *how much of it has been disputed* —
and the slice-12-flagged `/claims` rows are the drill-in target. The slice realizes the
**orientation / at-a-glance facet of J-003b**. It is scoped to **own-claims-countered only**;
**peer-claims-countered is deferred per WD-CC-7**. It REUSES the slice-17 `LandingSummary` +
`render_count` and the slice-12 counter-reference data, and adds **ONE new read-only count**,
`count_countered_own_claims`.

The load-bearing thesis: **a reader's front door surfaces not just store volume but store
contestation — read-only, LOCAL, and at exactly one aggregate read of cost.** The countered count
is render-only text beside the unchanged own-claims line; it never re-weights or re-orders
anything. The CARDINAL concerns are six: (1) **read-only / no-key** — a render-only count text;
no write/sort/filter control, and the count NEVER re-weights or re-orders the claims; (2) **LOCAL
/ offline** — a LOCAL aggregate over the counter-reference tables, no network; (3) **no-N+1** —
ONE aggregate read per render, invariant to store size (the landing's read budget grows by
**exactly 1**); (4) **missing≠zero** — a 4th additive `Option<usize>` field on `LandingSummary`,
`Some(0)` → "(0 countered)" honest zero, `None` → "(— countered)" missing marker, a fabricated
`0` unrepresentable; independent per-count `.ok()` degrade, never 5xx; (5) **presence-count** — a
claim countered by N peers counts ONCE, via `COUNT(DISTINCT)` + an IN-set; (6) **anti-misread** —
neutral "(N countered)", reusing the slice-14 sensibility — no penalty / deduction / "disputed by
N" / verdict.

The slice ships **ZERO new crates** (workspace stays at **21 members**) and **ZERO new routes**
(it thickens the existing `GET /` landing and the existing `/claims` header). It is an **additive
enrichment of two existing surfaces, not a re-architecture**: it extends `viewer-domain` (the
shared `render_countered(Option<usize>)` helper delegating to slice-17's `render_count`, used by
BOTH `render_landing` and `render_claims_page`), `adapter-http-viewer` (resolving the new count
into both surfaces via `.ok()` degrade + the 3rd fault seam), the `adapter-duckdb` read impl (ONE
new count query), the `ports` (one read seam + the 4th `LandingSummary` field), and the `cli`
(`ui` wiring, still no key). The one new read method is **read-only on the existing
`StoreReadPort`**. The defining design choice is the **4th additive `Option<usize>` field on
`LandingSummary`** — which keeps a fabricated `0` unrepresentable — together with the **shared
`render_countered` helper** that is the single SSOT copy site for both routes.

### What shipped (one paragraph)

Two enriched surfaces — the **`GET /` landing own-claims line** and the **`/claims` list header**
— each now render a **countered-own-claims count** ("12 own claims (3 countered)"). On request
each surface resolves the NEW LOCAL count `count_countered_own_claims` (independently, per render)
via **`.ok()` degrade** and projects it through the shared **`render_countered(Option<usize>)`**
helper — `Some(n)` → "(n countered)" (a genuine zero renders "(0 countered)"), `None` → "(—
countered)" (the missing marker) — so a fabricated `0` is unrepresentable. The count is added as
the **4th additive `Option<usize>` field on `LandingSummary`**, beside the unchanged
slice-17 own-claims/peer-claims/active-peers fields; its degrade is **independent** (a failed
countered count never sinks the page, and never disturbs the other three counts). The new
`count_countered_own_claims = SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT
referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
peer_claim_references WHERE ref_type='counters')` is **count-only** (chosen over
`counter_presence_for(own_cids).len()` for symmetry with slice-17's `count_active_peer_subscriptions`
+ cheapness), **presence-once** by `COUNT(DISTINCT)` + the IN-set (a claim countered by N peers
counts once), and **own-only** by the outer `claims` table (own claims are countered by PEERS —
self-counter is blocked — so the own-claim cid lands as a countered `referenced_cid`). The SQL is
**parameter-free / injection-safe** and **xtask-green by construction** (it names
`claim_references` + `peer_claim_references`, not the standalone `claims`-vs-`peer_claims` pair the
anti-merging rule fires on). The shared `render_countered` helper (one SSOT copy site delegating to
slice-17's `render_count`) is used by BOTH `render_landing` (beside the unchanged own-claims line)
AND `render_claims_page`'s `/claims` header — both routes resolve the count INDEPENDENTLY per
render and render via the SAME helper, so the two surfaces **cannot diverge** (WD-CC-8). The store
read is **LOCAL and read-only** (offline, no network); the count is render-only and **never
re-weights or re-orders** the claims; nothing is persisted; the viewer holds no key; the framing is
the **neutral "(N countered)"** reusing the slice-14 sensibility (no penalty / "disputed by N" /
verdict).

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-09 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-09 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-09 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-09 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **7/7 roadmap steps** done across **3 phases** (all COMMIT/PASS in
  `deliver/execution-log.json`).
- **Acceptance scenarios GREEN**: the `viewer_counter_aware_counts` corpus (CC- ids —
  including the **thick walking skeleton** at 01-01 driving the landing countered-count +
  the `/claims` header) + the GOLD invariants (`viewer_counter_aware_counts_invariants` —
  read-only / no-reweight-no-reorder, LOCAL / offline, N+1-free, anti-misread). Plus the new
  `adapter-http-viewer` in-crate unit tests (the countered-seam pass-through, the Err-injection,
  and a `claims_page` header-render test over a fake store) and the `viewer-domain` unit/property
  tests (the `render_countered` projection + the 4th `LandingSummary` field). The
  `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through the
  REAL verbs.
- **Slices 06/11-14/16/17 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices).
- **NO new crate, NO new route**: extends `viewer-domain` (PURE — the `render_countered` helper +
  the 4th field) + `adapter-http-viewer` (EFFECT — the count resolution into both surfaces + the
  3rd fault seam) + `adapter-duckdb` (EFFECT, read impl) + `ports` (the read seam + the 4th
  `LandingSummary` field) + `cli` (DRIVER) in place. Workspace member count stays **21**;
  `cargo xtask check-arch` reports "21 workspace members".
- **NO new production dependency**: `maud`/`hyper` unchanged; no `deny.toml` change.
- **100% mutation kill rate on the genuinely-viable in-diff** (`viewer-domain` 6/6 caught;
  `adapter-http-viewer` 4/4 viable caught after adding 3 in-crate unit tests) — exceeds the ≥80%
  per-feature gate. The 2 remaining cargo-mutants "missed" are the `#[cfg(not(debug_assertions))]`
  release identity sibling of the fault seam (a cfg-dead-branch artifact, same class as
  slice-16/17's lone survivor), independently guarded.
- **1 ADR** (ADR-055) Accepted/shipped.
- DES integrity: **7/7** steps have complete DES traces.
- Adversarial review: **APPROVED**, 0 defects, zero Testing Theater (presence-once / single-source /
  additive / fault-seam-gating all verified).
- Gates: DoR 9/9, DESIGN APPROVED (xtask-green verified BY CONSTRUCTION against real code), DISTILL
  APPROVED 9.375/10.
- **Release build verified seam-free**: all **3** viewer fault tokens are ABSENT from the release
  rlib.
- `cargo xtask check-arch`: OK (21 workspace members; the `scan_viewer_fail_seam_guard`
  `VIEWER_FAIL_SEAM_TOKENS` set was extended to a **3rd token** — an ungated read of ANY of the
  three fails check-arch).

## Wave-by-wave changelog

### DISCUSS (2026-06-09)

Luna framed the slice as a **brownfield DELTA on slice-17 (the landing) + the counter-flag family
(slices 11-14)** that realizes **the orientation / at-a-glance facet of J-003b**. Persona is
**P-001 (the node operator)** opening the viewer to answer not just "what's in my store?" but "how
much of my store has been disputed?". The load-bearing DISCUSS decision: **enrich the existing
own-claims count with a countered-aware count on BOTH the landing own-claims line AND the `/claims`
header — render-only, never re-weighting or re-ordering anything**. The CARDINAL framing insight:
**the counter-flag family (slices 11-14) already carried the per-row contestation signal, and
slice-17 carried the front-door volume summary — slice-18 ties them together so a reader is
oriented to contestation at a glance and can drill into the slice-12-flagged rows**. Two scoping
calls: **own-claims-countered only — peer-claims-countered DEFERRED (WD-CC-7)**; and **a count, not
a list — `count_countered_own_claims`, not a re-materialization (WD-CC-5)**. The walking skeleton is
the thick thread (the new count read + the 4th `LandingSummary` field + the `render_countered`
helper + BOTH the landing own-claims line and the `/claims` header threaded), validating the
riskiest assumption first — that ONE aggregate read can drive BOTH surfaces presence-once and
own-only while staying read-only and never re-ordering.

### DESIGN (2026-06-09)

Morgan locked slice-18 as an **additive enrichment of two existing surfaces, not a
re-architecture** — ZERO new crates, ZERO new routes, ZERO new binary, ZERO new architectural
style, ZERO new persisted type. The open decisions were resolved adopting the DISCUSS leans,
captured in one ADR:

- **ADR-055** (counter-aware counts — count-only countered-own-claims read, 4th additive
  `LandingSummary` field, shared `render_countered` helper across both surfaces, presence-once
  aggregate): the NEW read
  **`count_countered_own_claims = SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT
  referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
  peer_claim_references WHERE ref_type='counters')`** is **count-only** (chosen over
  `counter_presence_for(own_cids).len()` for symmetry with slice-17's
  `count_active_peer_subscriptions` + cheapness — WD-CC-5), **presence-once** by `COUNT(DISTINCT)` +
  the IN-set (a claim countered by N peers counts once), **own-only** by the outer `claims` table
  (own claims are countered by PEERS — self-counter is blocked — so the own-claim cid lands as a
  countered `referenced_cid`), **parameter-free / injection-safe**, and **xtask-green by
  construction** (it names `claim_references` + `peer_claim_references`, NOT the standalone
  `claims`-vs-`peer_claims` pair the anti-merging rule fires on). The count is the **4th additive
  `Option<usize>` field on `LandingSummary`**, keeping a fabricated `0` unrepresentable
  (`Some(0)` → "(0 countered)" honest zero, `None` → "(— countered)" missing marker). The view
  delta is the **shared `render_countered(Option<usize>)` helper** — ONE SSOT copy site delegating
  to slice-17's `render_count` — used by BOTH `render_landing` (beside the unchanged own-claims
  line) AND `render_claims_page`'s `/claims` header; **both routes resolve the count INDEPENDENTLY
  per render and render via the SAME helper, so they cannot diverge (WD-CC-8)**. The framing is the
  **neutral "(N countered)"** reusing the slice-14 sensibility (no penalty / "disputed by N" /
  verdict — anti-misread). The **3rd fault seam — CC-DEGRADE** (a failed countered-count read → "(—
  countered)" on BOTH surfaces, no 5xx) is exercised via a **TEST-ONLY**
  `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` env seam honored ONLY by a `#[cfg(debug_assertions)]`
  function (the `#[cfg(not(debug_assertions))]` release sibling is the identity function, no env
  read compiled in), mirroring slice-16/17; the slice-16/17 `scan_viewer_fail_seam_guard`
  `VIEWER_FAIL_SEAM_TOKENS` set is **extended to a 3rd token**.

The read-only / no-reweight-no-reorder contract is enforced at THREE layers (a `StoreReadPort` with
no mutation method, the `xtask check-arch` viewer capability rule, and a behavioral GOLD invariant
— the count is render-only and never re-weights/re-orders). The C4 views, the dual-surface
data-flow, and the I-CC-1..n structural-guarantee table are in the DESIGN sections of
`feature-delta.md` and `design/`. DISTILL closed at **APPROVED 9.375/10**.

### DISTILL (2026-06-09)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_counter_aware_counts.rs`** (Tier A — `CC-` ids): the **thick walking skeleton**
  (**CC-WS** — the landing own-claims line rendering the countered count via the 4th
  `LandingSummary` field + `render_countered`, driven by the new `count_countered_own_claims`
  read), the **`/claims` header + presence-once** (**CC-HEADER / CC-PRESENCE** — the `/claims`
  header renders the SAME countered count via the SAME helper, and a claim countered by N peers
  counts ONCE), the **honest-zero** (**CC-ZERO** — a store with no countered own claims renders "(0
  countered)", not the missing marker), the **missing≠zero degrade** (**CC-DEGRADE** — a failed
  countered-count read renders "(— countered)" on BOTH surfaces, NEVER a fabricated "0", NO 5xx —
  driven by the TEST-ONLY `#[cfg(debug_assertions)]` fault seam), the
  **no-reweight / no-reorder** (**CC-NO-REWEIGHT / CC-NO-REORDER** — the count never re-weights or
  re-orders the claims list), and the **read-only / offline / N+1-free / anti-misread**
  (**CC-READONLY / CC-OFFLINE / CC-N+1 / CC-ANTI-MISREAD** — render-only count, LOCAL aggregate, ONE
  read per render, neutral framing).
- **`viewer_counter_aware_counts_invariants.rs`** (gold guardrails — 9 GOLD invariants): **read-only
  / no-reweight-no-reorder** (the count is render-only; the claims list order + weighting are
  unchanged across rich/empty/degraded renders), **LOCAL / offline** (the countered count reads the
  LOCAL counter-reference tables with no network), **N+1-free** (ONE FIXED aggregate read per render,
  invariant to store size — the landing's read budget grows by exactly 1), and **anti-misread** (the
  neutral "(N countered)" framing — no penalty / deduction / "disputed by N" / verdict).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL verbs (claims composed, then countered by peers); the CC-DEGRADE fault is
driven by the TEST-ONLY `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` env seam (honored only under
`debug_assertions`). RED classification: both targets COMPILE green, scenarios FAIL via `todo!()` /
unimplemented seam = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-09)

Executed **7 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — thick walking skeleton + `/claims` header + presence-once + honest-zero (01-xx)**:
  **01-01 is the THICK walking skeleton** (**CC-WS**) — the new `count_countered_own_claims` read +
  the 4th `LandingSummary` field + the `render_countered` helper + the landing own-claims line
  threaded. **The thick WS drove the landing side into existence.** **01-02 (CC-HEADER /
  CC-PRESENCE)** threaded the `/claims` header to render the SAME countered count via the SAME
  helper, and pinned presence-once (a claim countered by N peers counts once) — **the `/claims`
  header was real work, not confirmatory**; **01-03 (CC-ZERO)** the honest empty "(0 countered)".
- **Phase 02 — the genuinely-new fault seam + no-reweight/reorder + offline/N+1/anti-misread
  (02-xx)**: **02-01** the **CC-DEGRADE** fault seam — **the real implementation work of the slice**
  (the `#[cfg(debug_assertions)]` `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` env seam + the release
  identity sibling + the `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` set extended to a
  3rd token; a failed countered-count read → "(— countered)" on BOTH surfaces, no 5xx); **02-02**
  the **CC-NO-REWEIGHT / CC-NO-REORDER** (the count never re-weights or re-orders the claims);
  **02-03** the **CC-READONLY / CC-OFFLINE / CC-N+1 / CC-ANTI-MISREAD** (render-only count, LOCAL
  aggregate, ONE read per render, neutral framing).
- **Phase 03 — gold (03-xx)**: **03-01** the **9 GOLD invariants** (read-only /
  no-reweight-no-reorder, LOCAL / offline, N+1-free, anti-misread). They flipped GREEN off the
  confirmatory render path.

The 7-step shape: a **thorough WS at 01-01** drove the landing side into existence (the 4th
additive field + the `render_countered` helper + the count read), but **01-02 (the `/claims` header
threading) was real work** (a second surface had to resolve the count independently and render via
the shared helper), and **CC-DEGRADE (the 3rd fault seam + the guard token) was the real new
implementation work**. The rest were **unskipped per step** (the scaffolds were `#[ignore]`d) and
confirmatory. **Phase-3 refactor: none needed** — `render_countered` already SSOT-delegated to
`render_count`, and the **3-fault-seam unification was correctly DECLINED**: a security/guard
invariant overrides the Rule-of-Three — unifying would move the token literals off their cfg-gated
sites, defeating the `VIEWER_FAIL_SEAM_TOKENS` xtask guard.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-CC-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..17 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-CC-2 | Mutation = per-feature 100% on the genuinely-viable in-diff (`viewer-domain` 6/6 caught; `adapter-http-viewer` 4/4 viable caught after adding 3 in-crate unit tests), matching slice-02..17 DV-2. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally. The 2 remaining cargo-mutants "missed" are cfg-dead-branch artifacts (see Mutation note), not viable survivors; ≥80%-of-viable gate MET. |
| DV-CC-3 | **`count_countered_own_claims` is COUNT-ONLY, not a list re-materialization** (chosen over `counter_presence_for(own_cids).len()`) (ADR-055, WD-CC-5). | A count-only aggregate keeps the new read symmetric with slice-17's `count_active_peer_subscriptions` and cheap (no row materialization); the landing's read budget grows by exactly ONE fixed aggregate, invariant to store size. |
| DV-CC-4 | **The 4th field on `LandingSummary` is ADDITIVE `Option<usize>` — a fabricated `0` stays UNREPRESENTABLE** (`Some(0)` → "(0 countered)", `None` → "(— countered)") (ADR-055). | Adding the countered count as a 4th `Option<usize>` field (not a flag or a sentinel) inherits the slice-17 missing≠zero TYPE guarantee for free — there is no way to type a failed read as "0" — and the addition disturbs none of the existing three counts. |
| DV-CC-5 | **Per-count INDEPENDENT `.ok()` degrade — a failed countered count never sinks the page, never disturbs the other three, and `GET /` (and `/claims`) never 5xx** (ADR-055). | A front door that 500s because the countered count failed is worse than an honest partial summary; degrading the new count independently to "(— countered)" keeps both surfaces resilient and the reader oriented (CC-DEGRADE). |
| DV-CC-6 | **Presence-once via `COUNT(DISTINCT c.cid)` + an IN-set over both reference tables — a claim countered by N peers counts ONCE** (ADR-055). | Without `DISTINCT` + the IN-set, a claim countered by N peers would inflate the count N-fold; counting the DISTINCT countered own-claim cids makes the count a true "how many of my claims are contested" presence measure, not a counter-event tally (CC-PRESENCE). |
| DV-CC-7 | **Own-only by the OUTER `claims` table — own claims are countered by PEERS (self-counter blocked), so the own-claim cid lands as a countered `referenced_cid`** (ADR-055). | The count must be "MY claims that are contested", not "claims I counter"; anchoring the outer table on `claims` and matching against the union of `referenced_cid`s gets own-only without a peer-claim join, and self-counter being blocked means no own-on-own double-count. |
| DV-CC-8 | **Shared `render_countered(Option<usize>)` helper — ONE SSOT copy site delegating to slice-17's `render_count` — used by BOTH `render_landing` AND `render_claims_page`; both routes resolve the count INDEPENDENTLY per render** (ADR-055, WD-CC-8). | Two surfaces showing the same count must not be able to diverge; routing both through ONE helper (which itself delegates to `render_count`) means the missing≠zero distinction and the "(N countered)" framing have a single source — the surfaces cannot drift apart in framing or marker. |
| DV-CC-9 | **The countered count is RENDER-ONLY — it never re-weights or re-orders the claims list** (ADR-055). | A count that silently re-ranked the claims would be a control surface, not an orientation aid; keeping it render-only (a text suffix beside the own-claims line / in the header) preserves the read-only contract and the existing claim ordering (CC-NO-REWEIGHT / CC-NO-REORDER). |
| DV-CC-10 | **The SQL is xtask-green BY CONSTRUCTION — it names `claim_references` + `peer_claim_references`, NOT the standalone `claims`-vs-`peer_claims` pair the anti-merging rule fires on** (ADR-055). | The viewer anti-merging xtask rule fires on a query that pairs the standalone `claims` and `peer_claims` tables; the countered count instead unions the two *reference* tables, so it is structurally clear of the rule — verified BY CONSTRUCTION against the real code at DESIGN, not assumed. |
| DV-CC-11 | **The 3rd fault seam (CC-DEGRADE) is TEST-ONLY: `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` honored ONLY by a `#[cfg(debug_assertions)]` function; the release sibling is the identity function (NO env read compiled in)** — release build verified seam-free, all 3 viewer fault tokens ABSENT from the rlib (ADR-055, mirroring slice-16/17). | CC-DEGRADE needs a deterministic mid-request count failure on BOTH surfaces, but a fault hook compiled into release is a production liability; gating the env read behind `debug_assertions` keeps the release binary seam-free while the debug profile drives the degrade scenario. |
| DV-CC-12 | **The slice-16/17 `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` set was EXTENDED to a 3rd token** — an ungated read of ANY of the three fails check-arch (ADR-055). | The generalized token-set guard from slice-17 paid off: adding the 3rd seam was ONE set entry, and an ungated read of the new token is caught structurally — the cfg-gate enforcement stays in ONE place and extends to every future seam by one entry. |
| DV-CC-13 | **The fault-seam degrade is independently pinned**: the debug seam's pass-through + Err-injection is pinned by the new `adapter-http-viewer` unit tests; the release identity sibling is pinned by the xtask seam guard + the release-build seam-free check (ADR-055). | The 2 cargo-mutants "missed" land on the release identity sibling (not compiled under the debug test profile, so neither reachable nor genuinely viable); the debug twin is killed by the in-crate tests, and the release sibling is structurally pinned — so the cfg-dead-branch artifact is covered without theatre. |
| DV-CC-14 | **Phase-3 refactor: none needed — `render_countered` already SSOT-delegated to `render_count`; the 3-fault-seam unification was DECLINED (a security/guard invariant overrides Rule-of-Three).** | Unifying the three fault-seam tokens behind one abstraction would move the token literals off their cfg-gated sites and defeat the `VIEWER_FAIL_SEAM_TOKENS` guard's per-token classification; declining the merge keeps each literal individually guardable — a deliberate non-refactor, the right call. |
| DV-CC-15 | **Closed the package-scoped mutation harness gap with 3 in-crate `adapter-http-viewer` unit tests** (the countered-seam pass-through + Err-injection + a `claims_page` header-render test over a fake store), mirroring the slice-16/17 precedent. | The slice-18 adapter logic is acceptance-covered in the `cli` package and invisible to `cargo mutants -p adapter-http-viewer`; the 3 in-crate tests reach the 4 viable adapter mutants locally (including the second-surface header render) so the per-feature gate is met without a cross-package harness. |

## Cardinal release gates + slice-18 invariants (I-CC-1..n)

The cardinal release gates realized on the landing + `/claims` surfaces — all release-blocking:

1. **Read-only / no-reweight-no-reorder (CARDINAL, I-CC-1)** — the countered count is render-only
   text; no write/sort/filter control; the count NEVER re-weights or re-orders the claims; the web
   process holds no signing key; the read seam has NO mutation method (type-level). Three-layer:
   TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL
   (gold read-only / no-reweight-no-reorder).
2. **Missing≠zero (CARDINAL, I-CC-2)** — the 4th additive `Option<usize>` `LandingSummary` field
   makes a fabricated `0` unrepresentable: `Some(0)` → "(0 countered)" (honest zero), `None` → "(—
   countered)" (the missing marker); the count degrades independently via `.ok()`, the surfaces
   never 5xx (CC-DEGRADE + CC-ZERO).
3. **Presence-once (CARDINAL, I-CC-3)** — a claim countered by N peers counts ONCE, via
   `COUNT(DISTINCT c.cid)` + the IN-set over both reference tables (CC-PRESENCE).
4. **No-N+1 (CARDINAL, I-CC-4)** — ONE FIXED aggregate read per render, invariant to store size; the
   landing's read budget grows by EXACTLY 1, never one-per-row (the N+1-free gold).
5. **Single-source / both-surfaces (CARDINAL, I-CC-5)** — BOTH `render_landing` and
   `render_claims_page`'s `/claims` header render the count via the SAME `render_countered` helper
   (one SSOT copy site delegating to `render_count`); both routes resolve the count INDEPENDENTLY
   per render — the surfaces cannot diverge (CC-HEADER, WD-CC-8).
6. **Own-only (I-CC-6)** — `count_countered_own_claims` is anchored on the outer `claims` table
   (own claims, countered by peers — self-counter blocked); it counts MY contested claims, not
   claims I counter.
7. **LOCAL / offline (I-CC-7)** — the countered count reads the LOCAL counter-reference tables with
   no network (fully offline); nothing persisted (the offline gold).
8. **Anti-misread (I-CC-8)** — the neutral "(N countered)" framing reusing the slice-14 sensibility
   — no penalty / deduction / "disputed by N" / verdict (the anti-misread gold).
9. **Fault seam release-gated (I-CC-9)** — the CC-DEGRADE fault trigger
   (`OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`) is honored ONLY by a `#[cfg(debug_assertions)]`
   function; the release sibling is the identity function (no env read compiled in); release build
   verified seam-free (all 3 viewer fault tokens absent from the rlib); the `VIEWER_FAIL_SEAM_TOKENS`
   xtask guard (now 3 tokens) fails any ungated read.

| # | Invariant | Enforcement |
|---|---|---|
| I-CC-1 | Read-only / no-reweight-no-reorder (the countered count is render-only text; no executable write/sort/filter control; the count never re-weights or re-orders the claims; no key in the process; the read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-reweight-no-reorder, DV-CC-9). Cardinal. |
| I-CC-2 | Missing≠zero (the 4th additive `Option<usize>` `LandingSummary` field makes a fabricated `0` unrepresentable; `Some(0)` → "(0 countered)", `None` → "(— countered)"; per-count independent `.ok()` degrade; never 5xx). | TYPE (the 4th `Option<usize>` field, DV-CC-4) + STRUCTURAL (`render_countered` → `render_count` `Some`/`None` split + the `MISSING_COUNT_MARKER` SSOT, DV-CC-5/8) + BEHAVIORAL (CC-DEGRADE + CC-ZERO). Cardinal. |
| I-CC-3 | Presence-once (a claim countered by N peers counts once; `COUNT(DISTINCT c.cid)` + the IN-set over both reference tables). | STRUCTURAL (the `COUNT(DISTINCT)` + IN-set aggregate, DV-CC-6) + BEHAVIORAL (CC-PRESENCE). Cardinal. |
| I-CC-4 | No-N+1 (ONE FIXED aggregate read per render, invariant to store size; the landing's read budget grows by exactly 1, never one-per-row). | STRUCTURAL (the single fixed count read, DV-CC-3) + BEHAVIORAL (N+1-free gold). Cardinal. |
| I-CC-5 | Single-source / both-surfaces (BOTH `render_landing` and the `/claims` header render the count via the SAME `render_countered` helper; both routes resolve the count independently per render — the surfaces cannot diverge). | STRUCTURAL (the shared `render_countered` SSOT copy site delegating to `render_count`, DV-CC-8) + BEHAVIORAL (CC-HEADER). Cardinal. |
| I-CC-6 | Own-only (`count_countered_own_claims` anchored on the outer `claims` table — own claims, countered by peers, self-counter blocked; counts MY contested claims, not claims I counter). | STRUCTURAL (the outer `claims` table + the union-of-reference-tables IN-set, DV-CC-7) + BEHAVIORAL (CC-WS / CC-PRESENCE). |
| I-CC-7 | LOCAL / offline (the countered count reads the LOCAL counter-reference tables with no network; nothing persisted). | STRUCTURAL (the read-only local aggregate count) + BEHAVIORAL (CC-OFFLINE gold). |
| I-CC-8 | Anti-misread (the neutral "(N countered)" framing reusing the slice-14 sensibility — no penalty / deduction / "disputed by N" / verdict). | STRUCTURAL (the neutral text in `render_countered`, reusing the slice-14 framing) + BEHAVIORAL (CC-ANTI-MISREAD gold). |
| I-CC-9 | Fault seam release-gated (CC-DEGRADE trigger honored ONLY by `#[cfg(debug_assertions)]`; release sibling = identity; release build seam-free, all 3 tokens absent; the `VIEWER_FAIL_SEAM_TOKENS` xtask guard — now 3 tokens — fails any ungated read). | TYPE/COMPILE (the `#[cfg(debug_assertions)]` gate; the release identity sibling, DV-CC-11) + STRUCTURAL (the `scan_viewer_fail_seam_guard` over the 3-token set, DV-CC-12; release-build seam-free check) + BEHAVIORAL (the in-crate seam pass-through + Err-injection tests, DV-CC-13). Cardinal. |

All slice-18 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 + slice-17 I-LD-1..8
sets (read-only / no key / human gate / offline + loopback / progressive enhancement / structural
fragment/page parity / the missing≠zero `LandingSummary` shape); the countered count is shown
verbatim through the shared `render_countered` helper on both surfaces.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_counter_aware_counts` corpus (CC-WS, CC-HEADER /
  CC-PRESENCE, CC-ZERO, CC-DEGRADE, CC-NO-REWEIGHT / CC-NO-REORDER, CC-READONLY / CC-OFFLINE /
  CC-N+1 / CC-ANTI-MISREAD — the thick walking skeleton at CC-WS) + the GOLD
  `viewer_counter_aware_counts_invariants` (read-only / no-reweight-no-reorder, LOCAL / offline,
  N+1-free, anti-misread) GREEN + the `viewer-domain` unit/property tests (the `render_countered`
  projection + the 4th `LandingSummary` field) + the new `adapter-http-viewer` in-crate unit tests
  (the countered-seam pass-through + Err-injection + the `claims_page` header-render test over a
  fake store); slices 06/11-14/16/17 corpora GREEN — zero regression. The `ViewerServer` harness
  drives the REAL `openlore ui` over HTTP; the CC-DEGRADE fault is driven by the TEST-ONLY
  `#[cfg(debug_assertions)]` env seam.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, no new
  allowlist edge: `render_countered` is a total fn of `Option<usize>` delegating to `render_count`
  (no `viewer-domain → claim-domain` reachout). The viewer capability rule is unchanged (read-only
  counts; no signing/identity/PDS, no store-write; the count never re-weights/re-orders). The SQL is
  clear of the anti-merging rule BY CONSTRUCTION (it names `claim_references` + `peer_claim_references`,
  not the standalone `claims`-vs-`peer_claims` pair). The `scan_viewer_fail_seam_guard` now iterates
  `VIEWER_FAIL_SEAM_TOKENS` (slice-16 active-set + slice-17 peer-claims-count + slice-18
  countered-count); an ungated read of ANY of the three fails check-arch.
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor: none needed**
  (`render_countered` already SSOT-delegates to `render_count`; the 3-fault-seam token unification
  was correctly DECLINED — a security/guard invariant overrides Rule-of-Three, DV-CC-14);
  `viewer-domain` purity intact (no I/O imports; maud + ports only; the store threading + `.ok()`
  degrade live in the effect shell; NO `claim-domain` reachout).
- **Release-build seam check**: the `#[cfg(not(debug_assertions))]` release build was verified
  **seam-free** — all **3** viewer fault tokens (slice-16 active-set, slice-17 peer-claims-count,
  slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`) are NOT compiled into the release rlib (each
  release sibling is the identity function, no env read).
- **Adversarial review**: **APPROVED**, **0 defects, zero Testing Theater**. Presence-once verified
  (a claim countered by N peers counts once — the `COUNT(DISTINCT)` + IN-set confirmed); single-source
  verified (both surfaces render via the SAME `render_countered` helper — they cannot diverge);
  additive verified (the 4th `Option<usize>` field disturbs none of the existing three counts); the
  fault-seam confirmed release-gated in 3 layers (the `#[cfg(debug_assertions)]` gate + the xtask
  3-token-set guard + the release-build seam-free check).
- **DES integrity**: PASS — all 7 steps have complete DES traces (7/7).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the `render_countered` projection +
the 4th `LandingSummary` field render) AND the `adapter-http-viewer` slice-18 logic (the countered
count resolution into both surfaces + the fault-seam pass-through + the `/claims` header render).
The slice-04/05 cross-package lesson stays applied — the killing properties are kept IN-CRATE.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`render_countered` projection + the 4th `LandingSummary` field render, in-diff) | 6 | 6 | 0 | **100%** (6/6 in-diff viable) |
| `adapter-http-viewer` slice-18 logic (countered-count resolution into both surfaces + fault-seam pass-through + `/claims` header render, in-diff) | 4 | 4 | 0 | **100%** (4/4 viable, after the 3 in-crate unit tests) |

Slice-18 per-feature gate SATISFIED (≥80%; actual **100% on the genuinely-viable in-diff**, 0
viable missed). **The 2 remaining cargo-mutants "missed" are both the `#[cfg(not(debug_assertions))]`
release identity sibling of the fault seam** — a **cfg-dead-branch artifact** NOT compiled under the
debug test profile (neither reachable nor genuinely viable), the **same class as slice-16/17's lone
survivor**. They are **independently pinned** by the `VIEWER_FAIL_SEAM_TOKENS` xtask guard + the
release-build seam-free check (DV-CC-13) — covered without theatre.

**Closing the package-scoped harness gap**: the slice-18 `adapter-http-viewer` logic is
acceptance-covered in the `cli` package and therefore invisible to `cargo mutants -p
adapter-http-viewer` (the package-scoped harness cannot see cross-package acceptance tests). Three
**in-crate `adapter-http-viewer` unit tests** were added (the countered-seam pass-through, the
Err-injection, and a `claims_page` header-render test over a fake store) to close that gap —
**mirroring the slice-16/17 adapter-unit-test precedent** — so the 4 viable adapter mutants
(including the second-surface header render) are caught locally. `adapter-duckdb` is NOT mutated by
design (effect shell; covered by the GOLD invariants through the real binary). DEVOPS sweep is the
ongoing backstop.

## Lessons learned / issues

- **Tying two shipped families together is mostly a SHAPE choice, not new architecture**: the
  counter-flag family (slices 11-14) already carried the per-row contestation signal and slice-17
  carried the front-door volume summary — slice-18 connected them with ONE count read, ONE additive
  field, and ONE shared render helper. **Lesson: when a slice's value is "tie two already-shipped
  capabilities into one at-a-glance signal", the work is almost entirely the SHAPE of the joining
  read (a count, not a list; one additive field; one shared helper) — get that shape right and the
  surfaces fall out.**
- **An additive `Option<usize>` field inherits the missing≠zero TYPE guarantee for free (DV-CC-4)**:
  adding the countered count as a 4th `Option<usize>` field on `LandingSummary` (not a flag, not a
  sentinel) meant a fabricated `0` was unrepresentable from day one — `Some(0)` → "(0 countered)",
  `None` → "(— countered)" — and the addition disturbed none of the existing three counts. **Lesson:
  when extending a summary that already encodes absence in the TYPE, add the new field in the SAME
  shape (`Option`) — the new count inherits the cardinal guarantee with no new convention.**
- **Two surfaces showing one count must share ONE render site or they WILL diverge (DV-CC-8)**:
  routing both the landing own-claims line and the `/claims` header through the SAME
  `render_countered` helper (itself delegating to `render_count`) is what makes the framing and the
  missing-marker identical on both — and both surfaces resolve the count independently per render so
  neither goes stale. **Lesson: when the same value appears on two surfaces, give it ONE render SSOT
  (a shared helper) and let each surface resolve it independently — single render source + independent
  resolution is how you get "identical framing, never stale" without coupling the routes.**
- **A count must be presence-once or it lies about contestation (DV-CC-6)**: a claim countered by N
  peers must count ONCE — `COUNT(DISTINCT c.cid)` + the IN-set over both reference tables makes the
  count a true "how many of my claims are contested" measure rather than a counter-event tally.
  **Lesson: when a count answers "how many X are in state Y" and an X can enter state Y via multiple
  events, count DISTINCT X — a raw event count silently inflates and misrepresents the very thing the
  count is for.**
- **A third `#[cfg]`-gated seam is ONE set entry because slice-17 generalized the guard (DV-CC-12)**:
  CC-DEGRADE added a third token (`OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`); because slice-17 had
  already refactored `scan_viewer_fail_seam_guard` to iterate `VIEWER_FAIL_SEAM_TOKENS`, extending it
  was a single set entry, and an ungated read of the new token is caught structurally. **Lesson: a
  structural guard generalized to a token SET in an earlier slice pays its dividend the moment the
  next seam arrives — the cost of the third seam is one entry, and the release-build seam-free check
  still verifies the binary, not just the cfg gate.**
- **A security/guard invariant overrides the Rule-of-Three (DV-CC-14)**: with three fault-seam tokens
  now in the tree, the "DRY" instinct is to unify them — but unifying would move the token literals
  off their cfg-gated sites and DEFEAT the `VIEWER_FAIL_SEAM_TOKENS` per-token classification. The
  merge was correctly DECLINED. **Lesson: the Rule-of-Three does not apply when the three copies are
  the very literals a structural guard classifies — record the non-refactor as a decision so it reads
  as a deliberate call, not an oversight.**
- **xtask-green BY CONSTRUCTION beats xtask-green by luck (DV-CC-10)**: the countered-count SQL was
  verified at DESIGN to name `claim_references` + `peer_claim_references` (the two *reference* tables),
  NOT the standalone `claims`-vs-`peer_claims` pair the viewer anti-merging rule fires on — checked
  against the real rule + real code, not assumed. **Lesson: when a structural rule fires on a specific
  table pairing, verify the new query is clear of it BY CONSTRUCTION at DESIGN against the real rule —
  "it'll probably pass" is not a gate; reading the rule and the SQL together is.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-055 fixed the contracts; field-level shaping (the 4th `LandingSummary` field, the `render_countered` helper, the dual-surface threading) left to DELIVER. | All adopted; the 4th `Option<usize>` field, `render_countered` (`Some`→"(n countered)" / `None`→"(— countered)"), and BOTH the landing line and the `/claims` header materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-055 fixed the new count-only read `count_countered_own_claims` (COUNT(DISTINCT) + IN-set over both reference tables, over `counter_presence_for(own_cids).len()`). | Shipped exactly — count-only, presence-once, own-only, read-only on the existing port, injection-safe; a claim countered by N peers counts once (CC-PRESENCE green). | Resolved at DELIVER. |
| 3 | ADR-055 fixed the 3rd fault seam as TEST-ONLY (`#[cfg(debug_assertions)]`), the release sibling = identity, and the xtask guard extended to a 3rd token. | The `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` seam landed `#[cfg(debug_assertions)]`-only; `scan_viewer_fail_seam_guard` `VIEWER_FAIL_SEAM_TOKENS` extended to 3 tokens; release build verified seam-free (all 3 tokens absent from the rlib). | Resolved at DELIVER. |
| 4 | ADR-055 fixed "no new pure-core edge; check-arch unchanged (member count, allowlist); SQL clear of the anti-merging rule by construction." | `render_countered` is a total fn delegating to `render_count`; `check-arch` reports 21 members, no new allowlist edge, no new route; the SQL is clear of the anti-merging rule by construction. | Resolved at DELIVER. |
| 5 | The `/claims` header threading expected to be real work (a second surface resolving the count independently and rendering via the shared helper). | The `/claims` header (01-02) was real work — it resolved the count independently and rendered via the SAME `render_countered` helper (CC-HEADER / CC-PRESENCE green; single-source held). | Resolved at DELIVER. |
| 6 | Phase-3 refactor anticipated. | NONE needed — `render_countered` already SSOT-delegated to `render_count`; the 3-fault-seam token unification was DECLINED to keep each literal individually `cfg`-gated (a security/guard invariant overrides Rule-of-Three, DV-CC-14). | No refactor at DELIVER (a deliberate non-refactor). |
| 7 | `adapter-http-viewer` mutation coverage expected via the cli-package acceptance suite. | The package-scoped harness gap was closed with 3 in-crate `adapter-http-viewer` unit tests (mirroring slice-16/17); 4/4 viable adapter mutants caught (including the second-surface header render). | Closed at DELIVER (DV-CC-15). |
| 8 | Peer-claims-countered scope. | DEFERRED per WD-CC-7 — slice-18 ships own-claims-countered only; peer-claims-countered is a later slice. | Deferred at DISCUSS; honored at DELIVER (own-only). |
| 9 | Review expected to pass clean. | Review APPROVED, 0 defects, zero Testing Theater; presence-once / single-source / additive / fault-seam-gating all verified. | Confirmed at DELIVER. |
| 10 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-CC-2, 100% viable in-diff, 0 viable missed; the 2 "missed" are cfg-dead-branch artifacts). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-counter-aware-counts/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, requirements, user-stories,
  acceptance-criteria, outcome-kpis, dor-checklist), `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-17 archive** (the read-only `GET /` landing this slice extends, home of the
  `LandingSummary` ADT + `render_count` + the `.ok()`-degrade count-resolution pattern):
  `docs/evolution/viewer-landing-dashboard-evolution.md`
- **Parent counter-flag family archives** (the counter-reference data this slice aggregates, the
  slice-12-flagged `/claims` rows, and the slice-14 neutral-framing sensibility):
  `docs/evolution/viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`
- **Parent slice-16 archive** (the `#[cfg(debug_assertions)]` test-only fault-seam + the
  `scan_viewer_fail_seam_guard` xtask guard pattern this slice reuses + extends to a 3rd token):
  `docs/evolution/viewer-search-follow-state-evolution.md`
- **Slice-18 ADR**:
  `docs/adrs/ADR-055-counter-aware-counts-count-only-countered-own-claims-read-4th-additive-landingsummary-field-shared-render-countered-helper-across-both-surfaces.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-counter-aware-counts/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-counter-aware-counts/deliver/execution-log.json`,
  `docs/feature/viewer-counter-aware-counts/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_counter_aware_counts.rs` (CC-WS, CC-HEADER / CC-PRESENCE, CC-ZERO,
  CC-DEGRADE, CC-NO-REWEIGHT / CC-NO-REORDER, CC-READONLY / CC-OFFLINE / CC-N+1 / CC-ANTI-MISREAD —
  the thick walking skeleton at CC-WS), `tests/acceptance/viewer_counter_aware_counts_invariants.rs`
  (the 9 gold invariants — read-only / no-reweight-no-reorder, LOCAL / offline, N+1-free, anti-misread)
- **Reused fault-seam + xtask-guard pattern**: `xtask` (`scan_viewer_fail_seam_guard`,
  `VIEWER_FAIL_SEAM_TOKENS` — slice-16 active-set + slice-17 peer-claims-count + slice-18
  countered-count); the ADR-026 pubkey-seam release-gate pattern (`classify_cfg_gated_token`)
- **Extended viewer crates**: `crates/viewer-domain` (the shared `render_countered` helper delegating
  to `render_count` + the 4th `LandingSummary` field), `crates/adapter-http-viewer` (the countered
  count resolved into BOTH the landing own-claims line and the `/claims` header via `.ok()` degrade +
  the `#[cfg(debug_assertions)]` fault seam + the release identity sibling + the 3 in-crate unit
  tests), `crates/adapter-duckdb` (the read-only `count_countered_own_claims` impl —
  `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM
  claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references
  WHERE ref_type='counters')`), `crates/ports` (the read seam + the 4th `LandingSummary` field)
- **Reused count reads + data (NOT re-implemented)**: the slice-17 `LandingSummary` / `render_count`;
  the slice-12 counter-reference tables (`claim_references` + `peer_claim_references`,
  `ref_type='counters'`)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml` — J-003b (the
  orientation / at-a-glance facet realized here)
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-claim-threads-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`,
  `viewer-search-follow-state-evolution.md`, `viewer-landing-dashboard-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS c2096d3 → DESIGN a654403 → DISTILL a3b65e9 → roadmap (post-a3b65e9) → 01-01 950a2f4 →
01-02 2616e95 → 01-03 d142eaf → 02-01 2a1125a → 02-02 6dd068d → 02-03 d92a5c3 → 03-01 5589a0c →
mutation-gate unit tests dacbdad.
