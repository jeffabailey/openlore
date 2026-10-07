# ADR-078: Per-DID Fault Isolation, Bounded Fan-Out and Pass Observability for the Indexer Ingest Pass

- **Status**: Accepted (2026-10-05) — implemented; see `docs/evolution/indexer-per-did-pds-fetch-evolution.md` (proposed 2026-10-05). User decisions (2026-10-05): defaults confirmed (cap 4, 30 s); total outage exits 3.
- **Date**: 2026-10-05
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: indexer-per-did-pds-fetch (DESIGN)
- **Realizes**: ADR-024 §"Per-record / per-source fault isolation" (the `source_skipped` row was
  specified but not implemented: `run.rs` returns exit 2 on the first listing error).
- **Amends**: ADR-024 (adds concrete bounds and event shapes).
- **Amended 2026-10-05** (fix-indexer-follow-ups, `docs/feature/fix-indexer-follow-ups/rca.md`
  D1): §4 used to give the fallback "the remaining budget". When resolving used up the budget,
  the fallback ran on an expired deadline and the DID was skipped with `pds_timeout`, which broke
  ADR-077 NFR-3 ("when PLC is down the fallback reproduces the old outcome"). User decision: every
  fallback listing gets a fresh per-DID budget. The pure `plan_listing` still makes the decision
  (`ListingSource::budget()`: `OwnPds` → remaining of the shared deadline, `Fallback` → fresh);
  the shell only builds the deadline. Accepted cost: worst-case time per DID, and so per pass,
  can double.
- **Amended 2026-10-06** (indexer-deployment, ADR-080/081/082; user decision OQ-IXD-9 "PURGE"):
  - **§1, skips vs removals.** A skip still persists nothing and still deletes nothing. A DID that the
    operator **removed from the configured list** is different: with `OPENLORE_INDEXER_PURGE_UNLISTED=1`
    (production), its indexed claims are purged at the start of the next pass through a separate
    `IndexPurgePort` that only the pass runner holds (ADR-082). The purge set is the pure difference
    `indexed authors − configured list`, so a listed-but-skipped DID can never be purged. An empty list
    suppresses purge, and a malformed or unreadable list refuses the pass before any purge. With the
    flag unset, the original §1 sentence holds unchanged.
  - **§2, exit codes.** A purge store failure is a local fault, so it gives **2**. A refused DID-list
    file (ADR-081) gives **2**. Every pass, including one ending in 2, emits exactly one `pass_summary`
    with `exit_code` (previously an upsert failure returned before the summary).
  - **§5, events.** `pass_summary` gains `pass_id`, `exit_code` (already emitted, now part of the
    contract) and `purged_authors`. `indexer.config.loaded` is also emitted at the start of each pass
    run inside `serve`. New events: `indexer.ingest.pass_refused`, `indexer.ingest.author_purged` and
    `indexer.ingest.purge_suppressed`. Every pass event carries `pass_id`.
  - **§3, fan-out.** The gate phase now consumes each DID's listing as the ordered `buffered(cap)`
    stream yields it, instead of after all fetches complete. This bounds the listings held in
    memory to `cap + 1`. Outstanding outbound requests become ≤ cap + 1 (one author-key resolve
    alongside the fetches). Author keys are cached per author per pass. A pass-level deadline
    (25 min) ends an overrunning pass with exit 2 (ADR-080 §8).
  - Exit code **4** exists only for the new `trigger` client and means "no pass ran: `serve` unreachable"
    (ADR-080).

## Context

ADR-024 requires that an unreachable source DID be skipped while the pass continues. The shipped
`ingest` exits 2 on the first failed listing, so one dead PDS hides every author's new claims.
Per-DID fetch (ADR-077) multiplies the number of independent hosts, which makes isolation
mandatory and makes bounding the fan-out a courtesy to third parties (bsky.social). The scale is
under 50 authors, with a solo maintainer, so there is no appetite for new infrastructure.

## Decision

1. **Unit of failure = one DID.** Each DID's resolve + plan + list runs as one unit that yields
   `DidFetch::Read` or `DidFetch::Skipped { reason, fallback_used, pds_url, fallback_failure }`.
   Reasons are `did_unresolvable | pds_unreachable | pds_timeout | listing_failed |
   pds_address_refused`. The pure `classify_fetch_failure` maps them:
   - transport error, 5xx or 429 → `pds_unreachable`
   - other non-2xx (including 3xx, since redirects are disabled) or a malformed body →
     `listing_failed`
   - deadline → `pds_timeout`
   - the SSRF guard refused the address (ADR-077 §4) → `pds_address_refused`

   A skip persists
   nothing. The next pass retries the DID. `IndexStorePort` has no delete, so skipping can never
   remove indexed rows.
2. **Exit codes (user decision 2026-10-05).**
   - **0**: the pass completed and at least one DID was listed (own PDS or fallback), or no DIDs
     are configured. A partial skip is still 0.
   - **3**: the pass completed but **every** configured DID was skipped (`configured ≥ 1` and
     `own_pds + fallback == 0`). This is a total source outage. The `verified`, `rejected` and
     `pass_summary` events are emitted **before** exiting. The decision is a pure
     `pass_exit_code(&PassSummary)`.
   - **2**: startup refusal (config or probe), runtime construction failure, or an
     **index-store upsert failure**. The upsert case is a local fault: continuing would silently
     drop verified claims every pass. Rows upserted before the failure stay committed. An upsert
     failure ends the pass immediately, so exit 2 takes precedence over 3.
