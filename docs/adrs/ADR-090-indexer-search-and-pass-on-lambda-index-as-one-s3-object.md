# ADR-090: Indexer Search and Pass on AWS Lambda, With the Index as One S3 Object

- **Status**: Proposed (2026-10-26)
- **Date**: 2026-10-26
- **Amended 2026-10-30 (DISTILL, accepted decisions)**: `last-pass.json` is written on every pass (U-STZ-3); noncurrent index versions are kept 7 days (DEVOPS); one CloudFront Function guards all three behaviors (U-STZ-4); a pass that cannot get a usable starting index is a failed pass (user decision, UI-2); claim artifact files are discarded on Lambda (UI-10).
- **Amended 2026-10-30 (final review, revision cycle 1)**: read and write are separate ports; the Lambda pass order and failure rulings are set; the reused pass entry is corrected; NFR-IXD-8 is superseded on Lambda; the spend guard gains A8, A9 and a kill switch that also disables the distribution; the pre-flip pass gate is no-publish; §8-§10 take U-STZ-1, U-STZ-2 and U-STZ-5. See `## Changed Assumptions (Final review, 2026-10-30)` in the feature delta.
- **Amended 2026-10-30 (DEVOPS revision)**: the cost text carries DEVOPS's final figures, from the AWS Price List API.
- **Amended 2026-10-30 (final review, revision cycle 2)**: the pass event is strict; every AWS call has an operation timeout and `max_attempts = 1`; A9 is 13,000 requests/hour and the sub-threshold month is about $1.93; search reads the index object only; §5 gains plain statements of empty results, outages and errors.
- **Deciders**: Jeff Bailey (Option B, 15-minute pass, global-cap abuse guard, CloudFront without OAC; 2026-10-26; NFR-IXD-8, spend guard, no-publish gate, one signed image; 2026-10-30), Apex (nw-platform-architect)
- **Feature**: scale-to-zero-hosting (DESIGN). Requirements come from SPIKE-6 and `docs/feature/scale-to-zero-hosting/spike/findings.md`. There are no DISCUSS artifacts.
- **Supersedes**: ADR-075 **for the indexer only**. The review app stays co-located.
- **Amends**: ADR-023 (self-host artifact, a Lambda composition root), ADR-080 (store ownership, pass trigger and summary order in production), ADR-081 (the pass reads SSM itself on Lambda), ADR-082 (where the purge contract lives), ADR-083 (edge layer, abuse guard, `/healthz` freshness meaning, `Server` header), and NFR-IXD-8 (superseded for the indexer on Lambda).
- **Builds on**: ADR-077/078 (pass, exit codes, events), ADR-081 (DID list validation), ADR-082 (purge), ADR-065 (DuckDB single process).

## Context

Today the indexer runs on the shared t4g.micro PDS host (ADR-075 pattern, ADR-080..083):
- one `serve` container behind Caddy;
- a systemd timer that triggers a pass every 15 minutes;
- a host health timer that feeds alarm A3.

SPIKE-6 (2026-10-26) showed that the unchanged binary serves search on arm64 Lambda, with the DuckDB index fetched from S3 to `/tmp` at init:
- cold 0.36-0.96 s and warm median 63-75 ms, up to 100k claims;
- a cold start fails the 3 s budget at about 140 MB of index (about 500k claims);
- the spike projected about $0.25/month. That figure is historical; the design's cost is under Consequences.

It also found:
- **finding 5**: the per-client rate limit does not hold on Lambda;
- **finding 7**: warm instances serve a stale index;
- **finding 8**: `trigger` and the control socket cannot work across instances;
- **finding 1**: the image needs `WORKDIR /`.

The forces:

| Force | Source |
|---|---|
| Cold start < 3 s, warm < 200 ms | SPIKE-6 |
| Well under $2/month | SPIKE-6, NFR-IXD-6 |
| No loss of signed-digest deploys, the CI-green gate, rollback, alarms or abuse limits | user |
| Last good pass ≤ 30 min old in ≥ 98% of hourly checks; DID edits with no deploy | KPI-IXD-3, KPI-IXD-4 |
| The self-host binary stays AWS-free and unchanged | ADR-023 |
| The CLI does not change | user |

## Decision

