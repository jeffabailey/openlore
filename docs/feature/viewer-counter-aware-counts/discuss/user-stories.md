<!-- markdownlint-disable MD024 -->
# User Stories: viewer-counter-aware-counts (slice-18)

> Combined file (one section per story). Brownfield DELTA on slice-17 (the `GET /` landing
> `LandingSummary`) and slice-06 (the `GET /claims` list header), reusing the slice-12
> counter-reference data (`claim_references ∪ peer_claim_references`, `ref_type='counters'`).
> Both user-visible stories trace to **J-003b** (counter-claim awareness — the orientation /
> at-a-glance-count facet, `docs/product/jobs.yaml`). The read-wiring story is
> `infrastructure-only` with rationale. The viewer is read-only, holds no key. NO new route,
> NO new crate; workspace stays 21.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: `/` and `/claims` hold `StoreReadPort` only — no
  mutation method, no signing key in the viewer process, no write/compose/sign/subscribe/
  follow control. The countered count is a COUNT only. Enforced 3 layers (type: the read
  port has no mutation method + xtask check-arch viewer-capability rule + behavioral gold).
  [KPI-VIEW-2, slice-06–17]
- **C-2 LOCAL-only / offline + graceful degrade (CARDINAL)**: the countered count is a LOCAL
  aggregate over the indexed counter-reference tables; NO network seam. `/` and `/claims`
  render fully with the network down, referencing only the vendored `/static/htmx.min.js`
  (no CDN). If the countered-count read FAILS, the surface degrades gracefully — the missing
  marker renders WITHOUT blanking the own-claims count, the other landing counts, the nav
  hub, or the `/claims` rows (never a 5xx, never blank, never a raw stack trace). [KPI-5,
  KPI-VIEW-5, NFR-VIEW-6, slice-17 WD-LD-2]
- **C-3 Cheap / no N+1 / invariant to store size (CARDINAL)**: the countered count is a SMALL
  FIXED number of aggregate reads per render — ideally ONE count-only aggregate, OR it folds
  into the existing summary resolution — invariant to store size. The landing's "3 fixed
  reads" budget grows by AT MOST 1. NO per-claim `counter_presence_for` loop. [slice-17 C-4,
  slice-12 I-LF-8]
- **C-4 Presence count, never a total / re-weight / verdict (CARDINAL — J-003b accuracy)**:
  the countered count is how many own claims have ≥1 counter — a PRESENCE count. A claim
  countered by 2 peers counts ONCE. It is NEVER a "disputed by N" total, NEVER a re-weight of
  the own-claims count (the "12" is unchanged), NEVER a verdict. [BR-CC-1]
- **C-5 Missing ≠ zero (inherited slice-17 WD-LD-8)**: the countered count is Option-shaped:
  Some(0) = honest "no claims countered" (renders "(0 countered)"), None = failed read →
  the missing marker (the slice-17 `MISSING_COUNT_MARKER` "—"). A fabricated 0 on a failed
  read is forbidden. [BR-CC-1, NFR-CC-4]
- **C-6 Anti-misread / neutral copy (inherited slice-14)**: "(N countered)" reads as NEUTRAL
  disputed-claim awareness, not a penalty/score/deduction. No penalty, deduction, "refuted",
  "false", or score language; the own-claims count stands unchanged beside it. [BR-CC-3]
- **C-7 No new crates; no new route; reuse the counter-reference data**: extend the PURE
  `viewer-domain` (`LandingSummary` gains a countered-own-claims field; `render_landing` +
  the `/claims` header render it) + EFFECT `adapter-http-viewer` (`landing_page` /
  `claims_page` resolve the countered count) + at most `ports` / `adapter-duckdb` IF DESIGN
  elects a count-only countered-own-claims aggregate. NO new `GET /` or `GET /claims` route.
  Workspace stays 21. Functional paradigm (ADR-007). [slice-06–17]

