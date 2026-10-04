# Platform Architecture: bluesky-claim-review-app (DEVOPS)

> DEVOPS wave, 2026-10-04, Apex (nw-platform-architect), autonomous. These are design documents
> only. Nothing here has been applied, built or deployed.
>
> Inputs: `../design/*` (including the SPIKE RESULTS), ADR-071..076, `../discuss/outcome-kpis.md`,
> `../discuss/requirements.md` (I-BRA-1..8, NFR-BRA-1..9), `deploy/**`, `.github/workflows/*`, and
> the module source `jeffabailey/tofu-aws-pds` at v1.6.0.
>
> Companion files: `infrastructure-integration.md` (module, IAM, compose, Caddy, runbooks),
> `ci-cd-pipeline.md`, `observability-design.md`, `monitoring-alerting.md`,
> `kpi-instrumentation.md`, `branching-strategy.md`, `wave-decisions.md`.

## 1. Platform requirements (quantified)

| Requirement | Value | Source |
|---|---|---|
| Users | < 50 | DESIGN driver |
| Availability | 99% monthly for sign-in and review (~7.3 h/month budget). Published claims do not depend on the app. | NFR-BRA-9 |
| Latency | Feedback < 100 ms; 10-repo scan <= 60 s p90; publish <= 3 s p90 | NFR-BRA-4 |
| Deploy frequency (target) | On demand, about weekly while in active delivery; laptop-initiated | ADR-068 / ADR-075 |
| Deployment strategy | Recreate (stop, then start). A few seconds of downtime per deploy. | User decision |
| Host | Shared t4g.micro (2 vCPU burstable, `cpu_credits = standard`, 1 GiB RAM + 1 GiB swap), arm64, AL2023 | `deploy/`, module v1.6.0 |
| Memory evidence | 71 MB peak RSS for a 10-repo scan (macOS proxy, SPIKE-4b) | design wave-decisions |
| Team | 1 operator plus agents; trunk-based; no PRs | User decision |
| Cost ceiling | Close to $0 for compute (co-located). Observability is trimmed to essentials at about $2-3/month (§7, user decision 2026-10-04). | ADR-075 |

## 2. Topology

```text
Internet ──HTTPS──▶ EIP ──▶ EC2 t4g.micro (openlore-pds-prod), AL2023 arm64
                              │
   compose project "pds" (/pds/compose.yaml, module-owned)    compose project "review-app"
   ┌─────────────────────────────────────────────┐            (/pds/app/compose.yaml, OpenLore-owned)
   │ caddy:2.8-alpine  :80/:443                  │            ┌──────────────────────────────────┐
   │   /etc/caddy/Caddyfile (module-rendered)    │  network   │ review-app (distroless, uid 65532)│
   │   import /etc/caddy/sites/*.caddy  (v1.7.0) ├─pds_default┤  :8080, read-only rootfs           │
   │   /pds/caddy/sites ─▶ /etc/caddy/sites:ro   │ (external) │  mounts: /pds/app/data  -> /data   │
   │ pds (pinned digest) :3000, mounts /pds      │            │          /pds/app/secrets -> /run/secrets:ro
   └─────────────────────────────────────────────┘            │  mem 256m, no swap, oom_score_adj 800
                                                              └──────────────────────────────────┘
   host: dockerd (awslogs driver ─▶ CloudWatch Logs), systemd timer review-app-health (1/min ─▶ host.health log lines)
   host role: + read /openlore/prod/review-app/*, + logs to one group (no PutMetricData)
   IMDS: hop limit 1 (v1.7.0), so no container can obtain the host role
```

DNS: the Cloudflare wildcard `*.openlore.jeffbailey.us` (DNS only, not proxied) already resolves
`app.openlore.jeffbailey.us` to the EIP. Nothing changes in DNS. The Route 53 records stay inert.

## 3. Existing infrastructure: reuse decisions

