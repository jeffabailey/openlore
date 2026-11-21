# Acceptance brief: fix\-repos\-read\-truthful

## Slices

### S1

##### Covers

P1

```text
A signed-in owner whose review store refuses to save that a repo was read starts a scan; then the store accepts those writes again and she picks the unfinished scan up from her queue.
```

```text
The scan ends interrupted, her queue offers only to resume it and claims no repos read at all; resuming reads every repo whose read was never saved, finishes, and her queue then accounts for exactly as many repos as the resumed scan read, no more and no fewer.
```

```text
She is never told a scan finished when nothing it did was saved: she sees an interrupted scan, picks that same run up rather than starting over, and gets a repos-read count she can trust.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_scan\_store\_failure\.rs\:\:a\_scan\_that\_cannot\_save\_that\_it\_read\_a\_repo\_is\_offered\_for\_resuming

```text
The scan keeps reading the rest of its planned repos after the refused save and only ends interrupted at the end of the plan, instead of stopping at the refusal. That loses no repo whose read was never saved and reports no finished run with an untrue count, so it does not break this promise.
```

###### Obligations

None.

##### Covers

P1

```text
A signed-in owner whose review store refuses to save how her GitHub accounts' checks ended starts a scan; then the store accepts those writes again and she picks the unfinished scan up from her queue.
```

```text
The scan ends interrupted, her queue offers only to resume it and claims no repos read at all; resuming reads her repos, finishes, and her queue then accounts for exactly as many repos as the resumed scan read, no more and no fewer.
```

```text
A scan whose record of her account checks could not be saved is shown to her as interrupted and resumable, not as a finished scan that read no repos, and the count she finally sees is the true one.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_scan\_store\_failure\.rs\:\:a\_scan\_that\_cannot\_save\_her\_account\_checks\_is\_offered\_for\_resuming

```text
The run ends interrupted before any repo is read, so the check that no repo whose read was never saved is skipped has nothing to compare; a scan that instead read some repos and then ended interrupted would also pass. It still loses nothing, because the resume reads the repos again and the queue then accounts for exactly those, so this promise is not broken.
```

###### Obligations

None.

### S2

##### Covers

P2

```text
A signed-in owner whose latest completed run was kept by the release before this app stored a scan plan opens her queue.
```

```text
Her queue is a finished scan's queue, shows exactly the suggestions that were kept, and says nothing at all about repos read.
```

```text
She is no longer told her scan read none of her repos when nothing about which repos it read was ever recorded.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_legacy\_run\_repos\_read\.rs\:\:a\_run\_kept\_before\_scan\_plans\_were\_stored\_says\_nothing\_about\_repos\_read

```text
The line is hidden for every run whose count is nought, plan or no plan; the planned-run-that-read-nothing case rejects that.
```

###### Obligations

None.

##### Covers

P2

```text
A signed-in owner who saved a choice of five repos, and whose latest scan read exactly those five, opens her queue.
```

```text
Her queue says five repos read, the true count of the repos that scan read.
```

```text
She still sees how many of her repos her scan read.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_legacy\_run\_repos\_read\.rs\:\:a\_scan\_that\_read\_the\_repos\_it\_planned\_says\_how\_many

```text
The count reports planned repos rather than repos read, which agrees here because all five planned repos were read; the planned-run-that-read-nothing case rejects that.
```

###### Obligations

None.

##### Covers

P2

```text
A signed-in owner who chose one repo that was archived after her choice, and whose scan then finished having read no repo, opens her queue.
```

```text
Her queue names that repo as the one it could not read and says nought repos read.
```

