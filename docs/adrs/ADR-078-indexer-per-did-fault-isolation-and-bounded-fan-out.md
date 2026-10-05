# ADR-078: Per-DID Fault Isolation, Bounded Fan-Out and Pass Observability for the Indexer Ingest Pass

- **Status**: Accepted (2026-10-05) — implemented; see `docs/evolution/indexer-per-did-pds-fetch-evolution.md` (proposed 2026-10-05). User decisions (2026-10-05): defaults confirmed (cap 4, 30 s); total outage exits 3.
- **Date**: 2026-10-05
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: indexer-per-did-pds-fetch (DESIGN)
- **Realizes**: ADR-024 §"Per-record / per-source fault isolation" (the `source_skipped` row was
  specified but not implemented: `run.rs` returns exit 2 on the first listing error).
- **Amends**: ADR-024 (adds concrete bounds and event shapes).

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
4. **Time bound.** One deadline per DID: `per_did_time_budget = 30 s` (env
   `OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS`, 1..=600), applied with `tokio::time::timeout_at`
   across resolve and listing, including a fallback listing. Expiry while resolving counts as a
   resolution failure (the fallback gets the remaining budget). Expiry while listing gives
   `pds_timeout`. The worst-case pass is ⌈N/cap⌉ × budget (N = 50 → 6.5 min). The budget does
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
