# ADR-048: Batch Counter-Presence Read — `IN (...)` Bound List, Ref-Tables-Only, Presence Set (no N+1)

- **Status**: Accepted
- **Date**: 2026-06-07
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-counter-claim-list-flags` (slice-12)
- **Feature**: viewer-counter-claim-list-flags (slice-12)
- **Extends**: ADR-046 (slice-11 counter-thread 2-step read — this is its Step-A widened to a set), ADR-015 (counter = reference of type `counters`), ADR-030 (read-only `StoreReadPort`), ADR-014/042 (peer-storage anti-merging, two-store UNION-ALL reads), ADR-007 (functional paradigm)

## Context

slice-12 surfaces a neutral "Countered" presence flag on each `GET /claims` list row whose
claim has ≥1 counter. The `/claims` page renders up to 50 rows. Answering "is each of these
50 claims countered?" by calling the slice-11 per-CID `query_counter_claims(target_cid)`
once per row would be **50 queries per page (N+1)** — the exact risk ADR-046 §Consequences
explicitly DEFERRED to this slice.

DESIGN must specify a BATCH read that answers "which of these page CIDs are countered?" in
ONE aggregate query, presence-only (anti-merging-safe), read-only, LOCAL, and with the
variable-length CID list bound safely (no SQL injection).

## Decision

Add a read-only `StoreReadPort::counter_presence_for(target_cids: &[String]) ->
Result<HashSet<String>, StoreReadError>` implemented in `adapter-duckdb` as a **single
aggregate `IN (...)` read over the reference tables ONLY**:

```sql
SELECT DISTINCT referenced_cid FROM (
    SELECT referenced_cid FROM claim_references
        WHERE referenced_cid IN (?,?,…?) AND ref_type = 'counters'
    UNION ALL
    SELECT referenced_cid FROM peer_claim_references
        WHERE referenced_cid IN (?,?,…?) AND ref_type = 'counters'
)
```

Resolved sub-decisions:

1. **`IN (...)` dynamic bind, NOT string interpolation.** The placeholder run (`?,?,…`,
   count == input length) is generated; the CIDs are bound via
   `duckdb::params_from_iter(target_cids.iter().chain(target_cids.iter()))` (two `IN`
   arms → the slice is bound twice, mirroring slice-11's `[target_cid, target_cid]`
   precedent). CIDs are NEVER concatenated into the SQL text → injection-safe. (A
   `WITH ids(cid) AS (VALUES …)` CTE binding once is an accepted alternative; the
   contract is bound params + one query + DISTINCT CID set.)
2. **Presence SET, not a count.** Returns `HashSet<String>` — the countered subset of the
   input. A membership set can never be misread as a "disputed by N" verdict (no N exists
   in the type). This realizes I-LF-3 (anti-merging) at the type level. If a future slice
   shows a count, it MUST be the true per-CID `COUNT(*)` of distinct counters, never an
   aggregate "net verdict" — but slice-12 ships presence-only.
3. **Ref-tables-ONLY, no JOIN to `claims`/`peer_claims`, no Step-B artifact read.** The
   flag carries no author and no reason text, so the read needs neither the claims tables
   nor the on-disk artifact. This is the sole divergence from ADR-046's 2-step read:
   slice-12 is **Step-A only, widened `= ?` → `IN (...)`, projected to the bare
   `referenced_cid`**.
4. **Empty input → empty set, ZERO queries.** `target_cids.is_empty()` returns
   `Ok(HashSet::new())` without preparing a statement (an empty `IN ()` is a SQL error and
   pointless work).
5. **`UNION ALL` + outer `DISTINCT`.** Collapses a CID countered by multiple authors /
   across both stores to ONE set member → ONE flag per CID (I-LF-3).
6. **ONE query per page, invariant to page size** (the N+1 guard, I-LF-8) — pinned by a
   gold/`@property` acceptance test asserting query count is independent of row count.

## Alternatives considered

| Option | Pro | Con | Verdict |
|---|---|---|---|
| **Batch `IN (...)` presence set over ref tables only** (chosen) | ONE indexed query/page; presence-only (anti-merging by type); no artifact read; no migration; reuses slice-11 Step-A + slice-03 indexes; out of the `no_cross_table_join_elides_author` rule's scope | introduces the first variable-length `IN` bind in the codebase (no existing precedent) | **ACCEPTED** |
| Per-CID loop reusing `query_counter_claims` | zero new method | **N+1 (50 queries/page)** — the exact regression DISCUSS forbids (I-LF-8); also does a needless Step-B artifact read per counter | REJECTED |
| Return a count map (`HashMap<String, usize>`) | enables a future count badge | invites a "disputed by N" merged-verdict misread on the list (violates I-LF-3); slice-12 needs only presence | REJECTED (presence-only is the product default) |
| JOIN the claims tables in the presence query | one shape closer to slice-11 | unnecessary (flag carries no author/content); WOULD bring `claims`+`peer_claims` into one literal → must then project `author_did` to satisfy the anti-merging xtask rule, adding cost for nothing | REJECTED |
| Denormalize a `has_counter` boolean onto `claims` (migration) | one-column read | a schema migration is out of a ~1-day reuse-first slice; duplicates the authoritative ref graph → drift surface | REJECTED |

## Consequences

- **Positive**: N+1 risk retired by a single indexed aggregate read; anti-merging
  preserved by the SET return type AND the ref-tables-only literal (no attribution to
  elide); no migration; reuses the slice-03 `referenced_cid` indexes and the slice-11
  Step-A shape; offline/LOCAL; the pure render becomes a total function of
  `(page, presence)`.
- **Negative**: the first variable-length `IN (...)` bind in `adapter-duckdb` — CRAFT must
  generate the placeholder run + bind via `params_from_iter` (the empty-slice guard makes
  the degenerate case total). Bounded by page size 50.
- **Enforcement (read-only, 3-layer + anti-merging)**:
  - *Type*: `StoreReadPort` declares no mutation method (read-only by trait shape);
    `HashSet` return (no count).
  - *Arch (xtask)*: `check_viewer_capability_boundary` (no signing/PDS/network dep on the
    viewer) unchanged; `no_cross_table_join_elides_author` structurally NOT tripped (the
    literal names `claim_references`/`peer_claim_references`, never the bare
    `claims`/`peer_claims` words). **No new xtask rule, no new dep edge — xtask delta is
    NONE.**
  - *Behavioral (gold)*: single-query-per-page invariant (N+1 guard); read-only
    (row-count universe unchanged); flag iff a real `ref_type='counters'` ref exists;
    empty-set-when-none; renders offline.
- **`/peer-claims` and the graph/score surfaces**: OUT of slice-12. The same
  `counter_presence_for` read serves them later (slice-13) against different row
  projections + render sites; this ADR's read is reusable as-is.

## Enforcement tooling recommendation

Rust-appropriate, already in place: `xtask check-arch` (`syn`-based dep-graph + SQL-literal
rules) for the structural/arch layers; `cargo test` gold + `proptest`-style `@property`
tests for the behavioral layer (single-query invariant, byte-identity render). No new
tooling required.
