//! `probe` — Earned-Trust startup gauntlet for the ATProto PDS adapter.
//!
//! Implements the three checks ADR-004 + `architecture-design.md §6.2`
//! require of the PDS adapter:
//!
//! 1. **TLS handshake** against the configured PDS endpoint. The
//!    underlying TLS implementation is `rustls` (ADR-004) which honors
//!    `webpki-roots` only — system trust stores are not consulted. If
//!    the handshake fails (cert chain invalid, hostname mismatch, no
//!    network) the adapter refuses to start with
//!    `ProbeRefusalReason::PdsTlsHandshakeFailed`.
//!
//! 2. **describeServer DID match**. After a successful TLS handshake the
//!    adapter calls `com.atproto.server.describeServer` and asserts the
//!    returned DID equals the DID the user configured at
//!    `openlore init`. A mismatch indicates the user is pointed at the
//!    wrong PDS (or a PDS impersonation). Refusal reason:
//!    `ProbeRefusalReason::PdsDidMismatch`.
//!
//! 3. **rkey-collision idempotency sentinel**. The adapter writes a
//!    sentinel record under a fixed `rkey` twice. ATProto-compliant PDS
//!    implementations return 409/conflict on the second write (which the
//!    adapter treats as idempotent success). A non-compliant PDS that
//!    silently overwrites is a wire-protocol bug we MUST refuse to
//!    operate against — silent overwrite means CID-as-rkey is no longer
//!    a unique key, breaking the WS-9 idempotent-publish contract.
//!    Refusal reason: `ProbeRefusalReason::PdsIdempotencyViolation`.
//!
//! ## Pure-arm design (nw-fp-domain-modeling §8)
//!
//! Each arm is a pure function over a small input shape so the gauntlet
//! composes cleanly as a railway. The arm signatures return
//! `Result<(), ProbeRefusal>` and the `lib.rs::probe()` boundary lifts
//! them into `ProbeOutcome::Refused { reason, detail, structured }`.
//!
//! The arms in this module are SLICE-01 pure: they take the *result* of
//! the I/O step (TLS handshake outcome, describeServer DID payload,
//! rkey-collision write outcomes) and produce structured refusals. The
//! actual `reqwest` / `rustls` calls live in `lib.rs` so the arms here
//! stay unit-testable without standing up a real PDS.
//!
//! Slice-03 will rewire the I/O step against the federated PLC
//! directory; the arm signatures stay identical.

use serde_json::json;

use ports::ProbeRefusalReason;

/// Outcome of one arm of the probe. Aggregated by `lib.rs::probe()`
/// into a `ports::ProbeOutcome`.
#[derive(Debug, Clone)]
pub enum ArmOutcome {
    Ok,
    Refused(ProbeRefusal),
}

/// Structured refusal an arm emits when it trips. Mirrors the public
/// `ProbeOutcome::Refused` shape so the lift at `lib.rs::probe()` is a
/// 1:1 field copy.
#[derive(Debug, Clone)]
pub struct ProbeRefusal {
    pub reason: ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

// -----------------------------------------------------------------------------
// Arm 1 — TLS handshake against configured PDS endpoint
// -----------------------------------------------------------------------------

/// Inspect the outcome of a TLS handshake attempt. The caller (lib.rs)
/// runs the handshake using `reqwest` over `rustls`; this arm decides
/// whether the outcome is acceptable.
///
/// `tls_error` is `None` on success (handshake completed, cert chain
/// verified) and `Some(detail)` on any failure — cert chain invalid,
/// hostname mismatch, network unreachable, malformed TLS, etc.
pub fn check_tls_handshake(endpoint: &str, tls_error: Option<&str>) -> ArmOutcome {
    match tls_error {
        None => ArmOutcome::Ok,
        Some(detail) => ArmOutcome::Refused(ProbeRefusal {
            reason: ProbeRefusalReason::PdsTlsHandshakeFailed,
            detail: format!("TLS handshake against PDS endpoint {endpoint} failed: {detail}"),
            structured: json!({
                "endpoint": endpoint,
                "tls_error": detail,
            }),
        }),
    }
}

// -----------------------------------------------------------------------------
// Arm 2 — describeServer DID match
// -----------------------------------------------------------------------------

/// Inspect the DID returned by `com.atproto.server.describeServer`
/// against the DID the user configured. Passes iff they match exactly.
///
/// `actual_did` is the DID the PDS advertised; `expected_did` is the
/// DID the user configured at `openlore init`. Mismatch means the user
/// is talking to the wrong PDS — refuse with structured detail so the
/// observability layer can surface "you pointed at X but it claims to
/// be Y".
pub fn check_describe_server_did_matches(
    endpoint: &str,
    expected_did: &str,
    actual_did: &str,
) -> ArmOutcome {
    if expected_did == actual_did {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::PdsDidMismatch,
        detail: format!(
            "PDS {endpoint} reported describeServer.did = {actual_did:?}; \
             user configured {expected_did:?}"
        ),
        structured: json!({
            "endpoint": endpoint,
            "expected_did": expected_did,
            "actual_did": actual_did,
        }),
    })
}

