# Acceptance brief: fix\-plc\-resolver\-flake

## Slices

### S1

##### Covers

P1

```text
A client connects to the PLC resolver HTTP endpoint and sends a DID lookup request for Priya after an initial pause, then sends DID lookup requests for Priya and Sven split across multiple TCP writes with pauses between chunks.
```

```text
Each delayed and split-write request receives HTTP 200 with a JSON DID document whose id matches the requested DID and whose verification method carries a real Ed25519 multibase key, with matching keys across delayed and split Priya requests and distinct keys between Priya and Sven.
```

```text
Delayed and fragmented HTTP requests for Priya and Sven DID documents return HTTP 200 with the matching DID document and verification key instead of spurious 404 responses.
```

##### Disposition

Tested

tests\/acceptance\/indexer\_ingest\.rs\:\:plc\_did\_document\_lookup\_succeeds\_for\_delayed\_and\_split\_requests

```text
A resolver that reads only up to the first request-line CRLF rather than waiting for the trailing CRLFCRLF header terminator, or that relies on OS default blocking inheritance instead of explicitly setting blocking mode on accepted sockets.
```

###### Obligations

None.

##### Covers

P1

```text
The indexer ingests one validly signed claim and one tampered-signature claim from Priya while resolving Priya's real Ed25519 multibase verification key from the PLC resolver HTTP endpoint with no environment key override.
```

```text
The indexer verifies and indexes the valid claim attributed to Priya with a non-empty verification record, rejects the tampered claim as a bad signature, and leaves the tampered claim absent from the index.
```

```text
Ingesting signed claims against the PLC resolver reliably verifies valid claims and rejects tampered signatures using the real decoded Ed25519 key.
```

##### Disposition

Tested

tests\/acceptance\/indexer\_ingest\.rs\:\:indexer\_verifies\_against\_real\_decoded\_plc\_z6mk\_key\_not\_the\_test\_seam

```text
A resolver that only succeeds when the HTTP client writes the full request head in a single immediate TCP segment without delay or fragmentation.
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
When a client connects to FakePlcResolver and delays before sending a valid DID HTTP request, FakePlcResolver waits for the request head and returns HTTP 200 with the matching DID document rather than a spurious 404.
```

```text
An oracle that writes the full HTTP request immediately upon connect passes even if accepted streams remain non-blocking and return 404 under scheduling delay.
```

yes

None.

None.

#### E2

##### Promises

P1

```text
When a client transmits a valid DID HTTP request head in multiple separate writes ending with CRLFCRLF, FakePlcResolver reads the complete head before routing and returns HTTP 200 with the matching DID document instead of 404.
```

```text
An oracle that sends the entire request in a single write (even after a delay) does not see whether FakePlcResolver loops until CRLFCRLF or stops after the first partial read.
```

yes

None.

None.

#### E3

##### Promises

P1

```text
Every accepted connection on FakePlcResolver handles delayed or split requests by returning HTTP 200 with the DID document matching the requested DID, not only the first connection.
```

```text
An oracle that exercises only a single connection or a single fixture DID does not observe per-stream handling across subsequent accepted connections or verify that the returned document matches the requested DID.
```

yes

None.

None.

## Charter

None.

## Examination

None.
