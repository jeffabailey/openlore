# Visual Journey — explore-the-graph (weighted, traversal-aware)

- **Feature**: openlore-scoring-graph (slice-04)
- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)
- **Persona**: P-002 Researcher / Tech Lead (graph-explorer hat) — primary; P-001 (Solo Builder) wears the same hat when choosing a stack
- **Job**: J-002 (sub-jobs J-002a query-by-dimension, J-002b traverse-edges, J-002c weighted-scoring)
- **Structured schema**: `docs/product/journeys/explore-the-graph.yaml`

This document is the human-readable companion to the YAML schema. It captures
the visual flow, the emotional arc, and the per-step TUI mockups in one place
so the reviewer and DESIGN wave can read the journey without context-switching.

The graph being explored is the LOCAL federated graph already present on disk:
the user's own authored claims (slice-01), peer claims pulled via slice-03
(`peer_claims`), and claims the user signed from scraper candidates (slice-02).
**This journey adds NO new write surface and NO new network surface.** It is a
pure read/view over claims that are already signed and stored. Scoring does not
create, sign, or publish a claim.

## Flow at a glance

```
        Trigger: Maria must justify a stack choice to her team by Friday
                              |
                              v
+------------+   +------------+   +------------+   +------------+
| Step 1     |-->| Step 2     |-->| Step 3     |-->| Step 4     |
| query by   |   | query by   |   | traverse   |   | weighted   |
| subject    |   | object /   |   | edges      |   | / scored   |
| (project)  |   | contributor|   | (--traverse|   | (--weighted|
|            |   |            |   |  optional) |   |  view)     |
+------------+   +------------+   +------------+   +------------+
  Curious          Orienting       Connecting      Defensible-
                                   (the "aha")     confident
```

## Emotional arc — curiosity-to-defensible-decision (with a transparency buffer)

Pattern: **Discovery Joy** layered over **Confidence Building**.

- **Entry**: Curious-but-skeptical. Maria has heard "the Rust community values
  X" but cannot articulate WHY a community feels right. She also carries the
  J-002 anxiety: "What if the graph is sparse, biased toward popular projects,
  or full of speculative claims and I make a bad call?"
