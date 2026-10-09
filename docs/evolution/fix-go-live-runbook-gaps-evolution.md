# Evolution: fix-go-live-runbook-gaps

- **Type**: bugfix delivery (3 steps, one phase), brownfield deploy scripts, runbooks and one
  review-app setting
- **Dates**: 2026-10-09: RCA and roadmap (`f2ba683`) through the last review fix (`8972c86`)
- **Origin**: a full-model, read-only go-live readiness review (2026-10-09) of the review app and
  the indexer together. Verdict **NOT-READY**: 2 blockers (B1, B2), 7 should-fix (S3–S9), 4
  nice-to-haves (N1–N4).
- **Records**: `docs/feature/fix-go-live-runbook-gaps/` (`rca.md` with the root cause, findings and
  user decisions, `deliver/roadmap.json`, `deliver/execution-log.json`,
  `deliver/mutation/mutation-report.md`).
- **State**: code complete, **not deployed**. No new crate, no schema change, no new alarm. One new
  setting (`OPENLORE_REVIEW_SCAN_CONCURRENCY`). Go-live is blocked on B1 (operator, other repo).

## Summary

Each feature had written its own operator runbook. Nobody merged them or ran them end to end, and
no test compared them with the code. They had drifted from the scripts and from each other: a plain
`tofu plan` that would not re-run user-data, three go-live orders, a test-fire route that does not
exist, a memory gate that could not be run, and a review-app install that did not fail closed.
`deploy/README.md` now holds **one go-live checklist for both apps**, and every other runbook points
to it. Both `deploy.sh` scripts share one fail-closed isolation block. The scan concurrency is a
validated setting. The xtask test `go_live_runbook_drift` fails when a runbook names a route, mode or
setting the code does not have, or a replacement plan without `-replace=`.

## Root cause (shared)

Per-feature runbooks (`docs/feature/bluesky-claim-review-app/devops/*`, `deploy/README.md`,
`deploy/indexer/README.md`, the indexer-deployment evolution doc) were reviewed one feature at a
time and never as one sequence. Nothing checked that a runbook command matched the scripts, routes
and settings it named.

## Findings, fixes and commits

| Finding | Fix | Commit |
|---|---|---|
| **B1**: tofu-aws-pds v1.7.0 not written (roots pin `v1.6.0`) | Out of scope: operator, other repo. The checklist makes it **step 0**, gated on `git ls-remote --tags --refs` showing exactly `v1.7.0` before the ref bump. | `23a23b6`, `db852e8` |
| **B2**: plain `tofu plan -out=tfplan` is an in-place stop/start; user-data never re-runs | `tofu plan -replace=module.pds.aws_instance.pds -out=tfplan`. The drift test fails on any replacement plan without `-replace=`. | `23a23b6` |
| **S3**: three go-live sequences disagreed (alarm enable vs test-fire order, duplicate applies) | One ordered checklist in `deploy/README.md`: enable the alarms, then test-fire. 15 docs point to it. | `23a23b6` |
| **S4**: `POST /admin/test-alarm` does not exist | A-7 / A-8 test-fire injects `guardrail.breach` / `github.token.expiring` via `aws logs put-log-events` into a `test-fire` stream, with exact commands. | `23a23b6` |
| **S5**: the memory gate could not be run | Real sampling loop on `memory.peak`, thresholds from platform-architecture §6 (indexer ≤100 MB, review app ≤128 MB, MemAvailable >128 MB), a defined load subject and a rate under the 10/s burst-50 limit. | `23a23b6`, `db852e8` |
| **S6**: "lower the scan concurrency to 1" had no setting | `OPENLORE_REVIEW_SCAN_CONCURRENCY`, default 2, accepts 1..=4, refuses anything else at startup naming the variable. The pure budget takes it as input; compose and docs wired. | `57e0d19`, `8972c86` |
| **S7**: the review-app install did not fail closed and created `/pds/caddy/sites` itself | Identical block in both `deploy.sh`: digest-pinned IMDS probe (exit 7/28 only); the PDS Caddy must mount `$SITES` at `/etc/caddy/sites` and import `/etc/caddy/sites/*.caddy` on a live line; `caddy adapt` must show the host and `caddy validate` must pass before reload, else the site is removed. No `mkdir` of sites. Isolation also on `start` / `rollback`. | `457ca01`, `db852e8` |
| **S8**: the R-REPLACE rollback re-pinned v1.6.0 (hop limit 2) with the apps running | Stop both apps (`deploy.sh stop`) first. | `23a23b6` |
| **S9**: doc mismatches (256 MB vs 192m, anonymous-pull claim, missing review-app README) | Docs corrected; `deploy/review-app/README.md` is a short stub pointing to the checklist. | `23a23b6` |
| **N1–N4** | A1 drill commands and the stale-DID-list warning; operator prerequisites (IAM, `gh variable set`, `dig`); `caddy validate` before reload; `INDEXER_READY_WAIT_S` (default 60 s) for the first deploy's certificate issuance. | `457ca01`, `23a23b6` |

