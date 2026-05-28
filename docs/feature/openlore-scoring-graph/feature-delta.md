# Feature Delta: openlore-scoring-graph

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: Cross-cutting (CLI read surface + ports/read methods + scoring pure core + storage revisit)
> Walking skeleton: Yes (this sibling IS the walking skeleton for the scoring slice)
> Research depth: Comprehensive (scoring transparency + anti-merging-in-aggregates are load-bearing)
> JTBD: mandatory (every story carries `job_id` -> `docs/product/jobs.yaml`)
> Inherits from: `docs/feature/openlore-foundation/feature-delta.md` (WD-9..WD-13, ADR-001..012), `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25, ADR-013..016), `docs/feature/openlore-github-scraper/feature-delta.md` (WD-46..WD-58 + WD-59/WD-65/WD-67, ADR-017..019)
> Date: 2026-05-28
> Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `openlore-scoring-graph`,
the fourth sibling feature in the OpenLore umbrella (slice-04). Tier-1 content
is inlined under `## Wave: DISCUSS / [REF] <Section>` headings; SSOT content
lives under `docs/product/`; per-journey artifacts under
`docs/feature/openlore-scoring-graph/discuss/`.

Slice-04 is a READ/VIEW slice over the LOCAL federated graph: the user's own
authored claims (slice-01), peer claims pulled via slice-03 (`peer_claims`), and
claims signed from scraper candidates (slice-02, which are normal author claims).
It creates, signs, and publishes NOTHING and introduces NO new network surface.
Per the brief, slice-04 "may swap or augment adapter-duckdb with a graph store;
revisits ADR-001/WD-8" — that store decision is DESIGN's call; DISCUSS stays
solution-neutral about storage.

---

## Wave: DISCUSS / [REF] Wave Decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-69 | Slice-04 ships in a SIBLING feature `openlore-scoring-graph` (this feature) per the carpaccio split locked by WD-9. Slice-04 IS the walking skeleton for this feature (one slice = one feature). | Inherits WD-9. Sibling-feature pattern keeps each slice independently shippable. Continues the umbrella sequence (WD-13): federation -> scrapers -> scoring -> appview. | LOCKED |
| WD-70 | Persona priority for slice-04: **P-002 Researcher / Tech Lead (graph-explorer hat) = primary**; **P-001 Senior Engineer Solo Builder = secondary** (wears the same explorer hat when choosing a stack/community). | Slice-04's load-bearing job (J-002) is an exploration/decision job; P-002 is the natural explorer. P-001 wears the same hat when committing to a stack. | LOCKED |
| WD-71 | **Scoring is transparent and auditable — NO ML (load-bearing).** Every adherence weight is a SMALL closed-form function of `count x confidence x triangulation`. The formula is displayed alongside the weight; `--explain` reproduces the per-claim arithmetic. No opaque, learned, or non-reproducible score is permitted. | Directly mitigates the J-002 anxiety ("speculative/biased/sparse data -> bad call"). An ML score would make the weighting unauditable and re-trigger the aggregator distrust the whole product exists to avoid. The formula lives in a pure `scoring` core (ADR-007) as the SSOT. | LOCKED, enforced by US-GRAPH-003/005 AC + KPI-GRAPH-3 |
| WD-72 | **Weights/scores are DERIVED + DISPLAY-ONLY.** Adherence weights and weight buckets ([STRONG]/[MODERATE]/[SPARSE]) are computed at query time and NEVER persisted, signed, or published. They extend the WD-10 display-only-bucket discipline. | A persisted score would become stale relative to the claims it summarizes, creating a trust hazard, and would tempt federation of a derived value. Computing at query time keeps the score honestly tied to the current local graph. (A future slice needing persistence requires a WD + ADR with rationale; slice-04 does not.) | LOCKED, enforced by US-GRAPH-003/006 AC + acceptance test `weight_and_bucket_never_persisted` |
| WD-73 | **Anti-merging extends to aggregates (load-bearing).** A score is an AGGREGATE VIEW, never a merge that loses attribution. Every weighted/traversed result decomposes to its contributing `(author_did, claim_cid)` tuples; every output row retains its author DID; no "consensus weight" is shown without the contributing authors visible. | Carries the slice-03 I-FED-1 anti-merging invariant into the scoring surface. Violating it collapses scoring into yet another aggregator that hides provenance — the exact failure mode J-002 distrusts. The slice-03 `xtask check-arch` `no_cross_table_join_elides_author` rule extends to scoring queries. | LOCKED, enforced by US-GRAPH-003/004/005 AC + KPI-GRAPH-2 + acceptance test `scoring_aggregate_preserves_attribution` |
| WD-74 | **Sparse renders sparse (load-bearing).** A thin subgraph (few claims, few authors) is visibly labeled [SPARSE] with a "based on N claims by M authors" honesty line. The system NEVER manufactures confidence from thin evidence. | The direct mitigation of the J-002 "bad call on sparse data" anxiety. A single-claim philosophy must look thin, never dressed up as a confident number. This is the most load-bearing UX in the slice. | LOCKED, enforced by US-GRAPH-003 AC + KPI-GRAPH-4 + acceptance test `sparse_renders_sparse` |
| WD-75 | **Query dimensions to ship: by subject (inherited), by object (philosophy, NEW), by contributor (DID, NEW).** `--object` and `--contributor` are the new orienting surfaces; `--subject` is unchanged from slice-01/03. | J-002 functional needs all three dimensions. `--subject` already exists; `--object` (philosophy-first) and `--contributor` (developer-first) are the new orientation entry points. Each preserves per-author attribution (WD-73). | LOCKED, US-GRAPH-001/002 |
| WD-76 | **`--traverse` ships in slice-04** (Release 2), bounded to a default depth of 2 (`--depth K` override). Every traversed edge maps to exactly one signed claim; traversal invents no edges. | Traversal is the headline J-002 success signal ("surface a non-obvious connection in one query"); deferring it would gut the slice's value. Bounded default depth prevents fan-out explosion on dense graphs. No-invented-edges preserves auditability (WD-71). | LOCKED, US-GRAPH-004 |
| WD-77 | **Default scoring formula (product default):** `weight = sum over contributing claims of [ confidence x author_distinct_bonus x cross_project_triangulation_bonus ]`, where author_distinct_bonus = 1.0 for the first author +0.25 per additional distinct author on the SAME (subject,object), and cross_project_triangulation_bonus = +0.5 when the SAME author asserts the philosophy on >=2 distinct subjects. Bucket thresholds and constants are DESIGN-tunable but MUST stay small, closed-form, and reproducible. | A concrete default forces the transparency contract to be testable now. The constants are a product judgment call made in auto-mode; DESIGN may tune them but cannot replace the function with a learned/opaque model (WD-71). The formula constants are the SSOT in the pure `scoring` core. | LOCKED (formula shape); constants DESIGN-tunable |
| WD-78 | **WD-8 storage revisit is a DESIGN decision, NOT a user story.** Whether slice-04 swaps `adapter-duckdb` for a graph store OR augments DuckDB with recursive traversal is decided in DESIGN; DISCUSS stays solution-neutral. The user-visible contracts (dimensions, traversal, weighting, sparse honesty, anti-merging) hold regardless of the storage choice. | A graph-DB migration as a user story would balloon the slice and is invisible to the user (carpaccio guardrail). The brief explicitly scopes the store decision to DESIGN ("revisits ADR-001/WD-8"). Framing it as DESIGN-internal keeps slice-04 right-sized. | LOCKED |
| WD-79 | **Local scope only.** Scoring/traversal operate over the LOCAL graph (own + pulled peers + scraper-signed). Multi-user / cohort aggregation across many users' graphs is OUT of slice-04 (slice-05 AppView). | The carpaccio guardrail: cross-user aggregation needs an indexer service (a slice-05 concern with its own JTBD). Slice-04 scores what is already on disk. | LOCKED |

