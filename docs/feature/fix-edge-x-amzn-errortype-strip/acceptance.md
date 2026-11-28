# Acceptance brief: fix\-edge\-x\-amzn\-errortype\-strip

## Slices

### S1

##### Covers

P1

```text
Inspect the prod-indexer CloudFront response headers strip policy configuration for removed origin headers
```

```text
The strip policy removes x-amzn-errortype alongside the existing six x-amzn-* headers and no others
```

```text
CloudFront strips x-amzn-errortype and all six existing origin headers from responses before clients see them
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:the\_edge\_strips\_every\_header\_that\_names\_the\_origin

```text
Static HCL declares all seven headers in the strip policy, while live CloudFront edge enforcement is not exercised against AWS
```

###### Obligations

None.

##### Covers

P2

```text
Run the edge verification check against a stubbed edge whose responses vary which endpoint carries an origin header
```

```text
The check passes when both health check and POST search are clean, fails when POST search returns x-amzn-errortype or any mixed-case x-amzn-* header even with a clean health check, and still fails when the health check returns an x-amzn-* header
```

```text
Deployment edge verification blocks release whenever an origin x-amzn-* header leaks on either POST search or the health check
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_ci\.rs\:\:the\_edge\_check\_rejects\_origin\_headers\_on\_post\_search\_as\_well\_as\_healthz

```text
POST search returns an x-amzn-* header only on a non-200 error response while 200 POST search responses remain clean
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
The CloudFront response headers policy "strip" includes x-amzn-errortype in its removed headers list so that x-amzn-errortype is stripped from responses.
```

```text
Inspecting the policy configuration alone does not observe a live CloudFront edge stripping x-amzn-errortype from an actual origin error response or verify that the policy is attached to the active distribution behavior.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
The CloudFront response headers policy "strip" retains all six previously configured x-amzn-* headers in its removed headers list without dropping any of them when x-amzn-errortype is added.
```

```text
The promise does not enumerate the six existing x-amzn-* header names, so a check verifying only header count or only x-amzn-errortype could miss a renamed, misspelled, or dropped existing header.
```

None.

None.

None.

#### E3

##### Promises

P2

```text
edge_checks rejects a response when a POST search response contains any x-amzn-* header (including x-amzn-errortype).
```

```text
Checking edge_checks only against a happy-path or mocked POST search response may not observe origin error responses that emit x-amzn-errortype, mixed-case header names, or real deployed edge responses.
```

None.

None.

None.

#### E4

##### Promises

P2

```text
edge_checks continues to reject a response when a GET /healthz response contains any x-amzn-* header.
```

```text
An oracle focused on the new POST search check could miss a regression in GET /healthz rejection, and edge_checks does not observe x-amzn-* header leakage on routes other than GET /healthz and POST search.
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
Blocked by empty public projection (O1) and unexposed edge/policy probe surface (O2: admitted commands are openlore-review-app, review-app-oracle, indexer-ingest-oracle). Cannot observe CloudFront response headers policy strip or live edge stripping of x-amzn-errortype from user view.
```

### E2

not_examined

```text
Blocked by empty public projection (O1) and unexposed edge/policy probe surface (O2). Cannot observe whether the CloudFront response headers policy strip retains all six previously configured x-amzn-* headers without reading private repo source.
```

### E3

not_examined

```text
Blocked by empty public projection (O1) and unexposed edge_checks probe surface (O2: edge_checks is not an admitted probe command). Cannot invoke edge_checks against a POST search response carrying x-amzn-errortype or other x-amzn-* headers.
```

### E4

not_examined

```text
Blocked by empty public projection (O1) and unexposed edge_checks probe surface (O2: edge_checks is not an admitted probe command). Cannot invoke edge_checks against a GET /healthz response carrying x-amzn-* headers.
```
