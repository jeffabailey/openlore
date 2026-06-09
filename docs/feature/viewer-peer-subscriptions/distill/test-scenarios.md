# Test Scenarios — viewer-peer-subscriptions (slice-15) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09
> Driving surface: the real `openlore ui` subprocess, route `GET /peers` (port-to-port
> via the real `ViewerServer`; htmx fragment vs no-JS full page via `HX-Request`).
> Mirrors the slice-10 (`viewer-graph-traversal`) DISTILL structure (the most recent
> net-new-route slice): one thick walking-skeleton scenario first, story scenarios mapping
> the AC IDs, + a GOLD invariants file for the cross-cutting guardrails.

## Test files

- Story scenarios: `tests/acceptance/viewer_peer_subscriptions.rs` (PS-1..PS-7)
- GOLD invariants: `tests/acceptance/viewer_peer_subscriptions_invariants.rs` (PS-INV-*)
- Shared seeds + asserts: `tests/acceptance/support/mod.rs` (slice-15 block, appended)
- Cargo `[[test]]` entries: `crates/cli/Cargo.toml`

All scenarios are layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only (Mandate 9/11) — the
sad paths (empty state, removed-peer-absent, only-removed-empty) are enumerated explicitly,
never PBT-generated at this layer (the strict 1-query bound + the pure projection
exploration are layer-1/2 DELIVER concerns).

## Story scenarios (`viewer_peer_subscriptions.rs`)

| ID | Scenario | Story | Tags | AC theme |
|---|---|---|---|---|
| PS-1 | `open_peers_with_htmx_returns_only_the_peers_fragment_with_did_count_and_revoke_command` | US-PS-002 | `@walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment @i-ps-2 @i-ps-3 @i-ps-5 @i-ps-8 @kpi-fed-4 @happy` | 1 (list: DID + per-peer count + render-only revoke command) |
| PS-2 | `the_peers_list_full_page_and_fragment_render_the_same_region` | US-PS-002 | `@driving_port @real-io @no-js @full-page @parity @i-ps-5 @happy` | 6 (htmx-vs-no-JS parity) |
| PS-3 | `the_per_peer_count_is_never_a_merged_total` | US-PS-002 | `@driving_port @real-io @anti-merging @i-ps-3 @kpi-av-2 @boundary` | 7 (anti-merging / per-peer counts, J-003a) |
| PS-4 | `a_peer_removed_via_the_cli_is_absent_from_peers_even_though_its_cache_remains` | US-PS-002 | `@driving_port @real-io @active-only @residue-made-visible @i-ps-2 @kpi-fed-4 @boundary` | 2 (active-only / residue made visible, CARDINAL) |
| PS-5 | `a_followed_peer_with_zero_cached_claims_appears_with_count_zero_and_its_revoke_command` | US-PS-002 | `@driving_port @real-io @left-join @zero-claims @i-ps-8 @boundary` | 1 (zero-claims peer; LEFT JOIN + COUNT(pc.cid), DD-PS-2) |
| PS-6 | `no_active_subscriptions_shows_the_guided_empty_state_in_both_shapes` | US-PS-003 | `@driving_port @real-io @empty-state @parity @i-ps-5 @error` | 4 (guided empty state) |
| PS-7 | `a_store_with_only_a_soft_removed_peer_still_shows_the_empty_state` | US-PS-003 | `@driving_port @real-io @empty-state @active-only @i-ps-2 @error` | 4 (empty state via residue; the chained continuation of PS-4) |

## GOLD invariants (`viewer_peer_subscriptions_invariants.rs`)

| ID | Scenario | Tags | Invariant |
|---|---|---|---|
| PS-INV-ReadOnly | `every_peers_render_leaves_the_store_read_only` | `@property @driving_port @real-io @read-only @i-ps-1 @gold` | I-PS-1 / WD-PS-1 / KPI-VIEW-2 — `/peers` (populated + empty, both shapes) leaves `claims` + `peer_claims` row counts UNCHANGED (universe-bound `assert_store_read_only`, Mandate 8); the list is computed per request, persists nothing (I-PS-6) |
| PS-INV-NoWrite | `no_peers_response_adds_a_write_or_subscribe_control` | `@property @driving_port @real-io @read-only @no-write @i-ps-1 @gold` | I-PS-1 / WD-PS-1 (CARDINAL) — no write/subscribe/unsubscribe/remove/purge control on any shape; the revocation is render-only `openlore peer remove <did>` command TEXT only |
| PS-INV-OfflineChrome | `the_peers_page_chrome_stays_offline_no_cdn` | `@property @driving_port @real-io @offline @no-cdn @i-ps-4 @gold` | I-PS-4 / KPI-HX-G2 — the `/peers` page references ONLY the local `/static/htmx.min.js`, no off-host CDN |
| PS-INV-Offline | `the_peers_surface_works_fully_offline` | `@property @driving_port @real-io @offline @local-first @i-ps-4 @kpi-5 @gold` | I-PS-4 / KPI-5 — the subscription list + per-peer counts render network-down (LOCAL read; no PDS fetch / DID re-resolution / peer pull on this route) |
| PS-INV-NoNPlus1 | `a_large_active_set_resolves_per_peer_counts_in_one_request` | `@property @driving_port @real-io @no-n-plus-1 @i-ps-8 @gold` | I-PS-8 / DD-PS-1 — a multi-peer active set (counts 4/3/2/1) renders correctly in ONE request (behavioral N+1 proxy; the strict 1-query bound is the DELIVER adapter-duckdb unit/property test) |

