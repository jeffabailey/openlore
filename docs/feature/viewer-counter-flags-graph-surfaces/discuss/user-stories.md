<!-- markdownlint-disable MD024 -->
# User Stories: viewer-counter-flags-graph-surfaces (slice-13)

> Combined file (one section per story). Brownfield DELTA on slices 06/07/09/10/11/12.
> Every non-`@infrastructure` story traces to **J-003b** (`docs/product/jobs.yaml`).
> The shared counter-presence read (`counter_presence_for(&[cid]) -> HashSet<String>`,
> slice-12 / ADR-048) is **REUSED verbatim — NO new read method**.

## System Constraints (cross-cutting — apply to every story)

These are RESTATED as binding commitments (inherited, not re-litigated). Each story's
AC inherits them; they are not repeated per-story.

- **C-1 Read-only**: every flagged surface holds `StoreReadPort` only — no mutation
  method, no signing key in the viewer process, no write/sign/counter control on any
  rendered surface. Authoring stays EXCLUSIVELY in the CLI (`claim counter`). Enforced
  3 layers (type: the read port has no mutation method + xtask check-arch viewer
  capability rule + behavioral gold). [KPI-VIEW-2, slice-06/07/09/10/11/12]
- **C-2 Shown, never applied**: the flag is a NEUTRAL presence marker ("Countered").
  The flagged row/edge renders VERBATIM — its confidence, weight, bucket, rank, group,
  and position are byte-identical to a no-flag render. The flag NEVER
  re-orders / re-ranks / filters / re-weights / re-groups / re-paginates any surface,
  and changes NO data. [ADR-015, slice-08 I-NS-3, slice-11 I-CT-2, slice-12 I-LF-2]
- **C-3 Presence-only, no invented / no merged flag**: a row/edge is flagged ONLY if a
  real counter referencing its CID exists in the store (own `claims` ∪ local
  `peer_claims`). The presence read fabricates NO flag and merges NO claims. The flag
  is a boolean per row/edge (presence), NOT a count and NEVER a "disputed by N"
  aggregate. Per-counter attribution is deferred to the slice-11 thread the flag links
  to. [KPI-FED-1/2, KPI-AV-2, KPI-GRAPH-2, slice-12 I-LF-3]
- **C-4 Verbatim confidence / weight**: every confidence renders as `0.90` (never
  `0.9` / `90%`) and every weight/bucket renders exactly as today, via the single
  existing render site — UNCHANGED by the flag. [KPI-4, FR-VIEW-8, slice-09/10/12]
- **C-5 LOCAL-only / offline**: the presence read is LOCAL (the indexed
  `claim_references ∪ peer_claim_references` tables; NO per-row artifact read — the
  flag carries no reason text). NO network seam on any of these routes. Each flagged
  page renders fully with the network down and references only the vendored local
  `/static/htmx.min.js` (no CDN). [KPI-5, KPI-VIEW-5, KPI-HX-G2, slice-09/10/12]
- **C-6 Progressive enhancement + parity**: an `HX-Request` returns the surface's
  fragment (with flags); a no-JS / bookmark / direct-URL request returns the full page
  = chrome + the SAME fragment. The flag lives in the SAME fragment fn both shapes
  embed, so it renders identically in both. A swap is a nicety, never a requirement.
  [slice-07 KPI-HX-G1/G2/G3, slice-09/10/12]
- **C-7 No new crates**: extend the PURE `viewer-domain` + EFFECT `adapter-http-viewer`
  + `ports` (re-export only — NO new method) + `cli` + `xtask`. Workspace stays 21
  members. Functional paradigm (ADR-007). [slice-06..12 precedent]
- **C-8 Batch presence read, NOT N+1 — REUSED, no new read**: each surface collects
  its page's CID set and calls the slice-12 `counter_presence_for(&[cid])` ONCE
  (ONE aggregate query per render), then the pure projection maps the returned set
  onto rows/edges. NO new read method; NO per-surface SQL. [slice-12 ADR-048 / I-LF-8]

---

