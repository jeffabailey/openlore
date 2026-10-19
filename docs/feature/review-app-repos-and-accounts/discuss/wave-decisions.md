# Wave Decisions: review-app-repos-and-accounts (DISCUSS)

- **Mode**: subagent, autonomous, interactive=low. The user's decisions below are final; other
  questions are recorded as open questions for DESIGN.
- **Analyst**: Luna (nw-product-owner). **Date**: 2026-10-19.
- **Config**: feature_type=user-facing; walking_skeleton=depends (decided: not needed);
  research_depth=lightweight (happy path plus error paths; no new persona). Platform: web
  (WCAG 2.2 AA).

## Prior-wave reading

| | Artifact | Used for |
|---|---|---|
| ✓ | `docs/product/jobs.yaml` | J-009 (b, c, d) and J-004 (a); no new job needed |
| ✓ | `docs/product/kpi-contracts.yaml` | KPI-GDL-1..4, KPI-OOS-1..4 pattern; no KPI-BRA entries registered there |
| ✓ | `docs/product/personas/` (P-001, P-002) | P-003 lives in the bluesky-claim-review-app requirements, reused |
| ✓ | `docs/product/journeys/scrape-propose-sign.yaml` | Shared derivation; unchanged |
| ⊘ | `docs/product/journeys/` review-app journey | Did not exist; created `review-and-publish-my-claims.yaml` (SSOT) |
| ⊘ | `docs/project-brief.md`, `docs/stakeholders.yaml` | Missing (expected) |
| ⊘ | `docs/feature/review-app-repos-and-accounts/discover/`, `diverge/` | Missing (risk below) |
| ✓ | `docs/feature/bluesky-claim-review-app/discuss/` requirements, user-stories, story-map, jtbd-job-stories, journey yaml + visual, shared-artifacts-registry, outcome-kpis, wave-decisions | BR-3, I-BRA-*, D-3/D-12, KPI-BRA-*, vocabulary, story format |
| ✓ | `docs/feature/bluesky-claim-review-app/design/data-models.md` | One GitHub link per DID; pending visibility derived from link status |
| ✓ | `docs/feature/github-did-identity-link/design/wave-decisions.md` | A-4 (several accounts per DID), C-GDL-1, C-GDL-3, C-GDL-10 freshness |
| ✓ | `docs/feature/owner-only-signing/design/wave-decisions.md` (found while reading) | The user's import/sign rule, OOS-D5 (admin approximated by `.github/openlore-authority`), OOS-D13 B (import in the app), C-OOS-7 |
| ✓ | Code: `review-domain/src/views/{queue,github}.rs`, `review-domain/src/budget.rs`, `openlore-review-app/src/scan.rs`, `scraper-domain/src/person_repos.rs` | Current behaviour; commit 139dd09 "Manage your GitHub link" link |

**Risk (no DIVERGE/DISCOVER):** there is no pre-validated job for this feature. Mitigation:
stories are grounded in the review app's job J-009 (and J-004 for imports), which
bluesky-claim-review-app validated with JTBD, and in the user's own report of the gap on the
live app.

## Scope Assessment: SPLIT-WITHIN-FEATURE — 9 active stories plus 1 deferred (US-RRA-007), 3 contexts, about 16-20 days

Signals met: effort above 2 weeks, and several outcomes that can ship separately. Following the
user's suggested order, the feature is sliced into 4 releases, each shippable alone, in one
feature directory. R4 is deferred by the user (D-RRA-12) and is the candidate to carve out.

| Release | Stories | Est. |
|---|---|---|
| R1 Choose owned repos | US-RRA-001, 002 | 4 d |
| R2 Second GitHub account + fresh publish check | US-RRA-003, 004, 008, 010 | 7 d |
| R3 Any public repo, unsigned (one shipping unit) | US-RRA-005, 006, 009 | 5.5 d |
| R4 Administered repos (DEFERRED, backlog) | US-RRA-007 | 2-3 d once a proof is chosen |

Contexts: review-app UI and private state; GitHub public reads and the scraper derivation; the
publish path.

## Key Decisions

