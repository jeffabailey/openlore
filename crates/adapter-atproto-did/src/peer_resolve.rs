//! `peer_resolve` — slice-03 peer DID-document resolution.
//!
//! Backs `IdentityPort::resolve_peer`: fetch the peer's DID document via
//! `com.atproto.identity.resolveDid`, parse it into the `PeerInfo` shape,
//! and surface transport / parse failures as
//! `IdentityError::PeerResolutionFailed { did, detail }`.
//!
//! ## Why this lives in its own module (Extension Justification)
//!
//! WHY-NEW-FILE: crates/adapter-atproto-did/src/peer_resolve.rs
//!   CLOSEST-EXISTING: crates/adapter-atproto-did/src/probe.rs
//!   EXTENSION-COST: `probe.rs` holds pure probe ARMS (consume an I/O
//!     outcome, emit a structured refusal); folding the resolve pipeline
//!     into it would mix the probe's pure-arm contract with live PLC /
//!     did:web transport orchestration — two different lifecycles.
//!   PARALLEL-RATIONALE: peer resolution owns network transport (HTTP GET
//!     against the resolver) plus DID-document parsing; that is a distinct
//!     dependency surface (reqwest blocking client + DID-doc parser) from
//!     probe.rs's pure-arm signature, and the design's §6.3 probe table
//!     treats `resolve_peer` as the thing the probe DRIVES, not part of it.
//!
//! ## Architectural posture (ADR-009 effect shell; WD-29; ADR-016)
//!
//! The resolution is pure modulo the single network read: fetch the DID
//! document, parse it into `PeerInfo`, surface transport / parse failures
//! as `IdentityError::PeerResolutionFailed`. Per ADR-016 the result is NOT
//! cached on the adapter; every `peer add` / `peer pull` re-resolves.
//!
//! ## Resolver-endpoint seam (DD; acceptance-tests.md §test-doubles)
//!
//! In production the resolver base URL is the PLC directory
//! (`https://plc.directory`). For acceptance tests the per-peer
//! `FakePeerPds` hosts the `resolveDid` handler on its own random port; the
//! test threads that base URL in via the `OPENLORE_PEER_PDS_ENDPOINT_<did>`
//! env var (same explicit-env-seam pattern as slice-01's
//! `OPENLORE_PDS_ENDPOINT`). The env-var NAME encoding MUST match the test
//! harness: uppercase the DID, replace every non-`[A-Z0-9]` char with `_`.

use claim_domain::Did;
use ports::{IdentityError, PeerInfo, VerificationMethod};
use url::Url;

/// Production resolver base URL when no per-peer env override is present.
const DEFAULT_RESOLVER_BASE_URL: &str = "https://plc.directory";

/// Resolve a peer's DID document into a `PeerInfo` (handle, current PDS
/// endpoint, verification methods).
///
/// Dispatches on the DID method only insofar as the resolver path is the
/// same XRPC `resolveDid` endpoint for `did:plc:` and `did:web:` against
/// the configured resolver base URL. On any transport / parse failure
/// returns `IdentityError::PeerResolutionFailed { did, detail }` carrying
/// the underlying error verbatim for diagnostics (never panics, never
/// returns a silently-empty `PeerInfo`).
pub(crate) fn resolve_peer_did(peer_did: &Did) -> Result<PeerInfo, IdentityError> {
    let base = resolver_base_url(peer_did);
    let document = fetch_did_document(peer_did, &base)?;
    parse_peer_info(peer_did, &document)
}

/// Determine the resolver base URL: the per-peer env override if present,
/// else the production PLC directory.
fn resolver_base_url(peer_did: &Did) -> String {
    std::env::var(peer_resolver_env_var(&peer_did.0))
        .unwrap_or_else(|_| DEFAULT_RESOLVER_BASE_URL.to_string())
}

/// The per-peer resolver env-var NAME for a DID. Encoding: uppercase the
/// DID and replace every non-`[A-Z0-9]` character with `_` so the result
/// is a legal POSIX environment-variable name. MUST agree with the
/// acceptance harness (`tests/acceptance/peer_subscribe.rs`).
///
/// `did:plc:rachel-test` → `OPENLORE_PEER_PDS_ENDPOINT_DID_PLC_RACHEL_TEST`.
fn peer_resolver_env_var(did: &str) -> String {
    let encoded: String = did
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("OPENLORE_PEER_PDS_ENDPOINT_{encoded}")
}

/// Issue the synchronous `resolveDid` HTTP GET and return the parsed JSON
/// DID document. All transport-layer failures lift to
/// `PeerResolutionFailed`.
fn fetch_did_document(
    peer_did: &Did,
    resolver_base: &str,
) -> Result<serde_json::Value, IdentityError> {
    let url = format!(
        "{}/xrpc/com.atproto.identity.resolveDid?did={}",
        resolver_base.trim_end_matches('/'),
        urlencode(&peer_did.0),
    );

    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|err| fail(peer_did, format!("build HTTP client: {err}")))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|err| fail(peer_did, format!("resolveDid transport error: {err}")))?;

    if !response.status().is_success() {
        return Err(fail(
            peer_did,
            format!("resolveDid returned HTTP {}", response.status().as_u16()),
        ));
    }

    response
        .json::<serde_json::Value>()
        .map_err(|err| fail(peer_did, format!("resolveDid body is not JSON: {err}")))
}

