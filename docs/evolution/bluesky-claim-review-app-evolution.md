# Evolution: bluesky-claim-review-app

- **Type**: user-facing feature, brownfield, cross-cutting (OAuth, hosting, PDS writes)
- **Dates**: 2026-10-04: DISCUSS (`91cd463`) through the mutation report (`3e8c9be`)
- **Jobs**: J-009 (review and publish my claims), J-010 (be seen on Bluesky: share post in
  v1; the feed generator and labeler come later)
- **Design**: ADR-071 to ADR-076, all accepted at finalize.
  [`docs/architecture/bluesky-claim-review-app/`](../architecture/bluesky-claim-review-app/),
  [walking skeleton](../scenarios/bluesky-claim-review-app/walking-skeleton.md),
  [journey](../ux/bluesky-claim-review-app/). Wave records are in
  `docs/feature/bluesky-claim-review-app/` (`*/wave-decisions.md`, `deliver/roadmap.json`,
  `deliver/execution-log.json`, `deliver/mutation/mutation-report.md`).
- **State**: code complete and CI green, including the signed GHCR image. **Not deployed.**
  Go-live is a list of operator steps (see "Open follow-ups").

## Summary

`openlore-review-app` is a hosted web app planned for `https://app.openlore.jeffbailey.us`.
A Bluesky user signs in with ATProto OAuth. They prove their GitHub account by putting their
DID in their GitHub bio. They then review private claim suggestions that come from the
existing GitHub scrape pipeline. When they approve one, the app writes a **self-attested**
`org.openlore.claim` into **their own PDS**. They can also edit a suggestion, decline it
privately, share their profile with an opt-in post, retract a claim, rescan, or disconnect
and be forgotten. OpenLore's readers (`peer pull`, the indexer, the viewer and search) now
accept and label a second provenance mode, "self-attested (repo-signed)".

The work added 4 crates: `review-domain` (pure), `adapter-atproto-oauth`,
`adapter-review-store` and `openlore-review-app`, the third composition root. DISTILL wrote
123 acceptance scenarios, and 15 DELIVER steps took them to green.

## Business context

- **J-009**: a contributor wants OpenLore claims about their own work. They want to stay in
  control: nothing is published without their say-so, and declines stay private.
- **J-010**: they want those claims to be visible on Bluesky. v1 does this with an opt-in
  share post that links to a public profile page. The feed generator and labeler are later work.
- Outcome KPIs (`discuss/outcome-kpis.md`) are measured through aggregate-only counters, a
  daily `kpi.rollup` log line and the loopback admin listener. The baseline is a 4-week
  window that starts at announcement and is reported as numerators and denominators.

## Key decisions

User decisions (wizard, DISCUSS, DESIGN, DEVOPS):

| Decision | Source |
|---|---|
| Rejections (declines) are private app-side state, used only to suppress re-suggestion. They are never written to a PDS or anywhere public. | D-4 |
| Provenance of approved claims is the PDS repo commit signature. There is no app-level signature and no PLC change for Bluesky users. | D-5, ADR-071 |
| v1 suggestions come from GitHub only. Inference from Bluesky posts is deferred. | wizard |
| The share post is opt-in and part of Release 1. It is previewed and confirmed, declining has no side effect, and it covers approved claims only. | D-11 |
| Ownership proof is the exact DID token in the GitHub bio. It is re-checked before every scrape. On failure: no scrape, pending suggestions hidden (not deleted), published claims untouched. | D-3, D-12, ADR-076 |
| Pending suggestions stay private until approved. | I-BRA-1 |
| The app is co-located on the PDS host at `app.openlore.jeffbailey.us` as a container behind the existing Caddy, adopted in one planned instance replacement (option A). | ADR-075, DESIGN U-1 |
| IMDS hop limit 1 (tofu-aws-pds v1.7.0), so containers cannot use the host role. | DEVOPS U-1 |
| Monitoring is cut to 4 alarms: A-1 unreachable, A-3 down/restarted, A-7 guardrail breach, A-8 GitHub token expiry. | DEVOPS U-2 |
| Logs are kept 30 days. | DEVOPS U-3 |
| No backup of private state in v1. If the volume is lost, a declined suggestion can be offered once more. | DESIGN decision 3 |
| If granular scopes fail, fall back to `transition:generic` with a disclosure. Not needed on bsky.social. | DESIGN decision 2 |

