//! `FakePds` — deterministic `PdsPort` test double.
//!
//! Step 04-06 (DD-6): the canonical PDS test double used across slice-01
//! acceptance tests and the WS-9 / WS-10 / FR-1/2/3/4 scenarios. Real
//! ATProto PDS integration lands in `adapter-atproto-pds`; this fake is
//! the test seam that lets pure-core + cli composition be exercised
//! without touching a real PDS over the network.
//!
//! Functional-paradigm note (ADR-007): the fake owns a small piece of
//! `Arc<Mutex<...>>` state because `PdsPort` methods take `&self` (the
//! port contract is shared-reference, not consuming). Mutations happen
//! through the trait's async methods which the cli drives concurrently —
//! the mutex is the minimum primitive that keeps `Send + Sync` honest.
//! No external mutation surface is exposed; helpers like
//! [`FakePds::record_count`] read the snapshot under the same lock.
//!
//! ## Insertion model
//!
//! `create_record(collection, rkey, body)` writes a `FakePdsRecord` to an
//! in-memory `Vec`, keyed by the synthesized AT URI
//! `at://<author_did>/<collection>/<rkey>`. The author DID is the DID
//! configured at construction (`FakePds::for_did`) or the literal
//! `"did:plc:test-fake"` for the default constructor.
//!
//! ### Idempotency on rkey collision (WS-9 precondition)
//!
//! When `create_record` is called with a `(collection, rkey)` pair that
//! already exists, the fake DOES NOT insert a duplicate. It returns the
//! existing AT URI verbatim. This mirrors the real adapter's "treat 409
//! as idempotent success" behavior pinned in architecture §6.2.
//!
//! ### Unreachable simulation (WS-10 sad-path)
//!
//! `simulate_unreachable()` flips an atomic flag; subsequent
//! `create_record` calls return `PdsError::Unreachable`. `restore()`
//! flips it back. Read paths (`get_record`, `list_records`) also honor
//! the flag — a downed PDS is unreachable for reads too.
//!
//! ## RED-baseline replacement
//!
//! The previous scaffold lived inline in `lib.rs` with all bodies
//! `panic!("Not yet implemented")`. This module is the real
//! implementation; `lib.rs` keeps the in-memory shape re-exported flat
//! so call sites continue to write `openlore_test_support::FakePds::new()`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ports::{AtUri, PdsError, PdsPort, ProbeOutcome};
use serde_json::Value;

/// One record as observed by the fake PDS.
///
/// Mirrors the shape of a row in a real ATProto repo: the collection
/// (`org.openlore.claim`), the rkey (the claim's CID per ADR-004), the
/// JSON body, the author DID, and the synthesized AT URI. Acceptance
/// tests pattern-match on these fields to assert "this claim was
/// published to the PDS with the expected shape".
#[derive(Debug, Clone, PartialEq)]
pub struct FakePdsRecord {
    pub collection: String,
    pub rkey: String,
    pub body: Value,
    pub author_did: String,
    pub at_uri: String,
}

/// Internal mutable state. Held inside an `Arc` so the fake can be
/// cloned through the `PdsPort` test seam if a test ever needs to.
#[derive(Debug, Default)]
struct State {
    records: Mutex<Vec<FakePdsRecord>>,
    unreachable: AtomicBool,
}

/// Deterministic `PdsPort` test double.
///
/// One value per scenario. Configured with an author DID at construction
/// (defaults to `"did:plc:test-fake"`). All `create_record` calls
/// synthesize an AT URI under that DID. Insertion is append-only
/// (with rkey-collision dedup); reads return cloned snapshots.
///
/// Cloning the fake clones the `Arc<State>` — both clones see the same
/// underlying record store. This is intentional: a composition root may
/// hand out the same `PdsPort` to multiple call sites and the test
/// observes the union of their writes.
#[derive(Debug, Clone)]
pub struct FakePds {
    author_did: String,
    state: Arc<State>,
}

impl FakePds {
    /// Construct a fake bound to the default test author DID
    /// `"did:plc:test-fake"`. Use [`FakePds::for_did`] when a scenario
    /// needs a specific DID (e.g. `did:plc:test-jeff` or
    /// `did:plc:test-maria`).
    pub fn new() -> Self {
        Self::for_did("did:plc:test-fake")
    }

