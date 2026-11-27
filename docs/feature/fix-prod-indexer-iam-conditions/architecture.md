# Architecture brief: fix\-prod\-indexer\-iam\-conditions

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

deploy\/tofu\/environments\/prod\-indexer\/iam\.tf

Extend

None.

deploy\/tofu\/environments\/prod\-indexer\/ecr\.tf

Extend

None.

xtask\/tests\/scale\_to\_zero\_ci\.rs

#### Verification

```text
limactl shell local-dev -- sh -c 'cd ~/openlore && cargo test -p xtask --test scale_to_zero_ci'
```

#### Supports

None.

#### Public oracle

```text
Run xtask scale_to_zero_ci tests against deploy/tofu/environments/prod-indexer/iam.tf and ecr.tf
```

```text
scheduler_trust enforces both aws:SourceAccount and ArnLike aws:SourceArn for the openlore-indexer schedule group, and lambda_pulls_images enforces ArnLike on aws:SourceArn
```

```text
scheduler_trust omits aws:SourceArn or uses StringLike, or lambda_pulls_images still uses StringLike on aws:SourceArn
```

##### Promises

P1

P2

#### Prefactoring

None.

#### Records

None.

## ADRs

None.
