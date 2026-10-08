# Data Models: indexer-deployment

> **No schema migration** (`index.duckdb` stays at version 2) and **no lexicon change**. This file
> specifies config, files, events, the control and health payloads, and the purge plan as logical
> shapes. The crafter chooses the Rust types.

## 1. Configuration (environment)

| Variable | New? | Values | Notes |
|---|---|---|---|
| `OPENLORE_INDEXER_REPO_DIDS` | existing | DID list | Mutually exclusive with `…_FILE`. If both are set, startup is refused (exit 2). |
| `OPENLORE_INDEXER_REPO_DIDS_FILE` | **new** | Path | Read and parsed at the start of every pass (ADR-081) |
| `OPENLORE_INDEXER_CONTROL_SOCKET` | **new** | Filesystem path | Unset means no control channel. `serve` behaves as today. Unix only. A non-path value is refused. |
| `OPENLORE_INDEXER_PURGE_UNLISTED` | **new** | `1` or unset | Any other value is refused. Production uses `1` (ADR-082). |
| `OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB` | **new** | 16..=1024 | Unset keeps the DuckDB default. Production uses 48. Read back by the probe. |
| `OPENLORE_INDEXER_DUCKDB_THREADS` | **new** | 1..=4 | Unset keeps the DuckDB default. Production uses 1. Read back by the probe. |
| `OPENLORE_INDEXER_PASS_DEADLINE_SECS` | **new** | 60..=7200 | Default 1500. When a pass exceeds it, the pass ends with exit 2. |
| Review app: DuckDB memory and threads (B11) | **new** | as above | Production uses 48 MB and 1 thread. The review-app config names follow its own convention (crafter). |
| `OPENLORE_INDEXER_INDEX_PATH`, `…_LISTEN_ADDR`, `…_PLC_ENDPOINT`, `…_SOURCE_URL`, `…_MAX_CONCURRENT_FETCHES`, `…_PER_DID_TIMEOUT_SECS` | existing | unchanged | Production: `/data/index.duckdb`, `0.0.0.0:8080`, default PLC, no fallback, 4, 30 |

Every refusal uses the existing `ConfigError { variable, value, problem }`, which surfaces as
`health.startup.refused` (adapter `config`).

## 2. DID list file

- Path in the container: `/config/repo-dids`. On the host: `/pds/indexer/config/repo-dids`. The
  directory is mounted read-only.
- Content: the SSM value verbatim, with DIDs separated by commas or whitespace. Grammar is
  `parse_repo_dids` (did:plc or did:web, de-duplicated, first-seen order).
- The host writes `.repo-dids.new` in the same directory, sets `chmod 0444` (owned by root), and
  renames that **file** over `repo-dids`. It never swaps the directory. It also touches `.rendered-at`
  on success. The container uid only reads.

Logical per-pass read result:

```
ListRead = Loaded(dids: ordered set of Did)          -- may be empty
         | Refused(Malformed { value })               -- names the first bad entry
         | Refused(Unreadable { cause })              -- missing, permission, I/O
```

## 3. Purge plan (pure, `appview-domain`)

```
plan_purge(list: Loaded(dids), indexed_authors: set of BareDid) -> PurgePlan
PurgePlan = Purge(set of BareDid)        -- bare(indexed_authors) − dids; may be empty
          | Suppressed(EmptyList)        -- dids is empty
```

Invariants (PBT):

1. `Purge(s)` ⇒ `s ∩ dids = ∅`.
2. `s ⊆ indexed_authors`.
3. `dids = ∅` ⇒ `Suppressed`.
4. `plan_purge` is never called with a `Refused` list (made unrepresentable by the argument type).

**BareDid**: the text of an `author_did` before the first `#`. This is the same function the store
already uses for contributor search.

### Purge universe (bounded-change contract of `purge_author(bare)`)

| Store element | Changed iff |
|---|---|
| `indexed_claims` row | `author_did = bare` or `starts_with(author_did, bare || '#')` |
| `indexed_claim_evidence` row | its `cid` is a purged claim's CID |
| `indexed_claim_references` row | its `referencing_cid` is a purged claim's CID. Rows that only **reference** a purged CID are kept. |
| Artifact file at a purged row's stored `signed_record_path` | Its row is purged. Files are never located by recomputing `did_to_fs_segment`, which is not injective. A partition directory is removed only when it is left empty. |
| Everything else | Never changed |

`PurgeReport { claims_removed: u64 }`. The operation is idempotent: purging an absent author returns 0
and changes nothing.

## 4. Events (stdout JSON lines, structural only; WD-105)

