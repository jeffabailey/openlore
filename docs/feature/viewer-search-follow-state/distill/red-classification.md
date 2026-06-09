# RED Classification — viewer-search-follow-state (slice-16)

> Wave: **DISTILL** · Owner: Quinn (nw-acceptance-designer) · 2026-06-09
> Pre-DELIVER fail-for-the-right-reason gate (ADR-025 D2: this becomes the DELIVER RED
> phase entry/exit gate). DELIVER reads this at PREPARE to confirm RED is genuine.
> Run serially (`--test-threads=1`) for clean classification — the subprocess viewers +
> `peer add` seeding flake under parallel load (the known parallel-flake class; cf.
> commit 2629e56). Bins built first: `openlore` + `openlore-indexer`.

## Classification (serial run)

| Scenario | Status | Class | Why |
|---|---|---|---|
| SF-1 followed→Following, unfollowed→peer add | FAILED | **RED — MISSING_FUNCTIONALITY** | Rachel (seeded active sub) STILL renders `openlore peer add did:plc:rachel-test` — the hardcoded `NetworkUnfollowed`; the `SubscribedPeer` resolution + `render_following_indicator` arm don't exist. `assert_search_row_following` fires on the missing "Following" indicator. |
| SF-2 all-followed | FAILED | **RED — MISSING_FUNCTIONALITY** | Both followed authors still render `peer add`; no "Following" arm. |
| SF-3 none-followed status quo | ok | **green guard** | The slice-08 behavior (all `peer add`, no "Following") + the E degrade-TARGET — exercisable today. Validates the helpers do not false-RED. |
| SF-4 no executable control | ok | **green guard** | No executable follow/subscribe control exists today; guards the NEW arm DELIVER adds. |
| SF-5 LOCAL resolution | FAILED | **RED — MISSING_FUNCTIONALITY** | Rachel's row not resolved to "Following" against the LOCAL set. |
| SF-6 fragment-strip match | FAILED | **RED — MISSING_FUNCTIONALITY** | The fragmented result DID is not reconciled against the bare active-set DID (no resolution). |
| SF-7 large multi-result (no-N+1 proxy) | FAILED | **RED — MISSING_FUNCTIONALITY** | The one followed author among 8 not resolved to "Following". |
| SF-8 failed-read degrades | FAILED | **RED — MISSING_FUNCTIONALITY** | Panics at the `todo!()` fault-injection seam `start_viewer_with_failing_active_set_read` — the degrade-on-read-failure path doesn't exist. |
| SF-9 htmx vs no-JS parity | FAILED | **RED — MISSING_FUNCTIONALITY** | Neither shape renders Rachel "Following". |
| SF-10 attribution + ranking unchanged | FAILED | **RED — MISSING_FUNCTIONALITY** | Attribution/anti-merging/confidence assertions pass; the "Following" assertion fires (resolution missing). |
| SF-INV-NoControl | ok | **green guard** | No executable control today; guards the NEW arm. |
| SF-INV-ReadOnly | ok | **green guard** | The LOCAL read + in-memory resolution persists nothing; counts unchanged. |
| SF-INV-LocalPerUserNeutral | FAILED | **RED — MISSING_FUNCTIONALITY** | Following-operator's Rachel row still `peer add` (resolution missing). The same-rows / per-user-neutral assertions pass; the affordance-flip assertion fires. |
| SF-INV-AttributionUnchanged | FAILED | **RED — MISSING_FUNCTIONALITY** | Same — grouping/order/confidence pass; the affordance-flip assertion fires. |

**Totals (serial):** story suite 8 RED + 4 green-guard (incl. 2 support self-tests);
invariants suite 2 RED + 4 green-guard (incl. 2 support self-tests).

## Verdict

**RED is genuine.** Zero scenarios in the BROKEN classes (no IMPORT_ERROR / FIXTURE_BROKEN /
SETUP_FAILURE / WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT). Every RED is
MISSING_FUNCTIONALITY — the production `to_indexed_claim` hardcodes `NetworkUnfollowed` and
the `SubscribedPeer` render arm + the `SEARCH_FOLLOWING_INDICATOR` const + the fault-injection
degrade seam do not exist yet.

Evidence the RED is the PRODUCTION arm (not fixture theater): SF-1's panic body shows the full
end-to-end render — the viewer spawned, queried the REAL indexer, the `peer add` seeded
Rachel's subscription into the SAME store, BOTH authors render attributed + `[verified]`, and
Rachel's row reads `openlore peer add did:plc:rachel-test`. The fixtures correctly seed the
followed state; the test fails precisely because production code ignores it. A deletion test
holds: reverting the (future) DELIVER resolution + render arm would re-RED these scenarios.

The green guards (SF-3, SF-4, SF-INV-NoControl, SF-INV-ReadOnly) confirm the assert helpers do
not false-RED: the slice-08 status quo + the no-control + read-only invariants pass today, so
a RED on the follow-state scenarios is a true signal, not a helper bug.

## DELIVER entry

DELIVER unskips/implements one scenario at a time (ADR-025 RED→GREEN→COMMIT), starting with
SF-1 (the walking skeleton). The fault-injection seam (`start_viewer_with_failing_active_set_read`)
is materialized when SF-8 is reached. No AT is re-authored in RED — DISTILL is the canonical
AT author (ADR-025).
