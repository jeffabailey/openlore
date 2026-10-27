# ADR-090: Indexer Search and Pass on AWS Lambda, With the Index as One S3 Object

- **Status**: Proposed (2026-10-26)
- **Date**: 2026-10-26
- **Deciders**: Jeff Bailey (Option B, 15-minute pass, global-cap abuse guard, CloudFront without OAC; 2026-10-26), Apex (nw-platform-architect)
- **Feature**: scale-to-zero-hosting (DESIGN). Requirements come from SPIKE-6 and `docs/feature/scale-to-zero-hosting/spike/findings.md`. There are no DISCUSS artifacts.
- **Supersedes**: ADR-075 **for the indexer only**. The review app stays co-located.
- **Amends**: ADR-080 (store ownership and pass trigger in production) and ADR-083 (edge layer, abuse guard, `/healthz` freshness meaning, `Server` header).
- **Extends**: ADR-023. A Lambda composition root joins the indexer family, and the self-hostable binary is unchanged.
- **Builds on**: ADR-077/078 (pass, exit codes, events), ADR-081 (DID list validation), ADR-082 (purge), ADR-065 (DuckDB single process).

## Context

Today the indexer runs on the shared t4g.micro PDS host (ADR-075 pattern, ADR-080..083):
- one `serve` container behind Caddy;
- a systemd timer that triggers a pass every 15 minutes;
- a host health timer that feeds alarm A3.

SPIKE-6 (2026-10-26) showed that the unchanged binary serves search on arm64 Lambda, with the DuckDB index fetched from S3 to `/tmp` at init:
- cold 0.36-0.96 s and warm median 63-75 ms, up to 100k claims;
- a cold start fails the 3 s budget at about 140 MB of index (about 500k claims);
- projected cost is about $0.25/month.

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

   The root reuses the indexer through a lib split: wiring, the ADR-077/078 pass, the search and health handlers. AWS access lives only in the new effect crate `adapter-s3-index-object`, which provides `IndexObjectPort` over S3 and `RepoDidListSourcePort` over SSM.
2. **The index is one S3 object**, `index/index.duckdb`, in a versioned bucket that keeps 5 noncurrent versions for 1 day. Next to it is `index/last-pass.json`.
   - The pass function is the **only writer**. Reserved concurrency 1 means two passes never run at once, and `PutObject If-Match` (or `If-None-Match: *` for the first object) means a lost update fails with 412. A 412 logs `indexer.lambda.publish_conflict` and exits 2, which pages through A2.
   - A pass publishes only when the index fingerprint changed and the store is still usable. The fingerprint is (claim count, author count, XOR of a 64-bit hash of every CID).
   - Every exit-0 pass writes `last-pass.json`.
3. **Freshness.** Before it answers, a search instance whose last check is more than **60 s** old sends a `HEAD`. When the ETag changed, it downloads to a new `/tmp` file, opens it capped, swaps the read handle and deletes the old file.
   - A failed check never fails a search: the instance answers from its current copy and logs `indexer.lambda.freshness_check_failed`.
   - A missing object gives an empty index, as after a host `--reset-index`.
4. **The pass.** Each run:
   1. reads the SSM DID list and writes it to `/tmp/config/repo-dids` with a rename. ADR-081 validation is unchanged. A warm instance keeps its last good copy for up to 2 h if SSM fails;
   2. fetches the object;
   3. runs the unchanged pass with `PASS_DEADLINE_SECS=780` (function timeout 840 s);
   4. emits the same single `pass_summary`;
   5. publishes or not, as in §2.
5. **Public surface.**
   - **CloudFront** serves `index.openlore.jeffbailey.us` (ACM in us-east-1) with three behaviors:
     - search: all methods, no caching;
     - `/healthz`: GET/HEAD, no caching;
     - default: 404 from a CloudFront Function.
   - **No OAC.** The raw Function URL stays public with auth `NONE`, with the same bounds and the same concurrency cap.
   - Both paths go through one transport-free `dispatch` in `adapter-xrpc-query-server`, which the hyper `serve` also uses: routes, 8 KiB body, 512 B value, 1000 rows, 5 s → 503 `index_busy`, 500 on store error.
   - `/healthz` `last_successful_pass_at` means **index published at**, from object metadata `x-amz-meta-published-at`. The field name and shape are unchanged.
6. **Abuse and spend guard.** A global cap replaces per-client 429s on Lambda:
   - search reserved concurrency 3; throttled calls get 429;
   - alarm **A4** when search invocations exceed 3,000 per 5 min;
   - **AWS Budgets** alerts at $2 and $10 a month;
   - the kill switch `deploy.sh lambda stop` sets reserved concurrency to 0.

   The in-binary token bucket stays in `serve` and is not wired on Lambda.
