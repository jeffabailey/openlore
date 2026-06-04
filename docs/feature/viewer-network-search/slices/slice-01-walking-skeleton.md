# Slice 01 — Walking Skeleton: verified network search by philosophy, in the browser

> Release 1 (walking skeleton) · Stories: US-NS-001 (infra) + US-NS-002 (user-visible)
> Job: J-005 · Persona: P-001 (Maria, node operator) · Estimate: ~3.5 days

## Goal

The thinnest end-to-end thread: from the read-only `openlore ui` viewer, Maria
opens `/search`, picks the philosophy dimension, enters a value, and sees the
**verified + attributed** network result rows rendered as HTML — served from a
reachable slice-05 indexer, with an htmx fragment swap. This proves the new
outbound capability (viewer → indexer HTTP → verified rows → HTML) works and
preserves every inherited invariant.

## IN scope

- A NEW `/search` route in the viewer: a form (dimension selector + value) +
  a results region (US-NS-002).
- A NEW indexer-query effect in the viewer process (the outbound capability;
  OD-NS-1 — reuse the slice-05 `adapter-index-query` client?).
- Object dimension only for the skeleton (`/search?object=<nsid>`).
- Per-author-attributed verified rows rendered via the slice-05 `appview-domain`
  composition (`[verified]` marker + `author_did` + verbatim confidence; no merged
  row). (US-NS-001 ingest/verify path is inherited from slice-05 — the viewer does
  not re-verify; it renders the already-verified index results.)
- The `HX-Request` fragment / full-page fork (slice-07 `Shape`), serving the same
  results region in both shapes.

## OUT of scope

- Contributor / subject dimensions (→ slice 02).
- Unreachable/unconfigured graceful-degradation UX polish (→ slice 02; the skeleton
  assumes a reachable indexer, but MUST NOT crash if unreachable).
- Any write/sign/subscribe affordance (the viewer stays read-only — WD-NS-3).
- Persisting any search result (WD-NS-7).
- A standalone web AppView app.

## Learning hypothesis

If Maria can see verified, attributed network results in her browser viewer (not
just the CLI), then the browser becomes a viable discovery surface — and the
read-only + verified + attributed + progressive-enhancement invariants all hold on
a network-READ surface (not just the local-store reads of slice 06).

## Acceptance criteria (from US-NS-001/002 UAT)

- [ ] `GET /search` (no `HX-Request`) serves a complete full page: a dimension
      form + an (empty until submitted) results region.
- [ ] Submitting the philosophy dimension queries the configured indexer and renders
      per-author groups, each row showing `author_did`, the `[verified]` marker, and
      verbatim confidence (`0.90`, not `0.9`/`90%`).
- [ ] The SAME submit WITH `HX-Request` returns ONLY the results-region fragment
      (no chrome), structurally identical to the full page's results region.
- [ ] Two identical-content claims by different authors render as two rows (no merge).
- [ ] The viewer persists nothing and exposes no sign/subscribe control on the page.

## Dependencies

- slice-05 `openlore-indexer` + `appview-domain` result types + the
  `org.openlore.appview.searchClaims` query surface (REUSE).
- slice-06 `adapter-http-viewer` route table + `ViewerServer` + `html_ok`.
- slice-07 `Shape::from_request` fork + page=chrome+fragment pattern.

## Estimate

~3.5 days (US-NS-001 infra ~1.5d + US-NS-002 ~2d). The bulk is the new
indexer-query port + the render reuse; verification + composition are inherited.
