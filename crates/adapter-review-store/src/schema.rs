//! The v1 `review-app.duckdb` schema (ADR-074, data-models §3) and the
//! schema-version gate. Expand-only after v1: a file written by a NEWER app
//! is refused rather than read with a stale understanding.
//!
//! Every owner table is keyed by owner_did. kpi_counters carries no owner.
//! Each DDL statement is its own literal so `xtask check-arch` can classify
//! them one by one.

/// The schema version this build reads and writes.
pub const SCHEMA_VERSION: i64 = 1;

pub(crate) const CREATE_SCHEMA_VERSION: &str =
    "CREATE TABLE IF NOT EXISTS schema_version (version BIGINT NOT NULL)";
pub(crate) const READ_SCHEMA_VERSION: &str = "SELECT max(version) FROM schema_version";
pub(crate) const RECORD_SCHEMA_VERSION: &str = "INSERT INTO schema_version (version) VALUES (?)";

const CREATE_ACCOUNTS: &str = "CREATE TABLE IF NOT EXISTS accounts (
    owner_did VARCHAR PRIMARY KEY,
    handle_last_seen VARCHAR NOT NULL,
    pds_endpoint_last_seen VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL)";

const CREATE_GITHUB_LINKS: &str = "CREATE TABLE IF NOT EXISTS github_links (
    owner_did VARCHAR PRIMARY KEY,
    github_user_id BIGINT NOT NULL,
    github_login VARCHAR NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN ('verified', 'unverified')),
    verified_at TIMESTAMPTZ,
    last_checked_at TIMESTAMPTZ NOT NULL,
    last_verdict VARCHAR NOT NULL)";

const CREATE_SUGGESTIONS: &str = "CREATE TABLE IF NOT EXISTS suggestions (
    owner_did VARCHAR NOT NULL,
    subject VARCHAR NOT NULL,
    predicate VARCHAR NOT NULL,
    object VARCHAR NOT NULL,
    confidence_bp INTEGER NOT NULL,
    evidence VARCHAR NOT NULL,
    signals VARCHAR NOT NULL,
    source_repo VARCHAR NOT NULL,
    state VARCHAR NOT NULL CHECK (state IN ('pending', 'declined', 'published', 'retracted')),
    first_suggested_at TIMESTAMPTZ NOT NULL,
    state_changed_at TIMESTAMPTZ NOT NULL,
    published_uri VARCHAR,
    published_cid VARCHAR,
    published_object VARCHAR,
    published_confidence_bp INTEGER,
    edited BOOLEAN NOT NULL DEFAULT false,
    retraction_cid VARCHAR,
    PRIMARY KEY (owner_did, subject, predicate, object))";

const CREATE_SCAN_RUNS: &str = "CREATE TABLE IF NOT EXISTS scan_runs (
    owner_did VARCHAR NOT NULL,
    run_id VARCHAR NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN
        ('running', 'completed', 'rate_limited', 'interrupted', 'ownership_failed')),
    repos_total INTEGER NOT NULL DEFAULT 0,
    repos_done INTEGER NOT NULL DEFAULT 0,
    new_count INTEGER NOT NULL DEFAULT 0,
    published_count INTEGER NOT NULL DEFAULT 0,
    declined_count INTEGER NOT NULL DEFAULT 0,
    resume_after TIMESTAMPTZ,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    PRIMARY KEY (owner_did, run_id))";

const CREATE_PLANS: &str = "CREATE TABLE IF NOT EXISTS plans (
    owner_did VARCHAR NOT NULL,
    plan_id VARCHAR NOT NULL,
    kind VARCHAR NOT NULL CHECK (kind IN ('publish', 'retract', 'share')),
    suggestion_key VARCHAR,
    plan VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (owner_did, plan_id))";

const CREATE_OAUTH_AUTH_REQUESTS: &str = "CREATE TABLE IF NOT EXISTS oauth_auth_requests (
    state VARCHAR PRIMARY KEY,
    owner_did_expected VARCHAR,
    issuer VARCHAR NOT NULL,
    enc_blob BLOB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL)";

const CREATE_OAUTH_SESSIONS: &str = "CREATE TABLE IF NOT EXISTS oauth_sessions (
    owner_did VARCHAR PRIMARY KEY,
    issuer VARCHAR NOT NULL,
    granted_scopes VARCHAR NOT NULL,
    enc_blob BLOB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL)";

const CREATE_WEB_SESSIONS: &str = "CREATE TABLE IF NOT EXISTS web_sessions (
    session_hash VARCHAR PRIMARY KEY,
    owner_did VARCHAR NOT NULL,
    csrf_token_hash VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL)";

const CREATE_KPI_COUNTERS: &str = "CREATE TABLE IF NOT EXISTS kpi_counters (
    day DATE NOT NULL,
    event VARCHAR NOT NULL,
    count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (day, event))";

/// Every v1 table, in creation order.
pub(crate) const V1_TABLES: [&str; 9] = [
    CREATE_ACCOUNTS,
    CREATE_GITHUB_LINKS,
    CREATE_SUGGESTIONS,
    CREATE_SCAN_RUNS,
    CREATE_PLANS,
    CREATE_OAUTH_AUTH_REQUESTS,
    CREATE_OAUTH_SESSIONS,
    CREATE_WEB_SESSIONS,
    CREATE_KPI_COUNTERS,
];

/// What the recorded schema version means for this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVerdict {
    /// No version recorded: a fresh file, create v1.
    Fresh,
    /// Exactly this build's version.
    Current,
    /// Written by a newer build: refuse, never downgrade-read.
    TooNew { found: i64 },
    /// A version this build never wrote: refuse.
    Unknown { found: i64 },
}

/// Classify the recorded schema version (pure).
pub fn schema_verdict(recorded: Option<i64>) -> SchemaVerdict {
    match recorded {
        None => SchemaVerdict::Fresh,
        Some(v) if v == SCHEMA_VERSION => SchemaVerdict::Current,
        Some(v) if v > SCHEMA_VERSION => SchemaVerdict::TooNew { found: v },
        Some(v) => SchemaVerdict::Unknown { found: v },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Universe: every possible recorded version. Exactly one verdict
        /// admits the file for use, and only for this build's version.
        #[test]
        fn only_the_current_version_or_a_fresh_file_is_admitted(v in any::<Option<i64>>()) {
            let admitted = matches!(
                schema_verdict(v),
                SchemaVerdict::Fresh | SchemaVerdict::Current
            );
            prop_assert_eq!(admitted, v.is_none() || v == Some(SCHEMA_VERSION));
        }
    }
}
