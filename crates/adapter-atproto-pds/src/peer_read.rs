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
//! 2. Parses each returned record's `value` (lexicon JSON) into the
//!    domain `SignedRecord` ADT, carrying the peer-published `rkey`.
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

use ports::claim_domain::{
    self, Cid, ClaimReference, Confidence, Did, ReferenceType, SignatureBlock, SignedClaim,
    UnsignedClaim,
};
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
/// `SignedRecord`. The peer-published `rkey` is taken from `cid` (the
/// listRecords view echoes the rkey as `cid`) so the verb can byte-match
/// it against the locally-recomputed CID per WD-24.
fn parse_record_view(peer_did: &Did, view: &serde_json::Value) -> Result<SignedRecord, PdsError> {
    // The listRecords view wraps the claim body under `value`; getRecord
    // returns the same shape. Fall back to the top-level object if `value`
    // is absent (defensive — some PDS shapes inline the record).
    let body = view.get("value").unwrap_or(view);
    let rkey = view
        .get("cid")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            view.get("uri")
                .and_then(|u| u.as_str())
                .and_then(|u| u.rsplit('/').next())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| PdsError::PeerRecordSchemaInvalid {
            detail: "record view has neither `cid` nor `uri` to derive the rkey".to_string(),
        })?;

    let signed_claim = parse_signed_claim(peer_did, body)?;
    Ok(SignedRecord { rkey, signed_claim })
}

/// Parse a lexicon-shaped claim JSON body into the domain `SignedClaim`.
///
/// Maps the wire field names (`author`, `composedAt`, nested
/// `signature: {kid, alg, sig}`) onto the domain ADT. The signature `sig`
/// is base64url-no-pad decoded into raw bytes. The unsigned-CID is
/// recomputed locally via `canonicalize` + `compute_cid` so the returned
/// `SignedClaim.signature.signed_cid` is well-formed — but NO trust
/// decision is made here (the verb byte-matches it against `rkey` and runs
/// `verify`).
fn parse_signed_claim(peer_did: &Did, body: &serde_json::Value) -> Result<SignedClaim, PdsError> {
    let invalid = |detail: String| PdsError::PeerRecordSchemaInvalid { detail };

    let subject = required_str(body, "subject").map_err(invalid)?;
    let predicate = required_str(body, "predicate").map_err(invalid)?;
    let object = required_str(body, "object").map_err(invalid)?;
    let author = required_str(body, "author").map_err(invalid)?;
    let composed_at = required_str(body, "composedAt").map_err(invalid)?;

    let evidence: Vec<String> = body
        .get("evidence")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let confidence_value = body
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| invalid("confidence missing or not a number".to_string()))?;
    // `Confidence`'s inner field is crate-private; route through serde
    // (the wrapper serializes transparently to its inner number).
    let confidence: Confidence = serde_json::from_value(serde_json::json!(confidence_value))
        .map_err(|err| invalid(format!("confidence did not deserialize: {err}")))?;

    let references = parse_references(body).map_err(invalid)?;
    let reason = body
        .get("reason")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string());

    let unsigned = UnsignedClaim {
        subject,
        predicate,
        object,
        evidence,
        confidence,
        author_did: Did(author),
        composed_at,
        references,
        reason,
    };

    // Signature block: decode `sig` base64url-no-pad → raw bytes; carry the
    // `kid` as the verification method.
    let sig_obj = body
        .get("signature")
        .and_then(|s| s.as_object())
        .ok_or_else(|| invalid("signature block missing".to_string()))?;
    let sig_b64 = sig_obj
        .get("sig")
        .and_then(|v| v.as_str())
        .ok_or_else(|| invalid("signature.sig missing".to_string()))?;
    let signature_bytes = base64url_no_pad_decode(sig_b64)
        .map_err(|e| invalid(format!("signature.sig is not base64url: {e}")))?;
    let verification_method = sig_obj
        .get("kid")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}#org.openlore.application", peer_did.0));

    // Recompute the unsigned-CID locally so the returned SignedClaim is
    // self-consistent. NO trust decision — the verb byte-matches this
    // against the published rkey (WD-24).
    let canonical = claim_domain::canonicalize(&unsigned)
        .map_err(|e| invalid(format!("canonicalize peer claim: {e}")))?;
    let signed_cid: Cid = claim_domain::compute_cid(&canonical);

    Ok(SignedClaim {
        unsigned,
        signature: SignatureBlock {
            signed_cid,
            signature_bytes,
            verification_method,
        },
    })
}

