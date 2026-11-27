# Product brief: fix\-s3\-conditional\-conflict\-409

## Request

```text
Fix S3 409 ConditionalRequestConflict mapping (scale-to-zero-hosting fix-later): adapter-s3-index-object treats only HTTP 412 as PublishOutcome::Conflict (and RepeatAnswer::Honoured), mapping S3 409 ConditionalRequestConflict from concurrent conditional PUTs as a store failure instead of a lost-update conflict.
```

## Work

M

```text
S3 409 ConditionalRequestConflict on conditional PUTs maps to PublishOutcome::Conflict (and proves conditional writes are honoured in the write probe) while non-ConditionalRequestConflict 409s remain store failures
```

### Rework

None.

## Promises

### P1

Outcome

```text
A conditional PutObject answered with HTTP 409 ConditionalRequestConflict is classified as PublishOutcome::Conflict (and proves the write condition was honoured), while HTTP 409 with any other error code remains a store failure
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
publish_if_match returns Ok(PublishOutcome::Conflict) on HTTP 412 and HTTP 409 ConditionalRequestConflict, returns Err on HTTP 409 with any other error code or 403/500, and write_probe_seen treats 409 ConditionalRequestConflict as Honoured
```

#### Promises

P1

#### Depends on

None.

#### Slice work

None.
