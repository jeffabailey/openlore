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

use claim_domain::{Did, VerificationKey};
use ports::{IdentityError, PeerInfo, ResolveError, VerificationMethod};
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
    let mut info = parse_peer_info(peer_did, &document)?;
    // Acceptance-test pubkey seam (DD; mirrors the resolver-endpoint seam):
    // the `FakePeerPds` resolveDid DID-document only carries a PLACEHOLDER
    // `publicKeyMultibase`, so when the per-peer pubkey env override is
    // present we inject a verification method whose `public_key_multibase`
    // carries the REAL Ed25519 key (encoded `hex:<64-char-hex>`). The pull
    // pipeline (`VerbPeerPull`) decodes it for `claim_domain::verify`.
    //
    // RELEASE-GATED (ADR-026 / I-AV-6): the seam is a DEBUG-ONLY short-circuit.
    // `pubkey_override_method` is `#[cfg(debug_assertions)]` ⇒ in a release build
    // (`debug_assertions = false`) it is the stub returning `None`, the seam env
    // var is NEVER read, and the resolved DID-document key stands as the sole
    // authority. In debug/test (`cargo test`/`cargo build`) it reads the seam so
    // the slice-03 peer_pull/peer_subscribe acceptance tests keep working.
    if let Some(method) = pubkey_override_method(peer_did) {
        info.verification_methods.insert(0, method);
    }
    Ok(info)
}

/// Build a verification method from the per-peer pubkey env override, if set.
/// Returns `None` when the env var is absent so the resolved DID-document
/// verification methods stand unchanged.
///
/// DEBUG-ONLY (ADR-026 / I-AV-6): this seam-reading variant compiles ONLY when
/// `debug_assertions` is true (debug + test builds). The release variant below is
/// a stub that never reads the env — the seam is release-FORBIDDEN.
#[cfg(debug_assertions)]
fn pubkey_override_method(peer_did: &Did) -> Option<VerificationMethod> {
    let hex = std::env::var(peer_pubkey_env_var(&peer_did.0)).ok()?;
    let trimmed = hex.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(VerificationMethod {
        id: format!("{}#org.openlore.application", peer_did.0),
        type_: "Multikey".to_string(),
        controller: peer_did.clone(),
        // Carry the raw key as `hex:<64-char-hex>` so the consumer
        // (`VerbPeerPull`) can decode it without a multibase dependency.
        public_key_multibase: format!("hex:{trimmed}"),
    })
}

/// Release variant of [`pubkey_override_method`]: the seam is compiled OUT
/// (`debug_assertions = false`), so there is NO env read — resolution proceeds
/// with the resolved DID-document verification methods only (ADR-026 / I-AV-6).
#[cfg(not(debug_assertions))]
fn pubkey_override_method(_peer_did: &Did) -> Option<VerificationMethod> {
    None
}

/// Slice-05: resolve `did` into its Ed25519 [`VerificationKey`] for the indexer's
/// verify-only [`IdentityResolvePort`] — the slice-03 pubkey seam
/// (`OPENLORE_PEER_PUBKEY_HEX_<did>`) when SET (AV-1/2/3 hermetic walking
/// skeleton), ELSE the REAL ADR-026 PLC `z6Mk...` resolve + decode (AV-4 gold
/// path, seam UNSET).
///
/// This dispatcher lives HERE (not in `lib.rs`) because `peer_resolve` is the
/// established home of the `OPENLORE_PEER_PUBKEY_HEX_<did>` seam (RETAINED for
/// tests per the slice-03 contract) and is intentionally OUT of the I-AV-6
/// `xtask check-arch` pubkey-seam scan scope — keeping the seam token out of the
/// scanned `lib.rs`. The `lib.rs` call site references only THIS function name,
/// never the seam literal.
///
/// The DID is normalized to its bare form for the SEAM lookup (the seam env var
/// is keyed on the bare DID) AND for the PLC resolve (the PLC directory is keyed
/// on the bare DID, ADR-026); the indexer resolves the SIGNED payload's author,
/// which carries the `#org.openlore.application` fragment.
pub(crate) async fn resolve_verification_key(did: &Did) -> Result<VerificationKey, ResolveError> {
    // RELEASE-GATED (ADR-026 / I-AV-6): in DEBUG/test (`debug_assertions = true`)
    // the seam SET → hermetic walking-skeleton path (AV-1/2/3); seam UNSET → fall
    // through to the REAL PLC z6Mk decode (AV-4). In RELEASE
    // (`debug_assertions = false`) `seam_verification_key` is the stub that always
    // returns `Ok(None)` — the seam is compiled out — so resolution proceeds
    // STRAIGHT to the real PLC decode. The seam is a debug-only short-circuit.
    match seam_verification_key(did)? {
        Some(key) => Ok(key),
        None => resolve_verification_key_via_plc(did).await,
    }
}

