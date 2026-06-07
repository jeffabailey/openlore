<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-claim-list-flags

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a DELTA on the existing read-only `GET /claims` LIST view of the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-LF-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/11)
> JTBD: YES — every story traces to **J-003b** (`docs/product/jobs.yaml`, sub-job of J-003); no new job created
> Brownfield DELTA on: `htmx-scraper-viewer` (slice-06 `/claims` list), `viewer-htmx-swaps` (slice-07), `viewer-counter-claim-threads` (slice-11 detail thread + the `COUNTERED_PRESENCE_FLAG`), reusing the slice-08 "countered by" inline-annotation precedent (`SEARCH_COUNTERED_BY_PREFIX`) + the slice-03 counter model (ADR-015)
> Date: 2026-06-07 · Owner: Luna (nw-product-owner)
> Slice: slice-12

This file is the canonical DISCUSS-wave delta for `viewer-counter-claim-list-flags`
(slice-12): the explicitly-deferred at-a-glance follow-up to slice-11. slice-11
shipped the counter-claim THREAD on the `/claims/{cid}` DETAIL page; slice-12
surfaces a neutral **"Countered" presence flag on the LOCAL LIST rows** (`GET
/claims`, the own-claims list from slice-06) so the operator can SEE which claims
have disagreement BEFORE drilling in. It realizes the **at-a-glance half of
J-003b** (counter-claim as first-class disagreement): slice-11 made disagreement
LEGIBLE once you open a claim; slice-12 makes it DISCOVERABLE while scanning the
list.

This is a DELTA. It REUSES the slice-11 `COUNTERED_PRESENCE_FLAG = "Countered"`
neutral marker verbatim, the slice-06/07 `page = chrome + fragment` render pattern,
the slice-08 inline-annotation precedent, and the slice-03 counter model (a counter
is an ordinary signed claim with `references[].type == counters` + a mandatory
`reason`, ADR-015). It adds exactly ONE new read capability — a read-only
**batch per-CID counter-presence lookup** (`counter_presence_for(&[cid])`) over the
INDEXED `claim_references ∪ peer_claim_references` tables (the SAME indexed lookup
the slice-11 `query_counter_claims` Step-A uses, widened from one CID to the page's
CID set, in ONE aggregate query — explicitly NOT N+1). Tier-1 content is inlined
here (lean); SSOT lives under `docs/product/`; per-journey/registry artifacts under
`discuss/`.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003b at ~line 253; the slice-12 deferral note in the
  slice-11 changelog at ~line 730)
- ✓ `docs/feature/viewer-counter-claim-threads/discuss/feature-delta.md` (the deferral
  spec at lines ~186-214 + the slice-11 thread contract)
- ✓ `crates/ports/src/store_read.rs` (`list_claims`/`ClaimRow`, `query_counter_claims`
  slice-11 per-CID read + `CounterClaimRow`; confirmed NO batch presence read exists)
- ✓ `crates/adapter-duckdb/src/store_read.rs` (the slice-11 `query_counter_claims`
  Step-A indexed UNION-ALL ref lookup — the precedent the batch read widens; `list_claims`)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (the `/claims` route `claims_page` +
  `render_claims_view_panel_fragment` / `render_claims_page`)
- ✓ `crates/viewer-domain/src/lib.rs` (`ClaimRowView`/`render_claim_row`/`render_claims_table`;
  `COUNTERED_PRESENCE_FLAG = "Countered"` slice-11; `SEARCH_COUNTERED_BY_PREFIX = "countered by"` slice-08)
- ✓ `docs/product/kpi-contracts.yaml` (KPI-FED-3 / KPI-VIEW-1/2 / KPI-AV-2 / KPI-4 / KPI-5 / KPI-HX-G1/2/3)
- ⊘ `docs/product/journeys/author-counter-claim.yaml` (not read in full — slice is VIEW-only; authoring journey is the slice-03 CLI, out of scope; the relevant VIEW contract is inherited from slice-11 feature-delta)
- ⊘ `docs/feature/viewer-counter-claim-list-flags/diverge/` (no DIVERGE wave for this slice — noted as a non-blocking risk; the job is already validated J-003b)

No DISCUSS decision below contradicts the prior-wave evidence: J-003b is validated;
the deferral was explicitly recommended BY slice-11 as `slice-12
(viewer-counter-claim-list-flags)`; this slice executes exactly that recommendation.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME
persona as slices 06/07/08/09/10/11
(`docs/product/personas/senior-engineer-solo-builder.yaml`). She runs `openlore ui`
to GLANCE at her store in a browser (slice-06), navigate it without reloads
(slice-07), search the network (slice-08), read transparent scores (slice-09),
traverse the graph (slice-10), and read counter-claim threads on a claim's detail
page (slice-11). slice-12 extends the SAME read-only viewer with an at-a-glance
hat: when she SCANS her claims list, she now sees — per row — whether a claim has
been countered, WITHOUT having to open each one to find out.

slice-11 introduced the **counter-claim-reader hat**. slice-12 extends that same hat
with the SCANNING dimension: the operator wants to triage disagreement at the LIST
level (which claims are contested?) before committing to read any single thread. UX
guardrails inherited verbatim: read-only, never silently mutate, attribution always
visible (no merged "disputed by N" consensus), confidence display must NEVER read as
"the system thinks this is true," and a counter must never re-order/re-rank/re-weight
the list.

### Counter-claim-scanner hat (NEW facet — slice-12)

P-001 wearing the counter-claim-scanner hat is looking at her `/claims` list and
wants to instantly spot WHICH of her claims have drawn disagreement — so she can
decide where to spend her attention — WITHOUT the flag ever changing the list's
order, paging, counts, or the flagged claim's data.

