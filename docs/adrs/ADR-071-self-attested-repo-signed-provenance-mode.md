# ADR-071: Self-Attested (Repo-Signed) Provenance as a Second Verification Mode

- **Status**: Proposed (2026-10-04)
- **Date**: 2026-10-04
- **Deciders**: Jeff Bailey (D-5, 2026-10-03), Morgan (nw-solution-architect)
- **Feature**: bluesky-claim-review-app (DESIGN), resolves OD-BRA-1
- **Amends**: ADR-005 (lexicon field descriptions only, no schema change), ADR-006 (verification
  procedure gains a second branch), ADR-023/ADR-024 (I-AV-1 verify-before-index gains a second
  admissible proof)

## Context

D-5 says claims approved in the review app carry **no** app-level `#org.openlore.application`
signature. Their provenance is the PDS repo commit, which the account's own `#atproto` key signs.
Today every reader rejects such a record:

- `claim_domain::verify` needs a `SignatureBlock`.
- `adapter-atproto-pds::peer_read::parse_signed_claim` fails with "signature block missing".
- `appview_domain::ingest_decision` returns `RejectReason::Unsigned`.
- The viewer and search only know `[verified]`.

I-BRA-5 requires that these claims be accepted and labelled "self-attested", never "unverified".
AC-009.4 requires that app-signed behaviour stay byte-identical.

Two facts shape the decision:

1. **A record in a DID's repo is attested by that DID's repo.** On a standard PDS the PDS holds
   the repo signing key. "The commit signature verifies" and "the DID's own PDS, resolved from the
   DID document, served this record over TLS" therefore carry the same trust. The commit signature
   only adds protection when the record arrives through a *non-authoritative* path, such as a
   relay, a mirror, or a cache.
2. **Most OpenLore readers fetch directly from the author's DID-document PDS.** That covers
   `peer pull` (ADR-016 re-resolves on every pull) and the review app's profile page.
   The indexer is different: its `listRecords` source is an operator-configured URL, so the origin
   must be *checked*, not assumed (Decision 4). The relay and firehose path is deferred (ADR-024).

## Decision

