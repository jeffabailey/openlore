# Product brief: fix\-plc\-resolver\-flake

## Request

```text
Fix the open flake indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam: FakePlcResolver in tests/acceptance/support/mod.rs leaves accepted TcpStreams non-blocking and reads only a single chunk instead of the full HTTP request head, causing spurious 404 responses under parallel load or split TCP writes.
```

## Work

M

```text
FakePlcResolver reliably reads complete HTTP request heads on blocking accepted streams so indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam does not flake under parallel load or fragmented requests
```

### Rework

None.

## Promises

### P1

Outcome

```text
FakePlcResolver sets every accepted stream to blocking mode and reads the complete HTTP request head until CRLFCRLF so delayed or split client writes receive HTTP 200 with the matching DID document instead of spurious 404
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
FakePlcResolver serves HTTP 200 with the matching DID document when a client pauses before writing or sends the HTTP request line across multiple TCP chunks, and indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam passes reliably
```

#### Promises

P1

#### Depends on

None.

#### Slice work

None.
