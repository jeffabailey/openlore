# Architecture brief: fix\-repos\-read\-across\-resume

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

crates\/review\-domain\/src\/repo\_scope\/resume\.rs

Extend

```text
The resume verdict names the rows already read and answers with the status each kept row carries.
```

crates\/review\-domain\/src\/repo\_scope\/mod\.rs

Extend

```text
The kept-row type is re-exported beside the rest of the resume verdict.
```

crates\/ports\/src\/review\.rs

Extend

```text
The plan writer takes rows with their status, and says so in its contract.
```

crates\/adapter\-review\-store\/src\/scan\_runs\.rs

Extend

```text
The plan-row insert writes the status it is given instead of always planned, and the mark-repo-read write names its run, so the store fault seam can count the marks of that run.
```

crates\/adapter\-review\-store\/src\/purge\.rs

Extend

```text
Its seeded plan rows pass their status to the writer.
```

crates\/openlore\-review\-app\/src\/scan\.rs

Extend

```text
The fresh plan writes planned rows and the resumed plan writes every kept row with the status the verdict gives it.
```

crates\/openlore\-review\-app\/src\/config\.rs

Extend

```text
A development build parses one more fault name, refuse_mark_repo_read_after_first, and its in-crate property table gains that row.
```

crates\/openlore\-review\-app\/src\/wiring\.rs

Extend

```text
The fault map asks the store for the new after-first scan-write fault.
```

crates\/adapter\-review\-store\/src\/lib\.rs

Extend

```text
The store carries the scan-write seam: the new development-only fault variant, the runs that already saved a mark, and the pure decision over them.
```

xtask\/src\/check\_arch\.rs

Extend

```text
The release-build guard token list gains the new fault name and the store variant it maps to.
```

xtask\/tests\/review\_app\_test\_fault\_guard\.rs

Extend

```text
The guard test covers the new name gated and ungated, so the rule stays falsifiable for it.
```

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

#### Verification

```text
cargo test -p openlore-review-app --test review_app_repos_read_across_resume -- --nocapture
cargo test -p openlore-review-app --test review_app_scan_store_failure
cargo test -p review-domain -p adapter-review-store
cargo xtask check-arch
cargo test -p xtask --test review_app_test_fault_guard
cargo test -p openlore-review-app --bins
```

#### Supports

crates\/openlore\-review\-app\/Cargo\.toml

tests\/acceptance\/support\/review\_app\/repos\_and\_accounts\.rs

tests\/acceptance\/support\/review\_app\/app\.rs

#### Public oracle

```text
Priya is signed in with her GitHub account verified and has chosen three of her repos. She presses Scan my repos; GitHub reports its request budget low after the first repo is read, so the run pauses. She then presses Resume scan on her queue, and the run that picks the plan up reads the other two repos and ends completed.
```

```text
Her queue status region carries exactly one repos-read line, and it reads three repos read.
```

```text
The line reads two repos read, counting only the run that picked the plan up; or it reads more than three, counting a repo twice; or a completed run shows no repos-read line at all.
```

##### Promises

P1

P2

#### Prefactoring

None.

#### Records

##### Decisions

```text
Resume takes over the whole frozen plan. A resuming run records every non-skipped row of the paused run plan as its own plan, each row carrying the status it already holds, so the queue counts the reads of the whole scan and not only of the last run. The alternative, a scan-chain column counted across the runs of one scan, needs a stored schema change and a distinct-repo count to avoid counting a repo twice; the takeover needs neither.
```

```text
One writer freezes the plan and its statuses. ScanRunPort record_plan takes plan rows with their status, so freezing the resuming run plan and carrying its already-read rows is one transaction. No second writer of scan_run_repos appears, and the mark-repo-read seam keeps its meaning: a read made now, by this run.
```

```text
The count stays a set of plan positions. repos_read_of keeps counting the rows of the latest run whose status is read. A repo has one row per plan, so a repo read again by a resume after an unsaved read counts once, and no counter can be incremented twice.
```

```text
The resume verdict owns each kept row status. The pure resume verdict in review-domain returns its kept rows as positions paired with the status the resuming run gives them: read for a row already read, planned for one it reads now or carries. The scan task maps those rows onto plan rows and writes them, and decides nothing about a row itself.
```

```text
Carriers and their owners. ScanPlanRow carries one plan row and its status in ports; KeptRow carries one resume verdict row in review-domain; Resume carries the whole verdict; PlanRowStatus and ScanStatus carry row and run state; ReviewStoreError carries store refusals; the per-process scan state stays inside the scan task of the review app, under one admission per owner.
```

```text
Each layer states what it owns. Domain: a frozen plan row is planned, read or skipped, and a resume maps the plan it resumes onto its own kept rows. Application: the scan task reads the planned rows in order, pauses on a low GitHub budget, and ends completed, rate limited or interrupted. Adapter: record_plan writes every kept row with its status in one transaction, mark_repo_read turns one row read, latest_scan counts the read rows of the latest run. Infrastructure: a refused or interrupted write leaves the rows the store already holds untouched and the run resumable.
```

```text
Quint checks the count across resumes. The four count invariants are checked with quint verify over a three-row plan and twelve steps, because the claim is about state carried from run to run, not a proof over types: Agda would add nothing, and TLC is needed only for a temporal claim this design does not make.
```

