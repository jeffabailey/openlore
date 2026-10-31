# DEVOPS Decisions: scale-to-zero-hosting

- **Date**: 2026-10-27, revised 2026-10-30 (cycle 1 after the final four-reviewer gate).
  **Architect**: Apex (nw-platform-architect), autonomous. Design only.
- **Inputs**: `../feature-delta.md` (DESIGN), `../design/wave-decisions.md`, `../spike/findings.md`,
  ADR-090 and the ADR-075/080/083 amendments, `docs/feature/indexer-deployment/discuss/outcome-kpis.md`,
  `docs/feature/indexer-deployment/devops/*`, `deploy/indexer/deploy.sh`, `deploy/ops.sh`,
  `deploy/indexer/host/*`, the prod and bootstrap tofu roots, `deploy/environments/prod.json`,
  `.github/workflows/ci.yml`, `crates/openlore-indexer/Dockerfile`, and read-only AWS checks.
- **Full design**: `../feature-delta.md` § Wave: DEVOPS. Machine inventory: `../environments.yaml`.
  Contract changes for the architect: `upstream-changes.md`.

## Key Decisions

- [DV-STZ-1] **Image and CI**: both binaries in one image (`WORKDIR /`, lambda-root label),
  built by two separate cargo invocations so the self-host binary's features cannot change; a
  blocking artifact check that the self-host binary has no AWS or Lambda strings; serve-smoke
  unchanged; one trivy, sign, three SBOM attestations and one SLSA provenance on the single digest
  (see: feature-delta.md § CI/CD pipeline outline).
- [DV-STZ-2] **Lambda image smoke = RIE + moto**, run as an arbitrary uid on a read-only root with a
  512 MB `/tmp`: health, surface, 413, init refusal, one exit-3 pass, probe line, loopback seam
  refused, then, from an explicit seed, no-publish, a publish with `If-Match`, a stale ETag giving exit
  2 `publish_conflict`, and a misspelled key refused. No `file://` test seam: a release image refuses it, so it cannot smoke the artifact
  (settles DESIGN open question 1).
- [DV-STZ-3] **New `msrv` job**: 1.91.0 checks the workspace without the two Lambda crates; 1.94.1
  checks the two Lambda crates; `fallback` resolution because `Cargo.lock` is gitignored; an xtask
  check that only those two crates set 1.94.1. No MSRV job existed before.
- [DV-STZ-4] **cargo-deny**: ban `aws-lc-rs`, `aws-lc-sys`, `aws-lc-fips-sys`; no license change
  expected.
- [DV-STZ-5] **Tofu root `prod-indexer`**: state `openlore/indexer/prod.tfstate` in
  `jeffbaileyterraformstate` (us-west-2), `hashicorp/aws >= 6.67, < 7.0`, remote state from the prod
  root for the SNS topic, the indexer log group and the DID parameter name; a `phase` variable
  (`trial`/`cutover`) switches the log group, trial alarms and the prod log-group grant; Lambda env in
  `lambda-env.json`, shared with the CI smoke (see: feature-delta.md § Infrastructure).
- [DV-STZ-6] **Least-privilege IAM**: per-function roles with exact object ARNs; `s3:ListBucket`
  on the bucket ARN so a missing object is 404, with a `StringLike` `s3:prefix` condition for
  `index/*`; moto does not enforce IAM, so DELIVER verifies with the policy simulator; conditional-write IAM conditions on the
  index PUT and on `index/.write-probe`; the prod log group granted only at cut-over; a scheduler
  role limited to the two `live` aliases. No bootstrap or CI IAM delta.
- [DV-STZ-7] **Async invoke config**: 0 retries for pass and probe; the pass returns success for
  exit 0/2/3 (upstream U-STZ-5).
- [DV-STZ-8] **Edge**: one CloudFront Function on all three behaviors enforcing the exact
  (method, path) allowlist with 404; all methods reach the behaviors (upstream U-STZ-4); a response
  headers policy removes the `x-amzn-*` headers by name; logging off; no WAF.
- [DV-STZ-9] **Deploy gate order**: ref check → `ci_green` → `resolve_digest` → `cosign verify` →
  `lambda_capable` → ECR copy (skip if present, digest re-checked) → record rollback targets →
  update `$LATEST` ×3 → publish ×3 (`--code-sha256`, `--revision-id`) → `candidate` → post-checks
  on the candidate URL → no-publish pass gate (waits for a running pass, checks the `mode` echo, worst case about 1,710 s) →
  probe gate (`not_live = 1` stops unless accepted) → flip search `live` + edge checks and gap poller
  with automatic flip-back → flip pass and probe, `prev=` on all three → prune to 5 versions.
- [DV-STZ-10] **`lambda republish`** after any tofu change to versioned function configuration
  (upstream U-STZ-2). It first sets `$LATEST` to the live image digest, asserts `CodeSha256`, and
  publishes with `--code-sha256`; `lambda status` flags image drift.
