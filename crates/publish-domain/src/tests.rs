//! Property tests for the pure publish core. Each property enters through a
//! public function (the pure port) and asserts on its returned value.

use super::*;
use claim_domain::proptest_strategies::arb_unsigned_claim;
use claim_domain::{Confidence, SignatureBlock, UnsignedClaim};
use proptest::prelude::*;
use std::collections::BTreeSet;

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

/// A push-plan scenario: distinct local CIDs (in local order) and the
/// instance's manifest CIDs — some of the local ones (a prior push) plus
/// records the local store does not hold (pushed from elsewhere).
fn arb_local_and_remote() -> impl Strategy<Value = (Vec<Cid>, Vec<Cid>)> {
    (
        prop::collection::btree_set("bafy[a-z2-7]{6}", 0..40),
        prop::collection::btree_set("bafz[a-z2-7]{6}", 0..8),
        any::<u64>(),
    )
        .prop_flat_map(|(local, foreign, shuffle_seed)| {
            let local: Vec<String> = local.into_iter().collect();
            let len = local.len();
            (
                Just(local),
                prop::collection::vec(any::<bool>(), len),
                Just(foreign),
                Just(shuffle_seed),
            )
        })
        .prop_map(|(local, on_remote, foreign, shuffle_seed)| {
            let remote = local
                .iter()
                .zip(&on_remote)
                .filter(|(_, on)| **on)
                .map(|(cid, _)| cid.clone())
                .chain(foreign)
                .collect::<Vec<_>>();
            let rotate = if remote.is_empty() {
                0
            } else {
                (shuffle_seed as usize) % remote.len()
            };
            let mut remote = remote;
            remote.rotate_left(rotate);
            (
                local.into_iter().map(Cid).collect(),
                remote.into_iter().map(Cid).collect(),
            )
        })
}

fn cid_set(cids: &[Cid]) -> BTreeSet<String> {
    cids.iter().map(|cid| cid.0.clone()).collect()
}

