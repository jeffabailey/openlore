//! `peer_storage` — `DuckDbPeerStorageAdapter`, the EFFECT-shell impl of
//! the slice-03 `PeerStoragePort` over the SAME single-file DuckDB store
//! as `DuckDbStorageAdapter`.
//!
//! ## Shared single-writer connection (Q-DELIVER-3)
//!
//! DuckDB is single-writer: two independent `Connection`s to the same
//! file would race. Per the Q-DELIVER-3 resolution, this adapter SHARES
//! the very same `Arc<Mutex<Connection>>` handle as the slice-01
//! `DuckDbStorageAdapter` (see `DuckDbStorageAdapter::peer_adapter`).
//! All writes serialize through one mutex; no second open handle exists.
//!
//! ## On-disk layout (data-models.md §"On-disk artifact format")
//!
//! Peer claims are cached at
//! `<root>/peer_claims/<peer_did>/<cid>.json`, partitioned by peer DID so
//! that hard-purge is a directory removal. The `<peer_did>` path segment
//! is filesystem-safe-encoded (colons → underscores); the exact encoding
//! lands with the `write_peer_claim` implementation in a later slice-03
//! step.
//!
//! ## SCAFFOLD status
//!
//! SCAFFOLD: true (slice-03)
//!
//! Every `PeerStoragePort` method body below is a `todo!()` stub at this
//! bootstrap step (01-02). The real implementations are driven by the
//! PS-* / PP-* acceptance scenarios in Phase 03/04. Only the struct
//! skeleton + the shared-connection wiring + migration v3 (in
//! `schema_v3.rs`) are LIVE here. The `probe()` body is likewise a
//! scaffold; `xtask check-probes` is informed of its stub status via the
//! `// SCAFFOLD: true (slice-03)` marker so it does not yet demand a
//! non-stub body for the new port (I-FED-3 enforcement activates once the
//! real probe lands).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, ReferenceType, SignedClaim};
use duckdb::Connection;
use ports::{
    AddSubscriptionOutcome, HardPurgeOutcome, PeerStorageError, PeerStoragePort, PeerSubscription,
    ProbeOutcome, SoftRemoveOutcome, WritePeerClaimOutcome,
};
use url::Url;

/// Embedded-DuckDB `PeerStoragePort` adapter.
///
/// Shares the underlying `Connection` (behind an `Arc<Mutex<_>>` because
/// DuckDB's `Connection` is `!Sync` AND we need the SAME handle as
/// `DuckDbStorageAdapter` to honor the single-writer constraint). The
/// `peer_claims_root` is the colocated `peer_claims/` directory where
/// per-peer artifact subtrees live.
pub struct DuckDbPeerStorageAdapter {
    conn: Arc<Mutex<Connection>>,
    peer_claims_root: PathBuf,
}

impl DuckDbPeerStorageAdapter {
    /// Construct from a SHARED connection handle + the colocated
    /// `peer_claims/` root. Called from `DuckDbStorageAdapter::peer_adapter`
    /// so both adapters write through the same mutex.
    ///
    /// This constructor does NOT open a second DuckDB handle and does NOT
    /// run migrations — migration v3 is run once at
    /// `DuckDbStorageAdapter::open` time (see `schema_v3::run_migration`).
    pub(crate) fn from_shared(conn: Arc<Mutex<Connection>>, peer_claims_root: PathBuf) -> Self {
        Self {
            conn,
            peer_claims_root,
        }
    }

    /// The colocated `peer_claims/` root directory. Exposed for the
    /// (later) `write_peer_claim` / `hard_purge` implementations and for
    /// probe sentinels.
    #[allow(dead_code)]
    pub(crate) fn peer_claims_root(&self) -> &PathBuf {
        &self.peer_claims_root
    }

    /// Borrow the shared connection handle. Internal helper for the
    /// (later) real method bodies; retained now so the field is read and
    /// the single-writer contract is documented at one call site.
    #[allow(dead_code)]
    pub(crate) fn shared_connection(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }

    /// Acquire the shared single-writer lock. A poisoned mutex (a previous
    /// holder panicked) surfaces as a `DuckDb` error rather than a panic so
    /// callers compose railway-style.
    fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>, PeerStorageError> {
        self.conn
            .lock()
            .map_err(|_| PeerStorageError::DuckDb("peer-storage connection mutex poisoned".into()))
    }
}

// -----------------------------------------------------------------------------
// Row helpers — pure-ish projections from a DuckDB row to the port ADTs.
// Kept as free functions so both the trait methods and a held lock guard
// can reuse them without re-locking.
// -----------------------------------------------------------------------------

/// Look up one subscription row by DID (active OR soft-removed). Returns
/// `Ok(None)` when no row exists. Caller holds the connection lock.
fn lookup_subscription_row(
    conn: &Connection,
    peer_did: &Did,
) -> Result<Option<PeerSubscription>, PeerStorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT peer_did, peer_handle, peer_pds_endpoint, subscribed_at, removed_at \
             FROM peer_subscriptions WHERE peer_did = ?",
        )
        .map_err(|err| PeerStorageError::DuckDb(format!("prepare lookup_subscription: {err}")))?;

    let mut rows = stmt
        .query_map(duckdb::params![peer_did.0], row_to_subscription)
        .map_err(|err| PeerStorageError::DuckDb(format!("query lookup_subscription: {err}")))?;

    match rows.next() {
        Some(row) => Ok(Some(row.map_err(|err| {
            PeerStorageError::DuckDb(format!("read lookup_subscription row: {err}"))
        })?)),
        None => Ok(None),
    }
}

