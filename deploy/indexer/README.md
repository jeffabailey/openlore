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

**Moving to Lambda (scale-to-zero-hosting):** §1-§9 describe the host, which still serves. §10-§17
are the operator's runbook for the Lambda path: apply order, stages, verbs, recovery and evidence.

## Files

| Repo file | On the host | What it does |
|---|---|---|
| `deploy.sh` | runs from the laptop, and on the host as `deploy.sh host <mode>` over SSM | install, deploy, rollback, stop/start, trigger, status, standby, resume |
| `lambda.sh` | laptop only; `deploy.sh lambda <verb>` sources it | the Lambda verbs (§14) |
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

**Not narrowed, on purpose (decided 2026-10-13).** The indexer is attached to `pds_default`
only and publishes no port, so every connection it accepts already comes from that one subnet
(172.18.0.0/16 on the go-live host; read it with
`docker network inspect pds_default --format '{{range .IPAM.Config}}{{.Subnet}} {{end}}'`). Narrowing the list to that subnet would exclude no peer
that can reach it, and the subnet can change when the instance is replaced (the network is
recreated), where a stale value would put every client in Caddy's one bucket. The default-pool
value never needs updating. A bad entry makes the indexer refuse to start (exit 2, naming the
variable), so a typo shows at once.

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

