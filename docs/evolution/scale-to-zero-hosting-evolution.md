<!-- markdownlint-disable MD013 -->
# Evolution: scale-to-zero-hosting

> All 49 roadmap steps of `scale-to-zero-hosting`, finalized 2026-10-31. The feature
> directory `docs/feature/scale-to-zero-hosting/` stays in place as delivery history.
> Decision: ADR-090 (status still Proposed). Paradigm: functional Rust (ADR-007).
> Operator evidence: `docs/evolution/scale-to-zero-hosting-evidence.md`.

**Nothing is deployed. Nothing has run against real AWS. CI has not seen this work.**
No tofu apply ran. Every `deploy.sh lambda` verb has run only against stubs. The commits sit on
local `main`, ahead of `origin/main`, so no workflow has run on them. The host still serves
`index.openlore.jeffbailey.us`.

## Summary

The indexer's search and its 15-minute pass can now run on AWS Lambda instead of the shared
t4g.micro PDS host. The code, the tofu root, the image, the CI jobs, the operator verbs and the
runbook exist. The trial, cut-over and decommission are the operator's and have not started.

- **Runtime.** A fourth composition root, `openlore-indexer-lambda`, runs three functions from
  one signed image: `search`, `pass` and `probe`.
- **Index.** One S3 object, one writer. A pass publishes only when the index changed, under
  the version it fetched.
- **Reuse.** `openlore-indexer` split into lib and bin. `dispatch` in
  `adapter-xrpc-query-server` serves both hyper and Lambda, so the bounds are the same code.
  The self-hosted binary stays free of AWS.
- **AWS access.** Only the new `adapter-s3-index-object` and the Lambda root may link `aws-*`.
- **Operator surface.** `deploy.sh lambda` and host `standby`, driven by the runbook in
  `deploy/indexer/README.md`.

The review app and the PDS stay on the host. Bring-your-own PDS and the review app on Lambda
are the other SPIKE-6 halves and are out of scope.

## Business context

SPIKE-6 asked whether the indexer could stop sharing fate with the PDS at very low cost. It
showed that the unchanged binary serves search on arm64 Lambda (the Web Adapter path, measured
in us-west-2). ADR-090 chose a native Lambda root (Option B). There was no DISCUSS wave. The
requirements are the SPIKE-6 criteria, the spike findings, and the existing indexer KPIs and
NFRs (DWD-STZ-13 records the waiver).

Cost, from ADR-090. These are design estimates, not bills:

- About $0.72/month recurring on the bill, about $1.60 at list with no free tier.
- About $1.93 for a 30-day month held just under alarms A8 and A9.
- The host bill does not change after decommission. No downsizing is decided, so the offset is
  128 MB of host memory, not money.
- Lambda spend is capped by concurrency. Edge spend has no cap, so abuse now costs money.

Real cost is read at the 30-day baseline, after cut-over.

## Wave timeline

| Wave | Date | Output |
|---|---|---|
| SPIKE | 2026-10-26 | `spike/findings.md` |
| DESIGN | 2026-10-26, revised 2026-10-30 | ADR-090, D-STZ decisions |
| DEVOPS | 2026-10-27, revised 2026-10-30 | Monitoring contracts, tofu root design, cut-over outline |
| DISTILL | 2026-10-30 | 132 scenarios, RED scaffolds |
| DELIVER | 2026-10-30 to 2026-10-31 | 49 steps, review, mutation |

## Key decisions

From DESIGN and DEVOPS (full list in `design/wave-decisions.md` and `devops/wave-decisions.md`):

- Native Lambda root, no Web Adapter (D-STZ-1), in us-east-1 (D-STZ-4).
- Search reloads on an ETag change after a 60 s TTL. A failed check never fails a search.
- Capped in rate, alerted in spend: search reserved concurrency 3, alarms A4, A8, A9, a budget
  at $2 and $10, and the kill switch `deploy.sh lambda stop` (D-STZ-18).
- CloudFront without OAC, with one CloudFront Function allowlist. The raw Function URL is public
  with the same bounds.
- Pre-flip pass gate is no-publish, with no path to the write port (D-STZ-44).
- Only the two Lambda crates need Rust 1.94.1. The workspace stays 1.91.

Taken in DELIVER:

- **Unchanged functions.** A deploy refuses if ANY function came out unchanged, so a partial
  config change needs a new build. `status` DRIFT persists until then.
