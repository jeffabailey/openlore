# Infrastructure Integration: indexer-deployment (DEVOPS)

> How the indexer plugs into `deploy/` and the PDS host, plus the operational runbooks. Design
> specification only: DELIVER writes the files named here (paths are proposals that mirror
> `deploy/review-app/`). Contracts come from architecture-design §9.

## 1. Ownership map

| Artifact | Location | Lifecycle |
|---|---|---|
| Caddy sites hook, IMDS hop limit 1 | `tofu-aws-pds` **v1.7.0** (review-app feature; **no indexer change**) | The single planned replacement (R-REPLACE) |
| Host IAM: read the DID parameter, write and filter indexer logs | `deploy/tofu/bootstrap/indexer-iam.tf` (inline policy on `openlore-pds-host-prod`) | Laptop apply, no host impact |
| SSM parameter, log group, metric filters, alarms, saved queries | `deploy/tofu/environments/prod/indexer.tf` | Laptop apply, no host impact |
| DID list **value** | Operator edits with `aws ssm put-parameter` (tofu seeds it once and then ignores the value) | Any time, no deploy (§9.4) |
| Host files (compose, Caddy site, scripts, systemd units) | `deploy/indexer/host/`, installed by `deploy/indexer/deploy.sh` over SSM Run Command | Every deploy (idempotent) |
| Image | `ghcr.io/jeffabailey/openlore-indexer` (`ci-cd-pipeline.md`) | Every green push to main |

Repo layout:

```text
deploy/indexer/
  deploy.sh                          # laptop: install | deploy <sha|digest> | redeploy | rollback [--reset-index]
                                     #         | stop | start | trigger | status | host-status | measure <min> | kpi
                                     #         | test-alarm a1|a2|a3
  host/
    compose.yaml                     # -> /pds/indexer/compose.yaml
    index.caddy                      # -> /pds/caddy/sites/index.caddy
    render-dids.sh                   # -> /pds/indexer/bin/render-dids.sh
    indexer-health.sh                # -> /pds/indexer/bin/indexer-health.sh
    openlore-indexer-pass.service    # -> /etc/systemd/system/   (static files, so CI can systemd-analyze them)
    openlore-indexer-pass.timer
    openlore-indexer-health.service
    openlore-indexer-health.timer
  tests/render-dids.bats             # CI: H1 regression (rename a file, never swap the directory)
crates/openlore-indexer/Dockerfile
deploy/tofu/bootstrap/indexer-iam.tf
deploy/tofu/environments/prod/indexer.tf
```

On-host layout. Everything except the systemd units lives on the data volume and survives
replacement.

```text
/pds/indexer/compose.yaml        0644 root
/pds/indexer/.env                0644 root   INDEXER_DIGEST=sha256:<64hex>  (not secret; pins the digest across reboots)
/pds/indexer/bin/                0755 root   render-dids.sh, indexer-health.sh
/pds/indexer/config/             0755 root   repo-dids (0444 root), .rendered-at, transient .repo-dids.new
/pds/indexer/data/               0700 65532  index.duckdb (+ .wal), indexed_claims/, reset-<utc>/ (only after --reset-index)
/pds/indexer/state/              0700 root   releases (0600, "<utc> <sha> <digest>" lines), health.state
/pds/caddy/sites/index.caddy     0644 root
/etc/systemd/system/openlore-indexer-{pass,health}.{service,timer}
```

## 2. Compose (`/pds/indexer/compose.yaml`)

