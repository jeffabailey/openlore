# ADR-005: ATProto Lexicon Namespace = `org.openlore.*`

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect) — confirms user's prior proposal
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

ATProto records live under a Lexicon namespace (reverse-DNS scheme). The
namespace is the binding identity for a record collection; once peers ingest
records under that name, renaming is a federation-breaking change.

The user has proposed `org.openlore.claim` (mentioned in
`docs/feature/openlore-foundation/slices/slice-01-claim-skeleton.md` and in
the journey YAML). This ADR commits to it and reserves the parent namespace
for sibling-feature collections born in later slices.

## Decision

**The OpenLore Lexicon namespace is `org.openlore.*`.**

slice-01 ships two record types:

| Lexicon NSID | Slice | Purpose |
|---|---|---|
| `org.openlore.claim` | slice-01 | A signed claim record (subject, predicate, object, evidence, confidence, retraction-reference). |
| `org.openlore.philosophy` | slice-01 (starter seed) | A philosophy concept (used as the `object` of a claim). Initial seed ≤10 well-known philosophies; expanded in slice-04. |

Reserved for sibling features (informational; out of slice-01 scope):

| NSID | Slice | Notes |
|---|---|---|
| `org.openlore.scraper.candidate` | slice-02 | Draft-claim emitted by a scraper, pending human promotion. |
| `org.openlore.subscription` | slice-03 | A subscription to another DID's claim stream. |
| `org.openlore.weight` | slice-04 | A user's trust weight on (author, predicate). |

Lexicon JSON files live under `lexicons/org/openlore/*.json` in the repository
root (the standard ATProto convention). The build pipeline serializes them
into Rust types via `atrium-codegen` (or hand-written serde models if codegen
is not yet stable for Lexicon — DELIVER's call).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`com.example.openlore.*`** | `org.openlore` is more honest about the project being an organization-owned namespace, not a vendor. |
| **`app.openlore.*`** | Bluesky uses `app.bsky.*` for first-party Bluesky records; mimicking that convention would suggest OpenLore is a first-party Bluesky app. We are not. |
| **`org.openlore.v1.claim` (versioned NSID)** | ATProto recommends evolving via field-level optionality rather than NSID versioning. NSID change is a federation break; we prefer the conventional approach. |

## Consequences

### Positive

- Clear ownership and reservation for sibling features.
- Conventional ATProto naming; peers can grep their PDS for the namespace.
- Forward-compatible: new record types add new NSIDs under the same parent
  without conflict.

### Negative

- Lexicon evolution (adding/changing fields on `org.openlore.claim`) requires
  backward-compatible schema changes; field removal would break federated
  reads from slice-03 onward. **Mitigation**: every field added MUST be
  optional; field removal requires a deprecation cycle spanning ≥2 sibling
  slices.
- Squatting risk: a third party could publish records under `org.openlore.*`
  without our blessing. ATProto does not enforce namespace ownership.
  **Mitigation**: the federation slice (slice-03) MUST validate that records
  with `org.openlore.claim` carry signatures verifiable against the claimed
  author DID — anyone can publish under any NSID; signature validity is the
  trust gate.

### Earned Trust

The `lexicon` module MUST expose a `probe()` that:

1. Loads every `org.openlore.*.json` file at startup and validates each against
   the ATProto Lexicon schema-of-schemas.
2. Serializes a sentinel record under each NSID, deserializes it back, and
   asserts byte-equality (catches drift between hand-written serde models and
   the Lexicon JSON).
3. Refuses to start with `health.startup.refused{reason: lexicon.invalid | lexicon.serde_round_trip_failed}` on any failure.

## Revisit Trigger

- A naming conflict with another project claiming `org.openlore.*`.
- A formal ATProto namespace registry that requires registration.
