# ADR-080: `serve` Owns the Index Store and Runs Each Scheduled Pass In-Process, Triggered by the Host Timer

- **Status**: Accepted (2026-10-07) — implemented, not yet deployed; see `docs/evolution/indexer-deployment-evolution.md` (proposed 2026-10-06)
- **Date**: 2026-10-06
- **Deciders**: Morgan (nw-solution-architect); Jeff Bailey to confirm at DESIGN review
- **Feature**: indexer-deployment (DESIGN). Resolves OQ-IXD-1 (store sharing) and OQ-IXD-2 (container shape).
- **Builds on**: ADR-023 (single-binary indexer, re-buildable `index.duckdb`), ADR-024 (bounded pull pass),
  ADR-065 (DuckDB single-process access), ADR-075 (co-location pattern), ADR-078 (exit codes 0/2/3, events)
- **Corrects**: the `serve` doc comment in `crates/openlore-indexer/src/main.rs` ("runs the pull-ingest loop"),
  which the shipped body does not do (R-IXD-4).

## Context

Production needs `serve` (always on, public search) and a pass every 15 minutes (user decision WD-IXD-2:
a host systemd timer). Both use one `index.duckdb`.

DuckDB lets one process hold a file. A read-only open in a second process still fails with
`Could not set lock on file` (ADR-065). A one-shot `ingest` container next to a long-running `serve`
container would collide 96 times a day. Either the pass exits 2 (a false alarm under WD-IXD-5), or
search fails. FR-IXD-5 and AC-002.2/3 forbid both.

Requirements the mechanism must meet:

| Requirement | Source |
|---|---|
| Search answers in ≤ 1 s p95 during a pass | NFR-IXD-3, AC-002.2 |
| A pass is never refused because search holds the store | FR-IXD-5, AC-002.3 |
| Passes never overlap | FR-IXD-4, AC-002.4 |
| Exit codes 0/2/3 and `pass_summary` stay observable for alerting | FR-IXD-9, ADR-078 §2 |
| No data loss on a crash, and no restore on rollback | C-5, AC-006.2 |
| Indexer peak ≤ 256 MB alongside the PDS and the review app | NFR-IXD-4 |
| The smallest change to the binary | C-4 |

Facts from the code (2026-10-06):

- `serve` already opens the store **twice** in one process: `IndexerWiring::production` opens it for
  the probe, and `serve()` opens it again for the handler. Two DuckDB instances on one file in one
  process both pass the OS lock, because fcntl locks are per process. That is harmless only while
  nothing writes.
- The store adapter serializes every operation on one `Arc<Mutex<Connection>>`. An upsert is one
  short transaction per claim (fix-indexer-follow-ups D3).
- The pass's fetch phase holds no store lock. Only the gate phase's per-claim upserts do.

## Decision

1. **One process, one store handle.** The long-running `openlore-indexer serve` container is the only
   process that opens `index.duckdb` in production. Within that process the store is opened
   **exactly once**, and the probe, the search handler and the pass share that one handle (as in
   ADR-065 "one handle per store"). The second open in `serve()` is removed.
2. **The pass runs inside `serve`, on demand.** `serve` gains a **pass runner** and a **control
   channel** that starts the runner. The pass is the same ADR-077/078 pass that `ingest` runs, with
   the same pure core, events and outcome classification. Only where it is triggered from changes.
3. **The control channel is a Unix domain socket.** It lives at `OPENLORE_INDEXER_CONTROL_SOCKET`,
   on the container's tmpfs. It is not a TCP port, so Caddy, the compose network and the public
   listener cannot reach it. When the variable is unset, `serve` opens no control channel and
   behaves as it does today (local use is unchanged).
4. **A new client verb, `openlore-indexer trigger`.** It connects to the socket, asks for one pass,
   waits for the pass to end, prints the pass result, and exits with the pass's own code:
   **0 / 2 / 3** as in ADR-078 §2. Two more outcomes, neither of which is a pass:
   - **busy**: a pass is already running. No second pass starts. The verb exits **0** with
     `indexer.trigger.coalesced`.
   - **unreachable**: no socket, or the connection was refused or reset (serve is down or was
     restarted mid-pass). The verb exits **4** with `indexer.trigger.unreachable`.
5. **Single-flight is structural.** The runner holds at most one pass. systemd also never overlaps
   activations of one oneshot unit, so FR-IXD-4 holds at two independent layers.
