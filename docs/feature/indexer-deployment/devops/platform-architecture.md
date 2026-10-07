# Platform Architecture: indexer-deployment (DEVOPS)

> DEVOPS wave, 2026-10-06, Apex (nw-platform-architect), autonomous. Design documents only:
> nothing here has been built, applied or deployed.
>
> Inputs: `../wizard-decisions.md`, `../discuss/*` (outcome KPIs, ACs, user decisions),
> `../design/*` (architecture-design §9 is the contract list), ADR-075, ADR-080..083,
> `deploy/**` (the implemented review-app pattern: `deploy/review-app/*`, `review-app.tf`,
> `review-app-iam.tf`), `.github/workflows/ci.yml`, and `docs/feature/bluesky-claim-review-app/devops/*`.
>
> Companion files: `infrastructure-integration.md` (compose, units, Caddy, tofu, runbooks),
> `ci-cd-pipeline.md`, `observability-design.md`, `monitoring-alerting.md`,
> `kpi-instrumentation.md`, `branching-strategy.md`, `wave-decisions.md`.

## 1. Platform requirements (quantified)

| Requirement | Value | Source |
|---|---|---|
| Users | < 50 searchers, public read-only | WD-IXD-3 |
| Freshness | Claim searchable ≤ 30 min after publish; last successful pass ≤ 30 min old in ≥ 98% of hourly checks | KPI-IXD-2/3, NFR-IXD-1 |
| Availability | 99% monthly for search (about 7.3 h/month budget) | architecture-design §7 |
| Search latency | ≤ 1 s p95, including during a pass | NFR-IXD-3, AC-002.2 |
| Deploy downtime | ≤ 30 s search downtime per deploy; rollback ≤ 2 min; PDS and review app 200 throughout | NFR-IXD-2, KPI-IXD-6 |
| Pass cadence | Every 15 min; pass deadline 25 min; ≤ 1 pass at a time | WD-IXD-2, ADR-080 |
| DID count | 12-50 (worst-case pass about 13 min) | architecture-design §3 |
| Deploy frequency | On demand, laptop-initiated, about weekly during DELIVER | ADR-068/075 |
| Host | Shared t4g.micro (2 vCPU burstable, about 930 MB MemTotal + 1 GiB swap), AL2023 arm64 | user decision |
| Memory guardrail | Host `MemAvailable` > 128 MB during the concurrent peak; indexer peak RSS ≤ 100 MB | NFR-IXD-4 |
| Cost ceiling | Added ≤ $2/month; alarm resources ≤ $0.30/month | NFR-IXD-6, AC-004.6 |
| Team | 1 operator plus agents; trunk-based; no PRs | user decision |

## 2. Topology

```text
Internet ──HTTPS──▶ EIP ──▶ EC2 t4g.micro (openlore-pds-prod), AL2023 arm64
                              │
 compose "pds" (module-owned, /pds/compose.yaml)        compose "openlore-indexer" (/pds/indexer/compose.yaml)
 ┌─────────────────────────────────────────────┐        ┌───────────────────────────────────────────────┐
 │ caddy:2.8  :80/:443                         │        │ openlore-indexer (distroless, uid 65532)      │
 │   import /etc/caddy/sites/*.caddy (v1.7.0)  │ pds_   │  init: true, command serve, :8080 (no publish)│
 │   sites: app.caddy, index.caddy             ├default─┤  mounts: /pds/indexer/data   -> /data   (rw)  │
 │ pds (pinned digest) :3000                   │        │          /pds/indexer/config -> /config (ro)  │
 └─────────────────────────────────────────────┘        │  tmpfs /tmp (control socket, 0600)            │
 compose "review-app" (/pds/app/compose.yaml)            │  mem 128m no swap, oom_score_adj 900, cpus 0.5│
                                                        └───────────────────────────────────────────────┘
 host systemd:
   openlore-indexer-pass.timer   every 15 min (*:00/15)  -> render-dids.sh ; docker exec openlore-indexer openlore-indexer trigger
   openlore-indexer-health.timer every 2 min             -> probe /healthz via Caddy on loopback, DID-list age,
                                                            last pass_summary age (CloudWatch), memory -> one log line
 dockerd awslogs ──▶ CloudWatch Logs /openlore/prod/indexer (30 d) ──▶ 4 metric filters ──▶ 3 alarms ──▶ existing SNS topic
 host role: + ssm:GetParameter /openlore/prod/indexer/*, + logs write/filter on /openlore/prod/indexer (indexer-iam.tf)
 IMDS hop limit 1 (v1.7.0): the public indexer container cannot obtain the host role
```

