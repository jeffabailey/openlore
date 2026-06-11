<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-peer-counter-aware-counts

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (an EXTENSION of the slice-17 `GET /` landing summary PEER line + the slice-06/07 `GET /peer-claims` list header on the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-PC-001 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/17/18)
> JTBD: YES — both user-visible stories trace to **J-003b** (counter-claim awareness — the ORIENTATION / at-a-glance-count facet); the enabling read-wiring story is `infrastructure-only` with rationale; no new job/sub-job created
> Brownfield DELTA on: `viewer-counter-aware-counts` (slice-18 — the EXACT pattern this slice mirrors onto peer claims: `count_countered_own_claims` COUNT(DISTINCT)+IN-set, the additive `Option<usize>` `LandingSummary` field, the shared pure `render_countered(Option<usize>)` helper [single SSOT copy site], the missing≠zero independent `.ok()` degrade, the cfg-gated fault seam, presence-once, anti-misread, the `/claims` header render — slice-18 explicitly DEFERRED this peer sibling at WD-CC-7), `viewer-landing-dashboard` (slice-17 — the `LandingSummary` Option-shaped counts, `render_count`, `MISSING_COUNT_MARKER`, the count-only-read decision, the per-count `.ok()` degrade, the landing peer-claims line), `htmx-scraper-viewer` (slice-06/07 — the `/peer-claims` route + list header), and `viewer-counter-flags-graph-surfaces` (slice-13 — the per-row "Countered" flag on `/peer-claims` this slice's count complements)
> Date: 2026-06-10 · Owner: Luna (nw-product-owner)
> Slice: slice-19

This file is the canonical DISCUSS-wave delta for `viewer-peer-counter-aware-counts` (slice-19):
the deferred peer sibling of slice-18. slice-18 shipped the countered-OWN-claims count beside the
own-claims line on the landing ("12 own claims (3 countered)") + in the `/claims` header, and
explicitly deferred the symmetric peer count (WD-CC-7). slice-19 ships that peer count: it
surfaces, at a glance, **how many of the operator's cached PEER claims have been countered**,
beside the peer-claims count, on the landing ("4 peer claims (1 countered)") and in the
`/peer-claims` list header. With slice-18 (own) + slice-19 (peer), counter-aware orientation is
COMPLETE across both own and peer claims; a reader can drill into `/peer-claims` (where the
slice-13 per-row flags already render) to read the disagreements.

This is an ENRICHMENT — the symmetric completion of slice-18's own-claims work onto peer claims —
but a grounded one: it closes the remaining half of the gap between the shipped counter-flag
family and the front-door orientation. It realizes the orientation facet of **J-003b** +
**KPI-VIEW-1** (time-to-see-store-contents — now disputed-claim state across own AND peer).

It EXTENDS the existing `GET /` + `GET /peer-claims` routes (NO new route). It reuses the
slice-12 counter-reference data, the slice-17 `LandingSummary` shape, and the slice-18
`render_countered` helper + count-only-aggregate + fault-seam patterns. NO new render helper. NO
new read method (or, at most, one count-only countered-peer aggregate — the slice-18 mirror with
outer table `peer_claims`, an open DESIGN question). NO new crate; workspace stays 21. Read-only /
no-key / LOCAL / offline, like every viewer surface.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003b "counter-claim authoring as first-class disagreement" at ~line 253 — the VIEW/legibility half realized as orientation here, the SAME facet slice-18 realized for own claims)
- ✓ `crates/viewer-domain/src/lib.rs` (`LandingSummary` ~584 with `own_claims`/`peer_claims`/`active_peers`/`countered_own_claims: Option<usize>` — this slice adds a 5th `countered_peer_claims`; `render_count` ~624; `MISSING_COUNT_MARKER` "—" ~572; `render_countered(Option<usize>)` ~640 — the SHARED helper this slice REUSES, NO new helper; `render_landing` ~655 with the own line "(render_count(own_claims)) ' own claims ' (render_countered(countered_own_claims))" ~674-676 and the PEER line `p { (render_count(summary.peer_claims)) " peer claims" }` ~677 — the EXACT site this slice extends to "… (N countered)"; `render_peer_claims_page` ~1164 with `h1 { "Peer Claims" }` ~1170 — the mirror of the slice-18 `/claims` header `h1 { "My Claims " (render_countered(...)) }` ~389, the site this slice extends)
- ✓ `crates/ports/src/store_read.rs` (`count_claims` ~296; `count_peer_claims` ~316; `count_active_peer_subscriptions` ~333; `count_countered_own_claims` ~354 — the COUNT(DISTINCT)+IN-set aggregate this slice mirrors with outer table `peer_claims`; `counter_presence_for(&[String]) -> HashSet<String>` ~435 — the per-page presence read; NO mutation method on the trait — read-only by construction, I-VIEW-1)
- ✓ `crates/adapter-duckdb/src/store_read.rs` (`count_countered_own_claims` impl ~517 — `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')` — the EXACT shape to mirror with outer table swapped from `claims` to `peer_claims`; the inner `UNION` IN-set is IDENTICAL — only the outer table differs)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (`landing_page` ~447 — threads `count_countered_own_claims` via `countered_count_with_fault_seam(...).ok()`, the site this slice extends with a 5th `.ok()` count; `peer_claims_page` ~876 — currently reads `list_peer_claims` + `counter_presence_for` only, the site this slice extends to also resolve the countered-peer count for the header; the slice-17/18 `*_with_fault_seam` cfg-gated seams ~472-491 — the pattern for a new `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` seam)
- ✓ `docs/feature/viewer-counter-aware-counts/` (slice-18 — the EXACT pattern + ADR-055: count-only `count_countered_own_claims`, the additive `Option<usize>` field, the shared `render_countered`, the `.ok()` independent degrade, the fault seam, presence-once, anti-misread, the `/claims` header render; WD-CC-7 DEFERRED this peer sibling — "the count-only query shape makes the deferred sibling clean to add later")
- ✓ `docs/feature/viewer-counter-flags-graph-surfaces/` (slice-13 — the per-row "Countered" flag on `/peer-claims` this slice's header count complements)
- ⊘ `docs/feature/viewer-peer-counter-aware-counts/diverge/` (no DIVERGE wave for this slice — consistent with all prior OpenLore slices; noted as a non-blocking risk R-PC-1; J-003b validated, the counter-flag family + landing summary + slice-18 own mirror SHIPPED)

No DISCUSS decision below contradicts the prior-wave evidence: the viewer is read-only (slices
06–18); `GET /` and `GET /peer-claims` already exist; the counter-reference tables + the slice-17
`LandingSummary` + the slice-18 `render_countered` helper + the count-only-aggregate pattern
already exist; a cached peer claim is countered by the operator (in `claim_references`) or by
another peer (in `peer_claim_references`).

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona as
slices 06–18 (`docs/product/personas/senior-engineer-solo-builder.yaml`). slice-19 EXTENDS her
**counter-aware-orientation** hat (added in slice-18 for own claims) onto cached peer claims: she
opens `/` (or lands on `/peer-claims`) and immediately sees not just how many peer claims she has
cached but how many of those have been disputed — completing the own+peer orientation slice-18
began.

### Counter-aware-orientation hat (EXTENDED for slice-19 — now spans peer claims)

P-001 wearing this hat opens `/` to answer, in the first second, not only "What's in my store?",
"Where can I go?" (slice-17), and "How much of my OWN work has been disputed?" (slice-18) but also
**"How much of the PEER material I've cached has been disputed?"** — WITHOUT leaving the front
door, and WITHOUT the count ever re-weighting the peer claims or reading as a penalty.

- **Load-bearing anxiety** (from J-003b): "When someone counters a peer claim I've cached — even
  my own counter — will I even know how much of my cached peer material is contested without
  opening each one?"
- **Load-bearing signals of success**: "The moment I open `/`, I see '4 peer claims (1 countered)'
  beside '12 own claims (3 countered)' — how much of BOTH is disputed, at a glance." · "It says
  '(0 countered)' honestly when none of my cached peer claims has drawn a counter." · "A peer
  claim countered by two counterers counts once — it's awareness, not a 'disputed by N' score." ·
  "The count never re-weights the peer claims; the '4' is unchanged; the slice-18 own line is
  untouched." · "If the count can't be read, the rest of my summary still renders." · "It loads
  with the network down."

> This DISCUSS wave notes the slice-19 extension of the counter-aware-orientation hat (now
> spanning cached peer claims) in `docs/product/personas/senior-engineer-solo-builder.yaml`
> (changelog 2026-06-10, slice-19).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a counter-claim
> that stands on its own … so disagreement is a public structured artifact rather than a thread.*
> (`docs/product/jobs.yaml` ~line 253. The VIEW/legibility half of J-003b is realized across
> slices 11–14, 18; slice-19 realizes its ORIENTATION / at-a-glance-count facet for cached PEER
> claims.)

slice-19 realizes the **ORIENTATION / AT-A-GLANCE-COUNT** facet of J-003b for cached PEER claims.
The counter-flag family (slices 11–14) made individual disagreements legible. slice-17 made the
store glanceable. slice-18 connected the OWN-claim disputed count to the front door. slice-19
completes it: the operator sees, at the orientation surfaces, HOW MANY of her cached PEER claims
have drawn pushback — counter-aware orientation now spanning BOTH own AND peer claims.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Both user-visible stories (US-PC-001/002) → J-003b (orientation facet). The infra story enables them. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | The count is a store-level PRESENCE count ("how many cached peer claims are countered"), NOT a merge of authors, NOT a "consensus" — it never collapses per-author attribution (reading WHO countered WHAT stays the slice-11 thread + the slice-13 per-row flags). Reinforces J-003a (anti-merging) by linking OUT to the attributed surfaces. |
| No contradiction with cardinal invariants? | PASS | Read-only / no-key (I-VIEW-1/2/3, KPI-VIEW-2) HONORED. Local-first (KPI-5) HONORED — a LOCAL aggregate. Shown-never-applied (J-003b accuracy) HONORED — the count is presence-only, never a re-weight/verdict; the peer-claims count + confidences stay verbatim. |
| Count is a presence count, not a "by N" total or a re-weight? | PASS | A peer claim countered by N counterers counts ONCE (WD-PC-4); the peer-claims "4" is unchanged; the copy is neutral (no penalty/score/verdict — WD-PC-10). |
| New route introduced? | NO (extends `GET /` + `GET /peer-claims`) | slice-19 adds ZERO new routes. |
| New read method introduced? | NO (reuses existing data) — or at most ONE count-only countered-peer aggregate | the counter-reference tables already exist; the countered-peer count is either a count-only aggregate (recommended, the slice-18 mirror with outer `peer_claims`) or `counter_presence_for(peer_cids).len()` (OPEN DESIGN QUESTION WD-PC-5). |
| Job already fully served? | NO (the gap is real) | slice-18 connected the OWN disputed count to the front door; the PEER line is still bare. The operator must leave `/` and scan `/peer-claims` to learn how much cached peer material is disputed. |

The gate PASSES. A coherent, single-job, non-contradicting extension of the orientation
surfaces — the symmetric completion of slice-18.

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-19 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-7). Summary table:

