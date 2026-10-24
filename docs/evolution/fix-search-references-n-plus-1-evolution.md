# fix-search-references-n-plus-1 — evolution

Delivered 2026-10-24 through /nw:bugfix. The RCA is in `docs/feature/fix-search-references-n-plus-1/rca.md`.

## Defect

On CI, AS-53 (a search matching more than 1000 claims) got 503 `index_busy`. The store read each row's references with its own DuckDB statement, so 1001 rows took 1001 extra queries: about 1.2 s locally in a debug build, and around the 5 s deadline on a CI runner about 4x slower.

## Fix (e0ac14c, plus a bound tweak in the finalize commit)

- **Store (`adapter-index-store` `select_rows`):** now runs one references query, joined to the same row set as the row query.
  - Both queries are built from a single `bounded_row_set`, so the `WHERE`/`ORDER BY cid`/`LIMIT` cannot drift apart.
  - A pure function, `attach_references`, groups the references per claim.
  - `references_for` is gone.
- **Search handler (`search_handler::flat_attributed_rows`):** looks rows up in a map instead of scanning every row.
- **Unchanged:** response shapes, the 5 s deadline, the 503 reply and the 1001-row cap.

## Evidence

- **Equivalence test:** checks the results against the old per-row SQL.
- **Proptest:** covers `attach_references`.
- **Scaling ratio** (1001-row time over 101-row time, best of 3 runs each):
  - Before the fix: 9.7x (1.16 s against 119 ms). This was the RED.
  - After the fix: 2.6x (13 ms against 5 ms).
- **Bound widened from 3 to 5** after GREEN. A 2.6x result sat too close to 3 for a shared runner, and 5 still separates the two cases clearly.
- **Test runs:**
  - AS-53 passes, as do all 54 `indexer_deployment_*` tests.
  - Full cli suite: 1192 of 1192 passed with `--no-fail-fast`.
  - In an earlier full run, `indexer_ingest::indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam` failed once, in 0.5 s. It passed when re-run at `-j 2`. It's on the ingest path, not search, and is noted as a possible new flake.
