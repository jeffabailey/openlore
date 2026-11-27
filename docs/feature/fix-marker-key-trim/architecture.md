# Architecture brief: fix\-marker\-key\-trim

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

crates\/ports\/src\/probe\.rs

Extend

None.

crates\/adapter\-s3\-index\-object\/tests\/scale\_to\_zero\_write\_probe\.rs

#### Verification

```text
limactl shell local-dev -- sh -c 'cd ~/openlore && cargo test -p adapter-s3-index-object --test scale_to_zero_write_probe'
```

#### Supports

None.

#### Public oracle

```text
Call location_names_write_probe_key and S3IndexObjectAdapter::connect with whitespace-padded WRITE_PROBE_KEY values
```

```text
Both return true / Err(IndexObjectError::ProbeRefused) for leading/trailing whitespace around WRITE_PROBE_KEY in either key, and false / Ok for non-marker keys
```

```text
A whitespace-padded WRITE_PROBE_KEY in index_key or last_pass_key is accepted instead of refused
```

##### Promises

P1

P2

#### Prefactoring

None.

#### Records

None.

## ADRs

None.
