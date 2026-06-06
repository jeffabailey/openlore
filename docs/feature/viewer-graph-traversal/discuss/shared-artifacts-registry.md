# Shared Artifacts Registry: viewer-graph-traversal (slice-10)

> Every `${variable}` in the journey mockups has a single source of truth +
> documented consumers. The load-bearing three — `author_did`, `claim_cid`,
> `confidence` — are integration INVARIANTS (anti-merging + verbatim + no-invented-
> edges), not per-page choices. Brownfield DELTA: these artifacts already exist in
> the slice-01/03/04/06/09 store + render path; slice-10 adds two new consumers
> (the survey pages) and the cross-link consumers.

## Registry

```yaml
shared_artifacts:

  author_did:
    source_of_truth: "claims.author_did / peer_claims.author_did (DuckDB; derived from the signed payload `author`, slice-01/03)"
    consumers:
      - "/claims, /peer-claims row header (slice-06)"
      - "/score query arg + breakdown (slice-09)"
      - "/search result row (slice-08)"
      - "/project survey: each philosophy edge row + each contributor link (slice-10 NEW)"
      - "/philosophy survey: each project edge row + each contributor link (slice-10 NEW)"
      - "cross-link: contributor cell → /score?contributor=<did> (slice-10 NEW)"
    owner: "viewer-graph-traversal (consumes); slice-01/03 (produces)"
    integration_risk: "HIGH — attribution loss in any survey/aggregate is a fatal anti-merging failure (KPI-GRAPH-2 / KPI-FED-1 / I-FED-1). Non-Option on every survey row by type."
    validation: "Survey UAT: two authors on one (subject,object) → two rows, never averaged. xtask check-arch no-author-eliding SQL rule + behavioral gold."

  claim_cid:
    source_of_truth: "claims.cid / peer_claims.cid (DuckDB; each edge = exactly one signed claim)"
    consumers:
      - "/claims/{cid} detail (slice-06)"
      - "/score per-claim breakdown (slice-09)"
      - "/project survey: each philosophy edge row (slice-10 NEW)"
      - "/philosophy survey: each project edge row (slice-10 NEW)"
    owner: "viewer-graph-traversal (consumes); slice-01/03 (produces)"
    integration_risk: "HIGH — every displayed edge MUST carry a cid; a survey row without a cid is a fabricated edge (I-GT-4 / slice-04 traversal contract: no invented edges). Non-Option by type."
    validation: "Survey UAT: each edge row names its cid; empty survey → 'no claims', never a cid-less edge."

  confidence:
    source_of_truth: "claims.confidence / peer_claims.confidence (numeric DOUBLE, WD-10)"
    consumers:
      - "/claims, /claims/{cid}, /peer-claims (slice-06)"
      - "/score scoring input + display (slice-09)"
      - "/search result row (slice-08)"
      - "/project + /philosophy survey edge rows (slice-10 NEW)"
    owner: "viewer-graph-traversal (consumes); slice-01/03 (produces)"
    integration_risk: "HIGH — must render VERBATIM (0.90, never 0.9/90%) with the slice-04 display-only bucket; the viewer recomputes NO weight (J-002c stays at /score). KPI-4 / FR-VIEW-8 / WD-10."
    validation: "Reuse the single render_confidence + bucket site (no new formatting path). UAT: 0.90/0.88 shown verbatim with bucket."

  subject:
    source_of_truth: "claims.subject / peer_claims.subject (project URI, slice-01/03)"
    consumers:
      - "/claims row (slice-06)"
      - "/project query arg ?subject= (slice-10 NEW — the survey key)"
      - "/philosophy survey: each project edge row + its → /project link (slice-10 NEW)"
      - "cross-link: subject cell → /project?subject=<uri> (slice-10 NEW)"
    owner: "viewer-graph-traversal (consumes); slice-01/03 (produces)"
    integration_risk: "MEDIUM — a subject linked from /claims or /philosophy MUST resolve to the same /project survey key (URI-equality, percent-encoding-safe), or traversal continuity breaks (dead-link / wrong-entity)."
    validation: "Traversal UAT: click subject on /claims → land on /project for the SAME subject. DESIGN owns href percent-encoding (reuse percent_decode_form)."

  object:
    source_of_truth: "claims.object / peer_claims.object (philosophy URI, slice-01/03)"
    consumers:
      - "/claims row (slice-06)"
      - "/philosophy query arg ?object= (slice-10 NEW — the survey key)"
      - "/project survey: each philosophy edge row + its → /philosophy link (slice-10 NEW)"
      - "cross-link: object cell → /philosophy?object=<uri> (slice-10 NEW)"
    owner: "viewer-graph-traversal (consumes); slice-01/03 (produces)"
    integration_risk: "MEDIUM — an object linked from /claims or /project MUST resolve to the same /philosophy survey key; mismatch breaks traversal continuity."
    validation: "Traversal UAT: click object on /project → land on /philosophy for the SAME object."

  weight_bucket:
    source_of_truth: "DERIVED display-only label (speculative <0.3 / weighted 0.3–0.7 / well-evidenced 0.7–0.9 / triangulated >0.9); the slice-04 display-only bucket; inherits WD-10; NEVER persisted"
    consumers:
      - "/project + /philosophy survey edge rows (slice-10 — display only)"
    owner: "viewer-graph-traversal (consumes the slice-04 bucket fn)"
    integration_risk: "LOW — display-only; reuse the slice-04 bucket derivation; the viewer adds no new bucket logic and recomputes no weight."
    validation: "Reuse the slice-04 bucket fn (single site); UAT asserts the bucket label matches the verbatim confidence band."
```

## Integration validation questions (answered)

- **Does every `${variable}` in the survey mockups have a documented source?**
  Yes — all six above, each sourced to the slice-01/03 DuckDB store.
- **If a confidence changes, would all consumers update?** Yes — the survey reads
  the store row each query; there is no cached/persisted survey (WD-GT-9). The
  single `render_confidence` site is reused, so verbatim display is consistent.
- **Are there hardcoded values that should reference a shared artifact?** No — the
  survey rows are projected from the live store read; the cross-link hrefs are
  constructed from the live `subject`/`object`/`author_did` values.
- **Do any two surfaces show the same data from different sources?** No — `/project`
  and `/philosophy` read the SAME `claims ∪ peer_claims` store the existing
  surfaces read; the contributor traversal target `/score` is the slice-09 route
  over the same store.

## Anti-merging integration invariant (the cardinal one)

A survey is an AGGREGATE VIEW that NEVER merges authors. Two claims on the same
(subject, object) by two authors are TWO attributed rows; no average, no
"consensus" row, ever. Grouping happens in the PURE viewer-domain core (Rust),
never in SQL — the survey read is `UNION ALL` claims ∪ peer_claims with NO merge
JOIN, exactly mirroring the slice-09 `query_contributor_scoring_feed`. This
extends KPI-GRAPH-2 / KPI-FED-1/2 / I-FED-1 onto the two new survey surfaces and
is release-blocking.
