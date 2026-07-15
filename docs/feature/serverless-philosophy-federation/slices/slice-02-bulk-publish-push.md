# Slice 02 — Publish my whole local graph to my instance (bulk push)

> Release 1 · Story: US-SF-003 (J-008) · Persona: P-001 (Maria, publisher/sharer hat)
> Depends on: slice-01 · Estimate: ~1 day

## Goal

Generalize the validated one-claim round-trip (slice-01) to the WHOLE local graph:
`openlore publish push` diffs local vs instance and pushes only new claims — additive, idempotent,
per-claim CID-verified — so the instance faithfully mirrors local without duplicates or divergence.
This is the value that populates the public card (slice-04) with real data.

## Learning hypothesis

If Maria can push her entire local graph once, re-push cheaply (idempotent), and resume an interrupted
push with no duplicates — each claim CID-verified on the instance — then her instance is a faithful,
verifiable mirror of local, and pointing her card and peers at it is justified.

## IN scope

- `openlore publish push` diffs local vs instance (by CID set) and pushes only claims not already
  present (additive, idempotent).
- Per-claim CID verification on the instance (KPI-SF-1) across the whole batch.
- Skip-already-present reporting; interrupted-then-resumed push creates no duplicates.
- Local store never modified by a push (D-6).

## OUT of scope

- Pull-reconcile / fresh-machine rebuild (→ slice-03); the card UI (→ slice-04); cross-instance pull
  (→ slice-05).
- Selective/partial publish (publish everything that is not yet on the instance); retract propagation
  (soft-retract counter-claims push like any other claim — no special path here).

## Acceptance criteria (from US-SF-003 UAT)

- [ ] `publish push` sends only claims not already on the instance (additive, idempotent).
- [ ] Each pushed claim's CID recomputed on the instance matches its local CID (KPI-SF-1).
- [ ] Re-push (all present) pushes 0, skips all; no duplicates.
- [ ] An interrupted push, re-run, pushes only the remainder; no duplicates.
- [ ] The local store is never modified by a push (D-6).

## Dependencies

- slice-01 (a proven one-claim push/pull pipe + registered instance).
- The slice-01 transport (OD-SF-1).

## Estimate

~1 day: the diff/idempotency logic + batch CID verification over the slice-01 transport; the
re-push/resume tests.
