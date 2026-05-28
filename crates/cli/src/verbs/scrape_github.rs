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

use std::io::Write;

use anyhow::{anyhow, Result};
use ports::{CandidateClaim, TargetKind};
use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};

use crate::io::prompt_line;
use crate::render::{render_auth_report, render_candidate_list, render_public_data_banner};
use crate::verbs::claim_add::{build_unsigned_claim, render_compose_preview, ComposedClaim};
use crate::verbs::claim_publish::{
    build_tokio_runtime, publish_signed_claim, render_publish_success,
};
use crate::wiring::Wiring;
use claim_domain::{canonicalize, compute_cid, SignedClaim};

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

    // (6) WITHOUT --sign: derive + render only, ZERO writes (the human-gate;
    // scraper_never_persists_unsigned, I-SCR-1 / WD-49). Return now.
    let Some(raw_selection) = args.sign.as_deref() else {
        return Ok(ScrapeGithubOutcome {
            exit_code: 0,
            stdout: out,
        });
    };

    // (7) --sign N[,N...]: validate the selection BEFORE any compose begins
    // (out-of-range / duplicate rejected up front; SS-4 / SS-9), then walk
    // each selected candidate through its OWN slice-01 compose-sign-publish
    // pipeline — the SINGLE publish path (no parallel publish; WD-66 /
    // I-SCR-6). The candidate-list block already accumulated in `out` is
    // emitted to stdout NOW so the user reviews it before composing.
    let selection = parse_selection(raw_selection, candidates.len()).map_err(|e| anyhow!(e))?;
    print!("{out}");
    std::io::stdout().flush()?;

    // Batch is a SEQUENCE of individual human-gates (US-SCR-005; WD-49 /
    // J-004c) — never a "sign all" bypass. Each selected candidate is carried
    // through its OWN slice-01 compose-sign-publish gesture; between them we
    // surface a running "(k of M signed)" progress line so the human sees the
    // batch advancing one conscious signature at a time.
    let total_selected = selection.len();
    for (signed_so_far, index) in selection.into_iter().enumerate() {
        // After the first candidate is signed, announce progress BEFORE the
        // next candidate's compose preview: "(1 of 3 signed)" precedes the
        // second preview, "(2 of 3 signed)" the third, and so on.
        if signed_so_far > 0 {
            println!("\n({signed_so_far} of {total_selected} signed)");
            std::io::stdout().flush()?;
        }
        // 1-based selection -> 0-based slice access (validated above).
        let candidate = &candidates[index - 1];
        sign_candidate_via_slice01(wiring, candidate)?;
    }

    Ok(ScrapeGithubOutcome {
        exit_code: 0,
        stdout: String::new(),
    })
}

/// Parse + validate the raw `--sign N[,N...]` selection against the derived
/// candidate count. Returns the 1-based indices in input order, or a
/// domain-shaped error naming the offending value(s). Pure — no I/O — so the
/// rejection happens BEFORE any compose preview (SS-4 / SS-9 pre-compose
/// ordering). SS-1 exercises the single-index happy path; the multi-index +
/// duplicate-rejection paths are pinned by SS-7 / SS-9.
fn parse_selection(raw: &str, candidate_count: usize) -> Result<Vec<usize>, String> {
    let mut indices = Vec::new();
    for token in raw.split(',') {
        let token = token.trim();
        let index: usize = token.parse().map_err(|_| {
            format!("invalid --sign selection {token:?}; expected 1-based candidate indices")
        })?;
        if index == 0 || index > candidate_count {
            return Err(format!(
                "candidate {index} does not exist; valid range 1..{candidate_count}"
            ));
        }
        indices.push(index);
    }
    Ok(indices)
}