- **Damaged start.** A pass refuses it with exit 2 `starting_index_unusable` and never heals it.
- **Fingerprint failures.** An after-work failure is `publish_failed`; stderr `not published:`
  tells it apart. An empty start with a failing before-work fingerprint ends `no_usable_start`.
- **M1 rejected.** PR smoke does not fit: trunk-based, no PRs, and a deploy needs green main CI.
- **H6 added beyond U-STZ-1.** Host standby also runs `systemctl disable`, so timers stay off
  through a reboot.
- **Kill-switch preflight split.** `lambda_account_check` needs only aws and jq.
  `lambda_preflight` serves deploy, republish and seed.
- **A2 is not filtered on `no_publish`.**
- **Backwards clock.** It counts freshness as due.
- **`lambda rollback --to` deferred and refused.** Use `lambda deploy <older-sha>`.
- **Rollback record.** `rolled back from <N> <utc>` carries no `prev=`, so a second rollback is
  refused.
- **Timing guards.** `restore-index` is refused when minute % 15 is 14, 0 or 1. Parity runs at
  minute % 15 == 7.
- **Fake-only tests deleted**, including `the_stubs_answer_what_the_gates_will_ask`.

## Work completed

Git range `c108420..HEAD` holds 129 commits and touches 111 files (+34,618, -765). Three of the
commits are unrelated AGENTS.md housekeeping (ce51c5e, 6f697c6, 413157e). HEAD before this
finalize is 5300ebb.

| Roadmap phase | Steps | What landed |
|---|---|---|
| 01 Walking skeleton | 11 | Lib split, `dispatch`, pure rules in `decide`, WS-1, WS-2, fingerprint, per-call budgets, check-arch rules |
| 02 Pass and search story lines | 6 | Warm reuse of the DID list, summary last, list edits, TTL reload, reset |
| 03 Write check and no-publish | 2 | Refusal of a store that ignores the condition; no-publish runs |
| 04 Error paths | 8 | Lost-update refusal, unusable store, panics, bad keys, stalled reload, shrinking deadline, init refusal |
| 05 Probe | 3 | Liveness, each `not_live` cause, stale copy |
| 06 Real adapters and runtime | 4 | S3 and SSM adapter, real write check, `rio-object-store` job, the binary |
| 07 IaC, image, CI | 5 | `prod-indexer` root, edge, alarms, budget, one image, MSRV legs, lambda-smoke |
| 08 Operator verbs | 10 | Deploy gates, rollback, republish, kill switch, reset, restore, reporting, standby, runbook |

After the steps:

- **Phase 3.5 flake fix:** 33b57f2.
- **Refactor:** a926e95, 243abbd, 1542e27, af2eb4f, 28b1de5, c3eb264.
- **Review revision:**
  - A, deploy, CI and runbook: 9160c0b, dcb68fb, 1af2d12, 5b0c044, 8b8ec13, 1a45c42, 5e364c5, 481b24e.
  - B, Rust: aee5997, 7dd3a08, 3de1fcf, 608ca62.
  - C, tests: 9c7ca50, de3a20e, bd68d56.
  - D: 7e38910.
  - Docs: 7da2669.
- **Mutation tests:** 5300ebb.

## Gate results

All of these ran locally. None ran in CI.

| Gate | Result |
|---|---|
| Roadmap | 49 steps, all committed. `des-verify-integrity`: "All 49 steps have complete DES traces" |
| Full workspace nextest at 5300ebb | 3307 run, 3307 passed, 21 skipped, 1629 s, `-j 4` |
| Earlier full run (Phase 3.5) | 3274 of 3275: one OP-13 call-log flake, fixed in 33b57f2, 450 of 450 runs after |
| `cargo xtask check-probes` | OK, 20 sites, 1 pre-existing allowlisted stub (`DuckDbPeerStorageAdapter`, not this feature). Both feature adapters are de-allowlisted |
| SCAFFOLD markers | None in this feature's code. 0 added since c108420 |
| Clippy workspace `-D warnings`, fmt, check-arch | OK at the mutation gate, on the 5300ebb test files |
| Elevator Pitch demos | Nothing to run: no DISCUSS wave, so no Elevator Pitches. No DELIVER demo touched AWS |
| Phase 4 review | **REJECTED**, one revision pass (batches A, B, C, D). **No re-review ran.** The skill allows one revision, then proceed |
| Phase 5 mutation | 292 of 292 caught after new tests, gate >= 80%. See below |