- **Load-bearing anxieties**: "Do I have to open every single claim one-by-one just
  to find out which ones were countered?" · "Will a 'Countered' tag silently push
  contested claims to the top, or filter them, picking a triage order for me?" · "Is
  this flag a VERDICT ('disputed'/'refuted'/'false'), or just a neutral 'someone
  responded here' marker?"
- **Load-bearing signals of success**: "I can see at a glance, on the list, which
  rows are countered — and the flag links me straight to that claim's thread." · "A
  row with no counter looks EXACTLY like it does today — no badge, no noise." · "The
  list order, paging, and the claim's confidence are byte-identical to before the
  flag existed."

> A new hat facet is appended to
> `docs/product/personas/senior-engineer-solo-builder.yaml` by this DISCUSS wave
> (changelog 2026-06-07).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a
> counter-claim that stands on its own — not a reply on their record — so
> disagreement is a public structured artifact rather than a thread.*
> (`docs/product/jobs.yaml`, sub-job of **J-003**, opportunity score 15,
> `walking_skeleton_for: openlore-federated-read`.)

slice-12 realizes the **AT-A-GLANCE / DISCOVERABILITY half** of the VIEWING side of
J-003b. The job's success signal — "when a peer publishes a counter-claim against
the user's claim, the user learns of it within their next `peer pull` AND knows how
to engage or move on (no surprise-brigade aftermath)" — has two legibility surfaces:
(a) drill-in legibility (the slice-11 thread on `/claims/{cid}`, shipped), and (b)
at-a-glance legibility (KNOWING a claim is countered while scanning the list,
WITHOUT opening it). slice-11 explicitly DEFERRED (b) to this slice
(`viewer-counter-claim-list-flags`, see slice-11 feature-delta lines ~186-214).
slice-12 closes that gap: the operator learns a claim is countered at the moment she
scans, not only when she happens to open it.

No new job. No new sub-job. Every story below traces to J-003b (with the J-003a /
J-003c boundaries made explicit in the JTBD-trace section).

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Every story → J-003b. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-003a (attribute every peer claim without merging) is HONORED — the flag is presence-only (boolean per row); it is NOT a count-aggregate and NEVER a merged "disputed by N" verdict; the attribution lives in the slice-11 thread the flag links to. J-003c (revocable subscription) is untouched — purging a peer removes its counters from the presence read by construction (peer counters live in `peer_claim_references`); the operator's OWN claims being flagged by a peer's counter is correct (the disagreement is real and local). |
| No contradiction with cardinal invariants? | PASS | Shown-never-applied (OD-AV-7 / I-NS-3 / ADR-015 / slice-11 I-CT-2) is HONORED — the flagged claim renders verbatim with its original confidence; the flag NEVER re-ranks/re-weights/filters/re-orders the list, and never changes the row's data or position. Read-only (KPI-VIEW-2), anti-merging (KPI-AV-2), verbatim confidence (KPI-4), local-first (KPI-5) all carry forward. |
| Authoring NOT re-introduced on the viewer? | PASS | This slice adds ZERO write/sign/counter controls. Authoring stays EXCLUSIVELY in the CLI (I-VIEW-3). The flag only RENDERS the presence of counters that already exist; it links to the slice-11 read-only thread, never to a compose form. |
| No-regression on the slice-06 list? | PASS (commitment) | The flag is ADDITIVE. The slice-06 `/claims` list pagination, ordering (`composed_at DESC, cid`), and counts are UNCHANGED — the flag adds a per-row marker, never a `WHERE`/`ORDER BY`/`GROUP BY` that changes which rows appear or where. |
| Job already fully served? | NO (gap is real) | slice-11 serves the DRILL-IN half (the thread once you open a claim). The LIST today shows NO indication of which claims are countered — the operator must open each claim to discover disagreement. slice-11 explicitly deferred this to slice-12. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting DELTA.

---

## Wave: DISCUSS / [REF] Scope fork (DECISION for the user)

slice-11 flagged ONE open scope question for this slice. There are FOUR list-like
read surfaces in the viewer that COULD carry the "Countered" flag:

| Surface | Route | Row shape | Read | Flag cost |
|---|---|---|---|---|
| Own-claims list | `GET /claims` (slice-06) | `ClaimRow` (own claims, the operator's signed claims) | `list_claims` | **the core of this slice** |
| Project / philosophy survey edges | `GET /project`, `GET /philosophy` (slice-10) | `SurveyRow` (own ∪ peer edges) | `query_project_survey` / `query_philosophy_survey` | a SECOND batch presence read shape over a different row set |
| Contributor score rows | `GET /score` (slice-09) | `AttributedClaim` (the per-claim score decomposition) | `query_contributor_scoring_feed` | a THIRD batch presence read shape; risks coupling a presence flag into a SCORE view (where it could be misread as a weight input) |

**The fork**: flag ONLY `/claims` in slice-12, OR ALSO flag the slice-10
`/project` + `/philosophy` edge rows and the slice-09 `/score` rows in this slice.

**Recommendation: `/claims`-list ONLY for slice-12.** Defer `/project` +
`/philosophy` + `/score` flags to a recommended **slice-13
(`viewer-counter-flags-graph-surfaces`)**. Rationale:

1. **Thinnest valuable first slice (Elephant Carpaccio).** `/claims` is the operator's
   PRIMARY scanning surface for HER OWN claims — exactly the "did anyone push back on
   what I published?" triage the J-003b success signal names. It delivers the
   at-a-glance value standalone, in ≤1 day.
2. **One read shape, one render site.** `/claims` reuses the slice-11 Step-A indexed
   ref lookup widened to a CID-set — a single new `counter_presence_for(&[cid])`
   method and a single new render in `render_claim_row`. Adding `/project` +
   `/philosophy` + `/score` would require the SAME presence read against THREE
   different row-projection paths and three render sites — a multi-day,
   multi-surface slice that fails the "ship 4+ new components → not thin" taste test.
3. **`/score` carries a misread risk.** A presence flag on a SCORE row could be read
   as a score input ("does being countered lower the weight?"). It does NOT (and must
   not — shown-never-applied), but putting the flag there invites the confusion. That
   surface deserves its own deliberate slice with its own anti-misread copy.
4. **Disprovable hypothesis isolation.** slice-12's learning hypothesis (operators
   use a list-level flag to triage) is cleanly testable on `/claims` alone; bundling
   three surfaces muddies which surface drove the behavior.

