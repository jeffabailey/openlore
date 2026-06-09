# Test Scenarios: viewer-landing-dashboard (slice-17) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09 · ADR: ADR-054
> Per ADR-025, DISTILL authors ALL acceptance tests as scaffolded RED. DELIVER unskips
> + writes PBT unit tests; it does NOT re-author ATs.
> SSOT for executable scenarios = the two Rust test files below; this doc is the
> human-readable map.

## Driving port + layer

Every scenario enters through the REAL `openlore ui` subprocess (`ViewerServer::start`)
+ in-test HTTP `GET /` and asserts on the rendered HTML (Mandate 1; Mandate 8 universe =
the port-exposed rendered surface). NO scenario calls `viewer-domain::render_landing` or
the count reads directly. Layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only (Mandate
9/11 — no PBT machinery at this layer). The local DuckDB is REAL, seeded through the
production write paths (`claim add` / `peer add` / `peer pull`), Pillar 3.

## Two-tier decision (Mandate 10)

**Tier A ONLY.** Tier B (state-machine PBT) is NOT warranted: `GET /` is a single-shot
orientation render — no chained ≥3-scenario journey, no domain-rich input space (three
counts + 8 fixed links). Tier A example coverage is exact. Recorded per the Mandate 10
skip criteria (config/single-shot-shaped feature).

## Story scenarios (`tests/acceptance/viewer_landing_dashboard.rs`)

| # | Scenario (test fn) | Theme / AC | Category | Tags |
|---|---|---|---|---|
| LD-WS | `the_front_door_shows_the_local_store_summary_and_the_full_navigation_hub` | 1 / US-LD-001 | happy / **walking skeleton** | `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy` |
| LD-ZEROS | `a_fresh_empty_store_shows_honest_zero_counts_and_the_full_hub` | 1 Ex 2 | edge / empty-state | `@driving_port @real-io @empty-state @edge` |
| LD-DISCOVER | `the_front_door_links_every_shipped_surface_and_no_deep_route` | 2 / C-3 | happy + negative | `@driving_port @real-io @discoverability @c-3 @happy` |
| LD-URLCONST | `each_surface_link_uses_the_routes_url_constant_including_scrape` | 2 | happy | `@driving_port @real-io @discoverability @scrape-url @happy` |
| LD-READONLY | `the_front_door_exposes_no_write_compose_sign_subscribe_or_follow_control` | 3 / C-1 | guardrail (CARDINAL) | `@driving_port @real-io @read-only @c-1 @cardinal @happy` |
| LD-DEGRADE | `a_failed_peer_claims_read_degrades_to_a_missing_number_state_without_a_5xx` | 4 / C-2 | **error / infra-failure** | `@driving_port @real-io @infrastructure-failure @missing-not-zero @c-2 @cardinal @error` |
| LD-OFFLINE | `the_front_door_renders_fully_with_the_network_down` | 5 / C-2 | happy / offline | `@driving_port @real-io @offline @no-cdn @c-2 @happy` |
| LD-AGGREGATE | `the_store_summary_shows_an_aggregate_count_never_a_merged_consensus_record` | 7 / BR-LD-1 | guardrail | `@driving_port @real-io @anti-merging @c-7 @br-ld-1 @happy` |
| LD-SOFTREMOVED | `a_soft_removed_peer_is_not_counted_in_the_active_peer_summary` | 8 / US-LD-000 | boundary / active-only | `@driving_port @real-io @active-only @br-ld-2 @boundary` |

## GOLD invariants (`tests/acceptance/viewer_landing_dashboard_invariants.rs`)

