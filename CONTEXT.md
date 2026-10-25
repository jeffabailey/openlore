# OpenLore — Resume Context

## Current Task
owner-only-signing delivered and finalized (`docs/evolution/owner-only-signing-evolution.md`): pushed (fed4495, CI + publish contract green) and review app deployed 2026-10-22 (sha256:e5c5a143…, commit 3532e13). Follow-ups D30 (publish push gated, 5ada65b), D31, D32, D34 done after finalize. All 25 steps pass DES integrity; review revised once; mutation 100% (176/176).

## Key Decisions
- Only an account's owner (verified link) or a DID listed in a repo's `.github/openlore-authority` (or the repo owner) can sign about it; anyone can import unsigned candidates (ADR-086/087). Readers label and don't weigh third-party / unchecked claims.
- Withdrawn claims keep their weight and footer place (ADR-022/WD-85); the reviewer's premise was rejected.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`.

## Next Steps
- 491b7ae (duckdb capped <1.20000: DuckDB 2.0 broke the arm64 release link) is CI-green and deployed: indexer sha256:df8a32e9… on 2026-10-25 (first pass exit 0), review app redeployed the same day. Moving to DuckDB 2.0 is a separate change.
- RUSTSEC-2026-0387 ignored in deny.toml until 2026-11-24 recheck (crypto-bigint via atrium-oauth/p256 0.13). Search N+1 fix verified on CI (184d146).
- Possible flake: indexer_ingest `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam`. Backlog in memory `openlore-follow-up-backlog.md`.
