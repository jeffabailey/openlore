# ADR-008: Retraction = Counter-Claim Referencing Original CID; No Hard-Delete

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect), per WD-11 lock from Luna (nw-product-owner)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

Locked by WD-11 (OD-3): retraction is a counter-claim that references the
original CID. The PDS-published record persists; hard-delete is forbidden
even with `--force`. This ADR codifies the architectural implications of
that lock for slice-01.

Affected user-facing surfaces (per `gherkin-scenarios-expanded.md`):

- Anxiety scenario 1 (counter-claim from peer + retract-by-author flow)
- Anxiety scenario 2 (corrective-claim for a typo)
- US-003 publish success message (must mention `openlore claim retract`)

Affected data: see `shared-artifacts-registry.md` — `retraction_reference` is
a HIGH-risk shared artifact.

## Decision

**Retractions, counter-claims, and corrective claims all use the same
architectural mechanism: a NEW signed claim with a Lexicon field referencing
the original `claim_cid`.**

### Lexicon-level design

The `org.openlore.claim` Lexicon (full schema in `data-models.md`) carries an
optional `references` field whose value is an array of typed CID references:

```
references: [
  { type: "retracts",    cid: "bafy...n4ka" },  // self-retract
  { type: "corrects",    cid: "bafy...n4ka" },  // typo correction
  { type: "counters",    cid: "bafy...n4ka" },  // peer-published counter-claim
  { type: "supersedes",  cid: "bafy...n4ka" }   // newer version replaces older
]
```

The `type` enum is closed for slice-01: `retracts | corrects | counters | supersedes`.

### Behavioral rules

1. **No hard-delete CLI verb exists.** `claim retract <cid>` does NOT remove
   the original from the local store or the PDS; it ONLY composes and
   publishes a new claim of type `retracts`.
2. **The original at-uri remains resolvable.** Graph query MUST list both
   the original and the retraction with their respective CIDs and timestamps.
3. **Annotation, not mutation.** Graph query output annotates the original as
   "retracted by author" when a retraction references it; the original
   record's bytes are NOT modified.
4. **No cycles.** A retraction MUST NOT reference itself; the canonicalization
   step rejects self-reference at sign time.
5. **Dangling references are visible, not fatal.** If the referenced CID is
   not in the local store (slice-01) or any subscribed PDS (slice-03), the
   query annotates the reference as `unresolved` but does not fail the query.

### Adapter implications

- `storage-port`: gains a `query_referencing(cid)` method to find all claims
  that reference a given CID. The DuckDB adapter implements this via an
  index on a `claim_references(referencing_cid, referenced_cid, ref_type)`
  table (denormalized from the JSON `references` field for query speed).
- `pds-port`: no new methods needed; counter-claims publish through the same
  `create_record` path as any other claim.
- `cli`: the `claim retract <cid>` verb composes a new claim with
  `references: [{ type: "retracts", cid: <cid> }]` and otherwise empty
  subject/predicate/object (the retraction's payload IS the reference; the
  subject/predicate of the original are not duplicated to avoid confusion
  about which claim is which).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Hard-delete from PDS** (the standard tombstoning approach) | Violates WD-11 lock; defeats the audit-trail product hypothesis ("retraction is itself a public claim"). |
| **In-place mutation of the original record** (mark as retracted) | Breaks the signature; the signed payload of a record never changes once published. ATProto records are content-addressed; in-place mutation IS a hard-delete-and-republish under a new CID. |
| **A separate `org.openlore.retraction` Lexicon NSID** | Considered for cleanliness. Rejected because retraction, correction, counter, and supersession are all "claim about a claim" with the same shape; one Lexicon with a typed reference is more honest about the unified mechanism. |
| **Counter-claim type as a separate field** (e.g., `retracts: "bafy..."` at top level) | Rejected because the same architectural mechanism handles 4 reference types; a typed array generalizes; a flat field per type does not. |

## Consequences

### Positive

- One mechanism, four use cases. The Lexicon is simpler than four
  retraction-shaped NSIDs.
- Public history is preserved by construction; the audit trail IS the data
  structure.
- Federated peers in slice-03 will see retractions as ordinary claims with a
  reference field, no special-case handling needed.
- The `references` field generalizes to slice-04 (weighted claims could
  reference the evidence claims they aggregate) without re-architecting.

### Negative

- Users may expect hard-delete and be initially frustrated. **Mitigation**:
  the publish success message (US-003) MUST explain retraction is one
  command away; the retract-status output (anxiety scenario 1.2) MUST
  explicitly state that NO hard-delete option exists.
- Graph query must walk the reference graph to compute annotations; with
  10k+ claims this becomes a measurable cost. **Mitigation (slice-01)**: the
  denormalized `claim_references` table makes annotation lookup O(log n).

### Earned Trust

The `claim-domain::references` module MUST:

1. Property-test that walking the reference graph from any claim ALWAYS
   terminates (per `shared-artifacts-registry.md` validation rule 5).
2. Reject self-reference at sign time with a typed error
   (`ClaimError::SelfReference`).
3. Detect cycles ≥2 hops (claim A references claim B which references claim A)
   at sign time and reject with `ClaimError::CycleDetected`.
4. The `storage-port::query_referencing` adapter MUST be probed at startup
   with a sentinel reference pair (write claim A, write claim B referencing
   A, query for A's referrers, assert B is in the result).

## Revisit Trigger

- Regulatory requirement that forces hard-delete (e.g., GDPR right-to-erasure).
  Slice-01 ships no PII beyond a DID (which is itself the user's public
  identifier), so this is not a slice-01 concern; flagged for re-evaluation
  at slice-03 (federated read of others' claims).
