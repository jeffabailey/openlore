# Monitoring and Alerting: indexer-deployment (DEVOPS)

> **Exactly three alarms** (user decisions 2026-10-06), all on the **existing** SNS topic
> `openlore-pds-backup-alarm` (`module.pds.backup_alarm_topic_arn`), with `ok_actions` so
> recoveries are notified too. No new topic or subscription. Defined in
> `deploy/tofu/environments/prod/indexer.tf`; `indexer_alarms_enabled` (default `false`) sets
> `actions_enabled` until rollout step I-6. Namespace `OpenLore/Indexer`. Log group
> `/openlore/prod/indexer`.

## 1. Alarm catalogue

### Metric filters (4 filters, 3 metrics)

| Filter | Pattern | Metric | Value |
|---|---|---|---|
| `indexer-pass-outage` | `{ $.event = "indexer.ingest.pass_summary" && $.exit_code = 3 }` | `IndexerPassOutage` | `1` |
| `indexer-pass-not-outage` | `{ $.event = "indexer.ingest.pass_summary" && $.exit_code != 3 }` | `IndexerPassOutage` (same metric) | `0` |
| `indexer-failure` | `{ ($.event = "indexer.ingest.pass_summary" && $.exit_code = 2) \|\| $.event = "health.startup.refused" \|\| $.event = "indexer.store.unusable" }` | `IndexerFailure` | `1` |
| `indexer-not-live` | `{ $.event = "indexer.host.health" }` | `IndexerNotLive` | `$.not_live` (0 or 1) |

No `default_value` on any filter. A `default_value` is published for every ingested batch that does
not match, so other lines in a period (skips, config loads) would add 0s and the period's Minimum
could never reach 1: A1 would never fire. Consequently a period with **no** `pass_summary` has **no
data point**; A1 treats it as missing → `notBreaching`, so A1 breaches only when both evaluated
periods have data and both Minimums are 1. Gaps in passes are A3's job, not A1's.

### Alarms

| ID | Name | Metric | Statistic / period | Condition | Missing data | Meaning |
|---|---|---|---|---|---|---|
| **A1** | `openlore-indexer-total-outage` | `IndexerPassOutage` | Minimum / 900 s | ≥ 1 for **2 of 2** periods | notBreaching | Two consecutive passes skipped every DID |
| **A2** | `openlore-indexer-pass-failed` | `IndexerFailure` | Sum / 300 s | ≥ 1 for 1 of 1 | notBreaching | Any exit-2 pass, a refused start, or an unusable store |
| **A3** | `openlore-indexer-not-live` | `IndexerNotLive` | Maximum / 300 s | ≥ 1 for **2 of 2** periods | **breaching** | No `pass_summary` for 45 min, **or** public `/healthz` failing, **or** DID list stale > 2 h, **or** the search probe answers 500, **or** container down, **or** no health lines at all |

Every `alarm_description` carries a runbook pointer, for example "A3: check `deploy.sh status`
and the `not_live` cause fields; then monitoring-alerting.md §2.3".

### Why these work (threshold rationale)

- **A1, "2 consecutive exit 3":** passes start at :00/:15/:30/:45 and CloudWatch aligns 900 s
  periods to the same quarter hours, so each period holds one pass's summary. Exit 3 publishes 1,
  any other exit publishes 0, so a period's **Minimum** is 1 only if every pass in it was a total
  outage. Two consecutive periods at 1 means two consecutive exit-3 passes. A single exit 3 gives
  one breaching period and no alarm (AC-004.1); an exit 0 or 2 between two 3s resets it. Exit 0,
  with or without skips, can never breach (AC-004.3). Edge: if a total-outage pass ran longer
  than 15 min, its period would be empty and the streak would restart; total-outage passes are
  short (every DID fails within its 30 s timeout, about 6 min at 50 DIDs), and A3 bounds the worst
  case.
- **A2, "any exit 2":** 5-minute Sum; the alarm fires 5-10 min after the pass ends (AC-004.2,
  ≤ 15 min). It also catches a crash-looping `serve` (`health.startup.refused` on each restart)
  and a poisoned store. It returns to OK after one clean period, and the email says so (AC-004.4).
