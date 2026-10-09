# OpenLore PDS — deploy runbook

OpenLore's own ATProto PDS at **`https://openlore.jeffbailey.us`**, handle
**`jeff.openlore.jeffbailey.us`**, in the `jeff` AWS account (091153021562), us-east-1.

It is built from the shared module repo
[`jeffabailey/tofu-aws-pds`](https://github.com/jeffabailey/tofu-aws-pds) at a pinned tag
(ADR-066). The same module runs the-reality-base's PDS in a different account (ADR-067).
Sizing (ADR-068): one t4g.micro with 1 GiB swap, an 8 GB root, a 5 GB gp3 data volume and an
Elastic IP. There is one environment, `prod`, applied by a human from a laptop against a saved
plan. CI (`.github/workflows/deploy-pds-check.yml`) runs credential-free checks only.

> **R1 — read before relying on this PDS.** `openlore claim publish` cannot target this PDS
> yet. The `org.openlore.claim` lexicon declares `confidence` as a float, which the ATProto
> data model does not allow, and a stock PDS re-encodes it so the CID diverges (ADR-062). The
> adapter also sends `createRecord` without auth. This PDS can hold the account and its DID
> today. Publishing claims to it is the next feature.

## Layout

| Path | What it is |
|---|---|
| `environments/prod.json` | The descriptor, the single source of every name |
| `tofu/bootstrap/` | Root for `modules/pds-bootstrap`: backup bucket, host role + instance profile, default VPC. State key `openlore/pds/bootstrap.tfstate` |
| `tofu/environments/prod/` | Root for `modules/pds`. State key `openlore/pds/prod.tfstate` |
| `review-app/` | The hosted review app's deploy tooling and host files (see *Review app*) |
| `indexer/` | The public index's deploy tooling, host files and runbook (see *Indexer*) |
| `check-plan.sh` | Gate on a saved plan: refuses delete/replace of the volume, EIP, zone or buckets, and any other delete unless `OPENLORE_ALLOW_DELETE=1` |

State lives in the existing bucket `jeffbaileyterraformstate` (us-west-2) with native S3
locking (`use_lockfile`). There is no DynamoDB table.

The prod root takes the bootstrap's outputs as variables whose defaults are the known values
(`openlore-pds-host-prod`, `openlore-identity-backup-091153021562`, zone
`Z04289081C40P36K0S8LM`). Those names follow from `name_prefix` and the account id, so the root
does not read the bootstrap's remote state. Step 2 checks that the defaults still match.

## Prerequisites (once)

Tools: OpenTofu 1.12.x, the AWS CLI v2 with a `jeff` profile, `jq`, and the Session Manager
plugin for `aws ssm start-session`.

**0a. Turn on versioning for the state bucket.** It holds other projects' state too, and every
state file benefits:

```sh
aws s3api put-bucket-versioning --profile jeff --region us-west-2 \
  --bucket jeffbaileyterraformstate \
  --versioning-configuration Status=Enabled
aws s3api get-bucket-versioning --profile jeff --region us-west-2 --bucket jeffbaileyterraformstate
# -> "Status": "Enabled"
```

**0b. Put the ACME contact address in SSM.** It must never appear in a committed file, a
variable or state. The host reads it at boot by parameter name. Read it from the terminal so
it stays out of shell history:

```sh
read -r -p 'ACME contact address: ' ACME_CONTACT
aws ssm put-parameter --profile jeff --region us-east-1 \
  --name /openlore/prod/acme-contact-email --type SecureString --value "$ACME_CONTACT"
unset ACME_CONTACT
```

## 1. Bootstrap (once, before prod)

```sh
cd deploy/tofu/bootstrap
export AWS_PROFILE=jeff
tofu init
tofu plan -out=bootstrap.tfplan
../../check-plan.sh bootstrap.tfplan
tofu apply bootstrap.tfplan
tofu output
```

The plan creates about 8 resources: the backup bucket and its versioning, encryption and
public-access block, the `openlore-pds-host-prod` role, its policy, the SSM attachment and the
instance profile, and `aws_default_vpc`. The account guard refuses any account other than
091153021562. The zone check refuses a record outside `jeffbailey.us`.

**Default VPC (R2, unverified).** The account has no default VPC in us-east-1, so
`aws_default_vpc` should create one, with a default subnet in each AZ. If the apply does not
create it, or prod's plan says "no matching EC2 VPC found", create it once by hand and re-run
the bootstrap plan, which then adopts it:

```sh
aws ec2 create-default-vpc --profile jeff --region us-east-1
```

## 2. Prod: plan, gate, apply

```sh
cd deploy/tofu/environments/prod
export AWS_PROFILE=jeff
tofu init
tofu plan -out=tfplan
../../../check-plan.sh tfplan
tofu apply tfplan
```

Before the first apply, confirm that the variable defaults match the bootstrap outputs
(`backup_bucket`, `host_instance_profile_names["prod"]`, `hosted_zone_id`). The first plan
creates the security group and its rules, the data volume, the instance, the volume
attachment, the EIP and two Route 53 A records (the hostname and its wildcard).

`check-plan.sh` never lets through a delete or replace of `aws_ebs_volume`, `aws_eip`,
`aws_route53_zone` or `aws_s3_bucket`. Any other delete, such as replacing the instance, is
refused unless you have read the plan and run
`OPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan`.

## 3. Post-deploy checks

First boot takes a few minutes: package update, Docker, image pull, then the certificate.

```sh
curl -fsS https://openlore.jeffbailey.us/xrpc/_health          # -> {"version":"..."}
openssl s_client -connect openlore.jeffbailey.us:443 -servername openlore.jeffbailey.us </dev/null 2>/dev/null | grep -E 'subject=|issuer='
tofu plan                                                       # -> No changes.
```

If health fails, open a shell on the host. SSH is closed by design, so use Session Manager:

```sh
aws ssm start-session --profile jeff --region us-east-1 --target "$(tofu output -raw instance_id)"
sudo tail -n 100 /var/log/pds-bootstrap.log
sudo docker compose -f /pds/compose.yaml ps
sudo docker compose -f /pds/compose.yaml logs --tail 100 caddy pds
swapon --show                                                   # -> /swapfile 1024M
```

## 4. The account (created by the deployment)

`bootstrap_account = true` (module v1.2.0) makes the host create `jeff.openlore.jeffbailey.us`
on first boot, with the ACME contact as its email, and store two SSM SecureString parameters:
`/openlore/prod/account-password` and `/openlore/prod/cli-app-password`. It is idempotent: a
rebuilt instance finds the handle and does nothing. Turning it on for a running host changes
`user_data`, which only runs on a new instance, so apply it with a replacement (the data volume,
and the identity on it, survives):

```sh
AWS_PROFILE=jeff tofu plan -replace=module.pds.aws_instance.pds -out=tfplan
OPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan   # the instance replace is expected
AWS_PROFILE=jeff tofu apply tfplan
```

Then publish with the CLI:

```sh
export OPENLORE_PDS_ENDPOINT=https://openlore.jeffbailey.us
export OPENLORE_PDS_IDENTIFIER=jeff.openlore.jeffbailey.us
export OPENLORE_PDS_APP_PASSWORD=$(AWS_PROFILE=jeff aws ssm get-parameter --region us-east-1 \
  --with-decryption --name /openlore/prod/cli-app-password --query Parameter.Value --output text)
./cli.sh claim publish <cid>
```

The step logs to `/var/log/cloud-init-output.log`; re-run it by hand with
`/usr/local/bin/pds-ensure-account jeff.openlore.jeffbailey.us /openlore/prod us-east-1 /openlore/prod/acme-contact-email`.

## 4b. The claim-signing key (in the DID document)

Claims are signed with an Ed25519 key that peers look up in the account's DID document as
`#org.openlore.application`. Create it once, on the machine that signs (it stays in the macOS
keychain, service `openlore`):

```sh
OPENLORE_DID=did:plc:pnyxfnpkcldxtitsw64ycahw ./cli.sh key
```

It prints a `did:key:z6Mk...`; put it in `verification_methods` in
`deploy/tofu/environments/prod/main.tf` (module v1.3.0+). On the next instance replacement the
host signs a PLC update with its rotation key and the key appears at
`https://plc.directory/did:plc:pnyxfnpkcldxtitsw64ycahw`. Then sign and publish as that identity
(`cli.sh` uses the keychain key, and its own store, for any DID but the dev one):

```sh
export OPENLORE_DID=did:plc:pnyxfnpkcldxtitsw64ycahw   # plus the OPENLORE_PDS_* lines above
./cli.sh claim add --subject ... --confidence 0.85
./cli.sh claim publish <cid>
```

## 5. Identity backup

Set up 2026-10-04. The host archives `/pds/secrets.env` (the PLC rotation key) to
`s3://openlore-identity-backup-091153021562/prod/`, hybrid-encrypted to the public key at
`/pds/backup-pubkey.pem` (RSA-4096; tofu-aws-pds v1.4.0+). The private key is in the macOS
keychain of the operator's Mac, base64-encoded: service `pds-backup`, account `openlore`. It is
the only way to decrypt these archives -- keep the keychain backed up.

Take a backup (on the host; it survives instance replacement, the key is on the data volume):

```sh
AWS_PROFILE=jeff aws ssm send-command --region us-east-1 --instance-ids <instance_id> \
  --document-name AWS-RunShellScript --parameters 'commands=["/usr/local/bin/pds-backup-identity"]'
```

Restore (OpenSSL 3; writes `./restored/secrets.env`, to place at `/pds/secrets.env` on a volume
for a new host):

```sh
AWS_PROFILE=jeff aws s3 cp s3://openlore-identity-backup-091153021562/prod/identity-<stamp>.enc.tar .
<tofu-aws-pds>/scripts/pds-restore-identity.sh identity-<stamp>.enc.tar \
  <(security find-generic-password -s pds-backup -a openlore -w | base64 -d) ./restored
```

The first archive, `identity-20261004T002631Z.enc.tar`, was restored this way and matched the
host's `secrets.env` byte for byte.

## Review app (app.openlore.jeffbailey.us)

The hosted review app (ADR-072..075) runs on this host as a second compose project behind the
PDS's Caddy. Design: `docs/feature/bluesky-claim-review-app/devops/`. CI (`ci.yml`, main pushes)
builds the arm64 image, smoke-tests it, gates it on trivy, and pushes it to
`ghcr.io/jeffabailey/openlore-review-app` (tags `sha-<sha>` and `main`), signed with cosign
keyless and attested (SBOMs + SLSA provenance). CI never deploys.

| Path | Installed on the host as |
|---|---|
| `review-app/deploy.sh` | Laptop entry point; also runs on the host over SSM as `deploy.sh host <mode>` |
| `review-app/host/compose.yaml` | `/pds/app/compose.yaml`: image by digest only, read-only rootfs, 256 MB, no caps, exactly two mounts (`/pds/app/data`, `/pds/app/secrets` read-only), no cloud credentials |
| `review-app/host/app.caddy` | `/pds/caddy/sites/app.caddy`: `app.{$PDS_HOSTNAME}` → `review-app:8080` |
| `review-app/host/render-secrets.sh` | `/pds/app/bin/`: SSM `/openlore/prod/review-app/*` → `/pds/app/secrets`, 0400, uid 65532 |
| `review-app/host/health-timer.sh` | `/pds/app/bin/` + systemd `review-app-health.timer`: one `host.health` line a minute; restarts a hung app |
| `tofu/environments/prod/review-app.tf` | Log group (30 days), Route 53 health check, alarms A-1/A-3/A-7/A-8 on the backup-alarm topic |
| `tofu/bootstrap/review-app-iam.tf` | Inline policy on `openlore-pds-host-prod`: read the app's SSM params, write its log group |

Deploy (laptop, `AWS_PROFILE=jeff`, with `gh`, `cosign`, `jq` and `crane` or `docker buildx`):

```sh
deploy/review-app/deploy.sh deploy <40-hex git sha>   # CI green + cosign verify, then Recreate by digest
deploy/review-app/deploy.sh status                    # container, last logs, releases, host.health
deploy/review-app/deploy.sh rollback [--restore-db]   # previous digest from /pds/app/state/releases
deploy/review-app/deploy.sh redeploy | stop | install
```

Tags and short shas are refused. A deploy installs the host files, renders secrets, pulls the
digest while the old one serves, copies the DuckDB file aside, starts the new digest and waits
90 s for `/readyz` through Caddy; if it is not ready it rolls back by itself and exits 1.

Resource settings in `review-app/host/compose.yaml` (each refuses startup, naming itself, when out
of range): `REVIEW_DB_MEMORY_LIMIT_MB` (16..=1024, set to 48), `REVIEW_DB_THREADS` (1..=4, set to
1) and `OPENLORE_REVIEW_SCAN_CONCURRENCY` (1..=4, default and set to 2), the number of scans the
app runs at once; the memory gate's fail path lowers it to 1.

Operator follow-ups before the first deploy (in order):

1. Release `tofu-aws-pds` **v1.7.0** (Caddy `import /etc/caddy/sites/*.caddy` with
   `/pds/caddy/sites` mounted read-only, and IMDS hop limit 1). Then bump `?ref=` in **both**
   roots together (CI fails a mismatched pair) and run the R-REPLACE runbook
   (`devops/infrastructure-integration.md` §7.1). Until then the Caddy site is installed but
   not imported, and containers could still reach the instance role.
2. Apply the bootstrap root (`review-app-iam.tf`), then plan, gate and apply prod
   (`review-app.tf`; alarms are created with `review_app_alarms_enabled = false`).
3. Put the SSM SecureStrings without echoing them (`read -rs -p 'value: ' V; echo`, then
   `aws ssm put-parameter --type SecureString --name ... --value "$V" --overwrite; unset V`): `client-jwk`
   (`openlore-review-app gen-client-jwk`), `data-key` (JSON `{"kid":"d1","key":"<base64 32 bytes>"}`, key from
   `head -c32 /dev/urandom | base64`; rotate via `data-key-previous`, infrastructure-integration §7.4),
   `github-token` (fine-grained, public repositories read-only, no permissions) and `log-salt`
   (`openssl rand -base64 32`), all under `/openlore/prod/review-app/`.
4. After the first CI image push, set the GHCR package `openlore-review-app` to **public** (the
   host pulls without credentials).
5. After the first successful deploy: set the repo variable `REVIEW_APP_LIVE=true` (enables the
   nightly production smoke), run the rollback drill, fire each alarm once, then set
   `review_app_alarms_enabled = true` and apply.

## Indexer (index.openlore.jeffbailey.us)

The public search index runs as the `openlore-indexer` container next to the review app, with a
15-minute pass timer. The runbook is `deploy/indexer/README.md`: sequencing, first deploy,
update, rollback, DID-list edits, the memory gate and the alarm test-fire. Design:
`docs/feature/indexer-deployment/devops/`.

When you run the R-REPLACE runbook, step 7 ("redeploy apps") also runs
`deploy/indexer/deploy.sh redeploy` once the indexer has been deployed.

## DNS: Cloudflare, not Route 53

`jeffbailey.us` is served by Cloudflare (`nova`/`steven.ns.cloudflare.com`); the Route 53 zone
`Z04289081C40P36K0S8LM` in this account is not delegated, so the A records OpenTofu writes there
are inert. The live records are in Cloudflare, DNS only (not proxied, so ACME and the firehose
reach the host): `openlore` and `*.openlore` → the EIP (`public_ip` output). If the EIP ever
changes, update both. Moving DNS into OpenTofu means delegating `openlore.jeffbailey.us` to a new
Route 53 zone (~$0.50/mo) with NS records in Cloudflare.

## Costs

About **$10.84/month** (us-east-1 on-demand): t4g.micro $6.13, 8 GB root gp3 $0.64, 5 GB data
gp3 $0.40, public IPv4 (EIP) $3.65, plus S3, Route 53 queries and the SSM parameter at about
$0.02. The `jeffbailey.us` zone already exists and adds nothing. Upgrading to t4g.small (about
+$6.13) is a stop/start: change `instance_type` in `environments/prod.json`. The data volume can
grow in place but can never shrink.

## Teardown

No workflow can reach a destroy. Teardown is a human act from a laptop:

- Stopping or replacing the **instance** is ordinary, because the instance is cattle. Expect
  `check-plan.sh` to require `OPENLORE_ALLOW_DELETE=1`.
- Deleting the **data volume** or the **EIP** first needs a reviewed commit that removes
  `prevent_destroy` in the module. That is a module change, which is deliberate. The volume
  holds the PLC rotation key: a `did:plc` is permanent and public and cannot be re-minted. Take
  and verify an identity backup first.
- The backup bucket also carries `prevent_destroy`, and the host's role cannot delete from it.
- Removing the bootstrap's `aws_default_vpc` from config only forgets the VPC. It does not
  delete it.
- The state objects under `openlore/pds/` in `jeffbaileyterraformstate` are removed by hand,
  last.

## Local testing

There is no test environment in AWS (ADR-068). Pre-production checks run on a laptop with
the-reality-base's `deploy/spike/compose.local.yaml` pattern (a local compose of the same PDS
image).
