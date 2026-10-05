# Acceptance criteria: indexer-per-did-pds-fetch

This file consolidates the ACs from `user-stories.md`, where each AC is derived from that
story's UAT scenarios. The constraints I-IPF-1..6 apply to all of them.

| AC | Story | Criterion | Traces |
|---|---|---|---|
| AC-001.1 | 001 | Each DID is listed on the PDS its DID document names, and several PDSes are handled in one pass with correct attribution. | FR-1, I-IPF-5 |
| AC-001.2 | 001 | Self-attested records read from the freshly resolved own PDS are indexed as self-attested. App-signed records are handled as today. | FR-3, I-IPF-1 |
| AC-001.3 | 001 | An indexed self-attested claim is returned by `openlore search` with its author. | J-005 |
| AC-001.4 | 001 | Resolution happens every pass, and a moved PDS is followed. | FR-2 |
| AC-001.5 | 001 | Records of another repo are never indexed for the requested DID and are counted as refused. | FR-4 |
| AC-001.6 | 001 | CID == rkey still refuses tampered records, and other records are unaffected. | FR-3 |
| AC-002.1 | 002 | Unreachable, erroring or failed-listing DIDs are skipped. The others are indexed, and the pass exits 0. | FR-6, I-IPF-3 |
| AC-002.2 | 002 | An unresolvable DID with no fallback is skipped with `did_unresolvable`. | FR-6 |
| AC-002.3 | 002 | One structured skip event per DID: DID, reason, PDS URL when known, no claim content. | FR-7, NFR-4 |
| AC-002.4 | 002 | Skipping never removes previously indexed rows. | FR-6 |
| AC-002.5 | 002 | A non-answering PDS is abandoned after the per-DID time budget (`pds_timeout`). | NFR-2 |
| AC-002.6 | 002 | Skipped DIDs are retried next pass. | FR-6 |
| AC-002.7 | 002 | Concurrent outbound requests are at most `${max_concurrent_fetches}`. own_pds + fallback + skipped = configured. | NFR-1, FR-7 |
| AC-003.1 | 003 | An unresolvable DID with a fallback is listed there. App-signed records go through the normal gate, and self-attested ones are refused (provenance). | FR-5, I-IPF-2 |
| AC-003.2 | 003 | The fallback is never contacted for a DID resolved in this pass. | FR-5 |
| AC-003.3 | 003 | A fallback failure skips only the affected DIDs, with `fallback_used: true`. | FR-6, FR-7 |
| AC-004.1 | 004 | A malformed repo DID refuses start, naming the variable and the entry. | FR-8 |
| AC-004.2 | 004 | A malformed fallback URL refuses start, naming the variable and the value. | FR-8 |
| AC-004.3 | 004 | An unset fallback or empty DID list is reported, not refused. | FR-8 |
| AC-004.4 | 004 | For a single-source deployment, the indexed set and search output for existing data are unchanged, apart from the provenance label. | NFR-3 |
| AC-005.1 | 005 | Results carry the stored provenance, and the CLI shows `[self-attested]` for self-attested rows only. | FR-9 |
| AC-005.2 | 005 | An absent provenance is read as app-signed, and old-server output is unchanged. | FR-9 |
| AC-005.3 | 005 | Provenance is consistent across the object, subject and contributor dimensions. | FR-9 |
