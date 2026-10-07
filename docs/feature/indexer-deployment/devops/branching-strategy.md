# Branching Strategy: indexer-deployment (DEVOPS)

Unchanged from the project standard (see `docs/feature/bluesky-claim-review-app/devops/branching-strategy.md`).

| Aspect | Rule |
|---|---|
| Model | Trunk-based: commit to `main`, no pull requests, never force-push `main` |
| Integration | Every push runs the full `ci.yml`; green `main` commits get a signed `openlore-indexer` image (`sha-<40>`), so every green sha is deployable |
| Deploy trigger | Human, laptop: `deploy/indexer/deploy.sh deploy <sha>`; refuses non-green CI or an unverified signature |
| Infra changes | `deploy/**` committed to `main`, validated by `deploy-pds-check.yml` (+ `indexer-host` job); applied from the laptop against a saved plan gated by `check-plan.sh` |
| Incomplete work | Configuration, not branches: the indexer is not deployed until R-REPLACE is done and not announced until the readiness checklist passes (monitoring-alerting §4). New binary behaviour is opt-in by env (`CONTROL_SOCKET`, `PURGE_UNLISTED`, `REPO_DIDS_FILE`), so `main` stays releasable throughout DELIVER. |
| Same-commit rule | The first commit that adds `deploy/indexer/host/compose.yaml` also adds the mount/posture guard; the first image job lands with its serve smoke (or is marked not deployable) |
| Fixing a bad push | Fix forward or `git revert`; for production, `deploy.sh rollback` first |

Local gates: existing `.githooks/pre-commit` (`check-probes`); the optional pre-push hook gains
`shellcheck` and the indexer mount guard script when `deploy/indexer/**` changed (one script,
two callers: CI and the hook). Clippy and nextest stay CI-only (slow host).

Mutation testing: per-feature (project CLAUDE.md), ≥ 80% kill rate on modified files during DELIVER.
