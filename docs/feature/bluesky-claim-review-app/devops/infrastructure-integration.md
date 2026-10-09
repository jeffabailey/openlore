# Infrastructure Integration: bluesky-claim-review-app (DEVOPS)

> How the app plugs into `deploy/` and `tofu-aws-pds`, plus the operational runbooks. This is a
> design specification: DELIVER writes the files named here. File paths are proposals.

## 1. Ownership map (what lives where)

| Artifact | Owner / location | Lifecycle |
|---|---|---|
| Caddy site-import hook, IMDS hop limit 1 | `tofu-aws-pds` **v1.7.0** (`modules/pds`) | One instance replacement (R-2) |
| Host IAM: read app secrets, write app logs (including `host.health`) | `deploy/tofu/bootstrap/review-app-iam.tf` (an inline policy on the existing `openlore-pds-host-prod` role) | Laptop apply, no host impact |
| Log group, metric filters, Route 53 health check, alarms | `deploy/tofu/environments/prod/review-app.tf` | Laptop apply, no host impact |
| SSM parameters (values) | Put by the operator from the laptop, **never in tofu** (no secret in state) | Manual, see §5 |
| Host files: compose, Caddy site, secret renderer, health timer | `deploy/review-app/host/` in the repo, installed by `deploy/review-app/deploy.sh` through SSM Run Command | Every deploy (idempotent) |
| Image | `ghcr.io/jeffabailey/openlore-review-app` (CI, `ci-cd-pipeline.md`) | Every push to main |

Proposed repo layout:

```text
deploy/review-app/
  deploy.sh                    # laptop: install | deploy <sha> | rollback | redeploy | kpi | status
  host/
    compose.yaml               # -> /pds/app/compose.yaml
    app.caddy                  # -> /pds/caddy/sites/app.caddy
    render-secrets.sh          # -> /pds/app/bin/render-secrets.sh
    review-app-health.sh       # -> /pds/app/bin/review-app-health.sh
    review-app-health.service  # -> /etc/systemd/system/
    review-app-health.timer    # -> /etc/systemd/system/
deploy/tofu/bootstrap/review-app-iam.tf
deploy/tofu/environments/prod/review-app.tf
```

On-host layout. Everything except the systemd units is on the data volume, so it survives
instance replacement.

```text
/pds/app/compose.yaml          0644 root
/pds/app/bin/                  0755 root
/pds/app/data/                 0700 65532:65532   review-app.duckdb (0600), tmp/, pre-deploy copies
/pds/app/secrets/              0700 65532:65532   one file per secret, 0400 65532:65532
/pds/app/state/releases        0600 root           append-only "<utc> <sha> <digest>" lines
/pds/caddy/sites/app.caddy     0644 root
```

## 2. Module v1.7.0 changes (in `jeffabailey/tofu-aws-pds`)

Specification only; it is implemented in the module repo with its own tests (`modules/pds/tests`).

1. **M-1, site import.** In `user-data.sh.tftpl`:
   - `mkdir -p /pds/caddy/sites`;
   - add `- /pds/caddy/sites:/etc/caddy/sites:ro` to the caddy volumes;
   - append `import /etc/caddy/sites/*.caddy` at the end of the Caddyfile (top level, after
     the site blocks).

   Module test: the rendered user-data contains the mount and the import line, and
   `caddy validate` passes with an empty directory (SPIKE-4a evidence).
