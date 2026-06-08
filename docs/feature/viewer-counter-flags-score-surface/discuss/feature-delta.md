<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-flags-score-surface

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (a DELTA extending the slice-12/13 "Countered" presence flag to the LAST remaining local viewer surface — `/score`)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-CF-002 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/09/10/11/12/13)
> JTBD: YES — every non-`@infrastructure` story traces to **J-003b** (`docs/product/jobs.yaml`, sub-job of J-003); no new job created. The DECORATED surface serves **J-002c** (transparent, auditable weighting), which slice-14 must NOT disturb.
> Brownfield DELTA on: `viewer-contributor-scoring` (slice-09 — the `/score` surface, `ScoreState`, the `WeightedView` projection, the sum-to-weight CARDINAL), `viewer-counter-claim-threads` (slice-11 — thread + `COUNTERED_PRESENCE_FLAG`), `viewer-counter-claim-list-flags` (slice-12 — the `counter_presence_for` batch read + `from_row_with_presence` + `render_countered_link` pattern, all REUSED), `viewer-counter-flags-graph-surfaces` (slice-13 — the immediately-preceding flag slice; this slice mirrors its DISCUSS structure and reuses the slice-13-unified `render_countered_link` SSOT)
> Date: 2026-06-08 · Owner: Luna (nw-product-owner)
> Slice: slice-14

This file is the canonical DISCUSS-wave delta for `viewer-counter-flags-score-surface`
(slice-14): the explicitly-deferred slice-13 follow-up. slice-11 shipped the counter
THREAD on `/claims/{cid}`; slice-12 shipped the at-a-glance "Countered" presence flag on
the `/claims` own-claims LIST; slice-13 extended it to `/peer-claims` + the `/project` +
`/philosophy` traversal edges. slice-14 extends that SAME neutral "Countered" presence
flag to the LAST remaining viewer surface — the read-only `GET /score?contributor=<did>`
contributor-scoring surface (slice-09). This COMPLETES the at-a-glance facet of **J-003b**
across every local viewer surface.

This is a DELTA. It REUSES, verbatim and with NO new read method, the slice-12
`StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` batch presence read
(ADR-048, the same one slice-13 reused across three handlers), the slice-13-unified
`render_countered_link(cid, is_countered)` SSOT + the `COUNTERED_PRESENCE_FLAG = "Countered"`
neutral marker, the slice-12 `from_row_with_presence` effect-shell projection pattern, and
the slice-09 `page = chrome + fragment` render pattern. The `/score` handler collects the
page's contribution CID set and calls the EXISTING `counter_presence_for` ONCE per render
(no N+1, no new read). Tier-1 content is inlined here (lean); SSOT lives under
`docs/product/`; per-journey/registry artifacts under `discuss/`.

---

## What makes slice-14 DIFFERENT from slices 12/13 (the LOAD-BEARING nuance)

`/score` carries **SCORING SEMANTICS**. Slices 12/13 flagged neutral list rows / edges; on
`/score` the flag sits BESIDE a weight, a confidence, a bonus, and a subtotal inside a ranked
breakdown whose subtotals sum to a headline contributor weight. Two things follow:

1. **The slice-09 CARDINAL must be PRESERVED (`sum-to-weight`).** slice-09 renders the
   contributor's transparent adherence score as `WeightedPairing`s (per subject-object
   pairing), each decomposed into per-claim `Contribution` rows (author DID + CID + verbatim
   confidence + author-distinct bonus + cross-project-triangulation bonus + subtotal). The
   per-claim subtotals sum to the displayed pairing weight **BY CONSTRUCTION** — both project
   the SAME `WeightedPairing` (`render_score_breakdown`, viewer-domain ~line 1968). The viewer
   NEVER recomputes confidence / bonuses / buckets; it PROJECTS the reused `scoring::WeightedView`.
2. **slice-14's NEW guarantee: the flag is provably ORTHOGONAL to the score.** Adding the
   "Countered" flag must NOT change any displayed weight, confidence, bonus, subtotal, total,
   ranking, or row order. The counter is **SHOWN, never APPLIED/subtracted.** A countered claim
   **still contributes its full original weight** to the contributor's score. The scoring math
   is intentionally counter-agnostic: disagreement is surfaced for the reader to JUDGE, not
   auto-applied to weight.
