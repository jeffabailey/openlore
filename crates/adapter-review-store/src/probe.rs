//! Earned-Trust probe for the private store (ADR-074). Three hard arms:
//!
//! 1. schema version: the file is exactly this build's version;
//! 2. AEAD canary: a sealed canary opens under its own binding and a blob
//!    re-bound to another owner does not;
//! 3. cross-owner canary: inside a rolled-back transaction, a row written
//!    for one owner is invisible to an owner-scoped read for another.
//!
//! The arm verdicts are pure; the glue below only gathers their inputs.

use duckdb::Connection;
use ports::{ProbeOutcome, ProbeRefusalReason};
use serde_json::json;

use crate::schema::{schema_verdict, SchemaVerdict, SCHEMA_VERSION};
use crate::{db_error, recorded_version, ReviewStore, StoreError};

const CANARY_OWNER: &str = "did:plc:probe-canary-owner";
const OTHER_OWNER: &str = "did:plc:probe-canary-other";
const CANARY_PLAINTEXT: &[u8] = b"openlore-review-store-canary";

const INSERT_CANARY_ACCOUNT: &str = "INSERT INTO accounts \
    (owner_did, handle_last_seen, pds_endpoint_last_seen, created_at, last_seen_at) \
    VALUES (?, 'probe.invalid', 'https://probe.invalid', now(), now())";
const COUNT_ACCOUNTS_FOR_OWNER: &str = "SELECT count(*) FROM accounts WHERE owner_did = ?";

/// One arm's verdict.
type Arm = Result<(), (ProbeRefusalReason, String)>;

/// Run every hard arm; the first refusal wins.
pub(crate) fn run(store: &ReviewStore) -> ProbeOutcome {
    let arms: [fn(&ReviewStore) -> Arm; 3] = [schema_arm, aead_arm, cross_owner_arm];
    match arms.iter().find_map(|arm| arm(store).err()) {
        None => ProbeOutcome::Ok,
        Some((reason, detail)) => ProbeOutcome::Refused {
            reason,
            structured: json!({ "probe": "review-store", "detail": detail }),
            detail,
        },
    }
}

fn schema_arm(store: &ReviewStore) -> Arm {
    let recorded = store
        .with_connection(recorded_version)
        .map_err(|e| (ProbeRefusalReason::ReviewStoreSchemaMismatch, e.to_string()))?;
    classify_schema(recorded)
}

/// Pure: the probe admits only a file that records this build's version.
pub(crate) fn classify_schema(recorded: Option<i64>) -> Arm {
    match schema_verdict(recorded) {
        SchemaVerdict::Current => Ok(()),
        other => Err((
            ProbeRefusalReason::ReviewStoreSchemaMismatch,
            format!("schema is {other:?}, this build is version {SCHEMA_VERSION}"),
        )),
    }
}

fn aead_arm(store: &ReviewStore) -> Arm {
    let own = aad_for(CANARY_OWNER);
    let other = aad_for(OTHER_OWNER);
    let sealed = store.seal(&own, CANARY_PLAINTEXT);
    let opened = sealed.as_ref().ok().map(|blob| store.unseal(&own, blob));
    let rebound = sealed.as_ref().ok().map(|blob| store.unseal(&other, blob));
    classify_aead(
        opened.and_then(Result::ok).as_deref(),
        rebound.is_some_and(|r| r.is_err()),
    )
}

/// Pure: the canary must round-trip and must not open under another binding.
pub(crate) fn classify_aead(opened: Option<&[u8]>, rebound_refused: bool) -> Arm {
    match (opened == Some(CANARY_PLAINTEXT), rebound_refused) {
        (true, true) => Ok(()),
        (false, _) => Err((
            ProbeRefusalReason::ReviewStoreAeadCanaryFailed,
            "the sealed canary did not open under the data key".into(),
        )),
        (true, false) => Err((
            ProbeRefusalReason::ReviewStoreAeadCanaryFailed,
            "a blob re-bound to another owner opened".into(),
        )),
    }
}

fn aad_for(owner: &str) -> Vec<u8> {
    format!("{owner}|oauth_sessions.enc_blob|{SCHEMA_VERSION}").into_bytes()
}

fn cross_owner_arm(store: &ReviewStore) -> Arm {
    let counts = store.with_connection(canary_counts_rolled_back);
    match counts {
        Ok((own, other)) => classify_cross_owner(own, other),
        Err(e) => Err((
            ProbeRefusalReason::ReviewStoreCrossOwnerBleed,
            e.to_string(),
        )),
    }
}

/// Write a canary row for one owner, count it as that owner and as another,
/// then roll everything back so the probe leaves no trace.
fn canary_counts_rolled_back(conn: &Connection) -> Result<(i64, i64), StoreError> {
    conn.execute_batch("BEGIN TRANSACTION").map_err(db_error)?;
    let counts = (|| {
        conn.execute(INSERT_CANARY_ACCOUNT, [CANARY_OWNER])
            .map_err(db_error)?;
        let count = |owner: &str| -> Result<i64, StoreError> {
            conn.query_row(COUNT_ACCOUNTS_FOR_OWNER, [owner], |row| row.get(0))
                .map_err(db_error)
        };
        Ok((count(CANARY_OWNER)?, count(OTHER_OWNER)?))
    })();
    conn.execute_batch("ROLLBACK").map_err(db_error)?;
    counts
}

/// Pure: the owner sees exactly its canary; the other owner sees nothing.
pub(crate) fn classify_cross_owner(own_rows: i64, other_rows: i64) -> Arm {
    if own_rows == 1 && other_rows == 0 {
        Ok(())
    } else {
        Err((
            ProbeRefusalReason::ReviewStoreCrossOwnerBleed,
            format!("owner-scoped read saw {own_rows} own and {other_rows} foreign canary rows"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;

    proptest! {
        /// Universe: every (own, other) count pair. Only (1, 0) admits.
        #[test]
        fn cross_owner_arm_admits_only_exact_isolation(own in -2i64..4, other in -2i64..4) {
            prop_assert_eq!(classify_cross_owner(own, other).is_ok(), own == 1 && other == 0);
        }

        /// Universe: every opened plaintext x rebound outcome. Only the exact
        /// canary with a refused re-binding admits.
        #[test]
        fn aead_arm_admits_only_an_exact_round_trip_that_is_owner_bound(
            opened in proptest::option::of(proptest::collection::vec(any::<u8>(), 0..40)),
            exact in any::<bool>(),
            rebound_refused in any::<bool>(),
        ) {
            let opened = if exact { Some(CANARY_PLAINTEXT.to_vec()) } else { opened };
            let admits = opened.as_deref() == Some(CANARY_PLAINTEXT) && rebound_refused;
            prop_assert_eq!(classify_aead(opened.as_deref(), rebound_refused).is_ok(), admits);
        }
    }

    // bypass: the probe over a real store is one wiring example — it must
    // pass and must leave no canary row behind.
    #[test]
    fn a_fresh_store_passes_its_probe_and_keeps_no_canary() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        assert!(matches!(run(&store), ProbeOutcome::Ok));
        let left = store
            .with_connection(|c| {
                c.query_row(COUNT_ACCOUNTS_FOR_OWNER, [CANARY_OWNER], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(db_error)
            })
            .unwrap();
        assert_eq!(left, 0);
    }
}
