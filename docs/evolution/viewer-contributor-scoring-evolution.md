# Evolution: viewer-contributor-scoring (slice-09 contributor-scoring `/score` view on the read-only viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-contributor-scoring/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `slices/`, `deliver/`) and ADR-039..ADR-041 under `docs/adrs/`;
> this file is the post-mortem summary. This slice is a **DELTA on shipped work**:
> slice-04 (`openlore-scoring-graph` — the weighted-pairing scorer this view renders),
> slice-06 (`htmx-scraper-viewer` — the read-only viewer), and slice-07
> (`viewer-htmx-swaps` — the htmx progressive-enhancement layer). Read those parent
> archives (`docs/evolution/openlore-scoring-graph-evolution.md`,
> `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`) for the
> scorer and the surfaces this slice composes.

## Summary

`viewer-contributor-scoring` adds a **`GET /score?contributor=<did>` view** to the
`openlore ui` read-only viewer: the **browser surface for contributor scoring**, realizing
the **J-002c transparency** job. Given a contributor DID, the view renders the slice-04
scorer's **weighted pairings as a per-claim breakdown** — each row naming `author_did` +
`cid` + verbatim confidence + bonuses + subtotal — so a reader can see not just *that* a
contributor scored a weight but *how* every weight was earned. It forks htmx fragment vs
full page via the slice-07 `Shape::from_request` and is nav-linked alongside the existing
viewer views.

The load-bearing thesis: **the browser never recomputes a score — it PROJECTS the slice-04
result, and it shows the breakdown on EVERY weight (anti-opaque)**. The view reads the
LOCAL store read-only, runs the SAME pure `scoring::score`, and renders; the sum of the
per-claim subtotals equals the displayed weight **by construction** (sum-to-weight,
CARDINAL J-002c), because there is no viewer-side recompute of confidence, bonuses, or
buckets (WD-CS-6). Read-only is enforced at **three layers**: a `StoreReadPort` with no
mutation method (TYPE), the `xtask check-arch` viewer capability rule (STRUCTURAL), and a
behavioral GOLD invariant (BEHAVIORAL).

The slice ships **ZERO new crates** (workspace stays at **21 members**). It is an
**additive render surface, not a re-architecture**: it extends `viewer-domain` (a pure
`ScoreState` ADT + `render_score_*` projecting the slice-04 `WeightedPairing`),
`adapter-http-viewer` (the `/score` handler + `Shape` fork + a nav link), the
`adapter-duckdb` read impl (a read-only contributor-scoring feed query), the `ports`
(the read seam), the `cli` (`ui` wiring, still no key), and `xtask` (one new allowlist
edge `viewer-domain → scoring` + the capability rule). It REUSES the slice-04 pure
`scoring::score` + `WeightedPairing` (the scoring is consumed, NOT reimplemented).

### What shipped (one paragraph)

A `GET /score` view: a GET form (enter a contributor DID) → on submit the viewer runs the
read-only `query_contributor_scoring_feed` (a **read-only UNION-ALL** of `claims ∪
peer_claims`, **no merge JOIN**), feeds the rows to the slice-04 pure `scoring::score`,
maps the outcome to a `ScoreState` ADT (`Form | Scored{WeightedView} | NoClaims{
contributor}`), and projects it into HTML via `render_score_*`, forking by
`Shape::from_request` (the slice-07 `HX-Request` selector) — a full page without the
header, the score-results fragment with it. Every weight carries its **per-claim
breakdown** (author_did + cid + verbatim confidence + bonuses + subtotal); the subtotals
**sum to the displayed weight by construction** (no viewer recompute — WD-CS-6); identical
content from two distinct authors renders as **two attributed rows** (anti-merging); a
sparse contributor renders a `[SPARSE]` marker + an honesty line framing the score as
**breadth, not magnitude**; an unknown / no-claims contributor renders a guided
`NoClaims{contributor}` notice in **both shapes**. The store read is **LOCAL and read-only**
(no PDS, no network for the score itself); the bind stays loopback-only; nothing is
persisted.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-05 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-05 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-05 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-05 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **13/13 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **19 acceptance scenarios** GREEN: **14 `viewer_contributor_scoring`** (C-1..C-9 +
  C-10 — the walking-skeleton breakdown fragment, the no-write-surface infra assertion,
  the no-JS full page + fragment/page parity, the anti-opaque per-claim breakdown, the
  reproduce-by-hand sum-to-weight, the anti-merging two-author rows, the sparse honesty
  / breadth-not-magnitude render, the verbatim confidence + weight, and the unknown
  contributor guided `NoClaims` in both shapes) + **5 GOLD invariants**
  (`viewer_contributor_scoring_invariants` — C-INV read-only, no-write, offline-chrome,
  offline-data, and the CARDINAL sum-to-weight). Plus **71 `viewer-domain` unit/property
  tests** (the new `ScoreState` projection + `render_score_*` parity + the inherited
  slice-06/07 render properties). The `ViewerServer` harness drives the REAL `openlore
  ui` over HTTP; the store is seeded through the REAL ingest path.
