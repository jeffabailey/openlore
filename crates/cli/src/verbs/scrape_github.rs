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

use anyhow::{anyhow, Result};
use ports::{CandidateClaim, GithubError, LinkFilter, TargetKind};
use scraper_domain::{
    contributor_count_for, derive_candidates, load_mapping, new_inferred_candidate_count,
    select_contributors, shared_contributors, ContributorSelection, InferenceFilter,
    InferenceReport, SharedContributor, EMBEDDED_MAPPING_YAML,
};

use crate::render::{
    render_auth_report, render_candidate_list, render_contributors_block,
    render_contributors_not_recorded, render_new_inferred_candidates_hint,
    render_no_contributors_requested, render_public_data_banner, render_shared_contributors,
};
use crate::verbs::claim_publish::build_tokio_runtime;
use crate::verbs::infer_people::read_inference_report;
use crate::verbs::sign_batch::{self, SignableCandidate};
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
    /// Optional `--contributors N` override (validated by the pure
    /// `scraper_domain::contributor_count_for` before any request).
    pub contributors: Option<usize>,
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
    // (0) Validate `--contributors N` PURELY over the raw target, BEFORE the
    // banner or any GitHub request (OD-CPI-7 bound; UC-3 person refusal).
    let contributor_count = contributor_count_for(&args.target, args.contributors)?;

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

    // (3b) contributor-philosophy-inference (DDD-14): for a repo target, read
    // the RAW contributors (one request), select the top-N humans PURELY, and
    // record the snapshot as append-only contribution links in ONE tx. A
    // harvest failure aborts here, BEFORE any link write. Links are unsigned
    // local observations — never claims (the human-gate is untouched).
    let subject = subject_for(&kind);
    // (3a') The inference BEFORE this run writes anything (DDD-14): the
    // baseline the end-of-scrape hint diffs against. Repo targets only — a
    // user target records no links and signs no repo claim.
    let inference_before = match &kind {
        TargetKind::Repo { .. } => {
            Some(read_inference_report(wiring, &InferenceFilter::default())?)
        }
        TargetKind::User { .. } => None,
    };
    // N = 0 is a shell decision taken BEFORE the port: no request, no write.
    let contributors = match &kind {
        TargetKind::Repo { .. } if contributor_count == 0 => ContributorsOutcome::NoneRequested,
        TargetKind::Repo { owner, repo } => {
            match runtime.block_on(wiring.github.list_contributors(owner, repo)) {
                // UC-2: GitHub will not list them (too large / empty) — a
                // named notice, nothing recorded, the scrape carries on.
                Err(GithubError::ContributorsUnavailable { reason, .. }) => {
                    ContributorsOutcome::Unavailable(reason)
                }
                // Any other failure aborts BEFORE any link write (DDD-14).
                Err(fatal) => return Err(anyhow::Error::from(fatal)),
                Ok(rows) => {
                    let selection = select_contributors(&rows, contributor_count);
                    record_contributors(wiring, &subject, &selection)?;
                    let shared =
                        contributors_shared_with_other_repos(wiring, &subject, &selection)?;
                    ContributorsOutcome::Recorded { selection, shared }
                }
            }
        }
        TargetKind::User { .. } => ContributorsOutcome::NotARepo,
    };

    // (4) Derive candidates via the PURE scraper-domain (confidence 0.25;
    // each candidate names its source signal). The mapping is the embedded
    // SSOT snapshot — a parse failure is a build-time-verified impossibility,
    // surfaced as an error rather than a panic for railway discipline.
    let mapping = load_mapping(EMBEDDED_MAPPING_YAML)
        .map_err(|e| anyhow::anyhow!("embedded signal->predicate mapping failed to parse: {e}"))?;
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

    // (5a) The contributors block (repo targets only), below the candidates.
    out.push_str(&render_contributors_outcome(&contributors));

    // (6) WITHOUT --sign: derive + render only, ZERO claim writes (the
    // human-gate; scraper_never_persists_unsigned, I-SCR-1 / WD-49).
    // (7) --sign N[,N...]: validate + run the batch of individual human-gates.
    // The candidate-list block already accumulated in `out` is handed to the
    // batch, which emits it to stdout BEFORE composing so the user reviews it.
    let mut outcome = match args.sign.as_deref() {
        None => ScrapeGithubOutcome {
            exit_code: 0,
            stdout: out,
        },
        Some(raw_selection) => sign_selected_candidates(wiring, &candidates, raw_selection, &out)?,
    };

    // (8) The one-line new-inferred-candidates hint, AFTER `--sign` so repo
    // claims signed in this run count (DDD-14). Silent when nothing changed.
    if let Some(before) = inference_before {
        outcome
            .stdout
            .push_str(&new_inferred_candidates_hint(wiring, &before)?);
    }
    Ok(outcome)
}