    /// Construct a fake bound to the given author DID. The DID is used
    /// to synthesize AT URIs in `create_record`.
    pub fn for_did(author_did: impl Into<String>) -> Self {
        Self {
            author_did: author_did.into(),
            state: Arc::new(State::default()),
        }
    }

    /// Snapshot of all records the fake has accepted so far. Returned by
    /// value (a fresh `Vec` of clones) so the caller can hold it across
    /// further `create_record` calls without holding the lock.
    pub fn records(&self) -> Vec<FakePdsRecord> {
        self.state
            .records
            .lock()
            .expect("fake pds mutex poisoned")
            .clone()
    }

    /// Find one record by its AT URI. Returns `None` if no record at
    /// that URI has been inserted.
    pub fn record_at(&self, at_uri: &str) -> Option<FakePdsRecord> {
        self.state
            .records
            .lock()
            .expect("fake pds mutex poisoned")
            .iter()
            .find(|r| r.at_uri == at_uri)
            .cloned()
    }

    /// Number of distinct records the fake has accepted. Idempotent
    /// re-inserts on the same `(collection, rkey)` do NOT increment
    /// this — a property test pins that contract.
    pub fn record_count(&self) -> usize {
        self.state
            .records
            .lock()
            .expect("fake pds mutex poisoned")
            .len()
    }

    /// Toggle the "unreachable" failure mode on. Subsequent
    /// `create_record` / `get_record` / `list_records` calls return
    /// `PdsError::Unreachable`. Used by WS-10 (PDS-is-down sad path).
    pub fn simulate_unreachable(&self) {
        self.state.unreachable.store(true, Ordering::SeqCst);
    }

    /// Toggle the "unreachable" failure mode off. Restores normal read
    /// + write operation. Inverse of [`FakePds::simulate_unreachable`].
    pub fn restore(&self) {
        self.state.unreachable.store(false, Ordering::SeqCst);
    }

    /// Build the AT URI for a record under this fake's configured DID.
    /// `at://<author_did>/<collection>/<rkey>` — the ATProto-canonical
    /// shape. Pulled out so tests can synthesize URIs for assertions
    /// without re-implementing the format.
    fn synth_at_uri(&self, collection: &str, rkey: &str) -> String {
        format!("at://{}/{collection}/{rkey}", self.author_did)
    }

    /// Internal: are we currently simulating unreachable? Read under
    /// `SeqCst` to pair with the writes in [`simulate_unreachable`] /
    /// [`restore`].
    fn is_unreachable(&self) -> bool {
        self.state.unreachable.load(Ordering::SeqCst)
    }
}

impl Default for FakePds {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PdsPort for FakePds {
    /// Test doubles always probe `Ok`. The real
    /// `adapter-atproto-pds` probes TLS / describeServer DID / rkey
    /// idempotency; none of those are observable on the fake. Refusal
    /// paths live in the real adapter's integration suite.
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    /// Write the record to the in-memory store and return its synthesized
    /// AT URI. Idempotent on `(collection, rkey)` collisions: a second
    /// call with the same `(collection, rkey)` returns the existing
    /// AT URI verbatim and does NOT insert a duplicate. Matches the real
    /// adapter's "409 conflict treated as success" semantics per
    /// architecture §6.2.
    ///
    /// Returns `PdsError::Unreachable` if `simulate_unreachable()` is
    /// currently engaged.
    async fn create_record(
        &self,
        collection: &str,
        rkey: &str,
        body: Value,
    ) -> Result<AtUri, PdsError> {
        if self.is_unreachable() {
            return Err(PdsError::Unreachable {
                message: format!(
                    "fake pds is simulating unreachable; cannot write {collection}/{rkey}"
                ),
            });
        }

        let at_uri = self.synth_at_uri(collection, rkey);
        let mut records = self.state.records.lock().expect("fake pds mutex poisoned");

        // Idempotency: if a record with this (collection, rkey) already
        // exists, return the existing AT URI without modifying state.
        // The real adapter exhibits the same shape on 409/conflict per
        // architecture §6.2.
        let already_present = records
            .iter()
            .any(|r| r.collection == collection && r.rkey == rkey);
        if !already_present {
            records.push(FakePdsRecord {
                collection: collection.to_string(),
                rkey: rkey.to_string(),
                body,
                author_did: self.author_did.clone(),
                at_uri: at_uri.clone(),
            });
        }

        Ok(AtUri(at_uri))
    }

