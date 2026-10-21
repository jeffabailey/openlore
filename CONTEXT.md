# OpenLore — Resume Context

## Current Task
owner-only-signing delivered and finalized (`docs/evolution/owner-only-signing-evolution.md`): main fast-forwarded to 5316d62, NOT pushed (102 commits ahead of origin), not deployed. All 25 steps pass DES integrity; review revised once; mutation 100% (176/176).

## Key Decisions
- Only an account's owner (verified link) or a DID listed in a repo's `.github/openlore-authority` (or the repo owner) can sign about it; anyone can import unsigned candidates (ADR-086/087). Readers label and don't weigh third-party / unchecked claims.
- Withdrawn claims keep their weight and footer place (ADR-022/WD-85); the reviewer's premise was rejected.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`.

## Next Steps
- User decision: should `openlore publish push` need a SigningPermit (U-OOS-D30 in docs/feature/owner-only-signing/deliver/upstream-issues.md)? Other deferred: D31, D32, D34, D35.
- On "push": push main, watch CI, then deploy the review app (store goes v8/v9 via migration).
- Remove worktree ../openlore-oos (branch oos-deliver) once pushed; older backlog in memory `openlore-follow-up-backlog.md`.