7. **Monitoring.**
   - A1 and A2 are reused: the `pass_summary`, `health.startup.refused` and `indexer.store.unusable` events are unchanged.
   - A3 is fed by the probe. It emits `indexer.host.health` with `not_live` when `/healthz` or the canned search through CloudFront fails, or when `last-pass.json` is older than 45 min.
   - **A5** fires when the published index exceeds 100 MB.
8. **Deploy and rollback** (laptop-initiated, as before):
   1. `ci_green`;
   2. resolve the digest;
   3. `cosign verify` against GHCR;
   4. `crane copy` the same digest to ECR (immutable tags);
   5. update code and publish a version on all three functions (description `<sha> <digest>`);
   6. move the search `candidate` alias and run the existing post-checks on its URL;
   7. flip `live`.

   Rollback flips `live` back. The index rolls back with `reset-index` (the next pass rebuilds) or `restore-index <version-id>`.
9. **IaC.** A new root, `deploy/tofu/environments/prod-indexer/`, holds every new resource (state key `openlore/indexer/prod.tfstate`). It reads the SNS topic and the log group from the prod root's outputs. Tofu ignores `image_uri` and alias versions, which `deploy.sh` owns.
10. **Rollout.**
    - The host indexer keeps running during a trial, and Lambda logs to `/openlore/prod/indexer-lambda` with trial copies of A1-A3.
    - Cut-over is a manual Cloudflare exact CNAME to CloudFront, after which Lambda logs to `/openlore/prod/indexer` and the existing alarms cover it.
    - The host indexer is decommissioned after 7 green days.
    - During the trial, rollback to the host is deleting the CNAME.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **A: Lambda Web Adapter + unchanged `serve` + a Rust boot binary** | The spike's exact mechanism, 2-3 days cheaper. But freshness runs in a parent process while Lambda is thawed, so the call that thaws an instance is served stale. At current traffic that is most warm calls. Three processes to supervise per instance. "Recycle on publish" (bumping env vars) fights alias rollback and tofu-managed config. | Rejected |
| **C: stay on the host until the host is retired** | Near $0 marginal cost and per-client 429s today. It keeps shared fate with the PDS, 128 MB of pressure on the t4g.micro, and defers the infrastructure the later SPIKE-6 halves need. | Rejected (user chose to move now) |
| **WAF rate-based rule on CloudFront** | Restores per-client limiting, but costs about $5 per web ACL + $1 per rule + $0.60/M requests, about $6+/month, which breaks the $2 target | Rejected |
| **CloudFront with OAC (Function URL auth `AWS_IAM`)** | Closes the raw URL, but POST requests through OAC must carry `x-amz-content-sha256`. That would break every existing `openlore search` client. | Rejected |
| **Keep the domain on the host's Caddy, proxying to the Function URL** | No CloudFront or ACM, but search would still go down with the host, which defeats the purpose | Rejected |
| **API Gateway instead of a Function URL** | Adds a per-request cost and a 29 s limit, and buys nothing here (spike finding 9) | Rejected |
| **DuckDB `httpfs` reading S3 directly** | No fetch, but untested and may not be in the bundled build. Only needed above about 100 MB of index. | Deferred (revisit when A5 fires) |
| **Hourly or change-driven pass** | An hourly pass saves about $0.03/month but breaks KPI-IXD-3. Change-driven needs a firehose subscription. | Rejected (15 min, user decision) |

## Consequences

- **Positive**:
  - Search no longer shares fate with the PDS, and 128 MB is freed on the t4g.micro.
  - About $0.36/month, or about $0.30 with the Lambda free tier.
  - Rollback is an alias flip, in seconds.
  - The same bounds everywhere through `dispatch`.
  - The self-host binary is untouched.
  - This is the groundwork for retiring the host in later SPIKE-6 halves.
- **Negative**:
  - Abuse now costs money rather than only slowing search: about $0.28 per hour at saturation. A4, Budgets and the kill switch bound it, but stopping it needs the operator.
  - The raw `*.lambda-url` host is public alongside CloudFront.
  - Searches can lag a pass by up to 60 s plus one reload.
  - The index has a size ceiling of about 140 MB under fetch-on-init.
  - A fourth composition root and a new AWS adapter to maintain. The two Lambda crates set `rust-version = "1.94.1"` for the current AWS SDK; the workspace and the CLI stay at 1.91.
  - The bill does not fall until the host is retired.
- **Earned Trust**:
  - xtask `aws_sdk_only_in_lambda_edge` keeps `aws-*` and `lambda_*` out of `openlore-indexer`.
  - The indexer capability rules extend to the new root.
  - `indexer_search_handler_read_only` covers the Lambda search mode.
  - The `candidate` post-checks reuse today's: healthz, search, `createRecord` 404, `POST /healthz` 404, 8 KB ok, 9 KB 413.
  - A re-measure of cold and warm times in us-east-1 gates cut-over.
