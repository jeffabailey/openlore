<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-flags-graph-surfaces

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a DELTA extending the slice-12 "Countered" presence flag to the OTHER local viewer surfaces)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-CF-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/09/10/11/12)
> JTBD: YES — every non-`@infrastructure` story traces to **J-003b** (`docs/product/jobs.yaml`, sub-job of J-003); no new job created
> Brownfield DELTA on: `htmx-scraper-viewer` (slice-06), `viewer-htmx-swaps` (slice-07), `viewer-contributor-scoring` (slice-09 `/score`), `viewer-graph-traversal` (slice-10 `/project` + `/philosophy`), `viewer-counter-claim-threads` (slice-11 thread + `COUNTERED_PRESENCE_FLAG`), `viewer-counter-claim-list-flags` (slice-12 — the `counter_presence_for` batch read + the `from_row_with_presence` + `render_list_presence_flag` pattern, all REUSED)
> Date: 2026-06-07 · Owner: Luna (nw-product-owner)
> Slice: slice-13

This file is the canonical DISCUSS-wave delta for `viewer-counter-flags-graph-surfaces`
(slice-13): the explicitly-deferred slice-12 follow-up. slice-11 shipped the counter
THREAD on `/claims/{cid}`; slice-12 shipped the at-a-glance "Countered" presence flag on
the `/claims` own-claims LIST. slice-13 extends that SAME neutral "Countered" presence
flag to the OTHER local viewer surfaces so disagreement is discoverable EVERYWHERE while
scanning — realizing the rest of the at-a-glance facet of **J-003b**.

This is a DELTA. It REUSES, verbatim and with NO new read method, the slice-12
`StoreReadPort::counter_presence_for(&[cid]) -> HashSet<String>` batch presence read
(ADR-048), the slice-11/12 `COUNTERED_PRESENCE_FLAG = "Countered"` neutral marker, the
slice-12 `from_row_with_presence` effect-shell projection pattern + the
`render_list_presence_flag` `<a href="/claims/{cid}">` one-hop link shape, and the
slice-06/07/09/10 `page = chrome + fragment` render pattern. Each target surface
collects its page's CID set and calls the EXISTING `counter_presence_for` ONCE per render
(no N+1, no new read). Tier-1 content is inlined here (lean); SSOT lives under
`docs/product/`; per-journey/registry artifacts under `discuss/`.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003b at ~line 253; the slice-12 changelog deferral note
  at ~line 730 — the `/project,/philosophy,/score,/peer-claims` flags "→ recommended slice-13")
