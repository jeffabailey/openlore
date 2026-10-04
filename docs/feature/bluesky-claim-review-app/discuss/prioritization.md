# Prioritization: bluesky-claim-review-app

Formula: Value × Urgency / Effort (each 1–5). Tie-break: Walking Skeleton > Riskiest
Assumption > Highest Value.

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|---------------|-----|-----------|
| 1 | Walking Skeleton (000–004) | A Bluesky user publishes one consented, self-attested claim into their own PDS | KPI-BRA-1, KPI-BRA-2, guardrails KPI-BRA-4/5/6 | Validates the riskiest assumptions (hosted OAuth, PDS write, repo-signed provenance accepted) |
| 2 | R1 Consent with control, then show it off (005–008) | Users judge suggestions rather than rubber-stamp them, and opt in to share | KPI-BRA-3, KPI-BRA-7 | Highest remaining value (O1/O4/O5/O7); share post is in v1 by user decision D-11 |
| 3 | R2 Trust that lasts (009–012) | Claims read correctly everywhere; users return, retract or leave cleanly | KPI-BRA-6 (surfaces), KPI-BRA-8 | Lower urgency; split candidate |

## Backlog

| Story | Title | Release | V | U | E | Score | MoSCoW | Outcome link | Dependencies |
|-------|-------|---------|---|---|---|-------|--------|--------------|--------------|
| US-BRA-000 | Public hosted origin (@infrastructure) | WS | 4 | 5 | 3 | 6.7 | Must | enables KPI-BRA-1 | deploy/ (existing PDS infra knowledge); OD-BRA-2 |
| US-BRA-001 | Sign in with my Bluesky handle | WS | 5 | 5 | 3 | 8.3 | Must | KPI-BRA-1, KPI-BRA-9 | 000 |
| US-BRA-002 | Prove my GitHub with my DID in my bio | WS | 5 | 5 | 2 | 12.5 | Must | KPI-BRA-5 | 001 |
| US-BRA-003 | See my private suggestion queue | WS | 5 | 5 | 3 | 8.3 | Must | KPI-BRA-1, KPI-BRA-4 | 002; scrape person pipeline (exists) |
| US-BRA-004 | Approve into my own PDS, accepted as self-attested | WS | 5 | 5 | 4 | 6.3 | Must | KPI-BRA-1/2/6 | 003; verify.rs change (D-5) |
| US-BRA-005 | Edit confidence or swap philosophy before approving | R1 | 4 | 4 | 2 | 8.0 | Must | KPI-BRA-3 | 004; philosophy vocabulary (J-002e, exists) |
| US-BRA-006 | Decline privately, never re-suggested | R1 | 5 | 4 | 2 | 10.0 | Must | KPI-BRA-3, KPI-BRA-4 | 003 |
| US-BRA-007 | My profile of approved claims | R1 | 4 | 4 | 3 | 5.3 | Must | KPI-BRA-7 | 004 |
| US-BRA-008 | Opt-in share post with preview | R1 | 4 | 4 | 2 | 8.0 | Must (D-11) | KPI-BRA-7 | 007 |
| US-BRA-009 | Self-attested recognised in viewer, search, peer pull | R2 | 4 | 3 | 3 | 4.0 | Should | KPI-BRA-6 | 004 |
| US-BRA-010 | Rescan offers only new suggestions | R2 | 3 | 2 | 2 | 3.0 | Should | KPI-BRA-8 | 003, 006 |
| US-BRA-011 | Retract a claim I published via the app | R2 | 4 | 3 | 2 | 6.0 | Should | KPI-BRA-3 (trust) | 004 |
| US-BRA-012 | Disconnect and forget me | R2 | 4 | 3 | 2 | 6.0 | Should | guardrail (privacy) | 001–006 |

Within R2, US-BRA-011 and 012 outscore 009, but 009 runs first because it removes the
"second-class claim" risk (A6). That is riskiest-assumption ordering applied inside the
release.