3. **Anti-misread copy is a first-class deliverable.** A reader must not misread the flag as
   "this counter lowered the contributor's score." The flag's plain-language meaning is
   unambiguous: *this claim has been disagreed with elsewhere* — orthogonal to the scoring math.
   This is an explicit AC (AC-CF-002-anti-misread) AND a KPI (see Outcome KPIs).

This is the ONLY reason `/score` was carved out of slice-13 into its own slice: it is a
structurally-different ADT (the slice-04 `WeightedView`), it sits beside scoring math, and it
carries a genuine weight-misread risk that needs deliberate anti-misread copy + the sum-to-weight
cardinal re-asserted as an explicit AC.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003b at ~line 253; J-002c at ~line 150 — the surface slice-14
  decorates; the slice-13 changelog deferral note at ~line 731 carving `/score` to slice-14)
- ✓ `docs/feature/viewer-contributor-scoring/discuss/wave-decisions.md` (slice-09: WD-CS-4
  transparency/breakdown load-bearing, WD-CS-6 pure-scorer reuse, WD-CS-7 verbatim
  confidence/weight, the sum-to-weight CARDINAL, J-002c)
- ✓ `docs/feature/viewer-counter-flags-graph-surfaces/discuss/*` (slice-13 — the immediately-
  preceding flag slice; this DISCUSS mirrors its structure: `feature-delta.md`,
  `user-stories.md` with System Constraints C-1..C-8, `acceptance-criteria.md`,
  `dor-checklist.md`, `outcome-kpis.md`, `wave-decisions.md`)
- ✓ `docs/feature/viewer-counter-claim-list-flags/discuss/*` (slice-12 — the
  `counter_presence_for` batch read + the `/claims` flag + `from_row_with_presence`)
- ✓ `crates/ports/src/store_read.rs` (`counter_presence_for(&[String]) -> HashSet<String>`
  slice-12 / ADR-048 — CONFIRMED present + REUSABLE; `query_contributor_scoring_feed` — the
  slice-09 LOCAL scoring-feed read, claims ∪ local peer_claims, NO network)
- ✓ `crates/viewer-domain/src/lib.rs` (`ScoreState::Scored { view: scoring::WeightedView }`
  ~line 1819; `render_score_results_fragment` ~1853; `render_score_pairing` ~1940;
  `render_score_breakdown` ~1968 — one row per `Contribution`, the flag's host; the
  sum-to-weight "BY CONSTRUCTION" doc-comment at ~1935; `COUNTERED_PRESENCE_FLAG` ~679;
  `render_countered_link` SSOT (slice-13-unified) — the marker to REUSE)
- ✓ `crates/adapter-http-viewer/src/lib.rs` (`score_page` ~489 / `resolve_score_state` ~507 →
  `query_contributor_scoring_feed` → `ScoreState::Scored { view }`; the effect shell where the
  presence read is wired + the projection set)
- ✓ `crates/scoring/src/explain.rs` (`Contribution.cid: Cid` — confirms each /score contribution
  carries a CID; the flag key)
- ✓ `docs/product/kpi-contracts.yaml` (KPI-FED-3 / KPI-VIEW-1/2 / KPI-VIEW-3 / KPI-AV-2 /
  KPI-GRAPH-2/3/4 / KPI-4 / KPI-5 / KPI-HX-G1/2/3)
- ✓ `docs/product/personas/senior-engineer-solo-builder.yaml` (P-001 + the slice-11
  counter-claim-reader hat + the slice-12/13 counter-claim-scanner hat)
- ⊘ `docs/feature/viewer-counter-flags-score-surface/diverge/` (no DIVERGE wave — noted as a
  non-blocking risk; J-003b is validated and slice-13 explicitly recommended this slice with
  its scope + the anti-misread requirement)

No DISCUSS decision below contradicts prior-wave evidence: J-003b is validated; the deferral
was explicitly recommended BY slice-13 as `slice-14 (viewer-counter-flags-score-surface)`;
this slice executes that recommendation with the sum-to-weight cardinal re-asserted and
anti-misread copy added.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona as
slices 06–13 (`docs/product/personas/senior-engineer-solo-builder.yaml`). She runs
`openlore ui` to glance at her store, navigate without reloads, read transparent scores
(`/score`, slice-09), traverse the graph (slice-10), read counter threads (slice-11), and
spot countered claims on her own-claims list (slice-12), the federated peer list, and the
traversal edges (slice-13). slice-14 extends the SAME counter-claim-scanner facet to the LAST
surface she scans: the contributor-scoring breakdown — so disagreement is discoverable on the
scoring surface too, WITHOUT the flag ever touching a weight, confidence, bonus, subtotal,
rank, or row order.

