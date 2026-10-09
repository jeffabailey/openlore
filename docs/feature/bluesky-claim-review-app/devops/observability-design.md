# Observability Design: bluesky-claim-review-app (DEVOPS)

> CloudWatch plus structured JSON logs, sized for < 50 users and a solo operator. Privacy
> invariants I-BRA-1..8 and NFR-BRA-2 govern every field. Alarms are in
> `monitoring-alerting.md`; KPIs are in `kpi-instrumentation.md`.

## 1. SLOs (defined first; everything else derives from them)

| SLO | SLI | Target | Window | Error budget |
|---|---|---|---|---|
| **Availability** (NFR-BRA-9) | Fraction of 1-minute Route 53 health-check samples where `/healthz` at the public origin is healthy | **99%** | Calendar month | ~7.3 h/month |
| **Readiness** | Fraction of 1-minute `host.health` lines with `ready = 1` (Logs Insights, reviewed monthly; no alarm in v1) | 99% | Month | ~7.3 h |
| **Publish latency** (NFR-BRA-4) | `publish.confirm` events with `duration_ms <= 3000` / all | 90% | Weekly review | 10% |
| **Scan latency** (NFR-BRA-4) | `scan.completed` with `repos <= 10` and `duration_ms <= 60000` / all such scans | 90% | Weekly review | 10% |
| **Privacy guardrails** (KPI-BRA-4/4b/5/6) | `guardrail.breach` events | **0** | Always | none; any event pages |

Burn-rate alerting is not used: at < 50 users and a 7 h budget, a few single-window threshold
alarms (essentials only, monitoring-alerting §1) are sharper and cheaper. Latency SLOs are reviewed weekly
with Logs Insights; they do not alarm. Revisit at more than 500 users.

## 2. Signals and sources

| Signal | Source | Transport | Cost driver |
|---|---|---|---|
| External reachability (DNS, TLS handshake, Caddy, app; the certificate is **not** validated by Route 53) | Route 53 HTTPS health check on `app.openlore.jeffbailey.us/healthz` | `AWS/Route53 HealthCheckStatus` | 1 health check |
| Readiness, container up, memory, OOM and restarts, host memory | Host timer `review-app-health` (1/min) | One JSON `host.health` line per run, written with `aws logs put-log-events` to stream `host-health` in `/openlore/prod/review-app`. **No `PutMetricData`.** | log ingestion only (~10 MB/month) |
| CPU credits | EC2 basic monitoring | `AWS/EC2 CPUCreditBalance` (no alarm in v1) | free |
| App events (RED, audits, KPI rollups) | App stdout JSON | dockerd `awslogs` to `/openlore/prod/review-app` | ingestion < 100 MB/month |
| App down or restarted, guardrail breaches, token expiry | Log metric filters | `OpenLore/ReviewApp` (`AppDown`, `GuardrailBreaches`, `GithubTokenExpiring`) | 3 filter metrics, billed only for hours with data |
| Startup refusal, readiness, memory | Log lines only (`health.startup.refused`, `host.health`) | none until an alarm is added (monitoring-alerting §1.1) | — |
| Identity backup | existing `PDS/Backup` | unchanged | — |

There is no distributed tracing: one process, and the outbound calls are logged as span events
with durations.

## 3. Structured log contract (privacy-first)

`tracing-subscriber` JSON with **flattened** event fields, so CloudWatch metric filters can
match `$.event`. Every line has:

| Field | Type | Notes |
|---|---|---|
| `ts` | RFC 3339 UTC | |
| `level` | `INFO`, `WARN` or `ERROR` | DEBUG is never enabled in production |
| `event` | dotted name from a **closed set** (below) | Required. Metric filters key on it. |
| `req_id` | 128-bit random hex | Correlates one request. It is not a user identifier. |
| `route` | the route template (`/suggestions/{id}/preview`), **never the raw path** | Raw paths can carry handles (`/@{handle}`) |
| `status`, `duration_ms` | HTTP result | RED |
| `owner` | `hmac_sha256(log_salt, did)[:12]` | The only per-user field. Pseudonymous; unlinkable without the SSM salt. |
| `outcome` / `reason` | closed enum codes (`ok`, `rate_limited`, `did_missing`, `pds_unreachable`, ...) | Verdict codes, never free text from external systems |
| counts | `repos`, `new`, `published`, `declined`, `rows` | Integers only |

**Never logged** (enforced by the `review_app_log_field_allowlist` check-arch rule and by an
acceptance test that captures logs during a full journey and asserts that no fixture secret
or content string appears):

- tokens, codes, the DPoP or client JWK, cookies or their hashes, CSRF tokens;
- GitHub bio text (only the verdict code), login names, repo names;
- suggestion content: subject, predicate, object, evidence, signals, confidence;
- post text; handles; raw DIDs; raw request paths or query strings; request or response bodies;
- error `Display` output from external crates. These can echo URLs carrying `code=`, so the
  shell maps them to `reason` codes at the adapter boundary, and the raw error goes nowhere.

Caddy's access log is off by default in the module, which keeps it that way. The `/@handle`
paths in Caddy logs would otherwise be a leak vector.

### 3.1 Event catalogue (closed set; DELIVER may add, the reviewer checks against the allowlist)

