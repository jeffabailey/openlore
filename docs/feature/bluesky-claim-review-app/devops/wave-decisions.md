# Wave Decisions: bluesky-claim-review-app (DEVOPS)

- **Wave**: DEVOPS (platform readiness and infrastructure design). These are design documents
  only: nothing is applied, built or committed.
- **Date**: 2026-10-04
- **Architect**: Apex (nw-platform-architect), autonomous
- **Inputs**: DESIGN (ADR-071..076, the SPIKE RESULTS), DISCUSS (outcome KPIs, I-BRA-1..8),
  `deploy/**`, `.github/workflows/*`, and the `tofu-aws-pds` v1.6.0 source
- **User decisions applied (2026-10-04)**: AWS, co-located on the PDS host; Docker Compose behind
  Caddy; ADR-075 option A (one planned replacement); GitHub Actions extended; CloudWatch plus
  JSON logs; Recreate; no continuous learning; trunk-based; per-feature mutation testing; no
  backup of private state.

## Decisions

| ID | Decision | Doc |
|---|---|---|
| DV-BRA-1 | The module **v1.7.0** carries two generic changes: (M-1) mount `/pds/caddy/sites` into Caddy at `/etc/caddy/sites:ro` plus `import /etc/caddy/sites/*.caddy`; (M-2) IMDS hop limit 1. Both roots bump in lockstep. One planned replacement (R-REPLACE). | platform-architecture §4 |
| DV-BRA-2 | The image is built natively on `ubuntu-24.04-arm` **inside a `rust:1-bookworm` job container** (glibc 2.36 matches distroless/cc-debian12), then COPY'd into distroless `nonroot`. It is pushed to GHCR as `sha-<40>` and `main`, cosign keyless signed, with CycloneDX and syft SBOM attestations and SLSA provenance. It is built on every green main push. CI never deploys. | ci-cd-pipeline §3 |
| DV-BRA-3 | The host pulls a **public** GHCR package **by digest**. `deploy.sh` (laptop) verifies CI is green and the cosign identity, then runs SSM Run Command: install host files, render secrets, pull, stop, copy DuckDB, up, `/readyz`, and **auto-rollback** on failure. | infrastructure-integration §7.2 |
| DV-BRA-4 | Secrets are SSM SecureStrings under `/openlore/prod/review-app/` (`client-jwk`, `data-key`, `github-token`, `log-salt`, plus `*-previous` during rotation). They are rendered by the host as root into per-secret 0400 files owned by uid 65532 and mounted read-only at `/run/secrets`. Files replace ADR-075's `secrets.env`. The container has no AWS credentials and no `/pds` mount. | infrastructure-integration §5 |
| DV-BRA-5 | Host IAM is an inline policy `openlore-review-app-host` in the **bootstrap** root: read the app's SSM path and write one log group. There is no `PutMetricData`. Observability resources live in the **prod** root `review-app.tf`. | infrastructure-integration §3 |
| DV-BRA-6 | Container limits: `mem_limit = memswap_limit = 256m`, `oom_score_adj 800`, `cpus 1.0`, read-only rootfs, `cap_drop ALL`, `no-new-privileges`, `pids 128`. A re-measure gate on the host decides on t4g.small. | platform-architecture §5-6 |
| DV-BRA-7 | Recreate deploys. Caddy's `handle_errors` serves a "restarting, nothing changed" 503. Migrations are expand-only, and a pre-deploy DuckDB copy on the same volume makes binary rollback safe. No backup (user decision). | infrastructure-integration §4, §6 |
| DV-BRA-8 | Observability, **trimmed to essentials (user decision 2026-10-04)**:<ul><li>a Route 53 HTTPS health check (external);</li><li>a 1-minute host timer writing `host.health` **log lines** (ready, memory, restarts/OOM, host memory);</li><li>dockerd `awslogs` with **30-day** retention;</li><li>**4 alarms** on the **existing** SNS topic: A-1 unreachable, A-3 down/restarted, A-7 guardrail breach, A-8 token expiry;</li><li>`review_app_alarms_enabled` gating `actions_enabled`.</li></ul>Readiness, memory, CPU-credit and startup-refused alarms are deferred, with their signals kept as log lines (monitoring-alerting §1.1). The R-5 memory re-measure stays a hard gate. About $2-3/month. | observability-design, monitoring-alerting |
| DV-BRA-9 | The log contract is a closed event set, flattened JSON and pseudonymous `owner` = HMAC(log-salt, DID)[:12]. A new check-arch rule, `review_app_log_field_allowlist`, plus a log-capture acceptance test enforce it. | observability-design §3, ci-cd-pipeline §2 |
| DV-BRA-10 | KPIs: once-per-user cohort counters with owner-scoped flags [DM], a daily `kpi.rollup` log line, and a **loopback admin listener** (`kpi`, `purge`, `test-alarm`) reached via `docker exec`. A pure guardrail audit emits `guardrail.breach`, which pages. | kpi-instrumentation |
| DV-BRA-11 | CI gates for the new crates are the 6 DESIGN check-arch deltas, the log allowlist rule, check-probes, `cargo deny` with an explicit `hickory-* < 0.26` ban, an exact `atrium-oauth` pin, image smoke (`--version`, `probe --self-test`), a trivy CRITICAL/HIGH gate, and a compose mount guard in `deploy-pds-check.yml`. | ci-cd-pipeline §2, §5 |
| DV-BRA-12 | Nightly `live-contract-smoke` (advisory, no secrets): the production `/oauth/*` endpoints, the self-attested reader path against the SPIKE-1 test account (KPI-BRA-6), authorization-server metadata, and PLC. It opens an issue on failure. | ci-cd-pipeline §6 |
| DV-BRA-13 | No new ADR. Everything here implements ADR-074/075/076. The two corrections below are recorded as amendment notes for ADR-075 at DELIVER. | — |

