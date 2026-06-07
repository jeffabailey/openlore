<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-counter-flags-graph-surfaces — DISTILL wave (slice-13)

> Wave: **DISTILL** (lean mode) · Acceptance Designer: Quinn (nw-acceptance-designer)
> Date: 2026-06-07 · Brownfield DELTA on the read-only `openlore ui` viewer
> Scope (Option B, confirmed): `/peer-claims` rows + `/project`+`/philosophy` EDGE rows.
> **`/score` OUT (slice-14); `/search` OUT (own slice-08 annotation); `/claims` shipped slice-12.**
> ADR-025: ALL acceptance tests authored as scaffolded RED (`todo!()` → panic → RED).
> The executable `.rs` files are the scenario SSOT; this file is the structured summary.

This is the DISTILL-wave delta translating the DISCUSS user stories (US-CF-001/002/003)
and the DESIGN driving ports (ADR-049 reuse the slice-12 batch read across 3 handlers;
ADR-050 edge-CID flatten before `group_by` + `EdgeRow.is_countered`) into executable
acceptance tests. Mirrors the slice-12 corpus VERBATIM where shapes match (the
`COUNTERED_PRESENCE_FLAG` neutral marker, the `<a href="/claims/{cid}">` one-hop link, the
baseline+marker-elision byte-identity tactic, the production federation seeds).

---

## Wave: DISTILL / [REF] Wave-decision reconciliation HARD GATE

**Reconciliation passed — 0 contradictions.**

Read + cross-checked: DISCUSS `discuss/feature-delta.md` + `discuss/user-stories.md` +
`discuss/wave-decisions.md`; DESIGN `design/architecture-design.md` +
`design/component-boundaries.md`; ADR-049 (reuse `counter_presence_for` across the 3
handlers, NO new read method) + ADR-050 (flatten edge CIDs before `group_by`, set
`EdgeRow.is_countered` in the grouper) + ADR-048 (the REUSED slice-12 batch read). Every
DISCUSS decision (Option B scope; reuse-first / no new read method / no new crate /
no-regroup / presence-only / read-only / local-offline; `/score` deferred to slice-14) is
honored verbatim by DESIGN §7/§8 and the two ADRs. No DESIGN or DEVOPS decision contradicts
any DISCUSS decision.

---

## Wave: DISTILL / [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#US-CF-002 | At-a-glance "Countered" flag on each `/peer-claims` row with ≥1 counter, linking to the slice-11 thread; peer origin + confidence + order unchanged | n/a | CF-1..CF-4 + CF-NoNoise scenarios drive `GET /peer-claims` port-to-port through the real `openlore ui` subprocess |
| DISCUSS#US-CF-003 | At-a-glance "Countered" flag on each `/project`+`/philosophy` EDGE with ≥1 counter; never re-groups/re-orders the survey; byte-identical to slice-10 | n/a | CF-5..CF-8 + CF-N1 scenarios drive `GET /project` + `GET /philosophy`; one shared `render_edge_row` arm → one scenario set + a symmetric `/philosophy` check |
| DISCUSS#US-CF-001 | Reuse the slice-12 `counter_presence_for(&[cid])` batch read across the 3 handlers — ONE aggregate query per render, no N+1, NO new read method | ADR-049/050 | Observable THROUGH US-CF-002/003: the CF-N1 N+1-flatten behavioral proxy (MANY edges across MANY groups flagged in ONE request) |
| DESIGN#ADR-050 | Flatten all edge CIDs across `EdgeGroup`s into ONE call BEFORE grouping; set `EdgeRow.is_countered` in `group_by` | ADR-050 | CF-N1 + CF-INV-N1 pin the single-flattened-call behavior at the subprocess layer; the strict 1-query bound is a DELIVER adapter test |
| DISCUSS#I-CF-9 (CARDINAL) | No re-grouping / no re-ordering of the survey — byte-identical to slice-10 with markers elided | ADR-015 | CF-INV-ShownNeverApplied (the CARDINAL no-regroup gold) on BOTH traversal routes via the slice-12 baseline+marker-elision tactic |

---

## Wave: DISTILL / [REF] Scenario list with tags

### Story scenarios — `viewer_counter_flags_graph_surfaces.rs` (10 slice-13 tests)

