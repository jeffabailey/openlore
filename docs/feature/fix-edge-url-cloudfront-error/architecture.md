# Architecture brief: fix\-edge\-url\-cloudfront\-error

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

deploy\/indexer\/lambda\.sh

Extend

None.

xtask\/tests\/scale\_to\_zero\_platform\.rs

#### Verification

```text
limactl shell local-dev -- sh -c 'cd ~/openlore && cargo test -p xtask --test scale_to_zero_platform'
```

#### Supports

None.

#### Public oracle

```text
Invoke a deploy.sh lambda verb that calls edge_url when aws cloudfront list-distributions exits non-zero vs when it returns an empty distribution list
```

```text
Non-zero list-distributions exits non-zero with a refusal message stating CloudFront could not be read (not 'no distribution serves'); empty distribution list still refuses with 'no distribution serves'
```

```text
A failed CloudFront list-distributions call aborts without a CloudFront read refusal message or prints 'no distribution serves'
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
