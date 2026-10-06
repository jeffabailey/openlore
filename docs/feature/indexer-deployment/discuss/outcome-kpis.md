# Outcome KPIs: indexer-deployment

Baseline for every KPI: the indexer runs nowhere today, so network search has 0 results and
there is no measurement.

| ID | Who | Does what | By how much | Measured by | Story |
|---|---|---|---|---|---|
| KPI-IXD-1 | Maria (searchers using `OPENLORE_INDEXER_URL`) | Gets network results from authors on more than one PDS | ≥ 2 distinct PDS hosts in results within 1 day of go-live. 100% of results attributed to their author. | Smoke search from the laptop and nightly; count distinct author PDS hosts | US-IXD-001 |
| KPI-IXD-2 | Authors (Priya) via Maria | See a newly published claim become searchable | ≤ 30 min from publish, 5/5 samples | Timed publish-to-search samples | US-IXD-002 |
| KPI-IXD-3 | The index (served to Maria) | Stays fresh | Last successful pass ≤ 30 min old in ≥ 98% of hourly checks over 30 days | Freshness command / pass logs | US-IXD-002, US-IXD-005 |
| KPI-IXD-4 | Jeff | Changes index coverage without a deploy | 100% of DID-list edits take effect on the next pass, with 0 deploys | `config.loaded.repo_did_count` versus edit time | US-IXD-003 |
| KPI-IXD-5 | Jeff | Learns of real failures, and only those | 100% of exit-2 passes and 2x-exit-3 runs alerted. 0 alerts for exit-0 passes. ≤ 2 alert emails/week in steady state. | CloudWatch alarm history versus pass exit log | US-IXD-004 |
| KPI-IXD-6 | Jeff (and every PDS user) | Ships and rolls back the indexer with no PDS impact | 0 PDS `_health` failures attributable to the indexer. Rollback ≤ 2 min. Search downtime ≤ 30 s per deploy. | Deploy logs, PDS health, A-1 history | US-IXD-006 |

Guardrails: added cost ≤ $2/month (NFR-IXD-6). Host `MemAvailable` > 128 MB during passes
(NFR-IXD-4).
