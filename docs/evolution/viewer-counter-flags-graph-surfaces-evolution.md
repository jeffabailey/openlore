# Evolution: viewer-counter-flags-graph-surfaces (slice-13 at-a-glance "Countered" presence flag on the read-only graph surfaces `/peer-claims`, `/project`, `/philosophy`)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-counter-flags-graph-surfaces/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-049/050 under `docs/adrs/`; this file
> is the post-mortem summary. This slice is a **DELTA on shipped work**: slice-06
> (`htmx-scraper-viewer` — the read-only viewer + the `/peer-claims` route), slice-07
> (`viewer-htmx-swaps` — the htmx progressive-enhancement layer), slice-10
> (`viewer-graph-traversal` — the `/project` + `/philosophy` edge surfaces), slice-11
> (`viewer-counter-claim-threads` — the `GET /claims/{cid}` thread this slice links to),
> and slice-12 (`viewer-counter-claim-list-flags` — the `counter_presence_for` batch read
> + the `/claims` list flag this slice REUSES). Read those parent archives
> (`htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
> `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
> `viewer-counter-claim-list-flags-evolution.md`) for the surfaces and the batch read.
> slice-12 shipped the at-a-glance flag on `/claims`; **slice-13 completes J-003b across
> all LOCAL viewer surfaces** — the graph surfaces (`/peer-claims`, `/project`,
> `/philosophy`). `/score` is the one explicitly-deferred remainder (recommended slice-14).

## Summary

`viewer-counter-flags-graph-surfaces` extends the neutral **"Countered"** presence flag from
the slice-12 `/claims` list onto the read-only **graph surfaces** — `/peer-claims` rows,
`/project` edges, and `/philosophy` edges. Each row/edge whose cid has **≥1 counter** now
renders a render-only `<a href="/claims/{cid}">` one-hop link to the slice-11 counter-thread;
those with **no** counter render **no flag** (no-noise). This **completes the at-a-glance
J-003b across all LOCAL viewer surfaces**: slice-12 shipped `/claims`, slice-13 ships the
graph surfaces. Scope = **Option B (user-confirmed)**: `/score` is deferred to a recommended
**slice-14**. A reader scanning a peer-claims list or a project/philosophy graph now sees, at
a glance, *which surfaced claims have been disagreed with*, and can click straight through to
read the disagreement.

The load-bearing thesis is unchanged from slice-12: **the flag is ADDITIVE — shown but never
applied, and a strict no-regression on each existing render**. With the flag markers elided,
`/peer-claims` is **byte-identical to slice-06** and `/project` + `/philosophy` are
**byte-identical to slice-10** — same grouping, same group order, same edge order, same
deduped contributor list, every confidence/bucket, every cross-link. Threading the presence
projection through the shared `group_by` grouper changed **none** of the grouper's
key-order / accumulation / dedup. The presence is **presence-only**: an N-author-countered cid
renders **ONE** neutral marker (via the `HashSet` / `DISTINCT`). The read is guarded against
**N+1**: each render flattens **every edge cid across every group** from the FLAT `SurveyRow`
slice **before** grouping → **ONE** `counter_presence_for` call per render, invariant to group
and edge count. The read is **LOCAL and offline** (DB-only).

The slice ships **ZERO new crates**, **ZERO new route**, and **ZERO new read method**
(workspace stays at **21 members**). It **REUSES the slice-12
`counter_presence_for(&[String]) -> HashSet<String>` batch read VERBATIM** across all three
handlers (ADR-049 — no new read, no new SQL), adds **two `is_countered` projections** mirroring
the slice-12 `from_row_with_presence` pattern (`PeerClaimRowView.is_countered` set in the
shell + `EdgeRow.is_countered` set inside the grouper), one shared `render_edge_row` flag arm
serving both `/project` + `/philosophy`, and **NO new xtask edge**. The 3 byte-identical
presence-flag renders (list / peer / edge) are unified into a single
`render_countered_link` (the detail-view `CounterThread` flag is a different shape, kept
separate).

