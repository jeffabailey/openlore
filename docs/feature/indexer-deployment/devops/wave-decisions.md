# Wave Decisions: indexer-deployment (DEVOPS)

- **Wave**: DEVOPS (platform readiness and infrastructure design). Design documents only: nothing
  applied, built, deployed or committed.
- **Date**: 2026-10-06. **Architect**: Apex (nw-platform-architect), autonomous.
- **Inputs**: wizard decisions; DISCUSS (outcome KPIs, ACs, user decisions after DISCUSS);
  DESIGN (architecture-design §9 contracts, data-models, ADR-080..083); the implemented
  review-app deploy (`deploy/review-app/*`, `review-app.tf`, `review-app-iam.tf`, `ci.yml` jobs).
- **Decisions given by the caller (applied, not re-asked)**: co-located on the PDS host, riding the
  single v1.7.0 replacement; Docker Compose; one signed arm64 image deployed by digest from the
  laptop; one `serve` container (`init`, 5 s grace, 128m, DuckDB 48 MB / 1 thread); a 15-minute
  host timer running `docker exec … trigger`; SSM Standard String DID list rendered to a file by
  rename; public Caddy site with only search and `/healthz`; CloudWatch Logs 30 days; exactly 3
  alarms on the existing SNS topic; `indexer-iam.tf`; CI image job in `ci.yml`; trunk-based;
  per-feature mutation; no continuous learning.

## Decisions

