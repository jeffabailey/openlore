# ADR-002: Identity = Existing ATProto DID + Per-App Derived Key

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect), per WD-12 lock from Luna (nw-product-owner)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

slice-01 needs a signing identity for every claim. The signed payload must be:

- **Verifiable by any federated peer** without a side-channel — just from the
  user's DID document.
- **Portable across the user's devices** without forcing per-device DID minting.
- **Independently revocable from the user's main ATProto identity** so that an
  OpenLore key compromise does not nuke the user's social-graph identity.

Locked input: WD-12 in feature-delta.md fixes the identity model as "reuse the
user's existing ATProto DID with a per-application derived key." See
`docs/feature/openlore-foundation/discuss/alternatives-considered.md` Choice 2
for the full alternatives analysis.

## Decision

**Reuse the user's existing ATProto DID; sign claims with a per-application
derived key that lives in a separate verification method on the user's DID
document.**

- The CLI never mints a fresh `did:plc` or `did:web`.
- At `openlore init`, the CLI:
  1. Resolves the user's ATProto identity from a handle + app-password (or an
     existing session token).
  2. Generates a per-application Ed25519 keypair locally.
  3. Adds the public key to the user's DID document as an additional
     verification method tagged `org.openlore.application` (subject to the
     user's confirmation; ATProto's `com.atproto.identity.updateHandle` /
     equivalent verification-method update flow).
  4. Stores the private key in the OS keychain (macOS Keychain Services; Linux
     Secret Service via libsecret; WSL2 falls back to a file with `0600`
     permissions and a clear warning at init time).
- All `org.openlore.claim` records are signed with this derived key.
- The signature block in the signed payload carries:
  `{ kid: "<did>#org.openlore.application", alg: "EdDSA", sig: "..." }`.
  Peers verify by resolving the DID, finding the matching verification method,
  and validating the Ed25519 signature.
- Revocation = remove the verification method from the DID document. Existing
  signed claims remain on the user's PDS (per WD-11) but no longer verify
  against the live DID document; this is the intended semantics of an
  "OpenLore-key revocation."

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Mint a fresh `did:plc` per OpenLore install** | Doubles the recovery surface; peers cannot trivially correlate `did:plc:openlore-jeff-laptop-7a3f` with the same person they follow on Bluesky. Loses the "social-graph identity reuse" benefit without compensating gain. Full reasoning in alternatives-considered.md Choice 2. |
| **Sign claims with the user's primary ATProto signing key directly** | Conflates revocability: compromise of the OpenLore key would force the user to rotate their main ATProto identity. Unacceptable blast radius for an experimental application. |
| **Use a hardware key (YubiKey, etc.)** | Worth considering long-term but excluded from slice-01 (walking skeleton scope). Adds setup friction P-001 will tolerate but a broader audience would not. Re-open post-slice-05 if the user base extends past senior engineers. |

## Consequences

### Positive

- Peers reading a claim signed by `did:plc:jeff#org.openlore.application` can
  resolve `did:plc:jeff` and see this is the same person whose Bluesky posts
  they read.
- Independent revocability: the OpenLore key can be rotated without touching
  the main ATProto identity.
- Per-device keys are possible (each install adds a verification method tagged
  `org.openlore.application.<device>` if needed in a later slice). Slice-01
  ships with one device only.

### Negative

- The user must permit OpenLore to modify their DID document at init.
  **Mitigation**: clear consent prompt at init, explaining exactly what
  verification method is being added and how to revoke it later.
- Key storage on Linux requires Secret Service to be running. **Mitigation**:
  fall back to encrypted file at `~/.config/openlore/keys/` with a strong
  warning if Secret Service is unreachable.
- Some ATProto PDS implementations may not yet support arbitrary verification
  methods on user-controlled DID documents. **Mitigation**: probe at init
  (see Earned Trust below); refuse to start if the PDS rejects the
  verification-method add.

### Earned Trust (per principle 12)

The `atproto-did-adapter` MUST expose a `probe()` method that, at startup AND
at `openlore init` confirmation time, exercises:

1. Resolve the user's DID document; assert the OpenLore verification method is
   present with the expected key id.
2. Sign a sentinel payload with the local private key; verify the signature
   against the public key resolved from the DID document. If they disagree, the
   keychain has been tampered with or the DID document has drifted.
3. Verify keychain accessibility: write a sentinel secret, read it back, delete
   it. Refuses to start if the keychain backend is broken (e.g., Secret Service
   crashed; Keychain locked at boot).
4. On WSL2 fallback to file storage: assert `0600` perms on the key file and
   warn loudly. Refuses to start with `health.startup.refused{reason: identity.key_perms_unsafe}` if permissions are anything else.

## Revisit Trigger

- Regulatory environment that makes signing application data with the user's
  primary DID a liability.
- An ATProto convention emerging for per-app DIDs that gives discoverability
  the bare DID currently lacks.
- A cryptographic weakness discovered in the derivation scheme.