2. **M-2, IMDS** (approved 2026-10-04). Set `http_put_response_hop_limit = 1`.
   - **Replace the stale comment** at `modules/pds/main.tf:312` ("the container reads the
     instance role") with: "containers must not read the instance role; every AWS call
     (user-data SSM, backup `aws s3 cp`, `put-metric-data`) runs on the host".
   - Module test: a plan assertion on `metadata_options`.
   - Evidence is in platform-architecture §4: no container-side AWS use in either OpenLore or
     the-reality-base.
3. README and CHANGELOG: "Sites: drop `<name>.caddy` files into `/pds/caddy/sites` and reload
   Caddy. The module stays project-agnostic."

The OpenLore roots bump `?ref=v1.6.0` to `?ref=v1.7.0` in **both** `bootstrap/main.tf` and
`environments/prod/main.tf`, keeping them in lockstep as today.

## 3. Host IAM delta (bootstrap root)

An inline policy `openlore-review-app-host` on role `openlore-pds-host-prod`. It does not modify
the module's policy.

| Sid | Actions | Resource / condition |
|---|---|---|
| ReadReviewAppSecrets | `ssm:GetParameter`, `ssm:GetParameters`, `ssm:GetParametersByPath` | `arn:aws:ssm:us-east-1:091153021562:parameter/openlore/prod/review-app` and `.../review-app/*` |
| (existing) DecryptOwnParameter | `kms:Decrypt` via `ssm.us-east-1` | Already granted by the module, so nothing is added |
| WriteReviewAppLogs | `logs:CreateLogStream`, `logs:PutLogEvents`, `logs:DescribeLogStreams` | `arn:aws:logs:us-east-1:091153021562:log-group:/openlore/prod/review-app:*` |

There is no write to SSM and no `CreateLogGroup` (tofu owns the group). Only host processes use
the role (dockerd and root shell commands sent through SSM). M-2 keeps it out of containers.
No `PutMetricData` is granted: the alarms derive from log metric filters (monitoring-alerting §1).

## 4. Compose and Caddy (host files)

### 4.1 `/pds/app/compose.yaml` (shape)

```yaml
name: review-app
services:
  review-app:
    image: ${REVIEW_APP_IMAGE}            # ghcr.io/jeffabailey/openlore-review-app@sha256:..., from /pds/app/.env
    restart: unless-stopped
    user: "65532:65532"
    read_only: true
    tmpfs: ["/tmp:size=16m"]
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    pids_limit: 128
    mem_limit: 256m
    memswap_limit: 256m
    mem_reservation: 96m
    oom_score_adj: 800
    cpus: 1.0
    stop_grace_period: 20s
    command: ["serve"]
    environment:
      APP_ORIGIN: https://app.openlore.jeffbailey.us
      REVIEW_DB: /data/review-app.duckdb
      OPENLORE_REVIEW_SCAN_CONCURRENCY: "2"
      SECRETS_DIR: /run/secrets
      LOG_FORMAT: json
      OAUTH_SCOPES: "atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create"
    volumes:
      - /pds/app/data:/data
      - /pds/app/secrets:/run/secrets:ro
    networks: [pds_default]
    logging:
      driver: awslogs
      options:
        awslogs-region: us-east-1
        awslogs-group: /openlore/prod/review-app
        awslogs-stream: review-app
        mode: non-blocking
        max-buffer-size: 4m
networks:
  pds_default:
    external: true
```

`OPENLORE_REVIEW_SCAN_CONCURRENCY` is how many scans the app runs at once across everyone
(default 2). It must be a whole number from 1 to 4; anything else (including 0, which would
refuse every scan) refuses startup, naming the variable. The memory gate's fail path lowers it
to 1 (indexer-deployment infrastructure-integration, memory gate).

`/pds/app/.env` holds only `REVIEW_APP_IMAGE=<digest ref>`, which is not secret. It is written
by the deploy so `docker compose` restarts the exact pinned digest after a reboot.

### 4.2 `/pds/caddy/sites/app.caddy`

```caddyfile
app.{$PDS_HOSTNAME} {
	reverse_proxy review-app:8080
	# Only proxy failures (upstream down during a Recreate) are Caddy errors; the app's own 4xx/5xx pass through.
	handle_errors {
		respond "The OpenLore review app is restarting. Nothing changed. Try again in a minute." 503
	}
}
```

- An exact host is more specific than `*.{$PDS_HOSTNAME}`, so Caddy issues an ordinary HTTP-01
  certificate and never consults the on-demand `ask`.
- Security headers come from the app, not Caddy (one source of truth, so ATs can assert them).
- Reload: `docker compose -f /pds/compose.yaml exec -T caddy caddy reload --config /etc/caddy/Caddyfile`.

### 4.3 OAuth endpoints served by the app at the public origin

| Path | Content | Caching |
|---|---|---|
| `/oauth/client-metadata.json` | `client_id` (this URL), `application_type: web`, `client_name`, `client_uri: ${APP_ORIGIN}`, `redirect_uris: [${APP_ORIGIN}/oauth/callback]`, `grant_types: [authorization_code, refresh_token]`, `response_types: [code]`, `scope` (= `OAUTH_SCOPES`), `token_endpoint_auth_method: private_key_jwt`, `token_endpoint_auth_signing_alg: ES256`, `jwks_uri: ${APP_ORIGIN}/oauth/jwks.json`, `dpop_bound_access_tokens: true` | `Cache-Control: public, max-age=300` |
| `/oauth/jwks.json` | **Public** JWKs only: the active `client-jwk` plus, during rotation, `client-jwk-previous`, each with `kid`, `alg: ES256` and `use: sig` | `max-age=300` |

The startup self-probe (architecture-design §9) fetches both through the public origin and compares
them byte for byte. The deploy's external checks repeat this from the laptop. A CI unit test in
DELIVER asserts that the JWKS never contains a `d` member.

## 5. Secrets

### 5.1 Parameters (SSM SecureString, default `aws/ssm` key, standard tier)

| Name | Content | Created by | Rotation |
|---|---|---|---|
| `/openlore/prod/review-app/client-jwk` | ES256 **private** JWK with `kid` (e.g. `ol-2026-10`) | Operator: generated on the laptop (`openlore-review-app gen-client-jwk`, a DELIVER subcommand; fallback `step crypto jwk create --kty EC --crv P-256`) | Yearly, or on suspicion (§7.4) |
| `/openlore/prod/review-app/client-jwk-previous` | The previous private JWK. Present only during rotation; published in the JWKS, **never used to sign**. | Operator | Deleted at the end of rotation |
| `/openlore/prod/review-app/data-key` | `{"kid":"d1","key":"<base64 32 bytes>"}` | Operator: `head -c32 /dev/urandom \| base64` | On suspicion only (§7.4) |
| `/openlore/prod/review-app/data-key-previous` | The previous data key, present only during rotation | Operator | Deleted after re-encryption |
| `/openlore/prod/review-app/github-token` | Fine-grained PAT, **public repositories read-only, no permissions**, expiry <= 366 days | Operator on github.com | Before expiry; the alarm fires 14 days ahead (monitoring-alerting A-8) |
| `/openlore/prod/review-app/log-salt` | 32 random bytes, base64. Salts the DID hash in logs. | Operator | Never, because rotating breaks continuity of log correlation (acceptable if needed) |

Put values with the README §0b pattern, where `read -rs` keeps them out of shell history and
nothing is echoed:

```sh
read -rs -p 'GitHub token: ' V; echo
aws ssm put-parameter --profile jeff --region us-east-1 --type SecureString \
  --name /openlore/prod/review-app/github-token --value "$V" --overwrite; unset V
```

### 5.2 How the app reads them, isolated from PDS secrets

1. `render-secrets.sh` runs **on the host as root**, invoked by the deploy over SSM Run Command.
   It calls `aws ssm get-parameters-by-path --path /openlore/prod/review-app/ --with-decryption`
   with the host role.
2. It writes each value into a dot-prefixed temporary file inside `/pds/app/secrets` (umask 077)
   and checks that every required parameter is present and non-empty. Only then does it run
   `chown 65532:65532` and `chmod 0400` on each file and rename it over its final name, and remove
   the files of parameters that no longer exist. It never renames, removes or recreates the
   directory, because the container's bind mount pins the directory's inode. A render missing a
   required parameter leaves the directory unchanged.
   It runs with `set +x`; values never reach stdout, so the SSM command output stays clean.
3. The container mounts `/pds/app/secrets` read-only at `/run/secrets`. **It cannot see
   `/pds/secrets.env`** (PLC rotation key, JWT secrets), `/pds/pds.env`, `/pds/backup-pubkey.pem`
   or the PDS data. **It has no AWS credentials**, because M-2 sets hop limit 1 and nothing
   injects keys.
4. The app loads the secrets at startup only. A rotation means re-rendering and restarting.

Known residual risk (accepted, ADR-074): the **PDS** container mounts all of `/pds` and can
therefore read `/pds/app/secrets` and the DuckDB file. The PDS image is pinned by digest and is
upstream code. App-level AEAD keeps tokens opaque inside the DuckDB file, but the data key file is
readable by the PDS container. To remove that, the module would have to narrow the PDS mount,
which is out of scope (revisit if the PDS image provenance changes).

## 6. Private state: `review-app.duckdb`

- Path: `/pds/app/data/review-app.duckdb` on the EBS data volume. The volume is encrypted (the
  module encrypts the data volume) and the mount is a bind mount, not overlayfs, which the
  adapter probe checks via `/proc/mounts`.
- Permissions: the directory is 0700 and the file is 0600, both owned by 65532. The app sets
  `umask 077` at startup.
- **No backup in v1 (user decision).** It is not included in `pds-backup-identity` or in any
  snapshot policy.
- **Pre-deploy copy (a rollback aid, not a backup).** Before starting a new digest, the deploy
  stops the container and copies the file to `/pds/app/data/review-app.duckdb.pre-<sha12>`
  (same volume, same permissions). It keeps only the latest copy and deletes it after the next
  successful deploy. This is what makes a binary rollback safe across a schema migration (§7.3).
  It does not widen exposure: it is the same file on the same volume.
- **Migrations are expand-only.** Columns and tables are added; nothing is renamed or dropped in
  the same release. The binary records `schema_version` and **refuses to start** if the stored
  major version is newer than it knows, which is the rollback signal to restore the pre-deploy
  copy.
- **Volume capacity.** The data volume is 5 GB, shared with PDS blocks. The expected DuckDB size
  at < 50 users is < 50 MB. The health timer logs `/pds` free space as `pds_free_mb` (observability-design §4).
- **Single-process lock.** DuckDB allows one read-write process per file. A second process
  (`openlore-review-app kpi`, as DESIGN named it) **cannot open the file while `serve` runs.** KPI
  reads therefore go through the running server (`kpi-instrumentation.md` §4). This is a design
  correction handed to DELIVER.

## 7. Runbooks (outlines; DELIVER turns them into `deploy/review-app/README.md`)

The laptop needs: `AWS_PROFILE=jeff`, `gh` (signed in), `cosign`, and `crane` or
`docker buildx imagetools`. The instance id comes from
`tofu -chdir=deploy/tofu/environments/prod output -raw instance_id`.

### 7.1 R-REPLACE: planned instance replacement (once, for module v1.7.0; also any future user-data change)

Rollback for this runbook comes first. The old instance is terminated, so there is no rollback
to the same instance. The rollback path is: re-pin `v1.6.0`, run plan with
`-replace=module.pds.aws_instance.pds`, and apply. The data volume and EIP survive either way.
The identity is protected by step 1.

1. **Verify an identity backup** (gate; stop if it fails):
   - run `pds-backup-identity` through SSM (README §5);
   - on the host, print `sha256sum /pds/secrets.env`;
   - on the laptop, download the newest `identity-<stamp>.enc.tar`, restore it with
     `pds-restore-identity.sh` and the keychain key, and compare `sha256sum restored/secrets.env`;
   - the two hashes must be equal.
2. **Pre-flight on the current host:**
   - record `docker compose -f /pds/compose.yaml ps`, `curl -fsS https://openlore.jeffbailey.us/xrpc/_health`,
     and `com.atproto.repo.describeRepo` for `jeff.openlore.jeffbailey.us`;
   - confirm no container needs IMDS: `grep -i aws /pds/pds.env` returns nothing (the
     blobstore is disk). This is the evidence for M-2.
3. Bump both roots to `v1.7.0`. Apply the bootstrap root first (only `review-app-iam.tf` is new).
4. `tofu plan -out=tfplan` in prod. Expect exactly: instance **replace**, a metadata-options
   change folded into the replace, the volume attachment replace, and the new `review-app.tf`
   creates **with alarms disabled** (`review_app_alarms_enabled = false`). Run
   `OPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan`. It must refuse nothing protected.
5. Announce the downtime (a few minutes). Then `tofu apply tfplan`.
6. **Verify the PDS:**
   - `_health` returns 200;
   - the TLS subject and issuer are correct for `openlore.jeffbailey.us`;
   - `describeRepo` for `jeff` returns the same DID;
   - `resolveHandle` works;
   - `/pds/caddy/sites` exists and is empty;
   - `docker compose exec caddy caddy validate --config /etc/caddy/Caddyfile` passes;
   - `swapon --show`;
   - IMDS is closed to containers:
     `docker run --rm curlimages/curl -s -m 3 -X PUT http://169.254.169.254/latest/api/token -H 'X-aws-ec2-metadata-token-ttl-seconds: 60'`
     must fail or time out, while the same PUT from the host shell succeeds.
7. If the app was already deployed before this replacement, run **R-DEPLOY `redeploy`** (§7.2).
   That reinstalls the host units and starts the app at the recorded digest. Then verify the app.
8. `tofu plan` reports "No changes."

### 7.2 R-DEPLOY: deploy a new app version (Recreate)

`deploy/review-app/deploy.sh deploy <git-sha>` (and `install` on first use, which also writes
the Caddy site and reloads Caddy):

Rollback is designed first: the previous digest is in `/pds/app/state/releases`, and the
pre-deploy DuckDB copy exists. If readiness fails, the script rolls back automatically.

1. **Laptop gates:**
   - `gh run list --commit <sha> --workflow ci.yml` shows success, including the image jobs;
   - resolve the digest of `ghcr.io/jeffabailey/openlore-review-app:sha-<sha>`;
   - run `cosign verify <ref>@<digest>` with
     `--certificate-identity-regexp '^https://github.com/jeffabailey/openlore/.github/workflows/ci.yml@refs/heads/main$'`
     and `--certificate-oidc-issuer https://token.actions.githubusercontent.com`.
2. `aws ssm send-command` (`AWS-RunShellScript`). The payload is the `deploy/review-app/host/`
   tarball (base64) plus the digest. It contains no secret. On the host, in order:
   1. install or refresh the host files and systemd units (idempotent), then
      `systemctl enable --now review-app-health.timer`;
   2. `render-secrets.sh`;
   3. `docker pull <digest>` while the old version still serves, to minimize downtime;
   4. `docker compose -f /pds/app/compose.yaml stop review-app`;
   5. copy DuckDB to `review-app.duckdb.pre-<sha12>`;
   6. write `/pds/app/.env`, then `docker compose -f /pds/app/compose.yaml up -d`;
   7. wait up to 90 s for `curl -fsS --resolve app.openlore.jeffbailey.us:443:127.0.0.1 https://app.openlore.jeffbailey.us/readyz`;
   8. on success, append to `releases` and delete the older pre-deploy copy;
   9. **on failure, automatic rollback:** start the previous digest. If it logs
      `schema_version newer than supported`, restore the pre-deploy copy and start again. Exit
      non-zero.
3. **Laptop post-checks:**
   - `GET /healthz` returns 200;
   - `GET /readyz` returns 200;
   - `/oauth/client-metadata.json` parses and its `client_id` equals its URL;
   - the `/oauth/jwks.json` key ids match the expected `kid`.
4. **Smoke** (manual, 2 minutes): sign in with the test account, open the queue, sign out.

Expected downtime is about 5-15 s (container stop, start and probes). During it, Caddy serves the
503 "restarting" page. Running scans become `interrupted` and are resumable (ADR-076).

### 7.3 R-ROLLBACK

- `deploy.sh rollback` redeploys the previous digest from `releases`, with the same steps and
  checks as §7.2.
- If the rolled-back binary refuses to start on a newer schema major, run
  `deploy.sh rollback --restore-db`, which also restores `review-app.duckdb.pre-<sha12>`. Data
  written since the deploy is lost (pending suggestions can be rescanned; declines made since
  then may be re-offered once, which is the accepted KPI-BRA-4b exposure).
- If the app harms the PDS (memory, CPU) and no digest is good: stop the app container
  (`deploy.sh stop`). Caddy then serves the 503 page and the PDS is unaffected.
- **Rollback drill:** run `deploy` to N, then `rollback` to N-1, once during the R-5 soft
  launch. Deploy readiness is not complete until this has passed once.

### 7.4 R-ROTATE: rotate secrets

| Secret | Procedure |
|---|---|
| **Client JWK** (no downtime for users) | 1. Put the current key into `client-jwk-previous` and a new key (new `kid`) into `client-jwk`. 2. `deploy.sh redeploy`. The JWKS now serves both keys and the app signs with the new one. Authorization servers that cached the old JWKS refetch on an unknown `kid`; if any does not, a sign-in fails until its cache expires. 3. After **at least 24 h**, delete `client-jwk-previous` and redeploy. On suspected compromise, skip the overlap: replace the key and redeploy immediately, accepting failed sign-ins for up to the JWKS cache lifetime. Sessions survive, because refresh tokens are DPoP-bound, not client-key-bound; verify this during R-5. |
| **Data key** | 1. Move the current key to `data-key-previous` and put a new key (new `kid`) in `data-key`. 2. Redeploy. At startup the app re-encrypts every `enc_blob` row whose key id is not active, in one transaction (< 50 users means a trivial size), and logs `secrets.rekeyed{rows}`. The probe canary then uses the new key. 3. Confirm `rows_on_previous_key=0` in the startup log, delete `data-key-previous`, and redeploy. |
| **GitHub PAT** | Create a new fine-grained PAT (public read, no permissions), overwrite `github-token`, redeploy, check `/readyz`, then revoke the old PAT on GitHub. This is triggered by alarm A-8, 14 days before expiry. An expired token makes the probe **refuse start**, so do not let it lapse. |
| **Log salt** | Rotate only if a log export leaked. Overwrite and redeploy. |

### 7.5 R-REVOKE: revoke all sessions or disconnect

- **One user (in-app):** US-BRA-012 disconnect. Per the SPIKE-2 finding, the app treats
  revocation 200 or 204 as success and **always** deletes the local tokens and rows. The
  already-issued access token remains valid until it expires (the residual window; document the
  observed lifetime during R-5).
- **All users (incident: data-key or host compromise suspected):**
  1. `deploy.sh stop`.
  2. Rotate the data key with **no** `previous` (old ciphertexts become undecryptable, so every
     OAuth session is dead locally).
  3. Rotate the client JWK with no overlap (refresh grants bound to the client now fail at the
     authorization server).
  4. `deploy.sh redeploy`. On startup the app finds sessions it cannot decrypt; DELIVER makes
     it delete them and the matching `web_sessions` (logged as `sessions.invalidated{count}`).
  5. Users sign in again. Their suggestions and declines are kept.

  Best-effort server-side revocation is impossible without the decrypted tokens. This is
  documented: the access-token lifetime bounds the exposure.
- **Purge a user on request without their session** (operator): there is no hosted admin
  surface. DELIVER provides `deploy.sh purge <did>`, which calls an internal loopback-only admin
  endpoint (§kpi-instrumentation 4) that runs the same `purge(OwnerScope)` transaction.
