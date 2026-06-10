# Test Scenarios — viewer-counter-aware-counts (slice-18) · DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09 · ADR-055
> The `.feature`-equivalent SSOT for executable scenarios is the two Rust AT files:
> `tests/acceptance/viewer_counter_aware_counts.rs` (story scenarios) +
> `tests/acceptance/viewer_counter_aware_counts_invariants.rs` (GOLD). This file is the
> structured summary (scenario list + tags + AC mapping), not a duplicate of the bodies.

## Reconciliation HARD GATE

**Reconciliation passed — 0 contradictions.** Inputs read: DISCUSS `wave-decisions.md`
(WD-CC-1..12), DESIGN (ADR-055 + the feature-delta DESIGN section). No separate DESIGN/DEVOPS
`wave-decisions.md` files exist (brownfield; DESIGN decisions are recorded in ADR-055 + the
feature-delta). DESIGN RESOLVES the two inherited open DISCUSS questions CONSISTENTLY:
WD-CC-5 → count-only `count_countered_own_claims` aggregate; WD-CC-7 → own-claims-only. No
DISCUSS decision is contradicted (read-only, LOCAL/offline, missing≠zero, presence-once,
single-source, additive-no-regression, anti-misread are all upheld and tightened, not
reversed). No DEVOPS wave (inherits the viewer infra — clean local DuckDB + subprocess HTTP).

## Driving ports

`GET /` (landing) and `GET /claims` (My Claims list) exercised port-to-port via the REAL
`openlore ui` subprocess (`ViewerServer::start`) + in-test HTTP GET. No scenario calls a
`viewer-domain` render fn or a read method directly (Mandate 1 driving-port discipline).

## Story scenarios (`viewer_counter_aware_counts.rs`)

| ID | Scenario fn | Theme / AC | Tags |
|---|---|---|---|
| CC-WS | `the_front_door_shows_how_many_own_claims_are_countered` | T1 / US-CC-001 (headline "12 own claims (3 countered)") | `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy` |
| CC-HEADER | `the_claims_header_shows_the_same_countered_count_as_the_landing` | T1 Ex2 / US-CC-002 (single source) | `@us-cc-002 @driving_port @real-io @single-source @wd-cc-8 @happy` |
| CC-PRESENCE | `a_claim_countered_by_multiple_peers_counts_once` | T2 / US-CC-000/001 (presence-once) | `@driving_port @real-io @presence-once @c-4 @br-cc-1 @cardinal @boundary` |
| CC-ZERO-LANDING | `an_honest_zero_countered_on_the_landing_when_nothing_is_disputed` | T3 / US-CC-001 (honest "(0 countered)") | `@driving_port @real-io @honest-zero @c-5 @edge` |
| CC-ZERO-HEADER | `an_honest_zero_countered_in_the_claims_header_list_as_slice_06` | T3 / US-CC-002 (honest zero, list as slice-06) | `@driving_port @real-io @honest-zero @no-noise @c-5 @edge` |
| CC-DEGRADE-LANDING | `a_failed_countered_count_read_degrades_gracefully_on_the_front_door` | T4 / US-CC-000/001 (missing≠zero) | `@driving_port @real-io @infrastructure-failure @missing-not-zero @c-2 @c-5 @cardinal @error` |
| CC-DEGRADE-HEADER | `a_failed_header_count_degrades_without_blanking_the_claims_list` | T4 / US-CC-000/002 (missing≠zero) | `@driving_port @real-io @infrastructure-failure @missing-not-zero @c-2 @c-5 @cardinal @error` |
| CC-NO-REWEIGHT | `the_countered_count_never_re_weights_the_own_claims_count` | T5 / US-CC-001 (additive, "12" unchanged) | `@driving_port @real-io @additive @c-4 @anti-misread @happy` |
| CC-NO-REORDER | `the_claims_header_count_does_not_re_order_filter_or_re_weight_the_list` | T5 / US-CC-002 (byte-identical list) | `@driving_port @real-io @additive @no-regression @c-4 @wd-cc-9 @happy` |
| CC-READONLY-LANDING | `the_counter_aware_front_door_exposes_no_write_control` | T6 / US-CC-001 (C-1 CARDINAL) | `@driving_port @real-io @read-only @c-1 @cardinal @happy` |
| CC-READONLY-HEADER | `the_counter_aware_claims_header_adds_no_write_control` | T6 / US-CC-002 (C-1 CARDINAL) | `@driving_port @real-io @read-only @c-1 @cardinal @happy` |
| CC-OFFLINE-LANDING | `the_front_door_countered_count_renders_fully_with_the_network_down` | T7 / US-CC-001 (C-2 CARDINAL) | `@driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy` |
| CC-OFFLINE-HEADER | `the_claims_header_countered_count_renders_offline` | T7 / US-CC-002 (C-2 CARDINAL) | `@driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy` |
| CC-NO-N-PLUS-1 | `the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_size` | T8 / US-CC-000 (no-N+1) | `@property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @boundary` |
| CC-ANTI-MISREAD | `the_countered_count_is_neutral_awareness_never_a_verdict_or_penalty` | T9 / US-CC-001 (neutral copy) | `@driving_port @real-io @anti-misread @c-6 @wd-cc-10 @presence-once @boundary` |

