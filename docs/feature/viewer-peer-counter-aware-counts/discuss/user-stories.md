<!-- markdownlint-disable MD024 -->
# User Stories: viewer-peer-counter-aware-counts (slice-19)

> Combined file (one section per story). Brownfield DELTA on slice-17 (the `GET /` landing
> `LandingSummary` peer-claims line) and slice-06 (the `GET /peer-claims` list header),
> reusing the slice-12 counter-reference data (`claim_references ∪ peer_claim_references`,
> `ref_type='counters'`) and the slice-18 `render_countered` helper + count-only-aggregate
> pattern. Both user-visible stories trace to **J-003b** (counter-claim awareness — the
> orientation / at-a-glance-count facet, `docs/product/jobs.yaml`). The read-wiring story is
> `infrastructure-only` with rationale. The viewer is read-only, holds no key. NO new route,
> NO new crate; workspace stays 21. This is the own+peer COMPLETION of slice-18 — JUST the
> peer count (the own count shipped in slice-18); no third dimension.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: `/` and `/peer-claims` hold `StoreReadPort` only — no
  mutation method, no signing key in the viewer process, no write/compose/sign/subscribe/
  follow control. The countered-peer count is a COUNT only. Enforced 3 layers (type: the read
  port has no mutation method + xtask check-arch viewer-capability rule + behavioral gold).
  [KPI-VIEW-2, slice-06–18]
- **C-2 LOCAL-only / offline + graceful degrade (CARDINAL)**: the countered-peer count is a
  LOCAL aggregate over the indexed counter-reference tables; NO network seam. `/` and
  `/peer-claims` render fully with the network down, referencing only the vendored
  `/static/htmx.min.js` (no CDN). If the countered-peer-count read FAILS, the surface degrades
  gracefully — the missing marker renders WITHOUT blanking the peer-claims count, the other
  landing counts, the nav hub, or the `/peer-claims` rows + slice-13 per-row flags (never a
  5xx, never blank, never a raw stack trace). [KPI-5, KPI-VIEW-5, NFR-VIEW-6, slice-17 WD-LD-2,
  slice-18 C-2]
- **C-3 Cheap / no N+1 / invariant to store size (CARDINAL)**: the countered-peer count is a
  SMALL FIXED number of aggregate reads per render — ideally ONE count-only aggregate (a 5th
  sibling of `count_claims` / `count_peer_claims` / `count_active_peer_subscriptions` /
  `count_countered_own_claims`) — invariant to store size. The landing's "4 fixed reads"
  budget grows by EXACTLY 1 (a 5th count read); the `/peer-claims` header read grows by 1. NO
  per-claim `counter_presence_for` loop. [slice-17 C-4, slice-12 I-LF-8, slice-18 ADR-055 D1]
- **C-4 Presence count, never a total / re-weight / verdict (CARDINAL — J-003b accuracy)**:
  the countered-peer count is how many peer claims have ≥1 counter — a PRESENCE count. A peer
  claim countered by 2 counterers counts ONCE. It is NEVER a "disputed by N" total, NEVER a
  re-weight of the peer-claims count (the "4" is unchanged), NEVER a verdict. [BR-PC-1]
- **C-5 Missing ≠ zero (inherited slice-17 WD-LD-8 / slice-18 C-5)**: the countered-peer count
  is Option-shaped: Some(0) = honest "no peer claims countered" (renders "(0 countered)"),
  None = failed read → the missing marker (the slice-17 `MISSING_COUNT_MARKER` "—"). A
  fabricated 0 on a failed read is forbidden. [BR-PC-1, NFR-PC-4]
- **C-6 Anti-misread / neutral copy (inherited slice-14 / slice-18 C-6)**: "(N countered)"
  reads as NEUTRAL disputed-claim awareness, not a penalty/score/deduction. No penalty,
  deduction, "refuted", "false", or score language; the peer-claims count stands unchanged
  beside it. Reuses the SAME pure `render_countered(Option<usize>)` helper (single SSOT copy
  site — NO new render helper). [BR-PC-3]
- **C-7 No new crates; no new route; reuse the counter-reference data + the slice-18 helper**:
  extend the PURE `viewer-domain` (`LandingSummary` gains a 5th `countered_peer_claims` field;
  `render_landing` renders it on the peer line; `render_peer_claims_page` takes the bare
  `Option<usize>` for its header — all via the EXISTING `render_countered` helper) + EFFECT
  `adapter-http-viewer` (`landing_page` / `peer_claims_page` resolve the countered-peer count)
  + at most `ports` / `adapter-duckdb` IF DESIGN elects a count-only countered-peer aggregate.
  NO new `GET /` or `GET /peer-claims` route. Workspace stays 21. Functional paradigm
  (ADR-007). [slice-06–18]