```yaml
name: openlore-indexer
services:
  indexer:
    image: ghcr.io/jeffabailey/openlore-indexer@${INDEXER_DIGEST:?INDEXER_DIGEST must be a sha256 digest}
    container_name: openlore-indexer        # docker exec target (R-IXD-D7)
    command: ["serve"]
    init: true                              # PID 1 forwards SIGTERM; stop is ~1 s, not the grace period
    restart: unless-stopped
    stop_grace_period: 5s
    user: "65532:65532"
    read_only: true
    tmpfs: ["/tmp:size=8m,mode=0700,uid=65532,gid=65532"]   # control socket lives here
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    pids_limit: 64
    mem_limit: 128m
    memswap_limit: 128m
    oom_score_adj: 900
    cpus: 0.5
    environment:
      OPENLORE_INDEXER_INDEX_PATH: /data/index.duckdb
      OPENLORE_INDEXER_LISTEN_ADDR: 0.0.0.0:8080
      OPENLORE_INDEXER_REPO_DIDS_FILE: /config/repo-dids
      OPENLORE_INDEXER_CONTROL_SOCKET: /tmp/openlore-indexer.sock
      OPENLORE_INDEXER_PURGE_UNLISTED: "1"
      OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB: "48"
      OPENLORE_INDEXER_DUCKDB_THREADS: "1"
      OPENLORE_INDEXER_PASS_DEADLINE_SECS: "1500"
      OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES: "4"
      OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS: "30"
    volumes:
      - /pds/indexer/data:/data
      - /pds/indexer/config:/config:ro      # a DIRECTORY mount: a file mount would pin the old inode (ADR-081)
    networks: [pds_default]
    logging:
      driver: awslogs
      options:
        awslogs-region: us-east-1
        awslogs-group: /openlore/prod/indexer
        awslogs-stream: indexer
        mode: non-blocking
        max-buffer-size: 4m
networks:
  pds_default:
    external: true
```

The CI mount guard (`ci-cd-pipeline.md` §5) asserts exactly these two volumes, `/config` read-only,
`init: true`, `mem_limit == memswap_limit == 128m`, `read_only: true`, no `docker.sock`,
no `privileged`/host namespaces/added capabilities, and that `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP`
and `OPENLORE_INDEXER_REPO_DIDS` are absent.

## 3. Schedule: timer, pass unit and the trigger contract

`openlore-indexer-pass.timer`:

```ini
[Unit]
Description=Run one OpenLore indexer pass every 15 minutes

[Timer]
OnCalendar=*-*-* *:00/15:00
AccuracySec=1s
RandomizedDelaySec=0
Persistent=true            # a slot missed while the host was down runs once at boot

[Install]
WantedBy=timers.target
```

`openlore-indexer-pass.service`:

```ini
[Unit]
Description=OpenLore indexer pass (render the DID list, then trigger one pass inside serve)
After=docker.service
Requires=docker.service

[Service]
Type=oneshot
ExecStartPre=/pds/indexer/bin/render-dids.sh
ExecStart=/usr/bin/docker exec openlore-indexer openlore-indexer trigger
TimeoutStartSec=30min      # >= the 25-min pass deadline plus render; bounds the whole unit
```

Contract:

| Property | How it holds |
|---|---|
| Every 15 min, aligned to :00/:15/:30/:45 | `OnCalendar`, `AccuracySec=1s`. Alignment matters: alarm A1 uses 900 s periods, which CloudWatch aligns to the same quarter hours. |
| Never stacks | A `Type=oneshot` unit stays *activating* for the whole pass; a timer elapse during it merges into the running job instead of starting a second one. Inside `serve`, single-flight returns `busy` and `trigger` exits 0 (coalesced) for any other caller (for example `deploy.sh trigger`). |
| Timeout ≥ pass deadline | 30 min > 1500 s deadline. If systemd ever kills the `docker exec` client, the pass inside `serve` still ends at its own deadline with exit 2 and a `pass_summary`. |
| Unit result reflects the pass | No `SuccessExitStatus`: exit 0 succeeds; 2, 3 and 4 mark the unit *failed* (visible in `systemctl --failed`, `deploy.sh host-status`). A failed oneshot does not stop the timer. |
| Exit codes | 0 pass ok or coalesced, 2 config/store/purge/deadline failure, 3 every DID skipped, 4 `serve` unreachable (socket missing or container down). `docker exec` against a stopped container fails with the daemon's error (exit 1). Neither 4 nor 1 emits a `pass_summary`, so both surface through the liveness alarm A3. |
| Logs | The pass's events (`pass_summary` and the rest) come from `serve`'s stdout and go to CloudWatch. The `trigger` client's own stdout/stderr (`trigger.coalesced`, `trigger.unreachable`) go to the systemd journal only. |
| Reboot / replacement | Units are on the root volume: they survive a reboot, not a replacement. `deploy.sh redeploy` reinstalls them (AC-002.5). |

