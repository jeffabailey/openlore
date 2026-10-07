# Evolution: fix-indexer-follow-ups

- **Type**: bugfix delivery (3 steps, one phase), brownfield indexer
- **Dates**: 2026-10-05/06: first fix (`cb7b94b`) through the mutation report (`e441718`)
- **Origin**: the open follow-ups of
  [`indexer-per-did-pds-fetch-evolution.md`](indexer-per-did-pds-fetch-evolution.md).
- **Records**: `docs/feature/fix-indexer-follow-ups/` (`rca.md` with the root causes and user
  decisions, `deliver/roadmap.json`, `deliver/execution-log.json`,
  `deliver/mutation/mutation-report.md`).
- **State**: code complete. No new crates, no schema change. ADR-078 §4 and data-models §2
  amended.

## Defects, fixes and regression tests

Order D2 → D3 → D1. Each step began with a regression test that failed against the code at the time.

| Defect | Root cause | Regression tests (RED evidence) | Fix | Commit |
|---|---|---|---|---|
| **D2**: `OPENLORE_INDEXER_PLC_ENDPOINT` not validated at startup | `config.rs` read it raw. A policy-refused URL became `Unavailable` at runtime, which is fallback-eligible. With a fallback set, every did:plc author was read silently as relay origin, and self-attested claims were refused with exit 0. The startup checks went variable by variable, and the PLC endpoint was still listed as "unchanged" even though it now sat behind the guard. | `a_malformed_or_unsafe_plc_endpoint_is_explained_at_startup` (`http://plc.example`, `https://10.0.0.1`, `https://u:p@plc.directory`; RED: startup was accepted). A PLC dimension in `config_parsing_is_total…` and `a_plc_endpoint_is_accepted_iff_the_policy_admits_it`. | A `plc_endpoint(url, policy)` validator that shares the admissibility predicate with `fallback_url`, so startup and runtime can't disagree. A hostname that resolves only to private addresses is still refused at runtime (IPF-08). | `cb7b94b` |
| D2 follow-up | A set-but-blank value | `a_blank_plc_endpoint_is_refused_at_startup` | Set-but-blank is refused, naming the variable. Unset still means `https://plc.directory`. | `cba7b78` |
| **D3**: a store write failure mid-pass | Each `upsert` was an artifact write, 3 DELETEs and then INSERTs, all autocommitted. **The duplicate-reference hypothesis was confirmed.** `decode_references` kept duplicates, so a record that listed one reference twice broke the references PK after the claim row was committed. That left a half-written claim and exit 2 on every pass: a remote-data fault that looked like a local one. | `upsert_with_duplicate_references_keeps_the_claim_whole` (proptest, state-delta per CID; RED: PK violation, claim half-written). `indexed_references_have_no_duplicates_and_lose_none`. The guard `a_store_failure_for_one_author_keeps_earlier_authors_claims_searchable` (ADR-078 §2; it passed before the fix, by design). | Duplicates are removed at storage time with the pure `distinct_references` (first occurrence wins). CIDs are unchanged because the signed record is untouched. Each upsert is one DuckDB transaction. The claim row is `ON CONFLICT (cid) DO UPDATE` on non-indexed columns only, because DuckDB's FK check still sees children deleted earlier in the same transaction, and it rewrites an update of an indexed column as a delete plus an insert. The columns left out (`author_did`, `subject`, `object`, `composed_at`) are fixed by the CID, so this is safe. | `cba7b78` |
| **D1**: after a resolution timeout, the fallback ran on an exhausted deadline | `fetch_repo` made one per-DID deadline and reused it for the fallback. **The design specified this behaviour**: ADR-078 §4 said "the fallback gets the remaining budget". That broke ADR-077 NFR-3. It showed only with `PER_DID_TIMEOUT_SECS` ≤ 10 (`LOOKUP_TIMEOUT` is 10 s). | `a_did_document_that_never_arrives_leaves_the_fallback_a_full_budget` (RED: skipped `pds_timeout`). Bounded-time guard: `a_hanging_fallback_after_a_hanging_directory_is_still_abandoned`. `a_resolved_did_is_listed_on_its_own_pds` now asserts the budget. | `ListingSource::budget() -> ListingBudget`, decided in the pure core: `OwnPds` takes the remainder of the shared deadline and `Fallback` gets a fresh one. `fetch_repo` only builds the deadline. Worst case per DID is now 2× the budget. ADR-078 §4 and data-models §2 amended. | `df077df` |

Each step also has a `chore: execution log` commit (`a3a43a7`, `a5c07ee`, `d72ec89`).
The blank-endpoint decision is recorded in `52d0356`.

## User decisions (2026-10-05)

- D1: the fallback gets its own fresh per-DID budget, and the doubled worst-case pass time is accepted.
- D3: references are deduplicated before indexing, and each upsert runs in one transaction.
- D2: a set-but-blank PLC endpoint is refused at startup. Unset means the default. `peer_resolve.rs` is unchanged.
- One delivery, D2 → D3 → D1, each step starting with a failing test, auto-advancing.

## Quality gates

| Gate | Result |
|---|---|
| DES integrity | All 3 steps have complete DES traces |
| Adversarial review | APPROVED, 0 defects. The orchestrator separately checked the D3 `ON CONFLICT` column set. |
| Mutation (in-diff vs `0cfdd8e`, gate 80%) | `claim-domain` 100%, `openlore-indexer` config.rs 100%, `adapter-index-store` (advisory) 100%. `appview-domain`: 1 mutant, unviable (gate vacuous; behaviour pinned by property). No survivors. |

## Lessons

- **The design itself specified D1.** "Remaining budget" read well in ADR-078 but contradicted
  ADR-077 NFR-3. **Lesson:** check a timeout rule against the outage NFR it serves.
- **DISTILL judged D3 untestable, but it wasn't.** A regular file placed at an author's index
  directory injects a store failure hermetically through the binary. **Lesson:** look for a
  filesystem seam before declaring a failure path untestable.
- **The mutation scope left out `run.rs`.** D1's shell change (building the deadline) was
  covered only by acceptance tests. **Lesson:** list shell files explicitly when the scope is
  "the core".
- **Reviewers sometimes skip specific questions.** The adversarial review did not answer the
  `ON CONFLICT` column question, so the orchestrator checked it. **Lesson:** verify named
  questions yourself, not only the verdict.

## Remaining follow-ups (not fixed here)

- **Nothing schedules `openlore-indexer ingest`.** Whoever schedules it must treat exit 3 as
  "retry, alert on repeats" (see the indexer-per-did-pds-fetch operator notes).
  *Resolved by indexer-deployment (`3b80ae5`, `1c30474`): a 15-minute host timer runs `trigger` in the
  long-running `serve`; alarms on 2× exit 3, any exit 2 and liveness. See
  [`indexer-deployment-evolution.md`](indexer-deployment-evolution.md). Not yet deployed.*
- **The guarded DNS resolver is duplicated** in two adapters (about 20 lines). Sharing it needs a new crate.