---

## US-CC-000: Resolve the countered-own-claims count in a fixed aggregate read and thread it into the landing summary + `/claims` header, degrading independently on read failure (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-CC-000 resolves the LOCAL countered-own-claims count — how many of the operator's own
claims appear as a countered `referenced_cid` in `claim_references ∪ peer_claim_references`
(`ref_type='counters'`) — in a SMALL FIXED aggregate read (ideally ONE count-only aggregate;
the exact read is the open DESIGN question WD-CC-5), and threads it into the slice-17
`LandingSummary` (an added Option-shaped countered field) and the `/claims` header
resolution, degrading to a missing-number state on read failure (never a 5xx, never blanking
the sibling counts). It produces no user-visible output on its own (the rendered
"(3 countered)" on the landing + the `/claims` header are US-CC-001/002), so it enables a
user decision only THROUGH those stories. The slice contains TWO non-infrastructure,
user-visible stories (US-CC-001, US-CC-002) with a real decision, so the slice has release
value (Dimension-0 slice-level check passes). This is READ-ONLY by construction: it adds no
mutation method, and if DESIGN elects a count-only aggregate, that variant is on
`StoreReadPort`, which declares no mutation method.

### Problem

The viewer can today answer "is THIS ONE claim countered?" (slice-11) and "which of THESE
page CIDs are countered?" (slice-12 `counter_presence_for`). It can render the landing
summary's three LOCAL counts (slice-17). But there is NO read that answers, in one cheap
aggregate, "HOW MANY of my own claims have been countered?" — the orientation count this
slice surfaces. A naive implementation could loop `counter_presence_for` per claim (N+1),
fabricate a 0 when the read fails (misleading "nothing disputed"), or 5xx / blank the whole
landing — none acceptable for the front door.

### Who

- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the
  counter-aware-orientation stories (US-CC-001/002) consume.

### Solution

Resolve the LOCAL countered-own-claims count: the number of own-claim CIDs
(`SELECT cid FROM claims`) that appear as a countered `referenced_cid` in the indexed
`claim_references ∪ peer_claim_references` (`ref_type='counters'`). A PRESENCE count — a
claim countered by N peers counts ONCE (`COUNT(DISTINCT …)` / a set intersection, never a
sum of counters). Model the count as Option-shaped (extend `LandingSummary` with a countered
field, or a parallel Option) so a FAILED read degrades to the missing marker — DISTINCT from
a successful 0 — and the per-count `.ok()` degrade (slice-17 ADR-054 D2) keeps the failure
INDEPENDENT (the own-claims count + the other landing counts + the `/claims` rows still
render). NO mutation method; NO network; NO per-claim loop.

### Domain Examples

#### 1: Happy path — 12 own claims, 3 countered, in one aggregate read

Maria's store has 12 own claims; 3 of them have drawn a counter: `bafyMariaRust` (countered
by Tobias's peer counter), `bafyMariaTDD` (countered by Rachel AND Tobias — two peer
counters), and `bafyMariaSemver` (countered by Maria's own later counter to a different
claim). The countered-own-claims count resolves to `3` (NOT 4 — `bafyMariaTDD`'s two
counters count ONCE) in a single aggregate read, invariant to store size, and threads
`Some(3)` into the landing summary + `/claims` header resolution.

#### 2: Edge case — nothing of mine has been countered (honest zero)

Maria's store has 12 own claims; none has drawn a counter. The read SUCCEEDS and returns 0.
`Some(0)` threads through — the view will say "(0 countered)", an honest "nothing of mine
has been disputed", DISTINCT from a missing-number state.

#### 3: Boundary — the countered-count read fails, the other counts survive

Maria's countered-count read fails transiently (a `StoreReadError`) while
`count_claims() = 12` succeeds. The countered field threads `None` (renders the missing
marker), but the own-claims `Some(12)` and the other landing counts still render; the route
returns 200, never a 5xx, never a blanked summary, never a raw stack trace.

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (`GET /` and `GET /claims`, port-to-port via the
> real `openlore ui` subprocess). No scenario calls a read method directly.

