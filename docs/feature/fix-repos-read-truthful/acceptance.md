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

#### E2

##### Promises

P1

```text
An interrupted scan can be run again and resumes that run, instead of forcing the user to abandon it and start over.
```

#### E3

##### Promises

P1

```text
A run whose store write failed never shows a repos-read count that claims more repos were read than the store actually holds for that run.
```

#### E4

##### Promises

P2

```text
A completed run that had no planned repos shows no repos-read line at all, rather than a zero or placeholder count.
```

#### E5

##### Promises

P2

```text
A completed run that had planned repos still shows its repos-read line, with the true number of repos read for that run.
```

## Charter

None.

## Examination

None.
