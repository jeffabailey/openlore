# Prioritization: viewer-htmx-swaps (slice-07)

> Ordered by **value/risk**, walking-skeleton and riskiest-assumption first. The riskiest
> assumption is the **progressive-enhancement contract** (the SAME route can serve a fragment
> under `HX-Request` and a complete slice-06 full page without it, additively, with parity) —
> validated by the walking skeleton. Value = removed reload-jolt on the interaction; Effort =
> rendering split + handler branch + (for tab) history mechanism.

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-HX-001) | `/claims` paging swaps in place; no-JS still gets the full page | KPI-HX-1 (+ guardrails KPI-HX-G1/G2) | Proves the load-bearing progressive-enhancement contract on the lowest-risk, clearest-win interaction. |
| 2 | R1: Lists page without a jolt (US-HX-002) | Both lists page in place | KPI-HX-1 | Same proven pattern on the second list; cheap; completes the paging outcome. |
| 3 | R2: Triage + inspect in place (US-HX-003, US-HX-004) | Scrape submit + claim detail update in place | KPI-HX-2, KPI-HX-3 | Highest remaining reload pain (scrape) + a simple GET-fragment (detail); carries the read-only-sensitive guardrails in fragment shape. |
| 4 | R3: Move between views as one place (US-HX-006) | Tab switch in place + bookmarkable | KPI-HX-4 | Adds the only new sub-mechanism (URL/history, OD-HX-4); sequenced after simpler swaps. |
| 5 | R4: Provably offline (US-HX-005) | htmx asset local, single-source, no CDN | KPI-HX-G2 (hardened), KPI-HX-G3 | Property/guardrail hardening over a mechanism the skeleton already stands up; DESIGN may fold forward via OD-HX-1. |

## Value / Urgency / Effort scoring

| Story | Value (1-5) | Urgency (1-5) | Effort (1-5) | Score (V×U/E) | Notes |
|-------|------------|---------------|--------------|---------------|-------|
| US-HX-001 | 5 (proves the contract + clearest paging win) | 5 (derisks the whole slice; walking skeleton) | 2 (GET fragment + handler branch; slice-06 pagination exists) | 12.5 | **First by skeleton tie-break regardless of score.** |
| US-HX-002 | 4 (second list; 1,840 rows = most visible paging) | 3 | 1 (repeat of US-HX-001) | 12.0 | Cheapest high-value follow-on. |
| US-HX-003 | 4 (scrape submit is the next-biggest reload jolt) | 3 | 3 (results fragment + zero/network-down states + guardrails) | 4.0 | Read-only-sensitive; prove no-sign-control/no-persist in fragment. |
| US-HX-004 | 3 (inline detail is a quality-of-life win) | 2 | 2 (GET detail fragment + unknown-CID state) | 3.0 | Simple GET-fragment reusing the pattern. |
| US-HX-006 | 3 (tab switch) | 2 | 3 (adds URL/history mechanism, OD-HX-4) | 2.0 | Only story with a new sub-mechanism. |
| US-HX-005 | 3 (offline guarantee is a slice-06 promise) | 3 (guardrail; release-relevant) | 2 (vendor/inline + audit, OD-HX-1) | 4.5 | Sequenced last as hardening; can fold forward at DESIGN. |

> Effort-by-score would float US-HX-005 above US-HX-003/004, but the walking-skeleton and
> riskiest-assumption tie-break holds the interaction stories ahead: prove the contract and
> remove the biggest reload jolts first; harden the asset guarantee once the mechanism is
> chosen (OD-HX-1).

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-HX-001 | WS | P1 | KPI-HX-1 + KPI-HX-G1/G2 | slice-06 `/claims` + PageView (exist); minimal local htmx reference |
| US-HX-002 | R1 | P2 | KPI-HX-1 | US-HX-001 (pattern); slice-06 `/peer-claims` |
| US-HX-003 | R2 | P3 | KPI-HX-2 + KPI-HX-G3 | US-HX-001 (pattern); slice-06 `POST /scrape` |
| US-HX-004 | R2 | P3 | KPI-HX-3 | US-HX-001 (pattern); slice-06 `/claims/{cid}` |
| US-HX-006 | R3 | P4 | KPI-HX-4 | US-HX-001/002 (lists); OD-HX-4 history strategy |
| US-HX-005 | R4 | P5 | KPI-HX-G2 (hardened) + KPI-HX-G3 | OD-HX-1 asset mechanism; used by all of US-HX-001..006 |

## Riskiest-assumption note

The single assumption that could invalidate the slice: *"a fragment and a full page of the
SAME content can be served off one route by header without the two shapes drifting and
without breaking the no-JS path."* US-HX-001 validates it end-to-end before any further
investment. If parity (I-HX-5) or no-JS fallback (I-HX-1) cannot be made structural in the
viewer-domain rendering split (OD-HX-2), surface it at DESIGN before R1.
