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
    /// The LOCAL user's bare DID (fragment stripped). Held so
    /// `write_peer_claim` can reject `author_did == local_did` with
    /// `SelfAttribution` at the STORAGE write boundary — the WD-40
    /// defense-in-depth layer-2 guard, independent of the cli's pure
    /// pre-check (layer 1). Bound at construction from the composition
    /// root's `IdentityPort::author_did()`.
    local_did: Did,
}

impl DuckDbPeerStorageAdapter {
    /// Construct from a SHARED connection handle + the colocated
    /// `peer_claims/` root + the LOCAL user's DID. Called from
    /// `DuckDbStorageAdapter::peer_adapter` so both adapters write through
    /// the same mutex.
    ///
    /// The `local_did` is stored bare (fragment stripped) so the
    /// `write_peer_claim` SelfAttribution guard (WD-40) compares like-for-like
    /// against the record's bare author DID.
    ///
    /// This constructor does NOT open a second DuckDB handle and does NOT
    /// run migrations — migration v3 is run once at
    /// `DuckDbStorageAdapter::open` time (see `schema_v3::run_migration`).
    pub(crate) fn from_shared(
        conn: Arc<Mutex<Connection>>,
        peer_claims_root: PathBuf,
        local_did: &Did,
    ) -> Self {
        Self {
            conn,
            peer_claims_root,
            local_did: Did(bare_did(&local_did.0)),
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

/// Count cached `peer_claims` rows attributed to `peer_did`. Returns a
/// `u32` (the count drives the cli's "N cached peer claims retained" /
/// "Purged N" lines). Caller holds the connection lock.
fn count_peer_claims(conn: &Connection, peer_did: &Did) -> Result<u32, PeerStorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE author_did = ?",
            duckdb::params![peer_did.0],
            |row| row.get(0),
        )
        .map_err(|err| PeerStorageError::DuckDb(format!("count peer_claims: {err}")))?;
    Ok(count.max(0) as u32)
}

/// Count the user's OWN claims in the slice-01 `claims` (author) table.
/// The author store is single-tenant (the local user is the sole author),
/// so a global COUNT is the observable surface for "user counter-claims
/// were preserved" (WD-25 / WD-41). Hard-purge reads this purely to REPORT
/// the preserved count — it never deletes from `claims`. Caller holds the
/// connection lock.
fn count_author_claims(conn: &Connection) -> Result<u32, PeerStorageError> {
    let count: i64 = conn
        .query_row("SELECT count(*) FROM claims", [], |row| row.get(0))
        .map_err(|err| PeerStorageError::DuckDb(format!("count author claims: {err}")))?;
    Ok(count.max(0) as u32)
}

/// Best-effort removal of the on-disk peer-claim partition
/// `<peer_claims_root>/<encoded_did>/` (Q-DELIVER-2 colon→underscore
/// encoding). Runs AFTER the DB commit. A non-existent directory is a clean
/// no-op (nothing was ever cached on disk); a removal failure on an existing
/// directory is surfaced as an `Io` error so a half-purged filesystem cannot
/// pass as clean (KPI-FED-4 zero purge residue).
fn remove_peer_claims_dir(
    peer_claims_root: &std::path::Path,
    peer_did: &Did,
) -> Result<(), PeerStorageError> {
    let partition = peer_claims_root.join(did_to_fs_segment(&peer_did.0));
    if !partition.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&partition).map_err(PeerStorageError::Io)
}

/// Extract the inner `f64` from a `Confidence` without going through
/// `Confidence::value()` (still a RED smart-constructor scaffold in
/// `claim-domain`). `Confidence` serializes transparently to its inner
/// number, so a serde round-trip recovers the value. The smart constructor
/// already guaranteed `[0.0, 1.0]` at compose time, which also satisfies the
/// migration-v3 CHECK constraint.
fn confidence_as_f64(confidence: claim_domain::Confidence) -> Result<f64, PeerStorageError> {
    serde_json::to_value(confidence)
        .ok()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| {
            PeerStorageError::DuckDb("confidence value did not serialize to an f64".into())
        })
}

/// Encode a DID into the filesystem-safe partition segment used under
/// `peer_claims/<encoded_did>/` (Q-DELIVER-2): colons become underscores.
/// `did:plc:rachel-test` → `did_plc_rachel-test`. Single source of truth
/// shared with the acceptance-test `did_to_fs_segment` helper.
fn did_to_fs_segment(did: &str) -> String {
    did.replace(':', "_")
}

