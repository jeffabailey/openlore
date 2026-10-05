# Shared artifacts registry: indexer-per-did-pds-fetch

Each artifact has one source of truth. Consumers must read it from that source and never
re-derive it.

| Artifact | Single source | Produced at | Consumed at | Notes |
|---|---|---|---|---|
| `${repo_dids}` | `OPENLORE_INDEXER_REPO_DIDS` (parsed once at startup) | S1 | S2, S5 counts | Malformed entry → refuse start (US-IPF-004). |
| `${fallback_source_url}` | `OPENLORE_INDEXER_SOURCE_URL`, now **optional** | S1 | S2 (unresolvable DIDs only) | Never used for a resolvable DID. Its records are always relay-origin. |
| `${plc_endpoint}` | `OPENLORE_INDEXER_PLC_ENDPOINT` (default `https://plc.directory`) | S1 | S2 | Existing. |
| `${resolved_pds}` | DID document `#atproto_pds` service, resolved **once per DID per pass** | S2 | S3 (where to list), S4 (verdict) | The same value feeds both, which keeps listing and verdict consistent (IC-2). |
| `${fetched_from}` | Base URL the listing was actually read from (listing result) | S3 | S4 | Computed, never assumed (ADR-071 §4, IC-1). |
| `${record_origin}` | Pure comparison `fetched_from == resolved_pds` → `AuthorPds` or `Relay` | S4 | verdict | Exact match only. |
| `${provenance}` | ADR-071 verdict → `indexed_claims.provenance` | S4 | search DTO → CLI label (S6) | IC-3: index, wire and label agree. |
| `${skip_reason}` | One of `did_unresolvable`, `pds_unreachable`, `pds_timeout`, `listing_failed` | S2/S3 | S5 operator events | DESIGN may refine the vocabulary. The reasons must stay distinguishable. |
| `${source_counts}` | Pass tally: own-PDS / fallback / skipped / configured | S3 | S5 summary event | Invariant: own_pds + fallback + skipped = configured. |
| `${max_concurrent_fetches}` | Indexer config (DESIGN default) | S1 | S3 | NFR-1. |
| `${per_did_time_budget}` | Indexer config (DESIGN default) | S1 | S3 | NFR-2. A timeout produces `pds_timeout`. |
| `${page_bound}` | Existing per-repo page bound (`RepoListingPort`) | existing | S3 | Unchanged. |
