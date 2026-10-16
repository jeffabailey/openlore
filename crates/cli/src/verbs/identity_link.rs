//! `identity link github <login> --yes` — publicly link a GitHub account to
//! the user's DID (ADR-084 §2, ADR-085 §2/§7).
//!
//! The effect shell around the pure decisions in `claim_domain::identity_link`:
//! read the profile fresh (no cache), let `check_link` decide, record the one
//! check, and only for a proven link compose through `LinkDraft`, sign, keep
//! the claim locally and publish it through the single publish path. The
//! shared compose step (`claim add`'s `build_unsigned_claim`) is bypassed by
//! design: this verb never names the link predicate (Q-RM-4).

use anyhow::{anyhow, bail, Result};
use claim_domain::identity_link::{check_link, GithubObservation, LinkDraft, ProofCheck};
use claim_domain::{bare_did, canonicalize, compute_cid, SignedClaim};
use ports::{LinkCheck, PersonProfile};

use crate::verbs::claim_publish::{
    build_tokio_runtime, publish_signed_claim, render_publish_error, render_publish_success,
};
use crate::wiring::Wiring;

/// Arguments of `identity link github <login>`.
#[derive(Debug, Clone)]
pub struct IdentityLinkArgs {
    pub login: String,
    /// `--yes`: confirm without the interactive preview.
    pub yes: bool,
}

/// The observation a fresh profile read makes.
fn observation_of(profile: PersonProfile) -> GithubObservation {
    GithubObservation {
        login: profile.login,
        user_id: profile.id,
        bio: profile.bio,
    }
}

/// The check row one proof check records.
fn recorded_check(subject_did: &str, proof: &ProofCheck, wiring: &Wiring) -> LinkCheck {
    LinkCheck {
        subject_did: subject_did.to_string(),
        github_user_id: proof.user_id(),
        login_seen: proof.login_seen().to_string(),
        verdict: proof.verdict(),
        checked_at: wiring.clock.now_utc(),
    }
}

/// Read the account fresh from GitHub (one `GET /users/<login>`).
fn read_account(wiring: &Wiring, login: &str) -> Result<GithubObservation> {
    build_tokio_runtime()
        .block_on(wiring.github.read_person(login))
        .map(observation_of)
        .map_err(|err| anyhow!("GitHub could not be read for {login}: {err}"))
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

/// Run `identity link github <login>`; returns the stdout to print.
pub fn run(wiring: &Wiring, args: &IdentityLinkArgs) -> Result<String> {
    if !args.yes {
        bail!("the link preview is not available yet; confirm with --yes");
    }
    let author = wiring.identity.author_did().clone();
    let observation = read_account(wiring, &args.login)?;
    let proof = check_link(&author, &observation);
    let ProofCheck::Verified(proven) = &proof else {
        bail!(
            "github:{} is not linked ({})",
            args.login,
            proof.verdict().token()
        );
    };
    let composed_at = wiring.clock.now_utc().to_rfc3339();
    let signed = sign_and_keep(wiring, LinkDraft::new(proven, &composed_at))?;
    wiring
        .identity_link_check_writes
        .record_check(&recorded_check(bare_did(&author.0), &proof, wiring))?;
    let published = publish_signed_claim(wiring, &signed)
        .map_err(|err| anyhow!("{}", render_publish_error(&err).trim_end()))?;
    Ok(render_publish_success(&published))
}
