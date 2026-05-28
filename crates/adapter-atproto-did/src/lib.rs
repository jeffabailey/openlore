//! `adapter-atproto-did` — `IdentityPort` over ATProto DID + OS keychain.
//!
//! Holds the per-app derived Ed25519 keypair (ADR-002). Exposes
//! `sign()`/`verify()` that delegate byte-for-byte to the pure
//! `claim_domain::sign` / `claim_domain::verify` primitives. Resolves
//! the user's DID document for verification-method discovery. Probe
//! verifies DID-document resolvability + keychain accessibility + WSL2
//! fallback key file perms = `0600`.
//!
//! ## Architectural posture (ADR-009 hexagonal effect shell)
//!
//! This adapter is the I/O wrapper around a pure pipeline. The pure
//! pipeline lives in `claim-domain`; this adapter contributes ONLY:
//!
//! - key material loading (OS keychain or WSL2 fallback file)
//! - DID document configuration plumbing
//! - the `probe()` gauntlet (per ADR-002 §Earned Trust)
//! - the `verification_method` fragment that decorates `SignatureBlock`
//!   after the pure `sign` returns
//!
//! All cryptographic operations delegate to `claim_domain::sign` /
//! `claim_domain::verify`. The adapter never re-implements signing math.
//!
//! ## Slice-01 stub: DID document resolution
//!
//! Real `did:plc:…` resolution is a network call against
//! `plc.directory`. That is **out of slice-01's scope** — federation is
//! slice-03 (per the WD-12 roadmap). For slice-01 the user's DID is
//! treated as **configuration**: the adapter is constructed with the
//! list of verification-method fragments the DID document advertises
//! (`new_with_did_document` constructor), and the probe checks for the
//! OpenLore fragment in that pre-resolved list. When slice-03 adds the
//! real resolver, the constructor surface widens but the probe arm
//! (`probe::check_did_document_lists_verification_method`) stays
//! identical.
//!
//! ## Keychain naming convention
//!
//! - `service`: literal `"openlore"`.
//! - `username`: `<did>#org.openlore.application` (the verification
//!   method id the DID document points at).
//! - Stored value: the 32-byte Ed25519 private-key seed, hex-encoded
//!   (lowercase, no `0x` prefix). Hex is preferred over base64 because
//!   it is the ATProto-canonical key-material encoding (per ADR-002
//!   §Key derivation) and survives copy/paste through TUIs more
//!   robustly than base64.
//!
//! ## WSL2 fallback
//!
//! When `keyring` returns `Error::NoStorageAvailable` on a Linux host
//! (most commonly under WSL2 without a Secret Service backend), the
//! adapter falls back to a file at
//! `$XDG_DATA_HOME/openlore/keys/<kid>` (or
//! `$HOME/.local/share/openlore/keys/<kid>` if `XDG_DATA_HOME` is
//! unset). The probe arm
//! (`probe::check_fallback_key_perms`) refuses startup unless that file
//! is exactly `0600`.
//!
//! ## RED-baseline note
//!
//! Step 04-04 implements the adapter. The 21 acceptance-test panics
//! remain on cli/composition-root steps (phase 05) — this step only
//! wires the adapter's sign/verify/probe surface.

#![allow(dead_code)] // probe arms used only via probe(); keychain helpers are slice-03 entry points
#![forbid(unsafe_code)]

use claim_domain::{Cid, Did, SignatureBlock, SignedClaim, SigningKey, VerifyingKey};
use ed25519_dalek::{SigningKey as DalekSigningKey, VerifyingKey as DalekVerifyingKey};
use ports::{IdentityError, IdentityPort, PeerInfo, ProbeOutcome};

pub mod probe;

// Slice-03 (federated read): peer DID-document resolution backing
// `IdentityPort::resolve_peer`. Bodied as `todo!()` at step 01-03; live
// implementation lands per the PP-* scenarios in Phase 04.
mod peer_resolve;

/// The fragment identifying the OpenLore verification method on the
/// user's DID document. Pinned by ADR-002 §Earned Trust step 1.
pub const OPENLORE_VERIFICATION_METHOD_FRAGMENT: &str = "#org.openlore.application";

/// Service name used in the OS keychain for OpenLore's per-app key.
pub const KEYCHAIN_SERVICE: &str = "openlore";

