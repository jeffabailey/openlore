<!-- markdownlint-disable MD013 MD024 -->
# Test Scenarios: viewer-peer-counter-aware-counts (slice-19) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-10
> The deferred PEER sibling of slice-18, mirroring its DISTILL structure EXACTLY onto the PEER
> count. Source: `discuss/acceptance-criteria.md` (9 themes + GOLD invariants) + ADR-056.

## Wave-Decision Reconciliation HARD GATE

**Reconciliation passed — 0 contradictions.** Read: `discuss/wave-decisions.md` (incl. the embedded
DESIGN resolution + ADR-056), `feature-delta.md` (DISCUSS + DESIGN sections). No separate
`design/wave-decisions.md` or `devops/` dir (consistent with all prior OpenLore slices — DEVOPS
absent → default env matrix, WARN not block). Every DISCUSS cardinal (WD-PC-1..10) is HONORED by
DESIGN D2/D3/D4: WD-PC-5 (read shape) RESOLVED → count-only aggregate; WD-PC-10 (no new helper)
HONORED → REUSE `render_countered`; WD-PC-7 (own untouched) HONORED; D4 (4th DISTINCT fault-seam
token) confirms `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`. No contradiction surfaced.

## Story → scenario traceability (Dimension 8 Check A)

| Story | Scenarios |
|---|---|
| US-PC-000 (infra read-wiring) | presence-once, either-arm, missing≠zero (×2), no-N+1, + every gold (the count read backs all) |
| US-PC-001 (landing peer line) | PC-WS (walking skeleton), presence-once, either-arm, honest-zero (landing), missing≠zero (landing), no-re-weight, read-only (landing), offline (landing), no-N+1, anti-misread |
| US-PC-002 (`/peer-claims` header) | header-consistency, honest-zero (header), missing≠zero (header), no-re-order, read-only (header), offline (header) |

Every story has ≥1 scenario. No untraceable scenario.

## Scenario inventory — story suite (`viewer_peer_counter_aware_counts.rs`, 16 scenarios)

| Theme / AC | Scenario | Tags |
|---|---|---|
| 1 / US-PC-001 (WS) | the_front_door_shows_how_many_cached_peer_claims_are_countered | @us-pc-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy |
| 1 Ex 2 / US-PC-002 | the_peer_claims_header_shows_the_same_countered_count_as_the_landing | @us-pc-002 @driving_port @real-io @single-source @wd-pc-8 @happy |
| 2 / C-4 BR-PC-1 | a_peer_claim_countered_by_two_counterers_counts_once | @us-pc-000 @us-pc-001 @driving_port @real-io @presence-once @c-4 @br-pc-1 @cardinal @boundary |
| 2 Ex 2 / C-4 | a_peer_claim_countered_from_either_ref_table_contributes_once | @us-pc-000 @us-pc-001 @driving_port @real-io @presence-once @c-4 @br-pc-1 @boundary |
| 3 / C-5 | an_honest_zero_countered_on_the_landing_when_no_peer_claim_is_disputed | @us-pc-001 @driving_port @real-io @honest-zero @c-5 @edge |
| 3 / C-5 | an_honest_zero_countered_in_the_peer_claims_header_list_as_slice_06_07 | @us-pc-002 @driving_port @real-io @honest-zero @no-noise @c-5 @edge |
| 4 / C-2 C-5 | a_failed_countered_peer_count_read_degrades_gracefully_on_the_front_door | @us-pc-000 @us-pc-001 @driving_port @real-io @infrastructure-failure @missing-not-zero @c-2 @c-5 @cardinal @error |
| 4 / C-2 C-5 | a_failed_header_count_degrades_without_blanking_the_peer_claims_list | @us-pc-000 @us-pc-002 @driving_port @real-io @infrastructure-failure @missing-not-zero @c-2 @c-5 @cardinal @error |
| 5 / C-4 WD-PC-7 | the_countered_count_never_re_weights_the_peer_claims_count_and_own_line_untouched | @us-pc-001 @driving_port @real-io @additive @c-4 @anti-misread @no-regression @happy |
| 5 / C-4 WD-PC-9 | the_peer_claims_header_count_does_not_re_order_filter_or_re_weight_the_list | @us-pc-002 @driving_port @real-io @additive @no-regression @c-4 @wd-pc-9 @happy |
| 6 / C-1 | the_counter_aware_front_door_exposes_no_write_control | @us-pc-001 @driving_port @real-io @read-only @c-1 @cardinal @happy |
| 6 / C-1 | the_counter_aware_peer_claims_header_adds_no_write_control | @us-pc-002 @driving_port @real-io @read-only @c-1 @cardinal @happy |
| 7 / C-2 | the_front_door_peer_countered_count_renders_fully_with_the_network_down | @us-pc-001 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy |
| 7 / C-2 | the_peer_claims_header_countered_count_renders_offline | @us-pc-002 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy |
| 8 / C-3 | the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_store_size | @us-pc-000 @property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @boundary |
| 9 / C-6 WD-PC-10 | the_countered_peer_count_is_neutral_awareness_never_a_verdict_or_penalty | @us-pc-001 @driving_port @real-io @anti-misread @c-6 @wd-pc-10 @presence-once @boundary |