Each step also has a `chore: execution log` commit (`d6eebe5`, `4317a39`, `58ec7e2`).

**Regression tests.** `go_live_runbook_drift` failed on the docs as they were: `/admin/test-alarm`
×3, the plain plan, 15 non-existent `deploy.sh` modes and the over-limit burst. The fail-closed
refusals are xtask tests in `indexer_deployment_platform.rs` (probe, mount, import, adapt, deploy /
redeploy leaves the running app untouched, never creates sites, same contract in both scripts). The
setting has an acceptance test (`the_review_app_refuses_a_scan_concurrency_outside_its_range`) and a
wiring test that the configured value reaches the limiter.

## User decisions (2026-10-09)

- One bugfix delivery for B2 + S3–S9, cheap nice-to-haves folded in. B1 stays with the operator.
- S6: add a setting rather than drop the fail-path step.

## Reviews

- **Roadmap (full model)**: NEEDS_REVISION, then approved (`f2ba683`). F1: the sites-import check
  could be fooled by the mount source, a commented-out import, or a site Caddy does not adapt. F2:
  refusal on `deploy` / `redeploy` was not covered.
- **Implementation (full model)**: APPROVED; the test seams (`REVIEW_APP_BASE_DIR`,
  `REVIEW_APP_CADDY_SITES_DIR`, `INDEXER_BASE_DIR`) cannot weaken production. Five lows fixed in
  `db852e8`: exact tag check with `--refs`, pinned probe digest only, isolation on `start` /
  `rollback`, a real load subject, drift truth ignores comments.

## Quality gates

| Gate | Result |
|---|---|
| Roadmap review (full model) | NEEDS_REVISION → approved |
| DES integrity | All 3 steps have complete DES traces |
| Adversarial review (full model) | APPROVED; 5 lows fixed |
| Mutation (in-diff vs `f2ba683`, gate 80%) | 4/4 viable caught (**100%**), 4 unviable (`06d0564`). Its noted gap (configured concurrency → limiter) closed by `8972c86`. Shell and docs covered by xtask tests |
| CI | Green at `f2ba683`; later commits being pushed at finalize |

## Lessons

- **Per-feature runbooks are not a go-live plan.** Each was reviewed alone and none was run end to
  end, so they drifted from the code and from each other. **Lesson:** keep one checklist for the
  host, and a drift test that reads it against the code.
- **Review the whole go-live before going live.** A read-only end-to-end readiness review found two
  blockers that every per-feature review had missed. **Lesson:** run it as a gate before the first
  deploy, across all apps that share the host.
- **A fail-closed check must check what the live thing does.** A directory check or a grep for
  `import` can be fooled; asking Caddy (`adapt`, `validate`) cannot. **Lesson:** verify through the
  component that acts on the config.

## Remaining for go-live (operator)

1. **Step 0**: release tofu-aws-pds **v1.7.0** (M-1 sites dir, mount and import; M-2 hop limit 1 and
   the comment fix; tests, CHANGELOG, tag).
2. Then follow the [Go-live checklist](../../deploy/README.md#go-live-checklist).

Operator decisions, unchanged: t4g.small, I-0 separate vs folded, app go-live order,
`TRUSTED_PROXIES`, DID list contents, PAT expiry, downtime window.

## Open follow-ups (not fixed here)

Backlog unchanged: a pass-precedence test seam; a check-arch rule for indexer event fields; the
local-only clippy `double_must_use`.