1. **Runtime.** A new composition root, `openlore-indexer-lambda`, ships in the **same signed image** as `openlore-indexer` (`WORKDIR /`). It runs as three arm64 functions in **us-east-1**:
   - **search**: 512 MB, reserved concurrency **3**, Function URL on alias `live`;
   - **pass**: 512 MB, reserved concurrency **1**, EventBridge Scheduler `cron(0/15 * * * ? *)`;
   - **probe**: 128 MB, reserved concurrency 1, every 5 minutes.

   The root reuses the indexer through a lib split: the wiring, the search and health handlers, and the pass path that `in_serve_pass` runs today (§4). AWS access lives only in the new effect crate `adapter-s3-index-object`. It provides two ports over S3, `IndexObjectReadPort` (`head`, `fetch_to`, `get_last_pass`) and `IndexObjectWritePort` (`publish_if_match`, `put_last_pass`), and `RepoDidListSourcePort` over SSM.

   Search and probe are composed with the read port only; pass gets both. Each function has its own `Driven` set, so a read-only mode cannot be handed a write capability. The pass's store, ingest and DID-resolve clients and the probe's HTTP client are composed internally from the environment, and sit outside this split. Each object port has its own probe; the probes stay sync, and async init calls them through `spawn_blocking`. The read probe proves a `HEAD`. The write probe runs at pass init: a zero-byte `PutObject` with `If-None-Match: *` to `index/.write-probe`, a compile-time constant in the adapter. A 412 proves the condition is honoured. A 2xx is a first run or a store that ignores conditions, so the probe repeats the PUT, and the repeat must get 412. Two 2xx mean the store ignored the condition, and the pass refuses to start with `health.startup.refused`. That is the only refusal from the write probe outcome: any other answer on either PUT, including an outage or access denied, lets the pass run and report under §4, and pass init does not gate on the read probe either. The probe proves `If-None-Match` only; `If-Match` rests on reserved concurrency 1 plus the RIO suite. Each PUT has a 2 s operation timeout with `max_attempts = 1`. No IAM change is needed. Separately, in every Lambda mode, `connect` refuses with `health.startup.refused` a location whose `index_key` or `last_pass_key` equals the probe key, through a pure check (UI-21).

   **Timeouts.** Every AWS call has an operation timeout and `max_attempts = 1`, so each figure is a real worst case. The table is in the feature delta's § Timeouts. The SDK's default retries and stalled-stream protection are to be confirmed in DELIVER.

   **The signing claim.** "Cannot sign" is not claimed: `adapter-atproto-did`, which the pass needs to verify claims, also holds the keychain path, so signing code is linked into both indexer binaries. The claim is that the indexer family holds no signing capability and no path to publish a signature. The roots build that adapter only through `resolve_only()`, which holds an all-zero placeholder key, and hold it only behind the verify-only port. They have no real key, and they reach no PDS-write, OAuth, GitHub or local-store crate. The two capability rules check this with the dependency graph and a source scan over `crates/openlore-indexer/src` and `crates/openlore-indexer-lambda/src`.

   **Credentials (NFR-IXD-8 superseded on Lambda; user decision 2026-10-30).** The functions hold short-lived role credentials from the Lambda environment. No secret is in the image. Search reads the index object only: it calls `head` and `fetch_to`, and its IAM role grants `s3:GetObject` on that object plus `s3:ListBucket` on the bucket ARN with a `StringLike` `s3:prefix` condition for `index/*`, so an empty start and a `reset-index` see 404, not 403. An object-ARN resource would grant nothing. moto does not enforce IAM, so DELIVER verifies this with the policy simulator. The probe alone reads the pass record. The user approved a broader wording; the design is narrower. NFR-IXD-8 stays in force on the host and for self-hosters.
2. **The index is one S3 object**, `index/index.duckdb`, in a versioned bucket that keeps noncurrent versions for 7 days, and always at least the 5 newest. Next to it is `index/last-pass.json`.
   - The pass function is the **only writer**. Reserved concurrency 1 means two passes never run at once, and `PutObject If-Match` (or `If-None-Match: *` for the first object) means a lost update fails with 412. A 412 logs `indexer.lambda.publish_conflict` and makes the pass exit 2, cause `publish_conflict`, which pages through A2.
   - Any other store error on publish is exit 2, cause `publish_failed`, and nothing is published.
   - A pass publishes only when the index fingerprint changed and the store is still usable. The fingerprint is (claim count, author count, XOR of a 64-bit hash of every CID), read by one SQL literal.
   - Every pass that writes a `pass_summary` (exit 0, 2 or 3) writes `last-pass.json`, so its age means "no pass ran" (U-STZ-3). It holds `pass_id`, `exit_code`, `finished_at`, `published` and `index_published_at` (U-STZ-6). If that write fails, the pass is exit 2, cause `pass_record_failed`, and an index it published stays published.
