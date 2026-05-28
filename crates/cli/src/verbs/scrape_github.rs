//! `scrape github <target> [--sign N[,N...]]` — derive candidate claims from
//! a public GitHub target, optionally signing selected candidates through the
//! slice-01 pipeline (slice-02; US-SCR-001..004; ADR-017 / ADR-019).
//!
//! Step 01-04 BOOTSTRAP: this module declares the verb's argument struct +
//! outcome shape and a `todo!()` handler. The live pipeline lands per the
//! SCR-* acceptance scenarios in Phase 03/05:
//!
//! 1. print the public-data banner;
//! 2. `resolve_target` + `harvest_repo`/`harvest_user` via `GithubPort`
//!    (the effect-shell `adapter-github`);
//! 3. `derive_candidates(signals, mapping)` via the PURE `scraper-domain`;
//! 4. render the candidate list (each candidate names its source signals —
//!    auditability, KPI-SCR-3);
//! 5. IF `--sign`: the verb-level `SelectionParser` validates the raw index
//!    list (reject duplicates / out-of-range BEFORE any compose begins),
//!    then walks each selected candidate through its OWN compose preview and
//!    invokes the slice-01 `VerbClaimAdd` + `VerbClaimPublish` internals —
//!    NO parallel publish path (single-publish-path; ADR-003 + WD-22).
//!
//! ## The human-gate at the type level (I-SCR-1 / WD-49)
//!
//! `adapter-github` holds NO storage / identity / PDS reference — by
//! construction it cannot sign or publish. WITHOUT `--sign` this verb
//! performs ZERO writes (derive + render only; the
//! `scraper_never_persists_unsigned` acceptance gate). The ONLY path from a
//! candidate to a persisted claim is the human's `--sign` gesture routed
//! through the slice-01 pipeline.
//!
//! ## Pure-vs-effect split (ADR-009 / ADR-007)
//!
//! Candidate derivation + the candidate-list / banner rendering are pure
//! functions of the harvested signals; the effects — resolve, harvest, store
//! write, PDS publish — live in `run` (and, for `--sign`, in the reused
//! slice-01 verb internals).

use anyhow::Result;
use ports::TargetKind;
use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

use crate::render::{render_auth_report, render_candidate_list, render_public_data_banner};
use crate::verbs::claim_publish::build_tokio_runtime;
use crate::wiring::Wiring;

/// Argument struct for the `scrape github` verb (mirrors the clap subcommand).
///
/// `sign` is the RAW, unparsed `--sign N[,N...]` string (or `None`). The
/// verb-level `SelectionParser` (Phase 03/05; architecture-design §5.1) turns
/// it into validated 1-based indices — rejecting duplicates / out-of-range
/// BEFORE any compose begins. The clap layer deliberately does NOT parse it,
/// so a malformed list produces a domain-shaped error from the verb (with the
/// candidate count for context), not a generic clap parse error.
#[derive(Debug, Clone)]
pub struct ScrapeGithubArgs {
    /// The public GitHub target: `owner/repo` or a bare `user`.
    pub target: String,
    /// Optional raw `--sign N[,N...]` selection (1-based indices), unparsed.
    pub sign: Option<String>,
}