DNS: the Cloudflare wildcard `*.openlore.jeffbailey.us` (DNS only) already resolves
`index.openlore.jeffbailey.us` to the EIP. No DNS change.

## 3. Existing infrastructure: reuse decisions

| Need | Existing asset | Decision |
|---|---|---|
| Host, EIP, data volume, TLS | `tofu-aws-pds` (v1.7.0 planned for the review app) | **Reuse.** No module change. Rides the single planned replacement (R-REPLACE). |
| Site routing | v1.7.0 M-1 sites hook (`/pds/caddy/sites` → `/etc/caddy/sites:ro`) | **Reuse:** drop `index.caddy` in, reload Caddy |
| Container isolation from host role | v1.7.0 M-2 (IMDS hop limit 1) | **Reuse. Hard precondition** for the first indexer start (a public container must never reach the role) |
| Command channel | SSM Run Command (`AmazonSSMManagedInstanceCore`) | **Reuse** for deploy, rollback, measure |
| Deploy tooling | `deploy/review-app/deploy.sh` (digest-only, CI-green + cosign gates, releases file, auto-rollback) | **Mirror** as `deploy/indexer/deploy.sh` (§5). Same gates, same shape, no DuckDB pre-copy (the index is rebuildable). |
| Image pipeline | `ci.yml` `review-app-build` / `review-app-image` | **Mirror** as `indexer-build` / `indexer-image` |
| Alert channel | SNS `openlore-pds-backup-alarm` (module output), subscribed out of band | **Reuse.** No new topic or subscription. |
| Alarm toggle pattern | `review_app_alarms_enabled` | **Mirror:** `indexer_alarms_enabled` (default false) |
| Host IAM pattern | `deploy/tofu/bootstrap/review-app-iam.tf` | **Mirror:** `indexer-iam.tf` |
| Plan gate | `deploy/check-plan.sh` | **Reuse unchanged.** Indexer resources are creates only. |
| Host health timer | `deploy/review-app/host/health-timer.sh` | **Twin, not extend:** the indexer may deploy before or after the review app and logs to its own group. Same script shape. |
| Secret renderer | `render-secrets.sh` (directory swap) | **Do not mirror** (ADR-081 H1). New `render-dids.sh`: per-file staging + `chmod 0444` + `rename(2)` inside a directory mount. |
| External reachability check | Review-app Route 53 health check (A-1) | **Not duplicated.** It already pages for a dead host, EIP or Caddy. The indexer's own probe covers the indexer route (§6). |

New components, each with "no existing alternative":

1. The `openlore-indexer` container and compose project: the deployable (ADR-080).
2. The GHCR image `ghcr.io/jeffabailey/openlore-indexer`: no indexer image exists.
3. `openlore-indexer-pass.timer`/`.service`: the user's schedule decision; nothing runs passes today.
4. `render-dids.sh`: SSM → file with last-good semantics (ADR-081). The review-app renderer has the wrong swap semantics.
5. `openlore-indexer-health.timer`: the liveness signal (A3). The review-app timer is tied to that app's lifecycle and log group.
6. CloudWatch log group, 4 metric filters, 3 alarms, 1 SSM parameter, IAM inline policy.

## 4. Container contract

