//! `openlore infer people [--sign N[,N...]]` — propose "person adheres to
//! philosophy" candidates from the contribution links and the signed repo
//! claims, optionally signing selected ones (contributor-philosophy-inference
//! US-CPI-002/003; ADR-063; DDD-6/12/13).
//!
//! Effect shell only: read links (`ContributionLinkPort::list_links`) and
//! each linked repo's claims in every stored letter case
//! (`StoreReadPort` subject listing + `StoragePort::query_federated_by_subject`,
//! DDD-6 — no new storage method), hand the values to the PURE
//! `scraper_domain::infer_person_candidates`, render, and — only on `--sign`
//! — route the selection through the SHARED sign batch (DDD-12). Without
//! `--sign` nothing is written (D-1 / KPI-CPI-3), and no network is touched
//! (KPI-5): every read is a local store read. My own adherence claims are read
//! (`StoragePort::query_by_contributor` + `read_signed_claim`) so pairs I
//! already signed are shown, never re-proposed (DDD-8).
//!
//! Each listed person's published identity links show under their block as
//! this reader last checked them (ADR-085 §6, C-GDL-7): read through the
//! check, link-candidate and lineage READ handles only — no check is written,
//! purged or refreshed, and GitHub is never asked. Which person a link
//! belongs to is the pure U-6 join (`claim_domain::identity_link`).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use claim_domain::identity_link::{links_of_person, ListedLink, PersonKey, RecheckScope};
use claim_domain::{Cid, ClaimReference, ReferenceType};
use ports::{ContributionLink, LinkFilter, PageRequest, StoreReadError};
use scraper_domain::{
    encode_provenance, infer_people_report, possible_renames, repo_subjects_to_read,
    CandidateStatus, InferenceFilter, InferenceReport, NumberedCandidate, OwnClaim, PersonSubject,
    RepoClaim, ADHERES_TO_PHILOSOPHY,
};

use crate::render::{render_linked_inference_report, render_person_derived_from, PersonLinkLines};
use crate::verbs::identity_reads::read_listed_links;
use crate::verbs::sign_batch::{self, SignableCandidate};
use crate::wiring::Wiring;

/// Argument struct for `infer people` (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct InferPeopleArgs {
    /// Optional raw `--sign N[,N...]` selection (1-based), validated by the
    /// shared sign batch before any compose begins.
    pub sign: Option<String>,
    /// Optional raw `--person`; must be `github:<login>` (D-8).
    pub person: Option<String>,
    /// Optional `--min-repos N` (DDD-13).
    pub min_repos: Option<usize>,
}

/// Outcome of one `infer people` run — exit code + stdout chunk.
pub struct InferPeopleOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run `infer people`: read -> infer (pure) -> render -> optional sign batch.
pub fn run(wiring: &Wiring, args: &InferPeopleArgs) -> Result<InferPeopleOutcome> {
    let filter = InferenceFilter {
        person: args
            .person
            .as_deref()
            .map(PersonSubject::parse)
            .transpose()?,
        min_repos: args.min_repos.unwrap_or(0),
    };
    let (report, links) = read_inference(wiring, &filter)?;
    let people: BTreeSet<&str> = report
        .candidates
        .iter()
        .map(|numbered| numbered.candidate.person_subject())
        .collect();
    let identity_links = read_person_link_lines(wiring, people, &links)?;
    let outcome = sign_batch::list_or_sign(
        wiring,
        &person_signables(&report),
        args.sign.as_deref(),
        render_linked_inference_report(
            &report,
            &candidate_renames(&report, &links),
            &identity_links,
        ),
    )?;
    Ok(InferPeopleOutcome {
        exit_code: outcome.exit_code,
        stdout: outcome.stdout,
    })
}

/// The inference as the store holds it NOW, under `filter`: read the links,
/// every signed claim about each linked repo (all letter cases) and my own
/// adherence claims, then hand them to the PURE core. The one read path shared
/// by `infer people` and the `scrape github` new-candidates hint (DDD-14), so
/// the hint counts exactly what `infer people` would list. Read-only.
pub(crate) fn read_inference_report(
    wiring: &Wiring,
    filter: &InferenceFilter,
) -> Result<InferenceReport> {
    read_inference(wiring, filter).map(|(report, _)| report)
}