```text
She can tell a scan that read none of her chosen repos from a run that never recorded any, because the first still reports its count.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_legacy\_run\_repos\_read\.rs\:\:a\_scan\_that\_could\_read\_none\_of\_the\_repos\_it\_planned\_still\_says\_so

```text
The line is kept for every completed run, plan or no plan; the run kept from before scan plans were stored rejects that.
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
When a scan cannot write its results to the store, the run ends in the Interrupted state and never reports itself as Completed.
```

None.

None.

None.

None.

#### E2

##### Promises

P1

```text
An interrupted scan can be run again and resumes that run, instead of forcing the user to abandon it and start over.
```

None.

None.

None.

None.

#### E3

##### Promises

P1

```text
A run whose store write failed never shows a repos-read count that claims more repos were read than the store actually holds for that run.
```

None.

None.

None.

None.

#### E4

##### Promises

P2

```text
A completed run that had no planned repos shows no repos-read line at all, rather than a zero or placeholder count.
```

None.

None.

None.

None.

#### E5

##### Promises

P2

```text
A completed run that had planned repos still shows its repos-read line, with the true number of repos read for that run.
```

None.

None.

None.

None.

## Charter

None.

## Examination

### E1

met

#### Observations

```text
The probe admits two commands: openlore-review-app and review-app-oracle. "openlore-review-app serve --help" lists no options at all ("Usage: openlore-review-app serve"), so no fault seam, fixture sign-in or queue route is reachable by starting the server directly; the declared review-app-oracle suites are therefore the designated public surface for these scenarios. At HEAD ad7687c those suites do render what the owner sees: each scenario prints a block headed "what she sees", the stored run status, the queue status region verbatim line by line, whether the repos-read line is PRESENT or ABSENT, whether "Resume scan" is offered, and the scan actions offered.
```

```text
Probe "review-app-oracle store-failure --nocapture" (4 passed, 0 failed) printed the owner-visible result of both refused scan writes. For the store refusing to save that a repo was read, and separately for the store refusing to save her account checks, the first block reads identically on the points that matter: "the status her latest run is stored with: interrupted"; status region verbatim "| Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it."; "the repos-read line: ABSENT, her queue saying neither \"repos read\" nor \"repo read\" anywhere"; "\"Resume scan\": OFFERED"; "the scan actions her queue offers: Resume scan". No block reports the status completed for a run whose write was refused, and no repos-read count appears on either interrupted queue.
```

### E2

met

#### Observations

```text
Probe "review-app-oracle store-failure --nocapture" (4 passed, 0 failed) printed the owner-visible result of both refused scan writes. For the store refusing to save that a repo was read, and separately for the store refusing to save her account checks, the first block reads identically on the points that matter: "the status her latest run is stored with: interrupted"; status region verbatim "| Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it."; "the repos-read line: ABSENT, her queue saying neither \"repos read\" nor \"repo read\" anywhere"; "\"Resume scan\": OFFERED"; "the scan actions her queue offers: Resume scan". No block reports the status completed for a run whose write was refused, and no repos-read count appears on either interrupted queue.
```

```text
The same store-failure probe printed a second block per scenario, headed "the run she picked up over, her store saving again": "the status her latest run is stored with: completed", status region "| Ownership re-checked: priyaraman * - 5 new suggestions" then "| 3 repos read" for the refused account-checks scenario and "| Ownership re-checked: priyaraman * - 1 new suggestion" then "| 3 repos read" for the refused repo-read scenario, with "the repos-read line: PRESENT, reading \"3 repos read\"", "\"Resume scan\": not offered" and "the scan actions her queue offers: Scan again". So after the fault the owner resumes the run she had, and that picked-up run reaches completed with a repos-read count, rather than her having to abandon it and start over.
```

```text
Limits of these captures. The suites print the rendered queue and the stored run status, but never a run identifier, so the resumed run is attested by the heading "the run she picked up" rather than by a printed id; they never list which individual repos the resumed scan read, so the S1 falsifier "resume skips a repo whose read was never stored" is not directly exercised by any printed per-repo set; and the count line is compared against the scenario heading ("the five repos she chose", "none of the one repo she chose") rather than against a separately printed store count. All evidence comes from the declared acceptance suites rendering the queue, not from a live signed-in session on an installed host.
```

### E3

met

#### Observations

```text
Probe "review-app-oracle store-failure --nocapture" (4 passed, 0 failed) printed the owner-visible result of both refused scan writes. For the store refusing to save that a repo was read, and separately for the store refusing to save her account checks, the first block reads identically on the points that matter: "the status her latest run is stored with: interrupted"; status region verbatim "| Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it."; "the repos-read line: ABSENT, her queue saying neither \"repos read\" nor \"repo read\" anywhere"; "\"Resume scan\": OFFERED"; "the scan actions her queue offers: Resume scan". No block reports the status completed for a run whose write was refused, and no repos-read count appears on either interrupted queue.
```

```text
A harness negative control fires in both suites at HEAD ad7687c: review_app::state_delta::tests::assert_state_delta_fails_on_hidden_mutation panics with: state-delta assertion failed: slot "hidden.slot": implicit-unchanged violated; before="original" after="mutated", reported as "should panic ... ok". The state-delta machinery detects an unexpected state change, which supports trusting the suites PRESENT/ABSENT reports, though it exercises the harness rather than the product.
```

```text
Limits of these captures. The suites print the rendered queue and the stored run status, but never a run identifier, so the resumed run is attested by the heading "the run she picked up" rather than by a printed id; they never list which individual repos the resumed scan read, so the S1 falsifier "resume skips a repo whose read was never stored" is not directly exercised by any printed per-repo set; and the count line is compared against the scenario heading ("the five repos she chose", "none of the one repo she chose") rather than against a separately printed store count. All evidence comes from the declared acceptance suites rendering the queue, not from a live signed-in session on an installed host.
```

### E4

met

#### Observations

```text
Probe "review-app-oracle legacy-run --nocapture" (5 passed, 0 failed) printed three owner-visible queues. (a) The run kept before scan plans were stored: "the status her latest run is stored with: completed", status region exactly one line "| Ownership re-checked * - 10 new suggestions", and "the repos-read line: ABSENT, her queue saying neither \"repos read\" nor \"repo read\" anywhere". (b) Her scan having read none of the one repo she chose: completed, status region includes "| 0 repos read", "the repos-read line: PRESENT, reading \"0 repos read\"". (c) Her scan having read the five repos she chose: completed, status region includes "| 5 repos read", "the repos-read line: PRESENT, reading \"5 repos read\"". So the no-plan-rows run shows no line and no placeholder, while planned runs keep a count line whose number matches the repos the scenario had read.
```

```text
A harness negative control fires in both suites at HEAD ad7687c: review_app::state_delta::tests::assert_state_delta_fails_on_hidden_mutation panics with: state-delta assertion failed: slot "hidden.slot": implicit-unchanged violated; before="original" after="mutated", reported as "should panic ... ok". The state-delta machinery detects an unexpected state change, which supports trusting the suites PRESENT/ABSENT reports, though it exercises the harness rather than the product.
```

### E5

met

#### Observations

```text
The probe admits two commands: openlore-review-app and review-app-oracle. "openlore-review-app serve --help" lists no options at all ("Usage: openlore-review-app serve"), so no fault seam, fixture sign-in or queue route is reachable by starting the server directly; the declared review-app-oracle suites are therefore the designated public surface for these scenarios. At HEAD ad7687c those suites do render what the owner sees: each scenario prints a block headed "what she sees", the stored run status, the queue status region verbatim line by line, whether the repos-read line is PRESENT or ABSENT, whether "Resume scan" is offered, and the scan actions offered.
```

```text
Probe "review-app-oracle legacy-run --nocapture" (5 passed, 0 failed) printed three owner-visible queues. (a) The run kept before scan plans were stored: "the status her latest run is stored with: completed", status region exactly one line "| Ownership re-checked * - 10 new suggestions", and "the repos-read line: ABSENT, her queue saying neither \"repos read\" nor \"repo read\" anywhere". (b) Her scan having read none of the one repo she chose: completed, status region includes "| 0 repos read", "the repos-read line: PRESENT, reading \"0 repos read\"". (c) Her scan having read the five repos she chose: completed, status region includes "| 5 repos read", "the repos-read line: PRESENT, reading \"5 repos read\"". So the no-plan-rows run shows no line and no placeholder, while planned runs keep a count line whose number matches the repos the scenario had read.
```