---

## US-PC-000: Resolve the countered-peer-claims count in a fixed aggregate read and thread it into the landing summary + `/peer-claims` header, degrading independently on read failure (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-PC-000 resolves the LOCAL countered-peer-claims count — how many of the operator's cached
peer claims appear as a countered `referenced_cid` in `claim_references ∪ peer_claim_references`
(`ref_type='counters'`) — in a SMALL FIXED aggregate read (ideally ONE count-only aggregate, a
5th sibling of the four existing count reads; the exact read is the open DESIGN question
WD-PC-5, expected to mirror slice-18's `count_countered_own_claims` with the outer table swapped
to `peer_claims`), and threads it into the slice-17 `LandingSummary` (an added 5th Option-shaped
countered-peer field) and the `/peer-claims` header resolution, degrading to a missing-number
state on read failure (never a 5xx, never blanking the sibling counts/rows). It produces no
user-visible output on its own (the rendered "(1 countered)" on the landing peer line + the
`/peer-claims` header are US-PC-001/002), so it enables a user decision only THROUGH those
stories. The slice contains TWO non-infrastructure, user-visible stories (US-PC-001, US-PC-002)
with a real decision, so the slice has release value (Dimension-0 slice-level check passes).
This is READ-ONLY by construction: it adds no mutation method, and if DESIGN elects a count-only
aggregate, that variant is on `StoreReadPort`, which declares no mutation method.

### Problem

slice-18 added a read that answers "how many of my OWN claims have been countered?"
(`count_countered_own_claims`, outer table `claims`). But there is NO read that answers the
symmetric question for cached PEER claims: "how many of MY CACHED PEER claims have been
countered?" — by the operator's own counter (in `claim_references`) OR by another peer's
counter (in `peer_claim_references`). Without it, the landing's peer line ("4 peer claims") and
the `/peer-claims` header carry no disputed-claim awareness, even though the slice-13 per-row
flags already mark which individual peer rows are countered. A naive implementation could loop
`counter_presence_for` per peer claim (N+1), fabricate a 0 when the read fails (misleading
"nothing disputed"), or 5xx / blank the whole landing — none acceptable for the front door.

### Who

- P-001 (the viewer operator, "Maria") — indirectly; this story is the plumbing the
  counter-aware-orientation stories (US-PC-001/002) consume.

### Solution

Resolve the LOCAL countered-peer-claims count: the number of peer-claim CIDs (`SELECT cid FROM
peer_claims`) that appear as a countered `referenced_cid` in the indexed `claim_references ∪
peer_claim_references` (`ref_type='counters'`). A PRESENCE count — a peer claim countered by N
counterers counts ONCE (`COUNT(DISTINCT …)` over a de-duped `UNION` IN-set, never a sum of
counters). This is the EXACT mirror of slice-18's `count_countered_own_claims`, with the outer
table swapped from `claims` to `peer_claims` (a countered OWN claim is excluded, not filtered —
own-vs-peer is by outer-table shape). Model the count as a 5th additive Option-shaped field on
`LandingSummary` so a FAILED read degrades to the missing marker — DISTINCT from a successful 0
— and the per-count `.ok()` degrade (slice-17 ADR-054 D2 / slice-18 ADR-055 D4) keeps the
failure INDEPENDENT (the peer-claims count + the other landing counts + the `/peer-claims` rows
still render). NO mutation method; NO network; NO per-claim loop.

### Domain Examples

#### 1: Happy path — 4 peer claims, 1 countered, in one aggregate read

