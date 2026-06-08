<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-flags-score-surface — DISTILL wave (slice-14)

> Wave: **DISTILL** (lean mode) · Acceptance Designer: Quinn (nw-acceptance-designer)
> Date: 2026-06-08 · Brownfield DELTA on the read-only `openlore ui` viewer
> Scope (REUSE-ONLY, confirmed): the SCORING-BEARING `GET /score?contributor=<did>`
> per-contribution breakdown rows. **LAST viewer surface — completes J-003b.**
> ADR-025: ALL acceptance tests authored as scaffolded RED (`todo!()` → panic → RED).
> The executable `.rs` files are the scenario SSOT; this file is the structured summary.

This is the DISTILL-wave delta translating the DISCUSS user stories (US-CF-001 infra
wiring + US-CF-002 user-visible flag) and the DESIGN driving port (ADR-051: thread
`&presence` into the render chain; `score_counter_presence` flatten-once helper;
`SCORE_COUNTER_LEGEND` anti-misread constant) into executable acceptance tests. Mirrors
the slice-12/13 corpus VERBATIM where shapes match (the `COUNTERED_PRESENCE_FLAG` neutral
marker, the `<a href="/claims/{cid}">` one-hop link, the baseline+marker-elision
byte-identity tactic, the production federation seeds) and EXTENDS the slice-09 score
corpus (the reproduce-by-hand `parse_score_pairings` parser, the `WeightedView` trail
seeds, the `#score-results` fragment) for the slice-14 CARDINAL (sum-to-weight +
byte-identity orthogonality + anti-misread copy).

---

## Wave: DISTILL / [REF] Wave-decision reconciliation HARD GATE

**Reconciliation passed — 0 contradictions.**

Read + cross-checked: DISCUSS `discuss/acceptance-criteria.md` + `discuss/user-stories.md`
+ `discuss/wave-decisions.md` (D-14-1..5, R-14-1..3); DESIGN `design/architecture-design.md`
+ `design/component-boundaries.md` + `design/wave-decisions.md` (DD-14-1..5); ADR-051
(thread `&presence`; `score_counter_presence`; `SCORE_COUNTER_LEGEND`) building on ADR-048
(the REUSED slice-12 batch read) + ADR-050 (the slice-13 flatten tactic). Every DISCUSS
decision (REUSE-only / no new crate-route-read-method-render-fn; sum-to-weight CARDINAL
shown-never-applied; anti-misread copy AC+KPI; presence-only; read-only; local-offline;
no new KPI ID) is honored verbatim by DESIGN DD-14-1..5 and ADR-051. No DESIGN decision
contradicts any DISCUSS decision. DEVOPS directory absent → WARN, default env matrix
(offline by construction; `/score` is a LOCAL read + PURE compute, no network seam, no
new infra).

---

## Wave: DISTILL / [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#US-CF-002 | At-a-glance "Countered" flag on each `/score` per-contribution breakdown row whose contribution has ≥1 counter, linking to the slice-11 thread; weight/confidence/bonuses/subtotal/ranking/order unchanged | n/a | SF-1..SF-7 + SF-N1 drive `GET /score?contributor=<did>` port-to-port through the real `ViewerServer` subprocess; SF-1 walking skeleton |
| DISCUSS#US-CF-001 | Reuse the slice-12 `counter_presence_for(&[cid])` batch read in the `/score` handler — flatten EVERY `Contribution.cid` across EVERY `WeightedPairing` into ONE aggregate query per render, no N+1, NO new read method | ADR-051 | Observable THROUGH US-CF-002: SF-N1 + SF-INV-N1 (a large multi-pairing/multi-contribution breakdown flags the countered subset correctly in ONE request) |
| DISCUSS#D-14-2 / I-CF-9 (CARDINAL) | Sum-to-weight preserved + byte-identity vs slice-09 (markers + legend elided) — the flag is SHOWN, never APPLIED; a countered contribution keeps its FULL original subtotal | ADR-051 §7 | SF-3 (sum-to-weight on a FLAGGED breakdown) + SF-4 + SF-INV-ByteId (byte-identity gold), both via the slice-09 reproduce-by-hand parser + the slice-12/13 marker-elision tactic extended to elide the legend |
| DISCUSS#D-14-3 / AC-SCORE-ANTIMISREAD | The breakdown carries a SHORT NEUTRAL legend stating the marker is shown for the reader to judge + does NOT lower the score; never "disputed"/"refuted"/"false"/"penalty"/"deduction"/"lowered"/"disputed score" | DD-14-3 | SF-5 (identical-subtotal anti-misread proof) + the `assert_score_legend_present_and_blocklist_clean` helper pinning the `SCORE_COUNTER_LEGEND` SSOT constant byte-for-byte + the blocklist |
| DESIGN#ADR-051 / DD-14-1 | Thread `presence: &HashSet<String>` down `render_score_results_fragment → render_score_result → render_score_pairing → render_score_breakdown`; `render_score_breakdown` emits the REUSED `render_countered_link` BESIDE the verbatim subtotal; NO field added to `scoring::Contribution` | ADR-051 | The render is a TOTAL fn of `(ScoreState, presence)`; the seams the scaffolds target as the missing production API |

