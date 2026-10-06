# Wizard decisions — indexer-deployment

- **Problem:** `openlore-indexer` is not deployed anywhere: no image, service, schedule or alert. The per-DID fetch, fault isolation and exit codes (0 ok, 2 config/probe/store failure, 3 every DID skipped) exist but nothing runs them, so network search has no live index.
- **Where:** co-located on the OpenLore PDS host (t4g.micro, 1 GiB), like the review app (ADR-075). It runs as a container from a CI-built, signed arm64 image. Rollout depends on the planned tofu-aws-pds v1.7.0 instance replacement (Caddy sites mount, IMDS hop limit 1).
- **Schedule:** `openlore-indexer ingest` every 15 minutes via a systemd timer on the host. `openlore-indexer serve` runs as a service behind Caddy, serving the search XRPC API, so `openlore search` has a live endpoint.
- **Alert:** CloudWatch alarm to the existing SNS email when 2 consecutive passes exit 3 (total outage). Keep alarm cost minimal: the review app uses 4 alarms at about $2/month.
- **Config:** OPENLORE_INDEXER_REPO_DIDS (bare did:plc/did:web), optional OPENLORE_INDEXER_SOURCE_URL fallback, OPENLORE_INDEXER_PLC_ENDPOINT (default plc.directory), concurrency 4, 30 s per DID. The index store (`index.duckdb`) lives on the data volume.
- **Classification:** infrastructure-heavy, cross-cutting; brownfield (reuses deploy/review-app patterns, CI image pipeline, gated deploy.sh).
- **Process:** new nWave feature: light DISCUSS, then DESIGN and DEVOPS, then DISTILL and DELIVER, with gated pushes.
- **User decisions (2026-10-06):**
  - The search API is PUBLIC and read-only at `index.openlore.jeffbailey.us`, behind Caddy. The existing wildcard DNS covers it.
  - The DID list is a STATIC SSM parameter (OPENLORE_INDEXER_REPO_DIDS) the operator edits, picked up on the next pass.
  - Alerts: 2 consecutive exit-3 passes, AND any single exit 2.
- **Open for DESIGN/DEVOPS:** memory budget alongside the PDS and review app on 1 GiB (t4g.small fallback); how the 15-minute timer reads the SSM DID list without giving the container AWS credentials (host writes it to a read-only file, as the review app does); whether serve and ingest share one container image.