#### Scenario: The countered-own-claims count is one aggregate read, invariant to store size

Given Maria's store has 12 own claims, 3 of which have ≥1 counter
When she opens `GET /` in the `openlore ui` viewer
Then the countered-own-claims count resolves to 3
And it is resolved in a FIXED set of aggregate reads (the landing's read budget grows by at most one), invariant to store size — no per-claim counter-presence loop

#### Scenario: A claim countered by multiple peers counts once

Given Maria's claim `bafyMariaTDD` is countered by both Rachel and Tobias
And it is her only countered claim
When she opens `GET /`
Then the countered-own-claims count is 1, not 2
And the count is a presence count (how many own claims are countered), never a sum of counters

#### Scenario: An honest zero when nothing of mine has been countered

Given Maria has 12 own claims, none of which has drawn a counter
When she opens `GET /`
Then the countered-own-claims count is a successful read of 0
And it is distinct from a missing-number state (it is a real "(0 countered)")

#### Scenario: A failed countered-count read degrades independently without a 5xx

Given Maria's countered-own-claims count read fails while `count_claims` succeeds
When she opens `GET /`
Then the own-claims count and the other landing counts still render their numbers
And the countered count renders as a missing-number state (e.g. "—"), not a fabricated 0
And the route returns a 200 page, never a 5xx and never a blanked summary

### Acceptance Criteria

- [ ] The countered-own-claims count = the number of own-claim CIDs (`SELECT cid FROM claims`) that appear as a countered `referenced_cid` (`ref_type='counters'`) in `claim_references ∪ peer_claim_references` — a PRESENCE count (a claim countered N times counts ONCE).
- [ ] The count is resolved in a FIXED set of aggregate reads per render, invariant to store size (no N+1, no per-claim `counter_presence_for` loop); the landing's read budget grows by AT MOST 1.
- [ ] The count is Option-shaped: a successful 0 (`Some(0)`) is DISTINCT from a failed read (`None`); a failed read renders the slice-17 missing marker, never a fabricated 0.
- [ ] A failed countered-count read degrades INDEPENDENTLY — the own-claims count, the other landing counts, the nav hub, and the `/claims` rows still render; the route returns 200, never a 5xx.
- [ ] The change adds NO mutation method to `StoreReadPort` (read-only by construction); if a count-only countered aggregate is added it is a read-only method.
- [ ] The read is LOCAL only (no network seam) and runs over the SAME shared connection the CLI writes through (BR-VIEW-4).
- [ ] The countered count threads into the slice-17 `LandingSummary` (or a parallel Option) and the `/claims` header resolution from the SAME read (a single source of the number for both surfaces).

### Outcome KPIs

- **Who**: the viewer process serving `GET /` and `GET /claims`
- **Does what**: resolves the LOCAL countered-own-claims count for the orientation surfaces in a fixed aggregate read, degrading independently on failure
- **By how much**: the landing read budget grows by AT MOST 1 (ideally exactly 1 count-only aggregate), invariant to store size (0 N+1); 0 of N read failures produce a 5xx or blank the sibling counts
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (read count invariant to store size; seeded countered-count read failure → 200 with the missing marker + the other counts intact)
- **Baseline**: no read answers "how many of my own claims are countered"; only per-CID `query_counter_claims` (slice-11) and the per-page `counter_presence_for` subset (slice-12) exist

### Technical Notes

- The DATA: own claims are countered by PEERS (the self-counter rule blocks countering your own claim), so a countered own-claim = an own-claim cid (`SELECT cid FROM claims`) that appears as a countered `referenced_cid`. The counter references live in the indexed `claim_references ∪ peer_claim_references` (`ref_type='counters'`, keyed by `referenced_cid`) — `crates/adapter-duckdb/src/store_read.rs` ~735–755 (the slice-12 `counter_presence_for` SQL) is the closest precedent shape.
- **OPEN DESIGN QUESTION (WD-CC-5)**: the exact read is DESIGN's call. Two shapes both satisfy the FIXED-aggregate contract: (a) a count-only aggregate `count_countered_own_claims()` — `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')` (mirrors slice-17's `count_active_peer_subscriptions` count-only decision over `.len()`); OR (b) reuse the slice-12 `counter_presence_for(all_own_cids).len()` (zero new port surface, but materializes the own-cid list + the presence set just to count). PRODUCT contract: a SINGLE aggregate read for the countered count, invariant to store size, either way. Recommend the count-only aggregate for SYMMETRY (the landing's other counts are count-only) + CHEAPNESS (avoids materializing every own cid + the presence set), mirroring slice-17 ADR-054 D3 — but DESIGN decides.
- Thread into the slice-17 `LandingSummary` (`crates/viewer-domain/src/lib.rs` ~574) — add a `countered_own_claims: Option<usize>` field (or a parallel Option), resolved via `.ok()` (slice-17 ADR-054 D2 per-count independent degrade). The `/claims` header (`render_claims_page` ~373) reads the SAME number.
- Graceful degrade: the slice-17 `.ok()` per-count degrade + the slice-12 `counter_presence_for(...).unwrap_or_default()` precedent. DESIGN owns the exact `LandingSummary` shape.
- READ-ONLY: shares the CLI's connection (BR-VIEW-4); the trait declares no mutation method (I-VIEW-1). Reads the LOCAL indexed ref tables only — no network.

---

## US-CC-001: At the front door, see at a glance how many of my own claims have been countered

`job_id: J-003b`

### Problem

When Maria opens the viewer at `http://127.0.0.1:<port>/`, the landing summary (slice-17)
tells her HOW MUCH is in her store — "12 own claims · 7 peer claims · 2 active peers" — but
not how much of it has been DISPUTED. To learn how many of her own claims drew a counter,
she must leave the front door, open `/claims`, and scan for slice-12 flags (or drill each
claim). The orientation surface answers "what's here?" but not "what's been pushed back on?"
— and counter-claim awareness (the whole counter family, slices 11–14) is not yet connected
to the front-door orientation she starts every session at.