### Scope Assessment

`## Scope Assessment: PASS — 6 user stories (5 user-visible + 1 infra), 1 cohesive bounded context (graph query + traversal + scoring over the local federated graph), estimated ~10.5 days. Single slice = single feature; no further sub-slicing recommended.`

Carpaccio gate evaluation (5 taste tests):

- **Stories**: 6 (within <=10 threshold). PASS.
- **Bounded contexts**: 1 (read/view exploration of the local federated graph: query-by-dimension + traversal + scoring as a single coherent surface; lives within the larger claim/federation context inherited from slice-01/03). PASS.
- **Walking-skeleton integration points**: 2 NEW (the pure `scoring` core; the read-side query extension for the new dimensions/traversal/weighting). Both are extensions/additions over existing stores; NO new network surface, NO new write surface. Within the <=5 threshold. PASS.
- **Estimated effort**: ~10.5 days (within <=2 weeks threshold). PASS.
- **Multiple independent outcomes**: NO — all 6 stories serve J-002 and its sub-jobs (query-by-dimension, traverse, weighted-scoring); `--contributor` and `--explain` are aspects of the same explore-then-decide outcome, not independent outcomes. PASS.
- **Verdict**: RIGHT-SIZED. **Single slice = single sibling feature.** A graph-DB migration is explicitly NOT a user story (WD-78, DESIGN-internal); multi-user/cohort aggregation is explicitly deferred to slice-05 (WD-79). Local + over already-present claims.

### Risks logged

- KPI-GRAPH-1 (surface a non-obvious connection in >=60% of explorer sessions) is the slice's load-bearing behavioral hypothesis. Mitigation: instrumentation via the `graph.connection.surfaced` tracing event (handed off to DEVOPS).
- The default scoring formula constants (WD-77) are a product judgment call made in auto-mode without user-validation interviews. Mitigation: KPI-GRAPH-3 (transparency / reproducibility) plus the day-30 "could you explain why this ranked first?" interview surface whether users find the weighting sensible; the formula is small and trivially revisable in the pure `scoring` core.
- The WD-8 store revisit (WD-78) could expand DESIGN's effort significantly if a graph-store swap is chosen. Mitigation: the user-visible contract is storage-neutral; DESIGN may pick the smaller change (DuckDB recursive traversal) if the graph workload does not yet justify a store swap. Surfaced as Open Decision OD-GRAPH-1.
- Traversal fan-out on dense graphs could degrade latency (KPI-GRAPH-6). Mitigation: bounded default depth 2 (WD-76); DEVOPS instruments `graph.query.duration_seconds` per claim-count bucket.
- DISCOVER + DIVERGE skipped (same as slice-01/02/03). The four-forces analysis for J-002 was performed in prior DISCUSS waves and deepened here without prior validation interviews. Mitigation: KPI-GRAPH-1 + KPI-GRAPH-5 + the day-30 study surface mis-prioritization within 30 days of release.

