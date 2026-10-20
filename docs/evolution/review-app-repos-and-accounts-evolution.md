<!-- markdownlint-disable MD013 -->
# Evolution: review-app-repos-and-accounts

> Releases R1 to R3 of `review-app-repos-and-accounts`. Release R4 (administered
> org repos, US-RRA-007, KPI-RRA-6) was deferred by the user and has no code.
> The feature directory `docs/feature/review-app-repos-and-accounts/` stays in
> place as delivery history. Paradigm: functional Rust (ADR-007). Decisions:
> ADR-088 (store v2) and ADR-089 (publish permit). DELIVER rulings:
> `deliver/upstream-issues.md` (U-RRA-D1..D12).

## Summary

Before this feature, the review app scanned one GitHub account per Bluesky
sign-in and always read that account's top 5 repos. It now lets the owner
choose what is scanned and from where:

- **Choose repos to scan.** A "Choose repos" page lists each verified
  account's owned repos live from GitHub. The top 5 are ticked by default.
  One scan reads at most 10 repos in total, across all accounts and imports.
  A Save that would go over 10 is refused and says how many to untick.
- **Import any public repo.** The owner can add a public repo she does not
  own. A scan reads it and gives her private suggestions. Those suggestions
  are never signed, published, shown on a profile or shared. If one of her
  listed accounts later owns the repo, it moves to that account's owned list.
- **Publishing only for repos a verified account owns.** At preview and again
  at confirm, the app reads the repo's current owner and that owner's bio in
  the same request. Only `claim_domain::authority::may_sign` can turn those
  reads into a `SigningPermit`, and no publish plan can be built without one.
  A stored "verified" flag is never proof.
- **Up to 3 GitHub accounts per sign-in.** Each account has its own bio check.
  One scan checks and reads every verified account. An account whose check
  fails hides only its own suggestions.
- **Removal deletes pending suggestions.** Removing an account or an import
  deletes its pending suggestions at once, even while a scan is paused.
  Declined and published key records stay.

### What shipped (one paragraph)

`review-app.duckdb` moved to schema v2 (ADR-088): `github_accounts` keyed by
(DID, numeric GitHub id), saved repo choices, imports, a source on every
suggestion, and a frozen scan plan (`scan_run_repos`) so a paused scan
resumes with exactly what was in force when it started. The pure rules live
in `review_domain::repo_scope` (limits, default ticks, plan, resume, imports,
moves, visibility) and `review_domain::budget`. The publish rule lives in the
new `claim_domain::authority` owner tier (ADR-089), fed by a new
`GithubPort::read_repo`. Scope changes (Save, Add repo, account add, verify,
remove, forget me) are serialized per DID by a scope lock and never call
GitHub while holding it. check-arch gained `review_publish_plan_needs_permit`,
`authority_evidence_only_from_adapters` and `signing_permit_not_serializable`,
and `DELETE` is now allowed in the removal modules as well as purge and
expiry.

### Store v2 migration

The app migrates a v1 store once, at startup, in one transaction. Before the
step it copies the file to `review-app.v1.<secs>.bak`. A crash before commit
leaves v1, and the next start reuses the same copy, never adding a second.
After the v2 store opens and passes its startup probe, the copy is deleted
(review R5, user decision "Delete after upgrade"). The copy holds every
owner's private state in plain form, and "Disconnect and forget me" cannot
reach it, so it must not linger. If the copy cannot be deleted, the app logs
`store.v1_copy.not_removed`, still serves, and tries again on every start
(re-review N4). Rollback is forward only once the upgrade has completed.

### Page allowance

Page actions (opening "Choose repos", Save, Add repo, Verify, and the owner
and bio reads of every preview and confirm) share one server token with
scans. `review_domain::budget::admit_page_requests` admits them:

| Allowance | Limit | Why |
|---|---|---|
| Per DID | 200 GitHub requests per hour | Plenty for one person's clicks; stops one runaway tab |
| App-wide | 1000 GitHub requests per hour | A fifth of GitHub's 5000/h, so at least 4000/h stay for scans |

Every HTTP request counts. A listing is charged for its page limit, and
the pages it did not use are given back. A refused page read writes nothing,
keeps any confirmed plan, and says to try again in a few minutes. The scan
floor (pause at 300 remaining) is unchanged and is not applied to pages.

### Wave timeline

All waves ran on 2026-10-19.

- DISCUSS: 9 active stories (US-RRA-001..010 less 007), D-RRA-1..17,
  KPI-RRA-1..7. Split into releases R1 to R3; R4 deferred by the user.
