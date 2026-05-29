# Data Models — openlore-appview-search (slice-05) — DELTA from slice-04

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Authoritative for**: the NETWORK INDEX schema (`index.duckdb`: `indexed_claims` + `indexed_claim_evidence` + `indexed_claim_references`); the slice-05 ingest/search ADTs (`RawRecord`, `IngestOutcome`, `IndexedClaim`, `NetworkResultRow`, `NetworkSearchResult`); the `org.openlore.appview.searchClaims` XRPC query DTOs; the production PLC pubkey-decode value (`VerificationKey`); the anti-merging-preserving query SHAPES. **The user's `openlore.duckdb` is UNCHANGED** (the indexer never touches it — ADR-023/025). **No Lexicon change to `org.openlore.claim`** (nothing new is signed; the only Lexicon addition is the READ query lexicon).
- **Extends**: `docs/feature/openlore-scoring-graph/design/data-models.md` (slice-04, inherited unchanged) + `docs/feature/openlore-federated-read/design/data-models.md` (the `FederatedRow` non-`Option` author_did discipline this mirrors)

## The single most important data fact of slice-05

**Slice-05 adds a SEPARATE store (`index.duckdb`), owned by the
`openlore-indexer` binary, and DOES NOT change the user's `openlore.duckdb`.**
The network index is a re-buildable cache of signature-verified,
per-author-attributed PUBLIC claims; the user's local store remains the
source of truth the indexer cannot touch (WD-106 / ADR-023).

The index has **NO schema for a merged multi-author "consensus" record** — the
load-bearing absence (WD-103). Every `indexed_claims` row carries a
non-`Option` (`NOT NULL` + `CHECK <> ''`) `author_did`; aggregates (counts,
groupings) are composed at QUERY time from individually-attributed rows in the
PURE `appview-domain` core (Rust), NEVER as a stored merged row or a SQL
author-eliding aggregate.

Nothing new is SIGNED: the `org.openlore.claim` Lexicon is UNCHANGED; the only
Lexicon addition is a READ query (`org.openlore.appview.searchClaims`) with no
signed payload + no CID-stability concern.

## Four representations, one logical indexed claim

A network-indexed claim exists in several representations (symmetric with the
slice-01 author-claim + slice-03 peer-claim models per ADR-006/014):

| Where | Representation | Purpose |
|---|---|---|
| Network-published canonical | The signed-claim CBOR on the author's PDS | Source of truth across the network; recomputed locally for CID verification at INGEST |
| In-memory (Rust, indexer) | `IndexedClaim` value (non-`Option` `author_did`) | The boundary value `appview-domain` + the index-store adapter + the query handler share |
| On-disk artifact (index cache) | JSON at `<index dir>/indexed_claims/<author_did>/<cid>.json` | Greppable canonical artifact, partitioned by author for clean per-author purge (mirrors `peer_claims/<did>/`) |
| In-DB index | rows across `indexed_claims` + `indexed_claim_references` + `indexed_claim_evidence` in `index.duckdb` | Indexed dimensional lookup |
| Over the wire (CLI ← indexer) | `SearchQueryResponse` JSON (per-result `author_did` always present) | The XRPC query result the CLI renders (ADR-027) |
| CID input | RFC 8949 canonical CBOR | Used ONLY for `compute_cid()` at ingest verification; never stored; recomputed |

**Invariant** (extends slice-01/03): the indexed JSON file, the `indexed_claims`
row, and the wire response MUST all deserialize to a claim that canonicalizes to
bytes whose `sha2-256` multihash matches the `cid`, AND the `author` field in the
signed payload MUST match `indexed_claims.author_did` byte-equal — the row's
attribution is DERIVED from the signed payload, not asserted separately. AND the
signature MUST have verified against the key recorded in `verified_against`
(ADR-026) BEFORE the row existed (WD-104).

## In-memory value types (the ingest + search ADTs)