/// [`read_inference_report`], with the contribution links it read.
fn read_inference(
    wiring: &Wiring,
    filter: &InferenceFilter,
) -> Result<(InferenceReport, Vec<ContributionLink>)> {
    let links = wiring
        .contribution_links
        .list_links(&LinkFilter::All)
        .map_err(|e| anyhow!("reading contribution links: {e}"))?;
    let repo_claims = read_linked_repo_claims(wiring, &links)?;
    let own_claims = read_own_adherence_claims(wiring)?;
    let report = infer_people_report(&links, &repo_claims, &own_claims, filter);
    Ok((report, links))
}

/// Each candidate person's possible renames (other logins with the same
/// GitHub user id), keyed by person subject; people with none are left out.
/// Pure.
fn candidate_renames(
    report: &InferenceReport,
    links: &[ContributionLink],
) -> BTreeMap<String, Vec<String>> {
    report
        .candidates
        .iter()
        .map(|numbered| numbered.candidate.person_subject().to_string())
        .map(|person| {
            let others = possible_renames(&person, links);
            (person, others)
        })
        .filter(|(_, others)| !others.is_empty())
        .collect()
}

/// The published identity links of each of `people` (person subjects), as
/// this reader last checked them — read offline through the READ handles
/// only; nothing is read when there is nobody to show. Shared with the
/// `scrape github <user>` / `scrape person` person view.
pub(crate) fn read_person_link_lines<'a>(
    wiring: &Wiring,
    people: impl IntoIterator<Item = &'a str>,
    contribution_links: &[ContributionLink],
) -> Result<PersonLinkLines> {
    let people: Vec<&str> = people.into_iter().collect();
    let now = wiring.clock.now_utc();
    if people.is_empty() {
        return Ok(PersonLinkLines {
            by_person: BTreeMap::new(),
            now,
        });
    }
    let shown = read_listed_links(
        wiring.identity_link_checks.as_ref(),
        wiring.link_candidates.as_ref(),
        wiring.link_lineage.as_ref(),
        &RecheckScope::Everyone,
        now,
    )?;
    Ok(PersonLinkLines {
        by_person: links_by_person(&people, contribution_links, &shown),
        now,
    })
}

/// Each person's shown links (U-6), keyed by person subject; people with
/// none are left out. Pure.
fn links_by_person(
    people: &[&str],
    contribution_links: &[ContributionLink],
    shown: &[ListedLink],
) -> BTreeMap<String, Vec<ListedLink>> {
    people
        .iter()
        .map(|person| {
            let key = person_key(person, contribution_links);
            (person.to_string(), links_of_person(shown, &key))
        })
        .filter(|(_, listed)| !listed.is_empty())
        .collect()
}

/// A person row's join key: the GitHub account ids its contribution links
/// carry, else its login (U-6). Pure.
fn person_key(person_subject: &str, contribution_links: &[ContributionLink]) -> PersonKey {
    let login = person_subject
        .strip_prefix("github:")
        .unwrap_or(person_subject);
    PersonKey::of(
        login,
        contribution_links
            .iter()
            .filter(|link| link.person_subject.eq_ignore_ascii_case(person_subject))
            .map(|link| link.github_user_id),
    )
}

/// Every signed claim (own + peer) about each linked repo, in ANY letter case
/// (DDD-6 / UC-6). The federated read matches its subject exactly, so the
/// shell first learns which spellings the store holds (existing read-only
/// `StoreReadPort` listings — no new storage method), lets the pure core pick
/// every spelling of a linked repo, then reads each one. Distinct spellings
/// are disjoint exact-match reads, so no row is read twice.
fn read_linked_repo_claims(wiring: &Wiring, links: &[ContributionLink]) -> Result<Vec<RepoClaim>> {
    let stored = stored_claim_subjects(wiring)?;
    repo_subjects_to_read(links, stored.iter().map(String::as_str))
        .iter()
        .try_fold(Vec::new(), |mut claims, subject| {
            let rows = wiring
                .storage
                .query_federated_by_subject(subject)
                .map_err(|e| anyhow!("reading signed claims about {subject}: {e}"))?;
            claims.extend(rows.iter().map(RepoClaim::from));
            Ok(claims)
        })
}

