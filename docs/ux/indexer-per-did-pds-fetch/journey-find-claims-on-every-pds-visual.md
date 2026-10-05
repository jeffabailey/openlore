# Journey: find claims on every author's own PDS (visual)

Two actors share one journey. Jeff configures the indexer and watches a pass. Maria searches
and finds Priya. Emotional notes are lightweight, as this is a light DISCUSS.

```text
 JEFF (operator)                         INDEXER PASS                                MARIA (searcher)
 ───────────────                         ────────────                                ────────────────
 [1] configure                            [2] resolve each DID       [3] list on own PDS
  OPENLORE_INDEXER_REPO_DIDS ──────────▶  did:plc:priyaraman7x2k ─▶ morel.us-east.host.bsky.network
  OPENLORE_INDEXER_SOURCE_URL            did:plc:dvolkov3m9q    ─▶ pds.volkov.dev
   (optional fallback)                    did:plc:ghost0000      ─✗ unresolvable ─▶ fallback? ─▶ relay-origin
       │                                         │                        │
       │ startup check                           ▼                        ▼
       │ (bad DID / bad URL → refuse,      [4] verdict per record (ADR-071, unchanged)
       │  named variable)                   own PDS  + self-attested ─▶ INDEX [self-attested]
       │                                    any      + app-signed    ─▶ verify ─▶ INDEX [verified]
       │                                    fallback + self-attested ─▶ REFUSE (provenance)
       │                                    wrong repo               ─▶ REFUSE
       ▼                                         │
 [5] reads pass summary  ◀─────────────── events: source_skipped{did,reason}, summary counts
  "feels: in control — I know                    │
   exactly which DID failed and why"             ▼
                                          index.duckdb (unchanged schema)
                                                 │
                                                 ▼                         [6] openlore search
                                          searchClaims (+ provenance) ───▶ Priya found, labeled
                                                                           "feels: reassured — new
                                                                            authors, honest labels"
```

## Step 5: what Jeff sees (stdout events, one pass)

```text
{"event":"indexer.ingest.source_skipped","did":"did:plc:ghost0000","reason":"did_unresolvable","fallback_used":false}
{"event":"indexer.ingest.source_skipped","did":"did:plc:dvolkov3m9q","reason":"pds_unreachable","pds":"https://pds.volkov.dev","fallback_used":false}
{"event":"indexer.ingest.sources","own_pds":12,"fallback":0,"skipped":2,"configured":14}
{"event":"indexer.ingest.verified","count":41}
{"event":"indexer.ingest.rejected","count":1,"by_reason":{"unsigned":0,"bad_signature":0,"cid_mismatch":0,"schema_unknown":0,"provenance":1}}
```

Event names other than the existing `verified`/`rejected` and ADR-024's `source_skipped` are
illustrative. DESIGN owns the exact shape (OQ-IPF-4).

## Step 6: what Maria sees

```text
$ openlore search --object org.openlore.philosophy.reproducible-builds

Network results for org.openlore.philosophy.reproducible-builds
(5 claims across 4 subjects, 3 distinct authors)
=================================================================

Author: did:plc:priyaraman7x2k (priyaraman.bsky.social)        (not subscribed)
  - github:priyaraman/cargo-pin   confidence 0.82 (well-evidenced)  [verified] [self-attested]  bafy...r7
Author: did:plc:dvolkov3m9q (dmitri.volkov.dev)                (not subscribed)
  - github:nixos/nixpkgs          confidence 0.71 (well-evidenced)  [verified]  bafy...q8
...
```

## Error paths (key)

| Path | Trigger | Outcome |
|---|---|---|
| E1 DID unresolvable, no fallback | PLC returns 404 for `did:plc:ghost0000` | skipped `did_unresolvable`, the other DIDs are indexed, retried next pass |
| E2 DID unresolvable, fallback set | same, with `OPENLORE_INDEXER_SOURCE_URL` set | listed from the fallback, app-signed indexed, self-attested refused (provenance) |
| E3 PDS down | `pds.volkov.dev` returns 502 or times out | skipped `pds_unreachable`/`pds_timeout`, Dmitri's earlier claims stay searchable |
| E4 PDS moved | Priya migrates to `https://pds.priyaraman.dev` | next pass reads from the new PDS, and her claims are still admitted |
| E5 Wrong repo | DID doc points at a PDS that answers with another repo's records | those records are refused and never attributed to the requested DID |
| E6 Bad config | `OPENLORE_INDEXER_REPO_DIDS=did:plc:ok,priya` or fallback `ftp://x` | refuse start, naming the variable and value |