```text
A fault that refuses marks after the first: the development-only store fault refuse_mark_repo_read_after_first admits the first mark of each run and refuses every later one, so a run saves one read and then ends interrupted. That is the state an oracle needs to observe the whole-scan count after an interruption, and no other seam reaches it: refuse_mark_repo_read saves nothing, the GitHub seams act before the first read, and the restart sweep marks only a running run interrupted.
```

```text
The seam owns its per-run bookkeeping: the store carries one ScanWriteSeam holding the named fault and the runs that already saved a mark, and the decision stays a pure function of the fault, the write and whether this run already saved a mark, with the set held under the store lock in the effect shell. The scan-write name carries the run of a mark, which the store already has at that call site, so no caller learns anything new.
```

```text
No new ADR and no model change: ADR-091 accepts later faults of this family in the default suite and this one adds one acceptance journey of a few seconds. The Quint model already holds the refused-save transition, and a development-only seam adds no production state, so no model, no property and no checker run changes.
```

```text
A release build still knows no fault name: the new name sits inside the cfg(debug_assertions) parser and the new variant inside the development-only scan-write fault type, and the config property only_a_development_build_takes_the_test_fault_seam with the xtask guard review_app_test_fault_guard, whose token list gains both spellings, keep it so.
```

```text
The takeover ADR lives at ADR-092: the host moved and ratified this design's ADR as docs/adrs/ADR-092-review-app-resumed-scan-takes-over-plan-row-statuses.md, Accepted on the owner's 2026-11-21 decision, in the repository's ADR home; this design cites it there and keeps no second copy of it, because the key it was first written under names no ADR any more.
```

##### Constraints

```text
A scan plan holds at most ten repos in total (SCAN_REPO_CAP, D-RRA-16), so a resume carries at most ten rows. The authority sets no lower bound: a plan of one row is admitted, and a plan of none is a run with nothing to count.
```

```text
A plan row position and every stored count live in INTEGER columns: the adapter saturates a position or count above the signed 32-bit maximum and reads a negative one back as zero. This design keeps that existing bound and widens nothing.
```

```text
Out of product scope: the words of the repos-read line, and a run kept from before scan plans were stored, which still says nothing about repos read.
```

```text
Test budgets: each pure-core property case stays at or under 10 ms over at most 64 cases; the one acceptance scenario drives the real binary over HTTP and stays within a few seconds, the share ADR-091 already accepts for this family of review-app suites; the default cargo test suite keeps its 300 s ceiling.
```

```text
The repos-read line is shown only for a completed run, so a paused or interrupted run shows no count at all. This design does not change that, and adds no count to an unfinished run.
```

```text
One named fault per run of the binary: REVIEW_APP_TEST_FAULT names a single fault, so the journey that needs the after-first refusal and the journey that needs the budget pause are separate runs of the app, and a journey that wants the refusal to stop restarts the binary without the seam.
```

```text
Two plan repos make the refusal observable: with the fault held a run saves its first mark and is refused the next, so a resume of a two-row plan carries one read row, saves the one read it makes and ends completed with the whole-scan count; a longer plan would see the resume refused in its turn and show no count unless the binary is restarted without the seam.
```

##### Reuse

ScanPlanRow

crates\/ports\/src\/review\.rs\:555

Reuse

```text
It already pairs a plan row with its status for reads; the same shape now carries a plan row to the writer, so a plan row has one description.
```

record\_plan

crates\/ports\/src\/review\.rs\:457

Extend

```text
The single owner of plan-row writes; it takes each row status so freezing a resumed plan and its carried reads is one write.
```

mark\_repo\_read

crates\/ports\/src\/review\.rs\:465

Reuse

```text
Unchanged: it keeps its one meaning, a read this run made now, and stays the seam the store-fault scenarios refuse.
```

repos\_read\_of

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:170

Reuse

```text
It already counts the read rows of the latest run and owns the no-plan discriminator; the takeover makes its count the whole scan without changing it.
```

resume\_plan

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:126

Extend

```text
The single owner of the resume verdict; it now also names the rows already read, which no other function can decide.
```

kept

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:97

Extend

```text
It already answers which rows the resuming run keeps; it now answers with the status each kept row carries.
```

Resume

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:91

CreateNew

```text
The verdict needs a new kept-row type beside Resume, pairing a kept plan position with the status the resuming run gives it; no existing type says that, and a bare tuple would leave the meaning to the reader.
```

ResumedPlan

crates\/openlore\-review\-app\/src\/scan\.rs\:586

Extend

```text
It already maps the resumed verdict onto the rows and positions the store is given; it now carries each row status with it.
```

PlanRowStatus

crates\/ports\/src\/review\.rs\:575

Reuse

```text
The closed set planned, read and skipped already names every status a row can carry; nothing is added to it.
```

given\_scan\_paused\_part\_way

tests\/acceptance\/support\/review\_app\/repos\_and\_accounts\.rs\:1069

Reuse

```text
The paused-scan and resume steps of the review-app vocabulary already pause a plan on the GitHub budget and pick it up, so the new suite reuses them; only the three-repo choice and the reading of the count are new.
```

resume\_plan

crates\/review\-domain\/src\/repo\_scope\/mod\.rs\:51

Extend

```text
The module re-exports the resume verdict; the new kept-row type joins that list and nothing else changes there.
```