## US-CF-001: Reuse the slice-12 batch counter-presence read across the graph surfaces (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale
US-CF-001 adds NO new read method. It WIRES the existing slice-12
`StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` (ADR-048) into the
`/peer-claims`, `/project`, and `/philosophy` page handlers in `adapter-http-viewer`:
each handler collects the CID set for its page (peer-claim row CIDs; edge CIDs across
all `EdgeGroup`s) and calls `counter_presence_for` ONCE, then passes the returned
presence set into the pure `viewer-domain` projection. It produces no user-visible
output on its own (the rendered flag is US-CF-002/003), so it enables a user decision
only THROUGH those stories. The slice contains TWO non-infrastructure, user-visible
stories (US-CF-002, US-CF-003), so the slice has release value (Dimension-0 slice-level
check passes).

### Problem
The slice-12 batch presence read exists and is proven on `/claims`, but the
`/peer-claims` and `/project` + `/philosophy` handlers do not yet collect their page's
CID set nor call it. Without the wiring, the graph surfaces cannot show the flag, and a
naive per-row call would reintroduce the N+1 the slice-12 read was built to avoid.

### Who
- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the
  scanner-hat stories (US-CF-002/003) consume.

### Solution
Wire `counter_presence_for` into the three page handlers. Each handler: (1) pages its
rows/edges as today (UNCHANGED read), (2) collects the page's CID set (peer-claim CIDs;
edge CIDs flattened across `EdgeGroup`s), (3) calls `counter_presence_for(&cids)` ONCE,
(4) passes the presence set into the pure projection. For the edge surfaces the CID set
is the UNION of every edge's `cid` across all groups on the page — collected once,
queried once. NO new read method; NO new SQL; NO per-surface query shape.

### Domain Examples
#### 1: Happy path — `/peer-claims` page CID collection
Maria's `/peer-claims` page renders 12 peer-claim rows. The handler collects those 12
CIDs into one slice, calls `counter_presence_for(&[12 cids])` ONCE, and gets back the
subset (say 2 CIDs) that have ≥1 counter. ONE aggregate query for the whole page.

#### 2: Edge case — `/project` edges spanning multiple groups
Maria opens `/project?subject=github:rust-lang/cargo`. The traversal renders 3
`EdgeGroup`s (3 philosophies) with 4, 2, and 5 edges = 11 edges total. The handler
flattens all 11 edge CIDs into ONE slice, calls `counter_presence_for` ONCE (not once
per group, not once per edge), and gets back the countered subset. Edge order and group
order are UNCHANGED.

#### 3: Boundary — empty / all-un-countered page
Maria opens `/philosophy?object=org.openlore.philosophy.memory-safety` on a store with
no counters at all. The handler collects the edge CIDs, calls `counter_presence_for`,
which returns an EMPTY set (slice-12 short-circuits an empty input to no-query). The
projection flags nothing; the surface renders exactly as slice-10.

### UAT Scenarios (BDD)
> Each scenario names its DRIVING ROUTE (port-to-port via the real `openlore ui`
> subprocess). No scenario calls `counter_presence_for` or `viewer-domain` directly.

#### Scenario: A peer-claims page resolves counter presence in one aggregate query
Given Maria's local store has 12 peer claims on a `/peer-claims` page and 2 of them are countered
When she opens `GET /peer-claims` in the `openlore ui` viewer
Then the page's 12 CIDs are resolved against counters in exactly ONE aggregate query
And the returned presence set contains exactly the 2 countered CIDs

#### Scenario: A traversal page resolves edge presence once across all groups
Given Maria's `/project?subject=github:rust-lang/cargo` survey renders 3 edge groups totaling 11 edges
When she opens `GET /project?subject=github:rust-lang/cargo`
Then all 11 edge CIDs are resolved in exactly ONE aggregate query (not one per group, not one per edge)
And the edge order and group order are byte-identical to slice-10

#### Scenario: An un-countered page resolves to an empty presence set with no query
Given Maria's store has no counter claims at all
When she opens `GET /philosophy?object=org.openlore.philosophy.memory-safety`
Then `counter_presence_for` returns an empty set without preparing a query
And no edge is flagged