### Who

- P-001 (the viewer operator, "Maria"), counter-aware-orientation hat | opening the viewer
  at the start of a session | wants to know, the moment she lands, not just how much is in
  her store but how much of her own work has been disputed — without leaving the front door
  — confident the count changes nothing and never re-weights her claims.

### Solution

On `GET /`, render the countered-own-claims count BESIDE the own-claims count in the
slice-17 landing summary — e.g. "**12 own claims (3 countered)**". The countered count is a
PRESENCE count (how many own claims have ≥1 counter; a claim countered by N peers counts
once). A successful 0 renders "(0 countered)" (honest "nothing of mine disputed"); a failed
countered-count read renders the missing marker (DISTINCT from 0) while the own-claims count
and the rest of the summary still render. The copy is NEUTRAL disputed-claim awareness (not
a penalty/score); the own-claims "12" is unchanged beside it. The page stays read-only (no
key, no write control), LOCAL (renders offline), and degrades gracefully.

### Elevator Pitch

- **Before**: when Maria opens the viewer at `http://127.0.0.1:<port>/`, the landing summary shows "12 own claims · 7 peer claims · 2 active peers" — how MUCH is in her store, but not how much of it has been DISPUTED; to learn how many of her own claims drew a counter she must leave the front door and scan `/claims`.
- **After**: open `http://127.0.0.1:<port>/` → the own-claims count now carries her disputed-claim awareness inline — "12 own claims (3 countered)" — a neutral presence count (a claim countered by N peers counts once); "(0 countered)" when nothing of hers has been disputed; the missing marker if the count can't be read, with the rest of the summary intact; it loads with the network down.
- **Decision enabled**: Maria decides, the moment she lands, whether to go read the disagreements on her own claims first (drilling into the slice-12-flagged rows) or orient elsewhere — counter-claim awareness now part of her front-door orientation, not a separate scan.

