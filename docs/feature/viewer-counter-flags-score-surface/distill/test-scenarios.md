# Test Scenarios: viewer-counter-flags-score-surface (slice-14) — DISTILL

> Acceptance Designer: Quinn · 2026-06-08 · The executable `.rs` files are the scenario
> SSOT; this is the human-readable index. All RED (`todo!()` → panic → RED) per ADR-025.

Driving port (all scenarios): `GET /score?contributor=<did>` exercised port-to-port via
the real `ViewerServer` subprocess (hyper, 127.0.0.1, read-only LOCAL DuckDB), htmx
fragment vs no-JS full page via `HX-Request` (the slice-07 `Shape`). No scenario calls
`counter_presence_for`, `scoring::score`, or `viewer-domain` `render_score_*` directly.

## Story scenarios — `viewer_counter_flags_score_surface.rs`

| # | Scenario | AC | Invariant tag |
|---|---|---|---|
| SF-1 (WS) | A scored contributor with one countered contribution shows the neutral "Countered" marker beside that row (htmx fragment), linking to `/claims/{cid}`, the legend present, subtotals still summing to the weight | AC-002-MARKER, AC-002-LINK, AC-SCORE-SUMWEIGHT, AC-SCORE-ANTIMISREAD | `@walking_skeleton @presence-only @cardinal-sum-to-weight @anti-misread` |
| SF-2 | A contribution countered by TWO distinct authors shows ONE neutral marker, never "countered by N"; links to the thread | AC-SCORE-PRESENCE, AC-002-LINK | `@presence-only @anti-merging` |
| SF-3 | On a FLAGGED breakdown, the per-contribution subtotals still sum to the displayed pairing weight; the countered contribution's subtotal is its FULL original value (counter subtracts nothing) | AC-SCORE-SUMWEIGHT (CARDINAL) | `@cardinal-sum-to-weight @shown-never-applied` |
| SF-4 | Adding the flag changes no weight/ranking/row order vs slice-09; with markers + legend elided, byte-identical to slice-09 (every weight/confidence/bonus/subtotal/total/bucket/ranking/order) | AC-SCORE-BYTEID (CARDINAL) | `@no-regression @cardinal @shown-never-applied` |
| SF-5 | Two contributions with identical confidence+bonuses render identical subtotals; only the countered one shows the marker; the legend is present; the copy never uses "disputed"/"refuted"/"false"/"penalty"/"deduction"/"lowered"/"disputed score" | AC-SCORE-ANTIMISREAD | `@anti-misread @shown-never-applied` |
| SF-6 | A contributor with NO countered contributions renders `/score` with no markers, no noise, byte-identical to the slice-09 baseline (legend elided) | AC-002-NO-NOISE, AC-SCORE-BYTEID | `@no-noise @empty-set @no-regression` |
| SF-7 | The flag renders identically under htmx fragment and no-JS full page; both offline (LOCAL read, vendored htmx asset only) | AC-002-PARITY, AC-SCORE-LOCAL | `@parity @no-js @full-page @local-offline` |
| SF-N1 | A large multi-pairing/multi-contribution breakdown flags the countered subset correctly in ONE request (N+1-flatten behavioral proxy) | AC-001-ONE-CALL, AC-001-INVARIANT | `@batch-read @no-n-plus-1` |

## GOLD invariants — `viewer_counter_flags_score_surface_invariants.rs`

| # | Invariant | AC | Invariant tag |
|---|---|---|---|
| SF-INV-ReadOnly | Every flagged `/score` render (both shapes, both postures) leaves the store read-only (`claims` + `peer_claims` row counts unchanged; Mandate 8 universe-bound) | I-CF-1 / KPI-VIEW-2 | `@read-only @property` |
| SF-INV-NoWrite | No flagged `/score` response shape adds a write/sign/counter/publish/subscribe control; every `/claims/{cid}` reference is render-only `<a href>` TEXT | I-CF-1 | `@read-only @property` |
| SF-INV-OfflineChrome | The flagged `/score` chrome references only the local `/static/htmx.min.js`, no CDN | I-CF-5 / KPI-HX-G2 | `@offline @no-cdn @property` |
| SF-INV-Offline | The flagged `/score` surface renders fully offline (LOCAL read + PURE compute, no outbound edge); the peer-countered contribution still carries its marker, the viewer re-verifies nothing | I-CF-5 / KPI-5 / AC-SCORE-LOCAL | `@offline @local-first @property` |
| SF-INV-ByteId (CARDINAL) | The `/score` render is byte-identical with and without the flag (markers + legend elided) AND the subtotals still sum to the weight on a FLAGGED render | I-CF-9 / D-14-2 / AC-SCORE-BYTEID + AC-SCORE-SUMWEIGHT | `@cardinal @shown-never-applied @cardinal-sum-to-weight @property` |
| SF-INV-N1 | A large multi-pairing breakdown resolves presence in ONE request (the contribution-CID flatten across pairings is a single presence call) | I-CF-8 / ADR-051 | `@n-plus-1-guard @property` |

## Error/edge/guardrail share

11/14 (79%) are guardrail/edge/no-regression (no-noise SF-6; presence-only SF-2; anti-misread
SF-5; sum-to-weight CARDINAL SF-3 + SF-INV-ByteId; byte-identity SF-4 + SF-INV-ByteId; N+1
SF-N1 + SF-INV-N1; read-only/no-write/offline SF-INV-ReadOnly/NoWrite/OfflineChrome/Offline)
— well above the 40% floor.

## Traceability (scenario → AC → invariant)

Per `discuss/acceptance-criteria.md` §Traceability. Every AC (AC-001-ONE-CALL/INVARIANT,
AC-002-MARKER/LINK/NO-NOISE/PARITY, AC-SCORE-SUMWEIGHT/BYTEID/ANTIMISREAD/PRESENCE/LOCAL) has
at least one covering scenario; the five LOAD-BEARING invariants (sum-to-weight,
byte-identity, presence-only, local-offline, anti-misread) each have a dedicated GOLD/story
scenario.