**This is a DECISION the user should confirm.** If the user wants ALL four surfaces
flagged in one slice, that is a legitimate (larger) scope — but it is no longer a
≤1-day slice and the briefs/DoR below would need to expand. The default carried
forward is `/claims`-only.

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-003b, with boundaries)

| Story | Title | job_id | Sub-job realized | Boundary note |
|---|---|---|---|---|
| US-LF-001 | Read-only BATCH counter-presence READ capability in the viewer process (`counter_presence_for`) | `infrastructure-only` | (enables US-LF-002/003) | `infrastructure_rationale` below. NOT a J-003a/c story. |
| US-LF-002 | See a neutral "Countered" flag on each `/claims` list row whose claim has ≥1 counter, linking to that claim's thread | J-003b | J-003b (AT-A-GLANCE half) | NOT J-003c (no subscription change); NOT the authoring half of J-003b (slice-03 CLI); NOT the drill-in thread (slice-11). |
| US-LF-003 | An un-countered row shows no flag (no-noise); the flag never re-orders/re-ranks/filters/re-weights the list; ordering, paging, counts, and confidence are unchanged | J-003b | J-003b (the no-noise + shown-never-applied + no-regression discipline) | NOT J-002c (no scoring); the flag is a presence marker, never a weight, count, or verdict. |

**J-003a / J-003b / J-003c boundary statement (explicit per the brief):**

- **J-003a** (attribute every peer claim without merging) is the cardinal
  anti-merging invariant. slice-12 HONORS it: the list flag is PRESENCE-ONLY (a
  boolean per row — "this claim has ≥1 counter"), NOT a count and NEVER a merged
  "disputed by N" aggregate that would collapse distinct counters. The per-counter
  attribution (each counter's author DID + CID + verbatim reason) lives in the
  slice-11 THREAD the flag LINKS to — slice-12 mints NO J-003a story; it carries the
  invariant by deferring attribution to the existing detail thread.
- **J-003b** (counter-claim as first-class disagreement) is THIS slice's job —
  specifically the AT-A-GLANCE / list-discoverability facet of the VIEWING half. The
  AUTHORING half (the `claim counter` CLI verb) shipped in slice-03; the DRILL-IN
  thread shipped in slice-11. Both are explicitly OUT of scope here.
- **J-003c** (subscription revocable without residue) is untouched. slice-12 adds no
  subscription surface. Purge semantics are inherited unchanged: a purged peer's
  counters vanish from the presence read because they lived in
  `peer_claim_references`; the operator's own claims stay flagged when an ACTIVE
  peer's counter targets them, which is correct (the disagreement is real and local).

### Infrastructure rationale (US-LF-001)

US-LF-001 carries `job_id: infrastructure-only` with this rationale: it adds the
read-only `counter_presence_for(target_cids: &[String])` capability to
`StoreReadPort` (+ its `adapter-duckdb` read impl) — the batch presence plumbing
US-LF-002/003 consume. It produces no user-visible output on its own (no new route,
no rendered page; it returns a set/map of countered CIDs), so it enables a user
decision only THROUGH US-LF-002. The slice contains TWO non-infrastructure,
user-visible stories (US-LF-002, US-LF-003), so the slice has release value
(Dimension-0 slice-level check passes).

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

These are RESTATED as binding commitments for slice-12 (inherited, not re-litigated):

| ID | Commitment | Source |
|---|---|---|
| I-LF-1 (= I-VIEW-1/2/3 / I-CT-1) | **Read-only**: the `/claims` route + the new `counter_presence_for` method hold `StoreReadPort` only — no mutation method, no signing key in the viewer process, no write/sign/counter control on any rendered surface. Authoring stays EXCLUSIVELY in the CLI. Enforced 3 layers (type system: `counter_presence_for` is read-only on a no-mutation trait + xtask check-arch viewer capability rule + behavioral gold). | KPI-VIEW-2, slice-06/07/11 |
| I-LF-2 (= OD-AV-7 / I-NS-3 / I-CT-2) | **Shown, never applied**: the flag is a NEUTRAL presence indicator ("Countered"). The flagged claim is rendered VERBATIM with its ORIGINAL confidence — never overwritten, down-weighted, or re-scored. The flag NEVER filters, re-ranks, re-weights, re-orders, or paginates the list by counter presence: the row's data and position are byte-identical to a no-flag render. The flag adds a marker BESIDE the row; it changes nothing about WHICH rows appear or WHERE. | ADR-015, slice-08 I-NS-3, slice-11 I-CT-2 |
| I-LF-3 (= I-FED-1 / KPI-AV-2) | **Presence-only, no invented or merged flag**: a row is flagged ONLY if a real counter referencing its CID exists in the store (own claims ∪ local peer_claims). The presence read fabricates NO flag and merges NO claims. The flag is a boolean per row (presence), NOT a count claim and NEVER an aggregate verdict. Per-counter attribution is deferred to the slice-11 thread the flag links to (no faceless "disputed by N" badge on the list). | KPI-FED-1/2, slice-03/04/08/11 |
| I-LF-4 (= KPI-4 / FR-VIEW-8) | **Verbatim confidence**: the list row's confidence (the original claim's; never a counter-derived re-score) renders as `0.90`, never `0.9` or `90%`, via the single `render_confidence` site — UNCHANGED by the flag. | KPI-4, slice-06 |
| I-LF-5 (= KPI-5 / KPI-VIEW-5) | **LOCAL-only / offline**: the presence read is LOCAL (the INDEXED `claim_references ∪ peer_claim_references` tables in the local DuckDB — counter presence needs NO per-row artifact read, since the list flag carries NO reason text). NO network seam on this route. The flag renders fully with the network down. Only the vendored local `/static/htmx.min.js` is referenced (no CDN). | KPI-5, slice-06/07 KPI-HX-G2 |
| I-LF-6 (= I-HX-1/4/5 / I-CT-6) | **Progressive enhancement + parity**: an `HX-Request` returns the `/claims` view-panel fragment (list + flags); a no-JS / bookmark / direct-URL request returns the full page = chrome + the SAME fragment (structural parity via `Shape::from_request`). The flag is in the SAME fragment fn both shapes embed, so it renders identically in both. A swap is a nicety, never a requirement. | slice-07 KPI-HX-G1/G2/G3, slice-11 |
| I-LF-7 (= I-CT-7) | **No new crates**: extend the PURE `viewer-domain` + EFFECT `adapter-http-viewer` + `adapter-duckdb` read impl + `ports` + `cli` (composition root) + `xtask`. Workspace stays 21 members. Functional paradigm (ADR-007). | slice-06–11 precedent |
| I-LF-8 (= ADR-046, NEW for this slice) | **Batch presence read, NOT N+1**: the per-CID counter-presence lookup across the WHOLE list page is ONE aggregate query over the indexed ref tables (a single `referenced_cid IN (...)` UNION-ALL `DISTINCT` read over `claim_references ∪ peer_claim_references`), NOT one query per row. This is the load-bearing technical commitment DESIGN must realize. The presence read returns the SET of flagged CIDs for the page's CID list; the pure projection maps it onto rows. | slice-11 `query_counter_claims` Step-A widened |

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-12 does NOT, under any circumstance:

