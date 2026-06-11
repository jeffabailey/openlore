<!-- markdownlint-disable MD024 -->
# Acceptance Criteria (BDD): viewer-peer-counter-aware-counts (slice-19)

> Driving routes: the REAL `openlore ui` subprocess over HTTP — `GET /` (landing) and
> `GET /peer-claims` (peer list). No scenario calls a `viewer-domain` render fn or a read
> method directly; the contract is the rendered surface. Scenario titles describe WHAT the
> operator learns, never HOW the system reads it. All scenarios trace to J-003b. Themes mirror
> the slice-18 own-claims set, applied to the PEER count; the `@infrastructure` US-PC-000
> contract is asserted THROUGH these surfaces (its single-aggregate-read + independent-degrade
> contract is a GOLD invariant, below).

## Theme 1 — The landing + `/peer-claims` header show the countered-peer-claims count (US-PC-001/002)

### Scenario: The front door shows how many of my cached peer claims are countered

Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens `GET /` in the viewer
Then the landing summary shows "4 peer claims" with "(1 countered)" beside it
And the peer-claims count "4" is unchanged by the presence of the countered count

### Scenario: The `/peer-claims` header shows the same disputed-claim awareness (US-PC-002)

Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens `GET /peer-claims`
Then the list header shows the disputed-claim awareness "(1 countered)" beside "Peer Claims"
And it is the SAME count the landing shows beside "4 peer claims"

## Theme 2 — Presence count: a multiply-countered peer claim counts once (US-PC-000/001 — C-4 / BR-PC-1)

### Scenario: A peer claim countered by both the operator and another peer counts once

Given Maria's cached peer claim `bafyTobiasRust` is countered by Maria's own counter and by Rachel's counter
And it is her only countered peer claim
When she opens `GET /`
Then the landing summary shows "(1 countered)", not "(2 countered)"
And the count shows no "disputed by N" and no sum of counters

### Scenario: A peer claim countered from either ref table contributes exactly once

Given Maria's cached peer claim `bafyTobiasRust` is countered by her own counter (in `claim_references`)
And her cached peer claim `bafyRachelSemver` is countered by another peer's counter (in `peer_claim_references`)
And no other cached peer claim of hers is countered
When she opens `GET /`
Then the landing summary shows "(2 countered)"
And each countered peer claim contributes exactly once regardless of which ref table holds its counter

## Theme 3 — Honest zero when nothing is countered (US-PC-001/002 — C-5)

### Scenario: An honest "(0 countered)" on the landing when none of my cached peer claims is disputed

Given Maria's store caches 4 peer claims, none of which has drawn a counter
When she opens `GET /`
Then the landing summary shows "4 peer claims (0 countered)"
And "(0 countered)" is a successful read of zero, not a missing-number state and not an omitted count

### Scenario: An honest "(0 countered)" in the `/peer-claims` header, list renders as slice-06/07

Given Maria's store caches 4 peer claims, none of which has drawn a counter
When she opens `GET /peer-claims`
Then the list header shows "(0 countered)"
And the list renders its rows with no per-row Countered flags, exactly as slice-06/07

## Theme 4 — Missing ≠ zero: a failed count read degrades independently, no 5xx (US-PC-000/001/002 — C-2 / C-5)

### Scenario: A failed countered-peer-count read degrades gracefully on the front door

Given Maria's countered-peer-claims count read fails while the peer-claims count succeeds
When she opens `GET /`
Then the peer-claims count, the slice-18 own line ("12 own claims (3 countered)"), the rest of the summary, and the nav hub still render
And the countered-peer count renders as a missing-number state (e.g. "—"), not a fabricated "(0 countered)"
And the page is a normal 200, not a 5xx and not a blanked summary

### Scenario: A failed header count degrades without blanking the `/peer-claims` list

Given Maria's countered-peer-claims count read fails
When she opens `GET /peer-claims`
Then the list header renders the missing-number state for the countered count, not a fabricated "(0 countered)"
And the list rows and their slice-13 per-row flags still render
And the page is a normal 200, not a 5xx

## Theme 5 — The count never re-weights, re-orders, filters, or deducts (US-PC-001/002 — C-4)

### Scenario: The countered count never re-weights the peer-claims count, and the own line is untouched

Given Maria caches 4 peer claims, 1 countered, and has 12 own claims, 3 countered
When she opens `GET /`
Then the peer-claims count renders "4" exactly (the countered count is additive awareness, never a deduction)
And the slice-18 own line still renders "12 own claims (3 countered)" unchanged
And the front door contains no penalty, score, "refuted", or "false" language

### Scenario: The `/peer-claims` header count does not re-order, filter, or re-weight the list

