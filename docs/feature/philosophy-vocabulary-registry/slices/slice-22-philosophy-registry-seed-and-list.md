# slice-22 · philosophy-registry-seed-and-list

**Goal:** Ship a discoverable shared philosophy vocabulary — implement the
`org.openlore.philosophy` record type + `validate_philosophy_json`, seed ≥10
well-known philosophies, and expose `openlore philosophy list` — so a user can
DISCOVER the vocabulary instead of inventing object strings.

## IN scope
- Complete `lexicon::validate_philosophy_json` (the RED scaffold): validate the
  record shape (`name`, `description` required; `aliases`, `seeAlso` optional) per
  `lexicons/org/openlore/philosophy.json`; accept + reject arms.
- The philosophy record type in the pure core (claim-domain/lexicon), with the
  stable object id derivation `org.openlore.philosophy.<name>`.
- Seed ≥10 well-known philosophy records (memory-safety, type-safety, test-driven,
  documentation-first, dependency-pinning, semantic-versioning, + ~4 more, e.g.
  immutability, composition-over-inheritance, local-first, backwards-compatibility)
  each with a real one-paragraph description + aliases.
- `openlore philosophy list` — prints each seed's object id + name + description
  (text default, `--json` opt-in), LOCAL/offline.

## OUT of scope
- `philosophy show` (slice-23), `philosophy add`/minting (slice-24), claim-compose
  advisory (slice-25), alias triangulation (slice-26), viewer surface (slice-27),
  scraper wiring (slice-28). No blocking/validation of claim objects.

## Learning hypothesis
- **Confirms if it succeeds:** a curated seed vocabulary + a `list` verb makes
  classification *discoverable* — the user picks a shared object instead of
  guessing, which is the precondition for federation/triangulation.
- **Disproves if it fails:** that a flat seeded record set is enough — if the seeds
  feel arbitrary or the list unusable, the "shared vocabulary" model needs rethink
  (hierarchy? external taxonomy?) before investing in mint/triangulate slices.

## Acceptance criteria
feature-delta.md → US-PV-001 (AC-001.1..4). Plus: `validate_philosophy_json`
accept + reject (missing description) arms tested (completes the scaffold; closes
the lexicon coverage gap noted this session).

## Data
Production seed records (the ~10 well-known philosophies) shipped as signed/valid
records on disk; `philosophy list` reads them. ATs assert against the real seeds
via the real `openlore philosophy list` subprocess (real-io).

## Dogfood moment
Same day: `./cli.sh philosophy list` → see the vocabulary; then
`./cli.sh claim add --object org.openlore.philosophy.memory-safety …` using an id
copied from the list.

## Dependencies
slice-01 (record/store), the `philosophy.json` Lexicon + `validate_philosophy_json`
scaffold. No dependency on other slice-22+ slices.

## Effort estimate
≤1 day. Pure record type + validator + seed data + one CLI list verb. Prefer NO
new crate (extend lexicon/claim-domain + cli); check-arch stays 21.

## Reference class
slice-01 (claim record + validation + CLI verb) and the lexicon validate_claim_json
work — same shape (record type + validator + verb), comparable size.

## Pre-slice SPIKE
Not required. The record schema exists (`philosophy.json`); the validator pattern
mirrors `validate_claim_json` (already implemented). Open question for DESIGN, not a
blocker: where the seed records physically live (a seeded collection in the store vs
an embedded `include_str!` seed set) — a DESIGN storage decision.

## Taste tests
- Ships 4+ new components? NO — one record type + one validator + seed data + one verb.
- Depends on a new abstraction shipped first? NO — the record schema + validator
  pattern already exist; this completes them.
- Disproves a pre-commitment? YES — tests whether a flat seeded vocabulary makes
  classification discoverable (learning hypothesis).
- Synthetic-data only? NO — ships the real production seed vocabulary.
- 2+ slices identical but for scale? N/A.
