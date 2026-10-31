# DESIGN wave decisions: scale-to-zero-hosting

- **Mode**: propose. Phase 1 offered options; the user chose Option B (2026-10-26).
- **Requirements**: there are no DISCUSS artifacts. The requirements come from the SPIKE-6 pass
  criteria and `spike/findings.md`, plus the existing indexer KPIs it must not regress (KPI-IXD-3,
  KPI-IXD-4, KPI-IXD-5, NFR-IXD-6, NFR-IXD-7). See `../feature-delta.md` § Requirements.
- **Style**: unchanged. Hexagonal ports and adapters in the modular monolith, functional Rust
  (ADR-007). One new composition root, `openlore-indexer-lambda`, and one new effect crate,
  `adapter-s3-index-object`.
- **ADR**: ADR-090 is new. It supersedes ADR-075 and NFR-IXD-8 for the indexer on Lambda, and amends
  ADR-023, ADR-080, ADR-081, ADR-082 and ADR-083.
- **Revised 2026-10-30** (revision cycle 1 after the final four-reviewer gate). See
  `../feature-delta.md` § Changed Assumptions (Final review, 2026-10-30).

## Key Decisions

| ID | Decision | Rationale | Source |
|---|---|---|---|
| D-STZ-1 | Native Lambda root (Option B); no Web Adapter | Synchronous, pure freshness check; AWS stays out of the self-host binary | user |
| D-STZ-2 | `openlore-indexer` becomes lib + bin; self-host behaviour unchanged | Reuse without copying | user |
| D-STZ-3 | `adapter-s3-index-object` (S3 + SSM) is the only AWS adapter | ADR-023 boundary | user |
| D-STZ-4 | us-east-1 | Prod alarms, logs and SSM are there; CloudFront's ACM must be there | default |
| D-STZ-5..9 | One S3 object, noncurrent versions kept 7 days; single writer (reserved concurrency 1 + `If-Match`); publish only when the fingerprint changed; `last-pass.json` on every pass that writes a summary, with `index_published_at` | Atomic, consistent, cheap; rollback by version | design, U-STZ-3, U-STZ-6 |
| D-STZ-10/11 | Warm freshness TTL 60 s (`HEAD`, reload on ETag change); a failed check or a stall never fails a search: 1 s `HEAD`, 3 s reload, and on timeout keep the current copy. `REQUEST_TIMEOUT` (5 s) covers the gate and the query together, so the query has at least 1 s (to measure at 140 MB) | Bounded lag, warm < 200 ms | default, re-review |
| D-STZ-12/13 | `/healthz` `last_successful_pass_at` = index publish time on Lambda, including publishes from exit-2 and exit-3 passes; empty index when no object exists | No pass runner on search instances | default, decided explicitly 2026-10-30 |
| D-STZ-14 | A store failure or an unusable index at init refuses with `health.startup.refused`; an unusable newer index leaves the current copy in place | A broken store never looks healthy | design, UI-4 |
| D-STZ-15 | Pass every 15 min (`cron(0/15 …)`), with no retries in the Scheduler or in Lambda | KPI-IXD-3 unchanged; A1 alignment | user, U-STZ-5 |
| D-STZ-16 | SSM DID list read each pass, rendered to `/tmp`; ADR-081 validation unchanged | KPI-IXD-4 | design |
| D-STZ-18 | Capped in rate, alerted in spend: search reserved concurrency 3; A4 above 3,000 calls per 5 min; A8 above 1,900 calls per hour; A9 above 13,000 CloudFront requests per hour; a month just under both costs about $1.93; one tag-filtered budget at $2 and $10; `deploy.sh lambda stop` sets concurrency to 0 and disables the distribution | The per-client limit is void on Lambda; WAF breaks the budget; edge requests are not bounded by concurrency | user |
| D-STZ-20/21 | CloudFront without OAC; raw Function URL public with the same bounds; CLI unchanged; three all-method behaviors; one CloudFront Function allows the two routes and answers 404 otherwise | OAC would break CLI POSTs; a GET/HEAD behavior answers 403 before any function runs | user, U-STZ-4 |
| D-STZ-22 | Transport-free `dispatch` shared by hyper `serve` and Lambda | Identical bounds by construction | design |
| D-STZ-23 | A3 via a 5-minute probe function (`cron(2/5 …)`) emitting `indexer.host.health`; a stale, missing or unreadable record counts as no recent pass; a non-object event is a function error | Reuses the A3 filter and alarm; keeps one instance warm; gives A6 a real, harmless test-fire | user (2026-10-27), U-STZ-7 |
| D-STZ-24/25 | One signed digest with both binaries and `WORKDIR /`; `crane copy` to ECR after cosign verify | One verification covers every runtime | default, reconfirmed by the user 2026-10-30 |
| D-STZ-26/27 | Versions with `live` and `candidate` aliases; the DEVOPS gate order, with a no-publish pass gate; no schedule runs before the first deploy's gates; rollback by alias flip; `reset-index` and `restore-index` | Rollback in seconds | design |
| D-STZ-28/29 | New tofu root `deploy/tofu/environments/prod-indexer/`; `image_uri` and alias versions ignored by tofu; `lambda republish` after any change to versioned configuration | Isolates the PDS blast radius; a published version snapshots its configuration | default, U-STZ-2 |
| D-STZ-31/32 | Trial: host in parallel, separate log group; manual CNAME cut-over; host on standby at cut-over; decommission after 7 green soak days; logs repoint to `/openlore/prod/indexer` at cut-over | Evidence before removal; one `pass_summary` producer per log group; no alarm redefinition | default, U-STZ-1 |
| D-STZ-33 | A5: index size > 100 MB | The cold ceiling is about 140 MB | design |
| D-STZ-34 | Cloudflare records by hand | Not in tofu today | default |
| D-STZ-35/36 | Only the two Lambda crates require Rust 1.94.1; workspace stays 1.91; SDK on rustls with ring | Current SDK without raising the CLI's MSRV; no aws-lc or openssl | user (2026-10-27) |
| D-STZ-38/39 | `IndexObjectPort` splits into read and write ports; search and probe get read only; `Driven` is per-function, and the internally composed clients sit outside the split; each port has its own probe, sync, called through `spawn_blocking`. The write probe repeats a PUT that got 2xx, the repeat must get 412, and two 2xx is the only refusal from the write probe outcome; an outage never refuses a pass at init. It proves `If-None-Match` only, and its key is a constant. `connect` also refuses, in every Lambda mode, a location whose `index_key` or `last_pass_key` equals that constant (UI-21). No IAM change | A read-only mode cannot name a write; Earned Trust on conditional writes; a failed pass keeps its summary | final review, UI-17, UI-20, re-review |
| D-STZ-40/40a | Lambda pass order: pass exit, publish, `last-pass.json`, one summary with the final exit from pure `final_exit`; local post-pass steps (15 s, to measure), publish (35 s), record (5 s) and summary (5 s) fit the 60 s margin; operation timeouts and `max_attempts = 1` on every AWS call; a timed-out publish may have landed, and that is accepted | A 412 must be exit 2 inside the one summary (UI-14); retries would multiply each timeout | final review, re-review |
| D-STZ-41 | Failure rulings and four new `PassFailure` tokens | Each failed pass names its cause (UI-15, UI-2) | user, final review |
| D-STZ-42 | Reused entry is `in_serve_pass` or an extracted equivalent; `PassLabel` and the purge contract move to a neutral module | `run_pass` lacks what the Lambda pass needs (UI-16) | final review |
| D-STZ-43 | Event-invoke config and the return contract; a panic is a failed pass | `Errors` means a fault (U-STZ-5, UI-3) | final review |
| D-STZ-44 | The pre-flip pass gate is no-publish: requested by payload, and with no path to the write port. The pass event is strict: `{}` or `{"mode":"no-publish"}`, and anything else, including an unknown mode, is a function error before any work. A no-publish run writes nothing except the one-time write-check marker on a bucket's first run | A gate does not write the shared index from an unflipped version; a mistyped invoke does nothing | user (2026-10-30), U-STZ-8, re-review |
| D-STZ-45 | `cli` drops its `openlore-indexer` dev-dependency; dev edges stay in the rule | UI-7 | final review |
| D-STZ-46 | The fingerprint is one SQL literal; no allowlist, no counting in Rust | The scanner already passes it (UI-9) | final review |
| D-STZ-47 | RIO-1..9 run in a dedicated job, `rio-object-store` | UI-6; DEVOPS confirmed the name | final review |
| D-STZ-48 | The probe compares the served `/healthz` value with the record's `index_published_at` through pure `decide::served_index_stale`; `not_live` does not change. A damaged object that downloaded was seen, so the record names it; a failed download gives `null` | A search stuck on an old copy was invisible | U-STZ-6, UI-19 |
| D-STZ-50 | The indexer family holds no signing capability and no path to publish a signature. "Cannot sign" is not claimed: `resolve_only()` holds an all-zero placeholder key behind a verify-only port. Checked by the capability rules: the graph, with `adapter-atproto-oauth` added, plus a source scan over `openlore-indexer/src` and the Lambda root | `adapter-atproto-did` holds both the verify path and the keychain path | UI-18 |
| D-STZ-49 | A10 counts failed freshness checks and stale served index readings; A11 counts sustained search 5xx | Both only logged before | DEVOPS |

