<!-- markdownlint-disable MD024 -->

# User Stories — openlore-scoring-graph (slice-04)

All stories in this file belong to **slice-04-scoring-graph** (the fourth
sibling feature in the OpenLore umbrella). Every story carries a `job_id`
traceable to `docs/product/jobs.yaml` per Decision 1. Stories US-GRAPH-001..005
carry mandatory Elevator Pitches; US-GRAPH-006 is `@infrastructure` and carries
an `infrastructure_rationale` instead.

This slice is a READ/VIEW slice. It creates, signs, and publishes NOTHING. It
explores the LOCAL federated graph already on disk: the user's own authored
claims (slice-01), peer claims pulled via slice-03 (`peer_claims`), and claims
the user signed from scraper candidates (slice-02, which are normal author
claims). There is NO new network surface and NO new write surface.

## System Constraints

These are cross-cutting constraints that apply to every story in this feature.
The first six are **inherited from the prior slices' user-stories.md** and are
repeated here for the reviewer's convenience. They are NOT relitigated.

- **Local-first / read-only**: the entire slice-04 surface works with the network
  disabled. No story in this slice performs any network call or writes any claim,
  store row, or PDS record. Scoring is pure analysis over existing signed claims.
- **Solution-neutral**: stories describe user-observable behavior. The storage
  choice that backs traversal/scoring (swap `adapter-duckdb` for a graph store
  vs augment DuckDB with recursive traversal — the WD-8 revisit) is reserved for
  DESIGN.
- **Claims-not-truth invariant**: no UI surface frames any claim — or any
  weighted aggregate of claims — as a truth assertion. A weight is a transparent
  view, never a verdict.
- **Attribution-preserving (anti-merging, extended to aggregates)**: every claim
  shown anywhere — including inside a weighted/traversed aggregate — retains its
  author DID. A score is an AGGREGATE VIEW, never a merge that loses attribution.
  This is the load-bearing slice-03 I-FED-1 invariant carried into scoring. The
  `scoring_aggregate_preserves_attribution` test enforces it.
- **Confidence numeric-only; buckets display-only**: numeric `[0.0, 1.0]` is the
  only persisted confidence (WD-10 / I-6). Scoring operates on the numeric value;
  display buckets are render-only.
- **CLI-first**: the CLI remains the canonical interface; no web UI in slice-04.

Constraints introduced new by this slice:

- **Scoring stays transparent and auditable (no ML)**: every adherence weight is
  a SMALL closed-form function of `count x confidence x triangulation`. The
  formula is displayed alongside the weight, and `--explain` reproduces the
  per-claim arithmetic. No opaque, learned, or non-reproducible score is
  permitted.
- **Weights/scores are DERIVED + DISPLAY-ONLY**: adherence weights and weight
  buckets ([STRONG]/[MODERATE]/[SPARSE]) are computed at query time and NEVER
  persisted, signed, or published. They extend the WD-10 display-only-bucket
  discipline. (If a future slice ever needs to persist a score, it requires a
  WD + ADR with a stated rationale; slice-04 does not.)
- **Sparse renders sparse**: a thin subgraph (few claims, few authors) is
  visibly labeled SPARSE with a "based on N claims by M authors" honesty line.
  The system never manufactures confidence from thin evidence.
- **Traversal invents no edges**: every edge shown by `--traverse` corresponds to
  exactly one signed claim. Traversal walks existing claims only; it never
  interpolates or infers a connection no author signed.
- **Local scope only**: scoring is over the LOCAL graph (own + pulled peers +
  scraper-signed). Multi-user / cohort aggregation is slice-05 (AppView), out of
  scope here.

### Glossary (terms introduced by this slice)

- **Dimension**: the axis a query is run on — subject (project), object
  (philosophy), or contributor (DID).
- **Traversal**: walking the contributor<->project<->philosophy edges of the
  local graph, where each edge is one signed claim. Bounded by `--depth`
  (default 2).
- **Adherence weight**: a derived, display-only number expressing how
  well-supported a (subject, object) pairing is, computed from the contributing
  claims via the transparent formula. Higher = more/better-evidenced/more
  triangulated support.
- **Triangulation**: support arriving from multiple distinct authors and/or the
  same author asserting a philosophy across multiple distinct projects. Increases
  weight per the formula.
- **Weight bucket**: a display-only label ([STRONG] / [MODERATE] / [SPARSE])
  derived from the weight and the evidence breadth. Never persisted.
- **Sparse subgraph**: a (subject, object) region backed by thin evidence (e.g.,
  1 claim by 1 author). Rendered honestly as [SPARSE].

---

## US-GRAPH-001: Query the graph by philosophy (object) and by project (subject), attribution preserved

### Job link

- `job_id`: J-002 (sub-job J-002a query-by-dimension)

### Elevator Pitch

- **Before**: I can query my own claims about a project with
  `openlore graph query --subject X`, but I cannot ask the inverse — "which
  projects embody philosophy P, and who says so?" — so I cannot orient around a
  value I care about, only around projects I already know.
- **After**: I run
  `openlore graph query --object org.openlore.philosophy.dependency-pinning` and
  see "Projects embodying org.openlore.philosophy.dependency-pinning (4 claims
  across 3 subjects, 3 distinct authors)" grouped by project, each claim under
  its author DID with numeric confidence and a display-only bucket. The footer
  says "every claim retains its author DID. No claims are merged."
- **Decision enabled**: I can start a stack/community decision from a PHILOSOPHY
  I value rather than from projects I already know — which means I will discover
  candidate projects I would never have searched for by name.

### Problem

Maria Lopez (P-002) is a tech lead choosing between Rust libraries and wants to
commit to a community whose values match her team's. She cares about
dependency-pinning as a philosophy, but the slice-01 query only goes
project-first (`--subject`). She has no way to ask "show me every project that
embodies dependency-pinning, and who claims so," so she cannot orient her search
around the value she actually cares about. Worse, any tool that answered this by
collapsing authors into a faceless "X projects value this" count would reproduce
the HN/Reddit aggregator failure mode she already distrusts.

