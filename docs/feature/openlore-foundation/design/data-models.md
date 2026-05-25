# Data Models — openlore-foundation (slice-01)

- **Wave**: DESIGN
- **Date**: 2026-05-25
- **Architect**: Morgan
- **Authoritative source for**: the `org.openlore.claim` and
  `org.openlore.philosophy` Lexicon shapes (slice-01); the DuckDB schema;
  the canonical on-disk artifact format.

## Two representations, one logical claim

A claim exists in three places, each with a different representation
(per ADR-006):

| Where | Representation | Purpose |
|---|---|---|
| In-memory (Rust) | `claim_domain::SignedClaim` value | Used by all pure transformations and verb handlers. |
| On-disk artifact | JSON file at `~/.local/share/openlore/claims/<cid>.json` | Greppable canonical artifact (per US-002). |
| In-DB index | rows across `claims`, `claim_references`, `claim_evidence` tables in DuckDB | Indexed lookup by subject / author / referencing CID. |
| CID input | RFC 8949 canonical CBOR encoding of the signed claim | Used ONLY for `compute_cid()`; never stored. |

**Invariant**: the JSON file and the DB rows MUST both deserialize to a
`SignedClaim` value that canonicalizes (via CBOR) to bytes whose `sha2-256`
multihash matches the filename `<cid>` and the DB primary key.

## Lexicon: `org.openlore.claim`

Lexicon JSON (illustrative; DELIVER may adjust field-order display per
ATProto codegen tooling — wire CID is unaffected since CID is computed from
canonical CBOR per ADR-006):

```json
{
  "lexicon": 1,
  "id": "org.openlore.claim",
  "defs": {
    "main": {
      "type": "record",
      "key": "any",
      "description": "A signed philosophical claim about a subject (project, person, artifact).",
      "record": {
        "type": "object",
        "required": ["subject", "predicate", "object", "confidence", "author", "composedAt"],
        "properties": {
          "subject":     { "type": "string", "description": "URI of the thing being claimed about. Common schemes: github:owner/repo, at://did:.../..." },
          "predicate":   { "type": "string", "description": "The relation, e.g. embodiesPhilosophy, refutesPhilosophy, alignsWith." },
          "object":      { "type": "string", "description": "URI of the philosophy or counterparty, e.g. org.openlore.philosophy.memory-safety." },
          "evidence":    { "type": "array", "items": { "type": "string", "format": "uri" }, "description": "Zero or more evidence URIs supporting the claim." },
          "confidence":  { "type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Numeric only; display buckets are NEVER persisted (WD-10 / OD-2)." },
          "author":      { "type": "string", "description": "Author DID + key fragment, e.g. did:plc:jeff#org.openlore.application." },
          "composedAt":  { "type": "string", "format": "datetime", "description": "RFC3339 UTC timestamp captured at compose-time (NOT sign-time)." },
          "references":  {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["type", "cid"],
              "properties": {
                "type": { "type": "string", "enum": ["retracts", "corrects", "counters", "supersedes"] },
                "cid":  { "type": "string", "description": "Target claim's CID; MUST NOT be self-reference." }
              }
            },
            "description": "Zero or more typed references to other claims. See ADR-008."
          },
          "signature":   {
            "type": "object",
            "required": ["kid", "alg", "sig"],
            "properties": {
              "kid": { "type": "string", "description": "DID key fragment, e.g. did:plc:jeff#org.openlore.application." },
              "alg": { "type": "string", "enum": ["EdDSA"], "description": "Signature algorithm; Ed25519 only for slice-01." },
              "sig": { "type": "string", "description": "Base64url-encoded signature over the unsigned-claim CID (per ADR-006)." }
            }
          }
        }
      }
    }
  }
}
```

Notes:

- `confidence` is `number` with hard `min/max` constraints; the Lexicon
  rejects out-of-range at the wire boundary (additional safety on top of CLI
  pre-sign validation per US-001 Example 3).
