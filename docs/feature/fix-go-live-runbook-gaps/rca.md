# RCA — go-live runbook gaps (review app + indexer)

Source: full-model read-only go-live readiness review, 2026-10-09. Verdict NOT-READY. The orchestrator spot-checked B2 and S4 against the files. The user chose one bugfix delivery for B2 + S3–S9, with cheap nice-to-haves folded in.

## Root cause (shared)
Each feature wrote its own operator runbook (bluesky-claim-review-app: docs/feature/bluesky-claim-review-app/devops/*, deploy/README.md; indexer-deployment: deploy/indexer/README.md, its evolution doc). They were never merged into one sequence or executed end to end. No test checks that runbook commands match the scripts, routes and settings they name. So the docs drifted from the code and from each other, and some steps can't be carried out as written.

## Out of scope (operator, other repo)
B1: tofu-aws-pds v1.7.0 is not written. The roots pin ?ref=v1.6.0 (deploy/tofu/bootstrap/main.tf:52, environments/prod/main.tf:43). It still owes:
- M-1: /pds/caddy/sites + mount /etc/caddy/sites:ro + `import /etc/caddy/sites/*.caddy`;
- M-2: hop_limit 1 + comment fix;
- tests, CHANGELOG, tag.
The user does this in that repo. The runbook must state it as step 0, with a check (`git ls-remote --tags` shows v1.7.0) before the ref bump.

## Findings to fix
- **B2.** infrastructure-integration.md:262 uses plain `tofu plan -out=tfplan`. The module has no user_data_replace_on_change, so that is an in-place stop/start and user-data never re-runs. Fix: `tofu plan -replace=module.pds.aws_instance.pds -out=tfplan`, as deploy/README.md:142 already does.
- **S3.** Three go-live sequences disagree: deploy/indexer/README.md:86-114, deploy/README.md:240-259 and docs/evolution/indexer-deployment-evolution.md:200-213. They differ on alarm enable vs test-fire order (the correct order is enable first, then test-fire, per R-6 wave-decisions.md:75 and indexer README:235). deploy/README.md:247 also duplicates the applies. Fix: ONE ordered checklist in deploy/README.md covering both apps; the other docs point to it.
- **S4.** The review-app A-7/A-8 test-fire uses `POST /admin/test-alarm` (monitoring-alerting.md:132-133), which does not exist; admin.rs:27-28 has only /admin/kpi and /admin/purge. Fix: inject `guardrail.breach` / `github.token.expiring` lines via `aws logs put-log-events` into a `test-fire` stream (as the indexer A1 drill does), with exact commands.
- **S5.** The memory gate (deploy/indexer/README.md §6) cannot be run:
  - :211 echoes the memory.peak path instead of its value, and there is no 5 s / 20 min sampling loop;
  - :221 "under their limits" is unfalsifiable, because a cgroup cannot exceed its cap. Use the platform-architecture §6 thresholds: indexer ≤100 MB, review app ≤128 MB, MemAvailable >128 MB;
  - search.json is undefined;
  - `seq 100 | xargs -P 10` (:219) from one IP exceeds the 10/s burst-50 limit, so the 429s make "every response 200" fail for the wrong reason.
  Fix: concrete commands, a defined body, and a concurrency/rate under the limit (or 429 counted as expected).
- **S6.** The fail-path step "lower the review-app scan concurrency to 1" (README.md:226) has no setting: ScanLimiter::default() is hard-coded (crates/openlore-review-app/src/wiring.rs:578). USER DECISION: add a setting, `OPENLORE_REVIEW_SCAN_CONCURRENCY`, defaulting to today's value and validated like the caps (refuse 0 and out-of-range at startup). Wire it into compose and the docs.
- **S7.** The review-app install does not fail closed. deploy/review-app/deploy.sh:176-188 has no IMDS check and no check for the Caddy sites import, and it creates /pds/caddy/sites itself (:179), which defeats the indexer's directory check (deploy/indexer/deploy.sh:359). Fix:
  - reuse refuse_unless_isolated (the IMDS probe);
  - refuse unless the Caddy container sees /etc/caddy/sites and the Caddyfile imports it;
  - never mkdir /pds/caddy/sites;
  - run `caddy validate` before reload (nice-to-have N3).
- **S8.** The R-REPLACE rollback (infrastructure-integration.md:241-245) re-pins v1.6.0, which brings back hop limit 2 while the apps run. Fix: stop both apps (`deploy.sh stop`) first.
- **S9.** Doc mismatches:
  - deploy/README.md:220 says 256 MB; compose has 192m;
  - wave-decisions.md:72 claims deploy.sh checks anonymous pull, but it does not (fix the doc, or add the check if cheap);
  - monitoring-alerting.md:137 references a deploy/review-app/README.md that does not exist (create a short runbook stub that points to the consolidated checklist, or fix the reference).

## Nice-to-haves (fold in where cheap)
- N1: exact A1 drill commands (create-log-stream; put-log-events with a ms timestamp); warn that the pass timer stopped for over 2 h makes the DID list stale and fires A3.
- N2: operator prerequisites: IAM (ssm:SendCommand/StartSession, logs:StartQuery/PutLogEvents), `gh variable set REVIEW_APP_LIVE`, and `dig index.<host>` / `dig app.<host>` before install.
- N3: caddy validate before reload (see S7).
- N4: note that the first indexer deploy's 60 s wait_ready includes certificate issuance; make it configurable or extend it for the first deploy.

## Regression protection
Add an xtask doc-drift test that fails when:
- a runbook names an admin route, deploy.sh mode or OPENLORE_* setting that does not exist in the code;
- any replacement plan omits `-replace=`.
Plus xtask tests for the deploy.sh fail-closed refusals and a config test for the new setting.

## Operator decisions (not changed here)
t4g.small, I-0 separate vs folded, app go-live order, TRUSTED_PROXIES narrowing, DID list contents, PAT expiry, the downtime window.