- ✓ `CONTEXT.md` (slice-12 SHIPPED; the explicit "Deferred /project,/philosophy,/score,/peer-claims flags → recommended slice-13"; slice-09 /score, slice-10 /project+/philosophy)
- ✓ `docs/evolution/viewer-counter-claim-list-flags-evolution.md` (the slice-12 pattern: `counter_presence_for`, `from_row_with_presence`, `render_list_presence_flag`, the byte-identity baseline+marker-elision tactic, the own-arm adapter-seed lesson; the slice-13 deferral note at deviation #3)
- ✓ `crates/ports/src/store_read.rs` (`counter_presence_for(&[String]) -> HashSet<String>` at lines 360-384 — slice-12, CONFIRMED present and REUSABLE; `list_peer_claims`/`PeerClaimRow`; `query_project_survey`/`query_philosophy_survey`/`SurveyRow`; `query_contributor_scoring_feed`)
- ✓ `crates/viewer-domain/src/lib.rs` (`PeerClaimRowView` + `render_peer_claim_row` ~1059; `EdgeRow`/`EdgeGroup` ~2058-2078 + `group_by` ~2116 + `render_project_fragment`/`render_philosophy_fragment`; `ScoreState::Scored{WeightedView}` ~1772 + `render_score_results_fragment` ~1807; `COUNTERED_PRESENCE_FLAG` ~679; `ClaimRowView.is_countered` + `from_row_with_presence` ~78-88 + `render_list_presence_flag` ~479 — the slice-12 pattern to mirror)
- ✓ `crates/scoring/src/explain.rs` (`Contribution.cid: Cid` at line 25 — confirms each /score contribution carries a CID; relevant to the deferred slice-14)
- ✓ `docs/product/kpi-contracts.yaml` (KPI-FED-3 / KPI-VIEW-1/2 / KPI-VIEW-3 / KPI-AV-2 / KPI-GRAPH-2/4 / KPI-4 / KPI-5 / KPI-HX-G1/2/3)
- ✓ `docs/product/personas/senior-engineer-solo-builder.yaml` (P-001 + the slice-11 counter-claim-reader hat + the slice-12 counter-claim-scanner facet)
- ⊘ `docs/product/journeys/*.yaml` (the VIEW contract is inherited from slice-11/12; the authoring journey is the slice-03 CLI, out of scope)
- ⊘ `docs/feature/viewer-counter-flags-graph-surfaces/diverge/` (no DIVERGE wave for this slice — noted as a non-blocking risk; the job is already validated J-003b and slice-12 explicitly recommended this slice with its scope)

No DISCUSS decision below contradicts the prior-wave evidence: J-003b is validated; the
deferral was explicitly recommended BY slice-12 as `slice-13 (viewer-counter-flags-graph-surfaces)`;
this slice executes that recommendation, with a further scope fork carving `/score` out to slice-14.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona
as slices 06–12 (`docs/product/personas/senior-engineer-solo-builder.yaml`). She runs
`openlore ui` to glance at her store, navigate without reloads, read transparent scores
(`/score`, slice-09), traverse the graph (`/project` + `/philosophy`, slice-10), read
counter-claim threads (`/claims/{cid}`, slice-11), and spot countered claims on her
own-claims list (`/claims`, slice-12). slice-13 extends the SAME counter-claim-scanner
facet (introduced in slice-12) to the OTHER surfaces she scans: the FEDERATED peer-claims
list and the GRAPH-TRAVERSAL edge surveys — so disagreement is discoverable wherever she
is scanning, not only on her own-claims list.

UX guardrails inherited verbatim: read-only, never silently mutate, attribution always
visible (no merged "disputed by N" consensus), confidence/weight display must NEVER read
as "the system thinks this is true," and a counter must never re-order / re-rank /
re-weight / re-group any surface.

### Counter-claim-scanner hat — extended to the graph surfaces (slice-13)

P-001 wearing the counter-claim-scanner hat (slice-12) is now scanning the FEDERATED
peer-claims surface and TRAVERSING the graph, and wants to instantly spot WHICH peer
claims / WHICH edges have drawn disagreement — so she can decide where to spend her
attention — WITHOUT the flag ever changing the surface's order, grouping, paging, counts,
weights, or the flagged row/edge's data.

- **Load-bearing anxieties**: "Do I have to open every peer claim / every edge one-by-one
  to find out which are countered?" · "Will a 'Countered' tag silently re-sort or re-group
  my traversal, or push contested edges around — picking a path for me?" · "Is this flag a
  VERDICT, or just a neutral 'someone responded here' marker — and on a graph edge, does it
  change the edge's confidence or its place in the survey?"
- **Load-bearing signals of success**: "I can see at a glance, on the peer list and on each
  traversal edge, which are countered — and the flag links me straight to that claim's
  thread." · "A peer claim / edge with no counter looks EXACTLY like it does today — no
  badge, no noise." · "The peer-list order, the traversal grouping + edge order, the
  contributor list, and every confidence/bucket are byte-identical to before the flag
  existed."

> This DISCUSS wave appends the slice-13 facet to the slice-12 counter-claim-scanner hat
> in `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-07,
> slice-13). It EXTENDS the existing hat (the federated + traversal surfaces) rather than
> minting a new hat.

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a
> counter-claim that stands on its own — not a reply on their record — so disagreement is
> a public structured artifact rather than a thread.*
> (`docs/product/jobs.yaml`, sub-job of **J-003**, opportunity score 15.)

slice-13 realizes the REST of the AT-A-GLANCE / DISCOVERABILITY facet of the VIEWING side
of J-003b. slice-11 made disagreement legible once you OPEN a claim; slice-12 made it
discoverable while scanning YOUR OWN claims list. But the operator also scans the FEDERATED
peer claims (whom to engage with) and TRAVERSES the graph (which edges to trust in a
decision) — and on those surfaces a countered claim is still invisible until she opens it.
slice-13 closes that gap on the two SHARED-SHAPE local surfaces (`/peer-claims` and the
`/project` + `/philosophy` edge rows), so disagreement is discoverable wherever she scans
LOCALLY. (`/score` is carved out to slice-14 — see the scope fork.)

No new job. No new sub-job. Every non-`@infrastructure` story traces to J-003b.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Every user-visible story → J-003b. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-003a (attribute every peer claim without merging) is HONORED — the flag is presence-only (boolean per row/edge); never a merged "disputed by N" verdict; attribution lives in the slice-11 thread the flag links to. J-003c (revocable subscription) is untouched — purging a peer removes its counters from the presence read by construction (peer counters live in `peer_claim_references`). |
| No contradiction with J-002b (the traversal job slice-13 decorates)? | PASS | The flag is ADDITIVE on the edge — it never changes the `group_by` grouping, the edge order, the deduped contributor list, the cross-links, or any confidence/bucket. The J-002b "aha" (who spans the projects) is UNCHANGED; the flag only annotates which edges are contested. |
| No contradiction with cardinal invariants? | PASS | Shown-never-applied (ADR-015 / slice-11 I-CT-2 / slice-12 I-LF-2) HONORED — the flagged row/edge renders verbatim; the flag NEVER re-ranks / re-weights / filters / re-orders / re-groups any surface. Read-only (KPI-VIEW-2), anti-merging (KPI-AV-2 / KPI-GRAPH-2), verbatim confidence (KPI-4), local-first (KPI-5) all carry forward. |
| Authoring NOT re-introduced on the viewer? | PASS | This slice adds ZERO write/sign/counter controls. Authoring stays EXCLUSIVELY in the CLI (I-VIEW-3). The flag only RENDERS the presence of counters that already exist; it links to the slice-11 read-only thread, never to a compose form. |
| No-regression on the slice-06/09/10 surfaces? | PASS (commitment) | The flag is ADDITIVE. The `/peer-claims` paging + ordering and the `/project` + `/philosophy` grouping + edge ordering + contributor lists are UNCHANGED — the flag adds a per-row/per-edge marker, never a `WHERE`/`ORDER BY`/`GROUP BY` that changes which rows/edges appear or where. |
| Job already fully served? | NO (gap is real) | slice-12 serves ONLY the `/claims` own-claims list. The `/peer-claims` and `/project` + `/philosophy` surfaces show NO indication of which claims/edges are countered. slice-12 explicitly deferred them to slice-13. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting DELTA.

---

## Wave: DISCUSS / [REF] Scope fork (DECISION for the user) — Elephant-Carpaccio gate

slice-12 deferred FOUR candidate surfaces to "slice-13." Run against the right-sizing
gate, those four do NOT fit one ≤1-day slice. Here is the shape of each:

| Surface | Route(s) | Row/edge view-model | Render site | Shape vs slice-12 | Misread risk |
|---|---|---|---|---|---|
| `/peer-claims` | `GET /peer-claims` (slice-06) | `PeerClaimRowView` (flat row, has `cid`) | `render_peer_claim_row` | **mirrors slice-12 closely** (flat row list) | none |
| `/project` + `/philosophy` edges | `GET /project`, `GET /philosophy` (slice-10) | `EdgeRow` inside `EdgeGroup` (has `cid`) | shared `EdgeRow` render in `render_project_fragment` / `render_philosophy_fragment` | **close** — flat edges, but nested in groups; ONE render change covers both routes | low (must not re-group/re-order) |
| `/score` contribution rows | `GET /score` (slice-09) | `scoring::Contribution` inside `ScoreState::Scored{WeightedView}` (has `cid`) | `render_score_results_fragment` | **structurally different ADT** — projects the slice-04 `WeightedView`; the flag sits beside a WEIGHT/subtotal in a breakdown | **HIGH** — "does being countered lower the weight?" + must re-assert the slice-09 CARDINAL sum-to-weight guarantee |

**The fork**: ship all four in slice-13 (Option A), OR ship only the two SHARED-SHAPE
surfaces now and defer `/score` (Option B).

**Recommendation: Option B — ship `/peer-claims` + the `/project`,`/philosophy` EDGE rows
in slice-13; defer `/score` to a recommended slice-14 (`viewer-counter-flags-score-surface`).**
Rationale:

1. **Thinnest valuable first slice (Elephant Carpaccio).** The two shared-shape surfaces
   mirror the slice-12 pattern almost exactly: a flat-row/edge list whose rows carry a
   `cid`, fed by the SAME `counter_presence_for` batch read, rendered with the SAME
   `from_row_with_presence` + `<a href="/claims/{cid}">Countered</a>` pattern. Two
   view-models (`PeerClaimRowView`, `EdgeRow`), and because `/project` + `/philosophy`
   share the `EdgeRow` render, that is effectively TWO render changes covering THREE
   routes — a clean ≤1-day delta.
2. **`/score` is a different ADT with a genuine misread risk.** `/score` projects the
   slice-04 `WeightedView` (per-claim `Contribution` rows inside a ranked breakdown). A
   presence flag BESIDE a weight invites "does being countered lower this weight?" — it
   does NOT and MUST NOT (slice-09 CARDINAL: the flag must not change any weight, bucket,
   rank, or the breakdown sum-to-weight). That surface deserves its own deliberate slice
   with its own anti-misread copy and the sum-to-weight cardinal re-asserted as an
   explicit AC. Bundling it here both overflows the ≤1-day budget AND muddies the
   highest-risk surface.
3. **Disprovable-hypothesis isolation.** The slice-13 learning (operators triage
   disagreement on the federated + traversal surfaces) is cleanly testable on
   `/peer-claims` + the edge surveys alone; bundling `/score` muddies which surface drove
   the behavior, and `/score`'s risk profile (weight-misread) is distinct.

**Delivery sequence (recommended):** slice-13 = `/peer-claims` + `/project`,`/philosophy`
edges (this DISCUSS); slice-14 = `/score` contribution-row flags
(`viewer-counter-flags-score-surface`), with its own anti-misread copy + the slice-09
sum-to-weight CARDINAL re-asserted.

**This is a DECISION the user should confirm.** If the user wants `/score` flagged in this
same slice, that is a legitimate (larger) scope — but it is no longer a ≤1-day slice, it
re-opens the highest-risk surface, and the briefs/DoR below would need a fourth story
(US-CF-004) with the sum-to-weight cardinal AC. The default carried forward is the
two-shared-shape-surfaces split, `/score` → slice-14.

> If the user picks Option A (all four), Luna will add **US-CF-004** (`/score`
> contribution-row flag) with: the `ScoreState`/`Contribution` projection, the
> anti-misread "this is a presence marker beside the contribution, never a score input"
> copy, and the CARDINAL AC "the flag changes NO weight, bucket, rank, or the breakdown
> sum-to-weight (slice-09)". It would push the estimate to ~2 days.

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-003b, with boundaries)

| Story | Title | job_id | Sub-job realized | Boundary note |
|---|---|---|---|---|
| US-CF-001 | Reuse the slice-12 batch counter-presence read across the graph surfaces (wire `counter_presence_for` into the 3 handlers) | `infrastructure-only` | (enables US-CF-002/003) | `infrastructure_rationale` below. NO new read method. NOT a J-003a/c story. |
| US-CF-002 | See a neutral "Countered" flag on each `/peer-claims` row whose claim has ≥1 counter, linking to that claim's thread | J-003b | J-003b (at-a-glance facet, FEDERATED surface) | NOT J-003c (no subscription change); NOT the authoring half (slice-03 CLI); NOT the thread (slice-11). |
| US-CF-003 | See a neutral "Countered" flag on each `/project` + `/philosophy` traversal EDGE whose claim has ≥1 counter, without re-ordering or re-grouping the survey | J-003b | J-003b (at-a-glance facet, GRAPH-TRAVERSAL surface) | DECORATES J-002b (traversal) — never changes its grouping/order/contributor list. NOT J-002c (no scoring); the flag is a presence marker, never a weight or rank input. |

**J-003a / J-003b / J-003c boundary statement (explicit per the brief):**

- **J-003a** (attribute every peer claim without merging) is HONORED: each surface's flag
  is PRESENCE-ONLY (a boolean per row/edge — "this claim has ≥1 counter"), NOT a count and
  NEVER a merged "disputed by N" aggregate. Per-counter attribution (each counter's author
  DID + CID + verbatim reason) lives in the slice-11 THREAD the flag LINKS to. slice-13
  mints NO J-003a story; it carries the invariant by deferring attribution to the existing
  thread.
- **J-003b** (counter-claim as first-class disagreement) is THIS slice's job — the
  at-a-glance / list-and-edge-discoverability facet of the VIEWING half, on the FEDERATED +
  TRAVERSAL surfaces. The AUTHORING half (the `claim counter` CLI verb) shipped in
  slice-03; the DRILL-IN thread shipped in slice-11; the own-claims list flag shipped in
  slice-12. All are explicitly OUT of scope here.
- **J-003c** (subscription revocable without residue) is untouched. slice-13 adds no
  subscription surface. A purged peer's counters vanish from the presence read because they
  lived in `peer_claim_references`; a peer claim being flagged by an ACTIVE peer's (or the
  operator's own) counter is correct (the disagreement is real and local).

### Infrastructure rationale (US-CF-001)

US-CF-001 carries `job_id: infrastructure-only` with this rationale: it adds NO new read
method. It WIRES the EXISTING slice-12 `counter_presence_for(&[String]) -> HashSet<String>`
(ADR-048) into the `/peer-claims`, `/project`, and `/philosophy` page handlers — each
collects its page's CID set and calls the method ONCE per render, then passes the presence
set into the pure projection. It produces no user-visible output on its own (no new route,
no rendered page), so it enables a user decision only THROUGH US-CF-002/003. The slice
contains TWO non-infrastructure, user-visible stories (US-CF-002, US-CF-003), so the slice
has release value (Dimension-0 slice-level check passes).

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-13 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-8). Summary table:

| ID | Commitment | Source |
|---|---|---|
| I-CF-1 (= I-VIEW-1/2/3 / I-LF-1) | **Read-only**: every flagged surface holds `StoreReadPort` only; no mutation method; no signing key; no write/sign/counter control. Authoring stays CLI-only. 3-layer (type + xtask check-arch + behavioral gold). | KPI-VIEW-2, slice-06–12 |
| I-CF-2 (= I-CT-2 / I-LF-2 / ADR-015) | **Shown, never applied**: the flag is a neutral presence marker; the flagged row/edge renders VERBATIM (confidence, weight, bucket, rank, group, position all unchanged); the flag NEVER re-orders / re-ranks / filters / re-weights / re-groups / re-paginates. | ADR-015, slice-11/12 |
| I-CF-3 (= I-FED-1 / KPI-AV-2 / KPI-GRAPH-2 / I-LF-3) | **Presence-only, no invented / no merged flag**: a row/edge is flagged ONLY if a real `ref_type='counters'` reference to its CID exists (claims ∪ peer_claims); a boolean per row/edge, never a count or "disputed by N"; attribution deferred to the slice-11 thread. | KPI-FED-1/2, KPI-GRAPH-2, slice-03/04/11/12 |
| I-CF-4 (= KPI-4 / FR-VIEW-8 / I-LF-4) | **Verbatim confidence / weight**: every confidence (`0.90`) and weight/bucket renders exactly as today via the single existing render site — UNCHANGED by the flag. | KPI-4, slice-09/10/12 |
| I-CF-5 (= KPI-5 / KPI-VIEW-5 / I-LF-5) | **LOCAL-only / offline**: the presence read is the LOCAL indexed `claim_references ∪ peer_claim_references` lookup (no per-row artifact read; no network); each route renders fully offline and references only the vendored local htmx asset (no CDN). | KPI-5, slice-06/07/12 KPI-HX-G2 |
| I-CF-6 (= I-HX-1/4/5 / I-LF-6) | **Progressive enhancement + parity**: an `HX-Request` returns the surface fragment (with flags); a no-JS / direct-URL request returns the full page = chrome + the SAME fragment. The flag is in the SAME fragment fn both shapes embed. | slice-07 KPI-HX-G1/G2/G3, slice-09/10/12 |
| I-CF-7 (= I-LF-7) | **No new crates, NO new read method**: extend PURE `viewer-domain` + EFFECT `adapter-http-viewer` + `cli` + `xtask`; REUSE the slice-12 `counter_presence_for` read. Workspace stays 21. Functional (ADR-007). | slice-06–12 |
| I-CF-8 (= ADR-048 / I-LF-8) | **Batch presence read, NOT N+1 — REUSED**: each surface collects its page's CID set and calls `counter_presence_for` ONCE per render (one aggregate query, invariant to row/edge/group count). For the edge surfaces the CID set is the UNION of all edges across all groups, collected once, queried once. | slice-12 ADR-048 |
| I-CF-9 (NEW for traversal) | **No re-grouping / no re-ordering of the survey**: on `/project` + `/philosophy`, the flag NEVER changes the `group_by` grouping, the group order, the edge order within a group, the deduped contributor list, or any cross-link. Byte-identical to the slice-10 render with markers elided. | slice-10 I-GT-3/4, slice-12 byte-identity tactic |

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-13 does NOT, under any circumstance:

- **Flag the `/score` (`ScoreState::Scored{WeightedView}`) contribution rows** — DEFERRED
  to a recommended **slice-14 (`viewer-counter-flags-score-surface`)**. `/score` projects a
  structurally-different ADT and carries the weight-misread risk + the slice-09 CARDINAL
  sum-to-weight guarantee; it gets its own deliberate slice with anti-misread copy. (See
  the scope fork.)
- **Touch the slice-08 `/search` "countered by" annotation** — it already exists
  (`SEARCH_COUNTERED_BY_PREFIX`); OUT of this slice.
- **Touch the slice-12 `/claims` list flag or the slice-11 `/claims/{cid}` thread** — both
  already shipped; slice-13 adds the `/peer-claims` + edge flags only.
- **Author or compose a counter on the viewer.** No "counter / reply / dispute" button,
  form, or control. Authoring stays EXCLUSIVELY in the CLI `claim counter` verb (I-CF-1).
- **Re-rank, re-order, filter, hide, re-weight, re-group, or re-paginate any surface by
  counter presence.** The `/peer-claims` ordering + paging and the `/project` +
  `/philosophy` grouping + edge order + contributor list are UNCHANGED (I-CF-2 / I-CF-9).
- **Show a count, "net verdict", "consensus", "disputed score", or "X disagree" aggregate
  on any flag.** Every flag is PRESENCE-ONLY (boolean per row/edge). Per-counter attribution
  + count lives in the slice-11 thread the flag LINKS to (I-CF-3).
- **Render any reason text on any flag.** No flag carries a `--reason` — the verbatim
  reasons are the slice-11 thread's job. This is WHY the presence read needs NO per-row
  artifact read (a pure DB-index lookup, one aggregate query — I-CF-5 / I-CF-8).
- **Add any network seam to these routes.** Counter presence is read from the LOCAL indexed
  ref tables only. No PDS fetch, no indexer call, no live verification (peer counters were
  signature-verified at `peer pull` time per KPI-FED-6; the viewer re-verifies nothing).
  (I-CF-5)
- **Add a new read method or new SQL.** slice-13 REUSES the slice-12 `counter_presence_for`
  verbatim (I-CF-7 / I-CF-8).
- **Issue one query per row/edge (N+1).** The presence lookup is ONE batch aggregate query
  over the page's CID set, per render (I-CF-8).

### Deferred (recommend split — confirmed in the scope fork above)

- **The slice-09 `/score` contribution-row flags** → recommended **slice-14
  (`viewer-counter-flags-score-surface`)**. It projects a different ADT (the slice-04
  `WeightedView`), carries the weight-misread risk, and must re-assert the slice-09 CARDINAL
  sum-to-weight guarantee as an explicit AC. See "Scope fork" above.

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra-wiring + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure), `adapter-http-viewer` (effect), `cli`, `xtask` — all existing; NO new read method (REUSE slice-12) | No (single context) |
| Integration points (new) | 0 new read methods (REUSE `counter_presence_for`); 3 handler wirings + 2 view-model projections (`PeerClaimRowView`, `EdgeRow`) + 2 render-site arms (`render_peer_claim_row`, the shared `EdgeRow` render) | No (≤5) |
| Estimated effort | ~1 day (REUSE the slice-12 read + projection + render pattern; two view-models gain `is_countered`; `/project` + `/philosophy` share ONE edge render arm) | No (≤2 weeks) |
| Independent user outcomes | 1 (spot which peer claims / edges are countered while scanning the federated + traversal surfaces) | No |

**## Scope Assessment: PASS — 3 stories, 1 context, estimated ~1 day (Option B: `/peer-claims` + `/project`,`/philosophy` edges; `/score` carved out to slice-14).**

`/score` is explicitly carved OUT (deferred to slice-14) precisely to keep this at ~1 day
and to avoid re-opening the highest-risk (weight-misread) surface inside a shared-shape
slice. If DESIGN determines even the two shared-shape surfaces would exceed 1 day, split
US-CF-003 (`/project` + `/philosophy`) from US-CF-002 (`/peer-claims`) into two sequential
deliverables — but each remains a thin, independently-demonstrable end-to-end slice on its
own surface(s).

---

## Wave: DISCUSS / [REF] Proposed route(s) + read method

- **Routes**: EXTEND the existing `GET /peer-claims` (slice-06), `GET /project?subject=<uri>`
  and `GET /philosophy?object=<uri>` (slice-10). NO new route. Each page/fragment now renders
  its rows/edges (as today) PLUS a neutral "Countered" flag for rows/edges whose claim has ≥1
  counter.
- **Read method (REUSED — NO new method)**: each handler calls the EXISTING slice-12
  `StoreReadPort::counter_presence_for(target_cids: &[String]) -> Result<HashSet<String>, StoreReadError>`
  ONCE per render, after paging its rows/edges, with the page's CID set. For the edge
  surfaces the CID set is the UNION of every `EdgeRow.cid` across all `EdgeGroup`s on the
  page (flattened once). Returns the SET of countered CIDs; the pure projection maps it onto
  rows/edges. ONE query per render, NEVER N+1 (I-CF-8). NO new SQL.
- **Pure projection (new, in `viewer-domain`)**: extend `PeerClaimRowView` and `EdgeRow`
  each with an `is_countered: bool` set in the effect shell from the presence set
  (`presence.contains(&cid)` — mirroring slice-12 `from_row_with_presence`), and extend
  `render_peer_claim_row` + the shared `EdgeRow` render to emit the `COUNTERED_PRESENCE_FLAG`
  (REUSED verbatim) as a render-only `<a href="/claims/{cid}">Countered</a>` link — only when
  `is_countered`. An un-countered row/edge renders exactly as today.
  > DESIGN owns whether the view-models gain a bool field vs a wrapping projection, and
  > whether the `EdgeRow` flag is set during `group_by` or in a post-pass. The PRODUCT
  > contract is the AC in `user-stories.md`. The flag text + the `<a href>` one-hop pattern
  > REUSE the slice-12 `COUNTERED_PRESENCE_FLAG` / `render_list_presence_flag` — single
  > source of truth, no new string.

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-CF-001 | Reuse the slice-12 `counter_presence_for(&[cid])` batch read across the `/peer-claims` + `/project` + `/philosophy` handlers — ONE aggregate query per render, no N+1, NO new read method | infrastructure-only |
| US-CF-002 | See a neutral "Countered" flag on each `/peer-claims` row whose claim has ≥1 counter; the flag links to that claim's slice-11 thread; peer origin + confidence + order unchanged | J-003b |
| US-CF-003 | See a neutral "Countered" flag on each `/project` + `/philosophy` traversal EDGE whose claim has ≥1 counter; the flag never re-groups / re-orders / re-weights the survey; grouping, edge order, contributor list, and every confidence/bucket byte-identical to slice-10 | J-003b |

---

## Wave: DISCUSS / [REF] User stories with elevator pitches + AC

<!-- Full story bodies live in user-stories.md; elevator pitches + key AC themes summarized here for the single-narrative reader. Each AC names its driving route. -->

### US-CF-001 — Reuse the batch counter-presence read (`@infrastructure`)

`@infrastructure` — no Elevator Pitch (produces no user-visible output; enables
US-CF-002/003). It WIRES the EXISTING slice-12 `counter_presence_for(&[cid])` into the
three handlers: each collects its page CID set (peer-claim CIDs; edge CIDs flattened across
all `EdgeGroup`s) and calls the method ONCE per render. NO new read method, NO new SQL.

**Key AC themes**: each handler calls `counter_presence_for` exactly ONCE per render
(driving routes `GET /peer-claims`, `GET /project`, `GET /philosophy`); the edge surfaces
flatten all edge CIDs across groups into ONE call; the query count is invariant to
row/edge/group count (the inherited N+1 guard); an empty / all-un-countered page resolves
to an empty set with no query; the existing reads + their SQL/ordering/paging are unchanged;
no new method added to `StoreReadPort`.

### US-CF-002 — At-a-glance "Countered" flag on `/peer-claims`

**Elevator Pitch**
- Before: Maria cannot tell which of the peer claims on her `/peer-claims` list have been countered without opening each `/claims/{cid}` detail page one-by-one.
- After: open `http://127.0.0.1:<port>/peer-claims` → each peer-claim row whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered rows show nothing; the peer-origin column and row order are unchanged.
- Decision enabled: Maria decides WHICH contested peer claim to open and read the disagreement on first — triaging her attention on the federated surface instead of blind-opening every peer claim.

**Key AC themes** (driving route `GET /peer-claims`, both shapes): a countered peer row
shows the `COUNTERED_PRESENCE_FLAG` marker; the marker is a render-only
`<a href="/claims/{cid}">` one-hop link to the slice-11 thread; the flag renders identically
under htmx fragment + no-JS full page (parity); neutral presence text, never a verdict or
count; the row's peer origin + confidence + CID render verbatim beside the flag; a row
countered by N authors shows ONE marker (presence-only).

### US-CF-003 — At-a-glance "Countered" flag on `/project` + `/philosophy` edges

**Elevator Pitch**
- Before: Maria traverses `/project` or `/philosophy` and cannot tell which edges (claims) have been countered without copying each edge's CID and opening its thread one-by-one.
- After: open `http://127.0.0.1:<port>/project?subject=github:rust-lang/cargo` → each edge whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered edges look exactly as before; the grouping, edge order, contributor list, and every confidence/bucket are unchanged.
- Decision enabled: Maria spots a contested edge mid-traversal and decides whether to drill into the disagreement before trusting that edge — without the flag silently re-sorting or re-grouping the survey for her.

**Key AC themes** (driving routes `GET /project?subject=<uri>` + `GET /philosophy?object=<uri>`,
both shapes): a countered `EdgeRow` shows the marker as a render-only `<a href="/claims/{cid}">`
one-hop link; ONE `EdgeRow` render arm serves BOTH routes; an un-countered edge renders exactly
as slice-10 (no marker, no noise); the flag NEVER changes the `group_by` grouping, the edge/group
order, the deduped contributor list, or any cross-link — byte-identical to the no-flag render
(I-CF-9); parity under htmx + no-JS on both routes; an edge countered by N authors shows ONE
neutral marker; the flag is never a sort/filter/group control.

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-13 mints **NO new KPI ID**. Like slice-08/09/10/11/12 it REALIZES inherited KPIs on
new facets (the federated `/peer-claims` row flag + the traversal edge flag). The relevant
inherited KPIs:

- **KPI-FED-3** (`Counter-claim publication rate` — J-003b disagreement as first-class
  artifact, north-star): slice-13 STRENGTHENS the READ side of the J-003b loop further than
  slice-12. slice-11 closed the loop on the detail page; slice-12 on the own-claims list;
  slice-13 closes it on the FEDERATED + TRAVERSAL surfaces (see, while scanning peers or
  traversing the graph, that a claim/edge drew a counter — without hunting). Per-feature:
  GREEN (the flag renders for own + peer counters on both surfaces); cohort: YELLOW (pending
  the inherited opt-in telemetry endpoint, ADR-010).
- **KPI-VIEW-1** (`Time-to-see-store-contents` — legibility north-star): EXTENDED into the
  at-a-glance disagreement dimension on two more surfaces (zero drill-in, zero SQL).
- **KPI-VIEW-3** (`Federated peer claims distinguishable` — leading): EXTENDED — the
  `/peer-claims` surface now also surfaces which peer claims are contested, alongside the
  origin already shown. The flag is additive to the origin column (origin unchanged).
- **KPI-VIEW-2** (read-only, guardrail): MET — no write/sign/counter route, no key read.
  Release-blocking.
- **KPI-AV-2 / KPI-GRAPH-2 / KPI-FED-1/2** (anti-merging, guardrails): MET — each flag is
  presence-only (a set-membership boolean), never a merged "disputed by N" aggregate;
  per-counter attribution stays in the slice-11 thread; the traversal flag never collapses or
  re-groups attributed edges. Release-blocking.
- **KPI-GRAPH-4** (sparse renders sparse / shown-never-applied discipline on the graph
  surface, guardrail): MET by extension — the traversal flag never manufactures, re-weights,
  or re-orders an edge; it is a neutral annotation beside the verbatim edge. Release-blocking.
- **KPI-4** (verbatim confidence, guardrail): MET — every row/edge confidence renders
  verbatim, UNCHANGED by the flag. Release-blocking.
- **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS
  no-regression / read-only, guardrails): MET — the presence read is a LOCAL indexed lookup,
  each route renders offline, references only the vendored htmx asset, serves a full page
  without HX-Request, and adds no write surface. Release-blocking.
