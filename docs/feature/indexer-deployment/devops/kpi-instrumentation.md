# KPI Instrumentation: indexer-deployment (DEVOPS)

> Instruments every KPI in `../discuss/outcome-kpis.md`. Baseline for all of them is "the indexer
> runs nowhere; 0 results; no measurement". Sources are structural logs (30-day retention), the
> host health line, alarm history, SSM parameter history, the host `releases` file, and the
> nightly `index-smoke`. No new binary event is needed beyond DESIGN's B4/B15
> (`pass_summary` per pass with `pass_id`; `repo_dids_age_secs` in `indexer.config.loaded`, which
> already carries `repo_did_count`). Note: `health.startup.refused` goes to **stderr**; dockerd's
> awslogs driver ships stderr too, so the filters see it.

## 1. KPI → collection → measurement

| KPI | Target | Data collection | Measurement (saved Logs Insights queries unless noted; the planned `kpi` mode of `deploy.sh` was not built) | Cadence |
|---|---|---|---|---|
| **KPI-IXD-1** network reach | ≥ 2 distinct PDS hosts in results within 1 day of go-live; 100% of rows attributed | Nightly `index-smoke` (ci-cd-pipeline §6): a fixed broad search, each distinct author DID resolved via PLC / `did:web` to its PDS endpoint | `hosts = count(distinct endpoint)`; `attributed = rows with non-empty author / rows`. Printed in the nightly job log; no laptop variant (the planned `kpi` mode was not built) | Nightly; first reading within 24 h of I-7 |
| **KPI-IXD-2** publish-to-search | ≤ 30 min, 5 of 5 samples | Manual timed sample: publish a claim from a test author on a listed PDS at T (record T); poll the public search for its subject every 60 s; record T_found | `T_found - T` per sample; recorded in the baseline doc | 5 samples in the first 2 weeks |
| **KPI-IXD-3** freshness | Last exit-0 pass ≤ 30 min old in ≥ 98% of hourly checks over 30 days | `pass_summary` lines (exit 0 timestamps) | The `indexer/kpi-freshness` saved query (not the `limit 8` status query) returns every exit-0 `@timestamp` in the window (≤ 2,880 rows, under the 10,000 limit); `jq` computes, for each hour boundary h, `h - max(ts ≤ h) ≤ 30 min`; ratio = good hours / hours | Weekly; 30-day baseline |
| **KPI-IXD-4** coverage changes without deploys | 100% of DID-list edits applied on the next pass; 0 deploys | `aws ssm get-parameter-history` (each version's `LastModifiedDate` and value); `indexer.config.loaded` (`repo_did_count`, `repo_dids_age_secs`); host `releases` | For each version v: the first `config.loaded` after v's timestamp has `repo_did_count = count(DIDs in v)` and is ≤ 15 min (+ pass duration) later; no `releases` line between v and that pass | Per edit; summarized weekly |
| **KPI-IXD-5** alert precision | 100% of exit-2 passes and 2×exit-3 runs alerted; 0 alerts on exit-0; ≤ 2 alert emails/week steady state | Alarm history (`aws cloudwatch describe-alarm-history --history-item-type StateUpdate`) for A1/A2/A3; `indexer/exit-codes` query | Join: every exit-2 pass has an A2 ALARM within 15 min; every exit-3 pair has an A1 ALARM; every A1/A2 ALARM maps to such a pass (A3 ALARMs map to a `not_live` cause line); count ALARM transitions per ISO week | Weekly |
| **KPI-IXD-6** safe ship / rollback | 0 PDS `_health` failures attributable to the indexer; rollback ≤ 2 min; search downtime ≤ 30 s per deploy | `deploy.sh` prints and the host `releases` line records `ready_s=<n>` (seconds from recreate to `/healthz` 200); the laptop poller records PDS `_health` results during each deploy; the rollback drill is timed (`time deploy.sh rollback`) | Per deploy: `ready_s ≤ 30`, PDS poll `n/n ok`; drill wall time ≤ 120 s; plus A3/PDS history over the window shows no PDS failure coinciding with an indexer event | Per deploy; reviewed monthly |

## 2. Guardrails

| Guardrail | Data | Check |
|---|---|---|
| Added cost ≤ $2/month (NFR-IXD-6) | Cost Explorer, service CloudWatch + Systems Manager, month after go-live versus month before | Delta ≤ $2 (expected about $1.00 list, likely $0 in free tier) |
| Host `MemAvailable` > 128 MB during passes (NFR-IXD-4) | `indexer.host.health.host_mem_available_mb` every 2 min | Daily min via the `indexer/host-health` query; any sample < 128 MB during a pass window → re-run the gate (infrastructure-integration §9.6). There is deliberately no memory alarm (3-alarm decision); a sustained breach ends in an OOM kill of the indexer, which A3 catches. |
| Search latency p95 ≤ 1 s (NFR-IXD-3) | `search_ms` of the canned health search | Weekly p95 |

## 3. Dashboards and reading cadence

- **Weekly (Jeff, 5 min):** the saved Logs Insights queries give KPI-IXD-3, -4, -5 and the guardrails with
  numerators and denominators (a `kpi` mode of `deploy.sh` was designed but not built). The nightly `index-smoke` log gives KPI-IXD-1.
- **Per deploy:** the `deploy.sh` summary gives KPI-IXD-6.
- **Dashboard:** none (saved queries suffice; see observability-design §5).
- **Baseline:** KPI-IXD-1 at day 1; KPI-IXD-2 after 5 samples; KPI-IXD-3/5 after 30 days;
  recorded in `docs/evolution/indexer-deployment-kpi-baseline.md` at DEVOPS close.
  Report numerators and denominators, not percentages, until 30 days have passed.

## 4. Privacy review

Every source is structural: repository DIDs (public), counts, exit codes, durations, timestamps.
The canned health search uses a fixed non-user value. No claim content and no user search
values are collected anywhere in this instrumentation.
