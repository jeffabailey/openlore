# ADR-026: Production PLC DID-Document Multibase Pubkey Decode — Resolving the slice-03 DV-4 Test-Only Seam

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-104 / KPI-AV-3 + the inherited KPI-FED-6 caveat for openlore-appview-search (slice-05)
- **Feature**: openlore-appview-search (slice-05)
- **Resolves**: the production multibase (`z6Mk...`) PLC DID-document pubkey-decode path that slice-03 left as a TEST-ONLY seam (its DV-4: `OPENLORE_PEER_PUBKEY_HEX_<did>`). This is the single biggest technical risk inherited into slice-05 (flagged in feature-delta.md Risks logged + the inherited-locks KPI-FED-6 caveat).
- **Extends**: ADR-002 (identity = ATProto DID), ADR-016 (pull-time verification). Reuses the pure `claim-domain::verify` core (no second verification path).

## Context

slice-03 shipped pull-time signature verification (KPI-FED-6) but, per its DV-4
DELIVER decision, used a TEST-ONLY seam for the peer's verification key:
`adapter-atproto-did` reads `OPENLORE_PEER_PUBKEY_HEX_<did>` from the environment
(mirroring the PDS-endpoint seam), and `FakePeerPds`'s `resolveDid` DID-document
carries a placeholder key. Production multibase (`z6Mk...`) PLC DID-document key
decode was an explicit documented TODO ("Real PLC key decode lands when production
PLC resolution ships (slice-04+)").

slice-05's verified-before-index gate (WD-104 / KPI-AV-3) verifies signatures for
ARBITRARY network authors the indexer has never met — there is no test seam
populated for them. **For KPI-AV-3 to hold against REAL network data, the indexer
MUST decode the real verification key from each author's PLC DID document.** This
is the hard technical dependency the task brief and DISCUSS both flag as the single
biggest inherited risk. It has been deferred since slice-03; slice-05 resolves it.

The verification contract (reused from slice-01/03, unchanged):

```
claim_domain::verify(record, author_public_key) -> Result<(), VerifyError>   // PURE
claim_domain::compute_cid(record) -> Cid                                     // PURE
```

The MISSING piece is the EFFECT-shell function that turns a DID into an
`author_public_key`: resolve the DID document, locate the assertion/verification
method for the `org.openlore.application` key, and decode the `z6Mk...` multibase
publicKeyMultibase into the Ed25519 verification key the pure core consumes.

## Decision

**Implement the production PLC DID-document multibase pubkey-decode path in the
indexer's VERIFY-ONLY identity adapter, as a driven-port method behind
`IdentityResolvePort`. The decode is an EFFECT (it does DID-document resolution /
network I/O), kept at the edge; the resulting Ed25519 key feeds the PURE
`claim_domain::verify` core unchanged (no second verification path). The slice-03
`OPENLORE_PEER_PUBKEY_HEX_<did>` env seam is RETAINED as a TEST-ONLY override (it
short-circuits the decode for hermetic acceptance fixtures) but is NEVER the
production path; a release build with the seam active is a guard violation.**

### The decode path (effect shell)

```
IdentityResolvePort::resolve_verification_key(did: &Did) -> Result<VerificationKey, ResolveError>
```

Production implementation (`AtProtoDidResolveAdapter`, verify-only):

1. **Resolve the DID document.** For `did:plc:*`, query the PLC directory
   (`https://plc.directory/<did>`) for the DID document. For `did:web:*` (future),
   the well-known path. (The walking skeleton targets `did:plc:*`, matching the
   slice-01/03 identity model, ADR-002.)
2. **Locate the verification method.** Find the verification method in the DID
   document whose id matches the OpenLore application key fragment
   (`#org.openlore.application`, per ADR-002's per-application derived key), with a
   `type` of `Multikey` / `Ed25519VerificationKey2020` and a
   `publicKeyMultibase` value.
3. **Decode the multibase `z6Mk...` value.** `z` is the multibase base58btc prefix;
   the decoded bytes are a multicodec-prefixed Ed25519 public key (`0xed01` prefix
   + 32 key bytes). Strip the multicodec prefix; the remaining 32 bytes are the
   Ed25519 verification key.
4. **Return the `VerificationKey`** the pure `claim_domain::verify` consumes.

The decode itself (base58btc + multicodec-prefix strip) is small, pure, and
testable in isolation; it lives in `claim-domain` (a pure helper,
`decode_ed25519_multibase(s: &str) -> Result<VerificationKey, DecodeError>`) so
it has no I/O dependency and is property-/mutation-tested like the rest of the
pure core. The EFFECT part (resolving the DID document over the network) is the
adapter's job.

### Test seam discipline (retained but production-forbidden)

- The `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam is RETAINED for hermetic
  acceptance: when set, `resolve_verification_key` returns the seam key WITHOUT
  network resolution. This keeps `indexer_rejects_unverified_claim` and the
  ingest acceptance suite hermetic (a fixture relay + fixture DID-doc keys).
- The seam is TEST-ONLY. A `release` build with the seam active is a guard
  violation, enforced like slice-03's `no_autoconfirm_in_release_build`:
  `xtask check-arch` adds `no_pubkey_seam_in_release_build` (the env-seam code path
  is `#[cfg(any(test, debug_assertions))]`-gated or guarded; a release binary that
  reads the seam fails the check). This is the structural guarantee that production
  verification uses the REAL decode, not the seam (KPI-AV-3 against real data).
- A NEW catalogued substrate-lie gold test exercises the REAL decode against a
  fixture DID document carrying a real `z6Mk...` value (a known test keypair),
  asserting the decoded key verifies a known-good signature AND rejects a tampered
  one. This proves the production decode path works, not just the seam.

### Verification is an ingest gate, not a runtime per-result check (reuse)

Per WD-104 + ADR-024: the decode + `verify` + `compute_cid` happen at INGEST. A
record enters `indexed_claims` ONLY if `verify` and the CID recompute both pass,
and the row records `verified_against` (the key id it verified against, ADR-025).
Search never re-verifies (the index is trusted local disk, exactly as slice-03
trusts `peer_claims` at query time). The `[verified]` marker is a construction
guarantee.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Keep the slice-03 test-only `OPENLORE_PEER_PUBKEY_HEX_<did>` seam as the slice-05 path too (defer real decode again)** | Rejected — this is the deferral that has accumulated since slice-03 and the brief explicitly says to resolve it now. KPI-AV-3 CANNOT hold against REAL network data with a test seam: arbitrary network authors have no seam populated. Deferring again would make the verified-before-index gate vacuous in production. The decode is the hard dependency; slice-05 resolves it. |
| **Verify against the author's `author` field key fragment WITHOUT resolving the DID document** | Rejected — trivially forgeable. The `author` field is part of the (attacker-controllable) record; trusting a key derived from it would let a fabricated record carry a fabricated key that verifies its own fabricated signature. The trust anchor MUST be the DID document resolved from the PLC directory (the authoritative source), exactly as slice-03 resolves the peer's DID doc (and refuses the cached endpoint, ADR-016 — same "trust the authoritative resolution, not the record" discipline). |
| **Re-verify at query/search time instead of (or in addition to) ingest time** | Rejected (consistent with ADR-016 + ADR-024). Every search would do N DID-doc resolutions + N Ed25519 verifications — network I/O on the search hot path, defeating graceful degradation and latency. Verification is centralized at ingest; `verified_against` records the result; search trusts the index. |
| **Put the DID-document resolution (the network I/O) in the pure `claim-domain` core** | Rejected — violates ADR-007 / ADR-009 (pure core has no I/O; I-2). The PURE part is the multibase/multicodec DECODE (no I/O, lives in `claim-domain`); the EFFECT part is the DID-document RESOLUTION (lives in the verify-only identity adapter). The boundary is exactly the pure-core/effect-shell line. |
| **A second verification implementation in the indexer (separate from `claim-domain::verify`)** | Hard reject. WD-104 mandates reuse of the pure `claim-domain` verification core — no second verification path (a second path could drift and verify-differently, breaking KPI-AV-3's "same trust contract as slice-03"). The indexer calls `claim_domain::verify` exactly as slice-03's `peer pull` does. |

## Consequences

### Positive

- KPI-AV-3 holds against REAL network data: arbitrary network authors' signatures
  are verified against keys decoded from their authoritative PLC DID documents.
- Resolves a dependency deferred since slice-03 — the brief's flagged single
  biggest inherited risk — with a clean pure-core/effect-shell split (pure decode
  helper in `claim-domain`; effect resolution in the verify-only adapter).
- Reuses the pure `claim_domain::verify` core unchanged (no second verification
  path); the slice-03 verification discipline carries forward verbatim.
- The retained-but-production-forbidden test seam keeps the ingest acceptance suite
  hermetic while the `no_pubkey_seam_in_release_build` guard guarantees production
  uses the real decode.

### Negative

- **DID-document resolution is a network dependency at ingest** (the PLC directory
  or the DID's PDS). Mitigation: it is at INGEST, not on the search/local-first hot
  path; per-source/per-record fault isolation (ADR-024) means an unresolvable DID
  rejects only that record (`indexer.ingest.rejected{reason: did_unresolvable}`),
  not the whole pass. The local-first CLI flows have zero new network dependency.
- **PLC directory availability** becomes an ingest-time dependency. Mitigation: a
  configurable PLC directory endpoint; the ingest pass degrades per-record on
  resolution failure; this is an indexer-side concern, invisible to the CLI's
  local-first flows.
- **Multibase/multicodec edge cases** (key type variants, prefix variations). The
  walking skeleton targets `did:plc:*` Ed25519 `z6Mk...` keys (the slice-01/03
  identity model). Other key types / DID methods are rejected with a clear
  `ResolveError::UnsupportedKeyType` (a documented, testable boundary), not a
  silent mis-decode. The decode helper is mutation-tested.

### PLC resolution failure-mode (KPI-AV-3 precision — review clarification)

To keep the cardinal verify-before-index guarantee (KPI-AV-3 / I-AV-1) unambiguous
for DISTILL + DELIVER, the failure semantics are:

1. **Config + default.** The indexer configuration specifies the PLC directory
   endpoint; the default is `https://plc.directory`. This is an indexer-side
   config key, invisible to the CLI's local-first flows.
2. **Per-record rejection (never silent index).** If a single author's DID cannot
   be resolved (PLC unreachable for that DID, DID-doc malformed, no usable
   verification key), that record is **REJECTED** —
   `indexer.ingest.rejected{reason: did_unresolvable}` — and **never indexed**. An
   unresolved key can NEVER result in an indexed (and therefore searchable) claim;
   KPI-AV-3 ("zero unverified claims indexed") holds by construction.
3. **No global hard-refuse on a transient outage.** If ALL DIDs in one ingest pass
   fail resolution (e.g. a transient PLC directory outage), the pass logs a warning
   and **continues** — subsequent passes retry. DID resolution is per-record
   fault-isolated (ADR-024); unlike the index-store `fsync`-honesty probe (a
   hard startup refusal, ADR-025), a transient PLC outage does NOT crash or refuse
   the indexer. It simply indexes nothing new until resolution recovers — the
   conservative, safe direction (under-index, never mis-index).

### Earned Trust

The verification key resolution is a dependency the indexer MUST probe (principle
12 — "every dependency you don't probe is an act of faith"). Concretely:

- **The pure decode helper** (`claim_domain::decode_ed25519_multibase`) has no
  `probe()` (it touches no substrate); its Earned-Trust analog is
  property/mutation testing: a known `z6Mk...` fixture decodes to a known 32-byte
  key; a malformed multibase string errors (never panics, never mis-decodes); the
  decoded key round-trips (encode∘decode == identity for valid keys). Mutation
  testing on the decode + prefix-strip.
- **The verify-only identity adapter** ships a `probe()` within the 250ms budget
  that exercises the catalogued substrate-lie scenario: resolve a FIXTURE DID
  document carrying a real `z6Mk...` value, decode it, and assert (a) the decoded
  key VERIFIES a known-good signature on a known record, and (b) the decoded key
  REJECTS a tampered signature / CID-mismatched record. A probe that passes only
  against the test SEAM (not the real decode) is a CI failure (the gold test runs
  the REAL decode path). This is the indexer's "what if the network lies about an
  author's key?" check — the design refuses to trust an unverified record and the
  probe proves the rejection path works against a real-shaped DID document.
- **The release-seam guard**: `xtask check-arch` rule `no_pubkey_seam_in_release_build`
  is the meta-probe that the production path is the REAL decode, not the seam
  (self-application of principle 12: a probe that verifies the verification uses the
  real key source).

## Revisit Trigger

- `did:web:*` or non-PLC DID methods become a J-005 need (network authors using
  other DID methods). Add the resolution branch — the pure decode helper is method-
  agnostic; only the DID-document resolution differs.
- A key type other than Ed25519 `z6Mk...` appears among network authors. Extend the
  decode helper's supported-key-type set with a new ADR (the current boundary
  rejects unsupported types explicitly, never mis-decodes).
- PLC directory resolution becomes a latency/availability bottleneck at ingest
  scale. Add a DID-document cache with a TTL (the DID-doc is the trust anchor; a
  cache needs careful invalidation on key rotation — its own ADR).
- Key rotation handling becomes load-bearing (an author rotates their key; old
  claims were signed with the old key). Mirror the slice-03 "trust the current
  resolution, don't fall back to a cached endpoint" discipline; define a rotation
  policy (likely: verify against the key valid at the claim's `composedAt`, which
  needs PLC audit-log resolution) — out of scope for the walking skeleton, flagged.
