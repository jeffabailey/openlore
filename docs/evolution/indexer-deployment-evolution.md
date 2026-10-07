# Evolution: indexer-deployment

- **Type**: infrastructure-heavy, cross-cutting, brownfield. The indexer binary changes (B1-B15),
  plus host deploy artefacts, CI image jobs and tofu. Reuses the review-app deploy pattern (ADR-075).
- **Dates**: 2026-10-06/07: DISCUSS (`281a406`) through the mutation report (`c9b041c`)
- **Job**: J-005 (discover signed claims across the network), now against a live public index.
- **Origin**: the "schedule `openlore-indexer ingest` and alert on repeated exit 3" follow-up of
  [`indexer-per-did-pds-fetch-evolution.md`](indexer-per-did-pds-fetch-evolution.md) and
  [`fix-indexer-follow-ups-evolution.md`](fix-indexer-follow-ups-evolution.md).
- **Design**: ADR-080 to ADR-083, accepted at finalize (ADR-083's "no per-IP rate limit" reversed in
  DELIVER). ADR-078 amended.
  [`docs/architecture/indexer-deployment/`](../architecture/indexer-deployment/),
  [walking skeleton](../scenarios/indexer-deployment/walking-skeleton.md),
  [journey](../ux/indexer-deployment/). Wave records are in `docs/feature/indexer-deployment/`
  (`wizard-decisions.md`, `*/wave-decisions.md`, `devops/*`, `deliver/roadmap.json`,
  `deliver/execution-log.json`, `deliver/mutation/mutation-report.md`). Runbook:
  `deploy/indexer/README.md`.
- **State**: code complete, **not deployed**. No new crates, no schema migration, no lexicon change.
  Going live needs the operator sequence below.

## Summary

Before this feature nothing ran `openlore-indexer`: no image, service, schedule or alert, so network
search had no live index. Now one long-running `serve` container on the PDS host owns the only
`index.duckdb` handle and answers public search at `https://index.openlore.jeffbailey.us` (search
and `GET /healthz` only). A 15-minute host systemd timer renders the SSM DID list to a file and runs
`docker exec … openlore-indexer trigger`, which asks `serve` over a Unix socket to run one pass
in-process (single-flight, dedicated thread, `catch_unwind`, 25-minute deadline) and exits with the
pass's 0/2/3 (busy → 0, `serve` unreachable → 4). Every pass ends with exactly one `pass_summary`.
The DID list is read and validated each pass; authors removed from it are purged (opt-in); skips
still never delete. Requests are bounded (8 KiB body, 512 B value, 1000 rows, 5 s request timeout,
64 connections) and rate limited per client (10/s, burst 50). The image is signed, built only from
green commits and deployed by digest with automatic rollback. Three CloudWatch alarms (about
$1/month) page the existing SNS email.

DISTILL wrote 74 scenarios (75 after the roadmap review), all committed `#[ignore]`; 11 DELIVER
steps took them to green.

## Business context

- **J-005, live**: a reader searching the network should find every signed claim, wherever its
  author's PDS is, without running an indexer themselves. Until now the per-DID fetch existed only
  as a binary nobody ran.
- **Operator jobs** (infrastructure-only stories): know when the index is down or stale (alerts,
  freshness), change who is indexed without a deploy (DID list), deploy and roll back safely on a
  1 GiB host shared with the PDS and the review app (memory gate).
- The CLI default stays local-first; readers opt in with `OPENLORE_INDEXER_URL`.

## User decisions

