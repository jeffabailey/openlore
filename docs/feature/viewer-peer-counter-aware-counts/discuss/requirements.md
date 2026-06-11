# Requirements: viewer-peer-counter-aware-counts (slice-19)

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-10
> Feature type: User-facing — a brownfield DELTA extending the slice-17 `GET /` landing
> summary (the PEER-claims line) and the slice-06 `GET /peer-claims` list header with
> disputed-peer-claim awareness.
> JTBD: **J-003b** (counter-claim awareness — the orientation / at-a-glance-count facet),
> the SAME job slice-18 realized for own claims. No new job, no new sub-job, no new KPI ID,
> no new route, no new crate. Workspace stays 21.

## Context

slice-18 (`viewer-counter-aware-counts`) shipped the countered-OWN-claims count beside the
own-claims line on the landing ("12 own claims (3 countered)") and in the `/claims` list
header — connecting the shipped counter-flag family (slices 11–14) to the front-door
orientation (slice-17). It explicitly DEFERRED the symmetric peer-claims sibling (WD-CC-7:
own-claims-only the core; peer-claims-countered "a recommended additive sibling … if dogfood
shows demand"; ADR-055 noted "the count-only query shape makes the deferred sibling clean to
add later").

slice-19 is that deferred sibling. It extends the SAME counter-aware-count pattern to the
PEER-claims line: "4 peer claims (1 countered)" on the landing summary AND in the
`/peer-claims` list header. So a reader orienting at the front door sees not just how many
peer claims she has cached but how many of THOSE have been disputed — by the operator's OWN
counter (which lands in `claim_references`) OR by another peer's counter (which lands in
`peer_claim_references` — slice-11: "Rachel counters Tobias's peer claim"). With slice-18
(own) + slice-19 (peer), counter-aware orientation is COMPLETE across both own and peer
claims, and the reader can drill into `/peer-claims` (where the slice-13 per-row "Countered"
flags already render) to read the disagreements.

## Functional Requirements

- **FR-PC-1**: `GET /` renders the **countered-peer-claims count** beside the peer-claims
  count in the landing summary — e.g. "4 peer claims (1 countered)". The countered count is
  the number of the operator's CACHED PEER claims that have ≥1 counter.
- **FR-PC-2**: `GET /peer-claims` renders the SAME disputed-peer-claim awareness in the list
  header (beside the "Peer Claims" heading) — the count of cached peer claims that have ≥1
  counter, consistent with the landing.
- **FR-PC-3**: The countered count is a **PRESENCE count**: how many peer claims have ≥1
  counter. A peer claim countered by N counterers (the operator + another peer, or two peers)
  counts ONCE (never "disputed by N", never a sum of counters, never a re-weight).
- **FR-PC-4**: The countered count is **Option-shaped** (like the slice-17 summary counts and
  the slice-18 countered-own count): a SUCCESSFUL read of 0 renders "(0 countered)" (an honest
  "none of my cached peer claims has been disputed"); a FAILED read renders the missing-number
  marker, DISTINCT from a real 0.
- **FR-PC-5**: A FAILED countered-peer-count read **degrades independently**: it must NOT
  5xx, must NOT blank the peer-claims count or the other landing counts, and must NOT blank
  the `/peer-claims` list. The landing's other counts and the nav hub still render; the
  `/peer-claims` list still renders its rows + the slice-13 per-row flags.
- **FR-PC-6**: The countered count is a LOCAL aggregate over the indexed counter-reference
  tables (`claim_references ∪ peer_claim_references`, `ref_type='counters'`), with the outer
  set being the cached PEER claims (`peer_claims`); NO network.

## Non-Functional Requirements

- **NFR-PC-1 (Read-only / no key — CARDINAL, inherited I-VIEW)**: the countered-peer count is
  a COUNT only; the surface adds no mutation method, no signing key, no write / compose / sign
  / subscribe / follow control. [KPI-VIEW-2]
- **NFR-PC-2 (LOCAL / offline — CARDINAL, inherited KPI-5 / KPI-VIEW-5)**: the countered-peer
  count read is a LOCAL aggregate; `/` and `/peer-claims` render fully with the network down,
  referencing only the vendored `/static/htmx.min.js` (no CDN).
- **NFR-PC-3 (Cheap / no N+1 — CARDINAL, inherited slice-17 C-4 + slice-12 I-LF-8)**: the
  countered-peer count is a SMALL FIXED number of aggregate reads per render (ideally ONE
  count-only aggregate — a 5th sibling of `count_claims` / `count_peer_claims` /
  `count_active_peer_subscriptions` / `count_countered_own_claims`). The landing read budget
  grows by EXACTLY 1 (a 5th count read); the `/peer-claims` header read grows by 1; invariant
  to store size — NOT a per-claim `counter_presence_for` loop.
