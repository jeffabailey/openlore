<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-aware-counts

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (an EXTENSION of the slice-17 `GET /` landing summary + the slice-06 `GET /claims` list header on the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (NO walking-skeleton Feature 0); the thinnest end-to-end slice is US-CC-001 itself
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/17)
> JTBD: YES — both user-visible stories trace to **J-003b** (counter-claim awareness — the ORIENTATION / at-a-glance-count facet); the enabling read-wiring story is `infrastructure-only` with rationale; no new job/sub-job created
> Brownfield DELTA on: `viewer-landing-dashboard` (slice-17 — the `LandingSummary` Option-shaped counts, `render_count`, `MISSING_COUNT_MARKER`, the count-only `count_active_peer_subscriptions` precedent, the per-count `.ok()` independent degrade — this slice EXTENDS that summary), `viewer-counter-claim-list-flags` (slice-12 — `counter_presence_for` + the indexed `claim_references ∪ peer_claim_references` counter tables, `ref_type='counters'`), and `htmx-scraper-viewer` (slice-06 — the `/claims` list + its header, the read-only viewer foundation)
> Date: 2026-06-09 · Owner: Luna (nw-product-owner)
> Slice: slice-18

This file is the canonical DISCUSS-wave delta for `viewer-counter-aware-counts` (slice-18):
tying the shipped counter-flag family (slices 11–14) INTO the front-door orientation
(slice-17). It surfaces, at a glance, **how many of the operator's own claims have been
countered**, beside the own-claims count, on the landing ("12 own claims (3 countered)") and
in the `/claims` list header. A reader orienting at the front door immediately sees not just
how MUCH is in her store but how much has been DISPUTED, and can drill into the flagged rows
(slices 12–14) to read the disagreements.

This is an ENRICHMENT, but a grounded one: it connects the shipped counter-flag family to the
front-door orientation, completing the "see what's in my store" picture with "and what's been
disputed". It realizes the orientation facet of **J-003b** + **KPI-VIEW-1**
(time-to-see-store-contents — now including disputed-claim state).

It EXTENDS the existing `GET /` + `GET /claims` routes (NO new route). It reuses the slice-12
counter-reference data and the slice-17 `LandingSummary` shape. NO new read method (or, at
most, one count-only countered aggregate — an open DESIGN question). NO new crate; workspace
stays 21. Read-only / no-key / LOCAL / offline, like every viewer surface.

---

## SSOT reading confirmation (READING ENFORCEMENT)