record\_plan

crates\/adapter\-review\-store\/src\/purge\.rs\:196

Extend

```text
Its seeded plan rows are built for the writer, so they pass the status each row carries.
```

TestFault

crates\/openlore\-review\-app\/src\/config\.rs\:44

Extend

```text
The closed set of fault names a development build honours already holds three; the new name joins it and the release twin still has no values.
```

store\_faults

crates\/openlore\-review\-app\/src\/wiring\.rs\:593

Extend

```text
The single owner of the name-to-store-fault map; the new name maps to the new scan-write fault and nothing else changes there.
```

ScanWriteFault

crates\/adapter\-review\-store\/src\/lib\.rs\:544

Extend

```text
The development-only set of scan writes the store may refuse; the after-first fault is one more member of it, so no second fault vocabulary appears.
```

admits

crates\/adapter\-review\-store\/src\/lib\.rs\:586

Extend

```text
The pure decision over a held fault and a write; it now also takes whether this run already saved a mark, which is the only new input the after-first fault needs.
```

ScanWrite

crates\/adapter\-review\-store\/src\/lib\.rs\:579

Extend

```text
The closed set of scan writes the seam names; the mark-repo-read member now carries the run it is for, which the store already has at that call site.
```

ScanWriteSeam

crates\/adapter\-review\-store\/src\/lib\.rs\:305

CreateNew

```text
Counting the marks of a run needs state beside the held fault; no existing type owns both, and leaving a bare set beside the fault field would let one be set without the other.
```

admits\_scan\_write

crates\/adapter\-review\-store\/src\/lib\.rs\:418

Extend

```text
The one effect-shell site that consults the seam; it names the run, reads and updates the seam state under the store lock and keeps the decision pure.
```

REVIEW\_APP\_TEST\_FAULT\_TOKENS

xtask\/src\/check\_arch\.rs\:1335

Extend

```text
The guard already lists every fault name and store variant it requires to be cfg-gated; the new pair joins that list.
```

the\_real\_tree\_passes

xtask\/tests\/review\_app\_test\_fault\_guard\.rs\:101

Extend

```text
The guard test already walks the real tree and synthetic gated and ungated shapes; the new name is covered by the same cases.
```

##### Impact

pure domain module

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:97

Changed

```text
The resume verdict names the rows already read and returns a status per kept row.
```

crate module exports

crates\/review\-domain\/src\/repo\_scope\/mod\.rs\:51

Changed

```text
The new kept-row type is re-exported beside the rest of the resume verdict.
```

port trait

crates\/ports\/src\/review\.rs\:457

Changed

```text
record_plan takes plan rows with their status.
```

store adapter

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:27

Changed

```text
The plan-row insert takes the row status instead of always writing planned; its in-crate tests move with it.
```

adapter test seed

crates\/adapter\-review\-store\/src\/purge\.rs\:196

Changed

```text
Its seed builds plan rows for record_plan and must pass their status.
```

composition root scan task

crates\/openlore\-review\-app\/src\/scan\.rs\:475

Changed

```text
The fresh plan writes planned rows and the resumed plan writes each kept row with its carried status.
```

test target manifest

crates\/openlore\-review\-app\/Cargo\.toml\:1

Changed

```text
The new acceptance target is declared there, as every review-app suite is.
```

queue view

crates\/review\-domain\/src\/views\/queue\.rs\:651

UnchangedCompatible

```text
It renders the count the store reports for a completed run; the count means more, its shape does not change.
```

port data type

crates\/ports\/src\/review\.rs\:423

UnchangedCompatible

```text
ScanRun repos_read stays an optional count with the same no-plan discriminator.
```

acceptance oracle

tests\/acceptance\/review\_app\_scan\_store\_failure\.rs\:265

UnchangedCompatible

```text
With the mark refused, no row is ever stored read, so there is nothing to take over and its counts stay exactly what the resumed run read.
```

acceptance oracle

tests\/acceptance\/review\_app\_legacy\_run\_repos\_read\.rs\:122

UnchangedCompatible

```text
A run with no plan row still reports no count, and a plan it reads to the end still counts every row.
```

domain property binding

tests\/acceptance\/review\_app\_repos\_and\_accounts\_core\.rs\:402

UnchangedCompatible

```text
It reads the to-read and skipped fields of the verdict, which keep their meaning and their shape.
```

config parser

crates\/openlore\-review\-app\/src\/config\.rs\:311

Changed

```text
The development-only parser knows one more name, the fault set one more value, and the in-crate property table one more row.
```

composition root fault map

crates\/openlore\-review\-app\/src\/wiring\.rs\:593

Changed

```text
The new name asks the store for the after-first scan-write fault.
```

store fault seam

crates\/adapter\-review\-store\/src\/lib\.rs\:544

Changed

```text
A new development-only fault variant, the seam that counts the marks of a run, and the pure decision over them.
```

store write call site

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:281

Changed

```text
The mark-repo-read write names its run when it consults the seam; the SQL it runs is unchanged.
```

check\-arch guard

xtask\/src\/check\_arch\.rs\:1335

Changed

```text
The guard requires the new name and its store variant to stay behind cfg gates.
```

guard test

xtask\/tests\/review\_app\_test\_fault\_guard\.rs\:101

Changed