/// Parse a W3C DID document JSON value into `PeerInfo`. Extracts the PDS
/// endpoint from `service[].serviceEndpoint`, the handle from
/// `alsoKnownAs[0]` (stripped of the `at://` prefix), and every
/// `verificationMethod` entry.
fn parse_peer_info(
    peer_did: &Did,
    document: &serde_json::Value,
) -> Result<PeerInfo, IdentityError> {
    let pds_endpoint_str = document
        .get("service")
        .and_then(|s| s.as_array())
        .and_then(|services| {
            services
                .iter()
                .find(|svc| {
                    svc.get("type").and_then(|t| t.as_str()) == Some("AtprotoPersonalDataServer")
                })
                .or_else(|| services.first())
        })
        .and_then(|svc| svc.get("serviceEndpoint"))
        .and_then(|e| e.as_str())
        .ok_or_else(|| {
            fail(
                peer_did,
                "DID document has no service[].serviceEndpoint (no PDS)".to_string(),
            )
        })?;

    let pds_endpoint = Url::parse(pds_endpoint_str).map_err(|err| {
        fail(
            peer_did,
            format!("DID document serviceEndpoint {pds_endpoint_str:?} is not a valid URL: {err}"),
        )
    })?;

    let handle = document
        .get("alsoKnownAs")
        .and_then(|a| a.as_array())
        .and_then(|aliases| aliases.first())
        .and_then(|h| h.as_str())
        .map(|h| h.trim_start_matches("at://").to_string())
        .unwrap_or_default();

    let verification_methods = parse_verification_methods(document);

    Ok(PeerInfo {
        did: peer_did.clone(),
        handle,
        pds_endpoint,
        verification_methods,
    })
}

/// Parse the `verificationMethod[]` array into the port's
/// `VerificationMethod` shape. Entries missing a required field are
/// skipped (a peer with zero usable methods still resolves — the
/// signature-verification path, slice-03 PP-*, decides whether that is
/// acceptable per record).
fn parse_verification_methods(document: &serde_json::Value) -> Vec<VerificationMethod> {
    document
        .get("verificationMethod")
        .and_then(|v| v.as_array())
        .map(|methods| {
            methods
                .iter()
                .filter_map(|m| {
                    Some(VerificationMethod {
                        id: m.get("id")?.as_str()?.to_string(),
                        type_: m.get("type")?.as_str()?.to_string(),
                        controller: Did(m.get("controller")?.as_str()?.to_string()),
                        public_key_multibase: m.get("publicKeyMultibase")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Construct the `PeerResolutionFailed` error for `peer_did` with `detail`.
fn fail(peer_did: &Did, detail: String) -> IdentityError {
    IdentityError::PeerResolutionFailed {
        did: peer_did.clone(),
        detail,
    }
}

/// Minimal percent-encoding for the `did` query parameter. DIDs contain
/// `:` which must be encoded in a query value; we encode the small set of
/// reserved characters DIDs can carry rather than pulling in a full
/// urlencoding dependency.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The resolver env-var encoding is the single source of truth shared
    /// with the acceptance harness; pin it so a divergence reds here, not
    /// silently in a subprocess test.
    #[test]
    fn peer_resolver_env_var_encodes_did_to_legal_name() {
        assert_eq!(
            peer_resolver_env_var("did:plc:rachel-test"),
            "OPENLORE_PEER_PDS_ENDPOINT_DID_PLC_RACHEL_TEST"
        );
    }

    /// `parse_peer_info` extracts the PDS endpoint, handle, and
    /// verification methods from a well-formed DID document (the shape the
    /// FakePeerPds resolveDid handler returns).
    #[test]
    fn parse_peer_info_extracts_endpoint_handle_and_methods() {
        let did = Did("did:plc:rachel-test".to_string());
        let document = serde_json::json!({
            "id": "did:plc:rachel-test",
            "alsoKnownAs": ["at://rachel-test.test"],
            "verificationMethod": [{
                "id": "did:plc:rachel-test#org.openlore.application",
                "type": "Multikey",
                "controller": "did:plc:rachel-test",
                "publicKeyMultibase": "z6Mkfake"
            }],
            "service": [{
                "id": "#atproto_pds",
                "type": "AtprotoPersonalDataServer",
                "serviceEndpoint": "http://127.0.0.1:54321"
            }]
        });

        let info = parse_peer_info(&did, &document).expect("parse well-formed document");
        assert_eq!(info.did, did);
        assert_eq!(info.handle, "rachel-test.test");
        assert_eq!(info.pds_endpoint.as_str(), "http://127.0.0.1:54321/");
        assert_eq!(info.verification_methods.len(), 1);
        assert_eq!(
            info.verification_methods[0].id,
            "did:plc:rachel-test#org.openlore.application"
        );
    }

    /// A DID document with no service entry fails resolution rather than
    /// returning a degenerate `PeerInfo`.
    #[test]
    fn parse_peer_info_without_service_endpoint_fails() {
        let did = Did("did:plc:no-pds".to_string());
        let document = serde_json::json!({ "id": "did:plc:no-pds" });
        let err = parse_peer_info(&did, &document).expect_err("must fail without a PDS endpoint");
        assert!(matches!(err, IdentityError::PeerResolutionFailed { .. }));
    }
}
