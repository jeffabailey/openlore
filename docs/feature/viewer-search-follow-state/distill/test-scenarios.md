# Test Scenarios — viewer-search-follow-state (slice-16)

> Wave: **DISTILL** · Owner: Quinn (nw-acceptance-designer) · 2026-06-09
> Driving port: `GET /search` port-to-port via the REAL `openlore ui` subprocess
> (`ViewerServer::start_with_indexer`, slice-08 harness) + in-test HTTP (full page +
> htmx fragment). Network index = the ONLY mocked boundary (a REAL slice-05
> `openlore-indexer serve` over a seeded corpus). LOCAL DuckDB = REAL; active
> subscriptions seeded via the REAL slice-03 `peer add` verb.
> Layer: 3/5 (subprocess + real-I/O), EXAMPLE-only (Mandate 9/11). Tier A only (Tier B
> not warranted — see §Tier decision). ALL scaffolded RED (ADR-025).

## Files

- `tests/acceptance/viewer_search_follow_state.rs` — 10 story scenarios (SF-1..SF-10).
- `tests/acceptance/viewer_search_follow_state_invariants.rs` — 4 GOLD guardrails (SF-INV-*).
- `tests/acceptance/support/mod.rs` — slice-16 seeds + asserts (appended).
- `crates/cli/Cargo.toml` — two new `[[test]]` binaries registered.

## Scenario list (tags + AC mapping)