### Domain Examples

#### 1: Happy path — 12 own claims, 3 countered, inline on the landing

Maria has 12 own claims; `bafyMariaRust`, `bafyMariaTDD`, and `bafyMariaSemver` each have ≥1
counter. `/` shows "12 own claims (3 countered)" beside "7 peer claims · 2 active peers" and
the full nav hub. She reads "(3 countered)" and clicks "My Claims" to drill into the flagged
rows (slice-12) and read the disagreements.

#### 2: Edge case — honest "(0 countered)" when nothing of mine is disputed

Maria has 12 own claims, none countered. `/` shows "12 own claims (0 countered)" — an honest
"nothing of mine has been disputed", NOT a missing marker and NOT a hidden/omitted count.
The "(0 countered)" reassures her, at a glance, that her store has drawn no pushback yet.

#### 3: Boundary / anti-misread — many counters, one count; neutral copy

Maria's `bafyMariaTDD` (confidence `0.30`) is countered by BOTH Rachel and Tobias; it is her
only countered claim. `/` shows "12 own claims (1 countered)" — ONE, not 2 (presence count);
the "12" is unchanged (no deduction); the confidence is untouched (the count is on the
landing, not a re-weight). The copy is "(1 countered)" — neutral, never "1 refuted",
"1 disputed by 2", or a score.

### UAT Scenarios (BDD)

> Driving route: `GET /` (the real `openlore ui` subprocess).

#### Scenario: The front door shows how many of my own claims are countered

Given Maria's store has 12 own claims, 3 of which have ≥1 counter
When she opens `GET /` in the viewer
Then the landing summary shows "12 own claims" with the disputed-claim awareness "(3 countered)" beside it
And the own-claims count "12" is unchanged by the presence of the countered count

#### Scenario: An honest zero when nothing of mine has been countered

Given Maria's store has 12 own claims, none of which has drawn a counter
When she opens `GET /`
Then the landing summary shows "12 own claims (0 countered)"
And "(0 countered)" is a successful read of zero, not a missing-number state and not an omitted count

#### Scenario: A claim countered by multiple peers counts once on the landing

Given Maria's claim `bafyMariaTDD` is countered by both Rachel and Tobias, and it is her only countered claim
When she opens `GET /`
Then the landing summary shows "(1 countered)", not "(2 countered)"
And the count shows no "disputed by N", no verdict, and no penalty or deduction language

#### Scenario: The countered count never re-weights the own-claims count

Given Maria has 12 own claims, 3 countered
When she opens `GET /`
Then the own-claims count renders "12" exactly (the countered count is additive awareness, never a deduction)
And the front door contains no penalty, score, "refuted", or "false" language

#### Scenario: A failed countered-count read degrades gracefully on the front door

Given Maria's countered-own-claims count read fails while the own-claims count succeeds
When she opens `GET /`
Then the own-claims count and the rest of the summary and the nav hub still render
And the countered count renders as a missing-number state (e.g. "—"), not a fabricated "(0 countered)"
And the page is a normal 200, not a 5xx and not a blanked summary

#### Scenario: The front door countered count renders fully with the network down

Given Maria's store has countered claims and the network is unavailable
When she opens `GET /`
Then the landing summary including the countered count renders
And no outbound network request is made by the route
And the page references only the vendored local /static/htmx.min.js (no CDN)

### Acceptance Criteria

