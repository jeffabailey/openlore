//! `SecretStorePort`: AEAD-sealed OAuth state (ADR-074, data-models §3.2).
//! A blob's associated data names its table and key, so a blob moved to
//! another row does not open.

use duckdb::{params, OptionalExt};
use ports::{ReviewStoreError, SecretStorePort};

use crate::{
    db_error, expiry, seal_with, split_kid, unseal_with, DataKey, ReviewStore, StoreError,
};

const PUT_AUTH_REQUEST: &str = "INSERT INTO oauth_auth_requests
    (state, owner_did_expected, issuer, enc_blob, created_at, expires_at)
    VALUES (?, ?, ?, ?, now(), CAST(now()::TIMESTAMP + INTERVAL '10 minutes' AS TIMESTAMPTZ))";

const READ_AUTH_REQUEST: &str =
    "SELECT enc_blob FROM oauth_auth_requests WHERE state = ? AND expires_at > now()";

const PUT_OAUTH_SESSION: &str = "INSERT INTO oauth_sessions
    (owner_did, issuer, granted_scopes, enc_blob, updated_at)
    VALUES (?, ?, ?, ?, now())
    ON CONFLICT (owner_did) DO UPDATE SET issuer = excluded.issuer,
        granted_scopes = excluded.granted_scopes, enc_blob = excluded.enc_blob,
        updated_at = excluded.updated_at";

const READ_OAUTH_SESSION: &str = "SELECT enc_blob FROM oauth_sessions WHERE owner_did = ?";

const AUTH_REQUEST_BLOBS: &str = "SELECT state, enc_blob FROM oauth_auth_requests";
const RESEAL_AUTH_REQUEST: &str = "UPDATE oauth_auth_requests SET enc_blob = ? WHERE state = ?";
const OAUTH_SESSION_BLOBS: &str = "SELECT owner_did, enc_blob FROM oauth_sessions";
const RESEAL_OAUTH_SESSION: &str = "UPDATE oauth_sessions SET enc_blob = ? WHERE owner_did = ?";

/// One table of sealed blobs: how to list them, re-store one, and the
/// associated data of a row.
struct SealedTable {
    list: &'static str,
    reseal: &'static str,
    aad: fn(&str) -> Vec<u8>,
}

const SEALED_TABLES: [SealedTable; 2] = [
    SealedTable {
        list: AUTH_REQUEST_BLOBS,
        reseal: RESEAL_AUTH_REQUEST,
        aad: auth_request_aad,
    },
    SealedTable {
        list: OAUTH_SESSION_BLOBS,
        reseal: RESEAL_OAUTH_SESSION,
        aad: oauth_session_aad,
    },
];

fn sealed_rows(conn: &duckdb::Connection, sql: &str) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
    let mut statement = conn.prepare(sql).map_err(db_error)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(db_error)?;
    rows.collect::<Result<_, _>>().map_err(db_error)
}

fn sealed_under(blob: &[u8], kid: &str) -> bool {
    split_kid(blob).is_some_and(|(tag, _)| tag == kid.as_bytes())
}

fn reseal_table(
    conn: &duckdb::Connection,
    table: &SealedTable,
    previous: &DataKey,
    active: &DataKey,
) -> Result<usize, StoreError> {
    let stale: Vec<(String, Vec<u8>)> = sealed_rows(conn, table.list)?
        .into_iter()
        .filter(|(_, blob)| sealed_under(blob, previous.kid()))
        .collect();
    for (key, blob) in &stale {
        let aad = (table.aad)(key);
        let resealed = seal_with(active, &aad, &unseal_with(previous, &aad, blob)?)?;
        conn.execute(table.reseal, params![resealed, key])
            .map_err(db_error)?;
    }
    Ok(stale.len())
}

impl ReviewStore {
    /// Data-key rotation (infrastructure-integration §7.4): re-encrypt every
    /// blob still sealed under `previous` with the active key, in one
    /// transaction. Returns how many rows moved (`secrets.rekeyed{rows}`).
    pub fn rekey(&self, previous: &DataKey) -> Result<usize, StoreError> {
        let active = self.active_key();
        if previous.kid() == active.kid() {
            return Ok(0);
        }
        self.with_connection(|conn| {
            conn.execute_batch("BEGIN TRANSACTION").map_err(db_error)?;
            let moved = SEALED_TABLES.iter().try_fold(0, |n, table| {
                Ok::<_, StoreError>(n + reseal_table(conn, table, previous, active)?)
            });
            match moved {
                Ok(rows) => conn
                    .execute_batch("COMMIT")
                    .map_err(db_error)
                    .map(|()| rows),
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(e)
                }
            }
        })
    }
}

fn auth_request_aad(state: &str) -> Vec<u8> {
    format!("oauth_auth_requests:{state}").into_bytes()
}

fn oauth_session_aad(owner_did: &str) -> Vec<u8> {
    format!("oauth_sessions:{owner_did}").into_bytes()
}

pub(crate) fn port_error(e: StoreError) -> ReviewStoreError {
    ReviewStoreError(e.to_string())
}

impl ReviewStore {
    fn read_blob(&self, sql: &str, key: &str) -> Result<Option<Vec<u8>>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(sql, [key], |row| row.get::<_, Vec<u8>>(0))
                .optional()
                .map_err(crate::db_error)
        })
        .map_err(port_error)
    }

    fn open_blob(
        &self,
        aad: &[u8],
        sealed: Option<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, ReviewStoreError> {
        sealed
            .map(|blob| self.unseal(aad, &blob))
            .transpose()
            .map_err(port_error)
    }
}

