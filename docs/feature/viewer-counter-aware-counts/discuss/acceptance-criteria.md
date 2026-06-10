<!-- markdownlint-disable MD024 -->
# Acceptance Criteria (BDD): viewer-counter-aware-counts (slice-18)

> Driving routes: the REAL `openlore ui` subprocess over HTTP — `GET /` (landing) and
> `GET /claims` (list). No scenario calls a `viewer-domain` render fn or a read method
> directly; the contract is the rendered surface. Scenario titles describe WHAT the operator
> learns, never HOW the system reads it. All scenarios trace to J-003b. Themes mirror the
> US-CC-001/002 UAT structure; the `@infrastructure` US-CC-000 contract is asserted THROUGH
> these surfaces (its single-aggregate-read + independent-degrade contract is a GOLD
> invariant, below).

## Theme 1 — The landing shows the countered-own-claims count beside the own-claims count (US-CC-001)

### Scenario: The front door shows how many of my own claims are countered

Given Maria's store has 12 own claims, 3 of which have ≥1 counter
When she opens `GET /` in the viewer
Then the landing summary shows "12 own claims" with "(3 countered)" beside it
And the own-claims count "12" is unchanged by the presence of the countered count

### Scenario: The `/claims` header shows the same disputed-claim awareness (US-CC-002)

Given Maria's store has 12 own claims, 3 of which have ≥1 counter
When she opens `GET /claims`
Then the list header shows the disputed-claim awareness "(3 countered)"
And it is the SAME count the landing shows beside "12 own claims"

## Theme 2 — Presence count: a twice-countered claim counts once (US-CC-000/001 — C-4 / BR-CC-1)

### Scenario: A claim countered by multiple peers counts once

Given Maria's claim `bafyMariaTDD` is countered by both Rachel and Tobias
And it is her only countered claim
When she opens `GET /`
Then the landing summary shows "(1 countered)", not "(2 countered)"
And the count shows no "disputed by N" and no sum of counters

### Scenario: Own counters and peer counters both make an own claim "countered" once

Given Maria's claim `bafyMariaRust` is countered by Tobias (a peer counter)
And her claim `bafyMariaSemver` is countered by her own later counter to it
And no other claim of hers is countered
When she opens `GET /`
Then the landing summary shows "(2 countered)"
And each countered claim contributes exactly once regardless of which ref table holds its counter

## Theme 3 — Honest zero when nothing is countered (US-CC-001/002 — C-5)

### Scenario: An honest "(0 countered)" on the landing when nothing of mine is disputed

Given Maria's store has 12 own claims, none of which has drawn a counter
When she opens `GET /`
Then the landing summary shows "12 own claims (0 countered)"
And "(0 countered)" is a successful read of zero, not a missing-number state and not an omitted count

### Scenario: An honest "(0 countered)" in the `/claims` header, list renders as slice-06

Given Maria's store has 12 own claims, none of which has drawn a counter
When she opens `GET /claims`
Then the list header shows "(0 countered)"
And the list renders its rows with no per-row Countered flags, exactly as slice-06

## Theme 4 — Missing ≠ zero: a failed count read degrades independently, no 5xx (US-CC-000/001/002 — C-2 / C-5)

### Scenario: A failed countered-count read degrades gracefully on the front door

Given Maria's countered-own-claims count read fails while the own-claims count succeeds
When she opens `GET /`
Then the own-claims count and the rest of the summary and the nav hub still render
And the countered count renders as a missing-number state (e.g. "—"), not a fabricated "(0 countered)"
And the page is a normal 200, not a 5xx and not a blanked summary

### Scenario: A failed header count degrades without blanking the `/claims` list

Given Maria's countered-own-claims count read fails
When she opens `GET /claims`
Then the list header renders the missing-number state for the countered count, not a fabricated "(0 countered)"
And the list rows still render
And the page is a normal 200, not a 5xx

## Theme 5 — The count never re-weights, re-orders, filters, or deducts (US-CC-001/002 — C-4)

### Scenario: The countered count never re-weights the own-claims count

Given Maria has 12 own claims, 3 countered
When she opens `GET /`
Then the own-claims count renders "12" exactly (the countered count is additive awareness, never a deduction)
And the front door contains no penalty, score, "refuted", or "false" language

### Scenario: The `/claims` header count does not re-order, filter, or re-weight the list

