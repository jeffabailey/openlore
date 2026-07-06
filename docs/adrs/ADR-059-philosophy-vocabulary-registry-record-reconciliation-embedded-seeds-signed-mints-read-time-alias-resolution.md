# ADR-059: Philosophy Vocabulary Registry — Record Reconciliation, Embedded Seeds + Signed Mints, and Read-Time Alias Resolution

- **Status**: Accepted
- **Date**: 2026-07-05
- **Deciders**: Morgan (nw-solution-architect) — DESIGN wave for `philosophy-vocabulary-registry`
- **Feature**: philosophy-vocabulary-registry (slices 22–28)
- **Paradigm**: Functional-leaning Rust (ADR-007) — pure record type + validator + vocabulary index; signing/persistence/reads in the effect shell.
- **Supersedes/Extends**: completes the `org.openlore.philosophy` RED scaffold left by slice-01 (ADR-005 namespace, ADR-006 CID/signing, ADR-009 hexagon boundaries).

## Context

Philosophy is the `object` of an `embodiesPhilosophy` claim. Today the record is a
RED scaffold: `lexicon::philosophy::Philosophy { id, label, description }` does NOT
match the shipped Lexicon schema (`lexicons/org/openlore/philosophy.json`:
`required: [name, description]`, optional `aliases`, `seeAlso`), and
`validate_philosophy_json` panics. There are no seed records, no `openlore philosophy`
verb, and the scraper hardcodes five philosophy strings plus a stray
`org.openlore.philosophy.mystery` with no shared record behind them. The result:
classification works mechanically but never federates — the opposite of OpenLore's point.

DISCUSS locked six decisions (feature-delta D1–D6): philosophy is a first-class
**signed record** (D1); **seeded but open** — curated seeds plus anyone-mints, no
gatekeeper, federated (D2); **advisory** at compose, never enforcing (D3); **aliases**
power read-time triangulation (D4); authoring stays **CLI**, viewer **read-only** (D5);
the seeds are the scraper's **single source** (D6). Constraints: local-first/offline,
claims-not-truth (no arbiter), anti-merging (triangulation groups but never merges
attribution), reuse-first (prefer NO new crate; check-arch stays 21 members).

The load-bearing tension is between D1 (signed record) and D2 (shipped seeds). A record
shipped inside the binary has **no natural signer**: signing the seeds with a project key
would manufacture exactly the central authority D2 forbids. This ADR resolves that
honestly rather than papering over it.

## Decisions

### D1 — Reconcile the record type to the Lexicon schema; object-id is DERIVED, not stored

Replace `Philosophy { id, label, description }` with the serde mirror of the shipped schema:

```
Philosophy {
    name: String,            // required — short identifier, e.g. "memory-safety"
    description: String,     // required — one-paragraph definition
    aliases: Vec<String>,    // optional (serde default) — near-synonym names
    see_also: Vec<String>,   // optional (serde rename "seeAlso", default) — URIs
}
```

The Lexicon record carries **no `id` field**; the stable object id is a pure derivation
from `name`:

```
object_id(name) = "org.openlore.philosophy." + normalize(name)
```

`normalize` is a total pure function, specified precisely (review-hardened) so no two
distinct names silently collide and the DELIVER crafter has zero ambiguity:

1. Unicode-NFKC, then lowercase.
2. Trim leading/trailing whitespace.
3. Replace every run of one-or-more chars NOT in `[a-z0-9]` (whitespace, `_`, punctuation,
   any `-` runs, any non-ASCII residue) with a SINGLE `-`.
4. Strip a leading and/or trailing `-`.

Worked examples (also encoded as build-time unit tests):
```
normalize("Memory-Safety")   -> "memory-safety"
normalize("memory  safety")  -> "memory-safety"
normalize("memory_safety")   -> "memory-safety"
normalize("memory---safety") -> "memory-safety"   (runs collapse to one -)
normalize("  memory-safety ")-> "memory-safety"   (trim)
normalize("Test-Driven")     -> "test-driven"      (matches the shipped seed object)
normalize("!!!")             -> ""                 (empty → REJECTED at the mint smart constructor, never derived)
```

The empty/all-invalid name is rejected at the smart-constructor boundary (mint path),
never at derivation. Because collisions are POSSIBLE in principle (two names normalizing
to one id), they are caught by: (a) a build-time test asserting no two SEED names collide
under `normalize` (KPI-PV-1); (b) the `UNIQUE(object_id)` column on the `philosophies`
table; (c) the mint pre-check against the seed set (D3/AC-003.3). `normalize` is FROZEN
once shipped — changing it would re-derive existing ids (a breaking migration), so it is
treated as a stable contract. The claim `object` string `org.openlore.philosophy.memory-safety`
is thus the derived id of the `memory-safety` record — the join between the claim graph and
the vocabulary, and it EXACTLY matches the raw object strings shipped by slice-01 claims
(backward-compatible by construction; no data migration).

### D2 — `validate_philosophy_json` mirrors `validate_claim_json` (accept + reject arms)

