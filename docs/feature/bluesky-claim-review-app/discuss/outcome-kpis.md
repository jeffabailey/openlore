# Outcome KPIs: bluesky-claim-review-app

## Feature: bluesky-claim-review-app

### Objective

Within 60 days of launch, ordinary Bluesky developers publish accurate, consented,
self-attested philosophy claims into their own PDSes, without the CLI and without anything
leaking before they approve it.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-BRA-1 (North Star) | Verified users with ≥1 suggestion | Publish ≥1 claim in their first session | ≥50% | 0 (no path for non-CLI users) | Aggregate funnel counters | Leading |
| KPI-BRA-2 | Same | Time from sign-in to first published claim | Median ≤5 min, p90 ≤12 min | n/a (today: CLI setup, hours) | Timestamped aggregate events | Leading |
| KPI-BRA-3 | Publishers | Show real judgment: edit before approving, or decline | ≥20% of approvals edited, OR ≥1 decline in ≥30% of sessions with ≥4 suggestions | J-004 analogue: "edits ≥1 candidate" | Counters (edited flag, decline count) | Leading |
| KPI-BRA-4 (guardrail) | All users | Pending suggestions exposed outside the owner's view (PDS, post, profile, search, feed, other user) | **0** | n/a | Automated privacy tests (I-BRA-1) plus a write audit: every PDS write has a confirm event | Guardrail |
| KPI-BRA-4b (guardrail) | Reviewers | Declined suggestions re-offered or written publicly | **0** | n/a | Suppression audit plus a decline-path PDS-write audit | Guardrail |
| KPI-BRA-5 (guardrail) | All users | Scrapes run without the signed-in DID currently in the GitHub bio | **0** | n/a | Scan-gate audit log (re-check before every scrape) | Guardrail |
| KPI-BRA-6 (guardrail) | OpenLore readers | App-approved claims rejected or shown "unverified" | **0** (100% accepted as self-attested) | 100% would be rejected today | Acceptance tests plus an indexer ingest audit | Guardrail |
| KPI-BRA-7 | Users with ≥1 published claim | Share a post (opt-in) | ≥25%, with 0 posts lacking a confirm event | 0 | Counters | Leading |
| KPI-BRA-8 | Returning users (≥7 days) | Rescan and act on ≥1 new suggestion | ≥30% | n/a | Counters | Leading |
| KPI-BRA-9 | First-time visitors | Complete sign-in after starting it | ≥90% | 0 | Funnel counters | Leading |

### Metric Hierarchy

- **North Star:** KPI-BRA-1, the first-session publish rate.
- **Leading indicators:** KPI-BRA-9 (sign-in), US-BRA-002 proof success ≥70%, non-empty
  queue ≥80%, KPI-BRA-2, KPI-BRA-3.
- **Guardrails (must never degrade):** KPI-BRA-4, KPI-BRA-4b, KPI-BRA-5, KPI-BRA-6, existing
  app-signed verification unchanged (AC-009.4), and ≥99% monthly availability (NFR-BRA-9).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1, 2, 3, 7, 8, 9 | App server events | Aggregate counters with no claim content and no handle (OD-BRA-11) | Weekly | Product owner |
| 4, 4b | Test suite plus write audit | CI privacy tests. Production check: PDS writes == confirm events | Every build; daily in prod | DELIVER / DEVOPS |
| 5 | Scan gate log | Count scrapes lacking a passing re-check (expected 0) | Daily | DEVOPS |
| 6 | Acceptance tests, indexer ingest | Reject or unverified count for self-attested claims | Every build; daily | DELIVER / DEVOPS |

### Hypothesis

We believe that a consent-first web app (Bluesky sign-in, DID-in-bio proof, private review
and self-attested publishing to the user's own PDS) for Bluesky developers will turn
OpenLore from a CLI-operator tool into one where subjects speak for themselves.
We will know this is true when ≥50% of verified users with suggestions publish ≥1 claim in
their first session, with zero privacy guardrail breaches.

### DEVOPS handoff notes

- Instrument the funnel events listed above. Aggregate only, with no PII (OD-BRA-11).
- Alert on any non-zero KPI-BRA-4, 4b, 5 or 6.
- Collect a baseline from the first 2 weeks before tuning targets.