### What shipped (one paragraph)

The read-only graph surfaces now render a neutral **"Countered"** presence flag wherever a
surfaced cid has **≥1 counter**: on `/peer-claims` rows, on `/project` edges, and on
`/philosophy` edges. The flag is a render-only `<a href="/claims/{cid}">` one-hop link to the
slice-11 counter-thread; un-countered rows/edges render **no flag** (no-noise). All three
handlers **REUSE the slice-12 `counter_presence_for(&[String]) -> HashSet<String>` batch read
VERBATIM** (ADR-049 — no new read method, no new SQL, no adapter change). The `/peer-claims`
handler sets `PeerClaimRowView.is_countered` in the **effect shell** (the slice-12 shell
pattern). The edge surfaces are the real work: the handler **flattens EVERY edge cid across
EVERY group** from the FLAT `SurveyRow` slice **BEFORE** grouping → **ONE**
`counter_presence_for` call per render (ADR-050 — never per-group / per-edge; the N+1 guard),
then `EdgeRow.is_countered` is set **inside** the `group_by` grouper so the render stays a
**total fn of the presence-projected `TraversalView`**; one shared `render_edge_row` arm flags
both `/project` and `/philosophy`. The 3 byte-identical presence-flag renders (list / peer /
edge) are unified into `render_countered_link`. The reads are **LOCAL and read-only** (DB-only
— no artifact read, no network); nothing is persisted. (The `/score` flag was **deferred to a
recommended slice-14** — Option B, user-confirmed.)

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-07 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-07 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-07 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-07 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **10/10 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **16 acceptance scenarios** GREEN: **10 `viewer_counter_flags_graph_surfaces`** (CF-1..CF-N1
  across the three driving ports `/peer-claims` + `/project` + `/philosophy`, incl. the CF-1
  walking skeleton — a flagged `/peer-claims` row linking to the slice-11 thread; one WS) + **6
  GOLD invariants** (`viewer_counter_flags_graph_surfaces_invariants` — read-only, no-write,
  offline-chrome, offline-data, N+1, and the CARDINAL **byte-identity** no-regression on BOTH
  edge routes) + the `viewer-domain` unit/property tests (the two `is_countered` projections +
  the `render_countered_link` arm). The `ViewerServer` harness drives the REAL `openlore ui`
  over HTTP; the store is seeded through the REAL ingest path.
- **Slices 06/07/10/11/12 corpora GREEN — zero regression** (the full workspace acceptance suite
  green across all slices; the byte-identity golds prove `/peer-claims` unchanged vs slice-06
  and `/project` + `/philosophy` unchanged vs slice-10).
- **NO new crate, NO new route, NO new read method**: extends `viewer-domain` (PURE) +
  `adapter-http-viewer` (EFFECT, the three handlers) on the existing `/peer-claims` + `/project`
  + `/philosophy` routes; REUSES the slice-12 `counter_presence_for` read VERBATIM (no
  `adapter-duckdb` change) + the slice-11 drill-link target. Workspace member count stays **21**
  (19 production + 1 test-support + 1 xtask); `cargo xtask check-arch` reports "21 workspace
  members".
- **NO new production dependency**: `maud`/`hyper`/`duckdb` unchanged; no `deny.toml` change;
  **NO new xtask edge** (the dependency graph was already in place).
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production functions
  (**6/6 in-diff viable caught, 0 missed**) — exceeds the ≥80% per-feature gate.
- **2 ADRs** (ADR-049, ADR-050) Accepted/shipped.
- DES integrity: 10/10 steps have complete DES traces.
- Adversarial review: **APPROVED** — zero defects, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members, no new allowlist edge).

## Wave-by-wave changelog

### DISCUSS (2026-06-07)