| Group | Events |
|---|---|
| Lifecycle | `app.start{version,sha}`, `health.startup.refused{probe,arm}`, `health.probe.soft_fail{probe,arm}`, `app.ready`, `app.shutdown{interrupted_scans}`, `secrets.rekeyed{rows}`, `sessions.invalidated{count}`, `schema.migrated{from,to}` |
| HTTP | `http.request{route,method,status,duration_ms}` (sampled 100% at this scale) |
| Auth | `signin.started`, `signin.completed{new_account}`, `signin.denied`, `signin.failed{reason}`, `signin.callback_panic_isolated` (SPIKE-2 finding 3), `signout`, `disconnect{revoke_outcome}` |
| Ownership and scan | `github.verify{outcome}`, `scan.gate{outcome}`, `scan.repos_listed{repos}`, `scan.repo_done{new}`, `scan.completed{repos,new,duration_ms}`, `scan.rate_limited{resume_after_s}`, `scan.interrupted` |
| Review | `suggestion.approved{edited}`, `suggestion.declined`, `suggestion.undecline` |
| Writes | `plan.created{kind}`, `plan.confirmed{kind}`, `pds.write{kind,outcome,duration_ms}`, `publish.readback{provenance,cid_match}`, `share.posted`, `retract.posted` |
| Upstream | `upstream.call{dep=github\|pds\|plc\|authz,outcome,duration_ms}`, `github.ratelimit{remaining}`, `github.token.expiring{days_left}` |
| Audit and KPI | `kpi.rollup{day,counters}`, `guardrail.breach{kpi,check,count}` |

## 4. Host health timer (`review-app-health`, 1/min)

A shell script on the data volume (`/pds/app/bin/review-app-health.sh`) with a systemd timer
(`OnCalendar=*-*-* *:*:00`, `AccuracySec=5s`). It runs as root on the host and makes one
`aws logs put-log-events` call per run to the stream `host-health` (created by the deploy if it
is missing). It publishes **no custom metrics**. Alarms derive from these lines through metric
filters, so a new alarm needs no host change.

Line shape: `{"event":"host.health","ts":...,"app_running":0|1,"ready":0|1,"restarts":N,"oom_killed":0|1,"app_mem_pct":N,"host_mem_available_mb":N,"swap_in_kb":N,"pds_free_mb":N}`

| Field | Computation | Used by |
|---|---|---|
| `app_running` | `docker inspect -f '{{.State.Running}}'` for the app container | **A-3** |
| `restarts` | Delta of `.RestartCount` since the last run, **plus** 1 if `.State.OOMKilled` flipped to true (state kept in `/run/review-app-health.state`) | **A-3** |
| `ready` | 1 if `curl -fsS -m 5 --resolve app.openlore.jeffbailey.us:443:127.0.0.1 https://app.openlore.jeffbailey.us/readyz` succeeds, else 0 | Readiness SLI; deferred A-2 |
| `app_mem_pct` | `docker stats --no-stream --format '{{.MemPerc}}'` | R-5 re-measure; deferred A-4 |
| `host_mem_available_mb`, `swap_in_kb` | `/proc/meminfo` `MemAvailable`; `vmstat` `si` over the last minute | R-5 re-measure; deferred A-5 |
| `pds_free_mb` | `df -m /pds` | capacity watch |

Host down: no lines arrive, and A-1 (external, missing data counts as breaching) pages. That is
why A-3 treats missing data as not breaching.

## 5. RED and USE views (Logs Insights saved queries, no dashboard cost)

Saved queries, created by tofu as `aws_cloudwatch_query_definition`, which is free:

1. `review-app/red`: `filter event="http.request" | stats count(), sum(status>=500)/count()*100 as err_pct, pct(duration_ms,50), pct(duration_ms,90), pct(duration_ms,99) by bin(1h), route`
2. `review-app/upstreams`: `filter event="upstream.call" | stats count(), sum(outcome!="ok"), pct(duration_ms,90) by dep, bin(1h)`
3. `review-app/slo-latency`: the publish (<= 3 s) and scan (<= 60 s at <= 10 repos) ratios, weekly
4. `review-app/startup`: `filter event like /^(app\.|health\.|schema\.|secrets\.|sessions\.)/`
5. `review-app/kpi-weekly`: see `kpi-instrumentation.md` §5
6. `review-app/guardrails`: `filter event="guardrail.breach"`

One optional CloudWatch dashboard, `openlore-review-app`, costs $3/month beyond the free tier
of 3 dashboards. The account has none today, so it is **free**. It holds:

- HealthCheckStatus;
- a Logs Insights widget over `host.health` (`ready`, `app_mem_pct`, `host_mem_available_mb`, `restarts`);
- AppDown;
- CPUCreditBalance;
- a Logs Insights widget for RED;
- the guardrail count.

## 6. Retention

| Data | Retention | Rationale |
|---|---|---|
| `/openlore/prod/review-app` log group (app events and `host-health` stream) | **30 days** (user decision 2026-10-04) | Minimizes the retention of pseudonymous data. KPI windows longer than 30 days, such as the 60-day objective and the 4-week baseline, are read from `kpi_counters` (indefinite, aggregate only) through the admin endpoint (`GET /admin/kpi`, kpi-instrumentation §4). |
| Log-filter metrics | CloudWatch default (15 months, downsampled) | No PII |
| `kpi_counters` (DuckDB) | Indefinite, aggregate only | ADR-074 |
| Docker local log cache | Default dual-logging cache (5 × 20 MB) on the root volume | Lost on replacement; acceptable |

## 7. What is deliberately not observed

- Per-user activity timelines. Only aggregate counters and pseudonymous `owner` hashes exist,
  so there is no product analytics on individuals (OD-BRA-11).
- Profile page visitors beyond `http.request` on the `/@{handle}` route template. The handle is
  never logged.
- GitHub bio contents, even on a verify failure (only the verdict code).
