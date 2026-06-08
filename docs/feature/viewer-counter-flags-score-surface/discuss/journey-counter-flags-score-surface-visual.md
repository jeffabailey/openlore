# Journey (visual): counter flag on the contributor-score surface (slice-14)

> Persona: **P-001 "Maria"** (counter-claim-scanner hat, extended to the LAST surface — the
> `/score` contributor-scoring breakdown). Brownfield DELTA on slices 09/11/12/13. The flag, the
> read (`counter_presence_for`), the render (`render_countered_link`), and the
> `page = chrome + fragment` pattern are all REUSED from slices 12/13 — this journey shows the
> SAME marker landing on the scoring surface, where it must be provably ORTHOGONAL to the score.

## Emotional arc

```
Reading a contributor's score   →   Spotting a contested contribution   →   Confident, calibrated trust
  Curious + slightly wary             Relief ("I can SEE which contributions       Calm ("the score is unchanged;
  ("can I trust this score? which       drew disagreement, without opening each")    contested ones are simply marked
   of these were pushed back on?")     + reassurance ("the counter didn't            for me to go judge — being
                                        secretly lower the score; it's shown          countered is NOT a deduction")
                                        for me to judge")
```

Pattern: **Problem Relief** (slightly-wary → relieved → calibrated trust). The peak tension is
TWO-fold here (unlike slices 12/13): (1) "do I have to open every contribution to find
disagreement?" AND (2) "does this 'Countered' tag mean the counter LOWERED the score?" The flag
resolves BOTH without a jarring transition: the breakdown looks byte-identical to slice-09 (same
weights, subtotals, ranking, order), plus a neutral marker where a counter exists, plus plain
copy that makes the orthogonality unmistakable.

## Surface — `/score?contributor=<did>` (US-CF-002)

```
+-- GET /score?contributor=did:plc:t0bi -------------------------------------------------------+
| Contributor adherence score: did:plc:t0bi    (read-only, computed LOCALLY over your store)    |
|                                                                                              |
|  github:rust-lang/cargo — dependency-pinning                                                 |
|  Weight: 1.42  [well-evidenced]                                                              |
|  ┌────────────┬────────────┬────────────┬────────────┬───────────────┬──────────┐            |
|  │ Author     │ CID        │ Confidence │ Author bon │ Triangulation │ Subtotal │            |
|  ├────────────┼────────────┼────────────┼────────────┼───────────────┼──────────┤            |
|  │ did:plc:t0bi│ bafy...t0bi│ 0.88       │ 0.10       │ 0.05          │ 1.03  [Countered] -> /claims/bafy...t0bi
|  │ did:plc:mr  │ bafy...mr1 │ 0.91       │ 0.00       │ 0.00          │ 0.39             (no marker)
|  └────────────┴────────────┴────────────┴────────────┴───────────────┴──────────┘            |
|              ^ subtotals 1.03 + 0.39 = 1.42 == Weight  (sum-to-weight CARDINAL, slice-09)     |
|                the [Countered] flag changed NEITHER subtotal NOR the weight                   |
|                                                                                              |
|  ℹ "Countered" = this contribution's claim has been disagreed with elsewhere — shown for you  |
|     to judge. It does NOT lower this contributor's score. (anti-misread legend)               |
|                                                                                              |
|  github:tokio-rs/tokio — async-first                                                         |
|  Weight: 0.79  [SPARSE — based on 1 claim by 1 author; treat as a lead, not a conclusion]     |
|    did:plc:t0bi  bafy...dup  0.77 ...  0.77  [Countered] -> thread (countered TWICE -> ONE marker)|
|              ^ rank, weight, [SPARSE] honesty line all byte-identical to slice-09             |
+----------------------------------------------------------------------------------------------+
  ^ every weight, confidence, bonus, subtotal, headline total, bucket, the pairing RANKING, and the
    contribution ROW ORDER are BYTE-IDENTICAL to slice-09 (markers + legend elided).
    The flag is SHOWN, never APPLIED — a countered claim contributes its FULL original weight.
```

`[Countered]` = the slice-11/12/13 `COUNTERED_PRESENCE_FLAG`, rendered via the slice-13-unified
`render_countered_link(cid, is_countered)` as `<a href="/claims/{cid}">Countered</a>` — REUSED
verbatim. The author DID, CID, confidence, bonuses, and subtotal columns are byte-identical to
slice-09.

## The load-bearing distinction from slices 12/13 (scoring semantics)

On `/score` the flag sits BESIDE scoring math. Two guarantees that slices 12/13 did not need:

1. **Sum-to-weight preserved (slice-09 CARDINAL).** The per-claim subtotals still sum to the
   displayed pairing weight — both project the SAME unchanged `WeightedPairing`. The flag adds a
   render-only annotation and changes no `WeightedPairing`.
2. **Score-orthogonal + anti-misread.** The counter is SHOWN, never APPLIED. A countered claim
   keeps its FULL original weight; two contributions with identical confidence/bonuses carry the
   identical subtotal whether or not one is countered. A short neutral legend on the breakdown
   states this in plain language so the marker is never misread as a deduction.

## What stays invariant (the load-bearing "shown, never applied")

- The flag is a NEUTRAL presence marker — never "disputed", "refuted", "false", a count, or a
  "disputed score".
- The flagged contribution renders VERBATIM: confidence, both bonuses, subtotal, the pairing
  weight, the `[SPARSE]` honesty line, the bucket, the rank, position — all unchanged. With markers
  + legend elided, the surface is byte-identical to its slice-09 render. **The subtotals still sum
  to the weight.**
- One aggregate `counter_presence_for` query per render — never N+1 (REUSED from slice-12).
- LOCAL + offline: no network on the `/score` route (slice-09 WD-CS-8); only the vendored local
  htmx asset.
- htmx fragment and no-JS full page render the flag identically (parity by construction).

## Out of this journey

- The slices 12/13 surfaces (`/claims`, `/peer-claims`, `/project`, `/philosophy`) — already
  flagged. The slice-11 `/claims/{cid}` thread + the slice-08 `/search` annotation — already exist.
  Applying/subtracting the counter from the score — explicitly OUT (the counter is shown, never
  applied). Recomputing any scoring math — OUT (the viewer projects the reused `WeightedView`).
