# Story Map: openlore-scoring-graph (slice-04)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## User: P-002 Researcher / Tech Lead (graph-explorer hat)

Secondary persona: P-001 Senior Engineer Solo Builder (wears the graph-explorer
hat when choosing a stack or community to commit to).

## Goal

Explore the LOCAL federated philosophy graph — query by subject, object, and
contributor; traverse the connections between them; and see transparent
adherence weights — so a tech/community decision becomes defensible to a team,
without sparse or speculative data being mistaken for confident truth.

## Backbone

| Query by dimension | Traverse edges | Weight / score | Audit the score |
|------|------|------|------|
| Query by subject (project) | Traverse contributor<->project<->philosophy | Rank by adherence weight | `--explain` per-claim arithmetic |
| Query by object (philosophy) | (Bounded depth; no invented edges) | (Transparent formula; no ML) | (Decomposes to author+cid) |
| Query by contributor (DID) | (Surface cross-project span) | (Sparse renders as sparse) | |

---

## Walking Skeleton (slice-04 walking skeleton)

The minimum slice that exercises explore-the-graph end-to-end over the local
federated graph:

1. **Query by dimension** — `openlore graph query --object <philosophy>` returns
   the projects embodying it, every claim attributed to its author DID.
2. **Weighted view** — `openlore graph query --object <philosophy> --weighted`
   ranks those projects by a transparent, auditable adherence weight, with
   sparse subgraphs rendered honestly as sparse.

This skeleton validates:

- The new query dimensions read the existing slice-01/02/03 stores (own + peer +
  scraper-signed claims) without a new write surface.
- The pure scoring core produces a reproducible weight from attributed claims.
- The anti-merging-in-aggregates invariant holds (a weight decomposes to
  per-author, per-cid contributions).
- Scoring transparency holds (formula displayed; sparse renders sparse; weights
  display-only, never persisted).

It does NOT include `--contributor`, `--traverse`, or `--explain` — those are
deliberately later releases. The walking skeleton is the thinnest proof that
"a transparent weighted view over the existing graph" holds.

---

## Release 1 — Walking Skeleton (target outcome: a transparent weighted view over the graph works end-to-end)

| Story | Target outcome | KPI |
|---|---|---|
| US-GRAPH-001 | Query the graph by object (philosophy) and by subject, attribution preserved | KPI-GRAPH-2 (anti-merging in aggregates, baseline), KPI-GRAPH-3 (transparency) |
| US-GRAPH-003 | Weighted / scored view with a transparent formula; sparse renders sparse | KPI-GRAPH-1 (non-obvious connection), KPI-GRAPH-3 (transparency), KPI-GRAPH-4 (sparse honesty) |
| US-GRAPH-006 | Bootstrap the pure `scoring` core + read-side query extensions (`@infrastructure`) | supports KPI-GRAPH-1..4 |

**Rationale**: this is the minimum bundle that disproves the slice-04 hypothesis
if it fails. Without query-by-dimension + a transparent weighted view, J-002's
"see weights/confidence so I can distinguish well-supported claims from
speculation" is unmet. The riskiest assumption — "a small transparent formula
produces a weight users trust, and sparse data does not mislead" — is validated
here.

**Demo gate (Phase 3.5)**: User runs
`openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted`
over a graph containing own + peer + scraper-signed claims. The output ranks
projects by a weight whose formula is printed, each weight decomposes to
per-author claims, and a single-claim philosophy is labeled [SPARSE].

---

## Release 2 — Connection discovery (target outcome: J-002 "surface a non-obvious connection" validated)

| Story | Target outcome | KPI |
|---|---|---|
| US-GRAPH-002 | Query by contributor (DID) to read one developer's full reasoning trail | KPI-GRAPH-2 (attribution — the contributor lens is where it is sharpest) |
| US-GRAPH-004 | Traverse contributor<->project<->philosophy edges to surface cross-project span | KPI-GRAPH-1 (the "aha" — surface a non-obvious connection in one query) |