---

## Wave: DISCUSS / [REF] JTBD Analysis Summary

Full analysis in `docs/product/jobs.yaml`. Summary for slice-04:

| Job | Name | Priority for slice-04 | Opportunity Score | In slice-04? |
|---|---|---|---|---|
| J-002 | Explore the philosophy graph to inform a decision | primary (walking-skeleton for this feature) | 14 (verdict raised to underserved-primary-for-slice) | yes — all 6 stories |
| J-002a (sub-job of J-002) | Query the graph by dimension (subject/object/contributor) | LOAD-BEARING | n/a (sub-job) | yes — US-GRAPH-001, US-GRAPH-002 |
| J-002b (sub-job of J-002) | Traverse contributor<->project<->philosophy edges | LOAD-BEARING | n/a (sub-job) | yes — US-GRAPH-004 |
| J-002c (sub-job of J-002) | See transparent, auditable adherence weighting | LOAD-BEARING | n/a (sub-job) | yes — US-GRAPH-003, US-GRAPH-005 |
| J-003 | Read another developer's federated claims with weighting | inherited (built on) | 15 | partial — slice-04 reads the `peer_claims` slice-03 populated; extends `graph query --federated` |
| J-004 | Evaluate a contributor's body of work through a philosophy lens | inherited (related) | 13 | partial — US-GRAPH-002 (contributor lens) + US-GRAPH-003 (adherence weighting) realize J-004's "surface adherence weighting" over the local graph |

J-002 was deepened during this DISCUSS with three load-bearing sub-jobs
(query-by-dimension, traverse-edges, weighted-scoring), a verdict promotion to
underserved-primary-for-slice, the `walking_skeleton_for: openlore-scoring-graph`
marker, and two slice-04 success signals (weight reproducible by hand;
sparse-recognized-as-sparse). Slice-04 BUILDS ON J-003 (it reads the federated
`peer_claims` store slice-03 created) and REALIZES part of J-004 (the contributor
lens + adherence weighting jobs.yaml had listed as J-004 functional needs, now
delivered over the local graph). It does NOT relitigate J-003/J-004.

---

## Wave: DISCUSS / [REF] Journey Artifacts

One journey to map (explore-the-graph is the single coherent surface; the four
steps query -> traverse -> weight -> audit are one continuous arc):

- Visual journey (query -> traverse -> weight -> audit): `docs/feature/openlore-scoring-graph/discuss/journey-explore-the-graph-visual.md`
- Structured schema (with embedded Gherkin per step): `docs/product/journeys/explore-the-graph.yaml`
- Shared artifacts registry: `docs/feature/openlore-scoring-graph/discuss/shared-artifacts-registry.md`

Emotional arc:

- Explore-the-graph journey: **curiosity-to-defensible-decision (with a transparency buffer)** — entry Curious-but-skeptical (heard the community values X but cannot say why; anxious about sparse/biased data) through Orienting (query by dimension) and Connecting (traversal surfaces the non-obvious link, the "aha" Discovery-Joy peak) to Defensible-confident at the weighted view. The weighted/scored view is deliberately LAST, AFTER the user has seen the raw attributed claims, so trust in the data precedes trust in the aggregate.

Two cross-cutting guarantees are elevated to their own section in the visual
journey:

- **Scoring transparency** (J-002c anxiety mitigation): formula is small/closed-form/no-ML, displayed with inputs; `--explain` reproduces the arithmetic; weights are derived + display-only (never persisted, WD-72); sparse renders sparse (WD-74).
- **Anti-merging in aggregates** (extends slice-03 I-FED-1, WD-73): a weight is an aggregate VIEW that decomposes to per-author, per-cid contributions; every row retains its author DID; the `xtask check-arch` no-elide-author rule and the `scoring_aggregate_preserves_attribution` test enforce it.

---

## Wave: DISCUSS / [REF] Story Map and Slicing

- Story map: `docs/feature/openlore-scoring-graph/discuss/story-map.md`

Slicing summary:

- **Release 1 (walking skeleton)**: US-GRAPH-001 + US-GRAPH-003 + US-GRAPH-006. Validates a transparent weighted view over the existing graph end-to-end, with sparse honesty and anti-merging.
- **Release 2 (connection discovery)**: US-GRAPH-002 + US-GRAPH-004. Validates J-002's headline "surface a non-obvious connection" via the contributor lens + traversal.
- **Release 3 (auditability drill-down)**: US-GRAPH-005. Deepens transparency from "shows the formula" to "reproduce by hand" via `--explain`.

Priority order is set by outcome impact and risk-of-failure consequence
(Release 1 fails = the J-002 distinguish-support-from-speculation thesis is
disproven AND a transparent/honest weighting is the riskiest assumption; Release 3
fails = survivable auditability gap). Rationale in story-map.md
`## Priority Rationale` section.

All 5 carpaccio taste tests evaluated for this slice (in the Scope Assessment
above): right-sized in stories, contexts, integration points, effort, and outcome
coherence. Verdict: SINGLE SLICE = SINGLE FEATURE; no further sub-slicing.

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All in `docs/feature/openlore-scoring-graph/discuss/user-stories.md`:

| Story | Title | Job link | Elevator Pitch | DoR status |
|---|---|---|---|---|
| US-GRAPH-001 | Query by philosophy (object) and project (subject), attribution preserved | J-002 | yes | PASS (see DoR section) |
| US-GRAPH-002 | Query by contributor (DID) — one developer's reasoning trail | J-002 | yes | PASS |
| US-GRAPH-003 | Transparent weighted/scored view; sparse renders sparse | J-002 | yes | PASS |
| US-GRAPH-004 | Traverse contributor<->project<->philosophy edges | J-002 | yes | PASS |
| US-GRAPH-005 | Audit a weight with `--explain` per-claim arithmetic | J-002 | yes | PASS |
| US-GRAPH-006 | Bootstrap pure `scoring` core + read-side extensions (`@infrastructure`) | `infrastructure-only` | n/a — @infrastructure | PASS |

Slice composition gate: PASS — 5 user-visible stories + 1 infrastructure story;
slice is NOT 100% `@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).

---

## Wave: DISCUSS / [REF] Outcome KPIs

Full table in `docs/feature/openlore-scoring-graph/discuss/outcome-kpis.md`.
North star:

> **KPI-GRAPH-1**: >=60% of dogfood explorer sessions surface a non-obvious
> connection (a contributor spanning >=2 candidate projects, or an unnoticed
> philosophy clustering) in a single query session within 30 days of release.

Guardrails: KPI-GRAPH-2 (anti-merging in aggregates — zero attribution loss),
KPI-GRAPH-3 (scoring transparency — every weight reproducible), KPI-GRAPH-4
(sparse renders sparse — zero manufactured confidence). All three MUST hold; any
failure is unshippable.

Leading indicators: KPI-GRAPH-5 (referenced justification — proves the connection
was decision-relevant) and KPI-GRAPH-6 (local-read latency — friction kills
exploration).

KPI numbering: KPI-GRAPH-1..6.

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-GRAPH-001 | US-GRAPH-002 | US-GRAPH-003 | US-GRAPH-004 | US-GRAPH-005 | US-GRAPH-006 |
|---|---|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS | PASS | PASS | PASS | PASS | PASS (infra rationale) |
| 2. Persona with specific characteristics | PASS (P-002) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | n/a (infra) |
| 3. >=3 domain examples with real data | PASS (4) | PASS (4) | PASS (5) | PASS (4) | PASS (4) | PASS (2 — within range for narrow infra surface) |
| 4. UAT in Given/When/Then (3-7) | PASS (4) | PASS (4) | PASS (5) | PASS (4) | PASS (4) | PASS (2 — within range for narrow infra surface) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (1.5d, 4) | PASS (1d, 4) | PASS (2.5d, 5) | PASS (2d, 4) | PASS (1.5d, 4) | PASS (2d, 2) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (depends US-GRAPH-006) | PASS (US-GRAPH-006) | PASS (US-GRAPH-001, US-GRAPH-006) | PASS (US-GRAPH-001/002, US-GRAPH-006) | PASS (US-GRAPH-003, US-GRAPH-006) | PASS (slice-01/03 stores) |
| 9. Outcome KPIs defined with measurable targets | PASS (KPI-GRAPH-2, 6) | PASS (KPI-GRAPH-2) | PASS (KPI-GRAPH-1, 3, 4) | PASS (KPI-GRAPH-1, 2) | PASS (KPI-GRAPH-3) | n/a — supports KPI-GRAPH-1..4 |

**Overall DoR status: PASSED** for all stories.

Notes:
- Item 3 + Item 4 (US-GRAPH-006): the spec allows 3-7 scenarios; US-GRAPH-006 ships 2 composite scenarios because the infrastructure surface is narrow and additional scenarios would be padding. Same pattern as US-005 (slice-01), US-FED-006 (slice-03), and US-SCR-006 (slice-02). Flagged for reviewer judgment but considered PASS.
- Item 2 (US-GRAPH-006): infrastructure-only stories do not require a persona; `infrastructure_rationale` present per Decision 1.
- US-GRAPH-003 is the largest story at 2.5 days / 5 scenarios — still within the right-sized band (1-3 days, 3-7 scenarios). It is the load-bearing scoring story; splitting it further would fragment the transparent-weighting outcome.

### Elevator Pitch verification (BLOCKING per Dimension 0)

Per `nw-po-review-dimensions` Dimension 0 (checked first, BLOCKING):

| Story | Section present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-GRAPH-001 | YES (Before/After/Decision enabled) | YES (`openlore graph query --object org.openlore.philosophy.dependency-pinning`) | YES (specific stdout: "Projects embodying ... (4 claims across 3 subjects, 3 distinct authors)" grouped by subject + no-merge footer) | YES (start a decision from a philosophy, discover projects not searched by name) | PASS |
| US-GRAPH-002 | YES | YES (`openlore graph query --contributor did:plc:rachel-test`) | YES (specific stdout: "Claims authored by ... (5 claims across 4 subjects)" + "one developer's reasoning trail, not a community consensus") | YES (weigh a peer's authority from breadth of reasoning) | PASS |
| US-GRAPH-003 | YES | YES (`openlore graph query --object ... --weighted`) | YES (specific stdout: ranked projects with weight + [STRONG]/[SPARSE] buckets + printed formula + "based on 1 claim by 1 author" sparse line) | YES (rank candidates by support and trust the ranking enough to base an architectural decision on it) | PASS |
| US-GRAPH-004 | YES | YES (`openlore graph query --object ... --traverse`) | YES (specific stdout: philosophy->projects->authors tree + "Connections found: did:plc:rachel-test spans 2 of these projects") | YES (surface a non-obvious cross-project contributor connection) | PASS |
| US-GRAPH-005 | YES | YES (`openlore graph query --object ... --weighted --explain github:denoland/deno`) | YES (specific stdout: per-claim breakdown with author DID, CID, confidence, bonuses, running sum == displayed weight) | YES (defend a ranking to a skeptical teammate by showing the exact math) | PASS |
| US-GRAPH-006 | n/a (@infrastructure with rationale) | n/a | n/a | n/a (`infrastructure-only` per Decision 1) | PASS via rationale |

Slice-level Elevator Pitch check (Dimension 0 §5): the slice has 5 user-visible
stories + 1 infrastructure story. Slice is NOT 100% `@infrastructure`. PASS.

---

## Wave: DISCUSS / [REF] Locks inherited from openlore-foundation + openlore-federated-read + openlore-github-scraper

These are binding inputs to this feature's DESIGN wave. They are NOT relitigated
here; any change requires returning to the owning slice's product-owner review
first.

| ID | Inherited from | Carries into slice-04 as |
|---|---|---|
| WD-9 | openlore-foundation | Carpaccio split: each slice is an independent sibling feature. slice-04 is this feature. |
| WD-10 | openlore-foundation | Numeric `[0.0, 1.0]` is the only persisted confidence; display-only buckets. Scoring operates on the NUMERIC confidence; weight buckets ([STRONG]/[MODERATE]/[SPARSE]) extend the display-only-bucket discipline (WD-72) — never persisted. |
| WD-11 | openlore-foundation | Retraction = counter-claim referencing the original CID; soft-retract only. Scoring is read-only and does not author retractions; a retracted/countered claim still appears in the graph (slice-03 semantics) and contributes to weights per its confidence unless DESIGN defines a retraction-aware filter (noted as Open Decision OD-GRAPH-2). |
| WD-12 | openlore-foundation | Identity = user's existing ATProto DID with per-application derived key. Scoring introduces NO new identity surface; the contributor dimension queries existing `author_did` values. |
| WD-13 | openlore-foundation | Sequence: federation -> scrapers -> scoring -> appview. slice-03 and slice-02 have shipped; slice-04 (scoring) is this deliverable; slice-05 (appview) follows. |
| WD-8 | openlore-foundation | "DuckDB revisit at slice-04 when graph traversal becomes the dominant workload." This slice TRIGGERS the revisit; the swap-vs-augment decision is DESIGN's call (WD-78). DISCUSS stays solution-neutral. |
| WD-22 | openlore-federated-read | Single publish path. NOT exercised by slice-04 (read-only; no publish). Listed so DESIGN knows scoring adds no publish path. |
| WD-25 / I-FED-1 | openlore-federated-read | Anti-merging at storage + query + display + test. Slice-04 EXTENDS this to aggregates (WD-73): the `no_cross_table_join_elides_author` rule and the non-`Option` `author_did` discipline (slice-03 `FederatedRow`) extend to scoring queries. |
| WD-58 | openlore-github-scraper | `derived-from` provenance is informational and never alters confidence/federation. Scraper-signed claims are normal author claims; they participate in scoring like any author claim, with no special weight. |
| ADR-001 | openlore-foundation | DuckDB single-file store. REVISITED this slice (WD-78); DESIGN decides swap vs augment. |
| ADR-003 / ADR-013 | foundation / federated-read | CLI verb contract + the `--federated` flag precedent. slice-04 EXTENDS `graph query` with `--object`, `--contributor`, `--traverse`, `--weighted`, `--explain`. Requires an **ADR amendment (next number after ADR-019, i.e. likely ADR-020)** as a DESIGN deliverable, in the same spirit as the ADR-013 amendment slice-03 raised. |
| ADR-007 | openlore-foundation | Functional Rust paradigm. The new `scoring` core is PURE (no I/O); any storage effect stays behind a port in the effect shell. |
| ADR-009 | openlore-foundation | Hexagonal ports + adapters. Any new read port/method surface MUST ship a `probe()` (I-4) within the 250ms budget (I-5). |
| ADR-016 | openlore-federated-read | Peer DID resolution / federated read. Slice-04 reads the `peer_claims` store slice-03 populated; it does NOT add peer-read network paths (no new network surface). |
| I-6 | brief (cross-feature) | Signed payload contains only numeric confidence; display buckets never serialized. Weights/buckets extend this: never serialized anywhere (WD-72). |

---

## Wave: DISCUSS / [REF] Ask-Intelligent Menu (lean mode, scoped to triggered items only)

Triggers evaluated; scoped expansion offered only for those that fired.

### Fired: cross-context complexity (>=3 contexts)

This slice spans CLI verbs/flags (new `--object`/`--contributor`/`--traverse`/`--weighted`/`--explain`) + a new pure `scoring` core + read-side query extensions + the WD-8 storage revisit (graph store vs DuckDB recursive traversal). That is >=3 contexts; the threshold fires.

- **Offer**: `alternatives-considered.md` — document the rejected alternatives for the three biggest choices (scoring model: transparent closed-form vs ML/learned; storage: swap-to-graph-store vs augment-DuckDB-recursive-traversal; weights: derived-display-only vs persisted-and-federated).
- **Cost**: ~10 minutes; ~3 pages output.
- **Recommendation**: **accept**. These are the choices DESIGN will second-guess if not documented now — especially the transparent-vs-ML scoring choice (load-bearing for WD-71) and the WD-8 store revisit framing.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be produced as `docs/feature/openlore-scoring-graph/discuss/alternatives-considered.md` alongside the DESIGN handoff. Flagged as a DESIGN read.

### Fired: AC ambiguity (scoring transparency + sparse-honesty + anti-merging-in-aggregates semantics are easy to disagree on)

The transparency (reproducible weight, no ML), sparse-honesty (thin evidence rendered as sparse), and anti-merging-in-aggregates invariants are conceptually rich, and the J-002 anxiety force ("sparse/biased/speculative data -> bad call") is load-bearing. The happy/edge/error scenarios in user-stories.md cover the functional surface but not the anxiety-path force explicitly.

- **Offer**: `gherkin-scenarios-expanded.md` — add anxiety-path and habit-path scenarios per the JTBD-BDD integration template. Target: >=3 anxiety (sparse-data fear, opaque-score fear, hidden-merge fear) + >=2 habit ("I already run `gh search`"; "the graph-query output is the slice-01/03 one I know").
- **Cost**: ~15 minutes; ~3 pages output.
- **Recommendation**: **accept**. The anxiety force is load-bearing for J-002; without dedicated scenarios DISTILL will have to invent them.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be produced as `docs/feature/openlore-scoring-graph/discuss/gherkin-scenarios-expanded.md`. Flagged as a DISTILL read.

### Fired: multi-stakeholder narrative (both personas active in this slice)

Slice-04 activates P-002 (graph-explorer hat) as primary AND extends P-001 with the same explorer hat. Both exercise the same verbs but from different starting mental models (P-002: pragmatic team-tooling decision; P-001: solo stack commitment).

- **Offer**: extend `docs/product/personas/researcher-tech-lead.yaml` with a `graph-explorer` hat (typical session, anxieties, success signals, UX guardrails), mirroring the slice-03 `federation-reader` and slice-02 `contributor-evaluator` hats.
- **Cost**: ~5 minutes; ~1 page output.
- **Recommendation**: **accept**. Keeps the journey YAML solution-neutral without losing persona-specific guidance.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be added as a `graph-explorer` entry under the existing `hats:` section of `researcher-tech-lead.yaml`. Flagged as a DESIGN read.

### NOT fired: regulatory / compliance complexity

Slice-04 reads only LOCAL, already-present, publicly-signed claims. It introduces no new data collection and no new external surface. The contributor lens aggregates only public signed claims (the J-004 no-surveillance mitigation already applies: the contributor is the SUBJECT of claims, never a controller). Re-evaluate at slice-05 (AppView) when cross-user aggregation widens the surface.

### NOT fired: integration density

Slice-04 adds 2 new internal surfaces (pure `scoring` core + read-side query extension) and 0 new external integrations. No new network surface. Below the threshold.

### Menu action

Three fired offers were **accepted (auto-mode)** in this DISCUSS wave. Two artifacts (`alternatives-considered.md`, `gherkin-scenarios-expanded.md`) are scoped to be produced alongside the DESIGN/DISTILL handoff and are flagged in the read-lists below; the persona-hat extension is a small in-place edit to `researcher-tech-lead.yaml`. (In strict interactive mode these would be offered to the user; in auto-mode the recommended `accept` verdict is taken per the auto-mode product-defaults instruction.)

Telemetry: each `expand` acceptance should ideally emit a `DocumentationDensityEvent`. The helper does not yet exist (greenfield repo); the events are recorded here for retroactive backfill.

| Trigger | Artifact | Should emit |
|---|---|---|
| `cross_context_complexity` | `alternatives-considered.md` | `DocumentationDensityEvent{ feature: openlore-scoring-graph, wave: DISCUSS, expansion: alternatives-considered, accepted: true, ts: 2026-05-28 }` |
| `ac_ambiguity` | `gherkin-scenarios-expanded.md` | `DocumentationDensityEvent{ feature: openlore-scoring-graph, wave: DISCUSS, expansion: gherkin-scenarios-expanded, accepted: true, ts: 2026-05-28 }` |
| `multi_stakeholder_narrative` | persona `graph-explorer` hat | `DocumentationDensityEvent{ feature: openlore-scoring-graph, wave: DISCUSS, expansion: persona-hats, accepted: true, ts: 2026-05-28 }` |

---

## Wave: DISCUSS / [REF] Open Decisions for User

The decisions below are surfaced for user input. Auto-mode default verdicts are
noted (and locked as WDs above where applicable); the user may confirm or
override.

| ID | Decision | Default verdict | Why it matters |
|---|---|---|---|
| OD-GRAPH-1 | WD-8 store revisit: swap `adapter-duckdb` for a graph store vs augment DuckDB with recursive traversal. | **DESIGN's call (WD-78); recommend augment-DuckDB-with-recursive-traversal for slice-04 unless the traversal workload proves it insufficient** | Trade-off: a graph store is the "right" long-term home if traversal/scoring becomes dominant, but a swap is a large migration with `cargo deny` and probe implications (I-11/I-4). Augmenting DuckDB (recursive CTEs) is the smaller change and keeps the single-file simplicity. DESIGN owns the final call; this DISCUSS stays solution-neutral. |
| OD-GRAPH-2 | Retraction/counter-aware scoring: should a claim that has been countered or soft-retracted contribute to weights normally, be down-weighted, or be excluded? | **Contribute normally in slice-04 (with the counter visible in `--explain`/traversal); down-weighting deferred** | Slice-03 keeps countered claims visible (coexist, never overwrite). Down-weighting countered claims is a richer scoring concern; for slice-04 the transparent default is "all signed claims contribute per their confidence; the counter relationship is shown, not silently applied." Surface for user override. |
| OD-GRAPH-3 | Scoring formula constants (author bonus 0.25, triangulation bonus 0.5, bucket thresholds). | **Ship the WD-77 defaults; DESIGN may tune; keep small/closed-form/no-ML** | The exact constants are a product judgment call made in auto-mode. KPI-GRAPH-3 + the day-30 interview surface whether users find the weighting sensible; the constants are trivially revisable in the pure `scoring` core. |
| OD-GRAPH-4 | Explorer-default federation: do the new explorer verbs (`--object`/`--contributor`/`--traverse`/`--weighted`) imply `--federated` (include peers) by default, or require it explicitly as in slice-03? | **Imply federated for explorer verbs (recommended); keep `--federated` accepted for symmetry** | The explorer is reading the WHOLE local graph (own + peers + scraper-signed); requiring `--federated` on every explorer verb is friction. DESIGN owns the final CLI grammar; listed because DISTILL will ask which claims are in scope by default. |

If the user has no objection, the defaults LOCK on handoff to DESIGN
(OD-GRAPH-1 remains DESIGN's storage call per WD-78).

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read (explicit list — every file matters):
  - `feature-delta.md` (this file)
  - Everything in `docs/feature/openlore-scoring-graph/discuss/`:
    - `user-stories.md`
    - `story-map.md`
    - `outcome-kpis.md`
    - `shared-artifacts-registry.md`
    - `journey-explore-the-graph-visual.md`
    - **`alternatives-considered.md`** (fired ask-intelligent expansion — to be produced)
    - **`gherkin-scenarios-expanded.md`** (fired ask-intelligent expansion — to be produced)
  - `docs/product/jobs.yaml` (J-002 deepened with sub-jobs J-002a/b/c)
  - `docs/product/journeys/explore-the-graph.yaml`
  - `docs/product/personas/researcher-tech-lead.yaml` (to be extended with the graph-explorer hat)
  - Slice-01 + slice-03 + slice-02 lock context (do NOT relitigate; treat as inherited inputs):
    - `docs/product/architecture/brief.md` (Component Inventory + cumulative CLI surface + invariants I-1..I-12 + the WD-8 DuckDB-revisit-at-slice-04 note)
    - `docs/feature/openlore-foundation/feature-delta.md` (especially WD-8, WD-9..WD-13)
    - `docs/feature/openlore-federated-read/feature-delta.md` (especially WD-24/WD-25 anti-merging, the ADR-013 verb-amendment precedent) + `docs/feature/openlore-federated-read/design/data-models.md` (the `FederatedRow` non-`Option` author_did discipline + `query_federated_by_subject` precedent)
    - `docs/feature/openlore-github-scraper/feature-delta.md` (WD-58 provenance; scraper-signed claims are normal author claims)
    - ADR-001 (DuckDB store being revisited), ADR-003 (verb contract being amended), ADR-007, ADR-009, ADR-013

- Decide:
  - **WD-8 store revisit (the headline DESIGN decision)**: swap `adapter-duckdb` for a graph store vs augment DuckDB with recursive traversal (CTEs). Trade-off framing in `alternatives-considered.md`; OD-GRAPH-1 default = augment-DuckDB unless the traversal workload proves it insufficient. Revisits ADR-001; document in the slice-04 design + a possible ADR.
  - **Scoring formula's home**: the pure `scoring` core module (ADR-007). Define the `AttributedClaim`, `Contribution`, `WeightedView` ADTs; the `score(claims) -> WeightedView` signature; the intermediate per-claim contributions surface needed for `--explain` (US-GRAPH-005). Formula constants (WD-77) are the SSOT here; tunable but small/closed-form/no-ML.
  - **Graph-query verb/flag ADR amendment (likely ADR-020)**: add `--object`, `--contributor`, `--traverse` (`--depth K`), `--weighted`, `--explain <subject>` to `graph query`. Document verb-grammar consistency with the slice-03 `--federated` precedent. Resolve OD-GRAPH-4 (do explorer verbs imply federated?).
  - **Anti-merging-preserved aggregate query shape**: the read-side method(s) that supply attributed claims to the pure scoring core (mirroring slice-03's `query_federated_by_subject`), the bounded-traversal query, and the `--object`/`--contributor` dimension queries. Every shape MUST preserve per-row attribution (non-`Option` `author_did`); the `xtask check-arch` `no_cross_table_join_elides_author` rule extends to scoring queries. Any new port surface ships `probe()` (ADR-009).
  - **Display-only discipline for weights**: confirm `adherence_weight` and `weight_bucket` have no persistence path (WD-72); the lexicon-conformance / no-persist test (extending the slice-01/03 confidence-bucket test) covers them.
  - **Component Inventory update**: if a new `scoring` crate (pure) and/or a graph-store adapter crate is added, the brief's Component Inventory gains rows at finalize. If scoring lives inside `claim-domain` and traversal augments `adapter-duckdb`, the crate count may stay at 10.

- Constraints inherited from this DISCUSS (DO NOT relitigate without coming back to PO):
  - **WD-71**: scoring is transparent + auditable; small closed-form formula; NO ML; formula displayed; `--explain` reproduces arithmetic.
  - **WD-72**: weights/buckets are DERIVED + DISPLAY-ONLY; never persisted/signed/published.
  - **WD-73**: anti-merging extends to aggregates; a score decomposes to per-author per-cid contributions; every row keeps its author DID.
  - **WD-74**: sparse renders sparse; thin evidence labeled [SPARSE] with an honesty line; no manufactured confidence.
  - **WD-75**: ship dimensions by subject (inherited), by object (new), by contributor (new).
  - **WD-76**: `--traverse` ships; bounded default depth 2; no invented edges.
  - **WD-77**: the default scoring formula shape (constants tunable, function small/closed-form/no-ML).
  - **WD-78**: the WD-8 store revisit is DESIGN-internal; not a user story; user-visible contracts are storage-neutral.
  - **WD-79**: local scope only; no multi-user/cohort aggregation (slice-05).

### To DEVOPS (nw-platform-architect, parallel)

- Read: `outcome-kpis.md` (Handoff to DEVOPS section).
- Deliver:
  - Instrumentation plan for KPI-GRAPH-1..6 (especially the `graph.connection.surfaced` tracing event for KPI-GRAPH-1 and the `graph.query.duration_seconds` histogram for KPI-GRAPH-6, both privacy-preserving — structural counts only, never claim contents).
  - Dashboards for KPI-GRAPH-1 (% of explorer sessions with >=1 connection surfaced per 30-day window) and KPI-GRAPH-6 (P50/P95 of `graph.query.duration_seconds` per claim-count bucket).
  - Alerting on KPI-GRAPH-2, KPI-GRAPH-3, KPI-GRAPH-4 != 100% (release-blocking); informational alert on KPI-GRAPH-6 P95 > 5s for the <=200-claim bucket.
  - No new external service to provision (read-only, local-first); confirm the scoring/traversal path adds no network call (the local-first guardrail, extending slice-01 KPI-5).

### To DISTILL (nw-acceptance-designer)

- Read:
  - `docs/product/journeys/explore-the-graph.yaml` (embedded Gherkin per step)
  - `docs/feature/openlore-scoring-graph/discuss/user-stories.md` (UAT scenarios per story)
  - `docs/feature/openlore-scoring-graph/discuss/shared-artifacts-registry.md` (integration gates 1-6)
  - **`docs/feature/openlore-scoring-graph/discuss/gherkin-scenarios-expanded.md`** (anxiety + habit scenarios; some will carry `# DISTILL: confirm` flags for verb-shape / store-choice resolution)