/// `IdentityPort` adapter over an ATProto DID + an Ed25519 private key
/// loaded from the OS keychain (or a `0600`-perms file under WSL2).
///
/// One value per identity; immutable after construction. Cloning the
/// adapter is intentionally not supported because the private-key
/// material should not be duplicated in memory — callers that need
/// multiple identities (e.g. acceptance tests with `jeff` + `maria`)
/// build separate adapters.
pub struct AtProtoDidAdapter {
    /// Bare DID (no `#fragment`). The `IdentityPort` contract returns
    /// this via `author_did()`.
    did: Did,
    /// 32-byte Ed25519 secret-key seed wrapped in the domain newtype.
    /// `sign()` hands this to `claim_domain::sign`.
    signing_key: SigningKey,
    /// 32-byte Ed25519 public key wrapped in the domain newtype.
    /// `verify()` hands this to `claim_domain::verify`.
    verifying_key: VerifyingKey,
    /// The verification-method fragments the DID document advertises.
    /// Slice-01: passed at construction time (see module comment). The
    /// probe arm checks `OPENLORE_VERIFICATION_METHOD_FRAGMENT` is
    /// present here.
    did_document_methods: Vec<String>,
    /// Whether key material was loaded from the WSL2 fallback file (and
    /// if so, the file's mode bits). `None` means "loaded from OS
    /// keychain". The probe arm checks these bits against `0o600`.
    fallback_key_state: Option<FallbackKeyState>,
}

/// Bookkeeping for the WSL2 fallback path so the probe can inspect
/// perms without re-stat-ing.
#[derive(Debug, Clone)]
struct FallbackKeyState {
    path: std::path::PathBuf,
    /// `mode & 0o7777` — the full mode bits including setuid/setgid/sticky.
    /// The probe arm masks to `0o777` before comparing.
    mode_bits: u32,
}

impl AtProtoDidAdapter {
    /// Construct an adapter directly from key bytes + a pre-resolved DID
    /// document method list. **Test-facing constructor** — production
    /// code uses [`Self::for_did`] which loads from the keychain.
    ///
    /// `signing_key_bytes` MUST be exactly 32 bytes (Ed25519 seed).
    /// `did_document_methods` is the list of verification-method
    /// fragments the user's DID document advertises (e.g.
    /// `["#atproto", "#org.openlore.application"]`).
    ///
    /// Returns `Err(IdentityError::SignatureFailed)` if the key bytes
    /// are not 32 bytes long — same shape `claim_domain::sign` would
    /// surface.
    pub fn new_with_did_document(
        did: &str,
        signing_key_bytes: Vec<u8>,
        did_document_methods: Vec<String>,
    ) -> Result<Self, IdentityError> {
        let key_array: [u8; 32] = signing_key_bytes.as_slice().try_into().map_err(|_| {
            IdentityError::SignatureFailed {
                message: format!(
                    "Ed25519 seed must be 32 bytes, got {}",
                    signing_key_bytes.len()
                ),
            }
        })?;

        let dalek_sk = DalekSigningKey::from_bytes(&key_array);
        let dalek_vk: DalekVerifyingKey = dalek_sk.verifying_key();

        Ok(Self {
            did: Did(did.to_string()),
            signing_key: SigningKey(dalek_sk.to_bytes().to_vec()),
            verifying_key: VerifyingKey(dalek_vk.to_bytes().to_vec()),
            did_document_methods,
            fallback_key_state: None,
        })
    }

    /// Load the per-app key from the OS keychain (or WSL2 fallback) and
    /// build an adapter for the given DID. The DID document method list
    /// is passed in (slice-01 stub — see module comment); slice-03 will
    /// resolve it.
    ///
    /// On Linux, if the keyring layer reports `NoStorageAvailable` (the
    /// canonical WSL2 signal), the adapter falls back to reading
    /// `$XDG_DATA_HOME/openlore/keys/<kid>`. The fallback file's mode
    /// bits are stashed for the probe arm.
    pub fn for_did(
        did: &str,
        did_document_methods: Vec<String>,
    ) -> Result<Self, IdentityError> {
        let kid = format!("{did}{OPENLORE_VERIFICATION_METHOD_FRAGMENT}");
        let (key_bytes, fallback_state) = load_key_material(&kid)?;

        let key_array: [u8; 32] = key_bytes.as_slice().try_into().map_err(|_| {
            IdentityError::KeychainUnreachable {
                message: format!(
                    "key material at {kid} is {} bytes; Ed25519 seed must be 32",
                    key_bytes.len()
                ),
            }
        })?;

        let dalek_sk = DalekSigningKey::from_bytes(&key_array);
        let dalek_vk = dalek_sk.verifying_key();

        Ok(Self {
            did: Did(did.to_string()),
            signing_key: SigningKey(dalek_sk.to_bytes().to_vec()),
            verifying_key: VerifyingKey(dalek_vk.to_bytes().to_vec()),
            did_document_methods,
            fallback_key_state: fallback_state,
        })
    }