Implement the validator as a per-field-gated pure function, mirroring `claim.rs`:
required-field presence (`name`, `description`) → typed `MissingField`; optional
`aliases`/`seeAlso` must be arrays-of-strings → typed `InvalidType`; then serde
deserialize into `Philosophy` with a `SchemaMismatch` catch-all. **Reuse the existing
`lexicon::LexiconError` enum** (already carries `MissingField`, `InvalidType`,
`SchemaMismatch`) rather than minting a parallel error type. This completes the RED
scaffold with both an accept arm and a named-field reject arm (AC-003.4).

### D3 — Seed storage/signing model: EMBEDDED well-known constants + SIGNED user mints

**Seeds are an embedded, unsigned, well-known constant set. Minted philosophies are
signed, content-addressed records.** The record TYPE is uniform (`Philosophy`, one
validator); only **provenance** differs:

- **Seed provenance** — ≥10 curated records shipped inside the `lexicon` crate via
  `include_str!("seeds.json")`, validated at compile-time-adjacent test time by
  `validate_philosophy_json`. No signer, no on-disk store, no network. They are the
  shared reference dictionary everyone holds by convention — the honest federation
  primitive for a no-gatekeeper namespace (D2).
- **Minted provenance** — `openlore philosophy add` composes a `Philosophy` record,
  **signs it via the existing claim signing model** (`claim_domain::canonicalize` →
  `compute_cid` → `IdentityPort::sign`, ADR-006), and persists it locally as
  `philosophies/<cid>.json` + a `philosophies` DuckDB row. Minted records federate
  content-addressed exactly like claims (D1).

`list`/`show` read the **union** of embedded seeds ∪ minted records. A minted name that
collides with a seed's derived object-id is refused with guidance (AC-003.3), enforced by
a `UNIQUE(object_id)` column on the `philosophies` table plus a pre-check against the seed
set.

*Rationale (Earned Trust, principle 12): we do not pretend a binary-shipped seed was
authored by anyone. Provenance is explicit in the type, so no reader mistakes a
convention-shared constant for a signed assertion. The signer that does not exist is named
as not existing.*

### D4 — Minted storage mirrors the claim sign→persist path (NO new crate)

`philosophies/<cid>.json` artifact (atomic tmp+fsync+rename) + a `philosophies` DuckDB
table added by a forward-only `schema_v4` migration in `adapter-duckdb`, reusing the
`write_signed_claim` transaction-equivalent pattern verbatim. No new crate; no new signing
model (AC-003.2). Column datatypes (review-pinned; mirror the `claims` table precedent):

```
cid           TEXT PRIMARY KEY,          -- the signed record CID
object_id     TEXT UNIQUE NOT NULL,      -- derived org.openlore.philosophy.<normalize(name)>
name          TEXT NOT NULL,
description   TEXT NOT NULL,
author_did    TEXT NOT NULL,             -- the minter's DID
composed_at   TEXT NOT NULL,             -- ISO-8601, e.g. "2026-07-05T12:00:00Z" (claim precedent)
artifact_path TEXT NOT NULL              -- relative: "philosophies/<cid>.json"
```
`aliases`/`seeAlso` live ONLY in the artifact JSON (denormalized; the table indexes the
join keys). The migration is forward-only + idempotent, mirroring the `schema_v3` runner.

### D5 — Alias triangulation is a pure read-time derivation over an immutable claim store

A pure `VocabularyIndex` (built in `lexicon` from seeds ∪ minted records) exposes:
`resolve(object_id) -> Option<canonical_object_id>` and
`equivalence_class(canonical) -> Vec<object_id>` (canonical + each alias mapped through the
SAME `normalize` into an object-id). At graph/survey read time (slice-26) the verb widens
the queried `--object` to its equivalence class and asks the store for claims whose `object`
is **any** member; the pure `scoring`/survey aggregation groups them under the canonical
object while keeping every per-author row distinct. **Stored claim `object` bytes are never
rewritten** (signed bytes are immutable, AC-005.2); resolution is display/aggregation only.
The widened store read stays a UNION-ALL projecting `author_did` explicitly (anti-merging,
`xtask check-arch::no_cross_table_join_elides_author`) — aggregation stays in the pure core,
never in SQL.

### D6 — Advisory compose is display-only; signed payload is byte-unchanged

`claim add` builds the `VocabularyIndex` and classifies `--object` as
known-canonical / known-via-alias / unknown, rendering one advisory preview line
(`↳ resolves to <canonical> (alias)` or `⚠ not a known philosophy — will be signed as-is`).
It **never** blocks and **never** alters the bytes signed — the user's typed `object` is
what gets canonicalized and signed (D3/claims-not-truth, AC-004.2/.3).

### D7 — CLI verb + scraper single-source

Add `Command::Philosophy(PhilosophyCommand { List{json}, Show{query}, Add{name,
description, alias, see_also} })` with `verbs/philosophy_{list,show,add}.rs`, mirroring the
existing verb structure. The scraper's `signal_predicate_mapping` (slice-28) is validated
against the embedded seed set so every proposed `object` is a known philosophy and no drift
string survives (D6/AC-007).

