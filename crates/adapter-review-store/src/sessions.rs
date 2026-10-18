//! `SessionPort`: browser sessions (data-models §3.2). Only hashes of the
//! cookie value and the CSRF token are stored; a session lives 30 days.

use duckdb::{params, OptionalExt};
use ports::{KpiTally, NewWebSession, ReviewStoreError, SessionPort, WebSession};

use crate::secrets::port_error;
use crate::{db_error, expiry, ReviewStore};

const UPSERT_ACCOUNT: &str = "INSERT INTO accounts
    (owner_did, handle_last_seen, pds_endpoint_last_seen, created_at, last_seen_at)
    VALUES (?, ?, ?, now(), now())
    ON CONFLICT (owner_did) DO UPDATE SET handle_last_seen = excluded.handle_last_seen,
        pds_endpoint_last_seen = excluded.pds_endpoint_last_seen,
        last_seen_at = excluded.last_seen_at";

/// A session lives 30 days from its start (ADR-073), by the caller's clock.
const SESSION_LIFETIME_SECS: i64 = 30 * 24 * 60 * 60;

const START_WEB_SESSION: &str = "INSERT INTO web_sessions
    (session_hash, owner_did, csrf_token_hash, created_at, last_used_at, expires_at)
    VALUES (?, ?, ?, to_timestamp(?), to_timestamp(?), to_timestamp(?))";

const RESOLVE_WEB_SESSION: &str = "SELECT w.owner_did, a.handle_last_seen, w.csrf_token_hash
    FROM web_sessions w JOIN accounts a ON a.owner_did = w.owner_did
    WHERE w.session_hash = ? AND w.expires_at > to_timestamp(?)";

impl SessionPort for ReviewStore {
    fn start_session(
        &self,
        session: &NewWebSession,
        now: i64,
        tally: KpiTally<'_>,
    ) -> Result<(), ReviewStoreError> {
        self.with_tally(
            tally,
            |_| true,
            |conn| {
                conn.execute(
                    UPSERT_ACCOUNT,
                    params![session.owner_did, session.handle, session.pds_endpoint],
                )
                .map_err(db_error)?;
                conn.execute(
                    START_WEB_SESSION,
                    params![
                        session.session_hash,
                        session.owner_did,
                        session.csrf_hash,
                        now,
                        now,
                        now.saturating_add(SESSION_LIFETIME_SECS)
                    ],
                )
                .map(drop)
                .map_err(db_error)
            },
        )
        .map_err(port_error)
    }

    fn resolve_session(
        &self,
        session_hash: &str,
        now: i64,
    ) -> Result<Option<WebSession>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(RESOLVE_WEB_SESSION, params![session_hash, now], |row| {
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

    /// A start instant far from the wall clock (2001-09-09), so only the
    /// passed-in time can decide a session's life.
    const STARTED: i64 = 1_000_000_000;

    /// The policy, spelled out: 30 days of 86 400 seconds (ADR-073), never
    /// read from the code under test.
    const THIRTY_DAYS_SECS: i64 = 2_592_000;

    /// 29 days: a session this old is still live.
    const TWENTY_NINE_DAYS_SECS: i64 = 2_505_600;

    fn new_session(session_hash: &str, did: &str) -> NewWebSession {
        NewWebSession {
            session_hash: session_hash.to_string(),
            csrf_hash: "csrf".to_string(),
            owner_did: did.to_string(),
            handle: "priya.example".to_string(),
            pds_endpoint: "https://pds.example".to_string(),
        }
    }

    proptest! {
        /// Universe: the web_sessions rows. A started session resolves to its
        /// owner and handle until it is ended; ending it leaves others live.
        #[test]
        fn a_session_resolves_until_it_is_ended(hash in "[0-9a-f]{64}", other in "[0-9a-f]{64}") {
            prop_assume!(hash != other);
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            store.start_session(&new_session(&hash, "did:plc:a"), STARTED, KpiTally::NONE).unwrap();
            store.start_session(&new_session(&other, "did:plc:b"), STARTED, KpiTally::NONE).unwrap();
            let live = store.resolve_session(&hash, STARTED).unwrap().map(|s| (s.owner_did, s.handle));
            prop_assert_eq!(live, Some(("did:plc:a".to_string(), "priya.example".to_string())));
            store.end_session(&hash, "did:plc:a").unwrap();
            prop_assert_eq!(store.resolve_session(&hash, STARTED).unwrap(), None);
            prop_assert!(store.resolve_session(&other, STARTED).unwrap().is_some());
        }

        /// A session is live exactly while the caller's `now` is before its
        /// start plus 30 days — judged by the passed-in time, never the wall
        /// clock (which is decades past `STARTED`).
        #[test]
        fn a_session_lives_thirty_days_by_the_callers_clock(elapsed in -1_000i64..=2 * THIRTY_DAYS_SECS) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            store.start_session(&new_session("hash", "did:plc:a"), STARTED, KpiTally::NONE).unwrap();
            let live = store.resolve_session("hash", STARTED + elapsed).unwrap().is_some();
            prop_assert_eq!(live, elapsed < THIRTY_DAYS_SECS, "elapsed {}", elapsed);
        }
    }

    /// The boundary: live at 29 days and one second before 30 days, ended
    /// at 30 days (2 592 000 seconds after it started).
    #[test]
    fn a_session_ends_at_exactly_thirty_days() {
        assert_eq!(SESSION_LIFETIME_SECS, THIRTY_DAYS_SECS);
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        store
            .start_session(&new_session("hash", "did:plc:a"), STARTED, KpiTally::NONE)
            .unwrap();
        assert!(store
            .resolve_session("hash", STARTED + TWENTY_NINE_DAYS_SECS)
            .unwrap()
            .is_some());
        let expires = STARTED + 2_592_000;
        assert!(store
            .resolve_session("hash", expires - 1)
            .unwrap()
            .is_some());
        assert_eq!(store.resolve_session("hash", expires).unwrap(), None);
    }
}
