//! `SecretStorePort`: AEAD-sealed OAuth state (ADR-074, data-models §3.2).
//! A blob's associated data names its table and key, so a blob moved to
//! another row does not open.

use duckdb::{params, OptionalExt};
use ports::{ReviewStoreError, SecretStorePort};

use crate::{expiry, ReviewStore, StoreError};

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