### Who

- Researcher / Tech Lead (P-002) wearing the graph-explorer hat
- Has a local graph populated with own claims (slice-01) + pulled peer claims
  (slice-03) + scraper-signed claims (slice-02)
- Comfortable with philosophy URIs and DIDs; may be offline (local read)
- Wants to orient a decision around a philosophy or a project, with attribution

### Solution

Extend `openlore graph query` with two new query dimensions: `--object
<philosophy>` (which projects embody this philosophy, grouped by subject) and
the existing `--subject <project>` (unchanged from slice-01). With either
dimension, every claim row shows its author DID, predicate, object/subject,
numeric confidence + display-only bucket, evidence, CID, and composed_at. Output
is grouped (by subject for `--object`, by author for `--subject`); NO row
represents a multi-author aggregate.

### Domain Examples

#### Example 1 (Happy Path — query by object/philosophy)

Maria runs
`openlore graph query --object org.openlore.philosophy.dependency-pinning`. Her
local graph has 4 matching claims: Rachel on `github:rust-lang/cargo` (0.91),
Tobias on `github:denoland/deno` (0.55), Maria herself on
`github:denoland/deno` (0.40), and Rachel on `github:nixos/nixpkgs` (0.88). The
output groups by subject (3 projects), lists each claim under its author DID,
and the footer reads "4 claims across 3 subjects, 3 distinct authors. Grouped by
subject; every claim retains its author DID. No claims are merged."

#### Example 2 (Happy Path — query by subject unchanged)

Tobias Weber runs `openlore graph query --subject github:rust-lang/cargo`. The
output is identical to slice-01/03 behavior: claims about cargo grouped by
author DID. Slice-04 does not change this surface; it only adds the inverse
`--object` dimension.

#### Example 3 (Edge — same subject+object by two authors)

Aanya Krishnan has her own claim that `github:denoland/deno` embodies
`dependency-pinning` (0.40) and has pulled Tobias's claim asserting the SAME
(subject, object) at 0.55. Running `--object dependency-pinning` displays BOTH as
distinct rows under `github:denoland/deno`, one under
`did:plc:aanya-test (you)` and one under `did:plc:tobias-test (subscribed peer)`.
There is NO single "deno: 2 authors agree" row.

#### Example 4 (Error/Edge — unknown philosophy URI)

Maria typos `openlore graph query --object org.openlore.philosophy.dependancy-pinning`
(misspelled). The CLI finds zero matches and prints "No claims found for object
org.openlore.philosophy.dependancy-pinning. Did you mean
org.openlore.philosophy.dependency-pinning?" (near-match suggestion). Exit code
is 0 (a valid empty result, not an error).

### UAT Scenarios (BDD)

```gherkin
Scenario: Querying a philosophy surfaces every project that embodies it, by author
  Given Maria's local graph has 4 claims asserting org.openlore.philosophy.dependency-pinning across 3 projects by 3 authors
  When Maria runs `openlore graph query --object org.openlore.philosophy.dependency-pinning`
  Then the output groups claims by subject (project)
  And every claim row shows its author DID, numeric confidence, and a display-only bucket
  And no project row collapses multiple authors into a single entry
  And the footer states "every claim retains its author DID. No claims are merged."

Scenario: Identical-content claims by different authors are displayed as separate rows
  Given Aanya has her own claim and a pulled peer claim asserting the SAME subject github:denoland/deno and object dependency-pinning
  When Aanya runs `openlore graph query --object org.openlore.philosophy.dependency-pinning`
  Then both claims appear as distinct rows under github:denoland/deno
  And one is under "did:plc:aanya-test (you)" and the other under "did:plc:tobias-test (subscribed peer)"
  And there is NO row that represents both claims combined

Scenario: Query by subject is unchanged from prior-slice behavior
  Given Tobias has claims about github:rust-lang/cargo in his local graph
  When Tobias runs `openlore graph query --subject github:rust-lang/cargo`
  Then the output matches the slice-01/03 subject-query behavior
  And every claim is grouped by author DID
  And exit code is 0

Scenario: Unknown philosophy URI returns an empty result with a suggestion
  Given Maria's local graph has no claims for object org.openlore.philosophy.dependancy-pinning
  When Maria runs `openlore graph query --object org.openlore.philosophy.dependancy-pinning`
  Then the output states no claims were found for that object
  And the output suggests a near-matching philosophy URI
  And exit code is 0
```

### Acceptance Criteria

- [ ] `openlore graph query --object <philosophy>` returns claims for that object grouped by subject (project).
- [ ] `openlore graph query --subject <project>` behavior is unchanged from slice-01/03.
- [ ] Every claim row displays: author DID, predicate, subject/object, numeric confidence + display-only bucket (per WD-10), evidence, CID, composed_at.
- [ ] Output is grouped (by subject for `--object`); NO row represents a multi-author aggregate. Two claims with identical (subject, object) but different authors appear as TWO rows.
- [ ] Footer states the count of distinct subjects AND distinct authors AND the no-merge guarantee.
- [ ] An unknown/unmatched philosophy URI returns an empty result with a near-match suggestion and exit code 0.
- [ ] All reads are local-only; the command succeeds with the network disabled.

### Outcome KPIs

See `outcome-kpis.md` KPI-GRAPH-2 (anti-merging in aggregates — baseline established at the dimension query) and KPI-GRAPH-6 (local-read latency).

### Technical Notes

