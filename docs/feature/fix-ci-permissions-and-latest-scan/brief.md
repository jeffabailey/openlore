# Product brief: fix\-ci\-permissions\-and\-latest\-scan

## Request

```text
Harden CI workflow security checks (scale-to-zero-hosting LOW): add top-level permissions: contents: read to .github/workflows/ci.yml, and extend the :latest image-pin scan in .github/workflows/deploy-pds-check.yml to cover deploy/indexer and deploy/tofu/environments/prod-indexer in addition to compose and deploy/review-app/deploy.sh.
```

## Work

M

```text
Enforce least-privilege default GITHUB_TOKEN permissions in ci.yml and prevent unpinned :latest image references in indexer deploy scripts and prod-indexer tofu
```

### Rework

None.

## Promises

### P1

Outcome

```text
ci.yml declares top-level permissions with contents: read
```

### P2

Outcome

```text
deploy-pds-check.yml scans deploy/indexer and deploy/tofu/environments/prod-indexer for :latest references
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
ci.yml declares top-level permissions: contents: read, and deploy-pds-check.yml scans deploy/indexer and deploy/tofu/environments/prod-indexer for :latest references
```

#### Promises

P1

P2

#### Depends on

None.

#### Slice work

None.
