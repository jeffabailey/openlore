# Acceptance brief: fix\-repos\-read\-across\-resume

## Slices

### S1

##### Covers

P1

```text
Priya is signed in with her GitHub account verified and has chosen three of her repos. She presses Scan my repos; GitHub reports its request budget low after the first repo is read, so the run pauses. She then presses Resume scan on her queue, and the run that picks the plan up reads the other two repos and ends completed.
```

```text
Her queue status region carries exactly one line accounting for repos read, and that line says three repos read.
```

```text
She sees that her scan read all three of the repos she chose, although it stopped part way and had to be picked up again.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_scan\_picked\_up\_after\_a\_pause\_accounts\_for\_the\_repos\_of\_the\_whole\_scan

```text
A queue that counted the whole scan across one pause but fell back to the last run's count once a scan was picked up twice would still pass: this journey pauses and picks the scan up once.
```

###### Obligations



##### Covers

P1

```text
A scan is paused with part of its plan already read, and a run picks that plan up, with any mix of its account checks passing, failing or unfinished and any of its sources removed meanwhile.
```

```text
The run keeps as already read exactly the repos of that plan an earlier run read, neither more nor fewer.
```

```text
What her scan already read stays accounted for, whatever changed while it was paused.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_run\_picking\_a\_paused\_scan\_up\_keeps\_every\_read\_it\_already\_made

```text
A run that keeps those reads and then loses them when it writes its plan would still pass this one: what the store keeps is observed by its own rows.
```

###### Obligations



##### Covers

P2

```text
A run picks up a paused plan that holds a repo already read whose GitHub account passes its check now.
```

```text
No repo the run keeps as already read is among the repos it reads now.
```

```text
A repo her scan already read is not read again, so it is counted once.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_repo\_already\_read\_is\_not\_read\_again\_by\_the\_run\_picking\_the\_scan\_up

```text
A run that read one repo twice within a single run would still pass: this law is about the reads carried over from an earlier run.
```

###### Obligations



##### Covers

P2

```text
A run picks up a paused plan holding a repo this scan will never read and a repo whose source the owner removed meanwhile.
```

```text
The repos the run keeps are part of the plan it picked up, in that plan order, each at most once.
```

```text
However often her scan is picked up, it never grows past the choice it started with, so no repo of it can be counted twice.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:the\_plan\_a\_run\_picks\_up\_never\_grows

```text
A run that kept too few repos, dropping one still to read, would still pass: this law forbids a plan growing, not a plan losing a row.
```

###### Obligations



##### Covers

P1

```text
A run freezes the plan of her scan with the first of its three repos already read, and the stored plan is read back.
```

```text
The plan reads back with the same three repos in the same order, the first of them still already read.
```

```text
Her store keeps which of her repos the scan has already read, so that account survives the pause.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_frozen\_plan\_keeps\_which\_of\_its\_repos\_were\_already\_read

```text
A store that kept every status it was given while the run gave it the wrong ones would still pass: what the run gives it is observed by the resume rows.
```

###### Obligations


##### Covers

P1

```text
A run freezes the plan of her scan with none of its three repos read yet, and the stored plan is read back.
```

```text
The plan reads back with all three repos still to read.
```

```text
No repo is counted as read before her scan has read it.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_frozen\_plan\_of\_repos\_none\_read\_keeps\_them\_all\_still\_to\_read

```text
A store that wrote every row as still to read, whatever it was given, would still pass this one: the already-read case is what tells them apart.
```

###### Obligations


##### Covers

P1

```text
A plan is frozen for a run her store no longer holds, while her latest run holds a plan of its own.
```

```text
No row is written for the run that is gone, and the plan of her latest run reads back empty.
```

```text
A scan she asked to be forgotten leaves nothing behind that a later count could name.
```

##### Disposition

Tested

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs\:\:a\_plan\_frozen\_for\_a\_run\_her\_store\_no\_longer\_holds\_leaves\_nothing\_behind

```text
A store that also refused rows for a run it does hold would still pass this one: the other two cases observe that a held run keeps its plan.
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
