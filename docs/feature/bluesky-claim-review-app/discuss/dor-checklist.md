# Definition of Ready: bluesky-claim-review-app

This is the 9-item hard gate, plus two project checks: **job_id traceability** (Decision 1,
2026-04-28) and **Elevator Pitch present** (every story except @infrastructure).

## Summary

| Story | Scen. | Est. | 1 Prob | 2 Persona | 3 Ex | 4 UAT | 5 AC | 6 Size | 7 Tech | 8 Deps | 9 KPI | job_id | Pitch | Status |
|-------|-------|------|--------|-----------|------|-------|------|--------|--------|--------|-------|--------|-------|--------|
| US-BRA-000 | 3 | 2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | infra + rationale | n/a (@infra) | READY |
| US-BRA-001 | 5 | 2–3 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009a | PASS | READY |
| US-BRA-002 | 6 | 1–2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009b | PASS | READY |
| US-BRA-003 | 5 | 2–3 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009c | PASS | READY |
| US-BRA-004 | 5 | 3 d | PASS | PASS | PASS | PASS | PASS | PASS* | PASS | PASS (tracked) | PASS | J-009 / J-009d | PASS | READY* |
| US-BRA-005 | 4 | 1 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009d | PASS | READY |
| US-BRA-006 | 4 | 1–2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009e | PASS | READY |
| US-BRA-007 | 4 | 2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009f | PASS | READY |
| US-BRA-008 | 5 | 1–2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009f | PASS | READY |
| US-BRA-009 | 4 | 2–3 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009d | PASS | READY |
| US-BRA-010 | 4 | 2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009c | PASS | READY |
| US-BRA-011 | 3 | 1–2 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009d | PASS | READY |
| US-BRA-012 | 3 | 1 d | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | PASS | J-009 / J-009a | PASS | READY |

\* US-BRA-004 is at the top of the size range because it includes the verify-path acceptance
(AC-004.3, D-5). **Remediation, if DESIGN finds the verify change exceeds 3 days:** split it
into US-BRA-004a "publish to my PDS" and US-BRA-004b "OpenLore accepts self-attested" and
keep both in the walking skeleton. The dependency on OD-BRA-1 (encoding) is tracked, not
blocking DISCUSS.

## Evidence per item (representative; full content in `user-stories.md`)

### US-BRA-000: Public address (@infrastructure)

| Item | Evidence |
|------|----------|
| 1 | "Viewer listens on 127.0.0.1 only; her PDS refuses an app whose metadata it can't fetch over HTTPS." |
| 2 | Maintainer Jeff deploying. Priya and Dmitri as affected users. |
| 3 | bsky.social fetch; pds.volkov.dev fetch; 503 during deploy. |
| 4–5 | 3 scenarios → AC-000.1–.3. |
| 6 | 2 days, 3 scenarios. |
| 7 | OD-BRA-2, OD-BRA-4, `${app_origin}` single source. |
| 8 | DNS under jeffbailey.us (exists). |
| 9 | ≥99% reach the consent screen. Baseline 0%. |
| job_id | `infrastructure-only` with `infrastructure_rationale`. The slice has 4 non-infra stories. |

### US-BRA-001: Sign in

| Item | Evidence |
|------|----------|
| 1 | "Participating means CLI install, key management and app password, far too heavy." |
| 2 | Priya Raman, Bluesky dev, never used the CLI, first visit, wary. |
| 3 | Priya happy path; Dmitri on pds.volkov.dev; typo and cancel. |
| 4–5 | 5 scenarios → AC-001.1–.7. |
| 6 | 2–3 days. |
| 7 | OD-BRA-2, OD-BRA-7, NFR-BRA-3. |
| 8 | US-BRA-000. |
| 9 | ≥90% sign-in completion (KPI-BRA-9). |
| Pitch | After = `${app_origin}` → "Signed in as @priyaraman.bsky.social". Decision = continue to proof. |

### US-BRA-002: Prove GitHub

| Item | Evidence |
|------|----------|
| 1 | "Sam could type BurntSushi and publish claims about repos he didn't build." |
| 2 | Signed-in developer linking her own account. Sam as adversary. |
| 3 | Priya's bio with DID; Dmitri's different DID; Sam/BurntSushi and rate limit. |
| 4–5 | 6 scenarios → AC-002.1–.6. |
| 6 | 1–2 days. |
| 7 | I-SCR-2, OD-BRA-3, OD-BRA-10, GitHub numeric id. |
| 8 | US-BRA-001. |
| 9 | ≥70% verify; 0 unverified scans (KPI-BRA-5). |
| Pitch | After = `${app_origin}/github`, Copy, Verify → "Verified: …". |

### US-BRA-003: Private queue

| Item | Evidence |
|------|----------|
| 1 | "Anxious a machine's guesses could become public statements about her." |
| 2 | Verified developer reviewing her own work. |
| 3 | tidepool's 4 signals and quill-docs; Aisha with only forks; Dmitri requesting Priya's queue. |
| 4–5 | 5 scenarios → AC-003.1–.8 (incl. re-check, owner-only, no exposure). |
| 6 | 2–3 days (reuses the shipped pipeline). |
| 7 | Reuse of `scrape person` beats, OD-BRA-3/4/5. |
| 8 | US-BRA-002. Scraper exists. |
| 9 | ≥80% non-empty queue; KPI-BRA-4 = 0. |
| Pitch | After = `${app_origin}/review` cards. Decision = card-by-card truth. |

### US-BRA-004: Approve to own PDS

| Item | Evidence |
|------|----------|
| 1 | "Wants it on record in her own repo; wary of a write she can't see in advance." |
| 2 | Verified developer, first publish. |
| 3 | Priya at 2500; Dmitri's pds.volkov.dev; PDS unreachable. |
| 4–5 | 5 scenarios → AC-004.1–.7, incl. **AC-004.3 self-attested accepted**. |
| 6 | 3 days* (see note). |
| 7 | D-5 consequence on `verify.rs`; OD-BRA-1, OD-BRA-9. |
| 8 | US-BRA-003. OD-BRA-1 tracked. |
| 9 | KPI-BRA-1 ≥50%, KPI-BRA-2 ≤5 min. |
| Pitch | After = Approve → preview → "Publish to my repo" → at:// URI. |

### US-BRA-005 to US-BRA-012

All pass on the same evidence pattern: a named persona (Priya, Dmitri, Aisha, Maria), three
real-data examples, 3–6 scenarios, AC IDs mapped 1:1 in `acceptance-criteria.md`, technical
notes naming the ODs, tracked dependencies, and KPIs from `outcome-kpis.md`. Each Elevator
Pitch's After line names a user entry point: `${app_origin}/review` Edit or Not me,
`/@handle` profile, "Share on Bluesky…", `openlore peer pull` plus viewer `/peer-claims` plus
`openlore search`, "Scan again", Retract, or `/settings`.

## Gate result

**DoR: PASSED** for all 13 stories. US-BRA-004 carries a pre-agreed split path. No
unresolved red cards block DESIGN. The remaining open items are DESIGN decisions
(OD-BRA-1..12 in `requirements.md`).