/// My own adherence claims, with their references, in the shape the pure
/// already-signed check reads (DDD-8). Retraction markers and successors copy
/// the adherence predicate, so they arrive in the same listing; the attributed
/// listing carries no references, so each is re-read by CID.
fn read_own_adherence_claims(wiring: &Wiring) -> Result<Vec<OwnClaim>> {
    let me = wiring.identity.author_did();
    wiring
        .storage
        .query_by_contributor(me)
        .map_err(|e| anyhow!("reading my own signed claims: {e}"))?
        .iter()
        .filter(|row| row.predicate == ADHERES_TO_PHILOSOPHY)
        .map(|row| {
            let signed = wiring
                .storage
                .read_signed_claim(&row.cid)
                .map_err(|e| anyhow!("reading my claim {}: {e}", row.cid.0))?
                .ok_or_else(|| anyhow!("my claim {} is listed but not stored", row.cid.0))?;
            Ok(OwnClaim {
                subject: signed.unsigned.subject,
                predicate: signed.unsigned.predicate,
                object: signed.unsigned.object,
                author_did: row.author_did.0.clone(),
                cid: row.cid.0.clone(),
                evidence: signed.unsigned.evidence,
                composed_at: signed.unsigned.composed_at,
                references: signed.unsigned.references,
            })
        })
        .collect()
}

/// Page size for enumerating stored claim subjects.
const SUBJECT_PAGE: u64 = 500;

/// The distinct subjects of every stored claim, own and peer.
fn stored_claim_subjects(wiring: &Wiring) -> Result<BTreeSet<String>> {
    let own = read_all_pages(|request| {
        let page = wiring.store_read.list_claims(request)?;
        Ok((
            page.rows.into_iter().map(|row| row.subject).collect(),
            page.total,
        ))
    })?;
    let peer = read_all_pages(|request| {
        let page = wiring.store_read.list_peer_claims(request)?;
        Ok((
            page.rows.into_iter().map(|row| row.subject).collect(),
            page.total,
        ))
    })?;
    Ok(own.into_iter().chain(peer).collect())
}

/// Walk an offset/limit listing to its end, collecting each page's subjects.
fn read_all_pages(
    mut read_page: impl FnMut(PageRequest) -> Result<(Vec<String>, u64), StoreReadError>,
) -> Result<Vec<String>> {
    let mut subjects = Vec::new();
    let mut offset = 0;
    loop {
        let (rows, total) = read_page(PageRequest {
            offset,
            limit: SUBJECT_PAGE,
        })
        .map_err(|e| anyhow!("listing stored claim subjects: {e}"))?;
        let fetched = rows.len() as u64;
        subjects.extend(rows);
        offset += fetched;
        if fetched == 0 || offset >= total {
            return Ok(subjects);
        }
    }
}

/// The report's numbered candidates, in order, as the shared sign batch's
/// input — the ONE builder both `infer people --sign` and the `scrape github
/// <user> --sign` person view use (DDD-12 / DDD-13), so either surface signs
/// the identical claim (same CID) for the same number.
pub(crate) fn person_signables(report: &InferenceReport) -> Vec<SignableCandidate> {
    report.candidates.iter().map(signable_from).collect()
}

/// Pre-fill the shared compose editor from one numbered candidate: ADR-064
/// subject/predicate/object, the provenance as `evidence[]`, the DDD-9
/// proposed confidence, and its references — none for a NEW inference, one
/// `supersedes` of my earlier claim for a STRONGER one (DDD-12; D-5: the
/// earlier claim itself is never touched).
fn signable_from(numbered: &NumberedCandidate) -> SignableCandidate {
    let candidate = &numbered.candidate;
    SignableCandidate {
        subject: candidate.person_subject().to_string(),
        predicate: ADHERES_TO_PHILOSOPHY.to_string(),
        object: candidate.philosophy().to_string(),
        evidence: encode_provenance(candidate),
        confidence: candidate.confidence().as_decimal(),
        references: references_for(&numbered.status),
        derived_from: render_person_derived_from(candidate),
    }
}

/// The typed references a candidate's status adds to the signed claim.
fn references_for(status: &CandidateStatus) -> Vec<ClaimReference> {
    match status {
        CandidateStatus::New => Vec::new(),
        CandidateStatus::Stronger { supersedes } => vec![ClaimReference {
            ref_type: ReferenceType::Supersedes,
            cid: Cid(supersedes.clone()),
        }],
    }
}
