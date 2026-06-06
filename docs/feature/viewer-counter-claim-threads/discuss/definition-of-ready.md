# Definition of Ready: viewer-counter-claim-threads (slice-11)

DoR is a HARD GATE. All 9 items must PASS with evidence before DESIGN handoff.

## Story: US-CT-001 — Read-only counter-claim thread READ capability

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "The viewer has no way to read the counter-claims that target a given claim" — names the gap in `StoreReadPort`. |
| 2. User/persona with specific characteristics | PASS | P-001 Maria (node operator) reading her local store via `openlore ui`; infra story enabling her reading. |
| 3. 3+ domain examples with real data | PASS | One counter (`bafy...new` by `did:plc:maria-test`), two counters (+ `bafy...t0bi` by `did:plc:tobias-test`), un-countered (`bafy...solo` → empty vec). |
| 4. UAT in Given/When/Then (3-7) | PASS | 4 scenarios (read counters, two-author attribution, empty result, no-write surface). |
| 5. AC derived from UAT | PASS | 5 AC, each maps to a scenario (read method shape, attributed rows, empty-ok, no-write, 21 members). |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 4 scenarios; one read method + view-model; ~0.3 day. |
| 7. Technical notes: constraints/dependencies | PASS | Reuses slice-03 model + slice-09/10 UNION-ALL anti-merging pattern; DESIGN owns SQL; no external dep. |
| 8. Dependencies resolved or tracked | PASS | Depends on existing `claims` + `peer_claims` tables + `references[]` shape (all present). |
| 9. Outcome KPIs defined with measurable targets | PASS | 100% of counters returned, attributed, zero merged; baseline 0. |
| (JTBD) job_id present | PASS | `infrastructure-only` + `infrastructure_rationale` (the only infra story; slice has 2 user-visible stories). |

## Story: US-CT-002 — See the counter-claim thread beneath a countered claim

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "When she opens a claim she sees only the claim; she cannot tell it is disputed nor read the countering reasoning on the browser surface." |
| 2. User/persona with specific characteristics | PASS | P-001 Maria, counter-claim-reader hat, drilling into a claim on the loopback viewer. |
| 3. 3+ domain examples with real data | PASS | One peer counter; two counters (Maria + Tobias); empty-reason boundary (non-OpenLore client). |
| 4. UAT in Given/When/Then (3-7) | PASS | 5 scenarios (thread render, two-author non-merge, CID link, empty-reason, htmx/no-JS parity, offline) — note the parity+offline pair counts within 3-7. |
| 5. AC derived from UAT | PASS | 7 AC, each traceable to a scenario; all observable on `GET /claims/{cid}`. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 6 scenarios; one render path on an existing route; ~0.5 day. |
| 7. Technical notes: constraints/dependencies | PASS | Extends `render_claim_detail*` + `claim_detail_page`; reuses Shape/render_confidence/PeerOrigin; depends on US-CT-001. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-CT-001 (in-slice). No external dep. |
| 9. Outcome KPIs defined with measurable targets | PASS | 100% counters shown attributed, zero merged, confidence unchanged; baseline 0; leading indicator of KPI-FED-3. |
| (JTBD) job_id present | PASS | `J-003b` + Elevator Pitch (Before/After/Decision, references the real `GET /claims/{cid}` entry point + observable HTML). |

## Story: US-CT-003 — Un-countered renders cleanly; countered is flagged

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "Most claims are not countered; an empty section on every claim would add noise and dilute the signal." |
| 2. User/persona with specific characteristics | PASS | P-001 Maria, scanning + drilling into claims. |
| 3. 3+ domain examples with real data | PASS | Un-countered (`bafy...solo`); countered+flagged (`bafy...n4ka`); non-existent (`bafy...nope`). |
| 4. UAT in Given/When/Then (3-7) | PASS | 4 scenarios (no-noise empty, disputed flag, both-shapes discipline, 404 unaffected). |
| 5. AC derived from UAT | PASS | 5 AC, each maps to a scenario. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 4 scenarios; a thin discipline branch on the same render; ~0.2 day. |
| 7. Technical notes: constraints/dependencies | PASS | Keys off empty-vs-non-empty result; reuses guided-empty precedent; flag wording is DESIGN's. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-CT-001 + US-CT-002 (in-slice). No external dep. |
| 9. Outcome KPIs defined with measurable targets | PASS | 100% un-countered clean, 100% countered flagged, 0 confidence altered; leading indicator of KPI-VIEW-1/FED-3. |
| (JTBD) job_id present | PASS | `J-003b` + Elevator Pitch. |

## Elevator Pitch check (Dimension 0)

- US-CT-001: `@infrastructure` (job_id `infrastructure-only`) — Elevator Pitch NOT
  required; rationale present; slice has ≥1 user-visible story → slice-level check PASS.
- US-CT-002: Elevator Pitch present — After line references the real entry point
  `http://127.0.0.1:<port>/claims/bafy...n4ka`, sees observable HTML (the thread with
  author DID/CID/reason), Decision enabled (trust/cite/counter-via-CLI). PASS.
- US-CT-003: Elevator Pitch present — real entry point `/claims/bafy...solo` +
  `/claims/bafy...n4ka`, observable output (clean render vs "Countered" flag),
  Decision enabled (trust clean vs read-with-both-sides). PASS.
- Slice-level: 2 of 3 stories are user-visible (non-`@infrastructure`) → the slice has
  release value. PASS.

## DoR Status: PASSED (9/9 for all three stories; Dimension-0 PASS)
