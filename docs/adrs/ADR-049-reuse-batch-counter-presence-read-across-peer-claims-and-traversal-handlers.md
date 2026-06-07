# ADR-049: Reuse the slice-12 Batch Counter-Presence Read Across the `/peer-claims` + `/project` + `/philosophy` Handlers (NO new read method)

- **Status**: Accepted
- **Date**: 2026-06-07
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-counter-flags-graph-surfaces` (slice-13)
- **Feature**: viewer-counter-flags-graph-surfaces (slice-13)
- **Extends**: ADR-048 (slice-12 batch counter-presence read — REUSED VERBATIM here), ADR-015 (counter = reference of type `counters`), ADR-030 (read-only `StoreReadPort`), ADR-007 (functional paradigm — pure core + effect shell)

## Context

slice-12 shipped `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>`
(ADR-048): a single aggregate `IN (...)` UNION-ALL DISTINCT read over
`claim_references ∪ peer_claim_references` answering "which of these page CIDs have ≥1
`ref_type='counters'` reference?" — presence-only, read-only, LOCAL, N+1-free. slice-12
wired it into ONE handler (`claims_page`, the own-claims list).

slice-13 extends the SAME neutral "Countered" presence flag to the other two LOCAL surfaces
the operator scans: the FEDERATED `/peer-claims` list and the GRAPH-TRAVERSAL `/project` +
`/philosophy` edge surveys. Each surface has rows/edges carrying a `cid` and needs the SAME
question answered: "which of these page CIDs are countered?"

DESIGN must decide whether to add new read method(s) tailored per surface, or REUSE the
slice-12 read across all three handlers.

## Decision

**REUSE `counter_presence_for` VERBATIM across all three handlers. Add NO new read method
and NO new SQL.** Each handler (`peer_claims_page`, `project_page`, `philosophy_page`)
collects its render's CID set and calls the EXISTING slice-12 read ONCE per render, then
passes the returned `HashSet<String>` into the pure `viewer-domain` projection.

- The read's contract (presence-only set membership, anti-merging-safe `HashSet`, one
  aggregate query, empty-input short-circuit, bound parameters, ref-tables-only) is EXACTLY
  what every slice-13 surface needs — it was designed surface-agnostic over a bare CID list.
- ONE aggregate query per render on EVERY surface (the N+1 guard, I-CF-8). The
  edge-surface flatten (collecting every edge CID across every `EdgeGroup` into ONE call) is
  specified in ADR-050.
- The own arm (`claim_references`) and the peer arm (`peer_claim_references`) both feed the
  set, so a survey edge that is an OWN claim and a peer-list row that is a PEER claim are
  both flaggable by cid with no per-surface branching.

## Alternatives Considered

### Alternative 1 — Add a per-surface read method (e.g. `peer_counter_presence_for`, `edge_counter_presence_for`)

- **Evaluation**: Each would be a near-identical copy of the slice-12 query. They would
  multiply the SQL surface, the binding logic, the empty-input guard, and the N+1 property
  test by three — for zero behavioral difference (the question is identical: "is this CID
  countered?" over the SAME two ref tables).
- **Rejected because**: it violates the slice's defining reuse-first constraint (I-CF-7),
  adds three maintenance points where one suffices, and risks drift between surfaces. The
  slice-12 read is already surface-agnostic; specializing it is pure duplication.

### Alternative 2 — A single new method taking a "surface" enum to vary the query

- **Evaluation**: Would let one method branch per surface — but every branch would issue the
  SAME query (the ref tables are shared; the flag is presence-only regardless of which list
  the cid came from). The enum would be dead variance.
- **Rejected because**: the query genuinely does not vary by surface — a CID is countered or
  not, independent of which list rendered it. An enum parameter would be ceremony with no
  behavioral payload, and it would force a signature change to a SHIPPED, property-tested
  read.

### Alternative 3 — Per-row / per-edge call to the slice-11 `query_counter_claims`

- **Evaluation**: Re-introduces the exact N+1 (one query per row/edge) that ADR-048 was
  built to avoid — catastrophic on the edge surfaces where a survey spans many groups.
- **Rejected because**: it is the regression I-CF-8 forbids. The batch read exists precisely
  to make this one query.

## Consequences

### Positive
- **Zero new read surface**: no new method, no new SQL, no new binding/short-circuit logic,
  no new N+1 property test — the slice-12 read's guarantees carry verbatim to three surfaces.
- **N+1-free on all three surfaces** by construction (one call per render).
- **No drift**: all four "Countered" surfaces (`/claims`, `/peer-claims`, `/project`,
  `/philosophy`) share ONE read with ONE behavior; a fix to the read fixes all four.
- **Workspace stays 21 members; `ports` + `adapter-duckdb` untouched** — the change is two
  view-model fields + render arms + three handler wirings (architecture-design.md §11).
- **xtask delta NONE**: no new SQL means `no_cross_table_join_elides_author` is not
  re-evaluated; the REUSED query is already in-bounds (ref-table-only literal).

### Negative / trade-offs
- The slice-13 handlers depend on the slice-12 read's contract; a future change to that read
  must consider four call sites, not one. Mitigated: the contract (presence-only set over a
  CID list) is minimal and stable, and a behavioral gold per surface pins the observable
  flag.
- The edge surfaces must take care to flatten CIDs across groups into ONE call (the
  N+1 trap is in the CALLER, not the read). This is the subject of ADR-050 and is enforced
  by a query-count behavioral test.

## Enforcement

- **Behavioral** (DISTILL/CRAFT): one query per render on each of the three routes, asserted
  through the real `openlore ui` subprocess; the inherited slice-12 adapter N+1 property.
- **Type**: the REUSED read returns `HashSet<String>` (presence, never a count — anti-merging
  by construction); the trait has no mutation method (read-only).
- **Arch** (`xtask check-arch`): UNCHANGED — no new dep edge, no new SQL literal, the viewer
  capability boundary holds.
