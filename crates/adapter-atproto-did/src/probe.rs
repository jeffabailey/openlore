//! `probe` — Earned-Trust startup gauntlet for the ATProto DID adapter.
//!
//! Implements the four checks ADR-002 §"Earned Trust" requires of the
//! identity adapter:
//!
//! 1. **DID document resolution + verification-method discovery.** The
//!    user's DID document must list the OpenLore verification method
//!    (`#org.openlore.application`). Slice-01 stubs the resolver: the
//!    DID document is treated as configuration handed to the adapter at
//!    construction (see `lib.rs` module comment) — real `did:plc`
//!    network lookup is deferred to slice-03 federation work.
//! 2. **Sentinel sign/verify.** Round-trip a small fixed payload through
//!    the local private key + the public key from the DID document. If
//!    they disagree, the keychain has been tampered with or the DID
//!    document has drifted.
//! 3. **Keychain accessibility.** Write a sentinel secret, read it back,
//!    delete it. Refuses to start if the keychain backend is broken.
//! 4. **WSL2 fallback perms.** When the OS keychain is unavailable on
//!    Linux and the adapter falls back to a file at
//!    `$XDG_DATA_HOME/openlore/keys/<kid>`, that file must be `0600`.
//!    Any other perms refuse startup.
//!
//! Each arm is a pure function over a small input shape so the gauntlet
//! composes cleanly (`nw-fp-domain-modeling §8` railway). The arm
//! signatures return `Result<(), ProbeRefusalReason>` and the caller
//! lifts them into `ProbeOutcome::Refused { reason, detail, structured }`
//! at the public `probe()` boundary.
//!
//! ## Scope
//!
//! This module owns the **logic** of each arm. The OS-level effects
//! (calling into `keyring`, stat-ing files) live in `lib.rs` so the arms
//! here stay testable from unit tests without touching real keychains.

use serde_json::json;

use ports::ProbeRefusalReason;

/// Outcome of one arm of the probe. Aggregated by `lib.rs::probe()`
/// into a `ports::ProbeOutcome`.
///
/// `Ok` means this arm passed; `Refused(refusal)` means the arm tripped
/// and the adapter must refuse startup, surfacing `refusal.reason` to
/// the tracing layer.
#[derive(Debug, Clone)]
pub enum ArmOutcome {
    Ok,
    Refused(ProbeRefusal),
}

/// The structured refusal an arm emits when it trips. Mirrors the
/// public `ProbeOutcome::Refused` shape so the lift at `lib.rs::probe()`
/// is a 1:1 field copy.
#[derive(Debug, Clone)]
pub struct ProbeRefusal {
    pub reason: ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

// -----------------------------------------------------------------------------
// Arm 1 — DID document verification-method presence
// -----------------------------------------------------------------------------

/// Inspect a (stubbed-for-slice-01) DID document for the OpenLore
/// verification method.
///
/// The DID document is modelled as the list of verification-method
/// fragments the document advertises (e.g.
/// `["#org.openlore.application", "#atproto"]`). The arm passes iff the
/// configured `expected_fragment` is present.
///
/// Why a stub: real `did:plc:…` resolution requires a network round-trip
/// to `plc.directory`, which is out of slice-01's federation scope. The
/// adapter accepts the DID document as constructor input and pins this
/// arm's logic to "is the expected fragment in the list?". When slice-03
/// adds the real resolver, this arm's signature stays the same — only
/// the input source changes.
pub fn check_did_document_lists_verification_method(
    did: &str,
    verification_methods: &[String],
    expected_fragment: &str,
) -> ArmOutcome {
    if verification_methods
        .iter()
        .any(|m| m == expected_fragment)
    {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::IdentityDidDocumentMismatch,
        detail: format!(
            "DID {did} document does not list OpenLore verification method \
             {expected_fragment}; got {verification_methods:?}"
        ),
        structured: json!({
            "did": did,
            "expected_fragment": expected_fragment,
            "got_methods": verification_methods,
        }),
    })
}

// -----------------------------------------------------------------------------
// Arm 3 — Keychain accessibility (lift, not action)
// -----------------------------------------------------------------------------
//
// Arm 2 (sentinel sign/verify) is implemented inline in
// `lib.rs::probe()` because it needs the live `SigningKey` /
// `VerifyingKey` already loaded by the adapter — pulling the dalek
// objects into a pure submodule would re-introduce the dep coupling we
// just removed. Tested via the per-adapter sign/verify roundtrip in
// `lib.rs`.