### Acceptance Criteria
- [ ] Each handler (`/peer-claims`, `/project`, `/philosophy`) collects its page CID set and calls `counter_presence_for` exactly ONCE per render (the slice-12 method, REUSED — no new read method added)
- [ ] The edge surfaces flatten all edge CIDs across every `EdgeGroup` into ONE call (not per-group, not per-edge)
- [ ] The query count is invariant to page size / edge count / group count (the N+1 guard, inherited from slice-12 ADR-048)
- [ ] An empty / all-un-countered page resolves to an empty presence set with no query
- [ ] The existing reads (`list_peer_claims`, `query_project_survey`, `query_philosophy_survey`) and their SQL / ordering / paging are UNCHANGED
- [ ] No new method is added to `StoreReadPort`; no new SQL is written

### Outcome KPIs
- **Who**: the viewer process serving `/peer-claims`, `/project`, `/philosophy`
- **Does what**: resolves counter presence for a whole page in one aggregate query
- **By how much**: exactly 1 `counter_presence_for` call per render, invariant to row/edge/group count (0 N+1)
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess + the inherited slice-12 adapter-duckdb N+1 property test
- **Baseline**: today these handlers issue 0 presence queries (no flag); slice-13 adds exactly 1, never N

### Technical Notes
- REUSES `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` (slice-12 / ADR-048) verbatim — confirmed present in `crates/ports/src/store_read.rs` (lines 360-384). NO new read method.
- Depends on the slice-12 read being shipped (it is — slice-12 SHIPPED, see CONTEXT.md).
- The edge CID set is collected from `EdgeRow.cid` (non-`Option`, `crates/viewer-domain/src/lib.rs` EdgeRow); the peer CID set from `PeerClaimRowView.cid`.

---

## US-CF-002: See a "Countered" flag on each `/peer-claims` row whose claim has ≥1 counter

`job_id: J-003b`

### Problem
Maria scans `/peer-claims` (the federated claims she pulled from peers) to decide which
peers' reasoning to engage with. Today she cannot tell which of those peer claims have
ALREADY drawn a counter — she must open each one's `/claims/{cid}` thread one-by-one to
find out. Disagreement is invisible while she scans the federated surface.

### Who
- P-001 (the viewer operator, "Maria"), counter-claim-scanner hat | scanning the
  federated `/peer-claims` surface | wants to triage which peer claims are contested
  before drilling in.

### Solution
On `/peer-claims`, each row whose `cid` is in the page's presence set renders a neutral
"Countered" marker (the slice-11/12 `COUNTERED_PRESENCE_FLAG`, REUSED verbatim) as a
render-only `<a href="/claims/{cid}">` one-hop link to that claim's slice-11 thread.
Un-countered rows render exactly as slice-06 (no marker). The peer-origin column, the
verbatim confidence, the CID cell, and the row order are all UNCHANGED.

### Elevator Pitch
- **Before**: Maria cannot tell which of the peer claims on her `/peer-claims` list have been countered without opening each `/claims/{cid}` detail page one-by-one.
- **After**: open `http://127.0.0.1:<port>/peer-claims` → each peer-claim row whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered rows show nothing; the peer-origin column and row order are unchanged.
- **Decision enabled**: Maria decides WHICH contested peer claim to open and read the disagreement on first — triaging her attention on the federated surface instead of blind-opening every peer claim.

### Domain Examples
#### 1: Happy path — a countered peer claim is flagged
Maria pulled Tobias's claim that `github:rust-lang/cargo` embodies `dependency-pinning`
(cid `bafy...t0bi`, confidence 0.88). She earlier authored a counter against it. On
`/peer-claims`, Tobias's row shows the "Countered" marker linking to
`/claims/bafy...t0bi`. The peer-origin cell still shows Tobias's DID + his PDS; the
confidence still shows `0.88`.

