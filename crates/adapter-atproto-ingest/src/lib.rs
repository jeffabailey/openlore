//! `adapter-atproto-ingest` — the indexer-side bounded-PULL ingest adapter.
//!
//! EFFECT shell for the `IngestSourcePort` trait (`crates/ports`). Performs a
//! bounded PULL of PUBLIC `org.openlore.claim` records via the ATProto
//! `com.atproto.repo.listRecords` XRPC (ADR-024). The fetched [`RawRecord`]s
//! flow to the pure `appview_domain::ingest_decision` gate; NO verification
//! happens here.
//!
//! ## Read-only by construction (capability boundary I-AV-5)
//!
//! This adapter holds NO `IdentityPort` / signing key and exposes NO write /
//! sign / publish method — the indexer is signing-incapable. The absence is the
//! design: there is structurally no path from this adapter to authoring or
//! mutating a claim.
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! Pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires an [`AtProtoIngestAdapter`] behind the
//! `IngestSourcePort` interface. The lexicon-JSON → domain `SignedClaim` parse
//! mirrors `adapter-atproto-pds::peer_read` byte-for-byte (the SAME wire shape:
//! `author`/`composedAt`/nested `signature:{kid,alg,sig}`; base64url-no-pad sig).
//
// SCAFFOLD: false  (step 03-01: live bounded-PULL `listRecords` for the AV-1
// walking skeleton; the relay/multi-source + network-lies probe arms are later).

#![allow(dead_code)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use claim_domain::{
    Cid, ClaimReference, Confidence, Did, ReferenceType, SignatureBlock, SignedClaim, UnsignedClaim,
};
use ports::{IngestError, IngestSourcePort, ProbeOutcome, RawRecord};

/// The ATProto collection the indexer pulls (public signed claims).
const CLAIM_COLLECTION: &str = "org.openlore.claim";

/// Bounded read-only PULL `IngestSourcePort` adapter over ATProto XRPC
/// (`listRecords`) — ADR-024.
///
/// READ-ONLY by construction (I-AV-5): holds NO signing identity and no local
/// store handle. Holds a `reqwest` client + the configured source base URL the
/// `enumerate` default reads when no explicit source is passed.
pub struct AtProtoIngestAdapter {
    client: reqwest::Client,
    /// The configured source base URL (a PDS / relay hosting `listRecords`).
    source: String,
}

impl AtProtoIngestAdapter {
    /// Construct the ingest adapter pointed at `source` (a base URL hosting the
    /// public `com.atproto.repo.listRecords` surface).
    pub fn new(source: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            source: source.to_string(),
        }
    }

    /// The configured source base URL.
    pub fn source(&self) -> &str {
        &self.source
    }
}

#[async_trait]
impl IngestSourcePort for AtProtoIngestAdapter {
    fn probe(&self) -> ProbeOutcome {
        // Earned-Trust probe (happy-path arm for the AV-1 walking skeleton): a
        // configured source URL must be present + well-shaped (the real adapter
        // cannot PULL from an empty seed). The network-lies (tampered/CID-mismatch)
        // reachability arms are AV-6/03-06; here we assert a REAL configuration
        // readiness check rather than a trivial `Ok`.
        if self.source.trim().is_empty() {
            return ProbeOutcome::Refused {
                reason: ports::ProbeRefusalReason::PdsTlsHandshakeFailed,
                detail: "ingest source URL is empty — cannot PULL listRecords".to_string(),
                structured: serde_json::json!({"adapter": "ingest_source"}),
            };
        }
        ProbeOutcome::Ok
    }

    async fn enumerate(&self, source: &str) -> Result<Vec<RawRecord>, IngestError> {
        let base = if source.trim().is_empty() {
            self.source.as_str()
        } else {
            source
        };
        if base.trim().is_empty() {
            return Err(IngestError::BadResponse {
                message: "ingest source URL is empty".to_string(),
            });
        }

        let url = format!(
            "{}/xrpc/com.atproto.repo.listRecords?collection={}",
            base.trim_end_matches('/'),
            CLAIM_COLLECTION
        );

        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|err| IngestError::Unreachable {
                    message: format!("listRecords transport error: {err}"),
                })?;

        if !response.status().is_success() {
            return Err(IngestError::BadResponse {
                message: format!("listRecords returned HTTP {}", response.status().as_u16()),
            });
        }

        let body: serde_json::Value =
            response
                .json()
                .await
                .map_err(|err| IngestError::BadResponse {
                    message: format!("listRecords body is not JSON: {err}"),
                })?;

        let records = body
            .get("records")
            .and_then(|r| r.as_array())
            .ok_or_else(|| IngestError::BadResponse {
                message: "listRecords response missing `records` array".to_string(),
            })?;