- **Slices 04/06/07 corpora GREEN — zero regression** (the full workspace acceptance
  suite green across all slices).
- **NO new crate**: extends `viewer-domain` (PURE) + `adapter-http-viewer` (EFFECT) +
  `adapter-duckdb` (EFFECT, read impl) + `ports` + `cli` (DRIVER) + `xtask` (tooling) in
  place; REUSES the slice-04 pure `scoring::score` + `WeightedPairing`. Workspace member
  count stays **21** (19 production + 1 test-support + 1 xtask); `cargo xtask check-arch`
  reports "21 workspace members".
- **NO new production dependency**: `scoring` is already in-workspace; `maud`/`hyper`
  unchanged; no `deny.toml` change.
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production
  functions (**13/13 in-diff viable caught, 0 missed**) — exceeds the ≥80% per-feature
  gate.
- **3 ADRs** (ADR-039..ADR-041) all Accepted/shipped.
- DES integrity: 13/13 steps have complete DES traces.
- Adversarial review: **APPROVED** after one revision (D1 sparse-coverage + D2
  read-only-doc, fixed in one pass).
- `cargo xtask check-arch`: OK (21 workspace members, one new `viewer-domain → scoring`
  allowlist edge).

## Wave-by-wave changelog

### DISCUSS (2026-06-05)

Luna framed the slice as a **brownfield DELTA on slices 04/06/07**: the browser surface
for contributor scoring, realizing the **J-002c transparency** job — a reader can see not
only a contributor's score but *how every weight was earned*. Persona is **P-001 (Maria,
the node operator)**, the viewer's operator wearing the scoring-transparency hat. The
load-bearing DISCUSS decision is **WD-CS-6: the viewer never recomputes confidence,
bonuses, or buckets — it PROJECTS the slice-04 `WeightedPairing`** so the displayed
breakdown sums to the weight by construction. slice-09 **REALIZES the existing scoring +
viewer KPI contracts on the browser surface** (the sum-to-weight transparency, the
anti-merging attribution, the read-only / offline guardrails) rather than minting new KPI
IDs. The walking skeleton is the C-1 thread (contributor DID → read-only feed → pure score
→ per-claim breakdown HTML fragment), validating the riskiest assumption first — that the
read-only viewer can render the scorer's full breakdown transparently while preserving the
sum-to-weight cardinal.

### DESIGN (2026-06-05)

Morgan locked slice-09 as an **additive render surface, not a re-architecture** — ZERO new
crates, ZERO new binary, ZERO new architectural style, ZERO new persisted type. The open
decisions were resolved adopting the DISCUSS leans, captured in three ADRs:

- **ADR-039** (viewer-local contributor-scoring read seam + pure-scorer reuse): a **NEW
  read-only seam** `query_contributor_scoring_feed` on the store read port (a read-only
  UNION-ALL of `claims ∪ peer_claims`, NO merge JOIN), feeding the **REUSED** slice-04
  pure `scoring::score`. ONE scoring path workspace-wide; the viewer consumes the
  `WeightedPairing`, it does not recompute (WD-CS-6).
