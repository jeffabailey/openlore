<!-- markdownlint-disable MD024 -->
# User Stories: viewer-counter-flags-score-surface (slice-14)

> Combined file (one section per story). Brownfield DELTA on slices 09/11/12/13.
> Every non-`@infrastructure` story traces to **J-003b** (`docs/product/jobs.yaml`).
> The shared counter-presence read (`counter_presence_for(&[cid]) -> HashSet<String>`,
> slice-12 / ADR-048) and the flag render (`render_countered_link(cid, is_countered)`,
> slice-13-unified SSOT) are **REUSED verbatim — NO new read method, NO new render fn**.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only**: the `/score` surface holds `StoreReadPort` only — no mutation method,
  no signing key in the viewer process, no write/sign/counter control. Authoring stays
  EXCLUSIVELY in the CLI (`claim counter`). Enforced 3 layers (type: the read port has no
  mutation method + xtask check-arch viewer capability rule + behavioral gold).
  [KPI-VIEW-2, slice-06–13]
- **C-2 Shown, never applied**: the flag is a NEUTRAL presence marker ("Countered"). The
  flagged contribution row renders VERBATIM — its confidence, author/triangulation bonuses,
  subtotal, the pairing weight, the bucket, the rank, and its position are byte-identical to
  a no-flag render. The flag NEVER re-orders / re-ranks / filters / re-weights / subtracts on
  the breakdown, and changes NO data. [ADR-015, slice-11 I-CT-2, slice-12 I-LF-2, slice-13 I-CF-2]
- **C-3 Presence-only, no invented / no merged flag**: a contribution is flagged ONLY if a
  real counter referencing its CID exists in the store (own `claims` ∪ local `peer_claims`).
  The presence read fabricates NO flag and merges NO claims. The flag is a boolean per
  contribution CID (presence), NOT a count and NEVER a "disputed by N" aggregate. Per-counter
  attribution is deferred to the slice-11 thread the flag links to.
  [KPI-FED-1/2, KPI-AV-2, KPI-GRAPH-2, slice-12 I-LF-3, slice-13 I-CF-3]
- **C-4 Verbatim confidence / weight / bonuses / subtotal**: every confidence renders as
  `0.90` (never `0.9` / `90%`); every author bonus, triangulation bonus, subtotal, headline
  pairing weight, and bucket renders exactly as slice-09, via the single existing render site
  (`render_score_breakdown` / `render_score_pairing`) — UNCHANGED by the flag.
  [KPI-4, WD-CS-7, slice-09/10/12/13]
- **C-5 LOCAL-only / offline**: the presence read is LOCAL (the indexed
  `claim_references ∪ peer_claim_references` tables; NO per-row artifact read — the flag
  carries no reason text). NO network seam on the `/score` route (already a fully-offline LOCAL
  read + pure compute, slice-09 WD-CS-8). The `/score` page renders fully with the network down
  and references only the vendored local `/static/htmx.min.js` (no CDN).
  [KPI-5, KPI-VIEW-5, KPI-HX-G2, slice-09 WD-CS-8]
- **C-6 Progressive enhancement + parity**: an `HX-Request` returns the score fragment (with
  flags); a no-JS / bookmark / direct-URL request returns the full page = chrome + the SAME
  fragment. The flag lives in the SAME fragment fn (`render_score_results_fragment`) both shapes
  embed, so it renders identically in both. A swap is a nicety, never a requirement.
  [slice-07 KPI-HX-G1/G2/G3, slice-09 WD-CS-9, slice-13]
- **C-7 No new crates, NO new read method, NO new render fn, NO new route**: extend the PURE
  `viewer-domain` + EFFECT `adapter-http-viewer` + `xtask`; REUSE the slice-12
  `counter_presence_for` read + the slice-13 `render_countered_link` SSOT + the
  `COUNTERED_PRESENCE_FLAG` constant. Workspace stays 21 members. Functional paradigm (ADR-007).
  [slice-06..13 precedent]
