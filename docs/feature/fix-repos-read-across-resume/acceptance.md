# Acceptance brief: fix\-repos\-read\-across\-resume

## Slices

### S1

##### Covers

P1

```text
Priya is signed in with her GitHub account verified and has chosen three of her repos. She presses Scan my repos; GitHub reports its request budget low after the first repo is read, so the run pauses. She then presses Resume scan on her queue, and the run that picks the plan up reads the other two repos and ends completed.
```

```text
The run she picked up reads the two repos the paused run had not read, and her queue status region carries exactly one line accounting for repos read, saying three repos read.
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
A scan picked up a fourth time, or a plan of more than three repos, is not observed here: the chain stops at three runs over three repos, the bound this slice walks.
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
The repos it reads now and the repos it gives up on are repos of the plan it picked up, in that plan order, each at most once.
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

None.