Architecture decisions: self-attested provenance is inferred from the record's shape and
origin, with no lexicon change, and a relay origin fails closed (ADR-071). A third
composition root has disjoint capabilities: no keychain, no user store, no app-password writer
(ADR-072). The app is a confidential OAuth client through `atrium-oauth` 0.1.7, writing
through a create-only `UserRepoWritePort` (ADR-073). Private state lives in an owner-scoped
`review-app.duckdb` with AEAD-sealed secrets (ADR-074). A single public-read PAT is protected
by a per-DID, per-IP and global budget (ADR-076). Every PDS write is a plan value that is
taken once on confirm and then read back and CID-verified (DD-7).

## Spikes

The spikes ran on 2026-10-04 against `canzantest.bsky.social`, using throwaway programs.

| Spike | Result |
|---|---|
| SPIKE-1: does a PDS accept a self-attested claim? | **Pass on bsky.social.** The record was stored and read back, and its recomputed CID equals the rkey. A post with a link facet renders. **Not run on the OpenLore PDS** (operator R-0). |
| SPIKE-2: `atrium-oauth` confidential flow | **Pass, with defects.** `revoke` expects 204 but a correct server sends 200. The callback panics through `todo!()` when the code exchange fails. Sessions cache the access token. `hickory` must be 0.26 or later (RUSTSEC-2026-0119). The `private_key_jwt` path needs the public origin, so it is deferred to the first deploy. |
| SPIKE-3: granular scopes | **Pass on bsky.social.** Writes outside the granted scopes return 403. Not run on the OpenLore PDS. |
| SPIKE-4a: Caddy import glob that matches no files | **Pass** on 2, 2-alpine and 2.6.1 |
| SPIKE-4b: app RSS during a scan | **Pass (proxy):** 71 MB peak for a 10-repo scan on macOS. Re-measure on t4g.micro (R-5). |

All of these findings were built in. Revoke treats 200 or 204 as success and always deletes
the local tokens. The callback panic is isolated. A 401 refreshes through `restore`. `hickory
< 0.26` is banned in `deny.toml`. The remaining access-token window after revoke is documented.

## Work completed

| Step | What | Commit |
|---|---|---|
| — | DESIGN (C4, ADR-071..076), spike results, DEVOPS, DISTILL (123 scenarios, walking skeleton RED), roadmap | `2150237`, `d544701`, `d32c401`, `73cd24c`, `012c26e` |
| 01-01 | Review-app crates, probes, client metadata, check-arch capability rules | `8554652` |
| 01-02 | Sign in with Bluesky (confidential client); every failure is explained and changes nothing | `6b8d4d2` |
| 01-03 | GitHub ownership proof with the DID token in the bio | `2725854` |
| 01-04 | Scan owned repos into the private queue, within budget, surviving restarts | `dc7a2ca` |
| 01-05 | Publish an approved claim as a self-attested record; peer pull accepts it (walking skeleton green); migration v6 adds the `provenance` column | `b6097d4` |
| 01-06 | Exactly-once publish under failure; container image, compose, Caddy site, CI image job | `c925450` |
| 02-01 | Edit confidence or philosophy before approving | `d0fe2d9` |
| 02-02 | Private decline with Undo | `4f61350` |
| 02-03 | Public profile of published claims, read live from the PDS | `d73a0ec` |
| 02-04 | Opt-in share post with preview and confirm | `45cb016` |
| 03-01 | CLI and viewer read and label self-attested claims | `475a541` |
| 03-02 | Indexer lists records per repo DID with paging and indexes self-attested claims | `3dc0879` |
| 03-03 | Rescan offers only new suggestions; interrupted scans resume | `6728c0a` |
| 03-04 | Retract by adding a retraction, never deleting; whole-journey write invariants | `e799083` |
| 03-05 | Disconnect and forget me; anonymous operator counts | `762574b` |
| fixes | Test helper fixed for migration v6; shellcheck SC2015; image build without `--locked` | `bbedcaf`, `8e0b849`, `6aa19fb` |

Each step also has a `chore: execution log` commit.

## Quality gates