- **C-8 Batch presence read, NOT N+1 — REUSED, no new read**: the `/score` handler collects its
  page's contribution CID set (every `Contribution.cid` across every `WeightedPairing`, flattened
  once) and calls the slice-12 `counter_presence_for(&[cid])` ONCE (ONE aggregate query per
  render), then the pure projection maps the returned set onto contribution rows. NO new read
  method; NO per-surface SQL. [slice-12 ADR-048 / slice-13 I-CF-8]
- **C-9 Sum-to-weight preserved + score-orthogonal (the slice-14 CARDINAL)**: adding the flag
  changes NO displayed weight, confidence, bonus, subtotal, headline total, bucket, ranking, or
  row order. The slice-09 per-claim subtotals STILL sum to the displayed pairing weight (both
  project the SAME unchanged `WeightedPairing`). The counter is SHOWN, never APPLIED/subtracted —
  **a countered claim contributes its FULL original weight** to the contributor's score; the
  scoring math is intentionally counter-agnostic. The flag's COPY makes this orthogonality
  unmistakable (a reader must not misread it as a score deduction). Byte-identical to the slice-09
  render with markers elided. [slice-09 sum-to-weight CARDINAL (WD-CS-4/6/7) + ADR-015
  shown-never-applied; this is the load-bearing distinction from slices 12/13]

---

## US-CF-001: Reuse the slice-12 batch counter-presence read in the `/score` handler (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-CF-001 adds NO new read method. It WIRES the existing slice-12
`StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` (ADR-048) into the `/score`
page handler (`score_page` / `resolve_score_state`) in `adapter-http-viewer`: after building the
`ScoreState::Scored { view: scoring::WeightedView }` from the existing
`query_contributor_scoring_feed` read, the handler collects the page's contribution CID set (every
`Contribution.cid` across every `WeightedPairing` in the view) and calls `counter_presence_for`
ONCE, then passes the returned presence set into the pure pairing-row projection. It produces no
user-visible output on its own (the rendered flag is US-CF-002), so it enables a user decision only
THROUGH that story. The slice contains ONE non-infrastructure, user-visible story (US-CF-002), so
the slice has release value (Dimension-0 slice-level check passes).

### Problem

The slice-12 batch presence read exists and is proven on `/claims` (slice-12) and the graph
surfaces (slice-13), but the `/score` handler does not yet collect its page's contribution CID set
nor call it. Without the wiring, the scoring surface cannot show the flag, and a naive per-row call
would reintroduce the N+1 the slice-12 read was built to avoid — across a breakdown that can have
many contributions per pairing and many pairings.

### Who

- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the scanner-hat
  story (US-CF-002) consumes on the scoring surface.

### Solution

Wire `counter_presence_for` into the `/score` handler. The handler: (1) builds the
`ScoreState::Scored { view }` as today (UNCHANGED `query_contributor_scoring_feed` read + UNCHANGED
slice-04 pure scoring), (2) collects the page's contribution CID set — every `Contribution.cid`
across every `WeightedPairing` in the view, flattened into ONE slice, (3) calls
`counter_presence_for(&cids)` ONCE, (4) passes the presence set into the pure projection. The
`ScoreState::Form` and `ScoreState::NoClaims` arms have no contributions, so they issue NO presence
query. NO new read method; NO new SQL; NO change to the scoring math, the feed read, or the ranking.

### Domain Examples

#### 1: Happy path — a `/score` page CID collection across pairings
Maria opens `GET /score?contributor=did:plc:t0bi`. Tobias's score renders 3 `WeightedPairing`s
(cargo→dependency-pinning, tokio→async-first, serde→zero-copy) with 4, 2, and 3 contributions = 9
contributions total. The handler collects those 9 contribution CIDs into one slice, calls
`counter_presence_for(&[9 cids])` ONCE, and gets back the subset (say 2 CIDs) that have ≥1 counter.
ONE aggregate query for the whole breakdown. The scoring math, the ranking, and the displayed
weights are UNCHANGED.