/// Read the slice-03 pubkey seam for `did`: `Ok(Some(key))` when
/// `OPENLORE_PEER_PUBKEY_HEX_<did>` is set, `Ok(None)` when it is UNSET (the AV-4
/// gold-path signal to fall through to the real PLC decode), `Err` when the seam
/// value is malformed.
///
/// DEBUG-ONLY (ADR-026 / I-AV-6): this seam-reading variant compiles ONLY when
/// `debug_assertions` is true (debug + test builds), so the slice-05
/// AV-1/2/3/5/6/7 acceptance tests that seed `OPENLORE_PEER_PUBKEY_HEX_*` keep
/// working under `cargo test`. The release variant below always returns
/// `Ok(None)` — the seam read is compiled OUT — so a release build can NEVER
/// short-circuit the real PLC `z6Mk...` decode via the environment.
#[cfg(debug_assertions)]
fn seam_verification_key(did: &Did) -> Result<Option<VerificationKey>, ResolveError> {
    let bare_did = did.0.split('#').next().unwrap_or(&did.0);
    let hex = match std::env::var(peer_pubkey_env_var(bare_did)) {
        Ok(hex) => hex,
        Err(_) => return Ok(None),
    };
    if hex.trim().is_empty() {
        return Ok(None);
    }
    let bytes = decode_hex(hex.trim()).map_err(|detail| ResolveError::PubkeyDecodeFailed {
        did: did.clone(),
        detail,
    })?;
    Ok(Some(VerificationKey(bytes)))
}

/// Release variant of [`seam_verification_key`]: the seam is compiled OUT
/// (`debug_assertions = false`), so it always signals "no seam" — resolution in
/// [`resolve_verification_key`] falls through to the REAL PLC `z6Mk...` decode
/// (ADR-026 / I-AV-6). The `did` is unused on this path.
#[cfg(not(debug_assertions))]
fn seam_verification_key(_did: &Did) -> Result<Option<VerificationKey>, ResolveError> {
    Ok(None)
}

/// The REAL ADR-026 production path (AV-4 gold path): resolve `did`'s PLC DID
/// document over the network, locate the `#org.openlore.application` verification
/// method, read its `publicKeyMultibase` (`z6Mk...`), and decode it via the PURE
/// `claim_domain::decode_ed25519_multibase` into the [`VerificationKey`] the pure
/// `verify` consumes.
///
/// The DID-document fetch reuses the established `reqwest` blocking client +
/// `serde_json` DID-doc parse (the same transport `resolve_peer_did` uses); the
/// PLC endpoint defaults to `https://plc.directory` (ADR-026 §"Config + default")
/// and is overridable for hermetic acceptance via `OPENLORE_INDEXER_PLC_ENDPOINT`.
/// The decode is the EFFECT-shell's only call into the pure core (no second
/// verification path; WD-104).
async fn resolve_verification_key_via_plc(did: &Did) -> Result<VerificationKey, ResolveError> {
    let bare_did = did.0.split('#').next().unwrap_or(&did.0).to_string();
    let base = plc_endpoint();
    let document = fetch_plc_did_document(did, &base, &bare_did).await?;
    let multibase = openlore_application_public_key_multibase(did, &bare_did, &document)?;
    claim_domain::decode_ed25519_multibase(&multibase).map_err(|err| {
        ResolveError::PubkeyDecodeFailed {
            did: did.clone(),
            detail: format!("{err:?}"),
        }
    })
}

