# ADR-088: Review-App Private Store v2 — Several GitHub Accounts Keyed by Numeric Id, Saved Repo Choices, Imported Repos, a Frozen Scan Plan, and Removal That Deletes Pending Suggestions

- **Status**: Accepted (2026-10-19, approved by the user in chat). Covers D-RRA-D2, D3, D6, D7, D8, D10 and D11
  (`docs/feature/review-app-repos-and-accounts/design/wave-decisions.md`).
- **Date**: 2026-10-19
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey (to approve)
- **Feature**: review-app-repos-and-accounts (DESIGN, scope: application)
- **Companion**: ADR-089 (publish eligibility and BR-RRA-1)
- **Builds on / amends**: ADR-074 (private state; amends "DELETE only in purge/expiry"), ADR-076
  (scan admission, GitHub budget), ADR-085 C-GDL-3 (accounts keyed by numeric id),
  `docs/architecture/bluesky-claim-review-app/data-models.md` §3.1 ("one GitHub link per DID")
- **References**: ADR-007 (functional core), ADR-082 (separate delete port precedent)

## Context

DISCUSS (US-RRA-001..010, D-RRA-1..17) widens the review app's private state:

- up to 3 GitHub accounts per Bluesky sign-in (D-RRA-15), each with its own bio status;
- a saved repo choice per account that survives a GitHub rename (D-RRA-1, D-RRA-6);
- a list of imported public repos (D-RRA-2, D-RRA-8);
- each suggestion's source (account or import), so visibility and removal work per source
  (OQ-5, D-RRA-17);
- one total per-scan repo limit of 10, default ticks included (BR-RRA-3, D-RRA-16);
- a scan that keeps what was in force when it started, through an automatic resume (BR-RRA-4);
- removal that deletes pending suggestions but keeps declined and published key records
  (D-RRA-17).

Today `review-app.duckdb` is schema v1: `github_links` has `owner_did` as its primary key (one
link per DID), `suggestions` has no source column, a resumed scan re-plans from scratch, and
`xtask check-arch` allows `DELETE` only in the purge and expiry modules. The app is live, so the
change must migrate real v1 data. DuckDB cannot change a primary key in place.

## Decision

1. **Schema v2, one forward migration at startup.** `schema_verdict(Some(1))` becomes `Migrate`;
   the app runs `CHECKPOINT`, closes, copies the file to `review-app.v1.<unix-seconds>.bak`
   (at most one such copy exists: a re-run after a crash replaces it rather than adding one),
   reopens, and runs the whole v1 → v2 step in one transaction, ending with
   `INSERT INTO schema_version VALUES (2)`. A crash before commit leaves v1 and the step re-runs.
   **The backup is deleted once the v2 store opens and passes its startup checks** (user decision
   2026-10-19, review R5, "Delete after upgrade"): it is a plaintext copy of every owner's private
   state that "Disconnect and forget me" cannot reach, so none may linger.
   Any other old or newer version is still refused. Expand-only: `github_links` is kept, read by
   the migration and never written again except by purge; a later migration drops it.
   Suggestions of a DID with no `github_links` row keep `source_account_id` NULL and stay hidden,
   as in v1.
2. **`github_accounts`** (new): PK (`owner_did`, `github_user_id`); `github_login_seen`,
   `status` (`verified` | `unverified`), `verified_at`, `last_checked_at`, `last_verdict`,
   `added_at` (account order for default ticks), `eligible_count_seen` (nullable; count of
   non-fork, non-archived owned repos at the last listing). Migrated 1:1 from `github_links`.
   At most 3 rows per DID and "one owner per GitHub account" (no other DID holds a `verified`
   row for the same id) are both checked in the add transaction.
3. **`repo_choices`** (new): PK (`owner_did`, `github_user_id`, `repo_id`); `full_name_seen`,
   `ticked` BOOLEAN, `saved_at`. A saved choice stores every repo shown at Save, ticked or not,
   so a repo with no row is "new since your last choice" (D-RRA-6). An account has a saved choice
   iff it has rows. Repos are keyed by GitHub's numeric repo id, so renames keep the choice.
4. **`imported_repos`** (new): PK (`owner_did`, `repo_id`); `full_name_seen`, `added_at`.
5. **`suggestions`** gains `source_kind` (`account` | `imported`), `source_account_id` (nullable
   BIGINT), `source_repo_id` (nullable BIGINT), `outside_default` (nullable VARCHAR:
   `owned_choice` | `other_account` | `imported`). The migration sets `source_kind = 'account'`
   and `source_account_id` from the DID's `github_links` row for every existing row. There is
   still no `hidden` column: visibility is derived (ADR-089 §5).