Given Maria's store caches a mix of countered and un-countered peer claims spanning the page
When she opens `GET /peer-claims`
Then the row order (`composed_at DESC`), page boundaries, total count, every row's confidence, and every row's peer origin are byte-identical to a render without the header count
And the countered rows are not pulled to the top or grouped by the header count

## Theme 6 — Read-only / no write control / no key (US-PC-001/002 — C-1 CARDINAL)

### Scenario: The counter-aware front door exposes no write, compose, sign, subscribe, or follow control

Given Maria opens `GET /`
When she inspects the rendered page
Then it contains no form, no button, and no control to compose, sign, subscribe, or follow
And every navigation affordance is a plain link, not a mutating control
And the viewer process holds no signing key

### Scenario: The counter-aware `/peer-claims` header adds no write control

Given Maria opens `GET /peer-claims`
When she inspects the rendered header
Then the countered count is render-only text, not a sort, filter, or mutating control
And the route adds no write, compose, sign, subscribe, or follow affordance

## Theme 7 — LOCAL / offline (US-PC-001/002 — C-2 CARDINAL)

### Scenario: The front door peer countered count renders fully with the network down

Given Maria's store caches countered peer claims and the network is unavailable
When she opens `GET /`
Then the landing summary including the peer countered count renders
And no outbound network request is made by the route
And the page references only the vendored local /static/htmx.min.js (no CDN)

### Scenario: The `/peer-claims` header countered count renders offline

Given the network is unreachable and Maria's store caches countered peer claims
When she opens `GET /peer-claims`
Then the header countered count renders
And no network call is made by the route

## Theme 8 — No N+1: the countered-peer count is a fixed aggregate read (US-PC-000 — C-3)

@property
### Scenario: The countered-peer-claims count is a fixed aggregate read, invariant to store size

Given Maria's store caches 4 peer claims, 1 countered
When she opens `GET /`
Then the countered-peer-claims count resolves to 1
And it is resolved in a FIXED set of aggregate reads (the landing's read budget grows by exactly one — a 5th count read), invariant to store size — no per-claim counter-presence loop
And the number of countered-peer-count reads does not grow with the number of peer claims

## Theme 9 — Anti-misread: neutral disputed-claim awareness copy (US-PC-001/002 — C-6 / BR-PC-3)

### Scenario: The countered-peer count is neutral disputed-claim awareness, never a verdict or penalty

Given Maria's cached peer claim `bafyTobiasRust` (Tobias's, confidence `0.40`) is countered by both Maria and Rachel, her only countered peer claim
When she opens `GET /`
Then the landing summary shows "4 peer claims (1 countered)" — a neutral presence count
And its confidence (when she drills in) renders `0.40` verbatim, never re-weighted by the count
And the copy is never "refuted", "false", "disputed by 2", a score, or a deduction

## GOLD invariants (release-gate — cardinal regression guards)

| Invariant | Guards | Source |
|---|---|---|
| Every landing + `/peer-claims` render leaves the store read-only (no mutation, no write/sign control, no key) | C-1 CARDINAL | KPI-VIEW-2, slice-06–18 |
| The countered-peer count is a single aggregate read, invariant to store size (no N+1) — the landing read budget grows by exactly 1 | C-3 CARDINAL | slice-17 C-4, slice-12 I-LF-8, slice-18 ADR-055 D1 |
| A failed countered-peer-count read → missing marker + the sibling counts/rows intact + 200 (never a 5xx, never a fabricated 0) | C-2 / C-5 | slice-17 WD-LD-2/WD-LD-8, slice-18 C-2/C-5 |
| A peer claim countered N times counts ONCE; the peer-claims count is never re-weighted/deducted | C-4 CARDINAL | J-003b accuracy |
| The landing "(N countered)" == the `/peer-claims` header "(N countered)" for the same store (single source) | US-PC-002 consistency | this slice |
| The `/peer-claims` list order/paging/count/confidence/origin is byte-identical to the no-header-count baseline | C-4 (additive) | slice-13 / slice-06-07 no-regression |
| The slice-18 own-claims countered count (landing + `/claims` header) is UNTOUCHED | BR-PC-4 | slice-18 surfaces shipped |
| Both surfaces render offline, referencing only the vendored htmx asset (no CDN) | C-2 | KPI-5 / KPI-HX-G2 |

> Error/edge ratio target ≥ 40%: of the scenario set, the missing≠zero degrade (×2),
> honest-zero (×2), presence-once boundary (×2), and no-regression (×2, incl. own-line-untouched)
> are error/edge/boundary — comfortably above 40%. PBT machinery is NOT introduced; the
> `@property` no-N+1 scenario is an EXAMPLE-shaped invariant assertion (read-count invariant to
> store size), consistent with the slice-12/17/18 single-aggregate gold posture.
