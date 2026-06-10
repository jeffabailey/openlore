# Requirements: viewer-counter-aware-counts (slice-18)

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-09
> Feature type: User-facing — a brownfield DELTA extending the slice-17 `GET /` landing
> summary and the slice-06 `GET /claims` list header with disputed-claim awareness.
> JTBD: **J-003b** (counter-claim awareness — the orientation / at-a-glance-count facet).
> No new job, no new sub-job, no new KPI ID, no new route, no new crate. Workspace stays 21.

## Context

The viewer has shipped the "Countered" presence flag on every per-row / per-edge surface
(slices 11–14): a reader on `/claims/{cid}`, `/claims`, `/peer-claims`, `/project`,
`/philosophy` can see which individual claims drew a counter. Separately, slice-17 shipped
the front-door orientation dashboard: `GET /` renders an at-a-glance LOCAL store summary
("12 own claims · 7 peer claims · 2 active peers") + a navigation hub to all 8 shipped
surfaces, realizing KPI-VIEW-1 (time-to-see-store-contents) as the front door.

This slice ties the counter family INTO orientation. Today the front-door summary answers
"how MUCH is in my store" but not "how much of it has been DISPUTED" — the operator must
open `/claims` and scan for slice-12 flags (or drill each claim) to learn how much of her
own work has drawn pushback. slice-18 surfaces, at a glance, **how many of the operator's
own claims have been countered**, beside the own-claims count, on the landing
("**12 own claims (3 countered)**") and in the `/claims` list header (the same
disputed-claim awareness). A reader orienting at the front door immediately sees not just
how much is in her store but how much has been disputed, and can drill into the flagged
rows (slices 12–14) to read the disagreements.

## Functional Requirements

- **FR-CC-1**: `GET /` renders the operator's **countered-own-claims count** beside the
  own-claims count in the landing summary — e.g. "12 own claims (3 countered)". The
  countered count is the number of the operator's OWN claims that have ≥1 counter.
- **FR-CC-2**: `GET /claims` renders the SAME disputed-claim awareness in the list header
  (alongside / near "My Claims") — the count of the operator's own claims that have ≥1
  counter, consistent with the landing.
- **FR-CC-3**: The countered count is a **PRESENCE count**: how many own claims have ≥1
  counter. A claim countered by N peers counts ONCE (never "disputed by N", never a sum of
  counters, never a re-weight).
