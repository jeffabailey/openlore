# Journey (visual): counter flags on the graph surfaces (slice-13)

> Persona: **P-001 "Maria"** (counter-claim-scanner hat, extended to the federated +
> traversal surfaces). Brownfield DELTA on slices 06/07/10/11/12. The flag, the read
> (`counter_presence_for`), and the `page = chrome + fragment` pattern are all REUSED
> from slice-12 — this journey shows the SAME marker landing on two more surfaces.

## Emotional arc

```
Scanning peers / traversing the graph   →   Spotting a contested row/edge   →   Confident triage
  Curious + slightly wary                    Relief ("I can SEE which is             Calm ("I scan the same way I
  ("which of these did someone               contested without opening each")        always did; the contested
   already push back on?")                    + trust ("nothing moved/re-grouped")    ones are simply marked")
```

Pattern: **Problem Relief** (slightly-wary → relieved → confident). The peak tension is
the moment BEFORE the flag exists — "do I have to open every peer claim / every edge to
find disagreement?" The flag resolves it WITHOUT a jarring transition: the surface looks
byte-identical to before, plus a neutral marker where (and only where) a counter exists.

## Surface 1 — `/peer-claims` (US-CF-002)

```
+-- GET /peer-claims --------------------------------------------------------------+
| Peer Claims  (read-only view of claims federated from your peers)                |
|                                                                                  |
| Subject              Predicate            Object              Conf  Peer origin   CID            |
| ------------------------------------------------------------------------------------------------ |
| github:rust.../cargo embodiesPhilosophy  dependency-pinning  0.88  did:plc:t0bi  bafy...t0bi [Countered]   <- flagged: a peer claim Maria countered
|                                                  (verbatim 0.88 — UNCHANGED by the flag)         |  (link -> /claims/bafy...t0bi thread)
| github:tokio.../tokio embodiesPhilosophy  async-first        0.79  did:plc:rach  bafy...rach    <- un-countered: NO marker (no noise)
| github:.../serde     embodiesPhilosophy  zero-copy           0.83  did:plc:t0bi  bafy...dup [Countered]    <- countered by TWO authors -> ONE marker (presence-only)
| ------------------------------------------------------------------------------------------------ |
|  [Prev]   1–12 of 27   [Next]   <- paging + order UNCHANGED (composed_at DESC); flag never re-orders/filters |
+----------------------------------------------------------------------------------+
```

`[Countered]` = the slice-11/12 `COUNTERED_PRESENCE_FLAG`, rendered as
`<a href="/claims/{cid}">Countered</a>` — REUSED verbatim. The peer-origin column +
confidence are byte-identical to slice-06.

## Surface 2 — `/project` + `/philosophy` traversal edges (US-CF-003)

```
+-- GET /project?subject=github:rust-lang/cargo ------------------------------------+
| Project survey: github:rust-lang/cargo                                            |
|                                                                                  |
|  Philosophy: dependency-pinning   (<- group key, traversal link to /philosophy)   |
|    did:plc:t0bi   0.88  [well-evidenced]  bafy...t0bi  [Countered]   <- flagged edge -> /claims/bafy...t0bi
|    did:plc:maria  0.91  [triangulated]    bafy...mr1                  <- un-countered edge: NO marker
|                                                                                  |
|  Philosophy: memory-safety        (<- group key)                                  |
|    did:plc:rach   0.84  [well-evidenced]  bafy...rach                 <- un-countered                |
|    did:plc:t0bi   0.77  [weighted]        bafy...dup  [Countered]     <- flagged edge (countered twice -> ONE marker) |
|                                                                                  |
|  Contributors: did:plc:t0bi, did:plc:maria, did:plc:rach  (deduped — UNCHANGED)   |
+----------------------------------------------------------------------------------+
  ^ grouping (group_by), group order, edge order within a group, the deduped contributor
    list, and every confidence + bucket are BYTE-IDENTICAL to slice-10 (markers elided).
    The flag NEVER re-groups, re-orders, or re-weights — it annotates the edge in place.
```

`/philosophy?object=<uri>` is the symmetric mirror (groups by subject) and shares the
SAME `EdgeRow` render — so the flag arm is added ONCE and serves both routes.

## What stays invariant (the load-bearing "shown, never applied")

- The flag is a NEUTRAL presence marker — never "disputed", "refuted", "false", or a count.
- The flagged row/edge renders VERBATIM: confidence, weight/bucket, peer origin, group,
  edge order, position — all unchanged. With markers elided, each surface is byte-identical
  to its pre-flag (slice-06 / slice-10) render.
- One aggregate `counter_presence_for` query per render — never N+1 (REUSED from slice-12).
- LOCAL + offline: no network on these routes; only the vendored local htmx asset.
- htmx fragment and no-JS full page render the flag identically (parity by construction).

## Out of this journey

- `/score` (the WeightedView contribution rows) — deferred to slice-14 (the weight-misread
  surface gets its own anti-misread copy + the sum-to-weight cardinal). `/search` already has
  its own slice-08 annotation. `/claims` shipped in slice-12.