## GOLD invariants (`viewer_peer_counter_aware_counts_invariants.rs`, 10 golds)

| Gold | Guards | RED pre-DELIVER? |
|---|---|---|
| every_peer_counter_aware_render_leaves_the_store_read_only | C-1 / Mandate 8 (state-delta universe = `claims`+`peer_claims` row counts, all `unchanged`) | GREEN guardrail (read-only holds by construction) |
| no_peer_counter_aware_render_adds_a_write_or_mutating_control | C-1 CARDINAL | GREEN guardrail |
| the_peer_counter_aware_chrome_stays_offline_no_cdn | C-2 / KPI-HX-G2 | GREEN guardrail |
| the_peer_counter_aware_surfaces_render_fully_offline | C-2 / KPI-5 | **RED** (asserts the peer count renders offline) |
| the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_store_size | C-3 CARDINAL | **RED** (asserts the count) |
| missing_is_distinct_from_zero_for_the_countered_peer_count | C-2 / C-5 / WD-PC-6 CARDINAL | **RED** (both sides: honest 0 + failed-read marker) |
| a_peer_claim_countered_by_two_counterers_counts_once | C-4 / BR-PC-1 CARDINAL | **RED** (presence-once) |
| the_landing_and_peer_claims_header_counts_are_consistent | WD-PC-8 single source | **RED** (asserts both counts) |
| the_peer_claims_list_is_byte_identical_to_the_no_header_count_baseline | C-4 / WD-PC-9 | GREEN guardrail (no-regression holds by construction) |
| the_slice_18_own_countered_surfaces_are_untouched | BR-PC-4 / WD-PC-7 | GREEN guardrail (slice-18 own surfaces shipped + untouched) |

The 5 GREEN golds are PROTECTIVE guardrails that hold by construction pre-implementation
(read-only, no-write, offline-chrome, no-regression byte-identity, own-untouched) and STAY green
after DELIVER — exactly the slice-18 invariants posture (read-only/offline/no-regression also held
before the count render). The 5 RED golds each assert the NEW peer count render →
MISSING_FUNCTIONALITY.

## Error/edge ratio (Dimension 1)

Of the 16 story scenarios: missing≠zero degrade (×2 @error), honest-zero (×2 @edge),
presence-once boundary (×2 @boundary), either-arm (×1 @boundary), no-N+1 (×1 @boundary),
anti-misread (×1 @boundary), no-regression/no-re-order (×2 @no-regression) = **11/16 ≈ 69%**
error/edge/boundary — comfortably above the 40% target.

## RED classification (pre-DELIVER fail-for-the-right-reason gate)

All 16 story scenarios + 5 of the 10 golds classify **RED = MISSING_FUNCTIONALITY**, NOT BROKEN:
the seeds run to completion (every `peer add` / `peer pull` / `claim counter` exits 0; the direct
ADR-056 `COUNT(DISTINCT)` oracle pins the genuine count), and each body runs to a `GET /` or
`GET /peer-claims` HTTP assertion that fails because the production routes do NOT render the peer
countered count yet (`count_countered_peer_claims` / the 5th `LandingSummary` field / the
`render_peer_claims_page` `Option<usize>` param do NOT exist). Verified by running the suites:
every failure message is "the … must show the countered-peer count (N countered)" / the missing
marker — never an import/setup/fixture error. The missing≠zero failed-read scenarios drive the 4th
DISTINCT `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` effect-shell fault seam, panicking at the
`start_inner` `todo!()`-equivalent until DELIVER materializes it (also MISSING_FUNCTIONALITY).

The 5 GREEN golds are documented above as protective guardrails (hold by construction), NOT
vacuous passes — the landing-count asserts are POSITION-AWARE (`assert_countered_count_beside_label`
scans the count at the "peer claims" vs "own claims" label position) so the slice-18 own line's
`"(N countered)"` parenthetical can NEVER satisfy a peer assert vacuously (No Fixture Theater).
