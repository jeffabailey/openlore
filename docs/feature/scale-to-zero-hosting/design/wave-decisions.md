# DESIGN wave decisions: scale-to-zero-hosting

- **Mode**: propose. Phase 1 offered options; the user chose Option B (2026-10-26).
- **Requirements**: there are no DISCUSS artifacts. The requirements come from the SPIKE-6 pass
  criteria and `spike/findings.md`, plus the existing indexer KPIs it must not regress (KPI-IXD-3,
  KPI-IXD-4, KPI-IXD-5, NFR-IXD-6). See `../feature-delta.md` § Requirements.
- **Style**: unchanged. Hexagonal ports and adapters in the modular monolith, functional Rust
  (ADR-007). One new composition root, `openlore-indexer-lambda`, and one new effect crate,
  `adapter-s3-index-object`.
- **ADR**: ADR-090 is new. It supersedes ADR-075 for the indexer, amends ADR-080 and ADR-083, and
  extends ADR-023.

## Key Decisions

| ID | Decision | Rationale | Source |
|---|---|---|---|
| D-STZ-1 | Native Lambda root (Option B); no Web Adapter | Synchronous, pure freshness check; AWS stays out of the self-host binary | user |
| D-STZ-2 | `openlore-indexer` becomes lib + bin; self-host behaviour unchanged | Reuse without copying | user |
| D-STZ-3 | `adapter-s3-index-object` (S3 + SSM) is the only AWS adapter | ADR-023 boundary | user |
| D-STZ-4 | us-east-1 | Prod alarms, logs and SSM are there; CloudFront's ACM must be there | default |
| D-STZ-5..9 | One S3 object; single writer (reserved concurrency 1 + `If-Match`); publish only when the fingerprint changed; `last-pass.json` on every good pass | Atomic, consistent, cheap; rollback by version | design |
| D-STZ-10/11 | Warm freshness TTL 60 s (`HEAD`, reload on ETag change); a failed check never fails a search | Bounded lag, warm < 200 ms | default |
| D-STZ-12/13 | `/healthz` `last_successful_pass_at` = index published at (object metadata); empty index when no object exists | No pass runner on search instances | default |
| D-STZ-15 | Pass every 15 min (`cron(0/15 …)`) | KPI-IXD-3 unchanged; A1 alignment | user |
| D-STZ-16 | SSM DID list read each pass, rendered to `/tmp`; ADR-081 validation unchanged | KPI-IXD-4 | design |
| D-STZ-18 | Global cap: search reserved concurrency 3; A4 above 3,000 calls per 5 min; Budgets at $2 and $10; `deploy.sh lambda stop` sets concurrency to 0 | The per-client limit is void on Lambda; WAF breaks the budget | user |
| D-STZ-20/21 | CloudFront without OAC; raw Function URL public with the same bounds; CLI unchanged; three behaviors with a 404 default | OAC would break CLI POSTs | user |
| D-STZ-22 | Transport-free `dispatch` shared by hyper `serve` and Lambda | Identical bounds by construction | design |
| D-STZ-23 | A3 via a 5-minute probe function emitting `indexer.host.health` | Reuses the A3 filter and alarm; keeps one instance warm | user (2026-10-27) |
| D-STZ-24/25 | One signed digest with both binaries and `WORKDIR /`; `crane copy` to ECR after cosign verify | One verification covers every runtime | default |
| D-STZ-26/27 | Versions with `live` and `candidate` aliases; rollback by alias flip; `reset-index` and `restore-index` | Rollback in seconds | design |
| D-STZ-28/29 | New tofu root `deploy/tofu/environments/prod-indexer/`; `image_uri` and alias versions ignored by tofu | Isolates the PDS blast radius | default |
| D-STZ-31/32 | Trial: host in parallel; separate log group; manual CNAME cut-over; decommission after 7 green days; then repoint logs to `/openlore/prod/indexer` | Evidence before removal; no alarm redefinition | default |
| D-STZ-33 | A5: index size > 100 MB | The cold ceiling is about 140 MB | design |
| D-STZ-34 | Cloudflare records by hand | Not in tofu today | default |
| D-STZ-35/36 | Only the two Lambda crates require Rust 1.94.1; workspace stays 1.91; SDK on rustls with ring | Current SDK without raising the CLI's MSRV; no aws-lc or openssl | user (2026-10-27) |

The full list of 37 decisions is in `../feature-delta.md` § Design decisions.

## Architecture Summary

Three Lambda functions (arm64) run from one signed image in us-east-1:

| Function | Mode | Reserved concurrency | Trigger | What it does |
|---|---|---|---|---|
| search | `openlore-indexer-lambda search` | 3 | Function URL `live`, behind CloudFront at `index.openlore.jeffbailey.us` | Fetches `index/index.duckdb` from S3 at init. Rechecks the ETag at most every 60 s, then answers through the same `dispatch` as `serve`. |
| pass | `pass` | 1 | EventBridge every 15 min | Reads the SSM DID list, fetches the object, runs the unchanged ADR-077/078 pass, and publishes with `If-Match` when the fingerprint changed. |
| probe | `probe` | 1 | EventBridge every 5 min | Checks `/healthz`, a canned search and the age of `last-pass.json`, and emits the A3 health line. |

Alarms A1/A2 are reused through the same `pass_summary` events. A4 (invocation spike), A5 (index size) and two Budgets are new. Deploys stay laptop-initiated:
1. CI green;
2. cosign verify;
3. `crane copy`;
4. publish versions;
5. post-check the `candidate` alias;
6. flip `live`.

## Reuse Analysis

| Existing | Decision |
|---|---|
| `adapter-index-store` `open_capped` | EXTEND (+ `fingerprint`) |
| `ingest_pass` (one-shot pass, events, exit codes, purge) | EXTEND (exposed through the lib) |
| `trigger`, control socket, pass runner | KEEP for self-host and host only |
| Per-client rate limiter | KEEP in `serve`; not wired on Lambda |
| `public_route`, `admit`, `REQUEST_TIMEOUT`, `index_busy` | EXTEND (extract `dispatch`) |
| `index.caddy` | REPLACE with CloudFront at cut-over |
| `deploy.sh` gates and rollback | EXTEND (`lambda` verbs) |
| `pass_summary`, `health.startup.refused`, `indexer.store.unusable` | REUSE as-is |
| `health-timer.sh` / A3 | REPLACE the producer (probe); REUSE the event, filter and alarm |
| `render-dids.sh` | REPLACE the producer (pass); REUSE ADR-081 validation |
| CI `indexer-image` | EXTEND (both binaries, one digest) |
| Prod-root indexer tofu | KEEP, then MOVE at decommission |
| `adapter-index-query` | EXTEND (`health`) for the probe |
| Module SNS topic | REUSE through remote state |

The full table with justifications is in `../feature-delta.md` § Reuse Analysis.

## Technology Stack

- **Rust crates**: `lambda_http` 1.3.2, `lambda_runtime` 1.5.0, `aws-config` 1.13.0, `aws-sdk-s3` 1.155.0, `aws-sdk-ssm` 1.131.0, `aws-smithy-http-client` 1.5.1 (`rustls-ring`), each with `default-features = false` where the crate allows it. All are Apache-2.0, so no `deny.toml` license change is expected.
- **Fallback if per-crate MSRV proves unworkable**: `aws-config` 1.8.14, `aws-sdk-s3` 1.124.0, `aws-sdk-ssm` 1.106.0.
- **AWS**: Lambda (container image, arm64), S3, SSM, ECR, EventBridge Scheduler, CloudFront with a CloudFront Function, ACM, CloudWatch, SNS (existing), Budgets.
- **IaC**: OpenTofu with `hashicorp/aws ~> 6.0`.

## Constraints Established

- The `openlore-indexer` dependency graph contains no `aws-*` or `lambda_*` crate (new xtask rule `aws_sdk_only_in_lambda_edge`).
- `openlore-indexer-lambda` obeys the same capability boundary as the indexer: no signing, no local store, no PDS write, no GitHub, and guarded ingest clients only.
- The search mode holds read access only. `publish_*` exists only in pass mode.
- Exactly one writer of the index object (reserved concurrency 1 + conditional writes).
- The pass deadline is ≤ 780 s, inside Lambda's 900 s limit.
- The index stays under about 100 MB (A5); above about 140 MB, cold starts break 3 s.
- Every search path goes through `dispatch`, so a bound changed in one place changes everywhere.
- Every deployed digest is CI-green and cosign-verified. Lambda has no unsigned image variant.

## Upstream Changes

- **ADR-075**: amended. It no longer applies to the indexer, which ADR-090 supersedes for it. The review app is unchanged.
- **ADR-080**: amended. On Lambda, the single writer is the pass function, and the runner, socket and `trigger` are for self-host only.
- **ADR-083**: amended:
  - CloudFront replaces the Caddy layer;
  - a global cap with alarms, Budgets and a kill switch replaces per-client 429s on Lambda;
  - `last_successful_pass_at` means index published at;
  - the `Server` rule is relaxed for CloudFront.
- **ADR-023**: extended with a fourth composition root; the self-host binary is unchanged.
- **ADR-081**: rule unchanged; the renderer moves from the host to the pass function (recorded in ADR-090).
- **KPI-IXD-3/4/5 and NFR-IXD-6**: unchanged and carried over.
- **`docs/product/architecture/brief.md`**: updated (Component Inventory, Application Architecture, Platform, and the handoff notes).
- **`rust-version`**: workspace stays 1.91; `openlore-indexer-lambda` and `adapter-s3-index-object` set 1.94.1 (D-STZ-35), applied in DELIVER.