6. **Search latency guarantee.**
   - A pass may hold the store for at most **one store operation at a time**: one claim's upsert, one
     purge step for one author (ADR-082), or the end-of-pass checkpoint.
   - **The pass runs on a dedicated OS thread with its own current-thread runtime.** This is
     mandated, not left to the crafter. The existing pass uses `runtime.block_on` in the gate phase,
     which panics inside the query server's executor, and an async task would also stall search for
     the whole gate phase.
   - **Checkpoint.** DuckDB's automatic WAL checkpoint runs inside a committing transaction, under
     the mutex, and is the longest single store operation. The pass therefore issues an explicit
     `CHECKPOINT` at its end, so that automatic checkpoints during the gate phase are rare and small.
     The AC-002.2 timing test must include a search burst that overlaps that checkpoint, and the
     ≤ 1 s bound covers it.
7. **Robustness: a broken store never looks healthy.**
   - Each pass runs inside `catch_unwind`. A panic gives `pass_summary {exit_code: 2, cause: "pass_panicked"}`
     and clears the single-flight slot, so a later `trigger` is not stuck on busy forever.
   - A **poisoned store mutex**, or any store error that marks the handle unusable, makes `serve`
     emit `indexer.store.unusable` and **exit 2**. `restart: unless-stopped` then reopens the store
     (WAL replay). A process that keeps running on a dead handle is not allowed.
   - Search returns **HTTP 500** on a store error. Today it degrades to an empty 200
     (`run.rs` `rows.unwrap_or_default()`), which looks like "no results" to the client and hides
     the fault.
   - `GET /healthz` returns **503** while the store is marked unusable (ADR-083).
   - *Amendment 2026-10-08 (fix-indexer-deployment-follow-ups RCA, D2):* a search store error is
     now logged as `indexer.search.store_error {dimension}` (never the query value) before the 500,
     and the host health check counts a canned search answering 500 (`search_status = 500`)
     toward `not_live`, so alarm A3 pages a store that cannot serve searches. A 503, 429 or 408
     does not count.
8. **Pass-level deadline.** A pass that has not finished within **25 minutes** (configurable) is
   ended with `pass_summary {exit_code: 2, cause: "pass_deadline_exceeded"}`. Upserts already
   committed stay committed. This bounds the gate phase, whose app-signed author-key resolution is
   outside the per-DID budget (ADR-078 §4). Within a pass, author keys are **cached per author
   DID**, so each author is resolved at most once per pass.

   **Gate each DID as its fetch completes, in config order.** The pass consumes the ordered
   `buffered(cap)` stream item by item. It no longer collects every listing before the gate phase,
   so at most `cap + 1` listings are held in memory at once. The worst case is (4 + 1) × the page
   bound (50 pages × 100 records), about 5 × 10 MB. The realistic figure at today's claim volumes is
   under 1 MB. Event order stays deterministic. The cost is that one author-key request may run
   alongside `cap` fetches (outstanding requests ≤ cap + 1), which amends ADR-078 §3. The re-measure
   gate measures a pass at the real DID count.
9. **`trigger` is dispatched before config parsing and store open.** It needs only
   `OPENLORE_INDEXER_CONTROL_SOCKET`. It must never parse the full indexer config, never run the
   probe gauntlet and never open the store, any of which would contend with `serve` or fail on
   variables it does not need (`run.rs::run` today wires and probes for every verb).
10. **Shutdown.** The container runs with an init process (`init: true`). A distroless binary as
    PID 1 gets no default SIGTERM action, so `docker stop` would wait out the whole grace period
    before sending SIGKILL. Under init, SIGTERM ends `serve` promptly, and any in-flight pass is
    abandoned (crash-safe). `stop_grace_period` is about 5 s, which keeps AC-006.3 (≤ 30 s
    downtime) with margin. The review-app compose has the same PID-1 issue (20 s grace, no init) and
    should adopt `init: true` too (DEVOPS).
11. **Container shape (OQ-IXD-2).**
   - One image, `ghcr.io/jeffabailey/openlore-indexer`, from the one binary.
   - **One** long-running container (`command: ["serve"]`).
   - The host timer runs `docker exec <indexer> openlore-indexer trigger`. No second container and
     no second store open, and the `exec`'d client holds only a socket.
   - The one-shot `ingest` verb stays for local and manual use. Against a store that `serve` holds,
     it fails safe with exit 2 and a lock error, and never corrupts the store.
12. **Every pass ends with exactly one `indexer.ingest.pass_summary` carrying `exit_code`.** This
   includes passes that end in exit 2: list refused (ADR-081), upsert failure, purge failure, panic,
   pass deadline.
   Today an upsert failure returns before the summary. The CloudWatch alarms key on this line, so
   they do not depend on the `trigger` process's exit code reaching CloudWatch.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **(b) Build-and-swap.** The pass copies `index.duckdb`, writes the copy, and renames it over the original. `serve` opens read-only and reopens on a new generation. | Exit codes come for free (`ingest` stays a separate process). But it needs a read-only open mode (migrations cannot run read-only), generation detection and reopen logic in `serve`, and a full file copy every pass. The `indexed_claims/` artifact directory is outside the DuckDB file, so the swap does not cover it, and purge (ADR-082) would need the same treatment. Two processes cost about 2× DuckDB baseline memory on a 1 GiB host. It is a larger change, with three new failure modes (copy, swap, reopen). | Rejected |
