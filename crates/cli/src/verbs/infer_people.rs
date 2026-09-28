//! `openlore infer people [--sign N[,N...]]` — propose "person adheres to
//! philosophy" candidates from the contribution links and the signed repo
//! claims, optionally signing selected ones (contributor-philosophy-inference
//! US-CPI-002/003; ADR-063; DDD-6/12/13).
//!
//! Effect shell only: read links (`ContributionLinkPort::list_links`) and
//! each linked repo's claims (`StoragePort::query_federated_by_subject`,
//! DDD-6 — no new storage method), hand the values to the PURE
//! `scraper_domain::infer_person_candidates`, render, and — only on `--sign`
//! — route the selection through the SHARED sign batch (DDD-12). Without
//! `--sign` nothing is written (D-1 / KPI-CPI-3).

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use ports::{ContributionLink, LinkFilter};
use scraper_domain::{
    encode_provenance, infer_person_candidates, PersonCandidate, RepoClaim, ADHERES_TO_PHILOSOPHY,
};

use crate::render::{render_person_candidates, render_person_derived_from};
use crate::verbs::sign_batch::{self, SignableCandidate};
use crate::wiring::Wiring;

/// Argument struct for `infer people` (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct InferPeopleArgs {
    /// Optional raw `--sign N[,N...]` selection (1-based), validated by the
    /// shared sign batch before any compose begins.
    pub sign: Option<String>,
}

/// Outcome of one `infer people` run — exit code + stdout chunk.
pub struct InferPeopleOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run `infer people`: read -> infer (pure) -> render -> optional sign batch.
pub fn run(wiring: &Wiring, args: &InferPeopleArgs) -> Result<InferPeopleOutcome> {
    let links = wiring
        .contribution_links
        .list_links(&LinkFilter::All)
        .map_err(|e| anyhow!("reading contribution links: {e}"))?;
    let repo_claims = read_linked_repo_claims(wiring, &links)?;
    let candidates = infer_person_candidates(&links, &repo_claims);
    let rendered = render_person_candidates(&candidates);

    let Some(raw_selection) = args.sign.as_deref() else {
        return Ok(InferPeopleOutcome {
            exit_code: 0,
            stdout: rendered,
        });
    };
    let signables: Vec<SignableCandidate> = candidates.iter().map(signable_from).collect();
    let exit_code = sign_batch::sign_selected(wiring, &signables, raw_selection, &rendered)?;
    Ok(InferPeopleOutcome {
        exit_code,
        stdout: String::new(),
    })
}

/// Every signed claim (own + peer) about each distinct linked repo (DDD-6).
fn read_linked_repo_claims(wiring: &Wiring, links: &[ContributionLink]) -> Result<Vec<RepoClaim>> {
    let repo_subjects: BTreeMap<String, &str> = links
        .iter()
        .map(|link| {
            (
                link.repo_subject.to_ascii_lowercase(),
                link.repo_subject.as_str(),
            )
        })
        .collect();
    repo_subjects
        .values()
        .try_fold(Vec::new(), |mut claims, subject| {
            let rows = wiring
                .storage
                .query_federated_by_subject(subject)
                .map_err(|e| anyhow!("reading signed claims about {subject}: {e}"))?;
            claims.extend(rows.iter().map(RepoClaim::from));
            Ok(claims)
        })
}

/// Pre-fill the shared compose editor from one person candidate: ADR-064
/// subject/predicate/object, the provenance as `evidence[]`, the DDD-9
/// proposed confidence, and no references (a NEW inference supersedes
/// nothing).
fn signable_from(candidate: &PersonCandidate) -> SignableCandidate {
    SignableCandidate {
        subject: candidate.person_subject().to_string(),
        predicate: ADHERES_TO_PHILOSOPHY.to_string(),
        object: candidate.philosophy().to_string(),
        evidence: encode_provenance(candidate),
        confidence: candidate.confidence().as_decimal(),
        references: Vec::new(),
        derived_from: render_person_derived_from(candidate),
    }
}
