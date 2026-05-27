# Shared Artifacts Registry — openlore-federated-read

- **Feature**: openlore-federated-read (slice-03)
- **Wave**: DISCUSS
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)

This registry catalogues every `${variable}` and shared identifier that
appears in more than one place across the two journeys
(`subscribe-and-read-federated.yaml` + `author-counter-claim.yaml`) plus the
locked artifacts inherited from openlore-foundation.

The registry's purpose is to make horizontal-integration failure modes
visible at DISCUSS time, before DESIGN starts choosing schemas.

## Artifact: `peer_did`

```yaml
peer_did:
  source_of_truth: argument to `openlore peer add` (validated against ATProto DID resolution at subscribe time)
  consumers:
    - peer-subscription record (peer_subscriptions store)
    - peer_claims attribution column (every peer claim row carries this)
    - graph query --federated output (author grouping header)
    - peer remove CLI argument
    - counter-claim audit trail ("counters X by <peer_did>" annotation)
  owner: openlore-federated-read (new in slice-03; was not present in slice-01)
  integration_risk: HIGH
  risk_explanation: >
    DID drift between subscribe-time validation and later use would corrupt
    attribution, which is the load-bearing anti-merging invariant. Possible
    causes: case-normalization differences, trailing-whitespace mishandling,
    DID-URL fragment stripping inconsistencies.
  validation: >
    All four touchpoints (subscribe, pull, query, remove) MUST compare DIDs
    byte-for-byte against the persisted subscription record. The integration
    test `peer_did_round_trip` in the acceptance suite asserts this.
```

## Artifact: `peer_claim_cid`

```yaml
peer_claim_cid:
  source_of_truth: >
    Recomputed locally at pull time via claim-domain::compute_cid against the
    received canonical CBOR. MUST byte-match the rkey under which the record
    is published on the peer's PDS.
  consumers:
    - peer_claims store key (CID is the primary key for peer claims, same as for author claims)
    - graph query --federated output (per-row CID)
    - counter-claim references[] field (target_cid)
    - counter-claim CLI argument (`openlore claim counter <peer_claim_cid>`)
  owner: claim-domain (reuses slice-01 canonicalization; no new logic, just applied to peer-sourced data)
  integration_risk: HIGH
  risk_explanation: >
    If a peer's CID does not match the locally-recomputed CID, the federation
    thesis is broken — canonicalization is non-deterministic across implementations.
    This is the same risk surfaced in slice-01 (KPI-4) but with the additional
    surface of inbound bytes from an untrusted peer.
  validation: >
    Every peer pull MUST recompute the CID locally and reject any record where
    recomputed != published. The acceptance test `peer_cid_round_trip` asserts
    this against a fixture peer that publishes deliberately-malformed records.
```

## Artifact: `peer_pds_endpoint`

```yaml
peer_pds_endpoint:
  source_of_truth: resolved from peer's DID document at subscribe time; refreshed at each pull
  consumers:
    - peer pull fetch URLs
    - peer subscription record (cached for diagnostics)
  owner: adapter-atproto-did (existing; resolves DID documents)
  integration_risk: MEDIUM
  risk_explanation: >
    Peers can legitimately rotate their PDS. The system must tolerate this:
    re-resolve at each pull, do NOT trust the cached endpoint past one pull.
    Resilience here is more important than strict consistency.
  validation: >
    Peer pull MUST re-resolve the peer DID document at each invocation. If the
    PDS endpoint has changed since the subscription was recorded, log it and
    continue. The acceptance test `peer_pds_rotation_tolerated` asserts this
    using a fixture peer whose DID document changes between two pulls.
```

## Artifact: `subscribed_at`

```yaml
subscribed_at:
  source_of_truth: ClockPort.now_utc() at peer-add time
  consumers:
    - peer subscription record
    - peer remove diagnostic output ("subscribed_at: <ts>")
    - idempotent re-subscribe message ("already subscribed since <ts>")
  owner: adapter-system-clock (existing)
  integration_risk: LOW
  risk_explanation: Informational; user-facing diagnostic only. Not part of any signed payload.
  validation: Round-trip serde test on the subscription record schema.
```

## Artifact: `target_cid` (counter-claim target)

```yaml
target_cid:
  source_of_truth: peer_claims (populated by `openlore peer pull`)
  consumers:
    - counter-claim CLI argument (`openlore claim counter <target_cid> ...`)
    - counter-claim references[] field (the signed payload)
    - counter-claim compose preview ("counters: <target_cid> (by <peer_did>)")
    - graph query --federated annotation on both target and counter rows
  owner: claim-domain (the references[] field is owned by ADR-008's ReferenceType.Counters variant)
  integration_risk: HIGH
  risk_explanation: >
    If the CLI-arg target_cid is normalized differently from the references[]
    field value, the counter-relationship will not be discoverable in queries.
    This is a high-risk drift surface because the CID lives in three places
    (CLI arg, references[] payload, query annotation) that may apply different
    string handling.
  validation: >
    Acceptance test `counter_target_cid_round_trip` asserts that the CLI arg
    is the SAME byte string as the references[].cid value in the signed
    payload AND the same string surfaces in subsequent graph query
    annotations.
```

