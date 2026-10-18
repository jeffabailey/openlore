//! `peer_storage` — the new slice-03 port that owns the peer-storage
//! surface (subscriptions + peer-claim CRUD + anti-merging defenses).
//!
//! Why a separate port from `StoragePort`? Author storage is
//! single-tenant (the user is the sole author); peer storage is
//! per-author (every row carries an `author_did` attribution). The two
//! ports MAY share an underlying DuckDB connection pool (and the
//! `DuckDbPeerStorageAdapter` is implemented exactly that way per
//! step 01-02), but the trait surface is distinct: callers think in
//! terms of "subscribe to a peer" and "pull peer's claims", not
//! "write a row to storage" without attribution context.
//!
//! All methods are sync — local DuckDB only, no network. The `probe`
//! method follows ADR-009's Earned-Trust contract (slice-03 additions
//! to `ProbeRefusalReason`: see `probe.rs`).
//!
//! See `docs/feature/openlore-federated-read/design/component-boundaries.md`
//! §`crates/ports` and ADR-014 (peer storage / soft-remove ↔ hard-purge).
//
// SCAFFOLD: false  (trait + ADTs are real; implementations land in 01-02)

use chrono::{DateTime, Utc};
use claim_domain::{Cid, ClaimRecord, Did, ReferenceType, SelfAttestedClaim, SignedClaim};
use url::Url;

use crate::federated_row::PeerSubscription;
use crate::probe::ProbeOutcome;

// -----------------------------------------------------------------------------
// Error type — railway-oriented per nw-fp-domain-modeling §8
// -----------------------------------------------------------------------------

/// Failure modes for `PeerStoragePort`.
///
/// `SelfAttribution` + `CrossAttribution` are anti-merging defenses
/// (I-FED-1 + WD-41): the adapter REJECTS writes whose attribution
/// drifts away from the subscribed peer's DID. `AntiMergingInvariantViolated`
/// is defensive — it should never fire if `probe()` and `xtask check-arch`
/// pass, but is retained so a regression surfaces as a structured error
/// rather than a silent merge.
#[derive(Debug, thiserror::Error)]
pub enum PeerStorageError {
    #[error("peer storage probe refused: {detail}")]
    ProbeRefused { detail: String },

    #[error("peer storage schema mismatch")]
    SchemaMismatch,

    #[error("peer storage I/O failure: {0}")]
    Io(#[from] std::io::Error),

    #[error("DuckDB failure: {0}")]
    DuckDb(String),

    /// The caller attempted to write a peer_claim row whose `author_did`
    /// equals the local user's DID. Rejected to preserve the
    /// own-vs-peer separation (anti-merging layer-2 enforcement).
    #[error("self-attribution rejected: peer_claim cannot be authored by the local user")]
    SelfAttribution,

    /// The pulled record's `author` field references a DID OTHER than
    /// the subscribed peer's DID (anxiety scenario 1.2 / WD-41). The
    /// slice-03 trust model rejects cross-attributed records.
    #[error(
        "cross-attribution rejected: record author {actual:?} does not match \
         subscribed peer {expected:?}"
    )]
    CrossAttribution { expected: Did, actual: Did },

    /// Defensive: should never fire if probe + check-arch pass. If it
    /// does, a regression has bypassed the layered anti-merging defenses
    /// (WD-30) and the adapter caught it at the storage layer.
    #[error("anti-merging invariant violated: {detail}")]
    AntiMergingInvariantViolated { detail: String },
}

// -----------------------------------------------------------------------------
// Outcomes — load-bearing for the cli's user-facing messages
// -----------------------------------------------------------------------------