| Type | Where defined | Persisted? | Purpose |
|---|---|---|---|
| `RawRecord` | `ports` | NO — transient ingest input | A fetched-but-not-yet-verified network record (the `IngestSourcePort::enumerate` output). |
| `IngestOutcome` | `appview-domain` | NO — the gate decision | `Index(IndexedClaim)` \| `Reject(RejectReason)`; the PURE verify-before-index decision (WD-104). |
| `IndexedClaim` | `ports` | YES — as an `indexed_claims` row + a JSON artifact | The verified, attributed indexed claim. `author_did` non-`Option`; `verified_against` never empty (Gate: verified-before-index). |
| `NetworkResultRow` | `appview-domain` | NO — computed per query | One result row; non-`Option` `author_did`; the unit the renderer emits. |
| `NetworkSearchResult` | `appview-domain` | NO — computed per query | The per-author-grouped result; `distinct_author_count` is a COUNT over rows, never a merge (anti-merging). |
| `VerificationKey` | `claim-domain` | NO — resolved per ingest | The Ed25519 key decoded from the author's PLC DID-doc `z6Mk...` (ADR-026); feeds the pure `verify`. |
| `SearchQueryRequest`/`Response` | `lexicon` | NO — over the wire | The XRPC query DTOs (per-result `author_did` always present, ADR-027). |

### The verified-before-index invariant, made concrete

```
property: a record enters indexed_claims ONLY if appview_domain::ingest_decision returns Index, i.e.:
    claim_domain::verify(record, resolved_key) == Ok
    AND claim_domain::compute_cid(record) == record.published_cid
    AND record.author (signed payload) == indexed_claims.author_did (byte-equal)
property: after any ingest pass:
    no indexed_claims row exists whose verified_against is empty
    no tampered/unsigned/CID-mismatch fixture record appears in indexed_claims or any search result
    a subsequent --object/--contributor/--subject search NEVER returns a rejected record
```

Enforced by the release gate `indexer_rejects_unverified_claim` (KPI-AV-3) + the
ingest-adapter probe (rejects a fixture tampered/CID-mismatch record) + the
`verified_against NOT NULL` + `CHECK <> ''` schema constraints (ADR-025).

### The anti-merging-at-network-scale invariant, made concrete

```
property: after indexing two records by DISTINCT authors on the SAME (subject, object):
    a --object search returns exactly TWO NetworkResultRows
    with TWO distinct, non-empty author_dids
    grouped under their respective authors (NetworkSearchResult.by_author has both)
    distinct_author_count == 2
    NO row/struct/table represents both claims combined
    no indexed_claims SQL aggregates over object/subject dropping author_did
property: opening a --share link re-composes the per-author result (never a stored snapshot)
```

Enforced by the release gate `network_result_preserves_attribution` (KPI-AV-2) +
the three-layer enforcement (type / extended `no_cross_table_join_elides_author`
xtask rule on `adapter-index-store` / behavioral).

## The ingest/search ADTs (Rust)

```rust
// ports — the fetched-but-unverified record (IngestSourcePort::enumerate output)
pub struct RawRecord {
    pub published_cid: Cid,         // the network-published rkey/CID (recomputed + verified at ingest)
    pub raw_payload: SignedClaim,   // the signed-claim value (author/subject/object/confidence/signature/...)
    pub source_pds: String,         // the PDS/relay URL it was pulled from (provenance; not signed)
}

// appview-domain — the PURE verify-before-index decision (WD-104)
pub enum IngestOutcome {
    Index(IndexedClaim),
    Reject(RejectReason),
}
pub enum RejectReason { Unsigned, BadSignature, CidMismatch, SchemaUnknown }

// ports — the verified, attributed indexed claim (an indexed_claims row + a JSON artifact)
pub struct IndexedClaim {
    pub author_did: Did,            // non-Option; LOAD-BEARING (anti-merging, WD-103); == signed payload author
    pub cid: Cid,                   // verified == compute_cid(payload) (WD-104)
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,            // numeric [0.0,1.0] (WD-10 / I-6)
    pub composed_at: DateTime<Utc>,
    pub verified_against: KeyId,    // the DID-doc key id the signature verified against (ADR-026); NEVER empty (WD-104)
    pub evidence: Vec<String>,
    pub references: Vec<ClaimReference>,   // for the OD-AV-7 counter annotation
    pub relationship: AuthorRelationship,  // resolved CLI-side (you/subscribed-peer/unsubscribed-cache/network-unfollowed)
}

// appview-domain — the search result (computed per query; NEVER persisted)
pub struct NetworkResultRow {
    pub author_did: Did,            // non-Option; LOAD-BEARING
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub verified_against: KeyId,    // drives the [verified] marker (never empty)
    pub relationship: AuthorRelationship,
    pub counter_annotation: Option<CounterRef>,   // OD-AV-7: shown, never applied
}

pub struct NetworkSearchResult {
    pub by_author: Vec<(Did, Vec<NetworkResultRow>)>,  // per-author; NO merged-author row
    pub distinct_author_count: u32,                    // COUNT over rows; not a merge
    pub total_claims: u32,
    pub suggestion: Option<String>,                    // near-match for empty result
}
```

