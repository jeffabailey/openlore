//! Property tests for the pure publish core. Each property enters through a
//! public function (the pure port) and asserts on its returned value.

use super::*;
use claim_domain::proptest_strategies::arb_unsigned_claim;
use claim_domain::{Confidence, SignatureBlock, UnsignedClaim};
use proptest::prelude::*;

/// Confidence values biased toward the f16-representable gold values
/// (`0.0`/`0.5`/`1.0`) that a re-encoding transport would corrupt (SPIKE-00).
fn arb_confidence() -> impl Strategy<Value = f64> {
    prop_oneof![Just(0.0), Just(0.5), Just(1.0), 0.0_f64..=1.0_f64]
}

fn with_confidence(mut claim: UnsignedClaim, confidence: f64) -> UnsignedClaim {
    let confidence: Confidence =
        serde_json::from_value(serde_json::json!(confidence)).expect("confidence in range");
    claim.confidence = confidence;
    claim
}

/// A signed claim whose `signed_cid` is the real Rust-minted CID. The
/// signature bytes are arbitrary: the transport never verifies them.
fn arb_signed_claim() -> impl Strategy<Value = SignedClaim> {
    (
        arb_unsigned_claim(),
        arb_confidence(),
        prop::collection::vec(any::<u8>(), 64),
    )
        .prop_map(|(claim, confidence, signature_bytes)| {
            let unsigned = with_confidence(claim, confidence);
            let signed_cid =
                claim_domain::compute_cid(&claim_domain::canonicalize(&unsigned).expect("canon"));
            SignedClaim {
                signature: SignatureBlock {
                    signed_cid,
                    signature_bytes,
                    verification_method: format!(
                        "{}#org.openlore.application",
                        unsigned.author_did.0
                    ),
                },
                unsigned,
            }
        })
}

/// Arbitrary JSON (depth-bounded) — including objects that carry an
/// `openlore` key with the wrong shape.
fn arb_json() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        "[a-z-]{0,16}".prop_map(serde_json::Value::from),
        Just(serde_json::Value::from(OPAQUE_INSTANCE_KIND)),
    ];
    leaf.prop_recursive(3, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
            prop::collection::btree_map(
                prop_oneof![
                    Just("openlore".to_string()),
                    Just("kind".to_string()),
                    Just("contract_version".to_string()),
                    "[a-z]{1,8}"
                ],
                inner,
                0..4
            )
            .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
        ]
    })
}

fn with_marker(mut manifest: serde_json::Value, contract_version: u64) -> serde_json::Value {
    let marker =
        serde_json::json!({ "kind": OPAQUE_INSTANCE_KIND, "contract_version": contract_version });
    match manifest.as_object_mut() {
        Some(object) => {
            object.insert("openlore".to_string(), marker);
            manifest
        }
        None => serde_json::json!({ "openlore": marker, "records": [] }),
    }
}

fn is_well_formed_marker(manifest: &serde_json::Value) -> bool {
    let envelope = manifest.get("openlore");
    envelope
        .and_then(|e| e.get("kind"))
        .and_then(|k| k.as_str())
        == Some(OPAQUE_INSTANCE_KIND)
        && envelope
            .and_then(|e| e.get("contract_version"))
            .and_then(serde_json::Value::as_u64)
            .is_some()
}

proptest! {
    /// Q-SF-D5: classification is TOTAL and marker-driven — any JSON is
    /// classified without panicking; it is an opaque instance exactly when it
    /// carries the well-formed openlore envelope, whose version is reported.
    #[test]
    fn classification_is_total_and_driven_only_by_the_openlore_marker(
        manifest in arb_json(),
        contract_version in any::<u64>(),
    ) {
        prop_assert_eq!(
            matches!(classify_instance(&manifest), InstanceKind::OpaqueInstance { .. }),
            is_well_formed_marker(&manifest)
        );
        prop_assert_eq!(
            classify_instance(&with_marker(manifest, contract_version)),
            InstanceKind::OpaqueInstance { contract_version }
        );
    }

    /// The verdict is `Verified` iff the pushed CID equals the recomputed CID.
    #[test]
    fn verdict_is_verified_iff_pushed_equals_recomputed(
        pushed in "bafy[a-z2-7]{4}",
        recomputed in "bafy[a-z2-7]{4}",
        same in any::<bool>(),
    ) {
        let pushed = Cid(pushed);
        let recomputed = if same { pushed.clone() } else { Cid(recomputed) };
        let verdict = round_trip_verdict(&pushed, &recomputed);
        prop_assert_eq!(
            matches!(verdict, RoundTripVerdict::Verified { .. }),
            pushed == recomputed
        );
    }

    /// The manifest v1 display projection copies author_did / subject /
    /// predicate / object / confidence / composed_at VERBATIM (0.0/0.5/1.0
    /// included) under the claim's own CID.
    #[test]
    fn display_projection_preserves_the_claim_fields_verbatim(signed in arb_signed_claim()) {
        let entry = display_projection(&signed);
        let claim = &signed.unsigned;
        prop_assert_eq!(&entry.cid, &signed.signature.signed_cid.0);
        prop_assert_eq!(&entry.author_did, &claim.author_did.0);
        prop_assert_eq!(&entry.subject, &claim.subject);
        prop_assert_eq!(&entry.predicate, &claim.predicate);
        prop_assert_eq!(&entry.object, &claim.object);
        prop_assert_eq!(entry.confidence.to_bits(), claim.confidence.value().to_bits());
        prop_assert_eq!(&entry.composed_at, &claim.composed_at);
    }

    /// KPI-SF-1 at the core: the verbatim record blob of ANY signed claim
    /// (confidence 0.0/0.5/1.0 included) recomputes to the CID it is pushed
    /// under; flipping the pushed CID yields a mismatch.
    #[test]
    fn a_verbatim_record_blob_round_trips_to_its_own_cid(signed in arb_signed_claim()) {
        let pushed = signed.signature.signed_cid.clone();
        let blob = record_bytes_of(&signed);
        prop_assert_eq!(
            verify_round_trip(&pushed, &blob),
            Ok(RoundTripVerdict::Verified { cid: pushed.clone() })
        );
        let wrong = Cid(format!("{}x", pushed.0));
        prop_assert_eq!(
            verify_round_trip(&wrong, &blob),
            Ok(RoundTripVerdict::CidMismatch { pushed: wrong.clone(), recomputed: pushed })
        );
    }
}

/// Manifest v1 parse: the marker + records[] entries surface verbatim; a
/// body without the marker is refused as not-an-openlore-instance.
#[test]
fn read_manifest_returns_entries_only_for_a_marked_manifest() {
    let entry = serde_json::json!({
        "cid": "bafyone", "author_did": "did:plc:maria-test", "subject": "github:a/b",
        "predicate": "embodiesPhilosophy", "object": "org.openlore.philosophy.x",
        "confidence": 0.5, "composed_at": "2026-05-25T12:00:00Z"
    });
    let marked = serde_json::json!({
        "openlore": { "kind": "opaque-instance", "contract_version": 1 },
        "records": [entry]
    });
    let manifest = read_manifest(&marked).expect("marked manifest parses");
    assert_eq!(manifest.contract_version, 1);
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(manifest.entries[0].cid, "bafyone");
    assert_eq!(manifest.entries[0].confidence, 0.5);

    let unmarked = serde_json::json!({ "records": [] });
    assert!(matches!(
        read_manifest(&unmarked),
        Err(InstanceError::NotAnOpenloreInstance { .. })
    ));
}
