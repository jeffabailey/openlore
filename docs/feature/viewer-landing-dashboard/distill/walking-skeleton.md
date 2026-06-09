# Walking Skeleton: viewer-landing-dashboard (slice-17) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09 · ADR: ADR-054

## The one walking-skeleton scenario

`the_front_door_shows_the_local_store_summary_and_the_full_navigation_hub`
(`tests/acceptance/viewer_landing_dashboard.rs`, tagged `@walking_skeleton
@driving_port @driving_adapter @real-io @kpi-view-1 @happy`).

```
Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions
      (did:plc:rachel-test, did:plc:tobias-test)
When  she opens GET / in the openlore ui viewer
Then  she sees a store summary showing 12 own claims, 7 peer claims, and 2 active peers,
      the full navigation hub to all 8 shipped surfaces, and the read-only notice
```

## Why this is the thinnest demo-able thread

It closes the complete slice loop in one scenario: viewer process → THREE LOCAL
aggregate reads (`count_claims`, `count_peer_claims`, the new
`count_active_peer_subscriptions`) → `Result→Option` via `.ok()` in the effect shell →
`LandingSummary` → the pure `render_landing(&summary)` → a full HTML page with the 3
counts + the 8-surface nav hub. It exercises the riskiest assumption — that the storeless
front door can be threaded with the read-only store and surface a real summary + hub
without breaking read-only / offline / no-key — and is demo-able to a non-technical
stakeholder: "open the viewer and orient — what's in my store, and where can I go."

## Litmus test (Mandate 5 / Dimension 5)

1. **Title = user goal?** YES — "the front door shows the LOCAL store summary and the
   full navigation hub" (orientation), not "GET / threads the store through the layers".
2. **Given/When = user actions/context?** YES — "Maria's store has 12/7/2; she opens
   GET /". Not "the route arm gains store.as_ref()".
3. **Then = user observations?** YES — "she sees 12 own claims, 7 peer claims, 2 active
   peers + links to all 8 surfaces + the read-only notice". Not "a LandingSummary struct
   is built with three Some values".
4. **Stakeholder confirms "yes, that is what users need"?** YES — the front door is the
   first surface the operator sees; the summary + hub are the orientation J-002 promises.

## Architecture of Reference posture (per-port treatment)

| Port class | Port | Treatment in the WS |
|---|---|---|
| Driving (entry) | the `openlore ui` CLI verb + `GET /` HTTP | REAL — `ViewerServer::start` spawns the production bin; in-test HTTP GET |
| Driven internal (shared state) | the read-only DuckDB store (`StoreReadPort`) | REAL — the SAME store the CLI writes, via `OPENLORE_HOME`; seeded by the production `claim add` + `peer add` + `peer pull` verbs (Pillar 3) |
| Driven external / non-deterministic | none on `/` | N/A — `/` is LOCAL + OFFLINE; no clock/email/network/LLM port. Offline-STRONGER than `/search` (indexer) and `/scrape` (GitHub) |

No external/non-deterministic port exists on the front door, so no fake/stub is wired —
consistent with the Project Infrastructure Policy (the viewer's driven-internal store is
real-via-`OPENLORE_HOME`; no external port to fake). This is the cleanest WS in the viewer
series.

## Seed (production write paths — Pillar 3)

`seed_landing_store_summary(env)`:
- 12 own claims via `seed_own_claims_via_cli` (real `claim add`).
- 7 peer claims via `seed_peer_authored_graph` (real `peer add` + `peer pull`, Rachel, 7
  distinct triples) — Rachel becomes an active subscription too.
- Tobias as a second active subscription via `seed_active_subscription_for` (real `peer
  add`, no pull — the active-peer COUNT reads `peer_subscriptions`).
- Pins the genuine 3-count shape (`assert_user_author_claim_count` 12,
  `assert_peer_claims_row_count_for` Rachel 7, `assert_one_active_subscription_for` ×2)
  so the fixture is the REAL state, not merely "the verbs exited 0".

## RED status

RED (MISSING_FUNCTIONALITY). The current storeless `/` renders the slice-06 front door
(`<h1>` + `READ_ONLY_NOTICE` + one `/claims` link); the WS reaches
`assert_landing_shows_count(body, "own claims", 12)` AFTER the real seeds succeed and
FAILS because the production summary + hub do not exist yet. The AT drives `GET /` via
subprocess HTTP (never the Rust `render_landing` signature), so DELIVER's production
signature change (`render_landing(&LandingSummary)`) does not affect AT compilation. The
WS turns GREEN in DELIVER once the store is threaded, the 3 counts resolved, and the
extended `render_landing` ships the summary + hub.
