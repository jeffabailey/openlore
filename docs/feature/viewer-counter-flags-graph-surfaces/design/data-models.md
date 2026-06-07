# Data Models — viewer-counter-flags-graph-surfaces (slice-13)

> Wave: DESIGN · Date: 2026-06-07 · NO new tables, NO migration, NO new DTO struct,
> NO new read method, NO new SQL.

slice-13 adds NO new table, NO migration, and NO new read. It REUSES the slice-12 batch
counter-presence read verbatim and adds ONE bool field to EACH of two existing view-models
(`PeerClaimRowView`, `EdgeRow`). The read returns a plain `HashSet<String>` — no boundary
struct.

## 1. The batch counter-presence read — REUSED VERBATIM (NO new method, NO new SQL)

### Port signature (`ports::StoreReadPort`) — UNCHANGED (slice-12)

```text
/// Read-only BATCH counter-presence: of the given page CIDs, which have ≥1
/// `ref_type='counters'` reference in claim_references ∪ peer_claim_references?
/// ONE aggregate query (no N+1, I-CF-8); LOCAL; presence-only set membership
/// (anti-merging, never a count). Empty input → empty set, zero queries.
fn counter_presence_for(&self, cids: &[String])
    -> Result<HashSet<String>, StoreReadError>;
```

Present at `crates/ports/src/store_read.rs:380`. slice-13 calls it from three handlers; the
signature, the SQL, the empty-input short-circuit, the bound-parameter binding, and the
`UNION ALL` + outer `DISTINCT` shape are ALL slice-12 and UNCHANGED. The exact SQL is
documented in the slice-12 data-models.md §1 and ADR-048; it is NOT restated or modified
here.

### Call sites (NEW — the only data-flow change)

| Surface | CID set collected from | Call site | Count per render |
|---|---|---|---|
| `/peer-claims` | `read_page.rows.iter().map(\|r\| r.cid)` (the page's peer-claim rows) | `peer_claims_page` | exactly 1 |
| `/project` | `survey_rows.iter().map(\|r\| r.cid)` (ALL `SurveyRow`s — i.e. every edge across every future group, flattened ONCE before `group_by`) | `project_page` (resolve step) | exactly 1 |
| `/philosophy` | `survey_rows.iter().map(\|r\| r.cid)` (same — flattened once before `group_by`) | `philosophy_page` (resolve step) | exactly 1 |

**The flatten point (the N+1 guard for the edge surfaces).** The survey read returns a FLAT
`Vec<SurveyRow>` BEFORE `group_by` nests it into `EdgeGroup`s. Collecting the CID set from
that flat slice is provably "every edge CID across every group, exactly once" — one `map`,
one `counter_presence_for` call. The presence set is then passed INTO the grouper, which
sets each `EdgeRow.is_countered`. The read is never per-group and never per-edge (I-CF-8).

### One-query-per-render guarantee (the N+1 guard)

`counter_presence_for` is called EXACTLY ONCE per render on each of the three surfaces,
with that surface's full CID list, regardless of row / edge / group count. The query count
is invariant to size (asserted by a gold/`@property` acceptance test in DISTILL/CRAFT +
the inherited slice-12 adapter-duckdb N+1 property). An empty CID slice (empty `/peer-claims`
page, or a `NoClaims` survey with zero edges) short-circuits to `Ok(HashSet::new())` with
NO query (slice-12 behavior, REUSED).

## 2. View-model deltas

### 2a. `viewer-domain::PeerClaimRowView` (US-CF-002)

```text
pub struct PeerClaimRowView {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub origin: PeerOrigin,
    pub is_countered: bool,   // NEW — projected from the presence set (slice-13)
}
```

- `is_countered` is set by `from_row_with_presence(row, &presence)` in the effect shell:
  `presence.contains(&row.cid)`. It drives ONLY the per-row flag render — never order,
  paging, count, the verbatim `confidence`, or the `origin` (I-CF-2 / I-CF-4).
- Mirrors `ClaimRowView.is_countered` (slice-12, `viewer-domain/src/lib.rs:78`) exactly.

### 2b. `viewer-domain::EdgeRow` (US-CF-003)

```text
pub struct EdgeRow {
    pub author_did: String,
    pub confidence: f64,
    pub cid: String,
    pub is_countered: bool,   // NEW — projected from the presence set (slice-13)
}
```

- `is_countered` is set inside `group_by` when each `EdgeRow` is constructed from its
  `SurveyRow`: `is_countered: presence.contains(&row.cid)` (the presence set is threaded as
  a new `group_by` / `group_project` / `group_philosophy` parameter — component-boundaries.md
  §2). It drives ONLY the per-edge flag render — never the `group_by` grouping, the group
  order, the edge order within a group, the deduped `contributors` list, any cross-link, or
  the verbatim `confidence` / bucket (I-CF-9 / I-CF-2 / I-CF-4).
- No change to `EdgeGroup` (`{ key, edges }`) or `TraversalView::Found { entity, groups,
  contributors }` — the bool lives on the leaf `EdgeRow` only.

No other view-model, DTO, `Page<T>`, `PageView<T>`, or `SurveyRow` field changes.

## 3. Reused storage (NO change)

| Table | Columns used | Index | Source |
|---|---|---|---|
| `claim_references` | `referenced_cid`, `ref_type` | `idx_claim_references_referenced (referenced_cid)` | schema.rs (v1, slice-03) |
| `peer_claim_references` | `referenced_cid`, `ref_type` | `idx_peer_claim_refs_referenced (referenced_cid)` | schema_v3.rs (v3, slice-03) |

The REUSED read filters `ref_type = 'counters'` over the UNION of both tables. NO schema
migration, NO new column, NO new index. Both the own arm (`claim_references` — an own claim
countered) and the peer arm (`peer_claim_references` — a peer claim countered) feed the SAME
`HashSet`, so a survey edge that is an OWN claim and one that is a PEER claim are both
flaggable by cid (architecture-design.md §9).

## 4. Existing reads — UNTOUCHED

`list_peer_claims` (`OFFSET/LIMIT` paging + `COUNT(*)` total), `query_project_survey`, and
`query_philosophy_survey` (their `ORDER BY` deterministic edge ordering) are ALL
byte-identical. The presence read is a SEPARATE query whose result is mapped onto
rows/edges by the pure projection AFTER paging/surveying. Order / paging / count / grouping
/ edge order / contributor list cannot change (US-CF-002 / US-CF-003 byte-identity
contracts).