`AuthorRelationship` (slice-03 enum + one new variant):
`You | SubscribedPeer | UnsubscribedCache | NetworkUnfollowed`. The
`NetworkUnfollowed` variant drives the `(not subscribed)` label + the `peer add`
follow affordance (US-AV-005). The relationship is resolved CLI-side by checking
the result's `author_did` against the user's `peer_subscriptions` (the index
itself stores no per-user relationship — it is per-user-neutral).

## Network index schema (`index.duckdb`, migration v1 of the index store)

The full DDL is in ADR-025. Restated here for the data-model record. **SEPARATE
file from `openlore.duckdb`** (ADR-023). Mirrors the slice-03 `peer_claims`
attributed-row pattern; the LOAD-BEARING decision is the single
`indexed_claims` table with non-`Option` `author_did` + NO merged schema.

```sql
-- openlore-indexer index store; idempotent forward-only.
-- Registered as index_schema_version(version=1, ..., description='slice-05 network index').

CREATE TABLE IF NOT EXISTS indexed_claims (
    cid                 VARCHAR PRIMARY KEY,           -- verified == compute_cid(payload) (WD-104)
    author_did          VARCHAR NOT NULL,              -- LOAD-BEARING (WD-103); == signed payload author
    subject             VARCHAR NOT NULL,              -- search dimension
    predicate           VARCHAR NOT NULL,
    object              VARCHAR NOT NULL,              -- the headline search dimension
    confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),  -- numeric only (WD-10)
    composed_at         TIMESTAMP NOT NULL,
    indexed_at          TIMESTAMP NOT NULL,            -- when this row entered the index (provenance)
    source_pds          VARCHAR NOT NULL,              -- pull provenance (ADR-024)
    signed_record_path  VARCHAR NOT NULL,              -- indexed_claims/<author_did>/<cid>.json
    verified_against    VARCHAR NOT NULL,              -- the DID-doc key id verified against (ADR-026); WD-104
    CHECK (author_did <> ''),                          -- anti-merging defense-in-depth
    CHECK (cid <> ''),
    CHECK (verified_against <> '')                     -- verified-before-index defense-in-depth (WD-104)
);
CREATE INDEX IF NOT EXISTS idx_indexed_object       ON indexed_claims (object);
CREATE INDEX IF NOT EXISTS idx_indexed_author       ON indexed_claims (author_did);
CREATE INDEX IF NOT EXISTS idx_indexed_subject      ON indexed_claims (subject);
CREATE INDEX IF NOT EXISTS idx_indexed_composed_at  ON indexed_claims (composed_at);

CREATE TABLE IF NOT EXISTS indexed_claim_evidence (
    cid VARCHAR NOT NULL, evidence VARCHAR NOT NULL, ordinal INTEGER NOT NULL,
    PRIMARY KEY (cid, ordinal), FOREIGN KEY (cid) REFERENCES indexed_claims (cid)
);

CREATE TABLE IF NOT EXISTS indexed_claim_references (
    referencing_cid VARCHAR NOT NULL, referenced_cid VARCHAR NOT NULL,
    ref_type VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
    PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
    FOREIGN KEY (referencing_cid) REFERENCES indexed_claims (cid)
);
CREATE INDEX IF NOT EXISTS idx_indexed_refs_referenced ON indexed_claim_references (referenced_cid);
```

- **No `UNIQUE` on `(author_did, subject, predicate, object)`** — same reasoning as
  ADR-014: an author may publish multiple claims on the same tuple over time; the
  CID PK is the only uniqueness constraint; de-dup at upsert is by CID.
- **No `consensus`/`merged`/`aggregate` table of ANY kind** — the absence is the
  design (WD-103). The index-store probe asserts no such table exists.
- **No `verified BOOLEAN` column** — there is no "unverified" state by construction
  (verification is an ingest gate); `verified_against NOT NULL` records that every
  row WAS verified. The `[verified]` marker is a construction guarantee, not a
  stored flag to interpret.