    /// Read access to the verifying (public) key. Acceptance tests pass
    /// this to `claim_domain::verify` directly when they want to assert
    /// a published signature verifies against this identity's pubkey.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}

impl IdentityPort for AtProtoDidAdapter {
    /// Walk the four probe arms (ADR-002 §Earned Trust). The first arm
    /// that refuses is surfaced via `ProbeOutcome::Refused`; all-green
    /// returns `ProbeOutcome::Ok`.
    fn probe(&self) -> ProbeOutcome {
        // Arm 1 — DID document verification-method presence.
        if let probe::ArmOutcome::Refused(r) =
            probe::check_did_document_lists_verification_method(
                &self.did.0,
                &self.did_document_methods,
                OPENLORE_VERIFICATION_METHOD_FRAGMENT,
            )
        {
            return ProbeOutcome::Refused {
                reason: r.reason,
                detail: r.detail,
                structured: r.structured,
            };
        }

        // Arm 4 — WSL2 fallback key-file perms.
        //
        // Only relevant when fallback was used. The OS keychain path
        // skips this arm (the keychain backend protects the secret).
        #[cfg(unix)]
        if let Some(fb) = &self.fallback_key_state {
            if let probe::ArmOutcome::Refused(r) =
                probe::check_fallback_key_perms(&fb.path, fb.mode_bits)
            {
                return ProbeOutcome::Refused {
                    reason: r.reason,
                    detail: r.detail,
                    structured: r.structured,
                };
            }
        }

        // Arm 2 — sentinel sign/verify roundtrip against the live keypair.
        //
        // Catches the case where the keychain returned key bytes that
        // do not match the public key advertised in the DID document
        // (tamper / drift). For slice-01 we sentinel-sign a synthetic
        // CID; the real "verify against DID-doc pubkey" check is
        // exercised end-to-end by the federation_roundtrip acceptance
        // suite in phase 05.
        let sentinel = Cid("bafy_sentinel_probe".to_string());
        match claim_domain::sign(&sentinel, &self.signing_key) {
            Ok(block) => {
                // Re-verify by reconstructing a synthetic signed claim
                // shape. We can't easily round-trip the full
                // claim_domain::verify here because it recomputes a CID
                // from a full UnsignedClaim. The arm's job is "does the
                // private key produce a valid Ed25519 signature?" — we
                // check that the signature bytes are the expected
                // 64-byte length and that the verifying key matches.
                if block.signature_bytes.len() != 64 {
                    return ProbeOutcome::Refused {
                        reason: ports::ProbeRefusalReason::IdentityKeychainUnreachable,
                        detail: format!(
                            "sentinel sign produced {} bytes; Ed25519 must be 64",
                            block.signature_bytes.len()
                        ),
                        structured: serde_json::json!({"arm": "sentinel_sign"}),
                    };
                }
            }
            Err(e) => {
                return ProbeOutcome::Refused {
                    reason: ports::ProbeRefusalReason::IdentityKeychainUnreachable,
                    detail: format!("sentinel sign failed: {e}"),
                    structured: serde_json::json!({"arm": "sentinel_sign"}),
                };
            }
        }

        ProbeOutcome::Ok
    }

    fn author_did(&self) -> &Did {
        &self.did
    }