```text
Its cases cover the new name, so an ungated occurrence of it is still caught.
```

acceptance support

tests\/acceptance\/support\/review\_app\/app\.rs\:49

Changed

```text
The support's fault table and its restart step name the new fault; the oracle owner writes that, not the crafter.
```

store adapter in\-crate tests

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:606

Changed

```text
They mark a repo read, so they pass the run to the seam and gain the cases of the after-first fault.
```

##### Agreement

ScanRunPort record\_plan\: the rows of a run frozen plan\, each with its status

crates\/openlore\-review\-app\/src\/scan\.rs\:475

Producer

Migrated

```text
The fresh plan passes every row as planned.
```

ScanRunPort record\_plan\: the rows of a run frozen plan\, each with its status

crates\/openlore\-review\-app\/src\/scan\.rs\:627

Producer

Migrated

```text
The resumed plan passes each kept row with the status the verdict gives it.
```

ScanRunPort record\_plan\: the rows of a run frozen plan\, each with its status

crates\/adapter\-review\-store\/src\/purge\.rs\:196

Producer

Migrated

```text
Its proptest seed builds plan rows and passes their status.
```

ScanRunPort record\_plan\: the rows of a run frozen plan\, each with its status

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:239

Consumer

Migrated

```text
The only implementor writes each row with the status it was given.
```

ScanRunPort record\_plan\: the rows of a run frozen plan\, each with its status

crates\/ports\/src\/review\.rs\:457

Consumer

Migrated

```text
The trait declares the new row shape and its documented meaning.
```

scan\_run\_repos\.status\: the stored vocabulary planned\, read and skipped

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:27

Producer

Migrated

```text
The plan-row insert now writes read as well as planned; the vocabulary itself is unchanged.
```

scan\_run\_repos\.status\: the stored vocabulary planned\, read and skipped

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:73

Consumer

UnchangedCompatible

```text
The latest-scan count still counts the rows whose status is read.
```

scan\_run\_repos\.status\: the stored vocabulary planned\, read and skipped

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:446

Consumer

UnchangedCompatible

```text
The plan reader still parses the same three values and drops an unknown one.
```

scan\_run\_repos\.status\: the stored vocabulary planned\, read and skipped

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:126

Consumer

Migrated

```text
The resume verdict now reads the read rows as well as the planned ones.
```

scan\_run\_repos\.status\: the stored vocabulary planned\, read and skipped

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:82

Consumer

UnchangedCompatible

```text
A row written by an earlier release is planned, read or skipped, the same three values; a store written before this change needs no migration.
```

REVIEW\_APP\_TEST\_FAULT\: the fault names a development build honours

tests\/acceptance\/support\/review\_app\/app\.rs\:49

Producer

Migrated

```text
The acceptance support sets the new name for the journey that needs a run to save one read and then be interrupted.
```

REVIEW\_APP\_TEST\_FAULT\: the fault names a development build honours

crates\/openlore\-review\-app\/src\/config\.rs\:311

Consumer

Migrated

```text
The development-only parser accepts the new name and still refuses every unknown one.
```

REVIEW\_APP\_TEST\_FAULT\: the fault names a development build honours

crates\/openlore\-review\-app\/src\/wiring\.rs\:593

Consumer

Migrated

```text
The map turns the new name into the after-first scan-write fault the store holds.
```

REVIEW\_APP\_TEST\_FAULT\: the fault names a development build honours

xtask\/src\/check\_arch\.rs\:1335

Consumer

Migrated

```text
The release-build guard knows the new name and requires it to stay behind a cfg gate.
```

REVIEW\_APP\_TEST\_FAULT\: the fault names a development build honours

tests\/acceptance\/review\_app\_scan\_store\_failure\.rs\:265

Producer

UnchangedCompatible

```text
It keeps setting refuse_mark_repo_read, whose meaning is unchanged.
```

##### Not applicable

contract_tests

```text
The producers and the consumers of both contracts ship in the one review-app binary and change in the same commit, so the acceptance oracle covers them. The only party that outlives a version is the stored row, and its status vocabulary is unchanged, so a store written by an earlier release reads back with the same three values.
```

prefactoring

```text
Each owner the change touches already holds its own decision: the resume verdict in review-domain, the plan write in the store adapter, the mapping in the scan task. No preserving move makes the change easier, and any move would need its own green run before the change for no gain.
```

##### Boundaries

```text
The review app HTTP port of the real binary: POST /scan starts or picks up a scan, GET /review renders the queue with its status region, GET /scan/status reports the stored run status.
```

###### Driven ports

```text
ScanRunPort and ScanPlanRead over the owner encrypted DuckDB store
```

```text
The GitHub read port, with its request budget
```

```text
The review app composition root wires the adapters to the ports; review-domain and ports hold no I O and depend on no adapter; the store adapter knows the SQL and nothing of the scan task.
```

###### Failures

####### B1

```text
The store refuses the plan write while a resume freezes its plan
```

Refusal

```text
The run is recorded interrupted, the queue offers Resume scan and shows no repos-read line; the rows the store already holds are untouched.
```

####### B2

```text
The store refuses to mark a repo read during a run
```

Refusal

```text
The run ends interrupted, the row stays planned, and a later resume reads that repo again and counts it once.
```

####### B3