6. **Frozen scan plan.** `scan_run_repos` (new): PK (`owner_did`, `run_id`, `position`);
   `repo_id`, `full_name`, `source_kind`, `source_account_id`, `status` (`planned` | `read` |
   `skipped`), `skip_reason`. `scan_runs` gains `summary` (VARCHAR JSON: per-account outcome
   and skipped repos with reasons; no bio, no DID). A scan freezes its intent (each verified
   account's saved ticks or "default", plus the imports) before admission, resolves it to
   concrete repos after each account's bio check and listing, and writes the plan rows before
   the first read. A resumed run carries over the paused run's unread rows instead of
   re-planning. Changes saved meanwhile apply from the next fresh scan (BR-RRA-4).
7. **Removals stop reads at once without a race.** Before each repo read the scan re-reads the
   source's presence. Suggestions are stored with a conditional insert that, in the same
   transaction, inserts only if the source account or import is still listed. A removal
   deletes the list row and that source's `pending` suggestions in one transaction. Under
   DuckDB's single-writer transactions either order ends with no pending rows for a removed
   source (AC-008.4, AC-009.4).
8. **Removal deletes only pending suggestions.** A new adapter module (`removal.rs`) behind a new
   `ScopeRemovalPort` held only by the two remove routes. Removing an account deletes its
   `github_accounts` row, its `repo_choices` rows (re-adding starts fresh, US-RRA-008) and its
   `pending` suggestions; removing an import deletes its `imported_repos` row and its `pending`
   suggestions. Its `DELETE` on `suggestions` names `state = 'pending'` and the source;
   declined, published and retracted rows stay (BR-1, BR-2). `scan_run_repos` rows stay as run
   history; expiry keeps the latest 2 runs per DID; purge removes all.
   The check-arch rule `review_store_owner_scoped_sql` is amended: `DELETE` may live in the
   purge, expiry and removal modules, and a removal `DELETE` on `suggestions` must name
   `state = 'pending'`.
9. **Purge covers every new table.** "Disconnect and forget me" deletes the DID's rows in
   `github_accounts`, `repo_choices`, `imported_repos`, `scan_run_repos` and the kept
   `github_links`, in the existing single transaction. The new tables join `REVIEW_OWNER_TABLES`
   so their SQL must name `owner_did`.
10. **Scope changes are serialized per DID.** Save, import, add/remove account, remove import and
    the BR-RRA-5 move run under an in-process per-DID lock (the app is one container, ADR-075),
    so the pure limit check and the write cannot interleave with another tab's change.

## Alternatives Considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **A. Normalized tables (this decision)** | Each item is an owner-scoped row; the SQL rule, conditional insert and targeted deletes apply directly; one migration. | Chosen |
| **B. One JSON column per DID** (accounts, choices, imports in a blob on `accounts`) | No migration of keys, but no row-level owner rule, no conditional insert against a listed source, every change rewrites the blob, and the check-arch SQL rule cannot see inside it. | Rejected |
| **C. Keep `github_links` (one per DID) and add `extra_github_accounts`** | Smallest migration, but two code paths for "an account", asymmetric removal, and the first account cannot be removed without special cases. | Rejected |
| **Hide on remove, purge only on disconnect** | Was the DISCUSS default; the user chose delete (D-RRA-17). | Rejected (user) |
| **Re-plan a resumed scan from the current choice** | Simple, but breaks BR-RRA-4 and AC-002.2 ("including after an automatic resume"). | Rejected |
| **Keep the plan in memory only** | No table, but a restart (which today interrupts and resumes scans) loses it. | Rejected |
| **Materialize default ticks as a saved choice when an account is verified** | Makes the limit arithmetic exact, but default ticks stop following the ranking, which changes today's behaviour for owners who never choose (US-RRA-002 "keeps today's behaviour"). | Rejected |
| **Limit check by a SQL constraint** | Default ticks depend on live GitHub listings, which SQL cannot see. | Rejected; pure check under the per-DID lock |

## Consequences

- **Positive**: several accounts and a rename-safe choice with no Lexicon change; visibility and
  removal follow the source; a scan reads exactly its frozen plan; removal is race-free; purge
  stays one transaction.
- **Negative**: first schema migration of the live store; a v1 binary refuses a v2 file
  (`TooNew`) and, once the upgrade succeeded, no v1 backup remains (R5): **roll forward only**.
  Until the upgrade succeeds (a start cut off before commit), the one backup and the untouched v1
  file are the rollback point; PDS records are never affected (DEVOPS runbook). A scan paused at
  deploy time has no plan rows and re-plans once on resume. `eligible_count_seen` can be stale;
  it only shapes the limit arithmetic for accounts without a saved choice (see
  architecture-design §6.3), and default ticks never push a scan over the limit.
- **Earned Trust**: the migration runs in CI against a real v1 file written by the v1 code
  (fixture), checks row counts and source backfill, and re-runs idempotently; the store's live
  probe adds a rolled-back conditional insert for an unlisted source (must insert nothing) and a
  rolled-back removal (must delete only `pending`). DuckDB `ALTER TABLE … ADD COLUMN` support for
  each new column is verified by that test, not assumed.
