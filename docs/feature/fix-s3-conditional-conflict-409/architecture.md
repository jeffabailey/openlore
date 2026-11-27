# Architecture brief: fix\-s3\-conditional\-conflict\-409

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

crates\/adapter\-s3\-index\-object\/src\/s3\.rs

Extend

None.

crates\/adapter\-s3\-index\-object\/tests\/s3\_answers\_and\_bounds\.rs

#### Verification

```text
cargo test -p adapter-s3-index-object
```

#### Supports

None.

#### Public oracle

```text
S3 returns HTTP 412 PreconditionFailed, HTTP 409 ConditionalRequestConflict, HTTP 409 with a different error code (e.g. OperationAborted), HTTP 403 AccessDenied, or HTTP 500 InternalError to a conditional PutObject
```

```text
publish_if_match returns Ok(PublishOutcome::Conflict) for 412 and for 409 ConditionalRequestConflict, and returns Err for 409 with another error code, 403, and 500; each makes exactly one attempt
```

```text
publish_if_match returns Err on 409 ConditionalRequestConflict, or returns Ok(PublishOutcome::Conflict) on a non-ConditionalRequestConflict 409, 403, or 500
```

##### Promises

P1

#### Prefactoring

None.

#### Records

None.

## ADRs

None.
