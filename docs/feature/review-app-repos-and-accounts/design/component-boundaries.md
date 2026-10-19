# Component Boundaries: review-app-repos-and-accounts

Zero new crates. Every change extends an existing crate. Contract shapes per component
(pure-function, bounded-change, unbounded-preservation) follow ADR-007 and the nWave effect
isolation rule. Interface names are indicative; the crafter owns the exact signatures.

## 1. Reuse analysis

| Need | Existing component | Decision | Contract shape | Universe / assertion |
|---|---|---|---|---|
| Rank owned repos, default top 5 | `scraper_domain::select_person_repos` | Reuse unchanged | pure-function | Property: default ticks = first `min(5, room)` of its ranking |
| Bio proof | `review_domain::ownership::prove_ownership` (delegates to `claim_domain::identity_link`) | Reuse unchanged, per account | pure-function | Existing proptests stay green |
| Unfinished vs failed check | `OwnershipRefusal::disproves_ownership` | Reuse | pure-function | Existing |
| Scan admission | `review_domain::budget::{admit_scan, github_budget_allows, auto_resume_wait}` | Reuse unchanged (one admission per scan) | pure-function | Existing budget properties |
| Harvest → candidates → reconcile | `harvest_repo`, `derive_candidates`, `reconcile` | Reuse per planned repo | pure / effect | `reconcile` never re-offers a declined key (BR-1) |
| Publish gate | `claim_domain::authority` (ADR-086, accepted, not built) | **Build owner tier now** (ADR-089) | pure-function | Rule-table property over generated facts |
| Publish plan | `review_domain::plans` | Extend: constructors take `&SigningPermit` | pure-function | Plan subject == permit subject, else `NotPermitted` |
| Repo-owner read | none (`read_repo_authority` planned by OOS) | **New `GithubPort::read_repo`**; OOS builds on it | effect (read-only) | Fixture contract incl. 301 and 404 |
| Private store | `adapter-review-store` | Extend to v2 (ADR-088) | bounded-change | Declared mutation sets below |
| KPI counters | `review_domain::kpi::KpiEvent` closed enum + `kpi_counters` | Extend the enum | pure + bounded-change | check-arch no-PII rule |

No existing component chooses repos, holds several accounts, or imports repos, so `repo_scope`
and the new store tables are justified.

## 2. Pure core

### `claim-domain::authority` (NEW module, owner tier)

- **Owns**: the rule "a DID may publish an assertion about `github:<owner>/<repo>`".
- **In**: signer DID, subject, the signer's listed GitHub ids, fresh `RepoFacts`, fresh profile of
  the repo owner. **Out**: `SigningPermit` (private fields, no serde) or `SigningRefusal`
  (`NotOwnedByListedAccount`, `OrgOwned`, `RepoGone`, `BioNoLongerHolds(id)`, `Unfinished`).
- **Shape**: pure-function. **Never**: reads a clock, a store or the network.
- **Later**: owner-only-signing adds `kind`, `requirement`, `classify`, the repo-listed tier and
  local checks to the same module.

### `review-domain::repo_scope` (NEW module)

- **Owns**: default ticks; the limit arithmetic (Save, import, scan start); "new since your last
  choice"; skipped-repo reasons; scan intent and per-account plan; BR-RRA-5 moves; pending
  visibility; the `outside_default` tag.
- **Shape**: pure-function throughout. Inputs are values (account rows, choices, listings,
  imports, cap); outputs are values (`ScanIntent`, `AccountPlan`, `Move`, `Visible`,
  `LimitRefusal { limit, total, excess }`).
- **Layout** (DELIVER refactor): one facade, private submodules by concern — `limits` (account
  and repo limits; the one `S` and the one room-left rule every admission shares), `choose`,
  `plan`, `imports`, `moves`, `resume`, `visibility`.
- **Never**: touches a port.

### `review-domain::plans` (EXTEND)

- `publish_plan`, `publish_edited_plan`, `restore_plan` take `&SigningPermit` (replaces
  `owner_did`). Retract, share and link plans unchanged.

### `review-domain::views` (EXTEND)

- Account list (status, add, remove with confirm naming the pending count), choose-repos page
  (per account fragment, ticks, new marks, skipped counts, limit line), "Repos you don't own"
  list, imported card ("Not yours to publish", who can publish, Decline only), per-card account
  line, per-account scan summary, refusal messages. Every message names the cause and next step.
- `DECLINE_LABEL` stays "Not me" on owned cards; imported cards use "Decline" (same action).