`philosophy show <query>` accepts EITHER form (review-pinned disambiguation, AC-002.1):
- if `<query>` starts with the `org.openlore.philosophy.` NSID prefix → treat as an
  object-id and `VocabularyIndex::resolve` it directly;
- otherwise → treat as a name, derive its object-id via `object_id(normalize(<query>))`,
  and look that up.
Either way an unknown query exits non-zero with the plain "no such philosophy; try
`philosophy list` or `philosophy add`" guidance (AC-002.2) — never a stack trace. The
disambiguation is a pure prefix test in the verb (effect-shell edge), not in the core.

## Consequences

### Positive
- slice-22 (seed + list) is buildable with ZERO storage/signing work: the list verb reads
  the embedded seed constant set — offline by construction (AC-001.4), the smallest
  end-to-end discovery skeleton.
- The D1/D2 tension is resolved without a fake gatekeeper: seeds are shared constants,
  mints are signed + federated. Provenance is a type, not a comment.
- Reuse-first honored: signing (claim-domain), storage pattern (adapter-duckdb), validator
  pattern (lexicon), verb pattern (cli) are all extended, not reinvented. **No new crate;
  workspace stays 21 members.**
- Alias resolution is pure and total; property-testable; the immutable claim store is never
  mutated, so triangulation cannot corrupt attribution.

### Negative
- Two provenance paths (seed vs minted) mean `list`/`show`/resolution must union two
  sources. Mitigation: `VocabularyIndex::from(seeds, minted)` is the single merge point;
  every consumer reads the index, not the two sources directly.
- A `schema_v4` migration adds a table. Mitigation: forward-only, idempotent, mirrors the
  established `schema_v3` runner.
- Seeds shipped in the binary can only change with a release. Accepted: seeds are a
  curated reference set by design; evolution beyond them is exactly what minting is for.

### Earned Trust (probes)
- `philosophy add` participates in the existing composition-root WIRE→PROBE→USE gauntlet;
  storage/identity adapters already probe fsync + key availability and refuse startup with
  `health.startup.refused` on failure. The `schema_v4` table is added to the storage
  probe's schema-version assertion.
- A test-time probe validates **every embedded seed** through `validate_philosophy_json`
  (KPI-PV-1) and asserts every scraper mapping predicate resolves in the seed set (KPI-PV-6)
  — the substrate lie guarded against is "a seed or a mapping drifted out of the shared
  vocabulary."

## Review resolutions (DESIGN review, 2026-07-05)

nw-solution-architect-reviewer: **APPROVED WITH CONDITIONS** — 0 blockers, 2 should-fix, 1
nit. The core model (provenance-as-a-type seeds, derived object-id backward-compat, alias
read-time seam, no-gatekeeper/no-arbiter) was validated sound.

| Finding | Resolution |
|---|---|
| [SHOULD-FIX] `normalize()` underspecified (collision/edge cases) | **D1** — formalized as a 4-step algorithm (NFKC→lowercase, trim, non-`[a-z0-9]` runs → single `-`, strip edge `-`) with 7 worked examples encoded as build-time tests; frozen-contract note; collision caught by build-test + `UNIQUE(object_id)` + mint pre-check. |
| [SHOULD-FIX] `philosophy show` name-vs-object-id disambiguation | **D7** — NSID-prefix test: `org.openlore.philosophy.*` → resolve as object-id; else derive `object_id(normalize(name))`; unknown → non-zero plain guidance. |
| [NIT] `philosophies` table datatypes | **D4** — full column spec pinned (TEXT PK/UNIQUE, ISO-8601 `composed_at`, relative `artifact_path`; aliases/seeAlso in the artifact only). |

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **(b) Seed the store at `init`** (seeds become ordinary rows) | Couples discovery to a mutable store, breaks the "seeds are a fixed shared dictionary" property, and forces a signer for records that have none. Re-seeding/upgrade semantics get murky. Rejected. |
| **(c) All philosophies (seeds + mints) as signed records** | Requires a signer for the seeds — either a project key (manufactures the central authority D2 forbids) or the installing user (makes "well-known" seeds per-user artifacts that don't federate as a shared set). Directly violates D2. Rejected. |
| **CID-less, name-addressed mints** (`philosophies/<name>.json`, no signature) | Simpler, but abandons D1 (no authorship/non-repudiation) and diverges from the claim federation model. Rejected — signing is cheap reuse and D1 is locked. |
| **Alias rewrite at write time** (normalize claim `object` to canonical on store) | Would triangulate by mutating stored objects — violates signed-bytes immutability (AC-005.2) and anti-merging. Rejected. |
| **New `vocabulary` crate** | No boundary justifies it: the record + validator + index are pure `lexicon` concerns; signing is `claim-domain`; storage is `adapter-duckdb`. A new crate would raise check-arch's member count for zero isolation benefit. Rejected. |
| **Parallel `PhilosophyLexiconError` enum** | The existing `LexiconError` already models MissingField/InvalidType/SchemaMismatch. A parallel type duplicates the reject-arm surface. Rejected — reuse. |
</content>
</invoke>
