# ADR-070: Confidence Travels as Integer Basis Points

- **Status**: Accepted (2026-10-03)
- **Date**: 2026-10-03
- **Deciders**: Jeff Bailey ("don't worry about existing data, this is all green field")
- **Amends**: ADR-005 (`org.openlore.claim` lexicon: `confidence` type), ADR-062 (resolves its
  float-CID revisit trigger for the PDS transport)

## Context

The first `claim publish` to a stock bluesky PDS (`openlore.jeffbailey.us`, 2026-10-03) was
refused:

> `Expected one of null, boolean, integer, string, cid, bytes, array or object value type
> (got 0.85) at $.record.confidence`

ATProto's data model has no float type, so no `org.openlore.claim` record could be written to
any real PDS. ADR-062 had already noted the related CID hazard (a JS PDS re-encodes floats).

## Decision

- **Wire:** `confidence` is an integer count of basis points, `0..=10000` (`0.85` → `8500`), in
  the lexicon schema and in every JSON encoder (`lexicon::Claim`'s serde). Readers also accept a
  legacy float, so older records and fixtures still parse.
- **Domain:** `claim_domain::Confidence` stays a `[0.0, 1.0]` `f64`, but every value is rounded to
  the basis-point grid when it is deserialized (the one entry point). So
  `from_basis_points(c.basis_points()) == c` bit for bit (property-tested), and a claim's CID,
  computed over the canonical CBOR of that `f64`, survives publish → pull.
- **Canonical CBOR is unchanged.** Values already on the grid (every value a user typed with ≤ 4
  decimals, and the scraper's 0.25) keep the same `f64` and so the same CID.

## Consequences

- Records publish to a stock PDS; verified live: the record read back from the PDS decodes to
  the CID it is stored under.
- A confidence with more than 4 decimals is rounded on entry (its CID changes). Accepted:
  greenfield, no data to migrate.
- An instance that re-encodes the float lossily (f32) no longer breaks the CID, since the grid
  absorbs it; the `CidMismatch` test fake now models an off-by-one basis point instead.

## Alternatives considered

- **String confidence (`"0.85"`)**: rejected; needs a canonical decimal-string rule and parsing
  everywhere, for no gain over an integer.
- **Change the canonical CBOR to the integer too**: rejected; every existing CID would change and
  the gold fixtures with it, while the grid already makes the `f64` exactly representable.
- **Percent (`0..=100`)**: rejected; loses the second decimal people use (0.825).