| ID | Commitment | Source |
|---|---|---|
| C-1 (= I-VIEW-1/2/3) — **CARDINAL** | **Read-only / no key**: the count is read + rendered only — no mutation method, no key, no write/compose/sign/subscribe/follow control; render-only text. | KPI-VIEW-2, slice-06–18 |
| C-2 (= KPI-5 / KPI-VIEW-5) — **CARDINAL** | **LOCAL-only / offline + graceful degrade**: a LOCAL aggregate; renders offline (vendored htmx only); a failed read → the missing marker WITHOUT blanking the sibling counts/rows/flags; never a 5xx. | KPI-5, KPI-VIEW-5, NFR-VIEW-6, slice-17 WD-LD-2, slice-18 C-2 |
| C-3 — **CARDINAL** | **Cheap / no N+1**: a SMALL FIXED number of aggregate reads (ideally ONE count-only aggregate — a 5th sibling), invariant to store size; the landing's 4-read budget grows by exactly 1. NO per-claim loop. | slice-17 C-4, slice-12 I-LF-8, slice-18 ADR-055 D1 |
| C-4 — **CARDINAL (J-003b accuracy)** | **Presence count, never a total / re-weight / verdict**: how many cached peer claims have ≥1 counter; a peer claim countered by N counterers counts ONCE; the peer-claims count is unchanged. | J-003b shown-never-applied (slices 11–14, 18) |
| C-5 | **Missing ≠ zero**: Option-shaped; Some(0)→"(0 countered)", None→the slice-17 missing marker "(— countered)"; no fabricated 0; independent degrade. | slice-17 WD-LD-8 / ADR-054 D2 / slice-18 C-5 |
| C-6 | **Anti-misread / neutral copy via the SHARED helper**: "(N countered)" is neutral disputed-claim awareness — no penalty/deduction/score/"refuted"/"false". Reuses the slice-18 `render_countered` (NO new helper). | slice-14 / slice-18 anti-misread |
| C-7 | **No new crate; no new route; reuse the counter-reference data + the slice-18 helper**: extend `viewer-domain` + `adapter-http-viewer` (+ at most `ports`/`adapter-duckdb`). Workspace stays 21. Functional paradigm (ADR-007). | slice-06–18 precedent |

