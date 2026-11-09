# Architecture brief: fix\-repos\-read\-truthful

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

crates\/openlore\-review\-app\/src\/scan\.rs

Extend

None.

crates\/openlore\-review\-app\/src\/config\.rs

Extend

None.

crates\/openlore\-review\-app\/src\/wiring\.rs

Extend

None.

crates\/adapter\-review\-store\/src\/scan\_runs\.rs

Extend

None.

crates\/adapter\-review\-store\/src\/lib\.rs

Extend

None.

xtask\/src\/check\_arch\.rs

Extend

None.

tests\/acceptance\/review\_app\_scan\_store\_failure\.rs

#### Verification

```text
cargo test -p openlore-review-app --test review_app_scan_store_failure
```

#### Supports

crates\/openlore\-review\-app\/Cargo\.toml

tests\/acceptance\/support\/review\_app\/app\.rs

#### Public oracle

```text
A signed-in owner starts a scan on a development build started with the review app's test-fault seam naming a refused scan write: refuse_mark_repo_read (the store refuses marking a planned repo read) or, separately, refuse_record_account_checks (the store refuses recording the run's account checks). The owner then restarts without the fault and resumes.
```

```text
The run's stored status is Interrupted, the queue offers 'Resume scan', and no completed-run 'repos read' line is shown; resuming once the store accepts writes reads the repos not yet marked read.
```

```text
The run ends Completed (or the queue shows a repos-read count) after a refused store write, or resume skips a repo whose read was never stored.
```

##### Promises

P1

#### Prefactoring

None.

#### Records

None.

### S2

#### Targets

crates\/review\-domain\/src\/views\/queue\.rs

Extend

None.

crates\/ports\/src\/review\.rs

Extend

None.

crates\/adapter\-review\-store\/src\/scan\_runs\.rs

Extend

None.

tests\/acceptance\/review\_app\_legacy\_run\_repos\_read\.rs

#### Verification

```text
cargo test -p openlore-review-app --test review_app_legacy_run_repos_read
```

#### Supports

crates\/openlore\-review\-app\/Cargo\.toml

#### Public oracle

```text
A signed-in owner whose latest completed run has no scan_run_repos rows opens the queue; and, as a control, one whose latest completed run has plan rows.
```

```text
The first queue has no 'repos read' status line; the control queue shows 'N repos read' with its true count, including '0 repos read' when every planned repo is unread.
```

```text
The legacy run's queue shows '0 repos read', or a planned run's count line disappears.
```

##### Promises

P2

#### Prefactoring

None.

#### Records

None.

## ADRs

None.
