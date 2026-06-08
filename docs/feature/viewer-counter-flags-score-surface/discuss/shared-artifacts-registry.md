# Shared-artifact registry: viewer-counter-flags-score-surface (slice-14)

> Every `${variable}` rendered by the slice-14 flag traces to a single source of truth.
> The defining property of this slice is that ALL of its load-bearing artifacts are REUSED from
> prior slices — slice-14 introduces NO new read method, NO new flag string, NO new render fn.
> The ONE genuinely new artifact is the anti-misread legend copy (a render-only constant).

## Registry

```yaml
shared_artifacts:

  counter_presence_for:
    source_of_truth: "crates/ports/src/store_read.rs (StoreReadPort::counter_presence_for, slice-12 / ADR-048)"
    consumers:
      - "GET /claims (slice-12 — already wired)"
      - "GET /peer-claims, /project, /philosophy (slice-13 — already wired)"
      - "GET /score (slice-14 — US-CF-001 wiring)"
    owner: "slice-12 (viewer-counter-claim-list-flags) — REUSED unchanged by slice-13 + slice-14"
    integration_risk: "HIGH — if the /score handler calls it per-contribution instead of once per page, the N+1 guard (C-8) is violated, and a breakdown can have many contributions across many pairings. Mitigation: US-CF-001 AC requires ONE call per render, flattening all contribution CIDs across all pairings."
    validation: "Behavioral assertion (query count invariant to contribution/pairing count) through the real openlore ui subprocess + the inherited slice-12 adapter-duckdb N+1 property test. NO new read method added (asserted by xtask check-arch + the unchanged StoreReadPort signature)."

  COUNTERED_PRESENCE_FLAG:
    source_of_truth: "crates/viewer-domain/src/lib.rs (COUNTERED_PRESENCE_FLAG = \"Countered\", slice-11, line ~679)"
    consumers:
      - "/claims/{cid} thread (slice-11)"
      - "/claims list flag (slice-12)"
      - "/peer-claims + /project + /philosophy flags (slice-13)"
      - "/score contribution-row flag (slice-14)"
    owner: "slice-11 (viewer-counter-claim-threads) — REUSED verbatim by slices 12/13/14"
    integration_risk: "MEDIUM — a new flag string would fork the neutral-marker vocabulary and risk a non-vetted verdict word, especially dangerous beside a weight. Mitigation: slice-14 introduces NO new string; the marker is the SAME constant on every surface."
    validation: "The slice-11 neutral-flag verdict-word blocklist (no 'disputed'/'refuted'/'false') reused on /score; a single source constant means one mutation site."

  render_countered_link:
    source_of_truth: "crates/viewer-domain/src/lib.rs (render_countered_link(cid, is_countered) — slice-13-unified SSOT that unified the list/peer/edge renders)"
    consumers:
      - "/claims list flag (slice-12, via the unified render)"
      - "/peer-claims + edge flags (slice-13)"
      - "/score contribution-row flag (slice-14)"
    owner: "slice-13 (viewer-counter-flags-graph-surfaces) — the unified flag render; REUSED verbatim by slice-14"
    integration_risk: "LOW — the render is a total function of (cid, is_countered) producing the <a href=\"/claims/{cid}\">Countered</a> link or nothing; reusing it guarantees identical flag shape on /score."
    validation: "render_score_breakdown calls render_countered_link(contribution.cid.0, is_countered) — no new render fn; AC-002-MARKER asserts the REUSED render."

  claim_thread_link:
    source_of_truth: "GET /claims/{cid} (slice-11 detail route)"
    consumers: ["all slice-12/13 flags", "/score flag (slice-14)"]
    owner: "slice-11"
    integration_risk: "LOW — the link target is the well-defined slice-11 route; the CID comes from the contribution being flagged."
    validation: "Each flag is a render-only <a href=\"/claims/{cid}\">Countered</a> one-hop link; AC asserts the href targets the flagged contribution's own CID."

  from_row_with_presence_projection:
    source_of_truth: "crates/viewer-domain/src/lib.rs (ClaimRowView::from_row_with_presence, slice-12) — the PATTERN slice-14 mirrors"
    consumers:
      - "ClaimRowView.is_countered (slice-12)"
      - "PeerClaimRowView / EdgeRow is_countered (slice-13)"
      - "/score contribution-row is_countered (slice-14 — same pattern, effect shell sets it)"
    owner: "slice-12 — the projection pattern (effect shell sets is_countered from presence.contains(&cid); the pure render stays a TOTAL fn of (ScoreState, presence))"
    integration_risk: "MEDIUM — the is_countered flag MUST be set in the effect shell (keeping the pure render total), not read in the pure core. Mitigation: mirror the slice-12/13 from_row_with_presence exactly; viewer-domain stays pure."
    validation: "viewer-domain purity (no I/O imports); the flag carries every other display field (confidence, bonuses, subtotal, weight) through UNCHANGED (the additive-only property, mirrored)."

  WeightedPairing_sum_to_weight:
    source_of_truth: "scoring::WeightedView / WeightedPairing (slice-04); projected by render_score_pairing + render_score_breakdown (crates/viewer-domain/src/lib.rs ~1940/1968, slice-09)"
    consumers:
      - "/score headline pairing weight (render_score_pairing)"
      - "/score per-claim subtotals (render_score_breakdown — one row per Contribution)"
    owner: "slice-04 (scoring) — the math; slice-09 (viewer-contributor-scoring) — the projection + the sum-to-weight CARDINAL"
    integration_risk: "HIGH (slice-14's load-bearing risk) — the flag sits beside the weight + subtotals; a naive change could alter a subtotal or break sum-to-weight, or the copy could imply the counter lowered the weight. Mitigation: the flag is a render-only annotation that changes NO WeightedPairing (sum-to-weight holds BY CONSTRUCTION since subtotals + weight both project the SAME unchanged pairing — slice-09 doc-comment ~1935); AC-SCORE-SUMWEIGHT + AC-SCORE-BYTEID + AC-SCORE-ANTIMISREAD enforce it."
    validation: "AC-SCORE-SUMWEIGHT (subtotals sum to weight on a flagged breakdown) + AC-SCORE-BYTEID (byte-identity vs slice-09 with markers elided) + AC-SCORE-ANTIMISREAD (copy is orthogonal). The slice-09 transparency-by-construction unit test pins it; slice-14 adds the flagged-render variant."

  Contribution_cid:
    source_of_truth: "scoring::Contribution.cid: Cid (crates/scoring/src/explain.rs); rendered at render_score_breakdown row (contribution.cid.0)"
    consumers:
      - "the /claims/{cid} flag link target on a flagged contribution"
      - "the page CID set the /score handler flattens into counter_presence_for"
    owner: "slice-04 (scoring) — every Contribution carries its claim CID"
    integration_risk: "LOW — every contribution maps to exactly one signed claim, so the CID is always present."
    validation: "The handler flattens Contribution.cid across all WeightedPairings into ONE counter_presence_for call (US-CF-001 AC)."

  anti_misread_legend:
    source_of_truth: "NEW slice-14 render-only constant in crates/viewer-domain/src/lib.rs (DESIGN owns exact wording within the AC)"
    consumers: ["/score breakdown render (US-CF-002)"]
    owner: "slice-14 — the ONLY genuinely new artifact; a short neutral legend on the breakdown"
    integration_risk: "MEDIUM — wrong wording could re-introduce the misread (imply a deduction). Mitigation: AC-SCORE-ANTIMISREAD constrains it (orthogonal to the score; never 'penalty'/'deduction'/'lowered'/'disputed score'); reuse the slice-11 verdict-word blocklist."
    validation: "AC-SCORE-ANTIMISREAD asserts the legend states the counter is shown for the reader to judge and does NOT lower the score, and never uses a deduction/verdict word."
```

