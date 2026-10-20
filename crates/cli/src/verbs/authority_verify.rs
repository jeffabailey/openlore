//! `authority verify` — re-check the authority of the local `github:` claims
//! (own and followed peers'), recording one repo observation per distinct
//! repo subject and the identity-link checks the labels need, then print
//! each claim's label (owner-only-signing, ADR-086 §8, ADR-087 §4-6).
//!
//! The verb never signs and never publishes (C-OOS-4): it holds the check
//! read/write handles, read-only claim reads and GitHub — the capability row
//! `authority verify` of the DESIGN table. Observations are insert-only and
//! never federated; a repo GitHub cannot answer for records nothing.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use claim_domain::authority::{
    classify, requirement, AuthorityEvidence, AuthorityFileSeen, ClaimShape, LatestRepoObservation,
    LinkedAccountEvidence, RepoFinding, Requirement,
};
use claim_domain::identity_link::{IdentityLink, RecheckScope};
use claim_domain::ReferenceType;
use ports::{
    AuthorityFile, PageRequest, RepoAuthorityCheck, RepoAuthorityCheckReadPort,
    RepoAuthorityCheckWritePort, RepoObservation, RepoSubjectKey, StoreReadPort, StoredLinkLineage,
};

use crate::render::{
    render_authority_labels, render_github_could_not_be_asked, render_nothing_to_recheck,
    LabelledClaim,
};
use crate::verbs::bare_did;
use crate::verbs::identity_reads::{links_in_scope, LinkToRecheck};
use crate::verbs::identity_verify::{recheck_and_record, IdentityVerifyHandles};

/// The narrow handles `authority verify` holds.
pub struct AuthorityVerifyHandles<'a> {
    /// The identity-link re-check path `identity verify` uses (link check
    /// read and write, the link claims and their lineage, GitHub).
    pub links: IdentityVerifyHandles<'a>,
    pub repo_checks: &'a dyn RepoAuthorityCheckReadPort,
    pub repo_check_writes: &'a dyn RepoAuthorityCheckWritePort,
    /// Read-only: the own and pulled claims whose authority is re-checked.
    pub claims: &'a dyn StoreReadPort,
}

/// Which claims one run re-checks.
#[derive(Debug, Clone, Default)]
pub struct AuthorityVerifyArgs {
    /// Only claims about this subject (letter case aside).
    pub subject: Option<String>,
    /// Only claims by this DID (`--mine`: the reader's own DID).
    pub author: Option<String>,
}

/// What one run reports: the labels (stdout) and the repos GitHub could not
/// be asked about (stderr; exit 1).
#[derive(Debug, Default)]
pub struct AuthorityVerifyReport {
    pub stdout: String,
    pub unreadable: Vec<String>,
}

/// A stored claim about a `github:` subject that needs authority.
#[derive(Debug, Clone)]
struct GithubClaim {
    author: String,
    subject: String,
    predicate: String,
    reference_types: Vec<ReferenceType>,
    required: Requirement,
}

impl GithubClaim {
    fn shape(&self) -> ClaimShape<'_> {
        ClaimShape {
            subject: &self.subject,
            predicate: &self.predicate,
            reference_types: &self.reference_types,
        }
    }

    fn repo_key(&self) -> Option<RepoSubjectKey> {
        matches!(self.required, Requirement::Repo { .. }).then(|| RepoSubjectKey::of(&self.subject))
    }
}

