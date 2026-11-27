# Acceptance brief: fix\-ci\-permissions\-and\-latest\-scan

## Slices

### S1

##### Covers

P1

```text
Check the continuous integration workflow for top-level token permissions
```

```text
The workflow declares top-level permissions granting only read access to repository contents
```

```text
Workflow runs default to least-privilege read-only repository contents permissions unless a job explicitly requests more
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:ci\_workflow\_declares\_top\_level\_read\_only\_contents\_permissions

```text
An individual job overrides permissions with broader scope than needed, or top-level permissions are declared via YAML anchors rather than a direct mapping
```

###### Obligations

None.

##### Covers

P2

```text
Run the deployment check workflow's latest-tag scan against clean digest-pinned files and against planted latest-tag references in review-app, indexer, and prod-indexer deployment files
```

```text
Clean digest-pinned files pass while any latest-tag reference in review-app compose or deploy script, indexer top-level or nested files, or prod-indexer infrastructure files causes a blocking failure
```

```text
Unpinned latest image tags in indexer deployment files and prod-indexer infrastructure files are blocked in continuous integration alongside review-app files
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:deploy\_check\_workflow\_scans\_indexer\_and\_prod\_indexer\_for\_latest\_image\_references

```text
A latest-tag reference is added only in a file type excluded by a custom filter other than shell, compose YAML, or Tofu files, or the workflow trigger path filters omit a newly added directory
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
The ci.yml workflow declares a top-level permissions block with contents: read.
```

```text
Checking for top-level permissions with contents: read in ci.yml does not see whether additional top-level or job-level permissions widen token scope, whether other workflows omit top-level permissions, or whether CI jobs succeed at runtime under read-only token permissions.
```

None.

None.

None.

#### E2

##### Promises

P2

```text
The deploy-pds-check.yml workflow scans deploy/indexer for :latest references.
```

```text
Checking that deploy/indexer is included in the :latest scan in deploy-pds-check.yml does not see whether a real :latest reference in nested or templated files under deploy/indexer actually fails the workflow at runtime, or whether pinned tags avoid false positives.
```

None.

None.

None.

#### E3

##### Promises

P2

```text
The deploy-pds-check.yml workflow scans deploy/tofu/environments/prod-indexer for :latest references.
```

```text
Checking that deploy/tofu/environments/prod-indexer is included in the :latest scan in deploy-pds-check.yml does not see whether a real :latest reference in HCL or nested files under deploy/tofu/environments/prod-indexer triggers a check failure at runtime, nor whether other deploy environments remain unscanned.
```

None.

None.

None.

## Charter

None.

## Examination

### E1

not_examined

```text
O1, O2: Public examination workspace has no projected files; .github/workflows/ci.yml is absent from nwdes examine read (ReadRefused) and no admitted probe command exposes it.
```

### E2

not_examined

```text
O1, O2: Public examination workspace has no projected files; .github/workflows/deploy-pds-check.yml is absent from nwdes examine read (ReadRefused) and no admitted probe command exposes it.
```

### E3

not_examined

```text
O1, O2: Public examination workspace has no projected files; .github/workflows/deploy-pds-check.yml is absent from nwdes examine read (ReadRefused) and no admitted probe command exposes it.
```
