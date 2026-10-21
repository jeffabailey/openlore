//! The one evidence path and the one judge every reader of `github:`
//! authority shares (owner-only-signing, ADR-086 §8, ADR-087 §1-5):
//! `authority verify`, `graph query`, the viewer, `infer people` and
//! `search` all label from the SAME local evidence — each author's active
//! identity links with this reader's latest checks, and the latest
//! observation of each repo — through the SAME pure
//! [`claim_domain::authority::classify`].
//!
//! READ ports only: nothing here asks GitHub, checks or records anything
//! (check-arch `authority_readers_hold_no_write_handle` holds this module).
//! The evidence itself is decoded by adapter-duckdb
//! (`authority_evidence_only_from_adapters`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use claim_domain::authority::{
    classify, requirement, Authority, AuthorityEvidence, ClaimShape, LatestRepoObservation,
    LinkedAccountEvidence, Requirement, ServedRepoObservation,
};
use ports::{
    ClockPort, FeedAuthority, FeedAuthorityReadPort, IdentityLinkCheckReadPort,
    LinkCandidateReadPort, LinkLineageReadPort, RepoAuthorityCheckReadPort, RepoSubjectKey,
    ScoringFeedRow, ScoringFilter, StoredLinkLineage,
};

use crate::render::AuthorityByCid;
use crate::wiring::Wiring;

/// Does a claim of this shape need `github:` authority (a person or a repo
/// subject)? Exempt and malformed claims are labelled by no reader.
pub(crate) fn needs_authority(claim: ClaimShape<'_>) -> bool {
    matches!(
        requirement(claim),
        Requirement::Person(_) | Requirement::Repo { .. }
    )
}

// -----------------------------------------------------------------------------
// The evidence path
// -----------------------------------------------------------------------------

/// The READ ports this reader's authority evidence comes from.
#[derive(Clone, Copy)]
pub struct AuthorityReads<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub candidates: &'a dyn LinkCandidateReadPort,
    pub lineage: &'a dyn LinkLineageReadPort,
    pub repo_checks: &'a dyn RepoAuthorityCheckReadPort,
}

impl AuthorityReads<'_> {
    /// Each of `authors`' active identity links, each with this reader's
    /// latest check not dated after `now`.
    pub fn linked_accounts_by_author<'s>(
        &self,
        authors: impl IntoIterator<Item = &'s str>,
        now: DateTime<Utc>,
    ) -> Result<BTreeMap<String, Vec<LinkedAccountEvidence>>> {
        let active_links = StoredLinkLineage::read(self.candidates, self.lineage)
            .context("reading the identity links")?
            .active_links();
        authors
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|author| {
                adapter_duckdb::linked_account_evidence(self.checks, author, &active_links, now)
                    .context("reading the identity link checks")
                    .map(|evidence| (author.to_string(), evidence))
            })
            .collect()
    }

    /// This reader's latest observation (not dated after `now`) of each repo
    /// subject; a repo never observed here has no entry.
    pub fn latest_repo_observations(
        &self,
        subjects: impl IntoIterator<Item = RepoSubjectKey>,
        now: DateTime<Utc>,
    ) -> Result<BTreeMap<RepoSubjectKey, LatestRepoObservation>> {
        let subjects: Vec<RepoSubjectKey> = subjects
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        adapter_duckdb::latest_repo_observations(self.repo_checks, &subjects, now)
            .context("reading the repos' latest authority observations")
    }

    /// This reader's latest observation of the repo `subject` names, with the
    /// name GitHub served it under (`None`: never observed here).
    pub fn served_repo_observation(
        &self,
        subject: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<ServedRepoObservation>> {
        adapter_duckdb::served_repo_observation(self.repo_checks, RepoSubjectKey::of(subject), now)
            .context("reading the repo's latest authority observation")
    }
}

// -----------------------------------------------------------------------------
// The judge
// -----------------------------------------------------------------------------

/// Everything this reader holds locally about some claims' authors and the
/// repos they are about.
#[derive(Debug, Default)]
pub struct LocalEvidence {
    pub links_by_author: BTreeMap<String, Vec<LinkedAccountEvidence>>,
    pub repos: BTreeMap<RepoSubjectKey, LatestRepoObservation>,
}

