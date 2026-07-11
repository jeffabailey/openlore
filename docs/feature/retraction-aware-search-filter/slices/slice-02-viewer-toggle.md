# Slice 02 — Viewer `?hide_retracted=1`: browser parity over the same pure predicate

> Release 2 · Story: US-RF-002 (user-visible) · Job: J-005 (sub-job J-005d)
> Persona: P-001 (Maria, node operator) · Estimate: ~1.5 days

## Goal

Bring the explicit hide to the read-only `/search` viewer as a `?hide_retracted=1` GET-param
toggle, invoking the SAME pure `appview-domain` predicate slice 01 shipped. Survivors render
with the honesty notice in both htmx shapes; the default (unticked) is byte-identical to
slice-08; read-only / loopback / offline-chrome / no-JS-full-page all preserved.

## Learning hypothesis

If Maria gets CLI-parity focus in the browser — hide retracted rows, see exactly how many
were hidden, restore in one click — WITHOUT any weakening of the read-only viewer guarantees,
then the reconciliation (D-1) generalizes cleanly across surfaces on a single pure predicate.

## IN scope

- Extend the slice-08 `/search` form with a "Hide retracted claims" control that sets
  `?hide_retracted=1` (OD-RF-2: checkbox default).
- On submit, run the slice-01 pure `appview-domain` predicate over the composed results;
  render survivors + a results-region notice with the hidden count (OD-RF-3).
- Full page without `HX-Request` + results-region fragment with it (slice-07 `Shape` fork);
  the notice appears in BOTH shapes.
- Empty-after-filter guided state ("untick to see them"); default (unticked) unchanged.

## OUT of scope

- Any write/sign/subscribe affordance — the toggle is a GET-param only (I-RF-6 / read-only).
- A second filter implementation — the viewer REUSES the slice-01 pure predicate (D-2).
- A browser filtered-`--share` link (deferred).
- Persisting the toggle state (D-7).

## Acceptance criteria (from US-RF-002 UAT)

- [ ] `/search?…&hide_retracted=1` (no `HX-Request`) serves a full page: survivors + hidden-count
      notice (I-RF-1/3).
- [ ] Without the param, render is identical to slice-08; retracted rows shown with annotation;
      no notice (I-RF-1).
- [ ] Same filtered submit WITH `HX-Request` returns only the results-region fragment,
      structurally identical to the full page's region — notice included (I-RF-6 / slice-07 parity).
- [ ] Every survivor carries `[verified]` + `author_did` + verbatim confidence; no merged row;
      order preserved (I-RF-2/8).
- [ ] Every result hidden → guided "untick to see them" state, not a blank region.
- [ ] The control is a GET-param toggle only — no write/sign/subscribe route, no key; an
      unreachable indexer degrades to the slice-08 calm message in both shapes (I-RF-6).
- [ ] `check-arch` stays green at 21 members (D-6).

## Dependencies

- slice 01 (US-RF-001) — the pure predicate + the CLI reconciliation validated first.
- slice-08 `/search` + `adapter-http-viewer` + `viewer-domain` render + slice-07 `Shape` fork +
  vendored htmx asset — all shipped.

## Estimate

~1.5 days: the toggle + notice + fragment/full-page parity + empty buffer are surface work over
a reused pure predicate and a reused render path.