| Decision | Source |
|---|---|
| Co-located on the OpenLore PDS host as a container from one CI-built, signed arm64 image (ADR-075 pattern), riding the single tofu-aws-pds v1.7.0 replacement | wizard, WD-IXD-1 |
| A pass every 15 minutes from a host systemd timer | wizard, WD-IXD-2 |
| Search is public and read-only at `index.openlore.jeffbailey.us` (existing wildcard DNS) | WD-IXD-3 |
| The DID list is a static SSM parameter the operator edits; picked up on the next pass, no redeploy | WD-IXD-4 |
| Alerts: 2 consecutive exit-3 passes, any exit 2, and a liveness alarm (no `pass_summary` in 45 min or `/healthz` failing) | WD-IXD-5, OQ-IXD-4 |
| Authors removed from the DID list are purged (skips still never delete) | OQ-IXD-9, ADR-082 |
| The CLI default `indexer_url` is unchanged | OQ-IXD-6, WD-IXD-9 |
| Stay on t4g.micro with tighter caps: indexer container 128m, DuckDB 48 MB / 1 thread; review app `mem_limit` 192m, DuckDB 48 MB / 1 thread (B11). t4g.small only by operator decision at the memory gate | DD-IXD-10 |
| `logs:FilterLogEvents` on the indexer log group only, so a broken log pipeline pages through A3 | DEVOPS U-1 |
| Seed the DID list with `did:plc:pnyxfnpkcldxtitsw64ycahw` only; more are added with the AWS CLI | DEVOPS U-2 |
| A1 test-fire uses synthetic `pass_summary` lines in a `test-fire` stream with the timer paused | DEVOPS U-3 |
| **Per-IP rate limit in the binary** (10 req/s, burst 50, 429 + Retry-After), keyed on X-Forwarded-For from trusted proxies only. Reverses ADR-083's "no per-IP rate limit": stock Caddy has no `rate_limit` module, and a custom build would be a shared-module change | DELIVER review H3 (2026-10-07) |
| Fix all high and medium DELIVER review findings in this delivery; lows beyond L1/L2 go to the backlog | 2026-10-07 |

## Design

- **ADR-080**: `serve` owns the store and runs each triggered pass in-process (DuckDB is
  single-process, so any two-process design collides on every pass). Read/write port split: the
  search handler holds `IndexReadPort` only.
- **ADR-081**: `OPENLORE_INDEXER_REPO_DIDS_FILE` read and validated each pass; a bad list refuses the
  pass, never `serve`. The host renders SSM into a read-only **directory** mount by renaming a
  **file** (never swapping the directory) and keeps the last good copy.
- **ADR-082**: opt-in purge = pure set difference `indexed authors − non-empty loaded list`, through
  an `IndexPurgePort` held only by the pass runner; DELETE only in `adapter-index-store/src/purge.rs`;
  artifacts deleted by stored path; resumable.
- **ADR-083**: public surface is search plus `/healthz`, bounded at Caddy and in the binary. The
  per-IP rate-limit clause was reversed in DELIVER.
