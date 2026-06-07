# Data Models — viewer-counter-claim-list-flags (slice-12)

> Wave: DESIGN · Date: 2026-06-07 · NO new tables, NO migration, NO new DTO struct.

slice-12 adds NO new table and NO migration. It reuses the slice-03 indexed reference
tables and adds ONE bool field to an existing view-model. The new read returns a plain
`HashSet<String>` — no boundary struct.

## 1. The batch counter-presence read (the load-bearing query shape)

### Port signature (`ports::StoreReadPort`)

```text
/// Read-only BATCH counter-presence: of the given page CIDs, which have ≥1
/// `ref_type='counters'` reference in claim_references ∪ peer_claim_references?
/// ONE aggregate query (no N+1, I-LF-8); LOCAL; presence-only set membership
/// (anti-merging, never a count). Empty input → empty set, zero queries.
fn counter_presence_for(&self, target_cids: &[String])
    -> Result<HashSet<String>, StoreReadError>;
```

### Return type decision — boolean PRESENCE set (not a count)

`HashSet<String>` — the subset of `target_cids` that are countered. The renderer projects
`is_countered = presence.contains(&row.cid)`.

- **PRESENCE chosen (DISCUSS default I-LF-3 confirmed).** A set membership is anti-
  merging-safe by construction: it can NEVER be misread as a "disputed by N" verdict
  because no N exists in the type. Per-counter attribution + count live in the slice-11
  thread the flag links to.
- **Count rejected.** A `HashMap<String, usize>` would invite a "disputed by 2" badge on
  the list — exactly the merged-verdict misread DISCUSS forbids. If a future slice ever
  shows a count, ADR-048 mandates it be the TRUE per-CID `COUNT(*)` of distinct counters
  (never an aggregate "net verdict") — but slice-12 ships presence-only.

### Exact SQL shape (`adapter-duckdb`)

Variable-length `IN (...)` built from a generated placeholder run; bound, never
interpolated. With `n = target_cids.len()` placeholders `p = "?,?,…,?"` (n times):

```sql
SELECT DISTINCT referenced_cid FROM (
    SELECT referenced_cid FROM claim_references
        WHERE referenced_cid IN (p) AND ref_type = 'counters'
    UNION ALL
    SELECT referenced_cid FROM peer_claim_references
        WHERE referenced_cid IN (p) AND ref_type = 'counters'
)
```

Binding: `duckdb::params_from_iter(target_cids.iter().chain(target_cids.iter()))` — the
CID slice is bound TWICE (once per `IN`-clause arm), in order. (Alternatively a single
CTE `WITH ids(cid) AS (VALUES (?),(?),…)` binds once; DESIGN's recommended form is the
two-arm `params_from_iter` chain — it mirrors the slice-11 `[target_cid, target_cid]`
double-bind precedent exactly and needs no VALUES-list construction. CRAFT may choose the
CTE form if it proves cleaner; the contract is: bound params, one query, DISTINCT CID
set.)

- **`UNION ALL` then outer `DISTINCT`** (not `UNION`): the outer `DISTINCT referenced_cid`
  collapses duplicates across BOTH arms AND within an arm (a CID countered by two authors
  appears once) — ONE flag per CID (I-LF-3). `UNION ALL` + outer `DISTINCT` is equivalent
  here and keeps each arm a simple indexed scan.
- **Indexed**: each arm filters on `referenced_cid` (covered by
  `idx_claim_references_referenced` / `idx_peer_claim_refs_referenced`, slice-03).
- **Ref-tables-only**: NO JOIN to `claims`/`peer_claims`. The flag needs neither author
  nor content. This keeps the query (a) Step-A-only (no slice-11 Step-B artifact read),
  and (b) out of the `no_cross_table_join_elides_author` xtask rule's scope (the literal
  names `claim_references`/`peer_claim_references`, never the bare `claims`/`peer_claims`).

### Empty-input handling

```text
if target_cids.is_empty() { return Ok(HashSet::new()); }   // no statement prepared
```

An empty slice never reaches the SQL (empty `IN ()` is an error). This guards the empty-
first-page and store-read-failure-degraded-to-empty paths.

### One-query-per-page guarantee (the N+1 guard)

`counter_presence_for` is called EXACTLY ONCE per `/claims` render, with the page's full
CID list, regardless of how many rows the page holds. The query count is invariant to
page size (asserted by a gold/`@property` acceptance test in DISTILL/CRAFT). This widens
slice-11's per-CID `query_counter_claims` (one CID, `= ?`) to the page set (n CIDs,
`IN (...)`) in ONE read — the precise N+1 risk ADR-046 §Consequences deferred to slice-12.

## 2. View-model delta (`viewer-domain::ClaimRowView`)

```text
pub struct ClaimRowView {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub is_countered: bool,   // NEW — projected from the presence set (slice-12)
}
```

- `is_countered` is set by `from_row_with_presence(row, &presence)` in the effect shell:
  `presence.contains(&row.cid)`. It drives ONLY the per-row flag render — never order,
  paging, count, or the verbatim `confidence` (I-LF-2 / I-LF-4).
- No other view-model, DTO, or `Page<T>`/`PageView<T>` field changes.

## 3. Reused storage (NO change)

| Table | Columns used | Index | Source |
|---|---|---|---|
| `claim_references` | `referenced_cid`, `ref_type` | `idx_claim_references_referenced (referenced_cid)` | schema.rs (v1, slice-03) |
| `peer_claim_references` | `referenced_cid`, `ref_type` | `idx_peer_claim_refs_referenced (referenced_cid)` | schema_v3.rs (v3, slice-03) |

`ref_type` CHECK already constrains values to `('retracts','corrects','counters','supersedes')`;
the read filters `ref_type = 'counters'`. NO schema migration, NO new column, NO new index.

## 4. `list_claims` — UNTOUCHED

The existing `list_claims` SQL (`ORDER BY composed_at DESC, cid LIMIT ? OFFSET ?`) and its
`COUNT(*)` total are byte-identical. The presence read is a SEPARATE query whose result is
mapped onto rows by the pure projection AFTER paging. Order/paging/count cannot change
(US-LF-003 byte-identity contract).
