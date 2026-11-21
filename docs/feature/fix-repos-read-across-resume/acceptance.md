# Acceptance brief: fix\-repos\-read\-across\-resume

## Slices

None.

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