| **(c) Per-burst open/close with retry (ADR-065 `SharedConn` in this adapter).** | The gate phase writes for seconds to minutes. Released between upserts, every upsert pays an open (~30 ms) and a checkpoint on close. Held, search waits up to the 15 s busy timeout, which violates ≤ 1 s. Sustained search traffic (the NFR-IXD-7 burst) can starve the writer into the busy timeout, giving exit 2 and a false alarm, which violates AC-002.3. Correct only with luck. | Rejected |
| **(d) An internal 15-minute loop inside `serve` (no host timer).** | It is what the old doc comment describes, and it is the smallest wiring. But it contradicts the user decision WD-IXD-2 (host timer), removes the supervisor-visible per-pass exit code, and puts the schedule out of the operator's reach (a pause would need a redeploy). | Rejected (user decision) |
| **(e) Trigger by signal (`docker kill -s USR1`).** | Smaller than a socket: no new verb. But there is no result channel, so the timer cannot see 0/2/3, the end-to-end exit-code contract (ADR-078) cannot be tested, and coalescing is invisible. It also needs the tokio `signal` feature. | Rejected |
| **(f) Loopback TCP admin listener instead of a Unix socket.** | Equivalent in function. But a mis-set address (`0.0.0.0`) would put the trigger on the compose network, where Caddy could be pointed at it. A socket path cannot be mis-bound to a network. | Rejected |
| **(g) Change store technology (SQLite WAL: many readers, one writer).** | It solves multi-process access natively, but it rewrites the index store, schema and check-arch rules for a deployment feature. | Rejected (disproportionate) |

## Consequences

- **Positive**:
  - Search and passes never contend for the file lock. They contend only for one short transaction.
  - One DuckDB instance gives the lowest memory of any option (NFR-IXD-4).
  - The ADR-078 exit codes survive end to end (`trigger` exits with the pass's code).
  - The double open in `serve` is removed.
  - The host timer stays the single schedule the operator controls.
- **Negative**:
  - The pass now shares fate with `serve`. A deploy or restart during a pass abandons it, and the
    next pass recovers it (idempotent upserts, rebuildable index).
  - A `serve` crash takes both search and ingest down, but the liveness alarm (OQ-IXD-4 user
    decision) covers both.
  - New surface: one verb, one socket, one runner. On non-Unix targets the control channel is
    compiled out, and a set `OPENLORE_INDEXER_CONTROL_SOCKET` is refused there.
- **Exit code 4** is new and belongs to `trigger` only. It means "no pass ran". It never alarms by
  itself, because the liveness alarm detects a dead `serve`.
- **No schema migration in this feature.** A rollback to the previous digest therefore never meets
  a newer schema (the probe refuses a version mismatch) and needs no data restore (C-5). Any future
  schema bump must state its rollback path. The default is to delete the rebuildable index.

## Earned Trust (required fault-injection scenarios)

| Scenario | Required behavior |
|---|---|
| SIGKILL `serve` mid-gate-phase, then restart | The store opens, the probe passes, and every claim committed before the kill is searchable. The next pass completes. |
| A second `trigger` while a pass runs | Exit 0, `coalesced`, and no second pass (no interleaved `pass_summary`). |
| A search burst (100 requests in 10 s) during the gate phase | Every search answers in ≤ 1 s, and the pass exits 0 (not 2). |
| Socket path left over from a crash | `serve` replaces it at bind. The probe does a connect round-trip before `serve` reports ready. |
| `trigger` with `serve` down | Exit 4 and `indexer.trigger.unreachable`. No store is opened by the client. |
| Upsert failure inside the in-process pass (forced store fault) | `pass_summary` with `exit_code: 2` is emitted, `trigger` exits 2, and `serve` keeps answering searches. |
| Panic inside a pass | `pass_summary {exit_code: 2}`, and the next `trigger` starts a new pass (not busy) |
| Poisoned store mutex | `indexer.store.unusable`, then process exit 2 and a restart. `/healthz` is never 200 and search never returns an empty 200 while the store is unusable. |
| Pass exceeds the pass deadline (a hung author-key resolve) | `exit_code: 2` and `cause: pass_deadline_exceeded`. Committed upserts are kept. |
| Search burst during the end-of-pass checkpoint | Every search answers in ≤ 1 s |
| `trigger` with an empty environment except the socket variable | Works, and never opens the store |
| `docker stop` during a pass | Stops within the grace period (init process). The next pass recovers. |
