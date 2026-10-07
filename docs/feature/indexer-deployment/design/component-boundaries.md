# Component Boundaries: indexer-deployment

> These boundaries are for the production `serve` process and its host neighbours. Every component lives
> in an existing crate (no new crates). See `architecture-design.md` §5.3 (C4 L3) and §8.

## 1. In-process components (`openlore-indexer serve`)

| Component | Crate / module | Responsibility | Holds (capabilities) | Must NOT hold |
|---|---|---|---|---|
| Public router | `adapter-xrpc-query-server` | HTTP/1 listener. Routes only `POST /xrpc/org.openlore.appview.searchClaims` and `GET /healthz`. Enforces body, value, connection and header bounds. | A search handler closure and a health handler closure | Any store port, the control channel |
| Search handler | `openlore-indexer` (own module) | Runs one dimension query and the pure `compose_results`, and projects flat attributed rows | `IndexReadPort` | `IndexStorePort` (upsert), `IndexPurgePort`, the runner |
| Health handler | `openlore-indexer` | Reports `status` and `last_successful_pass_at` | Read-only `PassStatus` view | Store ports, the runner |
| Control channel | `openlore-indexer` (Unix only) | Accepts one "run a pass" request per connection and replies with the pass result or busy | The runner's start handle | Any network listener (Unix socket only) |
| Pass runner | `openlore-indexer` (own module) | Single-flight pass on a **dedicated OS thread** (its own runtime), inside `catch_unwind`, with a pass deadline: load the DID file, `plan_purge`, purge, fetch and gate each DID as it arrives (author keys cached), `CHECKPOINT`, summary. Publishes `PassStatus`, including whether the store is unusable. | `IndexStorePort` (upsert), `IndexPurgePort`, ingest/listing/lookup/resolve ports, DID-file reader | The public router |
| Pure core | `appview-domain` (+ `parse_repo_dids` in indexer config) | `plan_purge`, `plan_listing`, `ingest_repo_record`, `summarize`, `pass_exit_code` | Nothing (pure) | I/O of any kind |
| Index store adapter | `adapter-index-store` (`lib.rs`, new `purge.rs`) | Implements `IndexReadPort`, `IndexStorePort` and `IndexPurgePort` over **one** DuckDB connection, plus the artifact directory | The DuckDB connection | Any second open of the same file in one process |
| `trigger` verb | `openlore-indexer` | Client: connects to the socket, waits, prints the result, exits 0/2/3 (busy → 0, unreachable → 4) | A socket path | The store (never opens it) |

Dependency direction: router → handlers (closures injected by the composition root); runner → pure
core and ports; adapters → ports. There are no adapter-to-adapter dependencies. The composition root
wires a single `Arc` store and hands each component only the trait it needs (capability injection).

## 2. Host-side components (DEVOPS implements; contracts in architecture-design §9)

| Component | Responsibility | Boundary contract |
|---|---|---|
| `render-dids.sh` | SSM parameter → `/pds/indexer/config/repo-dids` by staging and atomic rename. Keeps the last good copy on a read failure. | Never validates DIDs. Never truncates or deletes the file. Always exits 0. Logs `indexer.dids.render_failed`. |
| `openlore-indexer-pass.timer` and `.service` | 15-minute schedule, `Persistent=true`. Runs render, then `docker exec … trigger`. | The oneshot never overlaps itself. The unit result reflects the `trigger` exit code. |
| `index.caddy` | Site for `index.{$PDS_HOSTNAME}` | Stock directives. Two routes only. 8 KiB body cap. |
| Host health timer (extended) | Probes `https://index…/healthz` through Caddy on loopback, for liveness | Emits a structural line only (no claim content) |
| `deploy/indexer/deploy.sh` | deploy, rollback, status | By digest, signature-verified, readiness through `/healthz`, automatic rollback |

## 3. Reuse analysis (principle 12 extension)

| Existing component | Overlap with this feature | Decision | Contract shape | Universe | Assertion mechanism |
|---|---|---|---|---|---|
| `run::ingest` pass body | The in-`serve` pass is the same pass | **Reuse** (call from the runner). Only add `pass_id` and the guaranteed summary. | bounded-change | Store rows of listed DIDs | Existing ADR-078 ATs, plus the exit-2-summary AT |
| `parse_repo_dids` | The DID file parser | **Reuse** unchanged | pure-function | n/a | Existing CORE-9 PBT, plus a file-source AT |
| `IndexStoreAdapter::open` and probe | The single store handle | **Reuse**. Open once, share an `Arc`. | effect | n/a | AT: the probe runs once on the shared handle |
| `query_by_contributor` bare-DID predicate | Purge author match | **Reuse** the predicate (no `LIKE`) | bounded-change (in purge) | All rows | Purge state-delta PBT with prefix-collision DIDs |
| `IndexStorePort` (read + upsert in one trait) | The search handler needs only reads | **Split** into `IndexReadPort` and `IndexStorePort` (supertrait), so the handler holds reads only | unbounded-preservation (reads) | The whole store | Read snapshot equality, plus the check-arch `indexer_search_handler_read_only` rule |
| `XrpcQueryServer` route | Public router | **Extend** (healthz, bounds). The control channel stays out of it. | effect | n/a | AC-001.3 AT (other paths and methods → 404, index unchanged) |
| Review-app `render-secrets.sh` | DID rendering | **Do NOT mirror** its directory swap (it pins the old inode in the container). A separate script that stages a file in the same directory, sets `chmod 0444` and renames the file. | effect (host) | `/pds/indexer/config/repo-dids` | AT: two consecutive edits seen by two consecutive passes with no restart |
| Review-app `health-timer.sh` | Liveness probe | **Extend** with the index `/healthz` probe, or add an indexer twin (DEVOPS chooses) | effect (host) | n/a | A3 test-fire |

## 4. Boundary rules (enforced)

1. Only the pass runner can delete (`IndexPurgePort`). Enforced by `index_purge_only_in_pass_runner`.
2. Only `adapter-index-store/src/purge*.rs` contains parent-table DELETE SQL. Enforced by
   `index_store_delete_only_in_purge`.
3. The search handler is read-only by type. Enforced by `indexer_search_handler_read_only` and the
   trait split.
4. The control channel is never on a TCP listener. Enforced by config validation, which refuses
   anything that is not a filesystem path, and by an AT that asserts the public listener has no trigger
   path (404).
5. One store open per process. Enforced by an AT: `serve` with the double open removed passes the
   in-process concurrency test.
