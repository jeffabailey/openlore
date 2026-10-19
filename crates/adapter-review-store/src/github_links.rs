//! The kept v1 `github_links` table (one link per DID, ADR-076). Expand-only
//! (ADR-088 §1): after v2 it is read by the v1 → v2 step below, deleted
//! from by purge, and never written; a later migration drops it.

/// ADR-088 §2: every v1 link becomes that DID's one GitHub account.
pub(crate) const COPY_LINKS_TO_ACCOUNTS: &str = "INSERT INTO github_accounts
    (owner_did, github_user_id, github_login_seen, status, verified_at, last_checked_at,
     last_verdict, added_at, eligible_count_seen)
    SELECT owner_did, github_user_id, github_login, status, verified_at, last_checked_at,
        last_verdict, coalesce(verified_at, last_checked_at), NULL
    FROM github_links";

/// ADR-088 §5: every v1 suggestion came from its DID's linked account; a DID
/// without a link keeps a NULL source account (and stays hidden, as in v1).
pub(crate) const BACKFILL_SUGGESTION_SOURCE: &str =
    "UPDATE suggestions SET source_kind = 'account',
    source_account_id = (SELECT l.github_user_id FROM github_links l
        WHERE l.owner_did = suggestions.owner_did)";