- **ADR-040** (`ScoreState` ADT + `viewer-domain` projection + transparent breakdown): a
  **NEW pure `viewer-domain` projection** of the slice-04 `WeightedPairing` into HTML — a
  `ScoreState` ADT (`Form | Scored{WeightedView} | NoClaims{contributor}`) with
  `render_score_*` renderers that show the per-claim breakdown on EVERY weight
  (anti-opaque), sum-to-weight by construction, with the `[SPARSE]` + honesty-line
  breadth-not-magnitude render for thin contributors.
- **ADR-041** (`GET /score` route + GET form + params + nav + arch enforcement): its
  **OWN route `GET /score?contributor=<did>`**, added to the nav; a GET form →
  bookmarkable/shareable URL + plain no-JS navigation, htmx fragment fork via
  `HX-Request` (the slice-07 pattern); plus the **`xtask check-arch` deltas** — the new
  pure-core allowlist edge `viewer-domain → scoring`, and the viewer capability rule
  confirming the contributor-scoring read is read-only (no signing/identity/PDS, no
  store-write).

The read-only contract is enforced at THREE layers (a `StoreReadPort` with no mutation
method, the `xtask check-arch` viewer capability rule, and a behavioral GOLD invariant).
The C4 views, the `/score` data-flow, and the I-CS-1..10 structural-guarantee table are in
the DESIGN sections of `feature-delta.md`.

### DISTILL (2026-06-05)

Quinn authored the executable acceptance corpus across two `[[test]]` targets:

- **`viewer_contributor_scoring.rs`** (Tier A — `C-` ids C-1..C-10): the walking-skeleton
  breakdown fragment (C-1) + the read-only no-write-surface infra assertion (C-1b), the
  no-JS full page + fragment/page parity (C-2/C-3), the anti-opaque per-claim breakdown
  naming `author_did` + `cid` + verbatim confidence (C-4), the reproduce-by-hand
  sum-to-weight HTML parser (C-5), the anti-merging two-author rows (C-6), the
  no-local-claims reliability render (C-5 reliability arm), the sparse honesty /
  breadth-not-magnitude render (C-7/C-10), the verbatim confidence + weight (C-8), and the
  unknown-contributor guided `NoClaims` in both shapes (C-9).
- **`viewer_contributor_scoring_invariants.rs`** (gold guardrails — 5 `C-INV-` ids):
  C-INV-ReadOnly (read-only across rich/sparse/empty × page/fragment via the row-count
  delta), C-INV-NoWrite (no `/score` response adds a write/sign control), the two offline
  invariants (no-CDN chrome + fully-offline score — the score reads the LOCAL store, no
  network), and the CARDINAL sum-to-weight gate (anti-opaque + sum-to-weight across
  rich/sparse/conflicting × both shapes).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the
store is seeded through the REAL ingest path. The NEW shared render assertion
`assert_score_html_breakdown_sums_to_displayed_weight` is an **observable-HTML
reproduce-by-hand parser** built for the sum-to-weight cardinal — it parses the rendered
breakdown rows out of the HTML and re-adds the subtotals, asserting they equal the
displayed weight (the score is checked against the OBSERVABLE output, not a re-call of the
scorer). RED classification: both targets COMPILE green, scenarios FAIL via `todo!()` =
MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-05)

Executed **13 roadmap steps** via DES-monitored crafter dispatches, each commit carrying a
`Step-ID: NN-NN` trailer. Walking skeleton `5cecac8` (01-01) → final cleanup `7999ff4`
(review D1/D2). Per-step SHAs are in `deliver/execution-log.json`.

- **WS / breakdown render (01-xx)**: the `/score` route + `Shape::from_request` dispatch +
  the `ScoreState` ADT + the read-only `query_contributor_scoring_feed` seam + the
  `render_score_*` parity split + the `scoring::score` wiring in the `ui` verb. The C-1
  walking skeleton: a contributor's weighted pairings rendered as a per-claim breakdown
  HTML fragment from the LOCAL store.