## Corrections and findings handed upstream or to DELIVER

1. **ADR-075 import path is wrong inside the container.** Caddy mounts only the Caddyfile, so
   the hook must be a sites mount plus an in-container import path (DV-BRA-1, M-1).
2. **The IMDS hop limit of 2 exposes the host role to containers.** That role reads Jeff's
   PDS account and app passwords from SSM. M-2 sets it to 1, an in-place change, verified
   before and after (R-REPLACE steps 2 and 6).
3. **DuckDB single-process lock.** `openlore-review-app kpi`, as a separate process, cannot
   open the file while `serve` runs. Replaced by the loopback admin listener plus the rollup
   (DV-BRA-10).
4. **glibc mismatch.** An ubuntu-24.04 build will not run on distroless debian12 (DV-BRA-2).
5. **An expired GitHub PAT stops the app** (the probe refuses start). Alarm A-8 warns 14 days
   ahead (header `github-authentication-token-expiration`).
6. Absorbed SPIKE findings:
   - revoke 200 is treated as success and local tokens are always deleted (runbook §7.5);
   - the residual access-token window is measured and documented at R-5;
   - the callback `todo!()` panic is isolated (`signin.callback_panic_isolated` event);
   - `hickory >= 0.26` is enforced.
7. **[DM] data-model additions** for the KPIs: `accounts.first_session_id_hash`,
   `first_published_at`, `kpi_flags`; `web_sessions.queue_max`, `declines`; finer
   `time_to_publish` buckets.
8. **DELIVER deliverables** implied by this design:
   - `probe --self-test`;
   - `gen-client-jwk`;
   - the admin listener (`kpi`, `purge`, `test-alarm`);
   - the `kpi.rollup` event and the guardrail audit;
   - startup re-encryption for data-key rotation and invalidation of sessions it cannot decrypt;
   - the `schema_version` refusal;
   - SIGTERM handling;
   - `deploy/review-app/**`, `review-app.tf`, `review-app-iam.tf`;
   - the workflow jobs;
   - the module v1.7.0 change in `tofu-aws-pds`.

## Rollout sequence

> *2026-10-09 (fix-go-live-runbook-gaps):* this table is the design-time history. The
> operator's one ordered sequence for both apps is the [Go-live checklist](../../../../deploy/README.md#go-live-checklist).