- Build executable acceptance tests including:
  - **Query-by-dimension**: `graph query --object` and `--contributor` return attributed, grouped results (US-GRAPH-001/002).
  - **Traverse**: bounded-depth traversal surfaces cross-project spans; every edge maps to a signed claim; no invented edges (`traversal_invents_no_edges`) (US-GRAPH-004).
  - **Weighted-scoring**: ranking equals the documented formula; weight reproducible (`weight_equals_formula`) (US-GRAPH-003/005).
  - **Sparse-graph-honesty**: thin subgraphs render [SPARSE] with the honesty line; no manufactured confidence (`sparse_renders_sparse`) (US-GRAPH-003).
  - **Anti-merging-in-aggregates**: every weighted/traversed aggregate decomposes to per-author per-cid contributions (`scoring_aggregate_preserves_attribution`); the `no_cross_table_join_elides_author` rule covers scoring queries (US-GRAPH-003/004/005).
  - **Display-only discipline**: `weight_and_bucket_never_persisted` (US-GRAPH-003/006).
- The `# DISTILL: confirm` comments throughout `gherkin-scenarios-expanded.md` mark behaviors implied by the requirements but not yet locked (e.g. the exact `--depth` default, whether explorer verbs imply `--federated`, the final ADR-020 verb/flag shape). Each must be resolved against DESIGN's final decisions before building tests.

### Handoff-ready?

**YES.** All WD-69..WD-79 LOCKED in this DISCUSS; three ask-intelligent
expansions accepted (auto-mode) — `alternatives-considered.md` and
`gherkin-scenarios-expanded.md` scoped for production alongside the DESIGN/DISTILL
handoff, persona `graph-explorer` hat to be added in place; lean Tier-1 output
stands. Four Open Decisions (OD-GRAPH-1..4) have auto-mode default verdicts and
may proceed unless the user overrides; none are blocking for DESIGN to start
(OD-GRAPH-1, the WD-8 store revisit, is DESIGN's call by design).

DESIGN + DEVOPS may proceed in parallel; DISTILL has the scenarios it needs.