#### 2: Edge case — an un-countered peer claim shows nothing
Rachel's peer claim that `github:tokio-rs/tokio` embodies `async-first` (cid
`bafy...rach`) has no counter. Its `/peer-claims` row renders exactly as slice-06 — no
marker, no "0 counters" noise.

#### 3: Boundary — a peer claim countered by two authors shows ONE marker
Maria's store holds peer claim `bafy...dup` countered by both Tobias and her own CLI
counter. Its `/peer-claims` row shows ONE neutral "Countered" marker (presence-only via
the slice-12 `DISTINCT` read), never "disputed by 2". The two distinct counters are
attributed in the slice-11 thread the marker links to.

### UAT Scenarios (BDD)
> Driving route: `GET /peer-claims` (the real `openlore ui` subprocess), both shapes.

#### Scenario: A countered peer-claim row shows the neutral marker linking to its thread
Given Maria pulled Tobias's peer claim `bafy...t0bi` (cargo embodies dependency-pinning, confidence 0.88) and it has ≥1 counter
When she opens `GET /peer-claims` in the viewer
Then Tobias's row shows the neutral "Countered" marker
And the marker is a render-only `<a href="/claims/bafy...t0bi">` one-hop link to that claim's slice-11 thread
And the row still shows Tobias's peer origin (DID + PDS) and the verbatim confidence `0.88`

#### Scenario: The peer-claims flag renders identically under htmx and no-JS
Given Maria's `/peer-claims` page has one countered row
When she requests `GET /peer-claims` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the peer-claims view-panel fragment with the flag
And the no-JS response is the full page = chrome + the SAME fragment, with the flag rendered identically

#### Scenario: A peer claim countered by two authors shows one neutral presence marker
Given Maria's peer claim `bafy...dup` is countered by two distinct authors
When she opens `GET /peer-claims`
Then its row shows exactly ONE neutral "Countered" marker (never "disputed by 2")
And the marker links to `/claims/bafy...dup` where the two counters are individually attributed

### Acceptance Criteria
- [ ] A peer-claim row whose CID is in the presence set shows the `COUNTERED_PRESENCE_FLAG` ("Countered") marker (reused verbatim from slice-11/12)
- [ ] The marker is a render-only `<a href="/claims/{cid}">` one-hop link to that claim's slice-11 thread
- [ ] The flag renders identically under the htmx fragment and the no-JS full page (parity by construction — same fragment fn)
- [ ] The flag is NEUTRAL presence text, never a verdict ("disputed"/"refuted"/"false") and never a count
- [ ] The row's peer origin (author DID + PDS), confidence, and CID render UNCHANGED (verbatim) beside the flag
- [ ] A row countered by N authors shows exactly ONE marker (presence-only)

### Outcome KPIs
- **Who**: P-001 dogfood operators scanning `/peer-claims`
- **Does what**: opens a contested peer claim's thread directly from the federated list flag (instead of blind drill-in)
- **By how much**: leading indicator OF KPI-FED-3 — a measurable share navigate list-flag → thread on the peer surface
- **Measured by**: per-feature GREEN (the flag renders for countered peer rows); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today the `/peer-claims` list shows no counter indication; the only way to discover a countered peer claim is to open each one

### Technical Notes
- Extends `PeerClaimRowView` with an `is_countered: bool` set in the effect shell from the presence set (mirrors the slice-12 `ClaimRowView.from_row_with_presence` pattern). DESIGN owns whether it is a bool field vs a wrapper.
- The marker render REUSES the slice-12 `render_list_presence_flag` shape (the `<a href="/claims/{cid}">Countered</a>` arm) — single source of truth for the flag string.
- The `render_peer_claim_row` site (`crates/viewer-domain/src/lib.rs` ~line 1059) gains the flag arm.

---

## US-CF-003: See a "Countered" flag on each `/project` + `/philosophy` traversal EDGE whose claim has ≥1 counter, without re-ordering or re-grouping the survey

`job_id: J-003b`

