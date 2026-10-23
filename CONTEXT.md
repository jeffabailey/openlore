# OpenLore — Resume Context

## Current Task
owner-only-signing delivered and finalized (`docs/evolution/owner-only-signing-evolution.md`): pushed (fed4495, CI + publish contract green) and review app deployed 2026-10-22 (sha256:e5c5a143…, commit 3532e13). Follow-ups D30 (publish push gated, 5ada65b), D31, D32, D34 done after finalize. All 25 steps pass DES integrity; review revised once; mutation 100% (176/176).

## Key Decisions
- Only an account's owner (verified link) or a DID listed in a repo's `.github/openlore-authority` (or the repo owner) can sign about it; anyone can import unsigned candidates (ADR-086/087). Readers label and don't weigh third-party / unchecked claims.
- Withdrawn claims keep their weight and footer place (ADR-022/WD-85); the reviewer's premise was rejected.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`.

## Next Steps
- Owner-only-signing leftovers all closed (D13, D35 4757ebb, README/cli.sh examples 89da132); review app needs no redeploy for them (CLI only).
- New flaky under load: indexer_deployment_did_list `a_purge_that_fails_ends_the_pass_with_exit_2...` (passed at -j 2).
- Older backlog in memory `openlore-follow-up-backlog.md` (per-DID indexer fetch, release/Homebrew hygiene).