| Need | Existing asset | Decision |
|---|---|---|
| Host, EIP, data volume, TLS | `tofu-aws-pds` v1.6.0 (`deploy/tofu/environments/prod`) | **Reuse.** Bump to v1.7.0 for two generic changes (§4). |
| Reverse proxy and certificates | Caddy 2.8 in the module compose | **Reuse** through a generic site-import hook |
| Remote shell and command channel | SSM Session Manager and Run Command (`AmazonSSMManagedInstanceCore` on the host role) | **Reuse** for deploys, secret rendering and KPI reads |
| Secrets | SSM SecureString pattern (`/openlore/prod/*`, default `aws/ssm` key) | **Reuse** under `/openlore/prod/review-app/` |
| Alerting channel | SNS topic `openlore-pds-backup-alarm`, already subscribed out of band | **Reuse** the topic for every app alarm. There is no new subscription and no address in state. |
| Alarm pattern | `PDS/Backup` host metric, `treat_missing_data = breaching` | **Reuse** the same pattern for the host-side app health metric |
| Plan gate | `deploy/check-plan.sh` | **Reuse** unchanged. The new resources are creates only. |
| CI | `ci.yml` (fmt, clippy, deny, check-arch, check-probes, nextest), `nightly.yml`, `release.yml` (arm64 runner precedent, cosign keyless, SBOM, SLSA) | **Extend.** Add image jobs to `ci.yml` and a live smoke to `nightly.yml`. `release.yml` is untouched. |
| Static deploy checks | `deploy-pds-check.yml` | **Extend** with checks for `deploy/review-app/**` |
| Local gate | `.githooks/pre-commit` (`check-probes`) | **Reuse.** An optional pre-push is described in `branching-strategy.md`. |

New components, each with the reason no existing one would do:

1. **The `review-app` container and its compose file.** This is the new deployable. ADR-075
   keeps it out of the module's compose file.
2. **The GHCR image `ghcr.io/jeffabailey/openlore-review-app`.** No container image exists
   today. `release.yml` ships CLI tarballs only.
3. **A host health timer (`review-app-health`).** It is the cheapest source of
   process-up, memory and OOM signals. It writes log lines, not custom metrics. The CloudWatch
   agent was rejected (§8).
4. **CloudWatch resources:** a log group, metric filters, a Route 53 health check and alarms.
   None exist for an HTTP service yet.

## 4. Required change to the shared module: `tofu-aws-pds` v1.7.0

Both changes are project-agnostic. The-reality-base (TRB) adopts them at its own pace, because it
pins its own tag.

| # | Change | Why | Effect on a running host |
|---|---|---|---|
| M-1 | Create `/pds/caddy/sites`. Mount it into Caddy as `/pds/caddy/sites:/etc/caddy/sites:ro`. Append `import /etc/caddy/sites/*.caddy` to the rendered Caddyfile. | **Correction to ADR-075 §4.** The Caddy container mounts only the Caddyfile (`/pds/caddy/Caddyfile:/etc/caddy/Caddyfile:ro`), so `import /pds/caddy/sites/*.caddy` would name a path that does not exist inside the container. SPIKE-4a validated `import /etc/caddy/sites/*.caddy`, the in-container path, and found an empty glob is fine. | The user-data changes, so the **instance is replaced** (option A, decided). |
| M-2 | Set `metadata_options.http_put_response_hop_limit = 1` (was 2, commented "the container reads the instance role"). | With hop limit 2, **any container on the bridge network can get the host role's credentials**. That role can read `/openlore/prod/account-password` and `cli-app-password` (Jeff's PDS account) and the new app secrets, and can write identity archives. A compromised public web app must not hold these. **Approved by the user (2026-10-04)**, on this evidence: every AWS call in the module runs on the **host**, not in a container. That covers the SSM get/put in user-data, the `aws s3 cp` identity backup and the `aws cloudwatch put-metric-data` `ExecStartPost`. The PDS blobstore is disk (`PDS_BLOBSTORE_DISK_LOCATION`). The-reality-base, the other module consumer, has no container-side AWS use either: its AWS use is GitHub Actions plus IAM, and `deploy/pds-upstream/installer.sh` only reads the public IP from the host. dockerd's awslogs driver also runs on the host. The module comment at `modules/pds/main.tf:312` ("the container reads the instance role") is **stale** and is corrected in v1.7.0. | An in-place update (`ModifyInstanceMetadataOptions`), with no replacement. It ships in the same apply. **Verify first** (runbook R-2, step 2). |

