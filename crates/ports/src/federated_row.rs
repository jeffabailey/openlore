//! `federated_row` — the cross-store row type returned by
//! `StoragePort::query_federated_by_subject` + the supporting peer
//! identity / record types referenced from `PeerStoragePort` and the
//! slice-03 extension of `PdsPort` and `IdentityPort`.
//!
//! **Invariant I-FED-1 layer 1 (type-level anti-merging)**: `FederatedRow`
//! carries `author_did: Did` as a NON-Option field. Slice-03's anti-merging
//! defense is layered (WD-30): this is the compile-time defense. The
//! structural defense (`xtask check-arch::no_cross_table_join_elides_author`)
//! and the behavioral defense (`tests/integration/federation_attribution_preserved.rs`)
//! add the other two layers.
//!
//! See `docs/feature/openlore-federated-read/design/component-boundaries.md`
//! §`crates/ports` for the source-of-truth Rust code blocks.
//
// SCAFFOLD: false  (data types are real; behavior arrives via traits)

use chrono::{DateTime, Utc};
use claim_domain::{Did, SignedClaim};
use serde::{Deserialize, Serialize};
use url::Url;

// -----------------------------------------------------------------------------
// Federated read row — output of `StoragePort::query_federated_by_subject`
// -----------------------------------------------------------------------------

/// One row of a federated subject query, carrying its attribution at the
/// type level.
///
/// `author_did` is intentionally `Did`, NOT `Option<Did>`: dropping
/// attribution at the type level is the slice-03 anti-merging defense
/// (I-FED-1 layer 1 per WD-30). The renderer uses `author_relationship`
/// to decide between "(you)", "(subscribed peer)", and "(unsubscribed
/// cache)"; `source_table` tells the adapter (for diagnostics) which
/// physical table the row came from.
///
/// `PartialEq` (not `Eq`) because `SignedClaim` carries a `Confidence`
/// f64 which cannot derive `Eq` (NaN). Equivalence is sufficient for
/// test assertions; canonical equality is at the CID level.
#[derive(Debug, Clone, PartialEq)]
pub struct FederatedRow {
    /// LOAD-BEARING: non-Option per I-FED-1 layer 1. A row without an
    /// author DID is a compile error, not a runtime check.
    pub author_did: Did,
    pub author_relationship: AuthorRelationship,
    pub signed_claim: SignedClaim,
    pub source_table: SourceTable,
}

/// How the federated row relates to the local user.
///
/// `UnsubscribedCache` is the soft-remove residue: the user used to be
/// subscribed, ran `openlore peer remove <did>` (no `--purge`), and the
/// peer_claims rows were retained but the subscription is now inactive
/// (per ADR-014 soft-remove semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorRelationship {
    You,
    SubscribedPeer,
    UnsubscribedCache,
}

/// Which physical table the row came from.
///
/// `Own` = `claims` (the user's own author table; slice-01).
/// `Peer` = `peer_claims` (any subscribed-or-formerly-subscribed peer's
/// table; slice-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceTable {
    Own,
    Peer,
}

// -----------------------------------------------------------------------------
// Peer identity + records — used by IdentityPort + PdsPort extensions
// -----------------------------------------------------------------------------

/// Result of `IdentityPort::resolve_peer`.
///
/// Carries everything the cli needs to (a) confirm to the user "this is
/// who you're subscribing to" and (b) fetch records from the peer's PDS.
/// `verification_methods` is loaded for parity with the user's own DID
/// document so future "verify peer signature" paths can consult it
/// without a second resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub did: Did,
    pub handle: String,
    pub pds_endpoint: Url,
    pub verification_methods: Vec<VerificationMethod>,
}

/// One verification method entry from a DID document.
///
/// Mirrors the subset of the W3C DID-core `verificationMethod` schema
/// that the OpenLore PLC + did:web resolvers actually expose; pure data,
/// adapter-populated at resolve time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationMethod {
    pub id: String,
    pub type_: String,
    pub controller: Did,
    pub public_key_multibase: String,
}

