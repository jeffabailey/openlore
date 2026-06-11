<!-- markdownlint-disable MD013 -->
# Walking Skeleton: viewer-peer-counter-aware-counts (slice-19) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-10
> Brownfield DELTA — NO walking-skeleton Feature 0 (the `openlore ui` viewer, the read-only
> `StoreReadPort`, the slice-12/13 counter-reference tables, the slice-17 `LandingSummary` +
> `render_landing`, the slice-18 `render_countered` helper + count-only-aggregate + fault-seam,
> and the slice-06/07 `/peer-claims` header all already exist). The thinnest end-to-end slice IS
> US-PC-001 itself.

## Architecture of Reference (port treatments — applied, not re-negotiated)

| Port class | Port | Treatment | Mechanism (this slice) |
|---|---|---|---|
| Driving | `GET /` + `GET /peer-claims` (the `openlore ui` HTTP surface) | Real adapter | `ViewerServer::start` spawns the REAL `openlore ui --port 0` subprocess; in-test HTTP GET |
| Driven internal (real) | `StoreReadPort` → `adapter-duckdb` (`count_countered_peer_claims`, `count_peer_claims`, `list_peer_claims`, `counter_presence_for`) | Real adapter | REAL local DuckDB, seeded through the PRODUCTION `peer add` + `peer pull` + `claim counter` write paths (Pillar 3 / BR-VIEW-4) — NO hand-inserted rows |
| Driven external / non-deterministic | (none on this route) | n/a | the countered-peer count is a LOCAL `COUNT(DISTINCT)` aggregate with NO outbound edge — offline-STRONGER than `/search`/`/scrape` |

There is NO external/network boundary on `/` or `/peer-claims`; the only "fake" is each peer's
read-only PDS double (`PeerPds`) used to seed the cached peer claims through the production
federation pull — the viewer never touches it (it reads the LOCAL store post-pull).

## The walking-skeleton scenario (the riskiest-assumption thread)

`PC-WS` — `the_front_door_shows_how_many_cached_peer_claims_are_countered`
(`@us-pc-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy`):

```gherkin
Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens GET / in the openlore ui viewer
Then the landing summary shows "4 peer claims" with "(1 countered)" beside it
And the peer-claims count "4" is unchanged by the presence of the countered count
```

It closes the thinnest complete thread the slice can demo: REAL `openlore ui` subprocess → the
LOCAL countered-PEER-count aggregate (`count_countered_peer_claims()`, ADR-056 D1 — the slice-18
SQL with outer table swapped `claims c → peer_claims p`) → the extended `LandingSummary` (5th
additive `Option<usize>` field) → the REUSED `render_countered` helper → the front-door summary
showing the disputed-peer-claim awareness count beside the unchanged peer-claims count, read-only,
offline. A non-technical stakeholder confirms: "yes — at a glance, how much of the peer material
I've cached has been disputed."

## Driving-adapter coverage (Mandate — RCA P1)

`GET /` and `GET /peer-claims` are exercised via the REAL `openlore ui` SUBPROCESS + in-test HTTP
GET (`ViewerServer::start(&env).get(LANDING_PATH | PEER_CLAIMS_LIST_PATH)`), asserting HTTP status
(200) + the rendered body (the count surface). NO scenario calls `render_landing` /
`render_peer_claims_page` / `render_countered` / the count read directly — every assertion is on
the rendered HTML the operator's browser shows (Mandate 1 + Mandate 8 universe = port-exposed
rendered surface).

## Adapter integration coverage (Mandate 6)

| Driven adapter | Real-I/O coverage | Covered by |
|---|---|---|
| `adapter-duckdb` `count_countered_peer_claims` (NEW, ADR-056) | YES — REAL DuckDB `COUNT(DISTINCT)` over production-written rows | EVERY scenario (the seeds pin the genuine count via the direct ADR-056 oracle `read_countered_peer_claims_count`); the no-N+1 scenario inflates the store and re-reads it |
| `adapter-duckdb` `count_peer_claims` / `list_peer_claims` (existing) | YES — REAL DuckDB | the peer-total + no-regression byte-identity scenarios |
| `adapter-http-viewer` `landing_page` / `peer_claims_page` (driving) | YES — REAL subprocess HTTP | every scenario |

There is exactly ONE driven-internal adapter touched (the DuckDB read adapter); it is exercised
with REAL I/O in every scenario. NO costly external dependency, so no `@requires_external` smoke.

## Layer placement (Layered Test Discipline matrix; Mandate 9/11)

Every scenario is a **layer-3/layer-5 subprocess + real-I/O** test — EXAMPLE-only. Sad paths
(honest "(0 countered)", failed countered-peer-count read → "(— countered)") are enumerated
explicitly, never PBT-generated at this layer. **Tier B (state-machine PBT) is NOT warranted**
(Mandate 10 skip criteria): a single-shot additive render with no chained ≥3-scenario journey and
no domain-rich input space (one `Option<usize>`) — Tier A example coverage is exact. The single
`@property`-tagged scenario (no-N+1) is an EXAMPLE-shaped invariant assertion (read-count invariant
to store size), consistent with the slice-12/17/18 single-aggregate gold posture.

## Build-before-run (DELIVER roadmap note)

`cargo test` does NOT rebuild a spawned binary. The DELIVER roadmap MUST `cargo build` the
`openlore` bin (the viewer) before running these ATs so `ViewerServer::start` spawns the CURRENT
viewer, not a stale one. The count is a LOCAL DuckDB read — no second binary needed.
