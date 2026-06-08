# Acceptance Criteria (BDD): viewer-counter-flags-score-surface (slice-14)

> BDD acceptance criteria derived from the UAT scenarios in `user-stories.md`. Every scenario
> names its DRIVING ROUTE (`GET /score?contributor=<did>`) exercised through the real
> `openlore ui` subprocess (port-to-port). No scenario calls `counter_presence_for` or
> `viewer-domain` directly. Brownfield DELTA on slices 09/11/12/13; REUSES the slice-12
> `counter_presence_for` read + the slice-13 `render_countered_link` SSOT (no new read method,
> no new render fn).

The five LOAD-BEARING invariants this slice must carry (per the brief):

| Invariant | Tag | Where asserted |
|---|---|---|
| **Sum-to-weight preserved** (subtotals still sum to the displayed pairing weight with the flag present) | `@cardinal-sum-to-weight` | AC-SCORE-SUMWEIGHT, Scenario 3 |
| **Byte-identity when markers elided** (every weight/confidence/bonus/subtotal/total/bucket/rank/order byte-identical to the slice-09 `/score` baseline) | `@no-regression` | AC-SCORE-BYTEID, Scenario 4 |
| **Presence-only / one marker per (≥1-counter) author-cid** (one neutral marker per countered contribution, never a count) | `@presence-only` | AC-SCORE-PRESENCE, Scenario 6 |
| **LOCAL / offline** (the `/score` flag read is a LOCAL indexed lookup; no network seam; renders offline) | `@local-offline` | AC-SCORE-LOCAL, Scenario 7 |
| **Anti-misread copy** (plain-language copy makes the flag unmistakably orthogonal to the score) | `@anti-misread` | AC-SCORE-ANTIMISREAD, Scenario 5 |

---

## US-CF-001 — Reuse the batch counter-presence read in the `/score` handler (`@infrastructure`)

```gherkin
@infrastructure @batch-read @no-n-plus-1
Scenario: A score breakdown resolves counter presence in one aggregate query
  Given Maria's "/score?contributor=did:plc:t0bi" renders 3 pairings totaling 9 contributions
  And 2 of those contributions are countered
  When she opens "GET /score?contributor=did:plc:t0bi" in the openlore ui viewer
  Then all 9 contribution CIDs are resolved against counters in exactly ONE aggregate query
  And the query is issued once, not once per pairing and not once per contribution
  And the returned presence set contains exactly the 2 countered CIDs
  And the displayed weights, ranking, and breakdown order are byte-identical to slice-09

@infrastructure @batch-read
Scenario: A contributor with no claims issues no presence query
  Given the DID "did:plc:nobody" has no claims in Maria's store
  When she opens "GET /score?contributor=did:plc:nobody"
  Then the NoClaims notice renders
  And counter_presence_for is not called
  And no contribution is flagged

@infrastructure @batch-read @local-offline
Scenario: An un-countered score resolves to an empty presence set with no query
  Given Maria's store has no counter claims at all
  When she opens "GET /score?contributor=did:plc:maria"
  Then counter_presence_for returns an empty set without preparing a query
  And no contribution row is flagged
  And the breakdown renders exactly as slice-09
```

### Derived AC (US-CF-001)
- **AC-001-ONE-CALL**: the `/score` handler calls `counter_presence_for` exactly ONCE per render (or 0 for `Form`/`NoClaims`/empty), with all contribution CIDs across all pairings flattened into ONE call.
- **AC-001-INVARIANT**: the presence-query count is invariant to contribution / pairing count (the N+1 guard, inherited slice-12 ADR-048).
- **AC-001-UNCHANGED-READ**: `query_contributor_scoring_feed`, the slice-04 pure scoring math, the `WeightedView`, the ranking, and the displayed weights/subtotals are UNCHANGED.
- **AC-001-NO-NEW-METHOD**: no new method added to `StoreReadPort`; no new SQL; no new route.