#### 2: Edge case — contributions reused across pairings, deduped into one call
Maria opens `/score?contributor=did:plc:maria`. The same contribution CID `bafy...mr1` appears in
two pairings (the claim spans two philosophies). The handler flattens all contribution CIDs across
all pairings into ONE slice (the read tolerates duplicate CIDs; the returned set is by CID), calls
`counter_presence_for` ONCE (not once per pairing, not once per contribution), and threads the
presence set back. The breakdown order, grouping, and every subtotal are UNCHANGED.

#### 3: Boundary — empty / `NoClaims` / all-un-countered score
Maria opens `/score?contributor=did:plc:nobody` on a store where that DID has no claims → the
`ScoreState::NoClaims` arm renders the slice-09 plain-language notice and issues NO presence query.
Separately, a contributor WITH claims but on a store with NO counters at all: the handler collects
the contribution CIDs, calls `counter_presence_for`, which returns an EMPTY set (slice-12
short-circuits an empty input to no-query); the projection flags nothing; the breakdown renders
exactly as slice-09.

### UAT Scenarios (BDD)
> Each scenario names its DRIVING ROUTE (port-to-port via the real `openlore ui` subprocess).
> No scenario calls `counter_presence_for` or `viewer-domain` directly.

#### Scenario: A score breakdown resolves counter presence in one aggregate query
Given Maria's `/score?contributor=did:plc:t0bi` renders 3 pairings totaling 9 contributions and 2 of them are countered
When she opens `GET /score?contributor=did:plc:t0bi` in the `openlore ui` viewer
Then all 9 contribution CIDs are resolved against counters in exactly ONE aggregate query (not one per pairing, not one per contribution)
And the returned presence set contains exactly the 2 countered CIDs
And the displayed weights, ranking, and breakdown order are byte-identical to slice-09

#### Scenario: A contributor with no claims issues no presence query
Given the DID `did:plc:nobody` has no claims in Maria's store
When she opens `GET /score?contributor=did:plc:nobody`
Then the `NoClaims` notice renders and `counter_presence_for` is not called
And no contribution is flagged

#### Scenario: An un-countered score resolves to an empty presence set with no query
Given Maria's store has no counter claims at all
When she opens `GET /score?contributor=did:plc:maria`
Then `counter_presence_for` returns an empty set without preparing a query
And no contribution row is flagged, and the breakdown renders exactly as slice-09

### Acceptance Criteria
- [ ] The `/score` handler collects its page contribution CID set (every `Contribution.cid` across every `WeightedPairing`) and calls `counter_presence_for` exactly ONCE per render (the slice-12 method, REUSED — no new read method added)
- [ ] The contribution CIDs are flattened across ALL pairings into ONE call (not per-pairing, not per-contribution); duplicate CIDs across pairings collapse to the by-CID presence set
- [ ] The query count is invariant to contribution / pairing count (the N+1 guard, inherited from slice-12 ADR-048)
- [ ] The `Form` and `NoClaims` arms issue NO presence query; an all-un-countered score resolves to an empty presence set with no query
- [ ] The existing `query_contributor_scoring_feed` read, the slice-04 pure scoring math, the `WeightedView`, the ranking, and the displayed weights/subtotals are UNCHANGED
- [ ] No new method is added to `StoreReadPort`; no new SQL is written; no new route is added

### Outcome KPIs
- **Who**: the viewer process serving `GET /score?contributor=<did>`
- **Does what**: resolves counter presence for a whole score breakdown in one aggregate query
- **By how much**: exactly 1 `counter_presence_for` call per render (or 0 for `Form`/`NoClaims`/empty), invariant to contribution/pairing count (0 N+1)
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess + the inherited slice-12 adapter-duckdb N+1 property test
- **Baseline**: today the `/score` handler issues 0 presence queries (no flag); slice-14 adds exactly 1, never N