---

## Wave: DISCUSS / [REF] Proposed change + count-read approach

- **Routes (EXTENDED — NO new route)**: `GET /` (landing) and `GET /peer-claims` (list) already
  exist. slice-19 renders the countered-peer-claims count beside the peer-claims count on the
  landing summary and beside "Peer Claims" in the `/peer-claims` header.
- **Read (REUSED data — NO new read method, with ONE open DESIGN question)**: the countered-peer
  count = the number of peer-claim CIDs (`SELECT cid FROM peer_claims`) that appear as a countered
  `referenced_cid` (`ref_type='counters'`) in `claim_references ∪ peer_claim_references`. A cached
  peer claim is countered by the OPERATOR (her counter in `claim_references`) OR by ANOTHER PEER
  (their counter in `peer_claim_references` — slice-11).

  > **OPEN DESIGN QUESTION (DD owns it — WD-PC-5)**: a count-only aggregate
  > `count_countered_peer_claims()` — the EXACT slice-18 mirror with outer table `peer_claims`:
  > `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM
  > claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
  > peer_claim_references WHERE ref_type='counters')` (a 5th count-only sibling, mirroring slice-18
  > ADR-055 D1) OR reuse the slice-12 `counter_presence_for(all_peer_cids).len()` (zero new port
  > surface; materializes every peer cid + the presence set just to count). The PRODUCT contract
  > is: a SINGLE aggregate read, invariant to store size (C-3) — DESIGN picks the cheaper/cleaner.
  > RECOMMEND the count-only aggregate (the 5th sibling; symmetry + cheapness, per slice-18 ADR-055
  > D1). The inner `UNION` IN-set is IDENTICAL to slice-18's — only the outer table differs. If
  > DESIGN adds it, it is a read-only method on `StoreReadPort`; `adapter-duckdb` gains ONE
  > aggregate impl; workspace stays 21. (See R-PC-9: DESIGN verifies the xtask anti-merging rule
  > stays GREEN with `peer_claims` in the outer FROM.)