proptest! {
    /// Q-SF-D3 partition: every local claim is either pushed or skipped
    /// (never lost, never both); nothing already on the instance is re-sent;
    /// only claims the instance already lists are skipped.
    #[test]
    fn the_push_plan_partitions_local_into_missing_and_already_present(
        (local, remote) in arb_local_and_remote(),
    ) {
        let plan = plan_push(&local, &remote);
        let (to_push, skipped, remote) =
            (cid_set(&plan.to_push), cid_set(&plan.skipped), cid_set(&remote));
        prop_assert_eq!(to_push.union(&skipped).cloned().collect::<BTreeSet<_>>(), cid_set(&local));
        prop_assert!(to_push.is_disjoint(&skipped));
        prop_assert!(to_push.is_disjoint(&remote));
        prop_assert!(skipped.is_subset(&remote));
        prop_assert_eq!(plan.to_push.len() + plan.skipped.len(), local.len());
    }

    /// Deterministic + stable: the plan keeps local order in both halves and
    /// does not depend on the order the manifest lists its records in.
    #[test]
    fn the_push_plan_keeps_local_order_whatever_the_manifest_order(
        (local, remote) in arb_local_and_remote(),
    ) {
        let plan = plan_push(&local, &remote);
        let mut reversed_remote = remote.clone();
        reversed_remote.reverse();
        prop_assert_eq!(&plan_push(&local, &reversed_remote), &plan);
        let local_order = |half: &[Cid]| {
            local.iter().filter(|cid| half.contains(cid)).cloned().collect::<Vec<_>>()
        };
        prop_assert_eq!(&plan.to_push, &local_order(&plan.to_push));
        prop_assert_eq!(&plan.skipped, &local_order(&plan.skipped));
    }

    /// Idempotence (PP-2/PP-3): once the plan has been applied (the instance
    /// now also lists every pushed CID), re-planning pushes nothing and skips
    /// every local claim.
    #[test]
    fn re_planning_after_applying_the_plan_pushes_nothing(
        (local, remote) in arb_local_and_remote(),
    ) {
        let plan = plan_push(&local, &remote);
        let applied: Vec<Cid> = remote.iter().chain(&plan.to_push).cloned().collect();
        let replan = plan_push(&local, &applied);
        prop_assert!(replan.to_push.is_empty(), "re-plan still pushes {:?}", replan.to_push);
        prop_assert_eq!(replan.skipped, local);
    }
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

// -----------------------------------------------------------------------------
// Pull reconcile (US-SF-004, D-6): classify every pulled record against the
// local set — Matched / New / Conflict / Rejected. There is no Overwrite.
// -----------------------------------------------------------------------------

/// A small identity pool so generated local and pulled sets collide often.
fn arb_identity() -> impl Strategy<Value = RecordIdentity> {
    (0_u8..4, 0_u8..3).prop_map(|(subject, object)| RecordIdentity {
        author_did: "did:plc:maria-test".to_string(),
        subject: format!("github:maria/project-{subject}"),
        predicate: "embodiesPhilosophy".to_string(),
        object: format!("org.openlore.philosophy.p{object}"),
    })
}

/// A small CID pool so pulled keys often coincide with local CIDs.
fn arb_cid() -> impl Strategy<Value = Cid> {
    (0_u8..8).prop_map(|n| Cid(format!("bafy{n}")))
}

fn arb_local_claims() -> impl Strategy<Value = Vec<LocalClaim>> {
    prop::collection::vec(
        (arb_cid(), arb_identity()).prop_map(|(cid, identity)| LocalClaim { cid, identity }),
        0..8,
    )
}

fn arb_pulled_record() -> impl Strategy<Value = PulledRecord> {
    prop_oneof![
        3 => (arb_cid(), arb_identity()).prop_map(|(cid, identity)| PulledRecord::Recomputed {
            key: cid.clone(),
            recomputed: cid,
            identity,
        }),
        1 => (arb_cid(), arb_cid(), arb_identity()).prop_map(|(key, recomputed, identity)| {
            PulledRecord::Recomputed { key, recomputed, identity }
        }),
        1 => (arb_cid(), "[a-z ]{0,12}")
            .prop_map(|(key, detail)| PulledRecord::Unreadable { key, detail }),
    ]
}

/// A local claim as it reads back from an instance that holds it verbatim.
fn as_pulled(local: &LocalClaim) -> PulledRecord {
    PulledRecord::Recomputed {
        key: local.cid.clone(),
        recomputed: local.cid.clone(),
        identity: local.identity.clone(),
    }
}

proptest! {
    /// Classification PARTITIONS the pulled set: one outcome per pulled
    /// record, in pulled order, and the tally sums to the pulled count.
    #[test]
    fn reconcile_partitions_the_pulled_set(
        local in arb_local_claims(),
        pulled in prop::collection::vec(arb_pulled_record(), 0..12),
    ) {
        let outcomes = reconcile(&local, &pulled);
        let keys: Vec<&Cid> = outcomes.iter().map(Reconciled::cid).collect();
        let pulled_keys: Vec<&Cid> = pulled.iter().map(PulledRecord::key).collect();
        prop_assert_eq!(keys, pulled_keys);
        let tally = tally_reconcile(&outcomes);
        prop_assert_eq!(
            tally.matched + tally.new + tally.conflicts + tally.rejected,
            pulled.len()
        );
    }

    /// Reconciling a store against its own verbatim copy is a no-op: every
    /// record is Matched.
    #[test]
    fn reconciling_local_against_itself_is_all_matched(local in arb_local_claims()) {
        let pulled: Vec<PulledRecord> = local.iter().map(as_pulled).collect();
        let outcomes = reconcile(&local, &pulled);
        prop_assert!(
            outcomes.iter().all(|o| matches!(o, Reconciled::Matched { .. })),
            "{:?}", outcomes
        );
        prop_assert_eq!(tally_reconcile(&outcomes).matched, local.len());
    }

    /// Verify-before-trust: a record whose recomputed CID differs from its
    /// key (or that cannot be re-parsed) is Rejected — never New, never
    /// Matched, never a Conflict.
    #[test]
    fn a_record_that_does_not_recompute_to_its_key_is_rejected(
        local in arb_local_claims(),
        pulled in arb_pulled_record(),
    ) {
        let outcome = &reconcile(&local, std::slice::from_ref(&pulled))[0];
        let verified = matches!(
            &pulled,
            PulledRecord::Recomputed { key, recomputed, .. } if key == recomputed
        );
        prop_assert_eq!(matches!(outcome, Reconciled::Rejected { .. }), !verified);
    }

    /// A verified record already held locally (same CID) is Matched; one the
    /// local store lacks is a Conflict iff a local claim is the same logical
    /// record (author_did, subject, predicate, object) under another CID, and
    /// New otherwise.
    #[test]
    fn a_verified_record_is_matched_new_or_a_conflict_by_cid_and_identity(
        local in arb_local_claims(),
        cid in arb_cid(),
        identity in arb_identity(),
    ) {
        let pulled = PulledRecord::Recomputed {
            key: cid.clone(),
            recomputed: cid.clone(),
            identity: identity.clone(),
        };
        let outcome = &reconcile(&local, std::slice::from_ref(&pulled))[0];
        let held = local.iter().any(|l| l.cid == cid);
        let same_record = local.iter().any(|l| l.identity == identity);
        match outcome {
            Reconciled::Matched { .. } => prop_assert!(held),
            Reconciled::Conflict { local: local_cid, .. } => {
                prop_assert!(!held && same_record);
                prop_assert!(local.iter().any(|l| &l.cid == local_cid && l.identity == identity));
            }
            Reconciled::New { .. } => prop_assert!(!held && !same_record),
            Reconciled::Rejected { .. } => prop_assert!(false, "a verified record was rejected"),
        }
    }

    /// KPI-SF-1 on pull: the verbatim blob of ANY signed claim (0.0/0.5/1.0
    /// confidences included) recomputes to its own CID and carries its
    /// logical identity; bytes that are not a signed claim are Unreadable.
    #[test]
    fn a_pulled_blob_recomputes_to_its_own_cid_with_its_identity(signed in arb_signed_claim()) {
        let key = signed.signature.signed_cid.clone();
        let claim = &signed.unsigned;
        prop_assert_eq!(
            recompute_pulled(&key, &record_bytes_of(&signed)),
            PulledRecord::Recomputed {
                key: key.clone(),
                recomputed: key.clone(),
                identity: RecordIdentity {
                    author_did: claim.author_did.0.clone(),
                    subject: claim.subject.clone(),
                    predicate: claim.predicate.clone(),
                    object: claim.object.clone(),
                },
            }
        );
        let garbage = recompute_pulled(&key, &RecordBytes(b"not json".to_vec()));
        prop_assert!(
            matches!(garbage, PulledRecord::Unreadable { .. }),
            "non-claim bytes must be Unreadable, got {:?}", garbage
        );
    }
}

/// Two distinct authors, each named in either DID form (bare, or a DID URL
/// naming one of their keys). The person index is the independent oracle
/// for "same author".
fn arb_author_in_either_form() -> impl Strategy<Value = (usize, String)> {
    (
        prop::sample::select(vec!["did:plc:maria-test", "did:plc:rachel-test"]),
        any::<bool>(),
    )
        .prop_map(|(did, with_fragment)| {
            let person = usize::from(did.ends_with("rachel-test"));
            let author = if with_fragment {
                format!("{did}#org.openlore.application")
            } else {
                did.to_string()
            };
            (person, author)
        })
}

proptest! {
    /// Logical-record identity law (PR-3): a verified pulled record the
    /// local store lacks, whose (author, subject, predicate, object) equals a
    /// local claim's — the author named in EITHER DID form on either side —
    /// is always a Conflict naming both CIDs and the record; never Matched,
    /// never New (so never inserted).
    #[test]
    fn same_logical_record_under_another_cid_is_always_a_conflict(
        base in arb_identity(),
        (local_author, pulled_author) in arb_author_in_either_form().prop_flat_map(|(person, local_author)| {
            let pulled = arb_author_in_either_form()
                .prop_filter("same person", move |(other, _)| *other == person)
                .prop_map(|(_, author)| author);
            (Just(local_author), pulled)
        }),
        local_cid in arb_cid(),
        pulled_cid in arb_cid(),
        others in arb_local_claims(),
    ) {
        prop_assume!(local_cid != pulled_cid);
        let local_identity = RecordIdentity { author_did: local_author, ..base.clone() };
        let pulled_identity = RecordIdentity { author_did: pulled_author, ..base.clone() };
        let local: Vec<LocalClaim> = others
            .into_iter()
            .filter(|other| other.cid != pulled_cid)
            .filter(|other| (&other.identity.subject, &other.identity.object) != (&base.subject, &base.object))
            .chain(std::iter::once(LocalClaim { cid: local_cid.clone(), identity: local_identity }))
            .collect();
        let pulled = PulledRecord::Recomputed {
            key: pulled_cid.clone(),
            recomputed: pulled_cid.clone(),
            identity: pulled_identity.clone(),
        };
        prop_assert_eq!(
            &reconcile(&local, std::slice::from_ref(&pulled))[0],
            &Reconciled::Conflict { pulled: pulled_cid, local: local_cid, record: pulled_identity }
        );
    }

    /// A verified pulled record whose CID the local store already holds is
    /// always Matched, whatever its logical identity or author DID form.
    #[test]
    fn a_verified_record_under_a_held_cid_is_always_matched(
        local in arb_local_claims().prop_filter("non-empty", |local| !local.is_empty()),
        pick in any::<prop::sample::Index>(),
        base in arb_identity(),
        (_, author_did) in arb_author_in_either_form(),
    ) {
        let held = local[pick.index(local.len())].cid.clone();
        let pulled = PulledRecord::Recomputed {
            key: held.clone(),
            recomputed: held.clone(),
            identity: RecordIdentity { author_did, ..base },
        };
        prop_assert_eq!(
            &reconcile(&local, std::slice::from_ref(&pulled))[0],
            &Reconciled::Matched { cid: held }
        );
    }
}

// -----------------------------------------------------------------------------
// Pull insert selection (US-SF-004 fresh-machine rebuild; anti-merging)
// -----------------------------------------------------------------------------

/// The local identity's bare DID in the insert-selection properties.
const OWN_DID: &str = "did:plc:maria-test";

/// Author DIDs paired with whether `OWN_DID` authored them (an independent
/// oracle): the bare DID and a DID URL naming one of its keys are own; a
/// different DID — including one that merely shares `OWN_DID` as a prefix —
/// is foreign.
fn arb_author() -> impl Strategy<Value = (String, bool)> {
    prop::sample::select(vec![
        (OWN_DID.to_string(), true),
        (format!("{OWN_DID}#org.openlore.application"), true),
        ("did:plc:rachel-test".to_string(), false),
        (format!("{OWN_DID}-impostor"), false),
        (format!("{OWN_DID}x#org.openlore.application"), false),
    ])
}

fn arb_authored_pulled_record() -> impl Strategy<Value = PulledRecord> {
    (arb_pulled_record(), arb_author()).prop_map(|(record, (author_did, _))| match record {
        PulledRecord::Recomputed {
            key,
            recomputed,
            identity,
        } => PulledRecord::Recomputed {
            key,
            recomputed,
            identity: RecordIdentity {
                author_did,
                ..identity
            },
        },
        unreadable => unreadable,
    })
}

fn owned_by_oracle(author_did: &str) -> bool {
    author_did == OWN_DID || author_did == format!("{OWN_DID}#org.openlore.application")
}

/// Each New outcome with the author of the pulled record it classified
/// (`reconcile` yields one outcome per pulled record, in pulled order).
fn new_records_with_authors(
    reconciled: &[Reconciled],
    pulled: &[PulledRecord],
) -> Vec<(Cid, String)> {
    reconciled
        .iter()
        .zip(pulled)
        .filter_map(|pair| match pair {
            (Reconciled::New { cid }, PulledRecord::Recomputed { identity, .. }) => {
                Some((cid.clone(), identity.author_did.clone()))
            }
            _ => None,
        })
        .collect()
}

proptest! {
    /// A pull inserts ONLY verified New records the local identity authored
    /// (selected ⊆ New ∧ every selected author == local DID); every other New
    /// record is reported as foreign with its author, and nothing Matched,
    /// Conflicting, or Rejected is ever selected or reported.
    #[test]
    fn a_pull_inserts_only_new_records_the_local_identity_authored(
        local in arb_local_claims(),
        pulled in prop::collection::vec(arb_authored_pulled_record(), 0..12),
    ) {
        let reconciled = reconcile(&local, &pulled);
        let selection = select_inserts(OWN_DID, &reconciled, &pulled);
        let new_records = new_records_with_authors(&reconciled, &pulled);

        // selected ⊆ New, and every selected author is the local identity.
        let expected_inserts: Vec<Cid> = new_records
            .iter()
            .filter(|(_, author)| owned_by_oracle(author))
            .map(|(cid, _)| cid.clone())
            .collect();
        prop_assert_eq!(&selection.to_insert, &expected_inserts);
        // Every other New record is reported foreign, with its author.
        let expected_foreign: Vec<ForeignRecord> = new_records
            .into_iter()
            .filter(|(_, author)| !owned_by_oracle(author))
            .map(|(cid, author_did)| ForeignRecord { cid, author_did })
            .collect();
        prop_assert_eq!(&selection.foreign, &expected_foreign);
    }
}

// -----------------------------------------------------------------------------
// 8. Peer transport selection (US-SF-006, OD-SF-3)
// -----------------------------------------------------------------------------

/// Any `/manifest` probe observation: no response at all, an arbitrary
/// response, or a marker-bearing JSON body (whatever its records look like)
/// under an arbitrary status.
fn arb_probe_observation() -> impl Strategy<Value = ManifestObservation> {
    prop_oneof![
        "[a-z ]{0,24}".prop_map(|detail| ManifestObservation::Unreachable {
            url: "https://openlore.rachel.workers.dev".to_string(),
            detail,
        }),
        (100_u16..600, arb_manifest_body())
            .prop_map(|(status, body)| ManifestObservation::Responded { status, body }),
        (100_u16..600, arb_json(), any::<u64>()).prop_map(|(status, json, version)| {
            ManifestObservation::Responded {
                status,
                body: with_marker(json, version).to_string().into_bytes(),
            }
        }),
    ]
}

/// The transport a probe observation must select (the spec, stated
/// independently of the implementation's composition).
fn expected_transport(observation: &ManifestObservation) -> PeerTransport {
    match observation {
        ManifestObservation::Unreachable { detail, .. } => PeerTransport::Unreachable {
            detail: detail.clone(),
        },
        ManifestObservation::Responded { status, body } if carries_the_marker(*status, body) => {
            PeerTransport::OpaqueInstance
        }
        ManifestObservation::Responded { .. } => PeerTransport::AtprotoPds,
    }
}

proptest! {
    /// Every probe observation selects exactly one transport, total and
    /// deterministic: no response ⇒ unreachable (skip the peer); a 2xx JSON
    /// body with the openlore marker ⇒ the opaque-instance read; any other
    /// response (404 from a PDS, HTML, marker-less JSON, non-2xx) ⇒ the
    /// shipped PDS XRPC path, unchanged.
    #[test]
    fn every_probe_observation_selects_exactly_one_peer_transport(
        observation in arb_probe_observation(),
    ) {
        let selected = select_peer_transport(&observation);
        prop_assert_eq!(&selected, &expected_transport(&observation));
        prop_assert_eq!(select_peer_transport(&observation), selected);
    }

    /// A marker-bearing 2xx `/manifest` ALWAYS selects the instance path,
    /// whatever else the manifest carries (even records that do not parse —
    /// that is the instance path's failure to report, never a PDS fallback).
    #[test]
    fn a_marker_bearing_2xx_manifest_always_selects_the_instance_path(
        status in 200_u16..300,
        manifest in arb_json(),
        records in arb_json(),
        contract_version in any::<u64>(),
    ) {
        let mut marked = with_marker(manifest, contract_version);
        if let Some(object) = marked.as_object_mut() {
            object.insert("records".to_string(), records);
        }
        let observation = ManifestObservation::Responded {
            status,
            body: marked.to_string().into_bytes(),
        };
        prop_assert_eq!(select_peer_transport(&observation), PeerTransport::OpaqueInstance);
    }
}

// -----------------------------------------------------------------------------
// Reporting surfaces: the reconcile tally, the readback refusal reason, and
// the manifest's committed CID set.
// -----------------------------------------------------------------------------

proptest! {
    /// The tally counts each outcome kind exactly, and a pull is in sync iff
    /// every pulled record is already held locally (all Matched) — any New,
    /// Conflict, or Rejected outcome means there is something to report.
    #[test]
    fn the_tally_counts_each_outcome_and_is_in_sync_iff_all_matched(
        local in arb_local_claims(),
        pulled in prop::collection::vec(arb_pulled_record(), 0..12),
    ) {
        let outcomes = reconcile(&local, &pulled);
        let count = |kind: fn(&Reconciled) -> bool| outcomes.iter().filter(|o| kind(o)).count();
        let tally = tally_reconcile(&outcomes);
        prop_assert_eq!(
            tally,
            ReconcileTally {
                matched: count(|o| matches!(o, Reconciled::Matched { .. })),
                new: count(|o| matches!(o, Reconciled::New { .. })),
                conflicts: count(|o| matches!(o, Reconciled::Conflict { .. })),
                rejected: count(|o| matches!(o, Reconciled::Rejected { .. })),
            }
        );
        prop_assert_eq!(
            tally.in_sync(),
            outcomes.iter().all(|o| matches!(o, Reconciled::Matched { .. }))
        );
    }

    /// Each readback refusal reads as its own reason, naming what the
    /// instance returned (the decode detail or the drifted CID).
    #[test]
    fn a_readback_refusal_describes_its_reason(
        detail in "[a-z]{1,12}",
        recomputed in arb_cid(),
    ) {
        let unreadable = ReadbackMismatch::Unreadable { detail: detail.clone() }.describe();
        let drift = ReadbackMismatch::CidDrift { recomputed: recomputed.clone() }.describe();
        let altered = ReadbackMismatch::BytesAltered.describe();
        prop_assert!(unreadable.contains("unreadable") && unreadable.contains(&detail));
        prop_assert!(drift.contains(&recomputed.0));
        prop_assert!(altered.contains("altered"));
        let reasons: BTreeSet<&String> = [&unreadable, &drift, &altered].into_iter().collect();
        prop_assert_eq!(reasons.len(), 3);
    }

    /// A manifest's committed CID set is exactly its entries' CIDs, in order —
    /// so a manifest of pushed projections lists the pushed claims' CIDs.
    #[test]
    fn manifest_cids_lists_every_entry_cid_in_order(
        signed in prop::collection::vec(arb_signed_claim(), 0..4),
        contract_version in any::<u64>(),
    ) {
        let manifest = InstanceManifest {
            contract_version,
            entries: signed.iter().map(display_projection).collect(),
        };
        let pushed: Vec<Cid> = signed.iter().map(|s| s.signature.signed_cid.clone()).collect();
        prop_assert_eq!(manifest_cids(&manifest), pushed);
    }
}
