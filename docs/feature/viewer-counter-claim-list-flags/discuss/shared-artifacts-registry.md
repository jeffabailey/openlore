# Shared Artifacts Registry: viewer-counter-claim-list-flags (slice-12)

Every value crossing a step boundary, its single source of truth, and its consumers.

## Artifacts

```yaml
shared_artifacts:

  COUNTERED_PRESENCE_FLAG:
    source_of_truth: "crates/viewer-domain/src/lib.rs::COUNTERED_PRESENCE_FLAG (= \"Countered\", slice-11)"
    consumers:
      - "slice-11 detail render_presence_flag (/claims/{cid})"
      - "slice-12 render_claim_row (the /claims list marker — THIS slice)"
    owner: "viewer-domain (pure)"
    integration_risk: "MEDIUM — if slice-12 introduces a NEW flag string instead of reusing this constant, the detail-page and list-page markers drift, breaking the single neutral-marker contract. Reuse verbatim."
    validation: "Unit test: the /claims list marker text equals COUNTERED_PRESENCE_FLAG; no second 'Countered'-like literal."

  countered_cid_set:
    source_of_truth: "StoreReadPort::counter_presence_for(&[cid]) over the LOCAL indexed claim_references ∪ peer_claim_references (NEW this slice; widens slice-11 query_counter_claims Step-A)"
    consumers:
      - "the effect shell (claims_page) — projects is_countered onto each ClaimRowView via presence.contains(&row.cid)"
      - "render_claim_row — emits the marker only for is_countered rows"
    owner: "ports (trait) + adapter-duckdb (impl)"
    integration_risk: "HIGH — must be ONE aggregate query (referenced_cid IN (...) DISTINCT), NEVER N+1. Must be presence-only (set membership), never a count or merged aggregate. Must be LOCAL (no network, no per-row artifact read)."
    validation: "Gold/@property test: query count invariant to page size; read-only gold (store row counts unchanged); offline render test."

  claim_cid:
    source_of_truth: "ClaimRow.cid (from list_claims, slice-06)"
    consumers:
      - "the /claims/{cid} path segment (the [Countered] link href)"
      - "slice-11 query_counter_claims(target_cid) on the detail page (the drill target)"
      - "counter_presence_for input (the page's CID set)"
    owner: "adapter-duckdb (list_claims) — slice-06 (unchanged)"
    integration_risk: "MEDIUM — the list flag's link CID MUST equal the detail route CID, or the one-hop drill lands on the wrong claim."
    validation: "Acceptance test: following a row's [Countered] link reaches /claims/{that row's cid} and shows that claim's thread."

  list_ordering_paging_count:
    source_of_truth: "adapter-duckdb::list_claims (ORDER BY composed_at DESC, cid; LIMIT/OFFSET; COUNT(*) total) — slice-06 (UNCHANGED)"
    consumers:
      - "render_claims_table / render_pagination (slice-06)"
    owner: "adapter-duckdb (slice-06)"
    integration_risk: "HIGH (no-regression) — the flag is ADDITIVE; it must NOT introduce any WHERE/ORDER BY/GROUP BY on the list query. Order, page boundaries, and total count must be byte-identical with and without the flag."
    validation: "Gold test: flagged vs un-flagged render of the same store has byte-identical row order, paging, and total count."

  row_confidence:
    source_of_truth: "ClaimRow.confidence (DOUBLE) rendered via render_confidence (FR-VIEW-8) — slice-06"
    consumers:
      - "render_claim_row confidence cell"
    owner: "viewer-domain::render_confidence (single site)"
    integration_risk: "HIGH (shown-never-applied) — a counter must NEVER re-weight/re-score the flagged claim's confidence. The cell renders 0.90 / 0.30 verbatim regardless of the flag."
    validation: "Gold test: a flagged claim's confidence cell is byte-identical to a no-flag render."
```

## Validation questions (answered)

- **Does every flag in the mockup have a documented source?** Yes —
  `COUNTERED_PRESENCE_FLAG` (slice-11 constant) for the text; `countered_cid_set`
  (the new batch read) for WHICH rows get it.
- **If the flag string changed, would both detail and list update?** Yes — both consume
  the single `COUNTERED_PRESENCE_FLAG` constant. That is the contract; no second literal.
- **Are there hardcoded values that should reference a shared artifact?** No — the slice
  reuses the slice-11 constant and the slice-06 list machinery; the only new value is
  the presence set from `counter_presence_for`.
- **Do any two steps display the same data from different sources?** No — the list flag
  and the detail thread both ultimately derive from the same counter references
  (`claim_references ∪ peer_claim_references`); the list reads PRESENCE (boolean set),
  the detail reads the FULL thread (attributed rows + reasons). One source, two read
  shapes, no divergence.