- ✓ `docs/product/jobs.yaml` (J-003b "counter-claim authoring as first-class disagreement" at ~line 253 — the VIEW/legibility half realized as orientation here; the slice-11/12 changelog entries at ~lines 702/730 confirming the viewer realizes the J-003b VIEWING facet on P-001's browser surface)
- ✓ `crates/viewer-domain/src/lib.rs` (`LandingSummary` ~574 with `own_claims: Option<usize>`; `render_count` ~606 mapping `Some(n)`→n / `None`→`MISSING_COUNT_MARKER`; `MISSING_COUNT_MARKER` "—" ~562; `render_landing` ~624 rendering "(render_count(own_claims)) ' own claims'" ~637 — the EXACT site this slice extends to "… (N countered)"; `render_claims_page` ~373 — the "My Claims" `h1` + read-only `p` header this slice extends; `counter_presence_for` consumers + the `ClaimRowView` presence flags from slice-12)
- ✓ `crates/ports/src/store_read.rs` (`count_claims` ~296; `count_active_peer_subscriptions` ~333 — the COUNT-ONLY aggregate precedent (`COUNT(*) … WHERE removed_at IS NULL`) for a count-only countered aggregate; `counter_presence_for(&[String]) -> HashSet<String>` ~435 — the presence-subset read; NO mutation method on the trait — read-only by construction, I-VIEW-1)
- ✓ `crates/adapter-duckdb/src/store_read.rs` (`count_active_peer_subscriptions` ~498 (`SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL`) — the count-only impl precedent; `counter_presence_for` SQL ~735–755 (`referenced_cid IN (...)` UNION-ALL DISTINCT over `claim_references ∪ peer_claim_references`, `ref_type='counters'`) — the closest shape for a countered-own-claims aggregate)
- ✓ `docs/feature/viewer-landing-dashboard/discuss/` (slice-17 — the landing summary this slice extends: Option-shaped counts, `MISSING_COUNT_MARKER`, count-only-read decision (ADR-054 D3), per-count independent degrade (ADR-054 D2), full-page-only `GET /` (ADR-054 D5))
- ✓ `docs/feature/viewer-counter-claim-list-flags/discuss/` (slice-12 — `counter_presence_for`, the counter-reference tables, the presence-not-count/verdict invariant, the shown-never-applied + no-regression discipline this slice's `/claims` header inherits)
- ⊘ `docs/feature/viewer-counter-aware-counts/diverge/` (no DIVERGE wave for this slice — consistent with all prior OpenLore slices; noted as a non-blocking risk R-CC-1; J-003b is validated and the counter-flag family + landing summary are SHIPPED)

No DISCUSS decision below contradicts the prior-wave evidence: the viewer is read-only
(slices 06–17); `GET /` and `GET /claims` already exist; the counter-reference tables + the
slice-17 `LandingSummary` already exist; the self-counter rule means own claims are countered
by peers.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME persona as
slices 06–17 (`docs/product/personas/senior-engineer-solo-builder.yaml`). slice-18 gives her
the **counter-aware-orientation** hat: she opens `/` (or lands on `/claims`) and immediately
sees not just how much is in her store but how much of her own work has been disputed.

### Counter-aware-orientation hat (NEW for slice-18)

P-001 wearing this hat opens `/` to answer, in the first second, not only "What's in my
store?" and "Where can I go?" (slice-17) but also **"How much of my own work has been
disputed?"** — WITHOUT leaving the front door, and WITHOUT the count ever re-weighting her
claims or reading as a penalty.

- **Load-bearing anxiety** (from J-003b): "When a peer counters my claim, will I even know?
  Do I have to open each claim to find out how much of my work has drawn pushback?"
- **Load-bearing signals of success**: "The moment I open `/`, I see '12 own claims
  (3 countered)' — how much of mine has been disputed, at a glance." · "It says
  '(0 countered)' honestly when nothing of mine has drawn a counter." · "A claim countered by
  two peers counts once — it's awareness, not a 'disputed by N' score." · "The count never
  re-weights my claims; the '12' is unchanged." · "If the count can't be read, the rest of my
  summary still renders." · "It loads with the network down."

> This DISCUSS wave appends the slice-18 counter-aware-orientation hat to
> `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09, slice-18).

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-003b**: *When a peer publishes a claim I disagree with, I want to publish a
> counter-claim that stands on its own … so disagreement is a public structured artifact
> rather than a thread.* (`docs/product/jobs.yaml` ~line 253. The VIEW/legibility half of
> J-003b is realized across slices 11–14; slice-18 realizes its ORIENTATION / at-a-glance-
> count facet.)

slice-18 realizes the **ORIENTATION / AT-A-GLANCE-COUNT** facet of J-003b. The counter-flag
family (slices 11–14) made individual disagreements legible (thread on `/claims/{cid}`,
per-row flags on the lists). slice-17 made the store glanceable at the front door. slice-18
connects them: the operator sees, at the orientation surfaces, HOW MANY of her own claims
have drawn pushback — counter-claim awareness as a front-door orientation signal, not a
per-claim scan.

### JTBD scope / contradiction gate

| Gate check | Verdict | Evidence |
|---|---|---|
| Single job? | PASS | Both user-visible stories (US-CC-001/002) → J-003b (orientation facet). The infra story enables them. No story straddles two primary jobs. |
| No contradiction with sibling sub-jobs? | PASS | The count is a store-level PRESENCE count ("how many own claims are countered"), NOT a merge of authors, NOT a "consensus" — it never collapses per-author attribution (reading WHO countered WHAT stays the slice-11 thread + the slice-12 per-row flags). Reinforces J-003a (anti-merging) by linking OUT to the attributed surfaces. |
| No contradiction with cardinal invariants? | PASS | Read-only / no-key (I-VIEW-1/2/3, KPI-VIEW-2) HONORED — the count is read + rendered only. Local-first (KPI-5) HONORED — a LOCAL aggregate, no network. Shown-never-applied (J-003b accuracy) HONORED — the count is presence-only, never a re-weight/verdict; the own-claims count + confidences stay verbatim. |
| Count is a presence count, not a "by N" total or a re-weight? | PASS | A claim countered by N peers counts ONCE (WD-CC-4); the own-claims "12" is unchanged; the copy is neutral disputed-claim awareness (no penalty/score/verdict — WD-CC-10). |
| New route introduced? | NO (extends `GET /` + `GET /claims`) | slice-18 adds ZERO new routes. |
| New read method introduced? | NO (reuses existing) — or at most ONE count-only countered aggregate | `count_claims` + the counter-reference tables already exist; the countered count is either a count-only aggregate (recommended, mirroring `count_active_peer_subscriptions`) or `counter_presence_for(own_cids).len()` (OPEN DESIGN QUESTION WD-CC-5). |
| Job already fully served? | NO (the gap is real) | Today the front-door summary shows "how much is here" but not "how much has been disputed"; the operator must leave `/` and scan `/claims` to learn it. The counter-flag family is not connected to the front-door orientation. |

The gate PASSES. A coherent, single-job, non-contradicting extension of the orientation
surfaces.

---

## Wave: DISCUSS / [REF] Cardinal invariants carried forward (commitments)

RESTATED as binding commitments for slice-18 (inherited, not re-litigated). Full text in
`user-stories.md` §"System Constraints" (C-1..C-7). Summary table:

| ID | Commitment | Source |
|---|---|---|
| C-1 (= I-VIEW-1/2/3) — **CARDINAL** | **Read-only / no key**: the count is read + rendered only — no mutation method, no key, no write/compose/sign/subscribe/follow control; render-only text, never a sort/filter/mutating control. | KPI-VIEW-2, slice-06–17 |
| C-2 (= KPI-5 / KPI-VIEW-5) — **CARDINAL** | **LOCAL-only / offline + graceful degrade**: the countered count is a LOCAL aggregate over the indexed counter-reference tables; NO network. Renders offline (vendored htmx only). A failed read → the missing marker WITHOUT blanking the sibling counts/rows; never a 5xx. | KPI-5, KPI-VIEW-5, NFR-VIEW-6, slice-17 WD-LD-2 |
| C-3 — **CARDINAL** | **Cheap / no N+1**: a SMALL FIXED number of aggregate reads (ideally ONE count-only aggregate, or folded into the summary resolution), invariant to store size; the landing's 3-read budget grows by at most 1. NO per-claim loop. | slice-17 C-4, slice-12 I-LF-8 |
| C-4 — **CARDINAL (J-003b accuracy)** | **Presence count, never a total / re-weight / verdict**: how many own claims have ≥1 counter; a claim countered by N peers counts ONCE; the own-claims count is unchanged. | J-003b shown-never-applied (slices 11–14) |
| C-5 | **Missing ≠ zero**: Option-shaped; Some(0)→"(0 countered)", None→the slice-17 missing marker; no fabricated 0; independent degrade. | slice-17 WD-LD-8 / ADR-054 D2 |
| C-6 | **Anti-misread / neutral copy**: "(N countered)" is neutral disputed-claim awareness — no penalty/deduction/score/"refuted"/"false". | slice-14 anti-misread |
| C-7 | **No new crate; no new route; reuse the counter-reference data**: extend `viewer-domain` + `adapter-http-viewer` (+ at most `ports`/`adapter-duckdb`). Workspace stays 21. Functional paradigm (ADR-007). | slice-06–17 precedent |

---

## Wave: DISCUSS / [REF] Proposed change + count-read approach

- **Routes (EXTENDED — NO new route)**: `GET /` (landing) and `GET /claims` (list) already
  exist. slice-18 renders the countered-own-claims count beside the own-claims count on the
  landing summary and in the `/claims` header.
- **Read (REUSED data — NO new read method, with ONE open DESIGN question)**: the countered
  count = the number of own-claim CIDs (`SELECT cid FROM claims`) that appear as a countered
  `referenced_cid` (`ref_type='counters'`) in `claim_references ∪ peer_claim_references`. The
  self-counter rule means own claims are countered by PEERS (so the countered own-claim cid
  appears in `peer_claim_references`, or in `claim_references` for the operator's own counter
  to a different claim of hers).

  > **OPEN DESIGN QUESTION (DD owns it — WD-CC-5)**: a count-only aggregate
  > `count_countered_own_claims()` (`COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN
  > (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT
  > referenced_cid FROM peer_claim_references WHERE ref_type='counters')` — mirrors slice-17's
  > `count_active_peer_subscriptions` count-only decision) OR reuse the slice-12
  > `counter_presence_for(all_own_cids).len()` (zero new port surface; materializes every own
  > cid + the presence set just to count). The PRODUCT contract is: a SINGLE aggregate read,
  > invariant to store size (C-3) — DESIGN picks the cheaper/cleaner. RECOMMEND the count-only
  > aggregate (symmetry + cheapness, per slice-17 ADR-054 D3). If DESIGN adds it, it is a
  > read-only method on `StoreReadPort`; `adapter-duckdb` gains ONE aggregate impl; workspace
  > stays 21.

- **Pure render (EXTENDED, in `viewer-domain`)**: extend the slice-17 `LandingSummary` with a
  countered field (`countered_own_claims: Option<usize>`, or a parallel Option) so a failed
  read degrades to the missing marker (slice-17 `render_count` reused); `render_landing`
  renders it beside the own-claims count ("(N countered)"); `render_claims_page`'s header
  renders the SAME number (single source — WD-CC-8). DESIGN owns the exact `LandingSummary`
  shape + the phrasing/markup.

---

## Wave: DISCUSS / [REF] JTBD trace (story → J-003b, with boundaries)

| Story | Title | job_id | Boundary note |
|---|---|---|---|
| US-CC-000 | Resolve the countered-own-claims count in a fixed aggregate read and thread it into the landing summary + `/claims` header, degrading independently | `infrastructure-only` | `infrastructure_rationale` in user-stories.md. Enables US-CC-001/002. NOT a mutation; read-only. Reuses the slice-12 counter-ref data. |
| US-CC-001 | At the front door, see at a glance how many of my own claims have been countered | J-003b | The ORIENTATION facet of J-003b. The count is a LOCAL presence aggregate; rendered beside the own-claims count. Reading WHO countered WHAT stays the slice-11 thread + slice-12 flags. |
| US-CC-002 | In the `/claims` list header, see the same disputed-claim awareness count | J-003b | The SAME count (single source) in the `/claims` header. Additive — the list order/paging/flags are untouched. |

### Infrastructure rationale (US-CC-000)

US-CC-000 carries `job_id: infrastructure-only`: it resolves the LOCAL countered-own-claims
count in a fixed aggregate read (reusing the slice-12 counter-reference tables) and threads it
into the slice-17 `LandingSummary` + the `/claims` header resolution, degrading independently
on read failure (never a 5xx). It produces no user-visible output on its own (the rendered
"(N countered)" is US-CC-001/002), so it enables a user decision only THROUGH those stories.
The slice contains TWO non-infrastructure, user-visible stories with a real decision, so the
slice has release value. READ-ONLY by construction: no mutation method (and if DESIGN elects
the count-only aggregate, it is on `StoreReadPort`, which declares no mutation method).

---

## Wave: DISCUSS / [REF] Out of scope (explicit)

See `requirements.md` §"Out of scope" and `user-stories.md` §"Out of scope". Headlines:
no write/compose/sign control on `/` or `/claims` (C-1, CARDINAL); no new route; no counter
CONTENT (authors/reasons/threads) in the count; no "disputed by N" total / re-weight / verdict
(C-4); no re-order/filter of `/claims` by countered state; no peer-claims-countered count this
slice (WD-CC-7, recommend deferred); no network seam (C-2); no N+1 (C-3); no penalty/score
copy (C-6); no fabricated 0 on a failed read (C-5); no new crate (workspace stays 21).

---

## Wave: DISCUSS / [REF] Scope assessment (Elephant Carpaccio gate)

Run BEFORE journey visualization investment (Phase 1.5). A thin delta extending an existing
summary + reusing existing counter data.

| Signal | Value | Oversized? |
|---|---|---|
| User stories | 3 (1 infra + 2 user-visible) | No (<10) |
| Bounded contexts / modules | 1 (the viewer) extending `viewer-domain` (pure) + `adapter-http-viewer` (effect) + at most `ports`/`adapter-duckdb` IF DESIGN elects a count-only aggregate — all existing; NO new crate | No (single context) |
| Walking-skeleton integration points | 3: (1) resolve the countered count (reuse slice-12 counter-ref tables), (2) thread into the slice-17 `LandingSummary` + the `/claims` header (single source), (3) render "(N countered)" on both surfaces | No (≤5) |
| Estimated effort | ~0.5–1 day | No (≤2 weeks) |
| Independent user outcomes | 1 (see how much of my own work has been disputed, at a glance, on the orientation surfaces) | No |

**## Scope Assessment: PASS — 3 stories (1 infra + 2 user-visible), 1 context, 3 integration points (resolve the countered count + thread into the slice-17 summary/`/claims` header + render on both surfaces), estimated ~0.5–1 day. No new route; reuses the counter-reference data + the slice-17 summary; no new crate; workspace stays 21.**

---

## User Stories

See `user-stories.md` (combined file, one section per story; `## System Constraints` at top).

| ID | One-line | job_id |
|---|---|---|
| US-CC-000 | Resolve the countered-own-claims count in a fixed aggregate read (reusing the slice-12 counter-reference data) and thread it into the slice-17 `LandingSummary` + the `/claims` header, degrading independently on read failure | infrastructure-only |
| US-CC-001 | At the front door, see at a glance how many of my own claims have been countered ("12 own claims (3 countered)") | J-003b |
| US-CC-002 | In the `/claims` list header, see the same disputed-claim awareness count, consistent with the landing | J-003b |

---

## Wave: DISCUSS / [REF] Outcome KPIs

slice-18 mints **NO new KPI ID**. Like slices 08–17 it REALIZES inherited KPIs on a new facet
(the disputed-claim awareness count on `/` + `/claims`). Full detail in `outcome-kpis.md`.
Relevant inherited KPIs: **KPI-VIEW-1** (time-to-see-store-contents — now including
disputed-claim state, realized); **KPI-VIEW-2 / KPI-5 / KPI-VIEW-5** (read-only / local-first
/ offline, guardrails — MET); **KPI-FED-3** (counter-claim publication rate — READ-side
strengthening, per the slice-11/12 precedent).

---

## Wave: DISCUSS / [REF] Walking-skeleton (WS) strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** The `openlore ui` viewer, the read-only
`StoreReadPort`, the indexed counter-reference tables (slice-12), the slice-17 `LandingSummary`
+ `render_landing` + `MISSING_COUNT_MARKER`, and the slice-06 `/claims` header all already
exist. The thinnest end-to-end slice IS US-CC-001 (the landing countered count), backed by
US-CC-000 (resolving the count), with US-CC-002 (the `/claims` header) reusing the SAME count.
Delivery sequence: US-CC-000 → US-CC-001 → US-CC-002. Each is demonstrable in a single session
against the real `openlore ui`.

---

## Wave: DISCUSS / [REF] Shared artifacts + journey

- Requirements (functional + NFR + business rules): `requirements.md`
- User stories (combined, `## System Constraints` at top): `user-stories.md`
- Acceptance criteria (BDD, by theme): `acceptance-criteria.md`
- Outcome KPIs: `outcome-kpis.md`
- Definition of Ready: `dor-checklist.md`
- Wave decisions (WD-CC-*): `wave-decisions.md`

> Lean mode: the standalone journey-visual + journey-yaml + shared-artifacts-registry are NOT
> produced for this thin DELTA (mirroring the slice-08/12/15/17 lean set). The shared
> artifacts are the slice-17 `LandingSummary` (extended with the countered field, the single
> source for both surfaces) and the slice-12 counter-reference tables (reused for the count).

---

## Wave: DISCUSS / [REF] Definition of Ready

See `dor-checklist.md`. Verdict: **PASS (9/9)** for all 3 stories.

---

## Wave: DISCUSS / [REF] Risks

See `wave-decisions.md` §"Risks logged" (R-CC-1..R-CC-8). Headlines: no DIVERGE (low/low —
J-003b validated, counter-flag family + landing summary shipped); a failed countered-count
read 5xxes / blanks the surface (medium/high — independent graceful degrade is a hard
commitment); the count becomes an N+1 (low/medium — fixed aggregate read); a twice-countered
claim is double-counted / reads as a "by N" total (low/high — presence count, counted once);
the count re-weights / reads as a penalty (low/medium — own-claims count unchanged, neutral
copy); landing/header drift (low/medium — single source); the `/claims` header re-orders the
list (low/medium — additive); scope creep to peer-claims-countered (low/low — surfaced as a
deferred scope decision).

---

---

## Wave: DESIGN (2026-06-09 · Morgan, nw-solution-architect · ADR-055)

DESIGN resolved the two inherited open questions and produced the lean architecture set
(`design/architecture-design.md`, `component-boundaries.md`, `technology-stack.md`,
`data-models.md`) + **ADR-055**.

- **WD-CC-5 RESOLVED → count-only aggregate.** New read-only `StoreReadPort` method
  `count_countered_own_claims() -> Result<usize, StoreReadError>`; `adapter-duckdb` impl:
  `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM
  claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
  peer_claim_references WHERE ref_type='counters')` — parameter-free, injection-safe, ONE
  aggregate, invariant to store size. Presence count by construction (de-duped `UNION` IN-set +
  `COUNT(DISTINCT)` — a claim countered N times counts once, no JOIN-fanout). Chosen over
  `counter_presence_for(own_cids).len()` for SYMMETRY (the 3 count-only landing reads) +
  CHEAPNESS (avoids materializing the own-cid list + presence set), mirroring slice-17 ADR-054 D3.
- **WD-CC-7 RESOLVED → own-claims-only.** Own-only by query shape (outer table `claims`);
  peer-claims-countered DEFERRED as a recommended additive sibling.
- **`LandingSummary` extension.** A FOURTH `Option<usize>` field `countered_own_claims` (additive;
  `0 ≠ missing` type-level; per-count `.ok()` independent degrade — ADR-054 D2 extended). The
  `/claims` header takes the bare `Option<usize>` as a `render_claims_page` param.
- **Single source (WD-CC-8).** A shared pure `render_countered(Option<usize>) -> String` helper
  renders "(N countered)" / "(— countered)" on BOTH the landing (`render_landing`, beside the
  unchanged own-claims line) and the `/claims` header — single source for the copy; a gold test
  pins landing == header.
- **xtask anti-merging SQL rule GREEN by construction.** The new SQL does NOT trip
  `no_cross_table_join_elides_author`: `peer_claim_references` is not the `peer_claims` WHOLE WORD
  (the `_references` suffix fails the word boundary), so `is_cross_store` is false and the
  classifier returns `None` — the EXACT reason slice-12's `counter_presence_for` is GREEN. The
  index-store `mentions_aggregation` variant scans `adapter-index-store` only. xtask UNCHANGED.
- **No new crate / route / KPI / persisted type** (WD-CC-11 honored). Workspace stays 21. The new
  read is read-only on the existing port; the slice touches `viewer-domain` (pure) +
  `adapter-http-viewer` (effect) + `ports` + `adapter-duckdb`. No external integration → no
  contract-test annotation.

---

## Wave: DISTILL (2026-06-09 · Quinn, nw-acceptance-designer)

### [REF] Reconciliation HARD GATE

**Reconciliation passed — 0 contradictions.** Read DISCUSS `wave-decisions.md` (WD-CC-1..12) +
DESIGN (ADR-055 + the DESIGN section above). No separate DESIGN/DEVOPS `wave-decisions.md` files
(brownfield; DESIGN decisions recorded in ADR-055 + feature-delta). DESIGN resolves the two open
DISCUSS questions CONSISTENTLY (WD-CC-5 → count-only aggregate; WD-CC-7 → own-claims-only) and
upholds every CARDINAL DISCUSS commitment (read-only, LOCAL/offline, missing≠zero, presence-once,
single-source, additive-no-regression, anti-misread). No contradiction. No DEVOPS wave (inherits
the viewer infra — clean local DuckDB + subprocess HTTP).

### [REF] Scenario list with tags

15 story scenarios (`tests/acceptance/viewer_counter_aware_counts.rs`) + 9 GOLD invariants
(`tests/acceptance/viewer_counter_aware_counts_invariants.rs`). Full table + AC mapping:
`distill/test-scenarios.md`. Headlines:

- **CC-WS** (`@walking_skeleton @driving_port @driving_adapter @real-io`): GET / over a seeded
  store (12 own, 3 countered by peers, one by 2) renders "12 own claims (3 countered)".
- **CC-HEADER** (`@single-source @wd-cc-8`): GET /claims renders the SAME "(3 countered)";
  landing == header.
- **CC-PRESENCE** (`@presence-once @c-4 @cardinal`): a claim countered by 2 peers counts ONCE.
- **CC-ZERO-LANDING / CC-ZERO-HEADER** (`@honest-zero @c-5 @edge`): "(0 countered)" Some(0).
- **CC-DEGRADE-LANDING / CC-DEGRADE-HEADER** (`@infrastructure-failure @missing-not-zero @c-2
  @c-5 @cardinal @error`): failed read → "(— countered)", page 200.
- **CC-NO-REWEIGHT / CC-NO-REORDER** (`@additive @no-regression @c-4 @wd-cc-9`): "12" unchanged;
  /claims list byte-identical.
- **CC-READONLY-* / CC-OFFLINE-*** (`@read-only @c-1 @cardinal` / `@offline @no-cdn @c-2`).
- **CC-NO-N-PLUS-1** (`@property @no-n-plus-1 @c-3 @cardinal`): one aggregate read, invariant to
  store size.
- **CC-ANTI-MISREAD** (`@anti-misread @c-6 @wd-cc-10`): neutral copy, confidence verbatim.

### [REF] WS strategy

Brownfield DELTA — no Feature-0 walking skeleton (the viewer + LandingSummary + counter-ref
tables are SHIPPED). ONE `@walking_skeleton` scenario closing the new vertical (count read →
4th LandingSummary field → render_countered → "12 own claims (3 countered)") through the
production composition root (real `openlore ui` subprocess). Driving treatment per the
Architecture of Reference: REAL HTTP subprocess (driving) + REAL local DuckDB (driven-internal)
seeded via production verbs; no external/non-deterministic port (the count is LOCAL, no network
edge). `[policy-mode] inherit`, `[port-mode] inherit`. Full detail: `distill/walking-skeleton.md`.

### [REF] Driving-adapter coverage

`GET /` and `GET /claims` both exercised via the REAL `openlore ui` subprocess + in-test HTTP
GET (status + rendered-HTML body asserted). NO scenario calls `render_landing` / `render_countered`
/ the count read directly (Mandate 1). Both routes are covered by ≥1 subprocess HTTP scenario.

### [REF] Adapter coverage table

| Driven adapter | @real-io scenario | Covered by |
|---|---|---|
| `count_countered_own_claims` (DuckDB COUNT(DISTINCT) read) | YES | CC-WS + every story scenario (real seeded DuckDB; the seed pins the genuine count via the direct ADR-055 `read_countered_own_claims_count` oracle) |
| the `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` fault seam | YES (failure path) | CC-DEGRADE-LANDING / CC-DEGRADE-HEADER / CC-INV-MissingNotZero (test-only effect-shell seam, slice-17 precedent — DELIVER materializes) |

No NEW external adapter (the count is a LOCAL aggregate over the slice-12 indexed ref tables —
no network edge). No contract-test annotation needed.

### [REF] Scaffolds (RED-ready)

- `tests/acceptance/viewer_counter_aware_counts.rs` — `// SCAFFOLD: true`, 15 story scenarios.
- `tests/acceptance/viewer_counter_aware_counts_invariants.rs` — `// SCAFFOLD: true`, 9 GOLD.
- `tests/acceptance/support/mod.rs` — slice-18 seeds + asserts + consts + the
  `start_inner` 7th `fail_countered_count` param + the `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`
  seam (all `// SCAFFOLD: true (slice-18)`).
- `crates/cli/Cargo.toml` — the two new `[[test]]` registrations.

RED confirmed: both binaries COMPILE; the WS + 4 sampled scenarios were RUN and FAIL for the
right reason (MISSING_FUNCTIONALITY — "(N countered)" absent because the routes don't render the
count + `count_countered_own_claims`/`render_countered`/the 4th field don't exist). The seeds'
ADR-055 oracle confirmed the genuine seeded counts (3/0/1, presence-once collapse included).
`check-arch: OK (21 workspace members)`. slice-17 + slice-12 suites still compile (start_inner
ripple clean). DELIVER unskips + implements per ADR-025 (DISTILL is the canonical AT author).

### [REF] Test placement

`tests/acceptance/` (the established OpenLore viewer-slice convention; slice-06..17 all live
here, registered as `[[test]]` bins in `crates/cli/Cargo.toml`). Shared harness in
`tests/acceptance/support/mod.rs`.

### [REF] Pre-requisites (DELIVER inherits)

- DESIGN driving ports: `GET /` + `GET /claims` (ADR-055 D3/D4).
- The new read `StoreReadPort::count_countered_own_claims` + the `adapter-duckdb`
  `COUNT(DISTINCT)` impl (ADR-055 D1).
- The 4th `LandingSummary.countered_own_claims: Option<usize>` field + the shared
  `render_countered(Option<usize>)` helper + the `render_claims_page` `Option<usize>` param
  (ADR-055 D2/D3).
- The `.ok()` per-route resolution in `landing_page` + `claims_page` (ADR-055 D4).
- The `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` `#[cfg(debug_assertions)]`-gated fault seam on both
  handlers (slice-17 precedent).

---

## Changelog

- 2026-06-09 — slice-18 (`viewer-counter-aware-counts`) DISTILL (Quinn). Reconciliation HARD GATE
  PASSED (0 contradictions). Authored 15 story scenarios + 9 GOLD invariants (all scaffolded RED,
  ADR-025) driving `GET /` + `GET /claims` via the REAL `openlore ui` subprocess; mirrors the
  slice-17 landing test structure + the slice-12 counter-seeding conventions. Added seeds
  (`seed_landing_store_with_countered_own_claims` / `seed_landing_store_none_countered` /
  `seed_landing_store_one_own_claim_countered_twice` / `start_viewer_with_failing_countered_count`
  / `read_countered_own_claims_count` ADR-055 oracle) + asserts (`assert_landing_countered_count`
  / `assert_landing_countered_missing` / `assert_claims_header_countered_count` /
  `assert_claims_header_countered_missing` / `assert_landing_and_claims_countered_consistent` /
  `assert_countered_copy_is_neutral`) + the `start_inner` 7th `fail_countered_count` param + the
  `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` seam. Registered the two new test bins. RED confirmed
  (MISSING_FUNCTIONALITY); check-arch OK (21); slice-17 + slice-12 suites still compile.
- 2026-06-09 — slice-18 (`viewer-counter-aware-counts`) DESIGN (Morgan / ADR-055). Resolved
  WD-CC-5 (count-only `count_countered_own_claims` aggregate) + WD-CC-7 (own-claims-only). Added a
  4th additive `Option<usize>` field on `LandingSummary` + a shared `render_countered` helper for
  single-source landing+header. xtask anti-merging SQL rule GREEN by construction. No new
  crate/route/KPI/persisted type; workspace stays 21.
- 2026-06-09 — slice-18 (`viewer-counter-aware-counts`) DISCUSS. Traces to J-003b (the
  ORIENTATION / at-a-glance-count facet of counter-claim awareness). 3 stories (1 infra + 2
  user-visible). EXTENDS the slice-17 `GET /` landing summary + the slice-06 `GET /claims`
  list header (NO new route); reuses the slice-12 counter-reference data (`claim_references ∪
  peer_claim_references`, `ref_type='counters'`) for the countered-own-claims count — NO new
  read method (OPEN DESIGN QUESTION WD-CC-5: count-only aggregate `count_countered_own_claims`
  vs `counter_presence_for(own_cids).len()`); renders "(N countered)" beside the own-claims
  count on both surfaces (single source — WD-CC-8). CARDINAL decisions: read-only / no-key
  (WD-CC-1); LOCAL-only / offline + independent graceful degrade (WD-CC-2); cheap / no-N+1 /
  fixed aggregate read (WD-CC-3); presence count, never a "by N" total / re-weight / verdict
  (WD-CC-4). SCOPE decision surfaced: own-claims-countered is the core; peer-claims-countered
  recommended DEFERRED (WD-CC-7). Missing≠zero (WD-CC-6); anti-misread neutral copy (WD-CC-10);
  additive on `/claims` — no list regression (WD-CC-9). NO new crate (workspace stays 21), no
  new KPI ID. Scope PASS (~0.5–1 day). DoR PASS (9/9 for all 3 stories; Dimension-0 PASS;
  JTBD PASS).