- Depends on US-GRAPH-006 (read-side query extension in place).
- Reads the existing slice-01 `claims` + slice-03 `peer_claims` stores; scraper-signed claims (slice-02) live in `claims` and participate automatically.
- New query path: extends the existing query method with an `--object` dimension. DESIGN owns whether this is a new flag on `graph query`, a new sub-verb, or an amendment to the existing federated query (`query_federated_by_object` mirroring slice-03's `query_federated_by_subject`).
- The renderer reuses the slice-03 anti-merging discipline: every output row carries exactly one `author_did`.

---

## US-GRAPH-002: Query the graph by contributor (DID) to read one developer's reasoning trail

### Job link

- `job_id`: J-002 (sub-job J-002a query-by-dimension; relates to J-004 contributor lens)

### Elevator Pitch

- **Before**: I can read a peer's claims about ONE project at a time, but I
  cannot ask "what does this contributor claim across EVERYTHING in my graph?" —
  so I cannot form a picture of a developer's overall reasoning trail without
  manually querying every subject.
- **After**: I run `openlore graph query --contributor did:plc:rachel-test` and
  see "Claims authored by did:plc:rachel-test (5 claims across 4 subjects)"
  listing each claim with its subject, philosophy, and confidence, ending with
  "All claims authored by ONE DID. This is one developer's reasoning trail, not a
  community consensus."
- **Decision enabled**: I can read a contributor's whole published reasoning
  trail in one query and decide whether their values align with mine — which
  means I will weigh a peer's authority from the breadth of their reasoning, not
  from a single claim I happened to see.

### Problem

Maria has pulled claims from several peers and signed some of her own. When she
sees a compelling claim from Rachel about cargo, she wants to know "who is Rachel
— what else does she claim, and is her reasoning consistent across projects?"
Today she would have to query every subject one at a time and mentally
cross-reference. She needs a contributor-first dimension. The danger: a list of
one person's claims could be mistaken for authoritative truth, so the framing
must keep it honest — this is one developer's trail, not consensus.

### Who

- Researcher / Tech Lead (P-002), has a populated local graph including >=1 peer
- Solo Builder (P-001) vetting a dependency's maintainer through the same lens
- Wants to evaluate a contributor's body of reasoning, with the honest framing
- Offline-capable (local read)

### Solution

Add a `--contributor <did>` query dimension to `openlore graph query`. It lists
every claim in the local graph authored by that DID, across all subjects,
showing subject, predicate, object, numeric confidence + bucket, and CID. The
output footer makes the honest framing explicit: all claims are by ONE DID; this
is one developer's reasoning trail, not a community consensus.

### Domain Examples

#### Example 1 (Happy Path)

Maria runs `openlore graph query --contributor did:plc:rachel-test`. The local
graph has 5 claims by Rachel across 4 subjects (cargo x2, nixpkgs, tokio,
serde). The output lists all 5 with subject/object/confidence/CID and the footer
"All claims authored by ONE DID (did:plc:rachel-test). This is one developer's
reasoning trail, not a community consensus."

#### Example 2 (Edge — querying one's own DID)

Tobias runs `openlore graph query --contributor did:plc:tobias-test` (his own
DID). The output lists his own authored claims, with the header annotating
`(you)` rather than `(subscribed peer)`. This is a valid self-review.

#### Example 3 (Edge — contributor not in local graph)

Aanya runs `openlore graph query --contributor did:plc:stranger-test`, a DID she
has never subscribed to or pulled. The CLI prints "No local claims authored by
did:plc:stranger-test. Subscribe and pull with `openlore peer add` +
`openlore peer pull`." Exit code is 0.

#### Example 4 (Edge — contributor with claims as unsubscribed cache)

Maria previously subscribed to and pulled Tobias, then soft-removed him (slice-03
`peer remove` without `--purge`). His cached claims remain. Running
`--contributor did:plc:tobias-test` lists his cached claims annotated
`(unsubscribed cache)` rather than `(subscribed peer)`, preserving the slice-03
relationship labeling.

### UAT Scenarios (BDD)

```gherkin
Scenario: Querying a contributor surfaces their full reasoning trail honestly
  Given did:plc:rachel-test has authored 5 claims across 4 subjects in Maria's local graph
  When Maria runs `openlore graph query --contributor did:plc:rachel-test`
  Then all 5 claims are listed under did:plc:rachel-test with subject, object, confidence, and CID
  And the footer states this is one developer's reasoning trail, not a community consensus

Scenario: Querying one's own DID is a valid self-review
  Given Tobias has authored several claims under did:plc:tobias-test
  When Tobias runs `openlore graph query --contributor did:plc:tobias-test`
  Then his own claims are listed annotated "(you)"
  And exit code is 0

Scenario: Querying a contributor absent from the local graph degrades gracefully
  Given Aanya has no claims by did:plc:stranger-test in her local graph
  When Aanya runs `openlore graph query --contributor did:plc:stranger-test`
  Then the output states no local claims were found for that DID
  And the output hints to subscribe and pull with `openlore peer add` + `openlore peer pull`
  And exit code is 0

Scenario: A soft-removed peer's cached claims are labeled as unsubscribed cache
  Given Maria soft-removed did:plc:tobias-test but retained his cached claims
  When Maria runs `openlore graph query --contributor did:plc:tobias-test`
  Then his cached claims are listed annotated "(unsubscribed cache)"
  And no claim is shown without its author DID
```

### Acceptance Criteria

- [ ] `openlore graph query --contributor <did>` lists every local-graph claim authored by that DID, across all subjects.
- [ ] Each row shows subject, predicate, object, numeric confidence + display-only bucket, and CID.
- [ ] The author-relationship label is correct: `(you)` for the user's own DID, `(subscribed peer)` for active subscriptions, `(unsubscribed cache)` for soft-removed peers (per slice-03 labeling).
- [ ] The footer states the claim is one developer's reasoning trail, not a community consensus.
- [ ] A DID with no local claims returns an empty result with a subscribe/pull hint and exit code 0.
- [ ] All reads are local-only; the command succeeds with the network disabled.

### Outcome KPIs

See `outcome-kpis.md` KPI-GRAPH-2 (attribution fidelity — the contributor lens is where the anti-merging guarantee is sharpest).

### Technical Notes

- Depends on US-GRAPH-006 and reuses the slice-03 author-relationship labeling (subscribed / unsubscribed-cache) derived from `peer_subscriptions.removed_at`.
- New query path: `--contributor` dimension queries `author_did` across `claims` + `peer_claims`. DESIGN owns the method shape.
- The renderer reuses the anti-merging discipline: every row carries one `author_did`.

---

## US-GRAPH-003: See a transparent, auditable weighted/scored view, with sparse subgraphs shown as sparse

### Job link

- `job_id`: J-002 (sub-job J-002c weighted-scoring — the load-bearing surface; relates to J-004 "surface adherence weighting")

### Elevator Pitch

- **Before**: I can list claims about a philosophy, but I cannot tell at a glance
  which projects are WELL-SUPPORTED versus backed by a single speculative claim —
  I have to eyeball confidences and count claims by hand, and I am afraid of
  making a bad call on sparse data.
- **After**: I run
  `openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted`
  and see projects RANKED by an adherence weight, each weight printed with its
  inputs (claims, distinct authors, max confidence) and the formula
  (`confidence x author bonus x triangulation bonus`, no ML); a philosophy backed
  by one claim is labeled [SPARSE] with "(!) based on 1 claim by 1 author —
  treat as a lead, not a conclusion."
- **Decision enabled**: I can rank candidate projects by how well a philosophy is
  supported AND trust the ranking because I can see exactly how each weight was
  computed and which are too thin to lean on — which means I will base an
  architectural decision on this view.

### Problem

After listing claims, Maria needs to know which projects are well-supported on a
given philosophy and which are backed by thin evidence. Eyeballing a dozen
confidences and counting authors by hand is exactly the friction the graph
should remove. But a single opaque score would re-trigger her J-002 anxiety:
"what if the graph is sparse, biased, or speculative and I make a bad call?" The
weighting MUST be transparent (she can see and reproduce the formula) and MUST
degrade honestly (a single-claim philosophy must look thin, never dressed up as
a confident number).

### Who

- Researcher / Tech Lead (P-002), has a populated local graph, evaluating
  candidate projects on a philosophy
- Solo Builder (P-001) ranking dependencies by a value they care about
- Carries the J-002 sparse-data anxiety; needs the weighting to be auditable and
  honest
- Offline-capable (the weight is computed locally at query time)

### Solution

Add a `--weighted` flag to `openlore graph query` (combinable with `--object`,
`--subject`, `--traverse`). It computes a DERIVED, DISPLAY-ONLY adherence weight
per (subject, object) from the contributing claims via a small transparent
formula: `weight = sum over claims of (confidence x author_distinct_bonus x
cross_project_triangulation_bonus)`. Results are ranked by weight; each weight is
displayed with its inputs AND the formula. A display-only bucket
([STRONG]/[MODERATE]/[SPARSE]) annotates each row. A thin subgraph renders as
[SPARSE] with a "based on N claims by M authors" honesty line. Weights and
buckets are NEVER persisted (computed at query time only).

### Domain Examples

#### Example 1 (Happy Path — ranked weighted view)

Maria runs
`openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted`.
The subgraph has cargo (1 claim, conf 0.91), nixpkgs (1 claim, conf 0.88), deno
(2 claims by 2 authors, max conf 0.55). The output ranks cargo (weight 1.82,
[STRONG], boosted by Rachel spanning cargo+nixpkgs), nixpkgs (1.76, [STRONG]),
deno (0.95, [MODERATE], boosted by 2 distinct authors). The formula and each
weight's inputs are printed. A footer notes weights are a display-only aggregate
view, never stored.

#### Example 2 (Boundary — sparse subgraph renders as sparse)

Tobias runs
`openlore graph query --object org.openlore.philosophy.actor-model --weighted`.
Only one claim matches (tokio, 1 author, conf 0.50). The output shows tokio with
weight 0.50, labeled [SPARSE], plus "(!) SPARSE: based on 1 claim by 1 author on
1 project. This is one developer's opinion, not a triangulated signal. Treat as a
lead, not a defensible conclusion." No confidence is manufactured.

#### Example 3 (Edge — multi-author triangulation raises weight)

Aanya runs `--object reproducible-builds --weighted`. For `github:denoland/deno`,
two distinct authors (Aanya 0.40, Tobias 0.55) both claim reproducible-builds.
The weight applies the `+0.25 per additional distinct author` bonus, ranking deno
above a single-author project with a similar max confidence. The breakdown line
shows "multi-author: 2 distinct authors raise triangulation."

#### Example 4 (Edge — conflicting confidences, nothing dropped)

Maria queries a philosophy where two authors disagree sharply on the same
project (one at 0.85, one at 0.20). Both contribute to the weight per their
confidence; the breakdown shows both authors and both confidences. NO claim is
averaged-into-oblivion or dropped — the view shows the spread honestly.

#### Example 5 (Edge — weights are never persisted)

Tobias runs the weighted query, then inspects his DuckDB and on-disk artifacts.
No `adherence_weight` or bucket label appears in any table, any `<cid>.json`, or
any PDS record. Re-running the same query after `openlore peer pull` produces
different weights (new claims arrived) — proving weights are computed at query
time, not stored.

### UAT Scenarios (BDD)

```gherkin
Scenario: Weighted view ranks projects with a transparent, auditable formula
  Given the dependency-pinning subgraph has cargo (1 claim, conf 0.91), nixpkgs (1 claim, conf 0.88), and deno (2 claims by 2 authors)
  When Maria runs `openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted`
  Then projects are ranked by adherence weight
  And each weight is displayed with its inputs (claim count, distinct author count, max confidence)
  And the output prints the formula (confidence x author bonus x triangulation bonus) and states "no ML"
  And a footer states weights are a display-only aggregate view, never stored

Scenario: A sparse subgraph renders honestly as sparse
  Given org.openlore.philosophy.actor-model has exactly 1 claim by 1 author on 1 project
  When Tobias runs `openlore graph query --object org.openlore.philosophy.actor-model --weighted`
  Then the single project is labeled [SPARSE]
  And the output states it is based on 1 claim by 1 author
  And the output advises treating it as a lead, not a defensible conclusion
  And no confidence is manufactured from the thin evidence

Scenario: Multi-author support raises the triangulation weight
  Given github:denoland/deno has reproducible-builds claims from 2 distinct authors
  When Aanya runs `openlore graph query --object org.openlore.philosophy.reproducible-builds --weighted`
  Then deno's weight includes the per-additional-distinct-author bonus
  And the breakdown line states "multi-author: 2 distinct authors raise triangulation"
  And both authors remain individually attributed in the breakdown

Scenario: Conflicting claims are both shown, nothing dropped or averaged away
  Given two authors claim the same project+philosophy at confidence 0.85 and 0.20
  When Maria runs a weighted query for that philosophy
  Then both claims contribute to the weight per their confidence
  And the breakdown shows both authors and both confidences
  And no claim is dropped or collapsed into a single averaged value

Scenario: Weights and buckets are never persisted
  Given Tobias has run a weighted query
  When Tobias inspects his DuckDB tables, on-disk claim artifacts, and any published records
  Then no adherence weight or weight-bucket label appears in any stored or published location
  And re-running the same query after a peer pull may produce different weights
```

### Acceptance Criteria

- [ ] `openlore graph query --weighted` (combinable with `--object`/`--subject`/`--traverse`) ranks results by a derived adherence weight.
- [ ] The weight equals the documented formula `sum(confidence x author_distinct_bonus x cross_project_triangulation_bonus)` applied to exactly the displayed contributing claims (reproducible).
- [ ] Each weight is displayed WITH its inputs: claim count, distinct author count, max confidence, cross-project span.
- [ ] The formula is printed in the output and explicitly states "no ML".
- [ ] A display-only bucket ([STRONG]/[MODERATE]/[SPARSE]) annotates each row; buckets are never persisted (extends WD-10).
- [ ] A thin subgraph (e.g., 1 claim by 1 author) renders [SPARSE] with a "based on N claims by M authors" honesty line; confidence is never manufactured from thin evidence.
- [ ] Conflicting claims both contribute per their confidence and both stay attributed; no claim is dropped or averaged-into-oblivion.
- [ ] `adherence_weight` and `weight_bucket` are NEVER written to any DuckDB table, on-disk artifact, signed payload, or PDS record.
- [ ] Every weighted aggregate row retains the per-author attribution of its contributing claims (anti-merging in aggregates).
- [ ] All computation is local; the command succeeds with the network disabled.

### Outcome KPIs

See `outcome-kpis.md` KPI-GRAPH-1 (non-obvious connection — triangulation surfacing), KPI-GRAPH-3 (transparency — reproducible weight), KPI-GRAPH-4 (sparse honesty — guardrail).

### Technical Notes

- Depends on US-GRAPH-001 (dimension query) and US-GRAPH-006 (pure `scoring` core + read-side extensions).
- The scoring logic lives in a PURE core module (`scoring`) per ADR-007: `fn score(claims: &[AttributedClaim]) -> WeightedView`. No I/O; trivially unit/mutation-testable; the formula constants live here as the SSOT.
- The exact formula constants (author bonus 0.25, triangulation bonus 0.5, bucket thresholds) are a product default; DESIGN may tune them but MUST keep the function small, closed-form, and reproducible (no ML). Any constant change is a code change, not a config or learned weight.
- Weights/buckets are computed in the render path from the pure scoring output; they are display-only by construction (no persistence code path exists for them).
- The WD-8 store revisit (graph store vs DuckDB recursive traversal) is DESIGN's call and is invisible to this story's contract.

---

## US-GRAPH-004: Traverse contributor<->project<->philosophy edges to surface non-obvious connections

### Job link

- `job_id`: J-002 (sub-job J-002b traverse-edges)

### Elevator Pitch

- **Before**: I can list claims about a philosophy or a project, but I cannot see
  the CONNECTIONS between them — "who spans the projects I'm evaluating?" — so the
  non-obvious link (a contributor whose values triangulate across my candidates)
  stays invisible and I keep relying on vibes and Hacker News threads.
- **After**: I run
  `openlore graph query --object org.openlore.philosophy.dependency-pinning --traverse`
  and see a tree from the philosophy to its projects to their claim authors, plus
  a "Connections found" callout: "did:plc:rachel-test spans 2 of these projects
  (cargo, nixpkgs) -> a contributor whose dependency-pinning claims triangulate
  across projects." Every edge is one signed claim.
- **Decision enabled**: I can surface a non-obvious cross-project contributor
  connection in a single query — the insight I could never get from `gh search`
  plus skimming READMEs — which means I will discover aligned people and projects
  I would otherwise have missed.

### Problem

The headline value of J-002 is surfacing a connection a developer could not get
elsewhere: "projects sharing philosophy X, and the contributors who span them."
Listing claims one dimension at a time (US-GRAPH-001/002) orients Maria but does
not reveal the cross-cutting structure. She needs to traverse the
contributor<->project<->philosophy edges to see who connects her candidate
projects. The risk: a traversal that interpolated or inferred edges would
fabricate reasoning no author signed, breaking the auditability promise. Every
edge must map to exactly one signed claim, and the traversal must be bounded so a
dense graph does not explode.

### Who

- Researcher / Tech Lead (P-002), has a populated local graph, looking for the
  non-obvious connection that informs a decision
- Solo Builder (P-001) discovering aligned maintainers across dependencies
- Wants connection discovery with auditable, non-fabricated edges
- Offline-capable (local read)

### Solution

Add a `--traverse` flag to `openlore graph query` (combinable with `--object`,
`--subject`, `--contributor`). Starting from the queried node, it walks the
contributor<->project<->philosophy edges to a bounded default depth (2;
overridable via `--depth K`) and renders a tree. Each edge corresponds to exactly
one signed claim. A "Connections found" callout names contributors who span
multiple projects (or projects sharing a philosophy via multiple contributors).
The output states explicitly that traversal does not invent edges and no claims
are merged.

### Domain Examples

#### Example 1 (Happy Path — cross-project contributor surfaced)

Maria runs `--object dependency-pinning --traverse`. The tree shows the
philosophy -> {cargo, nixpkgs, deno} -> their claim authors. The "Connections
found" callout reads "did:plc:rachel-test spans 2 of these projects (cargo,
nixpkgs) -> a contributor whose dependency-pinning claims triangulate across
projects." This is the non-obvious connection.

#### Example 2 (Edge — single node, no connecting edges)

Tobias runs `--object actor-model --traverse` where only tokio has a single
claim. The output renders tokio under the philosophy with "no connecting edges
found at depth 2." It does NOT fabricate a connection to any other project or
contributor.

#### Example 3 (Edge — depth bound prevents fan-out explosion)

Aanya runs `--contributor did:plc:rachel-test --traverse` on a dense graph where
Rachel's claims fan out to dozens of philosophies and co-claimants. The output
shows depth 2 by default and prints "Showing depth 2; 37 edges omitted. Use
`--depth 3` to go deeper." The traversal stays bounded and responsive.

#### Example 4 (Edge — every edge maps to a signed claim)

Maria runs a traversal and inspects the edges. Each displayed edge between a
project and a contributor corresponds to a specific `claim_cid` she can look up
with `openlore graph query --subject <project>`. There is no edge that does not
trace to a signed claim.

### UAT Scenarios (BDD)

```gherkin
Scenario: Traversal surfaces a non-obvious cross-project contributor connection
  Given did:plc:rachel-test asserts dependency-pinning on both cargo and nixpkgs in the local graph
  When Maria runs `openlore graph query --object org.openlore.philosophy.dependency-pinning --traverse`
  Then the output shows a tree from the philosophy to its projects to their claim authors
  And a "Connections found" callout names did:plc:rachel-test as spanning 2 projects
  And every displayed edge corresponds to exactly one signed claim
  And the output states "Traversal does not invent edges."

Scenario: A node with no connecting edges is rendered without fabrication
  Given org.openlore.philosophy.actor-model has exactly 1 claim and no cross-project span
  When Tobias runs `openlore graph query --object org.openlore.philosophy.actor-model --traverse`
  Then the single node is rendered with "no connecting edges found at depth 2"
  And no connection to any other project or contributor is fabricated

Scenario: Traversal depth is bounded by default to prevent fan-out explosion
  Given did:plc:rachel-test has a dense claim graph fanning out beyond depth 2
  When Aanya runs `openlore graph query --contributor did:plc:rachel-test --traverse`
  Then the output is bounded to depth 2 by default
  And the output reports how many edges were omitted and how to go deeper with --depth
  And the command returns responsively

Scenario: Every traversed edge maps to a verifiable signed claim
  Given Maria has run a traversal showing edges between projects and contributors
  When Maria looks up any displayed edge via `openlore graph query --subject <project>`
  Then the edge corresponds to a specific signed claim CID in her local graph
  And no displayed edge lacks a backing signed claim
```

### Acceptance Criteria

- [ ] `openlore graph query --traverse` (combinable with `--object`/`--subject`/`--contributor`) walks contributor<->project<->philosophy edges and renders a tree.
- [ ] Every displayed edge corresponds to exactly one signed claim (`claim_cid`); traversal never interpolates, infers, or fabricates an edge.
- [ ] A "Connections found" callout names contributors spanning multiple projects (or projects sharing a philosophy via multiple contributors), when such spans exist.
- [ ] A node with no connecting edges renders with an explicit "no connecting edges" message; no connection is fabricated.
- [ ] Traversal is bounded to a default depth of 2; `--depth K` overrides; the output reports omitted edges when bounded.
- [ ] Every traversed edge retains the author DID of its backing claim (anti-merging).
- [ ] The output states traversal does not invent edges and no claims are merged.
- [ ] All reads are local-only; the command succeeds with the network disabled.

### Outcome KPIs

See `outcome-kpis.md` KPI-GRAPH-1 (the north star — surface a non-obvious connection in one query) and KPI-GRAPH-2 (attribution preserved in traversal).

### Technical Notes

- Depends on US-GRAPH-001/002 (dimension queries) and US-GRAPH-006 (read-side traversal support).
- Traversal is the surface most affected by the WD-8 store revisit: a recursive query over DuckDB vs a graph store. DESIGN owns this; the product contract is "no invented edges, bounded depth, attribution preserved, responsive on the local graph."
- The bounded-depth default (2) is a product default; DESIGN may tune it but MUST keep traversal bounded to avoid fan-out explosion on dense graphs.

---

## US-GRAPH-005: Audit a weight with `--explain` to reproduce the per-claim arithmetic

### Job link

- `job_id`: J-002 (sub-job J-002c weighted-scoring — the auditability deepening)

### Elevator Pitch

- **Before**: The weighted view shows me a ranking and the formula, but when one
  project ranks first I cannot drill into EXACTLY which claims, by which authors,
  at which confidences, produced its weight — so I have to trust the number
  rather than reproduce it.
- **After**: I run
  `openlore graph query --object ... --weighted --explain github:denoland/deno`
  and see the per-claim arithmetic: each contributing claim with its author DID,
  CID, confidence, the bonuses applied, and the running sum that equals the
  displayed weight.
- **Decision enabled**: I can reproduce any adherence weight by hand from its
  contributing claims — the strongest form of the transparency promise — which
  means I will defend a ranking to a skeptical teammate by showing the exact
  math, not by appealing to a tool's authority.

### Problem

The weighted view (US-GRAPH-003) prints the formula and each weight's inputs,
which is transparent at the aggregate level. But when Maria must justify to a
skeptical teammate WHY cargo ranked above deno, she needs to drill into the exact
per-claim arithmetic for one project: which claims contributed, by whom, at what
confidence, with which bonuses, summing to the displayed weight. Without this,
the transparency stops at "trust the formula"; with it, the transparency reaches
"reproduce the number by hand." This is the strongest form of the J-002
auditability promise and the direct mitigation of "what if the score is
misleading?"

### Who

- Researcher / Tech Lead (P-002), defending a ranking to a teammate or auditing a
  surprising weight
- Solo Builder (P-001) double-checking why a dependency ranked the way it did
- Wants reproducible, per-claim arithmetic
- Offline-capable (local read)

### Solution

Add an `--explain <subject>` option to the weighted query. For the named subject,
it prints the per-claim breakdown: each contributing claim's author DID, CID,
confidence, the author-distinct and triangulation bonuses applied, and the
running sum that equals the displayed adherence weight. The explanation
decomposes the aggregate fully — every contributing claim is enumerated and
attributed — making the weight reproducible by hand.

### Domain Examples

#### Example 1 (Happy Path)

Maria runs
`openlore graph query --object dependency-pinning --weighted --explain github:denoland/deno`.
The output lists deno's 2 contributing claims: Tobias (bafy...d3no, conf 0.55,
author bonus 1.0) and Maria (bafy...mz01, conf 0.40, +0.25 second-author bonus),
showing the arithmetic `0.55x1.0 + 0.40x1.25 = 0.55 + 0.50 = 1.05` ... and how it
resolves to the displayed weight. She can reproduce it by hand.