- DESIGN: ADR-088 and ADR-089, accepted by the user in chat.
- DISTILL: 59 HTTP scenarios and 11 CORE-RRA properties in four suites.
  Cycle 1: SA approved, PO, PA and Sentinel rejected; all findings fixed.
  Cycle 2: approved; Sentinel's wording residuals fixed without a third
  review.
- DELIVER: roadmap of 20 steps (7 + 7 + 6). Roadmap review: round 1 rejected
  on F1-F8 (F3 accepted as a risk), round 2 approved. All 20 steps
  RED/GREEN/COMMIT PASS in `execution-log.json`. Then the L1-L4 refactor
  (`3a0766a`), the adversarial review, the re-review and mutation.

## DELIVER steps

- **R1, choose owned repos** (01-01..01-07): store v2 migration (`08dae26`);
  walking skeleton, publish a chosen repo outside the old top 5 (`170d81a`);
  the choose page with default ticks and empty and failure states
  (`592846f`); Save and the limit (`224205d`); a scan plans exactly the
  choice before admission (`1d4b18b`); unreadable chosen repos skipped and
  named, `read_repo` added (`9bc473a`); a paused scan finishes its frozen plan
  (`f598a20`).
- **R2, several accounts and the permit** (02-01..02-07): up to three accounts
  (`c6fb5f3`); each account's bio checked before its repos are read
  (`6e892f2`); unverified accounts re-checked (`f57be59`); removal deletes only
  that account's pending suggestions (`0073c5b`); a resumed scan adds nothing
  for a removed account (`16e97cc`); the `SigningPermit` gate (`7aa3cef`);
  preview and confirm each re-check owner and bio (`23d680a`).
- **R3, imports** (03-01..03-06): import a public repo (`f3d1273`); refusals
  name the cause (`7a17899`); imported suggestions can never be published,
  shown or shared (`2a7c3c2`); an import moves once owned (`26a521f`);
  removing an import deletes its pending suggestions, purge forgets all
  (`417ef72`); scope changes serialized per DID, KPIs registered (`906018e`).

## Review rounds

**Adversarial review, F1-F5** (`3d7978d`, `41683c0`):

- F1, F2: publishing is gated by the permit alone. No stored "verified" flag
  gates it, and a publish through an account stored unverified marks it
  verified again when its bio holds.
- F3: a refused permit puts the plan back with its deadline.
- F4: a renamed repo is refused under its old name (`RepoRenamed { now }`).
  It was blocked on CORE-RRA-6's generator (U-RRA-D11) until DISTILL aligned
  it.
- F5: an unused probe arm was deleted.

**Review, R1-R8 and D12:**

- R1: a Save re-reads its account under the scope lock, and the store writes
  the choice only while the account row exists (`a1e3543`).
- R2: no GitHub request while a scope lock is held; the lock is waited for at
  most 10 s; every GitHub request is bounded at 10 s (`a1e3543`, `7af08c5`).
- R3: the per-DID page allowance; each verified account is listed once per
  request (`a1e3543`, fixes U-RRA-D4).
- R4: forget me purges under the scope lock, and a purged scan run gets no
  plan rows (`6e7872f`, `c088606`).
- R5: the v1 backup is deleted after a successful upgrade (`6952e97`,
  `3876fc1`).
