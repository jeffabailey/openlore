# Architecture brief: fix\-plc\-resolver\-flake

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

tests\/acceptance\/support\/mod\.rs

Extend

None.

tests\/acceptance\/indexer\_ingest\.rs

#### Verification

```text
cargo test --test indexer_ingest
```

#### Supports

None.

#### Public oracle

```text
A client connects to FakePlcResolver and sends GET /did:plc:priya-test HTTP/1.1 after a delay and across split TCP writes, and openlore-indexer ingest resolves Priya's real z6Mk key against FakePlcResolver
```

```text
FakePlcResolver returns HTTP 200 with Priya's z6Mk DID document for delayed and split-write requests, and indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam passes with 1 verified claim and 1 bad_signature rejection
```

```text
FakePlcResolver returns HTTP 404 or an empty/truncated response on a delayed or split-write request, or indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam fails
```

##### Promises

P1

#### Prefactoring

None.

#### Records

None.

## ADRs

None.