- **FR-CC-4**: The countered count is **Option-shaped** (like the slice-17 summary counts):
  a SUCCESSFUL read of 0 renders "(0 countered)" (an honest "nothing of mine has been
  disputed"); a FAILED read renders the missing-number marker, DISTINCT from a real 0.
- **FR-CC-5**: A FAILED countered-count read **degrades independently**: it must NOT 5xx,
  must NOT blank the own-claims count or the other landing counts, and must NOT blank the
  `/claims` list. The landing's other counts and the nav hub still render; the `/claims`
  list still renders its rows.
- **FR-CC-6**: The countered count is a LOCAL aggregate over the indexed counter-reference
  tables (`claim_references ∪ peer_claim_references`, `ref_type='counters'`); NO network.

## Non-Functional Requirements

- **NFR-CC-1 (Read-only / no key — CARDINAL, inherited I-VIEW)**: the countered count is a
  COUNT only; the surface adds no mutation method, no signing key, no write / compose / sign
  / subscribe / follow control. [KPI-VIEW-2]
- **NFR-CC-2 (LOCAL / offline — CARDINAL, inherited KPI-5 / KPI-VIEW-5)**: the countered
  count read is a LOCAL aggregate; `/` and `/claims` render fully with the network down,
  referencing only the vendored `/static/htmx.min.js` (no CDN).
- **NFR-CC-3 (Cheap / no N+1 — CARDINAL, inherited slice-17 C-4 + slice-12 I-LF-8)**: the
  countered count is a SMALL FIXED number of aggregate reads per render (ideally ONE
  count-only aggregate, or it folds into the existing summary resolution), invariant to
  store size — NOT a per-claim `counter_presence_for` loop. The landing's "3 fixed reads"
  budget grows by at most 1.
- **NFR-CC-4 (Missing ≠ zero — inherited slice-17 WD-LD-8)**: the countered count is
  Option-shaped; Some(0) = honest "no claims countered", None = failed read → missing
  marker. A fabricated 0 on a failed read is forbidden.
- **NFR-CC-5 (Graceful degrade — inherited NFR-VIEW-6)**: a failed countered-count read
  never returns a 5xx, never blanks the other counts, never shows a raw stack trace.

## Business Rules

- **BR-CC-1 (Accuracy / shown-never-applied — CARDINAL, inherited J-003b)**: the countered
  count is a PRESENCE count (how many own claims have ≥1 counter), NEVER a "disputed by N"
  total, NEVER a re-weight of the own-claims count, NEVER a verdict. A claim countered by 2
  peers counts ONCE. The own-claims count ITSELF is unchanged — "(3 countered)" is additive
  awareness, not a deduction from "12".
- **BR-CC-2 (Own claims are countered by peers)**: the self-counter rule blocks countering
  your own claim, so a countered own-claim = an own-claim cid (`SELECT cid FROM claims`)
  that appears as a countered `referenced_cid`. The countered count counts the operator's
  OWN claims that have drawn a counter (from a peer's `peer_claim_references`, or her own
  later counter to a DIFFERENT claim of hers, in `claim_references`).
- **BR-CC-3 (Anti-misread / neutral copy — inherited slice-14)**: "(3 countered)" reads as
  NEUTRAL disputed-claim awareness, not a penalty/score/deduction. No penalty, deduction,
  "refuted", "false", or score language. The own-claims count stands unchanged beside it.
- **BR-CC-4 (Own-claims is the core; peer-claims countered is optional/deferred)**: this
  slice surfaces the COUNTERED-OWN-CLAIMS count as the load-bearing orientation signal ("how
  much of MY work has been disputed"). Whether to also add a "(N countered)" to the
  PEER-claims count is an explicit SCOPE decision (WD-CC-7) — recommended own-claims-only as
  the core, peer optional/deferred.

## Out of scope (explicit)

- Any write / compose / sign / subscribe / follow control on `/` or `/claims` (NFR-CC-1,
  CARDINAL). No key.
- A new route — both `GET /` and `GET /claims` already exist; slice-18 extends them.
- Rendering counter CONTENT (authors, reasons, threads) in the count — the count is a
  number; reading WHO countered WHAT stays the existing attributed surfaces
  (`/claims/{cid}` slice-11 thread, the slice-12 list flags).
- A "disputed by N" total / a re-weight / a verdict — presence count only (BR-CC-1).
- Any network seam — the countered count is a LOCAL aggregate (NFR-CC-2).
- A per-claim `counter_presence_for` loop (N+1) — a fixed aggregate read (NFR-CC-3).
- Re-ordering or filtering `/claims` by countered state — the count is additive awareness,
  the slice-12 per-row flags already mark individual rows (the count does not change order).
- Persisting anything; binding anything but 127.0.0.1; adding a new crate.

## Traceability

| Requirement | Job | Inherited from |
|---|---|---|
| FR-CC-1/2/3 | J-003b | counter family (slices 11–14) + slice-17 landing summary |
| FR-CC-4, NFR-CC-4 | J-003b | slice-17 WD-LD-8 (missing ≠ zero) |
| FR-CC-5, NFR-CC-5 | J-003b | slice-17 WD-LD-2 (graceful degrade) / NFR-VIEW-6 |
| FR-CC-6, NFR-CC-2 | J-003b | slice-12 (local counter-ref tables) / KPI-5 |
| NFR-CC-1 | J-003b | I-VIEW-1/2/3 / KPI-VIEW-2 |
| NFR-CC-3 | J-003b | slice-17 C-4 + slice-12 I-LF-8 (no N+1) |
| BR-CC-1/2/3 | J-003b | J-003b accuracy cardinal + slice-14 anti-misread |
