# OpenLore — Resume Context

## Current Task
scale-to-zero-hosting (indexer on Lambda, ADR-090): DELIVER finalized 2026-10-31, 49 of 49 steps, green locally. Not pushed, so CI has not seen it. Nothing deployed, and nothing has run against real AWS. Record: `docs/evolution/scale-to-zero-hosting-evolution.md`.

## Key Decisions
- Native Lambda root plus S3/SSM adapter; self-host binary stays AWS-free; one signed image; us-east-1; no-publish pre-flip gate; global cap, alarms and a kill switch replace per-client 429s (ADR-090, still Proposed).
- Design estimate about $0.72/month recurring; host bill unchanged until the PDS and review app move.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app|indexer deploy <40-hex sha>`.

## Next Steps
- User decisions: 05-01 probe not_live, 08-08 cold through the edge, the x-amzn refused-header path, how to produce a 140 MB index.
- After a push, the first CI run is the final proof. Then the operator stages in `deploy/indexer/README.md`; evidence goes in `docs/evolution/scale-to-zero-hosting-evidence.md`.
- Fix-later list is in the evolution doc. RUSTSEC-2026-0387 recheck 2026-11-24. Flake `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam` still open.
