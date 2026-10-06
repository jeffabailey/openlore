# Shared artifacts registry: indexer-deployment

| Artifact | Single source of truth | Producer | Consumers | Risk if inconsistent |
|---|---|---|---|---|
| `${image_digest}` | The CI-pushed GHCR digest for a main commit (`sha-<40 hex>` tag, then the digest) | CI image job | deploy script (S1, S8), host pin file, release history | Wrong or unsigned binary in production |
| `${public_index_url}` | `https://index.openlore.jeffbailey.us` (the Caddy site plus wildcard DNS) | Caddy site file | Maria's `OPENLORE_INDEXER_URL` / `[appview] indexer_url`, the freshness and smoke checks | Search unreachable, or pointed elsewhere |
| `${did_list_param}` | SSM parameter, proposed `/openlore/prod/indexer/repo-dids` (DESIGN finalizes) | Jeff | host renderer | Edits ignored |
| `${did_list_file}` | The host-rendered read-only file, with the last good copy kept | host renderer | ingest pass (`OPENLORE_INDEXER_REPO_DIDS`) | Stale or empty list (FR-IXD-7) |
| `${index_store}` | One `index.duckdb` on the data volume (re-buildable) | ingest pass | serve | Lock conflicts (FR-IXD-5) |
| `${exit_code}` | Process exit of each pass: 0, 2 or 3 (ADR-078 §2) | ingest pass | alarms (S6), freshness (S7) | Missed or noisy alerts |
| `${pass_summary}` | The `indexer.ingest.pass_summary` event `{configured, own_pds, fallback, skipped, duration_ms}` | ingest pass | freshness (S7) | Misread health |
| `${last_successful_pass_at}` | Derived: the timestamp of the latest pass with exit 0 | freshness command | Jeff (S7) | False sense of freshness |
| `${alarm_topic}` | The existing SNS `openlore-pds-backup-alarm` | module (exists) | 2 indexer alarms | Alerts go nowhere |

Vocabulary is consistent across all artifacts: "pass" (never "run" or "sync"), "DID list",
"public index", "freshness", "exit 2 / exit 3".
