# OpenLore — Resume Context

## Current Task
Nothing in flight. Six deliveries done: `bluesky-claim-review-app`, `indexer-per-did-pds-fetch`, `fix-indexer-follow-ups`, `indexer-deployment`, `fix-indexer-deployment-follow-ups`, `fix-go-live-runbook-gaps`. None deployed yet. History: `docs/evolution/`.

## Key Decisions
- One go-live checklist for both apps in `deploy/README.md`; `go_live_runbook_drift` keeps runbooks honest.
- Both `deploy.sh` fail closed (IMDS probe, Caddy sites mount/import, adapt + validate before reload).
- `OPENLORE_REVIEW_SCAN_CONCURRENCY` (default 2, 1..=4) is the memory-gate fail-path lever.

## Next Steps
- Release tofu-aws-pds v1.7.0 (operator, other repo), then the `deploy/README.md` go-live checklist.
- Batched live `canzantest` sign-in walkthrough.
- Backlog: pass-precedence test seam, check-arch rule for event fields, clippy `double_must_use`.