UX guardrails inherited verbatim: read-only, never silently mutate, attribution always visible
(no merged "disputed by N"), confidence/weight display must NEVER read as "the system thinks
this is true," and a counter must NEVER re-order / re-rank / re-weight / re-group / subtract on
any surface. On `/score` the last guardrail is LOAD-BEARING: the flag must be provably
orthogonal to the score.

### Counter-claim-scanner hat — extended to the scoring surface (slice-14)

P-001 wearing the counter-claim-scanner hat (slice-12/13) is now reading a contributor's
SCORE breakdown and wants to instantly spot WHICH of that contributor's contributions have
drawn disagreement — so she can decide which to scrutinize before trusting the score — WITHOUT
the flag ever changing any weight, confidence, bonus, subtotal, total, ranking, or row order,
and WITHOUT reading the flag as a deduction from the score.

- **Load-bearing anxieties**: "Do I have to open every contribution's `/claims/{cid}` thread
  to find out which were countered?" · "Does this 'Countered' tag mean the counter LOWERED this
  contributor's weight / score?" (it does NOT) · "Will the flag silently re-rank or re-order the
  breakdown, or change a subtotal?" (it must NOT) · "Do the subtotals still sum to the weight
  with the flag present?" (they MUST — slice-09 CARDINAL).
- **Load-bearing signals of success**: "I can see at a glance, in the breakdown, which
  contributions are contested — and the flag links me straight to that claim's thread." · "A
  contribution with no counter looks EXACTLY like it did in slice-09 — no badge, no noise." ·
  "Every weight, confidence, bonus, subtotal, total, the ranking, and the row order are
  byte-identical to slice-09; the subtotals still sum to the weight." · "The flag's copy makes
  it unmistakable that being countered does NOT subtract from the score — it's a 'someone
  disagreed here, go judge it' marker, not a score input."