| # | Scenario (fn) | Route / US | Walking skeleton? | Key tags |
|---|---|---|---|---|
| CF-1 | `open_the_peer_claims_list_with_htmx_flags_only_the_countered_row` | `GET /peer-claims` · US-CF-002 | **YES** | `@walking_skeleton @driving_port @real-io @htmx-fragment @i-cf-2 @i-cf-3 @i-cf-6 @happy` |
| CF-2 | `the_peer_claims_flag_renders_identically_under_htmx_and_no_js` | `GET /peer-claims` · US-CF-002 | — | `@driving_port @real-io @no-js @full-page @parity @i-cf-4 @i-cf-6 @happy` |
| CF-3 | `a_peer_claim_with_two_counters_shows_one_neutral_presence_marker_on_the_list` | `GET /peer-claims` · US-CF-002 | — | `@driving_port @real-io @presence-only @anti-merging @i-cf-3 @kpi-av-2 @gold` |
| CF-4 | `the_peer_claims_countered_marker_is_a_render_only_one_hop_link_to_the_thread` | `GET /peer-claims` · US-CF-002 | — | `@driving_port @real-io @drill-link @one-hop @i-cf-6 @happy` |
| CF-NoNoise(peer) | `a_store_with_no_counters_renders_the_peer_claims_list_exactly_as_slice_06` | `GET /peer-claims` · US-CF-002 | — | `@driving_port @real-io @no-noise @empty-set @shown-never-applied @i-cf-2 @happy` |
| CF-5 | `a_countered_edge_in_a_project_survey_is_flagged_in_its_unchanged_position` | `GET /project` · US-CF-003 | — | `@driving_port @real-io @project @i-cf-6 @i-cf-9 @happy` |
| CF-6 | `a_philosophy_survey_flags_only_countered_edges_and_never_regroups_or_reorders` | `GET /philosophy` · US-CF-003 | — | `@driving_port @real-io @philosophy @symmetric @i-cf-6 @i-cf-9 @happy` |
| CF-7 | `a_survey_with_no_counters_renders_the_edges_exactly_as_slice_10` | `GET /project` · US-CF-003 | — | `@driving_port @real-io @no-noise @empty-set @i-cf-2 @i-cf-9 @happy` |
| CF-8 | `a_twice_countered_edge_shows_one_neutral_marker_linking_to_its_thread` | `GET /project` · US-CF-003 | — | `@driving_port @real-io @presence-only @anti-merging @one-hop @i-cf-3 @i-cf-6 @kpi-graph-2 @gold` |
| CF-N1 | `a_large_multi_group_survey_flags_every_countered_edge_correctly_in_one_request` | `GET /project` · US-CF-001/003 | — | `@driving_port @real-io @n-plus-1-guard @i-cf-8 @i-cf-9 @gold` |

### GOLD invariants — `viewer_counter_flags_graph_surfaces_invariants.rs` (6 slice-13 tests)

| # | Invariant (fn) | Surfaces | Key tags |
|---|---|---|---|
| CF-INV-ReadOnly | `every_flagged_graph_surface_render_leaves_the_store_read_only` | all 3 × shapes × postures | `@property @driving_port @real-io @read-only @i-cf-1 @gold` |
| CF-INV-NoWrite | `no_flagged_graph_surface_render_adds_a_write_or_sign_control` | all 3 × shapes | `@property @driving_port @real-io @read-only @i-cf-1 @gold` |
| CF-INV-OfflineChrome | `the_flagged_graph_surface_chrome_stays_offline_no_cdn` | `/peer-claims` + `/project` | `@property @driving_port @real-io @offline @no-cdn @i-cf-5 @gold` |
| CF-INV-Offline | `the_flagged_graph_surfaces_render_fully_offline` | `/peer-claims` + `/project` | `@property @driving_port @real-io @offline @local-first @i-cf-5 @kpi-5 @gold` |
| CF-INV-ShownNeverApplied (CARDINAL) | `the_traversal_grouping_and_order_are_byte_identical_with_and_without_flags` | `/project` + `/philosophy` | `@property @driving_port @real-io @shown-never-applied @no-regroup @i-cf-9 @cardinal @gold` |
| CF-INV-N1 | `a_large_multi_group_survey_resolves_presence_in_one_request` | `/project` (many groups) | `@property @driving_port @real-io @n-plus-1-guard @i-cf-8 @gold` |

**Total: 16 slice-13 acceptance tests** (10 story + 6 GOLD invariants). All RED (`todo!()`
→ panic → MISSING_FUNCTIONALITY, never BROKEN) until DELIVER.

Error/edge/guardrail share: no-noise (CF-NoNoise, CF-7), presence-only/anti-merging
(CF-3, CF-8), N+1 (CF-N1, CF-INV-N1), no-regroup CARDINAL (CF-INV-ShownNeverApplied),
read-only / no-write / offline (CF-INV-ReadOnly/NoWrite/OfflineChrome/Offline) =
12/16 (75%) are guardrail/edge/no-regression, well above the 40% floor.

---

## Wave: DISTILL / [REF] Walking-skeleton strategy

