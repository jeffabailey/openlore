# ADR-047: CounterThread ADT — Depth-1 (No Recursion) Render + Empty-Reason Display

- **Status**: Accepted
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: viewer-counter-claim-threads (slice-11)
- **Extends**: ADR-029 (maud pure-core view-model), ADR-032 (fragment/page render split), ADR-037 (SearchState ADT projection precedent), ADR-015 (`reason` wire-optional asymmetry)

## Context

slice-11 renders a counter-claim thread beneath the claim on `GET /claims/{cid}`. The
pure `viewer-domain` core needs a view-model that makes three product invariants
structural rather than conventional:

1. **No-noise** (US-CT-003): an un-countered claim must render with NO section and NO
   "0 counters" empty-state text.
2. **Anti-merging** (I-CT-3): two counters render as two attributed items — there must be
   NO "disputed by N" / "consensus" aggregate, even by accident.
3. **Shown, never applied** (I-CT-2): the countered claim's confidence/fields are never
   re-weighted by the counters.

Two render-shape questions also need a decision: how deep does the thread go (does a
counter that is itself countered recurse?), and how is an empty-reason counter displayed
(a peer record from a non-OpenLore client may carry no `reason` per ADR-015's
wire-optional asymmetry)?

## Decision

### 1. The `CounterThread` ADT — two arms, no aggregate arm

```rust
pub enum CounterThread {
    None,                                         // un-countered → render claim alone
    Countered { counters: Vec<CounterEntry> },    // flag + one item per counter
}
```

- `None` is the no-noise branch (no section, no empty-state line) — built from an empty
  `query_counter_claims` result.
- `Countered` carries a `Vec<CounterEntry>` — **one entry per signed counter**. There is
  **no aggregate/consensus variant in the type**, so a "disputed by N" row is
  un-representable (anti-merging is structural).
- The countered claim is built independently from `get_claim` and rendered UNCHANGED; the
  `CounterThread` is rendered SEPARATELY below it — there is no code path from a
  `CounterEntry` back into the claim's confidence (shown-never-applied is structural).

### 2. Depth-1 — the thread does NOT recurse

A `CounterEntry` shows the counter's author DID, its own CID (as a link to
`/claims/{counter_cid}`), and its reason. It does **NOT** render the counter's OWN
counters. If a counter is itself countered, the operator drills into it via the existing
`/claims/{counter_cid}` route, which renders ITS thread via the same one-level render.
No nested/recursive thread is built.

### 3. Empty-reason counter → explicit "no reason provided"

`CounterEntry.reason` is `Option<String>`. A `reason` that is absent or empty-after-trim
(an ADR-015 wire-optional peer record) renders an explicit **"no reason provided"** state
— the author DID and CID are still shown. Never a blank line, never a crash.

## Alternatives considered

| Question | Option | Verdict |
|---|---|---|
| Aggregate surface | a `CounterThread::Disputed { count }` summary arm | REJECTED — would make a "disputed by N" verdict representable, violating I-CT-3; anti-merging must be structural |
| Empty thread | render a "Counter-claims (0)" section always | REJECTED — adds noise to the common (un-countered) case (US-CT-003); the `None` arm renders nothing |
| Depth | recurse into each counter's own counters | REJECTED — deep nesting is an explicit non-goal (feature-delta out-of-scope); risks unbounded render + cycles; the CID link already affords drilling |
| Empty reason | skip the counter / render a blank line | REJECTED — silently dropping a counter hides attributed disagreement; a blank line looks like a render bug; "no reason provided" is explicit and honest |

## Consequences

- **Positive**: anti-merging + no-noise + shown-never-applied are enforced by the type,
  not by reviewer vigilance; the empty-reason and depth-1 edge cases are total at the
  type level; the thread renders inside the single `render_claim_detail_fragment` so
  htmx/no-JS parity is structural (I-CT-6); `render_confidence` stays single-site.
- **Negative**: a counter that is itself heavily countered requires a click to explore
  (acceptable — depth-1 is the deliberate scope; the link makes it one hop).
- **Enforcement**: shown-never-applied gold (confidence byte-identical with/without
  counters), anti-merging gold (two counters → two items, no consensus row), no-noise
  assertion (un-countered claim has no section), parity gold (fragment ≡ full-page swap
  region).
