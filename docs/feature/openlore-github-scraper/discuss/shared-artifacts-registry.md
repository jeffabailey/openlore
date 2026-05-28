# Shared Artifacts Registry — openlore-github-scraper (slice-02)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

Every `${variable}` that flows across the scrape -> propose -> sign journey,
its single source of truth, and its consumers. Untracked artifacts are the
primary cause of horizontal integration failures.

## Registry

```yaml
shared_artifacts:
  github_target:
    source_of_truth: "the <target> CLI argument, resolved against the GitHub API by adapter-github"
    consumers:
      - "step 2 candidate subject (github:<owner>/<repo> or github:<user>)"
      - "step 3 compose subject"
      - "step 4 signed claim subject"
    owner: "openlore-github-scraper / cli"
    integration_risk: "HIGH - if the target string drifts between harvest and sign, claims attribute to the wrong subject"
    validation: "integration_validation.shared_artifact_consistency must_match_across [1,2,3,4]"

  harvested_signal:
    source_of_truth: "adapter-github harvest of PUBLIC artifacts (READMEs, manifests, file ratios, tags)"
    consumers:
      - "step 2 (every candidate names the exact signal that produced it)"
    owner: "openlore-github-scraper / adapter-github + scraper-domain"
    integration_risk: "HIGH - a candidate with no traceable signal is unauditable; breaks J-004b"
    validation: "acceptance test candidate_names_source_signal"

  signal_predicate_mapping:
    source_of_truth: "docs/product/jobs.yaml :: J-004.signal_predicate_mapping"
    consumers:
      - "scraper-domain derivation (step 2)"
    owner: "product (jobs.yaml) - DESIGN owns serde shape only"
    integration_risk: "MEDIUM - mapping drift changes which predicates are proposed; keep small + auditable"
    validation: "the mapping in jobs.yaml is the SSOT; scraper-domain must load/embed it, never hardcode a divergent copy"

  candidate_claim:
    source_of_truth: "scraper-domain (PURE) derivation; in-memory only until the user signs"
    consumers:
      - "step 3 (pre-fills the editable compose fields)"
      - "step 4 (becomes a signed claim ONLY via the slice-01 pipeline)"
    owner: "openlore-github-scraper / scraper-domain"
    integration_risk: "HIGH - a candidate must round-trip its fields unchanged into compose unless the user edits; it must NEVER be persisted as a signed claim or published on its own"
    validation: "acceptance test scraper_never_persists_unsigned + integration must_match_across [2,3,4]"

  confidence:
    source_of_truth: "default 0.25 (speculative) from signal_predicate_mapping; human-editable in step 3"
    consumers:
      - "step 2 display (0.25 speculative)"
      - "step 3 (editable; [0.0,1.0] constraint enforced)"
      - "step 4 signed payload (numeric only, per WD-10; buckets are display-only)"
    owner: "scraper-domain default; human override; claim-domain validates range"
    integration_risk: "HIGH - confidence must NEVER auto-inflate between proposal and sign; only the human may raise it"
    validation: "integration must_match_across [2,3,4] unless user-edited; WD-10 numeric-only invariant (I-6)"

  claim_cid:
    source_of_truth: "claim-domain::compute_cid (PURE) - computed only at sign time (step 4)"
    consumers:
      - "local store filename ~/.local/share/openlore/claims/<cid>.json"
      - "publish at-uri"
      - "future graph query / federated read"
    owner: "claim-domain (slice-01, reused unchanged)"
    integration_risk: "HIGH - CID drift breaks the federation thesis (I-10 inherited)"
    validation: "slice-01 CID round-trip tests apply unchanged; scraper adds no new CID path"

  derived_from_provenance:
    source_of_truth: "cli sets this informational line when a claim originates from a scraper run"
    consumers:
      - "step 4 compose preview (informational line)"
      - "optional: stored as a non-CID-affecting note OR omitted from signed payload (DESIGN's call)"
    owner: "openlore-github-scraper / cli"
    integration_risk: "LOW - informational only; MUST NOT change the signed payload's confidence or federation behavior. If included in the signed payload it must be an OPTIONAL field per ADR-005 (forward-compatible) and must be CID-stable when absent (mirrors the slice-03 `reason` field treatment in WD-32/ADR-015)."
    validation: "DESIGN decides inclusion; if included, lexicon conformance test asserts CID-stability-when-absent"
```

## Integration gates (handed to DISTILL)

| Gate | Asserts | Maps to KPI |
|---|---|---|
| `scraper_never_persists_unsigned` | running `scrape github` WITHOUT `--sign` produces zero `author_claims` rows and zero PDS writes | KPI-SCR-2 (human-gate guardrail) |
| `candidate_names_source_signal` | every rendered candidate carries the exact signal that produced it | KPI-SCR-3 (auditability) |
| `scraper_only_reads_public_data` | a private/non-existent target produces zero candidates and exits non-zero; no private endpoint is ever called | KPI-SCR-4 (no-surveillance guardrail) |
| `candidate_confidence_no_autoinflate` | confidence at sign == proposed confidence UNLESS the user edited it; never higher by default | KPI-SCR-2 (human-gate guardrail) |
| `scraper_reuses_slice01_publish_path` | a signed candidate publishes via the SAME VerbClaimPublish path as a hand-authored claim (no parallel path) | preserves ADR-003 invariant |

## Consistency check (performed during this DISCUSS)

- Every `${variable}` in the journey TUI mockups has a documented source above: PASS.
- `github_target`, `candidate_claim`, and `confidence` each have an explicit
  `must_match_across` rule in the journey YAML: PASS.
- The `signal_predicate_mapping` SSOT lives in `jobs.yaml` (product layer), not
  hardcoded in the journey or in two places: PASS.
- `claim_cid` reuses the slice-01 source of truth (`claim-domain::compute_cid`);
  no new CID computation path is introduced: PASS.
