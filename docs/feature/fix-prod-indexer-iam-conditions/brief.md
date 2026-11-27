# Product brief: fix\-prod\-indexer\-iam\-conditions

## Request

```text
Tighten prod-indexer IAM trust and ECR repository policy conditions (scale-to-zero-hosting M5): add an ArnLike aws:SourceArn condition scoped to the openlore-indexer schedule group on scheduler_trust in deploy/tofu/environments/prod-indexer/iam.tf, and change lambda_pulls_images aws:SourceArn condition test from StringLike to ArnLike in deploy/tofu/environments/prod-indexer/ecr.tf.
```

## Work

M

```text
Prevent cross-schedule and cross-ARN-segment confused-deputy access to the prod-indexer scheduler role and ECR repository
```

### Rework

None.

## Promises

### P1

Outcome

```text
scheduler_trust requires aws:SourceArn matching the openlore-indexer schedule group via ArnLike in addition to aws:SourceAccount
```

### P2

Outcome

```text
lambda_pulls_images evaluates aws:SourceArn with ArnLike instead of StringLike
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
scheduler_trust pins aws:SourceArn with ArnLike to the openlore-indexer schedule group alongside aws:SourceAccount, and lambda_pulls_images uses ArnLike instead of StringLike on aws:SourceArn
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
