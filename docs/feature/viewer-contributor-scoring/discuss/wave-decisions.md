# Wave Decisions: viewer-contributor-scoring (slice-09, DISCUSS)

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · Date: 2026-06-05
> Feature type: User-facing · JTBD: YES (J-002, esp. J-002c) · UX depth: Lightweight · Walking skeleton: YES (thin)
> Brownfield DELTA on slices 04 (scoring-graph) / 06 (htmx-scraper-viewer) / 07 (viewer-htmx-swaps) / 08 (viewer-network-search).

This slice adds a **contributor-scoring view** to the read-only `openlore ui` viewer:
a `GET /score?contributor=<did>` route that reads the LOCAL claim graph for a
contributor, runs the slice-04 PURE `scoring` scorer, and renders the contributor's
**transparent adherence score + its component breakdown** as HTML, with an htmx
fragment swap. It is the **browser UI for `graph query --contributor <did> --weighted
--explain`** (J-002, especially sub-job **J-002c** — transparent, auditable,
reproduce-by-hand weighting). No new product job is created — every story traces to
the already-validated **J-002** ("Explore the philosophy graph to inform a decision").

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`).
Proceeded without re-running JTBD; J-002 + sub-job J-002c are validated (slice-04).

## Scope Assessment: PASS — 4 stories (3 user-visible + 1 infra), 1 bounded context (the viewer's `/score` surface), estimated ~7 days

Carpaccio gate (5 taste tests):

- **Stories**: 4 (1 infra walking-skeleton-enabler + 3 user-visible). Well within <=10. PASS.
- **Bounded contexts**: 1 — the viewer `/score` surface. It adds ONE new capability
  (a LOCAL contributor-scoring read effect in the viewer process — a read + pure
  compute over the local DuckDB store, distinct from the slice-06 GithubPort and the
  slice-08 network IndexQueryPort). It REUSES the slice-04 pure `scoring` crate
  (`score` + `WeightedView` + `Contribution`) verbatim and the slice-06/07
  page=chrome+fragment render pattern. PASS.
- **Walking-skeleton integration points**: US-CS-001 needs (1) the new `/score` route
  in the viewer, (2) a local contributor-scoring read effect (reuse the slice-04
  scoring-feed read over the viewer's store?), (3) the slice-04 pure `scoring::score`
  core, (4) the slice-06/07 fragment-render fork. That is 4 — within <=5. PASS (3 of
  the 4 are REUSES — the score math is NOT reimplemented).
- **Estimated effort**: ~7 days (within <=2 weeks). PASS.
- **Multiple independent outcomes**: NO — all stories serve the single outcome "see a
  contributor's transparent, auditable score + breakdown in the browser." PASS.
- **Verdict**: RIGHT-SIZED. Single slice = single sibling feature. The thing that
  would make it oversized — building a write/sign affordance into the viewer, or
  reimplementing the scoring math, or persisting a derived score — is explicitly OUT
  of scope (the viewer stays read-only; the score is display-only + recomputed per
  query; the math is the reused slice-04 pure core).

## Locked decisions (WD-CS-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-CS-1 | slice-09 ships as a SIBLING feature `viewer-contributor-scoring`; it is a brownfield DELTA on slices 04/06/07/08. US-CS-001 is the (thin) walking skeleton. | Inherits WD-9 carpaccio split. One slice = one feature. | LOCKED |
| WD-CS-2 | Persona = **P-001 Senior Engineer Solo Builder ("Maria", the node operator)** — the SAME persona as slices 06/07/08. She wears the graph-explorer hat at her own loopback viewer. slice-04 framed P-002 (Researcher/Tech Lead) primary for the CLI graph-explorer; the BROWSER viewer's operator is P-001 — she now sees a contributor's transparent score from the same read-only UI she already uses. | The viewer is P-001's surface (slices 06/07/08). | LOCKED |
| WD-CS-3 | The viewer stays **READ-ONLY**: scoring is a READ + pure COMPUTE over the local store. No new write/sign route; the viewer holds no signing key. The derived score is DISPLAY-ONLY and never persisted/signed/published (inherits I-VIEW-1/2/3 / KPI-VIEW-2 / WD-72 display-only-bucket discipline). | The read-only invariant is cardinal across slices 06/07/08. Scoring is a pure projection over already-signed claims; no write seam exists. | LOCKED |
| WD-CS-4 | **Transparency is load-bearing (the J-002c thesis)**: the view MUST render the score's COMPONENT BREAKDOWN — the `--explain` decomposition (per-claim confidence contribution, author-distinct bonus, cross-project triangulation bonus, running sum == displayed weight), each contribution named back to its author DID + cid — NEVER an opaque single number. Inherits KPI-GRAPH-3 (reproduce-by-hand) + KPI-GRAPH-2 (anti-merging in aggregates). | Directly mitigates the J-002 anxiety ("biased / speculative / sparse data → bad call"). A score with no visible breakdown re-creates the opaque-aggregator failure the product exists to avoid. | LOCKED |
| WD-CS-5 | **Sparse renders sparse** (inherits slice-04 KPI-GRAPH-4 / WD-74): a thin subgraph (single-claim / single-author / no cross-project span) renders the `[SPARSE]` bucket + the "based on N claim(s) by M author(s) — treat as a lead, not a conclusion" honesty line — regardless of weight magnitude. A single high-confidence opinion is NEVER dressed up as Strong. | Epistemic honesty. The breadth guard is the load-bearing slice-04 invariant; the browser surface inherits it verbatim from the pure core's `WeightBucket`. | LOCKED |
| WD-CS-6 | **Pure-scorer reuse**: the view REUSES the slice-04 `scoring` crate (`score(&feed, &ScoringConfig::DEFAULT) -> WeightedView`, `WeightedPairing`, `WeightBucket`, `Contribution`) verbatim. The viewer does NOT reimplement the scoring math; it PROJECTS the pure core's output to HTML. The formula constants stay the SSOT in `ScoringConfig::DEFAULT` (WD-77). | One formula, one source of truth (KPI-GRAPH-3). A second formula in the viewer would be unauditable + drift-prone. | LOCKED |
| WD-CS-7 | **Confidence + weight rendered VERBATIM** (inherits FR-VIEW-8 / KPI-4): every confidence shown is the stored `f64` rendered verbatim (`0.86`, never `0.9`/`86%`); the displayed weight is the consumed weight (no bucket-midpoint rounding — Gate 6). | The displayed number is the scored number; zero silent normalization. | LOCKED |
| WD-CS-8 | **Local-first / offline** (inherits KPI-5 / KPI-GRAPH-6): the contributor score is computed over the LOCAL DuckDB store/graph — the `/score` view works fully offline (no network), DISTINCT from the slice-06 `/scrape` (GitHub) and slice-08 `/search` (network index) routes, which are the only network-requiring surfaces. | Reinforces local-first. The score read is a local store read + pure compute; the network being down never degrades it. | LOCKED |
| WD-CS-9 | **Progressive enhancement** (inherits I-HX-1..5 / KPI-HX-G1): `/score` serves a complete full page WITHOUT `HX-Request` and a fragment of the SAME score region WITH it (the slice-07 `Shape::from_request` fork; page=chrome+fragment). htmx stays the vendored, SHA-256-pinned local asset (I-HX-2 / KPI-HX-G2). | Reuses the slice-07 `Shape` fork verbatim. The no-JS full page is the contract; the swap is a nicety. | LOCKED |
| WD-CS-10 | **Attribution preserved; zero new persisted types; loopback bind unchanged.** The breakdown names the contributing claims/authors (auditable back to attributed claims); the score is computed per-query and never persisted; the bind stays 127.0.0.1-only (inherits I-VIEW-4 / BR-VIEW-2 / WD-NS-7). | A score is an AGGREGATE VIEW that never loses per-author attribution (extends slice-03 I-FED-1 / slice-04 WD-73). Discovery is a per-query read surface. | LOCKED |

## Risks logged

- **Sparse-subgraph honesty under a NEW renderer (KPI-GRAPH-4 risk)**: the breadth
  guard lives in the pure core (`weight_bucket`), so the bucket decision is inherited
  — but the BROWSER renderer must surface `[SPARSE]` + the honesty line legibly (not
  bury it). Mitigation: the honesty line is a release-gate AC (US-CS-003); the
  renderer projects the pure-core `WeightBucket` + the honesty string, never recomputes.
- **Local-score read shape (OD-CS-2)**: whether the viewer's read-only `StoreReadPort`
  is extended with a contributor scoring-feed read, or a new viewer-process port reads
  the local graph, or the slice-04 `query_attributed_for_scoring` path is reused, is a
  DESIGN call. The new read must hold NO signing/identity/PDS surface (mirror the
  slice-06 GithubPort + slice-08 IndexQueryPort capability boundary) and read the LOCAL
  store only (no network).
- **Transparency legibility (J-002c)**: an opaque-number regression (showing the score
  but hiding the breakdown) silently re-creates the aggregator failure mode. Mitigation:
  the component breakdown is a BLOCKING AC on the walking skeleton (US-CS-001/002), not
  a later polish; the `--explain`-equivalent decomposition ships in Release 1.
- **DISCOVER + DIVERGE skipped** (same as all prior slices). J-002/J-002c is the
  validated source job; the four-forces for J-002 were done in slice-04's DISCUSS. No
  prior validation interviews specific to the BROWSER score surface — mitigated by the
  inherited KPI-GRAPH / KPI-VIEW behavioral hypotheses + the day-30 studies.
- **Scope creep into cross-user scoring / a standalone web AppView**: held off by
  WD-CS-3 (read-only, local-only) + WD-CS-10 (nothing persisted). The `/score` view is
  a render surface over the local graph + the pure scorer, not a new app.

## DIVERGE note

No DIVERGE artifacts exist for this slice
(`docs/feature/viewer-contributor-scoring/diverge/` absent) — consistent with all
prior OpenLore slices. Journey work is grounded in the validated J-002 / J-002c job
statement (slice-04) and the slice-06/07/08 viewer journey.
</content>
</invoke>