- **NFR-PC-4 (Missing ≠ zero — inherited slice-17 WD-LD-8 / slice-18 C-5)**: the countered
  count is Option-shaped; Some(0) = honest "no peer claims countered", None = failed read →
  missing marker. A fabricated 0 on a failed read is forbidden.
- **NFR-PC-5 (Graceful degrade — inherited NFR-VIEW-6)**: a failed countered-peer-count read
  never returns a 5xx, never blanks the other counts, never shows a raw stack trace, never
  blanks the `/peer-claims` rows.

## Business Rules

- **BR-PC-1 (Accuracy / shown-never-applied — CARDINAL, inherited J-003b)**: the countered
  count is a PRESENCE count (how many peer claims have ≥1 counter), NEVER a "disputed by N"
  total, NEVER a re-weight of the peer-claims count, NEVER a verdict. A peer claim countered
  by 2 counterers counts ONCE. The peer-claims count ITSELF is unchanged — "(1 countered)"
  is additive awareness, not a deduction from "4".
- **BR-PC-2 (Peer claims are countered by the operator OR by other peers)**: a countered peer
  claim = a peer-claim cid (`SELECT cid FROM peer_claims`) that appears as a countered
  `referenced_cid`. The operator's counter to a peer claim lands in `claim_references`
  (`ref_type='counters'`); another peer's counter to that peer claim lands in
  `peer_claim_references` (slice-11: "Rachel counters Tobias's peer claim"). EITHER source
  makes the peer claim countered; the union is taken and de-duped so it counts ONCE.
- **BR-PC-3 (Anti-misread / neutral copy — inherited slice-14 / slice-18 C-6)**: "(1
  countered)" reads as NEUTRAL disputed-claim awareness, not a penalty/score/deduction. No
  penalty, deduction, "refuted", "false", or score language. The peer-claims count stands
  unchanged beside it. Reuses the SAME pure `render_countered(Option<usize>)` helper slice-18
  established (single SSOT copy site).
- **BR-PC-4 (Scope: this is the own+peer COMPLETION — JUST the peer count)**: this slice adds
  the COUNTERED-PEER-CLAIMS count (the slice-18 deferred sibling). It is JUST the peer count —
  the own count shipped in slice-18; there is NO third dimension. The own-claims "(N
  countered)" rendered by slice-18 on the landing + `/claims` header is UNTOUCHED.

## Out of scope (explicit)

- Any write / compose / sign / subscribe / follow control on `/` or `/peer-claims` (NFR-PC-1,
  CARDINAL). No key.
- A new route — both `GET /` and `GET /peer-claims` already exist; slice-19 extends them.
- Rendering counter CONTENT (authors, reasons, threads) in the count — the count is a number;
  reading WHO countered WHAT stays the existing attributed surfaces (`/claims/{cid}` slice-11
  thread, the slice-13 per-row flags on `/peer-claims`).
- A "disputed by N" total / a re-weight / a verdict — presence count only (BR-PC-1).
- Any network seam — the countered-peer count is a LOCAL aggregate (NFR-PC-2).
- A per-claim `counter_presence_for` loop (N+1) — a fixed aggregate read (NFR-PC-3).
- Re-ordering or filtering `/peer-claims` by countered state — the count is additive
  awareness; the slice-13 per-row flags already mark individual rows (the count does not
  change order/paging).
- Re-touching the slice-18 own-claims countered count — that surface is shipped and UNCHANGED
  (BR-PC-4). No third dimension.
- Persisting anything; binding anything but 127.0.0.1; adding a new crate.

## Traceability

| Requirement | Job | Inherited from |
|---|---|---|
| FR-PC-1/2/3 | J-003b | counter family (slices 11–14) + slice-17 landing summary + slice-18 (own mirror) |
| FR-PC-4, NFR-PC-4 | J-003b | slice-17 WD-LD-8 / slice-18 C-5 (missing ≠ zero) |
| FR-PC-5, NFR-PC-5 | J-003b | slice-17 WD-LD-2 / slice-18 C-2 (graceful degrade) / NFR-VIEW-6 |
| FR-PC-6, NFR-PC-2 | J-003b | slice-12 (local counter-ref tables) / KPI-5 |
| NFR-PC-1 | J-003b | I-VIEW-1/2/3 / KPI-VIEW-2 |
| NFR-PC-3 | J-003b | slice-17 C-4 + slice-12 I-LF-8 + slice-18 ADR-055 D1 (count-only aggregate) |
| BR-PC-1/2/3 | J-003b | J-003b accuracy cardinal + slice-14 anti-misread + slice-11 (peer counters) |
| BR-PC-4 | J-003b | slice-18 WD-CC-7 (the deferred peer sibling) |