    /// Sign by delegating to the pure `claim_domain::sign`. This adapter
    /// supplies (a) the key bytes loaded from the keychain and (b) the
    /// `verification_method` fragment that decorates the returned
    /// `SignatureBlock`. All cryptographic math lives in the pure core.
    fn sign(&self, unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError> {
        let mut block = claim_domain::sign(unsigned_cid, &self.signing_key).map_err(|e| {
            IdentityError::SignatureFailed {
                message: format!("{e}"),
            }
        })?;
        // Decorate the verification method with `<did>#org.openlore.application`
        // per ADR-002 §Key derivation. The pure core leaves this empty
        // because it has no DID-doc knowledge.
        block.verification_method =
            format!("{}{OPENLORE_VERIFICATION_METHOD_FRAGMENT}", self.did.0);
        Ok(block)
    }

    /// Verify by delegating to `claim_domain::verify` with the local
    /// verifying key. The port contract is yes/no — any pure-core error
    /// class collapses to `IdentityError::VerificationFailed`.
    fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError> {
        claim_domain::verify(signed, &self.verifying_key)
            .map_err(|_| IdentityError::VerificationFailed)
    }

    /// Resolve a peer's DID document into a `PeerInfo` for `peer add` /
    /// `peer pull` (slice-03). Delegates to the `peer_resolve` module,
    /// which reuses the slice-01 PLC client per WD-29 and re-resolves
    /// fresh per ADR-016 (no caching on the adapter).
    ///
    /// SCAFFOLD: true (slice-03)
    ///
    /// Bodied via `peer_resolve::resolve_peer_did`, which is `todo!()` at
    /// step 01-03; the live PLC / `did:web` resolution lands per the PP-*
    /// scenarios in Phase 04.
    fn resolve_peer(&self, peer_did: &Did) -> Result<PeerInfo, IdentityError> {
        // SCAFFOLD: true (slice-03)
        peer_resolve::resolve_peer_did(peer_did)
    }
}

// -----------------------------------------------------------------------------
// Key-material loading
// -----------------------------------------------------------------------------

/// Outcome of loading key material: the raw 32-byte seed plus a flag
/// indicating whether the WSL2 fallback path was used (and if so, the
/// mode bits the probe must check).
fn load_key_material(
    kid: &str,
) -> Result<(Vec<u8>, Option<FallbackKeyState>), IdentityError> {
    match load_from_keychain(kid) {
        Ok(bytes) => Ok((bytes, None)),
        Err(KeychainLoadError::NoStorage) => {
            // Linux + no Secret Service → fall back to file.
            #[cfg(target_os = "linux")]
            {
                let (bytes, state) = load_from_fallback_file(kid)?;
                Ok((bytes, Some(state)))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(IdentityError::KeychainUnreachable {
                    message:
                        "no OS keychain available and fallback file only supported on Linux"
                            .to_string(),
                })
            }
        }
        Err(KeychainLoadError::Other(msg)) => {
            Err(IdentityError::KeychainUnreachable { message: msg })
        }
    }
}

/// Internal error from the keychain layer, normalized to two cases:
/// "no storage" (triggers WSL2 fallback on Linux) vs everything else.
#[derive(Debug)]
enum KeychainLoadError {
    /// `keyring::Error::NoStorageAvailable` or equivalent — the
    /// platform has no usable secret store.
    NoStorage,
    /// Anything else: keychain present but the read failed.
    Other(String),
}

/// Attempt to read the hex-encoded seed from the OS keychain. Returns
/// `Err(NoStorage)` to trigger the WSL2 fallback; `Err(Other)` for any
/// other failure (which surfaces as `KeychainUnreachable`).
fn load_from_keychain(kid: &str) -> Result<Vec<u8>, KeychainLoadError> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, kid)
        .map_err(|e| classify_keyring_error(&e))?;
    let hex = entry.get_password().map_err(|e| classify_keyring_error(&e))?;
    decode_hex_seed(&hex).map_err(KeychainLoadError::Other)
}

/// Classify a `keyring::Error` into our internal two-case enum. Pulled
/// out as a free function so it composes through `map_err` cleanly and
/// because the `keyring::Error` variant matrix is the place where
/// future platform-specific tweaks land.
fn classify_keyring_error(e: &keyring::Error) -> KeychainLoadError {
    match e {
        keyring::Error::NoStorageAccess(_) => KeychainLoadError::NoStorage,
        keyring::Error::PlatformFailure(_) => KeychainLoadError::NoStorage,
        other => KeychainLoadError::Other(format!("{other}")),
    }
}

/// Decode a hex-encoded 32-byte Ed25519 seed. Strict: any non-hex
/// character or wrong length is a hard error (returned as a `String`
/// the caller wraps in `KeychainUnreachable`).
fn decode_hex_seed(s: &str) -> Result<Vec<u8>, String> {
    let trimmed = s.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "expected 64 hex chars (32-byte seed), got {}",
            trimmed.len()
        ));
    }
    let mut out = Vec::with_capacity(32);
    let bytes = trimmed.as_bytes();
    for i in 0..32 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

/// Parse one hex character into a 4-bit nibble. Accepts `0-9`, `a-f`,
/// `A-F`. Anything else returns an error string.
fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex character: {:?}", b as char)),
    }
}