## Artifact: `counter_claim_reason`

```yaml
counter_claim_reason:
  source_of_truth: --reason CLI argument
  consumers:
    - compose preview (displayed verbatim, wrapped at 78 cols)
    - signed claim payload (`reason` field; first-class Lexicon field)
    - graph query --federated output (displayed on counter-claim rows)
  owner: lexicon (new field added to org.openlore.claim Lexicon schema)
  integration_risk: MEDIUM
  risk_explanation: >
    The reason field is the user's articulated disagreement. Display fidelity
    matters: a UI surface that truncates or escapes the reason without
    showing the full text would weaken the J-003b promise that disagreement
    is a public structured artifact.
  validation: >
    Acceptance test `counter_reason_display_fidelity` asserts the --reason
    text appears byte-equal in compose preview AND in subsequent graph query
    output (after Unicode normalization).
```

## Artifact: `counter_claim_cid`

```yaml
counter_claim_cid:
  source_of_truth: claim-domain::compute_cid at sign time (same pipeline as a regular claim)
  consumers:
    - local store filename (~/.local/share/openlore/claims/<cid>.json)
    - PDS record rkey
    - at-uri suffix
    - graph query --federated output (per-row CID)
    - subsequent retract reference if the user changes their mind
  owner: claim-domain (no new logic; same CID computer as slice-01)
  integration_risk: HIGH
  risk_explanation: Same as any signed claim CID — canonicalization determinism.
  validation: Slice-01's existing CID round-trip property tests cover this; no new test needed.
```

## Artifact: `purge_flag`

```yaml
purge_flag:
  source_of_truth: --purge CLI flag on `openlore peer remove`
  consumers:
    - peer remove dispatch (soft vs hard branch)
    - confirmation prompt visibility
    - peer_claims deletion transaction
  owner: cli (new in slice-03)
  integration_risk: LOW
  risk_explanation: >
    Boolean flag; misuse is bounded (irreversible delete with required
    confirmation). The only failure mode is forgetting to gate the prompt on
    the flag — caught by acceptance test.
  validation: >
    Acceptance test `peer_remove_purge_requires_confirmation` asserts that the
    --purge code path REQUIRES a yes/no prompt and that exit on "no" leaves
    both subscription and cached claims intact.
```

## Inherited artifacts from openlore-foundation (still load-bearing here)

These artifacts are owned by `openlore-foundation`; slice-03 consumes them
and must NOT redefine.

| Artifact | Owner | Why slice-03 cares |
|---|---|---|
| `author_did` | identity adapter (slice-01) | Counter-claim author = current user; needed in compose preview and signed payload |
| `claim_cid` (author's own claim) | claim-domain (slice-01) | Counter-claim itself is just a claim with a CID |
| `at_uri` | adapter-atproto-pds (slice-01) | Counter-claim's at-uri returned on publish; displayed in success message |
| The literal text "not as truth" | US-001 AC (slice-01) | MUST appear in counter-claim compose preview — content-frozen |
| `references[].type == Counters` | ADR-008 + claim-domain (slice-01) | Slice-03 uses this enum variant; no new variant added |
| Single publish path (VerbClaimPublish internals) | ADR-003 + cli (slice-01) | `claim counter` reuses this path; no parallel publish code |

## Integration validation gates (whole feature)

### Gate 1: Per-claim attribution preservation (the anti-merging gate)

```yaml
gate: federation_attribution_preserved
description: >
  Every output row from `openlore graph query --federated` must have a
  distinct (author_did, claim_cid) tuple. NO row may represent a
  multi-author "consensus."
asserted_at: acceptance suite, integration test layer
must_pass_for: every story in this feature
failure_consequence: feature is unshippable; the entire trust model collapses
```

### Gate 2: Peer-published CID equals locally-recomputed CID

```yaml
gate: peer_cid_round_trip
description: >
  For every peer claim pulled, the locally-recomputed CID MUST byte-match
  the rkey under which the record is published.
asserted_at: acceptance suite, peer pull adapter probe
must_pass_for: US-FED-002 (peer pull), US-FED-003 (federated query)
failure_consequence: federation contract broken; canonicalization differs
  across implementations; slice-03 hypothesis disproven
```

### Gate 3: Counter-claim reference is byte-stable end-to-end

```yaml
gate: counter_target_cid_round_trip
description: >
  The target_cid argument to `openlore claim counter` MUST be the
  byte-identical value persisted in the counter-claim's references[].cid
  field AND surfaced in subsequent graph query annotations.
asserted_at: acceptance suite
must_pass_for: US-FED-004 (counter-claim authoring)
failure_consequence: counter-relationships invisible in queries; the J-003b
  hypothesis cannot be validated
```

### Gate 4: Soft-remove vs hard-purge separation

```yaml
gate: peer_remove_purge_separation
description: >
  `openlore peer remove <did>` without --purge: subscription removed,
  cached peer_claims retained. With --purge: subscription removed AND
  cached peer_claims for that peer deleted; author_claims untouched.
  --purge REQUIRES confirmation prompt.
asserted_at: acceptance suite
must_pass_for: US-FED-005 (peer remove)
failure_consequence: user data destroyed by accident; J-003c (revocability
  without residue) trust violated
```