| Aspect | Specification |
|---|---|
| Image | `ghcr.io/jeffabailey/openlore-indexer@sha256:<digest>`, deployed **by digest only**. CI tags `sha-<40hex>` and `main`. |
| Base | `gcr.io/distroless/cc-debian12:nonroot` (arm64), pinned by digest (same pin as the review app). Binary built in `rust:1-bookworm` (glibc 2.36). |
| Binary path | `/usr/local/bin/openlore-indexer` (on distroless `PATH`, so `docker exec … openlore-indexer trigger` works); `ENTRYPOINT ["/usr/local/bin/openlore-indexer"]`, `CMD ["serve"]` |
| Process | One long-running `serve`. `init: true` (docker-init as PID 1 forwards SIGTERM), `stop_grace_period: 5s`, `restart: unless-stopped`, `container_name: openlore-indexer` (pinned for `docker exec`, R-IXD-D7) |
| User / FS | 65532:65532, `read_only: true`, `tmpfs /tmp` (8 MB, mode 0700, uid 65532) for the control socket |
| Mounts | **Exactly two**: `/pds/indexer/data:/data` (rw) and `/pds/indexer/config:/config:ro` (a **directory** mount). Never `/pds`, never `docker.sock`. |
| Privileges | `cap_drop: [ALL]`, `no-new-privileges:true`, `pids_limit: 64` |
| Memory / CPU | `mem_limit` = `memswap_limit` = **128m**, `oom_score_adj: 900` (dies before the review app at 800 and the PDS), `cpus: 0.5`; DuckDB `memory_limit` **48 MB**, `threads` **1** (B9, read back by the probe) |
| Network | External `pds_default` only, no published port. Caddy reaches `openlore-indexer:8080`. |
| Logs | stdout JSON → dockerd `awslogs` → `/openlore/prod/indexer`, stream `indexer`, `mode: non-blocking`, `max-buffer-size: 4m` |
| Credentials | None. No env secret, no AWS keys, IMDS closed (hop limit 1). The DID list is public data rendered by the host. |

Environment (non-secret; architecture-design §9.2):

```text
OPENLORE_INDEXER_INDEX_PATH=/data/index.duckdb
OPENLORE_INDEXER_LISTEN_ADDR=0.0.0.0:8080
OPENLORE_INDEXER_REPO_DIDS_FILE=/config/repo-dids
OPENLORE_INDEXER_CONTROL_SOCKET=/tmp/openlore-indexer.sock
OPENLORE_INDEXER_PURGE_UNLISTED=1
OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB=48
OPENLORE_INDEXER_DUCKDB_THREADS=1
OPENLORE_INDEXER_PASS_DEADLINE_SECS=1500
OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES=4
OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS=30
# never set in production: OPENLORE_INDEXER_REPO_DIDS, OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP,
# OPENLORE_INDEXER_SOURCE_URL (no fallback in v1); PLC endpoint stays the default.
```

### 4.1 Review-app changes in the same release (B11, user decision)