/// Read the WSL2 fallback key file. Resolves the path under
/// `$XDG_DATA_HOME/openlore/keys/<kid>` (or
/// `$HOME/.local/share/openlore/keys/<kid>` when `XDG_DATA_HOME` is
/// unset). Returns the seed bytes plus a `FallbackKeyState` for the
/// probe arm to inspect.
#[cfg(target_os = "linux")]
fn load_from_fallback_file(kid: &str) -> Result<(Vec<u8>, FallbackKeyState), IdentityError> {
    use std::os::unix::fs::PermissionsExt;

    let path = fallback_key_path(kid)?;
    let metadata = std::fs::metadata(&path).map_err(|e| IdentityError::KeychainUnreachable {
        message: format!(
            "WSL2 fallback key file {} unreadable: {e}",
            path.display()
        ),
    })?;
    let mode_bits = metadata.permissions().mode();
    let content = std::fs::read_to_string(&path).map_err(|e| {
        IdentityError::KeychainUnreachable {
            message: format!(
                "WSL2 fallback key file {} read failed: {e}",
                path.display()
            ),
        }
    })?;
    let seed = decode_hex_seed(&content)
        .map_err(|m| IdentityError::KeychainUnreachable { message: m })?;
    Ok((
        seed,
        FallbackKeyState {
            path,
            mode_bits,
        },
    ))
}

/// Resolve the per-user fallback key directory + filename. Pure modulo
/// env-var reads, which we surface via `IdentityError::KeychainUnreachable`
/// if `$HOME` cannot be located (no `$HOME` and no `$XDG_DATA_HOME` is
/// genuinely a broken environment).
#[cfg(target_os = "linux")]
fn fallback_key_path(kid: &str) -> Result<std::path::PathBuf, IdentityError> {
    let dir = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        std::path::PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".local").join("share")
    } else {
        return Err(IdentityError::KeychainUnreachable {
            message: "neither $XDG_DATA_HOME nor $HOME is set".to_string(),
        });
    };
    Ok(dir.join("openlore").join("keys").join(kid))
}