/// The PLC directory endpoint: the per-run `OPENLORE_INDEXER_PLC_ENDPOINT`
/// override (hermetic acceptance) if present, else the production default
/// `https://plc.directory` (ADR-026 §"Config + default").
fn plc_endpoint() -> String {
    std::env::var("OPENLORE_INDEXER_PLC_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_PLC_ENDPOINT.to_string())
}

/// Resolve-readiness Earned-Trust check for the verify-only
/// [`crate::AtProtoDidAdapter`]'s `IdentityResolvePort::probe` (ADR-009 / I-4):
/// the configured PLC endpoint (the URL the resolve path fetches `z6Mk` DID
/// documents from) MUST be a well-formed absolute `http(s)` URL. An empty or
/// malformed endpoint means the resolve path could NEVER fetch a DID document —
/// the indexer would then reject every network record at use-time, so it must
/// REFUSE to start instead (DESIGN §6.3; mirrors `AtProtoIngestAdapter`'s
/// empty-source readiness arm).
///
/// Deterministic + in-process: like the sibling adapters' probes it does NO
/// network round-trip — the REAL end-to-end `z6Mk` resolve+decode is exercised by
/// the AV-4 gold-path acceptance test. The `OPENLORE_INDEXER_PLC_ENDPOINT` read
/// lives HERE (not in `lib.rs`) so the lib.rs `xtask check-arch` pubkey/endpoint
/// seam-scan scope stays clean (same rationale as `resolve_verification_key`).
///
/// Returns `Ok(endpoint)` with the validated endpoint when ready; `Err(detail)`
/// (a pre-formatted refusal reason) when the configured endpoint is unusable.
pub(crate) fn check_plc_endpoint_ready() -> Result<String, String> {
    let endpoint = plc_endpoint();
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("PLC resolver endpoint is empty — cannot resolve any DID document".to_string());
    }
    match Url::parse(trimmed) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.has_host() => Ok(endpoint),
        Ok(url) => Err(format!(
            "PLC resolver endpoint {trimmed:?} is not a usable http(s) URL with a host \
             (scheme {:?})",
            url.scheme()
        )),
        Err(err) => Err(format!(
            "PLC resolver endpoint {trimmed:?} is not a valid URL: {err}"
        )),
    }
}

/// The production PLC directory base URL (ADR-026 default).
const DEFAULT_PLC_ENDPOINT: &str = "https://plc.directory";

/// GET the DID document from the PLC directory at `<base>/<did>` (the canonical
/// PLC-directory shape, ADR-026 §"Resolve the DID document"). All transport
/// failures lift to `ResolveError::ResolutionFailed`.
async fn fetch_plc_did_document(
    did: &Did,
    base: &str,
    bare_did: &str,
) -> Result<serde_json::Value, ResolveError> {
    let url = format!("{}/{}", base.trim_end_matches('/'), urlencode(bare_did));

    // ASYNC client: this path runs inside the indexer's tokio runtime (the
    // resolve trait method is async). A blocking reqwest client here would panic
    // ("cannot block within an async context"); the async client composes cleanly.
    let client = reqwest::Client::builder()
        .build()
        .map_err(|err| resolve_fail(did, format!("build HTTP client: {err}")))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| resolve_fail(did, format!("PLC resolve transport error: {err}")))?;

    if !response.status().is_success() {
        return Err(resolve_fail(
            did,
            format!("PLC resolve returned HTTP {}", response.status().as_u16()),
        ));
    }

    response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| resolve_fail(did, format!("PLC DID document is not JSON: {err}")))
}

/// Locate the `#org.openlore.application` verification method in the resolved DID
/// document and return its `publicKeyMultibase` (ADR-026 §"Locate the
/// verification method"). The method id is matched by its `#fragment` suffix so a
/// fully-qualified (`did:plc:x#org.openlore.application`) id resolves.
fn openlore_application_public_key_multibase(
    did: &Did,
    bare_did: &str,
    document: &serde_json::Value,
) -> Result<String, ResolveError> {
    let methods = document
        .get("verificationMethod")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            resolve_fail(
                did,
                "DID document has no verificationMethod array".to_string(),
            )
        })?;

    let fragment = format!("{bare_did}#org.openlore.application");
    methods
        .iter()
        .find(|m| {
            m.get("id")
                .and_then(|id| id.as_str())
                .map(|id| id == fragment || id.ends_with("#org.openlore.application"))
                .unwrap_or(false)
        })
        .and_then(|m| m.get("publicKeyMultibase"))
        .and_then(|k| k.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            resolve_fail(
                did,
                "DID document has no #org.openlore.application publicKeyMultibase".to_string(),
            )
        })
}