```text
The GitHub request budget runs low before the next planned repo
```

Retry

```text
The run is recorded rate limited with a time to resume after, the queue offers Resume scan, and the rows read so far keep their read status for the run that picks the plan up.
```

##### Observations

```text
The repos-read line the queue shows for the owner latest run, once that run is completed
```

an optional whole number\, absent when the run recorded no plan row

```text
which repos were read, which run of the scan read them, and a plan whose rows were read by one run from a plan whose rows were read by three
```

```text
The plan of the owner latest run as the store returns it
```

the plan rows in position order\, each with its repo\, its source and its status

```text
the run that first read a row from the run that took that read over
```

```text
The stored status of the owner latest run
```

one of running\, rate limited\, interrupted\, completed and ownership failed

```text
how many of the plan rows the run reached
```

##### Equality

the queue repos\-read line

```text
Two scans are equal when the latest run of each carries the same number of plan rows with status read. The identity of the repos, and of the run that read each of them, is invisible there, so a resume that takes a read row over is equal to a single run that read that row itself.
```

##### Constructors

ScanPlanRow

ScanPlanRow \{ repo\: PlannedScanRepo\, status\: PlanRowStatus \}

KeptRow

KeptRow \{ position\: usize\, status\: PlanRowStatus \}

Resume\:\:kept

fn kept\(\&self\) \-\> Vec\<KeptRow\>

ScanRunPort\:\:record\_plan

fn record\_plan\(\&self\, owner\_did\: \&str\, run\_id\: \&str\, rows\: \&\[ScanPlanRow\]\) \-\> Result\<\(\)\, ReviewStoreError\>

##### Laws

###### L1

```text
A resuming run keeps as read exactly the rows of the plan it resumes that were already read, whatever the bio checks and the removed sources say.
```

Invariant

Domain

review\_domain\:\:repo\_scope\:\:resume\:\:resume\_plan

###### Treatment

Broad

###### Generator

```text
frozen plans of 0 to 8 rows over account and imported sources, each row planned, read or skipped; a bio check of passed, failed or unfinished per account; any listed-source set
```

8 rows\, 4 accounts\, 4 imports\, 64 cases

the native proptest seeding with its committed regression file

###### Must include

```text
a plan holding at least one read row and at least one planned row
```

one resume of one plan

```text
native proptest shrinking to the shortest refuting plan
```

```text
the positions whose frozen status is read, computed in the test from the generated plan alone
```

```text
a kept row carries planned where its frozen row was read, or carries read where its frozen row was not
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

docs\/architecture\/models\/resume\_repos\_read\.qnt

###### L2

```text
A row a resume takes over as read is never among the rows it reads now, so a repo is counted once and read once.
```

Invariant

Domain

review\_domain\:\:repo\_scope\:\:resume\:\:Resume

###### Treatment

Broad

###### Generator

```text
the same frozen plans, bio checks and listed sources
```

8 rows\, 4 accounts\, 4 imports\, 64 cases

the native proptest seeding with its committed regression file

###### Must include

```text
a plan holding a read row whose account passes its bio check now
```

one resume of one plan

```text
native proptest shrinking to the shortest refuting plan
```

```text
the intersection of the taken-over positions and the positions to read, which the test computes and asserts empty
```

```text
one position appears both as taken over and as to read
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

###### L3

```text
A resume keeps a subset of the plan it resumes, in plan order, each position at most once, so a plan never grows from one run to the next.
```

Invariant

Domain

review\_domain\:\:repo\_scope\:\:resume\:\:Resume

###### Treatment

Broad

###### Generator

```text
the same frozen plans, bio checks and listed sources
```

8 rows\, 4 accounts\, 4 imports\, 64 cases

the native proptest seeding with its committed regression file

###### Must include

```text
a plan holding a skipped row and a removed source
```

one resume of one plan

```text
native proptest shrinking to the shortest refuting plan
```

```text
the generated plan positions, against which the kept positions are checked for membership, order and uniqueness
```

```text
a kept position is outside the resumed plan, repeats, or breaks plan order
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

###### L4

```text
A plan written with its row statuses reads back with the same repos, the same order and the same statuses.
```

RoundTrip

Adapter

adapter\_review\_store\:\:scan\_runs\:\:record\_plan

###### Treatment

Finite

###### Cases

```text
a three-row plan written as read, planned, planned reads back as read, planned, planned
```

```text
a three-row plan written all planned reads back all planned
```

```text
a plan written for a run the store no longer holds writes no row and reads back empty
```

```text
The written vocabulary is the closed set planned and read over a plan bounded at ten rows, so the cases exhaust the distinctions the round trip can lose.
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

###### L5

```text
For a three-repo plan paused after one repo and resumed to its end, the queue accounts for three repos read, exactly once.
```

Invariant

Application

openlore\_review\_app\:\:scan\:\:resume\_scan

###### Treatment

Finite

###### Cases

```text
three chosen repos; the GitHub budget runs low after the first; the owner picks the run up; the resumed run reads the other two and completes; the queue status region carries exactly one repos-read line and it reads three repos read
```