**Brownfield DELTA — NO walking-skeleton Feature 0** (the `openlore ui` viewer, the three
routes + renders, the read-only store port, the slice-12 `counter_presence_for` batch read,
the `from_row_with_presence` projection pattern, the `COUNTERED_PRESENCE_FLAG` marker + the
`<a href="/claims/{cid}">` one-hop link shape, and the `page = chrome + fragment` render
pattern all already exist, slices 03/06/07/10/11/12).

**One walking-skeleton scenario**: CF-1 (`open_the_peer_claims_list_with_htmx_flags_only_
the_countered_row`) — the thinnest end-to-end thread (the closest mirror of the slice-12
WS), `@walking_skeleton @driving_port @real-io`. It closes the loop viewer → LOCAL peer-list
read → LOCAL batch presence read (REUSED slice-12) → pure projection → HTML fragment through
the production composition root (the real `openlore ui` subprocess). Architecture of
Reference: **Driving** port = `GET /peer-claims` via the real `ViewerServer` subprocess (real
adapter); **Driven internal** = the DuckDB store via the real `DuckDbStoreReadAdapter` (real,
seeded through production `peer add`+`peer pull`); **Driven external / non-deterministic** =
NONE on these routes (LOCAL read, offline by construction — no clock/network/LLM to fake).

---

## Wave: DISTILL / [REF] Driving-port + adapter coverage

Three driving ports, each exercised port-to-port through the REAL `openlore ui` subprocess
(no direct call to `counter_presence_for` or `viewer-domain` in any AC):

| Driving port | Scenarios | Both shapes (get / get_htmx)? |
|---|---|---|
| `GET /peer-claims` | CF-1, CF-2, CF-3, CF-4, CF-NoNoise + CF-INV-* | YES (CF-1 fragment, CF-2 both, CF-INV both) |
| `GET /project?subject=<uri>` | CF-5, CF-7, CF-8, CF-N1 + CF-INV-* | full-page (CF-INV-NoWrite exercises both) |
| `GET /philosophy?object=<uri>` | CF-6 + CF-INV-ShownNeverApplied | full-page |

**Driven-adapter coverage (Mandate 6)**: the ONLY driven dependency exercised is
`counter_presence_for` on `DuckDbStoreReadAdapter` — **REUSED VERBATIM from slice-12**
(ADR-049), whose real-I/O integration + N+1 property test already exist (slice-12 /
ADR-048). slice-13 adds NO new driven adapter, NO new port, NO new SQL
(component-boundaries.md §3/§5/§6/§7 — Earned-Trust: no new probe owed). The CF-N1 +
CF-INV-N1 scenarios are the subprocess-layer behavioral proxy for the single-flattened-call
guarantee (the strict 1-query bound stays the DELIVER `adapter-duckdb` unit/property test).
No `@real-io @adapter-integration` row is owed BY this slice — the REUSED adapter's real-I/O
test is inherited.

---

## Wave: DISTILL / [REF] Scaffold files (RED-ready, Mandate 7 / ADR-025)

| File | Role |
|---|---|
| `tests/acceptance/viewer_counter_flags_graph_surfaces.rs` | 10 story scenarios (US-CF-002/003); CF-1 = walking skeleton. `// SCAFFOLD: true`; bodies reach `todo!()`-stubbed seeds/asserts → panic → RED. |
| `tests/acceptance/viewer_counter_flags_graph_surfaces_invariants.rs` | 6 GOLD/guardrail invariants (I-CF-1/2/5/8/9). `// SCAFFOLD: true`; bodies reach `todo!()` → panic → RED. |
| `tests/acceptance/support/mod.rs` (slice-13 block appended) | New seams, all `todo!()`-stubbed (compile, then panic). |
| `crates/cli/Cargo.toml` | Two new `[[test]]` targets registered (`viewer_counter_flags_graph_surfaces` + `_invariants`). |

### New support seams (all `todo!()`-stubbed — compile, then panic → RED)

**Types**: `SeededPeerClaimsList` (ordered/countered/uncountered CIDs + peer-origin DID),
`SeededSurveyEdges` (entity + ordered/countered/uncountered edge CIDs).

**Read helpers**: `read_peer_claim_cids_in_list_order`,
`read_survey_edge_cids_in_render_order(dimension, entity)`.

**Seeds**: `seed_peer_claims_one_countered`,
`seed_peer_claims_target_two_counters_distinct_authors`, `seed_peer_claims_none_countered`,
`seed_project_survey_one_edge_countered`, `seed_philosophy_survey_one_edge_countered`,
`seed_project_survey_edge_two_counters_distinct_authors`, `seed_survey_none_countered(dimension)`,
`seed_project_survey_many_groups_known_countered_subset`.

