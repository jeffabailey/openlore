//! `peer_read` — slice-03 peer-PDS read pipeline.
//!
//! Backs `PdsPort::list_peer_records` + `PdsPort::get_peer_record`. The
//! peer's PDS hosts records under the `org.openlore.claim` collection in
//! the ATProto wire shape (lexicon JSON: `author`, `composedAt`,
//! `signature: {kid, alg, sig}`). This module:
//!
//! 1. Issues `com.atproto.repo.listRecords` (walking ALL cursors per
//!    Q-DELIVER-5) / `com.atproto.repo.getRecord` against the peer's
//!    PDS endpoint (taken FRESH per ADR-016 — never cached on the adapter).
//! 2. Parses each returned record's `value` (lexicon JSON) through the ONE
//!    shared decoder (`claim_domain::decode_claim_record`) into the domain
//!    `SignedRecord`, carrying the peer-published `rkey`. ADR-071: a record
//!    with no `signature` parses as `ClaimRecord::SelfAttested`; the verb's
//!    provenance verdict decides whether it is admitted.
//!
//! Signature verification + CID byte-matching are NOT this adapter's job
//! (component-boundaries §adapter-atproto-pds) — they happen in
//! `claim_domain` (pure) called from `VerbPeerPull` (cli). This module
//! recomputes the unsigned-CID so the parsed `SignedClaim` is well-formed
//! (`signature.signed_cid` is populated), but it makes NO trust decision:
//! a record whose `rkey` disagrees with the recomputed CID is still
//! returned verbatim so the verb can reject it per WD-24.
//!
//! ## Why this lives in its own module (Extension Justification)
//!
//! WHY-NEW-FILE: crates/adapter-atproto-pds/src/peer_read.rs
//!   CLOSEST-EXISTING: crates/adapter-atproto-pds/src/probe.rs
//!   EXTENSION-COST: `probe.rs` holds pure probe ARMS that consume the
//!     outcome of an XRPC step and emit structured refusals; folding the
//!     peer-read pipeline into it would couple the probe's pure-arm
//!     contract to the live `listRecords` / `getRecord` paging + parse
//!     orchestration.
//!   PARALLEL-RATIONALE: peer read owns the `com.atproto.repo.listRecords`
//!     cursor walk and per-record parse into `SignedRecord`; the design's
//!     §6.3 probe table treats `list_peer_records` as the thing the probe
//!     DRIVES (it re-computes CIDs against the listed records), so the
//!     read path and the probe arm have different call directions.

use ports::claim_domain::{self, ClaimRecord, Did};
use ports::{PdsError, PeerRecordPage, SignedRecord};
use url::Url;

/// The collection peer claims live under (ADR-005).
const PEER_CLAIM_COLLECTION: &str = "org.openlore.claim";

/// Page through a peer's `org.openlore.claim` records via
/// `com.atproto.repo.listRecords`, walking EVERY cursor (Q-DELIVER-5).
///
/// `cursor = None` requests the first page. The ATProto `listRecords`
/// response carries an opaque `cursor`; this function follows it until the
/// server returns no more, accumulating every parsed record into ONE
/// `PeerRecordPage` whose `next_cursor` is `None` (the caller sees the
/// fully-walked stream as a single page — fault isolation happens
/// per-record in the verb, not per-page here). The `peer_pds_endpoint` is
/// taken fresh per ADR-016.
pub(crate) async fn list_peer_records_xrpc(
    peer_did: &Did,
    peer_pds_endpoint: &Url,
    cursor: Option<String>,
) -> Result<PeerRecordPage, PdsError> {
    let client = build_client()?;
    let mut all_records: Vec<SignedRecord> = Vec::new();
    let mut next = cursor;

    loop {
        let page = fetch_list_page(&client, peer_did, peer_pds_endpoint, next.as_deref()).await?;
        for value in page.records {
            all_records.push(parse_record_view(peer_did, &value)?);
        }
        match page.cursor {
            // ATProto convention: an absent OR empty cursor ends the walk.
            Some(c) if !c.trim().is_empty() => next = Some(c),
            _ => break,
        }
    }

    Ok(PeerRecordPage {
        records: all_records,
        next_cursor: None,
    })
}

