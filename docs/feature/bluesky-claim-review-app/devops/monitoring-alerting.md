# Monitoring and Alerting: bluesky-claim-review-app (DEVOPS)

> **Essentials only (user decision 2026-10-04).** Four alarms, all on the **existing** SNS topic
> `openlore-pds-backup-alarm` (module output `backup_alarm_topic_arn`), which is already
> subscribed out of band. No address is in state, and no new topic or subscription is added.
> Alarms carry `ok_actions` too, so a recovery is also notified, as the backup alarm does.
>
> Defined in `deploy/tofu/environments/prod/review-app.tf`. A root variable
> `review_app_alarms_enabled` (default `false`) sets `actions_enabled`, so the alarms can exist
> before the app's first deploy without paging. It is flipped to `true` at rollout step R-6.

## 1. Alarm catalogue (active)

| ID | Alarm name | Metric / source | Condition | Missing data | Tier | Runbook |
|---|---|---|---|---|---|---|
| A-1 | `openlore-review-app-unreachable` | `AWS/Route53 HealthCheckStatus` for health check `review-app-healthz`: HTTPS, FQDN `app.openlore.jeffbailey.us`, port 443, path `/healthz`, request interval 30 s, failure threshold 3, **no** string matching | Minimum < 1 for **5 of 5** 1-min periods | breaching | Page | §2.1 |
| A-3 | `openlore-review-app-down-or-restarted` | Log metric filter on the host's `host.health` lines: `{ $.event = "host.health" && ($.restarts > 0 \|\| $.app_running = 0) }` → `AppDown` (value 1) | Sum >= 1 in 5 min | notBreaching (host-down is A-1's job) | Urgent | §2.2 |
| A-7 | `openlore-review-app-guardrail-breach` | Log metric filter `{ $.event = "guardrail.breach" }` → `GuardrailBreaches` | Sum >= 1 in 5 min | notBreaching | **Page (privacy)** | §2.3 |
| A-8 | `openlore-review-app-github-token-expiring` | Log metric filter `{ $.event = "github.token.expiring" }` → `GithubTokenExpiring` (emitted daily when GitHub's `github-authentication-token-expiration` header is <= 14 days away) | Sum >= 1 in 1 day | notBreaching | Warning | infrastructure-integration §7.4 |
| (existing) | `openlore-pds-backup-missing` | `PDS/Backup ArchiveUploaded` | unchanged | breaching | Urgent | deploy/README §5 |

The alarm IDs are kept stable (A-1, A-3, A-7, A-8) so that references elsewhere stay valid.
The IDs not listed are deferred (§1.1).

The Route 53 health check needs no hosted zone. It resolves the FQDN through public DNS
(Cloudflare), and its metric lives in us-east-1, the deployment region. Route 53 HTTPS checks do
not validate certificates; certificate expiry is caught by the nightly `live-contract-smoke`
(curl validates) and the startup self-probe.

Every alarm description carries a one-line runbook pointer, as the backup alarm does, for
example: "Check `deploy.sh status`; then infrastructure-integration.md §7.3".

### 1.1 Deferred alarms: signals kept, no alarm in v1

Each signal below is still produced as a **log line** in `/openlore/prod/review-app`. Adding the
alarm later means one `aws_cloudwatch_log_metric_filter` plus one `aws_cloudwatch_metric_alarm`,
with no host or app change.

| Deferred ID | Would alarm on | Signal kept | Filter to add later |
|---|---|---|---|
| A-2 not ready | `/readyz` failing for 5 min | `host.health.ready` (0/1) | `{ $.event = "host.health" && $.ready = 0 }` |
| A-4 app memory high | >= 85% of 256 MB for 10 min | `host.health.app_mem_pct` | `{ $.event = "host.health" && $.app_mem_pct >= 85 }` |
| A-5 host memory low (PDS at risk) | `MemAvailable` < 100 MB for 10 min | `host.health.host_mem_available_mb` | `{ $.event = "host.health" && $.host_mem_available_mb < 100 }` |
| A-6 CPU credits low | `CPUCreditBalance` < 30 | `AWS/EC2 CPUCreditBalance` (free basic monitoring; no log needed) | Alarm directly on the EC2 metric |
| A-9 startup refused | any `health.startup.refused` | the app event `health.startup.refused{probe,arm}` | `{ $.event = "health.startup.refused" }` |

Partial coverage meanwhile:
- a refused start, or a crash loop, shows up as A-3 (restarts) and A-1 (Caddy serves 503);
- sustained memory pressure ends in an OOM kill, which is A-3.

**The memory re-measure in soft launch (R-5, platform-architecture §6) stays a hard gate.** The
alarms were trimmed on the assumption that the host has headroom, and only the gate proves it.
Revisit triggers for adding A-4 and A-5: a failed or marginal re-measure, any OOM (A-3) under
normal load, or more than 50 users.

### Threshold rationale

- **A-1 at 5 minutes:**
  - a Recreate deploy is 5-15 s of 503s, so a deploy never pages;
  - 5 minutes of detection against a 7.3 h monthly budget is plenty;
  - with missing data counted as breaching, a dead host pages through A-1.
- **A-3 at the first restart or a stopped container:** with `restart: unless-stopped`, a crash or
  OOM shows up only as a restart count. Each restart is worth a look on a 1 GiB shared host.
  A deliberate `deploy.sh stop` also fires it, which is expected during maintenance.
- **A-7 at the first event:** the guardrails' target is 0 (KPI-BRA-4/4b/5/6). One event is an
  incident.
- **A-8, 14 days ahead:** an expired token makes the probe refuse start.

## 2. Response playbooks

### 2.1 A-1 unreachable (external)

1. Check whether the host and app are up: `deploy.sh status` (container state, the last 50 log
   lines, `releases`, and the latest `host.health` line).
   - If the container is down or restarting, see §2.2.
   - If there is no response over SSM, the host is down: `aws ec2 describe-instance-status`, the
     PDS health and the instance's system log.
2. If the host and app are fine:
   - `dig +short app.openlore.jeffbailey.us` should return the EIP; check that the Cloudflare
     record is unchanged and still **DNS only**;
   - check that the security group still allows 443;
   - check Caddy with `docker compose -f /pds/compose.yaml logs caddy`.

### 2.2 A-3 down or restarted (also covers not-ready and startup-refused, which have no alarm)

1. `deploy.sh status`. `docker inspect` shows `OOMKilled`, and Logs Insights
   `review-app/startup` shows the last lifecycle events.
2. A `health.startup.refused{probe,arm}` names the failing arm:
   - `github.token_rejected` means the PAT expired or was revoked → rotate (infrastructure-integration §7.4);
   - `review_store.aead_canary` means the wrong data key → check `data-key` and `data-key-previous`;
   - `review_store.overlayfs` means the mount is wrong → check `compose.yaml`;
   - `oauth.jwk_load` means the client JWK is malformed;
   - `selfprobe.client_metadata` means the Caddy route is missing (is `/pds/caddy/sites/app.caddy`
     present? reload Caddy) or there is a DNS problem.
3. On an OOM, look at the `scan.*` events in the 10 minutes before, and at `host.health`
   (`app_mem_pct`, `host_mem_available_mb`).
   - If a scan pattern drives it, lower the ADR-076 global concurrent scans from 2 to 1 (config)
     and redeploy.
   - If it recurs under normal load, raise `mem_limit` **only** if `host_mem_available_mb` shows
     headroom; otherwise move to t4g.small (platform-architecture §6).
4. If the PDS is at risk (`host_mem_available_mb` < 100, or the PDS `_health` fails), **stop the
   app first** (`deploy.sh stop`). The PDS holds Jeff's identity; the app is expendable.
5. If the last deploy is the cause, run `deploy.sh rollback` (infrastructure-integration §7.3).

### 2.3 A-7 guardrail breach (privacy incident)

1. Read `review-app/guardrails` in Logs Insights. The event carries `kpi` and `check`
   (`writes_exceed_confirms`, `declined_reoffered`, `scrape_without_gate`,
   `readback_not_self_attested`) and a count. It carries no content.
2. For `writes_exceed_confirms` or `scrape_without_gate`, **stop the app** (`deploy.sh stop`).
   These are I-BRA-3 and I-BRA-4 violations.
3. For `readback_not_self_attested` (KPI-BRA-6): the app keeps running, because users' claims
   are in their own PDSes, but stop announcing it. Compare with the nightly `live-contract-smoke`.
4. For `declined_reoffered` (KPI-BRA-4b): this is expected **only** after a DuckDB restore or
   loss (the accepted v1 posture). Otherwise it is a reconcile bug.
5. Open an incident note in `docs/evolution/`. A post-incident review happens within 48 h.

## 3. Production readiness checklist (DEVOPS gate before announcing)

This checklist needs DELIVER and DEVOPS together: it can only be completed after DELIVER has
merged the app and the deploy tooling (wave-decisions, Sequencing).

- [ ] R-REPLACE done; PDS verified; IMDS closed to containers (infrastructure-integration §7.1)
- [ ] First deploy is ready; `/oauth/*` is served byte-equal at the public origin
- [ ] Confidential `private_key_jwt` sign-in verified on bsky.social **and** the OpenLore PDS (R-5)
- [ ] **RSS re-measure gate passed on the host** (platform-architecture §6). **Hard gate**: the
      memory alarms are deferred, so this is the only proof of headroom.
- [ ] Rollback drill done (N → N-1) (infrastructure-integration §7.3). **Hard gate:** no announcement without it.
- [ ] Disconnect verified, including revoke 200 handling, and the access-token window recorded
- [ ] Every active alarm fired once in a test and returned to OK:
  - `deploy.sh stop` fires A-3 (`app_running = 0`) and A-1 (Caddy 503), then `deploy.sh redeploy`;
  - `POST /admin/test-alarm?kind=guardrail` fires A-7;
  - `POST /admin/test-alarm?kind=token` fires A-8.
- [ ] `review_app_alarms_enabled = true` applied; the SNS subscription is confirmed (existing)
- [ ] Log sample reviewed by hand: no forbidden field (observability-design §3)
- [ ] Runbook `deploy/review-app/README.md` written from infrastructure-integration §7
