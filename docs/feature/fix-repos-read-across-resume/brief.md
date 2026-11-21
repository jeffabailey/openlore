# Product brief: fix\-repos\-read\-across\-resume

## Request

```text
A scan reads 1 of 3 repos, pauses because the GitHub request budget is used up, and then resumes and reads the other 2. The queue then says "2 repos read", not 3. Each resume starts a new run that counts only its own reads. User decision (2026-11-21, option A): count the whole scan across its resumes (here "3 repos read"); the resumed run takes over the reads the paused run already saved.
```

## Work

M

```text
After a budget pause and resume, the queue says how many of the scan plan repos were read in the whole scan, not only in the last run
```

### Rework

None.

## Promises

### P1

Outcome

```text
When a scan pauses (GitHub request budget used up, or interrupted) and a later run resumes its plan, the queue says "N repos read" with N the count of plan repos read in the whole scan: the paused run and every run that resumed it.
```

### P2

Outcome

```text
A plan repo read again by a resume (because the earlier read was not saved) counts once, never twice.
```

## Scope

None.

```text
The words of the repos-read line, and a run kept before scan plans were stored (it still says nothing about repos read).
```

## Decisions

None.

## Journey

None.

## Slices

### S1

```text
After a budget pause then a resume that reads the rest, the queue says "3 repos read" for a 3-repo plan
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
