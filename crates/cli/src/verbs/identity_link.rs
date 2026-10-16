//! `identity link github <login> --yes` — publicly link a GitHub account to
//! the user's DID (ADR-084 §2, ADR-085 §2/§7).
//!
//! The effect shell around the pure decisions in `claim_domain::identity_link`:
//! read the profile fresh (no cache), let `check_link` decide, record the one
//! check, and only for a proven link compose through `LinkDraft`, sign, keep
//! the claim locally and publish it through the single publish path. The
//! shared compose step (`claim add`'s `build_unsigned_claim`) is bypassed by
//! design: this verb never names the link predicate (Q-RM-4).

use std::io::Write;

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    check_link, is_already_linked, is_github_login, CheckVerdict, GithubObservation, LinkDraft,
    ProofCheck, ProvenLink,
};
use claim_domain::{canonicalize, compute_cid, SignedClaim};
use ports::{GithubError, LinkCheck};

use crate::io::prompt_line;
use crate::render::{
    render_already_linked, render_github_unreadable, render_link_declined,
    render_link_kept_locally, render_link_preview, render_link_refused, render_not_a_github_login,
};
use crate::verbs::claim_publish::{
    build_tokio_runtime, publish_signed_claim, render_publish_error, render_publish_success,
    PublishError,
};
use crate::verbs::identity_reads::{observation_of, StoredLinkLineage};
use crate::wiring::Wiring;

/// Arguments of `identity link github <login>`.
#[derive(Debug, Clone)]
pub struct IdentityLinkArgs {
    pub login: String,
    /// `--yes`: confirm without the interactive preview.
    pub yes: bool,
}

/// The one check a confirmed, passing link records (ADR-085 §7).
fn passing_check(proven: &ProvenLink, checked_at: DateTime<Utc>) -> LinkCheck {
    LinkCheck {
        subject_did: proven.did().to_string(),
        github_user_id: proven.user_id(),
        login_seen: proven.login().to_string(),
        verdict: CheckVerdict::Verified,
        checked_at,
    }
}

/// Read the account fresh from GitHub (one `GET /users/<login>`). `None` when
/// GitHub says it does not exist; any other failure — an answer naming no
/// account included — fails closed. The error
/// rendering never carries the token (`GithubError` never holds it).
fn read_account(wiring: &Wiring, login: &str) -> Result<Option<GithubObservation>> {
    match build_tokio_runtime().block_on(wiring.github.read_account(login)) {
        Ok(profile) => Ok(Some(observation_of(profile))),
        Err(GithubError::NotFound { .. }) => Ok(None),
        Err(err) => bail!("{}", render_github_unreadable(login, &err.to_string())),
    }
}

/// The proven link of a fresh read, or the refusal naming its verdict.
fn prove(wiring: &Wiring, login: &str) -> Result<ProvenLink> {
    let author = wiring.identity.author_did().clone();
    let Some(observation) = read_account(wiring, login)? else {
        bail!(
            "{}",
            render_link_refused(login, CheckVerdict::AccountNotFound)
        );
    };
    match check_link(&author, &observation) {
        Ok(ProofCheck::Verified(proven)) => Ok(proven),
        Ok(ProofCheck::Disproved { verdict, .. }) => {
            bail!("{}", render_link_refused(login, verdict))
        }
        Err(malformed) => bail!(
            "{}",
            render_github_unreadable(login, &malformed.to_string())
        ),
    }
}

/// D-a: refuse, before anything is signed or recorded, a link the local
/// store already holds as active.
fn refuse_if_already_linked(wiring: &Wiring, proven: &ProvenLink) -> Result<()> {
    let stored = StoredLinkLineage::read(
        wiring.link_candidates.as_ref(),
        wiring.link_lineage.as_ref(),
    )?;
    if is_already_linked(proven, &stored.active_links()) {
        bail!("{}", render_already_linked(proven.login()));
    }
    Ok(())
}

/// Sign the proven link and keep it in the local store.
fn sign_and_keep(wiring: &Wiring, draft: LinkDraft) -> Result<SignedClaim> {
    let unsigned = draft.into_unsigned();
    let canonical = canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing link: {e}"))?;
    let signature = wiring
        .identity
        .sign(&compute_cid(&canonical))
        .map_err(|e| anyhow!("signing link: {e}"))?;
    let signed = SignedClaim {
        unsigned,
        signature,
    };
    wiring
        .storage
        .write_signed_claim(&signed)
        .map_err(|e| anyhow!("keeping the signed link locally: {e:#}"))?;
    Ok(signed)
}

/// Show the exact record and ask; only an interactive `y` confirms (Q-GDL-D2).
fn confirmed_after_preview(draft: &LinkDraft) -> Result<bool> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(render_link_preview(draft.claim()).as_bytes())?;
    let answer = prompt_line(
        &mut stdout,
        &mut std::io::stdin().lock(),
        "Publish this link? [y/N]: ",
    )?;
    writeln!(stdout)?;
    Ok(answer.as_deref().map(str::trim) == Some("y"))
}

/// Record the one passing check, then publish the signed link.
fn link(wiring: &Wiring, proven: &ProvenLink, draft: LinkDraft) -> Result<String> {
    let signed = sign_and_keep(wiring, draft)?;
    wiring
        .identity_link_check_writes
        .record_check(&passing_check(proven, wiring.clock.now_utc()))?;
    let published = publish_signed_claim(wiring, &signed)
        .map_err(|err| anyhow!("{}", render_link_publish_failure(proven, &err)))?;
    Ok(render_publish_success(&published))
}

/// A failed publish keeps the signed link and its check (ADR-085 §7); an
/// unreachable repo host is said so, with the retry (Q-GDL-D1).
fn render_link_publish_failure(proven: &ProvenLink, err: &PublishError) -> String {
    match err {
        PublishError::PdsFailed { source, cid } => {
            render_link_kept_locally(proven.login(), &cid.0, &source.to_string())
        }
        PublishError::Other(_) => render_publish_error(err).trim_end().to_string(),
    }
}

/// Run `identity link github <login>`; returns the stdout to print. Nothing
/// is written unless the login is valid, a fresh read proves the link and
/// the link is confirmed (ADR-076, ADR-085 §4/§7).
pub fn run(wiring: &Wiring, args: &IdentityLinkArgs) -> Result<String> {
    if !is_github_login(&args.login) {
        bail!("{}", render_not_a_github_login(&args.login));
    }
    let proven = prove(wiring, &args.login)?;
    refuse_if_already_linked(wiring, &proven)?;
    let composed_at = wiring.clock.now_utc().to_rfc3339();
    let draft = LinkDraft::new(&proven, &composed_at);
    if !args.yes && !confirmed_after_preview(&draft)? {
        return Ok(render_link_declined());
    }
    link(wiring, &proven, draft)
}