/// One signed record fetched from a peer's PDS.
///
/// The `rkey` is the per-PDS record key (per ATProto); `signed_claim` is
/// the canonical claim ADT after parsing the JSON value against the
/// `org.openlore.claim` lexicon. Splitting `rkey` out (rather than
/// recomputing it from CID) lets `peer pull` log it for diagnostics
/// without re-encoding.
///
/// `PartialEq` (not `Eq`) for the same reason as `FederatedRow`
/// (SignedClaim carries a non-Eq Confidence f64).
#[derive(Debug, Clone, PartialEq)]
pub struct SignedRecord {
    pub rkey: String,
    pub signed_claim: SignedClaim,
}

/// One page of `PdsPort::list_peer_records` results.
///
/// `next_cursor: None` signals end-of-stream. The adapter is responsible
/// for honoring the ATProto cursor contract (opaque string echoed back).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerRecordPage {
    pub records: Vec<SignedRecord>,
    pub next_cursor: Option<String>,
}

// -----------------------------------------------------------------------------
// Peer subscription state — referenced by PeerStoragePort
// -----------------------------------------------------------------------------

/// A subscription row from `peer_subscriptions`.
///
/// `removed_at = None` ↔ active subscription. `removed_at = Some(ts)` ↔
/// soft-removed (ADR-014); the row stays so the renderer can annotate
/// retained peer_claims as "(unsubscribed cache)". `hard_purge` deletes
/// the row entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSubscription {
    pub peer_did: Did,
    pub peer_handle: String,
    pub peer_pds_endpoint: Url,
    pub subscribed_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}

impl PeerSubscription {
    /// Convenience: a subscription is "active" iff it has never been
    /// soft-removed. Equivalent to `self.removed_at.is_none()`; named for
    /// the call-site that uses it (renderer + add-idempotency check).
    pub fn is_active(&self) -> bool {
        self.removed_at.is_none()
    }
}

