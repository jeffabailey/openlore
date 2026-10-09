# KPI Instrumentation: bluesky-claim-review-app (DEVOPS)

> Instruments every KPI in `../discuss/outcome-kpis.md`. The rules (OD-BRA-11) are:
>
> - **aggregate only**: no DID, handle or content in any counter;
> - `kpi_counters (day, event, count)`, owned by `adapter-review-store`, with check-arch rule 6
>   forbidding `owner_did` there;
> - a 4-week baseline (a larger denominator at < 50 users), then tune targets.
>
> Data-model additions for DELIVER are marked **[DM]**.

## 1. Cohort problem and solution

Several KPIs are **per-user ratios** (for example "verified users with >= 1 suggestion who
publish in their first session"), yet counters carry no user. The solution is to count each user
**at most once per cohort event**, using owner-scoped flags kept on the owner's own `accounts`
row. The counters stay anonymous, and the ratio of two once-per-user counters is the per-user
rate.

**[DM] `accounts` gains** owner-scoped, purged-on-disconnect columns:

- `first_session_id_hash` (the web session that created the account);
- `first_published_at`;
- `kpi_flags` (a bitset: `eligible_counted`, `published_counted`, `shared_counted`,
  `returning_counted:<iso-week>`).

These are owner data, so the `owner_did` SQL rule applies. Disconnect purges them, so a returning
user who disconnected counts as new. That is acceptable, and documented.

## 2. Event → counter map

Counters are incremented in the **same transaction** as the state change they describe, where
one exists, so a counter can never disagree with the state.

| Counter event (`kpi_counters.event`) | Incremented when | Once per |
|---|---|---|
| `signin.started` | PAR is sent | attempt |
| `signin.completed` / `signin.completed.new_account` | Callback succeeds and the `sub` pin passes; `new_account` if the `accounts` row was created | attempt |
| `signin.denied` / `signin.failed.<reason>` | User cancels or denies, or an error occurs | attempt |
| `github.verify.ok` / `github.verify.fail.<code>` | Ownership verdict | attempt |
| `scan.completed` / `scan.nonempty` | Scan finishes; `nonempty` if new pending > 0 | scan |
| `cohort.first_session.eligible` | The first time an account has verified ownership **and** >= 1 visible suggestion, while in `first_session_id_hash` | **user** |
| `cohort.first_session.published` | The first publish happens in `first_session_id_hash` | **user** |
| `cohort.published` | The first publish ever (sets `first_published_at`) | **user** |
| `time_to_publish.bucket.{le_1m,le_5m,le_15m,gt_15m}` | At the first publish: `first_published_at - accounts.created_at`. Finer buckets **[DM]**: `le_2m`, `le_3m`, `le_5m`, `le_8m`, `le_12m`, `le_15m`, `le_30m`, `gt_30m`, so the median <= 5 and p90 <= 12 can be read. | **user** |
| `suggestion.approved` / `suggestion.approved.edited` | Publish confirmed and read back | claim |
| `suggestion.declined` | Transition to `declined` | claim |
| `review_session.ge4` / `review_session.ge4.with_decline` | At web-session end or expiry (or at the daily rollup for live sessions): the session viewed a queue of >= 4 suggestions; `with_decline` if >= 1 decline happened in it. **[DM]** per-`web_sessions` `queue_max`, `declines` columns. | session |
| `cohort.shared` | First `share.posted` for the account | **user** |
| `share.previewed` / `share.posted` | Share plan created / post created | event |
| `cohort.returning.rescan` / `cohort.returning.acted` | A scan started at least 7 days after the account's previous scan (from `scan_runs`); `acted` if an approve or decline follows in the same ISO week | **user per ISO week** |
| `retract.posted`, `publish.failed`, `disconnect` | as named | event |
| `audit.plan_confirmed.<kind>` / `audit.pds_write.<kind>` | `plan.confirmed` / a successful `UserRepoWritePort.create` (including `RecordAlreadyExists`) | event |
| `audit.scan_gate.verified` / `audit.scan_repos_listed` | Gate yields `VerifiedOwnership` / `list_owned_repos` called | event |
| `audit.reconcile.declined_reoffered` | The post-reconcile check finds a new pending key equal to a declined key | key |
| `audit.readback.self_attested` / `audit.readback.rejected` | After publish, the read-back record goes through the **reader's** pure provenance verdict (claim-domain, ADR-071) | claim |

## 3. KPI → measurement

| KPI | Formula (over a window W, from summed counters) | Target | Type |
|---|---|---|---|
| **KPI-BRA-1** (North Star) | `cohort.first_session.published / cohort.first_session.eligible` | >= 50% | Leading |
| KPI-BRA-2 | Median and p90 read from the cumulative `time_to_publish.bucket.*` distribution | median <= 5 min, p90 <= 12 min | Leading |
| KPI-BRA-3 | `suggestion.approved.edited / suggestion.approved` >= 20% **OR** `review_session.ge4.with_decline / review_session.ge4` >= 30% | either | Leading |
| KPI-BRA-4 (guardrail) | CI: I-BRA-1 property and privacy ATs (blocking). Production: `audit.pds_write.* <= audit.plan_confirmed.*` per kind per day | **0 breaches** | Guardrail |
| KPI-BRA-4b (guardrail) | CI: suppression ATs. Production: `audit.reconcile.declined_reoffered == 0`; the decline path has no write port (structural) | **0** | Guardrail |
| KPI-BRA-5 (guardrail) | Production: `audit.scan_repos_listed <= audit.scan_gate.verified` per day | **0** | Guardrail |
| KPI-BRA-6 (guardrail) | CI: AC-009 ATs. Production: `audit.readback.rejected == 0`, plus the nightly `live-contract-smoke` reader check | **0** | Guardrail |
| KPI-BRA-7 | `cohort.shared / cohort.published` | >= 25% | Leading |
| KPI-BRA-7 (integrity) | `audit.pds_write.post <= audit.plan_confirmed.share` | 0 posts without a confirm | Guardrail |
| KPI-BRA-8 | `cohort.returning.acted / cohort.returning.rescan` | >= 30% | Leading |
| KPI-BRA-9 | `signin.completed.new_account / (signin.started from new visitors)`. The denominator is approximated by `signin.started` minus `signin.completed` of existing accounts. Documented as an approximation: first-time status is unknown before the callback. | >= 90% | Leading |
| Leading: proof success (US-BRA-002) | `github.verify.ok / (github.verify.ok + Σ github.verify.fail.*)` | >= 70% | Leading |
| Leading: non-empty queue | `scan.nonempty / scan.completed` | >= 80% | Leading |
| Guardrail: availability | Route 53 SLI (observability-design §1) | >= 99% monthly | Guardrail |

Small-numbers caveat: at < 50 users, the weekly ratios move in steps of 2-20 percentage points.
Report numerators and denominators together, and judge targets only after the 2-week baseline.

## 4. Collection: daily rollup plus an internal admin endpoint

DuckDB allows **one process** per file, so the `kpi` subcommand cannot open the file while
`serve` runs (infrastructure-integration §6). DELIVER implements both of the following.

1. **Daily rollup log line** (for windows up to the 30-day log retention). At 00:05 UTC the server emits
   `{"event":"kpi.rollup","day":"<yesterday>","counters":{...all events...}}`. On graceful
   shutdown it also emits the current day with `"partial":true`. Logs Insights reads these.
2. **Loopback admin listener** (the primary source for windows longer than 30 days, such as the
   4-week baseline and the 60-day objective, because `kpi_counters` keeps all history). `127.0.0.1:9090` **inside the container's network namespace**.
   Caddy and the PDS cannot reach it, because they reach the app only on `:8080`.
   - Routes: `GET /admin/kpi?from=&to=` (JSON sums) and `POST /admin/purge` (body: a DID).
     *Corrected 2026-10-09:* a third route for alarm tests was designed here but never built;
     A-7 and A-8 are test-fired by putting `guardrail.breach` / `github.token.expiring` lines into
     a `test-fire` log stream (go-live checklist step 15, `deploy/README.md`).
   - `openlore-review-app kpi` and `purge` become **clients** of this listener, run as
     `docker exec <container> /openlore-review-app kpi --from ... --to ...`. That needs no shell
     in distroless. *As built (2026-10-09):* there are no `kpi`/`purge` subcommands and no
     `deploy.sh` wrapper; the listener is `ADMIN_LISTEN_ADDR` (default `127.0.0.1:8081`), read with
     `docker run --rm --network container:review-app-review-app-1 curlimages/curl -s 'http://127.0.0.1:8081/admin/kpi?from=<monday>&to=<sunday>'` on the host (SSM session).
   - check-arch: the admin router module is the only place that binds `127.0.0.1`. A test
     asserts the listener is never bound on `0.0.0.0`.

### 4.1 Guardrail audit (runtime half of KPI-BRA-4/4b/5/6)

At each rollup, and every 15 minutes for the current day, a **pure** function in
`review-domain` evaluates these checks over the day's counters:

| Check | Breach when |
|---|---|
| `writes_exceed_confirms` | `audit.pds_write.<k> > audit.plan_confirmed.<k>` for any kind `k` |
| `scrape_without_gate` | `audit.scan_repos_listed > audit.scan_gate.verified` |
| `declined_reoffered` | `audit.reconcile.declined_reoffered > 0` |
| `readback_not_self_attested` | `audit.readback.rejected > 0` |

The shell logs one `guardrail.breach{kpi,check,count}` per breached check, once per day per
check. Metric filter A-7 then pages. The checks are deliberately **independent of the
structural guarantees**: they would catch a regression that slipped past types and check-arch.

## 5. Dashboards and reading cadence

- **Weekly (product owner = Jeff):** read the sums from the admin listener (§4:
  `docker run --rm --network container:review-app-review-app-1 curlimages/curl -s 'http://127.0.0.1:8081/admin/kpi?from=<monday>&to=<sunday>'` on the host (SSM session)) and work out each section §3 formula with its numerator and denominator. Or use the Logs Insights
  saved query `review-app/kpi-weekly`, which parses `kpi.rollup` lines and sums each counter
  over the window (up to the 30-day log retention). The baseline and the 60-day objective use
  the admin listener.
- **Daily (DEVOPS):** guardrails alarm by themselves (A-7). Nothing to do unless paged.
- **Dashboard:** the optional `openlore-review-app` CloudWatch dashboard (observability-design
  §5) carries a Logs Insights widget with North Star numerator and denominator by week, the
  four leading indicators, and the guardrail count.
- **Baseline:** the first **4 weeks** after announcement (review issue 7). Report numerators and denominators, not percentages, until the baseline is complete. Record it in
  `docs/evolution/bluesky-claim-review-app-kpi-baseline.md` at the DEVOPS close.

## 6. Privacy review of the instrumentation

- Counters have no owner column (check-arch rule 6). Cohort flags live in the owner's row and
  are purged on disconnect.
- Logs carry counts and pseudonymous `owner` hashes only. `kpi.rollup` carries no `owner` at all.
- Bucketed durations only; no timestamps per user leave the DuckDB file.
- The admin endpoint is loopback-only, and reachable only by root on the host through `docker exec`.
