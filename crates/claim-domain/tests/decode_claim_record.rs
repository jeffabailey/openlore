//! ADR-071 shared decode + record origin, asserted at the crate boundary
//! (DELIVER Phase 5: these behaviours were exercised only from consumer
//! crates, so in-crate mutants of `decode_claim_record` and
//! `RecordOrigin::of` survived).

use claim_domain::{decode_claim_record, ClaimRecord, RecordOrigin, ReferenceType};
use proptest::prelude::*;
use serde_json::{json, Value};

const OWNER: &str = "did:plc:owner";
const FALLBACK_KID: &str = "did:plc:owner#org.openlore.application";

/// Reference base64url-no-pad encoder (RFC 4648 §5, no `=`).
fn base64url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    bytes
        .chunks(3)
        .flat_map(|chunk| {
            let padded = [
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
            ];
            let group =
                (u32::from(padded[0]) << 16) | (u32::from(padded[1]) << 8) | u32::from(padded[2]);
            let sextets = chunk.len() + 1;
            (0..sextets).map(move |i| ALPHABET[((group >> (18 - 6 * i)) & 0x3f) as usize] as char)
        })
        .collect()
}

fn claim_body() -> Value {
    json!({
        "subject": "github:priyaraman/tidepool",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.memory-safety",
        "evidence": ["https://github.com/priyaraman/tidepool/"],
        "confidence": 2500,
        "author": OWNER,
        "composedAt": "2026-10-04T15:02:11Z",
        "references": [],
    })
}

fn signed_body(sig: &str, kid: Option<&str>) -> Value {
    let mut body = claim_body();
    let mut signature = json!({"alg": "EdDSA", "sig": sig});
    if let Some(kid) = kid {
        signature["kid"] = json!(kid);
    }
    body["signature"] = signature;
    body
}

fn signature_bytes(record: ClaimRecord) -> Vec<u8> {
    match record {
        ClaimRecord::AppSigned(signed) => signed.signature.signature_bytes,
        ClaimRecord::SelfAttested(_) => panic!("a signature block makes the record app-signed"),
    }
}

proptest! {
    /// Universe: every byte string up to 96 bytes. The `sig` of a
    /// signature block decodes to exactly the bytes it encodes.
    #[test]
    fn a_signature_decodes_to_the_bytes_it_encodes(bytes in proptest::collection::vec(any::<u8>(), 0..96)) {
        let record = decode_claim_record(&signed_body(&base64url_no_pad(&bytes), None), FALLBACK_KID)
            .expect("well-formed signed claim decodes");
        prop_assert_eq!(signature_bytes(record), bytes);
    }

    /// Universe: every claim field taken from a small alphabet. The decoded
    /// claim carries each field exactly as the record wrote it.
    #[test]
    fn a_decoded_claim_carries_every_field_as_written(
        subject in "github:[a-z]{1,8}/[a-z]{1,8}",
        object in "org\\.openlore\\.philosophy\\.[a-z-]{1,12}",
        confidence_bp in 0i64..=10_000,
        composed_at in "2026-10-0[1-9]T1[0-9]:0[0-9]:00Z",
    ) {
        let mut body = claim_body();
        body["subject"] = json!(subject);
        body["object"] = json!(object);
        body["confidence"] = json!(confidence_bp);
        body["composedAt"] = json!(composed_at);
        let record = decode_claim_record(&body, FALLBACK_KID).expect("decodes");
        let claim = record.unsigned();
        prop_assert_eq!(&claim.subject, &subject);
        prop_assert_eq!(claim.predicate.as_str(), "embodiesPhilosophy");
        prop_assert_eq!(&claim.object, &object);
        prop_assert_eq!(claim.confidence.basis_points(), confidence_bp);
        prop_assert_eq!(claim.author_did.0.as_str(), OWNER);
        prop_assert_eq!(&claim.composed_at, &composed_at);
    }
}

#[test]
fn a_record_without_a_signature_is_self_attested() {
    let record = decode_claim_record(&claim_body(), FALLBACK_KID).expect("decodes");
    assert!(matches!(record, ClaimRecord::SelfAttested(_)));
}

#[test]
fn every_required_field_must_be_a_string() {
    for field in ["subject", "predicate", "object", "author", "composedAt"] {
        let mut missing = claim_body();
        missing.as_object_mut().expect("object").remove(field);
        assert!(
            decode_claim_record(&missing, FALLBACK_KID).is_err(),
            "{field} missing"
        );
        let mut not_a_string = claim_body();
        not_a_string[field] = json!(7);
        assert!(
            decode_claim_record(&not_a_string, FALLBACK_KID).is_err(),
            "{field} not a string"
        );
    }
}

#[test]
fn references_are_decoded_in_order_with_their_type() {
    let mut body = claim_body();
    body["references"] = json!([
        {"type": "retracts", "cid": "bafyretracted"},
        {"type": "supersedes", "cid": "bafysuperseded"},
    ]);
    let record = decode_claim_record(&body, FALLBACK_KID).expect("decodes");
    let references: Vec<(ReferenceType, &str)> = record
        .unsigned()
        .references
        .iter()
        .map(|reference| (reference.ref_type, reference.cid.0.as_str()))
        .collect();
    assert_eq!(
        references,
        vec![
            (ReferenceType::Retracts, "bafyretracted"),
            (ReferenceType::Supersedes, "bafysuperseded"),
        ]
    );
}

#[test]
fn an_unknown_reference_type_is_unreadable() {
    let mut body = claim_body();
    body["references"] = json!([{"type": "endorses", "cid": "bafyx"}]);
    assert!(decode_claim_record(&body, FALLBACK_KID).is_err());
}

#[test]
fn a_signature_outside_the_base64url_alphabet_is_unreadable() {
    for sig in ["AA+A", "AA/A", "AA=A", "AA A"] {
        assert!(
            decode_claim_record(&signed_body(sig, None), FALLBACK_KID).is_err(),
            "{sig}"
        );
    }
}

#[test]
fn the_signing_key_defaults_to_the_fallback_only_when_absent() {
    let verification_method =
        |kid: Option<&str>| match decode_claim_record(&signed_body("AAAA", kid), FALLBACK_KID)
            .expect("decodes")
        {
            ClaimRecord::AppSigned(signed) => signed.signature.verification_method,
            ClaimRecord::SelfAttested(_) => panic!("signed"),
        };
    assert_eq!(verification_method(None), FALLBACK_KID);
    assert_eq!(
        verification_method(Some("did:plc:other#k")),
        "did:plc:other#k"
    );
}

#[test]
fn a_record_is_from_the_author_pds_only_on_an_exact_base_url_match() {
    let pds = "https://pds.example.com";
    assert_eq!(RecordOrigin::of(pds, pds), RecordOrigin::AuthorPds);
    assert_eq!(
        RecordOrigin::of("https://pds.example.com/", pds),
        RecordOrigin::AuthorPds
    );
    assert_eq!(
        RecordOrigin::of("https://relay.example.com", pds),
        RecordOrigin::Relay
    );
    assert_eq!(
        RecordOrigin::of("https://pds.example.com.evil", pds),
        RecordOrigin::Relay
    );
}
