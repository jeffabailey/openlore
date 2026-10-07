# DESIGN wave decisions: indexer-deployment

- **Mode**: Propose, run autonomously (subagent). The drivers came from the caller and were not
  re-asked: solo maintainer, time-to-market, low ops cost, hard fault isolation, no regression to the
  PDS or review app, functional Rust (ADR-007).
- **Style**: unchanged. Hexagonal ports-and-adapters within the modular monolith. The indexer is the
  second composition root. **No new crates, no schema migration, no lexicon change.**
- **ADRs**: ADR-080 (serve owns the store, triggered in-process passes), ADR-081 (DID file per pass,
  host keeps the last good copy), ADR-082 (purge removed authors), ADR-083 (public surface and bounds).
  **ADR-078 amended** (skips versus removals, exit-2 summaries, new events, trigger exit 4).

## Decisions

| ID | Decision | Rationale | ADR |
|---|---|---|---|
| DD-IXD-1 | One long-running `serve` container owns the only store handle. The host timer runs `openlore-indexer trigger` (via `docker exec`), which runs one pass in-process over a Unix socket and exits with the pass's 0/2/3 (busy → 0, unreachable → 4). | DuckDB is single-process, so any two-process design collides on 100% of passes. One process has the smallest memory and keeps the exit codes. It keeps the user's host timer. | 080 |
| DD-IXD-2 | A pass holds the store for at most one transaction at a time and never monopolizes the query executor | ≤ 1 s search during a pass, by construction | 080 |
| DD-IXD-3 | Every pass ends with exactly one `pass_summary` carrying `exit_code` and `pass_id`, including exit-2 endings | Alarms key on logs, not on a host-side process exit | 080, 078 am. |
| DD-IXD-4 | `OPENLORE_INDEXER_REPO_DIDS_FILE` is read and validated each pass. A bad list refuses the pass, never `serve`. | Edits apply with no restart. Search survives a typo. | 081 |
| DD-IXD-5 | The host renders SSM (a Standard String) into a read-only **directory** mount, keeping the last good copy on a read failure and never validating | Availability belongs to the host, validity to the binary. A file mount would pin the old inode. | 081 |
| DD-IXD-6 | Purge = pure set difference `indexed authors − non-empty loaded list`, opt-in flag, separate `IndexPurgePort` held only by the runner, DELETE only in `purge.rs`, resumable, no mass-purge threshold | The user's PURGE decision. Skips never delete, by construction. The index is a cache. | 082 |
| DD-IXD-7 | Public routes are only search and `GET /healthz`, at Caddy (stock) and in the binary. Body 8 KiB, value 512 B, 1000 rows, header timeout, connection cap. No per-IP rate limit in v1. | Proportionate for under 50 users. Caddy is module-owned. | 083 |
| DD-IXD-8 | Freshness: operator view from CloudWatch Logs (`deploy.sh status`). Public `last_successful_pass_at` in `/healthz`. Not stored in the index. | Survives restarts and index loss. No schema change. | 083 |
| DD-IXD-9 | Read/write port split: the search handler holds `IndexReadPort` only | Principle 12. The FR-IXD-2 "no request can change the index" becomes structural. | 082/083 |
| DD-IXD-10 | Stay on t4g.micro (user). Indexer container 128m (no swap), `oom_score_adj` 900. DuckDB 48 MB with 1 thread in the indexer **and the review app (B11, in scope)**. Review app `mem_limit` proposed at 192m. Hard gate at MemAvailable > 128 MB, measured. t4g.small only by operator decision (an unplanned PDS outage). | Budget arithmetic (architecture-design §3): expected about 250 MB available, pessimistic about 150, cap-saturated about 50 (residual risk; the PDS is protected by OOM order) | §3, §9 |
| DD-IXD-12 | Robustness: `catch_unwind` per pass. An unusable or poisoned store makes `serve` exit 2. Search returns 500 and `/healthz` returns 503 on store faults. 25-minute pass deadline. Per-pass author-key cache. Gate as fetched (≤ cap + 1 listings). Explicit end-of-pass checkpoint. Pass on a dedicated OS thread. `trigger` dispatched before config and store. `init: true`. | Review findings H2, M1-M4 and M6 (2026-10-06) | 080, 083 |
| DD-IXD-13 | The DID render renames a **file**, never swaps the directory. A stale list (> 2 h) feeds A3 with no new alarm. `repo_dids_age_secs` in `config.loaded`. | Review findings H1 and M5 | 081 |
| DD-IXD-14 | Purge deletes artifacts by each row's `signed_record_path`, never by a recomputed partition (`did_to_fs_segment` is not injective) | Review finding (Low) | 082 |
| DD-IXD-11 | No module change and no second replacement. Rides v1.7.0 and R-REPLACE, in either order relative to the review app. | DEP-IXD-1 | §9 |

## Assumptions (documented, not asked)

- A1: The worst-case pass at 12-50 DIDs (about 13 min) stays under the 15-minute interval. If not, slots
  coalesce, which is acceptable.
- A2: The DuckDB `memory_limit` (48 MB) and `threads` (1) settings are honored inside a 128m cgroup.
  They are read back by the probe and verified by the re-measure gate, not assumed.
- A3: Realistic search results stay well under 1000 rows at under 50 authors.
- A4: `docker exec` of the `trigger` client inside the capped container adds negligible memory (one
  small process, no store).

## Handoffs

- **DISCUSS (scope amendment)**: C-4 ("binary unchanged") and I-IXD-2 ("deploys the binary without
  changing it") must be reworded. The pure verify-before-index gate, anti-merging and provenance
  verdict stay unchanged. The binary changes B1-B15 are in scope, including B11 in the review app (user
  decision). US-IXD-006 drops "t4g.small adopted" as a routine gate outcome. It becomes an operator
  decision with an unplanned PDS outage.
- **DISCUSS/DISTILL (AC change; wording accepted by the user)**: AC-003.2 and US-IXD-003 Example 2 and
  its scenario are superseded. Use the AC-003.2 text in ADR-082 Consequences: the purge happens at the
  start of the next pass that loads the list, whatever its exit code, including 3. A refused or empty
  list purges nothing. A listed-but-skipped DID keeps its claims. Add ACs for:
  - an empty list never purging;
  - a malformed or unreadable list never purging;
  - a purge failure giving exit 2 and an alarm;
  - every pass emitting one `pass_summary`, including exit 2;
  - `trigger` coalescing (busy → 0, no second pass);
  - `/healthz` showing `last_successful_pass_at`, and 503 on an unusable store;
  - search returning 500, not an empty 200, on a store error;
  - two consecutive DID edits picked up with no restart;
  - a panicking pass not leaving the runner busy;
  - the pass deadline giving exit 2;
  - a search burst over the end-of-pass checkpoint staying ≤ 1 s.

  The journey (S3, "openlore-indexer ingest") should read "openlore-indexer trigger (pass runs inside
  serve)". The user-visible behavior is unchanged.
- **DEVOPS**: the contracts in architecture-design §9 (image, compose, timer, render script, Caddy site,
  SSM, 3 alarm conditions, status command, deploy and rollback, re-measure gate, sequencing).
  Flag R-IXD-D4: the review-app DuckDB caps are not implemented in code.
- **DELIVER**: binary changes B1–B15 (architecture-design §8), check-arch rules (§11), and the Earned
  Trust scenarios in ADR-080/081/082.
- **Development paradigm**: functional. Use `@nw-functional-software-crafter`.

## Not decided here

- The control-channel framing (crafter).
- The exact alarm math for A1 and A3 (DEVOPS).
- Final memory caps (measured by the gate).