---

## US-CF-002 — "Countered" flag on `/score` contribution rows (score-orthogonal)

```gherkin
@flag @reuse-render @presence-only
Scenario 1: A countered contribution shows the neutral marker linking to its thread
  Given Maria's "/score?contributor=did:plc:t0bi" breakdown has, under the cargo→dependency-pinning pairing (weight 1.42), a contribution "bafy...t0bi" (confidence 0.88, author bonus 0.10, triangulation bonus 0.05, subtotal 1.03)
  And that claim has ≥1 counter
  When she opens "GET /score?contributor=did:plc:t0bi" in the viewer
  Then that contribution row shows the neutral "Countered" marker
  And the marker is a render-only <a href="/claims/bafy...t0bi"> one-hop link to that claim's slice-11 thread
  And the marker is rendered via the REUSED slice-13 render_countered_link (no new string, no new render fn)
  And the row still shows confidence 0.88, author bonus 0.10, triangulation bonus 0.05, and subtotal 1.03

@flag @no-noise
Scenario 2: An un-countered contribution shows nothing
  Given in the same breakdown Maria's own contribution "bafy...mr1" (confidence 0.91, subtotal 0.39) has no counter
  When she opens "GET /score?contributor=did:plc:t0bi"
  Then that contribution row renders exactly as slice-09 — no marker, no "0 counters" noise

@cardinal-sum-to-weight
Scenario 3 (AC-SCORE-SUMWEIGHT): The per-claim subtotals still sum to the pairing weight with the flag present
  Given Maria's cargo→dependency-pinning pairing (weight 1.42) has contributions with subtotals 1.03 and 0.39, and the 1.03 contribution is countered
  When she opens "GET /score?contributor=did:plc:t0bi"
  Then the pairing weight is still 1.42
  And the displayed subtotals still sum to the displayed weight (1.03 + 0.39 = 1.42)
  And the countered contribution's subtotal is its FULL original 1.03 (the counter subtracts nothing)

@no-regression
Scenario 4 (AC-SCORE-BYTEID): Adding the flag changes no weight, ranking, or row order versus the slice-09 baseline
  Given Maria's "/score?contributor=did:plc:t0bi" renders 3 pairings, 2 of whose contributions are countered
  When she opens "GET /score?contributor=did:plc:t0bi"
  Then exactly the 2 countered contributions show the marker and every other contribution renders exactly as slice-09
  And every displayed weight, confidence, author bonus, triangulation bonus, subtotal, headline total, bucket, the pairing ranking, and the contribution row order are byte-identical to the slice-09 render with the markers and the anti-misread legend elided

@anti-misread
Scenario 5 (AC-SCORE-ANTIMISREAD): The flag is shown for the reader to judge and is unmistakably orthogonal to the score
  Given two contributions in the same pairing have identical confidence and bonuses, but one is countered and one is not
  When Maria opens "GET /score?contributor=<did>"
  Then both contributions show the identical subtotal (the counter subtracts nothing)
  And only the countered one shows the "Countered" marker
  And the breakdown carries plain-language copy stating the counter is shown for the reader to judge and does NOT lower the contributor's score
  And the copy never uses "disputed", "refuted", "false", "penalty", "deduction", "lowered", or "disputed score"

@presence-only
Scenario 6 (AC-SCORE-PRESENCE): A contribution countered by two authors shows one neutral marker
  Given Maria's "/score" breakdown has a contribution "bafy...dup" countered by two distinct authors
  When she opens "GET /score?contributor=<did>"
  Then that contribution row shows exactly ONE neutral "Countered" marker (never "disputed by 2")
  And the marker links to "/claims/bafy...dup" where the two counters are individually attributed
  And its subtotal, the pairing weight, and the pairing's rank are unchanged

@local-offline @parity
Scenario 7 (AC-SCORE-LOCAL): The score flag reads LOCALLY, renders offline, and is identical under htmx and no-JS
  Given Maria's "/score" breakdown has one countered contribution and the network is down
  When she requests "GET /score?contributor=<did>" WITH HX-Request and again WITHOUT it
  Then both responses render fully with no network access (the presence read is a LOCAL indexed lookup; the page references only the vendored local /static/htmx.min.js)
  And the htmx response is the score fragment with the flag
  And the no-JS response is the full page = chrome + the SAME fragment, with the flag rendered identically
```

