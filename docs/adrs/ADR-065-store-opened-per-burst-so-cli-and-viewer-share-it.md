# ADR-065: The Store Is Opened Per Burst, So the CLI and the Viewer Share It

- **Status**: Accepted (2026-09-30)
- **Date**: 2026-09-30
- **Deciders**: Jeff Bailey (user request: "make it so the CLI and web can both run at the same time")
- **Amends**: ADR-030 (the viewer's `StoreReadPort` over the shared handle)

## Context

DuckDB lets one process hold a database file open read-write, and while any
process holds it, every other process's open fails with `Could not set lock on
file`. `DuckDbStorageAdapter` opened one `Arc<Mutex<Connection>>` at startup and
kept it for the life of the process. A running `openlore ui` therefore held the
store for as long as it served, and every CLI verb (`scrape github`, `claim add`,
`infer people`, ...) failed at wiring time until the viewer was stopped.

ADR-030 accepted this on the assumption that "the viewer runs in its own process
while the CLI is idle in the common case". In practice the viewer is left
running while the CLI is used.

## Decision

`adapter-duckdb` holds the store through `SharedConn` (`crates/adapter-duckdb/src/conn.rs`)
instead of a long-lived connection:

- `SharedConn::lock()` serializes the adapters of one process exactly as the
  mutex did, and opens the file if it is closed.
- A reaper thread closes the connection once no guard has been handed out for
  `IDLE_CLOSE` (100 ms), releasing the file lock. A burst of operations (one
  viewer page's queries, one CLI verb's writes) shares one open.
- If another process holds the file, `lock()` retries with backoff (2 ms, then
  doubling to 50 ms) for up to 15 s, then fails with a plain "store stayed busy"
  error that the adapters pass through instead of the old "mutex poisoned" text.

Within one process there is still exactly one handle per store (Q-DELIVER-3,
BR-VIEW-4): every adapter clones the same `SharedConn`. The viewer stays
read-only at the port level (ADR-030 is unchanged there).

## Consequences

- The viewer and CLI verbs run at the same time over one store, and the viewer
  shows CLI writes on its next request without a restart
  (`tests/acceptance/viewer_cli_concurrent.rs`).
- The first request after an idle period pays one file open (~30 ms on a small
  store); requests in a burst do not.
- A process holds the file for at most its current operation plus 100 ms, so
  the other process usually waits well under a second. Long-running work inside
  one guard would make the other process wait up to the 15 s busy timeout.
- Two openlore processes still never write concurrently. DuckDB's file lock
  serializes them.

## Alternatives considered

- **Viewer opens the file read-only (`access_mode=read_only`)**: rejected. A
  read-only open still conflicts with a read-write holder, so the CLI stays
  locked out while the viewer holds the file.
- **Open and close per operation, no idle window**: correct but multiplied page
  latency about 10x (10 ms to 90-135 ms), because each page runs several queries.
- **Route CLI writes through the running viewer (HTTP)**: rejected. It breaks the
  viewer's read-only guarantee (I-VIEW-1) and makes the CLI depend on the viewer.
