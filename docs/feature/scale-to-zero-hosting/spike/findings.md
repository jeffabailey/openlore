# Spike Findings -- scale-to-zero-hosting

Run 2026-10-26 for SPIKE-6 (`docs/feature/bluesky-claim-review-app/design/wave-decisions.md`).
Timebox: about 1 hour. Region us-west-2, arm64.

## Verdict: WORKS

The unchanged indexer binary (image `ghcr.io/jeffabailey/openlore-indexer@sha256:df8a32e9…`,
commit 491b7ae) serves `POST /xrpc/org.openlore.appview.searchClaims` on AWS Lambda (arm64)
behind a Lambda Function URL, with the DuckDB index downloaded from S3 to `/tmp` at init.
For the real index, and for synthetic indexes up to 100k claims (28 MB), every cold start
answered in under 1.4 s and the warm median was 63-75 ms. Both are measured on the client, end
to end over HTTPS. The cold budget (3 s) breaks somewhere between 100k and 1M claims. At 1M
claims (264 MB) a cold start took 4.2-5.6 s, because downloading the file dominates.

This answers only the search half of SPIKE-6. Dropping the OpenLore PDS, the review app on
Lambda, and the pass side on Lambda were not built.

## Question tested

Can the existing indexer's search endpoint run on AWS Lambda (arm64), reading a DuckDB index
stored in S3, and answer a search from a cold start in under 3 s and warm in under 200 ms,
measured on the client end to end over HTTPS?

## Core mechanism

- **Image.** The GHCR image was copied to a throwaway ECR repo with `crane` (no rebuild). One
  layer was appended with two files:
  - the AWS Lambda Web Adapter 0.9.1 at `/opt/extensions/lambda-adapter`;
  - a throwaway 9.6 MB static Go wrapper, used as the entrypoint. It downloads
    `s3://<bucket>/<key>` to `/tmp/index.duckdb`, then `exec`s
    `/usr/local/bin/openlore-indexer serve`.

  The indexer binary itself is unchanged.
- **Config.** The function used the production env from `deploy/indexer/host/compose.yaml`
  (`DUCKDB_MEMORY_LIMIT_MB=48`, `DUCKDB_THREADS=1`), with three changes: `INDEX_PATH=/tmp/index.duckdb`,
  `LISTEN_ADDR=127.0.0.1:8080`, and `CONTROL_SOCKET` in `/tmp`. It also set
  `AWS_LWA_PORT=8080` and `AWS_LWA_READINESS_CHECK_PATH=/healthz`. The function URL used
  auth NONE.
- **Result.** Searches returned the same JSON as the local container. The indexer's own guards
  work without Caddy: GET `createRecord` returns 404, POST `/healthz` returns 404, and a
  9 KB body returns 413.

## Index sizes

| Index | Claims | File size | How it was made |
|---|---|---|---|
| real | 1 | 3.16 MB | `openlore-indexer ingest` against the live PDS for `did:plc:pnyxfnpkcldxtitsw64ycahw` (pass took 790 ms) |
| syn10k | 10,000 | 8.1 MB | real file + rows inserted with the DuckDB CLI (200 authors, 1 evidence row each, 1 reference per 10 claims) |
| syn100k | 100,000 | 28.3 MB | same shape |
| syn1M | 1,000,000 | 264 MB | same shape, 2,000 authors |

The real index is almost all fixed DuckDB overhead. The synthetic rows cost about 260 bytes
per claim on disk.

Only one DID was ingested. The second configured DID is not in the repo (only the tofu seed
value is), and the spike rules forbade touching SSM, so the production parameter was not read.

## Timing

