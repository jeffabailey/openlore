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

/// A `/manifest` response body: arbitrary JSON (serialized), arbitrary
/// bytes (usually not JSON), or an ordinary HTML page.
fn arb_manifest_body() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        arb_json().prop_map(|json| json.to_string().into_bytes()),
        prop::collection::vec(any::<u8>(), 0..64),
        Just(b"<!doctype html><title>my blog</title><p>hello</p>".to_vec()),
    ]
}

/// Does this response carry the marker a registration requires (2xx + JSON +
/// well-formed openlore envelope)?
fn carries_the_marker(status: u16, body: &[u8]) -> bool {
    (200..300).contains(&status)
        && serde_json::from_slice::<serde_json::Value>(body)
            .map(|json| is_well_formed_marker(&json))
            .unwrap_or(false)
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

    /// Q-SF-D5 at the probe boundary: a response that is not a 2xx JSON body
    /// carrying the openlore envelope (HTML, non-JSON bytes, marker-less JSON,
    /// any non-2xx status) NEVER classifies as an openlore instance — it is
    /// refused as not-an-openlore-instance, never as unreachable.
    #[test]
    fn a_response_without_the_marker_is_never_an_openlore_instance(
        status in 100_u16..600,
        body in arb_manifest_body(),
    ) {
        prop_assume!(!carries_the_marker(status, &body));
        let classified =
            classify_manifest_observation(&ManifestObservation::Responded { status, body });
        prop_assert!(
            matches!(classified, Err(InstanceError::NotAnOpenloreInstance { .. })),
            "expected NotAnOpenloreInstance, got {:?}", classified
        );
    }

    /// Refusal ordering: an observation with no HTTP response is ALWAYS
    /// unreachable (checked before any marker), while a 2xx marked manifest
    /// is ALWAYS an instance, whatever else it carries.
    #[test]
    fn unreachable_precedes_the_marker_check_and_a_marked_manifest_registers(
        detail in "[a-z ]{0,24}",
        status in 200_u16..300,
        manifest in arb_json(),
        contract_version in any::<u64>(),
    ) {
        let url = "https://openlore.maria.workers.dev".to_string();
        let unreachable = classify_manifest_observation(&ManifestObservation::Unreachable {
            url: url.clone(),
            detail: detail.clone(),
        });
        prop_assert_eq!(unreachable, Err(InstanceError::Unreachable { url, detail }));

        let mut marked = with_marker(manifest, contract_version);
        if let Some(object) = marked.as_object_mut() {
            object.insert("records".to_string(), serde_json::json!([]));
        }
        let body = marked.to_string().into_bytes();
        let classified =
            classify_manifest_observation(&ManifestObservation::Responded { status, body });
        prop_assert_eq!(
            classified.map(|manifest| manifest.contract_version),
            Ok(contract_version)
        );
    }

    /// The public card lives at the instance root: `card_url` is the
    /// instance URL with exactly one trailing `/`, however many it was given.
    #[test]
    fn card_url_is_the_instance_root_with_one_trailing_slash(
        base in "https?://[a-z0-9.-]{1,24}(:[0-9]{2,5})?",
        trailing_slashes in 0_usize..3,
    ) {
        let instance_url = format!("{base}{}", "/".repeat(trailing_slashes));
        prop_assert_eq!(card_url(&instance_url), format!("{base}/"));
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

    /// Verify-before-commit (RT-2, KPI-SF-1): a read-back is accepted iff it
    /// is byte-identical to what was sent — the verbatim blob is accepted
    /// under its own CID, and ANY single-byte perturbation of it (anywhere:
    /// claim fields, JSON structure, the signature block) is refused.
    #[test]
    fn a_readback_is_accepted_iff_it_is_the_verbatim_blob(
        signed in arb_signed_claim(),
        position in any::<prop::sample::Index>(),
        flip in 1_u8..=255,
    ) {
        let pushed = signed.signature.signed_cid.clone();
        let sent = record_bytes_of(&signed);
        prop_assert_eq!(judge_readback(&pushed, &sent, &sent), Ok(pushed.clone()));

        let mut perturbed = sent.0.clone();
        let at = position.index(perturbed.len());
        perturbed[at] ^= flip;
        let verdict = judge_readback(&pushed, &sent, &RecordBytes(perturbed));
        prop_assert!(
            verdict.is_err(),
            "a byte perturbed at {} was accepted: {:?}", at, verdict
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