- **Author or compose a counter-claim on the viewer.** No "counter / reply / dispute"
  button, form, or control. Authoring stays EXCLUSIVELY in the CLI `claim counter`
  verb (I-VIEW-3 / I-LF-1). The flag only INDICATES that counters exist.
- **Re-rank, re-order, filter, hide, re-weight, or paginate the list by counter
  presence.** The slice-06 ordering (`composed_at DESC, cid`), paging, and counts are
  UNCHANGED. The flag is a per-row marker, never a sort/filter/group key (I-LF-2 /
  no-regression).
- **Show a count, "net verdict", "consensus", "disputed score", or "X people
  disagree" aggregate on the list row.** The list flag is PRESENCE-ONLY (boolean per
  row). The per-counter attribution + count lives in the slice-11 thread the flag
  LINKS to (I-LF-3). [If DESIGN later chooses to show a true per-CID count instead of
  a boolean, it MUST be the real per-CID count from the same presence read, never an
  aggregate verdict — but the PRODUCT default is presence-only boolean.]
- **Render any reason text on the list flag.** The list flag carries NO `--reason` —
  the verbatim reasons are the slice-11 thread's job (I-LF-3 / I-LF-5). This is WHY
  the presence read needs NO per-row artifact read (it is a pure DB-index lookup),
  keeping it a single aggregate query.
- **Add any network seam to this route.** Counter presence is read from the LOCAL
  indexed ref tables only (`claim_references ∪ peer_claim_references`). No PDS fetch,
  no indexer call, no live verification (peer counters were already signature-verified
  at `peer pull` time per KPI-FED-6; the viewer re-verifies nothing). (I-LF-5)
- **Issue one query per row (N+1).** The presence lookup is ONE batch aggregate query
  over the page's CID set (I-LF-8). DESIGN owns the exact SQL, but the PRODUCT
  contract forbids N+1.
- **Flag the `/peer-claims` list, the `/project` + `/philosophy` survey rows, or the
  `/score` rows** — those are DEFERRED to a recommended slice-13 (see scope fork).
  slice-12 touches ONLY the `/claims` own-claims list.
- **Touch the slice-08 `/search` "countered by" annotation or the slice-11
  `/claims/{cid}` thread** — both already exist; slice-12 adds the LIST flag only.

### Deferred (recommend split — confirmed in the scope fork above)

- **The slice-10 `/project` + `/philosophy` edge-row flags and the slice-09 `/score`
  row flags** → recommended **slice-13 (`viewer-counter-flags-graph-surfaces`)**.
  Each needs the SAME presence read against a DIFFERENT row projection + its own
  render site + (for `/score`) its own anti-misread copy. Bundling them here breaks
  the ≤1-day budget. See "Scope fork" above for the full rationale.

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending: `viewer-domain` (pure), `adapter-http-viewer` (effect), `adapter-duckdb` (read impl), `ports`, `cli`, `xtask` — all existing | No (single context: the viewer) |
| Integration points (new) | 1 (the new `counter_presence_for` batch read method over the existing shared connection) | No (≤5) |
| Estimated effort | ~1 day (one batch read method widening the slice-11 Step-A lookup + one per-row render marker on an EXISTING route; the list, fragment fork, flag constant, and anti-merging discipline all already exist and are REUSED) | No (≤2 weeks) |
| Independent user outcomes | 1 (spot which claims are countered while scanning the list) | No |