The 21 skipped tests include the 10 `rio-object-store` moto tests, `#[ignore]` by design. They
passed 10 of 10 against moto at step 06-03. The crate has changed since then (a926e95, bd68d56,
5300ebb), so the first CI run of that job is their next proof.

## Mutation results

Per-feature, gate >= 80%, run on git-archive exports of 7da2669.

- **Final:** 292 caught, 0 missed, 100%, after the new tests in 5300ebb.
- **First pass:** 274 of 292, 93.8%. `adapter-index-store` was below the gate at 61.9%.
- **Excluded:** 2 equivalent, 1 timeout (counted neither way), 123 unviable.
- **Mix:** 153 by cargo mutants, 139 by hand.
- **Outcome:** no production bug found and no production code changed.
- **Caveat:** the full cargo mutants set was not rerun after the tests. Each survivor was.

| File | Total caught | First-pass rate |
|---|---|---|
| `openlore-indexer-lambda` `decide.rs` | 45 | 100% |
| `openlore-indexer-lambda` `config.rs` | 32 | 93.8% |
| `openlore-indexer-lambda` `index_holder.rs` | 21 | 90.5% |
| `openlore-indexer-lambda` `pass.rs` | 58 | 100% |
| `adapter-s3-index-object` `s3.rs` | 67 | 91.0% |
| `adapter-s3-index-object` `lib.rs` `call_budget` | 13 | 100% |
| `adapter-s3-index-object` `client_settings.rs` | 3 | 100% |
| `adapter-s3-index-object` `ssm.rs` | 6 | 100% |
| `adapter-xrpc-query-server` `dispatch.rs` | 25 | 100% |
| `adapter-index-store` `lib.rs` (feature diff only) | 21 | 61.9% |
| `ports` `index_object.rs` constant | 1 | 100% |
| **Total** | **292** | **93.8%** |

Not in scope: `main.rs`, `search.rs`, `probe.rs` and `no_publish.rs` of the Lambda crate, and
index-store code outside the diff.

The 18 real gaps, all killed by new tests: 8 read_through columns; record key and content type;
head, record and fetch budgets; the 4xx boundary; DuckDB caps; config trim; config prefix.

## Lessons

- **Phase 4 found what the TDD steps had not:** test theater, and bad-index handling that exited
  0 silently.
- **Mutation found a claim with no proof:** "every column" for read_through, with no test for 8
  of its 12 columns.
- **Hand mutants mattered.** `dispatch.rs` gave cargo mutants only 2 viable mutants.
- **OP-13's root cause** was concurrent interleaved writes to a call log in the test stub. Each
  call is now a single write, in `xtask/tests/support/record_call.sh`.

## Issues encountered

Upstream issues and deviations:

- `lambda rollback --to` is deferred. The design said it existed.
- `lambda measure` cold calls use the raw candidate URL (open decision below).
- A partial config change needs a new build (the unchanged-function refusal above).
- `read_through` was added to fetch. The design did not have it.
- H1 undo semantics: see Known limits.
- DES: `.nwave/des-config.json` `tdd_phases` lists 5 phases and `des-log-phase` rejects those
  names, so logging used 3 phases.
- In `execution-log.json`, 08-09 is logged twice (rework) and 08-02 logs GREEN FAIL, then PASS.

Process:

- A crafter ran `kill` on its own stray `serve` PID after `pkill -f` was denied.
- The hook blocked two non-step edits. They were rerouted through DES markers, never bypassed.
- A crafter sent a placeholder report before the real one.
- One `cargo fmt --all` ran over a crafter's uncommitted work, without conflict.
- An accidental colliding mutant run was stopped and redone clean.

Flakes:

- `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam`: 3 of 6 in parallel, 0
  of 11 serial (CI runs serial). Not root-caused. It did not fail in the 3307 run.
- A 07-03 cli suite failure once. Its name was lost.
- `tests::batched_references::a_1001_row_search_costs_less_than_five_times_a_101_row_search`
  compares wall-clock and failed under load. It may flake on a loaded runner.

## Known limits

Index integrity:

- `read_through` cannot reach index-only (ART) blocks. About 1 block in 51 still gives a 500,
  and such damage can be republished. A checksum at publish would close it (a design change).
- 15 more blocks pass `read_through` but break upsert or checkpoint. That fails, not silently.
- Search process death leaves `.wal`, and `fetched_as` does not clear it.
- H8 is proven by hand (E0616), with no committed negative test.

