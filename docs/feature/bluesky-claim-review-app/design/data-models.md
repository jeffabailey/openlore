# Data Models: bluesky-claim-review-app

> Logical schemas. Column types are indicative (DuckDB). Exact DDL, indexes and migration
> mechanics are the crafter's call within these constraints. Every owner table is keyed by
> `owner_did`, and every query on it filters by `owner_did` (ADR-074 rule).

## 1. Claim record on the wire (`org.openlore.claim`, written to the USER's PDS)

There is no schema change (ADR-071). A self-attested record is an ordinary claim **without**
`signature`, whose `author` is the **bare** DID.

```json
{
  "$type": "org.openlore.claim",
  "subject": "github:priyaraman/tidepool",
  "predicate": "embodiesPhilosophy",
  "object": "org.openlore.philosophy.dependency-pinning",
  "evidence": ["https://github.com/priyaraman/tidepool/blob/main/Cargo.lock"],
  "confidence": 2500,
  "author": "did:plc:7x3kq2mzv5rj4w6hbn2tqclp",
  "composedAt": "2026-10-04T15:02:11Z"
}
```

- **URI**: `at://did:plc:7x3kq2mzv5rj4w6hbn2tqclp/org.openlore.claim/<cid>`. The rkey is the CIDv1
  (base32, `b…`, about 59 chars, a valid `any` record key) of the canonical unsigned claim.
- **Retraction**: the same shape with `references: [{"type":"retracts","cid":"<original cid>"}]`
  (and a `reason` if the user gives one).
- **Provenance classification** (pure, ADR-071):

  | `signature` | `author` | origin | Result |
  |---|---|---|---|
  | present | `did…#frag` | any | AppSigned (existing verify) |
  | absent | bare DID == repo DID | AuthorPds | **SelfAttested** |
  | absent | `did…#frag` | any | reject `MalformedProvenance` |
  | absent | bare DID ≠ repo DID | any | reject `ForeignRepo` |
  | absent | bare == repo | Relay | reject `UnverifiableProvenance` |

  In every admitted case, the recomputed CID must equal the rkey, or the record is rejected as an
  integrity failure.

## 2. Share post (`app.bsky.feed.post`, written to the user's PDS)

The standard Bluesky record: `text` (≤300 graphemes), `createdAt`, and `facets` with one
`app.bsky.richtext.facet#link` to `${app_origin}/@<handle>`. The PDS assigns the rkey (a TID).
The app stores nothing about the post except an aggregate counter.

## 3. Private store: `review-app.duckdb` (ADR-074)

### 3.1 Owner data

| Table | Columns | Notes |
|---|---|---|
| `accounts` | `owner_did` PK, `handle_last_seen`, `pds_endpoint_last_seen`, `created_at`, `last_seen_at` | One row per DID that has signed in |
| `github_links` | `owner_did` PK, `github_user_id` BIGINT, `github_login`, `status` ENUM(`verified`,`unverified`), `verified_at`, `last_checked_at`, `last_verdict` (verdict code, not the bio) | **The bio is never stored.** No uniqueness on `github_user_id`: two DIDs may legitimately both appear in one bio. |
| `suggestions` | PK (`owner_did`, `subject`, `predicate`, `object`); `confidence_bp` INT; `evidence` JSON (URLs); `signals` JSON (signal names and "why" text); `source_repo`; `state` ENUM(`pending`,`declined`,`published`,`retracted`); `first_suggested_at`; `state_changed_at`; `published_uri`, `published_cid`, `published_object`, `published_confidence_bp`, `edited` BOOL; `retraction_cid` | `published_*` records what was *actually* written when the user edited (BR-2 suppression uses the PK, which is the *suggested* key). Visibility is **derived**: `state='pending' AND github_links.status='verified'`. There is no `hidden` column (D-12: hidden, not deleted). |
| `scan_runs` | PK (`owner_did`, `run_id`); `status` ENUM(`running`,`completed`,`rate_limited`,`interrupted`,`ownership_failed`); `repos_total`, `repos_done`; `new_count`, `published_count`, `declined_count`; `resume_after`; `started_at`, `finished_at` | Progress for htmx polling. At startup, any `running` row becomes `interrupted`. |
| `plans` | PK (`owner_did`, `plan_id`); `kind` ENUM(`publish`,`retract`,`share`); `suggestion_key` (nullable); `plan` JSON (exact record + destination); `created_at`, `expires_at` (+30 min) | Taken **exactly once** on confirm; expired rows are swept. `plan_id` is the record CID for publish and retract, and random for share. |

