# Wave Decisions: review-app-repos-and-accounts (DESIGN)

- **Mode**: subagent, propose. Scope: APPLICATION (chosen by the user). **Architect**: Morgan
  (nw-solution-architect). **Date**: 2026-10-19. **Status**: ACCEPTED by the user in chat, 2026-10-19 (see "User decisions").
- User decisions from DISCUSS are final and kept: D-RRA-12 (R4 deferred), D-RRA-15 (3
  accounts), D-RRA-16 (10 repos incl. default ticks), D-RRA-17 (remove deletes pending).

## Prior-wave reading

| | Artifact | Used for |
|---|---|---|
| ✓ | `docs/product/architecture/brief.md` | Style, crate inventory, review-app, GDL and OOS entries |
| ✓ | `docs/adrs/` (latest ADR-087) | Next free numbers ADR-088, ADR-089 |
| ⊘ | `docs/product/architecture/adr-*.md` | None there; ADRs live in `docs/adrs/` |
| ✓ | `docs/product/journeys/review-and-publish-my-claims.yaml` | Steps 3-6, 9-12 |
| ✓ | `discuss/` wave-decisions, user-stories, story-map, outcome-kpis | Requirements |
| ⊘ | `discuss/journey-choose-repos-and-accounts.yaml` | Not read in full; stories and SSOT journey cover it |
| ⊘ | `docs/feature/bluesky-claim-review-app/design/` | Moved; read `docs/architecture/bluesky-claim-review-app/` (data-models) instead |
| ✓ | owner-only-signing `design/wave-decisions.md`, ADR-086, ADR-087 | Shared gate, OOS-D13, C-OOS-7 |
| ✓ | github-did-identity-link `design/wave-decisions.md` | A-4, C-GDL-3, C-GDL-10 |
| ✓ | Code: `openlore-review-app/src/{scan.rs, routes/review.rs, routes/publish.rs}`, `review-domain/src/{ownership.rs, plans.rs}` + module index, `ports/src/review.rs`, `ports` `GithubPort`, `scraper-domain/src/person_repos.rs`, `adapter-review-store/src/schema.rs`, `xtask/src/check_arch.rs` (review-app rules) | Current behaviour |
| ⊘ | `routes/github.rs`, `config.rs`, `wiring.rs`, `budget.rs`, `reconcile.rs`, `views/*` bodies | Signatures only; behaviour known from docs and callers |
| ⊘ | `crates/adapter-duckdb` | Not the review-app store (UC-RRA-6) |

## Key Decisions

| ID | Decision | ADR |
|---|---|---|
| D-RRA-D1 | In-place extension: zero new crates, no Lexicon or OAuth-scope change, one migration. | — |
| D-RRA-D2 | (OQ-4) Store v2: `github_accounts` keyed by (DID, GitHub id); `repo_choices` keyed by repo id (ticked and unticked rows); `imported_repos`; source columns on `suggestions`. | ADR-088 |
| D-RRA-D3 | (OQ-5) Visibility derived in pure code: account rows shown iff the account is listed and verified; imported rows shown iff the repo is on the import list. No `hidden` column. | ADR-089 §5 |
| D-RRA-D4 | (OQ-6) Build the owner tier of `claim_domain::authority` now; `may_sign` → `SigningPermit`; owner-only-signing extends the same module. Label "owner". | ADR-089 |
| D-RRA-D5 | BR-RRA-1 by type: `PublishPlan` constructors and `restore_plan` require a `SigningPermit`; three check-arch rules; property AT over every request path. | ADR-089 §3, §6 |
| D-RRA-D6 | Frozen scan plan in `scan_run_repos`; a resume reads the paused run's unread rows; one admission per scan; per-account bio check before its listing. | ADR-088 §6 |
| D-RRA-D7 | Limit arithmetic pure in `repo_scope`; Save/import count other accounts' default ticks as shown; `eligible_count_seen` stands in when no live listing is at hand. | ADR-088, UC-RRA-1 |
| D-RRA-D8 | Scope changes serialized by an in-process per-DID lock (one container). | ADR-088 §10 |
| D-RRA-D9 | New `GithubPort::read_repo` (one request, one same-host redirect); listing rows carry `repo_id`, `owner_id`; publish reads by id. | ADR-089 §1 |
| D-RRA-D10 | (D-RRA-17) Removal through a separate `ScopeRemovalPort` (`removal.rs`); conditional insert makes removal during a scan race-free. | ADR-088 §7-8 |
| D-RRA-D11 | (OQ-10) KPI counters: see the list below. Plus log event `ownership.publish_check.passed`. | — |
| D-RRA-D12 | (OQ-7) Live listing on open, concurrent per account, one htmx fragment per account, no cache. | — |

### Options per significant decision

**D-RRA-D2 Store shape (OQ-4)**

| Option | Trade-off | |
|---|---|---|
| A. Normalized tables, ids as keys | Row-level owner scoping, targeted deletes, conditional insert; one migration | **Recommended** |
| B. JSON blob per DID | No key migration; but blind to the SQL rule, no conditional insert, whole-blob rewrites | |
| C. Keep `github_links` + `extra_github_accounts` | Smallest migration; two code paths, first account special | |