#### Example 2 (Edge — explain a single-claim [SPARSE] subject)

Tobias runs `--object actor-model --weighted --explain github:tokio-rs/tokio`.
The breakdown shows the one contributing claim (1 author, conf 0.50, no bonuses)
and the running sum 0.50, with the [SPARSE] honesty line repeated: "based on 1
claim by 1 author."

#### Example 3 (Error — explain a subject not in the result set)

Aanya runs `--object dependency-pinning --weighted --explain github:foo/bar`
where foo/bar has no dependency-pinning claims. The CLI prints "Subject
github:foo/bar is not in this result set." and exits non-zero (a usage error,
unlike an empty dimension query which exits 0).

#### Example 4 (Edge — explain decomposes a triangulated weight to its authors)

Maria runs `--explain github:rust-lang/cargo` where Rachel's cross-project span
(cargo+nixpkgs) raised cargo's weight. The breakdown shows the base claim plus
the explicit `+0.5 cross-project triangulation` line attributed to
did:plc:rachel-test, so Maria sees exactly why the triangulation bonus applied
and to whom.

### UAT Scenarios (BDD)

```gherkin
Scenario: --explain reproduces a weight from its per-claim arithmetic
  Given a weighted view ranks github:denoland/deno using claims from did:plc:tobias-test and did:plc:maria-test
  When Maria runs `openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted --explain github:denoland/deno`
  Then the breakdown enumerates each contributing claim with its author DID, CID, and confidence
  And each applied bonus (author-distinct, cross-project triangulation) is shown explicitly
  And the running sum equals the displayed adherence weight
  And no contributing claim is merged into a faceless aggregate

Scenario: --explain on a sparse subject repeats the honesty line
  Given github:tokio-rs/tokio has a single actor-model claim by one author
  When Tobias runs `openlore graph query --object org.openlore.philosophy.actor-model --weighted --explain github:tokio-rs/tokio`
  Then the breakdown shows the single contributing claim with no bonuses
  And the [SPARSE] honesty line "based on 1 claim by 1 author" is shown

Scenario: --explain for a subject not in the result set is a usage error
  Given github:foo/bar has no claims for the queried object
  When Aanya runs `openlore graph query --object org.openlore.philosophy.dependency-pinning --weighted --explain github:foo/bar`
  Then the output states the subject is not in this result set
  And exit code is non-zero

Scenario: --explain attributes a triangulation bonus to the contributor who earned it
  Given did:plc:rachel-test asserts dependency-pinning on both cargo and nixpkgs, raising cargo's weight
  When Maria runs `--object org.openlore.philosophy.dependency-pinning --weighted --explain github:rust-lang/cargo`
  Then the breakdown shows the cross-project triangulation bonus line attributed to did:plc:rachel-test
  And the breakdown's running sum equals cargo's displayed weight
```