| ID | Decision | Doc |
|---|---|---|
| DV-IXD-1 | Image `ghcr.io/jeffabailey/openlore-indexer`, built in `rust:1-bookworm` on `ubuntu-24.04-arm` (no `--locked`), COPY'd to distroless `cc-debian12:nonroot` at `/usr/local/bin/openlore-indexer`; trivy gate, cosign keyless, cargo + image SBOM attestations, SLSA provenance. Jobs `indexer-build` (needs `test`) → `indexer-image`. A production-posture **serve smoke** (healthz, 404 surface, 413, `trigger` exit 3 with exactly one `pass_summary`, sub-3 s stop) gates deployability. `release-guard` unchanged. | ci-cd-pipeline §3 |
| DV-IXD-2 | Recreate deploy by digest via `deploy/indexer/deploy.sh` (mirrors the review-app script): CI-green + cosign gates, SSM Run Command, readiness on `/healthz` through Caddy on loopback, **automatic rollback** to the previous digest, releases file with `ready_s`. **No data copy** (no schema change; index rebuildable). Escape hatch `rollback --reset-index`. | infrastructure-integration §6 |
| DV-IXD-3 | Compose: `container_name: openlore-indexer`, `init: true`, `stop_grace_period: 5s`, `restart: unless-stopped`, 128m no swap, `oom_score_adj 900`, `cpus 0.5`, `pids 64`, read-only rootfs, tmpfs `/tmp` for the socket, **exactly two mounts** (`data` rw, `config` **directory** ro). CI posture guard. | infrastructure-integration §2 |
| DV-IXD-4 | Timer `*:00/15` (aligned, `Persistent=true`), oneshot service `ExecStartPre=render-dids.sh`, `ExecStart=docker exec openlore-indexer openlore-indexer trigger`, `TimeoutStartSec=30min`; oneshot semantics prevent stacking; unit result reflects exit 0/2/3/4. | infrastructure-integration §3 |
| DV-IXD-5 | `render-dids.sh`: staging file in the same directory, `chmod 0444`, `rename(2)` over `repo-dids`, `touch .rendered-at`; on failure keep last good, log `render_failed`, exit 0. Never swap the directory (H1); bats test asserts directory inode unchanged. | infrastructure-integration §4 |
| DV-IXD-6 | SSM parameter is **tofu-created** (creates only, visible to `check-plan.sh`) with seed = the operator's own DID and `ignore_changes = [value]`; the operator edits the value with the AWS CLI. | infrastructure-integration §7.2, §9.4 |
| DV-IXD-7 | Caddy `index.caddy`: POST search + GET `/healthz` only, everything else 404, `request_body max_size 8KB`, `header -Server`, 503 page **only** for upstream 502/503/504 (so 413 stays 413). Validate before reload; remove the file if invalid. | infrastructure-integration §5 |
| DV-IXD-8 | Exactly **3 alarms**: A1 total outage (Minimum of a 1/0 metric over aligned 900 s periods, 2 of 2), A2 any exit 2 / startup refused / store unusable (5 min), A3 not-live (Maximum of the host line's `not_live`, 2 × 5 min, missing = breaching). **Liveness folded into one alarm** by computing `not_live` on the host from: no `pass_summary` in 45 min (read from CloudWatch), `/healthz` failing, DID list > 2 h stale, container down. | monitoring-alerting §1 |
| DV-IXD-9 | Indexer health timer is a **twin** of the review-app timer (every 2 min, own log group), not an extension: the two apps deploy in either order. It also records memory and a canned-search latency sample. | observability-design §4 |
| DV-IXD-10 | Host IAM `openlore-indexer-host` (bootstrap root): `ssm:GetParameter` on `/openlore/prod/indexer/*`; logs create-stream/put/describe on the indexer group; **plus `logs:FilterLogEvents` on the same group** for the end-to-end heartbeat (one addition beyond the brief; see Open items U-1). No SSM write, no KMS, no `PutMetricData`. | infrastructure-integration §7.1 |
| DV-IXD-11 | Added cost about **$1.00/month** at list price (alarms $0.30, two always-on filter metrics $0.60, logs about $0.10), likely $0 inside the CloudWatch free tier; ≤ $2 ceiling met; alarm resources exactly $0.30 (AC-004.6). DESIGN's ≤ $0.40 estimate omitted filter metrics. | platform-architecture §7 |
| DV-IXD-12 | Sequencing: indexer AWS resources (I-0) apply on their own as creates only, any time; first container start **requires** R-REPLACE done (IMDS hop limit 1 + sites hook), checked by `deploy.sh install`. No module change, no second replacement. | infrastructure-integration §8 |
| DV-IXD-13 | Memory gate measured on the host with cgroup v2 `memory.peak` (`deploy.sh measure`); fail path tightens concurrency first; t4g.small only by operator decision (in-place stop/start = an unplanned PDS outage). | infrastructure-integration §9.6 |
| DV-IXD-14 | Review app in the same release: B11 DuckDB caps, `init: true`, `mem_limit` 192m (confirmed at the gate). Recommendation (out of scope): move `render-secrets.sh` to per-file rename. | platform-architecture §4.1, infrastructure-integration §9.8 |
| DV-IXD-15 | Nightly advisory `index-smoke` (health, freshness, ≥ 2 PDS hosts, surface, TLS) opens an issue on failure. | ci-cd-pipeline §6 |
| DV-IXD-16 | No new ADR. Everything implements ADR-075 and ADR-080..083. | — |

## Rollout sequence (summary; details in infrastructure-integration §8)

I-0 apply indexer IAM + prod resources (alarms off) → I-1 DELIVER merged, image public →
I-2 R-REPLACE (shared, once) → I-3 review-app B11 deploy (if live) → I-4 indexer first deploy →
I-5 memory gate + rollback drill (hard) → I-6 enable and test-fire A1/A2/A3 → I-7 announce, baselines.

## Handoffs

- **DISTILL**: ATs for the serve smoke contract (ci-cd-pipeline §3.2) can reuse the B-change ATs;
  bats tests for `render-dids.sh` and `indexer-health.sh` are platform tests (infrastructure-integration §4, observability-design §4).
- **DELIVER**: `deploy/indexer/**`, `crates/openlore-indexer/Dockerfile`, `indexer-build`/`indexer-image`
  jobs, the `deploy-pds-check` `indexer-host` job, `indexer.tf`, `indexer-iam.tf`, the
  `index-smoke` nightly job, review-app compose changes (B11, `init`, 192m), `deploy/README.md`
  Indexer section and the R-REPLACE step-7 line.
- **Operator**: I-0 apply, GHCR visibility, DID list, I-2..I-7.

## Open items (need the user)

| # | Item | Default if no answer |
|---|---|---|
| U-1 | Approve `logs:FilterLogEvents` on `/openlore/prod/indexer` for the host role (heartbeat reads the shipped `pass_summary`). Alternative: `docker logs --since 45m` locally, with no extra IAM but blind to a broken awslogs pipeline and reset by each deploy. | Use `FilterLogEvents` (read-only, own log group) |
| U-2 | Seed DID for the tofu-created parameter: `did:plc:pnyxfnpkcldxtitsw64ycahw` (from `deploy/README.md`). Confirm it is the operator's OpenLore DID, and supply the production list (≥ 2 PDS hosts) before I-4. | Seed as stated; production list required at I-4 |
| U-3 | A1 test-fire uses synthetic `pass_summary` lines in a `test-fire` stream (a real all-skip run would need a production config change). Acceptable? | Synthetic for A1; A2 and A3 fired end to end |

## AC implementation map (DEVOPS mechanism → verification)

| AC | Mechanism | Verified by |
|---|---|---|
| 001.1 | Public route, production DID list ≥ 2 hosts | I-4 step 5; nightly `index-smoke` |
| 001.2 | Caddy automatic HTTPS + redirect | post-checks; `index-smoke` TLS |
| 001.3 | Caddy allowlist + binary allowlist | serve smoke; post-checks (404); `index-smoke` |
| 001.4 | Empty store answers 200 | serve smoke; I-4 step 4 |
| 001.5, 006.3 | Recreate ≤ 15 s; laptop PDS poller | deploy summary (`ready_s`, PDS n/n) |
| 001.6, 006.1 | Digest-only deploy, cosign verify, `.env` pin | `deploy.sh` digest guard test; posture guard |
| 002.1 | 15-min aligned timer | KPI-IXD-2 samples |
| 002.2, 002.3 | One process (ADR-080) | DISTILL ATs; gate burst (§9.6) |
| 002.4 | oneshot merge + binary single-flight | serve smoke (concurrent trigger); ATs |
| 002.5 | `Persistent=true`, units reinstalled by `redeploy` | reboot check at I-5; R-REPLACE step 7 |
| 003.1, 003.4 | `render-dids.sh` rename, last good | bats test; §9.4 edit at I-5 |
| 003.2, 003.3 | Binary purge rules; A2 test-fire uses a malformed list | ATs; A2 test-fire |
| 003.5 | No credentials, IMDS hop 1 | `install` IMDS check; posture guard |
| 004.1-004.6 | A1/A2/A3 definitions, `ok_actions`, $0.30 | test-fire (monitoring-alerting §3); cost table |
| 005.1-005.4 | `deploy.sh status` (Logs Insights) | readiness checklist |
| 006.2 | Auto-rollback, `rollback`, `--reset-index` | rollback drill (I-5) |
| 006.4, 006.5 | Caps, OOM order, `measure` | memory gate (§9.6) |

## Peer review (nw-platform-architect-reviewer, 2026-10-06)

**Verdict: conditionally approved.** External validity was PASS on all four checks (deployment
path, observability, rollback, security gates). All conditions are addressed in iteration 1:

| # | Sev | Finding | Resolution |
|---|---|---|---|
| 1 | critical | A1 empty-period semantics unclear; suggested `default_value: 0` | **Suggestion rejected:** a `default_value` publishes 0 for every non-matching batch, so the Minimum could never be 1 and A1 would never fire. The semantics are now stated: empty period → no data point → notBreaching; A1 needs data in both periods (monitoring-alerting §1). |
| 2 | critical | A3 not fail-closed on a `FilterLogEvents` error | It already was (`summary_check = error` sets `summary_45m = 0`); now stated explicitly in the `not_live` formula and the bats test |
| 4 | high | Posture guard must be blocking | Stated: blocking, so main goes red and `deploy.sh` refuses the sha |
| 5 | high | KPI-IXD-3 cannot use the `limit 8` query | New saved query `indexer/kpi-freshness` (30 days); tofu now 13 creates |
| 6 | high | I-0 vs the module bump sequencing | Decision rule: apply I-0 only when `check-plan.sh` passes without the override, else let it ride R-REPLACE. `check-plan.sh` is unchanged (it is already the guardrail). |
| 7 | medium | Guard 128m vs review app 192m | Each compose file checked on its own |
| 10, 13 | medium/low | `sites` pre-check; first-deploy failure | `install` refuses without `/pds/caddy/sites`; runbook §9.1 recovery |
| 11 | medium | 8 KB boundary | Smoke and post-checks now assert 8 KB is not 413 |
| 3, 8, 12 | high/medium | render failure scope; FilterLogEvents rationale; deleted parameter | Documented (infrastructure-integration §4, §7.1, §9.5) |
| 17 | medium | AC traceability | AC implementation map above |
| 9, 14, 15, 16 | low/medium | Comment wording, restart field, releases rotation, DISTILL gate list | Accepted as-is; noted for DELIVER |

No second iteration was run: the critical items were a rejected suggestion and a misreading, both
now stated in the docs. The remaining items are documentation fixes or are deferred to DELIVER.

## User decisions (2026-10-06)
- **U-1:** APPROVED. `logs:FilterLogEvents` on the indexer log group only, so a broken log pipeline pages through A3.
- **U-2:** seed the DID list with `did:plc:pnyxfnpkcldxtitsw64ycahw` only. The operator adds more later with the AWS CLI, and they are picked up on the next pass.
- **U-3:** APPROVED. Test-fire A1 with synthetic `pass_summary` lines in a `test-fire` stream while the timer is paused (about 75 min), before go-live.
- The orchestrator verified the rejected A1 `default_value` suggestion: a default would publish 0 on every non-matching batch, so Minimum could never reach 1. With no default, an empty period is missing and treated as notBreaching. Rejection upheld.

## User decisions after the DELIVER review (2026-10-07)
- The full-model review found 3 high and 3 medium findings; ALL are fixed in this delivery, and the low findings beyond L1/L2 go to the backlog.
- **H3:** fix the CLI near-match sweep (skip it on an empty index; cap the probes) AND add a per-IP rate limit. This reverses the earlier "no per-IP rate limit" decision (ADR-083). Stock Caddy has no rate_limit module (it would need a custom build, i.e. a shared-module change), so the limit is enforced in the indexer binary, keyed on X-Forwarded-For from the trusted proxy only: 10 req/s, burst 50, 429 + Retry-After.