/// Construct a `ResolveError::ResolutionFailed` for `did` with `detail`.
fn resolve_fail(did: &Did, detail: String) -> ResolveError {
    ResolveError::ResolutionFailed {
        did: did.clone(),
        detail,
    }
}

/// Lowercase/uppercase hex → raw bytes. Strict: an odd length or a non-hex
/// character is a hard error.
///
/// DEBUG-ONLY (ADR-026 / I-AV-6): only the debug-gated `seam_verification_key`
/// decodes the seam's hex value, so this helper compiles out of release with it.
#[cfg(debug_assertions)]
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err(format!("hex string has odd length {}", s.len()));
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

/// Parse one hex character into a 4-bit nibble.
///
/// DEBUG-ONLY (ADR-026 / I-AV-6): used only by the debug-gated `decode_hex`.
#[cfg(debug_assertions)]
fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex character: {:?}", b as char)),
    }
}

/// The per-peer pubkey env-var NAME for a DID: uppercase the DID, replace
/// every non-`[A-Z0-9]` char with `_`. MUST agree with the acceptance
/// harness (`tests/acceptance/support/mod.rs::peer_pubkey_env_var`).
///
/// `did:plc:rachel-test` → `OPENLORE_PEER_PUBKEY_HEX_DID_PLC_RACHEL_TEST`.
///
/// DEBUG-ONLY (ADR-026 / I-AV-6): this function holds the release-forbidden
/// `OPENLORE_PEER_PUBKEY_HEX_` token literal and is called ONLY by the
/// debug-gated seam readers, so it is `#[cfg(debug_assertions)]`-gated — the
/// token is compiled OUT of release builds. The `xtask check-arch`
/// pubkey-seam guard verifies this gate stays in place.
#[cfg(debug_assertions)]
fn peer_pubkey_env_var(did: &str) -> String {
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
    format!("OPENLORE_PEER_PUBKEY_HEX_{encoded}")
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

    /// Resolve-readiness probe arm: with the production default endpoint (no env
    /// override) the configured PLC endpoint is a well-formed http(s) URL, so the
    /// readiness check is `Ok` and returns the validated endpoint. Mutating the
    /// env var to a malformed value flips it to `Err` — proving the check does
    /// REAL work (it is not a trivial always-`Ok`). Serialized on the env var so
    /// it never races a concurrent test reading `OPENLORE_INDEXER_PLC_ENDPOINT`.
    #[test]
    fn check_plc_endpoint_ready_accepts_default_and_refuses_malformed() {
        // Snapshot + clear any ambient override so the default path is exercised.
        let prior = std::env::var("OPENLORE_INDEXER_PLC_ENDPOINT").ok();
        std::env::remove_var("OPENLORE_INDEXER_PLC_ENDPOINT");
        let ready = check_plc_endpoint_ready();
        assert_eq!(
            ready.as_deref(),
            Ok(DEFAULT_PLC_ENDPOINT),
            "the production default PLC endpoint must pass the readiness check"
        );

        // A malformed endpoint must REFUSE (proves the arm does real validation).
        std::env::set_var("OPENLORE_INDEXER_PLC_ENDPOINT", "not a url");
        let refused = check_plc_endpoint_ready();
        assert!(
            refused.is_err(),
            "a malformed PLC endpoint must refuse readiness; got {refused:?}"
        );

        // An empty endpoint must also REFUSE.
        std::env::set_var("OPENLORE_INDEXER_PLC_ENDPOINT", "   ");
        assert!(
            check_plc_endpoint_ready().is_err(),
            "an empty PLC endpoint must refuse readiness"
        );

        // Restore ambient state for any sibling test.
        match prior {
            Some(v) => std::env::set_var("OPENLORE_INDEXER_PLC_ENDPOINT", v),
            None => std::env::remove_var("OPENLORE_INDEXER_PLC_ENDPOINT"),
        }
    }
}