The full list of decisions is in `../feature-delta.md` § Design decisions.

## Architecture Summary

Three Lambda functions (arm64) run from one signed image in us-east-1:

| Function | Mode | Reserved concurrency | Trigger | What it does |
|---|---|---|---|---|
| search | `openlore-indexer-lambda search` | 3 | Function URL `live`, behind CloudFront at `index.openlore.jeffbailey.us` | Fetches `index/index.duckdb` from S3 at init. Rechecks the ETag at most every 60 s, then answers through the same `dispatch` as `serve`. Read port only. |
| pass | `pass` | 1 | EventBridge every 15 min | Reads the SSM DID list, fetches the object, runs the `in_serve_pass` path, publishes with `If-Match` when the fingerprint changed, writes `last-pass.json`, and emits one summary with the final exit. |
| probe | `probe` | 1 | EventBridge every 5 min | Checks `/healthz`, a canned search and the age of `last-pass.json`, and emits the A3 health line. Read port only. |

Alarms A1/A2 are reused through the same `pass_summary` events. A4 (invocation spike), A5 (index size), A6 (errors), A7 (throttles), A8 (hourly calls), A9 (edge requests), A10 (failed freshness checks and stale served index), A11 (search 5xx) and one budget are new. Deploys stay laptop-initiated, in the DEVOPS gate order:
1. CI green;
2. cosign verify;
3. `crane copy`;
4. publish versions;
5. post-check the `candidate` alias;
6. no-publish pass gate and probe gate;
7. flip `live`, with edge checks.

