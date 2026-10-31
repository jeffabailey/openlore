# RED classification: scale-to-zero-hosting (DISTILL, 2026-10-30, closing)

Pre-DELIVER fail-for-the-right-reason gate. Every test was run once with
`cargo nextest run -j 2 --no-fail-fast --run-ignored only`, then left `#[ignore]`d.

- **MISSING_FUNCTIONALITY**: the scenario fails because the behaviour is not built (correct RED). For
  the Lambda suites that is a `panic!("Not yet implemented -- RED scaffold")` in a scaffold body.
- **GUARD**: not ignored; green at hand-off and must stay green.
- **NOT_RUN**: RIO-1..8 need moto. Without `OPENLORE_TEST_MOTO_ENDPOINT` they panic and name it, so a
  run that never reached the store cannot pass. They are reported as not run, not as RED. RIO-9 needs
  no moto: its store is a silent socket, so it is ordinary RED.
- **BROKEN** (setup, fixture, harness, compile) or **WRONG_ASSERTION**: none remain.

## Result

| Measure | Count |
|---|---|
| Scenarios | 132: 118 RED, 8 not run, 6 guards |
| Ignored test functions run | 146 |
| RED for the right reason | 137 test functions (the 8 CORE scenarios are 27 of them) |
| Not run (no moto) | 9 test functions: RIO-1..8 and the seed check |
| Passed while ignored | **0** |
| BROKEN | 0 |
| Green and not ignored | 37 results, 29 distinct tests: 2 `state_delta` self-tests are compiled into 5 binaries, so they count 10 times. They are XS-2, XS-2b, XS-3, XS-4, XS-5, XS-7 and checks that are not scenarios |

### RIO, not run without moto; run once by hand

moto 5.2.5 could be started cheaply outside the repo (`uvx --from 'moto[server]==5.2.5' moto_server`), so
the suite was also run against it, after this revision. Nothing was added to the repo.

| Test | Result with moto |
|---|---|
| `the_seeds_really_reach_the_moto_store` | **Passed**: the parameter reads back and versioning is `Enabled` |
| RIO-1..9 | All 10 tests ran: the seed check above passed, and these 9 stopped at the adapter's scaffold panic, RIO-1..8 after their seeds ran |

So the seeds are proven, and the adapter contract is not: the adapter is a scaffold. moto was the pip
package; the `motoserver/moto:5.2.5` image that CI pins was not run.

## What the RED does and does not prove

Most Lambda scenarios stop at the first scaffold call (`PassFunction::init` or
`SearchFunction::init`), so their later steps have never run. RED here proves that the Given steps
before that call work and that the suite links. It does not prove the assertions after it.

The risk is reduced, not removed, by tests that run green at hand-off:
`tests/acceptance/scale_to_zero_harness.rs` runs the fake bucket, the write check, the stalls,
`ForbiddenWrites`, the timeline, `each_row`, the canned edge, the body builders and the event readers;
`the_stubs_answer_what_the_gates_will_ask` runs the `deploy.sh` stubs;
`the_tofu_reader_reads_whole_values_in_the_right_block` runs the tofu reader;
`the_secret_check_catches_each_shape_and_passes_a_plain_environment` runs the XS-12 check; and
`the_guards_catch_a_planted_violation` gives XS-2..5 and XS-7 a positive control, which they need because
no `aws-*` crate is in the graph yet. A scenario that fails in DELIVER for a reason unrelated to the
behaviour should be read as a possible test bug first.

## Fixed during this gate

- **OP-3's stop half could pass with no search-0 call**: `None < Some(_)` is true for `Option`. It now
  asserts the call and guards the order with `.is_some()`.
- **OP-E3, OP-E4, OP-E7, OP-E8, OP-E9, OP-E16** prove the gate was reached (`assert_reached`), because
  "it stopped, and nothing flipped" also holds for a usage message. The stop arms also assert the
  refusal line. OP-E6's declined arms prove the verb ran.
- **OP-12 and OP-11** could pass with no verb, so each asserts that a tool ran.
- `scale_to_zero_core.proptest-regressions` is written when a panicking property runs. It held
  scaffold-panic seeds, not real findings, and was deleted. Delete it again if it reappears before
  `decide` is built.

## Per scenario

