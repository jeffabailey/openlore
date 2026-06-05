# Slice 01 — Walking Skeleton: a contributor's transparent score + breakdown, in the browser

> Release 1 (walking skeleton) · Stories: US-CS-001 (infra) + US-CS-002 (user-visible)
> Job: J-002 (esp. J-002c) · Persona: P-001 (Maria, node operator) · Estimate: ~4 days

## Goal

The thinnest end-to-end thread: from the read-only `openlore ui` viewer, Maria opens
`GET /score?contributor=<did>`, and sees the contributor's **transparent adherence
score WITH its per-claim breakdown** rendered as HTML — computed over the LOCAL graph by
the reused slice-04 pure scorer, with an htmx fragment swap. This proves the new
capability (viewer → local graph read → pure scorer → HTML) works and preserves every
inherited invariant — critically, that the BREAKDOWN ships with the number (J-002c),
not as later polish.

## IN scope

- A NEW `GET /score?contributor=<did>` route in the viewer: a contributor form +
  a score region (US-CS-002).
- A NEW local contributor-scoring read effect in the viewer process (the capability;
  OD-CS-2 — reuse the slice-04 `query_attributed_for_scoring(ByContributor)` /
  `query_by_contributor` feed read over the viewer's read-only store?). LOCAL + offline.
- Run the slice-04 PURE `scoring::score(&feed, &ScoringConfig::DEFAULT)` — REUSED
  verbatim; the viewer reimplements no formula.
- Render the ranked `WeightedView`: per subject pairing, the adherence weight + its
  `WeightBucket` label, AND the per-claim breakdown (each `Contribution`'s author DID +
  cid + verbatim base confidence + applied bonuses + subtotal), with a running sum equal
  to the displayed weight (reproduce-by-hand).
- Anti-merging visible: conflicting/identical-subject claims by different authors render
  as separate contributions under their own author DIDs (no merge).
- The `HX-Request` fragment / full-page fork (slice-07 `Shape`), serving the same score
  region in both shapes.

## OUT of scope

- The full `[SPARSE]` honesty surface + verbatim-edge hardening + empty/no-claims state
  (→ slice 02; the skeleton assumes a contributor WITH claims, but the pure core's
  `WeightBucket` is already projected so a sparse pairing is labelled, not crashed).
- Object-dimension `--weighted` / `--traverse` browser views (inherited CLI surfaces;
  deferred).
- Any write/sign affordance (the viewer stays read-only — WD-CS-3).
- Persisting any derived score (WD-CS-10 / WD-72).
- Reimplementing the scoring math (WD-CS-6).
- A standalone web AppView app.

## Learning hypothesis

If Maria can see a contributor's transparent adherence score WITH its auditable per-claim
breakdown in her browser viewer (not just the CLI), then the browser becomes a viable
defensible-scoring surface — and the read-only + transparent-not-opaque + local-first +
progressive-enhancement invariants all hold on a local-read + pure-compute surface (not
just the plain store reads of slice 06 or the network read of slice 08).

## Acceptance criteria (from US-CS-001/002 UAT)

- [ ] `GET /score?contributor=<did>` (no `HX-Request`) serves a complete full page: a
      contributor form + a score region.
- [ ] The score region renders each subject pairing's adherence weight + `WeightBucket`
      label AND a per-claim breakdown naming every contribution's author DID + cid + the
      verbatim base confidence (`0.86`, not `0.9`/`86%`) + applied bonuses + subtotal.
- [ ] The running sum of a pairing's per-claim subtotals equals its displayed weight
      (reproduce-by-hand; KPI-GRAPH-3).
- [ ] No score is rendered without its component breakdown (no opaque number; I-CS-2).
- [ ] Conflicting/identical-subject claims by different authors render as separate
      contributions under their own author DIDs (no merge; KPI-GRAPH-2).
- [ ] The SAME request WITH `HX-Request` returns ONLY the `#score-results` fragment (no
      chrome), structurally identical to the full page's score region.
- [ ] The viewer persists nothing, makes no network call, and exposes no sign control on
      the page (read-only + local-first + key-less).

## Dependencies

- slice-04 `scoring` crate (`score` + `ScoringConfig::DEFAULT` + `WeightedView` +
  `WeightedPairing` + `WeightBucket` + `Contribution`) + the attributed-scoring-feed read
  contract (`query_attributed_for_scoring` / `query_by_contributor`) (REUSE).
- slice-06 `adapter-http-viewer` route table + `ViewerServer` + `StoreReadPort` +
  `html_ok`.
- slice-07 `Shape::from_request` fork + page=chrome+fragment pattern.
- slice-08 `/search` route precedent (a new GET surface forked by `Shape` + the
  new-viewer-process-read-port capability-boundary discipline).

## Estimate

~4 days (US-CS-001 infra ~1.5d + US-CS-002 ~2.5d). The bulk is the local-scoring read
port + the HTML projection of `WeightedView`/`Contribution`; the score math + the bucket
decision are inherited (the pure core).
</content>