Existing events keep their shapes. Additions are marked **new** or **+field**. Every event emitted
during a pass carries `pass_id`, an identifier unique per process lifetime (for example, the pass start
time in RFC3339 plus a sequence number).

| Event | Fields | When |
|---|---|---|
| `indexer.config.loaded` | existing fields, **+** `pass_id`, **+** `repo_dids_source` (`env` \| `file`), **+** `repo_dids_age_secs` (file mode) | At process start (no `pass_id`), and at each in-`serve` pass start after the list loads |
| `indexer.ingest.pass_refused` **new** | `pass_id`, `cause` (`repo_dids_malformed` \| `repo_dids_unreadable`), `variable`, `value`? | The list was refused. No fetch and no purge. |
| `indexer.ingest.author_purged` **new** | `pass_id`, `did` (bare), `claims_removed` | Once per purged author |
| `indexer.ingest.purge_suppressed` **new** | `pass_id`, `reason: "empty_list"` | The list loaded but was empty, with purge enabled |
| `indexer.ingest.source_skipped`, `source_fallback`, `verified`, `rejected` | existing, **+** `pass_id` | unchanged |
| `indexer.ingest.pass_summary` | `configured`, `own_pds`, `fallback`, `skipped`, `duration_ms`, `exit_code`, **+** `pass_id`, **+** `purged_authors`, **+** `cause`? (on exit 2: `repo_dids_malformed` \| `repo_dids_unreadable` \| `upsert_failed` \| `purge_failed` \| `pass_panicked` \| `pass_deadline_exceeded`) | **Exactly once per pass**, last, for every outcome 0 / 2 / 3. On a refused list, counts are 0. |
| `indexer.serve.listening` | existing | unchanged |
| `indexer.search.truncated` **new** | `dimension`, `cap` | A search hit the row cap. No query value is logged. |
| `indexer.search.store_error` **new** | `dimension` | A search could not read the index and was answered 500. No query value and no store error text are logged. For diagnosis; it pages nothing (A3 sees the 500 through the host's canned search). |
| `indexer.trigger.coalesced` **new** (trigger stdout) | `running_pass_id` | A pass was already running. Exit 0. |
| `indexer.trigger.unreachable` **new** (trigger stderr) | `socket`, `cause` | Exit 4 |
| `health.startup.refused` | existing | unchanged |
| `indexer.store.unusable` **new** | `adapter`, `reason` (`mutex_poisoned` \| `io`) | Just before `serve` exits 2 so it can be restarted |
| `indexer.dids.render_failed` (host script, new) | `cause` (no value) | SSM read failed. The last good list is kept. The line is shipped to the indexer log group. |

Exit-code precedence for one pass is unchanged from ADR-078, extended: a refused list, a purge failure
or an upsert failure gives **2**, which takes precedence over **3**, which takes precedence over **0**.

## 5. Control channel (logical protocol over the Unix socket)

- Request: `run_pass`, with no parameters. Everything a pass needs comes from config and the DID file.
- Responses (one per request):
  - `completed { pass_id, exit_code: 0|2|3, summary }`
  - `busy { running_pass_id, started_at }`
- The framing is the crafter's choice: one JSON line each way, or HTTP over the socket. No new crate is
  allowed for it.
- The socket file is created at bind (replacing any stale one), with mode 0600 for the container uid.

## 6. Health response (`GET /healthz`, public)

```
200 {"status":"ok","last_successful_pass_at":"2026-10-06T14:45:12Z" | null}
503 {"status":"store_unusable"}
```

Search returns `500` (no body detail beyond a short reason, no query value) on a store error.

- 200 once the store probe has passed and the listener is bound.
- `last_successful_pass_at` is in memory only and is `null` after a restart until the next exit-0 pass.
- No other fields: no counts, no DIDs, no version.

## 7. Search bounds (public)

| Field | Bound |
|---|---|
| Request body | ≤ 8 KiB (Caddy and the binary) |
| `value` | ≤ 512 bytes |
| Rows returned | ≤ 1000, applied in the store query (`LIMIT`, attributed per-claim rows only) |

The wire `SearchQueryResponse` is unchanged. When the cap is hit, `total_claims` and
`distinct_author_count` describe the returned rows (the pure composition is over what was read).

## 8. Operator freshness query (US-IXD-005)

The operator's freshness view comes from the indexer log group:

- the latest `pass_summary` with `exit_code = 0`;
- the latest `pass_summary` of any kind;
- the last 8 `exit_code`s;
- the `source_skipped` events joined on the same `pass_id`.

No claim content exists in any of these lines, by construction.