Luna framed the slice as a **brownfield DELTA on slices 06/07/10/11/12** that **completes the
at-a-glance J-003b across all LOCAL viewer surfaces** — slice-12 shipped the `/claims` list
flag; slice-13 extends the SAME neutral presence flag onto the graph surfaces (`/peer-claims`
rows, `/project` + `/philosophy` edges). Persona is **P-001 (Maria, the node operator)** wearing
the disagreement-scanner hat across the graph views. The load-bearing DISCUSS decision: **the
flag is presence-only and additive** on every surface — it tells a reader *which* surfaced
claims have been countered so they can click through, but NEVER re-groups, re-orders, filters,
or re-weights the graph, and it shows ONE neutral marker per countered cid regardless of how
many counters (or, on the edge surfaces, how many groups) it appears across. **Scope decision
(Option B, user-confirmed): `/score` is deferred to a recommended slice-14** — the graph
surfaces ship now, `/score` next. slice-13 **REALIZES the existing viewer KPI contracts on the
graph surfaces** (read-only / offline guardrails, presence-only attribution, no-regression
rendering) rather than minting new KPI IDs. The walking skeleton is the CF-1 flagged
`/peer-claims` row (the simplest of the three surfaces — a flat list like `/claims`),
validating that the slice-12 shell projection pattern transfers before tackling the edge
surfaces.

### DESIGN (2026-06-07)

Morgan locked slice-13 as an **additive presence flag on three existing routes, not a
re-architecture** — ZERO new crates, ZERO new route, ZERO new read method, ZERO new persisted
type, ZERO new xtask edge. The riskiest design question was the EDGE surfaces (the grouped
traversal render, not a flat list), resolved in two ADRs:

- **ADR-049** (REUSE the slice-12 `counter_presence_for` batch read VERBATIM — no new
  read/SQL): all three handlers reuse the slice-12
  `counter_presence_for(&[String]) -> HashSet<String>` seam (the one-aggregate `SELECT DISTINCT
  referenced_cid` over the ref tables, `params_from_iter` double-bind, empty→no-query,
  ref-tables-only) exactly as shipped — **no new read method, no new SQL, no adapter change**.
  The presence projection is layered in the effect shell / grouper; the underlying reads
  (`list_peer_claims`, the `survey` traversal read) stay UNCHANGED.
- **ADR-050** (flatten edge cids BEFORE grouping — ONE presence call per render, the edge N+1
  guard): the edge surfaces flatten **EVERY edge cid across EVERY group** from the FLAT
  `SurveyRow` slice **BEFORE** `group_by` → **ONE** `counter_presence_for` call per render,
  invariant to group / edge count — **NEVER per-group or per-edge**. `EdgeRow.is_countered` is
  then set **inside** the grouper (`group_by`) so the render stays a **total fn of the
  presence-projected `TraversalView`**; one shared `render_edge_row` arm flags both `/project`
  and `/philosophy`. `PeerClaimRowView.is_countered` is set in the shell (the flat-list slice-12
  pattern). The L1-L4 refactor unifies the 3 byte-identical presence-flag renders (list / peer /
  edge) into `render_countered_link` (the detail-view `CounterThread` flag is a different shape,
  kept separate).

The read-only contract stays enforced at THREE layers (a `StoreReadPort` with no mutation method
[TYPE], the `xtask check-arch` viewer capability rule [STRUCTURAL], and behavioral GOLD
invariants [BEHAVIORAL]). The C4 views, the flatten-before-group_by data-flow, and the
I-CF-1..6 structural-guarantee table are in the DESIGN sections of `feature-delta.md` and
`design/`.

### DISTILL (2026-06-07)

Quinn authored the executable acceptance corpus across two `[[test]]` targets plus the inherited
adapter tests:

- **`viewer_counter_flags_graph_surfaces.rs`** (Tier A — `CF-` ids CF-1..CF-N1, 1 WS, driven
  across the `/peer-claims` + `/project` + `/philosophy` ports): the CF-1 walking skeleton (a
  flagged `/peer-claims` row linking to the slice-11 thread), CF-5 the EDGE foundation (a flagged
  `/project` edge — the real work), the symmetric `/philosophy` flag, the no-JS full page +
  fragment/page parity per surface, the neutral **"Countered"** flag as a `<a
  href="/claims/{cid}">` one-hop drill-link, the **un-countered row/edge → no flag** no-noise
  assertions, the **presence-only** assertions (an N-author-countered cid → ONE marker), and the
  CF-N1 N+1 proxy (an 8-group survey seeded with a SPREAD countered subset so a per-group bug is
  caught).