// -----------------------------------------------------------------------------
// Property-based tests — anti-merging type-level defense + accessors
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Properties exercised here:
    //!
    //! 1. `FederatedRow::author_did` byte-equals the input `Did`
    //!    for any constructible row — the compile-time non-Option
    //!    constraint is enforced by the type, but this test pins the
    //!    accessor contract so any future refactor that wraps `Did` in
    //!    a smart-constructor newtype either preserves byte-equality or
    //!    visibly breaks this test.
    //!
    //! 2. `PeerSubscription::is_active` ↔ `removed_at.is_none()` — the
    //!    "active iff no removed_at" predicate is the single source of
    //!    truth used by the renderer's annotation choice and add-idempotency
    //!    short-circuit; this property fixes the contract before any
    //!    adapter implements it.
    //!
    //! Universe (port-exposed observable surface): for FederatedRow it is
    //! `{author_did, author_relationship, source_table}` (signed_claim is
    //! a domain value with its own tests in `claim_domain`). For
    //! PeerSubscription it is `{peer_did, removed_at, is_active()}`.

    use super::*;
    use claim_domain::proptest_strategies::arb_unsigned_claim;
    use claim_domain::{Cid, SignatureBlock};
    use proptest::prelude::*;

    /// Strategy: arbitrary DID strings shaped like real DIDs. Body
    /// content is unconstrained — the property under test is about
    /// preserving whatever the caller passes in, not validating the
    /// format (validation lives in slice-04+).
    fn arb_did() -> impl Strategy<Value = Did> {
        // PLC method = lowercase a-z + digits (24 chars per spec).
        // Web method = arbitrary host. Mix both so the property holds
        // across the variants the adapter will see in production.
        prop_oneof![
            "did:plc:[a-z0-9]{8,24}".prop_map(Did),
            "did:web:[a-z0-9.-]{4,32}".prop_map(Did),
        ]
    }

    /// Strategy: a structurally-valid SignedClaim built atop the
    /// claim-domain `arb_unsigned_claim` strategy. We don't introspect
    /// the claim contents; FederatedRow only needs a populated
    /// `SignedClaim` so the test can assert byte-equal preservation of
    /// `author_did`. Reuses the pure-core strategy to avoid duplicating
    /// generator code outside its owning crate.
    fn arb_signed_claim() -> impl Strategy<Value = SignedClaim> {
        (
            arb_unsigned_claim(),
            "[a-zA-Z0-9]{16,64}", // signature bytes (placeholder string → bytes)
            "bafy[a-z0-9]{52}",   // signed cid (CIDv1 base32-lower shape)
        )
            .prop_map(|(unsigned, sig_str, cid_str)| SignedClaim {
                unsigned,
                signature: SignatureBlock {
                    signed_cid: Cid(cid_str),
                    signature_bytes: sig_str.into_bytes(),
                    verification_method: "did:plc:author-test#org.openlore.application".to_string(),
                },
            })
    }

    proptest! {
        /// Property: FederatedRow preserves author_did byte-for-byte.
        /// This is the layer-1 anti-merging compile-time defense — if
        /// anyone refactored `author_did` to `Option<Did>` or to a
        /// derived projection that elided the DID, this test would red
        /// because the accessor would no longer return the input verbatim.
        #[test]
        fn federated_row_preserves_author_did(
            did in arb_did(),
            signed in arb_signed_claim(),
            rel in prop_oneof![
                Just(AuthorRelationship::You),
                Just(AuthorRelationship::SubscribedPeer),
                Just(AuthorRelationship::UnsubscribedCache),
            ],
            src in prop_oneof![Just(SourceTable::Own), Just(SourceTable::Peer)],
        ) {
            let row = FederatedRow {
                author_did: did.clone(),
                author_relationship: rel,
                signed_claim: signed,
                source_table: src,
            };
            prop_assert_eq!(&row.author_did, &did,
                "FederatedRow.author_did must preserve the input Did byte-equal (I-FED-1 layer 1)");
            // Defense in depth: the field is *named* author_did, not
            // attributed_to or similar. Renaming the field is a breaking
            // change that must red this assertion at the source level.
            prop_assert_eq!(row.author_did.0, did.0,
                "FederatedRow.author_did.0 must equal the input Did.0 string");
        }

        /// Property: PeerSubscription is_active iff removed_at is None.
        /// This pins the single source of truth used by every caller
        /// (renderer annotation, add idempotency, soft-remove probe).
        #[test]
        fn peer_subscription_is_active_iff_not_removed(
            did in arb_did(),
            handle in "[a-z]{1,16}\\.test",
            subscribed_at in 0i64..2_000_000_000,
            maybe_removed in proptest::option::of(0i64..2_000_000_000),
        ) {
            let pds = Url::parse("https://pds.example.test").unwrap();
            let subscribed = DateTime::<Utc>::from_timestamp(subscribed_at, 0).unwrap();
            let removed = maybe_removed.map(|t| DateTime::<Utc>::from_timestamp(t, 0).unwrap());

            let sub = PeerSubscription {
                peer_did: did,
                peer_handle: handle,
                peer_pds_endpoint: pds,
                subscribed_at: subscribed,
                removed_at: removed,
            };

            prop_assert_eq!(sub.is_active(), removed.is_none(),
                "is_active() must equal removed_at.is_none() — the single source of truth used by renderer + idempotency callers");
        }
    }

    /// Single-example: AuthorRelationship + SourceTable enum variants
    /// are pattern-match exhaustive. This is a compile-time check (no
    /// runtime assertions); the test exists so a missing variant in a
    /// future refactor lights up as a missing match arm, not as a silent
    /// fall-through in the renderer.
    ///
    /// bypass: enum-variant-presence assertion is not amenable to proptest —
    /// the universe is the finite set of enum variants; there is no
    /// equivalence class to sample over.
    #[test]
    fn author_relationship_and_source_table_variants_exhaustive() {
        for rel in [
            AuthorRelationship::You,
            AuthorRelationship::SubscribedPeer,
            AuthorRelationship::UnsubscribedCache,
        ] {
            let name = match rel {
                AuthorRelationship::You => "you",
                AuthorRelationship::SubscribedPeer => "subscribed-peer",
                AuthorRelationship::UnsubscribedCache => "unsubscribed-cache",
            };
            assert!(!name.is_empty(), "rendered name must be non-empty");
        }
        for src in [SourceTable::Own, SourceTable::Peer] {
            let label = match src {
                SourceTable::Own => "own",
                SourceTable::Peer => "peer",
            };
            assert!(!label.is_empty(), "rendered label must be non-empty");
        }
    }
}