/// Run `authority verify` as of `now` over the claims `args` selects.
pub fn run(
    handles: &AuthorityVerifyHandles<'_>,
    args: &AuthorityVerifyArgs,
    now: DateTime<Utc>,
) -> Result<AuthorityVerifyReport> {
    let claims = github_claims_in_scope(handles, args)?;
    if claims.is_empty() {
        return Ok(AuthorityVerifyReport {
            stdout: render_nothing_to_recheck(),
            unreadable: Vec::new(),
        });
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    let unreadable = observe_repos(handles, &runtime, &claims, now)?;
    let repos = latest_repo_observations(handles.repo_checks, &claims, now)?;
    recheck_needed_links(handles, &runtime, &claims, &repos, now)?;
    let labelled = label_claims(handles, &claims, &repos, now)?;
    Ok(AuthorityVerifyReport {
        stdout: render_authority_labels(&labelled),
        unreadable,
    })
}

// -----------------------------------------------------------------------------
// Which claims
// -----------------------------------------------------------------------------

/// Every stored claim (own and pulled) that needs `github:` authority and
/// is within `args`, ordered by subject then author.
fn github_claims_in_scope(
    handles: &AuthorityVerifyHandles<'_>,
    args: &AuthorityVerifyArgs,
) -> Result<Vec<GithubClaim>> {
    let mut claims: Vec<GithubClaim> = github_predicates(handles.claims)?
        .iter()
        .map(|predicate| handles.links.claims.link_candidates(predicate))
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("reading the stored GitHub claims")?
        .into_iter()
        .flatten()
        .map(|candidate| {
            let reference_types: Vec<ReferenceType> = candidate
                .references
                .iter()
                .map(|reference| reference.ref_type)
                .collect();
            let required = requirement(ClaimShape {
                subject: &candidate.subject,
                predicate: &candidate.predicate,
                reference_types: &reference_types,
            });
            GithubClaim {
                author: bare_did(&candidate.author_did),
                subject: candidate.subject,
                predicate: candidate.predicate,
                reference_types,
                required,
            }
        })
        .filter(|claim| {
            matches!(
                claim.required,
                Requirement::Person(_) | Requirement::Repo { .. }
            )
        })
        .filter(|claim| in_scope(claim, args))
        .collect();
    claims.sort_by(|a, b| {
        (a.subject.to_lowercase(), &a.author).cmp(&(b.subject.to_lowercase(), &b.author))
    });
    Ok(claims)
}

fn in_scope(claim: &GithubClaim, args: &AuthorityVerifyArgs) -> bool {
    let subject_matches = args
        .subject
        .as_ref()
        .is_none_or(|subject| subject.eq_ignore_ascii_case(&claim.subject));
    let author_matches = args
        .author
        .as_ref()
        .is_none_or(|author| bare_did(author) == claim.author);
    subject_matches && author_matches
}

/// The predicates the stored claims about `github:` subjects carry, own and
/// pulled, read through the read-only store surface.
fn github_predicates(store: &dyn StoreReadPort) -> Result<BTreeSet<String>> {
    let is_github = |subject: &str| {
        subject
            .split_once(':')
            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("github"))
    };
    let own = store
        .list_claims(whole_table(store.count_claims()?))?
        .rows
        .into_iter()
        .map(|row| (row.subject, row.predicate));
    let pulled = store
        .list_peer_claims(whole_table(store.count_peer_claims()?))?
        .rows
        .into_iter()
        .map(|row| (row.subject, row.predicate));
    Ok(own
        .chain(pulled)
        .filter(|(subject, _)| is_github(subject))
        .map(|(_, predicate)| predicate)
        .collect())
}

fn whole_table(rows: usize) -> PageRequest {
    PageRequest {
        offset: 0,
        limit: rows.max(1) as u64,
    }
}

// -----------------------------------------------------------------------------
// Repo observations: one look at each repo and its authority file
// -----------------------------------------------------------------------------

/// Observe each distinct repo subject once (case-folded) and record what
/// GitHub answered; a repo GitHub cannot answer for records nothing and is
/// reported instead.
fn observe_repos(
    handles: &AuthorityVerifyHandles<'_>,
    runtime: &tokio::runtime::Runtime,
    claims: &[GithubClaim],
    now: DateTime<Utc>,
) -> Result<Vec<String>> {
    let repos: BTreeMap<RepoSubjectKey, (&str, &str)> = claims
        .iter()
        .filter_map(|claim| match &claim.required {
            Requirement::Repo { owner, name } => Some((
                RepoSubjectKey::of(&claim.subject),
                (owner.as_str(), name.as_str()),
            )),
            _ => None,
        })
        .collect();
    let mut unreadable = Vec::new();
    for (subject_key, (owner, name)) in repos {
        match runtime.block_on(handles.links.github.read_repo_authority(owner, name)) {
            Ok(observation) => handles.repo_check_writes.record(&RepoAuthorityCheck {
                subject_key,
                observation,
                checked_at: now,
            })?,
            Err(err) => unreadable.push(render_github_could_not_be_asked(
                subject_key.as_str(),
                &err.to_string(),
            )),
        }
    }
    Ok(unreadable)
}