## Reuse Analysis

| Existing | Decision |
|---|---|
| `adapter-index-store` `open_capped` | EXTEND (+ `fingerprint`) |
| `in_serve_pass` (list reload, purge, panic catch, faults, events, exit codes) | EXTEND (exposed through the lib, summary split out) |
| `trigger`, control socket | KEEP for self-host and host only |
| Pass runner | KEEP single-flight for self-host and host; MOVE `PassLabel` and the purge contract to a neutral module |
| Per-client rate limiter | KEEP in `serve`; not wired on Lambda |
| `public_route`, `admit`, `REQUEST_TIMEOUT`, `index_busy` | EXTEND (extract `dispatch`) |
| `index.caddy` | REPLACE with CloudFront at cut-over |
| `deploy.sh` gates and rollback | EXTEND (`lambda` verbs) |
| `pass_summary`, `health.startup.refused`, `indexer.store.unusable` | REUSE, with four new `cause` tokens |
| `health-timer.sh` / A3 | REPLACE the producer (probe); REUSE the event, filter and alarm |
| `render-dids.sh` | REPLACE the producer (pass); REUSE ADR-081 validation |
| CI `indexer-image` | EXTEND (both binaries, one digest) |
| Prod-root indexer tofu | KEEP, then MOVE at decommission |
| `adapter-index-query` | EXTEND (`health`) for the probe |
| Module SNS topic | REUSE through remote state |

