//! `SessionPort`: browser sessions (data-models §3.2). Only hashes of the
//! cookie value and the CSRF token are stored; a session lives 30 days.

use duckdb::{params, OptionalExt};
use ports::{NewWebSession, ReviewStoreError, SessionPort, WebSession};

use crate::secrets::port_error;
use crate::{db_error, expiry, ReviewStore};

const UPSERT_ACCOUNT: &str = "INSERT INTO accounts
    (owner_did, handle_last_seen, pds_endpoint_last_seen, created_at, last_seen_at)
    VALUES (?, ?, ?, now(), now())
    ON CONFLICT (owner_did) DO UPDATE SET handle_last_seen = excluded.handle_last_seen,
        pds_endpoint_last_seen = excluded.pds_endpoint_last_seen,
        last_seen_at = excluded.last_seen_at";

const START_WEB_SESSION: &str = "INSERT INTO web_sessions
    (session_hash, owner_did, csrf_token_hash, created_at, last_used_at, expires_at)
    VALUES (?, ?, ?, now(), now(), CAST(now()::TIMESTAMP + INTERVAL '30 days' AS TIMESTAMPTZ))";

const RESOLVE_WEB_SESSION: &str = "SELECT w.owner_did, a.handle_last_seen, w.csrf_token_hash
    FROM web_sessions w JOIN accounts a ON a.owner_did = w.owner_did
    WHERE w.session_hash = ? AND w.expires_at > now()";

impl SessionPort for ReviewStore {
    fn start_session(&self, session: &NewWebSession) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                UPSERT_ACCOUNT,
                params![session.owner_did, session.handle, session.pds_endpoint],
            )
            .map_err(db_error)?;
            conn.execute(
                START_WEB_SESSION,
                params![session.session_hash, session.owner_did, session.csrf_hash],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn resolve_session(&self, session_hash: &str) -> Result<Option<WebSession>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(RESOLVE_WEB_SESSION, [session_hash], |row| {
                Ok(WebSession {
                    owner_did: row.get(0)?,
                    handle: row.get(1)?,
                    csrf_hash: row.get(2)?,
                })
            })
            .optional()
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn end_session(&self, session_hash: &str, owner_did: &str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| expiry::end_web_session(conn, session_hash, owner_did))
            .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;

    proptest! {
        /// Universe: the web_sessions rows. A started session resolves to its
        /// owner and handle until it is ended; ending it leaves others live.
        #[test]
        fn a_session_resolves_until_it_is_ended(hash in "[0-9a-f]{64}", other in "[0-9a-f]{64}") {
            prop_assume!(hash != other);
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let start = |session_hash: &str, did: &str| NewWebSession {
                session_hash: session_hash.to_string(),
                csrf_hash: "csrf".to_string(),
                owner_did: did.to_string(),
                handle: "priya.example".to_string(),
                pds_endpoint: "https://pds.example".to_string(),
            };
            store.start_session(&start(&hash, "did:plc:a")).unwrap();
            store.start_session(&start(&other, "did:plc:b")).unwrap();
            let live = store.resolve_session(&hash).unwrap().map(|s| (s.owner_did, s.handle));
            prop_assert_eq!(live, Some(("did:plc:a".to_string(), "priya.example".to_string())));
            store.end_session(&hash, "did:plc:a").unwrap();
            prop_assert_eq!(store.resolve_session(&hash).unwrap(), None);
            prop_assert!(store.resolve_session(&other).unwrap().is_some());
        }
    }
}