- R6: an import refusal names the would-be total ("Adding it would make
  11") (`8e2f4ac`).
- R7: the migration fault seam exists only in development builds, with a
  check-arch guard (`efaca54`).
- R8: an account already verified by another sign-in gets a neutral refusal
  that does not reveal the other sign-in (`a637d68`, user decision).
- D12: one failed account hides only its own suggestions, not the whole
  queue (`acfb7c8`, fixes U-RRA-D12).

**Re-review, N1-N8:**

- N1, N2: a permit's repo and bio reads spend the page allowance; every page
  HTTP request counts, and pages share 1000/h app-wide (`4b054ce`).
- N3: left unchanged. It asked to remove dead code
  (`review_domain::lifecycle::visible_and_approvable` and the adapter-duckdb
  probe's `dead_code` allowance); both predate this feature, and a shipped
  acceptance test calls the first.
- N4: a v1 copy that cannot be deleted warns and is retried each start
  (`f88e99c`).
- N5: the design now says why listings read before the lock are safe for
  ticks and moves (`26a71e7`).
- N6, N7: publish-path status writes hold the scope (never the GitHub reads);
  a confirm whose plan is not restored puts the plan back (`9b7b1d4`).
- N8: forget me on a busy scope says the grant is revoked but the data is
  not yet purged, and pressing it again finishes (`c17b68f`).

## Mutation

Per-feature, gate 80% (`deliver/mutation/mutation-report.md`).

- **Main run** on `c088606`: 981 mutants listed, all run. **767 of 778
  viable caught (98.6%)**. The first pass caught 695 (89.3%); 72 survivors
  were killed by new crafter-owned tests, including
  `crates/openlore-review-app/tests/page_mutation_pins.rs` and `xtask/tests/review_app_architecture_pins.rs`.
  Of the 11 left, most are equivalent or reachable only through a race; #214
  and #424 are real gaps (below). The security paths (`authority.rs`,
  `repo_scope/*`, `plans.rs`, `removal.rs`, the publish route) have no
  surviving mutant.
- **Re-run** on the re-review fixes (`c088606..c17b68f`): 90 mutants listed.
  **54 of 55 viable caught (98.2%)**. Two new tests killed 2 of the 3
  survivors; the last is the same equivalent hour boundary as #481.

## Deferred and follow-ups

- **R4, administered org repos** (US-RRA-007, KPI-RRA-6): deferred by the
  user. Proving that someone administers an org repo needs GitHub OAuth,
  which the app does not have. No code, no table.
- **Acceptance gap #214**: a moved import should keep its own name in the
  saved choice (`full_name_seen`). Low impact; only a later "skipped" line
  would show the wrong name.
- **Acceptance gap #424**: the GitHub double cannot rate-limit a listing
  after a passed bio check, so "listing rate-limited, so the scan pauses" is
  not observed. Needs a test-support switch.
- **Suggested @security acceptance test** for the cross-origin guard on page
  POSTs. It had no test anywhere until mutation pinned it at crate level
  (12 mutants, older routes included).
- **U-RRA-D10 accepted**: no test can show a move split across two
  transactions. The move runs in one DuckDB transaction; mutation cannot
  judge atomicity.
- `github_links` is kept, unwritten, and is dropped by a later migration
  (ADR-088 §1).
- U-RRA-D3: R1 to R3 deploy together (the forks-only state links to the R3
  import form).

## Lessons

1. A stored flag is not proof. Moving the publish decision into one pure gate
   fed by same-request reads (ADR-089) made F1 and F2 deletions, not new
   checks.
2. Hold a lock only around the store. Calling GitHub under the per-DID lock
   let one stalled request block every scope change (R2); reading first and
   deciding under the lock fixed it.
3. Count every HTTP request against an allowance. Charging a listing as one
   unit undercounted it, and permit reads were not charged at all (N1, N2).
4. A backup is private data too. The v1 copy sat outside forget me's reach
   until R5 deleted it after the upgrade.
5. Mutation found an untested security guard: the same-origin check on every
   page POST had no test before this run.

## Try it

1. Sign in, then open "Choose repos": each verified account's top 5 repos
   are ticked. Tick a repo outside the top 5, Save, and scan.
2. Add a second GitHub account (its bio must hold your DID) and scan again:
   both accounts are read in one scan.
3. Add a public repo you do not own: its suggestions appear, but they have
   no publish control.
4. Remove the second account: its pending suggestions are gone at once.

## Lasting artifacts

- `docs/adrs/ADR-088-review-app-store-v2-accounts-by-github-id-repo-choices-imports-frozen-scan-plan.md`
- `docs/adrs/ADR-089-review-app-publish-permit-fresh-repo-owner-and-bio-read-shared-authority-gate.md`
- `docs/feature/review-app-repos-and-accounts/design/architecture-design.md`
  and `component-boundaries.md`
- `docs/feature/review-app-repos-and-accounts/distill/walking-skeleton.md`
- `docs/feature/review-app-repos-and-accounts/discuss/journey-choose-repos-and-accounts.yaml`
- `docs/feature/review-app-repos-and-accounts/deliver/upstream-issues.md`
  and `deliver/mutation/mutation-report.md`

## Commit trail

Branch `rra-deliver`, 55 commits ahead of `main`: steps `08dae26` to
`906018e` (with DISTILL fixes for U-RRA-D1..D9 interleaved); refactor
`3a0766a`; review `3d7978d`, `82efeb8`, `a637d68`, `41683c0`, `efaca54`,
`acfb7c8`, `7af08c5`, `a1e3543`, `6e7872f`, `2acd556`, `8e2f4ac`, `6952e97`,
`3876fc1`, `c088606`; re-review `4b054ce`, `f88e99c`, `26a71e7`, `9b7b1d4`,
`c17b68f`; mutation `9c7a0a2`, `b574528`. Trunk-based, no PR. Not pushed.
