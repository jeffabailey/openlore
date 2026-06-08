# Walking Skeleton: viewer-counter-flags-score-surface (slice-14) — DISTILL

> Acceptance Designer: Quinn · 2026-06-08

## Strategy

**Brownfield DELTA — NO walking-skeleton Feature 0.** Every layer the WS would otherwise
stand up already exists and is green: the `openlore ui` viewer + the `GET /score` route +
the `resolve_score_state` / `render_score_*` chain (slice-09); the read-only DuckDB store
port + `DuckDbStoreReadAdapter` (slice-03/06); the `counter_presence_for(&[cid])` batch
read (slice-12 / ADR-048); the `render_countered_link` + `COUNTERED_PRESENCE_FLAG` neutral
marker SSOT (slice-13); the reproduce-by-hand `parse_score_pairings` parser + the
`WeightedView` trail seeds (slice-09); the `page = chrome + fragment` + `#score-results`
render pattern (slice-07/09).

## The ONE walking-skeleton scenario

**SF-1** — `open_the_score_breakdown_with_htmx_flags_only_the_countered_contribution`
(`tests/acceptance/viewer_counter_flags_score_surface.rs`).

Tags: `@walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment @flag
@reuse-render @presence-only @cardinal-sum-to-weight @anti-misread @happy`.

The thinnest complete end-to-end thread the `/score` flag feature can demo:

```
viewer (real ViewerServer subprocess, GET /score?contributor=<did>, HX-Request)
  → LOCAL scoring-feed read (query_contributor_scoring_feed, no network)
  → PURE scoring::score (the REUSED slice-04 core, UNCHANGED)
  → LOCAL batch presence read (counter_presence_for, REUSED slice-12, flattened ONCE)
  → pure projection threading &presence into render_score_* (the slice-14 seam)
  → HTML #score-results fragment
```

Observable outcome (the Then a non-technical stakeholder confirms): on a scored
contributor whose multi-row breakdown has exactly one peer-countered contribution, the
htmx fragment shows the neutral "Countered" marker beside that one contribution row
(linking to its `/claims/{cid}` thread), no marker on the others, the anti-misread legend
present, and the per-contribution subtotals STILL summing to the displayed pairing weight
— proving at a glance that the flag is shown for the reader to judge and is orthogonal to
the score (shown, never applied). This single scenario exercises the marker (AC-002-MARKER/
LINK), the presence read wiring (US-CF-001), the legend (AC-SCORE-ANTIMISREAD), and the
CARDINAL sum-to-weight orthogonality (AC-SCORE-SUMWEIGHT) in one thread.

## Architecture of Reference (port treatment)

| Port class | Port | Treatment |
|---|---|---|
| **Driving** | `GET /score?contributor=<did>` | Real adapter — the production `ViewerServer` subprocess (hyper, 127.0.0.1) |
| **Driven internal** | DuckDB store (`StoreReadPort::counter_presence_for` + `query_contributor_scoring_feed`) | Real adapter — `DuckDbStoreReadAdapter`, seeded through production `peer add` + `peer pull` |
| **Driven external / non-deterministic** | NONE | `/score` is a LOCAL read + PURE compute — offline by construction, no clock/network/LLM to fake |

## RED gate

SF-1 compiles and panics at the slice-14 `todo!()` seed
(`seed_score_breakdown_one_contribution_countered`) → MISSING_FUNCTIONALITY (genuine RED),
NOT BROKEN. It stays RED until DELIVER's per-scenario RED → GREEN → COMMIT cycle.