## GOLD invariants (`viewer_counter_aware_counts_invariants.rs`)

| ID | Scenario fn | Invariant / Source |
|---|---|---|
| CC-INV-ReadOnly | `every_counter_aware_render_leaves_the_store_read_only` | C-1 — / + /claims leave the store byte-unchanged (Mandate 8 universe-bound `assert_store_read_only`) |
| CC-INV-NoWrite | `no_counter_aware_render_adds_a_write_or_mutating_control` | C-1 CARDINAL — count is render-only text, no sort/filter/mutating control |
| CC-INV-OfflineChrome | `the_counter_aware_chrome_stays_offline_no_cdn` | C-2 — vendored /static/htmx.min.js, no CDN (both pages) |
| CC-INV-Offline | `the_counter_aware_surfaces_render_fully_offline` | C-2 — count is a LOCAL read, no outbound edge (both pages) |
| CC-INV-NoNPlus1 | `the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_size` | C-3 CARDINAL — one aggregate read invariant to store size |
| CC-INV-MissingNotZero | `missing_is_distinct_from_zero_for_the_countered_count` | C-2 / C-5 / WD-CC-6 CARDINAL — "(— countered)" ≠ "(0 countered)"; both sides |
| CC-INV-PresenceOnce | `a_claim_countered_by_two_peers_counts_once` | C-4 / BR-CC-1 CARDINAL — "(1 countered)" never "(2 countered)" (both surfaces) |
| CC-INV-SingleSource | `the_landing_and_claims_header_counts_are_consistent` | WD-CC-8 — landing "(N countered)" == /claims header |
| CC-INV-NoRegression | `the_claims_list_is_byte_identical_to_the_no_header_count_baseline` | C-4 / WD-CC-9 — list order/paging/count/confidence + slice-12 flags byte-identical |

## Coverage + ratios

- **Story-to-scenario (Dim 8 Check A)**: US-CC-000 (read wiring) → CC-PRESENCE, CC-DEGRADE-*,
  CC-NO-N-PLUS-1 + the gold proxies. US-CC-001 (landing) → CC-WS, CC-ZERO-LANDING, CC-NO-REWEIGHT,
  CC-READONLY-LANDING, CC-OFFLINE-LANDING, CC-ANTI-MISREAD. US-CC-002 (`/claims` header) →
  CC-HEADER, CC-ZERO-HEADER, CC-DEGRADE-HEADER, CC-NO-REORDER, CC-READONLY-HEADER,
  CC-OFFLINE-HEADER. Every story covered. PASS.
- **9 acceptance-criteria themes**: all 9 mapped (T1→CC-WS+CC-HEADER, T2→CC-PRESENCE,
  T3→CC-ZERO-*, T4→CC-DEGRADE-*, T5→CC-NO-REWEIGHT+CC-NO-REORDER, T6→CC-READONLY-*,
  T7→CC-OFFLINE-*, T8→CC-NO-N-PLUS-1, T9→CC-ANTI-MISREAD). PASS.
- **GOLD table**: all 9 acceptance-criteria GOLD invariants mapped to a CC-INV-* gold test. PASS.
- **Error/edge ratio (Dim 1)**: of 15 story scenarios, the error/edge/boundary set is
  CC-PRESENCE, CC-ZERO-LANDING, CC-ZERO-HEADER, CC-DEGRADE-LANDING, CC-DEGRADE-HEADER,
  CC-NO-N-PLUS-1, CC-ANTI-MISREAD (7/15 ≈ 47%) — comfortably above the 40% target. PASS.

## Layer + PBT posture (Mandate 9/11)

Every scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The sad paths
(honest "(0 countered)", failed read → "(— countered)") are enumerated explicitly, never
PBT-generated at this layer (Mandate 11). The `@property`-tagged CC-NO-N-PLUS-1 is an
EXAMPLE-shaped invariant assertion (the read-count is invariant to store size), consistent with
the slice-12/17 single-aggregate gold posture — no PBT machinery is introduced. **Tier B is NOT
warranted** (Mandate 10 skip): the count is a single-shot additive render with no chained
≥3-scenario journey and no domain-rich input space (one `Option<usize>`) — Tier A example
coverage is exact.