3. **Bounded fan-out.** Fetch phase: `futures-util` `stream::iter(dids).map(fetch_unit)
   .buffered(max_concurrent_fetches)` on the existing current-thread runtime. Gate phase
   (decode, author key, pure gate, upsert) runs sequentially afterwards, in config order. Each
   unit issues its requests sequentially, so outstanding PLC/PDS requests ≤ the cap (NFR-1).
   Defaults: `max_concurrent_fetches = 4` (env `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES`,
   1..=16). The per-repo page bound is unchanged (`MAX_PAGES` 50 × 100).
4. **Time bound.** A per-DID budget: `per_did_time_budget = 30 s` (env
   `OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS`, 1..=600), applied with `tokio::time::timeout_at`.
   Resolving and an own-PDS listing share one deadline. A fallback listing runs under a fresh
   budget of its own (see the amendment below). Expiry while resolving counts as a resolution
   failure, so the DID goes to the fallback when one is configured. Expiry while listing gives
   `pds_timeout`. The worst-case DID costs two budgets (resolve, then fallback), so the
   worst-case pass is ⌈N/cap⌉ × 2 × budget (N = 50 → 13 min). The budget does
   not cover app-signed author-key resolution in the gate phase, which is unchanged and bounded
   by the resolve adapter's own client timeout.
5. **Observability (stdout JSON, `indexer.*`, WD-105).**
   - `indexer.ingest.source_skipped`: `{did, reason, fallback_used, pds_url?, fallback_failure?, detail?}`, one per skipped DID.
   - `indexer.ingest.source_fallback`: `{did, reason:"did_unresolvable", fallback_url}`, one per DID read via the fallback.
   - `indexer.ingest.pass_summary`: `{configured, own_pds, fallback, skipped, duration_ms}`, with own_pds + fallback + skipped == configured.
   - `indexer.ingest.rejected.by_reason` gains `foreign_repo` (FR-4 records of another repo, previously filtered silently).
   - `indexer.config.loaded`: `{repo_did_count, fallback_configured, fallback_url?, max_concurrent_fetches, per_did_time_budget_secs, plc_endpoint}`. This is how an empty DID list or an absent fallback is "reported, not refused".
   - The `stats` verb is not extended.
6. **Config refusal.** The pure `parse_config` refuses on a malformed DID entry, an unsupported
   method, a malformed or non-https fallback URL (or one that is a refused IP literal), an
   out-of-range bound, or `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP` set in a release build. It refuses through the existing
   `health.startup.refused` shape (`adapter:"config"`, `reason: IndexerConfigInvalid`,
   `structured:{variable, value}`) with exit 2.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Sequential only (cap fixed at 1)** | Rejected as the default. One slow host per DID serializes the budget (50 × 30 s). Still allowed via env. |
| **`JoinSet` + `Semaphore`** | Rejected. It needs `'static + Send` tasks (`Arc`-wrapped wiring, tokio `sync` feature) for no benefit at this scale. |
| **Interleave the gate with fetches (`buffer_unordered`, process as they arrive)** | Rejected. Author-key resolution would add requests beyond the cap, and event order would become non-deterministic. |
| **Exit non-zero when some DIDs were skipped** | Rejected. It contradicts AC-002.1, and cron/systemd would treat a routine outage of one third-party host as a failure. Only total outage is non-zero (exit 3). |
| **Exit 0 even on total outage (summary only)** | Rejected by the user (2026-10-05). A supervisor must be able to detect "indexed nothing" without parsing events. |
| **Reuse exit 2 for total outage** | Rejected. Exit 2 means "fix the local deployment" (config, probe, store). Exit 3 means "every remote source failed this pass", which is usually transient. A distinct code lets the operator alert on them differently. |
| **Upsert failure skips only the DID** | Rejected. A local store fault would recur for every DID and silently drop verified claims. Fail loud (OQ-IPF-5). |
| **Per-host rate limiting** | Deferred. The cap and page bound already keep load to tens of requests per pass. Revisit if a host returns 429 routinely (visible as `pds_unreachable` detail). |

## Consequences

- **Positive**: one dead PDS no longer hides everyone (KPI: 0 passes aborted by one DID). Every
  skip is attributable. Bounds are explicit and tunable without code.
- **Negative**: a pass that lists at least one DID but skips most still exits 0, so operators
  should still watch `pass_summary`. A supervisor that treats any non-zero code as fatal must
  learn that 3 is usually transient. The event count per pass grows linearly with skipped DIDs
  (fine at under 50).
- **Earned Trust**: ATs drive fake PLC/PDS hosts through the lie catalogue (hang, 502, 429,
  wrong repo, endless cursor, refused/private address, redirect, total outage → exit 3), see
  `docs/feature/indexer-per-did-pds-fetch/design/architecture-design.md` §9.