Maria's store caches 4 peer claims: `bafyTobiasRust`, `bafyTobiasTDD`, `bafyRachelSemver`, and
`bafyRachelDDD`. ONE of them — `bafyTobiasRust` — has drawn a counter: countered by Maria's OWN
counter (which lands in `claim_references`) AND, separately, by Rachel's counter (which lands in
`peer_claim_references` — "Rachel counters Tobias's peer claim"). The countered-peer count
resolves to `1` (NOT 2 — `bafyTobiasRust`'s two counters count ONCE) in a single aggregate read,
invariant to store size, and threads `Some(1)` into the landing summary + `/peer-claims` header
resolution.

#### 2: Edge case — none of my cached peer claims has been countered (honest zero)

Maria's store caches 4 peer claims; none has drawn a counter. The read SUCCEEDS and returns 0.
`Some(0)` threads through — the view will say "(0 countered)", an honest "none of my cached peer
claims has been disputed", DISTINCT from a missing-number state.

#### 3: Boundary — the countered-peer-count read fails, the other counts survive

Maria's countered-peer-count read fails transiently (a `StoreReadError`) while
`count_peer_claims() = 4` succeeds. The countered-peer field threads `None` (renders the missing
marker), but the peer-claims `Some(4)`, the own-claims count, the slice-18 countered-own count,
and the active-peer count still render; the route returns 200, never a 5xx, never a blanked
summary, never a raw stack trace.

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (`GET /` and `GET /peer-claims`, port-to-port via the
> real `openlore ui` subprocess). No scenario calls a read method directly.

#### Scenario: The countered-peer-claims count is one aggregate read, invariant to store size

Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens `GET /` in the `openlore ui` viewer
Then the countered-peer-claims count resolves to 1
And it is resolved in a FIXED set of aggregate reads (the landing's read budget grows by exactly one), invariant to store size — no per-claim counter-presence loop

#### Scenario: A peer claim countered by both the operator and another peer counts once

Given Maria's cached peer claim `bafyTobiasRust` is countered by Maria's own counter and by Rachel's counter
And it is her only countered peer claim
When she opens `GET /`
Then the countered-peer-claims count is 1, not 2
And the count is a presence count (how many peer claims are countered), never a sum of counters, regardless of which ref table holds each counter

#### Scenario: An honest zero when none of my cached peer claims has been countered

Given Maria caches 4 peer claims, none of which has drawn a counter
When she opens `GET /`
Then the countered-peer-claims count is a successful read of 0
And it is distinct from a missing-number state (it is a real "(0 countered)")

#### Scenario: A failed countered-peer-count read degrades independently without a 5xx

Given Maria's countered-peer-claims count read fails while `count_peer_claims` succeeds
When she opens `GET /`
Then the peer-claims count and the other landing counts (including the slice-18 countered-own count) still render their numbers
And the countered-peer count renders as a missing-number state (e.g. "—"), not a fabricated 0
And the route returns a 200 page, never a 5xx and never a blanked summary

### Acceptance Criteria

- [ ] The countered-peer-claims count = the number of peer-claim CIDs (`SELECT cid FROM peer_claims`) that appear as a countered `referenced_cid` (`ref_type='counters'`) in `claim_references ∪ peer_claim_references` — a PRESENCE count (a peer claim countered N times counts ONCE).
- [ ] The count is resolved in a FIXED set of aggregate reads per render, invariant to store size (no N+1, no per-claim `counter_presence_for` loop); the landing's read budget grows by EXACTLY 1 (a 5th count read).
- [ ] The count is Option-shaped: a successful 0 (`Some(0)`) is DISTINCT from a failed read (`None`); a failed read renders the slice-17 missing marker, never a fabricated 0.
- [ ] A failed countered-peer-count read degrades INDEPENDENTLY — the peer-claims count, the other landing counts (incl. the slice-18 countered-own count), the nav hub, and the `/peer-claims` rows + slice-13 per-row flags still render; the route returns 200, never a 5xx.
- [ ] The change adds NO mutation method to `StoreReadPort` (read-only by construction); if a count-only countered-peer aggregate is added it is a read-only method.
- [ ] The read is LOCAL only (no network seam) and runs over the SAME shared connection the CLI writes through (BR-VIEW-4).
- [ ] The countered-peer count threads into the slice-17 `LandingSummary` (a 5th Option field) and the `/peer-claims` header resolution from the SAME read (a single source of the number for both surfaces).
- [ ] The slice-18 countered-OWN count is UNTOUCHED — own-vs-peer is by outer-table shape (`claims` vs `peer_claims`); this read is the peer mirror, not a re-touch.

### Outcome KPIs

- **Who**: the viewer process serving `GET /` and `GET /peer-claims`
- **Does what**: resolves the LOCAL countered-peer-claims count for the orientation surfaces in a fixed aggregate read, degrading independently on failure
- **By how much**: the landing read budget grows by EXACTLY 1 (a 5th count-only aggregate), invariant to store size (0 N+1); 0 of N read failures produce a 5xx or blank the sibling counts/rows
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (read count invariant to store size; seeded countered-peer-count read failure → 200 with the missing marker + the other counts + the `/peer-claims` rows intact)
- **Baseline**: no read answers "how many of my cached peer claims are countered"; slice-18 answers only the OWN-claims version (`count_countered_own_claims`)

### Technical Notes

- The DATA: a cached peer claim is countered by the OPERATOR (her counter lands in
  `claim_references`, `ref_type='counters'`) OR by ANOTHER PEER (their counter lands in
  `peer_claim_references` — slice-11 "Rachel counters Tobias's peer claim"). So a countered
  peer-claim = a peer-claim cid (`SELECT cid FROM peer_claims`) that appears as a countered
  `referenced_cid` in EITHER table. The counter references live in the indexed
  `claim_references ∪ peer_claim_references` (`ref_type='counters'`, keyed by `referenced_cid`).
- **OPEN DESIGN QUESTION (WD-PC-5)**: the exact read is DESIGN's call — but it is the EXACT
  MIRROR of slice-18's `count_countered_own_claims` (`crates/adapter-duckdb/src/store_read.rs`
  ~517) with the OUTER table swapped from `claims` to `peer_claims`:
  `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM
  claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
  peer_claim_references WHERE ref_type='counters')`. The de-duped `UNION` IN-set + `COUNT(DISTINCT)`
  make it a presence count (countered N times → counts once, no JOIN-fanout); the outer
  `peer_claims` table makes it peer-only by query shape; parameter-free → injection-safe. PRODUCT
  contract: a SINGLE aggregate read for the countered-peer count, invariant to store size.
  Recommend the count-only aggregate (a 5th `StoreReadPort` sibling) for SYMMETRY (the landing's
  other four counts are count-only) + CHEAPNESS — the natural choice per the brief; but DESIGN
  decides (an alternative is reusing `counter_presence_for(all_peer_cids).len()`, which
  materializes the peer-cid list + presence set just to count).
- Thread into the slice-17 `LandingSummary` (`crates/viewer-domain/src/lib.rs` ~584) — add a 5th
  `countered_peer_claims: Option<usize>` field, resolved via `.ok()` (slice-17 ADR-054 D2 /
  slice-18 ADR-055 D4 per-count independent degrade). The `/peer-claims` header
  (`render_peer_claims_page` ~1164) takes the bare `Option<usize>` as a param (the SAME number).
- Graceful degrade: the slice-17 `.ok()` per-count degrade + the slice-18 fault-seam pattern
  (`#[cfg(debug_assertions)]`-gated `OPENLORE_VIEWER_FAIL_*` seam, e.g.
  `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`). DESIGN owns the exact `LandingSummary` shape.
- READ-ONLY: shares the CLI's connection (BR-VIEW-4); the trait declares no mutation method
  (I-VIEW-1). Reads the LOCAL indexed ref tables only — no network.

---

## US-PC-001: At the front door, see at a glance how many of my cached peer claims have been countered

`job_id: J-003b`

### Problem

When Maria opens the viewer at `http://127.0.0.1:<port>/`, the landing summary now shows her
own disputed-claim awareness (slice-18: "12 own claims (3 countered)") — but the peer-claims
line beside it is still bare: "4 peer claims", with no indication of how many of those cached
peer claims have been disputed. To learn how many of her cached peer claims drew a counter, she
must leave the front door, open `/peer-claims`, and scan for slice-13 flags (or drill each
claim). Counter-aware orientation is HALF-complete: the front door tells her how much of HER OWN
work has been pushed back on, but not how much of the PEER material she has cached is contested.

### Who

- P-001 (the viewer operator, "Maria"), counter-aware-orientation hat | opening the viewer at
  the start of a session | wants to know, the moment she lands, not just how many peer claims
  she has cached but how many of THOSE have been disputed — completing the counter-aware
  orientation across both own and peer claims — without leaving the front door, confident the
  count changes nothing and never re-weights the peer claims.

### Solution

On `GET /`, render the countered-peer-claims count BESIDE the peer-claims count in the slice-17
landing summary — e.g. "**4 peer claims (1 countered)**" — the EXACT mirror of slice-18's own
line ("12 own claims (3 countered)"), via the SAME pure `render_countered` helper. The countered
count is a PRESENCE count (how many cached peer claims have ≥1 counter; a peer claim countered by
N counterers counts once). A successful 0 renders "(0 countered)" (honest "none of my cached peer
claims disputed"); a failed countered-peer-count read renders the missing marker (DISTINCT from
0) while the peer-claims count and the rest of the summary still render. The copy is NEUTRAL
disputed-claim awareness (not a penalty/score); the peer-claims "4" is unchanged beside it. The
page stays read-only (no key, no write control), LOCAL (renders offline), and degrades
gracefully.

### Elevator Pitch

- **Before**: when Maria opens the viewer at `http://127.0.0.1:<port>/`, the landing summary
  shows "12 own claims (3 countered)" (slice-18) beside a bare "4 peer claims" — her OWN
  disputed-claim awareness is at the front door, but the peer line carries none; to learn how
  many of her cached peer claims drew a counter she must leave the front door and scan
  `/peer-claims`.
- **After**: open `http://127.0.0.1:<port>/` → the peer-claims count now carries its
  disputed-claim awareness inline — "4 peer claims (1 countered)" — a neutral presence count (a
  peer claim countered by N counterers counts once); "(0 countered)" when none of her cached
  peer claims has been disputed; the missing marker if the count can't be read, with the rest of
  the summary intact; it loads with the network down. Counter-aware orientation is now COMPLETE
  across own + peer claims.
- **Decision enabled**: Maria decides, the moment she lands, whether to go read the disagreements
  on her cached peer claims first (drilling into the slice-13-flagged rows on `/peer-claims`) or
  orient elsewhere — peer-claim disputed-awareness now part of her front-door orientation, not a
  separate scan, completing the picture slice-18 started for her own claims.

### Domain Examples

#### 1: Happy path — 4 peer claims, 1 countered, inline on the landing

Maria caches 4 peer claims; `bafyTobiasRust` has ≥1 counter. `/` shows "4 peer claims (1
countered)" beside "12 own claims (3 countered)" (slice-18, unchanged) and "2 active peers" and
the full nav hub. She reads "(1 countered)" and clicks "Peer Claims" to drill into the flagged
row (slice-13) and read the disagreement.