### On-disk artifact format

`<index dir>/indexed_claims/<author_did>/<cid>.json` — the verified network-claim
artifact, partitioned by author DID (mirrors `peer_claims/<did>/`). Same DID→safe-
filename encoding as slice-03 (`did:plc:priya-test` → `did_plc_priya-test`; DELIVER
confirms). File content is the same `SignedClaim` JSON shape as slice-01/03 (it IS
the author's published record, verified). Write strategy: `<cid>.json.tmp` → fsync
→ rename atomic (slice-01 pattern; the fsync the index-store probe verifies on the
container substrate).

## Read-side query shapes (over the `indexed_claims` table)

All slice-05 search queries read the `indexed_claims` table with EXPLICIT
`author_did` projection (NEVER an author-eliding aggregate) — the slice-03/04
anti-merging SQL discipline (I-FED-1 / I-GRAPH-2) extended to the index store
(I-AV-2). Aggregation (the `distinct_author_count`) happens in the PURE
`appview-domain` core (Rust), NOT in SQL.

### Dimension query — by object (philosophy) — the headline

```sql
-- query_by_object(?object): SAFE pattern (explicit author_did; no aggregation across authors)
SELECT ic.author_did, ic.cid, ic.subject, ic.predicate, ic.object,
       ic.confidence, ic.composed_at, ic.verified_against, ic.signed_record_path
FROM indexed_claims ic
WHERE ic.object = ?object;
-- Returns Vec<IndexedClaim> (author_did non-Option). appview_domain::compose_results
-- groups by author in the PURE core; distinct_author_count is COUNT(DISTINCT author_did)
-- computed over the returned rows in Rust, NEVER via a SQL GROUP BY that drops author_did.
```

### Dimension query — by contributor (DID)

```sql
-- query_by_contributor(?author_did): SAFE pattern
SELECT ic.author_did, ic.cid, ic.subject, ic.predicate, ic.object,
       ic.confidence, ic.composed_at, ic.verified_against, ic.signed_record_path
FROM indexed_claims ic
WHERE ic.author_did = ?author_did;
-- One DID's whole network trail. The relationship label + "one developer's reasoning trail,
-- not a community consensus" footer are CLI-side render concerns.
```

### Dimension query — by subject (project)

```sql
-- query_by_subject(?subject): SAFE pattern
SELECT ic.author_did, ic.cid, ic.subject, ic.predicate, ic.object,
       ic.confidence, ic.composed_at, ic.verified_against, ic.signed_record_path
FROM indexed_claims ic
WHERE ic.subject = ?subject;
-- Grouped by author in the PURE core; NO "the network thinks X about this project" merged row.
```

### Counter-relationship annotation (OD-AV-7 — shown, not applied)

```sql
-- For a result set, look up counter relationships among indexed claims (NEVER filters the result):
SELECT icr.referencing_cid, icr.referenced_cid, icr.ref_type, ic2.author_did AS counter_author
FROM indexed_claim_references icr
JOIN indexed_claims ic2 ON ic2.cid = icr.referencing_cid       -- the COUNTERING claim (carries its author)
WHERE icr.referenced_cid IN (?result_cids) AND icr.ref_type IN ('counters','retracts');
-- NOTE: this JOIN is over indexed_claims↔indexed_claim_references (same store, same author projected);
-- it is NOT a cross-store join and it PROJECTS counter_author (anti-merging preserved). The annotation
-- is added to NetworkResultRow.counter_annotation in the PURE core; the countered row is NEVER removed.
```

### FORBIDDEN pattern (would merge in SQL, hiding attribution — caught by `xtask check-arch`)

```sql
-- DO NOT: aggregate across authors in SQL. Flagged because it aggregates over indexed_claims
-- (object/subject) and the GROUP BY drops author_did → anti-merging violation (I-AV-2).
SELECT object, COUNT(*) AS faux_consensus, AVG(confidence) AS faux_network_confidence
FROM indexed_claims
GROUP BY object;   -- author_did eliminated → a faceless "network consensus" row → WD-103 violation
```

## The XRPC query DTOs (CLI ← indexer; ADR-027)

```
org.openlore.appview.searchClaims  (a `query` lexicon; a READ query; no signed payload)

Request (query params):
    dimension : "object" | "contributor" | "subject"
    value     : <philosophy URI | did | project URI>
    cid?      : <cid>     (for --show: inspect one result)

Response (200):
{
  "results": [
    { "author_did": "did:plc:priya-test", "cid": "bafy...k2",
      "subject": "github:bazelbuild/bazel", "predicate": "embodiesPhilosophy",
      "object": "org.openlore.philosophy.reproducible-builds", "confidence": 0.82,
      "composed_at": "...", "verified_against": "did:plc:priya-test#org.openlore.application",
      "evidence": ["..."], "references": [...] },
    ...
  ],
  "distinct_author_count": 9,
  "total_claims": 12
}

Response (empty dimension): { "results": [], "suggestion": "<near-match>" }  -- CLI exits 0

INVARIANT (anti-merging across the transport, I-AV-2): EVERY element of `results`
carries `author_did`. There is NO `consensus` / `merged` / `aggregate` object in
the response shape. Grouping by author is the CLI renderer's job (the wire carries
flat attributed rows; the CLI groups via appview_domain::compose_results-equivalent
or re-uses the same pure core). The response carries `verified_against` (drives the
[verified] marker; every result was verified at ingest).
```

The `--share` link encodes ONLY the query (dimension + value):
`openlore://search?object=org.openlore.philosophy.reproducible-builds` — NO
results, NO snapshot, NO merged view (WD-110 / I-AV-8). Opening it re-runs the
query → current per-author-attributed verified results.

## Why a separate store + no new persisted aggregate (the WD-103/106 contract restated)

| Candidate | Decision | Rationale |
|---|---|---|
| The network index | SEPARATE `index.duckdb` (indexer-owned) | The indexer never touches the user's source-of-truth `openlore.duckdb` (WD-106 / ADR-023); the index is re-buildable. |
| A `consensus` / `merged` / `aggregate` row or table | NOT persisted; NO schema for it | The load-bearing absence (WD-103). A merged row collapses provenance — the aggregator failure the product replaces. Aggregates are composed at query time in the pure core. |
| `distinct_author_count` / "N authors" footer | DERIVED at query time | A COUNT over attributed rows in the pure core; never a stored aggregate. |
| `verified` boolean | NOT a column | No "unverified" state exists (ingest gate); `verified_against NOT NULL` records that every row was verified. |
| confidence bucket | NOT persisted (inherits WD-10) | Display-only render; numeric `[0.0,1.0]` is the only persisted/indexed confidence. |
| Cross-user score / weight | NOT persisted; NOT computed in slice-05 | Cross-user network-scale scoring is DEFERRED (WD-79); the index ranks nothing. A future scoring would compute in a pure core + persist nothing merged (WD-72 carries forward). |

## Shared artifact ↔ data model mapping (slice-05)

Per `shared-artifacts-registry.md` + the visual journey, the slice-05 artifacts
resolve to:

| Shared artifact | Source of truth |
|---|---|
| `subject` (project URI) | `indexed_claims.subject`; the `--subject` query key + the `--share` encoding; byte-equal across ingest/index/search/share. |
| `object` (philosophy URI) | `indexed_claims.object`; the headline `--object` query key + the `--share` encoding; near-match suggestion for typos (US-AV-002 Ex 4). |
| `author_did` (contributor) | `indexed_claims.author_did` (derived from the signed payload `author`); carried into EVERY `IndexedClaim`, `NetworkResultRow`, and wire `results` element as non-`Option<Did>` (Gate I-AV-2). |
| `claim_cid` | `indexed_claims.cid` PK; the `--show` key + the verified addressable unit; recomputed + verified at ingest (Gate I-AV-1). |
| `confidence` (numeric) | `indexed_claims.confidence` (`DOUBLE`); numeric-only persisted/indexed (WD-10); display bucket render-only. |
| `verified_marker` (`[verified]`) | DERIVED from `indexed_claims.verified_against` (never empty); a construction guarantee (ingest gate), never a per-result runtime guess (I-AV-1). |
| `verified_against` | `indexed_claims.verified_against`; the DID-doc key id the signature verified against (ADR-026); `--show` renders "Signature: VERIFIED against <did>". |
| `relationship_label` | Resolved CLI-side: the result `author_did` checked against the user's `peer_subscriptions` (you/subscribed-peer/unsubscribed-cache/network-unfollowed). The index is per-user-neutral. |
| `share_link` (query-encoding) | Encodes `dimension`+`value` only; resolves back to a current search (never a stored snapshot; I-AV-8). |
| `VerificationKey` | DECODED from the author's PLC DID-doc `publicKeyMultibase` (`z6Mk...`) by `claim_domain::decode_ed25519_multibase` (ADR-026); resolved at ingest; never persisted. |

## Validation rules — translated to data assertions

| Registry rule / Gate | Data-model assertion |
|---|---|
| `indexer_rejects_unverified_claim` (KPI-AV-3; I-AV-1) | A record enters `indexed_claims` ONLY if `appview_domain::ingest_decision` returns `Index` (verify + CID both pass via the pure core); `verified_against NOT NULL`; tampered/unsigned/CID-mismatch fixtures never produce a row. |
| `network_result_preserves_attribution` (KPI-AV-2; I-AV-2) | Every `IndexedClaim`/`NetworkResultRow`/wire-`results`-element carries a non-empty `author_did`; two distinct-author rows on the same (subject,object) produce two result rows; no merged row/table exists; the index-store SQL never aggregates across authors. |
| `verified_marker_is_universal` (I-AV-1) | Every search result carries `[verified]` (derived from `verified_against`); there is no `[unverified]` state. |
| `public_data_banner_shown` (KPI-AV-5; I-AV-4) | The banner is printed before results; the ingest path reads only public `listRecords` (no auth-scoped/private read). |
| `local_first_preserved` (KPI-5; I-AV-3) | `claim add`/offline `claim publish`/`graph query` write/read `openlore.duckdb` with NO indexer dependency; `search` with the indexer down is non-fatal. |
| `discovery_follow_reuses_slice03_path` (KPI-AV-4; I-AV-7) | The follow affordance prints the slice-03 `peer add` command; no parallel subscription row exists; after `peer add` + `peer pull` the author's claims are in `peer_claims` (the slice-03 store), participating in local `graph query`. |
| `share_link_encodes_query_not_snapshot` (KPI-AV-6; I-AV-8) | The `--share` link contains only `dimension`+`value`; no result payload; opening re-runs the query. |
| `countered_claim_still_appears` (OD-AV-7; I-AV-9) | A `references[type=counters]` relationship adds a `counter_annotation`; the countered `indexed_claims` row is still returned by search. |
| `no_pubkey_seam_in_release_build` (I-AV-6) | A release binary does not read `OPENLORE_PEER_PUBKEY_HEX_<did>`; production verification uses the real PLC decode. |

## Confidence buckets stay UNPERSISTED (inherits WD-10 / I-6)

Slice-05 does NOT change this. The `indexed_claims` table has the same numeric
`confidence DOUBLE` column as `claims`/`peer_claims` with the same `CHECK`
constraint. The render-time bucket mapping (`claim-domain::confidence_bucket`) is
invoked for search-result rows exactly as for local query rows; persistence of a
bucket string in the index store or any artifact is a CI-failable invariant (the
slice-01/03/04 no-persist unit test extends to scan `index.duckdb` +
`indexed_claims/` artifacts).

## indexer config — NEW (the indexer's own config, NOT identity.toml)

The indexer reads its OWN config (separate from the CLI's `identity.toml`):

```toml
# <indexer config path>, e.g. ~/.config/openlore-indexer/config.toml
[indexer]
index_path     = "~/.local/share/openlore-indexer/index.duckdb"
listen_addr    = "127.0.0.1:7619"            # the HTTP/XRPC query surface (ADR-027)
plc_endpoint   = "https://plc.directory"     # DID-document resolution (ADR-026)
ingest_interval = "15m"                       # bounded-pull cadence (ADR-024; DELIVER tunes)

[indexer.sources]
seed_dids = ["did:plc:...", "..."]            # bounded seed set (ADR-024)
relay     = "https://relay.example..."        # optional; still PULL, not a firehose subscription
```

The CLI's `identity.toml` gains ONE optional key for the indexer URL (so `search`
knows where to query):

```toml
[appview]
indexer_url = "http://127.0.0.1:7619"   # the self-hosted indexer (ADR-023/027); localhost default
```

Both are local-only, no telemetry. The CLI never reads the indexer's config; the
indexer never reads `identity.toml` (the two binaries are config-disjoint, ADR-023).
