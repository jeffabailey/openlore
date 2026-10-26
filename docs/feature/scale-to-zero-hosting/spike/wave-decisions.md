# SPIKE Decisions -- scale-to-zero-hosting

## Assumption Tested

The existing indexer's search endpoint can run on AWS Lambda (arm64), reading a DuckDB index
stored in S3. It must answer a search from a cold start in under 3 s and warm in under 200 ms,
measured on the client end to end over HTTPS. This is the search half of SPIKE-6. Dropping the
PDS, the review app, and the pass side were not built.

## Verdict

WORKS.

The unchanged binary ran behind the Lambda Web Adapter and a Function URL, fetching the index
from S3 to `/tmp` at init:

- **Real index** (1 claim, 3.2 MB) at 512 MB: cold 359-650 ms, median 404 ms. Warm median
  63 ms, max 81 ms.
- **100k claims** (28 MB) at 512 MB: cold 632-957 ms. Warm median 75 ms, max 87 ms.
- **1M claims** (264 MB) at 1024 MB: cold 4.2-5.6 s. This fails the budget, because the
  download takes 3.8-5.0 s.

The cold start ceiling is an index of about 140 MB, or about 500k claims.

Projected cost at current traffic is about $0.25/month, or about $0.03 with the free tier,
against about $12 today. The 15-minute pass is about $0.20 of that.

## Design Implications

- Move the indexer's search off the PDS host to Lambda, at 512 MB, behind a Function URL, with
  CloudFront for `index.openlore.jeffbailey.us`. This reverses ADR-075 for the indexer.
- The index becomes one S3 object. A scheduled pass Lambda running the one-shot `ingest`, with
  reserved concurrency 1, is its only writer. Search instances read it at cold start.
- Warm instances need a freshness check (S3 ETag) and a way to reopen the store, because
  `serve` cannot reopen it today.
- Reserved concurrency on the search function replaces the in-process per-client rate limit as
  the abuse and cost guard.
- Track index size, and plan a faster fetch before the index reaches about 100 MB. The options
  are parallel ranged GETs, a compressed index, or `httpfs`.
- The pass cadence, not search traffic, sets the floor cost. Consider an hourly or
  change-driven pass.
- Still open for SPIKE-6: bring-your-own PDS, and the review app on Lambda (cold sign-in and
  scan, and ADR-074 private state).

## Constraints Discovered

- **`WORKDIR`.** The image's `WORKDIR /home/nonroot` (mode 0700, uid 65532) makes Lambda fail
  with `Runtime.InvalidEntrypoint`. A Lambda image needs `WORKDIR /`.
- **S3 fetch.** The binary cannot read from S3. A fetch step has to run before
  `serve` opens the store.
- **Rate limit.** The per-instance limit is defeated by Lambda's one-request-per-instance
  fan-out: 70 parallel calls gave 54 cold starts and no 429.
- **Memory and disk.** The `/tmp` page cache counts toward Lambda memory (338 MB for a 264 MB
  index). `/tmp` defaults to 512 MB.
- **Cold start vs index size.** Cold start is about 150 ms plus the S3 download at about
  50-90 MB/s. 1024 MB is no faster than 512 MB. 128 MB works, but warm calls are 1.5-2x slower.
- **First call after a deploy.** It took 3.1 s while Lambda cached the image. Calls after weeks
  of no traffic were not measured.
- **Function URL permissions.** Public auth NONE needs both `lambda:InvokeFunctionUrl` and
  `lambda:InvokeFunction` (`--invoked-via-function-url`) in the resource policy.
- **Control socket.** `trigger` and the control socket do not carry over to Lambda. The pass
  must use `ingest`.
- **DuckDB compatibility.** The bundled libduckdb (crate 1.10506.1, CI arm64 build) read files
  written by the DuckDB 2.0 CLI and ran on Lambda unchanged.
