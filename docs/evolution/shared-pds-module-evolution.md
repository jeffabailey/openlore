# Evolution: shared-pds-module

- **Type**: platform feature (`/nw:new` → DESIGN → delivery in-session)
- **Dates**: 2026-10-02 (design) to 2026-10-04 (backups and alarm live)
- **Repos**: this repo (`deploy/`, CLI), [`jeffabailey/tofu-aws-pds`](https://github.com/jeffabailey/tofu-aws-pds)
  (v1.0.0 → v1.6.0), and `Save-The-Republic/the-reality-base` (TRB)
- **Design**: [`feature-delta.md`](../feature/shared-pds-module/feature-delta.md),
  ADR-066 to ADR-069, ADR-070 (added in delivery), brief §"Platform Architecture (PDS hosting)"
- **Process note**: delivered directly against the DESIGN, without a DES roadmap. There is no
  `deliver/execution-log.json`; the record is the commits below, the module repo's tags and
  CHANGELOG, and the live checks listed under "Verified".

## Goal

Run an ATProto PDS for OpenLore like the one in `the-reality-base/deploy`, with the PDS
infrastructure in a module both projects share, at the lowest cost.

## What shipped

**Shared module** (`jeffabailey/tofu-aws-pds`, public, `v*` tags locked by a ruleset):

| Tag | Change |
|---|---|
| v1.0.0 | Extracted TRB's `modules/pds` with its history; generic inputs `name_prefix`, `project`, `require_namespace_matches_hostname`; `modules/pds-bootstrap` (per-account foundation) |
| v1.1.0 | `swap_mb` (swap file on the root volume) |
| v1.2.0 / v1.2.1 | `bootstrap_account`: the host creates the first account and an app password, stored in SSM; `user_data` gzipped only when it exceeds 16 KiB |
| v1.3.0 | `verification_methods`: the host adds keys to the account's did:plc document, signed with the PDS rotation key |
| v1.4.0 / v1.4.1 | Identity backup made hybrid (AES-256-CBC + HMAC-SHA256, key wrapped with RSA-OAEP) plus `scripts/pds-restore-identity.sh`; CI lint fix |
| v1.5.0 | `backup_on_calendar`: a systemd timer for the backup |
| v1.6.0 | `backup_alarm`: a CloudWatch alarm after two UTC days with no successful backup, to SNS |

Every optional input defaults to off, and a golden `user_data` hash test pins what TRB gets.
Each CHANGELOG entry lists the addresses it changes. Every release so far: none.

**OpenLore** (`deploy/`, AWS account 091153021562, profile `jeff`):

- Bootstrap root and prod root on the shared module. t4g.micro with 1 GiB swap, 5 GB data
  volume, EIP: about $10.84/month, plus about $0.40/month for the backup metric and alarm.
- `openlore.jeffbailey.us` is live. The account `jeff.openlore.jeffbailey.us` is
  `did:plc:pnyxfnpkcldxtitsw64ycahw`.
- Claims publish there, signed by that DID. `#org.openlore.application` is in its PLC
  document; the key is from `openlore key` and is held in the macOS keychain.
- Daily encrypted identity backup, plus an alarm emailed through SNS (subscribed out of band).

**TRB**: both roots moved onto the shared module with a no-op plan (test live, prod
undeployed). The local module and `check-user-data.sh` were deleted. TRB is now on v1.6.0 with
the daily backup and the alarm; its bootstrap IAM gained the metric and alarm grants.

## Key decisions

- Separate host per project, each in its own account (ADR-067), rather than several PDSes on
  one host or one PDS with accounts for both projects.
- Module in a standalone public repo consumed by git tag (ADR-066). The call stays
  `module "pds"`, so TRB moved with no `moved` blocks and no resource address change (ADR-069).
- Laptop apply against a saved plan, with a delete gate (`deploy/check-plan.sh`). OpenLore's CI
  is credential-free (ADR-068).
- Confidence on the wire is integer basis points (ADR-070). A stock PDS refuses floats, and the
  basis-point grid keeps the CID stable from publish through pull.
- Secrets never pass through OpenTofu. Account passwords are generated on the host and go to
  SSM. Backup private keys stay in the operator's keychain. The alarm email is subscribed out
  of band (TRB's C-P5).

## Verified (live)

- TRB test planned 0 changes after the source swap, in CI and locally.
- A claim published from the CLI and read back from the PDS recomputes to the CID it is
  stored under.
- `peer pull` from another identity reported `verified 1/2` against the DID document on
  plc.directory. The other record was a deliberate self-attribution rejection.
- Backups on both hosts were restored from S3 with the keychain key, and the restored
  `secrets.env` matched the host's sha256.
- Both timers are enabled, and a service-run backup published the metric.
- Alarm email delivery was confirmed by the user, after an OK → ALARM test.

## Issues found along the way

- **Application gaps found only against a real PDS.** The test fakes masked each of these:
  - Publish sent an unauthenticated, non-lexicon body.
  - The PDS rejected float `confidence`.
  - Peer resolution called `plc.directory/xrpc/...resolveDid`, which does not exist.
  - `peer pull` read only `hex:` keys, and took the rkey from the repo record CID.
  - `keyring` had no backend enabled, so it used an in-memory mock and keys never persisted.

  All are fixed and covered by unit tests. **Lesson:** the fakes encoded our assumptions
  about ATProto, not the protocol. Run a live round trip before calling federation done.
- **DNS.** `jeffbailey.us` is served by Cloudflare. The Route 53 zone the design assumed is not
  delegated, so the OpenTofu A records are inert. The live records were added in Cloudflare
  by hand (`deploy/README.md`).
- **Caddy backoff.** Caddy backed off on ACME after the first NXDOMAIN; a restart fixed it.
- **First-boot changes require replacement.** Every first-boot input needs an instance
  replacement, because cloud-init does not run again. TRB's CI apply refuses any delete
  (C-P7), so TRB replacements are a laptop action.
- **CI misses.**
  - Module CI was red from v1.3.0 to v1.4.0 on a shellcheck SC2015 that the local, older
    shellcheck did not report; I did not check CI after the v1.3.0 push.
  - A new `undici` advisory and two new RustSEC advisories broke audits unrelated to this
    work. They were fixed by dependency bumps.
- **Alarm test.** `set-alarm-state ALARM` on an alarm already in ALARM sends nothing. Test
  with OK → ALARM. A daily-period alarm evaluates complete UTC days, so a new alarm reads
  ALARM until a whole day with a backup has passed.

## Open follow-ups

- TRB prod needs `/pds/backup-pubkey.pem` placed on its first deploy.
- Optional: delegate `openlore.jeffbailey.us` to Route 53 (about $0.50/month) so DNS is in
  OpenTofu. Optional: move TRB's bootstrap onto `modules/pds-bootstrap` (ADR-069 phase 2) and
  its local state into S3.
- The backup alarms should report OK after 00:00 UTC on 2026-10-05.