Given Maria's store has a mix of countered and un-countered claims spanning the page
When she opens `GET /claims`
Then the row order (`composed_at DESC, cid`), page boundaries, total count, and every row's confidence are byte-identical to a render without the header count
And the countered rows are not pulled to the top or grouped by the header count

## Theme 6 — Read-only / no write control / no key (US-CC-001/002 — C-1 CARDINAL)

### Scenario: The counter-aware front door exposes no write, compose, sign, subscribe, or follow control

Given Maria opens `GET /`
When she inspects the rendered page
Then it contains no form, no button, and no control to compose, sign, subscribe, or follow
And every navigation affordance is a plain link, not a mutating control
And the viewer process holds no signing key

### Scenario: The counter-aware `/claims` header adds no write control

Given Maria opens `GET /claims`
When she inspects the rendered header
Then the countered count is render-only text, not a sort, filter, or mutating control
And the route adds no write, compose, sign, subscribe, or follow affordance

## Theme 7 — LOCAL / offline (US-CC-001/002 — C-2 CARDINAL)

### Scenario: The front door countered count renders fully with the network down

Given Maria's store has countered claims and the network is unavailable
When she opens `GET /`
Then the landing summary including the countered count renders
And no outbound network request is made by the route
And the page references only the vendored local /static/htmx.min.js (no CDN)

### Scenario: The `/claims` header countered count renders offline

Given the network is unreachable and Maria's store holds countered claims
When she opens `GET /claims`
Then the header countered count renders
And no network call is made by the route

## Theme 8 — No N+1: the countered count is a fixed aggregate read (US-CC-000 — C-3)

@property
### Scenario: The countered-own-claims count is a fixed aggregate read, invariant to store size

Given Maria's store has 12 own claims, 3 countered
When she opens `GET /`
Then the countered-own-claims count resolves to 3
And it is resolved in a FIXED set of aggregate reads (the landing's read budget grows by at most one), invariant to store size — no per-claim counter-presence loop
And the number of countered-count reads does not grow with the number of claims

## Theme 9 — Anti-misread: neutral disputed-claim awareness copy (US-CC-001/002 — C-6 / BR-CC-3)

### Scenario: The countered count is neutral disputed-claim awareness, never a verdict or penalty

Given Maria's claim `bafyMariaTDD` (confidence `0.30`) is countered by both Rachel and Tobias, her only countered claim
When she opens `GET /`
Then the landing summary shows "(1 countered)" — a neutral presence count
And its confidence (when she drills in) renders `0.30` verbatim, never re-weighted by the count
And the copy is never "refuted", "false", "disputed by 2", a score, or a deduction

## GOLD invariants (release-gate — cardinal regression guards)

| Invariant | Guards | Source |
|---|---|---|
| Every landing + `/claims` render leaves the store read-only (no mutation, no write/sign control, no key) | C-1 CARDINAL | KPI-VIEW-2, slice-06–17 |
| The countered count is a single aggregate read, invariant to store size (no N+1) — the landing read budget grows by at most 1 | C-3 CARDINAL | slice-17 C-4, slice-12 I-LF-8 |
| A failed countered-count read → missing marker + the sibling counts/rows intact + 200 (never a 5xx, never a fabricated 0) | C-2 / C-5 | slice-17 WD-LD-2/WD-LD-8 |
| A claim countered N times counts ONCE; the own-claims count is never re-weighted/deducted | C-4 CARDINAL | J-003b accuracy |
| The landing "(N countered)" == the `/claims` header "(N countered)" for the same store (single source) | US-CC-002 consistency | this slice |
| The `/claims` list order/paging/count/confidence is byte-identical to the no-header-count baseline | C-4 (additive) | slice-12 I-LF-2 |
| Both surfaces render offline, referencing only the vendored htmx asset (no CDN) | C-2 | KPI-5 / KPI-HX-G2 |

> Error/edge ratio target ≥ 40%: of the scenario set, the missing≠zero degrade (×2),
> honest-zero (×2), presence-once boundary (×2), and no-regression (×1) are error/edge/
> boundary — comfortably above 40%. PBT machinery is NOT introduced; the `@property`
> no-N+1 scenario is an EXAMPLE-shaped invariant assertion (read-count invariant to store
> size), consistent with the slice-12/17 single-aggregate gold posture.