    /// Look up a record by `(collection, rkey)`. Returns the JSON body
    /// of the most recently inserted matching record, or `None` if no
    /// record exists at that key.
    ///
    /// Returns `PdsError::Unreachable` if `simulate_unreachable()` is
    /// currently engaged.
    async fn get_record(&self, collection: &str, rkey: &str) -> Result<Option<Value>, PdsError> {
        if self.is_unreachable() {
            return Err(PdsError::Unreachable {
                message: format!(
                    "fake pds is simulating unreachable; cannot read {collection}/{rkey}"
                ),
            });
        }

        let records = self.state.records.lock().expect("fake pds mutex poisoned");
        let found = records
            .iter()
            .rev()
            .find(|r| r.collection == collection && r.rkey == rkey)
            .map(|r| r.body.clone());
        Ok(found)
    }

    /// Return all record bodies for the given collection, in insertion
    /// order. Deterministic so acceptance tests can pattern-match by
    /// index ("the third claim Jeff published is about Mastodon").
    ///
    /// Returns `PdsError::Unreachable` if `simulate_unreachable()` is
    /// currently engaged.
    async fn list_records(&self, collection: &str) -> Result<Vec<Value>, PdsError> {
        if self.is_unreachable() {
            return Err(PdsError::Unreachable {
                message: format!("fake pds is simulating unreachable; cannot list {collection}"),
            });
        }

        let records = self.state.records.lock().expect("fake pds mutex poisoned");
        Ok(records
            .iter()
            .filter(|r| r.collection == collection)
            .map(|r| r.body.clone())
            .collect())
    }
}

// -----------------------------------------------------------------------------
// Unit tests — the FakePds contract is load-bearing for slice-01 acceptance
// scenarios, so we pin its shape with real async-runtime tests here.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const COLLECTION: &str = "org.openlore.claim";

    /// Roundtrip: `create_record` then `get_record` returns the body
    /// byte-for-byte. The load-bearing contract for FR-1/FR-2/FR-3 —
    /// without this, federation roundtrip acceptance cannot wire.
    #[tokio::test]
    async fn create_then_get_roundtrips_body_bytes() {
        let fake = FakePds::for_did("did:plc:test-jeff");
        let body = json!({
            "subject": "github:rust-lang/rust",
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.memory-safety",
        });

        let at_uri = fake
            .create_record(COLLECTION, "bafy_test_cid_001", body.clone())
            .await
            .expect("create succeeds");

        assert_eq!(
            at_uri.0, "at://did:plc:test-jeff/org.openlore.claim/bafy_test_cid_001",
            "AT URI must follow at://<did>/<collection>/<rkey> shape"
        );

        let fetched = fake
            .get_record(COLLECTION, "bafy_test_cid_001")
            .await
            .expect("get succeeds");
        assert_eq!(
            fetched,
            Some(body),
            "get_record must return the exact body bytes inserted"
        );
    }

    /// Idempotency: re-inserting the same `(collection, rkey)` returns
    /// the same AT URI and DOES NOT increment record_count. Mirrors the
    /// real adapter's "409 treated as success" semantics (WS-9
    /// precondition).
    #[tokio::test]
    async fn rkey_collision_is_idempotent_no_duplicate_insert() {
        let fake = FakePds::new();
        let body_v1 = json!({"version": 1});
        let body_v2 = json!({"version": 2}); // would-be overwrite, must be ignored

        let uri_first = fake
            .create_record(COLLECTION, "bafy_collision_rkey", body_v1.clone())
            .await
            .expect("first insert");
        let uri_second = fake
            .create_record(COLLECTION, "bafy_collision_rkey", body_v2)
            .await
            .expect("second insert (collision)");

        assert_eq!(
            uri_first, uri_second,
            "rkey collision must return the same AT URI both times"
        );
        assert_eq!(
            fake.record_count(),
            1,
            "rkey collision must NOT increment record_count"
        );

        let fetched = fake
            .get_record(COLLECTION, "bafy_collision_rkey")
            .await
            .expect("get succeeds")
            .expect("record exists");
        assert_eq!(
            fetched, body_v1,
            "on rkey collision the original body must be preserved (no silent overwrite)"
        );
    }