> This DISCUSS wave appends the slice-14 facet (the `/score` surface + the anti-misread copy)
> to the slice-12/13 counter-claim-scanner hat in
> `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-08, slice-14).
> It EXTENDS the existing hat (one more surface) rather than minting a new hat.

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a
> counter-claim that stands on its own — not a reply on their record — so disagreement is a
> public structured artifact rather than a thread.*
> (`docs/product/jobs.yaml`, sub-job of **J-003**, opportunity score 15.)

slice-14 realizes the LAST surface of the AT-A-GLANCE / DISCOVERABILITY facet of the VIEWING
side of J-003b. slice-11 made disagreement legible once you OPEN a claim; slice-12 made it
discoverable on your own claims list; slice-13 on the federated peer list + traversal edges.
But the operator also reads CONTRIBUTOR SCORES (`/score`, slice-09) to decide whether to trust
a contributor's adherence — and on that surface a countered contribution is still invisible
until she opens it. slice-14 closes that last gap, completing the at-a-glance facet across
EVERY local viewer surface.

The decorated surface (`/score`) serves **J-002c** (transparent, auditable, reproduce-by-hand
weighting). slice-14 must DECORATE J-002c without disturbing it: the flag never changes the
weight, the breakdown, the sum-to-weight, the buckets, or the ranking. No new job. No new
sub-job. Every non-`@infrastructure` story traces to J-003b.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Every user-visible story → J-003b. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | J-003a (attribute every peer claim without merging) is HONORED — the flag is presence-only (a boolean per contribution CID), never a merged "disputed by N"; per-counter attribution lives in the slice-11 thread the flag links to. J-003c (revocable subscription) is untouched — purging a peer removes its counters from the presence read by construction. |
| **No contradiction with J-002c (the SCORING job slice-14 decorates)?** | **PASS** | The flag is ADDITIVE beside each contribution. It NEVER changes any weight, confidence, bonus, subtotal, headline total, bucket, the ranking, or the row order. The slice-09 sum-to-weight CARDINAL holds (subtotals still sum to the displayed weight — both project the SAME `WeightedPairing`, unchanged). The reproduce-by-hand transparency (KPI-GRAPH-3) is UNCHANGED. The counter is SHOWN, never APPLIED. |
| No contradiction with cardinal invariants? | PASS | Shown-never-applied (ADR-015 / slice-11 I-CT-2 / slice-12 I-LF-2 / slice-13 I-CF-2) HONORED — the flagged contribution renders verbatim; the flag NEVER re-ranks / re-weights / subtracts / re-orders. Read-only (KPI-VIEW-2), anti-merging (KPI-AV-2 / KPI-GRAPH-2), verbatim confidence/weight (KPI-4 / WD-CS-7), local-first (KPI-5) all carry forward. |
| Authoring NOT re-introduced on the viewer? | PASS | This slice adds ZERO write/sign/counter controls. Authoring stays EXCLUSIVELY in the CLI (`claim counter`). The flag only RENDERS the presence of counters that already exist; it links to the slice-11 read-only thread, never to a compose form. |
| No-regression on the slice-09 `/score` surface? | PASS (commitment) | The flag is ADDITIVE. The ranking, the per-pairing weight, every confidence/bonus/subtotal, the `[SPARSE]` honesty line, and the row order are UNCHANGED — byte-identical to the slice-09 render with the markers elided. |
| Job already fully served? | NO (gap is real) | slices 12/13 serve `/claims`, `/peer-claims`, `/project`, `/philosophy`. The `/score` surface shows NO indication of which contributions are countered. slice-13 explicitly deferred it to slice-14. |

The gate PASSES. The slice is a coherent, single-job, non-contradicting DELTA that DECORATES
J-002c without disturbing it.

---

## Wave: DISCUSS / [REF] JTBD trace (every story → J-003b, with boundaries)

| Story | Title | job_id | Sub-job realized | Boundary note |
|---|---|---|---|---|
| US-CF-001 | Reuse the slice-12 batch counter-presence read in the `/score` handler (collect contribution CIDs → ONE `counter_presence_for` call → thread the presence set into the pairing-row projection) | `infrastructure-only` | (enables US-CF-002) | `infrastructure_rationale` below. NO new read method. NOT a J-003a/c story. |
| US-CF-002 | See a neutral "Countered" flag on each `/score` contribution row whose claim has ≥1 counter, with copy that makes it unmistakable the flag is orthogonal to the score, and the slice-09 sum-to-weight CARDINAL preserved | J-003b | J-003b (at-a-glance facet, SCORING surface) | DECORATES J-002c — never changes a weight/confidence/bonus/subtotal/total/rank/order; the flag is SHOWN, never APPLIED. NOT the authoring half (slice-03 CLI); NOT the thread (slice-11). |

**J-003a / J-003b / J-003c boundary statement:**

- **J-003a** (attribute every peer claim without merging) is HONORED: the `/score` flag is
  PRESENCE-ONLY (a boolean per contribution CID — "this claim has ≥1 counter"), NOT a count and
  NEVER a merged "disputed by N". Per-counter attribution (each counter's author DID + CID +
  verbatim reason) lives in the slice-11 THREAD the flag LINKS to.
- **J-003b** (counter-claim as first-class disagreement) is THIS slice's job — the at-a-glance
  facet of the VIEWING half, on the SCORING surface. The AUTHORING half (`claim counter` CLI)
  shipped in slice-03; the drill-in thread in slice-11; the list/peer/edge flags in slices
  12/13. All are OUT of scope here.
- **J-003c** (subscription revocable without residue) is untouched. A purged peer's counters
  vanish from the presence read because they lived in `peer_claim_references`.

### Infrastructure rationale (US-CF-001)

US-CF-001 carries `job_id: infrastructure-only` with this rationale: it adds NO new read
method. It WIRES the EXISTING slice-12 `counter_presence_for(&[String]) -> HashSet<String>`
(ADR-048) into the `/score` page handler (`score_page` / `resolve_score_state`): it collects
the page's contribution CID set (every `Contribution.cid` across every `WeightedPairing` in the
`ScoreState::Scored { view }`) and calls the method ONCE per render, then passes the presence
set into the pure pairing-row projection. It produces no user-visible output on its own (no new
route, no rendered marker — that is US-CF-002), so it enables a user decision only THROUGH
US-CF-002. The slice contains ONE non-infrastructure, user-visible story (US-CF-002), so the
slice has release value (Dimension-0 slice-level check passes).

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-14 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-9). Summary table:

| ID | Commitment | Source |
|---|---|---|
| I-CF-1 (= I-VIEW-1/2/3) | **Read-only**: the `/score` surface holds `StoreReadPort` only; no mutation method; no signing key; no write/sign/counter control. Authoring stays CLI-only. | KPI-VIEW-2, slice-06–13 |
| I-CF-2 (= ADR-015 / slice-13 I-CF-2) | **Shown, never applied**: the flag is a neutral presence marker; the flagged contribution renders VERBATIM (confidence, bonuses, subtotal, weight, bucket, rank, position all unchanged); the flag NEVER re-orders / re-ranks / re-weights / subtracts / filters. | ADR-015, slice-11/12/13 |
| I-CF-3 (= KPI-AV-2 / KPI-GRAPH-2) | **Presence-only, no invented / no merged flag**: a contribution is flagged ONLY if a real `ref_type='counters'` reference to its CID exists (claims ∪ peer_claims); a boolean per contribution, never a count or "disputed by N"; attribution deferred to the slice-11 thread. | KPI-FED-1/2, KPI-GRAPH-2, slice-03/04/11/12/13 |
| I-CF-4 (= KPI-4 / WD-CS-7) | **Verbatim confidence / weight / bonuses / subtotal**: every confidence (`0.90`), bonus, subtotal, and headline weight renders exactly as slice-09 via the single existing render site — UNCHANGED by the flag. | KPI-4, slice-09/10/12/13 |
| I-CF-5 (= KPI-5 / KPI-VIEW-5) | **LOCAL-only / offline**: the presence read is the LOCAL indexed `claim_references ∪ peer_claim_references` lookup (no per-row artifact read; no network); the `/score` route renders fully offline (slice-09 WD-CS-8) and references only the vendored local htmx asset (no CDN). | KPI-5, slice-09 WD-CS-8 |
| I-CF-6 (= slice-09 WD-CS-9) | **Progressive enhancement + parity**: an `HX-Request` returns the score fragment (with flags); a no-JS / direct-URL request returns the full page = chrome + the SAME fragment. The flag is in the SAME fragment fn (`render_score_results_fragment`) both shapes embed. | slice-07 KPI-HX-G1/G2/G3, slice-09 WD-CS-9 |
| I-CF-7 | **No new crates, NO new read method, NO new route**: extend PURE `viewer-domain` + EFFECT `adapter-http-viewer` + `xtask`; REUSE the slice-12 `counter_presence_for` read + the slice-13 `render_countered_link` SSOT. Workspace stays 21. Functional (ADR-007). | slice-06–13 |
| I-CF-8 (= ADR-048) | **Batch presence read, NOT N+1 — REUSED**: the `/score` handler collects the page's contribution CID set (every `Contribution.cid` across every `WeightedPairing`, flattened once) and calls `counter_presence_for` ONCE per render (one aggregate query, invariant to contribution/pairing count). | slice-12 ADR-048, slice-13 |
| **I-CF-9 (NEW, slice-14 CARDINAL)** | **Sum-to-weight preserved + score-orthogonal**: adding the flag changes NO weight, confidence, bonus, subtotal, headline total, bucket, ranking, or row order; the slice-09 per-claim subtotals STILL sum to the displayed pairing weight; the counter is SHOWN, never APPLIED/subtracted — a countered claim contributes its FULL original weight. Byte-identical to the slice-09 render with markers elided. AND the flag copy makes the orthogonality unmistakable (anti-misread). | **slice-09 sum-to-weight CARDINAL (WD-CS-4/6/7) + ADR-015 shown-never-applied** |

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

slice-14 does NOT, under any circumstance:

- **Apply / subtract / re-weight by the counter.** The counter is SHOWN, never APPLIED. A
  countered claim contributes its FULL original weight; no displayed weight, confidence, bonus,
  subtotal, headline total, bucket, ranking, or row order changes (I-CF-9 — the slice-14
  CARDINAL).
- **Recompute any scoring math.** The viewer PROJECTS the reused `scoring::WeightedView`
  (slice-04 / slice-09 WD-CS-6). slice-14 adds NO scoring logic; the flag is a render-only
  annotation read from the presence set.
- **Touch the slices 12/13 surfaces** (`/claims`, `/peer-claims`, `/project`, `/philosophy`) or
  the slice-11 `/claims/{cid}` thread or the slice-08 `/search` annotation — all shipped.
- **Author or compose a counter on the viewer.** No "counter / reply / dispute" button, form, or
  control. Authoring stays EXCLUSIVELY in the CLI `claim counter` verb (I-CF-1).
- **Re-rank, re-order, filter, hide, re-weight, or subtract on the breakdown by counter
  presence.** The ranking + the per-pairing weight + every confidence/bonus/subtotal + the
  `[SPARSE]` honesty line + the row order are UNCHANGED (I-CF-2 / I-CF-9).
- **Show a count, "net verdict", "consensus", "disputed score", or "X disagree" aggregate on the
  flag.** Every flag is PRESENCE-ONLY (boolean per contribution CID). Per-counter attribution +
  count lives in the slice-11 thread the flag LINKS to (I-CF-3).
- **Render any reason text on the flag.** No flag carries a `--reason` — the verbatim reasons are
  the slice-11 thread's job. The presence read needs NO per-row artifact read (a pure DB-index
  lookup, one aggregate query — I-CF-5 / I-CF-8).
- **Add any network seam to the `/score` route.** Counter presence is read from the LOCAL indexed
  ref tables only (slice-09 WD-CS-8 — `/score` is already a fully-offline LOCAL read + pure
  compute). (I-CF-5)
- **Add a new read method, new SQL, new route, or new crate.** slice-14 REUSES the slice-12
  `counter_presence_for` + the slice-13 `render_countered_link` verbatim; extends the existing
  `GET /score` route only; workspace stays 21 members (I-CF-7 / I-CF-8).
- **Issue one query per contribution (N+1).** The presence lookup is ONE batch aggregate query
  over the page's contribution CID set, per render (I-CF-8).

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey investment (Phase 1.5).

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 2 (1 infra-wiring + 1 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure), `adapter-http-viewer` (effect), `xtask` — all existing; NO new read method (REUSE slice-12), NO new flag render (REUSE slice-13 `render_countered_link`) | No (single context) |
| Integration points (new) | 0 new read methods (REUSE `counter_presence_for`); 0 new render functions (REUSE `render_countered_link`); 1 handler wiring (`score_page`/`resolve_score_state`) + 1 projection seam (contribution row gains `is_countered` via `from_row_with_presence` pattern) + 1 render-site arm in `render_score_breakdown` | No (≤5) |
| Estimated effort | ~1 day (REUSE the slice-12 read + the slice-13 render SSOT + the slice-12 projection pattern; one render-site arm; the only NEW work is the anti-misread copy + the sum-to-weight byte-identity gold) | No (≤2 weeks) |
| Independent user outcomes | 1 (spot which of a contributor's contributions are countered while reading the score, without the flag touching the score) | No |

**## Scope Assessment: PASS — 2 stories, 1 context, estimated ~1 day** (reuse-only: REUSE
`counter_presence_for` + `render_countered_link`; NO new crate / route / read-method; workspace
stays 21).

`/score` is the LAST viewer surface; after slice-14 the at-a-glance J-003b facet is complete
across all local viewer surfaces. The ONLY thing that distinguishes this from a slice-12/13
copy is the scoring-semantics nuance (sum-to-weight + anti-misread), which is handled as one
extra AC + one extra KPI + the anti-misread copy — not extra stories.

---

## Wave: DISCUSS / [REF] Proposed route + read method

- **Route**: EXTEND the existing `GET /score?contributor=<did>` (slice-09). NO new route. The
  score fragment now renders each contribution row (as today) PLUS a neutral "Countered" flag
  for contributions whose claim has ≥1 counter.
- **Read method (REUSED — NO new method)**: the `/score` handler (`resolve_score_state` →
  `score_page`) calls the EXISTING slice-12
  `StoreReadPort::counter_presence_for(target_cids: &[String]) -> Result<HashSet<String>, StoreReadError>`
  ONCE per render, after building the `ScoreState::Scored { view }`, with the page's contribution
  CID set (every `Contribution.cid` across every `WeightedPairing`, flattened once). Returns the
  SET of countered CIDs; the pure projection maps it onto contribution rows. ONE query per render,
  NEVER N+1 (I-CF-8). NO new SQL.
- **Pure projection (new seam, in `viewer-domain`)**: the pure render stays a TOTAL function of
  `(ScoreState, presence)` — mirror the slice-12/13 `from_row_with_presence` projection seam. The
  contribution-row render (`render_score_breakdown`, ~line 1968) gains a flag arm that emits the
  slice-13 `render_countered_link(contribution.cid.0, is_countered)` — REUSED verbatim — only when
  the contribution's CID is in the presence set. An un-countered contribution renders exactly as
  slice-09.
  > DESIGN owns the precise projection shape (a presence-aware wrapper around the breakdown render
  > vs. a per-contribution bool threaded through), AND whether the presence set is passed into the
  > pure render alongside the `ScoreState` or pre-applied into a flagged view-model. The PRODUCT
  > contract is the AC in `user-stories.md` + `acceptance-criteria.md`. The flag text + the
  > `<a href="/claims/{cid}">` one-hop pattern REUSE the slice-13 `render_countered_link` /
  > `COUNTERED_PRESENCE_FLAG` — single source of truth, no new string.

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-CF-001 | Reuse the slice-12 `counter_presence_for(&[cid])` batch read in the `/score` handler — collect all contribution CIDs across all pairings → ONE aggregate query per render → thread the presence set into the pairing-row projection; no N+1, NO new read method | infrastructure-only |
| US-CF-002 | See a neutral "Countered" flag on each `/score` contribution row whose claim has ≥1 counter, linking to that claim's slice-11 thread, with copy that makes it unmistakable the flag is orthogonal to the score (shown, never applied), the slice-09 sum-to-weight CARDINAL preserved, and every weight/confidence/bonus/subtotal/total/rank/order byte-identical to slice-09 | J-003b |

---

## Wave: DISCUSS / [REF] User stories with elevator pitches + AC

<!-- Full story bodies live in user-stories.md; elevator pitch + key AC themes summarized here. -->

### US-CF-001 — Reuse the batch counter-presence read in the `/score` handler (`@infrastructure`)

`@infrastructure` — no Elevator Pitch (produces no user-visible output; enables US-CF-002). It
WIRES the EXISTING slice-12 `counter_presence_for(&[cid])` into the `/score` handler: it collects
the page's contribution CID set (every `Contribution.cid` across every `WeightedPairing` in the
`ScoreState::Scored { view }`) and calls the method ONCE per render. NO new read method, NO new SQL.

**Key AC themes**: the handler calls `counter_presence_for` exactly ONCE per render (driving route
`GET /score?contributor=<did>`); it flattens all contribution CIDs across all pairings into ONE
call; the query count is invariant to contribution/pairing count (the inherited N+1 guard); an
empty / all-un-countered score resolves to an empty set with no query; the `NoClaims` / `Form`
arms issue NO presence query; the existing `query_contributor_scoring_feed` read + the scoring
math + ranking are UNCHANGED; no new method added to `StoreReadPort`.

### US-CF-002 — At-a-glance "Countered" flag on `/score` contribution rows (score-orthogonal)

**Elevator Pitch**
- Before: Maria reads a contributor's `/score` breakdown and cannot tell which of their
  contributions have been countered without opening each `/claims/{cid}` thread one-by-one — and
  she has no in-context signal of where disagreement exists before trusting the score.
- After: open `http://127.0.0.1:<port>/score?contributor=did:plc:t0bi` → each contribution row
  whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread;
  un-countered rows show nothing; every weight, confidence, bonus, subtotal, the headline total,
  the ranking, and the row order are byte-identical to slice-09 (the subtotals still sum to the
  weight); the marker's copy makes it unmistakable that being countered does NOT lower the score.
