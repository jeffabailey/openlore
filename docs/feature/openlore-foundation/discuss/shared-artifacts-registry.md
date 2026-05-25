# Shared Artifacts Registry — openlore-foundation

Tracks data values flowing across journey steps. Every `${variable}` in a TUI mockup
must appear here with a single source of truth.

## Active artifacts (slice-01-claim-skeleton)

### `author_did`

- **Source of truth**: `~/.config/openlore/identity.toml` (resolved from the user's ATProto session at session-start; never re-read mid-flow)
- **Consumers**:
  - Compose preview (step 1)
  - Signing payload (step 2)
  - PDS record author (step 3)
  - Graph query output (step 4)
  - All future federated-read attribution (slice-03)
- **Owner**: identity subsystem (DESIGN wave responsibility)
- **Integration risk**: **HIGH** — silent drift breaks signature verification AND breaks the user's sense of "this is mine."
- **Validation**: assert `author_did` is captured once at compose-time and threaded through to graph output, not re-resolved per step.

### `claim_cid`

- **Source of truth**: content-addressed hash of the canonical signed claim, computed once at sign-time (step 2)
- **Consumers**:
  - Local store filename (`~/.local/share/openlore/claims/<cid>.json`)
  - PDS record `rkey` (step 3)
  - Graph node identifier (step 4)
  - Retract reference (`openlore claim retract <cid>`)
- **Owner**: claim model + canonicalization (DESIGN wave)
- **Integration risk**: **HIGH** — non-deterministic canonicalization breaks round-trip identity. If two re-runs produce different CIDs for the same logical claim, the system is broken.
- **Validation**: re-canonicalize the same claim N times locally; CIDs MUST match byte-for-byte.

### `at_uri`

- **Source of truth**: derived — `at://{author_did}/org.openlore.claim/{claim_cid}`
- **Consumers**: publish step (3), graph query output (4)
- **Owner**: derived; no separate source
- **Integration risk**: **MEDIUM** — a mismatch implies upstream `author_did` or `claim_cid` drift.
- **Validation**: must be reconstructible from author_did + claim_cid at any time.

### `local_claim_store`

- **Source of truth**: `~/.local/share/openlore/claims/` (XDG_DATA_HOME-respecting)
- **Consumers**: write (step 2), publish-retry queue (step 3), graph query (step 4), retract (future)
- **Owner**: local storage subsystem
- **Integration risk**: **MEDIUM** — path drift between write and read = silent data loss from user's POV.
- **Validation**: read-back of just-written claim returns identical field values.

### `composed_at`

- **Source of truth**: system clock at compose-time, UTC, RFC3339
- **Consumers**: claim record body, graph query output
- **Owner**: claim model
- **Integration risk**: **LOW** — but must be captured at COMPOSE time, not at sign time, so the user can trust the timestamp matches their intent.

### `pds_endpoint`

- **Source of truth**: ATProto session resolved from `identity.toml`
- **Consumers**: publish step (3), federated read (slice-03, future)
- **Owner**: identity subsystem
- **Integration risk**: **MEDIUM** — endpoint rotation mid-flow would be confusing; resolve once per session.

### `retraction_reference` (added 2026-05-25 per WD-11 / OD-3)

- **Source of truth**: the original `claim_cid` of the claim being retracted (or corrected). A retraction is itself a new claim whose body carries a CID-reference field pointing at the original.
- **Consumers**:
  - Retract command (`openlore claim retract <cid>`) — emits the CID reference into the new claim body.
  - Corrective-claim flow (anxiety scenario 2 in `gherkin-scenarios-expanded.md`) — uses the same reference mechanism with a different field name (`supersedes` / `corrects`); DESIGN owns the field-name choice.
  - Graph query annotation — when a query result contains a claim that has been retracted-by or superseded-by a later claim, the annotation is computed by walking these references.
  - Federated read (slice-03) — counter-claims published by peers also use this reference shape.
- **Owner**: claim model (DESIGN wave responsibility — choose Lexicon field name and arity).
- **Integration risk**: **HIGH** — if the reference is not stable across retract/correct/counter flows, the "claims are never deleted; they are only superseded" invariant cannot be enforced.
- **Validation**: every claim type that references another claim MUST carry the referenced `claim_cid` byte-for-byte. Walking the reference graph from any claim MUST always terminate (no cycles, no dangling references that resolve to nothing on the local PDS or any subscribed peer's PDS).

## Deferred artifacts (later slices, listed so DESIGN sees the full shape)

| Artifact | Introduced in | Notes |
|---|---|---|
| `subscribed_dids[]` | slice-03 federated-read | list of DIDs the user follows for claim ingestion |
| `trust_weight(did, predicate)` | slice-04 graph-and-scoring | per-author, per-predicate weight in [0.0, 1.0] |
| `philosophy_lexicon` | slice-01 (initial) -> slice-04 (refined) | controlled vocabulary for predicates and philosophy objects |
| `scraper_provenance` | slice-02 one-source-scraper | which scraper produced a claim, with version + ran-at |

## Validation rules across journey

1. `author_did` MUST match across all 4 steps. Drift = bug.
2. `claim_cid` MUST match across steps 2-4. Drift = canonicalization bug.
3. `at_uri` MUST be reconstructible from `author_did + claim_cid` at any time.
4. Graph query output (step 4) MUST display exactly the same field values shown at compose-time (step 1). Any divergence = silent normalization bug.
5. Every `retraction_reference` MUST resolve to a real `claim_cid` somewhere reachable (own PDS or a subscribed peer's PDS); dangling references = bug. Cycles in the reference graph = bug.