- **Transparency + anti-merging (02-xx)**: the anti-opaque per-claim breakdown (C-4), the
  observable-HTML reproduce-by-hand sum-parser (C-5, `assert_score_html_breakdown_sums_to_
  displayed_weight`), and the anti-merging two-author seam (C-6) — identical content from
  two authors renders as two attributed rows.
- **Sparse honesty + empty (03-xx)**: the `[SPARSE]` marker + the breadth-not-magnitude
  honesty render (C-7/C-10), the verbatim confidence + weight (C-8), and the
  unknown-contributor guided `NoClaims` in both shapes (C-9).
- **Gold invariants (04-xx)**: the C-INV-* guardrails (read-only, no-write, offline-chrome,
  offline-data, the CARDINAL sum-to-weight) driving the real binary — they flipped GREEN
  for free off the confirmatory render path.

The 13-step shape: **01-01** is the thick walking skeleton; **01-02/01-03/02-01/03-02/
03-03** are confirmatory (the render fell out of the skeleton); **02-02** (the HTML
sum-parser), **02-03** (the anti-merging seam), and **03-01** (the sparse honesty render)
carried real work; **04-01..04-04** (the gold invariants) flipped green for free.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-CS-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..08 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-CS-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the `ScoreState` projection + `render_score_*` renderers), matching slice-02..08 DV-2. The killing properties are kept IN-CRATE (the 71 `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally (no cross-package cargo-mutants scope detour). 13/13 in-diff viable caught, 0 missed. |
| DV-CS-3 | **The viewer PROJECTS the slice-04 `WeightedPairing` — it never recomputes confidence/bonuses/buckets** (WD-CS-6 / ADR-039/040). The breakdown subtotals sum to the displayed weight BY CONSTRUCTION. | Sum-to-weight (CARDINAL J-002c) is guaranteed not by a viewer-side test but by the ABSENCE of a viewer-side recompute path — the viewer consumes the slice-04 score. A second scoring path is the classic place a displayed weight drifts from its breakdown. ONE scorer workspace-wide. |
| DV-CS-4 | **`query_contributor_scoring_feed` is a read-only UNION-ALL of `claims ∪ peer_claims` with NO merge JOIN** (ADR-039). | A merge JOIN is where two distinct authors' identical content would collapse into one row (anti-merging is cardinal); the UNION-ALL preserves every author's claim as its own row, feeding the scorer the same shape the CLI sees. Read-only by construction — no mutation method on the read seam. |
| DV-CS-5 | **The sum-to-weight assertion is an observable-HTML reproduce-by-hand parser** (`assert_score_html_breakdown_sums_to_displayed_weight`), not a re-call of the scorer. | The cardinal is checked against the OBSERVABLE rendered output — the test parses the breakdown rows out of the HTML and re-adds the subtotals, asserting they equal the displayed weight. Re-calling the scorer would test the scorer, not the render; parsing the HTML tests what the reader actually sees. |
| DV-CS-6 | **The contributor view is author-scoped (prefix-match)**, so genuinely-unrelated co-claimants surface via the OBJECT dimension (`graph_query --object`), not `/score`. | `/score` answers "how did THIS contributor earn their weight," scoped to the author; finding everyone who touched a claim is a different question answered by the object dimension. Calling the scope out keeps the view's contract honest (it is not a whole-claim co-author finder). |
| DV-CS-7 | **Read-only enforced at three layers** (a `StoreReadPort` with no mutation method [TYPE] + the `xtask check-arch` capability rule [STRUCTURAL] + the C-INV-ReadOnly / C-INV-NoWrite gold [BEHAVIORAL]) plus the new `viewer-domain → scoring` allowlist edge. | The read-only guarantee cannot be defeated by any single-layer slip — the type forbids a write call, the arch check forbids a write capability in the viewer surface, and the gold proves the store row counts are unchanged across every shape. |

## Cardinal release gates + slice-09 invariants (I-CS-1..10)

The cardinal release gates realized on the browser surface — all release-blocking:

1. **Read-only / no key (I-CS-1)** — `/score` is a READ; no write/sign/subscribe route;
   the web process holds no signing key; the contributor-scoring read seam has NO mutation
   method (type-level). Three-layer: TYPE (no write method) + STRUCTURAL (`xtask
   check-arch` viewer capability rule) + BEHAVIORAL (C-1b + gold C-INV-ReadOnly /
   C-INV-NoWrite).
2. **Anti-opaque / breakdown-on-every-weight (I-CS-2/3)** — every displayed weight carries
   its per-claim breakdown (author_did + cid + verbatim confidence + bonuses + subtotal);
   no weight is shown as a bare number. BEHAVIORAL (C-4).
3. **Sum-to-weight (CARDINAL J-002c, I-CS-4)** — the per-claim subtotals sum to the
   displayed weight BY CONSTRUCTION (the viewer projects the slice-04 `WeightedPairing`,
   no recompute); verified against the OBSERVABLE HTML (C-5 + gold sum-to-weight across
   rich/sparse/conflicting × both shapes).
4. **Verbatim confidence (I-CS-5)** — confidence rendered through the EXISTING
   `render_confidence` (`0.90`, never `0.9`/`90%`) (C-8).
5. **Anti-merging two-author rows (I-CS-6)** — identical content from two distinct authors
   renders as two attributed rows; no merged/consensus row (the read seam UNION-ALLs, no
   merge JOIN) (C-6).
6. **Sparse honesty / breadth-not-magnitude (I-CS-7)** — a thin contributor renders a
   `[SPARSE]` marker + an honesty line framing the score as breadth, not magnitude
   (C-7/C-10).
7. **Guided NoClaims both shapes (I-CS-8)** — an unknown / no-claims contributor renders a
   guided `NoClaims{contributor}` notice in BOTH the fragment and the full page (C-9).
8. **Offline / local-only (I-CS-9/10)** — the `/score` page references only the vendored
   local htmx asset (no CDN), AND the score itself reads the LOCAL store with no network
   (fully offline); loopback-only bind; nothing persisted (the two offline golds).

The full slice-09 invariant set (I-CS-1..10; structural-guarantee detail in the DESIGN
section of `feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-CS-1 | Read-only / no key (`/score` is a READ; no write/sign/subscribe route; no key in the process; the read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (C-1b + gold C-INV-ReadOnly/NoWrite). Cardinal. |
| I-CS-2 | Anti-opaque (every displayed weight carries its breakdown; no bare-number weight). | STRUCTURAL (`render_score_*` always renders the breakdown) + BEHAVIORAL (C-4). |
| I-CS-3 | Breakdown-on-every-weight (per-claim row names author_did + cid + verbatim confidence + bonuses + subtotal). | STRUCTURAL (the per-claim row template) + BEHAVIORAL (C-4). |
| I-CS-4 | Sum-to-weight (per-claim subtotals sum to the displayed weight by construction; viewer projects, never recomputes). | TYPE/STRUCTURAL (consumes the slice-04 `WeightedPairing`, WD-CS-6; no viewer recompute path) + BEHAVIORAL (C-5 + gold sum-to-weight). CARDINAL (J-002c). |
| I-CS-5 | Confidence verbatim (rendered through the EXISTING `render_confidence` — `0.90`, never `0.9`/`90%`). | STRUCTURAL (one `render_confidence` site, reused) + BEHAVIORAL (C-8). |
| I-CS-6 | Anti-merging (identical-content-different-author = two attributed rows; no merged/consensus row). | STRUCTURAL (read seam UNION-ALL, no merge JOIN, DV-CS-4) + BEHAVIORAL (C-6). |
| I-CS-7 | Sparse honesty (a thin contributor gets a `[SPARSE]` marker + a breadth-not-magnitude honesty line). | STRUCTURAL (the sparse render arm) + BEHAVIORAL (C-7/C-10). |
| I-CS-8 | Guided NoClaims both shapes (unknown/no-claims contributor → a guided `NoClaims` notice in fragment AND full page). | TYPE (`ScoreState::NoClaims{contributor}` arm) + BEHAVIORAL (C-9). |
| I-CS-9 | Offline / no-CDN chrome (the `/score` page references only the vendored local htmx asset; zero off-host references). | STRUCTURAL (the shared `htmx_script` fn + pinned asset) + BEHAVIORAL (gold offline-chrome). |
| I-CS-10 | Fully-offline score / local-only (the score reads the LOCAL store with no network; loopback-only; nothing persisted). | STRUCTURAL (the read-only local feed query; loopback guard unchanged) + BEHAVIORAL (gold offline-data + C-INV-ReadOnly row-count delta). |

All slice-09 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets
(read-only / no key / human gate / offline + loopback / progressive enhancement /
structural fragment/page parity); confidence stays shown verbatim in both shapes.

## Quality gates — final report

- **Acceptance / integration**: 14 `viewer_contributor_scoring` (C-1..C-10) + 5 GOLD
  `viewer_contributor_scoring_invariants` GREEN + 71 `viewer-domain` unit/property tests;
  slices 04/06/07 corpora GREEN — zero regression. The `ViewerServer` harness drives the
  REAL `openlore ui` over HTTP; the store is seeded through the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate; the new delta is
  the `viewer-domain → scoring` pure-core dependency allowlist entry (pure → pure edge) +
  the confirmed viewer capability rule (read-only contributor-scoring read; no
  signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch + check-probes clean; `viewer-domain` purity
  intact (no I/O imports; maud + ports + the slice-04 `scoring` pure dep only; the `Shape`
  dispatch lives in the effect shell, not the pure core).
- **Adversarial review**: **APPROVED after one revision** — D1 (sparse-coverage: close the
  sparse-weight coverage gap) + D2 (document the read-only boundary) were both fixed in one
  pass (commit `7999ff4`). The cardinal sum-to-weight verified load-bearing (the
  observable-HTML reproduce-by-hand parser, DV-CS-5); the anti-merging confirmed structural
  (UNION-ALL, no merge JOIN, DV-CS-4); the no-recompute confirmed (the viewer projects the
  slice-04 score, DV-CS-3). Zero Testing Theater.
- **DES integrity**: PASS — all 13 steps have complete DES traces (13/13).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the `ScoreState`
projection + the `render_score_*` parity renderers + the inherited slice-06/07 render
arithmetic). The slice-04/05 cross-package lesson stays applied — the 71 `viewer-domain`
unit/property tests pin the production functions IN/against the crate, so the per-feature
mutation measurement reaches the real killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (`ScoreState` projection + `render_score_*` renderers, in-diff) | 13 | 13 | 0 | **100%** (13/13 in-diff viable) |

Slice-09 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0
missed). `adapter-http-viewer` + `adapter-duckdb` are NOT mutated by design (effect shell;
covered by the C-INV gold tests through the real binary); `scoring` is REUSED (already
mutation-covered at slice-04). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **Multi-posture peer-seeding must happen in ONE pull (carry-forward)**: seeding a rich /
  sparse / conflicting contributor mix for the `/score` tests revealed that **separate
  `peer pull`s drop earlier peers' PDS → 404** — each pull resolves only the peers it knows
  about, so a second pull leaves the first peer's PDS unreachable. The fix seeds ALL
  postures in ONE `seed_own_plus_peer_graph` / single pull
  (`seed_contributor_rich_sparse_and_conflicting`). **Institutional lesson: when a test
  fixture needs multiple peers in the store, seed them in a SINGLE pull — incremental
  per-peer pulls drop the PDS of peers seeded earlier, surfacing as a 404 at score time,
  not at seed time.**
- **Assert the cardinal against the OBSERVABLE HTML, not a re-call of the scorer
  (DV-CS-5)**: the sum-to-weight cardinal (J-002c) is checked by an observable-HTML
  reproduce-by-hand parser that pulls the breakdown rows out of the rendered HTML and
  re-adds the subtotals, asserting they equal the displayed weight. **Lesson: when a
  transparency guarantee is "the breakdown adds up to the headline number," verify it by
  parsing the OBSERVABLE output and re-deriving the number — re-calling the producer tests
  the producer, not the render the reader actually sees.**
- **The contributor view is author-scoped — co-claimants live on the object dimension
  (DV-CS-6)**: `/score` is prefix-matched to the author, so genuinely-unrelated
  co-claimants surface via `graph_query --object`, not `/score`. **Lesson: keep a view's
  contract honest about its SCOPE — a contributor view answers "how did THIS author earn
  their weight," not "who else touched this claim"; the latter is a different dimension,
  and conflating them would silently widen the view's promise.**
- **Render is confirmatory off a thick walking skeleton**: the C-1 walking skeleton shipped
  page = chrome + fragment, so the no-JS + parity work was structural from step one and
  most later steps (01-02/01-03/02-01/03-02/03-03) were confirmatory; the gold invariants
  (04-xx) flipped green for free. **Lesson: a thick walking skeleton that gets the page =
  chrome + fragment structure right on day one turns most of the remaining render steps
  into confirmation, concentrating the real work into the few seams that carry new behavior
  (the HTML sum-parser, the anti-merging seam, the sparse honesty render).**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-039/040/041 fixed the contracts; field-level shaping (`ScoreState` arms, the `WeightedView` shape, the read-seam query) left to DELIVER. | All adopted; the `ScoreState` arms (`Form`/`Scored{WeightedView}`/`NoClaims{contributor}`), the `query_contributor_scoring_feed` UNION-ALL, and the `render_score_*` renderers materialized at DELIVER against the render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | DESIGN fixed the breakdown render intent (anti-opaque, sum-to-weight by projection). | The observable-HTML reproduce-by-hand sum-parser (`assert_score_html_breakdown_sums_to_displayed_weight`) materialized at DELIVER to verify the cardinal against the rendered output (DV-CS-5). | Resolved at DELIVER. |
| 3 | The `xtask check-arch` rule edits (the allowlist edge + the capability-rule scope) — ADR-041 fixed the intent. | The `viewer-domain → scoring` allowlist edge + the read-only capability rule landed; `check-arch` reports 21 members. | Resolved at DELIVER. |
| 4 | Review expected to pass clean. | Review APPROVED after ONE revision (D1 sparse-coverage + D2 read-only-doc), fixed in one pass (`7999ff4`). | Found + fixed within DELIVER. |
| 5 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-CS-2, 100% in-diff, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-contributor-scoring/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, slices), `slices/`,
  `deliver/` (roadmap.json, execution-log.json).
