# OpenLore indexer: deploy runbook

The public index (`https://index.openlore.jeffbailey.us`) runs as one container,
`openlore-indexer`, on the PDS host next to the PDS and the review app. It serves only
`POST /xrpc/org.openlore.appview.searchClaims` and `GET /healthz`. A systemd timer runs one
ingest pass every 15 minutes inside that container. Design:
`docs/feature/indexer-deployment/devops/` (infrastructure-integration, platform-architecture,
monitoring-alerting, observability-design) and ADR-075, ADR-080..083.

The index is a cache. Nothing in it is backed up, and every author's claims can be re-read
from their own PDS. When the host is short of memory the indexer goes first (oom_score_adj 900),
then the review app (800), and never the PDS.

## Files

| Repo file | On the host | What it does |
|---|---|---|
| `deploy.sh` | runs from the laptop, and on the host as `deploy.sh host <mode>` over SSM | install, deploy, rollback, stop/start, trigger, status |
| `host/compose.yaml` | `/pds/indexer/compose.yaml` | the image pinned by digest; read-only rootfs; 128 MB with no swap; 0.5 CPU; 64 pids; **exactly two mounts**: `/pds/indexer/data` (rw) and the `/pds/indexer/config` **directory** (ro); no published port; no cloud credentials |
| `host/index.caddy` | `/pds/caddy/sites/index.caddy` | `index.{$PDS_HOSTNAME}`: POST search and GET /healthz, 404 for everything else, 8 KB body cap, no `Server` header, 503 page only for proxy failures |
| `host/render-dids.sh` | `/pds/indexer/bin/` | SSM `/openlore/prod/indexer/repo-dids` → `config/repo-dids` (0444). It renames the **file** and never swaps the directory. On failure it keeps the last good list and exits 0 |
| `host/health-timer.sh` | `/pds/indexer/bin/` | every 2 min: one `indexer.host.health` line with `not_live` (alarm A3's only input); restarts a hung container |
| `host/openlore-indexer-pass.{timer,service}` | `/etc/systemd/system/` | :00/:15/:30/:45 (Persistent) → `render-dids.sh`, then `docker exec openlore-indexer openlore-indexer trigger` |
| `host/openlore-indexer-health.{timer,service}` | `/etc/systemd/system/` | every 2 min → `health-timer.sh run` |

Host state: `/pds/indexer/.env` (`INDEXER_DIGEST=sha256:…`, not secret),
`/pds/indexer/state/releases` (`<utc> <sha> <digest> ready_s=<n>` and `<utc> rollback <digest>`),
`/pds/indexer/state/health.state`. Everything except the systemd units is on the data volume
and survives an instance replacement.

Laptop needs: `AWS_PROFILE=jeff`, `gh` (signed in), `cosign`, `jq`, and `crane` or
`docker buildx`. The instance id comes from
`tofu -chdir=deploy/tofu/environments/prod output -raw instance_id` (or set `INDEXER_INSTANCE_ID`).

```sh
deploy/indexer/deploy.sh deploy <40-hex git sha>      # CI green + cosign verify, then Recreate by digest
deploy/indexer/deploy.sh status                       # freshness, exit codes, skips, last health line
deploy/indexer/deploy.sh rollback [--reset-index]     # previous digest from state/releases
deploy/indexer/deploy.sh install | redeploy | stop | start | trigger | host-status
```

Tags, branches and short shas are refused before any tool runs. `deploy` also refuses a sha
whose `ci.yml` run on main is not `success`, and a digest without a CI cosign signature, before
it touches the host.

## Trusted proxies (per-client rate limit)

The indexer limits each client to 10 searches/s (burst 50) and answers 429 with `Retry-After`
beyond that; `openlore search` then says "Network index is busy — try again in N s" and exits 0.
It reads the client from `X-Forwarded-For` (the last entry, which Caddy's `reverse_proxy`
sets to the peer it saw) only when the connection comes from loopback or a network in
`OPENLORE_INDEXER_TRUSTED_PROXIES`. Without that, every public client would share Caddy's
one bucket.

`host/compose.yaml` sets `OPENLORE_INDEXER_TRUSTED_PROXIES=172.16.0.0/12,192.168.0.0/16`.
Caddy connects from `pds_default`, which the tofu-aws-pds module creates and Docker numbers from
its default local address pools (172.17.0.0/16 to 172.31.0.0/16, then 192.168.0.0/16 in /20s).
The container publishes no port, so only containers on this host's Docker networks can connect
from those ranges. xtask XP-18 checks the value covers every default pool and nothing else.

To narrow it to the actual subnet, on the host:

```sh
docker network inspect pds_default --format '{{range .IPAM.Config}}{{.Subnet}} {{end}}'
```

`deploy.sh install`/`redeploy` copy `host/compose.yaml` from the repo, so narrow it there: set
that subnet (e.g. `172.18.0.0/16`) as the value, change XP-18's Docker-pool list in
`xtask/tests/indexer_deployment_platform.rs` to the same subnet, commit, and run
`deploy.sh redeploy`. The subnet can change when the instance is replaced (the network is
recreated), so recheck it then; the default-pool value never needs this. A bad entry makes the
indexer refuse to start (exit 2, naming the variable), so a typo shows at once.

## Pass exit codes

| Exit | Meaning | Unit | Alarm |
|---|---|---|---|
| 0 | pass ok, or coalesced into a running pass | succeeded | none |
| 2 | config, store, purge or deadline failure (`pass_summary.cause`) | **failed** | A2 |
| 3 | every DID skipped | **failed** | A1 after two in a row |
| 4 | `serve` unreachable (no socket, container down); no `pass_summary` | **failed** | A3 (after 45 min) |

A failed run does not stop the timer. You can see failures with `systemctl --failed` and
`deploy.sh host-status`.

## 1. Sequencing (once)

There is exactly one instance replacement, and the review-app rollout owns it (R-REPLACE). The
indexer adds no module change.

1. **I-0** (any time): apply `deploy/tofu/bootstrap/indexer-iam.tf` and
   `deploy/tofu/environments/prod/indexer.tf` with `indexer_alarms_enabled = false`. Run
   `check-plan.sh tfplan` **without** `OPENLORE_ALLOW_DELETE`. If it refuses because the v1.7.0
   bump is already committed, do not override it. Fold these creates into R-REPLACE step 4
   instead.
2. **I-1**: CI publishes the first `ghcr.io/jeffabailey/openlore-indexer` image. Make the GHCR
   package **public** once, and check that an anonymous `docker pull <image>@<digest>` works.
3. **I-2 (hard precondition): the tofu-aws-pds v1.7.0 replacement** (R-REPLACE: the Caddy sites
   hook plus IMDS hop limit 1). No indexer container may start before it. `deploy.sh install`
   checks this, failing closed. It refuses unless `/pds/caddy/sites` exists, the probe image
   (`curlimages/curl`, pinned by digest; override with `IMDS_PROBE_IMAGE=<image>@sha256:...`)
   runs `curl --version` on `pds_default`, **and** the IMDS PUT from that container then ends
   with curl exit 7 (cannot connect) or 28 (timeout). Any other outcome refuses.
4. **I-3**: if the review app is live, deploy its **B11** build (DuckDB 48 MB / 1 thread,
   `init: true`, 192m): `deploy/review-app/deploy.sh deploy <sha>`, then check `/readyz` 200.
   The two apps fit the host only with both sets of caps in place.
5. **I-4**: the indexer's first deploy (§2).
6. **I-5**: the memory re-measure gate (§6) and the rollback drill (§4.1).
7. **I-6**: confirm the SNS subscription, set `indexer_alarms_enabled = true`, apply, and
   test-fire A1, A2 and A3 (§7).
8. **I-7**: announce `OPENLORE_INDEXER_URL=https://index.openlore.jeffbailey.us`.

After any later replacement, R-REPLACE step 7 ("redeploy apps") includes
`deploy/indexer/deploy.sh redeploy` (§8).

## 2. First deploy (I-4)

Plan the rollback first. There is no previous release, so a first deploy that is not ready
stops the container and leaves the PDS and the review app alone. `index.` then answers 503
until you fix it or remove the site
(`rm /pds/caddy/sites/index.caddy` and reload Caddy).

1. Set the production DID list (§5) and check that `aws ssm get-parameter` shows it. Without
   it the deploy stops with "put /openlore/prod/indexer/repo-dids first".
2. `deploy/indexer/deploy.sh install` checks IMDS and the sites hook, then installs the
   directories, scripts, units, the health timer and the Caddy site. Caddy must accept the
   config (`caddy validate`) before it reloads. Otherwise `index.caddy` is removed and the
   install aborts.
3. `deploy/indexer/deploy.sh deploy <sha>`. Read the summary. The post-checks are: `/healthz`
   ok, a POST search returns 200 JSON, GET `createRecord` returns 404, POST `/healthz` returns
   404, an 8 KB body is not 413, a 9 KB body is 413, and `PDS _health: n/n ok during deploy`.
4. Before the first pass, a POST search returns an empty result with 200, not a 5xx.
5. Wait for the next quarter hour (or run `deploy.sh trigger`). `deploy.sh status` shows exit 0
   and the counts, and a search returns results from at least 2 PDS hosts.
6. `curl -I http://index.openlore.jeffbailey.us/healthz` redirects to HTTPS, and the
   certificate is publicly trusted.

## 3. Update

`deploy/indexer/deploy.sh deploy <sha>`. The host pulls the digest while the old container
still serves, recreates the container, and waits up to 60 s for `/healthz` through Caddy. It
then records the release, re-enables the pass timer and starts one pass (which coalesces if a
pass is already running). If `/healthz` is not ready in time, it **restarts the previous digest
by itself** and exits 1. Expect at most 15 s of search downtime. You can deploy at any time of
day. A pass in flight is abandoned (the unit fails once) and the next slot recovers it.

## 4. Rollback

- `deploy/indexer/deploy.sh rollback`: the previous digest in `state/releases`. It takes about
  1 minute and restores no data, because releases have no schema change.
- If the rolled-back binary reports `indexer.store.unusable` or refuses its store, run
  `deploy.sh rollback --reset-index`. This moves `index.duckdb`, its `.wal` and
  `indexed_claims/` into `data/reset-<utc>/`, starts the previous digest and triggers a pass.
  The index rebuilds from the authors' PDSes within one or two passes, and search returns fewer
  results until then. The moved-aside copy is deleted at the next successful deploy, because
  the volume is a shared 5 GB.
- If the indexer harms the PDS (memory, CPU) and no digest is good, run `deploy.sh stop`. Caddy
  serves 503 for `index.`, and the PDS and the review app are unaffected. If alarms are on,
  A3 fires; that is expected.

### 4.1 Rollback drill (I-5, hard gate)

Deploy N, then deploy N+1. Run `rollback` and confirm that it returns to N in about a minute,
that `/healthz` is ok, and that the next pass exits 0 (`deploy.sh status`). Record the times.

## 5. Edit the DID list (no deploy)

```sh
export AWS_PROFILE=jeff AWS_REGION=us-east-1
P=/openlore/prod/indexer/repo-dids
aws ssm get-parameter --name $P --query Parameter.Value --output text | tr ', ' '\n\n' | sed '/^$/d' | sort > /tmp/dids.old
cp /tmp/dids.old /tmp/dids.new && $EDITOR /tmp/dids.new          # one bare did:plc / did:web per line
diff /tmp/dids.old /tmp/dids.new                                 # every "<" line will be PURGED on the next pass
aws ssm put-parameter --name $P --type String --overwrite --value "$(paste -sd, /tmp/dids.new)"
```

The next pass (within 15 min) renders and loads it. Check with `deploy.sh status`
(`repo_did_count`, `repo_dids_age_secs`). A malformed entry gives one exit-2 pass that names it
(A2 pages). The old index stays searchable and **nothing is purged**. Fix the value and wait for
the next pass.

The parameter is a Standard String. It holds up to 4 KB, which is about 110 `did:plc` entries.
Beyond that, move it to the Advanced tier.

### 5.1 Purge safety

- Removing a DID purges that author's claims at the start of the next pass that loads the list
  (ADR-082). Skips never purge. An empty, malformed or unreadable list purges nothing:
  `render-dids.sh` keeps the last good file and logs `indexer.dids.render_failed`.
- **Before a large edit**, run the `diff` above and count the `<` lines. If you are unsure,
  pause passes first with `deploy.sh stop`, edit, review, then `deploy.sh start`.
- **Deleted parameter:** passes keep the last good file, and the health line goes
  `not_live = 1` after 2 h (A3). `tofu apply` would recreate the parameter with the seed value,
  so first restore the production value (keep `/tmp/dids.old` as the backup;
  `get-parameter-history` only survives while the parameter exists).
- **Undo a wrong edit:** `aws ssm get-parameter-history --name $P` lists every version. Put the
  previous value back. The next one or two passes re-ingest the removed authors from their PDSes.
- Every purge logs `indexer.ingest.author_purged` with `did` and `claims_removed`.
  `deploy.sh status` lists them for the last good pass.

## 6. Memory re-measure gate (I-5, hard)

Run this with the production DID list in place and both apps on their final caps (indexer
128m, review app 192m with DuckDB 48 MB / 1 thread).

1. Open a session on the host (`aws ssm start-session --target <instance-id>`) and sample for
   20 minutes, every 5 s:

   ```sh
   for c in openlore-indexer review-app-review-app-1; do
     id=$(docker inspect -f '{{.Id}}' "$c"); echo "$c /sys/fs/cgroup/system.slice/docker-$id.scope/memory.peak"
   done
   grep -E '^(MemAvailable)' /proc/meminfo; grep '^pswpin ' /proc/vmstat   # minimum and delta over the window
   ```

   Also poll PDS `_health` every 10 s.
2. During the window, run `deploy.sh trigger` (a full pass), start a 10-repo review-app scan
   from the app UI, and run the search burst from the laptop:
   `seq 100 | xargs -P 10 -I{} curl -s -o /dev/null -w '%{http_code} %{time_total}\n' -X POST -H 'content-type: application/json' --data @search.json https://index.openlore.jeffbailey.us/xrpc/org.openlore.appview.searchClaims`.
   Every response must be 200, with p95 ≤ 1 s.
3. PASS when both `memory.peak` values stay under their limits, `MemAvailable` stays above the
   platform-architecture §6 floor, `pswpin` does not grow, and PDS `_health` is 200 throughout.
   Also check the DuckDB read-back (48 MB / 1 thread) in each app's startup probe event.
4. The next day, `CPUCreditBalance` for the last 24 h must be flat or rising.
5. **On FAIL:** lower `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` to 2 (edit `host/compose.yaml`,
   then redeploy), lower the review-app scan concurrency to 1, and re-measure. If it still
   fails, **stop. Moving to t4g.small is an operator decision.** It means editing
   `instance_type` in `deploy/environments/prod.json` and applying a plan that shows an
   in-place update (no replace). The instance stops and starts, which is **a second PDS outage
   of a few minutes**. Then redeploy both apps and re-measure. Record the decision in
   `docs/feature/indexer-deployment/devops/wave-decisions.md`.

## 7. Alarm test-fire (I-6, U-3)

Preconditions: `indexer_alarms_enabled = true` is applied, and
`aws sns list-subscriptions-by-topic` shows no `PendingConfirmation`. Each alarm must reach
ALARM and then return to OK, and both emails must arrive.

- **A2 (end to end, purge-safe):** append `,not-a-did` to the SSM value (§5), then run
  `deploy.sh trigger`. A refused list purges nothing, and the old index stays searchable. The
  ALARM email arrives within 15 min. Restore the value and run `deploy.sh trigger` again; the
  OK email follows the clean pass.
- **A1 (synthetic, about 75 min):** stop the pass timer (`deploy.sh stop` would also stop the
  container, so instead use an SSM session: `sudo systemctl stop openlore-indexer-pass.timer`).
  Create the stream `test-fire` in `/openlore/prod/indexer`. In quarter-hour P1, inject one
  line `{"event":"indexer.ingest.pass_summary","exit_code":3,"pass_id":"TEST-a1-1"}` with
  `aws logs put-log-events`, and in P2 inject `exit_code` 0: **no alarm**. Then inject exit 3 in
  P3 and P4: **ALARM**. Then inject exit 0 in P5: **OK**. Restart the timer
  (`sudo systemctl start openlore-indexer-pass.timer`). The TEST lines keep `summary_45m = 1`,
  so A3 stays quiet, and `deploy.sh status` ignores `TEST*` pass ids.
- **A3 (end to end):** `deploy.sh stop`, wait about 10 min (ALARM, `running = 0`,
  `healthz_ok = 0`), then `deploy.sh start` (OK).

The xtask test XP-13 (`health-timer.sh` with fake `docker`, `curl` and `aws`) covers the
stale-list and missing-heartbeat causes of A3. They are not fired live.

## 8. After a reboot or replacement

- **Reboot:** nothing to do. Docker restarts the container at the pinned digest, the timers
  survive, and `Persistent=true` runs one missed slot.
- **Replacement** (R-REPLACE step 7): after the PDS checks pass, run
  `deploy/indexer/deploy.sh redeploy`. It reinstalls the units and the Caddy site, and recreates
  the current digest from `/pds/indexer`.

## 9. When an alarm fires

`deploy.sh status` shows the latest `indexer.host.health` line. Act on the first failing field
(monitoring-alerting §2):

| Field | Look at |
|---|---|
| no lines at all | Is the host up? Then `systemctl status openlore-indexer-health.timer` over SSM; `deploy.sh redeploy` reinstalls the units |
| `running = 0` / `oom_killed = 1` | `deploy.sh host-status`; on OOM see §6; if the PDS is at risk, `deploy.sh stop` first |
| `healthz_ok = 0` | a hung `serve` (the timer restarts it after 3 failed runs), an unusable store (503), or a missing Caddy site |
| `summary_45m = 0` | the pass timer, exit 4 in the pass unit's journal, or broken awslogs shipping (`journalctl -u docker`) |
| `summary_check = error` | the host role cannot call `FilterLogEvents`: check that `indexer-iam.tf` is applied |
| `dids_age_s > 7200` | `render_failed` in the journal and the `host-dids` stream: SSM, IAM, or a deleted parameter |