The go-live order for the indexer **and** the review app is the one
[Go-live checklist](../README.md#go-live-checklist) in `deploy/README.md`: tofu-aws-pds v1.7.0
first, the one instance replacement (R-REPLACE, which also creates `indexer.tf` with alarms
disabled), the DID list, the public image, install and first deploy, the rollback drill, the
memory gate, enabling the alarms and then test-firing them, and the announcement. This runbook
keeps only the detail of the indexer's own steps.

*Superseded 2026-10-09 (fix-go-live-runbook-gaps): the I-0..I-7 list that stood here disagreed
with the review app's sequence on the alarm enable vs test-fire order; the checklist replaces it.*

`deploy.sh install` fails closed. It refuses unless `/pds/caddy/sites` exists, the PDS Caddy
imports it, the probe image (`curlimages/curl`, pinned by digest; override with
`IMDS_PROBE_IMAGE=<image>@sha256:...`) runs `curl --version` on `pds_default`, **and** the IMDS
PUT from that container then ends with curl exit 7 (cannot connect) or 28 (timeout). Any other
outcome refuses.

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
3. `deploy/indexer/deploy.sh deploy <sha>` (the first time, with `INDEXER_READY_WAIT_S=300`:
   the readiness wait includes certificate issuance for `index.`). Read the summary. The post-checks are: `/healthz`
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

The runnable gate (a 5 s / 20 min sampling loop that prints `memory.peak`, literal PASS
thresholds, a defined search body and a load under the rate limit) is the *Memory gate* step of
the [Go-live checklist](../README.md#go-live-checklist). Re-run it after any change to the caps,
the DID list size or the instance type. Its fail path lowers
`OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` to 2 and `OPENLORE_REVIEW_SCAN_CONCURRENCY` to 1.

## 7. Alarm test-fire (I-6, U-3)

The alarms are enabled first and then test-fired: the exact A1, A2 and A3 procedures (the A1
`put-log-events` commands into the `test-fire` stream, and the warning that a pass timer stopped
for more than 2 h fires A3) are steps 14 and 15 of the
[Go-live checklist](../README.md#go-live-checklist).

The xtask test XP-13 (`health-timer.sh` with fake `docker`, `curl` and `aws`) covers the
stale-list and missing-heartbeat causes of A3. They are not fired live.

## 8. After a reboot or replacement

- **Reboot:** nothing to do. Docker restarts the container at the pinned digest, the timers
  survive, and `Persistent=true` runs one missed slot. **In standby it differs**: the timers stay
  off and a stopped container stays stopped, so run the reboot check in §15 to confirm that.
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
| `search_status = 500` | the index cannot serve searches: read the `indexer.search.store_error` events (dimension only, no query); `deploy.sh redeploy` reopens the store. A 503, 429, 408 or 0 sets `search_ok = 0` but never `not_live` |
| `summary_check = error` | the host role cannot call `FilterLogEvents`: check that `indexer-iam.tf` is applied |
| `dids_age_s > 7200` | `render_failed` in the journal and the `host-dids` stream: SSM, IAM, or a deleted parameter |

## 10. Lambda path: before you start

**Nothing here has run on real AWS.** Every verb has run on stubs only. The first real run of each
verb is an operator stage. Expect surprises, and record each first run (§17).

Run every Lambda verb through the wrapper. It sets the profile and region, and gives crane a
throwaway `DOCKER_CONFIG` for the ECR login:

```sh
deploy/ops.sh indexer lambda status
```

Below, `deploy.sh <verb>` is short for `deploy/ops.sh indexer <verb>`.

The tools: `aws` and `jq` for every verb. `lambda deploy`, `seed` and `republish` also need `gh`,
`cosign`, `crane` and `curl`. `lambda stop` needs only `aws` and `jq`, so it runs on any machine.

**Your credentials need `ecr:BatchDeleteImage`.** No IAM for it exists in this repo. Without it, a
failed copy's cleanup prints `could not delete <digest> from ECR` and keeps the copy. The copy uses
one of the 10 retained image slots. Nothing else breaks.

Who does what: every verb, every `tofu apply`, both Cloudflare records and every judgement call are
yours. The only automatic actors are the two schedules, the alarms and the budget.

## 11. First apply (Stage 0)

Use the plan, gate, apply shape from `deploy/README.md`, in `deploy/tofu/environments/prod-indexer`.
Carry every `-var` forward to each later plan. A dropped value returns to its default, and
`schedules_enabled` defaults to false.

1. Apply the parts the image needs. No function exists yet.

   ```sh
   cd deploy/tofu/environments/prod-indexer
   export AWS_PROFILE=jeff
   tofu init
   tofu plan -out=tfplan \
     -target=aws_ecr_repository.indexer -target=aws_ecr_lifecycle_policy.indexer \
     -target=aws_ecr_repository_policy.indexer \
     -target=aws_s3_bucket.index -target=aws_s3_bucket_versioning.index \
     -target=aws_s3_bucket_lifecycle_configuration.index \
     -target=aws_s3_bucket_server_side_encryption_configuration.index \
     -target=aws_s3_bucket_public_access_block.index \
     -target=aws_s3_bucket_ownership_controls.index -target=aws_s3_bucket_policy.index \
     -target=aws_iam_role_policy.search -target=aws_iam_role_policy.pass \
     -target=aws_iam_role_policy.probe -target=aws_iam_role.scheduler \
     -target=aws_cloudwatch_log_group.lambda -target=aws_acm_certificate.index
   ../../../check-plan.sh tfplan
   tofu apply tfplan
   ```

   Leave out `aws_iam_role_policy.scheduler`: it needs the aliases, which need the image. Add the
   ACM validation CNAME in Cloudflare. `tofu state show aws_acm_certificate.index` prints it under
   `domain_validation_options`.
2. Copy the image and read its URI. This runs the verification gates and the ECR copy only.

   ```sh
   deploy.sh lambda seed <40-hex sha>      # prints bootstrap_image_uri = "..."
   ```

3. Full apply, with schedules off:

   ```sh
   tofu plan -out=tfplan -var 'bootstrap_image_uri=<uri>' -var schedules_enabled=false
   ```

   **If AWS refuses `openlore-index-strip`:** the response headers policy removes `x-amzn-*` headers
   by name, and CloudFront may refuse to remove `X-Amzn-RequestId`. The apply stops with an error
   naming the header, and the distribution is not created. Do not work around it in the console.
   Fix it in the repo: take the refused header out of the policy in `edge.tf` and out of
   `the_edge_strips_every_header_that_names_the_origin`, and make `edge_checks` in `lambda.sh` allow
   that one header. Otherwise every deploy fails its edge check and flips search back at step 14.
   That header then reaches clients, against the design's `x-amzn-*` line, so record the change as a
   design change. Wait for CI, then re-apply.
4. First deploy. There is no pass record yet, so the probe gate needs the acceptance:

   ```sh
   LAMBDA_ACCEPT_NOT_LIVE=1 deploy.sh lambda deploy <40-hex sha>
   ```

5. Only now turn the schedules on: `-var schedules_enabled=true`, plan, gate, apply. Nothing
   scheduled runs before this.
6. **A day later**, apply again with `-var cost_allocation_tag_active=true`. AWS refuses to activate
   a tag key before it appears in billing data, which takes up to 24 h after the first tagged
   resource exists. If it still refuses, wait and apply again. The budget reads $0 until then.

## 12. Stages

| Stage | Who serves the public name | Ends when |
|---|---|---|
| 0 bootstrap | host | the metric check is complete and alarm actions are on |
| 1 trial | host | 7 consecutive green days, plus the trial's other checks |
| 2 cut-over | host, then Lambda | step 7 is clean |
| 3 soak | Lambda | 7 green days |
| 4 decommission | Lambda | the checklist is done |

### Stage 0: bootstrap (one evening)

1. Run §11 steps 1-5. The post-checks and edge checks in the deploy must be green.
2. The first scheduled pass exits 0 with `published: true`. Read it in `lambda status`.
3. `deploy.sh lambda probe` shows `not_live` 0.
4. Test-fire with alarm actions still off, so these prove the alarm moves, not that mail arrives:

   ```sh
   deploy.sh lambda test-alarm a1     # and a2 a3 a4 a5 a8 a9 a10 a11 errors throttles
   deploy.sh lambda test-alarm a11 --metric
   ```

5. `lambda status` must show the A8 and A9 metrics seen, and A11's on its candidate series. The
   `:live` dimension is checked for shape only.
6. Set `-var lambda_alarms_enabled=true`, plan, gate, apply.
7. Record P-15, P-17 and the metric check (§17).

**Stage 1 does not start until the metric check is complete.**

### Stage 1: trial, 7 consecutive green days

The host serves the public name. Lambda logs to `/openlore/prod/indexer-lambda`.

Each day read `deploy.sh lambda kpi 1` and `deploy.sh lambda parity`. `parity` refuses unless the
minute is 7 past a quarter hour (:07, :22, :37, :52).

A **green day** is a UTC day where all of these hold:

1. Freshness ≥ 23 of 24 hourly checks, and ≥ 165 of 168 over the 7 days (≥ 98%).
2. 0 exit-2 passes, 0 `publish_conflict`, 0 A1-T/A2-T/A3-T alarms, 0 A6 errors, 0 A10 and 0 A11.
3. Probe `not_live = 0` in ≥ 285 of 288 lines, never 2 in a row.
4. Parity: the three canned searches match the host's.
5. Init p95 < 2,000 ms; probe `search_ms` p95 < 200 ms; max memory ≤ 400 MB (search, pass).
6. No A4, A7, A8, A9 or budget alert.

**A red day resets the count to zero.** The 7-day freshness figure restarts with it. Test-fires
(`TEST` ids, and the `errors` invoke at the time the verb prints) do not make a day red.

Also during the trial:

- [ ] Check the saved queries return rows. `indexer-lambda/cold-starts` and `indexer-lambda/memory`
      assume the stream format of a log group shared by the three functions. No rows, or an empty
      `function_name`, means the format differs: fix the `parse` before you trust them or the
      memory figures in `lambda kpi`.
- [ ] Measure connect time on Lambda against the 10 s init budget (§17). 6-15 s was seen on macOS.
- [ ] `lambda measure` passes for the digest you will cut over. A new digest runs it again.
- [ ] Time a rollback drill, N → N-1, ≤ 2 min. A second rollback is refused, so return with
      `lambda deploy <sha of N>`.
- [ ] Time a `lambda stop` and `lambda start` drill, the edge in both directions.
- [ ] Try P-18.
- [ ] Take 5 KPI-IXD-2 samples.

### Stage 2: cut-over (one quarter hour)

Start at minute 1 after a quarter-hour pass.

0. Check that `lambda measure` passed for this digest: its recorded `measure: PASS <digest>` line
   must name the same digest as `live` in `lambda status`. If not, stop and run it.
1. `deploy.sh standby --timers`. The container keeps serving.
2. Tofu: `-var phase=cutover`, plan, then gate with `OPENLORE_ALLOW_DELETE=1` for the trial filters
   and alarms only. Read the plan first. Apply. Then `deploy.sh lambda republish`, so `live` carries
   the new log target. The first probe line must reach `/openlore/prod/indexer` by minute 12. If this
   step will overrun, stop and run the abort below.
3. Start `deploy.sh lambda cutover-watch` in a second terminal. In Cloudflare create the exact CNAME
   `index.openlore` → `<distribution>.cloudfront.net`, DNS-only, TTL 60.
   (`tofu output -raw distribution_domain` gives the domain.)
4. Check through the public name: `/healthz` and search 200, a `via`/`x-cache` header from
   CloudFront, edge 404s.
5. Wait ≥ 10 min for the wildcard A record to drain. Press Ctrl-C on the watch and read its summary.
   A gap above 30 s fails KPI-IXD-6: investigate before step 6, while the host can still serve.
6. `deploy.sh standby --container`.
7. Watch the next quarter-hour pass and two probe periods. A1-A3 must be OK. Run
   `deploy.sh lambda kpi 1`.

**Abort before step 6:**

1. `deploy.sh resume`.
2. Delete the CNAME if it exists.
3. Tofu: `-var phase=trial`, plan, gate, apply.
4. `deploy.sh lambda republish`.

### Stage 3: soak, 7 green days

The same green-day rule, on the prod group and A1-A3. Parity no longer applies, and `lambda parity`
refuses once the public name serves the edge. Standby lasts through a host reboot. **After any
reboot, confirm it with the check in §15.**

**Rollback to the host (soak only, target ≤ 10 min):**

1. `deploy.sh lambda pass-off` and `deploy.sh lambda probe-off`.
2. `deploy.sh resume`.
3. `deploy.sh trigger`.
4. Delete the Cloudflare CNAME. The wildcard sends the name back to the host in ≤ 60 s.
5. Tofu: `-var phase=trial`, plan, gate, apply. This sets pass and probe concurrency back to 1, so
   they run again from here. `live` keeps its prod log target until step 6, and that group's write
   grant is gone, so their lines are lost until then.
6. `deploy.sh lambda republish`. Not before step 5: its gates invoke pass and probe, and at
   concurrency 0 each throttles for 870 s and then refuses.

After decommission there is no host rollback. A return to the host is a reinstall from git history.

### Stage 4: decommission

- [ ] Host, over SSM: stop and remove the `openlore-indexer` container; disable and remove the pass
      and health timers and services; remove `/pds/caddy/sites/index.caddy`, validate, reload Caddy;
      remove `/pds/indexer/{bin,config,state,data}`.
- [ ] Prod root: replace `indexer.tf` with `removed { … lifecycle { destroy = false } }` for the 13
      resources (SSM parameter, log group, 4 filters, 3 alarms, 4 queries). Drop the three indexer
      outputs and `indexer_alarms_enabled`. The plan shows only forgets. `check-plan.sh`, apply.
- [ ] Prod-indexer root: add the same 13 with `import { to = …, id = … }` (parameter name, log group
      name, `<group>:<filter>` for filters, alarm names, query definition ids). Replace the
      remote-state log group and parameter reads with local references, and keep the remote state
      for the SNS topic. The plan shows 13 imports and at most in-place description updates. Apply.
      Delete the `import` blocks in a follow-up commit.
- [ ] Bootstrap: delete `indexer-iam.tf`. The plan shows one policy delete. `OPENLORE_ALLOW_DELETE=1`.
- [ ] Repo: remove `deploy/indexer/host/*`, the host verbs in `deploy.sh` (install, redeploy,
      host-status, measure, host-timer, standby, resume, the SSM `remote` path), the `indexer-host`
      job in `deploy-pds-check.yml` and its bats tests. Keep `serve-smoke`.
- [ ] Docs: rewrite this file as the Lambda runbook; update the go-live section of
      `deploy/README.md`; mark ADR-075's indexer amendment as done; update the brief.
- [ ] Remove the trial log group once its 30 days have passed.
- [ ] Record the 30-day KPI baseline in `docs/evolution/`.

## 13. Which alarm names are current

`lambda status`, `kpi` and `test-alarm` follow where `live` logs. In the trial A1-A3 mean
`openlore-indexer-lambda-*`. After cut-over they mean the host's names, on the prod group. A tofu
change reaches `live` only through `lambda republish`, and `lambda status` prints `DRIFT` until then.

## 14. Lambda verbs

All of these have run on stubs only (§10). A refusal starts with `refusing` and changes nothing.

| Verb | What it does | Know this |
|---|---|---|
| `lambda deploy <git-sha\|digest>` | Blue-green by alias: all gates, then `live` flips, search first | Takes a 40-hex sha or `sha256:<64 hex>`. The gates can take 28.5 min. A stop before the flip leaves nothing flipped. `LAMBDA_ACCEPT_NOT_LIVE=1` accepts the probe gate alone. `LAMBDA_ACCEPT_EXIT_3=1` accepts exit 3 alone. **Refused if any function publishes nothing new** (§16). A failure in step 15 or an interrupt has its own recovery (§16). |
| `lambda seed <git-sha\|digest>` | Gates and ECR copy only; prints `bootstrap_image_uri` | First apply only (§11). |
| `lambda republish` | Republishes each function's live image, through the same gates and flips | Run it after any tofu change to function configuration. Takes no arguments. Refuses if the live image is not in ECR. **Refused if any function publishes nothing new** (§16): a change that reaches only some functions needs a deploy of a new build. |
| `lambda rollback` | Every `live` back to its recorded `prev=`; `candidate` follows | Prints the pre-publish index version. **`lambda rollback --to <sha>` is refused today**: rollback takes no arguments. **A second rollback is refused**: a rollback leaves no `prev=`. A partly failed rollback cannot be rerun (§16). Afterwards `lambda status` shows `$LATEST` drift. That is expected. |
| `lambda stop [--wait]` | The kill switch: search concurrency 0, then the distribution off | Lambda stops in seconds, the edge in minutes. Pass and probe keep running. Expect A3 within about 10 min, staying in ALARM while stopped. A deploy during a stop fails its post-checks. |
| `lambda start` | Search concurrency back to 3, the distribution on | Safe to run twice. The raw URL answers at once, the public name when the distribution is `Deployed`. |
| `lambda pass-off` | Pass concurrency 0 | A later apply sets it back to 1. |
| `lambda pass-on` | Pass concurrency 1 | |
| `lambda probe-off` | Probe concurrency 0 | A later apply sets it back to 1. |
| `lambda probe-on` | Probe concurrency 1 | |
| `lambda trigger` | One real pass on `live`, payload `{}` | Prints `pass_id` and exits with the pass's exit code. |
| `lambda probe` | One real probe on `live` | Prints the health line. Judging it is yours. |
| `lambda reset-index [--trigger]` | After you type `reset`: a delete marker on the index | Versions stay. Search serves an empty index within 60 s. `--trigger` runs the rebuilding pass now. Without it, A10 can fire once. |
| `lambda restore-index <version-id>` | Copies that version back as a new current version | **Refused when minute % 15 is 14, 0 or 1**, when a pass is due or running. Wait and rerun. The next pass ingests on top, so this is a starting point, not a pin. A10 can fire once. |
| `lambda status` | The whole estate: versions, drift, concurrency, schedules, logs, index, alarms, budget, edge, metric check | Takes no arguments. |
| `lambda measure` | The R-STZ-1 and R-STZ-2 gate. Exit 1 blocks Stage 2 | Pass: 5 cold calls < 3 s, 20 warm calls < 200 ms. Refuses unless search is at 3 and pass at 1. Cold calls use the raw `candidate` URL. Warm calls use the raw URL and the CloudFront domain. **Cold through CloudFront is not measured**: the edge reaches only `live`. Run it from your laptop. Prints `measure: PASS <digest>` or `FAIL`. |
| `lambda kpi [days]` | KPIs over the current log group, 1-30 days, default 7 | Read this, not the saved `indexer/kpi-freshness` query, which counts gate and test lines. |
| `lambda parity` | The host and the edge answer alike for three DIDs | Trial only. Refused unless minute % 15 is 7. If a pass was in flight, rerun it 15 min later. |
| `lambda test-alarm <target>` | Fires one alarm and waits for ALARM, then OK | Targets: `a1`, `a2`, `a3`, `a4`, `a5`, `a8`, `a9`, `a10`, `a11`, `errors`, `throttles`, `a11 --metric`. `a1` turns the pass off and on itself, and takes about an hour. `a4`, `a8`, `a9`, `a11` test the notification path only. `errors` is a real failing probe invoke. `throttles` sets search concurrency 0 for a moment. `a11 --metric` is Stage 0 only and refuses once alarm actions are on. |
| `lambda cutover-watch` | Polls the public name once a second | Stage 2 only. Runs until you press Ctrl-C, then prints the summary. Exit 1 if the gap passed 30 s. |

## 15. Host verbs

| Verb | What it does | Know this |
|---|---|---|
| `deploy.sh standby --timers` | Stops and disables the pass and health timers; the container keeps serving | A reboot does not start them again. `resume` re-enables both. |
| `deploy.sh standby --container` | Stops the container; files, units, Caddy site and releases stay | `index.` on the host answers 503. A stopped container stays stopped across a reboot. |
| `deploy.sh resume` | Reinstalls, starts the container, then both timers | Runs no pass. Run `deploy.sh trigger` for that. Refuses if nothing was ever deployed. |

`standby` with no flag, or both flags, is refused.

**Reboot check.** Standby should last through a reboot. Confirm it after any host reboot in standby
(Stage 2 from step 1, and all of Stage 3):

```sh
deploy.sh host-status
```

- Both timers must show no next run. A disabled timer can drop off the list altogether, which reads
  the same. Any `openlore-indexer-*` timer with a next run was started by hand or by a host verb since
  standby. Run `deploy.sh standby --timers` at once. Until you do, the host writes health lines beside
  the probe, and the pass timer fires its missed slot.
- In Stage 3 the container must not be running. If it is, run `deploy.sh standby --container`.

## 16. Recovery

**By-hand alias flip.** The commands below all use this shape:

```sh
F=openlore-indexer-pass        # or openlore-indexer-search, openlore-indexer-probe
aws lambda get-alias --region us-east-1 --function-name $F --name live   # its version and prev=
aws lambda update-alias --region us-east-1 --function-name $F --name live \
  --function-version <target version> --description "<record>"
```

### A deploy failed during step 15

The deploy undoes it. Search and `candidate` go back to the version they replaced, each under the
record it had, and so does the pass if it had flipped. The last line names the version each function
ended on. If all three show the old version, find out why the flip failed, then run the deploy again.

If an undo failed too, the verb prints `could not move <function> back to version <n>` and the record
that function held, and the last line shows it still on the new version:

1. Flip each named function to `<n>` by hand, with the printed record as `--description`.
2. If search was one of them, move `candidate` to its `<n>` too, with the command under
   [You interrupted a deploy](#you-interrupted-a-deploy-during-the-edge-checks).
3. Run `deploy.sh lambda status`. All three `live` must show the old version.

Do not run `lambda rollback` before that: a function that did not flip still records the version
before it, so it would go back one too far. Do not rerun the deploy either: it refuses with
`nothing changed`, because a function still on the new version answers with the version it holds.

### A deploy or republish refused with `nothing changed`

The refusal names each function whose publish answered with the version `live` already holds. Lambda
publishes no new version for unchanged code and configuration, and a flip to that version would make
`prev=` name the version it now holds, so the real rollback target would be gone. The refusal comes
before `candidate` moves, so no alias moved. A version a changed function did publish stays until a
later prune, and `$LATEST` holds what the run set.

- **All three named:** nothing needs deploying. `lambda status` shows what `live` runs.
- **Some named, after a tofu change:** the change reached only the others, and it cannot reach `live`
  through `republish`. Run `lambda deploy` on a newer build, which publishes all three.
- **Some named, after a flip that stopped partway:** those functions are ahead of the others. Do not
  try again: read the state with `lambda status`, then recover with the part of this section that fits.

### You interrupted a deploy during the edge checks

Search stays flipped, and its edge checks did not finish. `candidate` is on the new version. Pass
and probe did not flip. Put search back:

1. Read `prev=<old>` from search's `live` alias.
2. Flip search `live` to `<old>` with the record `rolled back from <new> <utc>`.
3. Move `candidate` too:
   `aws lambda update-alias --region us-east-1 --function-name openlore-indexer-search --name candidate --function-version <old>`
4. Run `deploy.sh lambda status`, then run the deploy again.

The record has no `prev=`, so `lambda rollback` refuses until the next deploy. That is safe.

### A rollback failed partway

The verb prints `<function> stayed on version <n>: could not flip live back`. Functions before it,
in search, pass, probe order, are already back. A rerun is refused, because those have no `prev=`.
So flip the rest by hand:

1. For the named function and each one after it, read `prev=<target>` from its `live` alias.
2. Flip it to `<target>` with the record `rolled back from <n> <utc>`. On the pass, keep the id:
   `rolled back from <n> <utc> idx=<id>`.
3. If `candidate` did not follow, move it to search's `<target>` with the command above.
   The verb prints `every live is back, but candidate did not follow` when only this is left.

### Other failures

| What fails | Do this |
|---|---|
| Bad code, any function | `deploy.sh lambda rollback` |
| Abuse or runaway spend | `deploy.sh lambda stop` |
| `refusing: could not read the index object` | The read failed, and only a 404 counts as an empty bucket. Read the error. Without `s3:ListBucket` a missing object answers 403. Nothing flipped; fix it and run the verb again |
| A bad index | `lambda rollback` first if new code wrote it. Then `lambda reset-index --trigger`, or `lambda restore-index <id>`. The id is in the deploy summary, the pass alias's `idx=`, or the rollback output |
| Traffic after cut-over | Rollback to the host, §12 Stage 3 |
| Infrastructure | Revert the commit, saved plan, `check-plan.sh`, apply; then `lambda republish` if function config changed |

## 17. Evidence

DELIVER did not touch production, so these are owed. Record each in
`docs/evolution/scale-to-zero-hosting-evidence.md`, with the date, the digest and the output.

| Item | Stage | How to get it | Passes when |
|---|---|---|---|
| P-15 | Stage 0 | Before the first production pass, run the simulator below on the applied pass policy, both keys, present and absent. Keep the output. The first production publish and write probe finish the proof | Allowed with a key, implicit deny with neither. A wrong condition shows as `publish_failed` after the flip |
| P-17 | Stage 0 | Run the simulator below on the list statement. Keep `/healthz` from the first deploy on the empty bucket | `/healthz` 200 with a `null` pass time. If init refuses with 403, change the condition in `iam.tf` to `StringLikeIfExists`, and its row in `scale_to_zero_ci.rs`, re-apply, and record that the wider fallback is in use |
| Metric check | Stage 0 | Keep the metric check from `lambda status`, after `lambda test-alarm a11 --metric` | A8 and A9 metrics seen, A11's on its candidate series, `:live` for shape only |
| Test-fires | Stage 0 | Keep each `lambda test-alarm` output | Each reaches ALARM, then OK |
| First real runs | Stage 0 | One line per verb, at its first real run in any stage: the date, the outcome, and anything that differed from §14 | Every verb in §14 and §15 has a line |
| Saved queries | Stage 1 | Run `indexer-lambda/cold-starts` and `indexer-lambda/memory` in Logs Insights. Keep one result of each | Rows per function |
| Connect time | Stage 1 | Read `init_max_ms` from `indexer-lambda/cold-starts`, per function. Init includes the connect, so this is its upper bound. `lambda measure` prints recent init durations too | Under the 10 s init budget (6-15 s was seen on macOS). Green days need init p95 < 2,000 ms |
| P-5 | Stage 1 | `deploy.sh lambda measure` from your laptop. Keep the `measure: PASS <digest>` line, min/median/max and the init durations | PASS for the digest cut over. Another vantage point needs a recorded user waiver. Record that cold through CloudFront is not measured |
| P-18 | Stage 1 | Run the 20 concurrent raw-URL calls below, up to 5 times. Keep the counts | At least one 429, and nothing but 200 and 429. If none overlaps, record it as unverified |
| 140 MB post-pass steps | Stage 1 | Time the local post-pass steps (fingerprint read, WAL check, handle close, staging) with an index near 140 MB. No verb does this, and no index that size exists today. If none exists by the end of the trial, record `not measured` and `index_bytes` from `lambda status`. A5 fires at 100 MB: measure then | ≤ 15 s. An overrun moves `PASS_DEADLINE_SECS` earlier |
| 140 MB query | Stage 1 | Time a search and a reload with an index near 140 MB. Same limits and same fallback as the row above | The query fits the 1 s floor, and the reload fits 3 s |
| Rollback drill | Stage 1 | Time `lambda rollback`, N → N-1 | ≤ 2 min |
| Stop and start drill | Stage 1 | Time `lambda stop` and `lambda start`, the edge in both directions | The figures are the evidence |
| KPI-IXD-2 | Stage 1 | 5 timed samples, publish to search, against the CloudFront domain | ≤ 30 min, 5 of 5 |
| Green days | Stage 1 | `lambda kpi 1` and `lambda parity` for each UTC day. Keep the same for Stage 3, without parity | 7 consecutive in each stage |
| KPI-IXD-6 | Stage 2 | Keep the `lambda cutover-watch` summary | Gap ≤ 30 s |
| KPI-IXD-1 | Stage 2 | A fixed broad search through the public edge; resolve each author DID to its PDS. On cut-over day, then weekly | ≥ 2 PDS hosts, 100% of rows attributed |
| 30-day KPI baseline | Stage 4 | `lambda kpi 30` at close, in `docs/evolution/` | Recorded |

The simulator, for P-15 and P-17:

```sh
POLICY=$(aws iam get-role-policy --role-name openlore-indexer-pass \
  --policy-name openlore-indexer-pass --query PolicyDocument --output json)
BUCKET=arn:aws:s3:::openlore-indexer-index-091153021562
OBJECT=$BUCKET/index/index.duckdb
sim() {
  aws iam simulate-custom-policy --policy-input-list "$POLICY" \
    --query 'EvaluationResults[].EvalDecision' --output text "$@"
}
KEY_SET='ContextKeyValues=x,ContextKeyType=string'
# P-15: expect allowed, allowed, implicitDeny
sim --action-names s3:PutObject --resource-arns "$OBJECT" \
  --context-entries "ContextKeyName=s3:if-match,$KEY_SET"
sim --action-names s3:PutObject --resource-arns "$OBJECT" \
  --context-entries "ContextKeyName=s3:if-none-match,$KEY_SET"
sim --action-names s3:PutObject --resource-arns "$OBJECT"
# P-17: expect allowed
sim --action-names s3:ListBucket --resource-arns "$BUCKET" \
  --context-entries 'ContextKeyName=s3:prefix,ContextKeyValues=index/index.duckdb,ContextKeyType=string'
```

The simulator reads the policy only. It does not prove S3 sends the keys, and CI cannot check this.

P-18, 20 concurrent calls:

```sh
RAW=$(aws lambda get-function-url-config --region us-east-1 \
  --function-name openlore-indexer-search --qualifier live --query FunctionUrl --output text)
seq 20 | xargs -P 20 -I{} curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  -H 'content-type: application/json' --data '{"dimension":"subject","value":"p18"}' \
  "${RAW%/}/xrpc/org.openlore.appview.searchClaims" | sort | uniq -c
```