#### 2: Edge case — honest "(0 countered)" when none of my cached peer claims is disputed

Maria caches 4 peer claims, none countered. `/` shows "4 peer claims (0 countered)" — an honest
"none of my cached peer claims has been disputed", NOT a missing marker and NOT a hidden/omitted
count. The "(0 countered)" reassures her, at a glance, that none of the peer material she has
cached has drawn pushback yet.

#### 3: Boundary / anti-misread — multiple counterers, one count; neutral copy

Maria's cached peer claim `bafyTobiasRust` (Tobias's, confidence `0.40`) is countered by BOTH
Maria's own counter AND Rachel's counter; it is her only countered peer claim. `/` shows "4 peer
claims (1 countered)" — ONE, not 2 (presence count); the "4" is unchanged (no deduction); the
peer claim's confidence is untouched (the count is on the landing, not a re-weight). The copy is
"(1 countered)" — neutral, never "1 refuted", "1 disputed by 2", or a score.

### UAT Scenarios (BDD)

> Driving route: `GET /` (the real `openlore ui` subprocess).

#### Scenario: The front door shows how many of my cached peer claims are countered

Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens `GET /` in the viewer
Then the landing summary shows "4 peer claims" with the disputed-claim awareness "(1 countered)" beside it
And the peer-claims count "4" is unchanged by the presence of the countered count