### Derived AC (US-CF-002)
- **AC-002-MARKER**: a contribution row whose CID is in the presence set shows the `COUNTERED_PRESENCE_FLAG` marker via the REUSED slice-13 `render_countered_link(cid, is_countered)`.
- **AC-002-LINK**: the marker is a render-only `<a href="/claims/{cid}">` one-hop link to the slice-11 thread.
- **AC-SCORE-SUMWEIGHT** `@cardinal-sum-to-weight`: with the flag present, the per-claim subtotals STILL sum to the displayed pairing weight, exactly as slice-09.
- **AC-SCORE-BYTEID** `@no-regression`: every weight/confidence/bonus/subtotal/headline-total/bucket + the pairing ranking + the contribution row order are byte-identical to the slice-09 render with markers + legend elided; a countered claim contributes its FULL original weight (shown, never applied).
- **AC-SCORE-ANTIMISREAD** `@anti-misread`: the breakdown carries plain-language copy making it unmistakable the marker is orthogonal to the score (shown for the reader to judge; not a deduction); the copy never implies subtraction/penalty/verdict.
- **AC-SCORE-PRESENCE** `@presence-only`: a contribution countered by N authors shows exactly ONE neutral marker (presence-only); the marker is neutral presence text, never a verdict or count.
- **AC-SCORE-LOCAL** `@local-offline`: the `/score` flag read is a LOCAL indexed lookup; no network seam on the route; the page renders fully offline referencing only the vendored htmx asset.
- **AC-002-PARITY**: the flag renders identically under the htmx fragment and the no-JS full page (parity by construction — same fragment fn).
- **AC-002-NO-NOISE**: an un-countered contribution renders exactly as slice-09 (no marker, no noise).

---

## Traceability (scenario → AC → invariant)

| Scenario | AC | Invariant tag | Story |
|---|---|---|---|
| US-CF-001 #1 (one aggregate query) | AC-001-ONE-CALL, AC-001-INVARIANT | `@no-n-plus-1` | US-CF-001 |
| US-CF-001 #2 (NoClaims → no query) | AC-001-ONE-CALL | `@batch-read` | US-CF-001 |
| US-CF-001 #3 (empty → no query) | AC-001-INVARIANT | `@local-offline` | US-CF-001 |
| US-CF-002 Scenario 1 | AC-002-MARKER, AC-002-LINK | `@flag @reuse-render` | US-CF-002 |
| US-CF-002 Scenario 2 | AC-002-NO-NOISE | `@no-noise` | US-CF-002 |
| US-CF-002 Scenario 3 | AC-SCORE-SUMWEIGHT | `@cardinal-sum-to-weight` | US-CF-002 |
| US-CF-002 Scenario 4 | AC-SCORE-BYTEID | `@no-regression` | US-CF-002 |
| US-CF-002 Scenario 5 | AC-SCORE-ANTIMISREAD | `@anti-misread` | US-CF-002 |
| US-CF-002 Scenario 6 | AC-SCORE-PRESENCE | `@presence-only` | US-CF-002 |
| US-CF-002 Scenario 7 | AC-SCORE-LOCAL, AC-002-PARITY | `@local-offline @parity` | US-CF-002 |

All AC are observable user outcomes (a marker shown / its link target; subtotals summing to the
weight; byte-identity vs the slice-09 baseline; plain-language copy; one query) — none prescribe an
implementation. DESIGN owns the projection shape; the product contract is these AC.
