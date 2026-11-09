# Acceptance brief: fix\-repos\-read\-truthful

## Slices

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
