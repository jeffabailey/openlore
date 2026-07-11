# Slice 01 — CLI `--hide-retracted`: the whole I-AV-9 reconciliation on the primary surface

> Release 1 · Story: US-RF-001 (user-visible) · Job: J-005 (sub-job J-005d)
> Persona: P-002 (Rachel, researcher/tech lead) · Estimate: ~1 day

## Goal

The thinnest thread that carries the entire cardinal decision (D-1): a pure retraction
predicate + `openlore search … --hide-retracted` + the "N retracted claim(s) hidden"
honesty line + the empty-after-filter buffer + the default-unchanged regression guard —
over the already-shipped slice-05 search path. Proves an opt-in, non-destructive,
self-disclosing filter can live on a "never silently filter" surface without weakening
I-AV-9.

## Learning hypothesis

If Rachel can hide author-retracted claims with an explicit flag AND always see exactly
what was hidden AND get byte-identical output without the flag, then a filter is
reconcilable with I-AV-9 (nothing silently disappears) — and the pure-core + reuse
approach holds before the viewer inherits it. Settling OD-RF-1 (does the DTO distinguish
author-retraction from third-party counter?) against real indexer data is the load-bearing
learning.

## IN scope

- A NEW pure total function in `appview-domain` (indicative `partition_retracted(results,
  hide_retracted) -> (survivors, hidden_count)`), added to the slice-05 pure-core allowlist.
  Targets **soft-retracted (author-withdrawn) claims only** (D-3): a result is retracted iff
  its references graph carries a retraction-type counter whose author DID equals the result's
  author DID.
- Extend the `openlore search` verb (ADR-027) with a `--hide-retracted` boolean flag.
- The honesty footer line when ≥1 hidden ("N retracted claim(s) hidden … re-run without it").
- The empty-after-filter guided state ("all N were soft-retracted … re-run to see them").
- The default-unchanged guard: without the flag, output is byte-identical to slice-05.

## OUT of scope

- The viewer toggle (→ slice 02).
- Hiding third-party-countered claims (D-3 — never).
- Any index mutation / re-verify / re-rank / re-weight / persisted preference (I-RF-2/7).
- A `--share` that encodes the filter (deferred).

## Acceptance criteria (from US-RF-001 UAT)

- [ ] `--hide-retracted` removes every author-soft-retracted claim and no other (D-3).
- [ ] Without the flag, output is byte-identical to the pre-feature search; retracted claims
      shown with annotation (I-RF-1).
- [ ] ≥1 hidden → a footer states the exact count + how to re-run without the flag (I-RF-3).
- [ ] Filter active but nothing matched → no misleading "hidden" line (D-4).
- [ ] Every result hidden → guided "all N soft-retracted / re-run to see them" state, not a
      bare empty result.
- [ ] Survivors keep original order + verbatim confidence; each still `[verified]` + attributed;
      no merged row (I-RF-2/8, D-5).
- [ ] The filter is a pure `appview-domain` decision; the index is not re-queried (I-RF-5/D-2).
- [ ] `check-arch` stays green at 21 members (D-6).

## @property criterion

- Hiding never re-orders or re-weights survivors: survivor order + each survivor's confidence
  are identical to the unfiltered run (D-5).

## Dependencies

- slice-05 `appview-domain` (`compose_results`) + `adapter-index-query` + `IndexQueryPort` +
  `SearchResultDto.references` (DV-5) + `openlore search` verb (ADR-027) — all shipped.
- **OD-RF-1 (settle first)**: confirm the shipped DTO distinguishes author-retraction from a
  third-party counter; if not, a minimal additive ingest/compose marker is needed (Risk R-1).

## Estimate

~1 day: the pure predicate + the count are small; the flag + footer + empty buffer are thin
wiring; verify/compose are inherited. Add up to ~0.5 day if OD-RF-1 forces an ingest marker.