---

## Wave: DISTILL / [REF] Scenario list with tags

### Story scenarios — `viewer_counter_flags_score_surface.rs` (8 slice-14 tests)

| # | Scenario (fn) | Route / US | Walking skeleton? | Key tags |
|---|---|---|---|---|
| SF-1 | `open_the_score_breakdown_with_htmx_flags_only_the_countered_contribution` | `GET /score` · US-CF-002 | **YES** | `@walking_skeleton @driving_port @real-io @htmx-fragment @flag @reuse-render @presence-only @cardinal-sum-to-weight @anti-misread @happy` |
| SF-2 | `a_contribution_with_two_counters_shows_one_neutral_presence_marker_on_the_score` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @presence-only @anti-merging @gold` |
| SF-3 | `the_per_contribution_subtotals_still_sum_to_the_pairing_weight_with_the_flag` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @cardinal-sum-to-weight @shown-never-applied @gold` |
| SF-4 | `adding_the_score_flag_changes_no_weight_ranking_or_row_order_versus_slice09` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @no-regression @cardinal @shown-never-applied @gold` |
| SF-5 | `two_identical_subtotal_contributions_render_identically_only_one_flagged` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @anti-misread @shown-never-applied @gold` |
| SF-6 | `a_contributor_with_no_countered_contributions_renders_score_with_no_markers` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @no-noise @empty-set @no-regression @happy` |
| SF-7 | `the_score_flag_renders_identically_under_htmx_and_no_js` | `GET /score` · US-CF-002 | — | `@driving_port @real-io @no-js @full-page @parity @local-offline @happy` |
| SF-N1 | `a_large_multi_pairing_breakdown_flags_every_countered_contribution_in_one_request` | `GET /score` · US-CF-001 | — | `@us-cf-001 @driving_port @real-io @batch-read @no-n-plus-1 @gold` |

### GOLD invariants — `viewer_counter_flags_score_surface_invariants.rs` (6 slice-14 tests)

| # | Invariant (fn) | Surface | Key tags |
|---|---|---|---|
| SF-INV-ReadOnly | `every_flagged_score_render_leaves_the_store_read_only` | `/score` × shapes × postures | `@property @driving_port @real-io @read-only @i-cf-1 @gold` |
| SF-INV-NoWrite | `no_flagged_score_render_adds_a_write_or_sign_control` | `/score` × shapes | `@property @driving_port @real-io @read-only @i-cf-1 @gold` |
| SF-INV-OfflineChrome | `the_flagged_score_chrome_stays_offline_no_cdn` | `/score` | `@property @driving_port @real-io @offline @no-cdn @i-cf-5 @gold` |
| SF-INV-Offline | `the_flagged_score_surface_renders_fully_offline` | `/score` | `@property @driving_port @real-io @offline @local-first @i-cf-5 @kpi-5 @gold` |
| SF-INV-ByteId (CARDINAL) | `the_score_render_is_byte_identical_with_and_without_the_flag` | `/score` (mixed subset) | `@property @driving_port @real-io @shown-never-applied @no-regression @cardinal-sum-to-weight @cardinal @i-cf-9 @gold` |
| SF-INV-N1 | `a_large_multi_pairing_breakdown_resolves_presence_in_one_request` | `/score` (many pairings) | `@property @driving_port @real-io @n-plus-1-guard @i-cf-8 @gold` |

**Total: 14 slice-14 acceptance tests** (8 story + 6 GOLD invariants). All RED (`todo!()`
→ panic → MISSING_FUNCTIONALITY, never BROKEN) until DELIVER.

Error/edge/guardrail share: no-noise (SF-6), presence-only/anti-merging (SF-2), anti-misread
(SF-5), sum-to-weight CARDINAL (SF-3, SF-INV-ByteId), byte-identity (SF-4, SF-INV-ByteId),
N+1 (SF-N1, SF-INV-N1), read-only / no-write / offline (SF-INV-ReadOnly/NoWrite/OfflineChrome/
Offline) = **11/14 (79%)** are guardrail/edge/no-regression, well above the 40% floor.

---

## Wave: DISTILL / [REF] Walking-skeleton strategy

**Brownfield DELTA — NO walking-skeleton Feature 0** (the `openlore ui` viewer, the
`/score` route + the `resolve_score_state`/`render_score_*` chain, the read-only store
port, the slice-12 `counter_presence_for` batch read, the slice-13 `render_countered_link`
SSOT + the `COUNTERED_PRESENCE_FLAG` marker + the `<a href="/claims/{cid}">` one-hop link,
the slice-09 reproduce-by-hand parser + the `WeightedView` trail seeds, and the
`page = chrome + fragment` render pattern all already exist, slices 03/06/07/09/10/11/12/13).

**One walking-skeleton scenario**: SF-1 (`open_the_score_breakdown_with_htmx_flags_only_
the_countered_contribution`) — the thinnest end-to-end thread (the closest mirror of the
slice-09 score WS + the slice-13 flag WS), `@walking_skeleton @driving_port @real-io`. It
closes the loop viewer → LOCAL scoring-feed read → PURE `scoring::score` → LOCAL batch
presence read (REUSED slice-12) → pure projection threading `&presence` → HTML fragment
through the production composition root (the real `ViewerServer` subprocess), proving the
scoring surface carries an at-a-glance disagreement flag PROVABLY ORTHOGONAL to the score
(the marker present + the legend present + the subtotals still summing to the weight, all
in one scenario). Architecture of Reference: **Driving** port = `GET /score` via the real
`ViewerServer` (hyper, 127.0.0.1, read-only store); **Driven internal** = the DuckDB store
via the real `DuckDbStoreReadAdapter` (real, seeded through production `peer add`+`peer
pull`); **Driven external / non-deterministic** = NONE (LOCAL read + PURE compute, offline
by construction — no clock/network/LLM to fake).

---

## Wave: DISTILL / [REF] Driving-port + adapter coverage

One driving port, exercised port-to-port through the REAL `openlore ui` `ViewerServer`
subprocess (no direct call to `counter_presence_for`, `scoring::score`, or `viewer-domain`
`render_score_*` in any AC):

| Driving port | Scenarios | Both shapes (get / get_htmx)? |
|---|---|---|
| `GET /score?contributor=<did>` | SF-1..SF-7, SF-N1 + SF-INV-* | YES (SF-1 fragment, SF-7 both, SF-INV-ReadOnly/NoWrite both) |

**Driven-adapter coverage (Mandate 6)**: the ONLY driven dependency exercised is
`counter_presence_for` on `DuckDbStoreReadAdapter` — **REUSED VERBATIM from slice-12**
(ADR-048 / ADR-051), whose real-I/O integration + N+1 property test already exist (slice-12
/ ADR-048). slice-14 adds NO new driven adapter, NO new port, NO new SQL, NO new route
(component-boundaries.md — Earned-Trust: no new probe owed; xtask check-arch delta NONE).
The SF-N1 + SF-INV-N1 scenarios are the subprocess-layer behavioral proxy for the
single-flattened-call guarantee (the strict 1-query bound stays the DELIVER `adapter-duckdb`
unit/property test). No `@real-io @adapter-integration` row is owed BY this slice — the
REUSED adapter's real-I/O test is inherited.

---

## Wave: DISTILL / [REF] Scaffold files (RED-ready, Mandate 7 / ADR-025)

| File | Role |
|---|---|
| `tests/acceptance/viewer_counter_flags_score_surface.rs` | 8 story scenarios (US-CF-001/002); SF-1 = walking skeleton. `// SCAFFOLD: true`; bodies reach `todo!()`-stubbed seeds/asserts → panic → RED. |
| `tests/acceptance/viewer_counter_flags_score_surface_invariants.rs` | 6 GOLD/guardrail invariants (I-CF-1/5/8/9 + the slice-14 CARDINAL sum-to-weight/byte-identity). `// SCAFFOLD: true`; bodies reach `todo!()` → panic → RED. |
| `tests/acceptance/support/mod.rs` (slice-14 block appended) | New seams, all `todo!()`-stubbed (compile, then panic). |
| `crates/cli/Cargo.toml` | Two new `[[test]]` targets registered (`viewer_counter_flags_score_surface` + `_invariants`). |

