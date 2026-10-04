//! Row expiry: the only module (with purge) allowed to delete (check-arch
//! `review_store_owner_scoped_sql`). Consumed or declined authorizations,
//! refused OAuth sessions and ended browser sessions are removed outright.

use duckdb::Connection;

use crate::{db_error, StoreError};

const REMOVE_AUTH_REQUEST: &str = "DELETE FROM oauth_auth_requests WHERE state = ?";
const REMOVE_OAUTH_SESSION: &str = "DELETE FROM oauth_sessions WHERE owner_did = ?";
const END_WEB_SESSION: &str = "DELETE FROM web_sessions WHERE session_hash = ? AND owner_did = ?";

pub(crate) fn remove_auth_request(conn: &Connection, state: &str) -> Result<(), StoreError> {
    conn.execute(REMOVE_AUTH_REQUEST, [state])
        .map(drop)
        .map_err(db_error)
}

pub(crate) fn remove_oauth_session(conn: &Connection, owner_did: &str) -> Result<(), StoreError> {
    conn.execute(REMOVE_OAUTH_SESSION, [owner_did])
        .map(drop)
        .map_err(db_error)
}

pub(crate) fn end_web_session(
    conn: &Connection,
    session_hash: &str,
    owner_did: &str,
) -> Result<(), StoreError> {
    conn.execute(END_WEB_SESSION, [session_hash, owner_did])
        .map(drop)
        .map_err(db_error)
}
