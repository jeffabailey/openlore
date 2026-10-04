# OpenLore — Resume Context

## Current Task
Nothing in flight. Shipped 2026-09-30..10-04: `scrape person`; CLI+viewer share the store; OpenLore PDS live at `openlore.jeffbailey.us` (`jeff` account, `deploy/`) on shared module `jeffabailey/tofu-aws-pds` (v1.6.0; the-reality-base uses it too); claims publish signed by `did:plc:pnyxfnpkcldxtitsw64ycahw` (keychain key, `#org.openlore.application` in its PLC doc); daily encrypted identity backup + 2-day alarm on both PDSes.

## Key Decisions
- Confidence on the wire is integer basis points (ADR-070); a stock PDS refuses floats.
- `jeffbailey.us` DNS is Cloudflare (Route 53 records inert); first-boot changes need an instance replace (TRB: from a laptop, CI refuses deletes).
- Backup private keys live in the macOS keychain (`pds-backup`/`openlore`, `the-reality-base`); alarm email subscribed out of band (no address in state).

## Next Steps
- `docs/feature/shared-pds-module`: run DISTILL/finalize (evolution doc) — DESIGN + live delivery done, no acceptance/evolution docs yet.
- Expect backup alarms to go OK after 00:00 UTC 2026-10-05; TRB prod needs `/pds/backup-pubkey.pem` on first deploy.
- Older backlog: GitHub request budget check; KPI/`Confidence::try_new` housekeeping; cargo-mutants `-p cli` baseline (see `docs/evolution/*`).