Rejected for v1.7.0: a "boot-time extra compose files" hook. Instance replacement is always
operator-driven in this design, and the replacement runbook ends with an app redeploy
(`infrastructure-integration.md` §7.1). The app's compose file, site file, data and secrets sit
on `/pds` and survive replacement. Only the running container and the host systemd units do not.
Adding the hook would widen the module for a case that never happens unattended.

## 5. Container contract

| Aspect | Specification |
|---|---|
| Image | `ghcr.io/jeffabailey/openlore-review-app@sha256:<digest>`. Deploys always pin by **digest**, never by tag. CI publishes the tags `sha-<40-hex>` and `main`. |
| Base | `gcr.io/distroless/cc-debian12:nonroot` (arm64), pinned by digest in the Dockerfile. The binary is built in `rust:<stable>-bookworm` so its glibc (2.36) matches the base (`ci-cd-pipeline.md` §4). |
| User | 65532:65532 (distroless `nonroot`) |
| Filesystem | `read_only: true`. Writable mounts are `/data` (bind mount of `/pds/app/data`) and a `tmpfs: /tmp` of 16 MB. DuckDB's `temp_directory` points to `/data/tmp`. |
| Mounts | Exactly two: `/pds/app/data:/data:rw` and `/pds/app/secrets:/run/secrets:ro`. **Never `/pds`.** CI enforces this (`ci-cd-pipeline.md` §5). |
| Privileges | `cap_drop: [ALL]`, `security_opt: [no-new-privileges:true]`, `pids_limit: 128` |
| Memory | `mem_limit: 256m`, `memswap_limit: 256m` (no swap for the app), `mem_reservation: 96m`, `oom_score_adj: 800`, DuckDB `memory_limit='64MB'`, `threads=1`. See §6. |
| CPU | `cpus: 1.0` (at most one of the two vCPUs, which protects the PDS and the credit balance) |
| Network | Only the external network `pds_default`. Service name `review-app`. No published ports. |
| Restart | `restart: unless-stopped`; `stop_grace_period: 20s`. On SIGTERM the app stops accepting, marks running scans `interrupted` and exits. |
| Health | `/healthz` (liveness: the process serves) and `/readyz` (every hard probe passed and the self-probe of the public client metadata succeeded). The bodies are minimal (`{"ok":true}` / `{"ready":false}`); reasons go to the logs only. There is no Docker `HEALTHCHECK`, because distroless has no curl; the host timer checks instead. |
| Logs | stdout JSON. Docker `awslogs` driver to `/openlore/prod/review-app`, `mode: non-blocking`, `max-buffer-size: 4m`. Docker's local dual-logging cache keeps `docker logs` working. |
| Config (non-secret env) | `APP_ORIGIN=https://app.openlore.jeffbailey.us`, `OAUTH_SCOPES=atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create`, `REVIEW_DB=/data/review-app.duckdb`, `SECRETS_DIR=/run/secrets`, `LOG_FORMAT=json`, plus the ADR-076 limit values |
| Secrets (files) | `/run/secrets/{client-jwk,data-key,github-token,log-salt}` and, during rotation only, `client-jwk-previous` and `data-key-previous`. Files, not environment variables, so they are absent from `docker inspect` and `/proc/*/environ`. |

**Deviation from ADR-075 §2:** ADR-075 renders a `secrets.env`. This design renders
**per-secret files** into a 0700 directory and mounts it read-only. The intent is the same
(SSM-sourced, mode-restricted, no `/pds` mount) and the exposure is smaller.

## 6. Memory budget and the re-measure gate