| Step | What | Who | Downtime | Gate |
|---|---|---|---|---|
| R-0 | Before US-BRA-004 ships, run on the **OpenLore PDS** from the laptop: SPIKE-1 (a self-attested `createRecord` with the `jeff` app password `/openlore/prod/cli-app-password`, then read back with CID == rkey) and SPIKE-3 (granular scopes, using a loopback OAuth client signed in as `jeff`, a create outside the scope refused). If SPIKE-3 fails on this PDS, use the `transition:generic` fallback plus disclosure (user decision 2). | Operator | none | Pass recorded in design wave-decisions |
| R-1 | Put the SSM parameters (`client-jwk`, `data-key`, `github-token`, `log-salt`). Create the PAT (public read, no permissions, expiry noted). Release `tofu-aws-pds` v1.7.0. | Operator + module repo | none | Parameters exist; the v1.7.0 tag exists on GitHub (deploy-pds-check validates it) |
| R-2 | **R-REPLACE**: verified identity backup → apply bootstrap (IAM) → prod plan with `OPENLORE_ALLOW_DELETE=1` gate → apply (replace + alarms created disabled) → verify PDS, sites hook and IMDS closed | Operator | **a few minutes of PDS downtime** | infrastructure-integration §7.1 all green |
| R-3 | Make the GHCR package public (once, after the first image push): repo → Packages → openlore-review-app → Package settings → Change visibility. **Failure mode:** if this is skipped, `deploy.sh` fails at pull with an auth error. *Corrected 2026-10-09:* `deploy.sh` does **not** check anonymous pull; the go-live checklist (`deploy/README.md`, step 8) has the manual signed-out `docker pull <image>@<digest>` check. | Operator | none | Anonymous `docker pull` works |
| R-4 | First app deploy: `deploy.sh install` then `deploy <sha>` | Operator | none for the PDS | `/readyz` 200; `/oauth/*` byte-equal at the public origin; certificate issued for `app.` |
| R-5 | **Soft launch (not announced):** confidential `private_key_jwt` sign-in on bsky.social and the OpenLore PDS; publish, share and retract with the test accounts; **RSS re-measure gate** (10-repo scan, platform-architecture §6) → t4g.small if it fails; disconnect and revoke check, recording the access-token lifetime; **rollback drill** N → N-1; client-JWK rotation dry run (optional) | Operator | none | Production readiness checklist (monitoring-alerting §3) |
| R-6 | **Precondition: R-5 is complete, including the rollback drill, and O-4 is done** (`aws sns list-subscriptions-by-topic` shows a confirmed subscription, not `PendingConfirmation`). Then set `review_app_alarms_enabled = true` (plan, gate, apply). Fire A-1, A-3, A-7 and A-8 once as a test (monitoring-alerting §3). | Operator | none | Notifications received, then OK |
| R-7 | Announce, but **only after R-5 and R-6 pass**. Collect a **4-week** KPI baseline, reporting numerators and denominators (for example "6 of 10") rather than percentages until then. Then close DEVOPS with the evolution doc and baseline. | Jeff | — | Baseline recorded |

**Sequencing and ownership (review issues 3 and 5):**

- R-0 and R-1's parameters and module release can run in parallel with DELIVER.
- R-2 (replacement) needs only the module v1.7.0 and the IAM file, not the app code.
- R-4 onward needs DELIVER to have merged the app, its probes and `probe --self-test`, the
  instrumentation, `deploy/review-app/**` and the workflow jobs.
- **The new CI gates are DELIVER-owned.** They land with the code they check: the check-arch
  rules, the log allowlist, check-probes for the new adapters, the image jobs, and the compose
  mount guard. DELIVER's first slice that adds `deploy/review-app/host/compose.yaml` must add
  the mount guard **in the same commit**. No compose file may reach main without it.
- **R-5 is a hard gate.** The soft launch is not complete, and R-6 and R-7 may not start, until
  the N → N-1 rollback drill has passed on the host. If the drill fails:
  - fix `deploy.sh`;
  - redeploy N-1 by hand (`docker compose` with the digest from `releases`);
  - repeat the drill.

  An app that cannot be rolled back is not announced.
- **Module-ref lockstep (standing rule, issue 6):**
  - `deploy/tofu/bootstrap/main.tf` and `deploy/tofu/environments/prod/main.tf` always pin the
    same `tofu-aws-pds` ref;
  - a bump applies bootstrap first, then prod;
  - DELIVER adds this rule to `deploy/README.md` and a `deploy-pds-check.yml` step that fails
    if the two `?ref=` values differ.

