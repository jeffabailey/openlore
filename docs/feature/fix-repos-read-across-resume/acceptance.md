# Acceptance brief: fix\-repos\-read\-across\-resume

## Slices

### S1

##### Covers

P1

```text
Priya is signed in with her GitHub account verified and has chosen three of her repos. She presses Scan my repos; GitHub reports its request budget low after the first repo is read, so the run pauses. She then presses Resume scan on her queue, and the run that picks the plan up reads the other two repos and ends completed.
```

```text
The run she picked up reads the two repos the paused run had not read, her store saves both of those reads, and her queue status region carries exactly one line accounting for repos read, saying three repos read.
```

```text
She sees that her scan read all three of the repos she chose, although it stopped part way and had to be picked up again.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_scan\_picked\_up\_after\_a\_pause\_accounts\_for\_the\_repos\_of\_the\_whole\_scan

```text
A count that was right after one pause but fell back to the last run's reads after a second pause would still pass this one; the scan picked up twice is what tells them apart.
```

###### Obligations





##### Covers

P1

P2

```text
Priya chose three of her repos. Her scan reads one and pauses because GitHub's request budget is used up; she picks it up and it reads a second repo before the budget runs out again; she picks it up once more and it reads the last repo and finishes.
```

```text
Each of the three runs read a repo none of the others read, the three she chose between them, and her queue carries exactly one line accounting for repos read, saying three repos read.
```

```text
However often her scan had to stop and be picked up, it tells her it read all three of the repos she chose, and it never spends her GitHub budget reading one of them again.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_scan\_picked\_up\_twice\_accounts\_for\_every\_repo\_it\_read\_along\_the\_way

```text
Here every account passes its check and every source stays listed, so a read dropped because its account failed or its source was removed would still pass this journey; the law over those plans is observed by the property that keeps the reads the scan already made.
```

###### Obligations



##### Covers

P2

```text
A run picks up a paused plan that holds a repo this scan already read, whatever its GitHub accounts answer to their checks now and whichever of its sources the owner has removed.
```

```text
No repo the scan has already read is among the repos that run reads now.
```

```text
A repo her scan already read is not read again, so her GitHub budget is spent only on repos still to read.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_picking\_a\_paused\_scan\_up\_does\_not\_read\_a\_repo\_the\_scan\_already\_read

```text
A run that read one repo twice within a single run would still pass: this law is about the repos earlier runs of the scan already read. Observed red on the wrong behaviour it names (a run that takes on every row of the plan that is not given up) before its green was relied on.
```

###### Obligations



##### Covers

P2

```text
A run picks up a paused plan holding a repo this scan will never read and a repo whose source the owner removed meanwhile.
```

```text
The repos it keeps as its own plan and the repos it gives up on are repos of the plan it picked up, in that plan order, each at most once.
```

```text
However often her scan is picked up, it never grows past the choice it started with, so no repo of it can be counted twice.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_picks\_up\_repos\_of\_the\_plan\_it\_was\_given\_and\_no\_others

```text
A run that took on too few repos, dropping one still to read, would still pass: this law forbids a plan growing, not a plan losing a row.
```

###### Obligations



##### Covers

P1

```text
Every member of the workspace is checked for where the rule deciding what a picked-up scan keeps lives, and where the store's SQL lives.
```

```text
The crate holding that rule reaches no crate that reads or writes anything outside itself, no adapter depends on another adapter, and only a composition root depends on an adapter.
```

```text
The number her queue shows means one thing wherever it is shown, because the rule behind it has one home and the store has another.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs \(cargo xtask check\-arch\)

```text
A crate that reached an adapter's data only through the shared contract crate would still pass: the check follows crate dependencies, not the shape of the data they carry.
```

###### Obligations


##### Covers

P1

```text
A forget-me purge deletes the owner's run between the start of that run and the moment it writes its plan, and the plan is written after it.
```

```text
No plan row is left for the run that is gone, and her store holds no row naming her.
```

```text
A scan she asked to be forgotten leaves nothing behind that a later count could name.
```

##### Disposition

Tested

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:\:a\_purge\_between\_scan\_start\_and\_plan\_write\_leaves\_no\_rows

```text
A writer that also refused rows for a run her store does hold would still pass this one: that a held run keeps its plan is observed by the journeys, through the count they read.
```

###### Obligations


##### Covers

P1

P2

```text
A run picks up a paused plan holding repos this scan already read, over any mix of its accounts passing, failing or not finishing their checks now, and any of its sources the owner has removed since - including the account of a repo already read, whose check fails now and which she has removed.
```

```text
The run keeps as already read exactly the repos of that plan this scan already read, neither more nor fewer.
```

```text
What her scan already read stays counted, even when she has since removed the account it came from or that account no longer checks out.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_picking\_a\_paused\_scan\_up\_keeps\_every\_repo\_the\_scan\_already\_read