### `review-domain::kpi` (EXTEND)

- New `KpiEvent` variants (wave-decisions D-RRA-D11). Closed catalogue; no request data in names.

## 3. Ports (`crates/ports`, EXTEND)

| Port | Methods (indicative) | Held by | Shape |
|---|---|---|---|
| `GithubPort` | + `read_repo(owner, name) → RepoFacts`; `OwnedRepo` + `repo_id`, `owner_id` | scan, publish routes, import route | read-only |
| `GithubAccountsRead` | `list_accounts(owner) → Vec<LinkedAccount>` in `added_at` order | all routes, scan | read-only |
| `GithubAccountsWrite` | `add_or_reverify(owner, id, login, max, tally) → Added \| Reverified \| AlreadyListed \| HeldByAnotherOwner \| AtLimit` | `routes/github.rs` only | bounded-change: `github_accounts` rows of `owner` |
| `AccountStatusWrite` (narrow) | `mark_unverified(owner, id, verdict)` | scan, publish routes, `routes/github.rs` | bounded-change: `status`, `last_checked_at`, `last_verdict` of one row |
| `AccountListingNote` (narrow) | `record_eligible_count(owner, id, n)` | scan, `routes/repos.rs` | bounded-change: `eligible_count_seen` of one row |
| `RepoChoiceRead` / `RepoChoiceWrite` | `choices(owner)`; `save_choice(owner, id, rows, tally)` (replaces that account's rows) | `routes/repos.rs`, scan | bounded-change: `repo_choices` rows of (`owner`, `id`) |
| `ImportedReposRead` / `ImportedReposWrite` | `imports(owner)`; `add_import(owner, repo, tally)` | `routes/imports.rs`, scan | bounded-change: `imported_repos` rows of `owner` |
| `ScopeMovePort` | `move_import_to_account(owner, repo_id, account_id, choice_if_absent)` | scan, `routes/github.rs` (verify) | bounded-change: one import row, that account's choice rows, that repo's suggestion source |
| `ScopeRemovalPort` | `remove_account(owner, id) → deleted_pending`; `remove_import(owner, repo_id) → deleted_pending` | remove routes only | bounded-change: account = its `github_accounts` row + its `repo_choices` rows + its `pending` suggestions (re-adding starts fresh, US-RRA-008); import = its `imported_repos` row + its `pending` suggestions. Declined, published, retracted rows and `scan_run_repos` history are kept |
| `ScanRunPort` (EXTEND, write) | + `record_plan(owner, run, rows)`, `mark_repo(owner, run, pos, status)`, `finish_scan(…, summary)` | scan | bounded-change: that run's rows |
| `ScanPlanRead` (new, read) | `unread_plan(owner, run)`, `summary(owner, run)` | scan, queue | read-only |
| `ReviewStateRead` (EXTEND) | `pending_suggestions` returns source and `outside_default` | queue, publish | read-only |
| `ReviewStateWrite` (EXTEND) | `add_pending` becomes `add_pending_from_listed_source` (insert only if the source is listed, same transaction) | scan | bounded-change: new `pending` rows of `owner` |
| `ForgetPort` (EXTEND) | purge covers new tables | settings route | bounded-change: all rows of `owner` |
| `GithubLinkPort` | retired from app code (kept for the migration read) | migration only | — |

Read and write are split. No read-only holder gets a write or delete method. The removal port is
held only by the two remove routes (ADR-082 precedent).

## 4. Adapters

### `adapter-github` (EXTEND)

- `read_repo`: `GET /repos/{o}/{r}`, at most one same-host redirect; facts from the final body.
- Listing rows map `id` and `owner.id`. The listing follows `Link: rel="next"` up to 3 pages
  (300 repos) and reports whether it was complete.
- Contract tests (required CI acceptance-stage gate): recorded interactions for `read_repo`
  (200 user, 200 org, 301 rename, 301 transfer, 404, 403 rate limit) and the paged listing.
- Host allowlist and public-data-only rule unchanged. Probe: unchanged plus `read_repo` shape on
  a known public repo in the live probe.

### `adapter-review-store` (EXTEND)

- `schema.rs`: v2 DDL, `SchemaVerdict::Migrate`, migration in one transaction after a file copy.
- New modules: `github_accounts.rs`, `repo_choices.rs`, `imports.rs`, `removal.rs` (the only
  new place with `DELETE`), plan rows in `scan_runs.rs`, conditional insert in `suggestions.rs`,
  purge extended in `purge.rs`.
- Probe: live, rolled back — conditional insert for an unlisted source inserts nothing; removal
  deletes only `pending`.

### `adapter-atproto-oauth`

Unchanged. `create_claim_record` is still reached only from `executor.rs`.

## 5. Driver: `openlore-review-app` (EXTEND)

| Component | Change |
|---|---|
| `routes/github.rs` | Account list; add (≤ 3, refusals named); verify per account; remove with confirm; BR-RRA-5 move after a verify (one listing). The profile (and the move's listings) are read before the scope lock; under it the store is re-read, the admission decided and the rows written |
| `routes/repos.rs` (new) | `GET` choose page (per-account fragments, live listing); `POST` Save: each verified account listed once per request, then under the scope lock the account is looked up again and the limit checked against the store; the page re-renders from the same listings |
| `routes/imports.rs` (new) | `POST` add (shape → `read_repo` → classify → limit → insert) and remove. `read_repo` and the listings run before the scope lock; classify, limit and insert run under it |
| `routes/review.rs` | Visibility from `repo_scope`; `permit_for` replaces `owner_can_publish` for previews; decline/undo need visibility only |
| `routes/publish.rs` | `permit_for` at confirm, confirm-selected and retry; mark unverified on `BioNoLongerHolds` |
| `scan.rs` | Intent before admission; per-account bio check; plan; read loop with listed-source check; resume of the frozen plan |
| `wiring.rs` | New ports; scope lock (per-DID async mutex map, held at most 10 s, then the change is refused with "try again"); per-DID page-read allowance (`review_domain::budget::admit_page_reads`); new log events incl. `ownership.publish_check.passed` on the log-field allowlist |
| `config.rs` | No new setting: `SCAN_REPO_CAP = 10` is a constant (D-RRA-16) |

**Review R2/R3 (2026-10-19): no GitHub request while a scope lock is held.**
The first design held the lock across the GitHub reads, so one slow GitHub
answer (the client had only a connect timeout) held every other change of
that DID, and a Save looked its account up before the lock (R1). Now each
GitHub request is bounded as a whole (10 s, `adapter_github::client::REQUEST_TIMEOUT`),
GitHub is read first, and the lock covers only the store re-check and the
write: the limit counts the store as it is under the lock, so it holds
whatever another tab did meanwhile. The store's Save also writes only while
the account row exists, in the same transaction. Page actions' GitHub
requests are admitted against a per-DID hourly allowance, and each verified
account is listed at most once per request.

## 6. Enforcement (`xtask check-arch`)

| Rule | Layer | What it stops |
|---|---|---|
| `review_publish_plan_needs_permit` (new) | structural | Building or restoring a claim plan, or calling `create_claim_record`, outside the permit path |
| `authority_evidence_only_from_adapters` (new, ADR-086 name) | structural | Fabricating `RepoFacts` / `GithubAccountProfile` in app or domain code (stored status as proof). One allowlist shared with owner-only-signing: construction only in `adapter-github`, `adapter-duckdb` (OOS local checks; the review app cannot link it) and `test-support`/`#[cfg(test)]`; `ports` may define, never construct |
| `review_accounts_write_only_in_github_route` (new) | structural | `GithubAccountsWrite` held outside `routes/github.rs` (publish and scan get only `AccountStatusWrite`) |
| Existing: `identity_link_compose_only_via_proven_link`, `link_preview_proves_from_a_fresh_read` | structural | Identity-link PDS writes (`PlanKind::IdentityLink`) stay gated by a fresh `ProvenLink`; out of the claim-permit rule's scope on purpose (a `sameAs` link is a non-weight kind, ADR-086 §2) |
| `signing_permit_not_serializable` (new) | structural | Carrying a permit across requests |
| `review_store_owner_scoped_sql` (amended) | structural | New tables unscoped; `DELETE` outside purge/expiry/removal; removal `DELETE` on `suggestions` without `state = 'pending'` |
| Existing: capability boundary, create-only write, no signing identity, log-field allowlist, link preview fresh read | structural | Unchanged |
| `SigningPermit` private constructor | type | Any permit not from `may_sign` |

## 7. What stays out

- No Lexicon, record or OAuth-scope change. No PDS write without preview and confirm.
- No GitHub sign-in, no private-repo access, no write to GitHub, no contact with import owners.
- No change to the CLI, the indexer or the viewer.
- R4 administered repos: no code, no table.