| Scenario | Title | Classification | First failure observed |
|---|---|---|---|
| WS-1 | Maria finds Priya's claim after the scheduled pass published the index | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| WS-2 | The operator adds an author and, with no deploy, Maria finds their claim | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-1 | A fresh deployment answers before any index was published | MISSING_FUNCTIONALITY | `SearchFunction::init` scaffold panic. |
| SE-2 | The index reports when the copy it serves was published, not when it last looked | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-3 | A warm instance picks up a newly published claim within a minute | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-4 | When nothing was published, a warm instance only checks and keeps its copy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-5 | After the operator resets the index, warm instances answer empty until the next pass | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-6 | Serial searches from one client are all answered | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-7 | A warm instance on a fresh deployment picks up the first published index | MISSING_FUNCTIONALITY | `SearchFunction::init` scaffold panic. |
| SE-8 | Two warm instances disagree for at most about a minute after a publish | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-B1 | The search entry turns away what the self-hosted index turns away | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E1 | A search the index cannot answer within 5 seconds is told to retry shortly | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E2 | A search the index cannot read fails loudly, never as an empty result | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E6 | An index whose store breaks while it runs says so instead of looking healthy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E3 | When the freshness check fails, Maria still gets an answer from the current copy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E4 | A newer index the instance cannot use leaves Maria on the current copy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E7 | A freshness check that stalls still leaves Maria an answer from the current copy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E8 | A reload that stalls is abandoned, and Maria stays on the current copy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E5 | A search instance that cannot get a usable index never starts answering | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| SE-E9 | After a slow freshness check, a search has only what is left of its 5 seconds | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-1 | The first pass creates the index and records itself | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-2 | A pass that changes nothing publishes nothing but still records itself | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-3 | A pass that indexes a new claim replaces exactly the version it started from | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-4 | An edit to the DID list takes effect on the next pass with no redeploy | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-5 | A pass that fails part way still publishes the claims it committed | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-6 | A pass that reaches nobody publishes nothing and still records itself | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-7 | A warm pass rides out a parameter store outage on its last good list | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-8 | An author removed from the DID list leaves search on the next pass | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-9 | An emptied DID list purges nobody and publishes nothing | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-10 | A pass on an empty bucket with nothing to publish records that no index exists | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-11 | A pass reports itself last, after publishing and recording | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-12 | A no-publish run does the whole pass and writes nothing to the bucket | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-13 | On a bucket's very first run, a no-publish run leaves only the write-check marker | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-14 | The write check leaves one marker in a bucket's life, however often passes start | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E1 | A pass never overwrites an index somebody else published while it ran | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E2 | A pass whose store became unusable never publishes it | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E3 | A pass with no usable DID list refuses, publishes nothing, and pages | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E4 | A pass refuses a DID list with a typo and keeps the index as it was | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E5 | A pass that cannot get a usable starting index is a failed pass | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E6 | A pass that panics part way is reported as a failed pass and publishes nothing | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E7 | A pass during a full store outage still reports a failed pass | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E8 | A pass that cannot publish its changes fails and keeps the index as it was | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E9 | A pass whose record cannot be written reports exit 2 and its index stays published | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E10 | A store that ignores the write condition is refused before a pass can trust it | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E11 | A pass refuses any event but an empty one or a no-publish request | MISSING_FUNCTIONALITY | `PassFunction::init` scaffold panic. |
| PA-E12 | A function configured to keep its data under the write check's marker never starts | MISSING_FUNCTIONALITY | `SearchFunction::init` scaffold panic. |
| PR-1 | The probe reports a live index | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-2 | A failed pass that just ran does not also make the index look dead | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-3 | The probe reports a search stuck on an old copy, without calling the index dead | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-4 | The probe writes no stale-index line when it has nothing to compare or nothing lags | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-B1 | A pass exactly 45 minutes old still counts as recent | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-E1 | The probe says the index is not live, and why, for each failure | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-E2 | A public index that hangs is not live, and the probe still reports it in time | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-E3 | The probe refuses an event that is not an object and does nothing else | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| PR-E4 | A health answer the probe cannot read counts as a failed health check | MISSING_FUNCTIONALITY | `ProbeFunction::init` scaffold panic. |
| CORE-1 | A warm instance checks the index at most once per TTL, and always before its first answer | MISSING_FUNCTIONALITY | `decide::freshness_due` scaffold panic, in all its tests. |
| CORE-2 | A warm instance downloads only a different index, empties only when the index is gone, and never fails on a failed check | MISSING_FUNCTIONALITY | `decide::reload_needed` scaffold panic, in all its tests. |
| CORE-3 | A pass publishes exactly when it changed the index and the index is still usable | MISSING_FUNCTIONALITY | `decide::publish_decision` scaffold panic, in all its tests. |
| CORE-4 | The probe calls the index live only when every fact is good, and names each bad one | MISSING_FUNCTIONALITY | `decide::not_live` scaffold panic, in all its tests. |
| CORE-5 | A warm pass reuses its last good DID list for at most 2 hours | MISSING_FUNCTIONALITY | `decide::dids_copy_usable` scaffold panic, in all its tests. |
| CORE-6 | The summary carries the first failure, and a pass that did not fail keeps its exit | MISSING_FUNCTIONALITY | `decide::final_exit` scaffold panic, in all its tests. |
| CORE-7 | A pass accepts exactly two events and refuses every other | MISSING_FUNCTIONALITY | `decide::pass_mode` scaffold panic, in all its tests. |
| CORE-8 | A served index is stale only when it lags a record old enough to trust | MISSING_FUNCTIONALITY | `decide::served_index_stale` scaffold panic, in all its tests. |
| RIO-1 | The index object round-trips through a real object store, and a lost update is refused | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-2 | The pass record round-trips through a real object store | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-3 | A bucket that does not exist is a store failure, never "no index yet" | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-4 | The DID list is read verbatim from a real parameter store, and a missing list is a failure | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-5 | The start-up probes tell a reachable store from a missing one | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-6 | The write check creates one empty marker and never touches the index | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-7 | A write the store cannot take is a failure, never a lost update | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-8 | A pass record nobody can read is an error, never "no pass yet" | NOT_RUN | Panics naming `OPENLORE_TEST_MOTO_ENDPOINT`. With moto: the seeds run, then the adapter scaffold panics. |
| RIO-9 | A store that accepts the connection and never answers does not stop a pass starting | MISSING_FUNCTIONALITY | `S3IndexObjectAdapter::connect` scaffold panic. Needs no moto: its store is a silent socket. |
| TO-1 | Every store call has one attempt and its own time limit | MISSING_FUNCTIONALITY | `call_budget` scaffold panic. |
| FP-1 | An empty index has the zero fingerprint, and counts claims and authors | MISSING_FUNCTIONALITY | `IndexStoreAdapter::fingerprint` scaffold panic. |
| FP-2 | Indexing the same claim again does not change the fingerprint | MISSING_FUNCTIONALITY | `IndexStoreAdapter::fingerprint` scaffold panic. |
| FP-3 | The fingerprint does not depend on the order claims were indexed in | MISSING_FUNCTIONALITY | `IndexStoreAdapter::fingerprint` scaffold panic. |
| FP-4 | Adding a claim changes the fingerprint, and purging its author restores it | MISSING_FUNCTIONALITY | `IndexStoreAdapter::fingerprint` scaffold panic. |
| FP-5 | Two indexes with the same counts but different claims have different fingerprints | MISSING_FUNCTIONALITY | `IndexStoreAdapter::fingerprint` scaffold panic. |
| WP-1 | Only two accepted writes mean a store ignores the condition | MISSING_FUNCTIONALITY | `write_probe_outcome` scaffold panic. |
| WP-2 | The write check's marker can never be the index | MISSING_FUNCTIONALITY | `location_names_write_probe_key` scaffold panic. |
| XS-1 | check-arch knows the Lambda root and its rules | MISSING_FUNCTIONALITY | `COMPOSITION_ROOTS` lacks `openlore-indexer-lambda`. |
| XS-2 | The self-hosted indexer stays free of AWS and Lambda code | GUARD | Runs at hand-off, green. |
| XS-2b | The self-hosted indexer has no path to write to a PDS | GUARD | Runs at hand-off, green. |
| XS-3 | Only the Lambda root and its adapter may reach AWS | GUARD | Runs at hand-off, green. |
| XS-4 | The CLI links neither the Lambda root nor its adapter | GUARD | Runs at hand-off, green. |
| XS-5 | The Lambda root holds no claim store, no PDS write path and no GitHub client | GUARD | Runs at hand-off, green. |
| XS-6 | Only the pass can publish the index | MISSING_FUNCTIONALITY | The pass names no `publish_if_match` yet. Its read-only checks already pass. |
| XS-7 | The CLI's graph does not reach the self-hosted indexer, dev edges included | GUARD | Runs at hand-off, green. |
| XS-8 | The CLI no longer names the self-hosted indexer as a dependency | MISSING_FUNCTIONALITY | `crates/cli/Cargo.toml` still names `openlore-indexer`. |
| XS-9 | check-arch refuses a write capability in a read-only Lambda mode | MISSING_FUNCTIONALITY | `indexer_search_handler_read_only` does not scan the Lambda modes yet. |
| XS-10 | check-arch refuses a signing capability in either indexer root | MISSING_FUNCTIONALITY | `scan_indexer_family_holds_no_signing` scaffold panic, on the clean control. Past it, the real tree stays RED until the Lambda root has its `resolve_only` site. |
| XS-11 | check-arch refuses a write capability in the no-publish module | MISSING_FUNCTIONALITY | `scan_lambda_no_publish_is_read_only` scaffold panic. |
| XS-12 | The Lambda environment holds no secret and no static AWS key | MISSING_FUNCTIONALITY | `lambda-env.json` does not exist. |
| OP-1 | A deploy passes every gate in order and flips live last | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-2 | A rollback returns every function to the version it replaced | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-3 | The kill switch stops search and the edge, and leaves the pass and the probe alone | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-4 | The edge is one allowlist, in front of every path | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-5 | A crashed pass is not retried, one pass runs at a time, and the index keeps a week of history | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-6 | At most three searches run at once, and a stop survives an apply | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-7 | A republish carries the image already live, whatever the next version would have held | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-8 | Restoring the index copies the chosen version back once | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-9 | Maintenance switches change one function and nothing else | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-10 | Both schedules send an empty event, which the pass and the probe accept | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-17 | The status report looks for the search error metric where a test put it | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E1 | A deploy names one exact build or it is refused before any tool runs | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E2 | An image that cannot run the pass is refused before the registry sees it | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E3 | A pass gate that fails stops the deploy with nothing flipped | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E4 | A pass gate that reached nobody stops the deploy unless the operator accepts it | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E5 | A rollback to a version whose image was removed is refused | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E6 | Resetting the index needs the exact typed word | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E7 | A pass gate that does not say it ran as no-publish stops the deploy | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E8 | Accepting a gate that reached nobody accepts nothing else | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E9 | A probe gate that finds the index not live stops the deploy unless accepted | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E10 | A rollback with no recorded version is refused | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E11 | A republish that does not hold the image already live is refused | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E12 | Restoring the index is refused when a pass is due or running | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E13 | A gate waits for a running pass or probe, and gives up with nothing flipped | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E14 | A republish of an image that left the registry is refused | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E15 | A search that fails at the public edge after its flip goes straight back | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E16 | A candidate that fails its checks stops the deploy before any gate | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-11 | Each operator action sends exactly its own event | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-12 | No Lambda operation reaches the shared host | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-13 | Standby stops what it names and leaves the rest | MISSING_FUNCTIONALITY | `deploy.sh` has no `standby` verb: it prints usage. |
| OP-14 | Nothing runs on a schedule until the operator turns it on | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-15 | The registry keeps the newest 10 builds | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-16 | The probe watches the same address before and after the cut-over | MISSING_FUNCTIONALITY | `deploy/tofu/environments/prod-indexer` does not exist. |
| OP-E17 | A republish that cannot read the live image is refused, whatever the record says | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E18 | A copy that would push out an image in use is refused | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E19 | A failed deploy removes the copy it made, and only that | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E20 | A deploy stopped before the flip puts its candidate back | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E21 | The error alarm drill waits out a busy probe | MISSING_FUNCTIONALITY | `deploy.sh` has no `lambda` verb: it prints usage and exits 2. |
| OP-E22 | Standby needs to be told what to stop | MISSING_FUNCTIONALITY | `deploy.sh` has no `standby` verb: it prints usage. |