### Problem
Maria traverses `/project` and `/philosophy` to find which contributors span the
projects/philosophies she cares about (the J-002b "aha"). Each edge is one signed
claim. Today, when an edge's underlying claim has been countered, the survey gives no
sign of it — she'd have to copy each edge's CID and open `/claims/{cid}` to discover the
disagreement. She cannot tell, while traversing, which edges are contested.

### Who
- P-001 (the viewer operator, "Maria"), counter-claim-scanner hat | traversing the
  graph on `/project` + `/philosophy` | wants to see which edges (claims) are contested
  while traversing, WITHOUT the flag changing the survey's grouping or edge order.

### Solution
On `/project` and `/philosophy`, each `EdgeRow` whose `cid` is in the page's presence
set renders the neutral "Countered" marker as a render-only `<a href="/claims/{cid}">`
one-hop link to that edge's claim thread. Un-countered edges render exactly as slice-10
(author DID + verbatim confidence + bucket + CID, no marker). The flag NEVER changes the
grouping (`group_by`), the edge order, the contributor list, or any cross-link. Both
routes share the SAME `EdgeRow` render, so the flag is ONE render change covering two
routes.

### Elevator Pitch
- **Before**: Maria traverses `/project` or `/philosophy` and cannot tell which edges (claims) have been countered without copying each edge's CID and opening its thread one-by-one.
- **After**: open `http://127.0.0.1:<port>/project?subject=github:rust-lang/cargo` → each edge whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered edges look exactly as before; the grouping, edge order, contributor list, and every confidence/bucket are unchanged.
- **Decision enabled**: Maria spots a contested edge mid-traversal and decides whether to drill into the disagreement before trusting that edge in her decision — without the flag silently re-sorting or re-grouping the survey for her.

### Domain Examples
#### 1: Happy path — a countered edge in a `/project` group is flagged
Maria opens `/project?subject=github:rust-lang/cargo`. Under the `dependency-pinning`
group, the edge for Tobias's claim (cid `bafy...t0bi`, confidence `0.88`) shows the
"Countered" marker linking to `/claims/bafy...t0bi`. The edge still shows Tobias's DID,
the verbatim `0.88`, its bucket, and its CID — in its original group, in its original
position.

#### 2: Edge case — `/philosophy` flags only countered edges across groups
Maria opens `/philosophy?object=org.openlore.philosophy.memory-safety`. Three project
groups render. Only the two edges whose claims are countered show the marker; the other
nine edges render exactly as slice-10. The group order, the edge order within each
group, and the deduped contributor list are byte-identical to the no-flag render.

#### 3: Boundary — an edge countered twice shows ONE marker; grouping unchanged
An edge `bafy...dup` countered by two authors shows ONE neutral marker (presence-only),
never "disputed by 2", and stays in its original group at its original position. The
`group_by` grouping, the contributor dedup, and the cross-links (`subject` → `/project`,
`object` → `/philosophy`, contributor → `/score`) are all unchanged.

### UAT Scenarios (BDD)
> Driving routes: `GET /project?subject=<uri>` and `GET /philosophy?object=<uri>`
> (the real `openlore ui` subprocess), both shapes.

#### Scenario: A countered edge in a project survey shows the neutral marker linking to its thread
Given Maria's `/project?subject=github:rust-lang/cargo` survey has an edge for Tobias's claim `bafy...t0bi` (confidence 0.88) under the dependency-pinning group, and that claim has ≥1 counter
When she opens `GET /project?subject=github:rust-lang/cargo`
Then Tobias's edge shows the neutral "Countered" marker linking to `/claims/bafy...t0bi`
And the edge still shows Tobias's DID, the verbatim confidence `0.88`, its bucket, and its CID
And the edge stays in the dependency-pinning group at its original position

#### Scenario: A philosophy survey flags only countered edges and never re-groups or re-orders
Given Maria's `/philosophy?object=org.openlore.philosophy.memory-safety` renders 3 project groups totaling 11 edges, 2 of which are countered
When she opens `GET /philosophy?object=org.openlore.philosophy.memory-safety`
Then exactly the 2 countered edges show the marker and the other 9 render exactly as slice-10
And the group order, the edge order within each group, and the deduped contributor list are byte-identical to the no-flag render

