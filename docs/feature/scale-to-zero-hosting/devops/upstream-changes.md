# Upstream changes from DEVOPS: scale-to-zero-hosting

- **From**: Apex (nw-platform-architect), DEVOPS wave, 2026-10-27.
- **For**: the solution architect (DESIGN owner) and the user, before DISTILL.
- **Status**: U-STZ-1..5 accepted by the user on 2026-10-27; DESIGN sections stand as amended by those rows.
  - **U-STZ-6: accepted** by the architect, 2026-10-30, with the `null` rules in D-STZ-48.
  - **U-STZ-7: accepted with one condition** by the architect, 2026-10-30: both schedules set `input = "{}"`, and `lambda probe` and the deploy's probe gate send `{}`. The DEVOPS schedule row sets no `input`, and the claim that the Scheduler's event is an object is not verified. If it were not an object, every scheduled probe would fail. Applied to the DEVOPS schedule row, step 13 and the `lambda probe` verb on 2026-10-30.
  - **U-STZ-8: accepted** by the architect, 2026-10-30.
  - **U-STZ-9: accepted** (revision cycle 2, 2026-10-30); D-STZ-25 updated to keep 10 images with the eviction guard.
- **Scope**: none of these changes the architecture's shape (three functions from one signed
  digest, one S3 object, one writer, alias blue-green, CloudFront without OAC). Each one changes a
  contract or the rollout story that DESIGN wrote, because of how Lambda, CloudFront or EventBridge
  actually behave. The DEVOPS design in `../feature-delta.md` already assumes the proposed change;
  if the architect rejects one, DEVOPS must be revised.

