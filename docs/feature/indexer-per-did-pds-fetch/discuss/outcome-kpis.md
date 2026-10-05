# Outcome KPIs: indexer-per-did-pds-fetch

The KPIs are measured from the indexer's existing structural stdout events (WD-105, no claim
content), plus acceptance fixtures where noted. Baselines are taken from the current
production indexer before the upgrade.

| KPI | Who | Does what | Target | Measured by | Baseline | Story |
|---|---|---|---|---|---|---|
| KPI-IPF-1 (north star) | Authors with self-attested claims on their own PDS (e.g. bsky.social review-app users) | Become discoverable in `openlore search` | 100% of configured, resolvable and reachable authors indexed within 1 pass after the upgrade | Self-attested `verified` count per pass. Distinct self-attested authors in the index vs configured authors on non-fallback PDSes | 0 (all refused as relay-origin) | 001 |
| KPI-IPF-2 (guardrail) | Own-PDS self-attested records | Are refused for provenance | 0 refusals for records read from the author's resolved PDS | `indexer.ingest.rejected.by_reason.provenance` split by own-PDS / fallback | All such records refused today | 001, 003 |
| KPI-IPF-3 | Indexer passes | Complete despite individual DID failures | 0 passes aborted by a single DID or PDS failure. 100% of skips carry a reason | Pass exit codes. `source_skipped` events vs the summary's `skipped` | Any single listing failure aborts (exit 2) | 002 |
| KPI-IPF-4 (guardrail) | Self-attested claims read through the fallback | Are admitted | 0 | `provenance` refusals for fallback-read DIDs. Index audit of self-attested rows whose DID was fallback-read | n/a (new path) | 003 |
| KPI-IPF-5 | Searchers seeing self-attested results | See the correct provenance label | 100% labeled, 0 mislabeled | Mixed-provenance acceptance fixture. Periodic spot-check of `searchClaims` output | 0% (field absent) | 005 |
| KPI-IPF-6 (guardrail) | Existing single-source deployments | See unchanged search results | 0 diffs for existing data (apart from the label) | Before/after search fixture | n/a | 004 |
| KPI-IPF-7 (latency) | Priya, after approving a claim | Is found by Maria | At most 1 ingest interval plus pass duration | Pass timestamps against the claim's `composedAt` | Never (refused) | 001, 002 |

Leading indicator: the `own_pds` share of `configured` in the pass summary. Lagging
indicator: J-005 discovery (unfollowed authors surfaced per search session, KPI-AV-1 from
openlore-appview-search) should rise as review-app authors enter the index.