#### Scenario: The traversal flag renders identically under htmx and no-JS, and an edge countered twice shows one marker
Given Maria's `/project` survey has an edge `bafy...dup` countered by two distinct authors
When she requests the route WITH `HX-Request` and again WITHOUT it
Then the htmx response is the project fragment with the flag and the no-JS response is the full page = chrome + the SAME fragment
And the `bafy...dup` edge shows exactly ONE neutral marker (never "disputed by 2") in its unchanged group and position

### Acceptance Criteria
- [ ] An `EdgeRow` whose CID is in the presence set shows the `COUNTERED_PRESENCE_FLAG` ("Countered") marker as a render-only `<a href="/claims/{cid}">` one-hop link to that claim's slice-11 thread
- [ ] The same flag arm serves BOTH `/project` and `/philosophy` (one `EdgeRow` render change covering two routes)
- [ ] An un-countered edge renders exactly as slice-10 (author DID + verbatim confidence + bucket + CID, no marker, no noise)
- [ ] The flag NEVER changes the `group_by` grouping, the edge order, the group order, the deduped contributor list, or any cross-link (`subject`/`object`/contributor) — byte-identical to the no-flag render
- [ ] The flag renders identically under the htmx fragment and the no-JS full page (parity by construction) on both routes
- [ ] An edge countered by N authors shows exactly ONE neutral marker (presence-only), never a count/verdict
- [ ] The flag is never a sort/filter/group control

### Outcome KPIs
- **Who**: P-001 dogfood operators traversing `/project` + `/philosophy`
- **Does what**: spots a contested edge mid-traversal and drills into its thread from the edge flag
- **By how much**: leading indicator OF KPI-FED-3 — a measurable share navigate edge-flag → thread during traversal; guardrail: 0 cases of the flag changing grouping/order (KPI-GRAPH-2/4 inherited)
- **Measured by**: per-feature GREEN (the flag renders for countered edges; the no-regression byte-identity gold proves grouping/order unchanged); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today the traversal surfaces show no counter indication; discovering a countered edge requires copying its CID and opening the thread

### Technical Notes
- Extends `EdgeRow` with an `is_countered: bool` set in the effect shell from the presence set (mirrors slice-12). The `group_by` engine (`crates/viewer-domain/src/lib.rs` ~line 2116) carries the flag through to each `EdgeRow`; DESIGN owns whether the projection sets it during grouping or after.
- BOTH `render_project_fragment` and `render_philosophy_fragment` share the `EdgeRow` render, so the flag arm is added ONCE.
- The marker render REUSES the slice-12 flag string (`COUNTERED_PRESENCE_FLAG`) and the `<a href="/claims/{cid}">` one-hop pattern — single source of truth.
- The byte-identity no-regression for grouping/order follows the slice-12 baseline+marker-elision tactic (record the slice-10 survey render, elide the new markers, compare) — the lesson carried from slice-12's evolution archive.

---

## Out of scope (explicit — restated from feature-delta)

- **`/score` (the `ScoreState::Scored{WeightedView}` contribution rows)** — DEFERRED to
  a recommended **slice-14**. `/score` projects a structurally-different ADT (the
  slice-04 `WeightedView` / per-claim `Contribution`), and a presence flag beside a
  weight carries a genuine "does being countered lower the weight?" misread risk that
  needs its own anti-misread copy AND the slice-09 CARDINAL sum-to-weight guarantee
  re-asserted. It is NOT a shared-shape surface; bundling it breaks the ≤1-day budget.
- **`/search`** — already has its own slice-08 "countered by" inline annotation
  (`SEARCH_COUNTERED_BY_PREFIX`); OUT of this slice.
- **`/claims`** — shipped in slice-12; not re-touched.
- Authoring/composing a counter on the viewer; re-rank/filter/re-weight/re-group any
  surface; any count / "disputed by N" / verdict on a flag; any reason text on a flag;
  any network seam on these routes; any N+1 (one batch query per render).
