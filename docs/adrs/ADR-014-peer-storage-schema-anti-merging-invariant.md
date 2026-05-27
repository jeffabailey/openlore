# ADR-014: Peer Storage Schema — `peer_subscriptions` + `peer_claims` Tables with Anti-Merging Invariant

- **Status**: Accepted
- **Date**: 2026-05-27
- **Deciders**: Morgan (nw-solution-architect), per WD-19/WD-25 locks from Luna (nw-product-owner) for openlore-federated-read
- **Feature**: openlore-federated-read (slice-03)
- **Extends**: ADR-001 (DuckDB local storage). The slice-01 `claims`, `claim_evidence`, `claim_references` tables remain unchanged. This ADR ADDS new tables to the same single-file DuckDB store.

## Context

slice-03 introduces two new storage surfaces:

1. **`peer_subscriptions`**: which DIDs the user has chosen to follow, when, and at what cached PDS endpoint. Subscription is binary in slice-03 (no weighting; that's slice-04).
2. **`peer_claims`**: signed claims authored by peers (NOT the current user), ingested after passing per-claim signature + CID verification. Every row is attributed to a specific peer DID; no row may exist without one.

The **anti-merging invariant** (J-003a, KPI-FED-1, KPI-FED-2) is the
load-bearing trust contract of slice-03: at no point may any query or any
render surface emit a row that represents two authors' claims as a single
"consensus" entry. Storage design must make this invariant ENFORCEABLE,
not just achievable.

DISCUSS locked the layout (WD-19): single DuckDB file with two new tables
alongside `claims`, enforced by `xtask check-arch`. Alternatives (separate
DB file, new adapter crate) were rejected on simplicity grounds.

DESIGN owns:

1. The exact column shapes and constraints on both new tables.
2. The foreign-key relationship between them, which MUST allow dangling FK after soft-remove (per WD-25).
3. The forward-only schema migration from slice-01.
4. The enforcement mechanism for the anti-merging invariant (clippy lint vs CHECK constraint vs xtask rule vs all three).
5. The probe responsibilities for the extended `adapter-duckdb`.

## Decision

**Add two tables to the existing single-file DuckDB store with a
deliberately weak foreign-key relationship and a three-layer enforcement
of the anti-merging invariant.**

### Schema additions (slice-03 forward migration)

```sql
-- Slice-03 schema migration; idempotent forward-only.
-- Inserted into schema_version as (version=3, applied_at=now(), description='slice-03 peer storage')
-- Existing tables (claims, claim_evidence, claim_references) remain unchanged.

-- Subscriptions: one row per peer DID the user has chosen to follow.
CREATE TABLE IF NOT EXISTS peer_subscriptions (
    peer_did            VARCHAR PRIMARY KEY,           -- did:plc:... (no fragment)
    peer_handle         VARCHAR NOT NULL,              -- cached at subscribe; advisory only
    peer_pds_endpoint   VARCHAR NOT NULL,              -- cached at subscribe; re-resolved at each pull (per shared-artifacts-registry)
    subscribed_at       TIMESTAMP NOT NULL,            -- ClockPort.now_utc()
    removed_at          TIMESTAMP                      -- NULL while active; soft-remove sets this
);

CREATE INDEX IF NOT EXISTS idx_peer_subs_active
    ON peer_subscriptions (peer_did)
    WHERE removed_at IS NULL;

-- Peer claims: signed claims authored by peers, NOT by the current user.
-- Every row MUST have a non-NULL author_did. There is no JOIN with `claims`
-- (the author's own table) that elides this column.
CREATE TABLE IF NOT EXISTS peer_claims (
    cid                 VARCHAR PRIMARY KEY,           -- bafy... (peer-published CID; locally recomputed and verified at ingest)
    author_did          VARCHAR NOT NULL,              -- did:plc:...#kid; LOAD-BEARING — never NULL, never elided
    subject             VARCHAR NOT NULL,
    predicate           VARCHAR NOT NULL,
    object              VARCHAR NOT NULL,
    confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    composed_at         TIMESTAMP NOT NULL,            -- UTC
    -- Provenance of the ingest event itself (local-only; not part of signed payload):
    fetched_at          TIMESTAMP NOT NULL,            -- when this row was pulled
    fetched_from_pds    VARCHAR NOT NULL,              -- the PDS URL at pull time (may differ from peer_subscriptions.peer_pds_endpoint if peer rotated)
    signed_record_path  VARCHAR NOT NULL,              -- absolute path to the verified signed-record JSON file
    -- NO foreign-key enforcement on author_did. Soft-remove leaves rows
    -- whose author_did has no live peer_subscriptions row; this is INTENTIONAL.
    CHECK (author_did <> ''),                          -- defense-in-depth: forbid empty author
    CHECK (cid <> '')
);

CREATE INDEX IF NOT EXISTS idx_peer_claims_author       ON peer_claims (author_did);
CREATE INDEX IF NOT EXISTS idx_peer_claims_subject      ON peer_claims (subject);
CREATE INDEX IF NOT EXISTS idx_peer_claims_composed_at  ON peer_claims (composed_at);

-- Reference graph for peer claims (denormalized from each peer claim's references[] field).
-- Same shape as claim_references (slice-01); separate table to preserve the
-- author-store / peer-store separation invariant. Cross-store queries that
-- need bidirectional `counters` / `countered-by` annotation UNION across both
-- reference tables; they NEVER JOIN claims with peer_claims directly without
-- carrying author_did through.
CREATE TABLE IF NOT EXISTS peer_claim_references (
    referencing_cid     VARCHAR NOT NULL,              -- the peer claim's CID
    referenced_cid      VARCHAR NOT NULL,              -- the target CID (may resolve to author OR peer claims OR neither)
    ref_type            VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
    PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
    FOREIGN KEY (referencing_cid) REFERENCES peer_claims (cid)
);

CREATE INDEX IF NOT EXISTS idx_peer_claim_refs_referenced ON peer_claim_references (referenced_cid);

-- Evidence URIs for peer claims (denormalized, same shape as claim_evidence).
CREATE TABLE IF NOT EXISTS peer_claim_evidence (
    cid         VARCHAR NOT NULL,
    evidence    VARCHAR NOT NULL,
    ordinal     INTEGER NOT NULL,
    PRIMARY KEY (cid, ordinal),
    FOREIGN KEY (cid) REFERENCES peer_claims (cid)
);
```

### Why `author_did` has no FK to `peer_subscriptions.peer_did`

Per WD-25: soft-remove drops the subscription but retains cached peer
claims (annotated `(unsubscribed cache)`). A live FK would force
ON DELETE CASCADE (deleting the subscription deletes the claims, which is
the `--purge` semantic) or ON DELETE RESTRICT (which would prevent
soft-remove from ever succeeding while cached claims exist).

Neither is correct. **The relationship is logical, not relational**. The
soft-remove branch deletes the `peer_subscriptions` row and leaves the
`peer_claims` rows dangling-by-author_did. Query-time logic distinguishes
"subscribed peer" from "unsubscribed cache" by joining `peer_claims` to
`peer_subscriptions WHERE removed_at IS NULL` and treating an unjoinable
row as "unsubscribed cache" without it being a corruption.

### Why no UNIQUE constraint on `(author_did, subject, predicate, object)` in `peer_claims`

A peer may legitimately publish multiple claims with the same
(subject, predicate, object) tuple over time (revisions, additional
evidence, evolving opinion). Each is a distinct signed claim with a
distinct CID. The CID PK is the only uniqueness constraint; the
(subject, predicate, object) tuple is INDEXED for query but not constrained.

This deviates from how `claims` (slice-01) implicitly worked (the user's
own table has the same lack of uniqueness, but in practice the user rarely
re-publishes; for peer claims it is the norm).

### Anti-merging invariant (I-FED-1) — three-layer enforcement

The invariant: **NO query and NO render surface MAY emit a row that
represents claims from more than one author DID as a single entity.** This
is the load-bearing J-003a contract; KPI-FED-2 hard-fails on any
violation.

Enforced in three semantically orthogonal layers (mirrors ADR-009's
three-layer probe-contract enforcement):

| Layer | What it checks | Tool |
|---|---|---|
| **Subtype / schema** | The `StoragePort::query_federated_by_subject` method returns `Vec<FederatedRow>` where every `FederatedRow` carries a non-`Option<Did>` `author_did` field (compile-time enforced; you cannot construct the row without one). The new `PeerStoragePort::list_peer_claims_by_subject` returns `Vec<(Did, SignedClaim)>` with the Did first. | Rust trait + type system |
| **Structural / SQL** | `cargo xtask check-arch` extends slice-01's rules with a new check: every SQL string literal in the `adapter-duckdb` crate that mentions BOTH `claims` and `peer_claims` MUST also mention `author_did` in its SELECT projection. Violations fail CI. The check is a regex pass plus an AST walker over `sqlx`-style string literals if/when used. | `xtask check-arch` (new rule `no_cross_table_join_elides_author`) |
| **Behavioral / acceptance** | Integration test `federation_attribution_preserved` populates `claims` with 1 row and `peer_claims` with 2 rows from 1 peer, runs `query_federated_by_subject(<S>)`, and asserts: (a) exactly 3 result rows, (b) every result row has a distinct `(author_did, cid)` tuple, (c) NO row's `author_did` equals an empty string, (d) two peer rows that share `(subject, predicate, object)` produce TWO distinct results, never one merged. | `tests/federation_attribution_preserved.rs` (DISTILL gate) |

A single-layer bypass is caught by at least one of the other two.

### Forward migration policy

- The slice-01 `schema_version` table records each applied migration. The slice-03 migration inserts version=3 (slice-02 reserves version=2 for its own migration; slices may be installed in any order, idempotently).
- The migration is forward-only. No ALTER on slice-01 tables; only CREATE on new tables. Running `openlore init` on a slice-01 database after upgrading to a slice-03 binary creates the new tables and updates `schema_version`. Slice-01 data is bit-preserved (US-FED-006 acceptance test).
- The `adapter-duckdb::probe()` (extended) asserts that the schema_version table reports a version the binary knows about (0, 1, 2, 3 — never higher). If higher, refuse to start with `health.startup.refused{reason: storage.schema_mismatch}` (same mechanism as ADR-001).

### Composition with existing `claims` table

The slice-01 `claims` table holds the user's OWN claims (renamed in the
mental model as `author_claims` for symmetry, but the SQL identifier stays
`claims` to avoid a slice-01 migration). `peer_claims` is its sibling.

