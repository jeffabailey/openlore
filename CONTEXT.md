# OpenLore — Resume Context

## Current Task
Nothing in flight. `bluesky-claim-review-app`, `indexer-per-did-pds-fetch` and `fix-indexer-follow-ups` are all delivered. History: `docs/evolution/`.

## Key Decisions
- The fallback gets a fresh per-DID budget (ADR-078 §4 amended).
- References deduplicated, one transaction per indexed claim.
- A blank `OPENLORE_INDEXER_PLC_ENDPOINT` is refused at startup (unset = default).

## Next Steps
- Review-app go-live operator steps (tofu-aws-pds v1.7.0, SSM, PAT, instance replace, GHCR public, deploy, REVIEW_APP_LIVE, alarms), then the batched live `canzantest` sign-in walkthrough.
- Schedule `openlore-indexer ingest` and alert on repeated exit 3.
- Optional: share the duplicated guarded DNS resolver.