/// Parse the optional `references[]` array (`{type, cid}` entries) into
/// the domain `ClaimReference` list.
fn parse_references(body: &serde_json::Value) -> Result<Vec<ClaimReference>, String> {
    let Some(arr) = body.get("references").and_then(|r| r.as_array()) else {
        return Ok(Vec::new());
    };
    arr.iter()
        .map(|entry| {
            let type_str = entry
                .get("type")
                .and_then(|t| t.as_str())
                .ok_or_else(|| "reference entry missing `type`".to_string())?;
            let cid = entry
                .get("cid")
                .and_then(|c| c.as_str())
                .ok_or_else(|| "reference entry missing `cid`".to_string())?;
            let ref_type = match type_str {
                "retracts" => ReferenceType::Retracts,
                "corrects" => ReferenceType::Corrects,
                "counters" => ReferenceType::Counters,
                "supersedes" => ReferenceType::Supersedes,
                other => return Err(format!("unknown reference type `{other}`")),
            };
            Ok(ClaimReference {
                ref_type,
                cid: Cid(cid.to_string()),
            })
        })
        .collect()
}

/// Extract a required string field by `key`, naming it on absence.
fn required_str(body: &serde_json::Value, key: &str) -> Result<String, String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("required field `{key}` missing or not a string"))
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

/// base64url-no-pad decode (the lexicon `signature.sig` wire encoding per
/// ADR-006). Hand-rolled so the adapter does not pull a base64 crate; MUST
/// agree byte-for-byte with the encoder the acceptance harness uses.
fn base64url_no_pad_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'-' => Ok(62),
            b'_' => Ok(63),
            other => Err(format!("invalid base64url char {:?}", other as char)),
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        let mut bits = 0;
        for &c in chunk {
            acc = (acc << 6) | val(c)?;
            bits += 6;
        }
        // Emit the full bytes accumulated (drop the leftover < 8 bits).
        while bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// base64url round-trip across the byte domain (0..=255 covered via a
    /// representative vector including the high bit). The encoder lives in
    /// the acceptance harness; this pins the decoder contract so a drift
    /// reds here, not silently in a subprocess pull.
    #[test]
    fn base64url_decode_roundtrips_known_vectors() {
        // "Man" → "TWFu"; "Ma" → "TWE"; "M" → "TQ" (no padding).
        assert_eq!(base64url_no_pad_decode("TWFu").unwrap(), b"Man");
        assert_eq!(base64url_no_pad_decode("TWE").unwrap(), b"Ma");
        assert_eq!(base64url_no_pad_decode("TQ").unwrap(), b"M");
        // url-safe alphabet: 0xFB,0xFF,0xBF encodes with `-` and `_`.
        assert_eq!(
            base64url_no_pad_decode("-_-_").unwrap(),
            vec![0xfb, 0xff, 0xbf]
        );
    }

    /// A lexicon-shaped claim body parses into a domain SignedClaim whose
    /// unsigned fields map the wire keys (`author` → author_did,
    /// `composedAt` → composed_at) and whose signed_cid is recomputed.
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
        let signed = parse_signed_claim(&peer, &body).expect("well-formed body parses");
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

    /// A body missing the signature block fails to parse with a structured
    /// schema-invalid error (never a panic).
    #[test]
    fn parse_signed_claim_rejects_missing_signature() {
        let peer = Did("did:plc:rachel-test".to_string());
        let body = serde_json::json!({
            "subject": "s", "predicate": "p", "object": "o",
            "confidence": 0.5,
            "author": "did:plc:rachel-test#org.openlore.application",
            "composedAt": "2026-05-22T09:18:44Z"
        });
        let err = parse_signed_claim(&peer, &body).expect_err("missing signature must reject");
        assert!(matches!(err, PdsError::PeerRecordSchemaInvalid { .. }));
    }
}
