# Acceptance brief: fix\-prod\-indexer\-iam\-conditions

## Slices

### S1

##### Covers

P1

```text
Verify the prod-indexer IAM trust policy for the EventBridge Scheduler role
```

```text
The scheduler trust policy enforces both an exact account condition and an ArnLike source ARN condition scoped to the openlore-indexer schedule group
```

```text
Only EventBridge Scheduler schedules in the openlore-indexer schedule group within the production account can assume the scheduler role
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:each\_role\_can\_do\_what\_its\_table\_says\_and\_nothing\_else

```text
A trust statement with an unexpected Deny effect or an AWS runtime ARN evaluation difference not visible in static policy inspection would still pass
```

###### Obligations

None.

##### Covers

P2

```text
Verify the prod-indexer ECR repository pull policy for Lambda
```

```text
The Lambda image pull policy evaluates the source ARN condition using ArnLike rather than StringLike for openlore-indexer functions
```

```text
Lambda image pulls from the indexer repository require an ARN-segment-aware ArnLike match on the openlore-indexer function ARN
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:the\_registry\_is\_immutable\_and\_only\_lambda\_pulls

```text
A pull policy statement with an unexpected Deny effect or an AWS runtime ARN evaluation difference not visible in static policy inspection would still pass
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
In the scheduler_trust policy, assume-role access requires both aws:SourceAccount and an ArnLike condition on aws:SourceArn matching the openlore-indexer schedule group.
```

```text
A static policy check cannot observe live AWS STS/EventBridge Scheduler evaluation—whether real openlore-indexer schedule invocations match the ArnLike pattern without blocking legitimate runs or whether another same-account schedule group is actually denied at runtime; P1 also leaves the exact schedule-group ARN pattern (partition, region, wildcard suffix) unspecified.
```

None.

None.

None.

#### E2

##### Promises

P2

```text
In the lambda_pulls_images policy, the aws:SourceArn condition uses the ArnLike operator and does not use StringLike.
```

```text
Checking the policy operator alone cannot observe live AWS ECR/Lambda image-pull authorization—whether ArnLike matches the actual aws:SourceArn presented by Lambda at runtime across colon-delimited ARN segments where ArnLike differs from StringLike, or whether the target ARN pattern itself is scoped as intended.
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
Blocked by missing public evidence surface (O1, O2, O3): examination workspace projects 0 public files, nwdes examine read refuses deploy/tofu/environments/prod-indexer/iam.tf, and nwdes examine probe refuses xtask (admitting only openlore-review-app, review-app-oracle, indexer-ingest-oracle). Prerequisite: project public policy artifact or admit a public oracle CLI for prod-indexer IAM conditions; owner: host/harness evidence-capture setup; remedy: expose iam.tf or an admitted public oracle command in the examination workspace.
```

### E2

not_examined

```text
Blocked by missing public evidence surface (O1, O2, O3): examination workspace projects 0 public files, nwdes examine read refuses deploy/tofu/environments/prod-indexer/ecr.tf, and nwdes examine probe refuses xtask (admitting only openlore-review-app, review-app-oracle, indexer-ingest-oracle). Prerequisite: project public policy artifact or admit a public oracle CLI for prod-indexer ECR policy conditions; owner: host/harness evidence-capture setup; remedy: expose ecr.tf or an admitted public oracle command in the examination workspace.
```