// -----------------------------------------------------------------------------
// Arm 3 — rkey-collision idempotency sentinel
// -----------------------------------------------------------------------------

/// Inspect the outcome of the rkey-collision idempotency probe.
///
/// The caller writes a sentinel record under a fixed `rkey` twice. The
/// arm checks that the PDS did NOT silently overwrite the first write —
/// either it returned 409/conflict (treated as idempotent success) or it
/// echoed back the first record's AT URI verbatim. If the second write
/// *succeeded with a different AT URI* or the second body REPLACED the
/// first body, the PDS is non-compliant and the adapter must refuse to
/// start.
///
/// `first_at_uri` / `second_at_uri` are the AT URIs the PDS returned for
/// the two writes. `bodies_after_match` is `true` iff a subsequent
/// `getRecord` returned the FIRST body (idempotent) and `false` iff it
/// returned the SECOND body (silent overwrite — the violation).
pub fn check_rkey_collision_idempotent(
    endpoint: &str,
    first_at_uri: &str,
    second_at_uri: &str,
    bodies_after_match: bool,
) -> ArmOutcome {
    if first_at_uri == second_at_uri && bodies_after_match {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::PdsIdempotencyViolation,
        detail: format!(
            "PDS {endpoint} silently overwrote on rkey collision \
             (first_at_uri={first_at_uri}, second_at_uri={second_at_uri}, \
             body_match_after={bodies_after_match})"
        ),
        structured: json!({
            "endpoint": endpoint,
            "first_at_uri": first_at_uri,
            "second_at_uri": second_at_uri,
            "bodies_after_match": bodies_after_match,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT: &str = "https://pds.test.example";

    // -------- Arm 1: TLS handshake --------

    #[test]
    fn tls_arm_passes_when_no_error() {
        let outcome = check_tls_handshake(ENDPOINT, None);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn tls_arm_refuses_on_handshake_error() {
        let outcome = check_tls_handshake(ENDPOINT, Some("cert chain invalid"));
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::PdsTlsHandshakeFailed);
                assert!(r.detail.contains("cert chain invalid"));
                assert!(r.detail.contains(ENDPOINT));
            }
            ArmOutcome::Ok => panic!("expected refusal for TLS error"),
        }
    }

    // -------- Arm 2: describeServer DID match --------

    #[test]
    fn describe_server_arm_passes_when_dids_match() {
        let outcome =
            check_describe_server_did_matches(ENDPOINT, "did:plc:configured", "did:plc:configured");
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn describe_server_arm_refuses_on_did_mismatch() {
        let outcome =
            check_describe_server_did_matches(ENDPOINT, "did:plc:configured", "did:plc:imposter");
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::PdsDidMismatch);
                assert!(r.detail.contains("did:plc:configured"));
                assert!(r.detail.contains("did:plc:imposter"));
            }
            ArmOutcome::Ok => panic!("expected refusal for DID mismatch"),
        }
    }

    // -------- Arm 3: rkey-collision idempotency --------

    #[test]
    fn idempotency_arm_passes_when_uris_match_and_body_unchanged() {
        let outcome = check_rkey_collision_idempotent(
            ENDPOINT,
            "at://did:plc:test/x/sentinel",
            "at://did:plc:test/x/sentinel",
            /* bodies_after_match */ true,
        );
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn idempotency_arm_refuses_when_pds_silently_overwrites_body() {
        // Same AT URI but the second write *replaced* the body — the
        // PDS is treating rkey as a primary key with overwrite-on-update
        // semantics, which violates ATProto's idempotency contract.
        let outcome = check_rkey_collision_idempotent(
            ENDPOINT,
            "at://did:plc:test/x/sentinel",
            "at://did:plc:test/x/sentinel",
            /* bodies_after_match */ false,
        );
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::PdsIdempotencyViolation);
                assert!(r.detail.contains("silently overwrote"));
            }
            ArmOutcome::Ok => panic!("expected refusal for silent overwrite"),
        }
    }

    #[test]
    fn idempotency_arm_refuses_when_pds_returns_different_uri_on_collision() {
        // The PDS returned a NEW AT URI for the second write — it
        // treated the rkey as a generated-id instead of honoring our
        // explicit choice. Same violation class as silent overwrite:
        // CID-as-rkey is no longer a stable key.
        let outcome = check_rkey_collision_idempotent(
            ENDPOINT,
            "at://did:plc:test/x/sentinel",
            "at://did:plc:test/x/sentinel-2",
            true,
        );
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::PdsIdempotencyViolation);
            }
            ArmOutcome::Ok => panic!("expected refusal for differing AT URIs"),
        }
    }
}