**CREATE NEW**, each with the candidate checked:

| New | Candidate checked | Why it does not fit |
|---|---|---|
| `openlore-indexer-lambda` | A mode of `openlore-indexer` | It would break ADR-023's AWS-free rule |
| `decide.rs` | `appview_domain` pass decisions | Lambda-only decisions; object-store terms do not belong in the domain |
| `adapter-s3-index-object` | Every `crates/adapter-*` | No object-store or parameter-store adapter exists |
| `crates/ports/src/index_object.rs`, `repo_did_list.rs` | `IndexStorePort`, `IndexReadPort`, `read_did_list` | Local operations only; no versions, conditional writes or remote source |
| `prod-indexer` tofu root | The prod root | An apply there plans against the PDS module |
| Index holder | `SharedIndexReads` | It shares one handle that never changes; the holder swaps it over the same seam |
| `no_publish.rs` | `pass.rs` | Its own file is what lets an xtask token rule check it |
| `search.rs`, `pass.rs`, `probe.rs` | The `serve` path, `ingest_pass.rs` | They hold Lambda event handling only |
| `runner_contract` module | `pass_runner.rs` | The Lambda root must not depend on `pass_runner` |

The full table, with contract shapes, is in `../feature-delta.md` § Reuse Analysis. `decide.rs` is pure-function. The universes and deltas of the bounded-change writes are in § Component decomposition.

## Technology Stack

- **Rust crates**: `lambda_http` 1.3.2, `lambda_runtime` 1.5.0, `aws-config` 1.13.0, `aws-sdk-s3` 1.155.0, `aws-sdk-ssm` 1.131.0, `aws-smithy-http-client` 1.5.1 (`rustls-ring`), each with `default-features = false` where the crate allows it. All are Apache-2.0, so no `deny.toml` license change is expected.
- **Fallback if per-crate MSRV proves unworkable**: `aws-config` 1.8.14, `aws-sdk-s3` 1.124.0, `aws-sdk-ssm` 1.106.0.
- **AWS**: Lambda (container image, arm64), S3, SSM, ECR, EventBridge Scheduler, CloudFront with a CloudFront Function, ACM, CloudWatch, SNS (existing), Budgets.
- **IaC**: OpenTofu with `hashicorp/aws ~> 6.0`.

## Constraints Established

- The `openlore-indexer` dependency graph contains no `aws-*` or `lambda_*` crate (new xtask rule `aws_sdk_only_in_lambda_edge`).
- `openlore-indexer-lambda` obeys the same capability boundary as the indexer: no signing capability composed and no key held (the signing code is linked, D-STZ-50), no local store, no PDS write, no OAuth, no GitHub, and guarded ingest clients only.
- Search and probe hold the read port only. `IndexObjectWritePort` exists only in pass mode, and never in its no-publish arm. `SEARCH_HANDLER_WRITE_TOKENS` gains `publish_if_match`, `put_last_pass`, `IndexObjectPort` and `IndexObjectWritePort`.
- Exactly one writer of the index object (reserved concurrency 1 + conditional writes).
- The pass deadline is ≤ 780 s from handler entry. The publish, the record and the summary fit the 60 s margin before the 840 s timeout, inside Lambda's 900 s limit.
- The index stays under about 100 MB (A5); above about 140 MB, cold starts break 3 s.
- Every search path goes through `dispatch`, so a bound changed in one place changes everywhere.
- Every deployed digest is CI-green and cosign-verified. Lambda has no unsigned image variant.
- A no-publish run writes nothing to the bucket except the one-time write-check marker on a bucket's first run, which init writes, whatever the event.
- Every caller that expects a normal pass or probe run sends `{}` (`input = "{}"` on both schedules; `{}` from `lambda trigger`, `lambda probe` and the probe gate). A pass event other than `{}` or `{"mode":"no-publish"}` is a function error.
- Every AWS call has an operation timeout and `max_attempts = 1`.
- No schedule runs before the first deploy's gates have passed.
- Search reads the index object only; the probe alone reads the pass record.
- Spend has no automatic stop. A human runs `lambda stop`.