## AC → scenario mapping (acceptance-criteria.md themes)

| AC theme | Cardinal | Scenarios |
|---|---|---|
| 1 — list (DID + per-peer count + render-only revoke command) | C-7 (render-only command) | PS-1, PS-5 |
| 2 — active-only / residue made visible | C-2 (CARDINAL) | PS-4, PS-7, PS-INV-ReadOnly (companion) |
| 3 — read-only / no-write | C-1 (CARDINAL) | PS-INV-ReadOnly, PS-INV-NoWrite |
| 4 — guided empty state | — | PS-6, PS-7 |
| 5 — LOCAL / offline + single-query | C-4, C-8 | PS-INV-OfflineChrome, PS-INV-Offline, PS-INV-NoNPlus1 |
| 6 — htmx-vs-no-JS parity | C-5 | PS-2, PS-6 |
| 7 — anti-merging / per-peer | C-3 (J-003a) | PS-3, PS-INV-NoNPlus1 (companion) |
| 8 — read capability (infra) | infra | PS-1, PS-5 (the read surfaced through the route); PS-6 (empty result, no error) |

## Error-path ratio

7 story scenarios + 5 GOLD = 12 total. Error/edge/boundary scenarios: PS-3 (`@boundary`),
PS-4 (`@boundary`), PS-5 (`@boundary`), PS-6 (`@error`), PS-7 (`@error`) + the 5 GOLD
guardrails (each pins a cross-cutting failure mode: a write-surface breach, a CDN leak, a
network-down regression, an N+1/merge regression). Error/edge/guardrail share =
**10 / 12 ≈ 83%**, well above the 40% Mandate target (the slice is dominated by the four
cardinal guardrails + the residue/empty-state/zero-claims edges).

## RED confirmation (pre-DELIVER fail-for-the-right-reason gate)

- Both test binaries COMPILE (`cargo test --no-run` — only pre-existing support warnings,
  zero errors).
- PS-1 (walking skeleton) runs: seeding via the production `peer add` + `peer pull` path
  SUCCEEDS, the viewer spawns, the HTTP GET reaches the route, and `GET /peers` returns
  **404 `<p>Not found.</p>`** → the assertion fires (`left: 404, right: 200`). This is
  **MISSING_FUNCTIONALITY** (the `/peers` route does not exist yet) — genuine RED, NOT
  BROKEN (no import / fixture / setup error).
- PS-INV-ReadOnly confirms the same 404 RED.
- The production seams that must land in DELIVER to turn these GREEN:
  `StoreReadPort::list_active_peer_subscriptions` + `ports::PeerSubscriptionSummary`, the
  `adapter-duckdb` read impl (the active-only + per-peer-count LEFT JOIN SQL), the
  `viewer-domain` `PeersView` ADT + `render_peers_fragment` / `render_peers_page` +
  `PEER_REMOVE_GUIDANCE_PREFIX` / `PEER_ADD_GUIDANCE_PREFIX` / `render_remove_guidance`, and
  the `adapter-http-viewer` `GET /peers` handler + route arm + nav link.
- `cargo xtask check-arch` stays OK (21 workspace members).
- The slice-10 suite (`viewer_graph_traversal` + `_invariants`) still compiles (the support
  changes are additive).

## Mandate compliance evidence

- **CM-A (Mandate 1, hexagonal boundary)**: every scenario enters through the real `openlore
  ui` subprocess + in-test HTTP (`ViewerServer::get` / `get_htmx`); no scenario imports or
  calls `viewer-domain` `render_peers_*` / the read method directly. The only `use` is
  `support::*`.
- **CM-B (Mandate 2/Pillar 1, business language)**: scenario names + the seed/assert helper
  names speak the domain ("a peer removed via the CLI is absent from peers even though its
  cache remains", `assert_peer_remove_command_is_render_only`); technical detail lives inside
  step bodies (DuckDB, HTTP, maud) only.
- **CM-C (Mandate 3, complete journeys)**: PS-1 demos the full operator goal (open /peers →
  see who I follow + the clean revocation path); PS-4 + PS-7 chain the residue → empty-state
  narrative (Pillar 2 — `seed_only_subscription_removed` REUSES the residue seed).
- **CM-E (Mandate 8, universe-bound state-delta)**: the read-only GOLD asserts via
  `assert_store_read_only(before, after)` over the port-exposed universe
  (`claims.row_count` + `peer_claims.row_count`), each `unchanged`. The render asserts scan
  ONLY the port-exposed rendered surface (the HTTP response body), never internal struct
  fields. Layer-4+ scenarios (the render-content asserts) use traditional string-scan
  assertions per the Mandate-8 layer carve-out.
- **CM-F (Mandate 9) + CM-H (Mandate 11)**: zero PBT machinery (`proptest` / `RuleBasedState
  Machine`) imported at this layer; every sad path is a named example-based test. The
  `@property` tag on the GOLD marks them as universal invariants for the reader, not a PBT
  directive at layer 3+.
- **CM-G (Mandate 10, Tier B)**: NO Tier B state-machine file. The `/peers` journey is 1-2
  observable scenarios per posture and the input space is NOT domain-rich (the active set is
  a list of DIDs with counts; no free-text / dates / payloads at the journey level) — Tier A
  example coverage is sufficient (the two-tier "skip Tier B" criteria hold).
