# OpenLore — Resume Context

## Current Task
Nothing in flight. Six deliveries done (latest `fix-go-live-runbook-gaps`), none deployed; history in `docs/evolution/`. 2026-10-10/11: CI housekeeping — `nightly.yml` now runs weekly (Mon 08:00 UTC), all green, claim-domain mutation kill rate 98.6%.

## Key Decisions
- One go-live checklist for both apps in `deploy/README.md`; both `deploy.sh` fail closed; `go_live_runbook_drift` keeps runbooks honest.
- CI: acceptance job frees runner disk + slims dep debuginfo (CI-only); mutants step runs under `ulimit -v` 6 GiB so runaway-loop mutants abort, not kill the runner.
- Live smoke reads protected-resource metadata from the test account's PDS (bsky.social 404s it). 2 surviving mutants (`base64url_decode` `|`→`^`, `arb_nfc_equivalent_pair`) left by choice.

## Next Steps
- Release tofu-aws-pds v1.7.0 (operator, other repo), then the `deploy/README.md` go-live checklist.
- Batched live `canzantest` sign-in walkthrough.
- Backlog: pass-precedence test seam, check-arch rule for event fields, clippy `double_must_use`.
