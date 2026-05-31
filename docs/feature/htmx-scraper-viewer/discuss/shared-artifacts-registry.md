# Shared Artifacts Registry: htmx-scraper-viewer (slice-06)

Every `${variable}` that appears in the journey TUI/HTML mockups must have a single
documented source of truth. This is a **brownfield DELTA**: most sources already exist
in prior slices. The viewer is a **read-only consumer** of these sources — it MUST NOT
become a second source of truth for any of them.

---

## Registry

```yaml
shared_artifacts:

  duckdb_store:
    source_of_truth: "adapter-duckdb (local DuckDB file on the operator's machine)"
    owner: "slice-01 foundation (claims) + slice-03 federated-read (peer_claims)"
    consumers:
      - "viewer /claims page (renders rows from `claims`)"
      - "viewer /peer-claims page (renders rows from `peer_claims`)"
      - "existing CLI query paths (slice-01/03)"
    integration_risk: "HIGH — the viewer must read the SAME store the CLI writes; a
      divergent connection/path would show stale or empty data and break operator trust."
    validation: "Viewer reads via the existing adapter-duckdb seam; no separate schema,
      no duplicate store. Offline read returns the same rows a DuckDB shell would."

  claim_row:
    source_of_truth: "`claims` table in ${duckdb_store} (slice-01 signed-claim schema)"
    owner: "slice-01 foundation"
    fields: [subject, predicate, object, "evidence[]", confidence, author_did, composed_at, cid]
    consumers:
      - "viewer /claims list (one row per claim)"
      - "viewer /claims/{cid} detail page"
    integration_risk: "MEDIUM — field set surfaced in HTML must match the persisted
      schema; surfacing a field that does not exist, or mislabeling confidence, misleads
      the operator. Exact column-to-display mapping is OD-VIEW-3 for DESIGN."
    validation: "Each displayed field traces to a real column in the slice-01 schema.
      confidence rendered as the stored numeric (e.g. 0.90), not reformatted/rounded
      silently."

  peer_claim_row:
    source_of_truth: "`peer_claims` (+ evidence) tables in ${duckdb_store} (slice-03)"
    owner: "slice-03 federated-read"
    fields: [subject, predicate, object, "evidence[]", confidence, author_did, peer_origin, cid]
    consumers:
      - "viewer /peer-claims list"
      - "viewer /peer-claims/{cid} detail (if surfaced)"
    integration_risk: "MEDIUM — federated claims carry peer provenance (peer_origin) that
      own claims do not; the view must distinguish 'mine' vs 'federated' so the operator
      is not confused about authorship."
    validation: "peer_claims rendered on a distinct surface (or clearly labeled) from own
      claims; peer_origin shown."

  scrape_target:
    source_of_truth: "operator input (the GitHub target string entered in the browser form),
      mirroring the CLI `scrape github <target>` argument"
    owner: "slice-02 github-scraper (defines valid target semantics)"
    consumers:
      - "viewer live-scrape form input"
      - "viewer live-scrape result heading (echoes the requested target)"
      - "the harvest call passed to the slice-02 propose pipeline"
    integration_risk: "MEDIUM — target validation/semantics must match the CLI exactly so
      browser results equal CLI results for the same target."
    validation: "Same target string yields the same candidate set the CLI would propose."

  candidate_claim:
    source_of_truth: "live harvest — in-memory `CandidateClaim` ADTs derived per request by
      the slice-02 propose step. NOT PERSISTED. Lives and dies within one request."
    owner: "slice-02 github-scraper"
    fields: [subject, predicate, object, evidence, confidence, derived_from]
    consumers:
      - "viewer live-scrape result list (one row per candidate)"
    integration_risk: "HIGH — these are ephemeral and unsigned. The view must NEVER imply
      they are persisted or signed, and must NEVER offer a sign action (I-SCR-1)."
    validation: "No candidate is written anywhere; refreshing re-harvests; no sign control
      is rendered; `derived_from` shown as display-only (WD-62)."

  derived_from:
    source_of_truth: "computed display-only provenance on a `CandidateClaim` (slice-02, WD-62)"
    owner: "slice-02 github-scraper"
    consumers:
      - "viewer live-scrape candidate row (a 'derived-from' badge/label)"
    integration_risk: "HIGH (semantic) — `derived-from` is DISPLAY-ONLY and NEVER persisted
      (WD-62). Once a candidate is signed (in CLI), the resulting claim is byte-identical to
      a hand-authored claim. The viewer MUST present derived-from only on the live-scrape
      candidate view and MUST NOT claim to show it for persisted claims (it cannot — it is
      not stored)."
    validation: "derived-from appears ONLY on live-scrape candidates, NEVER on /claims rows."

  rendered_page:
    source_of_truth: "the viewer's HTML rendering layer (templating approach is OD-VIEW-1
      for DESIGN — e.g. a templating crate; axum is banned by deny.toml, hyper is the
      established HTTP choice per the indexer)"
    owner: "slice-06 (this feature) — DESIGN owns the tech"
    consumers:
      - "the operator's browser"
    integration_risk: "LOW (display layer) — but must render fields verbatim from their
      sources above; the page is a VIEW, never a source."
    validation: "Every value on the page traces to one of the sources above; the page
      introduces no new persisted state."
```

---

## Source-of-truth rule for this slice

The viewer is a **pure read surface**. It owns exactly one thing: `${rendered_page}`
(the HTML view). Every datum it shows is borrowed from an existing slice's source:

| Datum | Borrowed from | Persisted? | Offline? |
|-------|---------------|------------|----------|
| `${claim_row}` | slice-01 `claims` | yes | yes |
| `${peer_claim_row}` | slice-03 `peer_claims` | yes | yes |
| `${candidate_claim}` | slice-02 live harvest | **no** | no (needs network) |
| `${derived_from}` | slice-02 (display-only, WD-62) | **no** | n/a |

## Integration checkpoints

1. **Store identity**: viewer reads the exact same `${duckdb_store}` the CLI uses — no
   second store, no separate schema. (HIGH risk)
2. **Field fidelity**: every `${claim_row}` / `${peer_claim_row}` field on screen maps to
   a real persisted column. (MEDIUM risk — column mapping deferred to OD-VIEW-3)
3. **Provenance honesty**: `${derived_from}` shown ONLY on live-scrape candidates, never on
   persisted claims (because it is not stored — WD-62). (HIGH semantic risk)
4. **No-persist / no-sign**: `${candidate_claim}` is never written and never carries a sign
   control; the web process never holds the key (I-SCR-1). (HIGH risk)
