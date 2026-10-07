# OpenLore — Resume Context

## Current Task
Nothing in flight. Four deliveries done: `bluesky-claim-review-app`, `indexer-per-did-pds-fetch`, `fix-indexer-follow-ups`, `indexer-deployment`. None deployed yet. History: `docs/evolution/`.

## Key Decisions
- One `serve` container owns `index.duckdb`; a 15-min host timer runs `trigger` (ADR-080).
- Per-IP rate limit (10/s, burst 50) lives in the indexer binary; ADR-083 §4 reversed.
- Stay on t4g.micro with tight caps (indexer 128m, review app 192m, DuckDB 48 MB / 1 thread).

## Next Steps
- Go-live operator sequence (`deploy/indexer/README.md`): review app + indexer share the one tofu-aws-pds v1.7.0 replacement.
- Batched live `canzantest` sign-in walkthrough.
- Follow-ups in `docs/evolution/*` (indexer-deployment L3-L5, oracles, duplicated cap parsing).