| ID | DESIGN said | Infrastructure fact | Proposed change | Affects |
|---|---|---|---|---|
| U-STZ-1 | D-STZ-31: the host indexer keeps running for 7 days after cut-over; rollback is deleting the CNAME. D-STZ-32: at cut-over Lambda logs to `/openlore/prod/indexer`. | Both together put two `pass_summary` producers in one log group, which breaks A1's per-quarter-hour Minimum (the reason D-STZ-31 gave for a separate trial group). The host health timer probes its own Caddy over loopback and would keep feeding A3 regardless of DNS. | At cut-over the host indexer goes to **standby**: pass timer, health timer and container stopped; files, units, Caddy site and releases kept. New host verbs `deploy.sh standby --timers`, `deploy.sh standby --container` and `deploy.sh resume`. Rollback to the host during the soak becomes: Lambda pass and probe off, host `resume` and `trigger`, delete the CNAME, repoint Lambda logs to the trial group. About 10 min instead of about 1. | D-STZ-31, D-STZ-32, ADR-090 §10, `deploy.sh` (host side) |
| U-STZ-2 | D-STZ-29: tofu owns function configuration, `deploy.sh` owns code and versions; D-STZ-32: the cut-over repoints `logging_config`. | A published Lambda version snapshots its environment, logging config, memory and timeout. `live` points at a version, so a tofu apply changes only `$LATEST` and has no effect on traffic. | Every tofu change to versioned function configuration is followed by `deploy.sh lambda republish` (publish `$LATEST` with the image already live, candidate checks, flip). `lambda status` reports `$LATEST` drift. The cut-over runbook includes it. | D-STZ-29, D-STZ-32, `deploy.sh` |
| U-STZ-3 | D-STZ-7: "Every pass with exit 0 also writes `last-pass.json` (`pass_id`, `exit_code`, `finished_at`, `published`)." D-STZ-23: the probe calls the index not live when `last-pass.json` is older than 45 min. | The host's heartbeat counts any `pass_summary` in 45 min. With exit-0-only writes, three exit-2 passes in a row page through A2 and then A3 for one cause, which works against KPI-IXD-5 (≤ 2 alert emails a week). The `exit_code` field suggests other exits were meant to be recorded. | Write `last-pass.json` on **every** pass that writes a `pass_summary` (exit 0, 2 or 3). The probe's 45-minute rule then means "no pass ran", as on the host. PUT count is unchanged (≤ 2,880/month). | D-STZ-7, D-STZ-23, the probe's `not_live` rule |
| U-STZ-4 | D-STZ-21: three behaviors; `/healthz` allows GET/HEAD; the default answers 404 from a CloudFront Function. | A CloudFront behavior allows only {GET, HEAD}, {GET, HEAD, OPTIONS} or all seven methods, and answers **403** to the others before any function runs. `POST /healthz` would get 403, but the existing post-checks and ADR-083 require 404. | All three behaviors allow all methods. One CloudFront Function (viewer request) on all three passes `POST /xrpc/org.openlore.appview.searchClaims` and `GET`/`HEAD /healthz` and answers 404 to everything else. The edge stays an exact allowlist (ADR-083's first layer); the binary stays the second. Function runs are free at this volume. | D-STZ-21, ADR-083 amendment |
| U-STZ-5 | D-STZ-15: the Scheduler entry has no retries. | EventBridge Scheduler invokes Lambda asynchronously, and Lambda's own async retry (default 2 on a function error) is separate from the Scheduler's retry policy. A crashed pass would run up to three times. | `aws_lambda_function_event_invoke_config` with 0 retries (pass: max event age 600 s; probe: 240 s). Contract: the pass handler returns success with `{pass_id, exit_code, published, index_bytes}` for exit 0, 2 and 3, and fails only on a crash or timeout, so the `Errors` metric (alarm A6) means a fault. | D-STZ-15, the pass mode's return contract |
| U-STZ-6 | D-STZ-7: `last-pass.json` holds `pass_id`, `exit_code`, `finished_at`, `published`. D-STZ-23: the probe reads its age. D-STZ-11 and D-STZ-14: a failed check or an unusable newer index only leaves the old copy in place. | Nothing sees a search stuck on an old copy: `/healthz` answers 200, the probe calls it live, and the log line had no alarm. | `last-pass.json` gains `index_published_at`: the `published-at` of this pass's publish, or else of the object seen at pass start, or `null`. The probe reads `/healthz` after the canned search, compares, and when the served value is older and the record is more than 120 s old emits `indexer.lambda.served_index_stale {served, recorded, lag_s}`. A10 counts that line and `freshness_check_failed`. `not_live` and `decide::not_live` do not change. | D-STZ-7, D-STZ-23, the `LastPass` struct, the probe, the summary rows that say A4-A9, ADR-090 §4 and §7 |
| U-STZ-7 | D-STZ-44: no `mode` key means a normal pass. D-STZ-23: the probe reads no event. | `lambda test-alarm errors` needs a real function error from an invocation that does no harm. A test-fire must not be able to touch the single writer, so it does not use the pass, even now that D-STZ-44 makes the pass event strict. | The probe's handler refuses an event that is not a JSON object: a function error, with no health line and no calls. The Scheduler's event is an object, so nothing else changes. | D-STZ-23, the probe's entry |
| U-STZ-8 | D-STZ-44: a no-publish run writes nothing to the bucket, including `last-pass.json`. D-STZ-39: the write probe runs at pass init. | Init runs whatever the event, so on the very first run in a bucket a no-publish invoke creates `index/.write-probe`. Later runs get 412 and write nothing. | Keep both rules. "Writes nothing" applies to the no-publish arm. The write probe's row in the contract-shapes table already bounds it to one object, once. | D-STZ-44 wording, the no-publish test universe |

| U-STZ-9 | D-STZ-25: the ECR lifecycle keeps the last 5 images. | ECR's lifecycle counts pushes and knows nothing about aliases. Failed deploys can push out a digest that `live`, `candidate`, `$LATEST` or a `prev=` record still needs, and that image is what new instances of the version start from. | Keep the newest 10. `lambda deploy` refuses a copy that would evict a referenced digest, and deletes its own copy on a failure before publish when nothing references it. Storage rises to at most 0.45 GB, $0.045/month, which moves the totals by $0.025: $1.60 at list, $0.72 on the bill, $1.93 sub-threshold. | D-STZ-25, the ECR and total rows of § Cost |

## Not upstream (DEVOPS-level, recorded in `../feature-delta.md` § Changed Assumptions)

- IAM: `s3:ListBucket` added and `HeadObject` removed (it is not an IAM action).
- One tag-filtered budget with $2 and $10 thresholds instead of two budgets.
- S3 noncurrent versions kept 7 days (at least the 5 newest) instead of 1 day.
- Probe schedule `cron(2/5 * * * ? *)` instead of `rate(5 minutes)`.
- Lambda image smoke by RIE + moto; no `file://` test seam (DESIGN open question 1).
- `rio-object-store` job, name confirmed (D-STZ-47).
- A8-A11 thresholds and the alarm count.
- `PROBE_BASE_URL` on the CloudFront domain in both phases.
- Schedules created DISABLED (carries out D-STZ-26).

## DESIGN rows the 2026-10-30 revision leaves behind

**Done by the architect, 2026-10-30.** DISTILL did the scaffold item.

The DEVOPS revision edited only the `[DEVOPS to confirm]` figures in DESIGN sections. These lines
still say A4-A9 or omit U-STZ-6, and are for the architect:

- `../feature-delta.md`: the "Tofu resources" summary, the C4 Container alarm label, and the
  "Monitoring" row of the decisions table.
- ADR-090 §4 (record fields) and §7 (A10, A11). The ADR's cost text is already updated.
- `crates/ports/src/index_object.rs` still has one `IndexObjectPort` and no `index_published_at`; the
  D-STZ-38 scaffold change covers the first, and U-STZ-6 adds the second.