/// Map a `peer_subscriptions` result row to a `PeerSubscription`. A stored
/// `peer_pds_endpoint` that no longer parses as a URL is a data-corruption
/// signal surfaced as a duckdb error (never silently dropped).
fn row_to_subscription(row: &duckdb::Row<'_>) -> Result<PeerSubscription, duckdb::Error> {
    let peer_did: String = row.get(0)?;
    let peer_handle: String = row.get(1)?;
    let endpoint_str: String = row.get(2)?;
    let subscribed_at: DateTime<Utc> = row.get(3)?;
    let removed_at: Option<DateTime<Utc>> = row.get(4)?;

    let peer_pds_endpoint = Url::parse(&endpoint_str).map_err(|err| {
        duckdb::Error::FromSqlConversionFailure(
            2,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("stored peer_pds_endpoint {endpoint_str:?} is not a valid URL: {err}"),
            )),
        )
    })?;

    Ok(PeerSubscription {
        peer_did: Did(peer_did),
        peer_handle,
        peer_pds_endpoint,
        subscribed_at,
        removed_at,
    })
}

// -----------------------------------------------------------------------------
// `PeerStoragePort` impl — all bodies are RED scaffolds at step 01-02.
// Real impls land driven by PS-* / PP-* scenarios in Phase 03/04.
// -----------------------------------------------------------------------------

impl PeerStoragePort for DuckDbPeerStorageAdapter {
    fn probe(&self) -> ProbeOutcome {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::probe — peer-storage probe gauntlet (ADR-014) lands in a later slice-03 step")
    }

    fn add_subscription(
        &self,
        sub: PeerSubscription,
    ) -> Result<AddSubscriptionOutcome, PeerStorageError> {
        let conn = self.lock_conn()?;

        // Idempotency (US-FED-001 AC #3): if a row already exists for this
        // DID — active OR soft-removed — report `AlreadyExisted` with its
        // original `subscribed_at` and do NOT insert a duplicate. The
        // PRIMARY KEY on `peer_did` would reject a second insert anyway;
        // checking first lets us surface the original timestamp the cli
        // renders without relying on a constraint-violation error string.
        if let Some(existing) = lookup_subscription_row(&conn, &sub.peer_did)? {
            return Ok(AddSubscriptionOutcome::AlreadyExisted {
                since: existing.subscribed_at,
            });
        }

        conn.execute(
            "INSERT INTO peer_subscriptions \
                (peer_did, peer_handle, peer_pds_endpoint, subscribed_at, removed_at) \
             VALUES (?, ?, ?, ?, NULL)",
            duckdb::params![
                sub.peer_did.0,
                sub.peer_handle,
                sub.peer_pds_endpoint.to_string(),
                sub.subscribed_at.naive_utc(),
            ],
        )
        .map_err(|err| PeerStorageError::DuckDb(format!("insert peer_subscriptions: {err}")))?;

        Ok(AddSubscriptionOutcome::Added {
            subscribed_at: sub.subscribed_at,
        })
    }

    fn list_active_subscriptions(&self) -> Result<Vec<PeerSubscription>, PeerStorageError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT peer_did, peer_handle, peer_pds_endpoint, subscribed_at, removed_at \
                 FROM peer_subscriptions \
                 WHERE removed_at IS NULL \
                 ORDER BY subscribed_at",
            )
            .map_err(|err| {
                PeerStorageError::DuckDb(format!("prepare list_active_subscriptions: {err}"))
            })?;

        let rows = stmt
            .query_map([], row_to_subscription)
            .map_err(|err| {
                PeerStorageError::DuckDb(format!("query list_active_subscriptions: {err}"))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                PeerStorageError::DuckDb(format!("read list_active_subscriptions row: {err}"))
            })?;

        Ok(rows)
    }

    fn lookup_subscription(
        &self,
        peer_did: &Did,
    ) -> Result<Option<PeerSubscription>, PeerStorageError> {
        let conn = self.lock_conn()?;
        lookup_subscription_row(&conn, peer_did)
    }

    fn soft_remove(&self, _peer_did: &Did) -> Result<SoftRemoveOutcome, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::soft_remove — driven by PS-* scenarios")
    }

    fn hard_purge(&self, _peer_did: &Did) -> Result<HardPurgeOutcome, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::hard_purge — driven by PS-* scenarios")
    }

    fn write_peer_claim(
        &self,
        _peer_did: &Did,
        _signed: &SignedClaim,
        _fetched_from_pds: &Url,
        _fetched_at: DateTime<Utc>,
    ) -> Result<WritePeerClaimOutcome, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::write_peer_claim — driven by PP-* scenarios")
    }

    fn get_peer_claim_by_cid(
        &self,
        _cid: &Cid,
    ) -> Result<Option<(Did, SignedClaim)>, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::get_peer_claim_by_cid — driven by PP-* scenarios")
    }

    fn list_peer_claims_by_subject(
        &self,
        _subject: &str,
    ) -> Result<Vec<(Did, SignedClaim)>, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::list_peer_claims_by_subject — driven by PP-* scenarios")
    }

    fn query_peer_referencing(
        &self,
        _target_cid: &Cid,
    ) -> Result<Vec<(Did, Cid, ReferenceType)>, PeerStorageError> {
        // SCAFFOLD: true (slice-03)
        todo!("PeerStoragePort::query_peer_referencing — driven by PP-* scenarios")
    }
}
