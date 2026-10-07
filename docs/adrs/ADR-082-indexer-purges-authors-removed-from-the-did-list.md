# ADR-082: The Indexer Purges Authors the Operator Removed From the DID List; Skips Still Never Delete

- **Status**: Accepted (2026-10-07) — implemented, not yet deployed; see `docs/evolution/indexer-deployment-evolution.md` (proposed 2026-10-06)
- **Date**: 2026-10-06
- **Deciders**: Jeff Bailey (user decision 2026-10-06, OQ-IXD-9: "PURGE"), Morgan (nw-solution-architect)
- **Feature**: indexer-deployment (DESIGN)
- **Amends**: ADR-078 §1 (amendment recorded there), and the ADR-024/025 assumption that the index
  store has no delete path
- **Builds on**: ADR-023 (the index is a re-buildable cache), ADR-081 (DID list per pass)

## Context

The user decided that when an author is removed from the DID list, their claims leave the index on the
next pass. ADR-078 §1 says "a skip persists nothing… `IndexStorePort` has no delete, so skipping can never
remove indexed rows". Both must hold:

- A **skip** is a temporary failure, such as a down PDS, a PLC outage or a timeout. It keeps the
  author's claims.
- A **removal** is an operator decision. It purges them.

The danger is purging on a bad input. An unreadable, missing, malformed or empty list must never purge,
because otherwise an SSM blip or a typo would empty the public index.

Facts:

- A repo's indexed claims are authored by that repo's DID. `records_of` drops foreign records, and the
  stored `author_did` is the bare DID or `bare#fragment`. So the bare author DID identifies the
  configured DID it came from.
- Rows live in `indexed_claims` plus two child tables. Artifacts live under
  `indexed_claims/<did_to_fs_segment(author_did)>/` (one partition per stored `author_did` form).
- A code comment in `replace_claim_rows` records a DuckDB substrate quirk: the foreign-key check still
  sees child rows deleted earlier in the same transaction.

## Decision

1. **Opt-in.** Purge runs only when `OPENLORE_INDEXER_PURGE_UNLISTED=1`, which production sets. When it
   is unset, ADR-078 behavior is unchanged: removals and skips both keep rows. So running `ingest`
   locally with a subset list never deletes anything by surprise.
2. **Removed is a pure set difference.**
   `plan_purge(list: ListRead, indexed_authors: set of bare DIDs) -> PurgePlan` is a pure function in
   `appview-domain`:
   - `ListRead::Loaded(dids)`, with `dids` **non-empty** → `Purge(bare(indexed_authors) − dids)`, which may
     be empty.
   - `ListRead::Loaded(empty)` → `Suppressed(EmptyList)`.
   - A refused list (malformed, missing, unreadable; ADR-081) never reaches `plan_purge`, because the pass
     is refused before the plan.

   A DID that is **in** the list is never in the purge set, whatever its fetch outcome this pass. That
   is how skips stay non-destructive, and it holds by construction rather than by a check.
3. **When.** Each pass purges after the list loads and **before** the fetch phase. Purged DIDs are not in
   the list, so they are not fetched again.
4. **Port and capability.** A new driven port, **`IndexPurgePort`**, separate from the read and upsert
   surfaces (Effect Isolation, principle 12):
   - `indexed_authors() -> set of bare author DIDs` (read-only)
   - `purge_author(bare_did) -> PurgeReport { claims_removed }`

   Only the pass runner is given this port. The search handler, the health handler and the control
   channel never hold it (check-arch rule `index_purge_only_in_pass_runner`). The adapter implements
   it in `crates/adapter-index-store/src/purge.rs`, the only file in that crate allowed to DELETE from
   `indexed_claims` (check-arch rule `index_store_delete_only_in_purge`).
5. **What one author's purge removes (the bounded-change contract).** Every `indexed_claims` row whose
   bare `author_did` equals the target (`= bare` or `starts_with(bare || '#')`, the ADR-079 contributor
   match), their `indexed_claim_evidence` and `indexed_claim_references` rows (by the purged CIDs as
   `cid` / `referencing_cid`), and **each purged row's own artifact file, located by its stored
   `signed_record_path`**.

   Artifacts are never located by recomputing a partition directory. `did_to_fs_segment` (`:`
   becomes `_`) is not injective: for example `did:web:a_b` and `did:web:a:b` share a segment, so
   deleting a recomputed partition could delete a listed author's artifacts. A partition directory
   is removed only when it is left empty.

   **Nothing else changes.** Other authors' rows, including their references **to** a purged CID, stay.
