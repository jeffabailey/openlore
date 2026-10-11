# OpenLore — Resume Context

## Current Task
Nothing in flight. Seven deliveries done (latest `fix-pass-failure-precedence`, 2026-10-11: first failure ends the pass and outranks the deadline override; committed locally, push held), none deployed; history in `docs/evolution/`. CI: `nightly.yml` weekly (Mon 08:00 UTC), claim-domain mutation kill rate 98.6%.

## Key Decisions
- One go-live checklist for both apps in `deploy/README.md`; both `deploy.sh` fail closed; `go_live_runbook_drift` keeps runbooks honest.
- CI: acceptance job frees runner disk + slims dep debuginfo (CI-only); mutants step runs under `ulimit -v` 6 GiB so runaway-loop mutants abort, not kill the runner.
- Live smoke reads protected-resource metadata from the test account's PDS (bsky.social 404s it). 2 surviving mutants (`base64url_decode` `|`→`^`, `arb_nfc_equivalent_pair`) left by choice.

## Next Steps
- Operator go-live: follow the `deploy/README.md` checklist (tofu-aws-pds v1.7.0 is released and both roots pin it, 403d819).
- Batched live `canzantest` sign-in walkthrough.
- Backlog: check-arch rule for event fields, clippy `double_must_use`.
