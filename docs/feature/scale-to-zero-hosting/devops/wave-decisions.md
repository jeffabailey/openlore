# DEVOPS Decisions: scale-to-zero-hosting

- **Date**: 2026-10-27. **Architect**: Apex (nw-platform-architect), autonomous. Design only.
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
  refused. No `file://` test seam: a release image refuses it, so it cannot smoke the artifact
  (settles DESIGN open question 1).
- [DV-STZ-3] **New `msrv` job**: 1.91.0 checks the workspace without the two Lambda crates; 1.94.1
  checks the two Lambda crates; `fallback` resolution because `Cargo.lock` is gitignored; an xtask
  check that only those two crates set 1.94.1. No MSRV job existed before.
- [DV-STZ-4] **cargo-deny**: ban `aws-lc-rs`, `aws-lc-sys`, `aws-lc-fips-sys`; no license change
  expected.
- [DV-STZ-5] **Tofu root `prod-indexer`**: state `openlore/indexer/prod.tfstate` in
  `jeffbaileyterraformstate` (us-west-2), `hashicorp/aws >= 6.67, < 7.0`, remote state from the prod
  root for the SNS topic, the indexer log group and the DID parameter name; a `phase` variable
  (`trial`/`cutover`) switches the log group, trial alarms and probe URL; Lambda env in
  `lambda-env.json`, shared with the CI smoke (see: feature-delta.md § Infrastructure).
- [DV-STZ-6] **Least-privilege IAM**: per-function roles with exact object ARNs; `s3:ListBucket`
  so a missing object is 404; conditional-write IAM conditions on the index PUT; log permissions on
  both the trial and prod groups up front; a scheduler role limited to the two `live` aliases. No
  bootstrap or CI IAM delta.
- [DV-STZ-7] **Async invoke config**: 0 retries for pass and probe; the pass returns success for
  exit 0/2/3 (upstream U-STZ-5).
- [DV-STZ-8] **Edge**: one CloudFront Function on all three behaviors enforcing the exact
  (method, path) allowlist with 404; all methods reach the behaviors (upstream U-STZ-4); a response
  headers policy removes the `x-amzn-*` headers by name; logging off; no WAF.
- [DV-STZ-9] **Deploy gate order**: ref check → `ci_green` → `resolve_digest` → `cosign verify` →
  `lambda_capable` → ECR copy (skip if present, digest re-checked) → record rollback targets →
  update `$LATEST` ×3 → publish ×3 (`--code-sha256`, `--revision-id`) → `candidate` → post-checks
  on the candidate URL → pass gate (sync invoke of the new pass version) → probe gate → flip search
  `live` + edge checks with automatic flip-back → flip pass and probe → prune to 5 versions.
- [DV-STZ-10] **`lambda republish`** after any tofu change to versioned function configuration
  (upstream U-STZ-2).
- [DV-STZ-11] **Index rollback**: `reset-index [--trigger]` (delete marker, rebuild) is the default
  tool; `restore-index <version-id>` copies a noncurrent version back; S3 keeps noncurrent versions
  7 days, at least 5.
- [DV-STZ-12] **Monitoring**: A1-A3 reused (host in the trial, Lambda after cut-over); trial copies
  A1-T..A3-T in namespace `OpenLore/IndexerLambdaTrial`; A4 invocations > 3,000/5 min; A5 index
  > 100 MB with missing data `ignore`; A6 account-level `Errors` ≥ 1; A7 account-level `Throttles`
  ≥ 10 per 5 min; one tag-filtered budget alerting at $2 and $10.
- [DV-STZ-13] **Probe schedule `cron(2/5 * * * ? *)`** so each A3 period gets one line.
- [DV-STZ-14] **Rollout**: bootstrap → 7 green trial days (measurable definition) → cut-over with
  host standby → 7 green soak days → decommission (prod-root `removed`, then prod-indexer
  `import`). Host verbs `standby` and `resume` are new (upstream U-STZ-1).