### Acceptance Criteria

- [ ] `openlore graph query --weighted --explain <subject>` prints the per-claim arithmetic for the named subject.
- [ ] The breakdown enumerates every contributing claim with its author DID, CID, and numeric confidence.
- [ ] Each applied bonus (author-distinct, cross-project triangulation) is shown explicitly with the contributor it applies to.
- [ ] The running sum shown equals the displayed adherence weight (reproducible by hand).
- [ ] No contributing claim is merged or hidden; the aggregate fully decomposes (anti-merging in aggregates).
- [ ] `--explain` on a [SPARSE] subject repeats the "based on N claims by M authors" honesty line.
- [ ] `--explain` for a subject not in the result set is a usage error with a clear message and non-zero exit.
- [ ] All computation is local; the command succeeds with the network disabled.

### Outcome KPIs

See `outcome-kpis.md` KPI-GRAPH-3 (scoring transparency — reproducible by hand; this story is the load-bearing surface for it).

### Technical Notes

- Depends on US-GRAPH-003 (weighted view) and US-GRAPH-006 (pure `scoring` core).
- `--explain` reuses the SAME pure `scoring` core output as US-GRAPH-003; it renders the intermediate per-claim contributions rather than only the final weight. No separate scoring path (single source of truth for the arithmetic).
- The breakdown is a render of the pure scoring function's contribution list; the function must expose its intermediate per-claim contributions (not just the final sum) for this story to render them.

