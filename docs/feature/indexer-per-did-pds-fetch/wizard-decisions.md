# Wizard decisions — indexer-per-did-pds-fetch

- **Problem:** the indexer lists every repo DID in `OPENLORE_INDEXER_REPO_DIDS` from the single `OPENLORE_INDEXER_SOURCE_URL` (`crates/openlore-indexer/src/run.rs:117-121`). Per ADR-071 §4, a self-attested record counts as own-PDS only when that URL equals the DID's resolved PDS. Self-attested claims of authors on any other PDS (e.g. bsky.social users of the review app) are refused as relay-origin and never reach network search.
- **Goal:** resolve each repo DID to its own PDS (DID document `#atproto_pds` service, via the existing `IdentityResolvePort`, as `peer pull` does per ADR-016) and list its `org.openlore.claim` records there, with `repo=<DID>` and cursor paging (`RepoListingPort`, 02-03). Self-attested claims from any author's own PDS get indexed; app-signed indexing is unchanged.
- **Source URL:** it becomes an optional fallback. It is used only for DIDs that can't be resolved; those records stay relay-origin, so self-attested ones are still refused.
- **Keep:** the ADR-024 bounded pull (per-source fault isolation, cadence, freshness), ADR-025 anti-merging, and the ADR-071 verdict (CID == rkey from the at:// URI).
- **Classification:** backend, changes existing behaviour; brownfield.
- **Starting wave:** light /nw:discuss, then /nw:design (ADR amending ADR-071 §4 and ADR-024 source discovery).
- **Origin:** follow-up from bluesky-claim-review-app (docs/evolution/bluesky-claim-review-app-evolution.md).