### Technical Notes
- REUSES `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` (slice-12 / ADR-048) verbatim — confirmed present in `crates/ports/src/store_read.rs`. NO new read method.
- Depends on the slice-12 read being shipped (it is — slices 12/13 SHIPPED).
- The contribution CID set is collected from `Contribution.cid` (`crates/scoring/src/explain.rs`) across `WeightedPairing.contributions()` for every pairing in `ScoreState::Scored { view }`.
- The handler seam is `score_page` / `resolve_score_state` (`crates/adapter-http-viewer/src/lib.rs` ~lines 489/507); the presence set is set in the EFFECT shell (keeping the pure render total), mirroring the slice-12/13 `from_row_with_presence` pattern.

---

## US-CF-002: See a "Countered" flag on each `/score` contribution row whose claim has ≥1 counter, orthogonal to the score

`job_id: J-003b`

### Problem

Maria reads a contributor's `/score` breakdown to decide whether to trust their adherence score
before, say, weighing their projects in a decision. Each contribution row is one signed claim
(author DID + CID + verbatim confidence + bonuses + subtotal). Today, when a contribution's
underlying claim has been countered, the breakdown gives no sign of it — she would have to copy
each contribution's CID and open `/claims/{cid}` one-by-one to discover the disagreement. She
cannot tell, while reading the score, which contributions are contested — and the score itself is
intentionally counter-agnostic (a counter does not lower the weight), so without an in-context
marker she has no way to know where to apply her own judgment.

### Who

- P-001 (the viewer operator, "Maria"), counter-claim-scanner hat | reading a contributor's
  `/score` breakdown | wants to see which of the contributor's contributions are contested before
  trusting the score, WITHOUT the flag changing any weight/subtotal/rank/order and WITHOUT
  misreading the flag as a score deduction.

### Solution

On `/score`, each contribution row whose `cid` is in the page's presence set renders the neutral
"Countered" marker via the slice-13 `render_countered_link(cid, is_countered)` SSOT (REUSED
verbatim) — a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link to that claim's
slice-11 thread. Un-countered contributions render exactly as slice-09 (no marker). The author DID,
the verbatim confidence, both bonuses, the subtotal, the pairing weight, the bucket, the ranking,
and the row order are all UNCHANGED — the per-claim subtotals still sum to the displayed pairing
weight (slice-09 CARDINAL). The marker is accompanied by plain-language copy (a one-time legend /
caption on the breakdown) that makes the meaning unmistakable: *this contribution's claim has been
disagreed with elsewhere — the counter is shown for you to judge, and does NOT lower this
contributor's score.*

### Elevator Pitch
- **Before**: Maria reads a contributor's `/score` breakdown and cannot tell which of their contributions have been countered without opening each `/claims/{cid}` thread one-by-one — and the score is intentionally counter-agnostic, so she has no in-context signal of where disagreement exists before trusting it.
- **After**: open `http://127.0.0.1:<port>/score?contributor=did:plc:t0bi` → each contribution row whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered rows show nothing; every weight, confidence, bonus, subtotal, the headline total, the ranking, and the row order are byte-identical to slice-09 (the subtotals still sum to the weight); the marker's copy makes it unmistakable that being countered does NOT lower the score.
- **Decision enabled**: Maria decides WHICH of a contributor's contributions to scrutinize (open the disagreement on) before trusting their adherence score — without misreading the flag as a score deduction and without the breakdown silently re-ranking or re-weighting for her.

### Domain Examples

#### 1: Happy path — a countered contribution is flagged, weight unchanged
Maria opens `/score?contributor=did:plc:t0bi`. Under the cargo→dependency-pinning pairing
(weight `1.42 [well-evidenced]`), Tobias's contribution `bafy...t0bi` (confidence `0.88`, author
bonus `0.10`, triangulation bonus `0.05`, subtotal `1.03`) shows the "Countered" marker linking to
`/claims/bafy...t0bi`. Maria authored a counter against that claim earlier. The contribution still
shows confidence `0.88`, the same bonuses, the same subtotal `1.03`, and the pairing weight is
still `1.42` — the three contributions in the pairing still sum to `1.42`. The counter changed
nothing about the score; it is shown so Maria can go judge it.