6. **Crash-safe and resumable rather than atomic.**
   1. Read the author's rows (CIDs and `signed_record_path`s).
   2. Delete those artifact files, treating "already gone" as success.
   3. Delete the child rows.
   4. Delete the parent rows.

   The author remains in `indexed_authors()` until the parent rows are gone, so a crash or kill at
   any point leaves a state from which the next pass finishes the purge (idempotent). The adapter
   integration test against real DuckDB decides whether children and parents can share one
   transaction given the FK quirk, or need two.

   **If they need two, there is a short window**, between the child commit and the parent commit,
   in which a search can return the removed author's claims without their evidence and references.
   That is accepted: the claims are about to disappear, and the window is one transaction long,
   or until the next pass after a crash.
7. **Failure.** A purge store failure is a local fault. The pass ends with `exit_code: 2` (alarm), no
   fetch runs, and claims already purged stay purged.
8. **Observability (no claim content, WD-105).**
   - `indexer.ingest.author_purged {pass_id, did, claims_removed}`, one per purged author.
   - `indexer.ingest.purge_suppressed {pass_id, reason: "empty_list"}`.
   - `pass_summary` gains `purged_authors`.
9. **No mass-purge threshold.** The index is a cache of public records that still live on authors'
   PDSes (ADR-023). A wrong list that purges authors is repaired by restoring the list, and the next
   pass re-ingests them within 15 minutes. Every purge is logged per DID. A threshold would add a state
   machine (confirm or override) to protect data that cannot be lost.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **Runbook: delete `index.duckdb` and let passes rebuild (OQ-IXD-9 as asked)** | No code change. But it is manual and empties search for every author until the next pass. The user chose automatic purge. | Rejected (user decision) |
| **Soft delete (a `hidden` column filtered at query time)** | It needs a schema migration (v3), which breaks no-restore rollback (ADR-080). It also keeps a removed author's content on disk, and every query predicate grows. | Rejected |
| **Purge any author not listed by this pass's fetch** | It conflates skips with removals, so one PDS outage would delete an author. This is exactly what ADR-078 forbids. | Rejected |
| **Purge at the end of the pass** | It delays removal by one pass duration, and a refused or failed pass would skip it unpredictably. At the start the plan depends only on the list. | Rejected |
| **Delete through `IndexStorePort` (a `delete` method beside `upsert`)** | It hands delete capability to everything that holds the store port, including code near the search handler. A separate port keeps the capability narrow and checkable. | Rejected |
| **Mass-purge guard (refuse a purge of more than N% of authors)** | See point 9. Added complexity protects only a re-buildable cache. | Rejected |

## Consequences

- **Positive**:
  - An operator removal is honored within one pass, which serves the privacy expectation of an author
    who asked to be removed.
  - Skips remain non-destructive by construction (set difference over the list).
  - No schema change.
- **Negative**:
  - The index store gains its first delete path. It is confined by two check-arch rules and a
    bounded-change property test.
  - A list replaced by mistake purges the authors it drops until the list is fixed (the next pass
    restores them).
- **AC impact (handoff to DISTILL/DISCUSS; wording accepted by the user 2026-10-06)**: AC-003.2 ("a
  removed DID's indexed claims remain") is superseded. New text:

  > "A DID removed from the list is not listed by the next pass that loads the list. That pass purges
  > its claims at its start, before fetching, whatever the pass's exit code, including 3 (every listed
  > DID skipped). From then on, the claims are not searchable. A pass refused for its list (exit 2,
  > malformed or unreadable) purges nothing. An empty list purges nothing. A DID that is still listed
  > keeps its claims even when its fetch is skipped. A purge store failure ends the pass with exit 2,
  > and the next pass completes the purge."

## Earned Trust (required scenarios)

| Scenario | Required behavior |
|---|---|
| Removed DID with app-signed (`#fragment`) and self-attested (bare) rows | Both forms, their children and their artifact files (by `signed_record_path`) are gone. Other authors are byte-identical (property test over the store universe). |
| Another author's claim references a purged CID | That reference row stays, and the other author is untouched. |
| DID listed but its PDS is down (skip) | Nothing purged. Its claims stay searchable. |
| Empty list | `purge_suppressed`, and nothing deleted. |
| Malformed or unreadable list | The pass is refused (exit 2) before any purge. |
| Kill during a purge, then the next pass | The purge completes. There are no orphan child rows and no orphan artifact files for that author. |
| Segment collision: `did:web:a:b` removed, `did:web:a_b` listed (same `did_to_fs_segment`) | Only the removed author's files (by `signed_record_path`) are deleted. The listed author's artifacts remain. |
| A removed DID while every listed DID is skipped (an exit-3 pass) | The purge still happens at pass start. The pass exits 3. |
| DID prefix collision (`did:plc:priya` removed, `did:plc:priya_x` listed) | Only exact bare-DID matches are purged (`starts_with(bare || '#')`, never `LIKE`). |
