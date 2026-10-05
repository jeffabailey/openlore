# ADR-079: Provenance on the Search Wire (`SearchResultDto.provenance`) and CLI Rendering

- **Status**: Proposed
- **Date**: 2026-10-05
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: indexer-per-did-pds-fetch (DESIGN), US-IPF-005
- **Realizes**: ADR-071 §5 ("The search DTO gains an optional `provenance` field"), which was
  specified but never added to `crates/lexicon/src/appview_query.rs`.

## Context

`indexed_claims.provenance` exists and `IndexedClaim.provenance` is populated, but
`flat_attributed_rows` does not project it and `SearchResultDto` has no field for it. Once
ADR-077 makes self-attested rows routine, the CLI would show them with only `[verified]`.
Readers treat "absent" as app-signed, so that is a provenance misstatement (WD-IPF-6).

## Decision

1. `SearchResultDto.provenance: Option<String>`, serialized only when present. The indexer
   always sets it: `"app-signed"` or `"self-attested"`, from `IndexedClaim.provenance`.
2. CLI decode (`adapter-index-query`): absent or `"app-signed"` → `PeerClaimProvenance::AppSigned`
   (old servers keep working, ADR-071 §5). `"self-attested"` → `SelfAttested`. **Any other
   value**: the row is dropped and the CLI prints one stderr notice with the count ("N results
   with an unrecognized provenance were hidden; upgrade openlore"). A future mode is never shown
   as app-signed. `NetworkResultRowRaw` gains `provenance`.
3. Rendering: self-attested rows show `[verified] [self-attested]`. The `--show` verification
   line for a self-attested row reads "self-attested by <repo DID>; read from the author's own
   PDS" and does not claim a signature check. App-signed rows are byte-identical to today.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Typed serde enum on the wire** | Rejected. An unknown future value would fail the whole response (`BadResponse`), so one new row would blank the search. |
| **Unknown value → render as app-signed** | Rejected. That is a misstatement (ADR-071 I-BRA-5 spirit: never upgrade trust). |
| **Server omits the field for app-signed rows** | Rejected. Explicit values make the wire self-describing. The bytes are negligible. |
| **Bump the XRPC method / version the DTO** | Rejected. It is an additive optional field. Old clients ignore it (no `deny_unknown_fields`). |

## Consequences

- **Positive**: the searcher sees an honest label. Old servers and old clients interoperate
  both ways.
- **Negative**: every `NetworkResultRowRaw` constructor (fixtures, proptest strategies) must set
  the new field. This is mechanical.