| ID | Decision | Source |
|---|---|---|
| D-RRA-1 | The repo list shows each verified account's owned repos; today's top 5 are pre-ticked; the saved choice is remembered for every later scan. | User |
| D-RRA-2 | Any public repo may be named by owner/name. Its suggestions are unsigned, private to the owner, and never publishable unless a verified account of the owner owns (or, after R4, administers) the repo. | User ("Any public repo, unsigned") |
| D-RRA-3 | Several GitHub accounts per Bluesky sign-in. One scan reads the chosen repos of every account that passes its bio check in that scan; each suggestion names its account. | User ("every verified account") |
| D-RRA-4 | No separate walking skeleton: the review app's flow is live; R1 is the thin slice. | PO |
| D-RRA-5 | Forks and archived owned repos are still not offered on the owned list; they may be imported. | PO |
| D-RRA-6 | Repos that appear after a saved choice are shown unticked and marked new. | PO |
| D-RRA-7 | Unticking a repo stops future reads only: its pending suggestions are kept, declines stay declined, published claims and links are untouched. (Removing an account or imported repo: D-RRA-17.) | PO |
| D-RRA-8 | Any signed-in DID may import, without a verified GitHub account; the scan budget applies; such a scan says "No GitHub account to re-check". | PO (C-OOS-7) |
| D-RRA-9 | BR-RRA-2: at preview and confirm, the app reads the repo's current owner and that owner account's bio in the same request; a stored status is never proof; a failed bio marks the account unverified. | PO (C-GDL-10, OOS-D13 A) |
| D-RRA-10 | Imported cards offer Decline only. The UI says "Not yours to publish". Its decline is labelled "Decline" (same action and N key as "Not me", which reads wrong on someone else's repo). "Unsigned" stays a requirements term; "self-attested" stays reserved for published claims. | PO |
| D-RRA-11 | BR-RRA-3: one total limit across chosen (saved or default) and imported repos, checked at every Save, import and scan start. | PO |
| D-RRA-12 | (was OQ-1) US-RRA-007 (administered repos) is DEFERRED and stays in the backlog. R1-R3 ship with owned-repo publishing only. No proof method is chosen; the options stay listed: GitHub sign-in reading `permissions.admin`; the `.github/openlore-authority` listing (owner-only-signing OOS-D5 A); or stay deferred. | User (2026-10-19) |
| D-RRA-13 | BR-RRA-4: a scan uses what was in force when it started; removals stop reads at once. | PO (review fix) |
| D-RRA-14 | BR-RRA-5: a repo is on one list only; an imported repo owned by a listed account moves to its owned list and is written, ticked, into that account's saved choice (created from its current ticks if absent). It always fits: the total is unchanged. | PO (review fix) |
| D-RRA-15 | (was OQ-3) Up to 3 GitHub accounts per sign-in, with a refusal that names the fix. | User (2026-10-19) |
| D-RRA-16 | (was OQ-2) The per-scan limit is 10 repos in total; default ticks count toward it (BR-RRA-3). | User (2026-10-19) |
| D-RRA-17 | (was OQ-9) Removing a GitHub account or an imported repo DELETES its pending suggestions. Published claims stay in the PDS, untouched. The private records of declined keys (and, for an account, published keys) are kept so BR-1 (never re-offer a declined key) and BR-2 (show "already published") still hold; they are purged only by "Disconnect and forget me" (US-BRA-012). Replaces "hide; purge only on disconnect". No conflict found with an existing rule: D-12's "hidden, not deleted" covers a failed bio check, not a removal. | User (2026-10-19) |

## Alternatives considered (DISCUSS level)

| Topic | Alternative | Why not chosen |
|---|---|---|
| Choice model | One flat list across all accounts | Hides which account a repo belongs to; per-account lists match the account list and the per-account bio check |
| Choice model | No default ticks (start empty) | User decision: top 5 pre-ticked; also keeps today's behaviour for owners who never open the list |
| Limit | Per-account limit | Several accounts could multiply GitHub cost; one total limit bounds a scan |
| Limit | No limit | Breaks the NFR-BRA-4 timing budget and the GitHub floor |
| Imports | Require a verified GitHub account to import | Nothing imported can be published, so proof adds nothing; C-OOS-7 keeps import open to everyone |
| Imports | Publish imported suggestions as third-party claims | Contradicts the user's rule (only owners or admins sign) and owner-only-signing |
| Publish check | Trust the scan-time owner | A repo transferred after the scan would still publish; C-GDL-10 requires a fresh read |
| Admin proof | Decide now (OAuth or authority file) | User deferred US-RRA-007 (D-RRA-12) |
| Remove | Hide pending on remove, purge only on disconnect | User chose delete (D-RRA-17) |

## Requirements Summary

- 9 active stories plus 1 deferred (US-RRA-007), US-RRA-001..010, all user-facing, each with job_id, Elevator Pitch, 3-7 UAT
  scenarios, AC that each name their scenario, and an outcome KPI. See `user-stories.md`.
- Business rules: BR-RRA-1 (unsigned suggestions never reach the publish path, on any request),
  BR-RRA-2 (fresh ownership and bio read in the request), BR-RRA-3 (one total repo limit),
  BR-RRA-4 (scan uses what was in force at start), BR-RRA-5 (one list per repo).
- NFR-RRA-1..6 (responsiveness, list and scan timing, GitHub budget, accessibility, third-party
  privacy).
- Outcome KPIs: KPI-RRA-1..7 (`outcome-kpis.md`); North Star KPI-RRA-1; guardrails
  KPI-RRA-4, 5, 7. Targets are hypotheses with 0% baselines.
- Journey: `journey-choose-repos-and-accounts.yaml` and `-visual.md` (feature);
  `docs/product/journeys/review-and-publish-my-claims.yaml` (SSOT, created).

## Constraints Established

- C-RRA-1: No suggestion about a repo not owned by an account on the owner's list whose bio holds
  her DID, read in that request, is ever previewed, approved, written to a PDS, shared or shown
  on a profile, whatever request is sent.
- C-RRA-2: Each account's bio is checked in a scan before any of its repos is read; a failing
  account does not stop the other accounts or the imported repos.
- C-RRA-3: A scan reads only the saved choice (or default top 5, limited by the room left under BR-RRA-3) of each passing account plus
  imported repos, at most `${scan_repo_cap}` in total, under one scan admission.
- C-RRA-4: Repo choices, the account list and imported repos are private app state, purged by
  "Disconnect and forget me".
- C-RRA-5: Owners of imported repos are never contacted and no public trace of an import exists.
- Inherited unchanged: I-BRA-1..8, BR-1, NFR-BRA-8, C-GDL-1, scan admission in
  `review_domain::budget`.

## Upstream Changes

- **Changed Assumption (BR-3)**: `docs/feature/bluesky-claim-review-app/discuss/requirements.md`
  BR-3 reads: "Only repos **owned** by the verified login are scanned. Forks and archived repos
  are skipped (existing `select_person_repos` rule)." This feature widens it to: owned repos of
  every verified login, as chosen by the owner, plus any public repo the owner names, scanned as
  unsigned and never publishable. The earlier file is not edited.
- I-BRA-4 in the same file ("**No scrape without current proof.** The signed-in DID must be
  present in the GitHub bio, checked immediately before each scrape (D-3, D-12).") now applies
  per account and to owned repos; imported repos need no proof.
- D-12 (re-verify before every scrape; hide pending when it fails) now applies per account.
- The review app's private-state rule "one GitHub link per DID" (`data-models.md` §3.1) becomes
  several per DID (allowed by github-did-identity-link A-4).
- Owner-only-signing OOS-D13 option B is realized by US-RRA-005/006. OOS-D13 option A
  (publish-time authority check) is overlapped by US-RRA-006/010; see OQ-6.
- SSOT: created `docs/product/journeys/review-and-publish-my-claims.yaml`.
  `docs/product/jobs.yaml` unchanged (no new job). KPI-RRA-1..7 to be registered in
  `kpi-contracts.yaml` at DESIGN acceptance.

## Open Questions for DESIGN

| ID | Risk | Question | PO recommendation |
|---|---|---|---|
| OQ-4 | HIGH | Private store changes: several accounts per DID; choices that survive a GitHub rename; the imported repo list; each suggestion's source account; deleting pending rows on remove while keeping declined (and published) key records (D-RRA-17); purge coverage. | Identify accounts by their immutable GitHub id. |
| OQ-5 | HIGH | Visibility rule for pending suggestions per source account and for imported ones. | Visible iff (source account on the list and verified) or (repo on the imported list). |
| OQ-6 | MED | One publish-eligibility check for this feature and owner-only-signing (ADR-086/087, not yet delivered; `claim-domain::authority` does not exist). | A pure function now, shaped so `authority` can replace it; label "owner". |
| OQ-7 | MED | Freshness and cost of the owned-repo list (a GitHub read per account per view). | Live read on open; no cache in v1; meet NFR-RRA-2. |
| OQ-10 | LOW | New KPI counter names (no PII). | As listed in `outcome-kpis.md`. |

Resolved: OQ-8 in review ("No GitHub account to re-check", AC-005.5); OQ-1, OQ-2, OQ-3 and
OQ-9 by the user on 2026-10-19 (D-RRA-12, D-RRA-16, D-RRA-15, D-RRA-17).

## Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| An imported suggestion reaches a PDS through a request path missed in tests | M | H | BR-RRA-1 as a property over every request path (US-RRA-006); KPI-RRA-4 production audit |
| More repos and accounts per scan exhaust the GitHub budget | M | M | Total limit (BR-RRA-3); NFR-RRA-4; existing floor and pause; DEVOPS baseline |
| Per-account hiding leaks a failed account's suggestions, or hides the wrong ones | M | M | AC-004.3/004.4; OQ-5 |
| Divergence from owner-only-signing's publish check | M | M | OQ-6 |

## Review

Iteration 1 (nw-product-owner-reviewer): rejected_pending_revisions. Revisions in iteration 2:

| Issue | Fix |
|---|---|
| DoR item 5: AC without a scenario (all stories) | Every AC now names its scenario; scenarios added; oversized stories split (US-RRA-008 remove account, US-RRA-009 remove imported repo, US-RRA-010 publishing follows current ownership) |
| Implement-X title on US-RRA-005 | Renamed "Get private suggestions for any public repo I name" |
| Refusals without a next step | Every refusal message now names the next step; step 6 mockup has a way back |
| Over-limit data unreachable | Uses Dmitri (12 owned repos); cross-account case in US-RRA-004 |
| Limit rule ambiguous | BR-RRA-3: one total, checked at Save and import |
| Imported repo becoming owned | BR-RRA-5 + US-RRA-010 scenario |
| "Currently verified" undefined | BR-RRA-2: owner + bio read in the request; bio-edited-before-confirm scenario |
| YAML first-visit mockup wrong | Fixed |
| Missing error paths | GitHub unreachable at add account, import and confirm; mid-scan pause; concurrency (BR-RRA-4) |
| Medium/low | NFR section; third-party privacy (NFR-RRA-6); `outside_default` defined once; targets labelled hypotheses; alternatives table; US-RRA-007 DEFERRED-PENDING-OQ-1; R3 one unit; I-BRA-4 quoted from source; store shape stated as outcomes; display-only artifacts registered; decline label rationale |

Iteration 2 (nw-product-owner-reviewer): rejected_pending_revisions. All iteration-1 blockers were
confirmed resolved except as below. 1 critical and 3 high were raised; fixed in text afterwards.
**No third review (2-iteration limit): escalated to the user for sign-off before DESIGN.**

| Issue | Fix after iteration 2 |
|---|---|
| C1 GitHub unreachable during a scan undefined | New system rule "an unfinished check is not a failed check"; US-RRA-004 scenario + AC-004.8; preview-unreachable scenario in US-RRA-010 (AC-010.3); AC-006.4 narrowed to confirm |
| H1 BR-RRA-2 shipped only in R3 | US-RRA-010 moved to R2 (owned publish check); the BR-RRA-5 move scenarios moved into US-RRA-006 (AC-006.5) in R3 |
| H2 default ticks could exceed the limit | BR-RRA-3: default ticks count and fill only the room left, in account order; scan-start check refuses before admission; US-RRA-003 scenario + AC-003.7 |
| H3 moved repo "ticked" vs default top 5 | BR-RRA-5: the move is written into that account's saved choice (ticked; branch for "no room" dropped in iteration 3, F4); AC-001.1 and journey checkpoint updated |
| Medium | SSOT invariants BR-RRA-3/4/5 + unfinished-check rule; remove steps added as journey failure modes; BR-RRA-5 scan trigger scenario |

Iteration 3 (focused, user-requested): conditionally_approved. F1-F6 fixed in text:

| Issue | Fix |
|---|---|
| F1 default ticks read as unlimited top 5 per account | Every mention now says "top 5, limited by the room left under BR-RRA-3" (US-RRA-002 Solution and AC-002.1, US-RRA-004 notes, story map R2, SSOT step 9); journey oracle now first min(5, room left) in account order, matching AC-001.1/AC-003.7 |
| F2 "fail" undefined | Bio-check rule and US-RRA-004 Solution: fails only when the bio was read and lacks the DID; unreachable is not failure (AC-004.8) |
| F3 move's saved-choice write missing | Added to the journey failure mode, D-RRA-14, and the scan-triggered move scenario's Then-clauses |
| F4 unreachable "unticked" branch | Dropped: the move always fits because the total is unchanged |
| F5 AC-009.2 published keys | States an imported repo has no published keys (BR-RRA-1); a moved repo follows account rules |
| F6 counts | "9 active stories plus 1 deferred"; AC totals refreshed in dor-validation.md |

Open lows not changed: J-009c fit for repo choice; "claims" wording on the imported card vs the
vocabulary table; Given clauses on two US-RRA-005 scenarios; saved choice on re-adding a removed
account and removing the last account (US-RRA-008).