impl LocalEvidence {
    /// Who `author` is to `claim`'s subject, judged from this evidence as of
    /// `now`. `None`: the claim needs no authority, or names nothing GitHub
    /// could attest.
    pub fn judge(
        &self,
        claim: ClaimShape<'_>,
        author: &str,
        now: DateTime<Utc>,
    ) -> Option<Authority> {
        let evidence = AuthorityEvidence {
            author,
            author_links: self.links_by_author.get(author).map_or(&[], Vec::as_slice),
            repo: self.repos.get(&RepoSubjectKey::of(claim.subject)),
        };
        classify(claim, &evidence, now)
    }
}

// -----------------------------------------------------------------------------
// A scoring feed's authority
// -----------------------------------------------------------------------------

/// What this reader holds locally to judge a scoring feed's `github:`
/// assertions, as of the clock's `now`. The one judge `graph query`, the
/// viewer, `infer people` and `search` share, so they weigh and label alike.
pub struct LocalFeedAuthority<'a> {
    pub reads: AuthorityReads<'a>,
    pub clock: &'a dyn ClockPort,
}

impl FeedAuthorityReadPort for LocalFeedAuthority<'_> {
    fn feed_authority(
        &self,
        feed: &[ScoringFeedRow],
    ) -> std::result::Result<FeedAuthority, String> {
        self.judge(feed).map_err(|err| format!("{err:#}"))
    }
}

impl LocalFeedAuthority<'_> {
    /// [`FeedAuthorityReadPort::feed_authority`] for a CLI reader: a failed
    /// read names this reader's authority checks.
    pub fn judged(&self, feed: &[ScoringFeedRow]) -> Result<FeedAuthority> {
        self.feed_authority(feed)
            .map_err(|message| anyhow!("reading this reader's authority checks: {message}"))
    }

    fn judge(&self, feed: &[ScoringFeedRow]) -> Result<FeedAuthority> {
        let now = self.clock.now_utc();
        let needing: Vec<&ScoringFeedRow> = feed
            .iter()
            .filter(|row| needs_authority(row.shape()))
            .collect();
        let links_by_author = self.reads.linked_accounts_by_author(
            needing.iter().map(|row| row.claim.author_did.0.as_str()),
            now,
        )?;
        let repos = self.reads.latest_repo_observations(
            needing
                .iter()
                .filter(|row| matches!(requirement(row.shape()), Requirement::Repo { .. }))
                .map(|row| RepoSubjectKey::of(&row.claim.subject)),
            now,
        )?;
        let evidence = LocalEvidence {
            links_by_author,
            repos,
        };
        Ok(FeedAuthority::new(needing.into_iter().filter_map(|row| {
            evidence
                .judge(row.shape(), &row.claim.author_did.0, now)
                .map(|authority| (row.claim.cid.clone(), authority))
        })))
    }
}

/// [`LocalFeedAuthority`] over shared handles, for the viewer process.
pub struct SharedFeedAuthority {
    pub checks: Arc<dyn IdentityLinkCheckReadPort + Send + Sync>,
    pub candidates: Arc<dyn LinkCandidateReadPort + Send + Sync>,
    pub lineage: Arc<dyn LinkLineageReadPort + Send + Sync>,
    pub repo_checks: Arc<dyn RepoAuthorityCheckReadPort + Send + Sync>,
    pub clock: Arc<dyn ClockPort + Send + Sync>,
}

impl FeedAuthorityReadPort for SharedFeedAuthority {
    fn feed_authority(
        &self,
        feed: &[ScoringFeedRow],
    ) -> std::result::Result<FeedAuthority, String> {
        LocalFeedAuthority {
            reads: AuthorityReads {
                checks: self.checks.as_ref(),
                candidates: self.candidates.as_ref(),
                lineage: self.lineage.as_ref(),
                repo_checks: self.repo_checks.as_ref(),
            },
            clock: self.clock.as_ref(),
        }
        .feed_authority(feed)
    }
}

/// Who each `github:` assertion in `filter`'s feed is the word of, to this
/// reader, by cid: the same judge the weighted view uses, over the latest
/// local checks. Claims that need no authority (disputes, withdrawals,
/// identity links, non-GitHub subjects) carry no entry.
pub(crate) fn labels_here(wiring: &Wiring, filter: &ScoringFilter) -> Result<AuthorityByCid> {
    let feed = wiring
        .storage
        .query_attributed_for_scoring(filter)
        .context("reading the claims to label")?;
    Ok(wiring.feed_authority_reads().judged(&feed)?.into_by_cid())
}
