# ADR-025: Network Index Store = a Separate DuckDB File (`index.duckdb`) with Anti-Merging Carried to Network Scale

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-103 (cardinal anti-merging) + WD-104 (verified-before-index) for openlore-appview-search (slice-05)
- **Feature**: openlore-appview-search (slice-05)
- **Extends**: ADR-001 (DuckDB single-file store), ADR-014 (peer-storage schema + the three-layer anti-merging invariant + the `no_cross_table_join_elides_author` xtask rule), ADR-022 (anti-merging-in-aggregates). Inherits the slice-03 `FederatedRow` non-`Option<Did>` discipline.
- **Resolves**: the index-store choice (OD reuse-vs-search-engine) + the WD-103 anti-merging-at-network-scale enforcement.

## Context

The indexer needs a store for the searchable network index: signature-verified,
per-author-attributed PUBLIC claims, queryable by object (philosophy), subject
(project), and contributor (DID). Two axes of decision:

1. **Store technology**: reuse DuckDB (a new index DB/tables, consistent with
   ADR-001) vs introduce a dedicated search engine (Tantivy, Meilisearch,
   Elasticsearch, etc.).
2. **Schema shape**: how the index preserves per-author attribution (WD-103 / the
   cardinal trust guarantee) so that NO search result is ever a faceless
   "network-consensus" row — extending the slice-03 I-FED-1 / slice-04
   I-GRAPH-1/2 anti-merging invariant into network-scale aggregates.

The forces:

- **WD-103 is cardinal**: every indexed/searched/shared result MUST preserve
  per-author attribution; the index MUST have NO schema for a merged multi-author
  record; identical claims by different authors render as separate rows. Any
  attribution loss is unshippable (KPI-AV-2 disprover).
- **WD-104 verified-before-index**: every indexed record was signature-verified +
  CID-recomputed before insert; every row carries the author DID it was verified
  against.
- **Search semantics for the walking skeleton are exact-match dimensional**, not
  full-text relevance ranking: `--object <philosophy URI>`, `--contributor <DID>`,
  `--subject <project URI>` are EXACT keyed lookups over structured fields (with a
  near-match SUGGESTION for typos, US-AV-002 Example 4) — NOT free-text search
  over claim prose. This is the decisive force: the walking-skeleton search is a
  keyed/indexed lookup, exactly what slice-04's DuckDB dimension queries already do
  over `claims`/`peer_claims`.
- **ADR-001 consistency + dependency conservatism**: reusing DuckDB adds zero new
  heavy dependency, reuses the slice-01/03/04 anti-merging SQL enforcement
  substrate, and reuses the slice-03 `peer_claims`-shaped attributed-row pattern.

## Decision