#### 2: Edge case — an un-countered contribution shows nothing; sum-to-weight intact
In the same breakdown, Maria's own contribution `bafy...mr1` (confidence `0.91`, subtotal `0.39`)
has no counter. Its row renders exactly as slice-09 — no marker, no "0 counters" noise. The pairing
weight `1.42` is unchanged and the subtotals (`1.03 + 0.39 + ...`) still sum to it.

#### 3: Boundary — a contribution countered by two authors shows ONE marker; ranking unchanged
Tobias's contribution `bafy...dup` (under a different pairing) is countered by both Maria's CLI
counter and Rachel's peer counter. Its `/score` row shows ONE neutral "Countered" marker
(presence-only via the slice-12 `DISTINCT` read), never "disputed by 2". Its subtotal, the pairing
weight, and the pairing's rank in the breakdown are all unchanged; the two distinct counters are
attributed in the slice-11 thread the marker links to.

### UAT Scenarios (BDD)
> Driving route: `GET /score?contributor=<did>` (the real `openlore ui` subprocess), both shapes.

#### Scenario: A countered contribution shows the neutral marker and its weight is unchanged
Given Maria's `/score?contributor=did:plc:t0bi` breakdown has, under the cargo→dependency-pinning pairing (weight 1.42), a contribution `bafy...t0bi` (confidence 0.88, subtotal 1.03) that has ≥1 counter
When she opens `GET /score?contributor=did:plc:t0bi` in the viewer
Then that contribution row shows the neutral "Countered" marker
And the marker is a render-only `<a href="/claims/bafy...t0bi">` one-hop link to that claim's slice-11 thread
And the row still shows confidence 0.88, the same bonuses, and the same subtotal 1.03
And the pairing weight is still 1.42 and the contribution subtotals still sum to 1.42

#### Scenario: The flag is orthogonal to the score — countered and un-countered contributions carry identical weight
Given two contributions in the same pairing have identical confidence and bonuses, but one is countered and one is not
When Maria opens `GET /score?contributor=<did>`
Then both contributions show the identical subtotal (the counter subtracts nothing)
And only the countered one shows the "Countered" marker
And the breakdown copy states that a counter is shown for the reader to judge and does NOT lower the contributor's score

#### Scenario: Adding the flag changes no weight, ranking, or row order versus the slice-09 baseline
Given Maria's `/score?contributor=did:plc:t0bi` renders 3 pairings, 2 of whose contributions are countered
When she opens `GET /score?contributor=did:plc:t0bi`
Then exactly the 2 countered contributions show the marker and every other contribution renders exactly as slice-09
And every displayed weight, confidence, bonus, subtotal, headline total, bucket, the pairing ranking, and the row order are byte-identical to the slice-09 render with the markers elided

#### Scenario: The score flag renders identically under htmx and no-JS, and a contribution countered twice shows one marker
Given Maria's `/score` breakdown has a contribution `bafy...dup` countered by two distinct authors
When she requests `GET /score?contributor=<did>` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the score fragment with the flag and the no-JS response is the full page = chrome + the SAME fragment, with the flag rendered identically
And the `bafy...dup` contribution shows exactly ONE neutral "Countered" marker (never "disputed by 2") with its subtotal, weight, and rank unchanged

