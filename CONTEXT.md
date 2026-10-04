# OpenLore — Resume Context

## Current Task
`bluesky-claim-review-app` (J-009). Users sign in with Bluesky and prove GitHub ownership with their DID in the GitHub bio. They review private claim suggestions, and approving one writes a self-attested claim to their own PDS. DISCUSS, DESIGN (ADR-071..076), spikes and DEVOPS are committed (latest d32c401). DISTILL is running and its output is not committed yet. Next: commit DISTILL, start `/nw:deliver`, and push over SSH.

## Key Decisions
- Approved claims rely on the PDS repo signature, not an OpenLore signature (ADR-071). Pending and rejected suggestions are private and never go to a PDS. The OAuth scopes allow create only.
- The app runs as a container on the PDS host at `app.openlore.jeffbailey.us`. tofu-aws-pds v1.7.0 adds the Caddy sites mount and IMDS hop limit 1, and goes out in one planned instance replacement. There are 4 alarms, logs are kept 30 days, and there is no backup of private state.
- The spikes pass on bsky.social. atrium-oauth 0.1.7 has three problems: revoke expects 204 but servers send 200, a failed code exchange in the callback panics through `todo!()`, and hickory must be at 0.26 or later.

## Next Steps
- During DELIVER, go from the walking skeleton to R1 and R2 without asking. Demo each slice against fakes. Do all live canzantest sign-ins in one batch at the end.
- Operator work: SPIKE-1/3 on the OpenLore PDS (`jeff` app password), SSM secrets, the instance replacement, making the GHCR package public, then deploy (`devops/`).
- The backlog in memory includes the indexer bug: `listRecords` is called without `repo`, at `adapter-atproto-ingest/src/lib.rs:97`.