**Rationale**: traversal (US-GRAPH-004) is the Discovery-Joy peak — it surfaces
the non-obvious connection that is the headline J-002 success signal. It is
sequenced AFTER the walking skeleton because traversal builds on the same
attributed-read foundation, and if Release 1 has a latent attribution or scoring
bug it surfaces there first rather than corrupting traversal output. The
contributor lens (US-GRAPH-002) pairs naturally with traversal (it answers "who
is this contributor that spans my projects?").

**Demo gate**: Maria runs
`openlore graph query --object org.openlore.philosophy.dependency-pinning --traverse`
and sees a "Connections found" callout naming a peer who spans two of her
candidate projects — a connection she could not get from `gh search` + READMEs.

---

## Release 3 — Auditability drill-down (target outcome: scoring transparency made reproducible)

| Story | Target outcome | KPI |
|---|---|---|
| US-GRAPH-005 | `--explain <subject>` reproduces the per-claim weight arithmetic | KPI-GRAPH-3 (transparency — reproducible by hand) |

**Rationale**: `--explain` can ship LAST because the weighted view (Release 1)
is already transparent at the formula level (it prints the formula + the inputs).
`--explain` deepens transparency from "shows the formula" to "reproduce the
number by hand", which is the strongest form of the J-002 auditability promise
but not required for the first weeks of dogfooding. Worst case without it ("I
trust the formula but cannot drill into one project's arithmetic") is survivable;
worst case for Release 1 (an opaque or attribution-losing weight) is not.

**Demo gate**: Maria runs `--explain github:denoland/deno` and the output
enumerates each contributing claim with its author DID, CID, and the arithmetic
that sums to the displayed weight.

---

## Priority Rationale

Priority order: **Release 1 (Walking Skeleton) > Release 2 (Connection discovery) > Release 3 (Auditability drill-down)**.

The ordering is set by outcome impact and risk-of-failure consequence, NOT by
feature volume or implementation order:

1. **Release 1 first** because if a transparent weighted view over the existing
   graph does not work — or if it loses attribution or manufactures confidence
   from sparse data — the slice-04 thesis (J-002: distinguish well-supported
   claims from speculation) is disproven. The riskiest assumption is "a small
   closed-form formula produces a weight users trust AND sparse data is rendered
   honestly." Validating that is the walking skeleton (per `nw-user-story-mapping`
   "Riskiest Assumption First"). US-GRAPH-006 (`@infrastructure`) is bundled here
   because the pure scoring core and read-side query extensions it provides are
   prerequisites for every user-visible story.

2. **Release 2 second** because connection discovery (traversal + contributor
   lens) delivers the headline J-002 success signal — "surface a non-obvious
   connection in one query." It is the highest-value behavior change after the
   walking skeleton, but it benefits from being built on a stable, trusted
   weighted-read foundation. If Release 1 has a latent attribution bug, it
   surfaces during Release 1 rather than producing misleading traversal trees.

3. **Release 3 third** because `--explain` is an auditability deepening, not a
   primary outcome. The journey is usable and transparent without it for the
   first weeks of dogfooding (the formula and its inputs are already shown in
   Release 1). The worst case ("I can see the formula but cannot drill into one
   project's per-claim arithmetic yet") is survivable; the worst case for
   Release 1 (opaque or attribution-losing weights) is unsurvivable. Hence the
   priority order.

This ordering preserves the carpaccio principle: each release is independently
demo-able and delivers a verifiable working behavior. Release 1 alone is a
shippable end-to-end slice (transparent weighted exploration). Release 2 adds
connection discovery. Release 3 adds reproducible auditability.

---

## What is NOT in scope (explicitly deferred)

These were considered and deferred — most to the slice-05 AppView, NOT just to a
later release of this feature:

| Out-of-scope | Why deferred | Future home |
|---|---|---|
| Multi-user / cohort aggregation (scoring across many users' graphs) | Slice-04 scores the LOCAL graph only (own + pulled peers + scraper-signed). Cross-user aggregation needs an indexer service | `openlore-appview-search` (slice-05) |
| A graph-DB migration as a user story | The storage choice (swap-to-graph-store vs augment-DuckDB-with-recursive-traversal) is DESIGN's internal call (WD-8 revisit); it is invisible to the user | DESIGN wave (this slice) |
| Persisting weights/scores | Weights are DERIVED + DISPLAY-ONLY (WD-72). Persisting them would create a stale-score / trust hazard | Never (deliberate) |
| ML / learned scoring model | Would make scores unauditable, violating the J-002 transparency promise (WD-71) | Never (deliberate) |
| Trust weighting by author identity (per-author trust scores) | Slice-04's weighting is evidence-based (count x confidence x triangulation); per-author trust is a separate JTBD | Post-slice-05 |
| User-configurable scoring formula / weights | Slice-04 ships ONE small transparent default formula; configurability is premature before the default is validated | Post-MVP (revisit after KPI-GRAPH-3) |
| Pushing the scored view as a shareable link | Sharing a query result is a slice-05 AppView concern (J-004 success signal "shareable as a link to a query") | Slice-05 |
| New network / write surface | Slice-04 is read/view only over already-present claims | Out of scope by design |
