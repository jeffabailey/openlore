# Outcome KPIs: review-app-repos-and-accounts

## Feature: review-app-repos-and-accounts

### Objective

Within 30 days of each release, review-app owners get suggestions about the repos that really
show how they build (chosen repos, every account they own, and any public repo they want to
look at), while nothing about a repo they don't own ever leaves the app.

### Definitions

- **`outside_default`**: a suggestion flag set when its repo is one the pre-feature scan would
  not have read: an owned repo outside the first verified account's default top 5, any repo of
  a second or third account, or any imported repo. Used by KPI-RRA-1 and the story KPIs of
  US-RRA-002, 004 and 005 (each a subset of it).
- **Targets are hypotheses.** Baselines are 0% because the behaviours are impossible today.
  The 25/30/50/70% values follow the bluesky-claim-review-app bars (KPI-BRA-3: ≥20-30%; proof
  success ≥70%). Re-set them from the 2-week baseline after each release.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-RRA-1 (North Star) | Verified owners who scan after R1 | Act on (approve, edit or decline) ≥1 suggestion from a repo today's scan would not read (outside the default top 5, from a second account, or imported) | ≥30% within 30 days | 0% (impossible today) | Aggregate counter on actions over suggestions flagged `outside_default` / scanning owners | Leading |
| KPI-RRA-2 | Owners who start "Add another GitHub account" | Finish with the account verified | ≥70% | 0% (not possible) | `account.add.started` / `account.add.verified` | Leading |
| KPI-RRA-3 | Owners who import ≥1 repo | Act on ≥1 suggestion from an imported repo | ≥50% within 30 days | 0% | `import.added`, actions on imported suggestions | Leading |
| KPI-RRA-4 (guardrail) | All users | PDS writes, previews, profile entries or share posts of suggestions whose repo is not owned, at that request, by an account on the writer's list whose bio holds the writer's DID (BR-RRA-1/2) | **0** | n/a (no imports yet) | CI property over every publish request path (AC-006.3) + production audit: every PDS write has an `ownership.publish_check.passed` event in the same request | Guardrail |
| KPI-RRA-5 (guardrail) | All users | Repos read from an account whose bio check failed in that scan | **0** | 0 today (one account) | Scan-gate audit per account (extends KPI-BRA-5) | Guardrail |
| KPI-RRA-6 | Owners who import an org repo they administer | Publish ≥1 claim about it | ≥1 dogfood claim confirmed by a second reader, then ≥20% within 60 days | 0 | `publish.administered` | Leading (R4, deferred by the user 2026-10-19; not measured until US-RRA-007 is scheduled) |
| KPI-RRA-7 (guardrail) | All users | Scans that read a repo not chosen or imported, read more than `${scan_repo_cap}` repos, exceed 6 a day per DID, or re-offer a declined key (owned or imported) | **0** | 0 today | CI property on the scan's repo set + existing budget properties + suppression audit (extends KPI-BRA-4b) | Guardrail |

### Metric Hierarchy

- **North Star**: KPI-RRA-1.
- **Leading indicators**: share of owners who change the default choice (US-RRA-001: ≥25%),
  KPI-RRA-2, KPI-RRA-3, actions on second-account suggestions (US-RRA-004: ≥50%).
- **Guardrails (must not degrade)**: KPI-RRA-4, KPI-RRA-5, KPI-RRA-7, and the inherited
  KPI-BRA-4 (pending private), KPI-BRA-4b (declines private, not re-offered), KPI-BRA-5 (no
  scrape without current proof, now per account), KPI-GDL-1 (links only from the DID owner).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1, 2, 3, 6 | Review-app aggregate counters (`kpi_counters`) | New events, counts only: `repos.choice.viewed`, `repos.choice.changed_from_default`, `account.add.started`, `account.add.verified`, `account.removed`, `import.added`, `import.removed`, `import.refused.<code>`, `suggestion.acted.outside_default`, `publish.refused.not_owned`, `publish.administered`. No DID, handle, login or repo name (OD-BRA-11). | Weekly | Product owner |
| 4 | CI + production write audit | Property test over all publish request paths; daily query: PDS writes without a same-request ownership pass == 0 | Every build; daily | DELIVER / DEVOPS |
| 5 | Scan gate log | Count repo reads whose account check did not pass in that scan (expected 0) | Daily | DEVOPS |
| 7 | CI + scan run records | Property: repos read ⊆ choice ∪ imports and ≤ cap; existing `review_domain::budget` properties stay green | Every build | DELIVER |

### Hypothesis

We believe that letting owners choose their repos, add every GitHub account they own, and
import any public repo as private, unpublishable suggestions will make the queue reflect how
they actually build. We will know this is true when ≥30% of scanning owners act on a suggestion
from a repo the old top-5 scan would never have read, with zero claims about repos they don't
own reaching any PDS.

### DEVOPS handoff notes

- Add the new counter events to the existing `kpi_counters` table; keep the no-PII check-arch
  rule (data-models §3.3).
- Alert on any non-zero KPI-RRA-4, KPI-RRA-5 or KPI-RRA-7.
- GitHub budget: expect more requests per scan (one profile read per account, ≈10 requests per
  repo). Watch the share of scans that pause for the GitHub floor; collect a 2-week baseline
  after R1 and again after R2.
- Register KPI-RRA-1..7 in `docs/product/kpi-contracts.yaml` at DESIGN acceptance (house
  pattern for KPI-GDL/OOS).
