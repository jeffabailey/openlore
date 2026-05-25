# Slice 03 — Federated Read (sibling feature seed)

**Status**: deferred — births sibling feature `openlore-federated-read` after slice-01 lands.
**Slice priority**: P1 (immediately after walking skeleton)
**Effort estimate**: ~1 week
**Primary persona**: P-001 + P-002
**Primary job**: J-003 Read another developer's federated claims with weighting

## Hypothesis

> A user can subscribe to another developer's DID, pull their `org.openlore.claim`
> records, and read them through `openlore graph query --federated` with full
> attribution preserved per claim — no silent merging, no "consensus view."

## Disproves if it fails

- ATProto's read-side does not give us enough metadata to attribute every claim cleanly.
- Users find attribution-preserving reads cognitively confusing and would prefer merging
  (which would force us to reconsider the entire trust model).
- The local DuckDB schema cannot represent multi-author claim sets at usable query speed.

## In scope (when this slice runs)

- `openlore peer add <did>` subscribes to a DID's claim stream.
- `openlore peer pull` (or auto-pull) fetches claims into a separate `peer_claims` table.
- `openlore graph query --federated` includes peer claims with explicit author attribution.
- Counter-claim authoring: `openlore claim counter <cid> --reason ...`.

## Out of scope

- Trust weighting beyond binary subscribe/unsubscribe (that's slice-04).
- Automatic detection of "spam" claims.
- Real-time push subscriptions (start with pull on demand).

## Why this is P1 (before scrapers)

The federation contract — what fields must be wire-stable, what counts as "the same"
claim, how counter-claims reference originals — constrains every later design choice.
Building scrapers (slice-02) without first validating federation risks serialization
rework. Validate the harder boundary first.

## Hand-off

Sibling feature directory at planning time: `docs/feature/openlore-federated-read/`.