- `references` is the unified mechanism for retraction / correction /
  counter-claim / supersession (ADR-008).
- The `signature` block is part of the record as published; the unsigned CID
  is computed without the signature block, then signed, then the full record
  (with `signature` populated) is what gets canonicalized for the FINAL CID
  used as the on-disk filename and PDS rkey (per ADR-006 step 6).

## Lexicon: `org.openlore.philosophy`

```json
{
  "lexicon": 1,
  "id": "org.openlore.philosophy",
  "defs": {
    "main": {
      "type": "record",
      "key": "any",
      "description": "A philosophy concept; used as the `object` of a claim. Slice-01 ships ~10 well-known seeds; slice-04 expands.",
      "record": {
        "type": "object",
        "required": ["name", "description"],
        "properties": {
          "name":         { "type": "string", "description": "Short identifier, e.g. memory-safety." },
          "description":  { "type": "string", "description": "One-paragraph definition." },
          "aliases":      { "type": "array", "items": { "type": "string" }, "description": "Other names for the same concept." },
          "seeAlso":      { "type": "array", "items": { "type": "string", "format": "uri" } }
        }
      }
    }
  }
}
```

Seed philosophies (illustrative; DELIVER picks the final ~10):
`memory-safety | functional-purity | local-first | federation-first |
unix-philosophy | composability-over-monolith | explicit-over-implicit |
data-orientation | content-addressing | reproducible-builds`.

## On-disk artifact format

Path: `~/.local/share/openlore/claims/<cid>.json` (per US-002 AC).

Content: the rendered JSON of the `SignedClaim` value, with fields ordered
per the Lexicon field order (display order, not canonicalization order).
Approximately 400-600 bytes per claim.

Example:

```json
{
  "subject": "github:rust-lang/rust",
  "predicate": "embodiesPhilosophy",
  "object": "org.openlore.philosophy.memory-safety",
  "evidence": ["https://www.rust-lang.org/"],
  "confidence": 0.86,
  "author": "did:plc:jeff#org.openlore.application",
  "composedAt": "2026-05-25T12:00:00Z",
  "references": [],
  "signature": {
    "kid": "did:plc:jeff#org.openlore.application",
    "alg": "EdDSA",
    "sig": "MEUCIQDz...base64url..."
  }
}
```

Write strategy: write to `<cid>.json.tmp`, `fsync`, rename to `<cid>.json`
(atomic per POSIX; on Windows, equivalent via `MoveFileEx` with
`MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`). The `adapter-duckdb`
crate handles this atomicity since the JSON file and the DB rows are written
in the same transaction-equivalent.

## DuckDB schema

Path: `~/.local/share/openlore/openlore.duckdb`.

```sql
-- Single source of versioning.
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TIMESTAMP NOT NULL,
    description VARCHAR  NOT NULL
);

-- Core claims index. The on-disk JSON files are the authoritative artifact;
-- this table is a derived index for query speed.
CREATE TABLE IF NOT EXISTS claims (
    cid           VARCHAR PRIMARY KEY,            -- bafy... (base32-lower CIDv1)
    subject       VARCHAR NOT NULL,
    predicate     VARCHAR NOT NULL,
    object        VARCHAR NOT NULL,
    confidence    DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    author_did    VARCHAR NOT NULL,               -- did:plc:...#kid
    composed_at   TIMESTAMP NOT NULL,             -- UTC
    -- Local-only metadata; NOT part of the signed payload.
    published_at  TIMESTAMP,                      -- NULL until published
    at_uri        VARCHAR,                        -- NULL until published; derived from author_did + cid
    -- Provenance back to the artifact file.
    artifact_path VARCHAR NOT NULL,               -- absolute path to <cid>.json
    inserted_at   TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_claims_subject     ON claims (subject);
CREATE INDEX IF NOT EXISTS idx_claims_author      ON claims (author_did);
CREATE INDEX IF NOT EXISTS idx_claims_composed_at ON claims (composed_at);

-- Evidence URIs (denormalized for query; many-to-one with claims).
CREATE TABLE IF NOT EXISTS claim_evidence (
    cid         VARCHAR NOT NULL,
    evidence    VARCHAR NOT NULL,
    ordinal     INTEGER NOT NULL,                 -- preserves array order
    PRIMARY KEY (cid, ordinal),
    FOREIGN KEY (cid) REFERENCES claims (cid)
);

-- Reference graph (denormalized for the query_referencing port method).
CREATE TABLE IF NOT EXISTS claim_references (
    referencing_cid VARCHAR NOT NULL,
    referenced_cid  VARCHAR NOT NULL,
    ref_type        VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
    PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
    FOREIGN KEY (referencing_cid) REFERENCES claims (cid)
);

CREATE INDEX IF NOT EXISTS idx_claim_references_referenced ON claim_references (referenced_cid);
```

