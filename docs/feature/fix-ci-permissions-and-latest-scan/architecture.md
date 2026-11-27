# Architecture brief: fix\-ci\-permissions\-and\-latest\-scan

## Paradigm

functional

## Shared records

None.

## Slices

### S1

#### Targets

\.github\/workflows\/ci\.yml

Extend

None.

\.github\/workflows\/deploy\-pds\-check\.yml

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
Run xtask scale_to_zero_ci tests against .github/workflows/ci.yml and .github/workflows/deploy-pds-check.yml
```

```text
ci.yml defines top-level permissions with contents: read, and deploy-pds-check.yml's :latest scan includes deploy/indexer and deploy/tofu/environments/prod-indexer
```

```text
ci.yml lacks top-level permissions: contents: read, or deploy-pds-check.yml's :latest scan omits deploy/indexer or deploy/tofu/environments/prod-indexer
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