3. **Freshness.** Before it answers, a search instance whose last check is more than **60 s** old sends a `HEAD`. When the ETag changed, it downloads to a new `/tmp` file, opens it capped, swaps the read handle and deletes the old file.
   - A failed check never fails a search: the instance answers from its current copy and logs `indexer.lambda.freshness_check_failed`.
   - A stall never fails a search either. The `HEAD` has a 1 s operation timeout, which counts as a failed check. The reload has 3 s in all: when that fires, the download is dropped, the partial file is deleted, no swap happens, and the search answers from the current copy. The gate takes at most 4 s, and on Lambda the 5 s `REQUEST_TIMEOUT` covers the gate and the query together, so the query has at least 1 s (to measure at the 140 MB ceiling). The reload runs inline, one request at a time, so none is ever in flight when another search arrives.
   - A missing object gives an empty index, as after a host `--reset-index`.
   - A damaged or unusable index at init refuses with `health.startup.refused`. A newer index that cannot be used leaves the current copy in place.
4. **The pass.** Each run, in this order:
   1. reads the SSM DID list and writes it to `/tmp/config/repo-dids` with a rename. ADR-081 validation is unchanged. A warm instance keeps its last good copy for up to 2 h if SSM fails;
   2. fetches the object. A pass that cannot get a usable starting index (the store is unavailable or refuses, the object is damaged, or there is no room to download it) is a **failed pass**: exit 2, cause `starting_index_unusable`, `last-pass.json` written, nothing published, and the invocation does not crash (user decision, 2026-10-30);
   3. does the pass work with `PASS_DEADLINE_SECS=780`, and takes its exit, including the deadline check. The clock starts at handler entry, so steps 1 and 2 count against it. The entry is the path `in_serve_pass` runs today, or an equivalent extracted from it; the one-shot `run_pass` is not used, because it has no list reload, purge, panic catch or test faults. `PassLabel` and the purge contract move from `pass_runner` to a neutral lib module, so the Lambda root does not depend on `pass_runner`;
   4. publishes or not, as in §2;
   5. writes `last-pass.json`. Its `index_published_at` is the `published-at` of this pass's publish, otherwise of the object seen at the start of the pass, otherwise `null`. A damaged object that downloaded was seen, so the record names it; a failed download gives `null`;
   6. emits the single `pass_summary`, carrying the final exit.

   Everything after the deadline check must fit the **60 s margin** before the 840 s function timeout: 15 s for the local post-pass steps (fingerprint read, WAL check, handle close, staging the file; unmeasured, to measure at the 140 MB ceiling), a 35 s operation timeout for the publish (or the time left minus 10 s, if less), a 5 s operation timeout for the record, and 5 s for the summary. All with `max_attempts = 1`. A timed-out publish is `publish_failed`. It may still have landed, and that is accepted: the next pass fetches whatever is current.

   The pure `decide::final_exit(pass_exit, publish_outcome, record_outcome)` computes the final exit. A pass that already failed keeps its cause (first failure wins), then a publish failure, then a record failure, then the pass exit. The record carries the exit decided before its own write.

   **New `appview_domain::PassFailure` tokens**: `starting_index_unusable`, `publish_conflict`, `publish_failed`, `pass_record_failed`.

   **A full store outage** is exit 2, cause `starting_index_unusable`, with nothing published. The old record stays, and the invocation does not crash, on a warm or a cold instance: an outage never refuses a pass at init. A2 pages at once. A3 pages at about 50-55 min at the latest, and within about 10 min when the probe's own read fails too, because an unreadable record counts as no recent pass.

   **Invocation contract (U-STZ-5).** `aws_lambda_function_event_invoke_config` sets 0 retries, with max event age 600 s for the pass and 240 s for the probe. The pass returns success with `{pass_id, exit_code, published, index_bytes}` on exits 0, 2 and 3. It fails only on a crash, a timeout or a refused event. A panic inside the pass is a failed pass: exit 2 `pass_panicked`, the invocation succeeds, nothing is published.

   **No-publish mode (user decision 2026-10-30).** The deploy's pre-flip gate does a full run and skips the publish. It is requested by a synchronous invoke with payload `{"mode":"no-publish"}`. The pass event is strict: the pure `decide::pass_mode(event)` accepts `{}` (both schedules and `lambda trigger`) as a normal pass and an object whose only key is `mode` with a known value as no-publish. Anything else, including a non-object, a misspelled or extra key, or an unknown mode, is a function error before any work and any call. An unknown mode refuses; it does not run no-publish. The no-publish arm is `run_no_publish(read: &dyn IndexObjectReadPort, …)` in its own module, which takes the read port only, so it has no path to `publish_if_match` or `put_last_pass`, and an xtask rule refuses the write capabilities there. A no-publish run writes nothing to the bucket except the one-time write-check marker on a bucket's first run, which init writes (§1, U-STZ-8). The reply adds `mode` and `would_publish`, and `published` is always false; `deploy.sh` stops if `mode` does not come back. The `pass_summary` carries `no_publish: true`.

   The pass writes claim artifact files (`indexed_claims/<did>/<cid>.json`) next to the index in `/tmp`. Only `index.duckdb` is uploaded, so on Lambda the artifacts are discarded after every pass. Search does not read them.
