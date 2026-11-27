# Product brief: fix\-edge\-x\-amzn\-errortype\-strip

## Request

```text
Strip x-amzn-errortype in the prod-indexer CloudFront response headers policy (deploy/tofu/environments/prod-indexer/edge.tf) and sample POST search response headers for x-amzn-* headers in edge_checks (deploy/indexer/lambda.sh) in addition to GET /healthz (scale-to-zero-hosting M7).
```

## Work

M

```text
Prevent x-amzn-errortype and other origin x-amzn-* headers from leaking through the CloudFront edge on search or error responses
```

### Rework

None.

## Promises

### P1

Outcome

```text
aws_cloudfront_response_headers_policy.strip removes x-amzn-errortype in addition to the existing six x-amzn-* headers
```

### P2

Outcome

```text
edge_checks rejects an x-amzn-* response header on POST search as well as on GET /healthz
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
openlore-index-strip removes x-amzn-errortype alongside the existing six x-amzn-* headers, and edge_checks rejects an x-amzn-* header on the POST search response as well as GET /healthz
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