`openlore-indexer-health.timer` runs `indexer-health.sh run` every 2 minutes
(`OnCalendar=*:0/2`, `AccuracySec=5s`, `TimeoutStartSec=90s`). See `observability-design.md` §4.

## 4. DID list: SSM → read-only file (`render-dids.sh`)

Runs on the host as root as the pass unit's `ExecStartPre`, so every pass sees the latest value.

```text
PARAM=/openlore/prod/indexer/repo-dids   DIR=/pds/indexer/config
1. timeout 30 aws ssm get-parameter --region us-east-1 --name $PARAM \
       --query Parameter.Value --output text  > $DIR/.repo-dids.new      (same directory = same filesystem)
2. if that succeeded and the file is non-empty:
       chown root:root; chmod 0444 $DIR/.repo-dids.new
       mv -f $DIR/.repo-dids.new $DIR/repo-dids      # rename(2): atomic FILE replace; the directory inode never changes
       touch $DIR/.rendered-at
   else:
       rm -f $DIR/.repo-dids.new                      # last-good repo-dids is untouched
       log {"event":"indexer.dids.render_failed","cause":"<ssm_error|timeout|empty>"}  -> journal and
           aws logs put-log-events to stream host-dids in /openlore/prod/indexer (best effort)
3. exit 0 always                                       # availability is the host's job, validity is the binary's
```

"Failure" covers every step: SSM error, timeout, empty value, `chown`/`chmod` error and `mv` error
(for example ENOSPC or a read-only directory). Each leaves `repo-dids` untouched, removes the
staging file, logs `render_failed`, and exits 0; the pass then runs on the last good list.

Rules (ADR-081, DD-IXD-5/13): never validate DIDs; never truncate, delete or swap the
directory; never write the parameter value to the journal or stdout; `set +x`. A stale list
(`.rendered-at` older than 2 h) drives `not_live = 1` in the health line and pages through A3.
The CI test `deploy/indexer/tests/render-dids.bats` runs the script against a fake `aws` and asserts:
the directory inode is unchanged, the file inode changed, mode 0444, `.rendered-at` touched; on
failure the old file is byte-identical, the exit status is 0 and a `render_failed` line is printed,
including when the directory is made read-only between staging and rename.

The parameter is a Standard String: up to 4 KB, about 110 `did:plc` entries. Beyond that, move it
to the Advanced tier ($0.05/month).

## 5. Caddy site (`/pds/caddy/sites/index.caddy`)

```caddyfile
index.{$PDS_HOSTNAME} {
	header -Server
	request_body {
		max_size 8KB
	}
	@search {
		method POST
		path /xrpc/org.openlore.appview.searchClaims
	}
	@health {
		method GET
		path /healthz
	}
	handle @search {
		reverse_proxy openlore-indexer:8080
	}
	handle @health {
		reverse_proxy openlore-indexer:8080
	}
	handle {
		respond 404
	}
	handle_errors {
		# Only proxy failures (the container is restarting) become the 503 page.
		# Other errors keep their status (413 for an oversized body).
		@upstream expression `{err.status_code} in [502, 503, 504]`
		respond @upstream "The OpenLore index is restarting. Try again in a minute." 503
		respond "{err.status_code}" {err.status_code}
	}
}
```

- An exact host outranks `*.{$PDS_HOSTNAME}`: an ordinary HTTP-01 certificate, no on-demand `ask`.
  Automatic HTTPS adds the HTTP → HTTPS redirect (AC-001.2).
- Stock directives only. No access log (module default), so search values never reach Caddy logs.
- The binary enforces the same allowlist and bounds (ADR-083), so a Caddy mistake does not open a
  write path.
- Install path: `deploy.sh install` copies the file, runs
  `docker compose -f /pds/compose.yaml exec -T caddy caddy validate --config /etc/caddy/Caddyfile`,
  and only then `caddy reload`. If validation fails it removes `index.caddy` and aborts, so the
  PDS and app sites are never put at risk by this file.