| Change | Value | Why |
|---|---|---|
| DuckDB caps via review-app config (names are the crafter's, for example `REVIEW_DB_MEMORY_LIMIT_MB` / `REVIEW_DB_THREADS`) | 48 MB, 1 thread, read back by its probe | The documented "64 MB, 1 thread" was never set (R-IXD-D4) |
| `init: true` | on | SIGTERM reaches the app; stops stop costing the full 20 s grace |
| `mem_limit` / `memswap_limit` | 256m → **192m** (proposed; confirmed at the gate) | Frees 64 MB of worst case for the indexer; its measured peak 71 MB × 1.5 still fits |

These land in `deploy/review-app/host/compose.yaml` and ship with a normal review-app
`deploy.sh deploy <sha>` once the B11 binary is on main. The review-app runbook gains one line
under R-DEPLOY: "the probe refuses start if the DuckDB settings do not read back as configured".

## 5. Deployment strategy: Recreate, by digest, with automatic rollback

**Recreate** (stop, then start). Rejected alternatives:

- *Blue-green* (two `serve` containers behind Caddy): two processes cannot share one DuckDB
  file (ADR-065/080), and a second 128m container breaks the memory budget.
- *Rolling / canary*: one instance, one process, < 50 users; no traffic to split.

Recreate is acceptable because a stop is about 1 s with `init: true` and startup (open, WAL
replay, probe) is 2-5 s: about 10 s of search downtime against a 30 s budget. Caddy's
`handle_errors` serves a short 503 meanwhile. An in-flight pass is abandoned and recovers on
the next slot (idempotent upserts, resumable purge).

**Rollback is designed first:** the host keeps an append-only releases file; the previous digest
is always pullable (GHCR retention never deletes a released digest); there is **no schema change
and no data copy**. If a future digest ever changed the index schema, rollback uses
`--reset-index` (move the index aside; the next passes rebuild it, ADR-023), so a rollback never
needs a data restore. Mechanics are in `infrastructure-integration.md` §6.

## 6. Memory budget and the re-measure gate

The budget is architecture-design §3 (expected MemAvailable about 250 MB, pessimistic about 150,
cap-saturated about 50). The gate is **hard and measured on the host**, never computed:

| Signal (during a pass at production DID count + a review-app scan + a 100-request search burst) | Pass |
|---|---|
| Indexer cgroup `memory.peak` | ≤ 100 MB (cap 128m) |
| Review-app cgroup `memory.peak` | ≤ 128 MB (cap 192m) |
| Host `MemAvailable` minimum | > 128 MB |
| Swap-in during the window | about 0 |
| PDS `/xrpc/_health` | 200 throughout |
| DuckDB settings read back (both apps) | 48 MB, 1 thread |
| `CPUCreditBalance` over 24 h | not trending down |

Fail path, in order: (1) indexer concurrency 4 → 2, (2) review-app scan concurrency 2 → 1,
(3) re-measure, (4) **operator decision**: t4g.small (+$6.13/month), which is an in-place
stop/start, meaning a second, unplanned PDS outage of a few minutes. Never automatic. Runbook:
`infrastructure-integration.md` §9.6.

## 7. Cost (added per month, us-east-1 list prices)

| Item | Quantity | List price | Monthly |
|---|---|---|---|
| Compute, storage, EIP | co-located | — | $0 |
| SSM Standard parameter + `GetParameter` (96/day) | 1 | free | $0 |
| Metric alarms (standard resolution, single metric each) | 3 | $0.10 | **$0.30** |
| Log-filter metric `IndexerPassOutage` (data every pass, so every hour bills) | 1 | $0.30 | $0.30 |
| Log-filter metric `IndexerNotLive` (data every 2 min) | 1 | $0.30 | $0.30 |
| Log-filter metric `IndexerFailure` (data only when failing; prorated hourly) | 1 | ≤ $0.30 | about $0 |
| Logs ingestion: about 100 lines × 300 B × 96 passes/day + 720 health lines/day | about 95 MB | $0.50/GB | $0.05 |
| Logs storage (30-day retention, compressed) | < 0.1 GB | $0.03/GB | < $0.01 |
| Logs Insights (`deploy.sh status`/`kpi`, about 100 queries × ≤ 0.1 GB) | ≤ 10 GB scanned | $0.005/GB | ≤ $0.05 |
| `FilterLogEvents` from the health timer (720/day) | — | not billed | $0 |
| Route 53 health check | none added | — | $0 |
| **Total (list price)** | | | **about $1.00** (≤ $2 ceiling; alarm resources exactly $0.30, meeting AC-004.6) |

The CloudWatch always-free tier (10 custom metrics, 10 alarms, 5 GB logs) likely absorbs all of
it: after this feature the account has 7 custom metrics (PDS/Backup, 3 review-app, 3 indexer) and
8 alarms (1 backup, 4 review-app, 3 indexer). DESIGN's estimate (≤ $0.40) omitted the filter
metrics; this table supersedes it. t4g.small, if adopted at the gate, adds $6.13.

## 8. Simpler alternatives considered and rejected

1. **Host cron instead of a systemd timer.** No `Persistent=`, no overlap guard, no unit
   result. Rejected (technology-stack).
2. **Second container for passes.** Collides on the DuckDB lock on 100% of passes (ADR-080).
3. **Route 53 health check for `index.`** ($1.50/month, over half the ceiling) and a fourth
   alarm. The review app's external check already covers host, EIP and Caddy; the indexer's
   route is probed through Caddy on loopback with real TLS. Rejected on cost.
4. **Metric-math or composite alarm for liveness.** A composite needs two child alarms plus a
   $0.50 composite; metric math cannot give the 45-minute heartbeat and the 10-minute health
   failure different windows in one alarm. Folding all three liveness inputs into one host line
   gives one $0.10 alarm (monitoring-alerting §1).
5. **`docker logs --since 45m` for the heartbeat** (no extra IAM). It cannot see a broken
   awslogs pipeline, which would silently blind A1 and A2, and it resets on every deploy.
   Rejected for a scoped read on the indexer log group (`logs:FilterLogEvents`).
6. **Per-IP rate limiting.** Needs a custom Caddy build in a module-owned container (ADR-083).