Cold starts were forced by changing an env var between calls; each was the first call to a new
execution environment. Warm calls were 20 sequential calls right after a cold one. Each `curl`
opens a new TLS connection, as the CLI does. The client was the operator's laptop; the TLS
handshake took about 30 ms. Init and fetch times come from CloudWatch
(`INIT_REPORT`/`REPORT` and the wrapper's `spike.fetch` line).

| Index | Memory | Cold n | Cold client min / median / max (ms) | Lambda Init Duration (ms) | S3 fetch in init (ms) | Warm (n=20) min / median / p90 / max (ms) | Max memory used |
|---|---|---|---|---|---|---|---|
| real 3.2 MB | 128 MB | 3 | 438 / 458 / 801 | 169-203 | 57-79 | 75 / 102 / 142 / 193 | 52 MB |
| real 3.2 MB | 512 MB | 5 | 359 / 404 / 650 | 144-349 | 57-70 | 56 / 63 / 67 / 81 | 53 MB |
| real 3.2 MB | 1024 MB | 5 | 365 / 367 / 482 | 155-241 | 48-75 | 53 / 63 / 77 / 79 | 54 MB |
| syn10k 8.1 MB | 512 MB | 5 | 413 / 474 / 495 | 202-290 | 99-165 | 55 / 65 / 72 / 76 | 58 MB |
| syn100k 28 MB | 128 MB | 3 | 1027 / 1045 / 1051 | 566-689 | 454-574 | 125 / 137 / 158 / 186 | 81 MB |
| syn100k 28 MB | 512 MB | 5 | 632 / 690 / 957 | 405-692 | 304-594 | 62 / 75 / 84 / 87 | 80 MB |
| syn100k 28 MB | 1024 MB | 5 | 638 / 946 / 1331 | 449-762 | 361-632 | 58 / 68 / 80 / 128 | 81 MB |
| syn1M 264 MB | 1024 MB | 3 | **4205 / 5501 / 5552** | 3856-5135 | 3758-5001 | 155 / 165 / 182 / 220 | 338 MB |

Results against the budget (cold under 3 s, warm under 200 ms):
- **Real, 10k and 100k: PASS** at 128, 512 and 1024 MB.
- **1M: cold FAIL**, and warm max 220 ms is just over budget.
- The first call after a new image was deployed had an Init Duration of 2,829 ms (3.1 s on the
  client) for the 3 MB index. This happens once per deploy, while Lambda caches the image. It is
  not in the table.

**How cold start scales.** Init is about 100-150 ms of fixed cost plus the S3 download. A single
GET ran at about 50-90 MB/s. Opening DuckDB adds almost nothing at these sizes. On the client,
a cold call is about Init + 200 ms. So the 3 s budget allows an index of roughly 140 MB:
about 500k claims of this shape with a single-stream GET.

**Effect of memory.** 1024 MB was no faster than 512 MB, because the download is network-bound.
128 MB works, but its warm calls are 1.5-2x slower (less CPU), and the 100k cold start is
about 1 s. 512 MB is the sweet spot.

## Cost projection (arithmetic only)

Assumptions:
- us-west-2 arm64 prices: $0.0000133334 per GB-s and $0.20 per 1M requests;
- S3: PUT $0.005 per 1k, GET $0.0004 per 1k, storage $0.023 per GB-month;
- CloudWatch Logs ingest $0.50 per GB; ECR storage $0.10 per GB-month.

The figures below ignore the free tier. With the Lambda free tier (400k GB-s and 1M requests a
month), the Lambda lines are $0.

| Item | Arithmetic | $/month |
|---|---|---|
| Pass Lambda (not built) | 96/day × 30 = 2,880 runs; assume 10 s at 512 MB (fetch, ingest about 1 s today, put) = 2,880 × 10 × 0.5 = 14,400 GB-s × $0.0000133 | 0.19 |
| Pass requests | 2,880 × $0.20/1M | <0.01 |
| Search Lambda | assume 3,000 searches, all cold (worst case), 0.7 s billed at 512 MB (init is billed) = 1,050 GB-s | 0.014 |
| Search requests | 3,000 × $0.20/1M | <0.01 |
| S3 PUT (one per pass) | 2,880 × $0.005/1k | 0.014 |
| S3 GET (pass + cold starts) | about 6,000 × $0.0004/1k | <0.01 |
| S3 storage | 30 MB | <0.01 |
| CloudWatch Logs | about 20 MB | 0.01 |
| ECR image | about 40 MB | <0.01 |
| EventBridge Scheduler | 2,880 invocations (14M free a month) | 0 |
| Function URL, public IPv4, EBS | none | 0 |
| **Total** | | **about $0.25 (about $0.03 with the free tier)** |

Compared with about $12/month today, this meets the "<$2/month" target with a wide margin.

"About $0 at rest" holds only if the pass also stops when nothing changes. The 15-minute pass
costs about $0.20 of the $0.25 even with no searches. An hourly pass would cut that to about
$0.05. A custom domain (`index.openlore.jeffbailey.us`) needs CloudFront or an API Gateway
custom domain in front of the function URL; the CloudFront free tier covers this traffic.

## Edge cases and surprises

1. **The image's `WORKDIR` breaks Lambda.** The distroless image's `WorkingDir` is
   `/home/nonroot`, with mode 0700 and owner 65532. Lambda runs as a different uid and ignores
   `USER`, so init failed with `Runtime.InvalidEntrypoint`. Setting the workdir to `/` fixed it.
   A Lambda image must set `WORKDIR /`.
2. **The binary cannot fetch from S3.** The unmodified binary only opens a local path, so a
   wrapper (or a code change) has to download the index first. The wrapper was a static Go
   binary that `exec`s the indexer. A production version should do this inside the binary, or
   in a small entrypoint built in CI.
3. **The bundled DuckDB worked as is.** The binary from the existing CI arm64 build ran on
   Lambda unchanged. Files written by the DuckDB 2.0 CLI were read by the binary's bundled
   libduckdb (crate 1.10506.1) without error. The synthetic indexes depended on that.
4. **`/tmp` works, but page cache counts as memory.** DuckDB on `/tmp` worked with no special
   settings. The file's page cache counts toward "Max Memory Used" (338 MB for the 264 MB
   index), so plan memory as at least the index size plus about 70 MB. The default `/tmp` is
   512 MB, which caps the index there unless ephemeral storage is raised (billed).