#### Scenario: An honest zero when none of my cached peer claims has been countered

Given Maria's store caches 4 peer claims, none of which has drawn a counter
When she opens `GET /`
Then the landing summary shows "4 peer claims (0 countered)"
And "(0 countered)" is a successful read of zero, not a missing-number state and not an omitted count

#### Scenario: A peer claim countered by multiple counterers counts once on the landing

Given Maria's cached peer claim `bafyTobiasRust` is countered by both Maria and Rachel, and it is her only countered peer claim
When she opens `GET /`
Then the landing summary shows "4 peer claims (1 countered)", not "(2 countered)"
And the count shows no "disputed by N", no verdict, and no penalty or deduction language

#### Scenario: The countered count never re-weights the peer-claims count, and the own line is untouched

Given Maria caches 4 peer claims, 1 countered, and has 12 own claims, 3 countered
When she opens `GET /`
Then the peer-claims count renders "4" exactly (the countered count is additive awareness, never a deduction)
And the slice-18 own line still renders "12 own claims (3 countered)" unchanged
And the front door contains no penalty, score, "refuted", or "false" language

#### Scenario: A failed countered-peer-count read degrades gracefully on the front door

Given Maria's countered-peer-claims count read fails while the peer-claims count succeeds
When she opens `GET /`
Then the peer-claims count and the rest of the summary (including the slice-18 own line) and the nav hub still render
And the countered-peer count renders as a missing-number state (e.g. "—"), not a fabricated "(0 countered)"
And the page is a normal 200, not a 5xx and not a blanked summary

#### Scenario: The front door peer countered count renders fully with the network down

Given Maria's store caches countered peer claims and the network is unavailable
When she opens `GET /`
Then the landing summary including the peer countered count renders
And no outbound network request is made by the route
And the page references only the vendored local /static/htmx.min.js (no CDN)