- [DV-STZ-11] **Index rollback**: `reset-index [--trigger]` (delete marker, rebuild) is the default
  tool; `restore-index <version-id>` copies a noncurrent version back; S3 keeps noncurrent versions
  7 days, at least 5.
- [DV-STZ-12] **Monitoring**: A1-A3 reused (host in the trial, Lambda after cut-over); trial copies
  A1-T..A3-T in namespace `OpenLore/IndexerLambdaTrial`; A4 invocations > 3,000/5 min; A5 index
  > 100 MB with missing data `ignore`; A6 account-level `Errors` ≥ 1; A7 account-level `Throttles`
  ≥ 10 per 5 min; A8 search > 1,900/hour; A9 CloudFront requests > 13,000/hour; A10 freshness
  failures ≥ 3 in 15 min; A11 search 5xx ≥ 1 in 2 of 2 5-minute periods; one tag-filtered budget
  alerting at $2 and $10.
- [DV-STZ-13] **Probe schedule `cron(2/5 * * * ? *)`** so each A3 period gets one line.
- [DV-STZ-14] **Rollout**: bootstrap → 7 consecutive green trial days (measurable definition; a
  red day resets the count) → `lambda measure` gate → cut-over with host timers stopped first, then
  host standby → 7 green soak days → decommission (prod-root `removed`, then prod-indexer
  `import`). Host verbs `standby --timers`, `standby --container` and `resume` are new (upstream
  U-STZ-1).
- [DV-STZ-15] **`last-pass.json` on every pass** that writes a `pass_summary` (upstream U-STZ-3).
- [DV-STZ-16] **Cost**: about $1.60/month at list, about **$0.72 on the bill**; trial extras about
  $0.45 in total; a month just under A8 and A9 about $1.93 ($1.97 in 31 days); abuse about $0.34-0.38 per hour at
  saturation, and more in a flood, because the edge term is not capped. Prices are from the AWS Price
  List API (2026-10-30). The earlier $0.36, $0.56, $0.28, $1.58, $0.70, $2.22 and $0.15-0.29 figures are historical.
- [DV-STZ-17] **`rio-object-store`** (name confirmed, D-STZ-47): a blocking CI job on every push,
  moto 5.2.5 by digest, a skip fails it, and `indexer-build` needs it. moto 5.2.5 is proven to honour
  `If-Match` and `If-None-Match` on PutObject.
- [DV-STZ-18] **`lambda stop`** also disables the distribution; tofu ignores `enabled`; `lambda start`
  restores both, and only those.
- [DV-STZ-19] **`lambda measure` is a gate** before Stage 2, with the reserved-concurrency check.
- [DV-STZ-20] **First real publish**: the deploy records the pre-publish index version id (summary
  and `idx=` on the pass alias); A2, A3, A10 and A11 catch a bad one; recovery is `lambda rollback`,
  then `lambda restore-index`.
- [DV-STZ-21] **Served index against the record**: `last-pass.json` carries `index_published_at`, and
  the probe compares it with `/healthz` (upstream U-STZ-6).
- [DV-STZ-22] **KPI-IXD-1 and KPI-IXD-2 stay manual on Lambda**; KPI-IXD-6 has a defined gap for
  flips and for the cut-over, on `/healthz` and a search `POST`; PDS `_health` is measured on host
  steps only, and the host-path assertion is the attribution check for `lambda` verbs.
- [DV-STZ-23] **Schedules start DISABLED** and are enabled only after the first flip (D-STZ-26).
- [DV-STZ-24] **ECR keeps 10 images**, with an eviction guard at step 6 and a failed copy deleted
  (U-STZ-9).
- [DV-STZ-25] **`PROBE_BASE_URL` is the CloudFront domain in both phases.**
- [DV-STZ-26] **Metric check**: `lambda status` asserts the A8, A9 and A11 metrics with `list-metrics`;
  it closes Stage 0, and A11 blocks Stage 1 until its candidate series has been seen. The `:live`
  dimension is checked for shape only, because a healthy live search never has a 5xx.
- [DV-STZ-27] **Secret check** on the image and `lambda-env.json` for NFR-IXD-8.

## Infrastructure Summary

- Deployment: AWS us-east-1, three Lambda functions (arm64 container image) behind CloudFront,
  index in one versioned S3 object; blue-green by alias (`candidate` checked, then `live` flips);
  rollback by alias flip; laptop-run `deploy.sh lambda`.
- CI/CD: GitHub Actions `ci.yml` extended; trunk-based, every push to `main`; CI never deploys and
  has no AWS access.
- Observability: CloudWatch Logs (JSON), log metric filters + `AWS/Lambda` and `AWS/CloudFront`
  metrics, alarms on the
  existing SNS topic, AWS Budgets email; no tracing, no dashboards.
- Mutation testing: per-feature, ≥ 80% on modified files (unchanged in CLAUDE.md).

## Constraints Established

