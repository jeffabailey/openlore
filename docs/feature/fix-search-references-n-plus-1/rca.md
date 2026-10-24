# RCA: 1001-claim search misses the 5 s deadline on CI (`index_busy`)

Date: 2026-10-24. The test `indexer_deployment_public_surface::a_search_matching_more_than_1000_claims_returns_1000_and_logs_the_cut_without_the_query` (AS-53) failed in CI run 40053031297 (b297a35). It got a 503 with `{"error":"index_busy"}` and took 67 s in total.

## Root cause

The store makes one extra query per row. `select_rows` (`crates/adapter-index-store/src/lib.rs:297-331`) calls `references_for` (`:360-390`) once for each row, and each call prepares and runs its own DuckDB statement.

Measured locally in a debug build, a 1001-row `query_by_object` takes about 850 ms:

| Part of the query | Time |
|---|---|
| Base `SELECT … LIMIT 1001` | 3.7 ms |
| 1001 reference lookups, one per row | ~880 ms |
| Same lookups as one batched JOIN | 3.3 ms |

The CI host is about 4x slower, which puts the search right at the query server's 5 s deadline (`adapter-xrpc-query-server/src/lib.rs:91,418-423`). The 1001-row cap from ADR-083 §3 limits how many rows come back, but not how many statements run.

Ruled out:
- **Lock contention.** The pass checkpoints before `trigger` gets its reply.
- **CPU starvation alone.** It makes the slow search slower, but does not cause it.

## Fix (user-approved 2026-10-24: batch query, ratio test)

- Fetch all references in one query, joined to the same `WHERE … ORDER BY cid LIMIT 1001` row set, then group them per claim with a pure function. Order within a claim stays `referenced_cid, ref_type`. A claim with no references gets an empty list.
- In `flat_attributed_rows` (`openlore-indexer/src/search_handler.rs:182-200`), replace the per-row linear scan with a map lookup.
- The 5 s deadline and the 503 `index_busy` behaviour do not change.

## Regression check

A store test that:
- checks the references come back exactly as the per-row path returned them;
- asserts that a 1001-row search takes less than 3 times as long as a 101-row search. Today it is about 10x.

The check uses a ratio of two timings rather than an absolute time, so host load does not make it flaky.