### Acceptance Criteria

- [ ] `/` renders the countered-peer-claims count beside the peer-claims count (e.g. "4 peer claims (1 countered)").
- [ ] The countered count is a PRESENCE count — how many cached peer claims have ≥1 counter; a peer claim countered by N counterers counts ONCE (never "disputed by N", never a sum of counters).
- [ ] The peer-claims count is UNCHANGED by the countered count (additive awareness, never a deduction / re-weight); the slice-18 own line is UNTOUCHED.
- [ ] A successful read of 0 renders "(0 countered)" (honest zero); a failed read renders the slice-17 missing marker, DISTINCT from a real 0 (never a fabricated "(0 countered)").
- [ ] A failed countered-peer-count read degrades gracefully — the peer-claims count, the rest of the summary, and the nav hub still render; the route returns 200, never a 5xx.
- [ ] The copy is NEUTRAL disputed-claim awareness — no penalty, deduction, score, "refuted", "false", or verdict language; rendered via the SAME `render_countered` helper slice-18 established.
- [ ] The countered count renders LOCAL/offline, referencing only the vendored `/static/htmx.min.js` (no CDN); no network seam on the route.
- [ ] The route stays read-only: no form/button/mutating control, no write/compose/sign/subscribe/follow affordance, no signing key.

### Outcome KPIs

- **Who**: P-001 dogfood operators opening the viewer
- **Does what**: on opening `/`, immediately sees how many of their cached peer claims have been disputed (completing the own+peer counter-aware orientation) and decides whether to drill into the flagged peer rows first
- **By how much**: leading indicator OF KPI-VIEW-1 (time-to-see-store-contents — now including disputed-state across BOTH own AND peer claims) — a measurable share open `/` and, when the peer countered count is non-zero, navigate to `/peer-claims` to read a contested peer claim in the same session
- **Measured by**: per-feature GREEN (the peer countered count renders beside the peer-claims count; honest zero; presence-once; missing-not-zero degrade; own line untouched); cohort via the inherited opt-in telemetry endpoint (ADR-010)
- **Baseline**: today `/` shows "4 peer claims" with no disputed-claim awareness on the peer line (slice-18 added it only to the own line); the operator must leave the front door to learn how much of her cached peer material is contested

### Technical Notes

- Render: extend the slice-17 landing summary peer line (`render_landing`,
  `crates/viewer-domain/src/lib.rs` ~677: `p { (render_count(summary.peer_claims)) " peer claims" }`)
  to render the countered count beside it — e.g. `(render_count(summary.peer_claims)) " peer claims " (render_countered(summary.countered_peer_claims))` —
  the EXACT mirror of the slice-18 own line (~674–676). REUSES the existing `render_countered`
  helper (~640) — NO new render helper. DESIGN owns the exact markup.
- The countered count comes from US-PC-000 (the 5th `LandingSummary` field). NO new read invented
  here.
- Anti-misread: reuse the slice-14 / slice-18 neutral-copy sensibility (the `render_countered`
  helper is already proven neutral). The countered count is "(N countered)" — a neutral noun,
  never a verdict.
- The slice-18 own line is UNTOUCHED — this story adds the peer parenthetical only.

---

## US-PC-002: In the `/peer-claims` list header, see the same disputed-peer-claim awareness count

`job_id: J-003b`

### Problem

When Maria drills from the front door into `GET /peer-claims`, the list header ("Peer Claims" +
the federated-not-mine notice + the tab nav) shows her cached peer claims and the slice-13
per-row "Countered" flags — but it does not summarize, at the top, HOW MANY of those peer claims
are countered. She can see individual flags as she scans, but the header gives her no at-a-glance
total to orient the page ("am I scanning 1 contested peer claim or 30?"). The disputed-claim
awareness she gets on the landing (US-PC-001) is absent the moment she lands on the peer list
itself — the two orientation surfaces are inconsistent. The slice-18 `/claims` header already
solved this for OWN claims; `/peer-claims` is the symmetric gap.

### Who

- P-001 (the viewer operator, "Maria"), counter-aware-orientation hat | having drilled into
  `/peer-claims` to read her contested cached peer claims | wants the list header to tell her, at
  a glance, how many of her cached peer claims are countered — the SAME disputed-claim awareness
  the landing gave her — so the page orients consistently before she scans the per-row flags,
  exactly as the slice-18 `/claims` header does for her own claims.

### Solution