## 6. Deploy, rollback and status mechanics (`deploy/indexer/deploy.sh`)

Mirrors `deploy/review-app/deploy.sh`: same laptop gates, same SSM Run Command payload pattern
(host files + script tarball, base64, no secret), same releases file. Differences: no DuckDB
pre-copy (the index is rebuildable, no schema change), readiness is `GET /healthz`, it manages two
timers, and it adds `trigger`, `measure`, `kpi` and `test-alarm`. A shared `deploy/lib/` for the
laptop helpers (`ci_green`, `resolve_digest`, cosign verify, `remote`) is allowed if it is a pure
move with the review-app script's behaviour unchanged; DELIVER decides.

### 6.1 `deploy <git-sha|digest>`

Laptop:

1. Refuse anything that is not a full 40-hex sha or `sha256:<64hex>` (no tags).
2. `gh run list --workflow ci.yml --commit <sha> --branch main` is `success`; resolve
   `ghcr.io/jeffabailey/openlore-indexer:sha-<sha>` to a digest.
3. `cosign verify <image>@<digest> --certificate-identity-regexp '^https://github.com/jeffabailey/openlore/.github/workflows/ci.yml@refs/heads/main$' --certificate-oidc-issuer https://token.actions.githubusercontent.com`.
4. Pre-checks: PDS `https://openlore.jeffbailey.us/xrpc/_health` 200; review app `/healthz` 200
   if it is deployed. Start a background poller of the PDS `_health` every 2 s for the duration.
5. `remote deploy <digest> <sha>`.

Host (root, via SSM):

1. `install` (idempotent): refuse unless `/pds/caddy/sites` exists and the IMDS-from-container PUT
   fails (§8); dirs and modes (§1), host files, units, `daemon-reload`,
   `enable --now openlore-indexer-health.timer`, Caddy validate-then-reload (§5).
2. `render-dids.sh`; then require `/pds/indexer/config/repo-dids` to exist, or stop with
   "put /openlore/prod/indexer/repo-dids first" (first deploy only).
3. `docker pull <image>@<digest>` while the old container still serves.
4. `prev` = the last digest in `releases`.
5. Write `.env`; `docker compose -f /pds/indexer/compose.yaml up -d --force-recreate indexer`
   (stop ~1 s with init, start 2-5 s).
6. Readiness: up to 60 s of
   `curl -fsS --resolve index.openlore.jeffbailey.us:443:127.0.0.1 https://index.openlore.jeffbailey.us/healthz`
   → 200 with `"status":"ok"`. Record the seconds taken (search downtime evidence).
7. **Ready:** append `<utc> <sha> <digest> ready_s=<n>` to `releases`; `enable --now openlore-indexer-pass.timer`;
   `systemctl start --no-block openlore-indexer-pass.service` (one pass now, coalesces if running);
   delete any `data/reset-*` directory older than this deploy.
8. **Not ready → automatic rollback:** print the last 50 log lines; if `prev` exists, start it and
   wait for readiness again; exit 1. With no `prev` (first deploy), stop the container and exit 1.

Laptop post-checks: `GET /healthz` 200; a minimal `POST` search returns 200 JSON;
`GET /xrpc/com.atproto.repo.createRecord` → 404; `POST /healthz` → 404; an 8 KB body is not 413, a 9 KB body → 413; PDS
`_health` and review-app `/healthz` 200; the poller reports `PDS _health: n/n ok during deploy`.
Printed summary: digest, readiness seconds, PDS poll result (KPI-IXD-6 evidence).

An in-flight pass is abandoned by the recreate (the `trigger` client in the unit gets exit 4 or 1
and the unit fails once); the next slot recovers it.

### 6.2 `rollback [--reset-index]`

- Host: `prev` = the second-to-last digest in `releases`; recreate with it; readiness as above;
  append `<utc> rollback <digest>`. About 30-60 s end to end, including SSM overhead (≤ 2 min,
  KPI-IXD-6). No data restore: there is no schema change between releases.
