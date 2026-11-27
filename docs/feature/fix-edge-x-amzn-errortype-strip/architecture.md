# Architecture brief: fix\-edge\-x\-amzn\-errortype\-strip

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

deploy\/tofu\/environments\/prod\-indexer\/edge\.tf

Extend

None.

deploy\/indexer\/lambda\.sh

Extend

None.

xtask\/tests\/scale\_to\_zero\_ci\.rs

#### Verification

```text
limactl shell local-dev -- sh -c 'cd ~/openlore && cargo test -p xtask --test scale_to_zero_ci'
```

#### Supports

None.

#### Public oracle

```text
Run xtask scale_to_zero_ci tests against deploy/tofu/environments/prod-indexer/edge.tf and deploy/indexer/lambda.sh edge_checks
```

```text
openlore-index-strip includes x-amzn-errortype in remove_headers_config, and edge_checks fails when a POST search response carries x-amzn-errortype even if GET /healthz carries no x-amzn-* headers
```

```text
openlore-index-strip omits x-amzn-errortype, or edge_checks passes when POST search returns an x-amzn-* header
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