### New support seams (all `todo!()`-stubbed — compile, then panic → RED)

**Types**: `SeededScoreBreakdown` (contributor DID + ordered/countered/uncountered
contribution CIDs).

**Constants** (mirroring the production SSOT): `SCORE_COUNTER_LEGEND_TEXT` (the DD-14-3
legend copy), `SCORE_LEGEND_BLOCKLIST` (the AC-SCORE-ANTIMISREAD verdict/penalty words).

**Read helper**: `read_score_contribution_cids` (every contribution CID in the
contributor's scored breakdown, slice-09 ranked order).

**Seeds**: `seed_score_breakdown_one_contribution_countered`,
`seed_score_breakdown_target_two_counters_distinct_authors`,
`seed_score_breakdown_identical_subtotals_one_countered`,
`seed_score_breakdown_none_countered`,
`seed_score_breakdown_many_pairings_known_countered_subset`.

**Asserts**: `assert_score_row_flagged_countered`, `assert_score_row_not_flagged`,
`assert_score_flag_is_single_neutral_presence`, `assert_score_flag_links_to_thread`,
`assert_score_html_breakdown_sums_to_weight_with_flag` (the FLAGGED reproduce-by-hand
adapter of the slice-09 `assert_score_html_breakdown_sums_to_displayed_weight`),
`assert_score_legend_present_and_blocklist_clean`, `assert_score_legend_absent`,
`assert_score_render_byte_identical_to_slice09` (the byte-identity gold — slice-09 baseline
with markers + legend elided).

**REUSED slice-09/11/12/13 seams** (no new copy): `SCORE_RESULTS_ID`,
`parse_score_pairings` (the reproduce-by-hand parser),
`assert_score_html_breakdown_sums_to_displayed_weight` (the slice-09 sum-to-weight base),
`assert_score_html_has_no_write_or_sign_control`, `LIST_COUNTERED_FLAG_TEXT`,
`capture_store_row_count_universe` + `assert_store_read_only` (Mandate 8 universe-bound),
`build_verifiable_peer_records_for_triples` / `build_verifiable_peer_counter_record` /
`run_openlore_pull_multi` / `PeerSeam` / `PeerPds` / `read_peer_claim_cids_for` /
`COUNTER_AUTHOR_TOBIAS` / `COUNTER_TARGET_AUTHOR_RACHEL` / `COUNTER_PEER_REASON_VERBATIM`,
`ViewerServer::{start,get,get_htmx}` + `ViewerResponse::{is_fragment,is_full_page,
references_external_cdn}`.

---

## Wave: DISTILL / [REF] Test placement + layer / PBT-mode discipline

- **Placement**: `tests/acceptance/` (the established brownfield viewer acceptance corpus —
  precedent: slices 06–13 all live there; `crates/cli/Cargo.toml` registers each as a
  `[[test]]` target).
- **Layer (nw-tdd-methodology + Mandate 9/11)**: every slice-14 scenario is a
  layer-3/layer-5 subprocess + real-I/O test — **EXAMPLE-only**. Sad/edge paths
  (none-countered, multi-counter, identical-subtotal anti-misread) are enumerated
  explicitly, **never PBT-generated** at this layer (Mandate 11). No `@given`/PBT machinery
  is imported at this layer (Mandate 9).
- **Mandate 8 (Universe)**: the read-only GOLD (`SF-INV-ReadOnly`) uses the universe-bound
  `assert_store_read_only` (universe = the two port-exposed counts `claims.row_count` +
  `peer_claims.row_count`, each `unchanged`). Other layer-3+ assertions use traditional
  rendered-HTML scans (port-exposed surface), permitted at layer 4+ per Mandate 8.
- **Mandate 10 (Tier B)**: **Tier A only**. The journey is 1–2 chained scenarios over a
  config-shaped render (a boolean presence flag gating an additive marker + a one-shot
  legend); no ≥3-chained domain-rich journey → Tier B state-machine PBT is NOT warranted
  (the generative exploration of the pure projection/render + the sum-to-weight arithmetic
  is the DELIVER `viewer-domain`/`scoring` unit/property concern).

---

## Wave: DISTILL / [REF] Pre-requisites

- **DESIGN driving port**: `GET /score?contributor=<did>` (slice-09) — EXTENDED, no new
  route.
- **REUSED read**: `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>`
  (slice-12 / ADR-048) — confirmed present, REUSED verbatim (ADR-051). NO new read method.
- **REUSED render**: `render_countered_link(cid, is_countered)` + `COUNTERED_PRESENCE_FLAG`
  (slice-13 SSOT). NO new render fn, NO new string. The ONE genuinely-new render artifact is
  `SCORE_COUNTER_LEGEND` (a render-only constant in `viewer-domain`, DD-14-3).
- **NEW seams the scaffolds target (DELIVER materializes)**: `score_counter_presence`
  flatten-once helper (`adapter-http-viewer`); `render_score_*` chain widened to take
  `&presence`; `render_score_breakdown` emits the REUSED `render_countered_link`;
  `render_score_result` emits `SCORE_COUNTER_LEGEND` once per scored breakdown.
- **Environment**: LOCAL DuckDB seeded via production federation paths; NO network seam on
  `/score` (offline by construction; LOCAL read + PURE compute). Build-before-run:
  `cargo build -p cli --bin openlore` before running the ATs so `ViewerServer::start` spawns
  the current viewer.

---

## Wave: DISTILL / [REF] RED classification (pre-DELIVER fail-for-the-right-reason gate)

`cargo build -p cli --test viewer_counter_flags_score_surface --test
viewer_counter_flags_score_surface_invariants` → **compiles** (only pre-existing
`support/mod.rs` warnings; zero errors). Running SF-1 panics at the slice-14 `todo!()` seed
(`not yet implemented: slice-14 RED scaffold: seed a RICH-trail contributor whose multi-row
/score breakdown…`) → **MISSING_FUNCTIONALITY (genuine RED)**, NOT BROKEN (no
import/collection/setup error — the body reaches the seed call). The CARDINAL
`SF-INV-ByteId` likewise panics at its slice-14 `todo!()` seed. All 14 scenarios reach a
`todo!()`-stubbed seam → classify RED. DELIVER unskips + materializes one scenario at a time
(RED → GREEN → COMMIT per ADR-025).

---

## Changelog

- 2026-06-08 — slice-14 DISTILL. Reconciliation passed (0 contradictions across DISCUSS /
  DESIGN / ADR-048/051). 14 RED acceptance tests authored (8 story + 6 GOLD invariants)
  covering `GET /score?contributor=<did>` (US-CF-001 infra wiring + US-CF-002 user-visible
  flag), with the N+1-flatten proxy + the CARDINAL sum-to-weight + byte-identity gold
  (markers + legend elided) + the anti-misread identical-subtotal proof + read-only /
  no-write / offline guardrails. Mirrors the slice-12/13 corpus (the `COUNTERED_PRESENCE_FLAG`
  marker, the `<a href="/claims/{cid}">` one-hop link, the baseline+marker-elision
  byte-identity tactic, the production federation seeds) and EXTENDS the slice-09 score
  corpus (the reproduce-by-hand `parse_score_pairings` parser, the `WeightedView` trail
  seeds, the `#score-results` fragment). NO new read method, NO new crate, NO new route, NO
  new render fn (the ONE new artifact is the render-only `SCORE_COUNTER_LEGEND` constant).
  All RED (`todo!()` → panic → MISSING_FUNCTIONALITY) until DELIVER. LAST viewer surface —
  completes J-003b.
