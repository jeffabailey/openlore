# Data Models — openlore-federated-read (slice-03) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-27
- **Architect**: Morgan
- **Authoritative for**: the `reason` field extension to `org.openlore.claim` Lexicon; the slice-03 DuckDB schema additions (`peer_subscriptions`, `peer_claims`, `peer_claim_references`, `peer_claim_evidence`); the `peer_claims/<did>/<cid>.json` on-disk artifact layout
- **Extends**: `docs/feature/openlore-foundation/design/data-models.md`

## Three representations, one logical peer claim

A peer claim exists in four places, each with a different representation
(symmetric with the slice-01 author-claim model per ADR-006):

| Where | Representation | Purpose |
|---|---|---|
| In-memory (Rust) | `claim_domain::SignedClaim` value (now with optional `reason`) | All pure transformations and verb handlers; SAME type used for author and peer claims |
| Peer-published canonical | The signed-claim CBOR on the peer's PDS | Source of truth across the federation; recomputed locally for CID verification |
| On-disk artifact (local cache) | JSON file at `~/.local/share/openlore/peer_claims/<peer_did>/<cid>.json` | Greppable canonical local artifact partitioned by peer for clean purge |
| In-DB index | rows across `peer_claims` + `peer_claim_references` + `peer_claim_evidence` tables in DuckDB | Indexed lookup; cross-store federated query |
| CID input | RFC 8949 canonical CBOR encoding | Used ONLY for `compute_cid()`; never stored; recomputed at pull time |