| Gate | Result |
|---|---|
| Roadmap review | Approved (nw-software-crafter-reviewer): 3 phases, 15 steps, each scenario activated by exactly one step |
| Refactor | L1-L4 pass over the review app (`07ab655`) |
| Adversarial review | APPROVED |
| Mutation (per feature, gate 80%) | `review-domain` 72.4% → **100%** (`4b19b24`). `claim-domain` feature diff 9.7% → **96.8%** (`683abd2`); the one survivor is an equivalent mutant (`\|` → `^` on disjoint bits). `adapter-review-store` (advisory) 91.7%, with 13 survivors accepted. Report: `deliver/mutation/mutation-report.md` (`3e8c9be`). |
| DES integrity | All 15 steps have complete DES traces |
| CI | All 8 jobs green on `07ab655`, including the cosign-signed GHCR image with SBOM and provenance |

## Issues and lessons

- **Migration v6 broke a shared test helper.** Step 01-05 added `peer_claims.provenance`. The
  helper in `test-support` that bypasses the CHECK constraint rebuilt the table with a
  hard-coded 10-column schema, so two viewer suites failed. The step had run only its own
  suites. Fixed in `bbedcaf`. **Lesson:** after a schema or shared-helper change, run the
  dependent suites (viewer, peer, indexer) as well as the step's own.
- **A push went out on top of red CI.** An ungated background push sent new commits on top of a
  failing CI run. **Lesson:** a push waits for the previous run to be green.
- **New deploy and CI files failed CI.** shellcheck SC2015 (`A && B || C`) in `deploy.sh`
  (`8e0b849`). The image job used `cargo build --locked`, but `Cargo.lock` is gitignored
  (`6aa19fb`). **Lesson:** run shellcheck at CI's version and a CI-like build on new
  deploy artifacts before pushing.
- **A property test caught a security bug in step 03-04.** A retraction plan could be confirmed
  through the publish route, because plans had no type. Plans now carry a kind (publish,
  retract, share), and each kind can be confirmed only by its own route. **Lesson:** a value
  that authorizes a write must say which write it authorizes.
- **DES tooling mismatch.** The Agent hook requires the 5 legacy phase names
  (PREPARE/RED_ACCEPTANCE/RED_UNIT/GREEN/COMMIT), but `des-log-phase` accepts only
  RED/GREEN/COMMIT. In two steps (01-06 exactly-once publish, 03-04 retract), RED was logged
  after code had started, so their traces do not show test-first order.
- **Flaky viewer transport error.** A viewer suite intermittently fails with an HTTP
  transport error in CI. It is not caused by this feature, but it is now visible. Not yet
  investigated.

## Open follow-ups and decisions for the user

**Decisions:**

- **Indexer source.** The indexer reads every DID's records from one source URL. Self-attested
  claims stored on other PDSes (for example bsky.social) therefore look like they came from a
  relay and are refused under ADR-071. Fetching from each DID's resolved PDS would be a
  separate follow-up feature.
- **Legacy data key.** A 64-hex `data-key` is still accepted as the key with kid `legacy`.
  Decide whether to require the JSON key set and drop this path.
- **Not built:** the 5/day share budget, and automatic resume of interrupted scans. A manual
  resume counts toward the 6 scans/day limit.
- **KPI counters** are not updated in the same transaction as the state change, so a crash
  between the two can skew a count.
- **`adapter-review-store` survivors** (13, mostly AAD binding and schema-version edges) are
  accepted for now. The report lists the tests that would kill them.

**Operator steps before go-live** (DEVOPS rollout R-0..R-7):

1. Release `tofu-aws-pds` **v1.7.0**: the Caddy sites mount, IMDS hop limit 1, and the fix for
   the stale comment at `modules/pds/main.tf:312`. Bump the ref in both roots together.
2. SSM parameters: `client-jwk`, `data-key` (JSON key set), `github-token`, `log-salt`.
   Create the fine-grained GitHub PAT.
3. Run the instance replacement (R-REPLACE) after a verified identity backup. Make the GHCR
   package public. First deploy, then set the repo variable `REVIEW_APP_LIVE=true` (turns on
   the production half of the nightly live smoke).
4. Run SPIKE-1 and SPIKE-3 on the OpenLore PDS with the `jeff` app password. Check the
   confidential-client (`private_key_jwt`) sign-in on the live origin.
5. Re-measure RSS on the t4g.micro (move to t4g.small if it fails). Run the N → N-1 rollback
   drill. Then enable the alarms (`review_app_alarms_enabled`) and fire each one as a test.
6. Do the batched live sign-in walkthrough with `canzantest`: sign in, verify, scan, publish,
   share, retract, disconnect.