        records
            .iter()
            .map(parse_record_view)
            .collect::<Result<Vec<_>, _>>()
    }
}

// -----------------------------------------------------------------------------
// Lexicon-JSON → domain RawRecord parse (mirrors adapter-atproto-pds::peer_read)
// -----------------------------------------------------------------------------

/// Parse one ATProto record view (`{uri, cid, value}`) into a `RawRecord`. The
/// network-published `cid` becomes `RawRecord::published_cid` (recomputed +
/// verified by the pure gate); `value` is the lexicon claim body. NO trust
/// decision is made here — the gate verifies the signature + recomputes the CID.
fn parse_record_view(view: &serde_json::Value) -> Result<RawRecord, IngestError> {
    let bad = |detail: String| IngestError::BadResponse { message: detail };

    let body = view.get("value").unwrap_or(view);
    let published_cid = view
        .get("cid")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| bad("record view missing `cid`".to_string()))?;

    let signed = parse_signed_claim(body)?;
    Ok(RawRecord {
        published_cid: Cid(published_cid),
        raw_payload: signed,
        source_pds: view
            .get("uri")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default(),
    })
}

/// Parse a lexicon-shaped claim JSON body into the domain `SignedClaim`. Maps the
/// wire field names (`author`, `composedAt`, nested `signature:{kid,alg,sig}`)
/// onto the domain ADT; the signature `sig` is base64url-no-pad decoded into raw
/// bytes. The unsigned-CID is recomputed locally so the `SignedClaim` is
/// self-consistent — but NO trust decision is made (the gate verifies).
fn parse_signed_claim(body: &serde_json::Value) -> Result<SignedClaim, IngestError> {
    let bad = |detail: String| IngestError::BadResponse { message: detail };

    let subject = required_str(body, "subject").map_err(bad)?;
    let predicate = required_str(body, "predicate").map_err(bad)?;
    let object = required_str(body, "object").map_err(bad)?;
    let author = required_str(body, "author").map_err(bad)?;
    let composed_at = required_str(body, "composedAt").map_err(bad)?;

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
        .ok_or_else(|| bad("confidence missing or not a number".to_string()))?;
    let confidence: Confidence = serde_json::from_value(serde_json::json!(confidence_value))
        .map_err(|err| bad(format!("confidence did not deserialize: {err}")))?;

    let references = parse_references(body).map_err(bad)?;
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

    let sig_obj = body
        .get("signature")
        .and_then(|s| s.as_object())
        .ok_or_else(|| bad("signature block missing".to_string()))?;
    let sig_b64 = sig_obj
        .get("sig")
        .and_then(|v| v.as_str())
        .ok_or_else(|| bad("signature.sig missing".to_string()))?;
    let signature_bytes = base64url_no_pad_decode(sig_b64)
        .map_err(|e| bad(format!("signature.sig is not base64url: {e}")))?;
    let verification_method = sig_obj
        .get("kid")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let canonical = claim_domain::canonicalize(&unsigned)
        .map_err(|e| bad(format!("canonicalize ingested claim: {e}")))?;
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

/// Parse the optional `references[]` array (`{type, cid}` entries).
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

/// base64url-no-pad decode (the lexicon `signature.sig` wire encoding per
/// ADR-006). MUST agree byte-for-byte with the acceptance harness encoder + the
/// `adapter-atproto-pds::peer_read` decoder.
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

    /// The lexicon → RawRecord parse maps the wire fields onto the domain ADT and
    /// preserves the published CID + the author (the inner-loop contract the
    /// bounded PULL relies on; AV-1).
    #[test]
    fn parse_record_view_maps_wire_fields_to_raw_record() {
        let view = serde_json::json!({
            "uri": "at://did:plc:priya-test/org.openlore.claim/bafyabc",
            "cid": "bafyabc",
            "value": {
                "subject": "github:bazelbuild/bazel",
                "predicate": "embodiesPhilosophy",
                "object": "org.openlore.philosophy.reproducible-builds",
                "evidence": ["https://example.test/e"],
                "confidence": 0.82,
                "author": "did:plc:priya-test#org.openlore.application",
                "composedAt": "2026-05-26T12:00:00Z",
                "references": [],
                "signature": { "kid": "did:plc:priya-test#org.openlore.application", "alg": "EdDSA", "sig": "AAAA" }
            }
        });
        let record = parse_record_view(&view).expect("parse well-formed record view");
        assert_eq!(record.published_cid.0, "bafyabc");
        assert_eq!(
            record.raw_payload.unsigned.author_did.0,
            "did:plc:priya-test#org.openlore.application"
        );
        assert_eq!(
            record.raw_payload.unsigned.subject,
            "github:bazelbuild/bazel"
        );
    }
}