**## Scope Assessment: PASS — 3 stories, 1 context, estimated ~1 day.**

The graph/score surfaces are explicitly carved OUT (deferred to slice-13) precisely
to KEEP this at ~1 day. If DESIGN determines the `/claims` flag alone would exceed
1 day, split US-LF-003 (the no-noise + no-regression discipline) into a follow-up —
but US-LF-002 (the flag itself, backed by US-LF-001's batch read) is the irreducible
core and must ship as one slice.

---

## Wave: DISCUSS / [REF] Proposed route(s) + read method

- **Route**: EXTEND the existing `GET /claims` (`claims_page` in
  `adapter-http-viewer`). NO new route. The list page/fragment now renders each row
  (as today) PLUS a neutral "Countered" flag for rows whose claim has ≥1 counter.
- **Read method (new, read-only, BATCH)**:
  `StoreReadPort::counter_presence_for(target_cids: &[String]) -> Result<HashSet<String>, StoreReadError>`
  (or `BTreeSet`/`Vec` — DESIGN owns the exact collection type). Reads the LOCAL
  INDEXED `claim_references ∪ peer_claim_references` (UNION ALL, `WHERE referenced_cid
  IN (...) AND ref_type = 'counters'`, `DISTINCT referenced_cid`) in ONE aggregate
  query, returning the SET of CIDs from `target_cids` that have ≥1 counter. NO
  per-row artifact read (the flag carries no reason text — I-LF-5), so this is a pure
  DB-index lookup, NOT the slice-11 2-step read. ONE query for the whole page, NEVER
  N+1 (I-LF-8). Returns an EMPTY set when none of the page's CIDs are countered (the
  renderer then shows no flags — US-LF-003).
  > DESIGN owns the exact SQL shape for the `IN (...)` parameter binding (DuckDB
  > array param vs expanded placeholders) and the exact return collection. The
  > PRODUCT contract is: read-only, LOCAL, single aggregate query (no N+1),
  > presence-only (set membership, not a count or merge), empty-set-when-none.
- **Pure projection (new, in `viewer-domain`)**: extend `ClaimRowView` with a
  `is_countered: bool` field (projected by the effect shell from the presence set:
  `presence.contains(&row.cid)`), and extend `render_claim_row` to emit the
  `COUNTERED_PRESENCE_FLAG` (REUSED verbatim from slice-11) as a neutral marker —
  ideally an `<a href="/claims/{cid}">Countered</a>` link to that claim's thread —
  only when `is_countered`. An un-countered row renders exactly as today (no marker).
  > DESIGN owns whether `ClaimRowView` gains a bool field vs a wrapping view-model;
  > the PRODUCT contract is the AC below. The flag text is the slice-11
  > `COUNTERED_PRESENCE_FLAG` constant — single source of truth, no new string.

---

## User Stories

See `user-stories.md` (combined file, one section per story).

| ID | One-line | job_id |
|---|---|---|
| US-LF-001 | Read-only BATCH counter-presence READ capability in the viewer process (`counter_presence_for(&[cid])`) — ONE aggregate query, no N+1 | infrastructure-only |
| US-LF-002 | See a neutral "Countered" flag on each `/claims` row whose claim has ≥1 counter; the flag links to that claim's slice-11 thread | J-003b |
| US-LF-003 | An un-countered row shows no flag (no-noise); the flag never re-orders/re-ranks/filters/re-weights the list; ordering, paging, counts, and confidence are byte-identical to slice-06 | J-003b |

---

## Wave: DISCUSS / [REF] User stories with elevator pitches + AC

<!-- Full story bodies live in user-stories.md; the elevator pitches + key AC themes are summarized here for the single-narrative reader. -->

### US-LF-001 — Batch counter-presence read capability (`@infrastructure`)

`@infrastructure` — no Elevator Pitch (produces no user-visible output; enables
US-LF-002). It adds `counter_presence_for(&[cid])` to `StoreReadPort` + the
`adapter-duckdb` impl: ONE aggregate `referenced_cid IN (...)` UNION-ALL DISTINCT
read over the indexed `claim_references ∪ peer_claim_references` tables, returning
the set of countered CIDs for the page's CID list. Read-only, LOCAL, no N+1, no
per-row artifact read, anti-merging by construction (presence set, no JOIN/GROUP-BY
that elides authors).

**Key AC themes**: ONE query for a multi-CID input (a `@property`/gold assertion that
the query count is independent of page size — the N+1 guard); returns only CIDs that
genuinely have a `ref_type='counters'` reference; returns empty set for an
all-un-countered page; LOCAL only (renders offline); the store row-count universe is
unchanged after the read (read-only gold).

### US-LF-002 — At-a-glance "Countered" flag on the list

**Elevator Pitch**
Before: Maria cannot tell which of her signed claims have been countered without opening each `/claims/{cid}` detail page one-by-one.
After: open `http://127.0.0.1:<port>/claims` → each row whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered rows show nothing.
Decision enabled: Maria decides WHICH contested claim to open and read the disagreement on first — triaging her attention from the list instead of blind-opening every claim.

**Key AC themes**: a seeded countered claim's row shows the `COUNTERED_PRESENCE_FLAG`
("Countered") marker; the marker is a render-only `<a href="/claims/{cid}">` one-hop
link to that claim's slice-11 thread; the flag renders identically under htmx
fragment and no-JS full page (parity); the flag is NEUTRAL presence text, never a
verdict ("disputed"/"refuted"/"false") and never a count.