### 3.2 Auth and session data (secrets are AEAD-encrypted)

| Table | Columns | Notes |
|---|---|---|
| `oauth_auth_requests` | `state` PK; `owner_did_expected`; `issuer`; `enc_blob` (PKCE verifier, DPoP key, nonce); `created_at`, `expires_at` (+10 min) | Transient. atrium `StateStore` through `SecretStorePort`. |
| `oauth_sessions` | `owner_did` PK; `issuer`; `granted_scopes`; `enc_blob` (access token, refresh token, DPoP private JWK, expiry); `updated_at` | atrium `SessionStore`. Deleted on sign-out and on purge. |
| `web_sessions` | `session_hash` PK (SHA-256 of a 256-bit random cookie value); `owner_did`; `csrf_token_hash`; `created_at`, `last_used_at`, `expires_at` (30 days idle) | The cookie is `__Host-ol_session`; HttpOnly; Secure; SameSite=Lax; Path=/. |

Encryption:

- XChaCha20-Poly1305 with a random 24-byte nonce per write.
- AAD = `owner_did ‖ table.column ‖ schema_version` (or `state` for auth requests).
- Key: 32 bytes from SSM (`/openlore/prod/review-app/data-key`). Rotation means re-encrypting on
  read with a key id prefix (DEVOPS runbook).

### 3.3 Aggregate data (no owner)

| Table | Columns | Notes |
|---|---|---|
| `kpi_counters` | PK (`day` DATE, `event` VARCHAR); `count` BIGINT | Events: `signin.started`, `signin.completed`, `signin.denied`, `github.verify.ok`, `github.verify.fail.<code>`, `scan.completed`, `scan.nonempty`, `suggestion.approved`, `suggestion.approved.edited`, `suggestion.declined`, `publish.failed`, `share.previewed`, `share.posted`, `retract.posted`, `disconnect`, `time_to_publish.bucket.<le_1m|le_5m|le_15m|gt_15m>`. **No DID, handle or content.** Enforced by check-arch rule 6. |

### 3.4 Purge (US-BRA-012)

A single transaction runs `DELETE … WHERE owner_did = ?` on `accounts`, `github_links`,
`suggestions`, `scan_runs`, `plans`, `oauth_sessions` and `web_sessions`.
`oauth_auth_requests` rows for the DID expire on their own. `kpi_counters` is untouched, because it
holds no owner data. A post-purge audit query (count of rows per table for the DID equals 0) is
the AC-012.2 test oracle.

## 4. Reader-side schema deltas (ADR-071)

| Store | Table | Delta |
|---|---|---|
| `openlore.duckdb` (`adapter-duckdb`) | `peer_claims` | + `provenance` VARCHAR NOT NULL DEFAULT `'app-signed'` CHECK IN (`app-signed`,`self-attested`); signature columns nullable **only** when `provenance='self-attested'` (CHECK) |
| `index.duckdb` (`adapter-index-store`) | `indexed_claims` | + `provenance` (same domain and default); `verified_against` stays NOT NULL (the bare repo DID for self-attested) |
| Search XRPC DTO | result row | + optional `provenance`. If absent, readers use `app-signed`, so older servers stay compatible. |

## 5. Pure in-memory ADTs (`review-domain`, illustrative names)

- `OwnershipVerdict = Verified | DidMissing | DifferentDid(Did) | NoBio | AccountNotFound | RateLimited(Duration) | IdentityChanged`
- `VerifiedOwnership { did, github_user_id, github_login }`. It is constructible only from `Verified`.
- `SuggestionKey { subject, predicate, object }`. `Suggestion { key, confidence_bp, evidence, signals, state }`.
- `PublishPlan { destination: {did, pds}, collection, rkey: Cid, record_json, preview }`. The same
  shape is used for `RetractPlan`.
- `SharePostPlan { text, facets, link }`.
- `Budget` state plus a decision `Allow | Deny{retry_after}`.