/// Strip the `#fragment` from a DID, returning the bare DID. The record's
/// `author` carries `did:plc:rachel-test#org.openlore.application`; the
/// subscribed-peer comparison + the stored `author_did` use the bare form.
fn bare_did(did: &str) -> String {
    did.split('#').next().unwrap_or(did).to_string()
}

/// The wire string for a `ReferenceType` (matches the migration-v3 CHECK
/// constraint enum on `peer_claim_references.ref_type`).
fn ref_type_str(ref_type: ReferenceType) -> &'static str {
    match ref_type {
        ReferenceType::Retracts => "retracts",
        ReferenceType::Corrects => "corrects",
        ReferenceType::Counters => "counters",
        ReferenceType::Supersedes => "supersedes",
    }
}

/// Write the on-disk `peer_claims/<encoded_did>/<cid>.json` artifact
/// atomically (tmp + fsync + rename, the same POSIX-atomic pattern the
/// slice-01 author-claim write uses). Content is the domain `SignedClaim`
/// serde shape — byte-consistent with `claims/<cid>.json`. Runs AFTER the
/// DB commit so a DB rollback never leaves an orphan artifact.
fn write_peer_claim_artifact(
    peer_claims_root: &std::path::Path,
    peer_did: &Did,
    cid: &Cid,
    signed: &SignedClaim,
) -> Result<(), PeerStorageError> {
    use std::io::Write;

    let partition = peer_claims_root.join(did_to_fs_segment(&peer_did.0));
    std::fs::create_dir_all(&partition).map_err(PeerStorageError::Io)?;

    let artifact = partition.join(format!("{}.json", cid.0));
    let artifact_tmp = artifact.with_extension("json.tmp");

    let bytes = serde_json::to_vec_pretty(signed).map_err(|err| {
        PeerStorageError::DuckDb(format!("serialize peer claim artifact {}: {err}", cid.0))
    })?;

    {
        let mut f = std::fs::File::create(&artifact_tmp).map_err(PeerStorageError::Io)?;
        f.write_all(&bytes).map_err(PeerStorageError::Io)?;
        f.sync_all().map_err(PeerStorageError::Io)?;
    }
    std::fs::rename(&artifact_tmp, &artifact).map_err(PeerStorageError::Io)?;
    Ok(())
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

    fn soft_remove(&self, peer_did: &Did) -> Result<SoftRemoveOutcome, PeerStorageError> {
        let conn = self.lock_conn()?;

        // No subscription row (active OR already soft-removed) ⇒ no-op.
        // US-FED-005 Example 4: "Not subscribed to <did>; nothing to
        // remove." is rendered by the cli off `was_subscribed = false`.
        if lookup_subscription_row(&conn, peer_did)?.is_none() {
            return Ok(SoftRemoveOutcome {
                was_subscribed: false,
                cached_claim_count: 0,
            });
        }

        // Count the cached peer_claims rows the soft-remove RETAINS — this
        // is the number the cli echoes ("N cached peer claims retained").
        // Counted BEFORE the UPDATE so a future regression that deletes
        // claims would surface as a mismatch in the soft-remove-isolation
        // probe (#5) rather than silently.
        let cached_claim_count = count_peer_claims(&conn, peer_did)?;

        // The ONLY mutation soft-remove performs: stamp `removed_at`. The
        // peer_claims rows are deliberately untouched (WD-25 / ADR-014).
        conn.execute(
            "UPDATE peer_subscriptions SET removed_at = now() WHERE peer_did = ?",
            duckdb::params![peer_did.0],
        )
        .map_err(|err| {
            PeerStorageError::DuckDb(format!("soft-remove update peer_subscriptions: {err}"))
        })?;

        Ok(SoftRemoveOutcome {
            was_subscribed: true,
            cached_claim_count,
        })
    }

    fn hard_purge(&self, peer_did: &Did) -> Result<HardPurgeOutcome, PeerStorageError> {
        // Acquire the single-writer lock as `mut` so we can open a
        // transaction (DuckDB's `Connection::transaction` needs `&mut self`).
        let mut conn = self.lock_conn()?;

        // Read the observable counts the cli renders BEFORE the deletes:
        //   - `deleted_peer_claim_count`: rows that the purge will remove;
        //   - `was_subscribed`: whether a subscription row exists (active OR
        //     soft-removed) — drives the idempotent no-op render.
        // The user's OWN counter-claims live in the slice-01 `claims` table,
        // which this operation NEVER touches; `preserved_user_counter_claim_count`
        // is read so the cli can report the preservation explicitly (WD-25 /
        // WD-41) and so a regression deleting them would surface as a count
        // mismatch rather than silently.
        let deleted_peer_claim_count = count_peer_claims(&conn, peer_did)?;
        let was_subscribed = lookup_subscription_row(&conn, peer_did)?.is_some();
        let preserved_user_counter_claim_count = count_author_claims(&conn)?;

        // ALL three DB deletes run in ONE transaction (ADR-014 atomicity:
        // either the subscription AND every cached peer_claim of this peer
        // disappear together, or nothing does). Delete order honors the
        // migration-v3 foreign keys:
        //   1. peer_claim_references / peer_claim_evidence reference
        //      peer_claims(cid) → delete the child rows FIRST (scoped to the
        //      peer's CIDs via the author_did subquery);
        //   2. peer_claims for this author;
        //   3. the peer_subscriptions row.
        // The author `claims` table is NEVER named — the own-vs-peer
        // separation is structural (different tables), not a runtime filter.
        let tx = conn
            .transaction()
            .map_err(|err| PeerStorageError::DuckDb(format!("begin hard-purge tx: {err}")))?;

        tx.execute(
            "DELETE FROM peer_claim_references WHERE referencing_cid IN \
                (SELECT cid FROM peer_claims WHERE author_did = ?)",
            duckdb::params![peer_did.0],
        )
        .map_err(|err| {
            PeerStorageError::DuckDb(format!("hard-purge delete peer_claim_references: {err}"))
        })?;

        tx.execute(
            "DELETE FROM peer_claim_evidence WHERE cid IN \
                (SELECT cid FROM peer_claims WHERE author_did = ?)",
            duckdb::params![peer_did.0],
        )
        .map_err(|err| {
            PeerStorageError::DuckDb(format!("hard-purge delete peer_claim_evidence: {err}"))
        })?;

        tx.execute(
            "DELETE FROM peer_claims WHERE author_did = ?",
            duckdb::params![peer_did.0],
        )
        .map_err(|err| PeerStorageError::DuckDb(format!("hard-purge delete peer_claims: {err}")))?;

        tx.execute(
            "DELETE FROM peer_subscriptions WHERE peer_did = ?",
            duckdb::params![peer_did.0],
        )
        .map_err(|err| {
            PeerStorageError::DuckDb(format!("hard-purge delete peer_subscriptions: {err}"))
        })?;

        tx.commit()
            .map_err(|err| PeerStorageError::DuckDb(format!("commit hard-purge tx: {err}")))?;

        // Effect-shell tail (AFTER the DB commit, per the data-models.md
        // "remove the directory after the DB commit" contract): best-effort
        // removal of the on-disk `peer_claims/<encoded_did>/` partition. A
        // missing directory is not an error (the purge may run before any
        // artifact landed); a removal failure on an existing directory IS
        // surfaced so a half-purged filesystem cannot masquerade as clean.
        remove_peer_claims_dir(&self.peer_claims_root, peer_did)?;

        Ok(HardPurgeOutcome {
            was_subscribed,
            deleted_peer_claim_count,
            preserved_user_counter_claim_count,
        })
    }

    fn write_peer_claim(
        &self,
        peer_did: &Did,
        signed: &SignedClaim,
        fetched_from_pds: &Url,
        fetched_at: DateTime<Utc>,
    ) -> Result<WritePeerClaimOutcome, PeerStorageError> {
        // Step 04-01 shipped the CrossAttribution arm (WD-41); step 04-05
        // adds the SelfAttribution arm (WD-40). The DB core row +
        // references/evidence side tables write in one transaction, then the
        // on-disk `peer_claims/<encoded_did>/<cid>.json` artifact (atomic
        // tmp+rename, Q-DELIVER-2 colon→underscore).

        let record_author = bare_did(&signed.unsigned.author_did.0);

        // SelfAttribution (WD-40 — LOAD-BEARING, layer-2 storage guard): a
        // record whose author is the LOCAL user is rejected at the WRITE
        // boundary, INDEPENDENTLY of the cli's pure pre-check (layer 1).
        // This is the key-compromise defense: even if the offending record's
        // signature verified against the user's own key, the storage layer
        // refuses to file it under peer_claims (I-FED-2: peer_claims.author_did
        // NEVER == local user). Checked BEFORE CrossAttribution so a record
        // self-attributed to the local user reports SelfAttribution even when
        // the local user is also (improbably) the subscribed peer.
        if record_author == self.local_did.0 {
            return Err(PeerStorageError::SelfAttribution);
        }

        // Anti-merging (WD-41): the record's author (bare DID, fragment
        // stripped) MUST equal the subscribed peer. A mismatch is a
        // cross-attributed record — reject BEFORE any write.
        if record_author != peer_did.0 {
            return Err(PeerStorageError::CrossAttribution {
                expected: peer_did.clone(),
                actual: Did(record_author),
            });
        }

        let cid = signed.signature.signed_cid.clone();

        {
            let conn = self.lock_conn()?;
            // Idempotent re-write (US-FED-002): an existing CID is a no-op.
            let already: i64 = conn
                .query_row(
                    "SELECT count(*) FROM peer_claims WHERE cid = ?",
                    duckdb::params![cid.0],
                    |row| row.get(0),
                )
                .map_err(|err| {
                    PeerStorageError::DuckDb(format!("write_peer_claim existence: {err}"))
                })?;
            if already > 0 {
                return Ok(WritePeerClaimOutcome { written: false });
            }
        }

        let unsigned = &signed.unsigned;
        let confidence = confidence_as_f64(unsigned.confidence)?;
        let signed_record_path = format!(
            "peer_claims/{}/{}.json",
            did_to_fs_segment(&peer_did.0),
            cid.0
        );

        // ALL THREE DB inserts (core row + references + evidence) run in ONE
        // transaction so a failure leaves no orphaned side-table rows
        // (mirrors the slice-01 author-claim atomicity contract).
        {
            let mut conn = self.lock_conn()?;
            let tx = conn.transaction().map_err(|err| {
                PeerStorageError::DuckDb(format!("begin write_peer_claim tx: {err}"))
            })?;

            tx.execute(
                "INSERT INTO peer_claims \
                    (cid, author_did, subject, predicate, object, confidence, \
                     composed_at, fetched_at, fetched_from_pds, signed_record_path) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                duckdb::params![
                    cid.0,
                    peer_did.0,
                    unsigned.subject,
                    unsigned.predicate,
                    unsigned.object,
                    confidence,
                    unsigned.composed_at,
                    fetched_at.naive_utc(),
                    fetched_from_pds.to_string(),
                    signed_record_path,
                ],
            )
            .map_err(|err| PeerStorageError::DuckDb(format!("insert peer_claims: {err}")))?;

            // Reference graph (denormalized from references[]).
            for reference in &unsigned.references {
                tx.execute(
                    "INSERT INTO peer_claim_references \
                        (referencing_cid, referenced_cid, ref_type) \
                     VALUES (?, ?, ?)",
                    duckdb::params![cid.0, reference.cid.0, ref_type_str(reference.ref_type)],
                )
                .map_err(|err| {
                    PeerStorageError::DuckDb(format!("insert peer_claim_references: {err}"))
                })?;
            }

            // Evidence URIs (denormalized, ordinal-keyed).
            for (ordinal, evidence) in unsigned.evidence.iter().enumerate() {
                tx.execute(
                    "INSERT INTO peer_claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)",
                    duckdb::params![cid.0, evidence, ordinal as i32],
                )
                .map_err(|err| {
                    PeerStorageError::DuckDb(format!("insert peer_claim_evidence: {err}"))
                })?;
            }

            tx.commit().map_err(|err| {
                PeerStorageError::DuckDb(format!("commit write_peer_claim tx: {err}"))
            })?;
        }

        // Effect-shell tail (AFTER the DB commit): the on-disk artifact.
        // Same `<cid>.json.tmp` → fsync → rename atomic pattern as the
        // slice-01 author-claim write. Content is the domain `SignedClaim`
        // serde shape (consistent with `claims/<cid>.json`).
        write_peer_claim_artifact(&self.peer_claims_root, peer_did, &cid, signed)?;

        Ok(WritePeerClaimOutcome { written: true })
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

// -----------------------------------------------------------------------------
// Layer-2 SelfAttribution guard — adapter integration tests (Mandate 6: real
// DuckDB through the public `DuckDbStorageAdapter::open` → `peer_adapter`
// driving surface). These drive the WD-40 storage-boundary guard (step 04-05)
// that is INDEPENDENT of the cli's pure pre-check (layer 1): even a record
// that would pass signature + CID round-trip (the key-compromise vector) MUST
// be rejected at the write boundary when its author is the local user.
//
// Behavior budget: 1 distinct behavior (write_peer_claim rejects a
// self-attributed record + writes no row) → 1 test, within the 2×1=2 budget.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DuckDbStorageAdapter;
    use claim_domain::{Confidence, SignatureBlock, SignedClaim, UnsignedClaim};
    use tempfile::tempdir;

    /// Build a `SignedClaim` authored by `author_did` (bare DID; the
    /// `#fragment` is appended so the guard's `bare_did` comparison is the
    /// thing under test). `cid` becomes both the `signed_cid` and the row's
    /// primary key. The signature bytes are a placeholder — the STORAGE guard
    /// is upstream of any crypto, so the signature need not verify (the WD-40
    /// contract is "reject even if the signature WOULD verify").
    fn signed_claim_authored_by(author_did: &str, cid: &str) -> SignedClaim {
        // `Confidence` is crate-private; route through serde (the pattern used
        // across the workspace's adapter/test code).
        let confidence: Confidence = serde_json::from_value(serde_json::json!(0.42))
            .expect("0.42 is a well-formed confidence");
        SignedClaim {
            unsigned: UnsignedClaim {
                subject: "github:rust-lang/cargo".to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: "org.openlore.philosophy.dependency-pinning".to_string(),
                evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                confidence,
                author_did: Did(format!("{author_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            },
            signature: SignatureBlock {
                signed_cid: Cid(cid.to_string()),
                signature_bytes: vec![0u8; 64],
                verification_method: format!("{author_did}#org.openlore.application"),
            },
        }
    }

    /// Open a fresh adapter on a tmp DB and hand back the peer adapter bound
    /// to `local_did`, plus the owning tempdir + author adapter (kept alive so
    /// the shared connection + on-disk root outlive the test).
    fn open_peer_adapter(
        local_did: &str,
    ) -> (
        tempfile::TempDir,
        DuckDbStorageAdapter,
        DuckDbPeerStorageAdapter,
    ) {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("openlore.duckdb");
        let storage = DuckDbStorageAdapter::open(&db_path).expect("open adapter");
        let peer = storage.peer_adapter(&Did(local_did.to_string()));
        (dir, storage, peer)
    }

    /// WD-40 (step 04-05): `write_peer_claim` rejects a record whose
    /// `author_did` equals the LOCAL user's DID with
    /// `PeerStorageError::SelfAttribution` — at the STORAGE write boundary,
    /// independently of the cli's pure pre-check. Crucially this holds for the
    /// key-compromise vector: the local user is ALSO the subscribed peer here
    /// (`peer_did == local_did`), so the CrossAttribution arm (author ≠ peer)
    /// can NOT fire — only a dedicated SelfAttribution guard catches it. No
    /// `peer_claims` row is written (I-FED-2: author_did NEVER == local user).
    #[test]
    fn write_peer_claim_rejects_self_attributed_record_at_storage_boundary() {
        let local_did = "did:plc:test-jeff";
        let (_dir, storage, peer) = open_peer_adapter(local_did);

        // The offending record self-attributes to the local user. We pass the
        // SAME DID as the `peer_did` argument so the existing CrossAttribution
        // arm (author ≠ subscribed peer) does NOT fire — isolating the
        // SelfAttribution guard as the ONLY thing that can reject this write.
        let signed =
            signed_claim_authored_by(local_did, "bafyselfattrtest000000000000000000000000000");
        let endpoint = Url::parse("https://pds.example.test").expect("valid url");

        let result =
            peer.write_peer_claim(&Did(local_did.to_string()), &signed, &endpoint, Utc::now());

        assert!(
            matches!(result, Err(PeerStorageError::SelfAttribution)),
            "write_peer_claim MUST reject a record self-attributed to the local user \
             with PeerStorageError::SelfAttribution (WD-40 layer-2 guard); got {result:?}"
        );

        // No row leaked into peer_claims under ANY author_did (I-FED-2 +
        // anti-merging at the reject path). Asserted through the shared
        // connection the author adapter holds open.
        let conn = storage
            .peer_adapter(&Did(local_did.to_string()))
            .shared_connection()
            .clone();
        let conn = conn.lock().expect("lock conn");
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM peer_claims", [], |r| r.get(0))
            .expect("count peer_claims");
        assert_eq!(
            rows, 0,
            "a self-attributed write must leave ZERO peer_claims rows (I-FED-2); got {rows}"
        );
    }
}