**The network index is a SEPARATE DuckDB single-file store, `index.duckdb`, owned
by the `openlore-indexer` binary (NOT the user's `openlore.duckdb`). It reuses
DuckDB (ADR-001 consistency; no new search-engine dependency). Its schema mirrors
the slice-03 `peer_claims` shape — a single `indexed_claims` table with a
non-`Option` (`NOT NULL` + `CHECK <> ''`) `author_did` per row — so that
aggregation across authors happens at QUERY time in the query layer, never as a
stored merged row. The anti-merging invariant (WD-103) is enforced at three
semantically orthogonal layers, extending the slice-03 `no_cross_table_join_elides_author`
xtask rule to the index query path.**

### Why a SEPARATE `index.duckdb` (not the user's `openlore.duckdb`)

| Factor | Separate `index.duckdb` — **CHOSEN** | Reuse the user's `openlore.duckdb` |
|---|---|---|
| **WD-106 "indexer never overwrites/merges a local claim"** | Structural: the indexer's store is a different file; it cannot touch the user's source-of-truth claims. | The indexer would hold a handle to the user's store — a capability-boundary violation (ADR-023). |
| **Lifecycle** | The index is re-buildable from the network (ingest can be re-run); the user's local store is the source of truth (never re-buildable). Different durability/backup concerns. | Commingling a re-buildable cache with the source of truth muddies backup + recovery semantics. |
| **Ownership (ADR-023)** | The indexer binary owns `index.duckdb`; the CLI owns `openlore.duckdb`. Clean capability separation. | The two binaries would share a file — concurrent-access + capability-coupling problems. |
| **Anti-merging enforcement reuse** | The slice-03 `no_cross_table_join_elides_author` xtask SQL rule extends to the index-store crate's SQL literals on the SAME substrate. | n/a (rejected for the reasons above). |

### Why DuckDB (not a dedicated search engine)

| Factor | Reuse DuckDB — **CHOSEN** | Dedicated search engine (Tantivy / Meilisearch / Elasticsearch) |
|---|---|---|
| **Search semantics (slice-05)** | Exact dimensional keyed lookups (`object` / `author_did` / `subject`) over structured fields, with a near-match suggestion for typos — DuckDB indexed lookups + a simple edit-distance suggestion query handle this directly. | Over-provisioned: full-text relevance ranking, tokenization, and scoring are not what `--object <URI>` needs. The walking skeleton does NOT do free-text prose search. |
| **Dependency cost** | ZERO new heavy dependency; DuckDB is already pinned (ADR-001); the index-store adapter reuses the slice-01/03 connection/migration/probe patterns. | A new heavy dependency (a search-engine crate or, worse, an external service like Elasticsearch/Meilisearch — which would re-introduce the very "central service" concern ADR-023 avoids), a new license review, a new `cargo deny` entry, a new probe surface. |
| **Anti-merging substrate (WD-103, cardinal)** | The slice-03/04 `no_cross_table_join_elides_author` SQL-string rule EXTENDS naturally onto the index store's SQL literals — the cardinal trust guarantee reuses a proven enforcement substrate. | A non-SQL search engine would need a NEW structural enforcement mechanism for the anti-merging invariant — re-inventing the load-bearing guarantee on an unproven substrate. This is the decisive rejection: WD-103 is cardinal, and DuckDB lets us reuse the enforcement. |
| **Operational simplicity (ADR-023 self-hostable single binary)** | Single embedded file; no external service to run alongside the indexer. | An external search service breaks the single-binary self-hostable model and re-introduces a service-to-operate. |
| **Future relevance/full-text** | DuckDB has a built-in FTS extension if free-text claim search becomes a JTBD; revisit trigger documented. | Premature now; the right call only when evidence shows free-text relevance ranking is a J-005 need (currently it is not — search is dimensional). |

### Index schema (`index.duckdb`, migration v1 of the index store)

The schema mirrors the slice-03 `peer_claims` attributed-row pattern. The
LOAD-BEARING decision: ONE `indexed_claims` table where `author_did` is
`NOT NULL` + `CHECK (author_did <> '')` — there is NO table, view, or column for a
merged multi-author record.

```sql
-- openlore-indexer index store; idempotent forward-only.
-- Registered as index_schema_version(version=1, applied_at=now(),
--   description='slice-05 network index'). SEPARATE file from openlore.duckdb (ADR-023).

-- The network index: signature-verified, per-author-attributed PUBLIC claims.
-- LOAD-BEARING (WD-103): author_did is NEVER NULL, NEVER empty. There is NO
-- schema anywhere for a merged multi-author "consensus" record. An aggregate
-- (e.g., "all claims for an object") is COMPOSED at query time from these
-- individually-attributed rows, NEVER stored merged.
CREATE TABLE IF NOT EXISTS indexed_claims (
    cid                 VARCHAR PRIMARY KEY,           -- bafy... (network-published CID; recomputed + verified at ingest, WD-104)
    author_did          VARCHAR NOT NULL,              -- did:plc:...#kid; LOAD-BEARING — verified-against (ADR-026); never NULL, never elided
    subject             VARCHAR NOT NULL,              -- project URI (search dimension)
    predicate           VARCHAR NOT NULL,
    object              VARCHAR NOT NULL,              -- philosophy URI (the headline search dimension)
    confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),  -- numeric only (WD-10 / I-6)
    composed_at         TIMESTAMP NOT NULL,            -- from signed payload composedAt
    -- Provenance of the ingest event (local to the indexer; NOT part of signed payload):
    indexed_at          TIMESTAMP NOT NULL,            -- when this row entered the index
    source_pds          VARCHAR NOT NULL,              -- the PDS/relay URL the record was pulled from (ADR-024)
    signed_record_path  VARCHAR NOT NULL,              -- absolute path to the verified signed-record JSON file (index store dir)
    verified_against    VARCHAR NOT NULL,              -- the author DID-doc verification key id the signature verified against (ADR-026); never empty
    CHECK (author_did <> ''),                          -- defense-in-depth: forbid empty author (anti-merging)
    CHECK (cid <> ''),
    CHECK (verified_against <> '')                     -- defense-in-depth: every row was verified (WD-104)
);

CREATE INDEX IF NOT EXISTS idx_indexed_object       ON indexed_claims (object);       -- --object lookup
CREATE INDEX IF NOT EXISTS idx_indexed_author       ON indexed_claims (author_did);   -- --contributor lookup
CREATE INDEX IF NOT EXISTS idx_indexed_subject      ON indexed_claims (subject);      -- --subject lookup
CREATE INDEX IF NOT EXISTS idx_indexed_composed_at  ON indexed_claims (composed_at);

-- Evidence URIs for indexed claims (denormalized; same shape as claim_evidence / peer_claim_evidence).
CREATE TABLE IF NOT EXISTS indexed_claim_evidence (
    cid         VARCHAR NOT NULL,
    evidence    VARCHAR NOT NULL,
    ordinal     INTEGER NOT NULL,
    PRIMARY KEY (cid, ordinal),
    FOREIGN KEY (cid) REFERENCES indexed_claims (cid)
);

-- Reference graph for indexed claims (denormalized from references[]; enables the
-- OD-AV-7 "counter relationship shown, not applied" annotation at query time).
CREATE TABLE IF NOT EXISTS indexed_claim_references (
    referencing_cid     VARCHAR NOT NULL,
    referenced_cid      VARCHAR NOT NULL,              -- may resolve to another indexed_claims row OR neither
    ref_type            VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
    PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
    FOREIGN KEY (referencing_cid) REFERENCES indexed_claims (cid)
);

CREATE INDEX IF NOT EXISTS idx_indexed_refs_referenced ON indexed_claim_references (referenced_cid);
```

There is deliberately **no `UNIQUE` on `(author_did, subject, predicate, object)`**
(same reasoning as ADR-014: an author may publish multiple claims on the same
tuple over time; the CID PK is the only uniqueness constraint). And there is
deliberately **no `consensus` / `merged` / `aggregate` table** of any kind — the
absence is the design (WD-103).

### Anti-merging at network scale (WD-103) — three-layer enforcement

The invariant: **NO index query and NO search/share render surface MAY emit a row
that represents claims from more than one author DID as a single entity.** This
extends slice-03 I-FED-1 (storage/query/display) and slice-04 I-GRAPH-1/2
(aggregates) to NETWORK aggregates. Enforced in three semantically orthogonal
layers (mirroring ADR-014 / ADR-022):

| Layer | What it checks | Tool |
|---|---|---|
| **Subtype / type** | The index query methods return `Vec<IndexedClaim>` (and search results carry `Vec<NetworkResultRow>`) where every row carries a non-`Option<Did>` `author_did` — compile-error if dropped. There is NO type in the system representing a multi-author merged result; an `ObjectSearchResult` is a `Vec` of attributed rows grouped by author in the renderer, never a merged struct. (Mirrors slice-03 `FederatedRow` + slice-04 `Contribution`.) | Rust trait + type system |
| **Structural / SQL** | `cargo xtask check-arch` extends the slice-03/04 rule `no_cross_table_join_elides_author` to cover the index-store crate's SQL string literals: any literal that aggregates over `indexed_claims` (GROUP BY / COUNT / SUM across authors) without projecting `author_did` fails CI. The walking-skeleton search queries are per-author-projecting SELECTs with NO `GROUP BY author`; counts (e.g., "9 distinct authors") are computed in the query layer from the returned attributed rows, never via an author-eliding SQL aggregate. | `xtask check-arch` (rule extended to the index store) |
| **Behavioral / acceptance** | Release-gate integration test `network_result_preserves_attribution`: index two records by DISTINCT authors on the SAME (subject, object), run `--object`, assert (a) exactly two result rows, (b) two distinct non-empty `author_did`s, (c) NO merged "consensus" row exists, (d) the index has no row/table representing both combined. Plus the `--share` resolver test asserting the shared query re-composes per-author rows (never a stored snapshot). | `tests/network_result_preserves_attribution.rs` (DISTILL gate; KPI-AV-2) |

A single-layer bypass is caught by at least one of the other two. This is the
direct carry of the cardinal trust guarantee into the new network-scale failure
surface.

### OD-AV-7 (retraction/counter-aware search) at the schema level

Per WD-11 + OD-AV-7 default: a countered/soft-retracted claim that was published
+ verified IS indexed and IS discoverable normally; the counter RELATIONSHIP is
SHOWN (when both the claim and its counter are in the index, via
`indexed_claim_references`), never silently applied as a filter or a down-weight.
The schema supports this with the `indexed_claim_references` table; the query
layer annotates `countered-by <cid> (by <author_did>)` when known, mirroring the
slice-03 coexist semantics. A retraction-aware search FILTER is deferred (a future
WD + ADR).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **A dedicated search engine (Tantivy embedded)** | Rejected for the walking skeleton. The slice-05 search is EXACT dimensional keyed lookup over structured fields (`object`/`author_did`/`subject`), not free-text relevance ranking — DuckDB indexed lookups handle it directly. Tantivy adds a new heavy dependency + a NEW structural-enforcement substrate for the cardinal anti-merging invariant (WD-103), the decisive rejection. Revisit if free-text claim-prose search becomes a J-005 JTBD. |
| **An external search service (Meilisearch / Elasticsearch)** | Hard reject. Re-introduces a "central service to operate" — the exact concern ADR-023 (self-hostable single binary) avoids — plus a network dependency, an ops surface, and a non-SQL anti-merging-enforcement substrate. Catastrophic for the local-first/sovereignty ethos. |
| **Reuse the user's `openlore.duckdb` (one file for local + network)** | Rejected. Violates WD-106 (indexer never touches a local claim) + the ADR-023 capability boundary; commingles a re-buildable network cache with the source-of-truth local store. Separate `index.duckdb`. |
| **A stored `consensus` / aggregate / merged-view table for fast "object → N authors agree" rendering** | Hard reject — the cardinal WD-103 violation. A merged row collapses provenance, the exact aggregator failure the product exists to replace. Aggregates (counts, groupings) are composed at query time from individually-attributed rows; the index has NO merged schema. This is the load-bearing absence. |
| **DuckDB FTS extension for the walking-skeleton search** | Deferred. The walking-skeleton search is dimensional/exact, not full-text; FTS is the documented revisit path if claim-prose search becomes a need. Adding it now is premature complexity. |
| **A `verified BOOLEAN` column (so the index could hold unverified rows for later verification)** | Rejected. There is no "unverified" state in the index by construction (WD-104): verification is an INGEST gate (ADR-024), so every row is verified. A `verified` boolean would invite an unverified row to exist; instead `verified_against` is `NOT NULL` + `CHECK <> ''` (every row WAS verified). The `[verified]` marker is a construction guarantee, not a stored flag to interpret (US-AV-004). |

## Consequences

### Positive

- Zero new heavy dependency; reuses ADR-001 DuckDB, the slice-03 attributed-row
  schema pattern, and — decisively — the proven `no_cross_table_join_elides_author`
  anti-merging enforcement substrate for the cardinal WD-103 guarantee.
- Separate `index.duckdb` makes the WD-106 "indexer never touches a local claim"
  guarantee + the ADR-023 capability boundary structural.
- The single-`indexed_claims`-table-with-non-`Option`-author_did shape makes
  per-author attribution the only representable shape; a merged consensus row is
  un-writable because no schema for it exists.
- `verified_against NOT NULL` makes "every indexed row was verified" a schema
  invariant (WD-104), so the `[verified]` marker is a construction guarantee.
- Self-hostable single-binary operational model preserved (single embedded file,
  no external service).

### Negative

- **Two DuckDB files in the system** (`openlore.duckdb` + `index.duckdb`), owned by
  two binaries. Mitigation: they are disjoint by ownership (ADR-023); the index is
  re-buildable, so its backup story is "re-ingest", not "back up". No cross-file
  transaction is ever needed (the CLI never reads `index.duckdb` directly — it
  queries the indexer over HTTP, ADR-027).
- **No relevance ranking** in the walking skeleton (results are dimensional, not
  scored). Accepted: cross-user network-scale SCORING is explicitly DEFERRED
  (DISCUSS / WD-79); the walking skeleton ranks nothing. The DuckDB FTS revisit
  path exists if free-text relevance becomes a need.
- **The two-tables-look-alike DRY temptation** (same as ADR-014's negative): a
  future contributor might want to UNION `peer_claims` and `indexed_claims`. They
  must NOT — they live in different files owned by different binaries. The
  migration-file comment block cites this ADR.

### Earned Trust

The `IndexStoreAdapter` (driven port for the index store) ships a `probe()` within
the 250ms budget (ADR-009 I-4/I-5) that exercises, in addition to the inherited
ADR-001 schema-version + fsync substrate checks:

1. **Attribution round-trip**: write two sentinel `indexed_claims` rows with the
   SAME (subject, object) and DISTINCT non-empty `author_did`s; query `--object`;
   assert exactly two rows with the two distinct author DIDs read back byte-equal
   (the anti-merging-at-network-scale substrate check — detects any DuckDB column
   coercion or accidental dedup-by-tuple that would collapse authors).
2. **The substrate-lie scenario (catalogued, fsync)**: inherited from ADR-001 —
   the index store probes that `fsync` is honored on the deployment substrate
   (Docker overlayfs no-op, WSL2 DrvFs, tmpfs). The indexer is likely to run in a
   container; if the substrate lies about durability, the probe refuses to start
   with `health.startup.refused{reason: storage.fsync_unhonored}`. This is the
   slice-05 indexer's "what if the environment lies?" check.
3. **No-merge-schema assertion**: the probe asserts there is NO table named
   `consensus` / `merged_claims` / any aggregate table in the index schema (a
   structural defense that the cardinal anti-merging shape was not silently
   amended).

## Revisit Trigger

- `indexed_claims` grows beyond what a single-file DuckDB serves within the search
  latency budget (dogfood evidence). Consider partitioning, a separate
  per-host shard, or — if free-text relevance is also needed — re-evaluate a
  dedicated search engine.
- Free-text claim-prose search becomes a J-005 JTBD (users want to search the
  `reason` text, not just dimensional fields). Add the DuckDB FTS extension (a
  built-in, no new heavy dependency) before considering an external engine.
- Cross-user network-scale SCORING (deferred per WD-79) lands. The scoring would
  read `indexed_claims` and compute weights in a PURE core at query time (reusing
  the slice-04 `scoring` pattern + the anti-merging-in-aggregates discipline);
  NOTHING is persisted merged (WD-72 carries forward).
- A shared/community indexer (ADR-023 revisit) needs multi-host coverage merging.
  The merge happens at INGEST (each host ingests + verifies independently into its
  own attributed rows), never as a stored consensus row.