- **A3, liveness (all three user conditions folded into one alarm):** the host line computes
  `not_live` from every cause, so one cheap single-metric alarm serves all of them. Its causes:
  container not running, `/healthz` not ok, no shipped `pass_summary` in 45 min, a DID list older
  than 2 h, or the canned search answering **500** (`search_status = 500`: the index cannot serve
  searches). A search answering 503 (busy during a pass or purge), 429, 408 or nothing at all does
  **not** count, so a busy pass never pages. Two 5-minute
  periods (about 10 min) ride out a deploy's ≤ 30 s gap (at most one failing 2-minute sample) and
  a single transient API error. Missing data is breaching, so a dead host, a dead health timer,
  or broken log shipping from the host all page. The 45-minute heartbeat reads the **shipped**
  `pass_summary`, so a broken awslogs pipeline (which would blind A1 and A2) pages here too.
- A deliberate `deploy.sh stop` fires A3 if alarms are enabled. That is expected during
  maintenance (same posture as the review app's A-3).

### Cost

3 single-metric standard alarms = **$0.30/month** (AC-004.6). Filter metrics and logs bring the
feature to about $1.00/month at list price, likely $0 under the always-free tier
(platform-architecture §7).

## 2. Response playbooks

### 2.1 A1 total outage (every DID skipped twice)

1. `deploy.sh status`: the skip reasons for the last pass (`indexer/skips`).
2. All `plc_unreachable` or DNS-type reasons: check the host's egress
   (`aws ssm start-session`, `curl -sI https://plc.directory`). If the PDS is also affected, the
   host network is the incident. If only PLC is down globally, wait; the index stays searchable.
3. All `pds_*` reasons across different hosts: check a recent deploy (`deploy.sh host-status`
   releases). If it started after a deploy, `deploy.sh rollback`.
4. The index keeps serving the last good data throughout (skips never delete).

### 2.2 A2 pass failed / refused / store unusable

Read the `cause` of the last `pass_summary` or the refused event:

| Cause | Action |
|---|---|
| `repo_dids_malformed` | Fix the SSM value (infrastructure-integration §9.4). Nothing was purged. |
| `repo_dids_unreadable` | `deploy.sh host-status`; check `/pds/indexer/config/repo-dids` exists, is 0444, and the directory mount is intact (`docker inspect` mounts). Then `deploy.sh redeploy`. |
| `upsert_failed`, `purge_failed` | Check `pds_free_mb` in the latest health line (disk full?). If the disk is fine and it started after a deploy, `deploy.sh rollback`. If it persists, `rollback --reset-index`. |
| `pass_panicked` | A bug. `deploy.sh rollback` to the previous digest; open an issue with the `pass_id`. |
| `pass_deadline_exceeded` | Too many DIDs or slow PDSes. Check `duration_ms` trend; trim the list or raise `OPENLORE_INDEXER_PASS_DEADLINE_SECS` (≤ 1740 so the 30-min unit timeout still covers it). |
| `health.startup.refused{probe,arm}` | The named arm, typically DuckDB settings not read back as 48 MB / 1 thread, or a config refusal. `deploy.sh rollback`, then fix config. |
| `indexer.store.unusable` | `serve` exited and Docker restarted it. If it recurs, `deploy.sh rollback --reset-index`. |

### 2.3 A3 not live

`deploy.sh status` shows the latest `indexer.host.health` line; act on the first failing field:

| Field | Likely cause | Action |
|---|---|---|
| no lines at all | Host down, health timer dead, or host log shipping broken | PDS `_health` and the review app's A-1 tell you if the host is down (`aws ec2 describe-instance-status`). Else `systemctl status openlore-indexer-health.timer` via SSM; `deploy.sh redeploy` reinstalls units. |
| `running = 0` / `oom_killed = 1` | Crash, OOM, or a manual stop | `deploy.sh host-status`. On OOM: memory runbook (infrastructure-integration §9.6). If the PDS is at risk, `deploy.sh stop` first: the PDS holds the operator's identity; the index is expendable. |
| `healthz_ok = 0` | Hung `serve` (the timer restarts it after 6 min), store unusable (503), Caddy site missing | `curl` the route; check `/pds/caddy/sites/index.caddy` and reload Caddy; `deploy.sh rollback` if it began with a deploy. |
| `summary_45m = 0` | Pass timer stopped, `trigger` exit 4 (socket missing), passes hanging, or awslogs broken | `systemctl list-timers openlore-indexer-*`; journal of `openlore-indexer-pass.service` (exit codes); if the journal shows passes but CloudWatch does not, check dockerd's awslogs errors (`journalctl -u docker`) and the IAM policy. |
| `search_status = 500` | The index cannot serve searches (`/healthz` may still be ok) | Read `indexer.search.store_error` events (they name the dimension only) in `/openlore/prod/indexer`; `deploy.sh host-status`; if it persists, `deploy.sh redeploy` reopens the store; `deploy.sh rollback` if it began with a deploy. |
| `summary_check = error` | Host role cannot call `FilterLogEvents` | Check `indexer-iam.tf` was applied; IMDS reachable from the host. |
| `dids_age_s > 7200` | SSM reads failing for 2 h (`render_failed` in the journal and `host-dids` stream), parameter deleted, IAM | `aws ssm get-parameter` from the laptop; re-apply IAM. Passes keep using the last good list meanwhile. |

## 3. Test-fire procedures (AC-004.5; rollout step I-6)

Precondition: `indexer_alarms_enabled = true` applied, SNS subscription confirmed
(`aws sns list-subscriptions-by-topic` shows no `PendingConfirmation`).

| Alarm | Procedure (`deploy.sh test-alarm …`) | Expect |
|---|---|---|
| **A2** (end to end, purge-safe) | Append `,not-a-did` to the SSM value; `deploy.sh trigger`; wait. Restore the value; `deploy.sh trigger`. A refused list purges nothing (AC-003.2), and the old index stays searchable (AC-003.3). | ALARM email ≤ 15 min after the pass; OK email after the clean pass |
| **A1** (synthetic, because a real all-skip run needs a production config change) | Stop the pass timer. Create stream `test-fire`. Inject one `pass_summary` line (`exit_code: 3`, `pass_id: "TEST-a1-1"`) in quarter-hour period P1 and an `exit_code: 0` line in P2: **no alarm** (AC-004.1 negative). Then exit 3 in P3 and P4: **ALARM**. Then exit 0 in P5: **OK**. Restart the timer. About 75 min. The TEST lines keep `summary_45m = 1`, so A3 stays quiet; `deploy.sh status` excludes `TEST*` pass ids. | No alarm after P2; ALARM after P4; OK after P5 |
| **A3** (end to end) | `deploy.sh stop`; wait about 10 min; `deploy.sh start`. | ALARM (`running = 0`, `healthz_ok = 0`), then OK |

The stale-list and missing-heartbeat causes of A3 are covered by the health-script bats test
(observability-design §4), not fired live.

## 4. Production readiness checklist (gate before announcing, I-7)

- [ ] R-REPLACE done; IMDS PUT fails from a container; `/pds/caddy/sites` mounted (I-2)
- [ ] First deploy ready; post-checks passed (search 200, other paths 404, 9 KB → 413, HTTPS redirect, trusted cert)
- [ ] First `pass_summary` exit 0; search returns ≥ 2 distinct PDS hosts, every row attributed (AC-001.1)
- [ ] PDS `_health` and review-app `/healthz` 200 throughout the first deploy (AC-001.5)
- [ ] **Memory re-measure gate passed** (or the operator chose t4g.small) (AC-006.4)
- [ ] **Rollback drill N → N-1 passed**, about 1 min, no data restore (AC-006.2)
- [ ] DID-list edit taken by the next pass with no deploy (AC-003.1); a removed DID purged (AC-003.2)
- [ ] `deploy.sh status` answers in ≤ 10 s with the AC-005 fields
- [ ] A1, A2, A3 test-fired and back to OK; alarms enabled
- [ ] Log sample reviewed by hand: no claim content or search value
- [ ] `deploy/indexer/README.md` written from infrastructure-integration §9; `deploy/README.md` links it; R-REPLACE step 7 lists the indexer redeploy