- **Inherited guardrail (no new KPI ID):** the presence read is a SINGLE aggregate query per
  render regardless of row/edge/group count (no N+1, I-CF-8 = slice-12 ADR-048), asserted by
  a behavioral test (query count invariant to size) + the inherited slice-12 adapter property.

A product hypothesis specific to this slice (a leading indicator OF KPI-FED-3, not a new KPI ID):

> **Hypothesis**: We believe that surfacing the neutral "Countered" flag on the
> `/peer-claims` list and on the `/project` + `/philosophy` traversal edges (P-001,
> counter-claim-scanner hat) will increase the share of dogfood users who OPEN a countered
> claim's thread while scanning peers or traversing the graph (a leading indicator of
> KPI-FED-3), because seeing the flag in-context removes the need to blind-open every peer
> claim or copy every edge CID to discover disagreement. We will know this is true when,
> post-slice-13, users report (and opt-in telemetry shows) they navigate from a peer-list /
> edge flag to a counter thread, rather than only discovering counters by chance drill-in.

> Detail rationale is inlined here (lean — no separate `outcome-kpis.md`, matching the
> slice-08/11/12 precedent). The cross-feature SSOT is `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the
`/peer-claims` + `/project` + `/philosophy` routes + renders, the read-only store port, the
`counter_presence_for` batch read (slice-12), the `from_row_with_presence` projection
pattern, the `COUNTERED_PRESENCE_FLAG` neutral marker + the `<a href="/claims/{cid}">`
one-hop link shape, and the `page = chrome + fragment` render pattern all already exist
(slices 03/06/07/10/11/12). The thinnest end-to-end slice IS US-CF-002 (the flag render on
the existing `/peer-claims` route — the closest mirror of slice-12), backed by US-CF-001
(wiring the existing read). US-CF-003 (the edge surfaces) is a parallel thin slice on the
shared `EdgeRow` render. Delivery sequence: US-CF-001 → US-CF-002 → US-CF-003. Each is
demonstrable in a single session against the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Journey (visual + emotional arc + HTML mockups): `journey-counter-flags-graph-surfaces-visual.md`
- Journey schema (Gherkin embedded per step): `journey-counter-flags-graph-surfaces.yaml`
- Shared-artifact registry: `shared-artifacts-registry.md`

---

## Wave: DISCUSS / [REF] Definition of Ready

See `definition-of-ready.md`. Verdict: **PASS (9/9)**.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-13 | Low | Low | The job (J-003b) is already validated; slice-12 explicitly recommended this slice with its scope. No design-direction ambiguity — the flag is a single well-defined marker REUSED from slice-12. Noted as non-blocking risk. |
| N+1 query regression on the edge surfaces (per-group or per-edge call) | Medium | High | I-CF-8 makes the single-batch-call-per-render a HARD product commitment; US-CF-001 AC requires flattening ALL edge CIDs across groups into ONE call; a behavioral test asserts query count invariant to edge/group count. REUSES the slice-12 read whose N+1 guard is already property-tested at the adapter. |
| Traversal flag silently re-groups / re-orders the survey | Medium | High | I-CF-9 + US-CF-003 AC require byte-identity of grouping/edge-order/contributor-list with markers elided (the slice-12 baseline+marker-elision tactic, carried from its evolution archive). |
| Flag misread as a verdict | Low | Medium | The flag REUSES the slice-11/12 neutral `COUNTERED_PRESENCE_FLAG = "Countered"` — already vetted as neutral presence text; copy is "Countered", never "disputed/refuted/false". |
| Scope creep to `/score` (the weight-misread surface) | Medium | Medium | Scope fork explicitly defers `/score` to slice-14 with its own anti-misread copy + the sum-to-weight cardinal; user confirmation requested before any expansion to Option A. |
| Own-arm (`claim_references`) coverage gap (self-counter rule blocks own counters e2e) | Low | Low | Inherited slice-12 lesson: the own arm of the `counter_presence_for` UNION-ALL is covered at the adapter unit level (seed a `Counters` ref directly); the e2e path exercises the peer arm. No new adapter work — the read is REUSED. |

---

## Changelog

- 2026-06-07 — slice-13 (`viewer-counter-flags-graph-surfaces`) DISCUSS. Traces to J-003b
  (the at-a-glance facet of the VIEW half, extended to the FEDERATED `/peer-claims` +
  TRAVERSAL `/project`,`/philosophy` edge surfaces; authoring stays the slice-03 CLI,
  drill-in thread is slice-11, own-claims list flag is slice-12). 3 stories (1 infra-wiring
  + 2 user-visible). REUSES the slice-12 `counter_presence_for(&[cid])` batch read VERBATIM
  — **NO new read method**. No new crates (workspace stays 21), no new KPI ID, no new route
  (extends `GET /peer-claims`, `GET /project`, `GET /philosophy`). **Scope fork: Option B —
  ship `/peer-claims` + `/project`,`/philosophy` edges; defer `/score` (the WeightedView
  contribution rows, a different ADT + the weight-misread risk + the slice-09 sum-to-weight
  CARDINAL) to a recommended slice-14 (`viewer-counter-flags-score-surface`)** — DECISION
  flagged for user. Scope PASS (~1 day). DoR PASS (9/9).
