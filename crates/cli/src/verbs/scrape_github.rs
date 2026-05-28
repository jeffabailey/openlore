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

/// Run the `scrape github` verb.
///
/// SCAFFOLD: true (slice-02)
///
/// Bodied `todo!()` at step 01-04 — the live harvest -> derive -> render ->
/// [--sign] pipeline lands per the SCR-* acceptance scenarios in Phase 03/05.
/// The dispatch routing + the `GithubPort` wiring + the probe-gauntlet slot
/// are in place at this step; only this handler body is deferred.
pub fn run(_wiring: &Wiring, _args: &ScrapeGithubArgs) -> Result<ScrapeGithubOutcome> {
    // SCAFFOLD: true (slice-02)
    todo!("scrape github pipeline lands per SCR-* scenarios in Phase 03/05")
}
