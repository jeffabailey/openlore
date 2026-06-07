# Shared-artifact registry: viewer-counter-flags-graph-surfaces (slice-13)

> Every `${variable}` rendered by the slice-13 flag traces to a single source of truth.
> The defining property of this slice is that ALL of its load-bearing artifacts are
> REUSED from prior slices — slice-13 introduces NO new read method and NO new flag string.

## Registry

```yaml
shared_artifacts:

  counter_presence_for:
    source_of_truth: "crates/ports/src/store_read.rs (StoreReadPort::counter_presence_for, slice-12 / ADR-048, lines ~360-384)"
    consumers:
      - "GET /claims (slice-12 — already wired)"
      - "GET /peer-claims (slice-13 — US-CF-001 wiring)"
      - "GET /project (slice-13 — US-CF-001 wiring)"
      - "GET /philosophy (slice-13 — US-CF-001 wiring)"
    owner: "slice-12 (viewer-counter-claim-list-flags) — REUSED unchanged by slice-13"
    integration_risk: "HIGH — if a slice-13 handler calls it per-row/per-edge instead of once per page, the N+1 guard (I-CF-8) is violated. Mitigation: US-CF-001 AC requires ONE call per render, flattening all edge CIDs across groups."
    validation: "Behavioral assertion (query count invariant to row/edge/group count) through the real openlore ui subprocess + the inherited slice-12 adapter-duckdb N+1 property test. NO new read method added (asserted by xtask check-arch + the unchanged StoreReadPort signature)."

  COUNTERED_PRESENCE_FLAG:
    source_of_truth: "crates/viewer-domain/src/lib.rs (COUNTERED_PRESENCE_FLAG = \"Countered\", slice-11, line ~679)"
    consumers:
      - "/claims/{cid} thread (slice-11)"
      - "/claims list flag (slice-12, via render_list_presence_flag)"
      - "/peer-claims row flag (slice-13)"
      - "/project + /philosophy edge flag (slice-13)"
    owner: "slice-11 (viewer-counter-claim-threads) — REUSED verbatim by slice-12 + slice-13"
    integration_risk: "MEDIUM — a new flag string would fork the neutral-marker vocabulary and risk a non-vetted verdict word. Mitigation: slice-13 introduces NO new string; the marker is the SAME constant on every surface."
    validation: "The slice-11 neutral-flag verdict-word blocklist (no 'disputed'/'refuted'/'false') reused on the new surfaces; a single source constant means one mutation site."

  claim_thread_link:
    source_of_truth: "GET /claims/{cid} (slice-11 detail route)"
    consumers:
      - "/claims list flag (slice-12)"
      - "/peer-claims flag (slice-13)"
      - "/project + /philosophy edge flag (slice-13)"
    owner: "slice-11"
    integration_risk: "LOW — the link target is the well-defined slice-11 route; the CID comes from the row/edge being flagged."
    validation: "Each flag is a render-only <a href=\"/claims/{cid}\">Countered</a> one-hop link; AC asserts the href targets the flagged row/edge's own CID."

  from_row_with_presence_projection:
    source_of_truth: "crates/viewer-domain/src/lib.rs (ClaimRowView::from_row_with_presence, slice-12, lines ~78-88) — the PATTERN slice-13 mirrors"
    consumers:
      - "ClaimRowView.is_countered (slice-12)"
      - "PeerClaimRowView.is_countered (slice-13 — new field, same pattern)"
      - "EdgeRow.is_countered (slice-13 — new field, same pattern)"
    owner: "slice-12 — the projection pattern (effect shell sets is_countered from presence.contains(&cid); render stays a total fn of (view, presence))"
    integration_risk: "MEDIUM — the is_countered flag MUST be set in the effect shell (keeping the pure render total), not read in the pure core. Mitigation: mirror the slice-12 from_row_with_presence exactly; viewer-domain stays pure."
    validation: "viewer-domain purity (no I/O imports); the is_countered field carries every other display field through UNCHANGED (the flag is additive only — the slice-12 from_row_with_presence_preserves_every_display_field property, mirrored)."

  EdgeRow_cid:
    source_of_truth: "crates/viewer-domain/src/lib.rs (EdgeRow.cid, non-Option, slice-10, line ~2077)"
    consumers:
      - "the /claims/{cid} flag link target on a flagged edge"
      - "the page CID set the handler flattens into counter_presence_for"
    owner: "slice-10 (viewer-graph-traversal)"
    integration_risk: "LOW — every edge maps to exactly one signed claim (I-GT-4), so the CID is always present and non-Option."
    validation: "The handler flattens EdgeRow.cid across all EdgeGroups into ONE counter_presence_for call (US-CF-001 AC)."

  PeerClaimRowView_cid:
    source_of_truth: "crates/viewer-domain/src/lib.rs (PeerClaimRowView.cid, slice-06/10, line ~943)"
    consumers:
      - "the /claims/{cid} flag link target on a flagged peer row"
      - "the page CID set the handler collects into counter_presence_for"
    owner: "slice-06 (htmx-scraper-viewer)"
    integration_risk: "LOW — every peer row carries its CID."
    validation: "The handler collects PeerClaimRowView.cid for the page into ONE counter_presence_for call (US-CF-001 AC)."
```

## Validation questions (answered)

- **Does every `${variable}` in the mockups have a documented source?** Yes — the marker
  string (`COUNTERED_PRESENCE_FLAG`), the presence read (`counter_presence_for`), the link
  target (`/claims/{cid}`), and both CID fields all trace to a single slice-source above.
- **If the marker string changes, would all consumers update?** Yes — it is ONE constant
  (`COUNTERED_PRESENCE_FLAG`) consumed by every surface; one mutation site.
- **Are there hardcoded values that should reference a shared artifact?** No — slice-13
  adds NO new string and NO new read; it REUSES the slice-11/12 constants + read.
- **Do any two surfaces display the same data from different sources?** No — the presence
  truth comes from the SAME `counter_presence_for` read on every surface; the marker comes
  from the SAME constant. This is the load-bearing single-source-of-truth property of the
  slice.

## Integration risk summary

The single highest integration risk is the **N+1 regression** on the edge surfaces: the
edge CID set spans multiple `EdgeGroup`s, so a naive implementation could call the read
per-group or per-edge. US-CF-001's AC (flatten ALL edge CIDs across groups into ONE call)
+ I-CF-8 (the inherited slice-12 N+1 guard) + the behavioral query-count test mitigate it.
The second risk is the **traversal re-grouping/re-ordering** regression, mitigated by I-CF-9
+ the byte-identity (markers-elided) no-regression gold (the slice-12 baseline+elision tactic).