5. **Public surface.**
   - **CloudFront** serves `index.openlore.jeffbailey.us` (ACM in us-east-1) with three behaviors: search, `/healthz` and the default. All three allow all methods, because a behavior limited to GET/HEAD answers 403 before any function runs. Search and `/healthz` are not cached. Price class 100.
   - One **CloudFront Function** (viewer request) on all three behaviors is the edge allowlist: it passes `POST /xrpc/org.openlore.appview.searchClaims` and `GET`/`HEAD /healthz`, and answers 404 to everything else (U-STZ-4).
   - **No OAC.** The raw Function URL stays public with auth `NONE`, with the same bounds and the same concurrency cap.
   - Both paths go through one transport-free `dispatch` in `adapter-xrpc-query-server`, which the hyper `serve` also uses: routes, 8 KiB body, 512 B value, 1000 rows, 5 s → 503 `index_busy`, 500 on store error.
   - `/healthz` `last_successful_pass_at` means **index published at**, from object metadata `x-amz-meta-published-at`. The field name and shape are unchanged. On the host and self-hosted, the field moves only on exit-0 passes. On Lambda it reports the publish time, so it also moves when an exit-2 or exit-3 pass publishes. It does not say a pass was good.
   - **What users see**: at most 3 searches run at once, and a fourth concurrent one gets 429, which the CLI reports as busy. The cap is global, so one client's traffic can make others get 429. After `reset-index` or on a fresh bucket, searches return empty results and `/healthz` returns `null` until the next pass, up to about 15 min. During a store outage, searches keep answering from the last copy with no time limit. A damaged index at cold start returns 5xx. A store error returns 500, never an empty result. After a publish, consecutive searches can disagree for up to about 60 s plus one reload, and the search that triggers a reload is slower by about one cold fetch. The first search after a new image can take about 3.1 s. Searchers outside North America and Europe get higher latency.
6. **Abuse and spend guard: capped in rate, alerted in spend.** A global cap replaces per-client 429s on Lambda:
   - search reserved concurrency 3; throttled calls get 429;
   - alarm **A4** when search invocations exceed 3,000 per 5 min;
   - alarm **A8** when search invocations exceed 1,900 per hour: the $1.28 of room under the target, at $0.89 per million warm calls, over a 720-hour month, is about 2,000 an hour, rounded down;
   - alarm **A9** when CloudFront requests exceed 13,000 per hour, because edge 404s and throttled 429s are billed and concurrency does not bound them: the free 10M requests over the longest month, 744 hours, is 13,440 an hour, rounded down;
   - **one tag-filtered AWS budget** alerting at $2 and $10 a month;
   - the kill switch `deploy.sh lambda stop` sets reserved concurrency to 0 **and disables the distribution**. It is human-in-the-loop. Tofu ignores both values, so an apply never undoes a stop.

   A run held just under A8 and A9 all month pages nobody and costs about $1.93 on the bill, or about $1.97 in a 31-day month, under the $2 target. The in-binary token bucket stays in `serve` and is not wired on Lambda.