On `GET /peer-claims`, render the SAME countered-peer-claims count in the list header (beside the
"Peer Claims" heading) — the count of cached peer claims that have ≥1 counter, consistent with the
landing (US-PC-001) and driven by the SAME US-PC-000 read, via the SAME `render_countered` helper
(the EXACT mirror of slice-18's `/claims` header `h1 { "My Claims " (render_countered(...)) }`).
A successful 0 renders the honest-zero copy; a failed read renders the missing marker (the list
rows still render). The count is a PRESENCE count (a peer claim countered by N counterers counts
once), additive (it does NOT re-order, filter, re-page, re-count, or re-weight the list — the
slice-13 per-row flags and the slice-06/07 ordering/paging are untouched), and NEUTRAL copy (not
a penalty/score). Read-only, LOCAL/offline.

### Elevator Pitch

- **Before**: when Maria lands on `http://127.0.0.1:<port>/peer-claims`, the header shows "Peer
  Claims" and the federated-not-mine notice; she sees the slice-13 per-row "Countered" flags only
  as she scans — the header gives her no at-a-glance total of how many of her cached peer claims
  are contested (the slice-18 `/claims` header has this for her OWN claims; `/peer-claims` does
  not).
- **After**: open `http://127.0.0.1:<port>/peer-claims` → the list header carries the same
  disputed-claim awareness as the landing — how many of her cached peer claims have been countered
  ("(1 countered)") — a neutral presence count consistent with the front door; "(0 countered)"
  when none; the missing marker if the count can't be read, with the rows + per-row flags still
  rendering; the list order/paging/flags are untouched.
- **Decision enabled**: Maria orients the `/peer-claims` page at a glance — knows whether she is
  scanning 1 contested peer claim or 30 before she starts — and the disputed-claim awareness is
  consistent whether she reads it on the front door, on `/claims` (own), or on `/peer-claims`
  (peer), completing the symmetric orientation slice-18 began.

### Domain Examples

#### 1: Happy path — `/peer-claims` header shows "(1 countered)", consistent with the landing

Maria caches 4 peer claims, 1 countered. She opens `/peer-claims`: the header shows "Peer Claims"
with "(1 countered)" — the SAME number she saw on the landing's "4 peer claims (1 countered)" —
above the list whose `bafyTobiasRust` row carries the slice-13 per-row flag. The header total and
the per-row flag agree.

#### 2: Edge case — honest "(0 countered)" header, list renders as slice-06/07

Maria caches 4 peer claims, none countered. The `/peer-claims` header shows "(0 countered)"
(honest zero), the list renders its 4 rows with NO per-row flags (slice-13), and the
order/paging/origin columns are byte-identical to slice-06/07 — the header count is additive, it
changed nothing.

#### 3: Boundary — failed count, header shows the missing marker, rows still render

Maria's countered-peer-count read fails. The `/peer-claims` header shows the missing marker for
the countered count (DISTINCT from "(0 countered)"), but the list rows STILL render (the row read
is independent of the header count read); the page is a 200, not a 5xx. The per-row slice-13 flags
render or not per their own presence read, independent of the header total.

### UAT Scenarios (BDD)

> Driving route: `GET /peer-claims` (the real `openlore ui` subprocess).

#### Scenario: The `/peer-claims` header shows how many of my cached peer claims are countered

Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter
When she opens `GET /peer-claims`
Then the list header shows the disputed-claim awareness "(1 countered)" beside "Peer Claims"
And it is the SAME count the landing shows beside "4 peer claims"

#### Scenario: An honest zero in the `/peer-claims` header when nothing is countered

Given Maria's store caches 4 peer claims, none of which has drawn a counter
When she opens `GET /peer-claims`
Then the list header shows "(0 countered)"
And the list renders its rows with no per-row Countered flags, exactly as slice-06/07

#### Scenario: The header count does not re-order, filter, or re-weight the peer list

Given Maria's store caches a mix of countered and un-countered peer claims
When she opens `GET /peer-claims`
Then the row order, page boundaries, total count, every row's confidence, and every row's peer origin are byte-identical to a render without the header count
And the countered rows are not pulled to the top or grouped by the header count

#### Scenario: A failed header count degrades without blanking the peer list

Given Maria's countered-peer-claims count read fails
When she opens `GET /peer-claims`
Then the list header renders the missing-number state for the countered count, not a fabricated "(0 countered)"
And the list rows and their slice-13 per-row flags still render
And the page is a normal 200, not a 5xx

### Acceptance Criteria

- [ ] `/peer-claims` renders the countered-peer-claims count in the list header beside "Peer Claims" — the SAME number as the landing (US-PC-001), driven by the SAME US-PC-000 read, via the SAME `render_countered` helper.
- [ ] The count is a PRESENCE count (a peer claim countered by N counterers counts once); the copy is NEUTRAL (no penalty/score/verdict/"refuted"/"false").
- [ ] A successful 0 renders the honest-zero copy; a failed read renders the missing marker (DISTINCT from 0), and the list rows + slice-13 per-row flags STILL render.
- [ ] The header count is ADDITIVE — it does NOT change the slice-06/07 list ordering (`composed_at DESC`), page boundaries, total count, any row's verbatim confidence, or any row's peer origin; it does not re-order, group, or filter by countered state.
- [ ] The header count renders LOCAL/offline (no CDN); the route stays read-only (no write control, no key).
- [ ] A failed header-count read returns 200 (never a 5xx) and never blanks the list rows or the per-row flags.

### Outcome KPIs

- **Who**: P-001 dogfood operators landing on `/peer-claims`
- **Does what**: orients the peer list page at a glance via the header countered count, consistent with the landing — completing the symmetric own (`/claims`, slice-18) + peer (`/peer-claims`, this slice) orientation
- **By how much**: 100% consistency between the landing "(N countered)" and the `/peer-claims` header "(N countered)" for the same store (single-source-of-truth; gold test); 0 list-order/paging/confidence/origin regression vs slice-06/07 (zero tolerance)
- **Measured by**: gold acceptance test (landing peer count == `/peer-claims` header count for the same store; list render byte-identical to the no-header-count baseline except the header)
- **Baseline**: today `/peer-claims` shows the slice-13 per-row flags but no at-a-glance header total of how many cached peer claims are countered (slice-18 added the header total only to `/claims`)

### Technical Notes

- Render: extend the `render_peer_claims_page` header (`crates/viewer-domain/src/lib.rs` ~1170,
  the `h1 { "Peer Claims" }`) to render the countered count beside the heading — e.g. `h1 { "Peer Claims " (render_countered(countered_peer_claims)) }` —
  the EXACT mirror of the slice-18 `/claims` header (`h1 { "My Claims " (render_countered(...)) }`,
  ~389). `render_peer_claims_page` takes the bare `Option<usize>` as a new param. REUSES the
  existing `render_countered` helper.
- The count is the SAME US-PC-000 number the landing uses — a single source for both surfaces (BR
  consistency). NO new read invented here.
- Additive / no-regression: the header count is rendered in the header ONLY; the slice-06/07
  `list_peer_claims` SQL (ordering/paging/count) and the slice-13 per-row presence flags are
  UNTOUCHED. Pinned by a gold test asserting list byte-identity (order/paging/count/confidence/
  origin) vs the no-header-count baseline.
- Anti-misread: reuse the slice-14 / slice-18 neutral-copy sensibility (`render_countered` is
  proven neutral). Depends on: US-PC-000 (the read), US-PC-001 (the landing render establishes the
  copy/shape — identical to slice-18's). No new route (extends `GET /peer-claims`). No new crate.

---

## Out of scope (explicit — restated from requirements.md)

- **Any write / compose / sign / subscribe / follow control on `/` or `/peer-claims`** —
  read-only (C-1, CARDINAL). No key.
- **A new route** — `GET /` and `GET /peer-claims` already exist; slice-19 extends them (C-7).
- **Rendering counter CONTENT (authors, reasons, threads) in the count** — the count is a number;
  reading WHO countered WHAT stays the existing attributed surfaces (`/claims/{cid}` slice-11
  thread; the slice-13 per-row flags on `/peer-claims`).
- **A "disputed by N" total / a re-weight / a verdict** — presence count only (C-4 / BR-PC-1).
- **Re-weighting or deducting from the peer-claims count** — "(N countered)" is additive
  awareness; the "4" is unchanged (C-4).
- **Re-ordering or filtering `/peer-claims` by countered state** — the header count is additive;
  the slice-13 per-row flags already mark individual rows (US-PC-002 AC).
- **A third dimension / re-touching the slice-18 own-claims countered count** — this slice is the
  own+peer COMPLETION; it adds JUST the peer count; the slice-18 own surfaces are UNTOUCHED
  (BR-PC-4).
- **Any network seam on these routes** — the countered-peer count is a LOCAL aggregate (C-2).
- **A per-claim `counter_presence_for` loop (N+1)** — a FIXED aggregate read (C-3).
- **Penalty / score / "refuted" / "false" copy** — neutral disputed-claim awareness (C-6).
- **A fabricated 0 when the read failed** — a failed read is the missing marker, distinct from a
  real "(0 countered)" (C-5).
- **Persisting anything; binding anything but 127.0.0.1; adding a new crate** (C-7).
