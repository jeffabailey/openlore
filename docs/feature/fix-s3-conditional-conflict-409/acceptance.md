# Acceptance brief: fix\-s3\-conditional\-conflict\-409

## Slices

### S1

##### Covers

P1

```text
A conditional object upload receives HTTP 412 PreconditionFailed or HTTP 409 ConditionalRequestConflict from the object store, under either a create-only or matching-version condition.
```

```text
The upload reports a lost-update conflict rather than a store failure and sends exactly one request per upload.
```

```text
A concurrent conditional upload conflict is treated as a normal lost-update conflict without retrying or failing as a store outage.
```

##### Disposition

Tested

crates\/adapter\-s3\-index\-object\/tests\/s3\_answers\_and\_bounds\.rs\:\:conditional\_publish\_reports\_conflict\_for\_precondition\_and\_concurrent\_conflict\_and\_fails\_other\_errors

```text
The adapter only parses bare error XML and fails to recognize ConditionalRequestConflict when extra XML elements accompany the error code.
```

###### Obligations

None.

##### Covers

P1

```text
A conditional object upload receives HTTP 409 with a non-conflict error code such as OperationAborted, HTTP 409 with no error code, HTTP 403 AccessDenied, or HTTP 500 InternalError from the object store.
```

```text
The upload reports a store failure (refused for 4xx responses, unavailable for 5xx responses), never a lost-update conflict, and sends exactly one request per upload.
```

```text
Non-conflict store rejections and outages surface as store failures rather than being silently masked as lost-update conflicts.
```

##### Disposition

Tested

crates\/adapter\-s3\-index\-object\/tests\/s3\_answers\_and\_bounds\.rs\:\:conditional\_publish\_reports\_conflict\_for\_precondition\_and\_concurrent\_conflict\_and\_fails\_other\_errors

```text
An untested non-conflict HTTP 409 error code such as BucketAlreadyExists is erroneously included in an over-broad conflict allowlist.
```

###### Obligations

None.

##### Covers

P1

```text
A startup write-condition probe receives HTTP 409 ConditionalRequestConflict on its first conditional marker upload or on its repeat conditional marker upload after an initial HTTP 200.
```

```text
The startup write-condition probe succeeds without refusing startup as an ignored write condition, sending one upload when the first conflicts and two uploads when the second conflicts.
```

```text
Startup does not falsely refuse a bucket that answers a conditional marker upload with HTTP 409 ConditionalRequestConflict.
```

##### Disposition

Tested

crates\/adapter\-s3\-index\-object\/tests\/s3\_answers\_and\_bounds\.rs\:\:the\_write\_probe\_refuses\_only\_two\_accepted\_puts\_after\_at\_most\_two\_puts

```text
The adapter classifies HTTP 409 ConditionalRequestConflict as unknown rather than honoured inside the write probe, because the public startup probe succeeds on both honoured and unknown answers.
```

###### Obligations

None.

##### Covers

P1

```text
The internal write-condition probe classifies an HTTP 409 ConditionalRequestConflict response as proving the write condition was honoured rather than unknown.
```

```text
The probe records the conditional write as honoured rather than unknown when the object store answers with HTTP 409 ConditionalRequestConflict.
```

```text
Internally, HTTP 409 ConditionalRequestConflict is recorded as proof that the store honours conditional writes rather than an inconclusive error.
```

##### Disposition

NotTested

```text
The public startup write probe returns the same passing outcome for both honoured and unknown marker answers, and the intermediate probe classification helper is crate-private with no declared contract locator.
```

crafter unit and property tests in crates\/adapter\-s3\-index\-object\/src\/s3\.rs

## Expectations

### Rows

#### E1

##### Promises

P1

```text
A conditional PutObject answered with HTTP 409 and error code ConditionalRequestConflict is classified as PublishOutcome::Conflict and treated as proof that the store honoured the write condition.
```

```text
An oracle that only asserts PublishOutcome::Conflict on a synthetic HTTP 409 ConditionalRequestConflict response does not observe whether the conditional precondition was sent on the wire, whether existing stored data remained unmodified, or whether callers treat the conflict identically as proof of an honoured write condition.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
A conditional PutObject answered with HTTP 409 and any error code other than ConditionalRequestConflict (including an absent or unparseable error code) remains a store failure and is not classified as PublishOutcome::Conflict.
```

```text
An oracle that tests only a single alternate 409 error code does not observe behavior on HTTP 409 responses with missing or malformed error payloads, other distinct S3 409 codes, or a non-conditional PutObject returning HTTP 409 ConditionalRequestConflict.
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
No admitted public probe command exposes S3 conditional PutObject (publish_if_match) or write_probe_seen against HTTP 409 ConditionalRequestConflict (O1, O2, O3). Cannot observe wire precondition header, Conflict classification, Honoured probe result, or unchanged stored data from the user's point of view.
```

### E2

not_examined

```text
No admitted public probe command exposes S3 conditional PutObject (publish_if_match) against HTTP 409 with alternate, missing, or malformed error codes or non-conditional 409 (O1, O2, O3). Cannot observe store-failure classification from the user's point of view.
```