/// Carry ONE selected candidate through the SAME slice-01 compose-sign-publish
/// pipeline a hand-authored `claim add` uses (WD-66 / I-SCR-6 — the single
/// publish path). The candidate pre-fills the editable compose fields; the
/// human accepts each (Enter) or overrides, then performs the two-prompt sign
/// (Enter) + publish (Y) gesture. The `derived-from` provenance line is
/// DISPLAY-ONLY (WD-62 / I-SCR-7) — it appears in the preview but is NEVER a
/// signed-payload field, so the signed CID is byte-identical to a
/// hand-authored claim's.
fn sign_candidate_via_slice01(wiring: &Wiring, candidate: &CandidateClaim) -> Result<()> {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    // Pre-fill the editable compose fields from the candidate; the human
    // accepts each default (Enter) or overrides it.
    let subject = prompt_field(&mut stdout, &mut stdin, "subject", &candidate.subject)?;
    let predicate = prompt_field(&mut stdout, &mut stdin, "predicate", &candidate.predicate)?;
    let object = prompt_field(&mut stdout, &mut stdin, "object", &candidate.object)?;
    let evidence_default = candidate.evidence.join(", ");
    let evidence_raw = prompt_field(&mut stdout, &mut stdin, "evidence", &evidence_default)?;
    let evidence: Vec<String> = evidence_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let confidence = prompt_confidence(&mut stdout, &mut stdin, candidate.confidence)?;

    // Assemble the composed claim — SAME shape `claim add` builds. composed_at
    // from the clock port for testable determinism.
    let composed = ComposedClaim {
        subject,
        predicate,
        object,
        evidence,
        confidence,
        author_did: wiring.identity.author_did().0.clone(),
        composed_at: wiring.clock.now_utc().to_rfc3339(),
    };

    // Render the slice-01 compose preview (carries the "not as truth" framing
    // + the WD-10 bucket label) PLUS the DISPLAY-ONLY derived-from provenance
    // line naming the candidate's source signal (WD-62 / I-SCR-7).
    let preview = render_compose_preview(&composed);
    stdout.write_all(preview.as_bytes())?;
    stdout.write_all(render_derived_from_line(candidate).as_bytes())?;
    stdout.flush()?;

    // Two-prompt (ADR-003). Enter to sign; EOF/skip before any input is a
    // clean cancel (no side effects).
    let sign_prompt = "\nPress Enter to sign this candidate locally (or Ctrl-C to cancel): ";
    let confirmation = prompt_line(&mut stdout, &mut stdin, sign_prompt)?;
    if confirmation.is_none() {
        return Ok(());
    }

    // Canonicalize -> compute_cid -> sign -> persist. SAME pure-core path
    // `claim add` uses; the derived-from line is NOT folded in (display-only),
    // so the CID is byte-identical to a hand-authored claim's.
    let unsigned = build_unsigned_claim(&composed)?;
    let canonical_bytes =
        canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing candidate claim: {e}"))?;
    let unsigned_cid = compute_cid(&canonical_bytes);
    writeln!(stdout, "Computing claim CID {}", unsigned_cid.0)?;
    stdout.flush()?;

    let signature = wiring
        .identity
        .sign(&unsigned_cid)
        .map_err(|e| anyhow!("signing candidate claim: {e}"))?;
    let signed = SignedClaim {
        unsigned,
        signature,
    };

    // The signed-from-scraper claim is the user's OWN artifact — own `claims`
    // table + own `claims/<cid>.json`.
    wiring.storage.write_signed_claim(&signed).map_err(|e| {
        anyhow!(
            "persisting signed candidate claim {} to local store: {e:#}",
            signed.signature.signed_cid.0
        )
    })?;
    let artifact_path = wiring
        .paths
        .claims_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));
    writeln!(
        stdout,
        "Written to local store: {}",
        artifact_path.display()
    )?;
    stdout.flush()?;

    // Second prompt — publish? Y/y publishes via the SINGLE publish path
    // (WD-66 / I-SCR-6); anything else is a clean decline (local artifact
    // stays, no PDS call).
    let publish_prompt = "\nPublish this claim to your PDS now? (y/N): ";
    let publish_answer = prompt_line(&mut stdout, &mut stdin, publish_prompt)?;
    let confirmed_publish = matches!(
        publish_answer.as_deref().map(str::trim),
        Some("y") | Some("Y") | Some("yes") | Some("YES")
    );
    if confirmed_publish {
        drop(stdout);
        drop(stdin);
        // SINGLE publish code path (WD-66 / I-SCR-6) — the SAME helper
        // `claim add`'s Y branch, `claim counter`, and `claim retract` use.
        match publish_signed_claim(wiring, &signed) {
            Ok(publish_outcome) => {
                // The publish prompt above ends without a newline (the user's
                // y/N answer follows it inline). Start the success block on a
                // fresh line so its first line — `Published claim <cid>.` —
                // is recoverable line-by-line (the SS-1 oracle keys off it).
                println!();
                print!("{}", render_publish_success(&publish_outcome));
            }
            Err(err) => {
                eprint!(
                    "{}",
                    crate::verbs::claim_publish::render_publish_error(&err)
                );
                return Err(anyhow!("publishing signed candidate claim failed"));
            }
        }
    } else {
        // Decline (SS-6): the publish prompt was answered with anything other
        // than Y/yes (n / N / Enter / EOF). This is the local-only outcome —
        // the signed claim STAYS on disk (the sign + write above ran first and
        // is NOT rolled back) and NO PDS call is made (KPI-5 local-first). Hint
        // the standalone publish verb naming the exact CID so the human can
        // federate it later at will (`openlore claim publish <cid>`).
        let cid = &signed.signature.signed_cid.0;
        writeln!(
            stdout,
            "\nNot published. Publish it later with: openlore claim publish {cid}"
        )?;
        stdout.flush()?;
    }

    Ok(())
}

/// Prompt the user to accept (Enter) or override a pre-filled compose field.
/// An empty line keeps the candidate's pre-filled value; a non-empty line
/// replaces it. The slice-02 compose editor is "pre-fill + edit", so the
/// no-edit path signs the proposal byte-for-byte (SS-2).
fn prompt_field<W: Write, R: std::io::Read>(
    writer: &mut W,
    reader: &mut R,
    label: &str,
    default: &str,
) -> Result<String> {
    let prompt = format!("{label} [{default}]: ");
    match prompt_line(writer, reader, &prompt)? {
        Some(line) if !line.trim().is_empty() => Ok(line.trim().to_string()),
        _ => Ok(default.to_string()),
    }
}

/// Prompt for the confidence field, re-prompting on an out-of-range value
/// (SS-5). An empty line keeps the candidate's conservative default; a valid
/// `[0.0, 1.0]` value overrides it. No claim is written until a valid value is
/// entered (the re-prompt loop runs BEFORE the compose preview).
fn prompt_confidence<W: Write, R: std::io::Read>(
    writer: &mut W,
    reader: &mut R,
    default: f64,
) -> Result<f64> {
    loop {
        let prompt = format!("confidence [{default}]: ");
        match prompt_line(writer, reader, &prompt)? {
            Some(line) if !line.trim().is_empty() => match line.trim().parse::<f64>() {
                Ok(value) if (0.0..=1.0).contains(&value) => return Ok(value),
                _ => {
                    writeln!(writer, "confidence must be between 0.0 and 1.0")?;
                    writer.flush()?;
                }
            },
            _ => return Ok(default),
        }
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