```text
A run that kept those reads and then wrote them to the store wrongly would still pass: what reaches her queue is observed by the two journeys. Shown red on the regression it names - a read dropped when its source was removed - before its green was relied on.
```

###### Obligations



##### Covers

P1

```text
Priya chose three of her repos. Her scan reads one of them and pauses because GitHub's request budget is used up. Her store then stops saving that a repo was read; she picks the run up and it reads a second repo, cannot save that read and ends interrupted. Her store saves again and she picks the run up once more.
```

```text
The last run reads only the repos still owed and never the one her scan had already saved, and her queue carries exactly one line accounting for repos read, saying three repos read, although that last run read two.
```

```text
She sees that her scan read all three of her repos even though the run that finished read only two of them, and her GitHub budget is not spent reading again what her scan had already saved.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_cut\_short\_keeps\_what\_it\_saved\_and\_the\_run\_after\_it\_reads\_only\_the\_rest

```text
A count that was right after one interruption but fell back to the last run's reads after a second one would still pass this one; the repos read again across two pick-ups are observed by the scenario beside it. Shown red on a resume that drops the reads its plan holds, before its green was relied on.
```

###### Obligations



##### Covers

P2

```text
Priya chose three of her repos and her store cannot save that a repo was read. She scans; her store saves again and she picks the run up, which reads the lost repo again before the budget runs out; her store stops saving reads once more and she picks the run up, which reads a second repo and loses it; her store saves again and she picks the run up for the last time.
```

```text
Two of her repos were read by more than one run, five reads in all, and her queue carries exactly one line accounting for repos read, saying three repos read.
```

```text
However often her scan had to read one of her repos again, the queue tells her how many of her repos were read, not how many reads it took.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:repos\_her\_scan\_had\_to\_read\_again\_are\_counted\_once\_each

```text
A scan whose plan holds more than three repos, or which loses a read more than twice, is not observed here. Shown red on a resume that drops the reads its plan holds, before its green was relied on.
```

###### Obligations



##### Covers

P1

```text
Priya chose three of her repos and her scan read all three and finished, her queue saying three repos read. She then unticks one of them, leaving two, and scans again.
```

```text
The new scan reads the two repos she had chosen by then, and her queue carries exactly one line accounting for repos read, saying two repos read, not the five her two scans read between them.
```

```text
The number on her queue is about the scan she is looking at: her new scan's count does not carry her earlier scan's reads.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_new\_scan\_accounts\_for\_its\_own\_repos\_only

```text
A count that mixed in the reads of another owner's scan would still pass: only her own two scans are observed here. Shown red on a count that adds the reads of her earlier scans (it then said five repos read where the scan read two), before its green was relied on.
```

###### Obligations


##### Covers

P1

```text
Priya chose two of her repos, and her store saves the first read a run makes and refuses the ones after it. She scans: that run reads her first repo and saves it, reads the second and cannot save it, and ends unfinished. Her store saves her reads again and she picks the run up.
```

```text
While the run is unfinished her queue accounts for no repos read. The run she picks up reads only the repo still owed, never the one her scan had already saved, and her queue then carries exactly one line accounting for repos read, saying two repos read, although that run read one.
```

```text
A scan stopped by a failure, not by GitHub's budget, still tells her about the repo it had read before it stopped, and does not read it again.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_cut\_short\_after\_saving\_a\_read\_of\_its\_own\_keeps\_it\_for\_the\_run\_after\_it

```text
A scan of more than two repos, where the run picking the plan up is itself cut short, is not observed here. Shown red on a resume that drops the reads its plan holds (the queue then said one repo read where the scan read two), before its green was relied on.
```

###### Obligations



##### Covers

P1

```text
Every source file of the review app and of her store is checked for the names of its development-only faults, the one that saves a run's first read and refuses the rest included.
```

```text
Each of those names and each store fault variant appears only behind a gate that a release build switches off.
```

