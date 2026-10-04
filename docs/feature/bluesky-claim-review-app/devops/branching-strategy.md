# Branching Strategy: bluesky-claim-review-app (DEVOPS)

## Model: trunk-based, direct commits to `main`, no pull requests

This is the user's standing decision; this feature does not change it.

| Aspect | Rule |
|---|---|
| Branches | `main` only. Short-lived local branches are allowed but never pushed as PRs. |
| Integration | Every push to `main` runs the full `ci.yml`: commit stage, then the acceptance stage, then the review-app image jobs. The user's host is slow, so CI is the authoritative full-suite runner. |
| Releasable main | Every green `main` commit produces a signed, scanned image `sha-<40>`, so any green sha is deployable. |
| Deploy trigger | A human runs `deploy/review-app/deploy.sh deploy <sha>` from the laptop. It refuses a sha whose `ci.yml` run is not green or whose image signature does not verify. |
| Infra changes | `deploy/**` is committed to main. `deploy-pds-check.yml` validates it. It is applied from the laptop against a saved plan gated by `check-plan.sh`. |
| Incomplete work | Feature-flag by configuration, not by branch. The app is not announced until the production-readiness checklist passes (monitoring-alerting §3). Routes for unfinished stories return 404 behind a config flag (`REVIEW_APP_FEATURES`), which DELIVER decides. |
| Tags | `v*` tags remain the CLI release trigger (`release.yml`). The app does not need tags; its identity is the commit sha plus the image digest. |
| Fixing a bad push | Push a fix forward, or `git revert` on main. Never force-push main. |

## Pipeline triggers

| Workflow | Trigger | Gate type |
|---|---|---|
| `ci.yml` (+ `review-app-build`, `review-app-image`) | `push: [main]`. The existing `pull_request:` trigger is unused but harmless. | Blocking. The image jobs only run on a green test. |
| `deploy-pds-check.yml` | `push: [main]` with `paths: deploy/**`, the workflow file itself, and **`deploy/review-app/**`** | Blocking (red main) |
| `nightly.yml` | cron `0 8 * * *`, `workflow_dispatch` | Advisory (mutants; live smoke opens an issue) |
| `release.yml` | `tags: v*` | Blocking for the CLI release; unaffected |

There is no branch protection that requires PRs. `release.yml`'s `bump-formula` pushes to main
with `GITHUB_TOKEN`, which already relies on that. The safety net is the automated gates plus the
laptop deploy's "CI green and signature verified" precondition.

## Local quality gates (mirror the commit stage)

| Hook | Checks | Status |
|---|---|---|
| `pre-commit` (`.githooks/pre-commit`) | `scripts/check-probes.sh` | Existing. Opt-in with `git config core.hooksPath .githooks`. |
| `pre-push` (proposed, opt-in) | `cargo fmt --check`, `cargo run -p xtask -- check-arch`, `cargo deny check`; when `deploy/review-app/**` changed, also `shellcheck` and the compose mount guard script from `deploy-pds-check.yml` | **Proposed.** Clippy and nextest are deliberately left out, because the host is slow and CI runs them. |

The mount guard and the log-field allowlist are scripts that CI and the hook share, so
"mirror, not duplicate" holds: one script, two callers.

## Mutation testing

Per-feature, as already written to the project CLAUDE.md: during DELIVER, `cargo mutants` scoped
to the modified files of `review-domain`, `adapter-review-store` and `adapter-atproto-oauth`,
with a kill-rate gate of >= 80%. The nightly `claim-domain` advisory job is unchanged.