- **Pure render (EXTENDED, in `viewer-domain`)**: extend the slice-17 `LandingSummary` with a 5th
  `countered_peer_claims: Option<usize>` field so a failed read degrades to the missing marker;
  `render_landing` renders it on the PEER line ("(N countered)") via the EXISTING `render_countered`
  helper; `render_peer_claims_page` takes the bare `Option<usize>` and renders the SAME number in
  its header (single source — WD-PC-8). NO new render helper. DESIGN owns the exact markup. The
  slice-18 own line is UNTOUCHED.

---

## Wave: DISCUSS / [REF] JTBD trace (story → J-003b, with boundaries)

| Story | Title | job_id | Boundary note |
|---|---|---|---|
| US-PC-000 | Resolve the countered-peer-claims count in a fixed aggregate read and thread it into the landing summary + `/peer-claims` header, degrading independently | `infrastructure-only` | `infrastructure_rationale` in user-stories.md. Enables US-PC-001/002. NOT a mutation; read-only. Reuses the slice-12 counter-ref data; the slice-18 SQL with outer table `peer_claims`. |
| US-PC-001 | At the front door, see at a glance how many of my cached peer claims have been countered | J-003b | The ORIENTATION facet of J-003b. The count is a LOCAL presence aggregate; rendered beside the peer-claims count. Reading WHO countered WHAT stays the slice-11 thread + slice-13 flags. The slice-18 own line is untouched. |
| US-PC-002 | In the `/peer-claims` list header, see the same disputed-peer-claim awareness count | J-003b | The SAME count (single source) in the `/peer-claims` header. Additive — the list order/paging/flags/origin are untouched. |