- **`viewer_counter_flags_graph_surfaces_invariants.rs`** (gold guardrails — 6 ids): read-only,
  no-write, offline-chrome, offline-data, N+1, and the CARDINAL **byte-identity** on BOTH edge
  routes (with the flag markers elided, `/peer-claims` is byte-identical to slice-06 and
  `/project` + `/philosophy` are byte-identical to slice-10 — same grouping, group order, edge
  order, deduped contributor list, every confidence/bucket, every cross-link).
- **Inherited `adapter-duckdb` presence + N+1 bound (no adapter change)**: the slice-12
  `counter_presence_for` 1 / N / 5N + empty bound is inherited verbatim (this slice adds no
  adapter read), plus the `viewer-domain` unit/property tests (the two new `is_countered`
  projections + the `render_countered_link` arm).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL ingest path; multi-author / distinct-peer seeds use a single combined
peer pull (the slice-11/12 carry-forward). RED classification: both acceptance targets COMPILE
green, scenarios FAIL via `todo!()` = MISSING_FUNCTIONALITY (correct RED, not BROKEN).

### DELIVER (2026-06-07)

Executed **10 roadmap steps** via DES-monitored crafter dispatches, each commit carrying a
`Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`. The 10-step shape
phased the surfaces by difficulty — the flat `/peer-claims` foundation first, the grouped edge
foundation (the real work) second, the gold last:

- **Phase 01 — `/peer-claims` foundation (CF-1 WS)** + confirmatory: the CF-1 thick walking
  skeleton ships the `PeerClaimRowView.is_countered` shell projection (the slice-12 flat-list
  pattern) + the REUSED `counter_presence_for` wiring + the presence-flag render on the flat
  `/peer-claims` list — a flagged row linking to the slice-11 thread. Parity / no-noise /
  presence-only on `/peer-claims` fell out of the skeleton.
- **Phase 02 — edge foundation (CF-5 real)** + symmetric + confirmatory: **CF-5 is the genuine
  new work** — `EdgeRow.is_countered` threaded through the shared `group_by` engine (the render
  stays a total fn of the presence-projected `TraversalView`) + the **flatten-before-group_by**
  wiring (ONE flattened presence call per render, ADR-050) + the shared `render_edge_row` arm
  flagging both `/project` and `/philosophy`. The symmetric `/philosophy` flag and the edge
  parity / no-noise / presence-only steps confirmed off the shared arm.
- **Phase 03 — gold** (N+1, read-only / no-write, offline, **byte-identity CARDINAL last**): the
  gold invariants flipped GREEN off the confirmatory render path — the CF-N1 N+1 proxy (8-group
  survey, SPREAD countered subset), read-only / no-write, the two offline golds, and the CARDINAL
  byte-identity on BOTH edge routes last.