- **ADR-078 amended**: skips versus removals, exit-2 summaries, new events, `trigger` exit 4.
- **DEVOPS** (DV-IXD-1..16): `indexer-build` → `indexer-image` (trivy, cosign keyless, SBOMs, SLSA
  provenance, serve smoke); `deploy/indexer/deploy.sh` digest deploy with auto-rollback;
  compose posture guard; `render-dids.sh`; health timer (twin of the review app's); `indexer.tf` and
  `indexer-iam.tf` (13 in-place creates); alarms A1 total outage, A2 any exit 2 / refusal / unusable
  store, A3 not-live; nightly advisory `index-smoke`.

### Design reviews

- A **light (Haiku) review** of the first DESIGN found nothing.
- A **full-model review** then found 4 high findings, all closed before DEVOPS (`7accf17`):
  - **H1, pinned-inode DID render trap**: swapping the rendered directory would leave the running
    container on the old inode, so DID edits would silently never apply. Fixed: rename a file
    inside a directory mount (DD-IXD-13).
  - **H2, poisoned store false-green**: a panic or poisoned store could leave search answering
    empty 200s and health green. Fixed: `catch_unwind` per pass, `serve` exits 2 on an unusable
    store, search 500, `/healthz` 503 (DD-IXD-12).
  - **H3, missing log IAM**: the host role could not ship the logs the alarms depend on. Fixed in the
    IAM contract.
  - **H4, memory arithmetic**: the budget did not add up on 1 GiB. Recomputed (expected ~250 MB
    available, pessimistic ~150, cap-saturated ~50), caps tightened, hard measured gate.
- DEVOPS peer review: conditionally approved; conditions addressed in iteration 1 (one suggestion,
  `default_value: 0` on A1, rejected because A1 could then never fire).

## Work completed

| Step | What | Commit |
|---|---|---|
| — | DISCUSS (6 stories), user decisions, DESIGN (ADR-080..083), DEVOPS, DEVOPS user decisions, DISTILL (74 scenarios, all ignored, `indexer_live` harness), roadmap (11 steps, 75 scenarios, F1-F3 fixed) | `281a406`, `a8729a8`, `7accf17`, `f010be1`, `7652daf`, `604bc03`, `30cc3f7` |
| 01-01 | One index store handle per process; read/write port split (B1, B7, B10) | `66e3d70` |
| 01-02 | Single-flight pass runner on a dedicated thread | `ac164ce` |
| 01-03 | Walking skeleton: `trigger` over a Unix socket runs one pass in `serve` | `dd79705` |
| 01-04 | One `pass_summary` per pass, `/healthz`, end-of-pass checkpoint | `9bbc703` |
| 02-01 | DID list read from a file every pass (ADR-081) | `88d527e` |
| 02-02 | Purge authors removed from the DID list (ADR-082), two check-arch rules | `62e84a1` |
| 02-03 | Panic/poison recovery, 503/500 on an unusable store, pass deadline, DuckDB caps, test fault seam | `51815e1` |
| 02-04 | Public surface is search + `/healthz` only, with request and result bounds (ADR-083); viewer broken-pipe fix | `fbf79cd` |
| 03-01 | Review app applies its DuckDB memory and thread caps (B11) | `20bddba` |
| 03-02 | Host deploy artefacts: compose, Caddy site, pass/health timers, `render-dids.sh`, `deploy.sh`, runbook | `1c30474` |
| 03-03 | Indexer image CI jobs, host checks, tofu (IAM, logs, metric filters, 3 alarms, DID parameter) | `3b80ae5` |
| refactor | L1-L4 pass | `2423a25` |
| review fixes | See below | `89cb6ad`, `b296b6f`, `fa60e49`, `bf9cc70`, `943c955`, `6f9c38b` |
| mutation | Tests killing survivors; report | `d237fd3`, `84789aa`, `03bfaff`, `c9b041c` |

Each step also has a `chore: execution log` commit.

## DELIVER review findings and fixes

A full-model adversarial review returned NEEDS_REVISION (3 high, 3 medium, 5 low). All high and
medium findings plus L1/L2 were fixed:

| Finding | Problem | Fix | Commit |
|---|---|---|---|
| H1 | The `install` IMDS probe failed open: a pull failure or tag-pinned image counted as "isolated" | Probe image pinned by digest; `curl --version` positive control first; only curl exit 7/28 proves isolation, anything else refuses | `b296b6f` |
| L2 | A hung `aws` call in the health timer could block the not-live line | Every `aws` call under `timeout`; failure → `not_live 1` | `b296b6f` |
| H2 | Slow-body DoS: a client could hold a connection and the store by trickling a body | Body read and query under a 5 s request timeout (408 / 503 + Retry-After); idle keep-alive bounded | `943c955` |
| H3 | One typo'd object search made the CLI send a flood of single-edit near-match probes (about 1650 probes; `bf9cc70` measured 3306 requests for one typo), even against an empty index | The indexer returns the near-match in the same response (`objects_near`, bounded); the CLI sends one request per search. Plus the per-IP rate limit in the binary (user decision) | `bf9cc70`, `943c955` |
| M1 | check-arch purge rule could be evaded: the purge handle was a wiring field anyone could reach | The runner's thread owns the handle and lends each pass a `RunnerPurge` only it can build; check-arch flags the entry point and field outside the runner | `943c955` |
| M2 | Searches ran on the HTTP executor, so a checkpoint or purge holding the store stalled the accept loop and `/healthz` | Searches on the blocking pool; purge releases the connection while deleting files | `943c955` |
| M3 | CI could sign an image from a commit whose release guard was red; digest deploys did not verify CI | Image jobs need `test` and `release-guard`; digest deploys read the image revision label and require green CI | `fa60e49` |
| L1 | A panic while writing a summary left no `pass_summary` | Writes one exit-2 `pass_panicked` summary | `943c955` |
| follow-up | Behind Caddy every client shared the proxy's bucket | `OPENLORE_INDEXER_TRUSTED_PROXIES=172.16.0.0/12,192.168.0.0/16` in the compose; CLI degrades on 429 ("Network index is busy — try again in N s", exit 0) | `6f9c38b` |

## Quality gates

| Gate | Result |
|---|---|
| DISCUSS / DISTILL reviews | Approved |
| DESIGN review | Light review: nothing found. Full-model review: 4 high, all closed |
| DEVOPS review | Conditionally approved, conditions addressed |
| Roadmap review | Approved after F1-F3 (pass ids minted by the runner; CORE-8/9/9b/11 moved into owning crates per check-arch I-3; full `not_live` formula) |
| DES integrity | All 11 steps have complete DES traces |
| Refactor | L1-L4 pass (`2423a25`) |
| Adversarial review (full model) | NEEDS_REVISION → H1-H3, M1-M3, L1, L2 fixed |
| Mutation (in-diff vs `30cc3f7`, gate 80%) | `appview-domain` 29.1% → **100%**; `adapter-xrpc-query-server` 79.5% → **97.6%** (2 equivalent); `openlore-indexer` config/control 53.7% → **98.1%** (1 not observable in-process); `openlore-review-app` caps (advisory) 100%. `adapter-index-store` purge left to the CI nightly |
| CI | Green at `2423a25`; the run for `6f9c38b` was in progress at finalize; later commits not yet pushed |

## Lessons

- **Light (Haiku) reviews repeatedly missed real defects; full-model reviews caught them.** The light
  DESIGN review found nothing; the full-model one found H1-H4. In DELIVER the full-model review found
  H1-H3. **Lesson:** use full-model review for design and delivery gates.
- **The long-standing viewer "error sending request" CI flake was root-caused.** It was not a
  transport problem: `println!` panicked on a broken stdout pipe after the test harness stopped
  reading, killing the viewer. Fixed in `fbf79cd`; the retry in `417ddf3` only masked it.
- **Tautological property oracles again.** The `OPENLORE_INDEXER_TEST_FAULT` property used
  `TestFault::from_token` as its own oracle; CORE-4 mirrored the implementation and collapsed every
  refusal kind. Mutation testing exposed both; concrete tests now pin them. **Lesson (repeated):** a
  property needs an independent oracle.
- **The roadmap reviewer caught an invalid binding home before DELIVER**: CORE contract bindings
  placed in the workspace acceptance crate would have violated check-arch I-3. Moved into the owning
  crates (F2).
- **Crafters skipped dependent suites; the orchestrator verified.** **Lesson (repeated):** after a
  shared-crate change, run the dependent suites, not only the step's own.

## Open follow-ups

- **L3**: `PURGE_UNLISTED=0` is refused, while ADR-082 lists it as a valid value.
- **L4**: stale `SCAFFOLD` headers remain in some files.
- **L5**: `not_live` ignores a search 500 (only `/healthz` is probed).
- Tautological oracles (TEST_FAULT property, CORE-4) still present alongside the new concrete tests.
- `send_idempotent` retry is now redundant (one request per search) and could be removed.
- DuckDB cap parsing is duplicated across the indexer and the review app.
- `render-secrets.sh` (review app) should move to per-file rename (same inode trap as H1; today
  rotating needs a restart).
- Tighten `TRUSTED_PROXIES` to the actual `pds_default` subnet.
- `deploy.sh` lacks `measure`, `kpi` and `test-alarm` modes; the runbook does them by hand.
- Mutation run for `adapter-index-store` purge (CI nightly).

## Operator go-live sequence (`deploy/indexer/README.md`)

1. Release tofu-aws-pds v1.7.0 and bump the module ref.
2. Verified PDS identity backup.
3. Instance replacement (R-REPLACE, shared with the review app, once).
4. Review-app B11 deploy.
5. Apply indexer tofu (bootstrap IAM + prod resources, alarms off).
6. Make the GHCR package public.
7. Set the production DID list (≥ 2 PDS hosts).
8. Indexer deploy.
9. Memory re-measure gate (t4g.small is an operator decision).
10. N → N-1 rollback drill.
11. A1/A2/A3 test-fire (A1 synthetic).
12. Enable alarms.
13. Announce.