/// Result of `add_subscription`. Distinguishes fresh-subscribe from
/// idempotent-re-subscribe so the cli can render "already subscribed
/// since <ts>" (US-FED-001 AC #3) without a second DB lookup, and from
/// re-following a peer the user removed (U-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddSubscriptionOutcome {
    Added {
        subscribed_at: DateTime<Utc>,
    },
    AlreadyExisted {
        since: DateTime<Utc>,
    },
    /// The peer had been soft-removed (`peer remove` without `--purge`);
    /// the subscription is active again from `subscribed_at`.
    Refollowed {
        subscribed_at: DateTime<Utc>,
    },
}

/// What `add_subscription` must do to the stored subscription row. A pure
/// decision ([`plan_add_subscription`]); the adapter only carries it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddSubscriptionPlan {
    /// No row for this DID (never added, or hard-purged): insert this one.
    Insert(PeerSubscription),
    /// The DID is followed already: write nothing.
    KeepActive { since: DateTime<Utc> },
    /// The DID was soft-removed: overwrite its row with this active one.
    Refollow(PeerSubscription),
}

impl AddSubscriptionPlan {
    /// The outcome the caller sees once the plan is carried out.
    pub fn outcome(&self) -> AddSubscriptionOutcome {
        match self {
            Self::Insert(sub) => AddSubscriptionOutcome::Added {
                subscribed_at: sub.subscribed_at,
            },
            Self::KeepActive { since } => AddSubscriptionOutcome::AlreadyExisted { since: *since },
            Self::Refollow(sub) => AddSubscriptionOutcome::Refollowed {
                subscribed_at: sub.subscribed_at,
            },
        }
    }
}

/// Decide how `peer add` treats the stored row for the requested DID.
///
/// An active row is kept untouched (PS-2 idempotency). A soft-removed row
/// is followed again (U-20): the requested subscription — freshly resolved
/// handle and PDS, the new `subscribed_at`, never removed — replaces it, so
/// the peer's cached claims count again. No row means a fresh insert.
pub fn plan_add_subscription(
    existing: Option<&PeerSubscription>,
    requested: PeerSubscription,
) -> AddSubscriptionPlan {
    let active_request = PeerSubscription {
        removed_at: None,
        ..requested
    };
    match existing {
        None => AddSubscriptionPlan::Insert(active_request),
        Some(row) if row.is_active() => AddSubscriptionPlan::KeepActive {
            since: row.subscribed_at,
        },
        Some(_) => AddSubscriptionPlan::Refollow(active_request),
    }
}

/// Result of `soft_remove`. `was_subscribed = false` ↔ "this DID was
/// never a subscription" (no-op exit). `cached_claim_count` drives the
/// "N cached peer claims retained" line per US-FED-005.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftRemoveOutcome {
    pub was_subscribed: bool,
    pub cached_claim_count: u32,
}

/// Result of `hard_purge`. Reports counts so the cli can render
/// "deleted N peer claims; preserved M counter-claims" per
/// US-FED-005 + the WD-41 user-counter-claim preservation contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardPurgeOutcome {
    pub was_subscribed: bool,
    pub deleted_peer_claim_count: u32,
    pub preserved_user_counter_claim_count: u32,
}

/// Result of `write_peer_claim`. `written = false` ↔ "this CID is
/// already present in peer_claims" (idempotent re-pull); cli renders
/// "already in peer_claims" instead of "stored" per US-FED-002.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WritePeerClaimOutcome {
    pub written: bool,
}

// -----------------------------------------------------------------------------
// The port — sync trait, function-shaped per nw-fp-hexagonal-architecture
// -----------------------------------------------------------------------------

/// Owns the peer-storage surface.
///
/// Implementations (DuckDbPeerStorageAdapter ships in step 01-02) MAY
/// share the underlying connection pool with `StoragePort`'s adapter,
/// but the trait surface is intentionally distinct so callers cannot
/// silently treat peer rows as author rows (anti-merging defense at
/// the type level).
///
/// Every method returns `Result<_, PeerStorageError>` for
/// railway-oriented composition. The `probe()` method follows
/// ADR-009's Earned-Trust contract.
pub trait PeerStoragePort {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. Implementations
    /// exercise: schema_version row present at expected version;
    /// sentinel round-trip; soft-remove vs hard-purge isolation;
    /// SelfAttribution rejection at write time.
    fn probe(&self) -> ProbeOutcome;