- [ ] `/` renders the countered-own-claims count beside the own-claims count (e.g. "12 own claims (3 countered)").
- [ ] The countered count is a PRESENCE count — how many own claims have ≥1 counter; a claim countered by N peers counts ONCE (never "disputed by N", never a sum of counters).
- [ ] The own-claims count is UNCHANGED by the countered count (additive awareness, never a deduction / re-weight).
- [ ] A successful read of 0 renders "(0 countered)" (honest zero); a failed read renders the slice-17 missing marker, DISTINCT from a real 0 (never a fabricated "(0 countered)").
- [ ] A failed countered-count read degrades gracefully — the own-claims count, the rest of the summary, and the nav hub still render; the route returns 200, never a 5xx.
- [ ] The copy is NEUTRAL disputed-claim awareness — no penalty, deduction, score, "refuted", "false", or verdict language.
- [ ] The countered count renders LOCAL/offline, referencing only the vendored `/static/htmx.min.js` (no CDN); no network seam on the route.
- [ ] The route stays read-only: no form/button/mutating control, no write/compose/sign/subscribe/follow affordance, no signing key.

### Outcome KPIs

- **Who**: P-001 dogfood operators opening the viewer
- **Does what**: on opening `/`, immediately sees how many of their own claims have been disputed and decides whether to drill into the flagged rows first
- **By how much**: leading indicator OF KPI-VIEW-1 (time-to-see-store-contents — now including disputed-claim state) — a measurable share open `/` and, when the countered count is non-zero, navigate to `/claims` to read a contested claim in the same session
- **Measured by**: per-feature GREEN (the countered count renders beside the own-claims count; honest zero; presence-once; missing-not-zero degrade); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today `/` shows "12 own claims" with no disputed-claim awareness; the operator must leave the front door to learn how much of her work has drawn pushback

### Technical Notes

- Render: extend the slice-17 landing summary section (`render_landing`, `crates/viewer-domain/src/lib.rs` ~636) to render the countered count beside the own-claims count — e.g. `(render_count(summary.own_claims)) " own claims (" (render_countered(summary.countered_own_claims)) " countered)"`. DESIGN owns the exact markup + the `render_countered` (or reuse `render_count`) helper; the slice-17 `render_count` already maps `Some(n)`→n / `None`→`MISSING_COUNT_MARKER`.
- The countered count comes from US-CC-000 (the `LandingSummary` countered field). NO new read invented here.
- Anti-misread: reuse the slice-14 neutral-copy sensibility (no penalty/deduction language). The countered count is "(N countered)" — a neutral noun, never a verdict.
- DESIGN owns: the exact phrasing/markup ("(3 countered)" vs a separate line vs a parenthetical); whether the missing case renders "(— countered)" or omits the parenthetical. The PRODUCT contract is the AC.

---

## US-CC-002: In the `/claims` list header, see the same disputed-claim awareness count

`job_id: J-003b`

### Problem