Migration policy: forward-only; idempotent on each `openlore init`. The
`schema_version` table records every applied migration; the adapter probes
the latest version on startup and refuses to start if the file is from a
NEWER schema than the binary knows about (`storage.schema_mismatch`).

## Shared artifact ↔ data model mapping

Per `shared-artifacts-registry.md`, every shared artifact resolves to a
specific data location:

| Shared artifact | Source of truth |
|---|---|
| `author_did` | `claims.author_did` column AND signed-claim `author` field; resolved at `openlore init` from `~/.config/openlore/identity.toml`. |
| `claim_cid` | `claims.cid` PK; the on-disk filename suffix; computed by `claim_domain::compute_cid()` (ADR-006). |
| `at_uri` | `claims.at_uri` column; derived as `at://{author_did_root_did}/org.openlore.claim/{cid}` once published. |
| `local_claim_store` | `~/.local/share/openlore/claims/` directory containing `<cid>.json` files (XDG). |
| `composed_at` | `claims.composed_at` column AND signed-claim `composedAt` field. |
| `pds_endpoint` | NOT persisted in the DB. Resolved fresh from `identity.toml` at session start. |
| `retraction_reference` | `claim_references` rows (denormalized) AND `references[]` field in the on-disk JSON. |

## Validation rules — translated from shared-artifacts-registry.md to data assertions

| Registry rule | Data-model assertion |
|---|---|
| `author_did` matches across all 4 steps | `claims.author_did` is set exactly once per row at write; CLI rendering re-reads from the row (no per-step re-resolution). |
| `claim_cid` matches across steps 2-4 | Filename `<cid>.json` == DB primary key == PDS rkey. Enforced by writing all three in one transaction; probed by `adapter-duckdb`. |
| `at_uri` is reconstructible from `author_did + claim_cid` | `at_uri` computed by a pure function; the stored value is a cache. On read, the function MUST match the stored value or the row is corrupt. |
| Graph query output exactly matches compose-time field values | Read path goes through the same `lexicon::claim::Claim` serde model used by write; KPI-4 field-mismatch counter increments on any deviation. |
| Every retraction-reference resolves OR is annotated as unresolved | `query_referencing` joins `claim_references` to `claims`; missing target = `unresolved` annotation, not a query failure. |
| No cycles in the reference graph | `reference_rules_validate` (claim-domain) checks at sign time; the `claim_references` table additionally has no enforced cycle constraint at the SQL level (DuckDB doesn't support that), so the application-level check is the only line of defense. Cycle introduction by direct DB tampering is out-of-threat-model for slice-01. |

## Confidence buckets are NOT persisted (WD-10 / OD-2)

Buckets exist only in the CLI render layer. Neither the on-disk JSON, the
DB rows, nor the PDS record carries a bucket label. The mapping function
lives in `claim-domain::confidence_bucket(numeric: f64) -> ConfidenceBucket`
and is invoked ONLY in render code paths. Persisting a bucket label anywhere
is a CI-failable invariant (a unit test asserts the on-disk JSON and the
DB rows contain no string matching `speculative|weighted|well-evidenced|triangulated`
in the confidence-related columns).