    /// Add (or idempotently re-confirm) a subscription. Returns
    /// `Added` on first insert, `AlreadyExisted` for an active peer and
    /// `Refollowed` for a soft-removed one (see [`plan_add_subscription`]).
    fn add_subscription(
        &self,
        sub: PeerSubscription,
    ) -> Result<AddSubscriptionOutcome, PeerStorageError>;

    /// List all CURRENTLY-ACTIVE subscriptions (i.e.,
    /// `removed_at IS NULL`). Soft-removed subscriptions are excluded
    /// from this listing but their peer_claims rows remain queryable
    /// via `query_federated_by_subject` (annotated `UnsubscribedCache`).
    fn list_active_subscriptions(&self) -> Result<Vec<PeerSubscription>, PeerStorageError>;

    /// Look up one subscription by DID (active OR soft-removed). Used by
    /// `peer add` idempotency check, by `peer pull` to fetch the
    /// `peer_pds_endpoint`, and by the renderer's "subscribed peer" vs
    /// "unsubscribed cache" labeling.
    fn lookup_subscription(
        &self,
        peer_did: &Did,
    ) -> Result<Option<PeerSubscription>, PeerStorageError>;

    /// Soft-remove: set `removed_at = now()` on the subscription row but
    /// preserve all peer_claims rows. Per ADR-014, runs in its own
    /// transaction (distinct from `hard_purge`'s tx) so callers cannot
    /// confuse the two via composition.
    fn soft_remove(&self, peer_did: &Did) -> Result<SoftRemoveOutcome, PeerStorageError>;

    /// Hard-purge: delete the subscription row AND every peer_claims
    /// row attributed to this DID AND best-effort remove
    /// `peer_claims/<did>/`. Preserves user counter-claims (those live
    /// in the author table, untouched by this operation).
    fn hard_purge(&self, peer_did: &Did) -> Result<HardPurgeOutcome, PeerStorageError>;

    /// Persist one peer claim. The adapter MUST verify that
    /// `signed.unsigned.author_did == *peer_did` BEFORE writing; on
    /// mismatch it returns `CrossAttribution` (WD-41). It MUST also
    /// reject `peer_did == identity.author_did()` with `SelfAttribution`
    /// (anti-merging layer-2).
    fn write_peer_claim(
        &self,
        peer_did: &Did,
        signed: &SignedClaim,
        fetched_from_pds: &Url,
        fetched_at: DateTime<Utc>,
    ) -> Result<WritePeerClaimOutcome, PeerStorageError>;

    /// Persist one SELF-ATTESTED peer claim (ADR-071): admitted by the
    /// caller's provenance verdict (no signature; author = repo DID; fetched
    /// from the author's own PDS). Same attribution guards as
    /// [`write_peer_claim`](Self::write_peer_claim); the row is stored with
    /// `provenance = 'self-attested'`.
    fn write_self_attested_peer_claim(
        &self,
        peer_did: &Did,
        claim: &SelfAttestedClaim,
        fetched_from_pds: &Url,
        fetched_at: DateTime<Utc>,
    ) -> Result<WritePeerClaimOutcome, PeerStorageError>;

    /// Fetch one peer claim by CID along with its `author_did`. Returns
    /// `Ok(None)` if no row exists; never returns a claim without
    /// attribution (anti-merging layer-1: the type pair is
    /// `Option<(Did, SignedClaim)>`, not `Option<SignedClaim>`).
    fn get_peer_claim_by_cid(
        &self,
        cid: &Cid,
    ) -> Result<Option<(Did, SignedClaim)>, PeerStorageError>;