```text
The build she is served can be asked to lose a read by no one: the seams that make her store refuse a write exist only in a development build.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs \(cargo xtask check\-arch\)

```text
A name reachable through a value built at run time, rather than written in the source, would still pass: the check reads the source trees of the app and the store.
```

###### Obligations


##### Covers

P1

```text
Her store is asked to save the first read of a run and refuse the ones after it, and one run saves a read of its first repo and then of its second.
```

```text
The first of those reads is saved and the second is refused, and the refused repo stays on the plan as still to read.
```

```text
A read her store could not save is not counted and is read again later, so she is never told of a repo whose reading was lost.
```

##### Disposition

Tested

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:\:a\_run\_saves\_its\_first\_mark\_and\_is\_refused\_the\_next

```text
A store that refused the first read of the NEXT run as well would still pass this one; the run after it is observed by the row beside this.
```

###### Obligations


##### Covers

P1

```text
With that same request held, the run after it saves a read of its own, and both runs write their plan and their account checks.
```

```text
The first read of the run after it is saved again, its later read is refused, and every plan write and account-checks write of both runs is accepted.
```

```text
Only the saving of a read is ever lost while her store is asked to lose it: her scan's plan and its account checks are kept as always, and each run can still save a read.
```

##### Disposition

Tested

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:\:the\_run\_after\_it\_saves\_its\_own\_first\_mark

```text
A store that counted the runs of two different owners together would still pass: one owner's two runs are observed here.
```

###### Obligations


## Expectations

### Rows

#### E1

##### Promises

P1

```text
A user whose scan stopped because the GitHub request budget was used up, and who then starts a run that resumes that plan, sees the queue report a "repos read" count that covers the repos read by the stopped run plus the repos read by the resuming run, not only the resuming run.
```

```text
The promise says nothing about when the number becomes visible or what the queue shows while a run is still reading, so a user watching mid-run cannot tell a count that is merely not refreshed yet from a count that lost the earlier run.
```

None.

None.

```text
A public observation of the queue output after a budget-exhausted run and again after the resuming run, with the repos read by each run known from the public output.
```

#### E2

##### Promises

P1

```text
The same whole-scan count is reported when the earlier run ended by interruption rather than by a used-up budget: after a resume of an interrupted scan, the queue reports the repos read by the interrupted run plus those read by the resuming run.
```

```text
The promise does not say which stops count as "interrupted" for the user (a deliberate cancel, a crash, a lost machine), so a user cannot predict from the promise whether the count survives an abrupt kill.
```

None.

None.

```text
A public observation of the queue after a run is interrupted and after a later run resumes the same plan.
```

#### E3

##### Promises

P1

```text
When a scan is resumed more than once, the queue count keeps accumulating over every run: after the third run the user sees the total of the paused run and all runs that resumed it, not just the two most recent runs.
```

```text
The promise does not say what the user sees when a resuming run reads no repo at all, so a count that did not move cannot be told apart from a count that was reset and rebuilt.
```

None.

None.

```text
A public observation of the queue after each of at least three runs of one scan plan.
```

#### E4

##### Promises

P1

```text
The number the queue reports counts repos of that scan plan only, so a user who compares it with the size of the plan sees a number that stays within the plan and does not grow with work from other scans or other plans.
```

```text
The promise does not say whether a repo whose read failed or returned an error counts as read, so a user seeing a count below the plan size cannot tell an unread repo from a failed one.
```

None.

None.

```text
A public observation of the queue count together with the public plan size for the same scan.
```

#### E5

##### Promises

P2

```text
When a resuming run reads a plan repo a second time because the earlier read was not saved, the whole-scan count the queue reports rises by one for that repo, never by two.
```

```text
The promise reports only a number, so the duplicated work itself (the time and the GitHub requests spent reading the same repo twice) stays invisible to the user.
```

None.

None.

```text
A public observation of a scan in which a read by the paused run was not saved and the resuming run reads that repo again, with the queue count before and after.
```

#### E6

##### Promises

P2

```text
However many runs repeat a read, the number stays a count of repos and not a count of read attempts: a user who sees the queue after several resumes that re-read the same repos sees the number of different plan repos read, not the sum of the reads.
```

```text
The promise gives the user no way to see which repos were re-read, so a correct larger count and an over-count caused by double counting look the same in the queue output.
```

None.

None.

```text
A public observation of a scan with at least two repos re-read across at least two resumes, with the identities of the repos read in each run visible in public output.
```

## Charter

None.

## Examination

### E1

met

#### Observations

```text
Budget pause then one resume, examination 3 (across-resume scenario a_scan_picked_up_after_a_pause_accounts_for_the_repos_of_the_whole_scan, suite exit 0). Queue right after the pause: latest run stored with status rate_limited; status region verbatim, one line only, "GitHub is busy, resuming at 02:05 UTC. The suggestions found so far are kept, and the scan picks up again on its own then."; the repos-read line ABSENT, the queue saying neither "repos read" nor "repo read" anywhere; "Resume scan" OFFERED. Repos read by the paused run: priyaraman/tidepool, 1 in all. Repos read by the run she picked up: priyaraman/kelp and priyaraman/quill-docs, 2 in all, neither of them tidepool. Queue after that run: status completed; status region verbatim two lines, "Ownership re-checked: priyaraman - 2 new suggestions" and "3 repos read"; the repos-read line PRESENT exactly once reading "3 repos read"; "Resume scan" not offered, the scan action offered is "Scan again". 1 + 2 = 3; a count of the resuming run alone would have read "2 repos read", so the budget-paused run's read survives into the number.
```

```text
Examination 3 capture boundary. nwdes examine read on this workspace lists an empty public directory: no public document is projected, so the probe is the only public surface. nwdes examine probe admits only the candidate's declared console scripts; every capture below comes from the declared examiner driver review-app-oracle, which nwdes launched by path after the system refused its descriptor launch (Permission denied) and re-verified the same file by device, inode, size and modification time. The driver accepts only three fixed suites: across-resume (now 11 listed items, 9 of them scan scenarios plus 2 state-delta self-checks), store-failure (4) and legacy-run (6). All three ran to exit 0 (11 passed, 4 passed, 6 passed, 0 failed each). The examiner cannot author a new scan scenario, so the capturable public events are exactly those these suites print. Two new across-resume scenarios exist that the previous examination round did not have: a_new_scan_accounts_for_its_own_repos_only and a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it. Noise printed by the suites (a deliberate should-panic state-delta self-check and proptest source-path warnings) carries no queue text and bears on no expectation.
```

### E2

met

#### Observations

```text
Interrupted run whose own saved read survives, examination 3 (across-resume scenario a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it, suite exit 0). Run 1, printed as "her scan, her store saving only the first read it made", read priyaraman/quill-docs and priyaraman/tidepool, 2 in all; one of those two reads was saved and the other was not. Queue right after it: latest run stored with status interrupted; status region verbatim, one line only, "Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it."; the repos-read line ABSENT, the queue saying neither "repos read" nor "repo read" anywhere; "Resume scan" OFFERED. The run she picked up read priyaraman/quill-docs, 1 in all. Queue after it: status completed; status region verbatim two lines, "Ownership re-checked: priyaraman - 0 new suggestions" and "2 repos read"; the repos-read line PRESENT exactly once reading "2 repos read"; "Resume scan" not offered, "Scan again" offered. The resuming run read 1 repo, so a count of the resuming run alone would have read "1 repo read"; the number 2 can only be the interrupted run's own surviving saved read (priyaraman/tidepool, the read the resuming run did not repeat) plus the one repo the resuming run read. The interrupted run's read therefore survives into the whole-scan number after an interruption that was not a budget pause (status interrupted, not rate_limited).
```

```text
Examination 3 capture boundary. nwdes examine read on this workspace lists an empty public directory: no public document is projected, so the probe is the only public surface. nwdes examine probe admits only the candidate's declared console scripts; every capture below comes from the declared examiner driver review-app-oracle, which nwdes launched by path after the system refused its descriptor launch (Permission denied) and re-verified the same file by device, inode, size and modification time. The driver accepts only three fixed suites: across-resume (now 11 listed items, 9 of them scan scenarios plus 2 state-delta self-checks), store-failure (4) and legacy-run (6). All three ran to exit 0 (11 passed, 4 passed, 6 passed, 0 failed each). The examiner cannot author a new scan scenario, so the capturable public events are exactly those these suites print. Two new across-resume scenarios exist that the previous examination round did not have: a_new_scan_accounts_for_its_own_repos_only and a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it. Noise printed by the suites (a deliberate should-panic state-delta self-check and proptest source-path warnings) carries no queue text and bears on no expectation.
```

### E3

met

#### Observations

```text
More than one resume of one plan, examination 3 (across-resume, suite exit 0). Scenario a_scan_picked_up_twice_accounts_for_every_repo_it_read_along_the_way: queue after the pause shows status rate_limited, the "GitHub is busy, resuming at 02:05 UTC" line, the repos-read line ABSENT and "Resume scan" OFFERED; the paused run read priyaraman/tidepool (1), the first pick-up read priyaraman/quill-docs (1), the second pick-up read priyaraman/kelp (1); queue after the third run: status completed, status region verbatim "Ownership re-checked: priyaraman - 1 new suggestion" and "3 repos read", repos-read line PRESENT reading "3 repos read", "Scan again" offered. Three runs, three different repos, number 3: the two most recent runs together read only 2 and the last run alone read 1, so the number accumulates over every run of the scan. Scenario a_run_cut_short_keeps_what_it_saved_and_the_run_after_it_reads_only_the_rest gives a queue capture after each of three runs of one plan: run 1 read priyaraman/tidepool (1) and left status rate_limited with the repos-read line ABSENT; run 2 read priyaraman/quill-docs (1) with its read unsaved and left status interrupted, the line "Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it.", the repos-read line ABSENT and "Resume scan" OFFERED; run 3 read priyaraman/kelp and priyaraman/quill-docs (2) and the queue then read "3 repos read" with status completed.
```

```text
Examination 3 capture boundary. nwdes examine read on this workspace lists an empty public directory: no public document is projected, so the probe is the only public surface. nwdes examine probe admits only the candidate's declared console scripts; every capture below comes from the declared examiner driver review-app-oracle, which nwdes launched by path after the system refused its descriptor launch (Permission denied) and re-verified the same file by device, inode, size and modification time. The driver accepts only three fixed suites: across-resume (now 11 listed items, 9 of them scan scenarios plus 2 state-delta self-checks), store-failure (4) and legacy-run (6). All three ran to exit 0 (11 passed, 4 passed, 6 passed, 0 failed each). The examiner cannot author a new scan scenario, so the capturable public events are exactly those these suites print. Two new across-resume scenarios exist that the previous examination round did not have: a_new_scan_accounts_for_its_own_repos_only and a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it. Noise printed by the suites (a deliberate should-panic state-delta self-check and proptest source-path warnings) carries no queue text and bears on no expectation.
```

### E4

met

#### Observations

```text
A second scan of the same user, examination 3 (across-resume scenario a_new_scan_accounts_for_its_own_repos_only, suite exit 0). Her first scan read priyaraman/kelp, priyaraman/quill-docs and priyaraman/tidepool, 3 in all; queue after it: status completed, status region verbatim "Ownership re-checked: priyaraman - 6 new suggestions" and "3 repos read", repos-read line PRESENT reading "3 repos read". Her second scan then read priyaraman/quill-docs and priyaraman/tidepool, 2 in all, both of them repos her first scan had already read; queue after it: status completed, status region verbatim "Ownership re-checked: priyaraman - 0 new suggestions" and "2 repos read", repos-read line PRESENT reading "2 repos read", "Resume scan" not offered, "Scan again" offered. The second scan's number is 2, the size of its own plan work, and not 5 (the sum of both scans' reads) and not 3 (the first scan's number carried over). Two scans of one user existing one after the other therefore do not add their reads together in the queue number.
```

```text
The reported number against the plan, examination 3 (legacy-run suite, exit 0, 6 passed, read together with the across-resume and store-failure scans above). a_scan_that_read_the_repos_it_planned_says_how_many: the plan is the five repos she chose and the queue reads "5 repos read" with status completed. a_scan_that_could_read_none_of_the_repos_it_planned_still_says_so: the plan is the one repo she chose and the queue reads "0 repos read" together with "Skipped priyaraman/kelp: archived since you chose it. Untick it, or unarchive it on GitHub." and "We didn't find suggestions in your owned, public repos. Forks and archived repos are skipped." a_run_kept_before_scan_plans_were_stored_says_nothing_about_repos_read and its no-account variant: status completed but the repos-read line ABSENT, the queue saying neither "repos read" nor "repo read" anywhere. The across-resume three-repo plans report "3 repos read" and the two-repo second scan reports "2 repos read"; every repo name printed in any read list (priyaraman/tidepool, kelp, quill-docs, estuary) belongs to the plan of that scan. In every captured scan the reported number lies between 0 and the plan size and never exceeds it. The plan-confinement scenarios a_run_picks_up_repos_of_the_plan_it_was_given_and_no_others, a_run_picking_a_paused_scan_up_keeps_every_repo_the_scan_already_read and a_run_picking_a_paused_scan_up_does_not_read_a_repo_the_scan_already_read each print one paused scan as a numbered list of plan entries with their state (StillToRead, AlreadyRead, Skipped) and source, the account checks, the sources she no longer lists, and then "the run keeps as already read", "the run reads now" and "the run gives up on" as index lists (printed example: a 10-entry plan, keeps [3, 4, 5], reads now [6, 9], gives up on [2]). In every printed case the kept, read and given-up indices are indices of that same plan only, and no index marked AlreadyRead appears in "reads now". Whether a repo whose read failed counts as read is not told apart from an unread repo by any capture: only the archived-skip case prints a reason beside a number below the plan size.
```

```text
Examination 3 capture boundary. nwdes examine read on this workspace lists an empty public directory: no public document is projected, so the probe is the only public surface. nwdes examine probe admits only the candidate's declared console scripts; every capture below comes from the declared examiner driver review-app-oracle, which nwdes launched by path after the system refused its descriptor launch (Permission denied) and re-verified the same file by device, inode, size and modification time. The driver accepts only three fixed suites: across-resume (now 11 listed items, 9 of them scan scenarios plus 2 state-delta self-checks), store-failure (4) and legacy-run (6). All three ran to exit 0 (11 passed, 4 passed, 6 passed, 0 failed each). The examiner cannot author a new scan scenario, so the capturable public events are exactly those these suites print. Two new across-resume scenarios exist that the previous examination round did not have: a_new_scan_accounts_for_its_own_repos_only and a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it. Noise printed by the suites (a deliberate should-panic state-delta self-check and proptest source-path warnings) carries no queue text and bears on no expectation.
```

### E5

met

#### Observations

```text
Repeated reads counted once each, examination 3. (a) across-resume scenario repos_her_scan_had_to_read_again_are_counted_once_each (suite exit 0): run 1, printed as "her first scan, her store saving no read", read priyaraman/tidepool (1); the first run she picked up read priyaraman/tidepool again (1); the second read priyaraman/quill-docs (1); the last read priyaraman/kelp and priyaraman/quill-docs (2). That is 5 reads of 3 different plan repos, with two different repos re-read (tidepool in the first resume, quill-docs in the last) across two different resumes, each run's repo identities printed run by run. Queue after the last run: status completed; status region verbatim "Ownership re-checked: priyaraman - 1 new suggestion" and "3 repos read"; repos-read line PRESENT reading "3 repos read"; "Scan again" offered. The number is the count of different repos (3), not the count of reads (5) and not 4. (b) store-failure scenario a_scan_that_cannot_save_that_it_read_a_repo_is_offered_for_resuming (suite exit 0, 4 passed): the scan her store cut short read priyaraman/tidepool (1) and that read was not saved; queue then: status interrupted, the interrupted line only, repos-read line ABSENT, "Resume scan" OFFERED; a further pick-up with the store still refusing a write read no repo at all (0 in all) and left the same interrupted queue with no repos-read line; the run she finally picked up read priyaraman/estuary, priyaraman/quill-docs and priyaraman/tidepool (3), re-reading tidepool because the first read was not saved; queue then: status completed, "Ownership re-checked: priyaraman - 1 new suggestion" and "3 repos read", repos-read line PRESENT reading "3 repos read". Four reads, three different plan repos, number 3: the re-read repo raised the number by one and not by two, which would have shown "4 repos read". In every capture the number before a resume is not shown at all (a paused or interrupted queue carries no repos-read line), so the rise is observable only as the completed scan's number against the printed per-run repo identities.
```

```text
Interrupted run whose own saved read survives, examination 3 (across-resume scenario a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it, suite exit 0). Run 1, printed as "her scan, her store saving only the first read it made", read priyaraman/quill-docs and priyaraman/tidepool, 2 in all; one of those two reads was saved and the other was not. Queue right after it: latest run stored with status interrupted; status region verbatim, one line only, "Your last scan was interrupted. The suggestions found so far are kept; press Resume scan to finish it."; the repos-read line ABSENT, the queue saying neither "repos read" nor "repo read" anywhere; "Resume scan" OFFERED. The run she picked up read priyaraman/quill-docs, 1 in all. Queue after it: status completed; status region verbatim two lines, "Ownership re-checked: priyaraman - 0 new suggestions" and "2 repos read"; the repos-read line PRESENT exactly once reading "2 repos read"; "Resume scan" not offered, "Scan again" offered. The resuming run read 1 repo, so a count of the resuming run alone would have read "1 repo read"; the number 2 can only be the interrupted run's own surviving saved read (priyaraman/tidepool, the read the resuming run did not repeat) plus the one repo the resuming run read. The interrupted run's read therefore survives into the whole-scan number after an interruption that was not a budget pause (status interrupted, not rate_limited).
```

### E6

met

#### Observations

```text
Repeated reads counted once each, examination 3. (a) across-resume scenario repos_her_scan_had_to_read_again_are_counted_once_each (suite exit 0): run 1, printed as "her first scan, her store saving no read", read priyaraman/tidepool (1); the first run she picked up read priyaraman/tidepool again (1); the second read priyaraman/quill-docs (1); the last read priyaraman/kelp and priyaraman/quill-docs (2). That is 5 reads of 3 different plan repos, with two different repos re-read (tidepool in the first resume, quill-docs in the last) across two different resumes, each run's repo identities printed run by run. Queue after the last run: status completed; status region verbatim "Ownership re-checked: priyaraman - 1 new suggestion" and "3 repos read"; repos-read line PRESENT reading "3 repos read"; "Scan again" offered. The number is the count of different repos (3), not the count of reads (5) and not 4. (b) store-failure scenario a_scan_that_cannot_save_that_it_read_a_repo_is_offered_for_resuming (suite exit 0, 4 passed): the scan her store cut short read priyaraman/tidepool (1) and that read was not saved; queue then: status interrupted, the interrupted line only, repos-read line ABSENT, "Resume scan" OFFERED; a further pick-up with the store still refusing a write read no repo at all (0 in all) and left the same interrupted queue with no repos-read line; the run she finally picked up read priyaraman/estuary, priyaraman/quill-docs and priyaraman/tidepool (3), re-reading tidepool because the first read was not saved; queue then: status completed, "Ownership re-checked: priyaraman - 1 new suggestion" and "3 repos read", repos-read line PRESENT reading "3 repos read". Four reads, three different plan repos, number 3: the re-read repo raised the number by one and not by two, which would have shown "4 repos read". In every capture the number before a resume is not shown at all (a paused or interrupted queue carries no repos-read line), so the rise is observable only as the completed scan's number against the printed per-run repo identities.
```

```text
The reported number against the plan, examination 3 (legacy-run suite, exit 0, 6 passed, read together with the across-resume and store-failure scans above). a_scan_that_read_the_repos_it_planned_says_how_many: the plan is the five repos she chose and the queue reads "5 repos read" with status completed. a_scan_that_could_read_none_of_the_repos_it_planned_still_says_so: the plan is the one repo she chose and the queue reads "0 repos read" together with "Skipped priyaraman/kelp: archived since you chose it. Untick it, or unarchive it on GitHub." and "We didn't find suggestions in your owned, public repos. Forks and archived repos are skipped." a_run_kept_before_scan_plans_were_stored_says_nothing_about_repos_read and its no-account variant: status completed but the repos-read line ABSENT, the queue saying neither "repos read" nor "repo read" anywhere. The across-resume three-repo plans report "3 repos read" and the two-repo second scan reports "2 repos read"; every repo name printed in any read list (priyaraman/tidepool, kelp, quill-docs, estuary) belongs to the plan of that scan. In every captured scan the reported number lies between 0 and the plan size and never exceeds it. The plan-confinement scenarios a_run_picks_up_repos_of_the_plan_it_was_given_and_no_others, a_run_picking_a_paused_scan_up_keeps_every_repo_the_scan_already_read and a_run_picking_a_paused_scan_up_does_not_read_a_repo_the_scan_already_read each print one paused scan as a numbered list of plan entries with their state (StillToRead, AlreadyRead, Skipped) and source, the account checks, the sources she no longer lists, and then "the run keeps as already read", "the run reads now" and "the run gives up on" as index lists (printed example: a 10-entry plan, keeps [3, 4, 5], reads now [6, 9], gives up on [2]). In every printed case the kept, read and given-up indices are indices of that same plan only, and no index marked AlreadyRead appears in "reads now". Whether a repo whose read failed counts as read is not told apart from an unread repo by any capture: only the archived-skip case prints a reason beside a number below the plan size.
```
