//! Row expiry: the only module (with purge) allowed to delete (check-arch
//! `review_store_owner_scoped_sql`). Consumed or declined authorizations,
//! refused OAuth sessions and ended browser sessions are removed outright.

use duckdb::Connection;

use crate::{db_error, StoreError};

const REMOVE_AUTH_REQUEST: &str = "DELETE FROM oauth_auth_requests WHERE state = ?";
const REMOVE_OAUTH_SESSION: &str = "DELETE FROM oauth_sessions WHERE owner_did = ?";
/// A publish plan leaves the store as it is executed (taken exactly once).
const TAKE_PUBLISH_PLAN: &str = "DELETE FROM plans
    WHERE owner_did = ? AND plan_id = ? AND kind = 'publish'
    RETURNING suggestion_key, plan, expires_at";
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

/// A taken plan row: `(suggestion_key JSON, record JSON, expires_at)`.
pub(crate) type TakenPlanRow = (Option<String>, String, Option<i64>);

/// Remove and return the owner's publish plan `plan_id`, if any.
pub(crate) fn take_publish_plan(
    conn: &Connection,
    owner_did: &str,
    plan_id: &str,
) -> Result<Option<TakenPlanRow>, StoreError> {
    let mut statement = conn.prepare(TAKE_PUBLISH_PLAN).map_err(db_error)?;
    let mut rows = statement
        .query_map([owner_did, plan_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
                crate::scan_runs::unix_secs_of(&row.get::<_, duckdb::types::Value>(2)?),
            ))
        })
        .map_err(db_error)?;
    rows.next().transpose().map_err(db_error)
}