### US-LF-003 — No-noise + no-regression discipline

**Elevator Pitch**
Before: Maria worries an at-a-glance flag might silently re-order, filter, or re-weight her claims list — picking a triage order for her, or making un-countered claims look "wrong".
After: open `http://127.0.0.1:<port>/claims` → un-countered rows show NO marker (no badge, no "0 counters"); the list order, page boundaries, total count, and every row's confidence are byte-identical to the pre-flag render.
Decision enabled: Maria trusts the list as a faithful, un-reordered view of her claims — she scans it the same way she always has, now with a neutral countered marker where (and only where) disagreement actually exists.

**Key AC themes**: an un-countered row renders exactly as slice-06 (no marker, no
empty-state noise); the list ordering/paging/total is byte-identical with and
without the flag present; the flagged claim's confidence is byte-identical
(shown-never-applied); a page mixing countered + un-countered rows flags ONLY the
countered ones; the flag never appears as a sort/filter control.

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-12 mints **NO new KPI ID**. Like slice-08/09/10/11 it REALIZES inherited KPIs
on a new facet (the list-row flag on `/claims`). The relevant inherited KPIs:

- **KPI-FED-3** (`Counter-claim publication rate` — J-003b disagreement as
  first-class artifact, north-star): slice-12 STRENGTHENS the READ side of the J-003b
  loop further than slice-11. slice-11 closed the loop ON the detail page (author a
  counter → see it land in the thread you open). slice-12 closes the DISCOVERY loop
  (see, while scanning, that your claim drew a counter — without hunting for it). A
  plausible cause of low counter-engagement is that the operator never NOTICES her
  claims were countered. Per-feature: GREEN (the flag renders for own + peer
  counters); cohort: YELLOW (pending the inherited opt-in telemetry endpoint, ADR-010).
- **KPI-VIEW-1** (`Time-to-see-store-contents` — legibility north-star): EXTENDED
  into the at-a-glance disagreement dimension (the operator can now SEE, from the
  list alone, which claims are contested — zero drill-in, zero SQL).
- **KPI-VIEW-2** (read-only, guardrail): MET — no write/sign/counter route, no key
  read in the viewer process. Release-blocking.
- **KPI-AV-2 / KPI-FED-1/2** (anti-merging, guardrails): MET — the flag is
  presence-only (a set-membership boolean), never a merged "disputed by N" aggregate;
  per-counter attribution stays in the slice-11 thread. Release-blocking.
- **KPI-4** (verbatim confidence, guardrail): MET — the row's confidence renders
  verbatim, UNCHANGED by the flag; no counter-derived re-score. Release-blocking.
- **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS
  no-regression / read-only, guardrails): MET — the presence read is a LOCAL indexed
  lookup, renders offline, references only the vendored htmx asset, serves a full
  page without HX-Request, and adds no write surface. Release-blocking.
- **NEW guardrail commitment (no new KPI ID — a leading indicator OF KPI-VIEW-1):**
  the presence read is a SINGLE aggregate query regardless of page size (no N+1,
  I-LF-8). This is the slice's load-bearing performance guardrail and is asserted by
  a gold/`@property` test (query count invariant to page size).

A new product hypothesis specific to this slice (a leading indicator OF KPI-FED-3,
not a new KPI ID):

> **Hypothesis**: We believe that surfacing a neutral "Countered" flag on the
> `/claims` list (P-001, counter-claim-scanner hat) will increase the share of
> dogfood users who OPEN a countered claim's thread within the same session they
> author or pull a counter (a leading indicator of KPI-FED-3), because seeing the
> flag while scanning removes the need to blind-open every claim to discover
> disagreement. We will know this is true when, post-slice-12, users report (and
> opt-in telemetry shows) that they navigate from the list flag to a counter thread,
> rather than only discovering counters by chance drill-in.