/// Lift a `keyring`-layer error into the `IdentityKeychainUnreachable`
/// refusal shape. Pure function; the caller decides whether to invoke
/// it based on the live keyring round-trip result.
///
/// This separation lets unit tests exercise the refusal shape without a
/// real keyring backend.
pub fn keychain_unreachable_refusal(detail: &str) -> ProbeRefusal {
    ProbeRefusal {
        reason: ProbeRefusalReason::IdentityKeychainUnreachable,
        detail: detail.to_string(),
        structured: json!({"layer": "os_keychain"}),
    }
}

// -----------------------------------------------------------------------------
// Arm 4 — WSL2 fallback key file perms
// -----------------------------------------------------------------------------

/// Inspect the mode bits of a fallback key file. Pass iff the
/// permission bits (lowest 9 bits, i.e. `mode & 0o777`) equal exactly
/// `0o600`. Any other value refuses startup.
///
/// The mode is passed in (rather than stat-ed here) so the arm is pure;
/// the caller in `lib.rs` does the `fs::metadata` call.
#[cfg(unix)]
pub fn check_fallback_key_perms(path: &std::path::Path, mode: u32) -> ArmOutcome {
    let perm_bits = mode & 0o777;
    if perm_bits == 0o600 {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::IdentityKeyPermsUnsafe,
        detail: format!(
            "WSL2 fallback key file {} has perms {:o}; expected 600",
            path.display(),
            perm_bits
        ),
        structured: json!({
            "path": path.display().to_string(),
            "got_mode": format!("{:o}", perm_bits),
            "expected_mode": "600",
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn did_doc_arm_passes_when_fragment_present() {
        let methods = vec![
            "#atproto".to_string(),
            "#org.openlore.application".to_string(),
        ];
        let outcome = check_did_document_lists_verification_method(
            "did:plc:test-jeff",
            &methods,
            "#org.openlore.application",
        );
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn did_doc_arm_refuses_when_fragment_missing() {
        let methods = vec!["#atproto".to_string()];
        let outcome = check_did_document_lists_verification_method(
            "did:plc:test-jeff",
            &methods,
            "#org.openlore.application",
        );
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::IdentityDidDocumentMismatch);
                assert!(r.detail.contains("does not list"));
            }
            ArmOutcome::Ok => panic!("expected refusal for missing fragment"),
        }
    }

    #[test]
    fn keychain_unreachable_lift_carries_reason() {
        let r = keychain_unreachable_refusal("Secret Service crashed");
        assert_eq!(r.reason, ProbeRefusalReason::IdentityKeychainUnreachable);
        assert_eq!(r.detail, "Secret Service crashed");
    }

    #[cfg(unix)]
    #[test]
    fn fallback_perms_arm_passes_for_0600() {
        let path = std::path::Path::new("/tmp/openlore/key");
        let outcome = check_fallback_key_perms(path, 0o600);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[cfg(unix)]
    #[test]
    fn fallback_perms_arm_refuses_for_world_readable() {
        let path = std::path::Path::new("/tmp/openlore/key");
        let outcome = check_fallback_key_perms(path, 0o644);
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::IdentityKeyPermsUnsafe);
                assert!(r.detail.contains("644"));
            }
            ArmOutcome::Ok => panic!("expected refusal for 644 perms"),
        }
    }

    /// Even a file the *owner* alone can read+write+execute (0700) must
    /// refuse — the spec is exactly `0600`, not "no group/other access".
    /// Catches a hypothetical bug where we'd test only the group/other
    /// bits and let the execute bit slip through.
    #[cfg(unix)]
    #[test]
    fn fallback_perms_arm_refuses_for_owner_execute() {
        let path = std::path::Path::new("/tmp/openlore/key");
        let outcome = check_fallback_key_perms(path, 0o700);
        assert!(matches!(outcome, ArmOutcome::Refused(_)));
    }

    /// Higher-bits (sticky / setuid / setgid) MUST be ignored: only the
    /// low 9 bits define "perms" for this check. A file with
    /// `0o4600` (setuid + owner rw) is still 600 in perm bits and should
    /// pass. Pinning this prevents a regression where someone tightens
    /// the mask too aggressively.
    #[cfg(unix)]
    #[test]
    fn fallback_perms_arm_ignores_setuid_bits() {
        let path = std::path::Path::new("/tmp/openlore/key");
        let outcome = check_fallback_key_perms(path, 0o4600);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }
}
