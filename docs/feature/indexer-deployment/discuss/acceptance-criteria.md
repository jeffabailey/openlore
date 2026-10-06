# Acceptance criteria: indexer-deployment

> These are derived from the UAT scenarios in `user-stories.md`, which is the source of truth.
> This file collects the AC IDs for DISTILL.

| AC | Criterion | Traces |
|---|---|---|
| AC-001.1 | Search via the public URL returns claims from ≥ 2 distinct PDS hosts, each attributed to its author | FR-IXD-1, KPI-IXD-1 |
| AC-001.2 | Publicly trusted TLS. HTTP redirects to HTTPS. | FR-IXD-1 |
| AC-001.3 | Only search (plus an optional minimal health response) is reachable. Writes and other paths are refused, and the index is unchanged. | FR-IXD-2 |
| AC-001.4 | Before the first pass, search returns empty, with no 5xx | FR-IXD-1 |
| AC-001.5 | PDS `_health` and review-app `/healthz` stay 200 through the first deploy | NFR-IXD-9 |
| AC-001.6 | The CI-built, signature-verified digest is deployed, and the containers meet C-1 | FR-IXD-11, NFR-IXD-8 |
| AC-002.1 | A claim published at T is searchable by T+30 min | NFR-IXD-1, KPI-IXD-2 |
| AC-002.2 | Search during a pass answers in ≤ 1 s with no error | FR-IXD-5, NFR-IXD-3 |
| AC-002.3 | A pass never exits 2 because search holds the store | FR-IXD-5 |
| AC-002.4 | At most one pass runs at a time | FR-IXD-4 |
| AC-002.5 | Passes resume unattended after a reboot, or a replacement plus redeploy | FR-IXD-3 |
| AC-003.1 | A DID-list edit takes effect on the next pass, with no deploy or restart | FR-IXD-6, KPI-IXD-4 |
| AC-003.2 | A removed DID is not listed, and its indexed claims remain | FR-IXD-6 |
| AC-003.3 | A malformed list gives exit 2 naming the entry, and the old index stays searchable | FR-IXD-8 |
| AC-003.4 | A read failure uses the last good list, never an empty one | FR-IXD-7 |
| AC-003.5 | No AWS credentials in the container | NFR-IXD-8 |
| AC-004.1 | 2 consecutive exit-3 passes give exactly one alarm. A single exit 3 gives none. | FR-IXD-9 |
| AC-004.2 | Any exit 2 alarms within 15 min of the pass ending | FR-IXD-9 |
| AC-004.3 | Exit 0, with or without skips, never alarms | FR-IXD-9, KPI-IXD-5 |
| AC-004.4 | An OK notification on recovery. The existing SNS topic, with no new subscription. | FR-IXD-9 |
| AC-004.5 | Each alarm was test-fired and returned to OK before go-live | FR-IXD-9 |
| AC-004.6 | The added alarm cost is ≤ $0.30/month | NFR-IXD-6 |
| AC-005.1 | One laptop command shows the last successful pass time, its age and counts | FR-IXD-10 |
| AC-005.2 | It shows the skipped DIDs with reasons and the recent exit codes | FR-IXD-10 |
| AC-005.3 | It shows the age of the last pass of any kind | FR-IXD-10, R-IXD-2 |
| AC-005.4 | It answers in ≤ 10 s, with no interactive host shell and no claim content | FR-IXD-10, I-IXD-4 |
| AC-006.1 | Only a signed CI digest is deployed, and it is pinned across reboots | FR-IXD-11 |
| AC-006.2 | Automatic rollback when not ready. Manual rollback is one command with no data restore. | FR-IXD-11, C-5 |
| AC-006.3 | ≤ 30 s of search downtime per deploy. PDS and review app at 200 throughout. | NFR-IXD-2, NFR-IXD-9 |
| AC-006.4 | The memory gate passed, or t4g.small was adopted, before go-live | NFR-IXD-4 |
| AC-006.5 | C-1 is met, and an indexer process is killed before the PDS under memory pressure | NFR-IXD-4, NFR-IXD-8 |

Cross-cutting NFR checks for DEVOPS: NFR-IXD-5 (24 h CPU credits), NFR-IXD-6 (cost table),
NFR-IXD-7 (a 100-request burst).
