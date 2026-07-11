# DISTILL RED classification — retraction-aware-search-filter

> Pre-DELIVER fail-for-the-right-reason gate (nw-distill). One line per scenario.
> DELIVER reads this at the RED phase to confirm RED is genuine (ADR-025 D2).
> Wave: DISTILL · Owner: Sentinel (nw-acceptance-designer) · Date: 2026-07-11 · ADR-060.

## How to read

- **RED** = the scenario fails because the FEATURE is unimplemented
  (`MISSING_FUNCTIONALITY`) — the correct RED state. It reaches its assertions and
  they fire on missing behavior; no compile/import/fixture error.
- **GREEN (gold guard)** = a DEFAULT-UNCHANGED regression guard that is green
  BY DESIGN at DISTILL (it characterizes today's preserved behavior, the mechanical
  proof I-AV-9 was not weakened) and MUST STAY GREEN through DELIVER. Not RED, not
  BROKEN — an intended-green guard (I-RF-1 / D-RF-D6).
- **BROKEN** = fails before reaching a meaningful assertion (compile / import /
  fixture / wrong-observable). BLOCKS handoff. **Zero BROKEN after fix pass.**

Gate command (run 2026-07-11, both binaries pre-built):
`cargo build --bin openlore --bin openlore-indexer`
`cargo test -p cli --test search_hide_retracted --test viewer_search_hide_retracted`

## Scaffold

`crates/appview-domain/src/retraction.rs` — `partition_retracted(rows, hide_retracted)
-> RetractionPartition{survivors, hidden_count}` with `// SCAFFOLD: true` + a
`panic!("… RED scaffold …")` body (exported from `crates/appview-domain/src/lib.rs`).
Compiles clean (`cargo build -p appview-domain` OK); not yet wired into either
surface, so the subprocess/HTTP scenarios fail on the ABSENT flag/param/notice,
not on the scaffold panic — genuine RED.

## Slice-01 — CLI `openlore search … --hide-retracted` (`tests/acceptance/search_hide_retracted.rs`)

| # | Scenario (test fn) | Verdict | Why |
|---|---|---|---|
| RF-1 | `hide_retracted_removes_self_retracted_claim_and_discloses_the_count` (@walking_skeleton) | RED | `--hide-retracted` flag unknown → clap exits 2 → fails the exit-0 assertion (MISSING_FUNCTIONALITY) |
| RF-2 | `default_search_without_the_flag_still_shows_the_retracted_claim_and_no_footer` | GREEN (gold guard) | runs WITHOUT the flag; characterizes today's preserved default (retracted claim shown, no footer). MUST stay green (I-RF-1) |
| RF-3 | `a_third_party_countered_claim_is_not_hidden_by_the_filter` | RED | uses `--hide-retracted` → clap exits 2 → fails exit-0 (MISSING_FUNCTIONALITY) |
| RF-4 | `self_retraction_dominates_a_co_present_third_party_counter` | RED | uses `--hide-retracted` → clap exits 2 (MISSING_FUNCTIONALITY) |
| RF-5 | `hiding_never_reorders_or_reweights_the_survivors` (@property) | RED | the `--hide-retracted` run → clap exits 2 (MISSING_FUNCTIONALITY) |
| RF-6 | `hiding_every_result_shows_a_guided_state_not_a_bare_empty_result` | RED | uses `--hide-retracted` → clap exits 2 (MISSING_FUNCTIONALITY) |
| RF-7 | `hidden_count_reports_retraction_events_not_raw_rows` | RED | uses `--hide-retracted` → clap exits 2 (MISSING_FUNCTIONALITY) |
| RF-8 | `hide_retracted_over_a_set_with_no_retractions_prints_no_misleading_line` | RED | uses `--hide-retracted` → clap exits 2 (MISSING_FUNCTIONALITY) |

Observed: `test result: FAILED. 3 passed; 7 failed` (the 3 "passed" = RF-2 + 2
`support::state_delta` harness unit tests co-located in the shared support module;
7 failed = RF-1/3/4/5/6/7/8, each on `error: unexpected argument '--hide-retracted'`
→ exit 2 ≠ 0). Zero BROKEN.

## Slice-02 — Viewer `GET /search?hide_retracted=1` (`tests/acceptance/viewer_search_hide_retracted.rs`)

| # | Scenario (test fn) | Verdict | Why |
|---|---|---|---|
| RF-V1 | `hide_retracted_full_page_removes_self_retracted_claim_and_shows_the_notice` (@walking_skeleton) | RED | `?hide_retracted=1` ignored today → 200 with the self-retracted author still present + no notice → fails the Priya-absent assertion (MISSING_FUNCTIONALITY) |
| RF-V2 | `default_search_without_the_param_renders_identically_with_no_notice` | GREEN (gold guard) | no param; characterizes today's slice-08 render (retracted claim shown, no notice). MUST stay green (I-RF-1) |
| RF-V3 | `hide_retracted_with_htmx_returns_only_the_filtered_results_fragment_with_the_notice` | RED | fragment returned but the hidden-count notice is absent (MISSING_FUNCTIONALITY) |
| RF-V4 | `hiding_every_result_shows_a_guided_region_not_a_blank_region` | RED | no guided empty-after-filter copy rendered (MISSING_FUNCTIONALITY) |
| RF-V5 | `a_third_party_countered_claim_stays_shown_while_a_self_retraction_is_hidden` | RED | the self-retraction is not hidden + no notice (MISSING_FUNCTIONALITY) |
| RF-V6 | `the_hide_control_is_a_read_only_get_param_toggle_with_no_write_surface` (@invariant) | RED | the "Hide retracted claims" control is absent from `/search` (MISSING_FUNCTIONALITY); the no-write-surface half already holds |

Observed (after the wrong-observable fix, below): `test result: FAILED. 3 passed;
5 failed` (3 "passed" = RF-V2 + 2 `support::state_delta` harness unit tests; 5
failed = RF-V1/V3/V4/V5/V6). Zero BROKEN.

## Wrong-observable fix pass (BROKEN → RED, applied before sign-off)

The FIRST viewer run classified RF-V2 as failing and several viewer scenarios
asserted a per-row **cid** that the viewer HTML never renders (it renders
`[verified]` + author DID + subject/predicate/object + confidence per row, plus any
`countered by (cid)` annotation — but NOT the row's OWN cid). That is a
`WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT` smell (would have mis-fired in DELIVER).
Fixed by keying viewer presence/absence on the port-exposed observable (author DID
+ the self-retracted claim's confidence `0.82`) instead of a cid. After the fix
RF-V2 is GREEN (as intended) and RF-V1/V3/V4/V5/V6 are genuine RED. The CLI
surface renders each row's own `cid:` line (`crates/cli/src/render/search.rs:195`),
so the slice-01 cid-based assertions are valid observables and were left as-is.

## Verdict

- **12 RED** (genuine `MISSING_FUNCTIONALITY`): RF-1, RF-3..8, RF-V1, RF-V3..V6.
- **2 GREEN (gold guard, intended)**: RF-2, RF-V2 (default-unchanged, I-RF-1;
  must stay green through DELIVER as the mechanical proof I-AV-9 is not weakened).
- **0 BROKEN.** The pre-DELIVER gate PASSES — no scenario is blocked for a
  compile/import/fixture/wrong-observable reason.
