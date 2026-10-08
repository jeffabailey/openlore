# Observability Design: indexer-deployment (DEVOPS)

> CloudWatch Logs plus structured JSON lines, sized for a solo operator and < 50 searchers. No
> claim content and no search values are ever logged (WD-105, I-IXD-4). Alarms are in
> `monitoring-alerting.md`; KPIs in `kpi-instrumentation.md`.

## 1. SLOs (defined first)

| SLO | SLI | Target | Window | Error budget | Alerting |
|---|---|---|---|---|---|
| **Freshness** (KPI-IXD-3) | Fraction of hourly checks where the last exit-0 `pass_summary` is ≤ 30 min old | ≥ 98% | 30 days | about 14 hourly checks/month | A1/A2 catch the causes; A3 catches a stopped pipeline. Reviewed weekly with `deploy.sh kpi`. |
| **Search availability** | Fraction of 2-min `indexer.host.health` samples with `healthz_ok = 1` | ≥ 99% | Calendar month | about 7.3 h | A3 (10 min of failure) |
| **Search latency** (NFR-IXD-3) | `search_ms` of the health timer's canned search, p95 | ≤ 1000 ms | Weekly review | 5% of samples | None in v1 (review only) |
| **Pass integrity** | `pass_summary` with exit 2 | 0 | Always | none: any exit 2 pages | A2 |
| **Total outage** | 2 consecutive exit-3 passes | 0 | Always | one isolated exit 3 is tolerated | A1 |

Burn-rate alerting is not used: at this scale, three single-purpose alarms are sharper and
cheaper. Revisit above 500 users.

## 2. Signals and sources

| Signal | Source | Transport | Used by |
|---|---|---|---|
| Pass outcomes, skips, purges, config loads | `serve` stdout JSON (`indexer.ingest.*`, `indexer.config.loaded`) | dockerd awslogs → `/openlore/prod/indexer`, stream `indexer` | A1, A2, freshness, KPIs |
| Startup refusal, store faults | `health.startup.refused`, `indexer.store.unusable` | same | A2 |
| Liveness, memory, latency sample | Host timer `openlore-indexer-health` every 2 min (§4) | `aws logs put-log-events` → stream `host-health` | A3, availability and latency SLIs, memory guardrail |
| DID render failures | `render-dids.sh` (`indexer.dids.render_failed`) | journal + stream `host-dids` (best effort) | triage; staleness itself pages through A3 |
| Trigger client results | `docker exec … trigger` stdout/stderr | systemd journal only | triage (`deploy.sh host-status`) |
| CPU credits | `AWS/EC2 CPUCreditBalance` (free) | — | re-measure gate, no alarm |

No tracing: one process; per-DID outcomes and durations are already events.

## 3. Log contract

The event set is data-models §4 (closed; every pass event carries `pass_id`; exactly one
`indexer.ingest.pass_summary` per pass, last, for every exit code). Flattened JSON with a
top-level `event` field, so metric filters match `$.event`. Structural fields only: DIDs of
repositories (public), counts, durations, exit codes, closed-enum causes. Never: claim
subject/object/evidence, search `value`, request bodies, raw upstream error text. A log-capture
acceptance test (DISTILL) runs a pass and a search with a sentinel value and asserts the sentinel
appears in no line.

## 4. Host health timer (`indexer-health.sh`, every 2 min)

Runs as root on the host. One JSON line per run to the journal and to stream `host-health`:

```json
{"event":"indexer.host.health","ts":"…","running":1,"healthz_ok":1,"search_ok":1,"search_ms":42,
 "search_status":200,"summary_45m":1,"summary_check":"ok","dids_age_s":310,"restarts":0,"oom_killed":0,
 "indexer_mem_mb":61,"host_mem_available_mb":243,"swap_in_kb":0,"pds_free_mb":3810,"not_live":0}
```