/// Outcome of one `scrape github` invocation — exit code + stdout chunk.
/// Verbs do not write stdout themselves; the dispatcher prints `stdout`.
pub struct ScrapeGithubOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `scrape github` verb (Step 03-01: harvest -> derive -> render;
/// NO `--sign` path here — that lands in a later step).
///
/// The pipeline (architecture-design §5 / journey step 1-2):
///
/// 1. print the public-data-only banner (BEFORE any harvest — WD-51);
/// 2. resolve the target via `GithubPort::resolve_target` (refusing
///    private / non-existent targets);
/// 3. harvest the bounded public signal set via `GithubPort::harvest_repo`
///    / `harvest_user`, reporting the count;
/// 4. derive candidates via the PURE `scraper-domain::derive_candidates`
///    (confidence 0.25 speculative; each candidate names its source signal);
/// 5. render the numbered candidate list (or "No candidate claims could be
///    derived" when nothing matched the mapping — not an error).
///
/// WITHOUT `--sign` this verb performs ZERO writes (the human-gate at the
/// storage layer; `scraper_never_persists_unsigned`, I-SCR-1 / WD-49).
pub fn run(wiring: &Wiring, args: &ScrapeGithubArgs) -> Result<ScrapeGithubOutcome> {
    // (1) Public-data-only banner — printed BEFORE any harvest (WD-51). It
    // goes to stdout NOW (not into the returned chunk) so the user is
    // reassured BEFORE any network beat even when the resolve / harvest
    // later refuses. A refusal returns `Err(..)` so the dispatcher surfaces
    // the cause on stderr with a non-zero exit and renders NO partial
    // candidate list; the banner has already landed on stdout above.
    print!("{}", render_public_data_banner());

    // The rest of the verb's stdout is accumulated and returned to the
    // dispatcher (which prints it after a successful run).
    let mut out = String::new();

    // (2) Resolve the target (refuses private / non-existent). The harvest
    // is the only network step; both run on one tokio runtime.
    let runtime = build_tokio_runtime();
    let kind = runtime
        .block_on(wiring.github.resolve_target(&args.target))
        .map_err(anyhow::Error::from)?;
    out.push_str(&format!(
        "Resolving target {} ... ok ({})\n",
        args.target,
        target_kind_label(&kind)
    ));

    // (3) Harvest the bounded public signal set + report the count.
    let signals = runtime
        .block_on(harvest(wiring, &kind))
        .map_err(anyhow::Error::from)?;
    out.push_str(&format!(
        "Harvesting public signals ... {} signal{}\n",
        signals.len(),
        if signals.len() == 1 { "" } else { "s" }
    ));

    // (3a) Report the auth-mode + rate budget the harvest observed (ADR-019
    // §5; US-SCR-004; journey step 1). The adapter parsed the budget from the
    // harvest response and recorded it in its effect-shell slot; we take it
    // here and render the PURE auth-line ("authenticated (N/M rate budget)" /
    // "unauthenticated"). The token value is NEVER part of this — an
    // `AuthReport` carries only the budget numbers (no-token-leak).
    out.push_str(&render_auth_report(&adapter_github::take_last_auth_report()));

    // (4) Derive candidates via the PURE scraper-domain (confidence 0.25;
    // each candidate names its source signal). The mapping is the embedded
    // SSOT snapshot — a parse failure is a build-time-verified impossibility,
    // surfaced as an error rather than a panic for railway discipline.
    let mapping = load_mapping(EMBEDDED_MAPPING_YAML)
        .map_err(|e| anyhow::anyhow!("embedded signal->predicate mapping failed to parse: {e}"))?;
    let subject = subject_for(&kind);
    let candidates = derive_candidates(&subject, &signals, &mapping);

    // (5) Render the candidate list (or the no-candidates message).
    if candidates.is_empty() {
        out.push_str(
            "No candidate claims could be derived from the harvested signals \
             (nothing to propose).\n",
        );
    } else {
        out.push_str(&render_candidate_list(&subject, &candidates));
    }

    Ok(ScrapeGithubOutcome {
        exit_code: 0,
        stdout: out,
    })
}

/// Harvest the bounded public signal set for the resolved target kind.
/// `Repo` harvests the repo's signals; `User` harvests a bounded cross-repo
/// aggregate (deep triangulation deferred to slice-04 per WD-64).
async fn harvest(
    wiring: &Wiring,
    kind: &TargetKind,
) -> Result<Vec<ports::Signal>, ports::GithubError> {
    match kind {
        TargetKind::Repo { owner, repo } => wiring.github.harvest_repo(owner, repo).await,
        TargetKind::User { user } => wiring.github.harvest_user(user).await,
    }
}

/// The `github:<owner>/<repo>` or `github:<user>` subject string the
/// candidate list + any future signed claim carry (the `github_target`
/// shared artifact).
fn subject_for(kind: &TargetKind) -> String {
    match kind {
        TargetKind::Repo { owner, repo } => format!("github:{owner}/{repo}"),
        TargetKind::User { user } => format!("github:{user}"),
    }
}

/// The human-readable resolution label for the "Resolving target ... ok"
/// line (journey step 1: "ok (repository)" / "ok (user)").
fn target_kind_label(kind: &TargetKind) -> &'static str {
    match kind {
        TargetKind::Repo { .. } => "repository",
        TargetKind::User { .. } => "user",
    }
}