- Decision enabled: Maria decides WHICH of a contributor's contributions to scrutinize (open the
  disagreement on) before trusting their adherence score — without misreading the flag as a score
  deduction and without the breakdown silently re-ranking or re-weighting for her.

**Key AC themes** (driving route `GET /score?contributor=<did>`, both shapes): a countered
contribution row shows the `COUNTERED_PRESENCE_FLAG` marker via the REUSED `render_countered_link`;
the marker is a render-only `<a href="/claims/{cid}">` one-hop link to the slice-11 thread; the
flag renders identically under htmx fragment + no-JS full page (parity); neutral presence text,
never a verdict or count; **the slice-09 sum-to-weight CARDINAL holds (per-claim subtotals still
sum to the displayed pairing weight)**; **every weight/confidence/bonus/subtotal/headline-total +
the ranking + the row order are byte-identical to slice-09 with markers elided (shown, never
applied)**; **the flag's copy makes the score-orthogonality unmistakable (anti-misread)** — a
countered claim contributes its FULL original weight; a contribution countered by N authors shows
ONE marker (presence-only).

---

## Wave: DISCUSS / [REF] Outcome KPIs

See `outcome-kpis.md`. slice-14 mints **NO new KPI ID** (matching slice-08–13). It REALIZES
inherited KPIs on the LAST facet (the scoring-surface contribution flag) and adds one slice-specific
guardrail KPI for the score-orthogonality / sum-to-weight invariant (a realization of KPI-GRAPH-3 +
the slice-09 cardinal, not a new contract ID). North star realized: KPI-FED-3 (the READ side of the
J-003b loop, now on the scoring surface). Guardrails MET: KPI-VIEW-2 (read-only), KPI-AV-2 /
KPI-GRAPH-2 (anti-merging), KPI-GRAPH-3 (reproduce-by-hand / sum-to-weight UNCHANGED), KPI-4
(verbatim), KPI-5 / KPI-HX-G* (local-first / offline / parity).

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the `/score`
route + `ScoreState` + `WeightedView` projection + `render_score_breakdown`, the read-only store
port, the `counter_presence_for` batch read (slice-12), the `render_countered_link` SSOT
(slice-13), the `from_row_with_presence` projection pattern, and the `page = chrome + fragment`
render pattern all already exist (slices 03/06/07/09/11/12/13). The thinnest end-to-end slice IS
US-CF-002 (the flag render on the existing `/score` route), backed by US-CF-001 (wiring the
existing read). Delivery sequence: US-CF-001 → US-CF-002. Demonstrable in a single session against
the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Journey (visual + emotional arc + HTML mockups): `journey-counter-flags-score-surface-visual.md`
- Journey schema (Gherkin embedded per step): `journey-counter-flags-score-surface.yaml`
- Shared-artifact registry: `shared-artifacts-registry.md`

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)**.