## Upstream Changes

- **ADR-075**: amended. It no longer applies to the indexer, which ADR-090 supersedes for it. The review app is unchanged. Standby at cut-over is recorded.
- **ADR-080**: amended. On Lambda, the single writer is the pass function, and the runner, socket and `trigger` are for self-host and the host until standby. The Lambda pass reuses the `in_serve_pass` path with its summary after the publish.
- **ADR-083**: amended:
  - CloudFront replaces the Caddy layer, with three all-method behaviors and one viewer-request function;
  - a global cap with alarms, a budget and a kill switch replaces per-client 429s on Lambda;
  - `last_successful_pass_at` means index published at;
  - the `Server` rule is relaxed for CloudFront.
- **ADR-023**: amended: the self-host artifact is the `openlore-indexer` binary, the image also carries the Lambda binary, and a fourth composition root joins the family. The self-host binary is unchanged.
- **ADR-081**: amended: the list rule is unchanged, the renderer moves from the host to the pass function, and on Lambda the pass reads SSM itself.
- **ADR-082**: `index_purge_only_in_pass_runner` and "only the pass runner is given this port" move with the purge contract. Recorded in ADR-090; ADR-082 itself still needs its amendment note.
- **NFR-IXD-8**: superseded for the indexer on Lambda (user, 2026-10-30): short-lived role credentials, no secret in the image, read-only roles on the read-only functions. The user approved a broader wording for search; the design is narrower.
- **NFR-IXD-3**: carried over: search p95 ≤ 1 s.
- **NFR-IXD-7**: carried over and restated: after cut-over a burst no longer touches the PDS; part of a burst can get 429.
- **R-STZ-1**: the first search after a new image is an exception (about 3.1 s).
- **KPI-IXD-3/4/5 and NFR-IXD-6**: unchanged and carried over. The NFR-IXD-6 statement about a 6-hour attack is corrected: it is true only at saturation.
- **`docs/product/architecture/brief.md`**: updated (Component Inventory, Application Architecture, Platform, and the handoff notes). The hosting and "For Software Crafter" parts carry the 2026-10-30 revision.
- **`rust-version`**: workspace stays 1.91; `openlore-indexer-lambda` and `adapter-s3-index-object` set 1.94.1 (D-STZ-35), already applied to both crates.
- **Accepted from DEVOPS (`../devops/upstream-changes.md`)**:
  - **U-STZ-1**: host standby at cut-over, new verbs `standby` and `resume`, soak rollback about 10 min (D-STZ-31, D-STZ-32, ADR-090 §10).
  - **U-STZ-2**: `lambda republish` after any tofu change to versioned configuration, and a drift report in `lambda status` (D-STZ-29, ADR-090 §8 and §9).
  - **U-STZ-3**: `last-pass.json` on every pass that writes a summary (D-STZ-7, D-STZ-23).
  - **U-STZ-4**: all behaviors allow all methods, and one CloudFront Function is the allowlist (D-STZ-21).
  - **U-STZ-5**: event-invoke config with 0 retries, and the pass fails only on a crash or timeout (D-STZ-15, D-STZ-43).
- **Accepted by the architect from the DEVOPS revision (2026-10-30)**:
  - **U-STZ-6**: `last-pass.json` gains `index_published_at`, and the probe compares it with the served value (D-STZ-7, D-STZ-23, D-STZ-48, ADR-090 §2, §4, §7).
  - **U-STZ-7**: the probe's handler refuses a non-object event, on condition that both schedules set `input = "{}"` and `lambda probe` and the probe gate send `{}` (D-STZ-23, ADR-090 §7).
  - **U-STZ-8**: a no-publish run writes nothing except the one-time write-check marker on a bucket's first run, which init writes (D-STZ-44, ADR-090 §4).
- **DISTILL issues ruled on (2026-10-30)**: UI-17 and UI-20 (D-STZ-39, ADR-090 §1, §4), UI-18 (D-STZ-50, ADR-090 §1), UI-19 (D-STZ-48, ADR-090 §4).