/// Diff the inference before this run against the store now (PURE change
/// summary) and render the hint — empty when the run added no candidate.
fn new_inferred_candidates_hint(wiring: &Wiring, before: &InferenceReport) -> Result<String> {
    let after = read_inference_report(wiring, &InferenceFilter::default())?;
    Ok(render_new_inferred_candidates_hint(
        new_inferred_candidate_count(before, &after),
    ))
}

/// What the contributors beat of a scrape came to (DDD-14 / UC-1 / UC-2).
enum ContributorsOutcome {
    /// A user target — repos alone have contributors.
    NotARepo,
    /// `--contributors 0`: GitHub was not asked.
    NoneRequested,
    /// GitHub will not list them (too large / empty): a named notice.
    Unavailable(String),
    /// The snapshot was recorded; `shared` is the cross-repo overlap.
    Recorded {
        selection: ContributorSelection,
        shared: Vec<SharedContributor>,
    },
}

/// The contributors block below the candidate list. Pure.
fn render_contributors_outcome(outcome: &ContributorsOutcome) -> String {
    match outcome {
        ContributorsOutcome::NotARepo => String::new(),
        ContributorsOutcome::NoneRequested => render_no_contributors_requested(),
        ContributorsOutcome::Unavailable(reason) => render_contributors_not_recorded(reason),
        ContributorsOutcome::Recorded { selection, shared } => {
            render_contributors_block(selection) + &render_shared_contributors(shared)
        }
    }
}

/// Run the `--sign N[,N...]` batch over the repo candidates through the
/// SHARED verb-neutral sign batch (DDD-12 — the single sign path both
/// `scrape github --sign` and `infer people --sign` use). Each candidate's
/// `derived-from` line stays DISPLAY-ONLY (WD-62 / I-SCR-7) and it carries no
/// references, so the signed CID is byte-identical to a hand-authored claim's.
fn sign_selected_candidates(
    wiring: &Wiring,
    candidates: &[CandidateClaim],
    raw_selection: &str,
    candidate_list_out: &str,
) -> Result<ScrapeGithubOutcome> {
    let signables: Vec<SignableCandidate> = candidates.iter().map(signable_from).collect();
    let exit_code =
        sign_batch::sign_selected(wiring, &signables, raw_selection, candidate_list_out)?;
    Ok(ScrapeGithubOutcome {
        exit_code,
        stdout: String::new(),
    })
}

/// Pre-fill the shared compose editor from one repo candidate.
fn signable_from(candidate: &CandidateClaim) -> SignableCandidate {
    SignableCandidate {
        subject: candidate.subject.clone(),
        predicate: candidate.predicate.clone(),
        object: candidate.object.clone(),
        evidence: candidate.evidence.clone(),
        confidence: candidate.confidence,
        references: Vec::new(),
        derived_from: render_derived_from_line(candidate),
    }
}

/// Render the DISPLAY-ONLY `derived-from` provenance line (WD-62 / I-SCR-7).
/// Names the scraper tool + the candidate's source signal(s). This line
/// appears in the compose/publish output but is NEVER part of the signed
/// payload — the signed claim is byte-identical to a hand-authored one, so the
/// CID is unchanged. Pure function of the candidate.
fn render_derived_from_line(candidate: &CandidateClaim) -> String {
    let signals = candidate
        .source_signals()
        .iter()
        .map(|s| s.value.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    format!("  derived-from: openlore-github-scraper (signal: {signals})\n")
}

/// Record a repo's selected contributors as append-only links in ONE
/// transaction, observed at the clock port's `now` (DDD-5 / DDD-14).
fn record_contributors(
    wiring: &Wiring,
    repo_subject: &str,
    selection: &ContributorSelection,
) -> Result<()> {
    wiring
        .contribution_links
        .record_snapshot(repo_subject, wiring.clock.now_utc(), &selection.people)
        .map(|_| ())
        .map_err(|e| anyhow!("recording contributors of {repo_subject}: {e}"))
}

/// The recorded people of this repo who are also linked to OTHER repos the
/// user scraped (US-CPI-001 AC4): reads every recorded link through the port
/// and lets the PURE overlap exclude this repo (case-folded keys, DDD-5).
fn contributors_shared_with_other_repos(
    wiring: &Wiring,
    repo_subject: &str,
    selection: &ContributorSelection,
) -> Result<Vec<SharedContributor>> {
    let recorded_links = wiring
        .contribution_links
        .list_links(&LinkFilter::All)
        .map_err(|e| anyhow!("reading recorded contributors: {e}"))?;
    Ok(shared_contributors(
        repo_subject,
        &selection.people,
        &recorded_links,
    ))
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