### Infrastructure rationale (US-PC-000)

US-PC-000 carries `job_id: infrastructure-only`: it resolves the LOCAL countered-peer-claims
count in a fixed aggregate read (reusing the slice-12 counter-reference tables; the slice-18 SQL
with outer table `peer_claims`) and threads it into the slice-17 `LandingSummary` (a 5th field) +
the `/peer-claims` header resolution, degrading independently on read failure (never a 5xx). It
produces no user-visible output on its own (the rendered "(N countered)" is US-PC-001/002), so it
enables a user decision only THROUGH those stories. The slice contains TWO non-infrastructure,
user-visible stories with a real decision, so the slice has release value. READ-ONLY by
construction: no mutation method (and if DESIGN elects the count-only aggregate, it is on
`StoreReadPort`, which declares no mutation method).

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

See `requirements.md` §"Out of scope" and `user-stories.md` §"Out of scope". Headlines: no
write/compose/sign control on `/` or `/peer-claims` (C-1, CARDINAL); no new route; no counter
CONTENT (authors/reasons/threads) in the count; no "disputed by N" total / re-weight / verdict
(C-4); no re-order/filter of `/peer-claims` by countered state; no third dimension / re-touching
the slice-18 own count (BR-PC-4); no network seam (C-2); no N+1 (C-3); no penalty/score copy
(C-6); no fabricated 0 on a failed read (C-5); no new crate (workspace stays 21); no new render
helper (reuse slice-18 `render_countered`).

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey visualization investment (Phase 1.5). A near-exact mirror of slice-18 onto
peer claims — expected PASS, confirmed PASS.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure) + `adapter-http-viewer` (effect) + at most `ports`/`adapter-duckdb` IF DESIGN elects a count-only aggregate — all existing; NO new crate | No (single context) |
| Walking-skeleton integration points | 3: (1) resolve the countered-peer count (reuse slice-12 counter-ref tables; slice-18 SQL with outer `peer_claims`), (2) thread into the slice-17 `LandingSummary` (5th field) + the `/peer-claims` header (single source), (3) render "(N countered)" on both surfaces via the existing `render_countered` | No (≤5) |
| Estimated effort | ~0.5–1 day (cheaper than slice-18 — helper/fault-seam/SQL shape exist) | No (≤2 weeks) |
| Independent user outcomes | 1 (see how much of my cached peer material has been disputed, at a glance, on the orientation surfaces) | No |

**## Scope Assessment: PASS — 3 stories (1 infra + 2 user-visible), 1 context, 3 integration points (resolve the countered-peer count + thread into the slice-17 summary/`/peer-claims` header + render on both surfaces), estimated ~0.5–1 day. No new route; reuses the counter-reference data + the slice-17 summary + the slice-18 `render_countered` helper + count-only-aggregate + fault-seam patterns; no new crate; workspace stays 21.**

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-PC-000 | Resolve the countered-peer-claims count in a fixed aggregate read (reusing the slice-12 counter-reference data; the slice-18 SQL with outer `peer_claims`) and thread it into the slice-17 `LandingSummary` + the `/peer-claims` header, degrading independently on read failure | infrastructure-only |
| US-PC-001 | At the front door, see at a glance how many of my cached peer claims have been countered ("4 peer claims (1 countered)") | J-003b |
| US-PC-002 | In the `/peer-claims` list header, see the same disputed-peer-claim awareness count, consistent with the landing | J-003b |

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-19 mints **NO new KPI ID**. Like slices 08–18 it REALIZES inherited KPIs on a new facet
(the disputed-PEER-claim awareness count on `/` + `/peer-claims`). Full detail in
`outcome-kpis.md`. Relevant inherited KPIs: **KPI-VIEW-1** (time-to-see-store-contents — now
disputed-claim state across own AND peer, realized); **KPI-VIEW-2 / KPI-5 / KPI-VIEW-5**
(read-only / local-first / offline, guardrails — MET); **KPI-FED-3** (counter-claim publication
rate — READ-side strengthening, per the slice-11/12/18 precedent).

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the read-only
`StoreReadPort`, the indexed counter-reference tables (slice-12), the slice-17 `LandingSummary` +
`render_landing` + `MISSING_COUNT_MARKER`, the slice-18 `render_countered` helper +
`count_countered_own_claims` aggregate + fault-seam pattern, and the slice-06/07 `/peer-claims`
header all already exist. The thinnest end-to-end slice IS US-PC-001 (the landing peer countered
count), backed by US-PC-000 (resolving the count), with US-PC-002 (the `/peer-claims` header)
reusing the SAME count. Delivery sequence: US-PC-000 → US-PC-001 → US-PC-002. Each is
demonstrable in a single session against the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Requirements (functional + NFR + business rules): `requirements.md`
- User stories (combined, `## System Constraints` at top): `user-stories.md`
- Acceptance criteria (BDD, by theme): `acceptance-criteria.md`
- Outcome KPIs: `outcome-kpis.md`
- Definition of Ready: `dor-checklist.md`
- Wave decisions (WD-PC-*): `wave-decisions.md`