## Seeds + asserts added (`tests/acceptance/support/mod.rs`)

**Seeds** (drive the production `claim add` + `peer add` + `peer pull` paths; NO hand-inserted
rows; each PINS the genuine count with the direct ADR-055 `read_countered_own_claims_count`
`COUNT(DISTINCT)` oracle):

- `seed_landing_store_with_countered_own_claims` — 12 own claims, 3 countered by peers (one by
  TWO distinct peers, proving presence-once → count is 3, not 4). The headline fixture.
- `seed_landing_store_none_countered` — 12 own claims, 0 countered → "(0 countered)" honest zero.
- `seed_landing_store_one_own_claim_countered_twice` — 1 own claim countered by 2 distinct peers,
  no other countered → "(1 countered)" presence-once boundary (confidence 0.30 for anti-misread).
- `start_viewer_with_failing_countered_count` — the test-only `OPENLORE_VIEWER_FAIL_COUNTERED_
  COUNT` effect-shell fault seam (mirrors slice-17's `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`,
  threaded into `start_inner` as a 7th param; DELIVER materializes the `#[cfg(debug_assertions)]`-
  gated branch substituting `Err(StoreReadError)` for the real `count_countered_own_claims()`).
- `read_countered_own_claims_count` — the direct ADR-055 `COUNT(DISTINCT)` test-side oracle.

**Asserts** (scan ONLY the rendered HTML — Mandate 8 universe = port-exposed surface):

- `assert_landing_countered_count(body, n)` — landing shows "(n countered)".
- `assert_landing_countered_missing(body)` — landing shows "(— countered)" + no fabricated 0.
- `assert_claims_header_countered_count(body, n)` — /claims header shows "(n countered)".
- `assert_claims_header_countered_missing(body)` — /claims header shows "(— countered)".
- `assert_landing_and_claims_countered_consistent(landing, claims)` — landing == header.
- `assert_countered_copy_is_neutral(body)` — no penalty/verdict/"disputed by N" copy.

**Reused slice-17/12 harness**: `seed_own_claims_via_cli`, `seed_own_claim_with_evidence`,
`build_verifiable_peer_counter_record`, `run_openlore_with_peer_resolver`,
`run_openlore_pull_multi`, `assert_user_author_claim_count`, `assert_landing_shows_count`,
`assert_landing_links_all_surfaces`, `assert_landing_read_only_no_control`,
`capture_store_row_count_universe`, `assert_store_read_only`,
`read_slice06_list_baseline` + `assert_list_order_and_confidence_byte_identical`,
`seed_claims_list_mixed_pages`, `references_external_cdn`.

## RED confirmation

Both test binaries COMPILE. The walking-skeleton + 4 sampled scenarios were RUN and FAIL for
the right reason (MISSING_FUNCTIONALITY at the countered-count assertion: "(3 countered)" /
"(0 countered)" / "(1 countered)" / "(— countered)" are ABSENT because the production routes
do not render the countered count yet and `count_countered_own_claims` / `render_countered` /
the 4th `LandingSummary` field do not exist). The seeds' direct ADR-055 oracle confirmed the
genuine seeded counts (3 / 0 / 1) — including the presence-once COUNT(DISTINCT) collapse —
proving the fixtures are the REAL countered shape, not "the verbs exited 0". NOT a
setup/import/fixture error. `check-arch: OK (21 workspace members)`. The slice-17 + slice-12
suites still compile (the `start_inner` 7th-param ripple is clean).

### Missing≠zero seeding handling (documented DISTILL choice)

The viewer holds ONE long-lived DuckDB connection taken at startup (ADR-028/030), so there is
no readily-available mid-request per-count read-failure seam in the slice-06/15 harness. Per
the slice-16 SF-8 / slice-17 LD-DEGRADE precedent, the OBSERVABLE missing-marker contract (a
failed countered read → "(— countered)" while the own-claims count + rows render, page 200) is
scaffolded against a TEST-ONLY effect-shell fault seam: the `OPENLORE_VIEWER_FAIL_COUNTERED_
COUNT` env var, threaded by `start_inner` (7th param). The Some(0) success side is fully
exercisable TODAY via `seed_landing_store_none_countered`. DELIVER materializes the
`#[cfg(debug_assertions)]`-gated, release-forbidden, xtask-guarded branch substituting
`Err(StoreReadError)` for the real read on BOTH the `/` and `/claims` handlers — exactly as
slice-17 materialized the peer-claims-count seam. Today the failed-read scenarios reach the
HTTP assertion and fail because the missing-marker is absent (the field doesn't exist) — RED
MISSING_FUNCTIONALITY.
