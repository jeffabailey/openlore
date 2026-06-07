# ADR-050: Flatten Edge CIDs Before Grouping and Set `EdgeRow.is_countered` Inside `group_by` (Traversal Surfaces, N+1 + No-Regrouping)

- **Status**: Accepted
- **Date**: 2026-06-07
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-counter-flags-graph-surfaces` (slice-13)
- **Feature**: viewer-counter-flags-graph-surfaces (slice-13)
- **Extends**: ADR-049 (reuse the slice-12 read across handlers), ADR-048 (the batch presence read), ADR-042/043 (the `/project`+`/philosophy` two-method survey reads + the `TraversalView`/`group_by` projection), ADR-007 (functional paradigm)

## Context

On `/project` and `/philosophy`, each traversal EDGE is one signed claim (`EdgeRow`, carrying
a `cid`). Edges are nested inside `EdgeGroup`s by the pure `group_by` engine
(`viewer-domain/src/lib.rs:2116`): the survey read returns a FLAT `Vec<SurveyRow>`, and
`group_by` partitions it into groups (by `object` for `/project`, `subject` for
`/philosophy`), preserving order and deduping the contributor list.

slice-13 must flag each edge whose claim is countered, WITHOUT (a) issuing more than one
presence query per render — even though edges span multiple groups (the N+1 trap, I-CF-8) —
and WITHOUT (b) changing the `group_by` grouping, group order, edge order, or contributor
list (the no-regrouping invariant, I-CF-9). Two coupled questions: WHERE to collect the CID
set for the single read, and WHERE to set each `EdgeRow.is_countered`.

## Decision

**(1) Collect the edge CID set from the FLAT `SurveyRow` slice, BEFORE `group_by` nests it —
one `map`, one `counter_presence_for` call.** The flat survey `rows` slice IS the union of
every edge across every future group, so `rows.iter().map(|r| r.cid.clone())` is provably
"every edge CID, exactly once." The effect shell collects this set, calls the REUSED read
ONCE, and passes the resulting `HashSet` into the grouper.

**(2) Set `EdgeRow.is_countered` INSIDE `group_by`, at edge construction, from the threaded
presence set.** Widen `group_project` / `group_philosophy` / `group_by` to take
`presence: &HashSet<String>`; when each `EdgeRow` is built (`viewer-domain/src/lib.rs:2139`),
set `is_countered: presence.contains(&row.cid)`. The grouping algorithm — `key_order`, the
per-key edge accumulation, the `contributors` dedup — is UNCHANGED; the bool is an extra
leaf field, orthogonal to all ordering. The render stays a **total function of the
`TraversalView`** (no second argument to `render_edge_row`, no edge/flag misalignment).

`render_edge_row` (the SINGLE arm both `render_project_fragment` and
`render_philosophy_fragment` funnel through) emits the REUSED `COUNTERED_PRESENCE_FLAG`
`<a href="/claims/{cid}">Countered</a>` iff `edge.is_countered` — ONE render change covering
BOTH routes.

## Alternatives Considered

### Alternative 1 (CID collection) — Collect CIDs AFTER grouping, by walking the `EdgeGroup`s

- **Evaluation**: Would require iterating `view.groups.iter().flat_map(|g| g.edges.iter())`
  to rebuild the flat CID list AFTER `group_by` already had it — and tempts a per-group call
  shape (`for group in groups { counter_presence_for(group edge cids) }`), which is the N+1
  regression.
- **Rejected because**: the flat `rows` slice already holds every CID before nesting; walking
  the nested structure to recover it is wasted work and invites the per-group N+1 trap. Pre-
  grouping collection is the natural, provably-single-call flatten.

### Alternative 2 (where to set the bool) — A post-pass that mutates the built `TraversalView`

- **Evaluation**: Build the `TraversalView` un-flagged (current `group_by`), then walk it and
  set each edge's bool from the presence set.
- **Rejected (recommended against, not forbidden)**: it makes `EdgeRow` mutable-after-build
  and adds a second traversal. The in-grouper form keeps the edge immutable-once-built, sets
  the bool exactly where the edge is born from its `SurveyRow` (which carries the cid), and
  needs no extra walk. CRAFT MAY choose the post-pass if it proves cleaner — the PRODUCT
  contract is the AC; DESIGN recommends the in-grouper form. Either way the byte-identity
  no-regression gold is the same.

### Alternative 3 (where to set the bool) — Pass the presence set to the RENDERER, not the grouper

- **Evaluation**: Keep `EdgeRow` un-flagged and pass `&presence` as a second argument all the
  way down to `render_edge_row`, computing `contains` at render time.
- **Rejected because**: it threads an extra argument through `render_traversal_result` →
  `render_edge_group` → `render_edge_row`, breaking the "render is a total function of the
  view" property and risking the view and the flag-source diverging. Putting the bool on the
  view-model (as slice-12 did for `ClaimRowView`) keeps the render argument-free and the data
  self-contained.

## Consequences

### Positive
- **One presence query per render**, invariant to edge and GROUP count (I-CF-8) — the flatten
  is structurally single-call.
- **No re-grouping / no re-ordering** (I-CF-9): the bool is orthogonal to `group_by`'s
  ordering logic (which depends only on `rows` order + `key_of`); grouping, group order, edge
  order, and contributor dedup are byte-identical to slice-10 with markers elided.
- **ONE render arm covers BOTH routes**: `render_edge_row` is shared, so `/project` and
  `/philosophy` cannot drift; the flag is added once.
- **Render stays a total function of `TraversalView`** — pure, no-I/O, unit/property-testable
  with no presence argument at render time.

### Negative / trade-offs
- `group_by` (and its two public wrappers) gain a parameter — a signature change to a SHIPPED
  pure function. Mitigated: it is additive (an empty set yields the slice-10 behavior), the
  function stays pure, and the slice-10 callers are the two handlers being edited anyway.
- The CID-flatten correctness (one call, all groups) lives in the CALLER (the handler), so it
  is pinned by a behavioral query-count test rather than the type system. Accepted: the
  flat-slice collection is the simplest provably-single-call shape, and the test is cheap.

## Enforcement

- **Behavioral** (DISTILL/CRAFT): query count invariant to edge/group count on `/project` +
  `/philosophy` (one call per render across all groups); byte-identity gold of the survey
  render with markers elided (grouping/edge-order/group-order/contributor-list unchanged) —
  the slice-12 baseline+marker-elision tactic.
- **Structural**: the bool lives on `EdgeRow`; `render_edge_row` takes only `&EdgeRow` (render
  is a total function of the view).
- **Type**: presence is a `HashSet<String>` (presence, never a count); the trait stays
  read-only.