> Lean mode: the standalone journey-visual + journey-yaml + shared-artifacts-registry are NOT
> produced for this thin DELTA (mirroring the slice-08/12/15/17/18 lean set). The shared
> artifacts are the slice-17 `LandingSummary` (extended with the 5th countered-peer field, the
> single source for both surfaces), the slice-18 `render_countered` helper (reused for the copy),
> and the slice-12 counter-reference tables (reused for the count).

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)** for all 3 stories.

---

## Wave: DISCUSS / [REF] Risks

See `wave-decisions.md` §"Risks logged" (R-PC-1..R-PC-9). Headlines: no DIVERGE (low/low —
J-003b validated, counter-flag family + landing summary + slice-18 own mirror shipped); a failed
countered-peer-count read 5xxes / blanks the surface (medium/high — independent graceful degrade
is a hard commitment); the count becomes an N+1 (low/medium — fixed aggregate read); a
multiply-countered peer claim is double-counted / reads as a "by N" total (low/high — presence
count, counted once); the count re-weights / reads as a penalty (low/medium — peer-claims count
unchanged, neutral shared helper); landing/header drift (low/medium — single source); the
`/peer-claims` header re-orders the list (low/medium — additive); scope creep to a third
dimension / re-touching the slice-18 own count (low/low — peer-only, own untouched); the xtask
anti-merging rule with `peer_claims` in the outer FROM (low/medium — DESIGN verifies, expected
GREEN — single-table SELECT, no merging JOIN/GROUP BY).

---

---

## Wave: DESIGN (lean) — 2026-06-10 · Owner: Morgan (nw-solution-architect) · ADR-056

The deferred peer sibling of slice-18, designed as the EXACT mirror of ADR-055 onto peer claims.
Artifacts: `design/architecture-design.md` (C4 L1+L2), `design/component-boundaries.md` (the crate
touch map + the R-PC-9 xtask verification + the fault-seam decision), `design/technology-stack.md`
(unchanged), `design/data-models.md` (the 5th `LandingSummary` field + the reused render), and
`docs/adrs/ADR-056-*.md`.

### Resolutions

- **WD-PC-5 RESOLVED → count-only aggregate.** `count_countered_peer_claims() -> Result<usize,
  StoreReadError>` — the 5th count-only sibling on `StoreReadPort`. `adapter-duckdb` impl = the
  EXACT slice-18 SQL with outer `claims c → peer_claims p`:
  `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM
  claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references
  WHERE ref_type='counters')`. Inner IN-set byte-identical to slice-18; presence-once
  (de-duped UNION + `COUNT(DISTINCT)`); peer-only by the `peer_claims` outer table; parameter-free
  → injection-safe; invariant to store size.
- **R-PC-9 RESOLVED → GREEN by construction, VERIFIED.** Against `classify_sql_literal`:
  `mentions_peer_claims` TRUE (outer `FROM peer_claims p`), `mentions_own_claims` FALSE (no
  standalone `claims` whole-word — `peer_claims` is preceded by `_`; `claim_references` has no
  `claims` substring), so `is_cross_store = TRUE && FALSE = FALSE` → `None`, no violation.