    /// Fetch one peer claim by CID with its `author_did`, WHATEVER its
    /// provenance (ADR-071): an app-signed row as `ClaimRecord::AppSigned`
    /// (exactly what [`get_peer_claim_by_cid`] returns), a self-attested
    /// row as `ClaimRecord::SelfAttested`. Additive: the default knows only
    /// app-signed rows, so stores without self-attested rows are unchanged.
    ///
    /// [`get_peer_claim_by_cid`]: PeerStoragePort::get_peer_claim_by_cid
    fn get_peer_claim_record_by_cid(
        &self,
        cid: &Cid,
    ) -> Result<Option<(Did, ClaimRecord)>, PeerStorageError> {
        Ok(self
            .get_peer_claim_by_cid(cid)?
            .map(|(author, signed)| (author, ClaimRecord::AppSigned(signed))))
    }

    /// All peer claims matching a subject, each paired with its
    /// `author_did`. Used by `claim graph --federated` joined against
    /// the author table for the cross-store federated view.
    fn list_peer_claims_by_subject(
        &self,
        subject: &str,
    ) -> Result<Vec<(Did, SignedClaim)>, PeerStorageError>;

    /// Peer claims that reference `target_cid` (counter-claims, retracts,
    /// corrects, supersedes — anything with a typed reference). Returns
    /// `(author_did, source_cid, reference_type)` triples.
    fn query_peer_referencing(
        &self,
        target_cid: &Cid,
    ) -> Result<Vec<(Did, Cid, ReferenceType)>, PeerStorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn subscription(at_hour: u32, removed_at_hour: Option<u32>) -> PeerSubscription {
        let at = |hour| Utc.with_ymd_and_hms(2026, 10, 1, hour, 0, 0).unwrap();
        PeerSubscription {
            peer_did: Did("did:plc:rachel-test".to_string()),
            peer_handle: "rachel.test".to_string(),
            peer_pds_endpoint: Url::parse("https://pds.example").unwrap(),
            subscribed_at: at(at_hour),
            removed_at: removed_at_hour.map(at),
        }
    }

    #[test]
    fn a_peer_never_added_is_inserted_as_active() {
        let requested = subscription(9, None);

        let plan = plan_add_subscription(None, requested.clone());

        assert_eq!(plan, AddSubscriptionPlan::Insert(requested.clone()));
        assert_eq!(
            plan.outcome(),
            AddSubscriptionOutcome::Added {
                subscribed_at: requested.subscribed_at
            }
        );
    }

    #[test]
    fn an_active_peer_is_kept_and_reports_its_original_follow_time() {
        let stored = subscription(8, None);

        let plan = plan_add_subscription(Some(&stored), subscription(9, None));

        assert_eq!(
            plan,
            AddSubscriptionPlan::KeepActive {
                since: stored.subscribed_at
            }
        );
        assert_eq!(
            plan.outcome(),
            AddSubscriptionOutcome::AlreadyExisted {
                since: stored.subscribed_at
            }
        );
    }

    #[test]
    fn a_removed_peer_is_followed_again_from_now() {
        let stored = subscription(8, Some(9));
        let requested = subscription(10, None);

        let plan = plan_add_subscription(Some(&stored), requested.clone());

        assert_eq!(plan, AddSubscriptionPlan::Refollow(requested.clone()));
        assert_eq!(
            plan.outcome(),
            AddSubscriptionOutcome::Refollowed {
                subscribed_at: requested.subscribed_at
            }
        );
    }

    #[test]
    fn a_written_subscription_is_never_removed() {
        let stored_removed = subscription(8, Some(9));
        for existing in [None, Some(&stored_removed)] {
            match plan_add_subscription(existing, subscription(10, Some(11))) {
                AddSubscriptionPlan::Insert(sub) | AddSubscriptionPlan::Refollow(sub) => {
                    assert!(sub.is_active(), "{existing:?} wrote a removed row")
                }
                AddSubscriptionPlan::KeepActive { .. } => panic!("{existing:?} kept nothing"),
            }
        }
    }
}