## User decisions (2026-10-04, DEVOPS)

| # | Decision | Evidence and consequence |
|---|---|---|
| U-1 | **IMDS hop limit 1 in `tofu-aws-pds` v1.7.0: approved.** | Every AWS call in the module runs on the host: the user-data SSM get/put, the `aws s3 cp` backup, and the `aws cloudwatch put-metric-data` `ExecStartPost`. The PDS blobstore is disk. The-reality-base has no container-side AWS use (GitHub Actions plus IAM; `deploy/pds-upstream/installer.sh` only reads the public IP on the host). The stale comment at `modules/pds/main.tf:312` is corrected in v1.7.0 (platform-architecture §4, infrastructure-integration §2). |
| U-2 | **Monitoring trimmed to essentials.** Keep A-1 (external `/healthz`), A-3 (restart/OOM, process down), A-7 (guardrail breach, pages on the first one) and A-8 (GitHub token, 14 days ahead). | Readiness, app memory, host memory, CPU-credit and startup-refused alarms are dropped. Their signals stay as log lines (monitoring-alerting §1.1). The host timer writes logs instead of `PutMetricData`. The R-5 memory re-measure stays a hard gate. **Cost ~$2/month typical, at most ~$2.85** (was ~$4-5; platform-architecture §7). |
| U-3 | **Log retention: 30 days.** | KPI windows longer than 30 days (the 4-week baseline, the 60-day objective) read `kpi_counters` through the admin listener (`GET /admin/kpi`) (observability-design §6, kpi-instrumentation §4). |

## Open items: operator action

| # | Item | Status |
|---|---|---|
| O-1 | Make the GHCR package public (R-3) | Required |
| O-2 | Run SPIKE-1 and SPIKE-3 on the OpenLore PDS (R-0). These need `jeff` credentials. | Required before US-BRA-004 |
| O-3 | Create the fine-grained PAT and the four SSM parameters (R-1) | Required |
| O-4 | Confirm the existing SNS email subscription is still confirmed | Required before R-6 |
| O-5 | Confirm the PDS is invite-only, so nobody can register the `app` handle (ADR-075 reserves the label) | Check `PDS_INVITE_REQUIRED` in `/pds/pds.env` at R-2 |

## Peer review (nw-platform-architect-reviewer, 2026-10-04)

**Verdict: conditionally approved.** External validity passed on all four checks: deployment
path, observability, rollback, and security gates.

| # | Severity | Issue | Resolution |
|---|---|---|---|
| 1 | blocker | IMDS hop limit 2 in module v1.6.0 exposes the host role to containers | This was already the design's finding 2 and module change M-2. It is now an explicit **precondition for R-4**: no app container starts on a host where the R-REPLACE step 6 IMDS test has not failed from a container. |
| 2 | critical | The compose mount guard is not yet in CI | The design specifies it (ci-cd-pipeline §5). It is now a DELIVER rule that the guard lands **in the same commit** as the first compose file (Sequencing). |
| 3 | critical | Rollback drill timing is ambiguous | R-5 is a hard gate before R-6 and R-7, with a failure path (Sequencing). |
| 4 | high | GHCR visibility and SNS confirmation are not gates | R-3 now has the steps and the failure mode. R-6 has the SNS-confirmed precondition. The reviewer's extra idea (a check-plan rule on `actions_enabled`) was **not adopted**: the `review_app_alarms_enabled` variable already decides it, and an alarm firing early only emails the operator. |
| 5 | high | Ownership of the CI gates between DEVOPS and DELIVER | They are DELIVER-owned and land with the code they check (Sequencing). |
| 6 | medium | Module-ref lockstep | A standing rule plus a CI step that fails if the two refs differ (Sequencing) |
| 7 | medium | Small-sample KPIs | The baseline is now 4 weeks, with numerators and denominators reported (R-7). kpi-instrumentation §5 is updated. |
| 8 | medium | Log-retention decision timing | Decided: 30 days (U-3) |

Iteration 1 addressed all of the blocker and critical findings in the documents. No second
review was needed: the remaining conditions are ones DELIVER and the operator carry out, and they
are encoded as rollout gates.