> Detail rationale is inlined here (lean — no separate `outcome-kpis.md`, matching
> the slice-08/11 precedent). The cross-feature SSOT is `docs/product/kpi-contracts.yaml`.

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the
`GET /claims` route + list render, the read-only store port, the `page =
chrome + fragment` render pattern, the counter-claim domain model, and the
`COUNTERED_PRESENCE_FLAG` neutral marker all already exist (slices 03/06/07/11). The
thinnest end-to-end slice IS US-LF-002 (the flag render on the existing list route),
backed by US-LF-001 (the batch presence read). US-LF-003 (no-noise + no-regression)
is a thin discipline layer on the same render. Delivery sequence: US-LF-001 →
US-LF-002 → US-LF-003. Each is demonstrable in a single session against the real
`openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Journey (visual + emotional arc + HTML mockups): `journey-counter-claim-list-flag-visual.md`
- Journey schema (Gherkin embedded per step): `journey-counter-claim-list-flag.yaml`
- Shared-artifact registry: `shared-artifacts-registry.md`

---

## Wave: DISCUSS / [REF] Definition of Ready

See `definition-of-ready.md`. Verdict: **PASS (9/9)**.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-12 | Low | Low | The job (J-003b) is already validated; slice-11 explicitly recommended this slice with its scope. No design-direction ambiguity — the flag is a single well-defined marker. Noted as a non-blocking risk. |
| N+1 query regression | Medium | High | I-LF-8 makes the batch single-query a HARD product commitment; a gold/`@property` test asserts query count is invariant to page size. DESIGN must realize `referenced_cid IN (...)` in one aggregate read. |
| Flag misread as a verdict | Low | Medium | The flag REUSES the slice-11 neutral `COUNTERED_PRESENCE_FLAG = "Countered"` — already vetted as neutral presence text; copy is "Countered", never "disputed/refuted/false". |
| Scope creep to graph/score surfaces | Medium | Medium | Scope fork explicitly defers `/project`+`/philosophy`+`/score` to slice-13; user confirmation requested before any expansion. |

---

## Changelog

- 2026-06-07 — slice-12 (`viewer-counter-claim-list-flags`) DISCUSS. Traces to J-003b
  (the AT-A-GLANCE / list-discoverability facet of the VIEW half; authoring stays the
  slice-03 CLI, drill-in thread is slice-11). 3 stories (1 infra + 2 user-visible).
  New read-only BATCH `StoreReadPort::counter_presence_for(&[cid])` (ONE aggregate
  query, no N+1 — I-LF-8). No new crates (workspace stays 21), no new KPI ID, no new
  route (extends `GET /claims`). Scope fork: `/claims`-only recommended (defer
  `/project`+`/philosophy`+`/score` to slice-13) — DECISION flagged for user. Scope
  PASS (~1 day). DoR PASS (9/9).
- 2026-06-07 — slice-12 DISTILL (Quinn / nw-acceptance-designer). Reconciliation PASS
  (0 contradictions across DISCUSS/DESIGN/ADR-048). 12 RED acceptance scaffolds authored
  (`viewer_counter_claim_list_flags.rs` ×8 incl. WS LF-1 + `_invariants.rs` ×5 GOLD/wait
  — 13 total counting the registered targets; see below), `todo!()`-bodied per ADR-025;
  3 new support seeds + 5 assert seams stubbed in `support/mod.rs`; both targets
  registered in `crates/cli/Cargo.toml`. `cargo build -p cli` compiles; tests RED.

---

## Wave: DISTILL

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#US-LF-002 | The `/claims` list flags each countered row with the neutral "Countered" marker linking to its slice-11 thread | n/a | LF-1 (WS) + LF-2/LF-4 acceptance scaffolds enter via `GET /claims` (real `openlore ui` subprocess), asserting the rendered marker + one-hop `<a href>` link |
| DISCUSS#US-LF-003 | The flag is additive — order/paging/count/confidence byte-identical to slice-06; un-countered rows carry no noise | n/a | LF-5/LF-6/LF-7 + the LF-INV-ShownNeverApplied gold pin no-regression byte-identity on the rendered list HTML |
| DESIGN#ADR-048 | Batch `counter_presence_for(&[cid])` is ONE aggregate `IN (...)` read (no N+1); presence-only HashSet; ref-tables-only; LOCAL | n/a | LF-8 N+1 behavioral proxy (subprocess layer) + LF-INV-Offline; the strict 1-query bound deferred to the DELIVER adapter-duckdb unit/property test |
| DISCUSS#I-LF-3 | Presence-only boolean, never a count/verdict ("disputed by N") | n/a | LF-3 GOLD reuses the slice-11 neutral-flag verdict-word blocklist on the LIST surface |
| DISCUSS#I-LF-1 | Read-only; authoring stays the CLI; no write/sign/counter control on any surface | n/a | LF-INV-ReadOnly (universe-bound state-delta, Mandate 8) + LF-INV-NoWrite over every shape × posture |

### [REF] Scenario list with tags

| ID | Scenario (test fn) | Story | Invariant | Tags |
|---|---|---|---|---|
| **LF-1** (**WS**) | `open_the_claims_list_with_htmx_flags_only_the_countered_row` | US-LF-002 | I-LF-3/6 | `@walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment @happy` |
| LF-2 | `the_list_flags_render_identically_under_htmx_and_no_js` | US-LF-002 | I-LF-6 | `@driving_port @real-io @no-js @full-page @parity @happy` |
| LF-3 | `a_claim_with_two_counters_shows_one_neutral_presence_marker_on_the_list` | US-LF-002 | I-LF-3 | `@driving_port @real-io @presence-only @anti-merging @gold` |
| LF-4 | `the_countered_marker_is_a_render_only_one_hop_link_to_the_thread` | US-LF-002 | I-LF-6 | `@driving_port @real-io @drill-link @one-hop @happy` |
| LF-5 | `a_store_with_no_counters_renders_the_list_exactly_as_slice_06` | US-LF-003 | I-LF-2 | `@driving_port @real-io @no-noise @empty-set @happy` |
| LF-6 | `the_flag_never_reorders_repages_recounts_or_reweights_the_list` | US-LF-003 | I-LF-2/4 | `@driving_port @real-io @shown-never-applied @no-regression @gold` |
| LF-7 | `a_mixed_page_flags_only_the_countered_rows_in_their_unchanged_positions` | US-LF-003 | I-LF-2/4 | `@driving_port @real-io @mixed-page @shown-never-applied @happy` |
| LF-8 | `a_large_mixed_page_flags_every_countered_row_correctly_in_one_request` | US-LF-001/003 | I-LF-8 | `@driving_port @real-io @n-plus-1-guard @gold` |
| LF-INV-ReadOnly | `every_claims_list_render_with_flags_leaves_the_store_read_only` | US-LF-002/003 | I-LF-1 | `@property @driving_port @real-io @read-only @gold` |
| LF-INV-NoWrite | `no_claims_list_render_with_flags_adds_a_write_or_sign_control` | US-LF-002/003 | I-LF-1 | `@property @driving_port @real-io @read-only @gold` |
| LF-INV-OfflineChrome | `the_flagged_claims_list_page_chrome_stays_offline_no_cdn` | US-LF-002 | I-LF-5 | `@property @driving_port @real-io @offline @no-cdn @gold` |
| LF-INV-Offline | `the_flagged_claims_list_renders_fully_offline` | US-LF-002 | I-LF-5 | `@property @driving_port @real-io @offline @local-first @gold` |
| LF-INV-ShownNeverApplied | `the_list_order_and_confidence_are_byte_identical_with_and_without_flags` | US-LF-003 | I-LF-2/4 | `@property @driving_port @real-io @shown-never-applied @no-regression @gold` |

**13 scenarios total** (8 story + 5 invariant). Error/edge ratio: 7/13 ≈ 54% (no-noise,
empty-set, multi-counter, mixed-page, offline, read-only, no-write/no-regression guards)
— above the 40% mandate; for a read-only additive-flag DELTA the dominant risk surface is
no-regression + presence-only discipline rather than input-validation sad paths (there is
no user input on `GET /claims` beyond `?page`, inherited unchanged from slice-06).

### [REF] WS strategy

Brownfield DELTA — NO walking-skeleton Feature 0. The thinnest end-to-end thread IS
**LF-1** (`@walking_skeleton`): `GET /claims` WITH `HX-Request` over a one-countered store
→ ONLY the list fragment, the countered row flagged + linked, un-countered rows un-flagged.
Per the Architecture of Reference, the driving port (`openlore ui` CLI subprocess) and the
driven-internal store (DuckDB) are REAL (`@real-io`); the route has NO driven-external /
non-deterministic port (no clock/email/network) — it is offline by construction.

### [REF] Adapter coverage table

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| `adapter-duckdb` `StoreReadPort` (`list_claims` + NEW `counter_presence_for`) | YES | LF-1..LF-8 + all LF-INV-* (real seeded DuckDB; presence read over the indexed `claim_references ∪ peer_claim_references`) |
| `adapter-http-viewer` (the `claims_page` SANDWICH route) | YES | every scenario (real `openlore ui` subprocess + in-test HTTP, both shapes) |
| network / clock / external | n/a — NONE | route has no driven-external port (offline by construction, I-LF-5; LF-INV-Offline pins it) |

No `NO — MISSING` rows. The new `counter_presence_for` read is exercised through the
`GET /claims` driving port on a REAL store across every scenario; the strict single-query
N+1 bound is asserted at the DELIVER layer-1/2 (`adapter-duckdb` unit/property), with the
LF-8 subprocess-layer behavioral proxy here (Mandate 9/11 — layer 3+ is example-only).

### [REF] Driving Adapter coverage

`GET /claims` (the `openlore ui` CLI subprocess) is the single driving port for all 13
scenarios (port-to-port — no scenario calls `counter_presence_for` or `viewer-domain`
directly). LF-1 verifies the HX-Request fork (fragment), LF-2 the no-JS full-page fork
(status + shape + content-type + rendered flag parity). No new route, no new entry point.

### [REF] Scaffolds (RED-ready, Mandate 7 / ADR-025)

- `tests/acceptance/viewer_counter_claim_list_flags.rs` — 8 story scaffolds (`// SCAFFOLD: true`); each body `todo!(…)` → panics → RED (MISSING_FUNCTIONALITY).
- `tests/acceptance/viewer_counter_claim_list_flags_invariants.rs` — 5 GOLD/guardrail scaffolds (`// SCAFFOLD: true`); each body `todo!(…)` → RED.
- `tests/acceptance/support/mod.rs` — 3 new seeds (`seed_claims_list_one_countered` / `seed_claims_list_none_countered` / `seed_claims_list_mixed_pages` → `SeededClaimsList`) + 5 assert seams (`assert_list_row_flagged_countered` / `assert_list_row_not_flagged` / `assert_list_flag_links_to_thread` / `assert_list_flag_is_single_neutral_presence` / `assert_list_order_and_confidence_byte_identical`) + `LIST_COUNTERED_FLAG_TEXT` const, all `todo!()`-stubbed (compile, panic at runtime).
- `crates/cli/Cargo.toml` — both `[[test]]` targets registered so `cargo build -p cli` compiles them.