```text
One public journey with one count: the finite case is the whole observable claim, and the broad laws above cover the plans the journey does not walk.
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

###### L6

```text
With the after-first fault held, the first mark of a repo read in a run is saved and every later mark of that run is refused, while every other read and write of the store behaves normally and a different run's first mark is saved again.
```

Invariant

Adapter

adapter\_review\_store\:\:ScanWriteSeam

###### Treatment

Finite

###### Cases

```text
with the fault held, one run marks its first row read and the store accepts it, then marks a second row read and the store refuses it, the refused row staying planned
```

```text
with the fault held, a second run marks its first row read and the store accepts it, and the plan write and the account-checks write of both runs are accepted
```

```text
with no fault held, one run marks two rows read and both are accepted
```

```text
The seam's domain is closed: one fault name, the two named scan writes, and the first-or-later distinction within a run, so these cases exhaust the distinctions the decision can make.
```

###### Evidence

crates\/adapter\-review\-store\/src\/scan\_runs\.rs

##### Refuted

None.

##### Claims

###### C1

```text
A plan row status is one of planned, read and skipped, so no row can be stored read without naming that status, and the count is the size of a set of plan positions, so one row cannot be counted twice.
```

```text
An over-count tells the owner that repos were read which were not, and the count is the only account she gets of what her scan did.
```

ports\:\:PlanRowStatus

```text
a stored status string read from scan_run_repos
```

PlanRowStatus\:\:parse

review\_domain\:\:repo\_scope\:\:resume\:\:resume\_plan and adapter\_review\_store\:\:scan\_runs\:\:repos\_read\_of

```text
the repos-read line of the queue
```

```text
A row stored with a status string the parser does not know is dropped by the reader, which under-counts rather than over-counts; and the SQL count over the rows lives outside the Rust types, so only the adapter tests hold it.
```

###### C2

```text
A resuming run can write only rows of the plan it resumes, because its rows are built from the kept positions of that plan and from nothing else.
```

```text
A resume that planned afresh would count repos of a plan the owner never paused, and would spend her GitHub budget on them.
```

review\_domain\:\:repo\_scope\:\:resume\:\:Resume

```text
the plan rows of the paused run, the bio checks of this resume and the sources listed now
```

Resume\:\:kept

openlore\_review\_app\:\:scan\:\:ResumedPlan

```text
the repos the queue accounts for and the repos GitHub was asked to read
```

```text
The scan task maps the kept positions onto the rows it was given; a mapping defect there is caught by the acceptance journey, not by a type.
```

###### C3

```text
A release build cannot hold any store fault, the new one included: both fault types have no values under cfg(not(debug_assertions)) and the start refuses the seam variable outright, so no release binary can refuse a real repo read.
```

```text
A release binary that honoured the fault would refuse a user's real reads and leave her scan interrupted with an untrue account of it.
```

openlore\_review\_app\:\:config\:\:TestFault

```text
the REVIEW_APP_TEST_FAULT value of the environment the binary starts in
```

openlore\_review\_app\:\:config\:\:test\_fault

openlore\_review\_app\:\:wiring\:\:store\_faults and adapter\_review\_store\:\:ScanWriteSeam

```text
a release build refusing to start while the variable is set, and holding no fault when it is not
```

```text
a binary built with debug assertions on and deployed as production: the types exclude the fault only in a release build, and the xtask guard checks the names stay cfg-gated, not which build is shipped.
```

##### States

Admitted

```text
The latest run plan carries rows read by an earlier run of the same scan, stored read.
```

openlore\-review\-app

Admitted

```text
A plan row whose read was never saved stays planned and is read again by a resume, counting once.
```

openlore\-review\-app

Admitted

```text
A plan row already read whose source the owner removed meanwhile stays read and counted: the scan did read it, and nothing more is read for that source.
```

review\-domain

Excluded

```text
A row stored read in the latest run that no run of the scan read: only marking a repo read and carrying a stored read status write that value.
```

adapter\-review\-store

Excluded

```text
A count shown for a run that is not completed, and a count at all for a run that recorded no plan row.
```

review\-domain

Unknown

```text
A run that started and whose plan write never happened, through a crash between starting the run and freezing its plan: the frozen plan of that scan is lost and the next scan plans afresh, with its own count.
```

openlore\-review\-app

Admitted

```text
A run that saved one read and then ended interrupted, its other plan rows still planned.
```

openlore\-review\-app

Excluded

```text
A release build holding a store fault: both fault types have no values there and the start refuses the seam variable.
```

openlore\-review\-app

##### Models

Quint

docs\/architecture\/models\/resume\_repos\_read\.qnt

###### Properties

everySavedReadCounted

Invariant

```text
Every read the store accepted in this scan is still counted by the latest run of it.
```

review\_domain\:\:repo\_scope\:\:resume\:\:Resume\:\:kept

crates\/review\-domain\/src\/repo\_scope\/resume\.rs\:97

everyCountedReadSaved

Invariant

```text
Every row the count names as read is a row whose read the store accepted, so no row is counted twice and none is invented.
```

adapter\_review\_store\:\:scan\_runs\:\:repos\_read\_of

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:170

countsOnlyHarvested

Invariant

```text
Every row the count names as read is a row GitHub actually served in this scan.
```

openlore\_review\_app\:\:scan\:\:read\_planned

crates\/openlore\-review\-app\/src\/scan\.rs\:664

countNeverExceedsPlan

Invariant

```text
The count never exceeds the size of the frozen plan.
```

ports\:\:ScanRun

crates\/ports\/src\/review\.rs\:423

###### Atoms

State

stored

scan\_run\_repos\.status

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:27

```text
One row per plan position, written inside the plan transaction
```

State

outcome

scan\_runs\.status

crates\/adapter\-review\-store\/src\/scan\_runs\.rs\:17

None.

State

harvested

model\-only

model\-only\: the repos GitHub served\, which no stored row records

None.

State

saved

model\-only

model\-only\: the reads the store accepted\, observed through the stored statuses alone

None.

Action

readAndSave

ScanRunPort\:\:mark\_repo\_read

crates\/openlore\-review\-app\/src\/scan\.rs\:686

```text
One statement per row
```

Action

readUnsaved

ScanRunPort\:\:mark\_repo\_read

crates\/openlore\-review\-app\/src\/scan\.rs\:686

```text
The refused write: the row keeps its status and the run ends interrupted
```

Action

resumeRun

ResumedPlan\:\:record

crates\/openlore\-review\-app\/src\/scan\.rs\:626

```text
One transaction writing every kept row with its status
```

Action

pause

openlore\_review\_app\:\:scan\:\:paused

crates\/openlore\-review\-app\/src\/scan\.rs\:669

None.

Action

complete

ScanRunPort\:\:finish\_scan

crates\/openlore\-review\-app\/src\/scan\.rs\:276

None.

Action

skipRow

openlore\_review\_app\:\:scan\:\:named\_carried\_skip

crates\/openlore\-review\-app\/src\/scan\.rs\:637

None.

Constant

Positions

SCAN\_REPO\_CAP

crates\/review\-domain\/src\/repo\_scope\/mod\.rs\:1

```text
Three positions model a plan bounded at ten repos
```

###### Assumptions

```text
One owner, one scan, one frozen plan of three positions, the bound the slice journey walks
```

```text
One run of the owner is live at a time, which the scan admission already holds
```

```text
A read either is saved, which turns its row read, or is refused, which ends the run interrupted and leaves the row planned
```

```text
A resume carries the statuses of the plan it resumes; the ghost sets of harvested and saved reads have no runtime symbol
```

```text
Bounded symbolic check at twelve steps; no timing, no throughput and no unbounded run chain are covered
```

###### Mutants

######## N1

```text
The resume drops the rows already read instead of carrying them, which is the defect reported: the count falls back to the reads of the last run.
```

###### Kills

everySavedReadCounted

######## N2

```text
The takeover marks every carried row read, not only the rows already read, which counts repos nobody read.
```

###### Kills

everyCountedReadSaved

countsOnlyHarvested

##### Closure

```text
The whole-scan repos-read count
```

Safety

Pass

DesignedNotBuilt

```text
A plan row status is the closed set planned, read and skipped, and the count is the size of a set of plan positions, so a count above the reads the store holds is unrepresentable. The four count invariants were checked symbolically over a three-row plan at twelve steps, and each has a mutant that breaks it.
```

###### Evidence

docs\/architecture\/models\/resume\_repos\_read\.qnt

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

```text
The whole-scan repos-read count
```

Inhabitation

Pass

DesignedNotBuilt

```text
A three-row plan whose first row is read by one run and whose other two are read by the run that takes the plan over is built from admitted input: a saved choice of three repos, a GitHub budget that runs low after one repo, and a resume. The queue then shows three repos read.
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