---

## US-GRAPH-006 `@infrastructure`: Bootstrap the pure `scoring` core and read-side query extensions

### `infrastructure_rationale`

This story exists to add the pure `scoring` core module and the read-side query
extensions (`--object`, `--contributor`, `--traverse`, `--weighted`, `--explain`)
to the existing `graph query` surface. It is an `@infrastructure` story because
it has no end-user-observable behavior on its own — every user-visible behavior
is in US-GRAPH-001..005. Without this story, those five stories cannot ship. It is
grouped with US-GRAPH-001 and US-GRAPH-003 in Release 1 (the walking skeleton
release) because the walking-skeleton stories depend on it.

The slice satisfies the BLOCKING slice-level Elevator Pitch check (per
`nw-po-review-dimensions` Dimension 0 §5): five user-visible stories
(US-GRAPH-001..005) accompany this one infrastructure story. The slice is NOT
100% `@infrastructure`.

### Job link

- `job_id`: `infrastructure-only`

### Problem (infra perspective)

Slice-01 created `graph query --subject` over `claims`; slice-03 added
`--federated` over `claims` + `peer_claims`. Slice-04 needs (a) a PURE `scoring`
core that derives display-only adherence weights from attributed claims, and (b)
read-side query extensions for the new dimensions, traversal, and weighting. None
of these are user-visible on their own, but every US-GRAPH-001..005 story depends
on them. Critically, the WD-8 storage revisit (graph store vs DuckDB recursive
traversal) is decided in DESIGN, not here — this story scopes the
solution-neutral contract the user-visible stories rely on.