**D-RRA-D3 Visibility (OQ-5)**

| Option | Trade-off | |
|---|---|---|
| A. Derived in a pure function from source + lists | Property-testable; no drift; matches D-12 "hidden, not deleted" | **Recommended** (= PO recommendation) |
| B. Stored `hidden` flag | Must be updated on every status change; drifts | |
| C. SQL join in the adapter | Fast, but the rule lives in SQL, outside property tests | |

**D-RRA-D4 Publish gate (OQ-6)**

| Option | Trade-off | |
|---|---|---|
| A. Review-domain-local check, swapped later | Smallest now; two gates until OOS lands | Fallback |
| B. Owner tier of `claim_domain::authority` now | One rule and permit type from day one; commits the module shape early | **Recommended** |
| C. Wait for owner-only-signing | R2/R3 ship without BR-RRA-2 | |

**D-RRA-D5 BR-RRA-1 enforcement**

| Option | Trade-off | |
|---|---|---|
| A. Permit required by plan constructors (type) + check-arch + property AT | Non-representable; costs a signature change on 3 functions | **Recommended** |
| B. Boolean check in each route (today) | One missed route reopens the hole | |
| C. Check only in the executor | Previews of imported suggestions would still render | |

**D-RRA-D6 Scan plan (BR-RRA-4)**

| Option | Trade-off | |
|---|---|---|
| A. Persisted plan rows, resume reads unread rows | Exact BR-RRA-4; survives restart; one more table | **Recommended** |
| B. Re-plan on resume | Simple; breaks AC-002.2 | |
| C. In-memory plan | Lost on restart | |

**D-RRA-D7 Default ticks in the limit**