### Acceptance Criteria
- [ ] A contribution row whose CID is in the presence set shows the `COUNTERED_PRESENCE_FLAG` ("Countered") marker via the REUSED slice-13 `render_countered_link(cid, is_countered)` (no new render fn, no new string)
- [ ] The marker is a render-only `<a href="/claims/{cid}">` one-hop link to that claim's slice-11 thread
- [ ] **(Sum-to-weight CARDINAL)** With the flag present, the per-claim subtotals STILL sum to the displayed pairing weight, exactly as slice-09 — verified on a flagged breakdown
- [ ] **(Shown-never-applied / byte-identity)** Every displayed weight, confidence, author bonus, triangulation bonus, subtotal, headline total, bucket, the pairing ranking, and the contribution row order are byte-identical to the slice-09 render with the markers elided — a countered claim contributes its FULL original weight
- [ ] **(Anti-misread copy)** The breakdown carries plain-language copy making it unmistakable that the "Countered" marker is orthogonal to the score — it signals "this claim has been disagreed with elsewhere; shown for you to judge" and does NOT mean the counter lowered the contributor's weight/score; the copy never implies subtraction/deduction/penalty
- [ ] The flag renders identically under the htmx fragment and the no-JS full page (parity by construction — same fragment fn)
- [ ] The flag is NEUTRAL presence text, never a verdict ("disputed"/"refuted"/"false") and never a count
- [ ] A contribution countered by N authors shows exactly ONE marker (presence-only)
- [ ] An un-countered contribution renders exactly as slice-09 (no marker, no noise)

### Outcome KPIs
- **Who**: P-001 dogfood operators reading a contributor's `/score` breakdown
- **Does what**: opens a contested contribution's thread directly from the score-breakdown flag (instead of blind drill-in), and correctly understands the flag as orthogonal to the score
- **By how much**: leading indicator OF KPI-FED-3 — a measurable share navigate score-flag → thread; guardrail: 0 cases of a countered contribution's displayed weight/subtotal differing from its un-countered equivalent, and 0 sum-to-weight regressions
- **Measured by**: per-feature GREEN (the flag renders for countered contributions; the sum-to-weight + byte-identity gold proves the score is unchanged); comprehension via the anti-misread copy AC + dogfood feedback; cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today the `/score` breakdown shows no counter indication; discovering a countered contribution requires copying its CID and opening the thread, and there is no in-context cue that disagreement is orthogonal to the score

### Technical Notes
- Threads the page presence set into the contribution-row render (`render_score_breakdown`, `crates/viewer-domain/src/lib.rs` ~line 1968); the pure render stays a TOTAL function of `(ScoreState, presence)` — mirror the slice-12/13 `from_row_with_presence` projection seam. DESIGN owns whether the presence is passed alongside the `ScoreState` into the pure render or pre-applied into a flagged contribution view-model.
- The marker render REUSES the slice-13 `render_countered_link` SSOT + the `COUNTERED_PRESENCE_FLAG` constant (`crates/viewer-domain/src/lib.rs` ~line 679) — single source of truth for the flag string and the `<a href="/claims/{cid}">` one-hop shape.
- The sum-to-weight invariant is preserved BY CONSTRUCTION because the subtotals + the headline weight both project the SAME unchanged `WeightedPairing` (slice-09 doc-comment ~line 1935); slice-14 adds only a render-only annotation and changes no `WeightedPairing`. The byte-identity no-regression follows the slice-12/13 baseline+marker-elision tactic (record the slice-09 score render, elide the new markers + the anti-misread legend, compare).
- The anti-misread copy is a SHORT, NEUTRAL legend on the breakdown (DESIGN owns exact wording within the AC); it must not use "disputed"/"refuted"/"penalty"/"deduction"/"lowered".

---

## Out of scope (explicit — restated from feature-delta)

- **Applying / subtracting / re-weighting by the counter** — the counter is SHOWN, never APPLIED;
  a countered claim keeps its FULL original weight (C-2 / C-9).
- **Recomputing any scoring math** — the viewer PROJECTS the reused `scoring::WeightedView`
  (slice-04/09 WD-CS-6); no viewer scoring logic.
- **The slices 12/13 surfaces** (`/claims`, `/peer-claims`, `/project`, `/philosophy`), the
  slice-11 `/claims/{cid}` thread, the slice-08 `/search` annotation — all shipped, not re-touched.
- Authoring/composing a counter on the viewer; re-rank/filter/re-weight/subtract the breakdown;
  any count / "disputed by N" / verdict / "disputed score" on a flag; any reason text on a flag;
  any network seam on the `/score` route; any N+1 (one batch query per render); any new read
  method / render fn / route / crate.