### Decisions (the slice-18 pattern, mirrored)

- D2 — a 5th additive `Option<usize>` `countered_peer_claims` on `LandingSummary` (pure render now
  a total fn over 2⁵ combinations; independent `.ok()` degrade).
- D3 — REUSE the slice-18 `render_countered` helper (NO new helper): rendered on the landing PEER
  line ("4 peer claims (1 countered)") + the `/peer-claims` header ("Peer Claims (1 countered)"),
  single source (WD-PC-8). The slice-18 OWN surfaces UNTOUCHED (WD-PC-7).
- D4 — a 4th DISTINCT fault-seam token `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` (cfg-gated,
  appended to the xtask `VIEWER_FAIL_SEAM_TOKENS` guard) so the peer count fails INDEPENDENTLY of
  the own count.
- Boundary: NO new crate (workspace stays 21), NO new route, NO new render helper, NO mutation
  method, NO network seam, nothing persisted; read-only on the existing `StoreReadPort`.

### Quality gates (DESIGN)

Requirements traced to components · component boundaries with clear responsibilities · ADR-056 with
2+ alternatives per decision + rejection rationale · ISO 25010 attributes addressed (reliability /
correctness / performance / security / maintainability / portability) · dependency-inversion
(ports/adapters, deps inward) preserved · C4 L1+L2 in Mermaid · no new external integration (no
contract-test annotation) · OSS-only (MIT/Apache-2.0) · architectural enforcement recommended +
in-place (the three xtask layers + the fault-seam guard) · AC behavioral. No contract-test
annotation applies (the only dependency is the LOCAL read-only store).

---

## Changelog

- 2026-06-10 — slice-19 (`viewer-peer-counter-aware-counts`) DESIGN (Morgan). ADR-056. Resolved
  WD-PC-5 (count-only `count_countered_peer_claims` aggregate — the slice-18 SQL with outer
  `peer_claims`) + R-PC-9 (xtask anti-merging GREEN by construction, verified against
  `classify_sql_literal`). 5th additive `Option<usize>` `LandingSummary` field; REUSED
  `render_countered` (no new helper); 4th distinct fault-seam token. No new crate/route; workspace
  stays 21.
- 2026-06-10 — slice-19 (`viewer-peer-counter-aware-counts`) DISCUSS. Traces to J-003b (the
  ORIENTATION / at-a-glance-count facet of counter-claim awareness — the SAME job slice-18
  realized for own claims). 3 stories (1 infra + 2 user-visible). The deferred peer sibling of
  slice-18 (WD-CC-7). EXTENDS the slice-17 `GET /` landing PEER line + the slice-06/07
  `GET /peer-claims` list header (NO new route); reuses the slice-12 counter-reference data
  (`claim_references ∪ peer_claim_references`, `ref_type='counters'`) + the slice-18
  `render_countered` helper + count-only-aggregate + fault-seam patterns for the
  countered-peer-claims count — NO new read method (OPEN DESIGN QUESTION WD-PC-5: count-only
  aggregate `count_countered_peer_claims` — the slice-18 mirror with outer table `peer_claims` —
  vs `counter_presence_for(peer_cids).len()`); renders "(N countered)" beside the peer-claims
  count on both surfaces (single source — WD-PC-8). CARDINAL decisions: read-only / no-key
  (WD-PC-1); LOCAL-only / offline + independent graceful degrade (WD-PC-2); cheap / no-N+1 / a
  5th count-only sibling, landing budget grows by exactly 1 (WD-PC-3); presence count, never a
  "by N" total / re-weight / verdict (WD-PC-4). SCOPE: own+peer COMPLETION — JUST the peer count,
  no third dimension, the slice-18 own surfaces UNTOUCHED (WD-PC-7 / BR-PC-4). Missing≠zero
  (WD-PC-6); anti-misread neutral copy via the shared helper (WD-PC-10); additive on
  `/peer-claims` — no list regression (WD-PC-9). NO new crate (workspace stays 21), no new KPI
  ID, no new render helper. Scope PASS (~0.5–1 day). DoR PASS (9/9 for all 3 stories;
  Dimension-0 PASS; JTBD PASS).