| Option | Trade-off | |
|---|---|---|
| A. Live listing of every default account at each Save/import | Exact; up to 3 extra GitHub calls per Save/import | |
| B. Last-seen eligible count, live when at hand; scan recomputes inside the room | Cheap; staleness affects only refusal edges; scans never exceed the cap | **Recommended** |
| C. Materialize defaults as a saved choice at verify | Exact; defaults stop following the ranking (changes today's behaviour) | |

**D-RRA-D10 Removal**

| Option | Trade-off | |
|---|---|---|
| A. Separate removal port, delete pending in one tx, conditional insert in the scan | Race-free; DELETE confined to one module | **Recommended** |
| B. Delete after the scan ends (DISCUSS wording) | Needs a post-scan hook and a pending-delete queue | |
| C. Soft-delete flag | Contradicts D-RRA-17 | |

**D-RRA-D12 Listing freshness (OQ-7)**

| Option | Trade-off | |
|---|---|---|
| A. Live read on open, per-account fragments | Fresh; ≤ 3 calls per open (NFR-RRA-4) | **Recommended** (= PO) |
| B. TTL cache | Fewer calls; hides new and transferred repos | |
| C. Reuse the last scan's listing | Free; stale by days | |

### KPI counter names (OQ-10, D-RRA-D11)

Closed `KpiEvent` catalogue, counts only, no DID, login or repo name:
`repos.choice.viewed`, `repos.choice.saved`, `repos.choice.changed_from_default`,
`repos.choice.refused.over_limit`, `account.add.started`, `account.add.verified`,
`account.add.refused.{did_missing, held_by_another, already_listed, at_limit, unreachable}`,
`account.removed`, `import.added`, `import.removed`,
`import.removed.age.{lt_1d, lt_14d, ge_14d}`,
`import.refused.{shape, not_found, owned, over_limit, unreachable}`, `import.moved_to_owned`,
`suggestion.acted.outside_default`, `suggestion.acted.outside_default.other_account`,
`suggestion.acted.outside_default.imported`, `publish.refused.not_owned`,
`publish.refused.bio_failed`, `publish.refused.unreachable`, `scan.refused.nothing_to_read`,
`scan.refused.over_limit`, `scan.account.failed`, `scan.account.unfinished`.
Log events (not counters): `ownership.publish_check.passed` / `.refused` (KPI-RRA-4 audit),
`scan.repo.read` with the account outcome (KPI-RRA-5).

## Architecture Summary

Modular monolith, ports and adapters, functional core (unchanged, ADR-007/009). New pure modules:
`claim_domain::authority` (owner tier) and `review_domain::repo_scope`. Extended: `plans`
(permit), `views`, `kpi`, `ports`, `adapter-github` (`read_repo`, ids), `adapter-review-store`
(v2), `openlore-review-app` (two new route files, scan, publish), `xtask` (3 new rules, 1
amended). C4 L1/L2/L3: `architecture-design.md` §2-4. Releases: R1 choose (+ the whole v2
migration), R2 accounts + permit, R3 imports (one unit).

## Technology Stack

No new dependency. Rust (MIT/Apache-2.0 workspace), DuckDB (MIT) via the existing binding,
hyper, maud, htmx (BSD-2) — all already in the review app. GitHub REST API (public endpoints, one
public-read PAT, ADR-076).

## Constraints Established

- C-RRA-D1: A `PublishPlan` exists only with a `SigningPermit` minted in the same request from
  `RepoFacts` and the owner's profile read in that request.
- C-RRA-D2: Stored status and source tags decide visibility and controls only, never publishing.
- C-RRA-D3: Accounts are keyed by numeric GitHub id, repos by numeric repo id; logins and names
  are display-only.
- C-RRA-D4: `DELETE` on `review-app.duckdb` only in purge, expiry and removal modules; removal
  deletes only `pending`.
- C-RRA-D5: A scan reads only its frozen plan; one admission; no repo of an account is listed or
  read before that account's bio check passes in that run.
- C-RRA-D6: The scan limit and account limit are single named constants shown on every surface.
- C-RRA-D7: No GitHub call to anything but `api.github.com` public endpoints; the SSRF guard and
  allowlist rules stay.
- Inherited: C-RRA-1..5, I-BRA-1..8, BR-1, NFR-BRA-8, C-GDL-1/3/10, ADR-086 §4.

## Upstream Changes

See `upstream-changes.md`: UC-RRA-1 (limit arithmetic), UC-RRA-2 (scan cost includes listing),
UC-RRA-3 (fresh read decides, not the tag), UC-RRA-4 (shared gate with OOS), UC-RRA-5
(`read_repo` base for OOS), UC-RRA-6 (store crate and design location), UC-RRA-7 (KPI names),
UC-RRA-8 (removal during a scan). `data-models.md` §3.1 of bluesky-claim-review-app superseded
by ADR-088.

## User decisions (2026-10-19, in chat)

| ID | Question | User decision |
|---|---|---|
| OQ-D1 | Approve ADR-088 and ADR-089? | Approved as written ("Approve now"); the second architecture review was skipped |
| OQ-D2 | Should the `imported` tag also block publishing once a listed account owns the repo (UC-RRA-3)? | No: once moved, its suggestions follow the owned-repo rules; the fresh check at publish still applies |
| OQ-D3 | Do saved ticks of an unverified account count toward the limit (UC-RRA-1)? | Yes |
| OQ-D4 | Delete a removed source's pending suggestions at once and refuse a running scan's later inserts (UC-RRA-8)? | Yes |
| OQ-D5 | Keep `github_links` (expand-only) until a later migration drops it? | "It's not in prod, don't worry about it, just get it done": the design stands as written |
| OQ-D6 | Roll forward as the default? | Same answer: the design stands as written |

The scan limit is the constant 10 (D-RRA-16); not configurable. Changing it is a user decision.

## Review

**Iteration 1** (nw-solution-architect-reviewer): `rejected_pending_revisions` — 1 critical,
6 high, 6 medium, 3 low. Strengths noted: contract shapes per component, plan-returning pure
cores, no route found that writes without a permit, ADRs with full alternatives, valid C4.

| Issue | Sev | Revision |
|---|---|---|
| I-01 publish/scan held a broad accounts write port | critical | Split `AccountStatusWrite` (mark unverified) and `AccountListingNote`; `GithubAccountsWrite` only in `routes/github.rs`; new rule `review_accounts_write_only_in_github_route` |
| I-02 freshness inputs are plain data | high | ADR-089 and §7 restated as type + lint + property, residual risk named (Rust cannot restrict construction to another crate) |
| I-03 configurable cap 1..=30 contradicted D-RRA-16 | high | Constant 10, no setting |
| I-04 migration on a live store | high | `CHECKPOINT`, close, timestamped backup; idempotent re-run; rollback data loss stated; NULL-source rule for DIDs without a link; one `github_links` wording; migration test case |
| I-05 resume outcome per account | high | Pass / fail ("account unverified") / unfinished (carried) / removed; property on runs + resumes |
| I-06 listing capped at 100 | high | Up to 3 pages; truncated listing never proves a repo missing (one `read_repo` decides) |
| I-07 two definitions of `S` | medium | One definition in `repo_scope`; property ties Save admission to scan start |
| I-08 BR-RRA-4 wording | medium | Raised to the user as OQ-D4 |
| I-09 removal universe | medium | Account removal also deletes its choices (starts fresh); `scan_run_repos` kept, expiry keeps latest 2 runs, purge all |
| I-10 allowlist mismatch with OOS | medium | One shared allowlist recorded in ADR-089 and component-boundaries |
| I-11 contract tests only recommended | medium | Required CI acceptance-stage gate |
| I-12 identity-link write gate unnamed | medium | Named: existing fresh-proof rules; out of the claim permit on purpose |
| I-13 `ScanRunPort` mixed read/write | low | `ScanPlanRead` split out |
| I-14 unticking keeps pending | low | Stated in §7 |
| I-15 signatures in port table | low | Kept, marked indicative |

**Iteration 2**: not run. The single allowed revision pass addressed every issue in the text;
no third review. Escalated to the user with ADR-088/089 as Proposed.