| Consumer | Estimate on t4g.micro | Notes |
|---|---|---|
| Kernel and AL2023 base, SSM agent | ~180 MB | |
| dockerd + containerd | ~80 MB | |
| Caddy | ~30-50 MB | |
| PDS (node) | ~150-250 MB | **To be measured** (R-5) |
| review-app | **limit 256 MB** (expected 70-120 MB) | SPIKE-4b proxy: 71 MB peak |
| Page cache headroom | the rest of 1 GiB, plus 1 GiB swap for host processes | |

Protection, in order:

1. The cgroup limit of 256 MB caps the app. With no swap allowance, an app leak ends in an
   app-only OOM kill, not host swap thrash.
2. `oom_score_adj: 800` makes the kernel kill the app before the PDS under host-wide pressure.
3. The DuckDB caps are 64 MB and one thread.
4. ADR-076 allows at most 2 concurrent scans globally.

**Re-measure gate (rollout step R-5, operator).** Run a 10-repo scan as the operator with two
scans in flight if possible. Read `docker stats` and the `host.health` log lines (`app_mem_pct`, `host_mem_available_mb`,
`swap_in_kb`). This gate stays **hard**, because the memory alarms are deferred
(monitoring-alerting §1.1).

| Signal | Pass | Fail action |
|---|---|---|
| App peak RSS | < 150 MB | Investigate, and set `mem_limit` to the peak plus 50% |
| Host `MemAvailable` minimum during the scan | > 128 MB | Move to t4g.small |
| Swap-in during the scan (`vmstat si`) | ~0 sustained | Move to t4g.small |
| PDS `/xrpc/_health` during the scan | 200 throughout | Move to t4g.small |

**The t4g.small fallback** (+$6.13/month): change `instance_type` in
`deploy/environments/prod.json`. The plan is an in-place stop/start, not a replacement. Run
`check-plan.sh`, apply, then raise `mem_limit` to 512m in a follow-up deploy.

## 7. Cost

| Item | Monthly |
|---|---|
| Compute, storage, EIP | $0 extra (co-located) |
| Route 53 HTTPS health check (AWS endpoint) | ~$1.50 (basic $0.50 plus the HTTPS option $1.00; verify current pricing at apply time) |
| Log-filter metrics (`AppDown`, `GuardrailBreaches`, `GithubTokenExpiring`) | ~$0-0.90. They are prorated and bill only for hours with data, which in normal operation is close to $0. |
| `PutMetricData` | $0 (none; the host timer writes log lines) |
| Alarms (4 standard) | $0.40 |
| CloudWatch Logs ingest and storage (app events plus ~10 MB/month of `host.health`, 30-day retention) | < $0.05 |
| SSM standard parameters, GHCR (public) | $0 |
| **Total** | **~$2/month typical, at most ~$2.85** on top of today's ~$10.84 (was ~$4-5 before the trim) |

## 8. Simpler alternatives considered and rejected

1. **Run the app container with no CloudWatch integration and check it by hand.** This meets the
   functional needs but gives no detection for a 99% target (7 h/month budget) on a solo-operated
   host. Rejected.
2. **Use only the on-host health timer, with no external check.** It covers the process and memory
   cases through `host.health` log lines, but a dead host simply goes silent. It cannot see DNS, Cloudflare or
   routing failures, which are exactly the failures ATProto OAuth is sensitive to (the PDS
   fetches the client metadata). The user asked for an external check. Kept as the complement.
   Route 53 HTTPS checks do **not** validate the certificate, so an expired certificate is
   caught by the nightly `live-contract-smoke` (curl validates) and by the startup self-probe.
3. **CloudWatch agent for memory and logs.** It is another daemon on a 1 GiB host (~40-60 MB),
   while dockerd's awslogs driver and a 30-line shell timer cover the need. Rejected.
4. **CloudWatch Synthetics canary.** About $10/month at 5-minute intervals, as much as the whole
   host. Rejected for the Route 53 health check.
5. **CI-driven deploys (OIDC role).** Rejected by ADR-068/075 (laptop-initiated, no CI roles).