### Solution (infra)

- New pure `scoring` core (ADR-007): `fn score(claims: &[AttributedClaim]) -> WeightedView` plus the `AttributedClaim`, `Contribution`, and `WeightedView` ADTs. No I/O. The formula constants (author bonus, triangulation bonus, bucket thresholds) live here as the SSOT. Exposes intermediate per-claim contributions for `--explain` (US-GRAPH-005).
- Extend the read-side query port/method surface for the new dimensions: query by object, query by contributor, bounded traversal, and a federated read that supplies attributed claims to the pure scoring core. DESIGN owns whether these are new `StoragePort`/graph-store methods (mirroring slice-03's `query_federated_by_subject`) or a new read port. Any new port surface MUST ship a `probe()` per ADR-009.
- Carry the slice-03 anti-merging discipline into all new query/scoring paths: the `xtask check-arch` `no_cross_table_join_elides_author` rule extends to cover scoring queries; the renderer/scoring boundary uses a non-`Option` `author_did` (compile-error if dropped, mirroring slice-03's `FederatedRow`).
- The WD-8 store decision (swap-to-graph-store vs augment-DuckDB-with-recursive-traversal) is explicitly DEFERRED to DESIGN; this story's contract is storage-neutral.

### Acceptance Criteria

- [ ] A pure `scoring` core compiles with `score(claims) -> WeightedView` and exposes intermediate per-claim contributions (for `--explain`); it has NO I/O dependency (passes `xtask check-arch` pure-core allowlist).
- [ ] The scoring formula constants live in one place (SSOT) in the pure core.
- [ ] New read-side query methods (by object, by contributor, bounded traversal, attributed-claim feed for scoring) compile and have stub implementations in DELIVER's RED phase.
- [ ] Any new port surface ships `probe()` coverage per ADR-009.
- [ ] `adherence_weight` and `weight_bucket` have NO persistence path: they are not columns in any table, not fields in any on-disk artifact, and not serialized to any record. (A test asserts no scoring output is ever written.)
- [ ] `xtask check-arch` extends the `no_cross_table_join_elides_author` rule to cover any new scoring/traversal query that touches both `claims` and `peer_claims`.
- [ ] The storage backing (WD-8 revisit) is implemented per DESIGN's choice; this story's contract holds regardless of the choice.

### UAT Scenarios (BDD — infrastructure surface)

```gherkin
Scenario: The scoring core is pure and produces a reproducible weight
  Given a fixed set of attributed claims for a (subject, object) pairing
  When the pure scoring core computes the weighted view
  Then the resulting adherence weight equals the documented formula applied to those claims
  And computing it twice with the same input yields the identical weight
  And the scoring core has no I/O dependency

Scenario: No scoring output is ever persisted
  Given a weighted query has been run end-to-end through the new read-side path
  When the DuckDB tables, on-disk artifacts, and any published records are inspected
  Then no adherence weight and no weight-bucket label appear in any stored or published location
  And the anti-merging check rule covers the new scoring query path
```

### Outcome KPIs

n/a — supports KPI-GRAPH-1, KPI-GRAPH-2, KPI-GRAPH-3, KPI-GRAPH-4 indirectly.

### Technical Notes

- Depends on the slice-01 `claims` store, slice-03 `peer_claims` store, and the slice-03 `query_federated_by_subject` precedent being present (this story extends; it does not replace).
- Coordinates closely with US-GRAPH-003 (scoring) and US-GRAPH-004 (traversal) on the exact read-side contract.
- The WD-8 store revisit is the single biggest DESIGN decision this story enables; the contract here is deliberately storage-neutral so DESIGN can choose freely.
- Functional Rust paradigm (ADR-007): scoring is pure core; any storage effect stays behind a port in the effect shell.

---

## Summary table

| Story | Title | Job link | Right-sized? | DoR status |
|---|---|---|---|---|
| US-GRAPH-001 | Query by philosophy (object) and project (subject), attribution preserved | J-002 | YES (1.5 days, 4 scenarios) | PASS (see DoR section in feature-delta.md) |
| US-GRAPH-002 | Query by contributor (DID) — one developer's reasoning trail | J-002 | YES (1 day, 4 scenarios) | PASS |
| US-GRAPH-003 | Transparent weighted/scored view; sparse renders sparse | J-002 | YES (2.5 days, 5 scenarios) | PASS |
| US-GRAPH-004 | Traverse contributor<->project<->philosophy edges | J-002 | YES (2 days, 4 scenarios) | PASS |
| US-GRAPH-005 | Audit a weight with `--explain` per-claim arithmetic | J-002 | YES (1.5 days, 4 scenarios) | PASS |
| US-GRAPH-006 | Bootstrap pure `scoring` core + read-side extensions (`@infrastructure`) | `infrastructure-only` | YES (2 days, 2 scenarios) | PASS (with infra rationale) |

Total estimated effort: ~10.5 days at moderate confidence. Slice composition
gate: PASS — 5 user-visible stories + 1 infrastructure story; slice is NOT 100%
`@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).