7. **Monitoring.**
   - A1 and A2 are reused: the `pass_summary`, `health.startup.refused` and `indexer.store.unusable` events are unchanged.
   - A3 is fed by the probe. It runs the canned search, then `/healthz`, through CloudFront. It emits `indexer.host.health` with `not_live` when either fails, or when `last-pass.json` is older than 45 min, missing or unreadable.
   - **A5** fires when the published index exceeds 100 MB.
   - **A6** counts `Errors`, which means a fault: an uncaught abort, a timeout or a refused event, never an ordinary failed pass. The probe's handler refuses an event that is not a JSON object, with a function error and no calls, so `lambda test-alarm errors` can fire A6 for real (U-STZ-7). Both schedules set `input = "{}"`, and `lambda trigger`, `lambda probe` and the deploy's probe gate send `{}`.
   - **A7** counts throttles. A4, A8 and A9 are in §6.
   - **A10** counts `indexer.lambda.freshness_check_failed` and `indexer.lambda.served_index_stale`. The probe emits the second when the served `/healthz` value is older than the record's `index_published_at` and the record is more than 120 s old (U-STZ-6). The pure `decide::served_index_stale` decides. `not_live` does not change: a stale copy is still a correct index. The probe samples only the instance that answers it, so a stuck instance it does not reach is not seen, and `reset-index` can fire A10 once before the next pass.
   - **A11** counts sustained search 5xx.
8. **Deploy and rollback** (laptop-initiated, as before). The gate order is the DEVOPS one, steps 0-16 in the feature delta's § Deployment strategy:
   1. `ci_green`;
   2. resolve the digest;
   3. `cosign verify` against GHCR;
   4. `lambda_capable`;
   5. `crane copy` the same digest to ECR (immutable tags);
   6. update code and publish a version on all three functions (description `<sha> <digest>`);
   7. move the search `candidate` alias and run the existing post-checks on its URL;
   8. a no-publish pass gate and a probe gate on the new versions;
   9. flip `live`, with edge checks after the search flip.

   So the first real publish from a new version comes from the first scheduled pass after the flip. No schedule runs before the first deploy's gates have passed. Rollback flips `live` back. The index rolls back with `reset-index` (the next pass rebuilds) or `restore-index <version-id>`.

   **Republish (U-STZ-2).** `deploy.sh lambda republish` publishes `$LATEST` with the image already live, runs the candidate checks and flips. It follows every tofu change to versioned function configuration (§9).
9. **IaC.** A new root, `deploy/tofu/environments/prod-indexer/`, holds every new resource (state key `openlore/indexer/prod.tfstate`). It reads the SNS topic and the log group from the prod root's outputs. Tofu ignores `image_uri` and alias versions, which `deploy.sh` owns.

   **Drift rule (U-STZ-2).** A published version snapshots its environment, logging config, memory and timeout, and `live` points at a version. So a tofu apply to these changes only `$LATEST`, and has no effect on traffic until a republish. `lambda status` reports `$LATEST` drift against `live`.
10. **Rollout.**
    - The host indexer keeps running during a trial, and Lambda logs to `/openlore/prod/indexer-lambda` with trial copies of A1-A3.
    - Cut-over is a manual Cloudflare exact CNAME to CloudFront, after which Lambda logs to `/openlore/prod/indexer` (followed by a republish) and the existing alarms cover it.
    - **At cut-over the host goes to standby (U-STZ-1)**: pass timer, health timer and container stopped; files, units, Caddy site and releases kept. New host verbs `deploy.sh standby` and `resume`. Otherwise two `pass_summary` producers would share one log group, and the host's health timer would keep feeding A3.
    - During the trial, rollback is deleting the CNAME. During the soak it takes about 10 min: Lambda pass and probe off, host `resume` and `trigger`, delete the CNAME, repoint Lambda logs to the trial group.
    - The host indexer is decommissioned after 7 green soak days.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **A: Lambda Web Adapter + unchanged `serve` + a Rust boot binary** | The spike's exact mechanism, 2-3 days cheaper. But freshness runs in a parent process while Lambda is thawed, so the call that thaws an instance is served stale. At current traffic that is most warm calls. Three processes to supervise per instance. "Recycle on publish" (bumping env vars) fights alias rollback and tofu-managed config. | Rejected |