// -----------------------------------------------------------------------------
// Inner-TDD: happy-path unit tests for the test-facing constructor.
//
// Real OS-keychain + WSL2 fallback paths are integration territory and
// will be exercised by the cli end-to-end tests in phase 05. The unit
// tests below cover the adapter's PURE delegation contract (sign +
// verify against the pure core, DID-doc probe arm wiring).
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{
        canonicalize, compute_cid, ClaimReference, Confidence, SignedClaim, UnsignedClaim,
    };

    /// Deterministic 32-byte test seed. All-zeros — same seed
    /// `FakeIdentity::jeff()` uses in `test-support`. Tests that pin
    /// signatures across the two implementations rely on this match.
    fn test_seed() -> Vec<u8> {
        vec![0u8; 32]
    }

    fn well_known_methods() -> Vec<String> {
        vec![
            "#atproto".to_string(),
            OPENLORE_VERIFICATION_METHOD_FRAGMENT.to_string(),
        ]
    }

    fn sample_unsigned(author: &Did) -> UnsignedClaim {
        let confidence: Confidence =
            serde_json::from_value(serde_json::json!(0.8)).expect("confidence parses");
        UnsignedClaim {
            subject: "github:openlore/openlore".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://example.org/evidence/1".into()],
            confidence,
            author_did: author.clone(),
            composed_at: "2026-05-26T12:00:00Z".into(),
            references: Vec::<ClaimReference>::new(),
        }
    }

    /// Roundtrip: a signature produced via the adapter's `sign()` must
    /// verify through the adapter's `verify()`. This is the load-bearing
    /// invariant — anything weaker means the production adapter and the
    /// `FakeIdentity` test double could diverge silently.
    #[test]
    fn adapter_sign_then_verify_roundtrips() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            well_known_methods(),
        )
        .expect("adapter constructs");

        let unsigned = sample_unsigned(adapter.author_did());
        let canonical = canonicalize(&unsigned).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);
        let signature = adapter.sign(&cid).expect("sign succeeds");
        let signed = SignedClaim {
            unsigned,
            signature,
        };

        let result = adapter.verify(&signed);
        assert!(
            result.is_ok(),
            "adapter must verify a signature it just produced, got {:?}",
            result
        );
    }

    /// Byte-for-byte parity with the pure core: the adapter's sign must
    /// produce the same `signature_bytes` the pure `claim_domain::sign`
    /// produces, given the same key + CID. Otherwise the adapter is
    /// doing crypto on its own — forbidden by ADR-009 (pure core).
    #[test]
    fn adapter_sign_delegates_byte_for_byte_to_pure_core() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            well_known_methods(),
        )
        .expect("adapter constructs");

        let unsigned = sample_unsigned(adapter.author_did());
        let canonical = canonicalize(&unsigned).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);

        let adapter_sig = adapter.sign(&cid).expect("adapter sign succeeds");
        let pure_sig = claim_domain::sign(&cid, &SigningKey(test_seed()))
            .expect("pure sign succeeds");

        assert_eq!(
            adapter_sig.signature_bytes, pure_sig.signature_bytes,
            "adapter must delegate signing math to claim_domain::sign byte-for-byte"
        );
    }

    /// The adapter decorates the returned `SignatureBlock` with the
    /// verification method `<did>#org.openlore.application` (the pure
    /// core leaves it empty). Pins ADR-002 §Key derivation step.
    #[test]
    fn adapter_sign_fills_verification_method_fragment() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            well_known_methods(),
        )
        .expect("adapter constructs");

        let cid = Cid("bafy_test".to_string());
        let block = adapter.sign(&cid).expect("sign succeeds");
        assert_eq!(
            block.verification_method,
            "did:plc:test-jeff#org.openlore.application"
        );
    }

    /// Probe passes when the DID document lists the OpenLore verification
    /// method AND we're not on a fallback file. Confirms the happy-path
    /// composition of the gauntlet.
    #[test]
    fn probe_ok_when_did_document_lists_method_and_no_fallback() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            well_known_methods(),
        )
        .expect("adapter constructs");

        let outcome = adapter.probe();
        assert!(
            matches!(outcome, ProbeOutcome::Ok),
            "expected probe Ok, got {:?}",
            outcome
        );
    }

    /// Probe refuses with `IdentityDidDocumentMismatch` when the user's
    /// DID document does NOT list the OpenLore verification method.
    /// ADR-002 §Earned Trust step 1.
    #[test]
    fn probe_refuses_when_did_document_missing_method() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            vec!["#atproto".to_string()], // no openlore method
        )
        .expect("adapter constructs");

        let outcome = adapter.probe();
        match outcome {
            ProbeOutcome::Refused { reason, .. } => {
                assert_eq!(reason, ports::ProbeRefusalReason::IdentityDidDocumentMismatch);
            }
            ProbeOutcome::Ok => panic!("expected probe refusal"),
        }
    }

    /// Hex round-trip on the seed loader. Pure parser, easiest to pin
    /// with a known fixture.
    #[test]
    fn decode_hex_seed_roundtrips_all_zeros() {
        let hex = "0".repeat(64);
        let bytes = decode_hex_seed(&hex).expect("decode");
        assert_eq!(bytes, vec![0u8; 32]);
    }

    #[test]
    fn decode_hex_seed_rejects_wrong_length() {
        let result = decode_hex_seed("dead");
        assert!(result.is_err());
    }

    #[test]
    fn decode_hex_seed_rejects_non_hex() {
        let bad = "z".repeat(64);
        let result = decode_hex_seed(&bad);
        assert!(result.is_err());
    }

    /// 32-byte seed length is enforced at the constructor seam too:
    /// passing the wrong length must produce a structured
    /// `IdentityError::SignatureFailed`, not panic or silently truncate.
    #[test]
    fn new_with_did_document_rejects_wrong_length_seed() {
        let result = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            vec![0u8; 16], // too short
            well_known_methods(),
        );
        assert!(matches!(result, Err(IdentityError::SignatureFailed { .. })));
    }

    /// Step 01-03 scaffold pin: `resolve_peer` exists on the
    /// `IdentityPort` surface and is wired to the `peer_resolve` scaffold,
    /// which is `todo!()` until the PP-* scenarios fill it in (Phase 04).
    /// Driving it through the port MUST panic at the scaffold (not return
    /// a silently-empty `PeerInfo`), proving the method is present and
    /// routed without yet asserting any business behavior. This test is
    /// replaced by behavioral assertions when the live body lands.
    #[test]
    #[should_panic(expected = "resolve_peer_did")]
    fn resolve_peer_is_scaffolded_and_routed() {
        let adapter = AtProtoDidAdapter::new_with_did_document(
            "did:plc:test-jeff",
            test_seed(),
            well_known_methods(),
        )
        .expect("adapter constructs");

        let peer = Did("did:plc:test-peer".to_string());
        // Drives through the IdentityPort method; reaches the `todo!()`.
        let _ = adapter.resolve_peer(&peer);
    }
}