The L1-L4 refactor unified the 3 byte-identical presence-flag renders (list / peer / edge) into
`render_countered_link` (the detail-view `CounterThread` flag kept separate — different shape).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-CF-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..12 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-CF-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the two `is_countered` projections + the `render_countered_link` arm), matching slice-02..12 DV-2; killing properties kept IN-CRATE (the slice-04/05 cross-package lesson). | Per-feature gate at deliver-time + DEVOPS sweep backstop; the measurement reaches the real killing suite locally. 6/6 in-diff viable caught, 0 missed. |
| DV-CF-3 | **The flag is ADDITIVE — shown but never applied; `/peer-claims` byte-identical to slice-06, `/project` + `/philosophy` byte-identical to slice-10 with the markers elided** (I-CF-2, CARDINAL). | No-regression is the at-a-glance cardinal on the graph surfaces: a flag that re-grouped / re-ordered / re-weighted the graph would silently change what a reader sees. Guaranteed by leaving the grouper's key-order / accumulation / dedup UNCHANGED by the presence threading; the byte-identity golds (against recorded slice-06/10 baselines, markers elided) prove it byte-for-byte on all three surfaces. |
| DV-CF-4 | **REUSE the slice-12 `counter_presence_for` batch read VERBATIM across all three handlers — no new read method, no new SQL, no adapter change** (ADR-049). | Presence is the same set-membership question on every surface; the slice-12 one-aggregate `DISTINCT` read already answers it, so reusing it verbatim adds zero read surface, zero new SQL, and inherits the slice-12 injection-safety / empty→no-query / N+1 properties for free. |
| DV-CF-5 | **The edge surfaces flatten EVERY edge cid across EVERY group from the FLAT `SurveyRow` slice BEFORE grouping → ONE `counter_presence_for` call per render; `EdgeRow.is_countered` set INSIDE the grouper** (ADR-050). | A per-group or per-edge presence read would be an N+1 that scales with group / edge count; flattening before `group_by` keeps the per-render cost at ONE query regardless, and setting `is_countered` inside the grouper keeps the render a total fn of the presence-projected `TraversalView`. The real work of the slice. |
| DV-CF-6 | **One shared `render_edge_row` arm flags both `/project` and `/philosophy`; the 3 byte-identical presence-flag renders (list / peer / edge) unified into `render_countered_link`** (L1-L4 refactor). | `/project` and `/philosophy` share the edge render, so one arm flags both; the list / peer / edge presence flags were byte-identical, so unifying them into `render_countered_link` removes triplication. The detail-view `CounterThread` flag is a different shape — kept separate (not over-unified). |
| DV-CF-7 | **`/score` deferred to a recommended slice-14 — Option B, user-confirmed.** | Scope was the graph surfaces (`/peer-claims` + `/project` + `/philosophy`); `/score` is a distinct surface, deferred so this slice completes the graph-surface J-003b cleanly rather than stretching to a fourth surface. |

## Cardinal release gates + slice-13 invariants (I-CF-1..6)

The cardinal release gates realized on the graph surfaces — all release-blocking:

1. **Read-only / no key (I-CF-1)** — `/peer-claims` + `/project` + `/philosophy` (now flagged)
   are READs; no write/sign/subscribe route; the web process holds no signing key; the REUSED
   presence-read seam has NO mutation method (type-level). Three-layer: TYPE + STRUCTURAL
   (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only / no-write).
2. **No-regression / shown-never-applied (I-CF-2, CARDINAL)** — with the flag markers elided,
   `/peer-claims` is byte-identical to slice-06 and `/project` + `/philosophy` are byte-identical
   to slice-10 (grouping, group order, edge order, deduped contributor list, every
   confidence/bucket, every cross-link); the flag never re-groups / re-orders / re-weights, and
   the grouper's key-order / accumulation / dedup is unchanged by the presence threading (DV-CF-3
   + the byte-identity golds on both edge routes).
3. **Presence-only (I-CF-3)** — an N-author-countered cid renders ONE neutral marker (via the
   `HashSet` / `DISTINCT`), never "disputed by N" (DV-CF-4 + the presence-only scenarios);
   un-countered rows/edges render no flag (no-noise).
4. **Robust / graceful-degradation (I-CF-4)** — a failed presence read degrades to no flags
   (`unwrap_or_default`, inherited from slice-12), never 5xx; the surface still renders.
5. **N+1 guard (I-CF-5)** — exactly ONE `counter_presence_for` call per render, invariant to
   group and edge count (DV-CF-5 + the flatten-before-group_by wiring + the CF-N1 N+1 proxy on an
   8-group survey with a SPREAD countered subset + the inherited adapter 1 / N / 5N bound).
6. **Offline / local-only (I-CF-6)** — the presence reads hit the LOCAL DB ref tables only (no
   artifact read, no network — fully offline); the pages reference only the vendored local htmx
   asset (no CDN); loopback-only bind; nothing persisted (the two offline golds).

