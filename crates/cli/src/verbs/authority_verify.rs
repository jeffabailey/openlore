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
    requirement, ClaimShape, LatestRepoObservation, RepoFinding, Requirement,
};
use claim_domain::identity_link::RecheckScope;
use claim_domain::ReferenceType;
use ports::{
    GithubError, GithubPort, PageRequest, RepoAuthorityCheck, RepoAuthorityCheckError,
    RepoAuthorityCheckReadPort, RepoAuthorityCheckWritePort, RepoSubjectKey, StoreReadPort,
};

use crate::reader_authority::{needs_authority, AuthorityReads, LocalEvidence};
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

impl AuthorityVerifyHandles<'_> {
    /// The READ side of these handles: the evidence path every reader uses.
    fn reads(&self) -> AuthorityReads<'_> {
        AuthorityReads {
            checks: self.links.checks,
            candidates: self.links.claims,
            lineage: self.links.lineage,
            repo_checks: self.repo_checks,
        }
    }
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
    let named_repo = args.subject.as_deref().and_then(repo_named);
    if claims.is_empty() && named_repo.is_none() {
        return Ok(AuthorityVerifyReport {
            stdout: render_nothing_to_recheck(),
            unreadable: Vec::new(),
        });
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    let unreadable = observe_repos(
        handles,
        &runtime,
        repos_to_observe(&claims, named_repo),
        now,
    )?;
    let reads = handles.reads();
    let repos =
        reads.latest_repo_observations(claims.iter().filter_map(GithubClaim::repo_key), now)?;
    recheck_needed_links(handles, &runtime, &claims, &repos, now)?;
    let evidence = LocalEvidence {
        links_by_author: reads
            .linked_accounts_by_author(claims.iter().map(|claim| claim.author.as_str()), now)?,
        repos,
    };
    let labelled = label_claims(&claims, &evidence, now);
    Ok(AuthorityVerifyReport {
        stdout: render_authority_labels(&labelled),
        unreadable,
    })
}

/// A repo GitHub is asked about: its subject key, owner and name.
type RepoToObserve = (RepoSubjectKey, String, String);

/// The repo a `--subject github:<owner>/<repo>` names, whether or not a
/// stored claim names it yet: the first claim about a repo is signed only
/// after its authority was checked (U-OOS-D12).
fn repo_named(subject: &str) -> Option<RepoToObserve> {
    let as_an_assertion = ClaimShape {
        subject,
        predicate: "",
        reference_types: &[],
    };
    match requirement(as_an_assertion) {
        Requirement::Repo { owner, name } => Some((RepoSubjectKey::of(subject), owner, name)),
        _ => None,
    }
}

/// Each distinct repo subject (case-folded) the claims name, plus the one
/// `--subject` names.
fn repos_to_observe(
    claims: &[GithubClaim],
    named_repo: Option<RepoToObserve>,
) -> BTreeMap<RepoSubjectKey, (String, String)> {
    claims
        .iter()
        .filter_map(|claim| match &claim.required {
            Requirement::Repo { owner, name } => Some((
                RepoSubjectKey::of(&claim.subject),
                owner.clone(),
                name.clone(),
            )),
            _ => None,
        })
        .chain(named_repo)
        .map(|(key, owner, name)| (key, (owner, name)))
        .collect()
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
        .filter(|claim| needs_authority(claim.shape()))
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

/// Observe each repo once and record what GitHub answered; a repo GitHub
/// cannot answer for records nothing and is reported instead.
fn observe_repos(
    handles: &AuthorityVerifyHandles<'_>,
    runtime: &tokio::runtime::Runtime,
    repos: BTreeMap<RepoSubjectKey, (String, String)>,
    now: DateTime<Utc>,
) -> Result<Vec<String>> {
    let mut unreadable = Vec::new();
    for (subject_key, (owner, name)) in repos {
        let subject = subject_key.as_str().to_string();
        let observed = observe_and_record_repo(
            handles.links.github,
            handles.repo_check_writes,
            runtime,
            subject_key,
            (&owner, &name),
            now,
        )?;
        if let Err(err) = observed {
            unreadable.push(render_github_could_not_be_asked(&subject, &err.to_string()));
        }
    }
    Ok(unreadable)
}

/// One look at the repo `owner/name` and its authority file
/// (`read_repo_authority`), recorded as this reader's observation of
/// `subject_key` at `checked_at` once GitHub answered — the look `authority
/// verify` and a repo scrape share (ADR-087 §6). When GitHub cannot answer,
/// nothing is recorded and why is returned instead.
pub(crate) fn observe_and_record_repo(
    github: &dyn GithubPort,
    repo_check_writes: &dyn RepoAuthorityCheckWritePort,
    runtime: &tokio::runtime::Runtime,
    subject_key: RepoSubjectKey,
    (owner, name): (&str, &str),
    checked_at: DateTime<Utc>,
) -> std::result::Result<std::result::Result<(), GithubError>, RepoAuthorityCheckError> {
    match runtime.block_on(github.read_repo_authority(owner, name)) {
        Ok(observation) => repo_check_writes
            .record(&RepoAuthorityCheck {
                subject_key,
                observation,
                checked_at,
            })
            .map(Ok),
        Err(unanswered) => Ok(Err(unanswered)),
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

/// Each claim's label, judged from this reader's latest local checks.
fn label_claims(
    claims: &[GithubClaim],
    evidence: &LocalEvidence,
    now: DateTime<Utc>,
) -> Vec<LabelledClaim> {
    claims
        .iter()
        .filter_map(|claim| {
            evidence
                .judge(claim.shape(), &claim.author, now)
                .map(|authority| LabelledClaim {
                    subject: claim.subject.clone(),
                    author: claim.author.clone(),
                    authority,
                })
        })
        .collect()
}
