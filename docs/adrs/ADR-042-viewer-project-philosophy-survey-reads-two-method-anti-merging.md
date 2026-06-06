# ADR-042: The Viewer's LOCAL Project-Survey and Philosophy-Survey Reads — TWO Read-Only `StoreReadPort` Methods Returning a `SurveyRow` Feed, Grouped in Pure Rust (NOT SQL)

- **Status**: Accepted (slice-10 viewer-graph-traversal, DESIGN 2026-06-06). Resolves WD-GT open-question Q3 + the US-GT-001 `@infrastructure` capability.
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect), resolving Q3 for viewer-graph-traversal (slice-10).
- **Feature**: viewer-graph-traversal (slice-10)
- **Extends**: ADR-007 (pure/effect split), ADR-009 (hexagonal probe contract), ADR-014 (peer-storage anti-merging), ADR-022 (aggregation in pure Rust, never SQL), ADR-030 (the read-only `StoreReadPort`, NO mutation method), ADR-039 (the slice-09 read-only-seam precedent: add a method to `StoreReadPort`, mirror the `UNION ALL` shape).
- **Resolves**: WD-GT Q3 (one parametrized read vs two methods) + US-GT-001 (the project-survey + philosophy-survey read capability).

## Context

US-GT-001 (`@infrastructure`) stands up the two LOCAL read capabilities the
`/project` and `/philosophy` survey pages render: "every attributed claim about
this subject" and "every attributed claim about this object". The viewer reads
through the read-only `StoreReadPort` (ADR-030), which today exposes
`list_claims`/`get_claim`/`list_peer_claims`/`count_*`/`query_contributor_scoring_feed`
— none reads a subject- or object-keyed survey.

Three invariants bind the read (the I-GT-* set):

- **Read-only / no key (I-GT-1)**: the seam adds NO mutation method —
  `Box<dyn StoreReadPort>` stays structurally incapable of mutating (ADR-030).
- **LOCAL / offline (I-GT-2)**: the feed is read over the LOCAL DuckDB store ONLY
  (`claims ∪ peer_claims`) — no network. Both routes render fully network-down
  (distinct from `/search`/`/scrape`).
- **Anti-merging (I-GT-3)**: every survey row carries a non-`Option`
  `author_did` + `cid`; two same-content claims by two authors stay TWO rows; the
  grouping into "philosophies embodied" / "projects that embody" happens in PURE
  Rust, NEVER in SQL (the slice-03/04 discipline: `UNION ALL`, explicit
  `author_did`, no merge `JOIN`/`GROUP BY`).

**WD-GT Q3 (DESIGN-owned):** does ONE parametrized read (a dimension enum) back
both surveys, or TWO methods?

## Decision

**Add TWO read-only methods to `StoreReadPort` — `query_project_survey(&str
subject)` and `query_philosophy_survey(&str object)` — each returning
`Result<Vec<SurveyRow>, StoreReadError>`, implemented in `adapter-duckdb` as the
subject-/object-keyed read-only siblings of `query_contributor_scoring_feed`
(`claims UNION ALL peer_claims`, explicit `author_did`, NO merge JOIN). The
per-author grouping is done in the PURE `viewer-domain` core, never in SQL. No
mutation method, no key, no network, no new persisted type.**

### The boundary DTO (one new flat type in `ports`)

```text
pub struct SurveyRow {
    pub author_did: String,   // NON-Option — anti-merging (I-GT-3); never elided
    pub cid: String,          // NON-Option — no invented edges (I-GT-4); one edge = one signed claim
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,      // the stored DOUBLE — rendered VERBATIM (I-GT-5)
    pub origin: PeerOrigin,   // REUSED — own vs Known peer (author_did + fetched_from_pds)
    pub composed_at: DateTime<Utc>,
}
```

A FLAT DTO (mirrors `ClaimRow`/`PeerClaimRow`/`AttributedClaim`). The non-`Option`
`author_did` + `cid` make dropping attribution / fabricating an edge a COMPILE
error (type-level layers of the anti-merging + no-invented-edge invariants).

### The two read methods (read-only seam on `StoreReadPort`)

```text
pub trait StoreReadPort: Send + Sync {
    // ... existing slice-06/07/09 reads ...

    /// Every attributed claim whose subject == `subject` (own ∪ LOCAL peer,
    /// UNION ALL, NO merge JOIN). Read-only SQL only (I-GT-1). LOCAL only — no
    /// network (I-GT-2). Each row carries non-Option author_did + cid (I-GT-3/4).
    /// An empty result is Ok(vec![]) (the render layer shows the guided no-claims
    /// state — never an Err, never a crash).
    fn query_project_survey(&self, subject: &str) -> Result<Vec<SurveyRow>, StoreReadError>;

    /// Every attributed claim whose object == `object` (own ∪ LOCAL peer, UNION
    /// ALL, NO merge JOIN). Same invariants, mirrored key.
    fn query_philosophy_survey(&self, object: &str) -> Result<Vec<SurveyRow>, StoreReadError>;
}
```

### The `adapter-duckdb` impl shape (mirrors `query_contributor_scoring_feed`)

```sql
SELECT author_did, cid, subject, predicate, object, confidence, composed_at, fetched_from_pds, source_table
FROM (
  SELECT c.author_did, c.cid, c.subject, c.predicate, c.object, c.confidence, c.composed_at,
         '' AS fetched_from_pds, 'Own' AS source_table
    FROM claims c WHERE c.subject = ?            -- (philosophy: c.object = ?)
  UNION ALL
  SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object, pc.confidence, pc.composed_at,
         pc.fetched_from_pds, 'Peer' AS source_table
    FROM peer_claims pc WHERE pc.subject = ?     -- (philosophy: pc.object = ?)
) ORDER BY object, source_table, cid;            -- (philosophy: ORDER BY subject, source_table, cid)
```