/// Fetch one specific peer record by `rkey` via
/// `com.atproto.repo.getRecord`. A missing record surfaces as
/// `PdsError::PeerRecordNotFound`. Endpoint taken fresh per ADR-016.
pub(crate) async fn get_peer_record_xrpc(
    peer_did: &Did,
    peer_pds_endpoint: &Url,
    rkey: &str,
) -> Result<SignedRecord, PdsError> {
    let client = build_client()?;
    let url = format!(
        "{}/xrpc/com.atproto.repo.getRecord?repo={}&collection={}&rkey={}",
        endpoint_base(peer_pds_endpoint),
        urlencode(&peer_did.0),
        PEER_CLAIM_COLLECTION,
        urlencode(rkey),
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(classify_network_error)?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(PdsError::PeerRecordNotFound {
            collection: PEER_CLAIM_COLLECTION.to_string(),
            rkey: rkey.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(PdsError::PeerRecordNotFound {
            collection: PEER_CLAIM_COLLECTION.to_string(),
            rkey: rkey.to_string(),
        });
    }

    let value: serde_json::Value =
        response
            .json()
            .await
            .map_err(|err| PdsError::PeerRecordSchemaInvalid {
                detail: format!("getRecord body is not JSON: {err}"),
            })?;

    parse_record_view(peer_did, &value)
}

// -----------------------------------------------------------------------------
// HTTP helpers
// -----------------------------------------------------------------------------

/// One page of the raw `listRecords` response.
struct RawListPage {
    records: Vec<serde_json::Value>,
    cursor: Option<String>,
}

/// Build the shared reqwest client with a connect timeout so an
/// unreachable peer PDS surfaces as `Unreachable` quickly (PP-7) rather
/// than hanging the pull for minutes.
fn build_client() -> Result<reqwest::Client, PdsError> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| PdsError::Unreachable {
            message: format!("build reqwest client: {err}"),
        })
}