- **Parent slice-04 archive** (the weighted-pairing scorer this view renders):
  `docs/evolution/openlore-scoring-graph-evolution.md`
- **Parent slice-06 archive** (the read-only viewer this slice extends):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Slice-09 ADRs**:
  `docs/adrs/ADR-039-viewer-local-contributor-scoring-read-seam-pure-scorer-reuse.md`,
  `docs/adrs/ADR-040-score-state-adt-viewer-domain-projection-transparent-breakdown.md`,
  `docs/adrs/ADR-041-score-route-get-form-params-nav-arch-enforcement.md`
- **Architecture design / component boundaries / C4 / data-flow** (kept in the feature
  workspace): the DESIGN sections of `docs/feature/viewer-contributor-scoring/feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-contributor-scoring/deliver/execution-log.json`,
  `docs/feature/viewer-contributor-scoring/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_contributor_scoring.rs` (14 C-scenarios),
  `tests/acceptance/viewer_contributor_scoring_invariants.rs` (5 gold C-INV-scenarios)
- **Reused scorer**: `crates/scoring` (`score` + `WeightedPairing`)
- **Extended viewer crates**: `crates/viewer-domain` (`ScoreState` + `render_score_*`),
  `crates/adapter-http-viewer` (`GET /score` handler + `Shape` fork + nav link),
  `crates/adapter-duckdb` (the read-only `query_contributor_scoring_feed` impl),
  `crates/ports` (the contributor-scoring read seam)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
