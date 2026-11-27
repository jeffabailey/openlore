# Product brief: fix\-edge\-url\-cloudfront\-error

## Request

```text
Report a distinct CloudFront read failure message in edge_url (deploy/indexer/lambda.sh) when aws cloudfront list-distributions fails, instead of aborting without a refusal or reporting that no distribution serves INDEX_HOST (scale-to-zero-hosting LOW: edge_url message on a failed CloudFront read).
```

## Work

M

```text
Give operators an accurate refusal message when CloudFront cannot be read vs when no distribution matches INDEX_HOST
```

### Rework

None.

## Promises

### P1

Outcome

```text
When aws cloudfront list-distributions fails, edge_url refuses with a message identifying the failed CloudFront read rather than claiming no distribution serves the host
```

### P2

Outcome

```text
When aws cloudfront list-distributions succeeds with no matching alias, edge_url continues to refuse with 'no distribution serves'
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
edge_url fails with a refusal naming the CloudFront read failure when aws cloudfront list-distributions fails, and still reports no distribution serves INDEX_HOST when the list succeeds without a matching alias
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
