# Requirements: indexer-per-did-pds-fetch

## Problem

The indexer reads every configured repo DID (`OPENLORE_INDEXER_REPO_DIDS`) from one
operator-configured URL (`OPENLORE_INDEXER_SOURCE_URL`). ADR-071 §4 admits a self-attested
claim only when it was read from the PDS resolved from the author's DID document. So every
self-attested claim stored on a different PDS (all bsky.social review-app users) is refused as
relay-origin and never reaches `openlore search`. A second gap: one unreachable source aborts the
whole ingest pass, so nobody's new claims get indexed (contrary to ADR-024).

## Personas

| Persona | Role | Context | Wants |
|---|---|---|---|
| **Maria** | Searcher (J-005) | Cares about reproducible builds and follows nobody who claims it. Runs `openlore search --object org.openlore.philosophy.reproducible-builds`. | To find every public claim on the network, honestly labeled. |
| **Priya Raman** | Author (J-009) | `priyaraman.bsky.social`, DID `did:plc:priyaraman7x2k`, PDS `https://morel.us-east.host.bsky.network`. She approved 3 self-attested claims in the review app. | To have her approved claims become discoverable. Today they are invisible. |
| **Dmitri Volkov** | Author (CLI, app-signed) | DID `did:plc:dvolkov3m9q`, self-hosted PDS `https://pds.volkov.dev`. | To have his app-signed claims keep appearing exactly as today. |
| **Jeff Bailey** | Indexer operator | Runs `openlore-indexer` with 14 repo DIDs. The current single source is his own PDS `https://pds.jeffbailey.us` (illustrative). | Clear per-DID logs when a DID does not resolve or a PDS is down. Startup errors he can act on. |

Job beneficiaries: J-005 (primary), J-009 (its success depends on discoverability), J-002
(network results feed exploration).

## Functional requirements

- **FR-1 Per-DID origin.** Each pass resolves every repo DID to its DID document's
  `#atproto_pds` service, through the existing verify-only identity resolution, and lists that
  repo's `org.openlore.claim` records there with `repo=<DID>`, following every cursor within the
  existing page bound.
- **FR-2 Fresh every pass.** Resolution happens on every pass. A DID whose PDS changed since the
  last pass is read from the new PDS.
- **FR-3 Verdict unchanged.** Self-attested is admitted only when the read-from URL equals the
  freshly resolved PDS. The CID is compared with the rkey from the `at://` URI. App-signed
  records go through the unchanged verify-before-index gate.
- **FR-4 Repo binding.** Only records whose `at://` repo equals the requested DID are indexed
  for that DID. Records for any other repo are refused and counted.
- **FR-5 Fallback.** If the DID cannot be resolved and a fallback source is configured, the DID
  is listed from the fallback. Those records are relay-origin, so self-attested ones are refused
  and app-signed ones go through the normal gate.
- **FR-6 Fault isolation.** If a DID is unresolvable with no fallback, its PDS is unreachable or
  times out, or its listing errors, that DID is skipped with a reason. Other DIDs are still
  indexed, the pass still completes successfully, and the DID is retried next pass. Already
  indexed claims of a skipped DID stay searchable.
- **FR-7 Operator visibility.** Each skipped DID produces one structured event with the DID and
  a reason (`did_unresolvable`, `pds_unreachable`, `pds_timeout`, `listing_failed`) and whether
  the fallback was used. The pass summary reports counts of DIDs read from their own PDS, from
  the fallback, and skipped.
- **FR-8 Startup config errors.** A malformed repo DID entry or a malformed fallback URL refuses
  start with a message that names the variable and the bad value. An empty DID list or an
  absent fallback is reported, not refused.
- **FR-9 Provenance on the wire.** Search results carry an optional provenance
  (`app-signed` | `self-attested`). The CLI shows `[self-attested]` next to `[verified]`. An
  absent value is read as app-signed (old servers).

## Non-functional requirements

- **NFR-1 Bounded fan-out.** At most `${max_concurrent_fetches}` PDS/PLC requests are
  outstanding at once. DESIGN sets the default, and 1 (sequential) is acceptable. The per-repo
  page bound is unchanged.
- **NFR-2 Time bound.** No single DID can hold a pass longer than `${per_did_time_budget}`
  (DESIGN sets it).
- **NFR-3 No regression.** For a deployment whose DIDs all live on the configured source URL,
  the indexed set and `openlore search` output for existing data are unchanged, except for the
  added provenance label on self-attested rows.
- **NFR-4 Privacy (WD-105).** Events carry DIDs, URLs, counts and reasons only. They never carry
  claim content.
- **NFR-5 Capability boundary (ADR-023).** No signing identity, no user store, no write surface.
  This is unchanged.
- **NFR-6 Anti-merging (ADR-025).** Every row stays attributed to its repo DID. No cross-author
  merge.

## Out of scope

Firehose or relay commit-proof verification (ADR-071 revisit trigger), per-source `--since`
cursors, automatic DID discovery beyond the configured list, and `stats` verb coverage.
