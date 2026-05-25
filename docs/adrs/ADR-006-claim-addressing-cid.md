# ADR-006: Claim Addressing = IPLD CID (dag-cbor + sha2-256, base32 lowercase)

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

Every signed claim MUST have a stable, content-addressed identifier that:

- Is computable from the canonical bytes of the signed claim alone (no central
  authority, no UUID coordination).
- Is byte-identical across machines, across re-runs, and across language
  implementations (per US-002 Example 2 and KPI-4 round-trip identity).
- Is usable as the local filesystem name, the PDS record `rkey`, the graph
  node id, and the retraction-reference target (per the shared-artifacts
  registry).
- Is portable into the broader IPFS/IPLD ecosystem if and when OpenLore wants
  to address claims from outside ATProto.

ATProto records natively use CIDs (the at-uri `at://<did>/<collection>/<rkey>`
typically uses a CID as the rkey for content-addressed records). We align.

## Decision

**Use IPLD CIDv1 with codec `dag-cbor` (0x71) and hash function `sha2-256`
(0x12), encoded as `base32` lowercase (per the `multibase` `b` prefix).**

The canonicalization pipeline is:

1. Build the unsigned claim as a JSON object with fields ordered per the
   Lexicon (deterministic Lexicon-field order; NOT alphabetical — see
   "Alternatives Considered" below).
2. Serialize to canonical CBOR per [RFC 8949 §4.2.1](https://www.rfc-editor.org/rfc/rfc8949.html#name-core-deterministic-encoding)
   (Core Deterministic Encoding):
   - Shortest-form integer encoding.
   - Length-first lexicographic key sorting for maps (RFC 8949 §4.2.1 says
     lexicographic; for OpenLore we use this canonical CBOR rule, NOT a
     custom Lexicon order — the Lexicon order is for the JSON-rendered
     compose preview only).
   - No indefinite-length items.
3. Compute `sha2-256` over the canonical CBOR bytes; this is the
   `unsigned_claim_cid`.
4. Sign the `unsigned_claim_cid` (NOT the bytes; signing the CID lets verifiers
   reconstruct without re-canonicalizing).
5. The signed payload is `{ ...unsigned fields, signature: { kid, alg, sig } }`.
6. The final `claim_cid` is computed over the canonical CBOR of the SIGNED
   payload (so the CID is stable for a given signature, not just for a given
   set of unsigned fields).

Encoded form: `bafyrei...` (base32-lower, `b` multibase prefix, `1` CIDv1,
`71` dag-cbor codec, `12` sha2-256 hash, `20` hash length, then digest).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Canonical JSON (RFC 8785 JCS)** + `sha2-256` | Considered; JSON canonicalization is more brittle than CBOR (handling of numbers, Unicode escapes, etc.). CBOR is the IPLD/ATProto convention. |
| **`dag-json` codec instead of `dag-cbor`** | dag-json is a CBOR-equivalent JSON serialization; functionally equivalent for our needs but slightly larger on disk and less common in the ATProto ecosystem. |
| **`sha3-256` or `blake3` hash** | Stronger/faster but less ecosystem support; the entire ATProto/IPLD stack assumes `sha2-256` by default. Premature optimization. |
| **Base58btc encoding** (IPFS classic) | base32-lower is the CIDv1 default and is case-insensitive (filesystem-safe on case-insensitive macOS HFS+/APFS variants); base58 is case-sensitive. |
| **Lexicon-field-order serialization** (instead of canonical CBOR key sort) | Considered for human readability of the canonicalization rule. Rejected: a CID specification that says "sort keys per the Lexicon order" requires every verifier to ship the Lexicon and to know which version was used. Canonical CBOR is a one-line rule any IPLD library implements correctly. |

## Consequences

### Positive

- CID is byte-identical regardless of who computed it, as long as they use any
  RFC 8949-compliant canonical CBOR encoder. Multi-language interop is free.
- The signed payload's CID changes if and only if the signed bytes change —
  exactly the round-trip identity guarantee KPI-4 demands.
- IPLD ecosystem compatibility (`ipfs dag get bafy...` would resolve from any
  IPFS node hosting the bytes, if we later choose to host claims on IPFS).

### Negative

- CBOR is binary; not human-readable on disk. **Mitigation**: the canonical
  local file at `~/.local/share/openlore/claims/<cid>.json` is the
  **rendered JSON** of the signed claim (per US-002 Example 1, the file is
  ~412 bytes JSON). The CID is computed from the CBOR canonicalization of the
  same logical payload; the on-disk file is JSON for grep-ability. **Both
  representations must serialize to the same canonical CBOR for the CID to
  match** — the canonicalization function takes the parsed JSON, normalizes
  to canonical CBOR, hashes.
- A serde bug in any of the Rust CBOR crates (`ciborium`, `serde_cbor`) that
  drifts from RFC 8949 strictness would break CID stability silently.
  **Mitigation**: see Earned Trust below.

### Earned Trust

The `claim-domain::canonicalization` module MUST expose property tests AND a
`probe()`:

1. **Property test** (proptest): for any randomly generated claim, encoding to
   canonical CBOR, decoding, re-encoding, MUST yield byte-identical CBOR. Run
   in CI on every commit.
2. **Cross-implementation probe**: ship a fixture suite of JSON claims and
   their expected CIDs (computed once, frozen in the repo). On every release,
   compute CIDs for every fixture; assert byte-equality. This catches drift
   in the Rust CBOR library across versions.
3. **Mutation testing**: `cargo-mutants` MUST run against canonicalization with
   ≥95% mutation kill rate. Canonicalization is the single most load-bearing
   pure function in the system.

## Revisit Trigger

- A demonstrated need to host claims outside ATProto (IPFS, content-addressed
  blob storage) — the codec/hash choices already cover this, no change needed.
- A cryptographic break in sha2-256 (unlikely; ATProto would migrate first).
