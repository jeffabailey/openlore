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
| `review-app/host/compose.yaml` | `/pds/app/compose.yaml`: image by digest only, read-only rootfs, 192 MB (`mem_limit: 192m`), no caps, exactly two mounts (`/pds/app/data`, `/pds/app/secrets` read-only), no cloud credentials |
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

Going live (first deploy, alarms, announcement) follows the
[Go-live checklist](README.md#go-live-checklist) below; `deploy/review-app/README.md` is the app's
short runbook.

## Indexer (index.openlore.jeffbailey.us)

The public search index runs as the `openlore-indexer` container next to the review app, with a
15-minute pass timer. Its runbook is `deploy/indexer/README.md`: first deploy, update, rollback,
DID-list edits and alarm triage. The go-live order (with the memory gate and the alarm
test-fires) is the [Go-live checklist](README.md#go-live-checklist) below. Design:
`docs/feature/indexer-deployment/devops/`.

When you run the R-REPLACE runbook, step 7 ("redeploy apps") also runs
`deploy/indexer/deploy.sh redeploy` once the indexer has been deployed.

## Go-live checklist

This is the one ordered go-live sequence for **both** apps (the review app and the indexer). The
other runbooks (`deploy/review-app/README.md`, `deploy/indexer/README.md`, the devops design docs)
point here and keep only the detail of their own steps. Work top to bottom; each step is a gate
for the next. Nothing here runs from CI: every command is run by the operator from the laptop
(`export AWS_PROFILE=jeff AWS_REGION=us-east-1`) or, where it says so, on the host in an
`aws ssm start-session` shell.

**Operator decisions** (made by the operator at the step named, not by this runbook):

- **Instance size:** stay on t4g.micro, or move to t4g.small (memory gate, step 12).
- **I-0 separate vs folded:** apply the indexer's prod resources before the module bump, or let
  them ride the replacement plan (step 5).
- **App go-live order:** either app may go live first (steps 9-10, 17).
- **`OPENLORE_INDEXER_TRUSTED_PROXIES` narrowing** to the actual `pds_default` subnet
  (`deploy/indexer/README.md`, *Trusted proxies*).
- **DID list contents** (step 7).
- **GitHub PAT expiry** for `github-token` (step 7).
- **The downtime window** for the replacement (step 5).

### 0. tofu-aws-pds v1.7.0 is released

The roots pin module v1.7.0 (released 2026-10-10; rollback re-pins v1.6.0). v1.7.0 was operator work in the `tofu-aws-pds` repo (B1): M-1
creates `/pds/caddy/sites`, mounts it at `/etc/caddy/sites:ro` and adds
`import /etc/caddy/sites/*.caddy` to the Caddyfile; M-2 sets the IMDS hop limit to 1 (and fixes
its comment); plus tests, CHANGELOG and the tag. Check the tag exists **before** any ref bump:

```sh
git ls-remote --tags --refs https://github.com/jeffabailey/tofu-aws-pds v1.7.0   # must print exactly one refs/tags/v1.7.0 line
```

No line, no go: stop here.

### 1. Operator prerequisites

- Tools: OpenTofu 1.12.x, AWS CLI v2 (`jeff` profile) with the Session Manager plugin, `jq`, `gh`
  (signed in), `cosign`, `crane` or `docker buildx`, `docker`, `dig`.
- The `jeff` principal can call at least `ssm:SendCommand`, `ssm:StartSession`,
  `ssm:GetCommandInvocation`, `ssm:PutParameter`, `logs:StartQuery`, `logs:GetQueryResults`,
  `logs:CreateLogStream`, `logs:PutLogEvents` and `sns:ListSubscriptionsByTopic`, besides what
  `tofu apply` needs. `deploy.sh` uses SSM Run Command, `deploy/indexer/deploy.sh status` uses Logs
  Insights, and the test-fires write log lines.
- DNS (Cloudflare, see *DNS*): both app hostnames resolve to the EIP **before any install**:

  ```sh
  dig +short app.openlore.jeffbailey.us     # -> the public_ip output
  dig +short index.openlore.jeffbailey.us   # -> the same address
  ```

- The SNS topic for alarms has the operator's email subscribed (confirmed in step 13).

### 2. Bump both roots together

Set `?ref=v1.7.0` in `deploy/tofu/bootstrap/main.tf` **and** `deploy/tofu/environments/prod/main.tf`
in one commit (CI's deploy-pds-check fails a mismatched pair and validates the tag), then
`tofu init -upgrade` in each root.

### 3. Verified identity backup (gate)

Run `pds-backup-identity` (section 5), print `sha256sum /pds/secrets.env` on the host, restore the
newest archive on the laptop and compare `sha256sum restored/secrets.env`. The hashes must be
equal; stop if they are not.

### 4. Bootstrap apply (once)

This adds the review app's and the indexer's host IAM (`review-app-iam.tf`, `indexer-iam.tf`):

```sh
cd deploy/tofu/bootstrap
tofu plan -out=bootstrap.tfplan
../../check-plan.sh bootstrap.tfplan
tofu apply bootstrap.tfplan
```

### 5. R-REPLACE: the one instance replacement (prod apply, once)

**Rollback first.** The old instance is terminated, so the way back is a second replacement on
the old module. If either app is running, stop **both** before re-pinning, because v1.6.0 brings
back IMDS hop limit 2 (containers could reach the host role):

```sh
deploy/review-app/deploy.sh stop
deploy/indexer/deploy.sh stop
```

then set `?ref=v1.6.0` in both roots and run the same plan, gate and apply as below. Leave the
apps stopped until v1.7.0 is back.

Pre-flight on the current host: record `docker compose -f /pds/compose.yaml ps`, the PDS
`_health` and `describeRepo` for `jeff`, and check that `grep -i aws /pds/pds.env` prints nothing.
Announce the downtime (a few minutes). Then, in prod:

```sh
cd deploy/tofu/environments/prod
tofu plan -replace=module.pds.aws_instance.pds -out=tfplan
OPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan   # the instance replace is expected
tofu apply tfplan
```

`-replace=` is required: the module has no `user_data_replace_on_change`, so a plain plan is an
in-place stop/start and the new user-data never runs. Expect the instance replace, the metadata
options folded into it, the volume attachment replace, and the creates of `review-app.tf` and
`indexer.tf` with alarms disabled (`review_app_alarms_enabled = false`,
`indexer_alarms_enabled = false`). This is the only prod apply before step 14. If the indexer's
resources were already applied (I-0 separate), they show no change here.

### 6. Verify the replaced host

`_health` 200; the TLS subject and issuer for `openlore.jeffbailey.us`; `describeRepo` returns the
same DID; `swapon --show`; on the host, `/pds/caddy/sites` exists, the PDS Caddy mounts it at
`/etc/caddy/sites`, its Caddyfile has `import /etc/caddy/sites/*.caddy`, and
`docker compose -f /pds/compose.yaml exec caddy caddy validate --config /etc/caddy/Caddyfile`
passes; IMDS is closed to containers (a token PUT from a container on `pds_default` fails or times
out, from the host shell it succeeds); `tofu plan` reports "No changes."

After any later replacement, this is also where both apps come back:
`deploy/review-app/deploy.sh redeploy` and `deploy/indexer/deploy.sh redeploy`.

### 7. Secrets and the DID list

- Review app, under `/openlore/prod/review-app/`, as SecureStrings without echoing them
  (`read -rs -p 'value: ' V; echo`, then
  `aws ssm put-parameter --type SecureString --name ... --value "$V" --overwrite; unset V`):
  `client-jwk` (`openlore-review-app gen-client-jwk`), `data-key`
  (`{"kid":"d1","key":"<base64 32 bytes>"}`, key from `head -c32 /dev/urandom | base64`),
  `github-token` (fine-grained, public repositories read-only, no permissions; note its expiry)
  and `log-salt` (`openssl rand -base64 32`).
- Indexer: put the production DID list (at least 2 PDS hosts) in
  `/openlore/prod/indexer/repo-dids` (`deploy/indexer/README.md` section 5) and read it back with
  `aws ssm get-parameter`.

### 8. Public images

After CI's first push of each image, set the GHCR packages `openlore-review-app` and
`openlore-indexer` to **public** (repo, Packages, Package settings, Change visibility). `deploy.sh`
does not check this, so check it by hand, signed out, with the digest CI pushed:

```sh
docker logout ghcr.io
docker pull ghcr.io/jeffabailey/openlore-review-app@sha256:<digest>
docker pull ghcr.io/jeffabailey/openlore-indexer@sha256:<digest>
```

### 9. Review app: install and first deploy

```sh
deploy/review-app/deploy.sh install
deploy/review-app/deploy.sh deploy <40-hex git sha>
```

`install` fails closed (step 01-02 of fix-go-live-runbook-gaps): it refuses unless the IMDS probe
(`refuse_unless_isolated`: a container's token PUT ends in curl exit 7 or 28) shows containers
cannot reach IMDS, and unless the PDS Caddy mounts `/pds/caddy/sites` at `/etc/caddy/sites` and
imports it; it never creates `/pds/caddy/sites` itself, and a site that fails `caddy validate` is
removed before any reload. A refusal means step 5 or 6 is not done. The deploy waits 90 s for
`/readyz` through Caddy (the first one includes certificate issuance for `app.`). Then check
`/readyz` 200 and the `/oauth/*` documents at the public origin.

### 10. Indexer: install and first deploy

```sh
deploy/indexer/deploy.sh install
INDEXER_READY_WAIT_S=300 deploy/indexer/deploy.sh deploy <40-hex git sha>
```

`install` makes the same IMDS and Caddy-sites refusals. The first deploy's readiness wait
includes certificate issuance for `index.`, so give it more than the 60 s default
(`INDEXER_READY_WAIT_S`, whole seconds, at least 2). Then follow `deploy/indexer/README.md`
section 2 (post-checks, the first pass, HTTPS redirect).

### 11. Rollback drills (hard gate, both apps)

Deploy N, then N+1, then `deploy/review-app/deploy.sh rollback` and
`deploy/indexer/deploy.sh rollback`; each returns to N in about a minute and is ready again (the
indexer's next pass exits 0 in `deploy/indexer/deploy.sh status`). Record the times.

### Memory gate (step 12)

Hard gate: the memory alarms are deferred, so this is the only proof of headroom. Run it with the
production DID list in place and both apps on their final caps (indexer 128m, review app 192m
with DuckDB 48 MB / 1 thread, `OPENLORE_REVIEW_SCAN_CONCURRENCY` 2).

**PASS** (platform-architecture §6) when, over the whole 20 minutes:

- indexer `memory.peak` ≤ 100 MB (104857600 bytes);
- review app `memory.peak` ≤ 128 MB (134217728 bytes);
- `MemAvailable` > 128 MB (131072 kB) at its minimum;
- `pswpin` does not grow;
- PDS `_health` is 200 on every poll, and every search answers 200 or 429 with p95 ≤ 1 s.

A cgroup can never exceed its own cap, so "under the limit" proves nothing; these thresholds
leave headroom under it.

1. On the host (SSM session, as root), sample every 5 s for 20 minutes and print the values:

   ```sh
   ix=$(docker inspect -f '{{.Id}}' openlore-indexer)
   ra=$(docker inspect -f '{{.Id}}' review-app-review-app-1)
   for i in $(seq 240); do
     for app in "indexer $ix" "review-app $ra"; do
       set -- $app; cg=/sys/fs/cgroup/system.slice/docker-$2.scope
       echo "$(date -u +%T) $1 peak=$(cat "$cg/memory.peak") current=$(cat "$cg/memory.current")"
     done
     echo "$(date -u +%T) $(grep MemAvailable /proc/meminfo) $(grep '^pswpin ' /proc/vmstat)"
     sleep 5
   done | tee /tmp/memory-gate.log
   ```

   Read the result: the last `peak=` per app (bytes) and the smallest `MemAvailable` (kB):

   ```sh
   grep ' indexer peak=' /tmp/memory-gate.log | tail -n 1
   grep ' review-app peak=' /tmp/memory-gate.log | tail -n 1
   awk '/MemAvailable/ {if (min == "" || $3 < min) min = $3} END {print "MemAvailable min kB", min}' /tmp/memory-gate.log
   awk '/pswpin/ {print $NF}' /tmp/memory-gate.log | sed -n '1p;$p'   # first and last: must be equal
   ```

2. On the laptop, poll the PDS every 10 s for the same window:

   ```sh
   for i in $(seq 120); do curl -s -o /dev/null -w '%{http_code}\n' https://openlore.jeffbailey.us/xrpc/_health; sleep 10; done | sort | uniq -c
   ```

3. During the window: `deploy/indexer/deploy.sh trigger` (a full pass), start a 10-repo scan in the
   review app's UI, and run the search load from the laptop. The indexer allows each client
   10 searches/s with a burst of 50; four workers that each sleep 0.5 s after a request stay under
   8/s with at most 4 in flight. A 429 is the limiter, not a failure: it is counted and left out
   of the 200 and p95 checks; any other status fails.

   The search must hit real rows: take its subject from one of the seeded DID's own claims
   (`OPENLORE_DID`, section 4b) and check the index returns it before the load.

   ```sh
   subject=$(curl -fsS "https://openlore.jeffbailey.us/xrpc/com.atproto.repo.listRecords?repo=$OPENLORE_DID&collection=org.openlore.claim&limit=1" | jq -er '.records[0].value.subject')
   jq -n --arg value "$subject" '{dimension: "subject", value: $value}' > search.json
   curl -fsS -X POST -H "content-type: application/json" --data @search.json https://index.openlore.jeffbailey.us/xrpc/org.openlore.appview.searchClaims | jq -e '.total_claims > 0'   # must print true
   seq 100 | xargs -P 4 -I{} sh -c 'curl -s -o /dev/null -w "%{http_code} %{time_total}\n" -X POST -H "content-type: application/json" --data @search.json https://index.openlore.jeffbailey.us/xrpc/org.openlore.appview.searchClaims; sleep 0.5' > search-load.txt
   awk '{print $1}' search-load.txt | sort | uniq -c                  # only 200 (and 429) allowed
   awk '$1 == 200 {print $2}' search-load.txt | sort -n | awk '{t[NR] = $1} END {print "p95 s", t[int(NR * 0.95)]}'
   ```

4. Check the DuckDB read-back (48 MB / 1 thread) in each app's startup probe event, and the next
   day that `CPUCreditBalance` over 24 h is flat or rising.
5. **On FAIL:** in the repo, set `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` to 2 in
   `deploy/indexer/host/compose.yaml` and `OPENLORE_REVIEW_SCAN_CONCURRENCY: "1"` in
   `deploy/review-app/host/compose.yaml`, commit, run `deploy/indexer/deploy.sh redeploy` and
   `deploy/review-app/deploy.sh redeploy` (they copy the compose files from the repo), and
   re-measure. If it still fails, **stop: t4g.small is an operator decision** (`instance_type` in
   `deploy/environments/prod.json`, a plan showing an in-place update, a stop/start that is a
   second PDS outage of a few minutes; then redeploy both apps and re-measure). Record the
   decision in `docs/feature/indexer-deployment/devops/wave-decisions.md`.

### 13. Confirm the SNS subscription

`aws sns list-subscriptions-by-topic --topic-arn <backup-alarm topic arn>` shows the email with a
real subscription ARN, not `PendingConfirmation`.

### 14. Enable the alarms (apply), for both apps

Set the defaults of `review_app_alarms_enabled` (`review-app.tf`) and `indexer_alarms_enabled`
(`indexer.tf`) so that the root has `review_app_alarms_enabled = true` and
`indexer_alarms_enabled = true`, commit, and apply. The plan is in-place updates only, so the gate
runs without the override:

```sh
cd deploy/tofu/environments/prod
tofu plan -out=tfplan
../../../check-plan.sh tfplan
tofu apply tfplan
```

### 15. Then test-fire every alarm

Each alarm must reach ALARM, return to OK, and both emails must arrive. Injected lines go to a
`test-fire` stream so the real streams stay clean; the metric filters read the whole group.

**Review app** (`/openlore/prod/review-app`):

- **A-3 and A-1:** `deploy/review-app/deploy.sh stop`; within about 5 minutes A-3
  (`app_running = 0`) and A-1 (the Route 53 check sees a 503) fire. Then
  `deploy/review-app/deploy.sh redeploy`; both return to OK.
- **A-7 and A-8:** inject one line each:

  ```sh
  aws logs create-log-stream --log-group-name /openlore/prod/review-app --log-stream-name test-fire
  aws logs put-log-events --log-group-name /openlore/prod/review-app --log-stream-name test-fire \
    --log-events "$(jq -cn --argjson t "$(date +%s000)" --arg m '{"event":"guardrail.breach","kpi":"TEST"}' '[{timestamp: $t, message: $m}]')"
  aws logs put-log-events --log-group-name /openlore/prod/review-app --log-stream-name test-fire \
    --log-events "$(jq -cn --argjson t "$(date +%s000)" --arg m '{"event":"github.token.expiring","days_left":-1}' '[{timestamp: $t, message: $m}]')"
  ```

  A-7 (5-minute period) goes to ALARM within minutes and back to OK after the next empty period.
  A-8 has a 1-day period: its OK can take up to a day.

**Indexer** (`/openlore/prod/indexer`):

- **A2 (end to end, purge-safe):** append `,not-a-did` to the DID list (step 7), run
  `deploy/indexer/deploy.sh trigger`; the ALARM email arrives within 15 min and nothing is
  purged. Restore the value and trigger again; OK follows the clean pass.
- **A1 (synthetic, about 75 min):** on the host, `sudo systemctl stop openlore-indexer-pass.timer`
  (`deploy/indexer/deploy.sh stop` would also stop the container). **Do not leave the pass timer
  stopped for more than 2 h:** the DID list is rendered by the pass unit, so after 2 h it is stale,
  the health line sets `not_live = 1`, and that fires A3. On the laptop:

  ```sh
  aws logs create-log-stream --log-group-name /openlore/prod/indexer --log-stream-name test-fire
  fire() { # $1 = one pass_summary line
    aws logs put-log-events --log-group-name /openlore/prod/indexer --log-stream-name test-fire \
      --log-events "$(jq -cn --argjson t "$(date +%s000)" --arg m "$1" '[{timestamp: $t, message: $m}]')"
  }
  fire '{"event":"indexer.ingest.pass_summary","exit_code":3,"pass_id":"TEST-a1-1"}'   # quarter hour P1
  fire '{"event":"indexer.ingest.pass_summary","exit_code":0,"pass_id":"TEST-a1-2"}'   # P2: no alarm
  fire '{"event":"indexer.ingest.pass_summary","exit_code":3,"pass_id":"TEST-a1-3"}'   # P3
  fire '{"event":"indexer.ingest.pass_summary","exit_code":3,"pass_id":"TEST-a1-4"}'   # P4: ALARM
  fire '{"event":"indexer.ingest.pass_summary","exit_code":0,"pass_id":"TEST-a1-5"}'   # P5: OK
  ```

  Run each `fire` in its own quarter hour (:00-:14, :15-:29, ...). Then
  `sudo systemctl start openlore-indexer-pass.timer`. The TEST lines keep `summary_45m = 1`, and
  `deploy/indexer/deploy.sh status` ignores `TEST*` pass ids.
- **A3 (end to end):** `deploy/indexer/deploy.sh stop`, wait about 10 min (ALARM), then
  `deploy/indexer/deploy.sh start` (OK).

### 16. Turn on the production smoke

```sh
gh variable set REVIEW_APP_LIVE --body true
```

The nightly workflow's production half then runs against `app.`.

### 17. Announce

Only after steps 11, 12 and 15 pass: the review app at `https://app.openlore.jeffbailey.us`, and
`OPENLORE_INDEXER_URL=https://index.openlore.jeffbailey.us` for `openlore search`.

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
