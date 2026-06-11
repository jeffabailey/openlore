# Story Map: viewer-search-full-follow-state (slice-20)

## User: P-001 Senior Engineer Solo Builder ("Maria"), network-discovery hat
## Goal: scan `/search` results and read the FOUR honest follow-states of each discovered author — own claim, followed peer, removed-and-cached peer, genuinely-new author — so the only `peer add` shown is one she could meaningfully run.

> Brownfield DELTA completing the slice-16 `/search` follow-state (which resolved only the
> binary `SubscribedPeer`/`NetworkUnfollowed`). The backbone is the EXISTING slice-08/16
> network-discovery journey; slice-20 sharpens the per-row affordance step to four arms.

## Backbone

| Discover claims on `/search` | Read each result's follow-state | Decide the next action |
|------------------------------|----------------------------------|------------------------|
| Open `GET /search`, query by philosophy/subject/contributor (slice-08, UNCHANGED) | See per-row affordance: self / Following / residue / `peer add` | Follow only genuinely-new authors via the slice-03 CLI `openlore peer add` |
| See verified + attributed results (slice-08, UNCHANGED) | **[slice-20] own claim → neutral self indicator** | Ignore the add prompt on own claims + removed-peer residue |
| | **[slice-20] removed-and-cached peer → neutral residue indicator** | |
| | followed peer → "Following" (slice-16, UNCHANGED) | |
| | new author → `openlore peer add <did>` (slice-08/16, UNCHANGED) | |

---

### Walking Skeleton

Brownfield DELTA — NO walking-skeleton Feature 0. Everything in the backbone's first column
(open `/search`, query, see verified+attributed results) and the slice-16 binary follow-state
already ships. The thinnest end-to-end slice that connects the goal is:

- **US-FS-001** (the resolution plumbing) — read the operator's own-claim author DIDs + the
  cached-peer author DIDs (two NEW LOCAL batch reads) alongside the slice-16 active set, and
  resolve each result author to the full four-arm `AuthorRelationship` by precedence, threaded
  through `to_indexed_claim`.
- **US-FS-002** (the user-visible render) — fill the render `@match`'s two ALREADY-EMPTY arms
  with a neutral self indicator (`You`) and a neutral residue indicator (`UnsubscribedCache`);
  the slice-16 "Following"/`peer add` arms unchanged.

Both are demonstrable in a single session against the real `openlore ui`: seed an own claim + a
soft-removed-but-cached peer + a followed peer + a new author, search, observe four distinct
neutral affordances.

### Release 1 (the only release): four-arm `/search` follow-state completeness

| Story | Outcome KPI targeted | Rationale |
|---|---|---|
| US-FS-001 (infra) | enables KPI-AV-4 accuracy | the four-arm resolution against three LOCAL batch reads; no N+1; independent degrade |
| US-FS-002 | KPI-AV-4 (discovery→federation funnel accuracy) | the `peer add` affordance shown ONLY for genuinely-new authors (0% on own/removed-cached, on top of slice-16's 0% on followed); the slice-16 states byte-stable |

There is exactly one release: the slice is one thin, coherent outcome (the four-arm completeness).
No second band — the feature is complete when both stories ship.

## Priority Rationale

Priority is driven by outcome impact and the single hard dependency:

1. **US-FS-001 first (P1)** — the four-arm precedence resolution + the two NEW LOCAL presence
   reads. It is the walking-skeleton infrastructure; US-FS-002 cannot render a state the
   resolution does not produce. Validates the riskiest assumptions early: that the two presence
   reads stay batch-once (no N+1), degrade independently, and reconcile DIDs via `bare_did`.
2. **US-FS-002 second (P1, depends on US-FS-001)** — the two neutral render arms + the
   no-regression guarantee. This is the user-visible outcome that moves KPI-AV-4: the operator
   reads four honest states and the `peer add` affordance becomes fully actionable-only. Depends
   on US-FS-001's resolution; renders nothing new without it.

Both are P1 (the slice has no value until both ship). The tie-break (Walking Skeleton >
Riskiest Assumption > Highest Value) keeps US-FS-001 ahead of US-FS-002. No P2/P3 — anything
larger (a follow control, an own-identity surface, resolving the arms on other surfaces) is
explicitly out of scope and would be a separate slice.