5. **The per-client rate limit does not hold on Lambda.** 70 parallel requests caused 54 cold
   starts and returned 70 × 200, with no 429. Lambda sends one request at a time to each
   instance and starts a new instance for the next, so each new instance has an empty token
   bucket. Abuse protection and cost control have to come from reserved concurrency on the
   function (calls above the cap are throttled by Lambda), or from CloudFront or WAF. The
   account limit is 1,000 concurrent executions, which is the real blast radius if nothing caps it.
6. **The client IP comes from `X-Forwarded-For`.** The adapter connects over loopback, which
   the indexer already trusts, and Function URLs set `X-Forwarded-For` to the caller's IP. So
   the indexer should read the right client. Per-client limiting was not verified, and it is
   moot given item 5.
7. **Warm instances serve a stale index.** S3 PutObject is atomic and S3 reads are strongly
   consistent, so a cold start always gets a whole, current file, and a pass writing a new
   index cannot tear a read. But a warm instance keeps the copy it downloaded until Lambda
   recycles it (minutes to hours), so searches can lag passes. A freshness check is needed, and
   `serve` has no way to reopen its store, so this needs a code change or a process restart in
   the wrapper. One option is a HEAD request with an ETag check, at most every N seconds.
8. **The 15-minute pass needs a different shape.** The `trigger` command and the control socket
   cannot work across Lambda instances. The one-shot `ingest` subcommand fits a scheduled pass
   Lambda: get the object, ingest, put the object. Its reserved concurrency must be 1 so two
   passes never race on the S3 object. The pass function is the only writer, and conditional
   puts (`If-Match`) would guard against a lost update.
9. **Function URL vs API Gateway.** The Function URL needed no extra resources and costs
   nothing. It gave HTTPS on a `*.lambda-url.us-west-2.on.aws` host, and responses carry
   `x-amzn-*` headers. Public `NONE` auth needed two resource-policy statements:
   `lambda:InvokeFunctionUrl`, and `lambda:InvokeFunction` with `--invoked-via-function-url`.
   API Gateway would add a request cost and a 29 s timeout without helping here. The 6 MB
   response limit is not a concern, because search results are capped (`SearchTruncated`).
10. **Long-idle risk (not measured).** A function with no calls for weeks can be marked
    inactive by AWS, and its first call is then slower or fails while it reactivates. The
    timed cold starts here came seconds after an update, with the image cached. A
    rarely-searched function could see worse first calls. The 3.1 s first call after an image
    update is a rough upper bound for that case.
11. **Larger indexes need a faster download.** At 1M claims, init approaches the 10 s init
    limit as the index grows. Parallel ranged GETs, a compressed index (zstd), or querying S3
    directly through DuckDB's `httpfs` would be needed. None of these were tested, and `httpfs`
    may not be in the bundled build.

## Design implications

- **The search side can run scale-to-zero on Lambda now.** It can use the existing binary and
  image plus a fetch-on-init step, at 512 MB, with a cold start well under 1 s at today's size
  and up to about 100k claims. ADR-075 (co-locate on the PDS host) can be reversed for the
  indexer.
- **The index stays a rebuildable cache in S3.** It is one object, written only by a scheduled
  pass Lambda (`ingest`, reserved concurrency 1), and read by search instances at cold start.
- **Required changes before production.** All are small:
  - `WORKDIR /` in a Lambda image variant;
  - an S3 fetch step built into the binary or image;
  - a freshness check (ETag) for warm instances;
  - reserved concurrency on the search function, which replaces the per-client limiter as the
    real guard;
  - CloudFront for the custom domain.
- **Index size has a ceiling under this design.** At about 140 MB (about 500k claims) the cold
  start passes 3 s. Track index size as a metric and revisit before it reaches about 100 MB.
- **Pass cadence sets the floor cost.** Cost at current traffic is about $0.25/month (about
  $0.03 with the free tier), against about $12 today. The 15-minute pass, not search, drives the
  floor.
- **Still open in SPIKE-6.** Dropping the OpenLore PDS (bring-your-own PDS), the review app on
  Lambda (sign-in and scan cold starts, private state under ADR-074), and an ingest of the
  second configured DID.

## Spike code

Deleted. It was in the session scratchpad (`.../scratchpad/spike6/`): the Go fetch-and-exec
wrapper, `measure.sh`, and the synthetic index builder (DuckDB CLI SQL). All AWS resources
(all named `openlore-spike6-*` and tagged `spike=spike-6`) were deleted, and an empty listing
confirmed it: the ECR repo, the S3 bucket and objects, the IAM role and its policies, the
Lambda function, its Function URL and permissions, and the log group.