---

## Wave: DISCUSS / [REF] Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| No DIVERGE wave for slice-14 | Low | Low | The job (J-003b) is already validated; slice-13 explicitly recommended this slice with its scope + the anti-misread requirement. No design-direction ambiguity — the flag is the same vetted neutral marker REUSED from slices 11/12/13. Non-blocking risk. |
| **Flag misread as a score deduction ("countered → lower weight")** | **Medium** | **High** | **The slice-14 CARDINAL (I-CF-9): the flag is SHOWN, never APPLIED; a countered claim keeps its FULL original weight. Anti-misread COPY is an explicit AC (AC-CF-002-anti-misread) + a guardrail KPI. The sum-to-weight byte-identity gold (markers elided) proves the score is unchanged.** |
| **Sum-to-weight regression (flag silently changes a subtotal / weight / ranking)** | **Medium** | **High** | **I-CF-9 + US-CF-002 AC require byte-identity of every weight/confidence/bonus/subtotal/total + the ranking + the row order with markers elided (the slice-12/13 baseline+marker-elision tactic). The subtotals-sum-to-weight property is re-asserted as an explicit AC on the flagged render.** |
| N+1 query regression on the breakdown | Medium | High | I-CF-8 makes the single-batch-call-per-render a HARD product commitment; US-CF-001 AC requires flattening ALL contribution CIDs across all pairings into ONE call; a behavioral query-count test + the inherited slice-12 adapter N+1 property. |
| Scope creep (recomputing the score, adding a "disputed score") | Low | Medium | Explicit out-of-scope + I-CF-2/3/9; the flag is presence-only, render-only, and the score math is the REUSED pure core (no viewer scoring logic). |
| Own-arm (`claim_references`) coverage gap | Low | Low | Inherited slice-12/13 lesson: the own arm of the `counter_presence_for` UNION-ALL is covered at the adapter unit level; the e2e path exercises the peer arm. No new adapter work — the read is REUSED. |

---

## Changelog

- 2026-06-08 — slice-14 (`viewer-counter-flags-score-surface`) DISCUSS. Traces to J-003b (the
  at-a-glance facet of the VIEW half, extended to the LAST viewer surface — the `/score`
  contributor-scoring breakdown; authoring stays the slice-03 CLI, drill-in thread is slice-11,
  list/peer/edge flags are slices 12/13). 2 stories (1 infra-wiring + 1 user-visible). REUSES the
  slice-12 `counter_presence_for(&[cid])` batch read + the slice-13 `render_countered_link` SSOT +
  the `COUNTERED_PRESENCE_FLAG` constant VERBATIM — **NO new read method, NO new render fn, NO new
  route, NO new crate (workspace stays 21), NO new KPI ID**. The LOAD-BEARING slice-14 nuance: the
  flag must be provably ORTHOGONAL to the score — the slice-09 sum-to-weight CARDINAL is preserved
  (subtotals still sum to weight; every weight/confidence/bonus/subtotal/total/rank/order
  byte-identical), the counter is SHOWN never APPLIED (a countered claim keeps its full weight), and
  anti-misread COPY is an explicit AC + guardrail KPI. Scope PASS (~1 day). DoR PASS (9/9).