The scaffolds REUSE the slice-11 seeds (`build_verifiable_peer_counter_record`,
`seed_claim_two_counters_distinct_authors`), the slice-06 list harness (`ViewerServer`,
`get`/`get_htmx`, `is_full_page`/`is_fragment`, `references_external_cdn`), the universe-
bound read-only gold (`capture_store_row_count_universe` / `assert_store_read_only`,
Mandate 8), and the slice-11 no-write assertion (`assert_detail_html_has_no_write_or_sign_control`).
No production code is written — DELIVER fills the seed/assert `todo!()` bodies + the
`counter_presence_for` read + the `ClaimRowView.is_countered` render one scenario at a time.

### [REF] Test placement

`tests/acceptance/` (workspace-root acceptance corpus; targets registered in
`crates/cli/Cargo.toml` `[[test]]`) — mirrors `viewer_counter_claim_threads.rs` /
`viewer_store.rs` precedent exactly (one story file + one `_invariants.rs` GOLD file per
viewer slice).

### [REF] Pre-requisites

- DESIGN driving port: `GET /claims` (`adapter-http-viewer::claims_page`, the SANDWICH
  read→presence→project→render) + the NEW read-only `StoreReadPort::counter_presence_for`
  (`adapter-duckdb` impl, ADR-048). `viewer-domain::ClaimRowView.is_countered` +
  `render_claim_row` flag branch.
- DEVOPS environment matrix: **absent** (no `devops/` dir) — WARN, default matrix applied
  (clean local store; subprocess + real DuckDB; offline). No infra constraint affects these
  ATs (the route has no external seam). Mandate 4 environmental realism is satisfied by the
  real-subprocess + real-DuckDB harness inherited from slice-06/11.
