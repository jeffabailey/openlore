# OpenLore — Resume Context

## Current Task
Nothing in flight. Five deliveries done: `bluesky-claim-review-app`, `indexer-per-did-pds-fetch`, `fix-indexer-follow-ups`, `indexer-deployment`, `fix-indexer-deployment-follow-ups`. None deployed yet. History: `docs/evolution/`.

## Key Decisions
- One `serve` container owns `index.duckdb`; a 15-min host timer runs `trigger` (ADR-080).
- A3 `not_live` counts a search 500 (not 503/429/408); store errors log `indexer.search.store_error`.
- Stay on t4g.micro with tight caps (indexer 128m, review app 192m, DuckDB 48 MB / 1 thread).

## Next Steps
- Go-live operator sequence (`deploy/indexer/README.md`): review app + indexer share the one tofu-aws-pds v1.7.0 replacement.
- Batched live `canzantest` sign-in walkthrough.
- Queued `/nw:refactor` cleanups: drop `send_idempotent` retry, stale SCAFFOLD headers, shared DuckDB cap parsing.