- `--reset-index` (escape hatch if a digest ever changed the index files incompatibly, or the
  store is reported unusable): stop, move `index.duckdb`, `index.duckdb.wal` and
  `indexed_claims/` into `data/reset-<utc>/`, start, trigger a pass. The index rebuilds from the
  authors' PDSes within one to two passes (ADR-023). The moved-aside copy is deleted at the next
  successful deploy (5 GB shared volume).

### 6.3 Other modes

| Mode | Does | Where |
|---|---|---|
| `install` | Host files, units, Caddy site (no container change) | SSM |
| `redeploy` | `install` + recreate the current digest from `releases` (used after R-REPLACE) | SSM |
| `stop` | `systemctl stop openlore-indexer-pass.timer` then `compose stop` (Caddy serves 503; health timer keeps reporting, so A3 fires if alarms are on) | SSM |
| `start` | Reverse of `stop` | SSM |
| `trigger` | `systemctl start openlore-indexer-pass.service` (waits) and prints the unit result | SSM |
| `status` | **Laptop only**, Logs Insights over `/openlore/prod/indexer` (observability-design §5): last exit-0 pass time, age and counts; skipped DIDs with reasons for that `pass_id`; last 8 exit codes; age of the last pass of any kind; latest `indexer.host.health` line. ≤ 10 s, no host shell, no claim content (AC-005.1..4). | AWS API |
| `host-status` | Container state, `systemctl list-timers openlore-indexer-*`, last unit results, last 30 log lines, `releases` tail | SSM |
| `measure <minutes>` | The re-measure gate sampler (§9.6) | SSM |
| `kpi` | KPI-IXD-3 freshness ratio and KPI-IXD-5 alarm/exit reconciliation (`kpi-instrumentation.md`) | AWS API |
| `test-alarm a1\|a2\|a3` | Alarm test-fire procedures (`monitoring-alerting.md` §3) | mixed |

## 7. Tofu (creates only)

### 7.1 `deploy/tofu/bootstrap/indexer-iam.tf`

Inline policy `openlore-indexer-host` on `openlore-pds-host-prod`, mirroring `review-app-iam.tf`:

| Sid | Actions | Resource |
|---|---|---|
| ReadIndexerDidList | `ssm:GetParameter` | `arn:aws:ssm:us-east-1:091153021562:parameter/openlore/prod/indexer` and `…/indexer/*` |
| WriteIndexerLogs | `logs:CreateLogStream`, `logs:PutLogEvents`, `logs:DescribeLogStreams` | `arn:aws:logs:us-east-1:091153021562:log-group:/openlore/prod/indexer:*` |
| ReadIndexerHeartbeat | `logs:FilterLogEvents` | same log group only |

No SSM write, no `kms:Decrypt` (Standard String), no `CreateLogGroup`, no `PutMetricData`.
`ReadIndexerHeartbeat` is the one addition beyond the brief: the health timer reads the age of
the last shipped `pass_summary`, which also proves the awslogs pipeline works end to end. It reads
only the indexer's own structural log lines. `FilterLogEvents` is read-only; group scope is needed
because `pass_summary` is in stream `indexer`, not in the timer's own stream.

### 7.2 `deploy/tofu/environments/prod/indexer.tf`

