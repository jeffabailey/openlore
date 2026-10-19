# Definition of Ready Validation: review-app-repos-and-accounts

Nine DoR items per story, plus two gate checks (job_id, Elevator Pitch). Evidence points to
`user-stories.md`. Re-run after review iteration 1 (see `wave-decisions.md` → Review).

## Summary

| Story | 1 Problem | 2 Persona | 3 Examples | 4 UAT | 5 AC traced | 6 Size | 7 Tech notes | 8 Deps | 9 KPI | job_id | Pitch | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| US-RRA-001 | PASS | PASS | PASS (4) | PASS (7) | PASS (7/7) | PASS (2 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-002 | PASS | PASS | PASS (4) | PASS (7) | PASS (6/6) | PASS (2 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-003 | PASS | PASS | PASS (4) | PASS (7) | PASS (7/7) | PASS (2 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-004 | PASS | PASS | PASS (4) | PASS (7) | PASS (8/8) | PASS (2.5 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-008 | PASS | PASS | PASS (3) | PASS (4) | PASS (4/4) | PASS (1 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-005 | PASS | PASS | PASS (5) | PASS (7) | PASS (6/6) | PASS (2.5 d) | PASS | PASS | PASS | J-004 | yes | READY |
| US-RRA-009 | PASS | PASS | PASS (3) | PASS (4) | PASS (4/4) | PASS (1 d) | PASS | PASS | PASS | J-004 | yes | READY |
| US-RRA-006 | PASS | PASS | PASS (3) | PASS (7) | PASS (5/5) | PASS (2.5 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-010 (R2) | PASS | PASS | PASS (3) | PASS (3) | PASS (3/3) | PASS (1.5 d) | PASS | PASS | PASS | J-009 | yes | READY |
| US-RRA-007 | PASS | PASS | PASS (3) | PASS (4) | PASS (4/4) | PASS (2-3 d once a proof is chosen) | PASS | PASS (deferral tracked, D-RRA-12) | PASS | J-009 | yes | DEFERRED (backlog) |

## DoR Status: PASSED — 9 of 9 items for each of the 9 active stories plus 1 deferred (US-RRA-007)

AC totals after iteration 2 and 3 fixes: US-RRA-003 has 7 (incl. AC-003.7), US-RRA-004 has 8
(incl. AC-004.8), US-RRA-006 has 5 (incl. AC-006.5); 54 AC in all, each naming its scenario.

US-RRA-007 passes DoR on content but is deferred by the user (2026-10-19) and stays in the
backlog until a proof method is chosen.

Re-checked 2026-10-19 after the user's OQ-9 decision (remove deletes pending): US-RRA-008 and
US-RRA-009 still pass all 9 items. Each keeps 4 scenarios and 4 AC, every AC names its
scenario, and the size is unchanged (1 day each). The open dependency on OQ-9 is now resolved
(D-RRA-17).

Peer review: iteration 2 rejected (1 critical, 3 high); those four were fixed in text after
iteration 2 and have not been re-reviewed (2-iteration limit). See `wave-decisions.md` → Review.
The per-story evidence table below predates those fixes; the summary counts above are current.

## Evidence per story

| Story | Problem (item 1) | Persona (2) | Examples (3) | Item 5: each AC names its scenario | Tech notes / deps (7, 8) | KPI (9) |
|---|---|---|---|---|---|---|
| US-RRA-001 | Top 5 by stars never reads ferrous-notes, always reads driftwood | Priya (7 repos, 1 fork, 1 archived), Aisha (forks only), Dmitri (12 repos) | happy, forks only, over limit, rate limit | AC-001.1..7 → 7 scenarios incl. keyboard and needs-re-verify | Reuse `select_person_repos`; rename-safe choice; purge / live app | ≥25% change default |
| US-RRA-002 | A saved choice is worthless unless the scan reads exactly it | Priya, Dmitri | 5 read, new repo, archived, nothing chosen | AC-002.1..6 → 7 scenarios incl. concurrency and unticked pending | `scan.rs` call replaced / US-RRA-001 | ≥30% act on `outside_default` |
| US-RRA-003 | One link per sign-in drops one of Priya's accounts | Priya, Sam (adversarial), Dmitri | verified, DID missing, other DID, fourth account | AC-003.1..6 → 6 scenarios incl. GitHub unreachable | store holds one link (OQ-4), shared verify budget / live GitHub step | ≥70% complete |
| US-RRA-004 | Two accounts need one scan and clear provenance | Priya with two accounts | both pass, one fails, total over limit, pause | AC-004.1..7 → 7 scenarios incl. pause/resume | `run_scan` per account, per-account visibility (OQ-5) / US-RRA-003 | ≥50% act on 2nd-account suggestion |
| US-RRA-008 | Can't drop an account without disconnecting | Priya leaving her job | remove (pending deleted), re-add, remove during scan | AC-008.1..4 → 4 scenarios | D-RRA-17 delete on remove / US-RRA-003 | ≥60% scan again after removal |
| US-RRA-005 | Can't see suggestions for org or admired repos | Priya, Dmitri (unverified), maintainers of named repos (NFR-RRA-6) | cargo, bad names, hers, unverified, over limit | AC-005.1..6 → 7 scenarios incl. GitHub unreachable | reuse harvest; 404 for private; OOS-D13 B / ships with 006, 009, 010 | ≥50% act on imported |
| US-RRA-009 | No way to drop an imported repo | Priya | remove (pending deleted), re-add, at limit | AC-009.1..4 → 4 scenarios | D-RRA-17 delete on remove / US-RRA-005 | ≥40% keep an import 14 days |
| US-RRA-006 | Claims about others' repos must never land in Priya's repo | Priya, Sam (forged request) | visible refusal, forged, GitHub down | AC-006.1..4 → 5 scenarios incl. @property | publish has no repo re-check today; OQ-6 / US-RRA-005 | 0 unsigned writes (guardrail) |
| US-RRA-010 | Ownership changes after the scan | Priya publishing days later | transferred away, becomes hers, bio edited | AC-010.1..3 → 3 scenarios | OQ-6 / live publish path | 0 publishes of not-owned repos |
| US-RRA-007 | Admins can't publish; public data lacks admin rights | Priya as org admin | proof, no proof, lost proof | AC-007.1..4 → 4 scenarios | 3 proof options, none chosen / US-RRA-006/010, deferred | ≥1 dogfood claim, then ≥20% |

## Anti-pattern scan

| Anti-pattern | Result |
|---|---|
| Implement-X titles | None (US-RRA-005 renamed) |
| Generic data | None; real personas, logins, repos, DIDs |
| Technical AC | None; file and function names only in Technical Notes |
| Technical scenario titles | None |
| Oversized stories | None; max 7 scenarios, max 2.5 days |
| Abstract requirements | None; 3-5 examples per story |
