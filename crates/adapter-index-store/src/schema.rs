//! `schema` — the `index.duckdb` migration v1 DDL, SKETCHED IN COMMENTS.
//!
//! Bootstrap SCAFFOLD (step 01-03): this module records the ADR-025 /
//! data-models.md §"Network index schema" DDL as a COMMENT sketch only. The
//! LIVE `CREATE TABLE` statements + the migration runner land in step 03-01.
//! Keeping the DDL in comments (not string literals) here means:
//!
//! - the bootstrap crate compiles with no `duckdb` calls yet, and
//! - the SQL is documented at its eventual home for the 03-01 author.
//!
//! ## The single load-bearing schema decision (WD-103)
//!
//! ONE `indexed_claims` table with a NON-`Option` (`NOT NULL` + `CHECK <> ''`)
//! `author_did`, plus two child tables (`indexed_claim_evidence`,
//! `indexed_claim_references`). There is NO `consensus` / `merged` / `aggregate`
//! table of ANY kind — the absence is the design. Aggregates
//! (`distinct_author_count`) are composed at QUERY time in the PURE
//! `appview-domain` core, NEVER stored as a merged row nor produced by an
//! author-eliding SQL aggregate (the extended `no_cross_table_join_elides_author`
//! `xtask check-arch` rule enforces this structurally on this crate's SQL).
//
// SCAFFOLD: true  (DDL is a COMMENT sketch only; live DDL lands in step 03-01)
//
// -----------------------------------------------------------------------------
// index.duckdb migration v1 — SKETCH (verbatim from ADR-025 / data-models.md).
// SEPARATE file from openlore.duckdb (ADR-023). Idempotent, forward-only.
// Register as index_schema_version(version=1, ..., 'slice-05 network index').
// -----------------------------------------------------------------------------
//
// CREATE TABLE IF NOT EXISTS indexed_claims (
//     cid                 VARCHAR PRIMARY KEY,           -- verified == compute_cid(payload) (WD-104)
//     author_did          VARCHAR NOT NULL,              -- LOAD-BEARING (WD-103); == signed payload author
//     subject             VARCHAR NOT NULL,              -- search dimension
//     predicate           VARCHAR NOT NULL,
//     object              VARCHAR NOT NULL,              -- the headline search dimension
//     confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),  -- numeric only (WD-10)
//     composed_at         TIMESTAMP NOT NULL,
//     indexed_at          TIMESTAMP NOT NULL,            -- when this row entered the index (provenance)
//     source_pds          VARCHAR NOT NULL,              -- pull provenance (ADR-024)
//     signed_record_path  VARCHAR NOT NULL,              -- indexed_claims/<author_did>/<cid>.json
//     verified_against    VARCHAR NOT NULL,              -- the DID-doc key id verified against (ADR-026); WD-104
//     CHECK (author_did <> ''),                          -- anti-merging defense-in-depth
//     CHECK (cid <> ''),
//     CHECK (verified_against <> '')                     -- verified-before-index defense-in-depth (WD-104)
// );
// CREATE INDEX IF NOT EXISTS idx_indexed_object       ON indexed_claims (object);
// CREATE INDEX IF NOT EXISTS idx_indexed_author       ON indexed_claims (author_did);
// CREATE INDEX IF NOT EXISTS idx_indexed_subject      ON indexed_claims (subject);
// CREATE INDEX IF NOT EXISTS idx_indexed_composed_at  ON indexed_claims (composed_at);
//
// CREATE TABLE IF NOT EXISTS indexed_claim_evidence (
//     cid VARCHAR NOT NULL, evidence VARCHAR NOT NULL, ordinal INTEGER NOT NULL,
//     PRIMARY KEY (cid, ordinal), FOREIGN KEY (cid) REFERENCES indexed_claims (cid)
// );
//
// CREATE TABLE IF NOT EXISTS indexed_claim_references (
//     referencing_cid VARCHAR NOT NULL, referenced_cid VARCHAR NOT NULL,
//     ref_type VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
//     PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
//     FOREIGN KEY (referencing_cid) REFERENCES indexed_claims (cid)
// );
// CREATE INDEX IF NOT EXISTS idx_indexed_refs_referenced ON indexed_claim_references (referenced_cid);
//
// -----------------------------------------------------------------------------
// Notes carried from data-models.md (03-01 author must honor):
//   - NO UNIQUE on (author_did, subject, predicate, object): an author may
//     publish multiple claims on the same tuple over time; CID PK is the only
//     uniqueness constraint; de-dup at upsert is by CID (ADR-025).
//   - NO consensus/merged/aggregate table of ANY kind (WD-103); the probe
//     asserts no such table exists.
//   - NO `verified BOOLEAN` column: there is no "unverified" state by
//     construction; `verified_against NOT NULL` records every row WAS verified.
// -----------------------------------------------------------------------------