impl SecretStorePort for ReviewStore {
    fn put_auth_request(
        &self,
        state: &str,
        issuer: &str,
        expected_did: Option<&str>,
        blob: &[u8],
    ) -> Result<(), ReviewStoreError> {
        let sealed = self
            .seal(&auth_request_aad(state), blob)
            .map_err(port_error)?;
        self.with_connection(|conn| {
            conn.execute(
                PUT_AUTH_REQUEST,
                params![state, expected_did, issuer, sealed],
            )
            .map(drop)
            .map_err(crate::db_error)
        })
        .map_err(port_error)
    }

    fn auth_request(&self, state: &str) -> Result<Option<Vec<u8>>, ReviewStoreError> {
        let sealed = self.read_blob(READ_AUTH_REQUEST, state)?;
        self.open_blob(&auth_request_aad(state), sealed)
    }

    fn remove_auth_request(&self, state: &str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| expiry::remove_auth_request(conn, state))
            .map_err(port_error)
    }

    fn put_oauth_session(
        &self,
        owner_did: &str,
        issuer: &str,
        granted_scopes: &str,
        blob: &[u8],
    ) -> Result<(), ReviewStoreError> {
        let sealed = self
            .seal(&oauth_session_aad(owner_did), blob)
            .map_err(port_error)?;
        self.with_connection(|conn| {
            conn.execute(
                PUT_OAUTH_SESSION,
                params![owner_did, issuer, granted_scopes, sealed],
            )
            .map(drop)
            .map_err(crate::db_error)
        })
        .map_err(port_error)
    }

    fn oauth_session(&self, owner_did: &str) -> Result<Option<Vec<u8>>, ReviewStoreError> {
        let sealed = self.read_blob(READ_OAUTH_SESSION, owner_did)?;
        self.open_blob(&oauth_session_aad(owner_did), sealed)
    }

    fn remove_oauth_session(&self, owner_did: &str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| expiry::remove_oauth_session(conn, owner_did))
            .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;

    fn store() -> ReviewStore {
        ReviewStore::open_in_memory(DataKey::generate()).expect("in-memory store")
    }

    proptest! {
        /// Universe: every sealed blob of a store file. After a rotation, each
        /// blob sealed under the previous key opens under the new one with
        /// its plaintext unchanged, the count is exactly those rows, and a
        /// second rekey moves nothing.
        #[test]
        fn a_rotation_re_encrypts_every_blob_on_the_previous_key(
            sessions in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..16), 0..4),
            requests in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..16), 0..4),
        ) {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("review-app.duckdb");
            let old = || DataKey::new("d1", [7; 32]).unwrap();
            {
                let store = ReviewStore::open(&path, old()).unwrap();
                for (i, blob) in sessions.iter().enumerate() {
                    store.put_oauth_session(&format!("did:plc:{i}"), "iss", "atproto", blob).unwrap();
                }
                for (i, blob) in requests.iter().enumerate() {
                    store.put_auth_request(&format!("state-{i}"), "iss", None, blob).unwrap();
                }
            }
            let store = ReviewStore::open(&path, DataKey::new("d2", [9; 32]).unwrap()).unwrap();
            prop_assert_eq!(store.rekey(&old()).unwrap(), sessions.len() + requests.len());
            for (i, blob) in sessions.iter().enumerate() {
                prop_assert_eq!(store.oauth_session(&format!("did:plc:{i}")).unwrap(), Some(blob.clone()));
            }
            for (i, blob) in requests.iter().enumerate() {
                prop_assert_eq!(store.auth_request(&format!("state-{i}")).unwrap(), Some(blob.clone()));
            }
            prop_assert_eq!(store.rekey(&old()).unwrap(), 0);
        }

        /// Universe: the oauth_auth_requests rows of a fresh store. A pending
        /// authorization opens only under its own state, and once removed it
        /// is gone (single use); other states are untouched.
        #[test]
        fn a_pending_authorization_opens_only_under_its_state_and_only_until_used(
            state in "[a-zA-Z0-9_-]{8,24}", other in "[a-zA-Z0-9_-]{8,24}", blob in prop::collection::vec(any::<u8>(), 0..64)
        ) {
            prop_assume!(state != other);
            let store = store();
            store.put_auth_request(&state, "https://pds.example", Some("did:plc:x"), &blob).unwrap();
            store.put_auth_request(&other, "https://pds.example", None, b"other").unwrap();
            prop_assert_eq!(store.auth_request(&state).unwrap(), Some(blob));
            store.remove_auth_request(&state).unwrap();
            prop_assert_eq!(store.auth_request(&state).unwrap(), None);
            prop_assert_eq!(store.auth_request(&other).unwrap(), Some(b"other".to_vec()));
        }

        /// Universe: the oauth_sessions rows. A session is replaced in place
        /// per owner and forgetting one owner leaves the other's intact.
        #[test]
        fn oauth_sessions_are_per_owner(first in prop::collection::vec(any::<u8>(), 1..32), second in prop::collection::vec(any::<u8>(), 1..32)) {
            let store = store();
            store.put_oauth_session("did:plc:a", "iss", "atproto", &first).unwrap();
            store.put_oauth_session("did:plc:a", "iss", "atproto", &second).unwrap();
            store.put_oauth_session("did:plc:b", "iss", "atproto", &first).unwrap();
            prop_assert_eq!(store.oauth_session("did:plc:a").unwrap(), Some(second));
            store.remove_oauth_session("did:plc:a").unwrap();
            prop_assert_eq!(store.oauth_session("did:plc:a").unwrap(), None);
            prop_assert_eq!(store.oauth_session("did:plc:b").unwrap(), Some(first));
        }
    }
}
