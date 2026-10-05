# OpenLore — Resume Context

## Current Task
`bluesky-claim-review-app` (J-009/J-010) is delivered and finalized: 15 steps, mutation gate passed, CI green, signed GHCR image. It is not deployed yet. History and follow-ups: `docs/evolution/bluesky-claim-review-app-evolution.md`.

## Key Decisions
- Approved claims are self-attested and rely on the PDS repo signature (ADR-071). Pending suggestions and declines stay private.
- The app is co-located on the PDS host at `app.openlore.jeffbailey.us`, adopted in one planned replacement (tofu-aws-pds v1.7.0, IMDS hop limit 1). There are 4 alarms, logs are kept 30 days, and private state has no backup.
- Pushes are gated on green CI. After any schema or shared-helper change, run the dependent suites too.

## Next Steps
- Operator go-live: release tofu-aws-pds v1.7.0 and bump the ref, SSM parameters and PAT, the replacement, GHCR public, deploy, `REVIEW_APP_LIVE`, SPIKE-1/3 on the OpenLore PDS, RSS re-measure, rollback drill, then enable the alarms.
- Do the batched live `canzantest` sign-in walkthrough on the live origin.
- Decide on the indexer follow-up: fetch per DID from its resolved PDS, since claims on bsky.social are refused today as relay-origin.
