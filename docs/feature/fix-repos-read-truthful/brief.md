# Product brief: fix\-repos\-read\-truthful

## Request

```text
Follow-up from fix-review-app-repos-link (docs/feature/fix-review-app-repos-link/rca.md:16-24): scan.rs discards mark_repo_read (scan.rs:669) and record_account_checks (scan.rs:380) errors with let _ =, so a failed store write leaves a completed run showing '0 repos read'; and the queue shows repos_read_message for any completed run (views/queue.rs:651-653), including runs stored before scan plans existed, where '0 repos read' is misleading. User decisions 2026-11-08: when a store write fails during a scan, end the run Interrupted (as an unreadable source list does, scan.rs:663), so the queue offers Resume scan and resume re-reads repos not yet marked read; for a completed run with no plan rows, hide the repos-read line.
```

## Work

S

```text
The review app's 'repos read' line is true: a scan whose store writes fail ends Interrupted and can be resumed, and a run stored before scan plans existed shows no repos-read line.
```

### Rework

None.

## Promises

### P1

Outcome

```text
A scan whose store write fails ends Interrupted and is resumable, never Completed with a wrong repos-read count.
```

### P2

Outcome

```text
A completed run with no plan rows shows no repos-read line; a planned run keeps its true count.
```

## Scope

None.

## Decisions

None.

## Journey

None.

## Slices

### S1

```text
A scan whose store write fails (marking a repo read, or recording an account's check) ends Interrupted, and the queue offers Resume scan instead of a completed '0 repos read'.
```

#### Promises

P1

#### Depends on

None.

#### Slice work

None.

### S2

```text
A completed run stored before scan plans existed (no plan rows) shows no repos-read line on the queue; a run with plan rows still shows its count.
```

#### Promises

P2

#### Depends on

None.

#### Slice work

None.
