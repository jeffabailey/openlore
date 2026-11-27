# Acceptance brief: fix\-marker\-key\-trim

## Slices

### S1

##### Covers

P1

```text
Check whether an index key, pass-record key, or both with leading or trailing spaces, tabs, or newlines around the write-probe marker are recognized as the marker, and whether empty, whitespace-only, uppercase case-variant, padded non-marker, prefixed or suffixed marker paths, and keys with internal whitespace are allowed
```

```text
Returns true whenever either or both keys trim to the write-probe marker and false for empty, whitespace-only, case-variant, and all other non-marker keys
```

```text
Any index or pass-record key (or both) whose trimmed text matches the write-probe marker is identified as the marker, while empty, whitespace-only, case-variant, and distinct keys are permitted
```

##### Disposition

Tested

crates\/adapter\-s3\-index\-object\/tests\/scale\_to\_zero\_write\_probe\.rs\:\:whitespace\_around\_the\_write\_marker\_is\_still\_recognized\_as\_the\_marker

```text
An implementation that trims only ASCII whitespace (space, tab, carriage return, newline) rather than non-ASCII Unicode whitespace would still pass
```

###### Obligations

None.

##### Covers

P2

```text
Connect to an object-store location whose index key or pass-record key has leading or trailing whitespace around the write-probe marker, and connect to locations with non-marker keys
```

```text
Refuses connection with a probe refusal error when either key trims to the write-probe marker and succeeds for non-marker locations
```

```text
Startup refuses any configured store location whose index or pass-record key trims to the write-probe marker and accepts valid non-marker locations
```

##### Disposition

Tested

crates\/adapter\-s3\-index\-object\/tests\/scale\_to\_zero\_write\_probe\.rs\:\:connecting\_to\_a\_location\_with\_a\_padded\_write\_marker\_is\_refused

```text
An implementation that trims only ASCII whitespace when checking keys at connect time rather than non-ASCII Unicode whitespace would still pass
```

###### Obligations

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
When either index_key or last_pass_key (or both) equals WRITE_PROBE_KEY directly or after trimming leading or trailing whitespace, location_names_write_probe_key returns true.
```

```text
An oracle that tests only untrimmed equality or ASCII spaces on a single key does not see whether both index_key and last_pass_key are trimmed identically or whether non-space whitespace (such as tabs or newlines) is handled consistently.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
When neither index_key nor last_pass_key trims to WRITE_PROBE_KEY (including empty or whitespace-only keys, keys with extra non-whitespace characters, path-prefixed/suffixed keys, or keys with internal whitespace), location_names_write_probe_key returns false.
```

```text
An oracle that checks only a single ordinary unrelated key does not see false positives on near-miss keys containing WRITE_PROBE_KEY as a substring, prefix, suffix, or case variant, nor on whitespace-only keys.
```

None.

None.

None.

#### E3

##### Promises

P2

```text
S3IndexObjectAdapter::connect refuses with an error any location whose index_key or last_pass_key equals WRITE_PROBE_KEY directly or after trimming leading or trailing whitespace.
```

```text
An oracle that checks only exact untrimmed keys or only index_key does not see whether last_pass_key with surrounding whitespace is also refused, whether refusal occurs before any remote S3 network call or storage mutation, or whether the error distinguishes key refusal from unrelated connection failures.
```

None.

None.

None.

#### E4

##### Promises

P2

```text
S3IndexObjectAdapter::connect does not reject a location on the write-probe-key guard when neither index_key nor last_pass_key trims to WRITE_PROBE_KEY.
```

```text
An oracle that only asserts connect returns an error for probe keys does not distinguish targeted probe-key refusal from a blanket connection failure or overly broad substring matching.
```

None.

None.

None.

## Charter

None.

## Examination

None.