Over the SAME shared `Arc<Mutex<Connection>>` (BR-VIEW-4) via the existing
`lock_conn`. The literal names BOTH `claims` and `peer_claims` AND projects
`author_did` → the `xtask` `no_cross_table_join_elides_author` rule stays GREEN.
NO `GROUP BY`/`AVG`/`COUNT` over authors — the per-claim rows ARE the output.

### Where the grouping runs (pure `viewer-domain`, not SQL, not the shell)

The effect shell reads `Vec<SurveyRow>`, then calls the PURE
`viewer_domain::TraversalView::group_project / group_philosophy` (ADR-043) to
decompose into per-`(key, author, cid)` rows. The grouping is a pure total
function — anti-merging by construction (it CANNOT average; it only buckets +
preserves attribution). This mirrors slice-08 (`compose_results`) and slice-09
(`scoring::score`): the effect is the READ; the transform is pure.

### Earned Trust (principle 12; ADR-009)

The two reads add NO new outbound dependency edge — they read the LOCAL store the
viewer ALREADY probes. `ViewerServer::probe` (ADR-028) is UNCHANGED: store
readable (sentinel `count_claims`) + loopback bind. The new reads run over the
SAME probed connection, so **wire → probe → use** holds with no new probe. The
pure grouping's Earned-Trust analog is property + mutation testing
(two-authors → two rows, no-merge, empty → `NoClaims`, non-Option author/cid).
An "environment lies" check: a read that fails (poisoned lock / read error)
degrades to `NoClaims` — no crash, no stack trace (the slice-06/09 discipline).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **ONE parametrized read** `query_survey(dimension: SurveyDimension, key: &str)` | One method, one SQL with a templated `WHERE`. | **Rejected (Q3).** A dimension-enum param is a two-arm over-generalization on a read port whose every other method is single-purpose (`list_claims` vs `list_peer_claims`). The two SQLs differ only in the `WHERE`/`ORDER BY` key, so the "duplication" is one clause; the two named methods are self-documenting at every call site and match the established port shape. SAME rationale ADR-039 used to reject a `&ScoringFilter` param for the single-dimension contributor read. |
| **Reuse the slice-04 `StoragePort::query_attributed_for_scoring(ByObject/BySubject)`** | The exact filters exist (ADR-020). | **Rejected (I-GT-1 / read-only).** Those live on the FULL `StoragePort` (which carries `write_signed_claim`, …); handing it to the viewer breaches the read-only guarantee. The survey reads must live on the read-only `StoreReadPort`. The RETURN contract is the same idea (`Vec` of attributed rows); the SQL is the read-only sibling. |
| **A new survey-specific port** (`SurveyReadPort`) | Single-purpose seam. | **Rejected (simplest-solution / one read path).** A second port with its own probe + wiring + capability-rule entry, for two reads over the SAME store the `StoreReadPort` already reads, is needless ceremony (the ADR-039 verdict). Two methods on the existing port reuse the connection, the probe, and the capability rule. |
| **Group in SQL** (`GROUP BY object` + `array_agg`) | Fewer rows over the wire. | **Rejected (I-GT-3, the cardinal invariant).** A SQL `GROUP BY` across authors merges attribution — the faceless-consensus failure the whole project forbids. Grouping MUST be pure Rust; the SQL stays per-claim + attribution-projecting (the `no_cross_table_join_elides_author` rule enforces it). |
| **Group in the effect shell** | Fewer call sites. | **Rejected (effect/pure symmetry).** Grouping is pure logic with anti-merging semantics worth property-testing in isolation; it belongs in `viewer-domain` next to the render (mirrors slice-08/09 placing the pure transform in the pure core). |

## Consequences

### Positive
- The reads are structurally read-only: methods on a port with NO mutation method
  (I-GT-1 carries by construction).
- Zero new persisted type: `SurveyRow` is a per-query boundary DTO; surveys are
  never written (I-GT-8 / WD-GT-9).
- Anti-merging is unviolatable at the type level (`author_did`/`cid` non-Option)
  AND enforced in SQL (the existing `no_cross_table_join_elides_author` rule
  covers the two new literals) AND in the pure grouping (it cannot average).
- LOCAL-first / offline: local DuckDB SELECT only; the network being down never
  degrades `/project`/`/philosophy` (I-GT-2 — offline-STRONGER than `/search`).
- One read path: the viewer's whole read surface stays on `StoreReadPort`.

### Negative
- `StoreReadPort` gains two methods (and `adapter-duckdb` two impls). Accepted:
  the read-only siblings of `query_contributor_scoring_feed`, over the SAME shared
  connection — near-identical to existing code, one `WHERE` clause apart.
- One new boundary DTO (`SurveyRow`). Accepted: a flat DTO mirroring the existing
  row types; the non-Option fields are the load-bearing anti-merging guarantee.

## Revisit Trigger
- A future contributor-DIMENSION survey page (instead of the `/score` link) →
  add `query_contributor_survey` OR widen to the `ScoringFilter` ADT; the
  `SurveyRow` contract is unchanged. Out of scope for slice-10 (the contributor
  dimension links to slice-09 `/score`, WD-GT-1).
- A survey needs to exclude peer rows (own-only) → drop the `peer_claims` UNION-ALL
  leg; the port signature + the pure grouping are unchanged.