```text
The whole-scan repos-read count
```

PublicReachability

Pass

DesignedNotBuilt

```text
The owner presses Resume scan at POST /scan, the scan task builds its resumed plan through Resume::kept in crates/review-domain/src/repo_scope/resume.rs:97, writes it through ScanRunPort record_plan at crates/ports/src/review.rs:457, the store counts the read rows at crates/adapter-review-store/src/scan_runs.rs:170, and the queue renders the line at crates/review-domain/src/views/queue.rs:652 on GET /review. The path is acyclic and every site exists today.
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

```text
The whole-scan repos-read count
```

Liveness

Pass

DesignedNotBuilt

```text
No liveness claim: a paused scan goes on only when GitHub refills the request budget and the owner, or the automatic resume, starts the next run, both outside the app. The promise is a truthful count at every moment, which is a state invariant and needs no fairness assumption; nothing here claims a scan eventually completes.
```

###### Evidence

None.

```text
The whole-scan repos-read count after an interrupted run
```

PublicReachability

Pass

DesignedNotBuilt

```text
In a development build holding the after-first fault the owner presses Scan my repos at POST /scan; the store saves the first mark through admits_scan_write at crates/adapter-review-store/src/lib.rs:418 and refuses the next, so the run ends interrupted at crates/openlore-review-app/src/scan.rs:686 with one row stored read. She presses Resume scan; the resumed plan carries that row read through Resume::kept at crates/review-domain/src/repo_scope/resume.rs:97, the resume reads the remaining row, and GET /review shows the count of the whole scan. Every site exists today; only the fault name is not built.
```

###### Evidence

tests\/acceptance\/review\_app\_repos\_read\_across\_resume\.rs

##### Risks

###### Y1

Stressor

```text
Negated assumption, the store accepts the plan write: it refuses record_plan while a resume freezes its plan.
```

```text
The rows of the paused run stay exactly as the store holds them, under their own run id.
```

```text
The resuming run ends interrupted and its queue shows no repos-read line at all.
```

```text
A completed run showing a count below the reads the store holds.
```

```text
The owner picks the run up again; until a plan row is written, that new run reports no count.
```

openlore\-review\-app

###### Y2

Stressor

```text
Negated assumption, the owner source list is unchanged across runs: an account is removed after one of its repos was read.
```

```text
The already-read row stays read and counted, because the scan did read it.
```

```text
The count names a repo whose account the owner no longer lists.
```

```text
Reading that repo again, or adding any suggestion for it.
```

```text
The count accounts for the scan, never for the list in force now.
```

review\-domain

###### Y3

Stressor

```text
Negated assumption, a scan is resumed at most once: the plan is paused and picked up again and again.
```

```text
Each resume carries the statuses the one before it stored, so the count accumulates over the whole chain.
```

```text
Many runs for one scan, each with its own account checks.
```

```text
The count falling back to the reads of the last run.
```

```text
The plan never grows: each resume keeps a subset of the plan it resumes.
```

review\-domain

###### Y4

Stressor

```text
Negated assumption, the process survives the resume: the app restarts in the middle of one.
```

```text
The rows already written stay; the restart sweep marks the run interrupted so it can be picked up.
```

```text
The run is picked up again and re-reads the rows whose marks were never saved.
```

```text
Counting a re-read row twice.
```

```text
One row per repo in one plan, so one count whatever the number of runs.
```

openlore\-review\-app

###### Y5

SinglePointOfFailure

```text
The queue reads the latest run alone: a run that starts and writes no plan row hides the frozen plan of the scan it meant to resume.
```

```text
No untrue count: a run with no plan row reports no count at all.
```

```text
The next scan plans afresh and starts its own count.
```

```text
A count mixing the rows of two scans.
```

```text
The frozen plan of the interrupted scan is lost; its repos are read again under the new plan.
```

adapter\-review\-store

###### Y6

Bypass

```text
Deserialization: the plan reader drops a row whose stored status string it does not know.
```

```text
No unknown status is ever counted as read.
```

```text
An under-count, and that repo being read again.
```

```text
Treating an unknown status as read.
```

```text
Only this adapter writes the column, so the stored values stay the three known ones.
```

adapter\-review\-store

###### Y7

Bypass

```text
An outside writer of scan_run_repos, such as an operator with DuckDB in hand.
```

```text
Inside the workspace the store adapter is the only implementor of the port, so the app has one writer.
```

```text
Nothing: no second writer is admitted.
```

```text
A second writer in the app setting a row read.
```

```text
A direct write by an operator is outside what the app can hold.
```

adapter\-review\-store

###### Y8

Stressor

```text
Second stressor set, not used to derive the design: the owner presses Resume scan while the automatic resume is already running.
```

```text
The scan admission allows one run per owner, so the second attempt is refused as already scanning.
```

```text
The owner is told a scan is already running.
```

```text
Two runs taking the same plan over.
```

```text
Even if both ran, each run counts its own rows, so no run could report more than its own plan.
```

openlore\-review\-app

###### Y9

Stressor

```text
Second stressor set: a forget-me purge deletes the run while the resume is writing its plan.
```

```text
The plan write does nothing for a run the store no longer holds, so no row is left behind.
```

```text
The resume reads repos and records nothing of them.
```

```text
Rows surviving a purge for that owner.
```

```text
The owner data is gone, so no count is owed.
```

adapter\-review\-store

###### Y10

Stressor

```text
Second stressor set: two runs of one owner carry the same started-at second.
```

```text
The latest-run query breaks the tie on insertion order, so the newest run answers.
```

```text
Nothing.
```

```text
Counting the rows of an older run as the latest.
```

```text
Run ordering within one second rests on insertion order.
```

adapter\-review\-store

###### Y11

Bypass

```text
The runs that already saved a mark are held in the store process memory: a restart forgets them.
```

```text
The stored rows and their statuses, which the count reads, are untouched by the seam.
```

```text
After a restart the same run's next mark is saved again, so the fault refuses one mark less than before.
```

```text
A count above the reads the store holds.
```

```text
The seam is development-only, and a journey that needs the refusal keeps the binary running across the resume it observes.
```

adapter\-review\-store

###### Y12

Stressor

```text
Negated assumption, only a development build reads the seam: a release binary starts with REVIEW_APP_TEST_FAULT set to the new name.
```

```text
Every store write behaves normally, because a release build compiles no fault name in.
```

```text
The start refuses with the test-fault-in-release error and says the seam is development-only.
```

```text
A release binary refusing a real repo read.
```

```text
The config property and the xtask guard keep every name behind its cfg gate.
```

openlore\-review\-app

##### Architectural tests

###### T1

```text
The resume verdict stays in the pure core and the SQL stays in the effect shell: review-domain reaches no I O crate, no adapter depends on an adapter, and only a composition root depends on an adapter, over every workspace member.
```

xtask\/src\/check\_arch\.rs

```text
cargo xtask check-arch
```

###### T2

```text
Every review-app test fault name and store fault variant, refuse_mark_repo_read_after_first included, occurs only behind a cfg gate over the app and store source trees, so a release binary honours none of them.
```

xtask\/src\/check\_arch\.rs

```text
cargo xtask check-arch
```

##### Contract tests

None.

## ADRs

None.