**Asserts**: `assert_peer_claim_row_flagged_countered`, `assert_peer_claim_row_not_flagged`,
`assert_peer_claim_flag_links_to_thread`, `assert_peer_claim_flag_is_single_neutral_presence`,
`assert_peer_claim_row_origin_unchanged`, `assert_peer_claims_order_byte_identical`,
`assert_edge_flagged_countered`, `assert_edge_not_flagged`, `assert_edge_flag_links_to_thread`,
`assert_edge_flag_is_single_neutral_presence`, `assert_survey_grouping_and_order_byte_identical`.

**REUSED slice-11/12 seams** (no new copy): `LIST_COUNTERED_FLAG_TEXT`,
`assert_counter_thread_presence_flag_is_neutral`, `assert_traversal_html_has_no_write_or_sign_control`,
`capture_store_row_count_universe` + `assert_store_read_only` (Mandate 8 universe-bound),
`seed_peer_claims_via_pull` / `seed_project_survey_trail` / `seed_philosophy_survey_trail` /
`build_verifiable_peer_counter_record`, `ViewerServer::{start,get,get_htmx}` +
`ViewerResponse::{is_fragment,is_full_page,references_external_cdn}`.

---

## Wave: DISTILL / [REF] Test placement + layer / PBT-mode discipline

- **Placement**: `tests/acceptance/` (the established brownfield viewer acceptance corpus —
  precedent: slices 06–12 all live there; `crates/cli/Cargo.toml` registers each as a
  `[[test]]` target).
- **Layer (nw-tdd-methodology + Mandate 9/11)**: every slice-13 scenario is a
  layer-3/layer-5 subprocess + real-I/O test — **EXAMPLE-only**. Sad/edge paths
  (none-countered, multi-counter, mixed survey) are enumerated explicitly, **never
  PBT-generated** at this layer (Mandate 11). No `@given`/PBT machinery is imported at this
  layer (Mandate 9).
- **Mandate 8 (Universe)**: the read-only GOLD (`CF-INV-ReadOnly`) uses the universe-bound
  `assert_store_read_only` (universe = the two port-exposed counts `claims.row_count` +
  `peer_claims.row_count`, each `unchanged`). Other layer-3+ assertions use traditional
  rendered-HTML scans (port-exposed surface), permitted at layer 4+ per Mandate 8.
- **Mandate 10 (Tier B)**: **Tier A only**. Each journey is 1–2 chained scenarios over a
  config-shaped render (a boolean presence flag); no ≥3-chained domain-rich journey → Tier B
  state-machine PBT is NOT warranted (the generative exploration of the pure projection/render
  is the DELIVER `viewer-domain` unit/property concern).

---

## Wave: DISTILL / [REF] Pre-requisites

- **DESIGN driving ports**: `GET /peer-claims` (slice-06), `GET /project?subject=<uri>` +
  `GET /philosophy?object=<uri>` (slice-10) — all EXTENDED, no new route.
- **REUSED read**: `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>`
  (slice-12 / ADR-048, `crates/ports/src/store_read.rs:380`) — confirmed present, REUSED
  verbatim (ADR-049). NO new read method.
- **Environment**: LOCAL DuckDB seeded via production federation paths; NO network seam on
  any of the three routes (offline by construction). Build-before-run: `cargo build -p cli
  --bin openlore` before running the ATs so `ViewerServer::start` spawns the current viewer.

---

## Wave: DISTILL / [REF] RED classification (pre-DELIVER fail-for-the-right-reason gate)

`cargo build -p cli --test viewer_counter_flags_graph_surfaces --test
viewer_counter_flags_graph_surfaces_invariants` → **compiles** (only pre-existing
`support/mod.rs` warnings; zero errors). Running CF-1 panics at the slice-13 `todo!()` seed
(`not yet implemented: slice-13 RED scaffold: seed a /peer-claims page…`) →
**MISSING_FUNCTIONALITY (genuine RED)**, NOT BROKEN (no import/collection/setup error). All
16 scenarios reach a `todo!()`-stubbed seam → classify RED. DELIVER unskips + materializes
one scenario at a time (RED → GREEN → COMMIT per ADR-025).

---

## Changelog

- 2026-06-07 — slice-13 DISTILL. Reconciliation passed (0 contradictions across DISCUSS /
  DESIGN / ADR-048/049/050). 16 RED acceptance tests authored (10 story + 6 GOLD invariants)
  covering `GET /peer-claims` (US-CF-002) + `GET /project` + `GET /philosophy` edges
  (US-CF-003), with the N+1-flatten proxy + the CARDINAL no-regroup gold + read-only /
  no-write / offline guardrails. Mirrors the slice-12 corpus (the `COUNTERED_PRESENCE_FLAG`
  marker, the `<a href="/claims/{cid}">` one-hop link, the baseline+marker-elision
  byte-identity tactic, the production federation seeds). NO new read method, NO new crate,
  NO new route. All RED (`todo!()` → panic → MISSING_FUNCTIONALITY) until DELIVER. `/score`
  deferred to slice-14.