**Invariant** (extends slice-01's): the peer JSON file under
`peer_claims/<peer_did>/<cid>.json` and the corresponding `peer_claims` row
MUST both deserialize to a `SignedClaim` that canonicalizes (via CBOR) to
bytes whose `sha2-256` multihash matches the filename `<cid>` AND the row
PK `peer_claims.cid`. The `author` field in the signed payload MUST match
`peer_claims.author_did` byte-equal — the row's attribution is derived
from the signed payload, not asserted separately.

## Lexicon: `org.openlore.claim` — slice-03 extension

The schema is EXTENDED with one optional field. NO field is removed; NO
required field is added (preserves ADR-005 forward-compat).

```json
{
  "lexicon": 1,
  "id": "org.openlore.claim",
  "defs": {
    "main": {
      "type": "record",
      "key": "any",
      "description": "A signed philosophical claim about a subject (project, person, artifact). Slice-03 adds the optional `reason` field for counter-claims; forward-compatible with slice-01.",
      "record": {
        "type": "object",
        "required": ["subject", "predicate", "object", "confidence", "author", "composedAt"],
        "properties": {
          "subject":     { "type": "string", "description": "(slice-01) URI of the thing being claimed about." },
          "predicate":   { "type": "string", "description": "(slice-01) The relation." },
          "object":      { "type": "string", "description": "(slice-01) URI of the philosophy or counterparty." },
          "evidence":    { "type": "array", "items": { "type": "string", "format": "uri" }, "description": "(slice-01) Zero or more evidence URIs." },
          "confidence":  { "type": "number", "minimum": 0.0, "maximum": 1.0, "description": "(slice-01) Numeric only; buckets are display-only." },
          "author":      { "type": "string", "description": "(slice-01) Author DID + key fragment." },
          "composedAt":  { "type": "string", "format": "datetime", "description": "(slice-01) RFC3339 UTC." },
          "references":  {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["type", "cid"],
              "properties": {
                "type": { "type": "string", "enum": ["retracts", "corrects", "counters", "supersedes"] },
                "cid":  { "type": "string" }
              }
            },
            "description": "(slice-01; ADR-008) Zero or more typed references to other claims. Slice-03 reuses the existing `counters` variant; no new variant added."
          },
          "reason": {
            "type": "string",
            "minLength": 1,
            "maxLength": 1000,
            "description": "(slice-03; ADR-015) Optional free-text explanation. REQUIRED by the `claim counter` verb (CLI-level enforcement); permitted but semantically unused on other claim types. UTF-8 NFC-normalized at compose time. Byte-stable across the federation round-trip per ADR-006 canonicalization."
          },
          "signature":   {
            "type": "object",
            "required": ["kid", "alg", "sig"],
            "properties": {
              "kid": { "type": "string", "description": "(slice-01) DID key fragment." },
              "alg": { "type": "string", "enum": ["EdDSA"] },
              "sig": { "type": "string", "description": "(slice-01) Base64url signature over the unsigned-claim CID." }
            }
          }
        }
      }
    }
  }
}
```

Notes on the extension:

- `reason` is OPTIONAL at the wire level (NOT in `required[]`) per ADR-015 to preserve forward-compat with slice-01 binaries.
- `reason` is REQUIRED at the CLI verb level (`openlore claim counter --reason "..."`) per WD-20; the CLI rejects pre-compose if the flag is missing or empty.
- `claim-domain::validate_counter_claim` is the belt-and-braces check: if `references[]` contains a `counters` entry AND `reason` is None/empty, reject before canonicalization.
- The serde struct gains `#[serde(default, skip_serializing_if = "Option::is_none")] pub reason: Option<String>` — a claim with `reason: None` serializes byte-equal to a slice-01 claim, preserving CID stability across the upgrade.

### CID stability across slice-01 -> slice-03 upgrade

Per ADR-006, canonical CBOR field order is lexicographic by key bytes. The
new `reason` field is between `references` and `signature` in lex order,
which matters ONLY for claims that CARRY the field. A claim with
`reason: None` is serialized by `skip_serializing_if` to byte-identical
output as the slice-01-era binary.

Property test in `claim-domain`:

```
property: forall non_counter_claim C (no reason):
    cid_slice_01(C) == cid_slice_03(C with reason: None)
property: forall reason text R:
    normalize_reason(R) == normalize_reason(normalize_reason(R))   // idempotent
property: forall R, S where R != S and NFC(R) == NFC(S):
    normalize_reason(R) == normalize_reason(S)                     // NFC-unifying
```

## DuckDB schema — slice-03 additions (migration v3)

Path unchanged: `~/.local/share/openlore/openlore.duckdb`.

The slice-01 tables (`claims`, `claim_evidence`, `claim_references`) are
UNCHANGED. The slice-03 migration creates 4 new tables and registers
itself as `schema_version (version=3, ...)`.

```sql
-- Slice-03 migration; idempotent forward-only.
-- Registered as schema_version(version=3, applied_at=now(), description='slice-03 peer storage').
-- See ADR-014.

-- Subscriptions: one row per peer DID the user has chosen to follow.
-- `removed_at` distinguishes "active" from "soft-removed-but-cache-retained"
-- per WD-25. Hard-purge DELETEs the row entirely.
CREATE TABLE IF NOT EXISTS peer_subscriptions (
    peer_did            VARCHAR PRIMARY KEY,           -- did:plc:... (no fragment; the fragment lives in the per-claim signature.kid)
    peer_handle         VARCHAR NOT NULL,              -- cached at subscribe; advisory only
    peer_pds_endpoint   VARCHAR NOT NULL,              -- cached at subscribe; re-resolved at each pull
    subscribed_at       TIMESTAMP NOT NULL,
    removed_at          TIMESTAMP                      -- NULL while active; soft-remove sets this
);

CREATE INDEX IF NOT EXISTS idx_peer_subs_active
    ON peer_subscriptions (peer_did)
    WHERE removed_at IS NULL;

-- Peer claims: signed claims authored by peers, NOT by the current user.
-- LOAD-BEARING: author_did is NEVER NULL, NEVER empty. The anti-merging
-- invariant (I-FED-1) makes every cross-store query carry this column.
CREATE TABLE IF NOT EXISTS peer_claims (
    cid                 VARCHAR PRIMARY KEY,           -- bafy... (peer-published CID; recomputed locally and verified)
    author_did          VARCHAR NOT NULL,              -- did:plc:...#kid; LOAD-BEARING
    subject             VARCHAR NOT NULL,
    predicate           VARCHAR NOT NULL,
    object              VARCHAR NOT NULL,
    confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    composed_at         TIMESTAMP NOT NULL,            -- from signed payload composedAt
    fetched_at          TIMESTAMP NOT NULL,            -- local: when this row was pulled
    fetched_from_pds    VARCHAR NOT NULL,              -- local: PDS URL at pull time (may differ from peer_subscriptions if peer rotated)
    signed_record_path  VARCHAR NOT NULL,              -- absolute path to peer_claims/<did>/<cid>.json
    -- NO FK on author_did (intentional; soft-remove leaves dangling per WD-25).
    CHECK (author_did <> ''),
    CHECK (cid <> '')
);

CREATE INDEX IF NOT EXISTS idx_peer_claims_author       ON peer_claims (author_did);
CREATE INDEX IF NOT EXISTS idx_peer_claims_subject      ON peer_claims (subject);
CREATE INDEX IF NOT EXISTS idx_peer_claims_composed_at  ON peer_claims (composed_at);

-- Reference graph for peer claims (denormalized from references[] field).
-- Same shape as the slice-01 claim_references table; SEPARATE table preserves
-- the author-store / peer-store separation invariant per ADR-014.
CREATE TABLE IF NOT EXISTS peer_claim_references (
    referencing_cid     VARCHAR NOT NULL,              -- the peer claim's CID (resides in peer_claims)
    referenced_cid      VARCHAR NOT NULL,              -- the target CID (may resolve to claims OR peer_claims OR neither)
    ref_type            VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
    PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
    FOREIGN KEY (referencing_cid) REFERENCES peer_claims (cid)
);

CREATE INDEX IF NOT EXISTS idx_peer_claim_refs_referenced ON peer_claim_references (referenced_cid);

-- Evidence URIs for peer claims (denormalized; same shape as claim_evidence).
CREATE TABLE IF NOT EXISTS peer_claim_evidence (
    cid         VARCHAR NOT NULL,
    evidence    VARCHAR NOT NULL,
    ordinal     INTEGER NOT NULL,
    PRIMARY KEY (cid, ordinal),
    FOREIGN KEY (cid) REFERENCES peer_claims (cid)
);
```

Migration policy:

- Forward-only; idempotent. Slice-01 data (rows in `claims`,
  `claim_evidence`, `claim_references`) is BIT-PRESERVED — the migration
  adds tables only, never alters existing ones.
- The `schema_version` table (slice-01) registers `version=3` with
  description `'slice-03 peer storage'`. Slices may install in any order;
  each migration is independent.
- The `adapter-duckdb::probe()` rejects start with
  `storage.schema_mismatch` if the schema_version is HIGHER than the
  binary knows about (forward-incompatibility refusal).
- On a fresh `openlore init`, the migration runs from version=0 through
  version=3 idempotently; on an existing slice-01 database (version=1),
  the migration jumps to 3 (version=2 reserved for slice-02 if installed
  separately; safe to skip if absent).

## On-disk artifact format — slice-03 additions

### Existing slice-01 path (unchanged)

`~/.local/share/openlore/claims/<cid>.json` — user's own signed claims.
The slice-03 CLI ALSO writes user-authored counter-claims here (per
ADR-014: counter-claims authored by the user are the user's own claims).

### NEW slice-03 path

`~/.local/share/openlore/peer_claims/<peer_did>/<cid>.json`

- Partitioned by peer DID — makes hard-purge a directory removal.
- `<peer_did>` is the DID with safe filesystem encoding: colons become
  underscores for filesystem portability (e.g., `did:plc:rachel-test`
  becomes `did_plc_rachel-test`). DELIVER confirms the exact encoding
  scheme; the design constraint is "round-trippable + safe on all three
  target platforms (macOS APFS, Linux ext4, WSL2 DrvFs)."
- File content is the same `SignedClaim` JSON shape as slice-01 author
  claims (with `reason` field present if the peer published a
  counter-claim).
- Write strategy: same `<cid>.json.tmp` -> fsync -> rename atomic pattern
  as slice-01.
- Approximate size: 400-700 bytes per claim (counter-claims with reason
  text average larger).

### Example peer counter-claim file

`~/.local/share/openlore/peer_claims/did_plc_rachel-test/bafy...n4kb.json`:

```json
{
  "subject": "github:rust-lang/cargo",
  "predicate": "embodiesPhilosophy",
  "object": "org.openlore.philosophy.dependency-pinning",
  "evidence": ["https://github.com/rust-lang/cargo/issues/5359"],
  "confidence": 0.42,
  "author": "did:plc:rachel-test#org.openlore.application",
  "composedAt": "2026-05-22T09:18:44Z",
  "references": [
    { "type": "counters", "cid": "bafy...m9pq" }
  ],
  "reason": "Disagree that this is philosophical; pinning is a tool, evidence above.",
  "signature": {
    "kid": "did:plc:rachel-test#org.openlore.application",
    "alg": "EdDSA",
    "sig": "MEUCIQDz...base64url..."
  }
}
```

Note: a peer's published counter-claim looks structurally identical to
the user's own counter-claim. The only difference is which directory it
lives in locally (`claims/` vs `peer_claims/<peer_did>/`) and which DB
table indexes it (`claims` vs `peer_claims`).

## Shared artifact ↔ data model mapping (slice-03 additions)

Per `shared-artifacts-registry.md`, the slice-03 artifacts resolve to:

| Shared artifact | Source of truth |
|---|---|
| `peer_did` | `peer_subscriptions.peer_did` PK AND `peer_claims.author_did` for that peer's rows; argument to `peer add`/`peer remove`. Compared byte-equal across all touchpoints; the `peer_did_round_trip` integration test asserts this. |
| `peer_claim_cid` | `peer_claims.cid` PK; the on-disk filename suffix at `peer_claims/<peer_did>/<cid>.json`; computed by `claim-domain::compute_cid()` at pull time and verified byte-equal against the peer-published rkey. |
| `peer_pds_endpoint` | Resolved fresh from `peer_did`'s DID document at every pull; the cached `peer_subscriptions.peer_pds_endpoint` is advisory only. |
| `subscribed_at` | `peer_subscriptions.subscribed_at` column; set by `ClockPort::now_utc()` at `peer add` time. |
| `target_cid` (counter-claim target) | The user's CLI arg AND the `references[].cid` value in the counter-claim's signed payload AND the row PK lookup against EITHER `claims.cid` OR `peer_claims.cid` (the target may be the user's own claim or a peer's). |
| `counter_claim_reason` | `--reason` CLI argument AND the `reason` field in the counter-claim's signed payload (NFC-normalized) AND the rendered reason line in `graph query --federated` output. |
| `counter_claim_cid` | Computed at sign time by `claim-domain::compute_cid()`; stored as `claims.cid` (user's counter-claims live in user's own table) AND as the on-disk filename `claims/<cid>.json`. |
| `purge_flag` | The `--purge` CLI flag on `peer remove`; routes dispatch to `PeerStoragePort::hard_purge` vs `soft_remove`. Not persisted. |

## Validation rules — translated from shared-artifacts-registry to data assertions

| Registry rule | Data-model assertion |
|---|---|
| `peer_did` matches across subscribe/pull/query/remove | `peer_subscriptions.peer_did` is set exactly once at subscribe; `peer_claims.author_did` is derived from the signed payload's `author` field (parsed to drop the fragment) at write; CLI re-reads from the row at every render. The `peer_did_round_trip` test asserts byte-equality across all touchpoints. |
| `peer_claim_cid` recomputed at pull time matches the peer-published rkey | `VerbPeerPull` calls `claim_domain::compute_cid` on the canonical CBOR of the fetched record and rejects (does NOT write) if the result differs from the peer's published rkey. The `peer_cid_round_trip` integration test asserts this against fixture peers. |
| Per-claim signature verifies against peer's DID-doc key | `VerbPeerPull` calls `claim_domain::verify` with the peer's verification key (resolved via `IdentityPort::resolve_peer`) and rejects (does NOT write) on failure. The `peer_tampered_signature_rejected` test enforces this against an adversarial fixture. |
| Anti-merging: NO cross-store query elides author_did | Three layers: (a) `FederatedRow.author_did` is `Did`, not `Option<Did>` — compile-error if dropped; (b) `xtask check-arch` rule `no_cross_table_join_elides_author` — fails CI on SQL string literals that join `claims`+`peer_claims` without `author_did`; (c) integration test `federation_attribution_preserved`. |
| Soft-remove preserves peer_claims | `PeerStoragePort::soft_remove` updates `peer_subscriptions.removed_at` ONLY; the probe asserts row count of `peer_claims` is unchanged after invocation. |
| Hard-purge deletes peer_claims for that peer ONLY | `PeerStoragePort::hard_purge` runs a single DuckDB transaction with three DELETEs scoped by `author_did` AND removes the filesystem directory `peer_claims/<peer_did>/`. The user's `claims` table is untouched. Acceptance test `peer_remove_purge_separation` enforces this. |
| User's counter-claims survive peer hard-purge | The slice-01 `claims` table is never targeted by the hard-purge transaction. User counter-claims have `references[].cid` pointing at the (now-deleted) peer claim CIDs; the federated-query renderer annotates these as `counters <cid> (peer not subscribed)`. |
| `reason` field byte-stable across federation round-trip | Lexicon serde + canonicalization order is fixed (ADR-006); NFC normalization happens once at compose time and the normalized bytes are what get signed. Property test in `claim-domain` asserts round-trip identity. |

## Confidence buckets stay UNPERSISTED (inherits WD-10 / OD-2)

Slice-03 does NOT change this. Buckets exist only in the CLI render
layer; neither author claims nor peer claims carry a bucket label
anywhere. The `peer_claims` table has the same numeric `confidence DOUBLE`
column as `claims`, with the same `CHECK (confidence >= 0.0 AND confidence
<= 1.0)` constraint. The render-time mapping
(`claim-domain::confidence_bucket`) is invoked for both author and peer
claim rows; persistence of a bucket string in any table or any on-disk
file is a CI-failable invariant (existing slice-01 unit test extends to
include `peer_claims` and `peer_claim_evidence`).

## Cross-store query examples (for renderer)

The renderer for `graph query --subject <S> --federated` consumes
`StoragePort::query_federated_by_subject` and groups by author DID. The
implementation MUST satisfy the anti-merging invariant (I-FED-1).

**SAFE pattern** (UNION ALL with explicit author projection):

```sql
SELECT
    c.author_did AS author_did,
    c.cid        AS cid,
    c.predicate  AS predicate,
    c.object     AS object,
    c.confidence AS confidence,
    c.composed_at AS composed_at,
    c.artifact_path AS artifact_path,
    'Own'        AS source_table
FROM claims c
WHERE c.subject = ?subject

UNION ALL

SELECT
    pc.author_did AS author_did,
    pc.cid        AS cid,
    pc.predicate  AS predicate,
    pc.object     AS object,
    pc.confidence AS confidence,
    pc.composed_at AS composed_at,
    pc.signed_record_path AS artifact_path,
    'Peer'        AS source_table
FROM peer_claims pc
WHERE pc.subject = ?subject;
```

The result deserializes into `Vec<FederatedRow>` where every row carries
`author_did` (compile-time non-Option). The renderer joins to
`peer_subscriptions WHERE removed_at IS NULL` to distinguish
`SubscribedPeer` vs `UnsubscribedCache` for the `Peer`-sourced rows.

**FORBIDDEN pattern** (would silently merge — caught by xtask check-arch):

```sql
-- This kind of JOIN cannot pass the I-FED-1 check; flagged by xtask check-arch
-- because it mentions both `claims` and `peer_claims` without `author_did` in
-- the SELECT projection.
SELECT c.subject, c.predicate, c.object, COUNT(*) AS total
FROM claims c
JOIN peer_claims pc ON c.subject = pc.subject AND c.predicate = pc.predicate
WHERE c.subject = ?subject
GROUP BY c.subject, c.predicate, c.object;
```

## OrientationState — identity.toml extensions

The slice-01 `~/.config/openlore/identity.toml` gains three new optional
keys under a `[federation]` section (per ADR-016 + the gherkin-scenarios-
expanded.md habit scenarios):

```toml
# Slice-01 sections unchanged.

[federation]
# Set to the RFC3339 UTC timestamp of the FIRST EVER successful invocation
# of each verb. Used to gate once-per-user orientation messages.
first_pull_completed_at = "2026-05-27T10:14:32Z"          # set after first `peer pull`
first_federated_query_completed_at = "2026-05-27T10:15:08Z"  # set after first `graph query --federated`
first_counter_claim_completed_at = "2026-05-28T09:42:11Z"   # set after first successful `claim counter`
```

Semantics:

- Absence of any key (or empty value) means "the corresponding orientation message MUST fire on the next invocation."
- The orientation message is emitted to stdout BEFORE the normal output for that verb (so scripts grepping for specific stdout lines still find them; the orientation is additive at the top).
- On success of the operation, the key is written; failure to write the key is logged but not fatal.
- The keys are local-only; no telemetry. The user can delete `identity.toml` to reset all three at once (e.g., for re-testing).
