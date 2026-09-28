//! `contribution_link` — the append-only local link table port
//! (contributor-philosophy-inference DDD-4 / DDD-5 / ADR-063 §3).
//!
//! A contribution link records that a GitHub person was observed among a
//! scraped repo's top human contributors. Links are UNSIGNED local
//! observation data (ADR-064) — never claims, never federated.
//!
//! ## Append-only by type (Earned Trust layer 1)
//!
//! [`ContributionLinkPort`] exposes NO delete and NO caller-driven update
//! method. The only write is [`ContributionLinkPort::record_snapshot`], an
//! upsert of one repo's complete ranked snapshot in ONE transaction that
//! refreshes user id / rank / contributions / `last_observed_at` and NEVER
//! touches `first_observed_at`. People who drop out of a later snapshot stay
//! recorded; "stale" is derived from `last_observed_at`, never stored.

use chrono::{DateTime, Utc};

use crate::probe::ProbeOutcome;

/// One human contributor as the pure selection ranked it
/// (`scraper_domain::select_contributors`): 1-based `rank` among HUMANS only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedContributor {
    /// GitHub login, display form (the person subject is `github:<login>`).
    pub login: String,
    /// Stable numeric GitHub user id (rename detection).
    pub github_user_id: u64,
    /// 1-based rank among the repo's humans (contributions desc, login asc).
    pub rank: u32,
    /// Commit contributions GitHub attributes to the person.
    pub contributions: u64,
}

impl RankedContributor {
    /// The person subject the link records: `github:<login>` (ADR-064).
    pub fn person_subject(&self) -> String {
        format!("github:{}", self.login)
    }
}

/// One recorded link row, as DDD-5 pins the `contribution_links` columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionLink {
    /// Display-form repo subject, e.g. `github:BurntSushi/ripgrep`.
    pub repo_subject: String,
    /// Display-form person subject, e.g. `github:BurntSushi`.
    pub person_subject: String,
    pub github_user_id: u64,
    pub rank: u32,
    pub contributions: u64,
    /// When the person was FIRST seen linked to the repo — immutable.
    pub first_observed_at: DateTime<Utc>,
    /// When the person was last seen in a snapshot of the repo.
    pub last_observed_at: DateTime<Utc>,
}

/// Which links [`ContributionLinkPort::list_links`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkFilter {
    /// Every recorded link.
    All,
    /// Links of one person subject (`github:<login>`, matched case-insensitively).
    Person(String),
}

/// What one [`ContributionLinkPort::record_snapshot`] call wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordSnapshotOutcome {
    /// How many people the snapshot recorded (inserted or refreshed).
    pub recorded: usize,
}

/// Failure surface of [`ContributionLinkPort`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContributionLinkError {
    /// The link store failed (the whole snapshot transaction rolled back).
    #[error("contribution link store failure: {0}")]
    Store(String),
}

/// Append-only local link storage (sync, local-DB only). NO delete, NO
/// caller-driven update — by construction a link can only be added or
/// refreshed by a newer snapshot.
pub trait ContributionLinkPort {
    /// Earned-Trust probe: an upsert-twice sentinel inside a rolled-back
    /// transaction must refresh rank / `last_observed_at` and leave
    /// `first_observed_at` untouched (DDD-15).
    fn probe(&self) -> ProbeOutcome;

    /// Record one repo's complete ranked snapshot in ONE transaction.
    fn record_snapshot(
        &self,
        repo_subject: &str,
        observed_at: DateTime<Utc>,
        people: &[RankedContributor],
    ) -> Result<RecordSnapshotOutcome, ContributionLinkError>;

    /// Read recorded links, ordered by (repo, rank, person).
    fn list_links(
        &self,
        filter: &LinkFilter,
    ) -> Result<Vec<ContributionLink>, ContributionLinkError>;
}