| ID | Scenario | Theme / AC | Story | Tags | RED? |
|---|---|---|---|---|---|
| SF-1 | A followed author shows "Following" while an unfollowed author keeps `peer add` | A (accuracy, load-bearing) / C-2 / R-SF-3,-4 | US-SF-002 | `@walking_skeleton @driving_port @driving_adapter @real-io @c-2 @happy` | RED (followed still renders `peer add`) |
| SF-2 | All-followed results show "Following" everywhere, no add anywhere | A / C-2 / FR-SF-4 | US-SF-002 | `@driving_port @real-io @c-2 @happy` | RED |
| SF-3 | None-followed results preserve the slice-08 status quo | A / C-2 / FR-SF-5 | US-SF-002 | `@driving_port @real-io @status-quo @c-2 @boundary` | **green guard** (slice-08 behavior + E degrade-target) |
| SF-4 | Neither follow-state affordance is an executable control | B (read-only) / C-1 CARDINAL / NFR-SF-1 | US-SF-002 | `@driving_port @real-io @read-only @c-1 @happy` | **green guard** (no control today; guards the NEW arm) |
| SF-5 | The relationship is resolved against the LOCAL active set, not the network | C (LOCAL/offline) / C-3 / NFR-SF-4 | US-SF-001 | `@driving_port @real-io @local-resolution @offline @c-3 @happy` | RED |
| SF-6 | A followed author is matched despite the signing-key fragment on the result DID | C / R-SF-5 / FR-SF-3 | US-SF-001 | `@driving_port @real-io @fragment-strip @r-sf-5 @edge` | RED |
| SF-7 | A large multi-result search resolves all rows in one render (no-N+1 proxy) | D (no N+1) / C-4 / NFR-SF-3 | US-SF-001 | `@driving_port @real-io @no-n-plus-1 @c-4 @edge` | RED |
| SF-8 | A failed active-set read degrades to the slice-08 status quo without crashing | E (graceful degrade) / C-7 / FR-SF-7 / NFR-SF-6 | US-SF-001 | `@driving_port @real-io @graceful-degrade @error @c-7` | RED (via `todo!()` fault-injection seam) |
| SF-9 | The follow-state renders identically under htmx and no-JS | F (parity) / C-8 / FR-SF-6 / NFR-SF-7 | US-SF-002 | `@driving_port @real-io @parity @c-8 @happy` | RED |
| SF-10 | Following + unfollowed authors render side by side, attribution + order preserved | G (anti-merging) / C-5 / NFR-SF-5 / J-003a | US-SF-002 | `@driving_port @real-io @anti-merging @attribution @c-5 @edge` | RED |
| SF-INV-NoControl | No /search follow-state render adds an executable control | C-1 CARDINAL | US-SF-002 | `@property @driving_port @real-io @read-only @c-1 @gold` | green guard (NEW arm adds no control) |
| SF-INV-ReadOnly | Every follow-state render leaves the store read-only | C-1 / Mandate 8 | US-SF-001 | `@property @driving_port @real-io @read-only @c-1 @gold` | green guard (read persists nothing) |
| SF-INV-LocalPerUserNeutral | The resolution adds no network seam; the index stays per-user-neutral | C-3 / WD-SF-4 | US-SF-001 | `@property @driving_port @real-io @local-resolution @offline @c-3 @gold` | RED (following operator's row still `peer add`) |
| SF-INV-AttributionUnchanged | The resolution does not merge or re-rank | C-5 / J-003a / WD-SF-8 | US-SF-002 | `@property @driving_port @real-io @anti-merging @attribution @c-5 @gold` | RED |

### Coverage map (every AC theme A–G is covered)

| Theme | Story scenarios | Gold |
|---|---|---|
| A — accuracy | SF-1, SF-2, SF-3 | — |
| B — read-only / no write | SF-4 | SF-INV-NoControl, SF-INV-ReadOnly |
| C — LOCAL / offline | SF-5, SF-6 | SF-INV-LocalPerUserNeutral |
| D — no N+1 | SF-7 | (read-once is part of SF-INV-ReadOnly's read posture) |
| E — graceful degrade | SF-8 (true read-failure, RED scaffold) + SF-3 (the degrade-TARGET, exercisable today) | — |
| F — htmx vs no-JS parity | SF-9 | (parity by construction; both shapes in SF-4 + SF-INV-NoControl) |
| G — attribution + ranking unchanged | SF-10 | SF-INV-AttributionUnchanged |

Every US-SF-001 + US-SF-002 AC has at least one scenario (traceability complete).

### Error / edge ratio (Dimension 1)

Error/edge scenarios: SF-3 (boundary/status-quo), SF-6 (edge/fragment), SF-7 (edge/scale),
SF-8 (error/degrade), SF-10 (edge/anti-merging) = **5/10 story scenarios = 50%** (≥40%).

## Seeds + asserts added to `support/mod.rs`

### Seeds

| Helper | What it seeds |
|---|---|
| `seed_active_subscription_for(env, did, seed)` | ONE ACTIVE `peer_subscriptions` row for `did` via the REAL slice-03 `peer add` verb (subscribe-only, no pull — the resolution reads `peer_subscriptions`, not `peer_claims`). Parameterized mirror of slice-15 `seed_peer_subscribed_zero_claims`. Pins `assert_one_active_subscription_for`. |
| `sf_corpus_one_followed_one_unfollowed()` | Index corpus: Rachel (`did:plc:rachel-test`, FOLLOWED) + Priya (`did:plc:priya-test`, UNFOLLOWED) each assert the headline object. The MIX render. |
| `sf_corpus_all_authors_followed()` | Index corpus: Rachel + Tobias only (both FOLLOWED in the scenario). The all-followed case. |
| `sf_corpus_many_results_one_followed()` | Index corpus: 8 distinct authors, exactly ONE (Rachel) followed. The no-N+1 behavioral proxy. |
| `start_viewer_with_failing_active_set_read(env, indexer)` | RED scaffold (`todo!()`) for the TRUE mid-request active-set-read-failure path (see §Graceful-degrade seam). |
| (reused) `seed_network_index(ReproducibleBuildsNineAuthorsUnfollowed)` | The slice-08 nine-unfollowed-authors corpus — the none-followed status-quo (SF-3). |
| (made `pub`) `seed_network_index_from_specs(env, specs)` | The slice-08 explicit-corpus index seam, promoted to `pub` so slice-16 corpora can be served. No behavior change. |

### Asserts

| Helper | What it asserts (port-exposed rendered surface) |
|---|---|
| `assert_search_row_following(body, did)` | `did` attributed + the neutral `"Following"` indicator present + NO `openlore peer add <did>` command for `did` (the load-bearing accuracy fix, C-2 / R-SF-3). |
| `assert_search_row_offers_follow(body, did)` | `did` attributed + the render-only `openlore peer add <did>` affordance retained (no over-correction, C-2 / R-SF-4). |
| `assert_search_follow_state_is_render_only(body)` | NEITHER affordance is an executable control — no follow/unfollow/subscribe control, no `>Following<` control element, no mutating `hx-*` (C-1 CARDINAL). |
| (reused) `assert_search_html_every_row_verified_and_attributed`, `assert_search_html_has_no_merged_consensus_row`, `assert_search_html_leaks_no_transport_internals`, `capture_store_row_count_universe` + `assert_store_read_only` | slice-08 / slice-15 attribution / anti-merging / no-leak / read-only-delta gold helpers, REUSED verbatim. |

## Tier decision (Mandate 10)

**Tier A only.** Tier B (state-machine PBT) is NOT warranted: slice-16's resolution is a
**binary per-row enrichment** — `bare_did(author_did) ∈ active set → SubscribedPeer`, else
`NetworkUnfollowed` — not a ≥3-scenario chained journey over a rich state machine. The
observable is "which render-only affordance the row shows", a per-row config-shaped choice
(Mandate 10 skip criteria: 1–2 observable transitions, no state machine to model). The
generative exploration of the pure resolution fn (set membership, fragment-strip, degrade)
is a DELIVER layer-1/2 `@property` + mutation-testing concern (ADR-053 §Enforcement).

## Graceful-degrade (Theme E) read-failure seeding — documented choice

ADR-053 §Earned-Trust names the substrate "lie" to survive as a **mid-request active-set-read
FAILURE** degrading to an empty set → all-`NetworkUnfollowed`. The slice-08/15 `ViewerServer`
harness holds ONE long-lived DuckDB connection taken at STARTUP (wire→probe→use, ADR-028/030),
so the existing `make_store_unreadable` lock would refuse STARTUP rather than exercise a
mid-request read failure. There is **no readily-available mid-request read-failure seam** in
the slice-08/15 harness.

Per the DISTILL guidance, E is scaffolded as TWO complementary scenarios:

- **SF-3 (the degrade TARGET, exercisable TODAY):** the none-followed / empty-active-set case
  produces exactly the slice-08 status quo (all `peer add`, no "Following"). This is the
  OBSERVABLE state a failed read degrades TO — fully pinned now (green guard).
- **SF-8 (the TRUE read-failure path, RED scaffold):** `start_viewer_with_failing_active_set_read`
  is a `todo!()` seam DELIVER materializes (mirroring how slice-08 left `start_inner` as
  `todo!()`). DELIVER picks the fault-injection mechanism (e.g. an
  `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` test seam in the effect shell, or a poisonable
  per-request connection) with the SAME observable contract: a failed active-set read degrades
  to all-`NetworkUnfollowed`, no crash/blank/5xx/leak.

This keeps the AC-E contract pinned (SF-3) while honestly scaffolding the true-failure path
(SF-8) as a DELIVER concern.

## Mandate compliance

- **CM-A (hexagonal boundary):** every scenario enters through `GET /search` via the real
  `openlore ui` subprocess. No scenario imports/calls `viewer-domain` render fns or the adapter
  resolution fn directly. The two test files import only `support::*` + the public
  `openlore_test_support` DID constants.
- **CM-B (business language):** scenario names + the AC describe WHAT the operator achieves
  ("a followed author shows Following while an unfollowed author keeps peer add"), never the
  HTTP/SQL/HashSet mechanism. Technical detail lives in the seed/assert helper bodies.
- **CM-C (user journeys):** every scenario validates the operator's discovery→follow decision
  (does the surface correctly tell her whom she already follows). SF-1 is the demo-able thick
  thread.
- **CM-E/CM-H (Mandate 8/11):** layer-3 subprocess; the read-only gold uses the universe-bound
  `assert_store_read_only` (Mandate 8); sad paths (SF-3, SF-8) are named example-based, never
  PBT-generated (Mandate 11). No PBT machinery imported at this layer (Mandate 9 / CM-F).
- **CM-G (Mandate 10):** Tier B correctly ABSENT (binary per-row resolution; documented above).
