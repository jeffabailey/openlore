# Acceptance brief: fix\-edge\-url\-cloudfront\-error

## Slices

### S1

##### Covers

P1

```text
Run a Lambda operator command that resolves the public edge address when listing CloudFront distributions fails with access denied, throttling, or a network error
```

```text
The command exits non-zero with a refusal message stating CloudFront could not be read and does not claim that no distribution serves the host
```

```text
An operator sees a clear refusal that CloudFront could not be read instead of a misleading message that no distribution serves the host
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_platform\.rs\:\:a\_failed\_cloudfront\_read\_refuses\_with\_a\_cloudfront\_read\_failure

```text
A refusal that states CloudFront could not be read without echoing the raw AWS error text, or a different operator verb that bypassed shared edge resolution
```

###### Obligations

None.

##### Covers

P2

```text
Run a Lambda operator command that resolves the public edge address when listing CloudFront distributions succeeds with an empty list or only unrelated aliases
```

```text
The command exits non-zero with a refusal stating that no distribution serves the host
```

```text
An operator still receives the refusal stating that no distribution serves the host when no matching distribution alias exists
```

##### Disposition

Tested

xtask\/tests\/scale\_to\_zero\_platform\.rs\:\:an\_empty\_or\_unmatched\_cloudfront\_distribution\_list\_refuses\_with\_no\_distribution\_serves

```text
A CloudFront response that omits the Items array entirely rather than returning an empty array, or a refusal that says no distribution serves without repeating the hostname
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
When aws cloudfront list-distributions fails, edge_url refuses (non-zero exit without returning a URL) with a message identifying the failed CloudFront read.
```

```text
An oracle testing a single stubbed failure may check only for non-zero exit or a loose substring, missing whether stdout stays free of a fallback URL and whether distinct failure causes (auth, network, missing CLI) still identify the failed CloudFront read.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
When aws cloudfront list-distributions fails, edge_url does not claim that no distribution serves the host.
```

```text
An oracle that only asserts the new CloudFront failure message is present does not see if the misleading 'no distribution serves' wording is still emitted alongside it on stdout or stderr.
```

None.

None.

None.

#### E3

##### Promises

P2

```text
When aws cloudfront list-distributions succeeds and no distribution has an alias matching the requested host, edge_url refuses with a message containing 'no distribution serves'.
```

```text
An oracle testing only an empty distribution list does not see whether non-empty lists with non-matching aliases or distributions lacking aliases still refuse with 'no distribution serves' instead of misreporting a CloudFront read failure.
```

None.

None.

None.

## Charter

None.

## Examination

None.