/// This reader's latest observation of each repo subject the claims name,
/// decoded for the pure classifier.
fn latest_repo_observations(
    repo_checks: &dyn RepoAuthorityCheckReadPort,
    claims: &[GithubClaim],
    now: DateTime<Utc>,
) -> Result<BTreeMap<RepoSubjectKey, LatestRepoObservation>> {
    let subjects: Vec<RepoSubjectKey> = claims
        .iter()
        .filter_map(GithubClaim::repo_key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(repo_checks
        .latest(&subjects, now)?
        .into_iter()
        .map(|check| {
            (
                check.subject_key,
                latest_observation(check.observation, check.checked_at),
            )
        })
        .collect())
}

fn latest_observation(
    observation: RepoObservation,
    checked_at: DateTime<Utc>,
) -> LatestRepoObservation {
    let finding = match observation {
        RepoObservation::Found(found) => RepoFinding::Found {
            owner_id: found.owner_id,
            owner_kind: found.owner_type,
            authority_file: match found.authority_file {
                AuthorityFile::Absent => AuthorityFileSeen::Absent,
                AuthorityFile::Present(dids) => AuthorityFileSeen::Present(dids),
                AuthorityFile::Invalid => AuthorityFileSeen::Invalid,
            },
        },
        RepoObservation::RepoNotFound => RepoFinding::NotFound,
    };
    LatestRepoObservation {
        finding,
        checked_at,
    }
}

// -----------------------------------------------------------------------------
// Identity-link checks the labels need
// -----------------------------------------------------------------------------

/// Re-check, once each, the authors' active links the labels stand on: a
/// link to the account a person claim names, or to the user that owns a
/// repo a claim is about. A link GitHub cannot answer for records nothing.
fn recheck_needed_links(
    handles: &AuthorityVerifyHandles<'_>,
    runtime: &tokio::runtime::Runtime,
    claims: &[GithubClaim],
    repos: &BTreeMap<RepoSubjectKey, LatestRepoObservation>,
    now: DateTime<Utc>,
) -> Result<()> {
    let authors: BTreeSet<&str> = claims.iter().map(|claim| claim.author.as_str()).collect();
    for author in authors {
        let by_author: Vec<&GithubClaim> = claims
            .iter()
            .filter(|claim| claim.author == author)
            .collect();
        let links = &handles.links;
        let active = links_in_scope(
            links.checks,
            links.claims,
            links.lineage,
            &RecheckScope::Person(author.to_string()),
            now,
        )?;
        for to_recheck in active
            .iter()
            .filter(|to_recheck| link_is_needed(to_recheck, &by_author, repos))
        {
            // An unreadable answer records nothing: the latest local check decides.
            let _decided_or_unreadable = recheck_and_record(
                links.github,
                links.check_writes,
                runtime,
                &to_recheck.link,
                &to_recheck.login,
                now,
            )?;
        }
    }
    Ok(())
}

/// Does one of the author's claims stand on this link?
fn link_is_needed(
    to_recheck: &LinkToRecheck,
    claims: &[&GithubClaim],
    repos: &BTreeMap<RepoSubjectKey, LatestRepoObservation>,
) -> bool {
    claims.iter().any(|claim| match &claim.required {
        Requirement::Person(person) => {
            person.login().eq_ignore_ascii_case(&to_recheck.login)
                || person.login().eq_ignore_ascii_case(&to_recheck.link.login)
        }
        Requirement::Repo { .. } => claim
            .repo_key()
            .and_then(|key| repos.get(&key))
            .is_some_and(|observed| match observed.finding {
                RepoFinding::Found { owner_id, .. } => owner_id == to_recheck.link.user_id,
                RepoFinding::NotFound => false,
            }),
        Requirement::Exempt | Requirement::Malformed => false,
    })
}

// -----------------------------------------------------------------------------
// Labels
// -----------------------------------------------------------------------------

/// Each claim's label from the latest local checks.
fn label_claims(
    handles: &AuthorityVerifyHandles<'_>,
    claims: &[GithubClaim],
    repos: &BTreeMap<RepoSubjectKey, LatestRepoObservation>,
    now: DateTime<Utc>,
) -> Result<Vec<LabelledClaim>> {
    let active_links: Vec<IdentityLink> =
        StoredLinkLineage::read(handles.links.claims, handles.links.lineage)
            .context("reading the identity links")?
            .active_links();
    let mut evidence_by_author: BTreeMap<&str, Vec<LinkedAccountEvidence>> = BTreeMap::new();
    for claim in claims {
        if !evidence_by_author.contains_key(claim.author.as_str()) {
            let evidence = adapter_duckdb::linked_account_evidence(
                handles.links.checks,
                &claim.author,
                &active_links,
                now,
            )
            .context("reading the identity link checks")?;
            evidence_by_author.insert(&claim.author, evidence);
        }
    }
    Ok(claims
        .iter()
        .filter_map(|claim| {
            let evidence = AuthorityEvidence {
                author: &claim.author,
                author_links: evidence_by_author
                    .get(claim.author.as_str())
                    .map_or(&[], Vec::as_slice),
                repo: claim.repo_key().and_then(|key| repos.get(&key)),
            };
            classify(claim.shape(), &evidence, now).map(|authority| LabelledClaim {
                subject: claim.subject.clone(),
                author: claim.author.clone(),
                authority,
            })
        })
        .collect())
}