| Resource | Notes |
|---|---|
| `variable "indexer_alarms_enabled"` | default `false`; sets `actions_enabled` on all 3 alarms |
| `aws_ssm_parameter.indexer_repo_dids` | `/openlore/prod/indexer/repo-dids`, `type = "String"`, `tier = "Standard"`, seed value `did:plc:pnyxfnpkcldxtitsw64ycahw` (the operator's own OpenLore DID, from `deploy/README.md`), `lifecycle { ignore_changes = [value] }` so operator edits never show as drift |
| `aws_cloudwatch_log_group.indexer` | `/openlore/prod/indexer`, `retention_in_days = 30` |
| `aws_cloudwatch_log_metric_filter` × 4 | `indexer-pass-outage` (exit 3 → 1), `indexer-pass-not-outage` (exit ≠ 3 → 0, same metric), `indexer-failure`, `indexer-not-live` (monitoring-alerting §1) |
| `aws_cloudwatch_metric_alarm` × 3 | A1, A2, A3 on `module.pds.backup_alarm_topic_arn`, with `ok_actions` |
| `aws_cloudwatch_query_definition` × 4 | `indexer/freshness`, `indexer/kpi-freshness`, `indexer/exit-codes`, `indexer/skips` (free) |

Expected plan for this file alone: **13 to add, 0 to change, 0 to destroy** (1 parameter, 1 log
group, 4 filters, 3 alarms, 4 query definitions); the bootstrap plan adds 1 inline policy. Both
must pass `check-plan.sh` **without** `OPENLORE_ALLOW_DELETE`.

## 8. Sequencing with tofu-aws-pds v1.7.0 and the review-app go-live

> *Superseded 2026-10-09 (fix-go-live-runbook-gaps):* the I-0..I-7 table below is design
> history. The operator's one ordered sequence for both apps (alarms enabled, then test-fired) is
> the [Go-live checklist](../../../../deploy/README.md#go-live-checklist).

There is exactly **one** instance replacement (R-REPLACE, owned by the review-app rollout,
`docs/feature/bluesky-claim-review-app/devops/infrastructure-integration.md` §7.1). This feature
adds no module change and no replace.

| Step | What | Depends on | PDS downtime | Gate |
|---|---|---|---|---|
| I-0 | Apply `indexer-iam.tf` (bootstrap) and `indexer.tf` (prod), alarms disabled. Any time, even on v1.6.0. | nothing | none | Plan shows only creates; `check-plan.sh` passes with no override |
| I-1 | DELIVER merges B1-B15, `deploy/indexer/**`, the CI jobs and the `deploy-pds-check` extension. First image published. Operator makes the GHCR package `openlore-indexer` **public** (once). | — | none | Anonymous `docker pull` of the digest works |
| I-2 | **R-REPLACE** (shared, once): v1.7.0 sites hook + IMDS hop limit 1 | review-app rollout R-1/R-2 | a few minutes, **once** | R-REPLACE step 6 all green, including "IMDS PUT fails from a container" |
| I-3 | If the review app is live: deploy its B11 build (DuckDB caps, `init: true`, 192m) | B11 on main | none | Review app `/readyz` 200 |
| I-4 | Indexer first deploy: put the production DID list (§9.4), `deploy.sh install`, `deploy.sh deploy <sha>` | I-0, I-1, **I-2 (hard precondition)** | none | Post-checks (§6.1); first `pass_summary` exit 0 within 15 min |
| I-5 | Soft launch: **memory re-measure gate** (§9.6) and **rollback drill** N → N-1 (deploy a second green sha, then `rollback`); 24 h CPU credits | I-4 | none | Both pass; failures follow §9.6 |
| I-6 | Confirm the SNS subscription; set `indexer_alarms_enabled = true` (in-place update of 3 alarms); test-fire A1, A2, A3; each returns to OK | I-5 | none | AC-004.5 |
| I-7 | Announce `OPENLORE_INDEXER_URL=https://index.openlore.jeffbailey.us`; start the KPI baselines | I-6 | — | — |

Rules:

- **No indexer container may start on a host where IMDS hop limit is still 2.** The public
  container would otherwise be able to obtain the host role. `deploy.sh install` checks it: on the
  host, `docker run --rm --network pds_default curlimages/curl -s -m 3 -X PUT http://169.254.169.254/latest/api/token -H 'X-aws-ec2-metadata-token-ttl-seconds: 60'`
  must fail, and `/pds/caddy/sites` must exist; otherwise it refuses.
- **I-0 decision rule.** Run `tofu plan -out=tfplan` in prod and `check-plan.sh tfplan` **without**
  `OPENLORE_ALLOW_DELETE`. If it passes, the plan is creates/updates only: apply it. If it refuses
  (the plan contains the instance replace because the v1.7.0 bump is already committed), **do not
  set the override for I-0**: either wait and let the creates ride R-REPLACE step 4, or apply I-0
  from a commit before the bump. The gate itself is the guardrail; `check-plan.sh` needs no change.
- If I-0 is applied after the module bump is committed, its plan would include the replace. Then
  do not apply it separately: fold the indexer creates into R-REPLACE step 4 (the replacement
  plan already needs `OPENLORE_ALLOW_DELETE=1`; confirm the only delete/replace lines are the
  instance and its volume attachment). Either way there is no second replacement.
- Either app may go live first. If the indexer is ready inside the R-REPLACE window, it deploys in
  that window after the review app's checks; if later, onto the replaced host with no disruption.
- The R-REPLACE runbook's step 7 ("redeploy apps") gains: `deploy/indexer/deploy.sh redeploy`.
  `deploy/README.md` gains an *Indexer* section pointing here.

## 9. Runbooks (DELIVER turns these into `deploy/indexer/README.md`)

Laptop needs: `AWS_PROFILE=jeff`, `gh`, `cosign`, `jq`, `crane` or `docker buildx`. Instance id
from `tofu -chdir=deploy/tofu/environments/prod output -raw instance_id`.

### 9.1 First deploy (I-4)

Rollback first: there is no previous release, so a not-ready first deploy stops the container
and leaves the PDS and review app untouched; `index.caddy` serves 503 until fixed or removed
(`rm /pds/caddy/sites/index.caddy` + Caddy reload).

1. Set the production DID list (§9.4). Confirm `aws ssm get-parameter` shows it.
2. `deploy/indexer/deploy.sh install` (checks IMDS and the sites hook, installs units and site).
3. `deploy/indexer/deploy.sh deploy <sha>`; read the post-check summary. If it fails, the container
   is stopped and `index.` answers 503. Fix and re-run `deploy`, or remove `index.caddy` and reload
   Caddy until the retry (only the `index.` site is affected).
4. Before the first pass: `POST` search returns an empty result with 200, not 5xx (AC-001.4).
5. Wait for the first slot (or `deploy.sh trigger`). `deploy.sh status` shows exit 0 and the
   counts; a search returns results from ≥ 2 PDS hosts (AC-001.1).
6. `curl -I http://index.openlore.jeffbailey.us/healthz` redirects to HTTPS; the certificate is
   publicly trusted (AC-001.2).

### 9.2 Update (R-IXD-DEPLOY)

`deploy/indexer/deploy.sh deploy <sha>` (§6.1). Expected search downtime ≤ 15 s. Any time of
day; avoid the minute right after a quarter hour if you want to keep the current pass.

### 9.3 Rollback (R-IXD-ROLLBACK)

- `deploy/indexer/deploy.sh rollback`: previous digest, about 1 min, no data restore.
- If the rolled-back binary reports `indexer.store.unusable` or refuses its store:
  `deploy.sh rollback --reset-index` (search returns fewer results until the next one or two
  passes rebuild the index).
- If the indexer harms the PDS (memory, CPU) and no digest is good: `deploy.sh stop`. Caddy serves
  503 for `index.`; the PDS and the review app are unaffected.
- **Drill (I-5, hard gate):** deploy N, deploy N+1, `rollback` to N, confirm readiness and a pass.

### 9.4 Edit the DID list (no deploy, KPI-IXD-4)

```sh
export AWS_PROFILE=jeff AWS_REGION=us-east-1
P=/openlore/prod/indexer/repo-dids
aws ssm get-parameter --name $P --query Parameter.Value --output text | tr ', ' '\n\n' | sed '/^$/d' | sort > /tmp/dids.old
cp /tmp/dids.old /tmp/dids.new && $EDITOR /tmp/dids.new          # one bare did:plc / did:web per line
diff /tmp/dids.old /tmp/dids.new                                 # every "<" line will be PURGED on the next pass
aws ssm put-parameter --name $P --type String --overwrite --value "$(paste -sd, /tmp/dids.new)"
```

The next pass (≤ 15 min) renders and loads it. Verify with `deploy.sh status`
(`indexer.config.loaded.repo_did_count` and `repo_dids_age_secs`). A malformed entry gives one
exit-2 pass naming it (A2 pages), the old index stays searchable and **nothing is purged**; fix
the value and wait for the next pass.

### 9.5 Purge safety

- Removing a DID purges that author's claims at the start of the next pass that loads the list
  (ADR-082). Skips never purge. An empty, malformed or unreadable list purges nothing.
- **Before** a large edit: `diff` as in §9.4; count the `<` lines. If you are unsure, pause
  passes first: `sudo systemctl stop openlore-indexer-pass.timer` (via `deploy.sh stop` or an SSM
  session), edit, review, then `deploy.sh start`.
- **Deleted parameter:** passes keep the last good file; `tofu apply` would recreate it with the
  seed value, so first restore the production value from `get-parameter-history` (history
  survives only while the parameter exists; keep `/tmp/dids.old` from §9.4 as the backup).
- **Undo a wrong edit:** `aws ssm get-parameter-history --name $P` shows every version; put the
  previous value back. The next pass re-ingests the removed authors from their PDSes (the index
  is a cache, R-IXD-D2). Expect up to two passes for the full re-ingest.
- `indexer.ingest.author_purged{did,claims_removed}` is logged per author; read it with the
  `indexer/skips` saved query or Logs Insights.

### 9.6 Memory re-measure gate (I-5, hard)

1. Make sure the production DID list is in place and both apps run their final caps.
2. Sample every 5 s for 20 minutes: cgroup v2 `memory.peak` of both app containers
   (`/sys/fs/cgroup/system.slice/docker-<id>.scope/memory.peak`, resolved from `docker inspect`),
   `MemAvailable` minimum, `pswpin` delta, PDS `_health` every 10 s. *As built (2026-10-09):* no
   `measure` mode was written; the runnable loop, the PASS thresholds and the search load are the
   memory gate step of the go-live checklist.
3. During the window: `deploy.sh trigger` (a full pass), start a 10-repo review-app scan from
   the app UI, and run the search burst from the laptop:
   paced under the per-client limit of 10/s, burst 50 (the checklist's load uses four workers
   with a 0.5 s pause; a 429 is counted, not failed), with all other responses 200 and p95 ≤ 1 s
   (NFR-IXD-7 and AC-002.2).
4. PASS/FAIL is read against platform-architecture §6 (indexer ≤ 100 MB, review app ≤ 128 MB,
   `MemAvailable` > 128 MB), plus the DuckDB read-back from
   each app's startup probe event.
5. Next day: `CPUCreditBalance` for the last 24 h is flat or rising (NFR-IXD-5).
6. On FAIL: lower `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` to 2 (compose edit + redeploy),
   lower the review-app scan concurrency to 1 (`OPENLORE_REVIEW_SCAN_CONCURRENCY: "1"` in its
   compose file, then `deploy/review-app/deploy.sh redeploy`), and re-measure. If it still fails, **stop and ask
   the operator**: t4g.small means editing `instance_type` in `deploy/environments/prod.json`,
   a plan that shows an in-place update (no replace), and an apply that stops and starts the
   instance (a few minutes of PDS downtime). Then redeploy both apps and re-measure. Record the
   decision in `wave-decisions.md`.

### 9.7 After a reboot or replacement

- Reboot: nothing to do. Docker restarts the container at the pinned digest; the timers survive;
  `Persistent=true` runs one missed slot.
- Replacement (any future R-REPLACE): run `deploy/indexer/deploy.sh redeploy` after the PDS is
  verified. Units, site file reload and the container come back from `/pds/indexer`.

### 9.8 Review-app secrets: rotation recommendation (R-IXD-D8)

`deploy/review-app/host/render-secrets.sh` swaps the whole secrets **directory**. The running
container keeps the old directory inode, so rotation only takes effect after a restart (which
`deploy.sh redeploy` does today). Not broken, but fragile. Recommended follow-up (not in this
feature's scope): render each secret to `<dir>/.<name>.new` inside the existing directory,
`chown 65532`, `chmod 0400`, `mv -f` over `<name>`, and delete files whose parameter disappeared.
The directory then never changes inode, and a future in-process reload would see new values.
Add the same bats inode test as `render-dids.bats`.