/// Issue one `listRecords` GET and return its raw records + cursor.
async fn fetch_list_page(
    client: &reqwest::Client,
    peer_did: &Did,
    peer_pds_endpoint: &Url,
    cursor: Option<&str>,
) -> Result<RawListPage, PdsError> {
    let mut url = format!(
        "{}/xrpc/com.atproto.repo.listRecords?repo={}&collection={}",
        endpoint_base(peer_pds_endpoint),
        urlencode(&peer_did.0),
        PEER_CLAIM_COLLECTION,
    );
    if let Some(c) = cursor {
        url.push_str(&format!("&cursor={}", urlencode(c)));
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(classify_network_error)?;

    if !response.status().is_success() {
        return Err(PdsError::Unreachable {
            message: format!(
                "listRecords returned HTTP {} for {}",
                response.status().as_u16(),
                peer_did.0
            ),
        });
    }

    let body: serde_json::Value =
        response
            .json()
            .await
            .map_err(|err| PdsError::PeerRecordSchemaInvalid {
                detail: format!("listRecords body is not JSON: {err}"),
            })?;

    let records = body
        .get("records")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let cursor = body
        .get("cursor")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    Ok(RawListPage { records, cursor })
}

/// Strip a trailing `/` from the endpoint so the joined XRPC path does not
/// double-slash.
fn endpoint_base(endpoint: &Url) -> String {
    endpoint.as_str().trim_end_matches('/').to_string()
}

/// Classify a reqwest error into the slice-03 `PdsError` shape. Any
/// transport-level failure (connection refused, dropped socket, DNS) lifts
/// into `Unreachable` so the verb's per-peer fault isolation (PP-7) fires.
fn classify_network_error(err: reqwest::Error) -> PdsError {
    PdsError::Unreachable {
        message: err.to_string(),
    }
}

// -----------------------------------------------------------------------------
// Lexicon-JSON → domain SignedRecord parse
// -----------------------------------------------------------------------------

/// Parse one ATProto record view (`{uri, cid, value}`) into a domain
/// `SignedRecord`. The peer-published `rkey` is the last segment of `uri`
/// (`at://<did>/<collection>/<rkey>`) so the verb can byte-match it against
/// the locally-recomputed CID per WD-24. A real PDS's `cid` is the record's
/// own repo CID, never the openlore claim CID, so it is only a fallback for a
/// view without a `uri`.
fn parse_record_view(peer_did: &Did, view: &serde_json::Value) -> Result<SignedRecord, PdsError> {
    // The listRecords view wraps the claim body under `value`; getRecord
    // returns the same shape. Fall back to the top-level object if `value`
    // is absent (defensive — some PDS shapes inline the record).
    let body = view.get("value").unwrap_or(view);
    let rkey = view
        .get("uri")
        .and_then(|u| u.as_str())
        .and_then(|u| u.rsplit('/').next())
        .filter(|rkey| !rkey.is_empty())
        .or_else(|| view.get("cid").and_then(|c| c.as_str()))
        .map(|s| s.to_string())
        .ok_or_else(|| PdsError::PeerRecordSchemaInvalid {
            detail: "record view has neither `uri` nor `cid` to derive the rkey".to_string(),
        })?;

    let record = parse_claim_record(peer_did, body)?;
    Ok(SignedRecord { rkey, record })
}

/// Parse a lexicon-shaped claim JSON body into the domain `ClaimRecord`
/// (ADR-071: app-signed when a `signature` block is present, self-attested
/// otherwise). The CID is recomputed locally; NO trust decision is made here
/// (the verb byte-matches it against `rkey` and runs the provenance verdict).
/// An absent `kid` defaults to the peer's `#org.openlore.application` key.
fn parse_claim_record(peer_did: &Did, body: &serde_json::Value) -> Result<ClaimRecord, PdsError> {
    let fallback_kid = format!("{}#org.openlore.application", peer_did.0);
    claim_domain::decode_claim_record(body, &fallback_kid)
        .map_err(|detail| PdsError::PeerRecordSchemaInvalid { detail })
}

/// Minimal percent-encoding for XRPC query parameters. DIDs carry `:`
/// (reserved in query values); we encode the small reserved set rather
/// than pulling a full urlencoding dependency (matches `peer_resolve`).
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

    /// The url-safe base64 alphabet decodes (`-` and `_`) into the raw
    /// signature bytes of an app-signed record.
    #[test]
    fn signature_bytes_decode_from_url_safe_base64() {
        let peer = Did("did:plc:jeff".to_string());
        let body = serde_json::json!({
            "subject": "s", "predicate": "p", "object": "o", "confidence": 5000,
            "author": "did:plc:jeff#org.openlore.application",
            "composedAt": "2026-05-22T09:18:44Z",
            "signature": {"sig": "-_-_"}
        });
        let ClaimRecord::AppSigned(signed) = parse_claim_record(&peer, &body).expect("parses")
        else {
            panic!("a signature block makes the record app-signed");
        };
        assert_eq!(signed.signature.signature_bytes, vec![0xfb, 0xff, 0xbf]);
        assert_eq!(
            signed.signature.verification_method,
            "did:plc:jeff#org.openlore.application"
        );
    }

    /// A lexicon-shaped claim body parses into a domain SignedClaim whose
    /// unsigned fields map the wire keys (`author` → author_did,
    /// `composedAt` → composed_at) and whose signed_cid is recomputed.
    #[test]
    fn rkey_comes_from_the_uri_not_the_pds_record_cid() {
        // A real PDS's `cid` is the record's own repo CID; the openlore claim
        // CID is the rkey, the last segment of the at-uri.
        let peer = Did("did:plc:jeff".to_string());
        let view = serde_json::json!({
            "uri": "at://did:plc:jeff/org.openlore.claim/bafyreiclaimcid",
            "cid": "bafyreirepocid",
            "value": {
                "subject": "github:rust-lang/rust",
                "predicate": "embodiesPhilosophy",
                "object": "org.openlore.philosophy.memory-safety",
                "confidence": 8500,
                "author": "did:plc:jeff",
                "composedAt": "2026-10-03T18:38:48Z",
                "signature": {"kid": "did:plc:jeff#org.openlore.application", "alg": "EdDSA", "sig": "TWFu"}
            }
        });
        let record = parse_record_view(&peer, &view).expect("a well-formed view parses");
        assert_eq!(record.rkey, "bafyreiclaimcid");
    }

    #[test]
    fn parse_signed_claim_maps_lexicon_wire_to_domain() {
        let peer = Did("did:plc:rachel-test".to_string());
        let body = serde_json::json!({
            "subject": "github:rust-lang/cargo",
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.dependency-pinning",
            "evidence": ["https://github.com/rust-lang/cargo"],
            "confidence": 0.42,
            "author": "did:plc:rachel-test#org.openlore.application",
            "composedAt": "2026-05-22T09:18:44Z",
            "references": [],
            "signature": {
                "kid": "did:plc:rachel-test#org.openlore.application",
                "alg": "EdDSA",
                "sig": "TWFu"
            }
        });
        let ClaimRecord::AppSigned(signed) =
            parse_claim_record(&peer, &body).expect("well-formed body parses")
        else {
            panic!("a signature block makes the record app-signed");
        };
        assert_eq!(signed.unsigned.subject, "github:rust-lang/cargo");
        assert_eq!(
            signed.unsigned.author_did.0,
            "did:plc:rachel-test#org.openlore.application"
        );
        assert_eq!(signed.unsigned.composed_at, "2026-05-22T09:18:44Z");
        assert_eq!(signed.signature.signature_bytes, b"Man");
        assert!(
            signed.signature.signed_cid.0.starts_with('b'),
            "recomputed CID must be a CIDv1 base32-lower string"
        );
    }

    /// ADR-071: a body with no signature block parses as a self-attested
    /// record (the verdict, not the parser, decides whether it is admitted).
    #[test]
    fn a_body_without_a_signature_parses_as_self_attested() {
        let peer = Did("did:plc:rachel-test".to_string());
        let body = serde_json::json!({
            "subject": "s", "predicate": "p", "object": "o",
            "confidence": 2500,
            "author": "did:plc:rachel-test",
            "composedAt": "2026-05-22T09:18:44Z"
        });
        let record = parse_claim_record(&peer, &body).expect("an unsigned body parses");
        assert!(matches!(record, ClaimRecord::SelfAttested(_)));
        assert_eq!(record.unsigned().author_did, peer);
    }
}