When Maria drills from the front door into `GET /claims`, the list header (slice-06: "My
Claims" + the read-only notice + the slice-17 nav) shows her claims and the slice-12 per-row
"Countered" flags — but it does not summarize, at the top, HOW MANY of her claims are
countered. She can see individual flags as she scans, but the header gives her no at-a-glance
total to orient the page ("am I scanning 3 contested claims or 30?"). The disputed-claim
awareness she gets on the landing (US-CC-001) is absent the moment she lands on the list
itself — the two orientation surfaces are inconsistent.

### Who

- P-001 (the viewer operator, "Maria"), counter-aware-orientation hat | having drilled into
  `/claims` to read her contested claims | wants the list header to tell her, at a glance,
  how many of her claims are countered — the SAME disputed-claim awareness the landing gave
  her — so the page orients consistently before she scans the per-row flags.

### Solution

On `GET /claims`, render the SAME countered-own-claims count in the list header (near "My
Claims" / the read-only notice) — the count of the operator's own claims that have ≥1
counter, consistent with the landing (US-CC-001) and driven by the SAME US-CC-000 read. A
successful 0 renders the honest-zero copy; a failed read renders the missing marker (the
list rows still render). The count is a PRESENCE count (a claim countered by N peers counts
once), additive (it does NOT re-order, filter, re-page, re-count, or re-weight the list —
the slice-12 per-row flags and the slice-06 ordering are untouched), and NEUTRAL copy (not a
penalty/score). Read-only, LOCAL/offline.

### Elevator Pitch

- **Before**: when Maria lands on `http://127.0.0.1:<port>/claims`, the header shows "My Claims" and the read-only notice; she sees the slice-12 per-row "Countered" flags only as she scans — the header gives her no at-a-glance total of how many of her claims are contested.
- **After**: open `http://127.0.0.1:<port>/claims` → the list header carries the same disputed-claim awareness as the landing — how many of her claims have been countered ("(3 countered)") — a neutral presence count consistent with the front door; "(0 countered)" when none; the missing marker if the count can't be read, with the rows still rendering; the list order/paging/flags are untouched.
- **Decision enabled**: Maria orients the `/claims` page at a glance — knows whether she is scanning 3 contested claims or 30 before she starts — and the disputed-claim awareness is consistent whether she reads it on the front door or on the list itself.

### Domain Examples

#### 1: Happy path — `/claims` header shows "(3 countered)", consistent with the landing

Maria has 12 own claims, 3 countered. She opens `/claims`: the header shows "My Claims" with
"(3 countered)" — the SAME number she saw on the landing's "12 own claims (3 countered)" —
above the list whose `bafyMariaRust`, `bafyMariaTDD`, `bafyMariaSemver` rows carry the
slice-12 per-row flags. The header total and the per-row flags agree.

#### 2: Edge case — honest "(0 countered)" header, list renders as slice-06

Maria has 12 own claims, none countered. The `/claims` header shows "(0 countered)" (honest
zero), the list renders its 12 rows with NO per-row flags (slice-12), and the order/paging/
counts are byte-identical to slice-06 — the header count is additive, it changed nothing.

#### 3: Boundary — failed count, header shows the missing marker, rows still render

Maria's countered-count read fails. The `/claims` header shows the missing marker for the
countered count (DISTINCT from "(0 countered)"), but the list rows STILL render (the row
read is independent of the header count read); the page is a 200, not a 5xx. The per-row
slice-12 flags render or not per their own presence read, independent of the header total.

### UAT Scenarios (BDD)

> Driving route: `GET /claims` (the real `openlore ui` subprocess).

#### Scenario: The `/claims` header shows how many of my claims are countered

Given Maria's store has 12 own claims, 3 of which have ≥1 counter
When she opens `GET /claims`
Then the list header shows the disputed-claim awareness "(3 countered)"
And it is the SAME count the landing shows beside "12 own claims"

#### Scenario: An honest zero in the `/claims` header when nothing is countered

Given Maria's store has 12 own claims, none of which has drawn a counter
When she opens `GET /claims`
Then the list header shows "(0 countered)"
And the list renders its rows with no per-row Countered flags, exactly as slice-06

#### Scenario: The header count does not re-order, filter, or re-weight the list

Given Maria's store has a mix of countered and un-countered claims
When she opens `GET /claims`
Then the row order, page boundaries, total count, and every row's confidence are byte-identical to a render without the header count
And the countered rows are not pulled to the top or grouped by the header count

#### Scenario: A failed header count degrades without blanking the list

Given Maria's countered-own-claims count read fails
When she opens `GET /claims`
Then the list header renders the missing-number state for the countered count, not a fabricated "(0 countered)"
And the list rows still render
And the page is a normal 200, not a 5xx

### Acceptance Criteria

- [ ] `/claims` renders the countered-own-claims count in the list header — the SAME number as the landing (US-CC-001), driven by the SAME US-CC-000 read.
- [ ] The count is a PRESENCE count (a claim countered by N peers counts once); the copy is NEUTRAL (no penalty/score/verdict/"refuted"/"false").
- [ ] A successful 0 renders the honest-zero copy; a failed read renders the missing marker (DISTINCT from 0), and the list rows STILL render.
- [ ] The header count is ADDITIVE — it does NOT change the slice-06 list ordering (`composed_at DESC, cid`), page boundaries, total count, or any row's verbatim confidence; it does not re-order, group, or filter by countered state.
- [ ] The header count renders LOCAL/offline (no CDN); the route stays read-only (no write control, no key).
- [ ] A failed header-count read returns 200 (never a 5xx) and never blanks the list rows.

### Outcome KPIs

- **Who**: P-001 dogfood operators landing on `/claims`
- **Does what**: orients the list page at a glance via the header countered count, consistent with the landing
- **By how much**: 100% consistency between the landing "(N countered)" and the `/claims` header "(N countered)" for the same store (single-source-of-truth; gold test); 0 list-order/paging/confidence regression vs slice-06 (zero tolerance)
- **Measured by**: gold acceptance test (landing count == `/claims` header count for the same store; list render byte-identical to the no-header-count baseline except the header)
- **Baseline**: today `/claims` shows the slice-12 per-row flags but no at-a-glance header total of how many claims are countered

### Technical Notes

- Render: extend the `render_claims_page` header (`crates/viewer-domain/src/lib.rs` ~373–387, the "My Claims" `h1` + the read-only `p`) to render the countered count near the header. DESIGN owns the exact placement/markup (in the `h1`, a sub-line, or beside the read-only notice).
- The count is the SAME US-CC-000 number the landing uses — a single source for both surfaces (BR consistency). NO new read invented here.
- Additive / no-regression: the header count is rendered in the header ONLY; the slice-06 `list_claims` SQL (ordering/paging/count) and the slice-12 per-row presence flags are UNTOUCHED. Pinned by a gold test asserting list byte-identity (order/paging/count/confidence) vs the no-header-count baseline.
- Anti-misread: reuse the slice-14 neutral-copy sensibility. Depends on: US-CC-000 (the read), US-CC-001 (the landing render establishes the copy/shape). No new route (extends `GET /claims`). No new crate.

---

## Out of scope (explicit — restated from requirements.md)

- **Any write / compose / sign / subscribe / follow control on `/` or `/claims`** — read-only
  (C-1, CARDINAL). No key.
- **A new route** — `GET /` and `GET /claims` already exist; slice-18 extends them (C-7).
- **Rendering counter CONTENT (authors, reasons, threads) in the count** — the count is a
  number; reading WHO countered WHAT stays the existing attributed surfaces (`/claims/{cid}`
  slice-11 thread; the slice-12 per-row flags).
- **A "disputed by N" total / a re-weight / a verdict** — presence count only (C-4 / BR-CC-1).
- **Re-weighting or deducting from the own-claims count** — "(N countered)" is additive
  awareness; the "12" is unchanged (C-4).
- **Re-ordering or filtering `/claims` by countered state** — the header count is additive;
  the slice-12 per-row flags already mark individual rows (US-CC-002 AC).
- **A "(N countered)" on the PEER-claims count this slice** — own-claims countered is the
  load-bearing signal; peer-claims countered is an explicit SCOPE decision (WD-CC-7),
  recommended deferred. (Surfaced, not silently dropped.)
- **Any network seam on these routes** — the countered count is a LOCAL aggregate (C-2).
- **A per-claim `counter_presence_for` loop (N+1)** — a FIXED aggregate read (C-3).
- **Penalty / score / "refuted" / "false" copy** — neutral disputed-claim awareness (C-6).
- **A fabricated 0 when the read failed** — a failed read is the missing marker, distinct
  from a real "(0 countered)" (C-5).
- **Persisting anything; binding anything but 127.0.0.1; adding a new crate** (C-7).