- The host indexer and Lambda never write to the same log group at the same time.
- No tofu change to function configuration is live until `lambda republish` (or a deploy).
- The index bucket is never deleted by tofu; there is one conditional writer.
- Every deployed digest passed serve-smoke and lambda-smoke in CI, is CI-green and cosign-verified,
  and has the Lambda root.
- The ECR lifecycle never evicts a digest referenced by `live`, `candidate`, `$LATEST` or any `prev=` record: it
  keeps 10 images, against 5 pruned versions, and `lambda deploy` refuses a copy that would evict
  one. The two counts are not "in step": the lifecycle counts pushes, not references. Rollback still
  refuses a target whose image is gone.
- No schedule runs before the first flip.
- `republish` never falls back to `prev=`.
- A6/A7 are account-level and valid only while these are the only Lambda functions in us-east-1.
- The search kill switch, both halves, is never undone by a tofu apply.
- `republish` only ever publishes the image already live.
- No digest is signed unless `rio-object-store` ran RIO-1..9 with no skip.
- Caddy's `index.` certificate must have ≥ 14 days left at cut-over for host rollback to work.

## Upstream Changes

See `upstream-changes.md`: U-STZ-1 (host standby at cut-over), U-STZ-2 (`republish` after
versioned config changes), U-STZ-3 (`last-pass.json` on every pass), U-STZ-4 (edge allowlist by
CloudFront Function on all behaviors), U-STZ-5 (0 async retries, pass return contract), and new on
2026-10-30, proposed: U-STZ-6 (`index_published_at` and the probe comparison), U-STZ-7 (the probe
refuses a non-object event), U-STZ-8 (first write probe on a no-publish run). DEVOPS-level changes
(budget, S3 retention, probe schedule, IAM, smoke method) are in `../feature-delta.md` § Changed
Assumptions (DEVOPS) and § Changed Assumptions (DEVOPS revision, 2026-10-30).

## User decisions (2026-10-27)

All five were confirmed as proposed.

| # | Decision | Answer |
|---|---|---|
| UD-1 | Rollout length: 7 green trial days **before** the CNAME flip plus a 7-day soak after it (DESIGN had only the 7 days after), or a single 7-day window | 7 + 7 (user) |
| UD-2 | Keep both new alarms A6 (Errors) and A7 (Throttles), +$0.20/month list, or drop A7 (A4 already covers abuse) | Keep both (user) |
| UD-3 | Pass gate: invoke the new pass version against the production bucket before flipping `live` | Yes (user). **Changed 2026-10-30**: the gate stays, but is no-publish (user, D-STZ-44). |
| UD-4 | Accept U-STZ-1..5 (architect to confirm) | Accepted (user) |
| UD-5 | moto (Apache-2.0) as the S3/SSM stand-in in CI | Yes (user) |

## Handoff

- **DISTILL**: `../environments.yaml`; the event and payload contracts in feature-delta.md
  § Monitoring contracts; the lambda-smoke assertions; the deploy gate order for stubbed tests.
- **DELIVER**: Dockerfile, `indexer-build`/`indexer-image`, `msrv` and `rio-object-store` jobs,
  `deny.toml`, xtask, `deploy-pds-check.yml`, the prod-indexer root, `deploy.sh lambda` verbs and host
  `standby`/`resume`, runbook updates, and the backlog below.
- **Operator**: the apply order, Cloudflare records, the trial, cut-over and decommission.
- **Review**: a new deployment target triggers the optional per-wave review
  (`/nw-review nw-platform-architect-reviewer`); it was not run here. The consolidated review runs at
  the end of DISTILL.

## DELIVER backlog

In order where order matters:

1. **UI-7 first** (D-STZ-45): in the lib-split step, drop `cli`'s `openlore-indexer` dev-dependency,
   use build-before-run, and add the xtask test. Update the comment in `ci.yml`'s `test` job.
2. **The reused pass entry is `in_serve_pass`** or its extracted equivalent (D-STZ-42), not `run_pass`.
3. **Green before commit (L-10)**: every scaffold test that is not `#[ignore]`d, the GUARD rows in
   `../distill/red-classification.md` included, must be green before any commit; only the `#[ignore]`d
   RED scenarios may be red. Keep `rio-object-store` out of `ci.yml` until RIO-1..9 are green, in the
   same commit as the adapter, or `main` goes red.
4. Build the seed index helper and the race shim for lambda-smoke.
5. Evidence to record: P-5 (`lambda measure`, with the digest), P-15 (policy simulator on the applied
   policy), P-17 (empty-bucket 404 with the prefix condition), P-18 (429 at cap 3), the metric check,
   the time `lambda stop` and `start` take at the edge, and the first scheduled publish.

Out of scope for this feature, pre-existing, recorded only:

- **L-11**: the dots in the cosign certificate identity regex in `deploy/indexer/deploy.sh` are
  unescaped.
- **L-12**: most actions in `.github/workflows/` are pinned by tag, not by commit sha, including
  `sigstore/cosign-installer@v3`, which installs the verifier. `aquasecurity/trivy-action` is already
  pinned by sha.