    /// Unreachable toggle: after `simulate_unreachable()`, every port
    /// method returns `PdsError::Unreachable`. After `restore()`,
    /// operations succeed again. Covers WS-10 sad-path.
    #[tokio::test]
    async fn simulate_unreachable_blocks_writes_and_reads_then_restore_unblocks() {
        let fake = FakePds::new();
        // Seed one record while reachable.
        fake.create_record(COLLECTION, "rkey_seed", json!({"ok": true}))
            .await
            .expect("seed");
        assert_eq!(fake.record_count(), 1);

        // Engage failure mode.
        fake.simulate_unreachable();

        let write = fake
            .create_record(COLLECTION, "rkey_new", json!({"x": 1}))
            .await;
        assert!(
            matches!(write, Err(PdsError::Unreachable { .. })),
            "create_record must return Unreachable while simulated, got {write:?}"
        );
        let read = fake.get_record(COLLECTION, "rkey_seed").await;
        assert!(
            matches!(read, Err(PdsError::Unreachable { .. })),
            "get_record must return Unreachable while simulated, got {read:?}"
        );
        let list = fake.list_records(COLLECTION).await;
        assert!(
            matches!(list, Err(PdsError::Unreachable { .. })),
            "list_records must return Unreachable while simulated, got {list:?}"
        );

        // The seeded record must survive the outage.
        assert_eq!(
            fake.record_count(),
            1,
            "seeded records must persist across simulated outage"
        );

        // Restore and confirm operations succeed.
        fake.restore();
        let read_after = fake
            .get_record(COLLECTION, "rkey_seed")
            .await
            .expect("read after restore");
        assert_eq!(read_after, Some(json!({"ok": true})));
    }

    /// `list_records` returns inserted records in insertion order and
    /// filters by collection. Determinism here is load-bearing for FR-1
    /// (round-trip three claims, assert by index).
    #[tokio::test]
    async fn list_records_returns_inserted_in_order_filtered_by_collection() {
        let fake = FakePds::for_did("did:plc:test-jeff");
        fake.create_record(COLLECTION, "rkey_a", json!({"i": 0}))
            .await
            .expect("ins a");
        fake.create_record(COLLECTION, "rkey_b", json!({"i": 1}))
            .await
            .expect("ins b");
        // A record under a different collection MUST be filtered out.
        fake.create_record("org.other.collection", "rkey_z", json!({"i": 99}))
            .await
            .expect("ins z");
        fake.create_record(COLLECTION, "rkey_c", json!({"i": 2}))
            .await
            .expect("ins c");

        let listed = fake.list_records(COLLECTION).await.expect("list");
        assert_eq!(
            listed,
            vec![json!({"i": 0}), json!({"i": 1}), json!({"i": 2})],
            "list_records must return only the queried collection in insertion order"
        );
    }

    /// `record_at` returns the inserted record by AT URI. Synthesized URI
    /// shape: `at://<did>/<collection>/<rkey>`.
    #[tokio::test]
    async fn record_at_finds_by_synthesized_at_uri() {
        let fake = FakePds::for_did("did:plc:test-maria");
        fake.create_record(COLLECTION, "bafy_maria_001", json!({"k": "v"}))
            .await
            .expect("insert");

        let at_uri = "at://did:plc:test-maria/org.openlore.claim/bafy_maria_001";
        let found = fake.record_at(at_uri).expect("record present");
        assert_eq!(found.collection, COLLECTION);
        assert_eq!(found.rkey, "bafy_maria_001");
        assert_eq!(found.author_did, "did:plc:test-maria");
        assert_eq!(found.body, json!({"k": "v"}));
        assert_eq!(found.at_uri, at_uri);
    }
}