- **Middle**: Orienting -> Connecting. Querying by dimension orients her;
  traversing edges surfaces the non-obvious connection (the "aha" — a
  contributor who spans three projects she's evaluating). This is the
  Discovery-Joy peak.
- **The transparency buffer (load-bearing)**: when a weight/score appears, it is
  NEVER an opaque number. The score line is ALWAYS accompanied by its formula
  inputs (how many claims, by how many distinct authors, across how many
  projects, at what confidence). A sparse subgraph renders AS sparse — the
  output says "based on 1 claim by 1 author" rather than dressing thin evidence
  in a confident-looking number. This buffer is what converts the anxiety into
  trust.
- **End**: Defensible-confident. Maria walks away able to say "I picked Rust
  because dependency-pinning, memory-safety, and documentation-first
  philosophies are well-supported here — 14 claims across 6 projects by 9
  distinct authors — and here are the people who back that up." The decision is
  defensible to teammates with shared vocabulary.

The arc deliberately puts the weighted/scored view LAST (step 4), AFTER the
user has seen the raw attributed claims (steps 1-3). The user builds trust in
the underlying data BEFORE being shown an aggregate of it. A score shown before
the user trusts the claims would re-trigger the aggregator anxiety.

## Step 1 — query by subject (project)

```
$ openlore graph query --subject github:rust-lang/cargo

Claims about github:rust-lang/cargo (3 found across 2 authors)
===============================================================

Author: did:plc:maria-test (you)
  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    confidence  0.78  (well-evidenced)
    cid         bafy...m9pq

Author: did:plc:rachel-test (subscribed peer)
  - embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
    confidence  0.91  (triangulated)
    cid         bafy...n4ka
  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    confidence  0.65  (weighted)
    cid         bafy...x7ts

Each claim is attributed to its author DID. No claims are merged.

Tip: explore the philosophy across projects with
     `openlore graph query --object org.openlore.philosophy.dependency-pinning`
```

**Note**: this is the slice-01 `graph query --subject` surface UNCHANGED at the
read level (slice-03 added `--federated`; slice-04 makes the federated view the
natural default for the explorer hat — DESIGN owns whether `--federated` stays
an explicit flag or the explorer verbs imply it). The new surfaces are the
`--object` and `--contributor` dimensions (step 2), `--traverse` (step 3), and
`--weighted` (step 4).

**Feels**: entry Curious -> exit Orienting.

## Step 2 — query by object (philosophy) and by contributor

Query by OBJECT (philosophy) — "which projects embody dependency-pinning, and
who says so?":

```
$ openlore graph query --object org.openlore.philosophy.dependency-pinning

Projects embodying org.openlore.philosophy.dependency-pinning
(4 claims across 3 subjects, 3 distinct authors)
=============================================================

Subject: github:rust-lang/cargo
  did:plc:rachel-test (subscribed peer)   confidence 0.91 (triangulated)  bafy...n4ka

Subject: github:denoland/deno
  did:plc:tobias-test (subscribed peer)   confidence 0.55 (weighted)      bafy...d3no
  did:plc:maria-test  (you)               confidence 0.40 (speculative)   bafy...mz01

Subject: github:nixos/nixpkgs
  did:plc:rachel-test (subscribed peer)   confidence 0.88 (well-evidenced) bafy...nx99

Grouped by subject; every claim retains its author DID. No claims are merged.

Tip: see who spans these projects with
     `openlore graph query --object ... --traverse`
```

Query by CONTRIBUTOR (DID) — "what does Rachel claim, across everything?":

```
$ openlore graph query --contributor did:plc:rachel-test

Claims authored by did:plc:rachel-test (rachel.example.com, subscribed peer)
(5 claims across 4 subjects)
============================================================================

github:rust-lang/cargo   embodiesPhilosophy  dependency-pinning    0.91 (triangulated)   bafy...n4ka
github:rust-lang/cargo   embodiesPhilosophy  reproducible-builds   0.65 (weighted)       bafy...x7ts
github:nixos/nixpkgs     embodiesPhilosophy  dependency-pinning    0.88 (well-evidenced) bafy...nx99
github:tokio-rs/tokio    embodiesPhilosophy  memory-safety         0.72 (well-evidenced) bafy...tk44
github:serde-rs/serde    embodiesPhilosophy  documentation-first   0.50 (weighted)       bafy...sd12

All claims authored by ONE DID (did:plc:rachel-test). This is one developer's
reasoning trail, not a community consensus.
```

**Why the "one developer's reasoning trail, not a community consensus" footer
is load-bearing**: the contributor lens is exactly where the anti-merging
anxiety is sharpest — a list of one person's claims could be mistaken for
authoritative truth. The footer keeps the J-002 framing honest.

**Feels**: entry Orienting -> exit Orienting (deeper). The query-by-dimension
surfaces are the orientation phase; the connection comes in step 3.

## Step 3 — traverse edges (contributor <-> project <-> philosophy)

```
$ openlore graph query --object org.openlore.philosophy.dependency-pinning --traverse

Traversal from philosophy: org.openlore.philosophy.dependency-pinning
(depth 2: philosophy -> projects -> contributors)
=====================================================================

org.openlore.philosophy.dependency-pinning
 |
 +- github:rust-lang/cargo
 |    +- did:plc:rachel-test (subscribed peer)   0.91 (triangulated)
 |
 +- github:nixos/nixpkgs
 |    +- did:plc:rachel-test (subscribed peer)   0.88 (well-evidenced)
 |
 +- github:denoland/deno
      +- did:plc:tobias-test (subscribed peer)   0.55 (weighted)
      +- did:plc:maria-test  (you)               0.40 (speculative)

Connections found:
  * did:plc:rachel-test spans 2 of these projects (cargo, nixpkgs)
    -> a contributor whose dependency-pinning claims triangulate across projects.

Each edge is one signed claim by one author. Traversal does not invent edges;
it only walks claims that already exist. No claims are merged.

Tip: weigh these by support with
     `openlore graph query --object ... --traverse --weighted`
```

**Why "Traversal does not invent edges" is load-bearing**: a graph traversal
that interpolated or inferred edges would be fabricating reasoning the author
never signed. Every displayed edge MUST correspond to exactly one signed claim.
This is the J-002 auditability promise carried into multi-hop.

**Why the "Connections found" callout is the Discovery-Joy peak**: this is the
"surface a non-obvious connection in one query" success signal from jobs.yaml.
Rachel spanning two of Maria's candidate projects is the insight Maria could not
get from `gh search` + skimming READMEs.

**Feels**: entry Orienting -> exit Connecting (the "aha").

## Step 4 — weighted / scored view

```
$ openlore graph query --object org.openlore.philosophy.dependency-pinning --traverse --weighted

Weighted view: org.openlore.philosophy.dependency-pinning
=========================================================

Projects ranked by adherence weight (transparent formula below):

  1. github:rust-lang/cargo      weight 1.82   [STRONG]
       claims  : 1   authors: 1   max-confidence 0.91 (triangulated)
       also-claimed-by: did:plc:rachel-test spans cargo + nixpkgs

  2. github:nixos/nixpkgs        weight 1.76   [STRONG]
       claims  : 1   authors: 1   max-confidence 0.88 (well-evidenced)
       also-claimed-by: did:plc:rachel-test spans cargo + nixpkgs

  3. github:denoland/deno        weight 0.95   [MODERATE]
       claims  : 2   authors: 2   max-confidence 0.55 (weighted)
       multi-author: 2 distinct authors raise triangulation

How weight is computed (auditable, no ML):
  weight = sum over claims of [ confidence
                                x author_distinct_bonus
                                x cross_project_triangulation_bonus ]
  - author_distinct_bonus        : 1.0 for the first author, +0.25 per add'l distinct author on the SAME (subject,object)
  - cross_project_triangulation  : +0.5 if the SAME author asserts this philosophy on >=2 distinct subjects
  - bucket labels [STRONG]/[MODERATE]/[SPARSE] are DISPLAY-ONLY; never persisted.

Run with `--explain <subject>` to see the per-claim arithmetic.

Note: weights are a DISPLAY-ONLY aggregate VIEW computed at query time from the
claims above. They are NOT stored, NOT signed, and NOT published. Re-running
after a `peer pull` may change them. Each weight decomposes to the exact claims
that produced it; nothing is merged or invented.
```

A sparse subgraph renders HONESTLY as sparse:

```
$ openlore graph query --object org.openlore.philosophy.actor-model --weighted

Weighted view: org.openlore.philosophy.actor-model
==================================================

  1. github:tokio-rs/tokio       weight 0.50   [SPARSE]
       claims  : 1   authors: 1   max-confidence 0.50 (weighted)
       (!) SPARSE: based on 1 claim by 1 author on 1 project.
           This is one developer's opinion, not a triangulated signal.
           Treat as a lead to investigate, not a defensible conclusion.

How weight is computed (auditable, no ML): [formula as above]

Only 1 claim matched. A weighted view over thin evidence is shown honestly as
SPARSE; it does not manufacture confidence. Pull more peers or author your own
claims to enrich this philosophy's subgraph.
```

**Why the SPARSE rendering is the most load-bearing UX in the slice**: it is the
direct mitigation of the J-002 anxiety ("what if I make a bad call on sparse
data?"). The system MUST visibly degrade — a single-claim philosophy must look
thin, never dressed up as a confident score. The `[SPARSE]` bucket plus the
"(!) based on 1 claim by 1 author" line is the contract.

**Why `--explain` exists**: the formula is auditable in aggregate, but the user
can drill into the exact per-claim arithmetic. Transparency is not just "show
the formula" — it is "let the user reproduce the number by hand."

**Feels**: entry Connecting -> exit Defensible-confident.

## Shared artifacts highlighted

| Artifact | First appears | Reused at | Risk |
|---|---|---|---|
| `subject` (project URI) | step 1 | steps 2, 3, 4 | HIGH — drift breaks edge identity |
| `object` (philosophy URI) | step 2 | steps 2, 3, 4 | HIGH — drift breaks philosophy grouping |
| `author_did` (contributor) | step 1 | steps 2, 3, 4 | HIGH — drift = attribution loss (anti-merging) |
| `claim_cid` | step 1 | steps 3, 4 (`--explain`) | HIGH — the auditable unit of a weight |
| `confidence` (numeric) | step 1 | step 4 (formula input) | HIGH — the load-bearing scoring input; numeric-only persisted (WD-10) |
| `adherence_weight` | step 4 | step 4 only (`--explain`) | DERIVED + DISPLAY-ONLY — never persisted (WD-72) |
| `weight_bucket` ([STRONG]/[MODERATE]/[SPARSE]) | step 4 | step 4 only | DERIVED + DISPLAY-ONLY — never persisted |

Full registry: `shared-artifacts-registry.md` (this directory).

## Scoring-transparency + anti-merging guarantees (cross-cutting)

Two load-bearing invariants span the whole slice — called out separately
because they are not step-local concerns.

### Scoring transparency (J-002 anxiety mitigation)

- **At computation**: the formula is a SMALL closed-form function of
  `count x confidence x triangulation`. NO ML, NO opaque model. It lives in a
  pure `scoring` core module (`fn score(claims) -> WeightedView`) that is
  trivially unit/mutation-testable.
- **At display**: every weight is shown WITH its inputs (claim count, distinct
  author count, cross-project span, max confidence) AND the formula. `--explain`
  reproduces the per-claim arithmetic.
- **At storage**: weights are DERIVED + DISPLAY-ONLY. They are computed at query
  time and NEVER persisted, signed, or published (WD-72). Display buckets
  ([STRONG]/[MODERATE]/[SPARSE]) inherit the WD-10 display-only rule, exactly as
  confidence buckets do.
- **Sparse renders sparse**: a thin subgraph is visibly labeled SPARSE with a
  "based on N claims by M authors" line. The system never manufactures
  confidence from thin evidence.

### Anti-merging in aggregates (slice-03 I-FED-1 carried forward)

A score is an AGGREGATE VIEW, never a merge that loses attribution.

- **At ingest**: nothing new is ingested — the graph reads slice-01/02/03 stores
  as-is. The slice-03 store separation (`author_claims` vs `peer_claims`) holds.
- **At aggregate computation**: a weight is computed FROM attributed claims, but
  the underlying claims remain individually addressable. The weighted view always
  decomposes (`--explain`) to per-claim, per-author rows.
- **At display**: even in the ranked weighted view, every contributing claim
  retains its author DID in the breakdown lines (`also-claimed-by:`,
  `multi-author:`). No "consensus weight" is shown without the authors visible.
- **At test time**: `scoring_aggregate_preserves_attribution` asserts that every
  weighted row can enumerate its contributing (author_did, claim_cid) tuples,
  and that the same `xtask check-arch` no-cross-table-join-elides-author rule
  (slice-03 I-FED-1) extends to any scoring query.

## Failure scenarios summary

| Step | Mode | User-visible behavior |
|---|---|---|
| 1 | Subject has zero claims | "No claims found for github:... . Author one with `openlore claim add` or pull peers." exit 0 |
| 2 | `--object` philosophy URI typo / unknown | "No claims found for object org.openlore.philosophy.foo. Did you mean ...?" (suggest near-matches); exit 0 |
| 2 | `--contributor` DID not in local graph | "No local claims authored by did:plc:... . Subscribe + pull with `openlore peer add/pull`." exit 0 |
| 3 | `--traverse` depth would explode (huge fan-out) | Bounded default depth (2); "Showing depth 2; N edges omitted. Use `--depth K` to go deeper." |
| 3 | Traversal hits only one node (no edges) | Renders the single node with "no connecting edges found at depth 2"; never fabricates a connection |
| 4 | `--weighted` over a single claim | Renders [SPARSE] with the "based on 1 claim by 1 author" honesty line |
| 4 | `--explain <subject>` for a subject not in the result | "Subject github:... is not in this result set." exit non-zero |
| 4 | Conflicting claims (same subject+object, opposing confidence) | Both contribute to the weight per their confidence; breakdown shows both authors; NO claim is dropped or averaged-into-oblivion |
| any | Mixed own + peer + scraper-signed claims | All three sources participate; each row keeps its author DID and (you)/(subscribed peer)/(unsubscribed cache) label |