1. **No new lexicon field.** The provenance mode is determined by the record's shape plus where it
   was fetched from:

   | `signature` | `author` | Mode |
   |---|---|---|
   | present | `did#fragment` | **AppSigned**: today's path, unchanged |
   | absent | bare DID (no `#`) **and** equal to the repo DID in the record's `at://` URI | **SelfAttested** |
   | absent | has a `#fragment` | Reject: `MalformedProvenance` (it claims a key but carries no signature) |
   | absent | bare DID ≠ repo DID | Reject: `ForeignRepo` (nobody can self-attest into another DID's repo) |

   Only the `author` and `signature` descriptions in `lexicons/org/openlore/claim.json` change, to
   document that `author` may be a bare DID when `signature` is absent. `required[]` is unchanged,
   so the change is backward compatible (ADR-005 forward-compat).

2. **The CID path does not change.** A self-attested claim is an `UnsignedClaim` whose
   `author_did` is the bare DID. Its CID comes from the same `canonicalize` + `compute_cid`, and
   that CID is the record's rkey, exactly as for CLI publishes. The integrity check is the same
   one used today: **recomputed CID == rkey**. As a side effect, publishing is idempotent: a
   retried `createRecord` gets `RecordAlreadyExists`, which the existing adapter treats as success.

3. **Pure-core shape (claim-domain).**
   - A `ClaimRecord` ADT has two arms: `AppSigned(SignedClaim)` and `SelfAttested(UnsignedClaim)`.
   - A pure provenance verdict takes the record, its rkey, a `RecordOrigin`, and an optional
     resolved app key. It returns `Provenance::AppSigned { key_id }` or
     `Provenance::SelfAttested { repo_did }`, or a structured rejection.
   - The existing `verify` function is **unchanged**. The AppSigned arm calls it verbatim, which is
     the regression guardrail.

4. **Origin rule for v1.** SelfAttested is admitted **only** when `RecordOrigin` is `AuthorPds`,
   meaning the record was fetched from the PDS endpoint freshly resolved from the author's DID
   document. A record from any other origin (`Relay`) is rejected with `UnverifiableProvenance`
   until commit-proof verification exists (see the revisit trigger).

   **The origin is computed, never assumed.** The effect shell compares the base URL it actually
   fetched from with the PDS endpoint it resolved for the repo DID, and only an exact match yields
   `AuthorPds`:

   - `peer pull` re-resolves on every pull (ADR-016).
   - The review app reads from the session's or the resolved PDS.
   - The indexer's ingest source is an **operator-configured URL** that may be a relay or another
     PDS. The indexer root therefore resolves each record's repo DID through its existing
     `IdentityResolvePort` before classifying.

   The CID check uses the **rkey from the `at://` URI**, not the listRecords view's `cid` field.
   That field is the ATProto record CID, which is a different hash.

5. **Every reader gets a provenance column or marker** (additive):
   - `peer_claims` and `indexed_claims` gain `provenance` (`app-signed` | `self-attested`, default
     `app-signed`). Signature columns become nullable for self-attested rows only.
   - `verified_against` stays `NOT NULL`. For self-attested rows it holds the bare repo DID.
   - The search DTO gains an optional `provenance` field. It is absent on old servers, and readers
     treat absent as app-signed.
   - The viewer and CLI render `[self-attested]` next to the unchanged `[verified]`.

6. **Self-retraction (RC-02 / ADR-008) applies unchanged.** A self-attested retraction is another
   self-attested claim whose `references` holds `retracts` → original CID. "Same author" compares
   bare DIDs, which is the existing `bare_did` helper.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Explicit optional field** (e.g. `attestation: "repo"`) | Rejected. It is just as forgeable as an absent signature, because it is only a claim inside the record. It adds a field to canonical CBOR, which is CID-relevant, and to every encoder. It also creates a new contradiction to police (marker present *and* signature present). The record's shape already carries the information. |
| **Infer from absent `signature` alone** | Rejected as incomplete. Without the bare-author and author == repo-DID rules, a record copied into someone else's repo, or an app-signed record with a stripped signature, would quietly downgrade to "self-attested by" the wrong party. |
| **Verify the repo commit signature + MST proof now** (`com.atproto.sync.getRecord` CAR) | Deferred. With direct-from-author-PDS fetches it adds no trust (fact 1 above). It costs a CAR reader, an MST proof walker and secp256k1/P-256 verification: about 100–200 lines of glue over `atrium-repo` + `atrium-crypto` (MIT). No permissively licensed crate verifies proofs out of the box: `jacquard-repo` is MPL-2.0, and `rsky-repo` is immature. |
| **The app signs with its own key** | Rejected by D-5. It would also make the hosted app hold a signing key for other people's claims. |

## Consequences

- **Positive**:
  - No lexicon schema change and no new CID path. App-signed CIDs are byte-identical.
  - One pure decision serves the CLI, the indexer, the viewer and the review app.
  - Publishing is idempotent through the content-addressed rkey.
- **Negative**:
  - The `ports` ADTs that carry `SignedClaim` (`SignedRecord`, `RawRecord`) widen to `ClaimRecord`.
  - Two DuckDB schemas get an additive migration.
  - Self-attested trust is origin-based (TLS to the DID-document PDS), so it is only as strong as
    DID resolution plus WebPKI.
- **Revisit trigger (mandatory)**: if the indexer ingests from a relay or the firehose (the
  ADR-024 revisit), or any reader accepts a record from a non-author origin, commit-proof
  verification must ship first (SPIKE-5). The `RecordOrigin::Relay → UnverifiableProvenance` arm
  keeps this fail-closed until then.
