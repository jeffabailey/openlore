# Story Map: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06.** The backbone is the four operator interactions the enhancement
> touches, plus the local htmx asset that makes any swap possible offline. Each rib is one
> response *shape* of the SAME slice-06 content. The walking skeleton proves the whole
> progressive-enhancement contract on the lowest-risk interaction (pagination) end-to-end.

## User: Maria Santos — node operator
## Goal: navigate the local viewer with in-place updates (no reload / scroll jump / flash), while a no-JS / offline / direct-URL operator still gets the exact slice-06 full page.

## Backbone

| Activity A: Serve htmx locally | Activity B: Page lists | Activity C: Triage a scrape | Activity D: Open a claim | Activity E: Switch view |
|--------------------------------|------------------------|----------------------------|--------------------------|-------------------------|
| Reference htmx from this process (no CDN) | Swap claims-table on Prev/Next | Swap scrape results on submit | Load claim detail inline | Swap My↔Peer view panel |
| Offline test: swaps work network-down | Preserve full page for no-JS | Keep form + value; no sign control | Full detail page for no-JS | Update URL so bookmark/Back work |
| Single-source asset | Fragment/full-page parity | Network-down + zero-candidate guidance fragments | Unknown-CID guided in both shapes | Full page for no-JS |

---

### Walking Skeleton (thinnest end-to-end htmx thread)

**US-HX-001 — Pagination swaps the claims table in place; full page still served without JS.**

This is the minimum slice that proves the ENTIRE progressive-enhancement contract end to
end on the SAME route:

- **Activity A (thin)**: htmx is referenced from the process (whatever minimal mechanism)
  so the swap can fire at all — the skeleton carries just enough of the asset to make
  `/claims` paging swap. (Full asset hardening = US-HX-005.)
- **Activity B**: `GET /claims?page=N` under `HX-Request` returns the **claims-table
  fragment** (rows + `X–Y of N` + Prev/Next) → swapped in place; WITHOUT the header returns
  the **full slice-06 page**, byte-equivalent.

Every other interaction (scrape, detail, tab) reuses this exact pattern — header-drives-
shape + fragment/full-page parity + no-JS fallback. Proving it once on pagination de-risks
all of them. Pagination is chosen because it is read-only GET, already paginated in
slice-06 (PageView, clamp), lowest blast radius, and the clearest perceptible win on a
1,840-row peer list.

> Skeleton stays THIN: peer-claims paging, scrape, detail, and tab are NOT in the skeleton —
> they are later slices that repeat the proven pattern.

### Release 1: "Lists page without a jolt" — outcome: in-place paging on BOTH lists

- **US-HX-001** (walking skeleton) — `/claims` pagination swap + no-JS full page.
- **US-HX-002** — `/peer-claims` pagination swap + no-JS full page (peer origin preserved in
  the fragment; reuses the US-HX-001 pattern on the second list).
- Outcome KPI targeted: KPI-HX-1 (paging is an in-place swap, not a full reload) + guardrails
  KPI-HX-G1 (no-JS full page) and KPI-HX-G2 (offline).

### Release 2: "Triage and inspect without leaving the page" — outcome: scrape + detail in place

- **US-HX-003** — scrape form swaps results below the form (candidates / zero / network-down
  guidance); form stays; no sign control; nothing persisted.
- **US-HX-004** — claim detail loads inline; list stays; unknown-CID guided in both shapes;
  confidence verbatim.
- Outcome KPI: KPI-HX-2 (scrape submit in place) + KPI-HX-3 (detail inline), guardrails carried.

### Release 3: "Move between views as one place" — outcome: tab switch in place + bookmarkable

- **US-HX-006** — My Claims ↔ Peer Claims swap the view panel in place; the URL updates so
  the active view is bookmarkable and Back works (history strategy is OD-HX-4).
- Outcome KPI: KPI-HX-4 (tab switch in place with URL/history coherence).

### Release 4 (hardening, can fold earlier): "Provably offline" — outcome: htmx asset is local + single-source

- **US-HX-005** — htmx served locally (vendored or inlined; OD-HX-1), single source, no CDN;
  offline test is the gate; no-page-references-a-CDN property holds.
- Outcome KPI: guardrail KPI-HX-G2 (offline) hardened to a property; KPI-HX-G3 (no new write
  surface) reaffirmed.

> **Note on US-HX-005 placement**: the walking skeleton carries a *minimal* local reference
> to htmx so US-HX-001 can swap at all. US-HX-005 *hardens* that into the audited, single-
> source, offline-proven, no-CDN guarantee. It is sequenced last because it is a property/
> guardrail hardening over a mechanism the skeleton already stands up — but DESIGN may
> resolve OD-HX-1 up front and fold the hardening into the skeleton if the chosen mechanism
> (e.g. inlining) makes the guarantee trivial.

---

## Priority Rationale

Ordering is by **value/risk**, riskiest-assumption and walking-skeleton first:

1. **US-HX-001 (walking skeleton, pagination on /claims)** — validates the load-bearing
   assumption (header-drives-shape + fragment/full-page parity + no-JS fallback can ride the
   SAME route additively) on the **lowest-risk, highest-clarity** interaction. Read-only GET,
   already paginated in slice-06, smallest blast radius. If this pattern does not hold, the
   whole slice is in question — so it goes first.
2. **US-HX-002 (peer-claims pagination)** — same pattern, second list; cheap once US-HX-001
   proves it; completes the "lists page without a jolt" outcome.
3. **US-HX-003 (scrape) then US-HX-004 (detail)** — scrape before detail: scrape is the most
   visible reload pain after paging (submit currently reloads and re-runs the harvest in a
   full page) and carries the read-only-sensitive guardrails (no sign control, nothing
   persisted, network-down guidance) that are worth proving early in fragment shape; detail
   is a simple GET-fragment that reuses the pattern.
4. **US-HX-006 (tab switch)** — last of the interactions because it adds the only NEW
   sub-mechanism (URL/history update so bookmark/Back work, OD-HX-4); higher coordination,
   lower marginal pain than paging/scrape, so it is sequenced after the simpler swaps.
5. **US-HX-005 (local htmx asset hardening)** — the offline/no-CDN guarantee is a property/
   guardrail over the asset mechanism; the skeleton already carries a minimal local
   reference, so the audited single-source hardening is sequenced last (DESIGN may pull it
   forward when it resolves OD-HX-1).

Tie-breaking (per user-story-mapping skill): Walking Skeleton (US-HX-001) > Riskiest
Assumption (the progressive-enhancement contract, proven by US-HX-001) > Highest Value
(US-HX-003 scrape) > remaining by value/effort.