| **C: stay on the host** | Near $0 marginal cost and per-client 429s today. It keeps shared fate with the PDS, 128 MB of pressure on the t4g.micro, and defers the infrastructure the later SPIKE-6 halves need. | Rejected (user chose to move now) |
| **WAF rate-based rule on CloudFront** | Restores per-client limiting, but costs about $5 per web ACL + $1 per rule + $0.60/M requests, about $6+/month, which breaks the $2 target | Rejected |
| **CloudFront with OAC (Function URL auth `AWS_IAM`)** | Closes the raw URL, but POST requests through OAC must carry `x-amz-content-sha256`. That would break every existing `openlore search` client. | Rejected |
| **Keep the domain on the host's Caddy, proxying to the Function URL** | No CloudFront or ACM, but search would still go down with the host, which defeats the purpose | Rejected |
| **API Gateway instead of a Function URL** | Adds a per-request cost and a 29 s limit, and buys nothing here (spike finding 9) | Rejected |
| **DuckDB `httpfs` reading S3 directly** | No fetch, but untested and may not be in the bundled build. Only needed above about 100 MB of index. | Deferred (revisit when A5 fires) |
| **Hourly or change-driven pass** | An hourly pass saves about $0.03/month but breaks KPI-IXD-3. Change-driven needs a firehose subscription. | Rejected (15 min, user decision) |
| **A second, Lambda-only image** | Leaves the self-host image exactly as it was, but needs a second cosign verification path, and a digest that passes one gate says nothing about the other | Rejected (user kept one signed image, 2026-10-30) |
| **A pre-flip gate that really publishes** | Proves the publish path before the flip, but writes the shared index from an unflipped version, and a failed gate then needs an index restore | Rejected (user decision, 2026-10-30) |

## Consequences

- **Positive**:
  - Search no longer shares fate with the PDS, and 128 MB is freed on the t4g.micro.
  - A recurring add of about $0.72/month on the bill, or about $1.60 at list with no free tier (eight new alarms and two custom metrics are most of it).
  - Rollback is an alias flip, in seconds.
  - The same bounds everywhere through `dispatch`.
  - The self-host binary is untouched.
  - This is the groundwork for the later SPIKE-6 halves.
- **Negative**:
  - Abuse now costs money rather than only slowing search. Lambda spend is capped at about $0.15/hour by concurrency, but the edge term has no cap: $1.10 per million requests from the US and $1.30 from Europe once the free tiers are used. At the saturation rate the total is about $0.34-0.38/hour, data transfer included. A flood of 1,000 requests/s costs about $4.27-4.99/hour, and heavy replies could cost about $3.67/hour in transfer alone as the index grows (an estimate; today's 3 MB index cannot produce them). Alarms report it; stopping it needs the operator, and the edge takes minutes to stop.
  - The raw `*.lambda-url` host is public alongside CloudFront.
  - Searches can lag a pass by up to 60 s plus one reload.
  - The index has a size ceiling of about 140 MB under fetch-on-init.
  - A fourth composition root and a new AWS adapter to maintain. The two Lambda crates set `rust-version = "1.94.1"` for the current AWS SDK; the workspace and the CLI stay at 1.91.
  - The host bill does not change after decommission, because no downsizing is decided. The offset is 128 MB of host memory, not money.
  - The publish path of a new version is first exercised by the first scheduled pass after the flip.
  - Soak rollback takes about 10 min instead of about 1.
- **Earned Trust**:
  - xtask `aws_sdk_only_in_lambda_edge` keeps `aws-*` and `lambda_*` out of `openlore-indexer`.
  - The indexer capability rules extend to the new root, and both gain `adapter-atproto-oauth` and a source scan for signing entry points.
  - `indexer_search_handler_read_only` covers the Lambda search mode, and its tokens gain `publish_if_match`, `put_last_pass`, `get_last_pass`, `IndexObjectPort` and `IndexObjectWritePort`. `get_last_pass` is refused in the search module only, because the probe needs it.
  - `index_purge_only_in_pass_runner` moves with the purge contract and also scans the Lambda root.
  - An xtask test keeps `openlore-indexer` out of `cli`'s graph, dev edges included.
  - The write probe proves the store honours `If-None-Match`, on a first run or any later one, on a key that cannot harm the index. `If-Match` rests on reserved concurrency 1 plus the RIO suite.
  - `lambda_no_publish_is_read_only` keeps the write capabilities out of the no-publish module.
  - The `candidate` post-checks reuse today's: healthz, search, `createRecord` 404, `POST /healthz` 404, 8 KB ok, 9 KB 413.
  - A re-measure of cold and warm times in us-east-1 gates cut-over.
