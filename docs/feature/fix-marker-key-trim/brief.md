# Product brief: fix\-marker\-key\-trim

## Request

```text
Trim whitespace before comparing index_key and last_pass_key against WRITE_PROBE_KEY in location_names_write_probe_key (crates/ports/src/probe.rs) so padded marker keys are refused at init and connect (scale-to-zero-hosting LOW: marker-key trim).
```

## Work

M

```text
Prevent whitespace-padded index or last-pass keys from bypassing the write-probe marker key refusal
```

### Rework

None.

## Promises

### P1

Outcome

```text
location_names_write_probe_key returns true when either key trims to WRITE_PROBE_KEY and false otherwise
```

### P2

Outcome

```text
S3IndexObjectAdapter::connect refuses a location whose index_key or last_pass_key trims to WRITE_PROBE_KEY
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
location_names_write_probe_key and S3IndexObjectAdapter::connect refuse index_key or last_pass_key when trimming whitespace equals WRITE_PROBE_KEY, while allowing non-marker keys
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