## Validation questions (answered)

- **Does every `${variable}` in the mockups have a documented source?** Yes — the marker string
  (`COUNTERED_PRESENCE_FLAG`), the flag render (`render_countered_link`), the presence read
  (`counter_presence_for`), the link target (`/claims/{cid}`), the contribution CID
  (`Contribution.cid`), and the weight/subtotal host (`WeightedPairing`) all trace to a single
  slice-source above. The only NEW artifact is the anti-misread legend (slice-14).
- **If the marker string changes, would all consumers update?** Yes — it is ONE constant consumed
  by every surface; one mutation site.
- **Are there hardcoded values that should reference a shared artifact?** No — slice-14 adds NO new
  read, NO new flag string, NO new render fn; it REUSES the slice-11/12/13 constants + read + render.
- **Do any two surfaces display the same data from different sources?** No — the presence truth
  comes from the SAME `counter_presence_for` read on every surface; the marker from the SAME
  constant + the SAME `render_countered_link`. The weight/subtotal come from the SAME unchanged
  `WeightedPairing` slice-09 already projects.

## Integration risk summary

The single highest integration risk is **the sum-to-weight / score-orthogonality regression**: the
flag sits beside a weight, a confidence, bonuses, and a subtotal inside a ranked breakdown whose
subtotals sum to a headline weight. A naive implementation could alter a subtotal, break
sum-to-weight, re-rank, or — via copy — imply the counter lowered the score. Mitigated by C-9 (the
slice-14 CARDINAL) + AC-SCORE-SUMWEIGHT (subtotals still sum to weight on a flagged breakdown) +
AC-SCORE-BYTEID (byte-identity vs the slice-09 baseline with markers elided — the slice-12/13
baseline+marker-elision tactic) + AC-SCORE-ANTIMISREAD (the legend is orthogonal). The second risk
is the **N+1 regression** on a breakdown with many contributions, mitigated by US-CF-001's AC
(flatten all contribution CIDs across all pairings into ONE call) + the inherited slice-12 N+1
guard + a behavioral query-count test.