| # | Invariant (test fn) | Guards | Tags |
|---|---|---|---|
| LD-INV-ReadOnly | `every_landing_render_leaves_the_store_read_only` | C-1 / Mandate 8 (state-delta `unchanged`) | `@read-only @c-1 @gold` |
| LD-INV-NoWrite | `no_landing_response_adds_a_write_or_mutating_control` | C-1 CARDINAL | `@read-only @no-write @c-1 @cardinal @gold` |
| LD-INV-OfflineChrome | `the_landing_page_chrome_stays_offline_no_cdn` | C-2 / KPI-HX-G2 | `@offline @no-cdn @c-2 @gold` |
| LD-INV-Offline | `the_landing_surface_works_fully_offline` | C-2 / KPI-5 | `@offline @c-2 @gold` |
| LD-INV-NoNPlus1 | `the_landing_summary_is_a_fixed_set_of_reads_invariant_to_store_size` | C-4 / I-LD-7 (N+1 proxy) | `@property @no-n-plus-1 @c-4 @gold` |
| LD-INV-MissingNotZero | `missing_is_distinct_from_zero_on_the_front_door` | C-2 / WD-LD-8 / BR-LD-3 (both sides) | `@missing-not-zero @c-2 @cardinal @infrastructure-failure @gold` |
| LD-INV-Discoverability | `the_front_door_links_all_eight_surfaces` | C-3 / WD-LD-7 | `@discoverability @c-3 @gold` |

These INHERIT the slice-06 viewer GOLD invariants (`viewer_is_read_only` /
`store_views_work_offline` / `web_process_holds_no_signing_key`) which cover the
whole-viewer read-only / offline / no-key guarantees; the slice-17 golds add the
FRONT-DOOR specifics (missing≠zero, discoverability completeness, the 3-fixed-reads
no-N+1 proxy) that the near-empty slice-06 `/` did not cover.

## Theme 6 (htmx-vs-no-JS parity) — covered by design, not by a test

ADR-054 D5 makes `GET /` full-page-only (no `Shape` fork). Parity holds BY CONSTRUCTION
— one render means the no-JS page and any htmx request return identical bytes. There is
no fragment to fork, so the slice-15 `Shape` parity scenario pattern is N/A. Recorded as
covered-by-design.

## Coverage / mandate notes

- **Error/edge ratio**: 4 of 9 story scenarios are error/edge/boundary (≈ 44% — above
  the 40% mandate); the GOLD suite adds 2 more failure/degrade golds.
- **CM-A (Mandate 1)**: all scenarios drive the real `openlore ui` subprocess + HTTP; no
  internal `viewer-domain` / read-method import in any test.
- **CM-B/Pillar 1 (Mandate 2)**: scenario titles + step helpers use domain language
  ("front door", "store summary", "own/peer/active", "navigation hub", "missing-number
  state"); no HTTP/SQL/struct-field jargon on the rendered-surface assertions.
- **CM-C (Mandate 3)**: each scenario is a complete operator journey (open the front door
  → orient: what's in my store + where to go).
- **CM-F (Mandate 9) / CM-H (Mandate 11)**: layer-3/5, example-only; sad paths
  (empty-store, failed-read, soft-removed) enumerated explicitly; no PBT machinery
  imported.
- **CM-G (Mandate 10)**: Tier B correctly ABSENT (skip criteria met).
- **Mandate 8 at this layer**: the read-only gold uses `assert_store_read_only` (the
  state-delta universe = the two port-exposed row counts, each `unchanged`); the
  rendered-surface scans are the port-exposed observable (Mandate 8 universe = the HTML
  the browser shows).

## RED classification (fail-for-the-right-reason gate — ADR-025 RED entry)

Run against the current storeless `/` (slice-06 front door):

- **8 story + 4 gold FAIL = RED (MISSING_FUNCTIONALITY)** — counts + hub + missing-marker
  absent; each reaches its business assertion after the real seeds succeed (no
  import/fixture/setup error).
- **1 story + 3 gold PASS = legitimate guardrails** (read-only / no-write /
  offline-chrome / store-read-only) — invariants that hold today and must stay green; not
  Fixture Theater (they regression-guard the new richer page).
- The missing≠zero failed-read seam (`OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`) is a no-op
  until DELIVER materializes the effect-shell degrade arm; the scenario fails at the
  count/hub assertion in the meantime — the SAME RED reason.

DELIVER reads this classification at its RED phase to confirm RED is genuine before
unskipping each scenario for its RED→GREEN→COMMIT cycle.
