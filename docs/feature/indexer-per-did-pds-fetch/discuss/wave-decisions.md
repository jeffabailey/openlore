# DISCUSS wave decisions: indexer-per-did-pds-fetch

- **Mode**: light DISCUSS (research depth LIGHTWEIGHT: happy path plus key error paths, no
  emotional-arc depth). Backend, brownfield. Inputs: `../wizard-decisions.md`,
  `docs/evolution/bluesky-claim-review-app-evolution.md`, ADR-016/023/024/025/071,
  `crates/openlore-indexer/src/run.rs`.
- **DIVERGE artifacts**: none. Not run, by user decision (JTBD: NO). Stories map onto existing
  jobs in `docs/product/jobs.yaml`. Risk: low, because the job (network discovery, J-005) and
  its success criteria were validated in openlore-appview-search.

## Scope Assessment: PASS: 5 stories, 1 bounded context, about 7–9 days

- One context: the indexer ingest pass (composition root `openlore-indexer` plus
  `adapter-atproto-ingest`/`adapter-atproto-did`). US-IPF-005 adds one optional field to the
  search wire DTO and one CLI label. That is an additive reader change in the same discovery context.
- Integration points: PLC/DID resolution, per-author PDS `listRecords`, optional fallback source,
  index store (unchanged schema), search DTO. There are 4 of them, so the count stays at or under 5.
- No oversized signals. No split needed.

## Decisions

| ID | Decision | Rationale |
|---|---|---|
| WD-IPF-1 | Each repo DID is read from the PDS that is freshly resolved from its DID document on every pass. | ADR-071 §4 and ADR-016. This is the only origin that admits a self-attested record. |
| WD-IPF-2 | `OPENLORE_INDEXER_SOURCE_URL` becomes an **optional fallback**, used only for DIDs that cannot be resolved. Records read through it keep the relay origin, so self-attested ones are still refused. | User decision (wizard). It fails closed per ADR-071. |
| WD-IPF-3 | A failure is isolated to one DID. An unresolvable DID with no fallback, an unreachable or timed-out PDS, or a listing error skips that DID with a reason. The pass continues, and the DID is retried on the next pass. | ADR-024 per-source fault isolation. **Brownfield gap:** today a single listing failure aborts the whole pass with exit 2 (`run.rs` `ingest`, "listing … failed" → `return 2`). |
| WD-IPF-4 | Previously indexed claims of a skipped DID stay searchable. Skipping never deletes. | ADR-024 staleness is observable, not destructive. |
| WD-IPF-5 | Fan-out is bounded: a fixed cap on concurrent PDS requests and the existing per-repo page bound. DESIGN picks the default, and sequential (cap 1) is acceptable. A slow PDS is time-bounded. | ADR-024 bounded pull. Protects third-party PDSes such as bsky.social. |
| WD-IPF-6 | **The search DTO provenance field is IN scope** as a small separate story (US-IPF-005). | ADR-071 §5 requires it, and it is missing from the wire `SearchResultDto` today. This feature is what makes self-attested rows reach search routinely. Without the field, readers treat "absent" as app-signed, so Priya's claims would be shown to Maria as app-signed. That is a provenance misstatement. It is kept as its own story so the thin slice does not depend on it. |
| WD-IPF-7 | Stories trace to **J-005** (discover signed claims across the network). It is the job `openlore search` serves. The brief suggested J-002 or J-009. J-005 is the closer match, so J-002 and J-009 are listed as beneficiaries in `requirements.md`. The operator-only config story is `infrastructure-only`. | jobs.yaml traceability rule. |
| WD-IPF-8 | No schema migration. `indexed_claims.provenance` already exists (migration from 03-02). | Read from `adapter-index-store/src/schema.rs`. |

## Risks

- R-IPF-1: hitting bsky.social PDS hosts from the indexer is a third-party load. Mitigated by WD-IPF-5 and the existing cadence.
- R-IPF-2: `IdentityLookup` is built from the PLC endpoint only. `did:web` resolution coverage is an open question for DESIGN (OQ-IPF-2).