| Field | Computation |
|---|---|
| `running`, `restarts`, `oom_killed`, `indexer_mem_mb` | `docker inspect` / `docker stats` on `openlore-indexer` (restart deltas kept in `/pds/indexer/state/health.state`, as the review-app timer does) |
| `healthz_ok` | `curl -fsS -m 5 --resolve index.openlore.jeffbailey.us:443:127.0.0.1 https://index.openlore.jeffbailey.us/healthz` returns 200 with `"status":"ok"` (a 503 for an unusable store is a failure) |
| `search_ok`, `search_ms` | `POST` of a fixed canned body (`/pds/indexer/bin/search-probe.json`, no user data) through the same route; `search_ok` is 1 only for a 2xx answer, `search_ms` its `time_total` |
| `search_status` | that search's HTTP status (curl without `-f`, so a 500 is still read); 0 when there was no HTTP answer (no connection or a timeout) |
| `summary_45m`, `summary_check` | `aws logs filter-log-events --log-group-name /openlore/prod/indexer --filter-pattern '{ $.event = "indexer.ingest.pass_summary" }' --start-time <now-45m> --max-items 1`. 1 if any event; `summary_check = error` if the API call fails (then `summary_45m = 0`, fail closed). Because it reads the **shipped** log, it also proves the awslogs pipeline works end to end. |
| `dids_age_s` | `now - mtime(/pds/indexer/config/.rendered-at)`; a missing file counts as infinitely old |
| `host_mem_available_mb`, `swap_in_kb`, `pds_free_mb` | `/proc/meminfo`, `/proc/vmstat` delta, `df -Pm /pds` |
| **`not_live`** | **1 if** `running = 0` **or** `healthz_ok = 0` **or** `summary_45m = 0` (which includes `summary_check = error`) **or** `dids_age_s > 7200` **or** `search_status = 500` (the index could not serve the canned search; the binary logs `indexer.search.store_error`); else 0. A 503 (busy during a pass or purge), 429, 408 or 0 (no answer) makes `search_ok = 0` but does **not** count. This is the single A3 input. The bats test asserts `not_live = 1` for a failing `FilterLogEvents`. |

Hung-process guard (mirrors the review-app timer): if the container is running but `/healthz`
has failed 3 runs in a row (6 min), `docker restart openlore-indexer` and count the restart.

If the host is down, or the timer itself dies, no lines arrive: A3 treats missing data as
breaching, so that pages too.

A bats test with a fake `docker`, `curl` and `aws` covers each `not_live` cause, including the
stale-list and missing-heartbeat paths that are not test-fired live.

## 5. Saved queries (tofu `aws_cloudwatch_query_definition`, free)

| Name | Query (Logs Insights) | Used by |
|---|---|---|
| `indexer/freshness` | `filter event = "indexer.ingest.pass_summary" and pass_id not like /^TEST/ \| fields @timestamp, exit_code, configured, own_pds, fallback, skipped, purged_authors, duration_ms, pass_id \| sort @timestamp desc \| limit 8` | `deploy.sh status` (last 8 exit codes, age of last pass of any kind) |
| (same, `and exit_code = 0 … limit 1`) | last successful pass time, age, counts | `deploy.sh status` |
| `indexer/skips` | `filter event in ["indexer.ingest.source_skipped", "indexer.ingest.author_purged", "indexer.ingest.pass_refused"] and pass_id = "<id>" \| fields event, did, reason, cause, claims_removed` | `deploy.sh status`, purge audit |
| `indexer/kpi-freshness` | `filter event = "indexer.ingest.pass_summary" and exit_code = 0 and pass_id not like /^TEST/ \| fields @timestamp \| sort @timestamp asc \| limit 10000` over the full 30 days (no small limit; ≤ 2,880 rows) | KPI-IXD-3 (`deploy.sh kpi` computes hourly freshness in `jq`) |
| `indexer/exit-codes` | `filter event = "indexer.ingest.pass_summary" and pass_id not like /^TEST/ \| stats count() by exit_code, bin(1d)` | KPI-IXD-5 |
| (ad hoc) `indexer/host-health` | `filter event = "indexer.host.health" \| stats min(host_mem_available_mb), max(indexer_mem_mb), pct(search_ms, 95), avg(healthz_ok) by bin(1h)` | availability and latency SLIs, memory guardrail |

`deploy.sh status` starts the three queries in parallel and polls; each scans at most a few MB,
so it answers in ≤ 10 s (AC-005.4) without a host shell.

No CloudWatch dashboard: the saved queries and `deploy.sh status` cover a solo operator. (A
dashboard would be free under the 3-dashboard tier if wanted later.)

## 6. Retention

| Data | Retention | Rationale |
|---|---|---|
| `/openlore/prod/indexer` (pass events, host-health, host-dids) | **30 days** | Same as the review app. Structural data only. KPI-IXD-3's 30-day window fits exactly; longer history is recorded in the baseline doc. |
| Log-filter metrics | CloudWatch default (15 months) | Alarm history for KPI-IXD-5 |
| The index itself | Not backed up (WD-IXD-7) | Rebuildable from authors' PDSes |
| systemd journal (trigger results) | Host default; lost on replacement | Triage only |