Deploy and rollback:

- M2 checks only the deployed sha. `deploy-pds-check.yml` runs on `deploy/**` pushes only, and an
  absent run passes.
- H1 treats an update-alias that errored after AWS applied it as not flipped.
- A partly failed rollback cannot be rerun.
- A step 15 failure is undone, but an interrupt during edge checks leaves search flipped.
- A 404 cannot tell a missing key from a missing bucket.
- `deploy/review-app/deploy.sh` `ci_green` is unchanged.

Unproven on real infrastructure:

- All lambda verbs are stub-only. README recipes are unexecuted.
- `jq -cn` in `remote` is a production change to every host verb, unproven on real SSM.
- `bootstrap_image_uri` null at apply step 1 may be rejected.
- Stage 0 test-fires run with alarm actions off, so delivery is unproven.

Tests and CI:

- The fake answers Conflict for If-Match on a missing object, while the adapter treats only 412
  as conflict. Both exit 2.
- Local smoke binaries used a faster release profile. The first CI run is the final proof.
- The 1.94.1 and exact 1.91.0 legs first run in CI.
- The `aws-lc` string check may false-positive on a rustls panic message. Narrow it to
  `aws-lc-(rs|sys)-[0-9]`.
- RIE v1.38 was one day old when pinned.

## Fix later

Ruled by the reviewer:

- M4: restore outside publisher IAM (a suspicion).
- M5: scheduler SourceArn, ecr StringLike.
- M6: candidate URL public.
- M7: x-amzn-errortype strip and edge sample.
- M8: same-length download republish.
- 409 mapping.
- A timed-out PUT reports failed.
- Reload 8 s against the 3 s budget.

LOW:

- No top-level ci permissions.
- moto digest stated twice.
- The `:latest` scan misses `deploy/indexer` and `prod-indexer`.
- Marker-key trim.
- `edge_url` message on a failed CloudFront read.

## Open user decisions

The reviewer recommends ACCEPT on the first two.

1. **05-01.** Any failed probe search, including 429 and 503, sets `not_live`. The host's timer
   does so only on 500. A3 is 2 of 2, and the stop design depends on it.
2. **08-08.** `lambda measure` cold calls use the raw candidate URL, so cold through CloudFront
   is unmeasured. R-STZ-1's text is unchanged, with an as-built note in feature-delta. The
   alternative is a candidate origin.
3. **The x-amzn header path.** The runbook's route for a header refused at apply needs approval.
4. **A 140 MB index.** How one gets produced.

ADR-090's status also stays Proposed until someone decides to accept it.

## Operator evidence owed

Nothing in DELIVER touched production. Tofu apply, deploy, trial, DNS and decommission are the
operator's. The operator needs `ecr:BatchDeleteImage`.

Owed: P-15, P-17, P-18 (429 at cap 3 unverified), P-5 `lambda measure`, the 140 MB measurements,
metric checks, connect time against the 10 s init, and read_through timing on Lambda. The runbook
lists how to get each (`deploy/indexer/README.md` section 17), and the results go in
`docs/evolution/scale-to-zero-hosting-evidence.md`. KPI baselines are read at the trial.

## Lasting artifacts

Phase B copied nothing. The destination map matches no file in this feature:

| Stays in `docs/feature/scale-to-zero-hosting/` | Why |
|---|---|
| `feature-delta.md` | The wave record, and the design itself: `design/` has no architecture documents to copy |
| `*/wave-decisions.md` | Extracted above |
| `devops/upstream-changes.md`, `distill/upstream-issues.md` | Process, captured here and in feature-delta |
| `distill/red-classification.md` | RED evidence, not on the map |
| `spike/findings.md` | Spike evidence, not on the map. ADR-090 and feature-delta cite it here |
| `environments.yaml` | Environment matrix, not on the map |
| `deliver/roadmap.json`, `deliver/execution-log.json` | Scaffolding, captured here |

ADR-090 is already at `docs/adrs/ADR-090-indexer-search-and-pass-on-lambda-index-as-one-s3-object.md`.
`docs/architecture/scale-to-zero-hosting/` does not exist and is not needed. No FUTURE DESIGN label
exists under `docs/`. The feature's status in `docs/product/architecture/brief.md` now says
delivered in code, not deployed.

The operator's lasting documents are `deploy/indexer/README.md` and the evidence file.
