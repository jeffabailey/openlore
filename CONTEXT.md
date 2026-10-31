# OpenLore — Resume Context

## Current Task
scale-to-zero-hosting (indexer on Lambda, ADR-090): SPIKE, DESIGN, DEVOPS, DISTILL done; 132 scenarios, 137 RED, all four final reviewers conditionally approved after two revision cycles. DELIVER not started. No AWS resources exist yet.

## Key Decisions
- Native Lambda root plus S3/SSM adapter; self-host binary stays AWS-free; one signed image; us-east-1; 15-minute pass; no-publish pre-flip gate; global cap, alarms and a kill switch replace per-client 429s (ADR-090).
- Adds about $0.72/month recurring; host bill unchanged until the PDS and review app move (bring-your-own PDS and the review app on Lambda are still open SPIKE-6 halves).
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app|indexer deploy <40-hex sha>`.

## Next Steps
- `/nw-deliver scale-to-zero-hosting`: start with UI-7 (drop cli's dev-dependency on openlore-indexer before the lib split). The action list is in feature-delta.md's DISTILL handoff.
- RUSTSEC-2026-0387 ignored in deny.toml until the 2026-11-24 recheck. DuckDB capped below 1.20000 (2.0 is a separate change).
- Possible flake: indexer_ingest `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam`. Backlog in memory `openlore-follow-up-backlog.md`.
