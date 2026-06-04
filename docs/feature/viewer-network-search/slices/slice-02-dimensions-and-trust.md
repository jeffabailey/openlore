# Slice 02 — Dimensions + trust + graceful degradation

> Release 2 · Stories: US-NS-003 (contributor/subject dimensions) + US-NS-004 (trust + degradation UX)
> Job: J-005 · Persona: P-001 (Maria, node operator) · Estimate: ~3.5 days

## Goal

Complete the three search dimensions in the browser (contributor + subject, on top
of the skeleton's object), and make the trust + failure surfaces honest: the
`[verified]` + public-data framing is visible up front, the counter-annotation is
shown (never applied), and an unreachable/unconfigured indexer renders a
plain-language guidance message instead of a crash or a leaked transport error.

## IN scope

- Contributor dimension (`/search?contributor=<handle-or-did>`) and subject
  dimension (`/search?subject=<project>`) in the same `/search` form + render
  (US-NS-003), each preserving per-author attribution.
- The contributor footer honesty line ("one developer's reasoning trail, not a
  community consensus") and the subject survey grouped-by-author (no consensus row).
- The up-front public-data framing on the `/search` page (inherits slice-05 WD-105 /
  KPI-AV-5: discovery indexes only PUBLIC signed claims, verified before indexing).
- `counter_annotation` SHOWN on a row, never applied (anti-merging; WD-NS-5).
- Graceful degradation (US-NS-004): an unreachable OR unconfigured indexer
  (`OPENLORE_INDEXER_URL` unset / connection fails) renders a fixed plain-language
  message (mirror the slice-07 `/scrape` `NetworkDown` unit-variant render) — in BOTH
  the fragment and full-page shapes — pointing Maria at the local store views; no
  crash, no leaked HTTP status / "connection refused" / raw URL.
- The discovery → federation follow affordance shown as GUIDANCE TEXT only
  (`openlore peer add <did>` for an unfollowed author) — never an executable control
  (WD-NS-3).

## OUT of scope

- Executing a follow / subscribe from the browser (stays a CLI action — WD-NS-3).
- A `--share`-equivalent shareable browser link (deferred; CLI `--share` already
  realizes KPI-AV-6 in slice-05).
- Persisting search results or relationship state (WD-NS-7).

## Learning hypothesis

If Maria can search all three dimensions and TRUST what she sees (verified marker +
public-data framing + honest degradation), then the browser surface is a complete
discovery front-door — and the failure modes never erode the read-only trust that
slices 06/07 established.

## Acceptance criteria (from US-NS-003/004 UAT)

- [ ] Contributor search renders one author's trail under a single `author_did` with
      the "one developer's reasoning trail, not a community consensus" footer.
- [ ] Subject search renders N authors' rows grouped BY AUTHOR (no "the network
      thinks X" merged row).
- [ ] A row carrying a counter-annotation SHOWS it; the annotation is not applied
      (the claim is not merged/over-ridden).
- [ ] The `/search` page shows the public-data framing before results.
- [ ] An unreachable/unconfigured indexer renders the fixed plain-language guidance
      (no crash, no leaked transport internals) in both fragment and full-page shapes.
- [ ] An unfollowed-author row shows the `openlore peer add <did>` guidance text and
      NO executable follow control.

## Dependencies

- Slice 01 (the `/search` route + indexer-query port + render fork).
- slice-05 contributor/subject dimension query surface + the handle→DID resolution
  convention + the `degrade_to_local_only` precedent (REUSE).
- slice-07 `NetworkDown` ADT + leak-absence discipline (precedent to mirror).

## Estimate

~3.5 days (US-NS-003 ~2d + US-NS-004 ~1.5d).
