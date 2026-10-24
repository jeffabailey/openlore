# OpenLore — Resume Context

## Current Task
owner-only-signing delivered and finalized (`docs/evolution/owner-only-signing-evolution.md`): pushed (fed4495, CI + publish contract green) and review app deployed 2026-10-22 (sha256:e5c5a143…, commit 3532e13). Follow-ups D30 (publish push gated, 5ada65b), D31, D32, D34 done after finalize. All 25 steps pass DES integrity; review revised once; mutation 100% (176/176).

## Key Decisions
- Only an account's owner (verified link) or a DID listed in a repo's `.github/openlore-authority` (or the repo owner) can sign about it; anyone can import unsigned candidates (ADR-086/087). Readers label and don't weigh third-party / unchecked claims.
- Withdrawn claims keep their weight and footer place (ADR-022/WD-85); the reviewer's premise was rejected.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`.

## Next Steps
- Local, unpushed: e540f54 (ci cyclonedx pin), 2c057be..7a0cd92 (fix-search-references-n-plus-1: batched references read, AS-53 index_busy). Push when told; then confirm CI green.
- Possible new flake: indexer_ingest `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam` failed once in 0.5 s (passed at -j 2). Check new indexer ATs with OPENLORE_TEST_COLLECTOR_LAG_MS=150.
- Backlog in memory `openlore-follow-up-backlog.md` (GitHub request budget; bump-formula unverified until next GA tag; SBOM is cli-only).