The full slice-13 invariant set (I-CF-1..6; structural-guarantee detail in the DESIGN section of
`feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-CF-1 | Read-only / no key (the flagged `/peer-claims` + `/project` + `/philosophy` are READs; no write/sign/subscribe route; no key in the process; the REUSED presence-read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only/no-write). Cardinal. |
| I-CF-2 | No-regression / shown-never-applied (with the markers elided, `/peer-claims` is byte-identical to slice-06 and `/project` + `/philosophy` are byte-identical to slice-10 — grouping, group order, edge order, deduped contributor list, every confidence/bucket, every cross-link; the grouper's key-order/accumulation/dedup unchanged by the presence threading). | STRUCTURAL (the underlying reads + the grouper UNCHANGED; the flag layered on top, DV-CF-3) + BEHAVIORAL (the byte-identity golds against the recorded slice-06/10 baselines, markers elided, on both edge routes). CARDINAL. |
| I-CF-3 | Presence-only (an N-author-countered cid = ONE neutral marker via the `HashSet`/`DISTINCT`, never "disputed by N"; un-countered rows/edges = no flag). | STRUCTURAL (the `HashSet` membership + slice-12 `SELECT DISTINCT`, DV-CF-4) + BEHAVIORAL (presence-only + no-noise scenarios). Cardinal. |
| I-CF-4 | Robust / graceful-degradation (a failed presence read → no flags via `unwrap_or_default`, never 5xx; the surface still renders). | STRUCTURAL (the presence read is additive enrichment over the existing reads, inherited from slice-12) + BEHAVIORAL (the degradation path). |
| I-CF-5 | N+1 guard (exactly ONE `counter_presence_for` call per render, invariant to group/edge count). | STRUCTURAL (the flatten-before-group_by wiring — every edge cid flattened from the FLAT `SurveyRow` slice before grouping, ADR-050, DV-CF-5) + BEHAVIORAL (the CF-N1 N+1 proxy on an 8-group SPREAD-countered survey + the inherited adapter 1/N/5N bound). Cardinal. |
| I-CF-6 | Offline / local-only (the presence reads hit the LOCAL DB ref tables only — no artifact read, no network; no-CDN chrome; loopback-only; nothing persisted). | STRUCTURAL (the ref-tables-only DB read inherited from slice-12, no Step-B artifact read; the shared `htmx_script` fn + pinned asset; loopback guard unchanged) + BEHAVIORAL (the two offline golds + read-only row-count delta). Cardinal. |

All slice-13 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets (read-only /
no key / human gate / offline + loopback / progressive enhancement / structural fragment/page
parity); confidence stays shown verbatim on every row and edge.

## Quality gates — final report

- **Acceptance / integration**: 10 `viewer_counter_flags_graph_surfaces` (CF-1..CF-N1 across the
  three driving ports, the CF-1 walking skeleton + CF-5 edge foundation) + 6 GOLD
  `viewer_counter_flags_graph_surfaces_invariants` GREEN (incl. the CARDINAL byte-identity on
  both edge routes + the N+1) + the `viewer-domain` unit/property tests (the two `is_countered`
  projections + the `render_countered_link` arm) + the inherited `adapter-duckdb` presence/N+1
  bound (no adapter change); slices 06/07/10/11/12 corpora GREEN — zero regression (the
  byte-identity golds prove `/peer-claims` unchanged vs slice-06 and `/project` + `/philosophy`
  unchanged vs slice-10). The `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the
  store is seeded through the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, no new
  read method, **no new allowlist edge** (the dependency graph was already in place) + the
  confirmed viewer capability rule (read-only counter-presence reads; no signing/identity/PDS, no
  store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; the 3 byte-identical presence-flag renders
  (list / peer / edge) unified into `render_countered_link` (the detail-view `CounterThread` flag
  kept separate); `viewer-domain` purity intact (no I/O imports; maud + ports only; the shell
  projection for `/peer-claims` and the in-grouper projection for the edges keep the renders total
  fns of the presence-projected views).
- **Adversarial review**: **APPROVED** — zero defects, zero Testing Theater. The no-regression
  confirmed load-bearing (the byte-identity golds against the recorded slice-06/10 baselines, both
  edge routes, DV-CF-3); the presence-only confirmed structural (the `HashSet`/`DISTINCT`,
  DV-CF-4); the edge N+1 guard confirmed (one flattened presence call per render, the CF-N1
  8-group SPREAD proxy, DV-CF-5).
- **DES integrity**: PASS — all 10 steps have complete DES traces (10/10).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the two `is_countered`
projections — `PeerClaimRowView.is_countered` + `EdgeRow.is_countered` — and the
`render_countered_link` arm unifying the list / peer / edge presence flags). The slice-04/05
cross-package lesson stays applied — the `viewer-domain` unit/property tests pin the production
functions IN/against the crate, so the per-feature mutation measurement reaches the real killing
suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (the two `is_countered` projections + the `render_countered_link` arm, in-diff) | 6 | 6 | 0 | **100%** (6/6 in-diff viable) |

Slice-13 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0
missed). `adapter-http-viewer` (the three handlers + the flatten-before-group_by wiring) +
`adapter-duckdb` (the REUSED slice-12 read, unchanged) are NOT mutated by design (effect shell;
covered by the GOLD invariants + the inherited adapter presence/N+1 bound through the real
binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **The edge flag is the real work — thread the presence projection THROUGH the shared grouper,
  do NOT flag after grouping (CF-5, DV-CF-5)**: the `/project` + `/philosophy` surfaces are a
  GROUPED traversal render, not a flat list; threading `EdgeRow.is_countered` *inside* the
  `group_by` engine (so the render stays a total fn of the presence-projected `TraversalView`)
  while flattening every edge cid *before* grouping (so the presence read is ONE call per render,
  not per-group) is the load-bearing wiring. **Lesson: to flag a grouped/aggregated render, set
  the presence projection INSIDE the grouper (keeping the render a total fn of the projected
  view) but resolve the batch presence read OVER THE FLAT pre-grouped slice (one call per render)
  — flagging after grouping tempts a per-group N+1, and flagging in the shell breaks the grouper's
  totality.**
- **The CF-N1 N+1 proxy must SPREAD the countered subset across groups, not concentrate it**: the
  N+1 guard seeds an 8-group survey with a countered subset SPREAD across the groups, so a
  per-group presence read (the N+1 regression) would issue 8 queries and be caught — a countered
  subset concentrated in one group would not exercise the spread. **Lesson: an N+1 proxy for a
  grouped render must spread the flagged subset across MANY groups so a per-group regression
  diverges from the one-call baseline; a concentrated subset hides the N+1 because the per-group
  and per-render query counts coincide.**
- **REUSING the slice-12 read verbatim is the win — no new read method, no SQL, no adapter change
  (ADR-049)**: presence is the same set-membership question on every surface, so the slice-12
  `counter_presence_for` one-aggregate `DISTINCT` read answered all three new surfaces with zero
  new read surface and inherited its injection-safety / empty→no-query / N+1 properties for free.
  **Lesson: when a new surface asks the same data question as a shipped one, REUSE the existing
  read verbatim rather than minting a parallel seam — it inherits the prior slice's safety
  properties (binding, empty-guard, batch shape) and keeps the read surface (and the mutation /
  adapter test burden) flat.**
- **The byte-identity baseline tactic transfers from slice-12 to the GROUPED surfaces unchanged
  (DV-CF-3)**: the no-regression proof on `/project` + `/philosophy` records the slice-10 grouped
  output as a fixed baseline and elides the new markers — same tactic as slice-12's slice-06 list
  baseline, now proving grouping / group order / edge order / deduped contributor list unchanged.
  **Lesson (carry-forward): the record-the-prior-slice-output + marker-elision byte-identity
  tactic scales from a flat list to a grouped/aggregated render unchanged — record the grouped
  baseline and elide the new markers; do NOT re-seed a twin store (content-addressed CIDs diverge)
  and do NOT add a production "disable the flag" seam.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-049 fixed the REUSE-verbatim contract; field-level shaping (the two `is_countered` projections, the wiring per handler) left to DELIVER. | All adopted; `PeerClaimRowView.is_countered` (shell) + `EdgeRow.is_countered` (in-grouper) + the REUSED `counter_presence_for` wiring materialized at DELIVER against the render + byte-identity tests; no new read method. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-050 fixed the flatten-before-group_by intent (one presence call per render, never per-group/per-edge). | The edge wiring landed; the CF-N1 N+1 proxy (8-group SPREAD-countered survey) pinned the one-call-per-render guarantee behaviorally; the inherited adapter 1/N/5N bound backstops it. | Resolved at DELIVER. |
| 3 | The L1-L4 refactor anticipated unifying the byte-identical presence-flag renders. | The 3 renders (list / peer / edge) unified into `render_countered_link`; the detail-view `CounterThread` flag kept separate (different shape — not over-unified). | Resolved at DELIVER. |
| 4 | `/score` in scope discussion. | **Deferred to a recommended slice-14 — Option B, user-confirmed** — this slice ships the graph surfaces (`/peer-claims` + `/project` + `/philosophy`) only. | Deferred (recommended slice-14). |
| 5 | Review expected to pass clean. | Review APPROVED — zero defects, zero Testing Theater (no revision needed). | Confirmed at DELIVER. |
| 6 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-CF-2, 100% in-diff 6/6, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-counter-flags-graph-surfaces/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer + the `/peer-claims` route; the byte-identity
  baseline for `/peer-claims`): `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Parent slice-10 archive** (the `/project` + `/philosophy` edge surfaces + the grouper; the
  byte-identity baseline for the edges): `docs/evolution/viewer-graph-traversal-evolution.md`
- **Parent slice-11 archive** (the `GET /claims/{cid}` counter-thread this slice links to):
  `docs/evolution/viewer-counter-claim-threads-evolution.md`
- **Parent slice-12 archive** (the `counter_presence_for` batch read + the `/claims` list flag
  this slice REUSES and mirrors): `docs/evolution/viewer-counter-claim-list-flags-evolution.md`
- **Slice-13 ADRs**: `docs/adrs/ADR-049-reuse-counter-presence-batch-read-graph-surfaces.md`,
  `docs/adrs/ADR-050-flatten-edge-cids-before-group-by-one-presence-call-per-render.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-counter-flags-graph-surfaces/design/` + the DESIGN sections of
  `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-counter-flags-graph-surfaces/deliver/execution-log.json`,
  `docs/feature/viewer-counter-flags-graph-surfaces/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_counter_flags_graph_surfaces.rs` (10 CF-scenarios across the three
  driving ports, the CF-1 walking skeleton + CF-5 edge foundation),
  `tests/acceptance/viewer_counter_flags_graph_surfaces_invariants.rs` (6 gold invariants, incl.
  the byte-identity no-regression on both edge routes + the N+1)
- **Reused read + render constant + drill-link target**: the slice-12
  `counter_presence_for(&[String]) -> HashSet<String>` read (verbatim) + `crates/viewer-domain`
  (`COUNTERED_PRESENCE_FLAG`, slice-11) via `render_countered_link`; the `/claims/{cid}` slice-11
  thread as the link terminus
- **Extended viewer crates**: `crates/viewer-domain` (`PeerClaimRowView.is_countered` +
  `EdgeRow.is_countered` + `render_countered_link`), `crates/adapter-http-viewer` (the existing
  `/peer-claims` + `/project` + `/philosophy` handlers + the presence wiring +
  flatten-before-group_by). `crates/adapter-duckdb` + `crates/ports` UNCHANGED (the slice-12 read
  reused verbatim).
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-claim-list-flags-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
