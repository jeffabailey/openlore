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

## 5. Identity backup (R6, do this once the account exists)

The host can write an encrypted archive of `/pds/secrets.env` (the PLC rotation key) to
`s3://openlore-identity-backup-091153021562/prod/`. It refuses to upload until an operator
public key is in place. Generate the key pair **off the host** and keep the private key
offline:

```sh
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:4096 -out openlore-backup-private.pem
openssl pkey -in openlore-backup-private.pem -pubout -out backup-pubkey.pem
```

Use 4096-bit RSA. The host encrypts the small gzipped archive directly with `openssl pkeyutl`,
so a 2048-bit key may be too small for it.

Copy `backup-pubkey.pem` to `/pds/backup-pubkey.pem` on the host (for example, paste it into
`sudo tee` in a Session Manager shell). Then run `sudo pds-backup-identity`. It should print
`archived identity-<stamp>.tar.gz.enc`.

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