- [DV-STZ-15] **`last-pass.json` on every pass** that writes a `pass_summary` (upstream U-STZ-3).
- [DV-STZ-16] **Cost**: about $0.36/month for DESIGN's scope (confirmed), about $0.56 with the
  Errors and Throttles alarms, about $0.28 with free tiers; trial extras about $0.30 in total;
  abuse about $0.15-0.29 per hour at saturation.

## Infrastructure Summary

- Deployment: AWS us-east-1, three Lambda functions (arm64 container image) behind CloudFront,
  index in one versioned S3 object; blue-green by alias (`candidate` checked, then `live` flips);
  rollback by alias flip; laptop-run `deploy.sh lambda`.
- CI/CD: GitHub Actions `ci.yml` extended; trunk-based, every push to `main`; CI never deploys and
  has no AWS access.
- Observability: CloudWatch Logs (JSON), log metric filters + `AWS/Lambda` metrics, alarms on the
  existing SNS topic, AWS Budgets email; no tracing, no dashboards.
- Mutation testing: per-feature, ≥ 80% on modified files (unchanged in CLAUDE.md).

## Constraints Established

- The host indexer and Lambda never write to the same log group at the same time.
- No tofu change to function configuration is live until `lambda republish` (or a deploy).
- The index bucket is never deleted by tofu; there is one conditional writer.
- Every deployed digest passed serve-smoke and lambda-smoke in CI, is CI-green and cosign-verified,
  and has the Lambda root.
- The ECR lifecycle (5 images) and the Lambda version prune (5 versions) stay in step; rollback
  refuses a target whose image is gone.
- A6/A7 are account-level and valid only while these are the only Lambda functions in us-east-1.
- The search kill switch is never undone by a tofu apply.
- Caddy's `index.` certificate must have ≥ 14 days left at cut-over for host rollback to work.

## Upstream Changes

See `upstream-changes.md`: U-STZ-1 (host standby at cut-over), U-STZ-2 (`republish` after
versioned config changes), U-STZ-3 (`last-pass.json` on every pass), U-STZ-4 (edge allowlist by
CloudFront Function on all behaviors), U-STZ-5 (0 async retries, pass return contract). DEVOPS-level
changes (budget, S3 retention, probe schedule, IAM, smoke method) are in `../feature-delta.md`
§ Changed Assumptions (DEVOPS).

## User decisions (2026-10-27)

All five were confirmed as proposed.

| # | Decision | Answer |
|---|---|---|
| UD-1 | Rollout length: 7 green trial days **before** the CNAME flip plus a 7-day soak after it (DESIGN had only the 7 days after), or a single 7-day window | 7 + 7 (user) |
| UD-2 | Keep both new alarms A6 (Errors) and A7 (Throttles), +$0.20/month list, or drop A7 (A4 already covers abuse) | Keep both (user) |
| UD-3 | Pass gate: invoke the new pass version against the production bucket before flipping `live` | Yes (user) |
| UD-4 | Accept U-STZ-1..5 (architect to confirm) | Accepted (user) |
| UD-5 | moto (Apache-2.0) as the S3/SSM stand-in in CI | Yes (user) |

## Handoff

- **DISTILL**: `../environments.yaml`; the event and payload contracts in feature-delta.md
  § Monitoring contracts; the lambda-smoke assertions; the deploy gate order for stubbed tests.
- **DELIVER**: Dockerfile, `indexer-build`/`indexer-image`, `msrv` job, `deny.toml`, xtask,
  `deploy-pds-check.yml`, the prod-indexer root, `deploy.sh lambda` verbs and host `standby`/
  `resume`, runbook updates.
- **Operator**: the apply order, Cloudflare records, the trial, cut-over and decommission.
- **Review**: a new deployment target triggers the optional per-wave review
  (`/nw-review nw-platform-architect-reviewer`); it was not run here. The consolidated review runs at
  the end of DISTILL.