| Concern | `claims` (slice-01) | `peer_claims` (slice-03) |
|---|---|---|
| Author | Always the local user (single value per install) | Always a peer (any DID except the local user's) |
| `at_uri`, `published_at` columns | Present (publish state) | Absent (every peer claim was already published; that's how it was fetched) |
| `artifact_path` column | Present (canonical JSON file under `claims/`) | Present as `signed_record_path` (parallel directory `peer_claims/<did>/<cid>.json` — see data-models.md for layout) |
| Counter-claims user authors | Stored here (user is the author of their own counter-claims) | Never stored here (a peer's counter-claim is a peer claim; if the peer counters the current user, the row lands in `peer_claims` with the peer's DID, references[].cid = the user's CID) |
| Purge semantics | NEVER purged (WD-11: no hard-delete of user's own published claims even via `--purge`) | Hard-purged by `peer remove --purge` (cached records are local-only; the source records still live on the peer's PDS) |

### Soft-remove vs hard-purge transactional shape

The `peer remove` verb's two modes are implemented as two distinct
transactions on the `PeerStoragePort` (see ADR-013 + component-boundaries):

```
soft-remove(did):
    BEGIN
    UPDATE peer_subscriptions SET removed_at = ClockPort.now_utc() WHERE peer_did = ?did
    COMMIT
    -- peer_claims rows untouched. Query layer joins to peer_subscriptions
    -- WHERE removed_at IS NULL to determine subscribed vs unsubscribed-cache.

hard-purge(did):
    BEGIN
    DELETE FROM peer_claim_evidence WHERE cid IN (SELECT cid FROM peer_claims WHERE author_did = ?did)
    DELETE FROM peer_claim_references WHERE referencing_cid IN (SELECT cid FROM peer_claims WHERE author_did = ?did)
    DELETE FROM peer_claims WHERE author_did = ?did
    DELETE FROM peer_subscriptions WHERE peer_did = ?did
    -- Filesystem: rm -rf ~/.local/share/openlore/peer_claims/<did>/
    COMMIT
    -- claims (author's own) untouched, including any counter-claims the user
    -- authored against this peer's CIDs. Those rows in claims.references_to
    -- now have unresolvable references; query layer annotates "(peer not subscribed)".
```

Both transactions MUST be atomic at the DuckDB level (single `BEGIN`/`COMMIT`).
The filesystem `rm -rf` for the peer's claim file directory happens AFTER
the DB commit (best-effort cleanup; orphaned files are harmless and
detectable by a probe).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Single `all_claims` table with an `is_own` boolean column** | Rejected. Conflates author and peer claims at the SQL level; the anti-merging invariant becomes harder to enforce because every query needs `WHERE is_own = TRUE` discipline; the slice-01 `claims` table would require a column ALTER (forbidden by our forward-only migration policy). |
| **Separate DuckDB file `peer_claims.duckdb`** | Rejected per WD-19 + alternatives-considered.md Choice 2 Option B. Federated query needs both stores in a single read; two-file transactions are awkward; backup target multiplication. |
| **FK on `peer_claims.author_did` -> `peer_subscriptions.peer_did` with ON DELETE CASCADE** | Rejected. Would force `--purge` semantics on every soft-remove; would prevent the WD-25 two-mode model. |
| **CHECK constraint `peer_claims.author_did != <local user's DID>`** | Considered as defense-in-depth against accidentally inserting an author claim into peer_claims. Rejected: the local DID is not known at schema-creation time, and would require DuckDB function evaluation in a CHECK that the embedded engine does not consistently support across versions. The check moves to the adapter's write path instead: `PeerStoragePort::write_peer_claim` rejects with `PeerStorageError::SelfAttribution` if `peer.author_did == identity.author_did()`. |
| **In-table partitioning by peer_did** | Premature; slice-03 peer claim counts are bounded in the low thousands. Re-evaluate at slice-04 if peer_claims exceeds 100k rows. |

## Consequences

### Positive

- Single DuckDB file; one migration; one backup target. Operational simplicity for P-002.
- The anti-merging invariant is enforced at three independent layers; a developer cannot accidentally write a query that elides author attribution without at least one layer flagging it.
- Soft-remove vs hard-purge maps cleanly to two distinct transactions; the boundary is explicit and testable.
- The peer storage extension reuses `adapter-duckdb` infrastructure (connection pooling, transaction helpers, probe wiring) — no new adapter crate to write, test, or maintain.
- Schema migration is forward-only and idempotent; slice-01 data is preserved bit-equal.

### Negative

- Two tables that look like `claims` invite a future contributor to "DRY" them via a UNION view or a single table with a discriminator column. The three-layer enforcement is the explicit answer; the comment block at the top of the migration file MUST explain why the duplication is intentional, citing this ADR.
- DuckDB has no native foreign-key enforcement for the deliberately-dangling `peer_claims.author_did -> peer_subscriptions.peer_did` relationship; the "subscribed vs unsubscribed cache" semantics live in the application layer (query joins to `peer_subscriptions WHERE removed_at IS NULL`). A schema reader cannot infer this from the DDL alone. **Mitigation**: comment in the migration file and a referenced ADR.
- `xtask check-arch`'s SQL-string regex pass is a structural check, not a true SQL parser; sophisticated query construction (e.g., string concatenation across functions) may evade it. **Mitigation**: the behavioral acceptance gate (`federation_attribution_preserved`) catches semantic violations even if the structural check is bypassed.

### Earned Trust

The `adapter-duckdb::probe()` (extended for slice-03) MUST exercise:

1. **Schema version reachable**: existing slice-01 probe extended — assert schema_version row exists for version 3 after init; refuse if newer.
2. **Peer-claim sentinel round-trip**: write a sentinel peer_claim row with a known CID, read it back, assert author_did is the EXACT bytes written. Detects DuckDB column type coercion (historically VARCHAR-to-int silent narrowing in some adapter versions).
3. **Soft-remove vs purge isolation**: write a peer subscription + 3 peer_claims rows, run soft-remove, assert peer_subscriptions row gone AND peer_claims rows present; then run hard-purge, assert both gone. Detects regression in the transaction shape.
4. **No-cross-store-elide round-trip**: write 1 row to `claims` (own) and 1 row to `peer_claims` (peer fixture), run `query_federated_by_subject(<S>)`, assert result is 2 rows with two distinct author_dids. Detects accidental UNION ALL that drops the author column.
5. **fsync honored** (inherited from ADR-001): unchanged.

The new `PeerStoragePort` ALSO ships a `probe()` (it is a distinct port,
per ADR-009 hexagonal invariant I-4). The PeerStoragePort probe exercises
points 2-4 above through its own contract surface, in addition to the
StoragePort probe exercising them through the underlying DuckDB adapter.
This is intentional redundancy: an adapter that satisfies one port may
still fail another's contract; both probes run at startup.

## Revisit Trigger

- `peer_claims` grows beyond ~100k rows in a single install (dogfeed
  evidence). Consider per-peer-DID partitioning, separate file, or
  switching to Kùzu / a graph DB alongside slice-04.
- A second federated source (e.g., a non-ATProto peer protocol) emerges.
  Schema may need a `source_protocol` column.
- GDPR right-to-erasure or similar regulatory requirement applies to
  cached peer data. Currently out-of-scope per slice-03's "peer DIDs and
  peer-published claims are public" framing (deferred to slice-05 AppView
  review). If it lands, hard-purge becomes the default and soft-remove
  needs a retention-window policy.
