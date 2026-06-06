# Shared-Artifacts Registry: viewer-counter-claim-threads (slice-11)

Every `${variable}` rendered in the counter-thread on `GET /claims/{cid}` has a
single source of truth and documented consumers. Slice-11 introduces NO new persisted
data — it READS existing slice-03 counter-claims (signed claims with a `counters`
reference + a verbatim `reason`).

## Registry

```yaml
shared_artifacts:

  target_cid:
    source_of_truth: "the GET /claims/{cid} path parameter (the claim being viewed)"
    consumers:
      - "the countered claim's own detail render (existing get_claim)"
      - "the argument to StoreReadPort.query_counter_claims(target_cid)"
    owner: "adapter-http-viewer (route) → ports (read method)"
    integration_risk: "HIGH — if the path CID and the query_counter_claims argument
      diverge, the viewer shows the wrong claim's counters."
    validation: "acceptance: the counters shown are exactly those whose references[]
      counters-CID equals the path CID."

  counter_cid:
    source_of_truth: "the counter's own content-addressed CID, from claims ∪ peer_claims
      (computed at the counter's author/pull time; slice-01/03 — NOT recomputed by the viewer)"
    consumers:
      - "the counter thread item (shown verbatim)"
      - "the href to /claims/{counter_cid} (drill into the counter's own detail)"
    owner: "ports.CounterClaimRow.cid (non-Option) → viewer-domain CounterClaimView"
    integration_risk: "HIGH — a displayed CID that differs from the link target breaks
      drill-into-counter navigation; a recomputed CID would violate the trust contract."
    validation: "acceptance: the displayed CID equals the /claims/{counter_cid} href;
      the viewer recomputes no CID (it trusts the verified-at-write store)."

  counter_author_did:
    source_of_truth: "claims.author_did (own counter) OR peer_claims.author_did (peer
      counter) — NON-Option, projected EXPLICITLY by query_counter_claims (anti-merging)"
    consumers:
      - "each counter thread item's attribution line"
      - "the (you) vs peer distinction (PeerOrigin)"
    owner: "ports.CounterClaimRow.author_did + origin → viewer-domain"
    integration_risk: "HIGH — eliding or merging the author DID collapses the
      anti-merging guarantee (I-CT-3 / KPI-AV-2 / KPI-FED-1); a 'disputed by 2' row
      would be a cardinal violation."
    validation: "anti-merging gold: two counters render as two items each under a
      distinct author DID; no consensus/aggregate row anywhere."

  counter_reason:
    source_of_truth: "the signed claim's `reason` field (top-level optional on
      org.openlore.claim, ADR-015; NFC-normalized at author time; verbatim on disk)"
    consumers:
      - "each counter thread item's reason line (shown VERBATIM)"
    owner: "ports.CounterClaimRow.reason → viewer-domain"
    integration_risk: "MEDIUM — the reason IS the disagreement artifact; it must render
      verbatim (not truncated, summarized, or markdown-rendered). An empty reason
      (non-OpenLore client, ADR-015 wire-optional asymmetry) must render an explicit
      'no reason provided' state, never a blank line or crash."
    validation: "acceptance: the reason renders byte-for-byte as authored; the
      empty-reason boundary renders the explicit state."

  original_confidence:
    source_of_truth: "the countered claim's stored confidence DOUBLE (claims/peer_claims)"
    consumers:
      - "the countered claim's Confidence field — UNCHANGED by the existence of counters"
    owner: "ports.ClaimDetail.confidence → viewer-domain render_confidence (single site)"
    integration_risk: "HIGH — re-weighting the countered claim because it has counters
      is a shown-never-applied violation (I-CT-2 / OD-AV-7 / ADR-015). It must render
      VERBATIM (0.90 not 0.9/90%, KPI-4) and identical with or without counters."
    validation: "shown-never-applied gold: the claim's rendered confidence/fields are
      identical with and without counters present."

  claim_detail_fragment:
    source_of_truth: "viewer-domain render_claim_detail_fragment (extended to compose
      the counter thread below the claim fields + evidence)"
    consumers:
      - "the htmx swap response (HX-Request → fragment only)"
      - "the full-page #claim-detail region (no-JS → page embeds the SAME fragment)"
    owner: "viewer-domain (pure) → adapter-http-viewer claim_detail_page (Shape fork)"
    integration_risk: "MEDIUM — a fragment/full-page divergence breaks progressive-
      enhancement parity (I-HX-5); a CDN reference breaks offline (I-CT-5)."
    validation: "parity gold: both shapes embed the same fragment; references_external_cdn
      scan: zero off-host htmx references."

  countered_flag:
    source_of_truth: "the non-empty result of query_counter_claims(cid)
      (CounterThread::Countered arm)"
    consumers:
      - "the neutral 'Countered' presence marker near the claim"
    owner: "viewer-domain CounterThread ADT"
    integration_risk: "MEDIUM — the flag must be a NEUTRAL presence marker, never a
      score/weight/count-verdict/consensus judgement, and must NOT alter the claim's
      confidence. An empty result must render NO flag and NO empty-state noise (US-CT-003)."
    validation: "acceptance: empty → no flag/no section; non-empty → neutral flag with
      no verdict; confidence unchanged."
```

## Consistency checks (validation questions)

- **Does every `${variable}` in the mockups have a documented source?** Yes — all six
  thread variables + the flag are sourced above.
- **Could the displayed counter CID ever differ from its link target?** No — both
  derive from the single `CounterClaimRow.cid`.
- **Could the countered claim's confidence change because counters exist?** No — it
  derives from `ClaimDetail.confidence` via the single `render_confidence` site; the
  counters never feed it (shown-never-applied).
- **Could two counters be merged into one row?** No — `query_counter_claims` projects
  `author_did` + `cid` explicitly via UNION ALL (no merging JOIN); the renderer emits
  one item per row.
- **Is any value read from the network on this route?** No — `claims ∪ peer_claims`
  is the LOCAL store; peer counters were verified at `peer pull` time (slice-03), not
  here.
