//! `openlore search` — the slice-05 network-discovery verb (ADR-027).
//!
//! A NEW top-level verb (WD-113): `graph query` stays unambiguously LOCAL; this
//! is the only NETWORK verb. It queries the self-hosted indexer over HTTP/XRPC
//! (`org.openlore.appview.searchClaims`) along one of three dimensions
//! (`--object` / `--contributor` / `--subject`), or inspects one result
//! (`--show <cid>`), or emits a shareable query-encoding link (`--share`).
//!
//! ## Graceful degradation is the design (WD-116 / KPI-5)
//!
//! The indexer is wired + SOFT-probed at CLI startup (an unreachable indexer is
//! informational, NOT a startup refusal — it MUST NOT block `claim add`). When
//! the indexer is down, `search` degrades to a clear local-only message and exits
//! 0 — never a hang, panic, or fatal error.
//!
//! ## Anti-merging across the transport (I-AV-2)
//!
//! The wire carries FLAT attributed rows (every row's `author_did` non-Option);
//! the verb re-composes the per-author view via the pure `appview-domain` core
//! and the `render` layer surfaces per-author groups + the `[verified]` marker +
//! relationship labels + the no-merge footer. No merged/consensus row exists.
//!
//! ## Discovery → federation funnel (WD-110 / I-AV-7)
//!
//! For an author the user does NOT follow (`NetworkUnfollowed`), the render layer
//! emits a render-only `peer add` follow affordance — it REUSES the slice-03
//! command verbatim; there is no auto-follow, no parallel subscription state.
//!
//! Bootstrap SCAFFOLD (step 01-04): the verb + its `SearchArgs`/`SearchOutcome`
//! + the dimension dispatch are established; the bodies are `todo!()` (the live
//! XRPC query + the renders + the graceful-degradation path land in Phase 03/04,
//! driven by the AV-* acceptance scenarios registering at 01-05).
//
// SCAFFOLD: true

#![allow(dead_code)] // scaffold; the live search dispatch lands in Phase 03/04

use anyhow::Result;

use crate::wiring::Wiring;

/// Parsed `openlore search` arguments (clap-parsed in `lib.rs` per ADR-027).
///
/// `--object` / `--contributor` / `--subject` are the three search dimensions
/// (mutually-exclusive entry points; the verb adjudicates which was supplied).
/// `--show <cid>` inspects one result (full record + the verification line).
/// `--share` emits a query-encoding link instead of running the search.
#[derive(Debug, Clone, Default)]
pub struct SearchArgs {
    /// Query by OBJECT (philosophy URI) — the headline dimension (US-AV-002).
    pub object: Option<String>,
    /// Query by CONTRIBUTOR (DID) — one developer's network trail (US-AV-003).
    pub contributor: Option<String>,
    /// Query by SUBJECT (project URI) (US-AV-004).
    pub subject: Option<String>,
    /// `--show <cid>`: inspect one result — full record + the verification line.
    pub show: Option<String>,
    /// `--share`: emit a stable query-encoding link instead of running the query
    /// (WD-110 / I-AV-8 — encodes the QUERY, never a snapshot).
    pub share: bool,
}

/// The captured search output + exit code (mirrors the other verbs' outcomes).
pub struct SearchOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run `openlore search`. Dispatches on the supplied dimension / mode:
///
/// - `--share` → emit the query-encoding link (no network call; I-AV-8).
/// - `--show <cid>` → inspect one result + render the `--show` verification line.
/// - a dimension (`--object`/`--contributor`/`--subject`) → query the indexer,
///   re-compose per-author, render the attributed network result; on an
///   unreachable indexer degrade gracefully to the local-only message (exit 0).
///
/// Bootstrap SCAFFOLD (step 01-04): the dispatch SHAPE is established; the bodies
/// are `todo!()`. The verb reads the SOFT-probed `HttpIndexQueryAdapter` the CLI
/// composition root wired (WD-116).
pub fn run(wiring: &Wiring, args: &SearchArgs) -> Result<SearchOutcome> {
    if args.share {
        return run_share(args);
    }
    if let Some(cid) = &args.show {
        return run_show(wiring, cid);
    }
    if let Some(object) = &args.object {
        return run_dimension_object(wiring, object);
    }
    if let Some(contributor) = &args.contributor {
        return run_dimension_contributor(wiring, contributor);
    }
    if let Some(subject) = &args.subject {
        return run_dimension_subject(wiring, subject);
    }
    run_no_dimension(args)
}

/// `--object <philosophy>`: the headline dimension search (US-AV-002). SCAFFOLD.
fn run_dimension_object(_wiring: &Wiring, _object: &str) -> Result<SearchOutcome> {
    // SCAFFOLD: true — query the indexer along the object dimension, re-compose
    // per-author via appview-domain, render the attributed network result with
    // the public-data banner + relationship labels + the `peer add` follow
    // affordance + the no-merge footer; degrade gracefully if unreachable.
    todo!("openlore search --object — network object-dimension search (Phase 03/04, ADR-027)")
}

/// `--contributor <did>`: one developer's network trail (US-AV-003). SCAFFOLD.
fn run_dimension_contributor(_wiring: &Wiring, _contributor: &str) -> Result<SearchOutcome> {
    // SCAFFOLD: true — query along the contributor dimension; render the trail +
    // the "one developer's reasoning trail, not a community consensus" framing.
    todo!("openlore search --contributor — network contributor-dimension search (Phase 03/04)")
}

/// `--subject <project>`: the project-dimension search (US-AV-004). SCAFFOLD.
fn run_dimension_subject(_wiring: &Wiring, _subject: &str) -> Result<SearchOutcome> {
    // SCAFFOLD: true — query along the subject dimension; render per-author.
    todo!("openlore search --subject — network subject-dimension search (Phase 03/04)")
}

/// `--show <cid>`: inspect one result — the full record + the verification line
/// ("Signature: VERIFIED against <did>" / "CID recomputed, matches published
/// record"). A CID not in any result is a usage error (non-zero exit), distinct
/// from an empty dimension search (exit 0). SCAFFOLD.
fn run_show(_wiring: &Wiring, _cid: &str) -> Result<SearchOutcome> {
    // SCAFFOLD: true — fetch the one result, render the full record + the
    // `--show` verification line (the SAME pure-core verification result; no
    // second path). Lands in Phase 03/04.
    todo!("openlore search --show — inspect one result + verification line (Phase 03/04)")
}

/// `--share`: emit a stable query-encoding link (WD-110 / I-AV-8) — encodes only
/// the dimension + value, never a result snapshot. SCAFFOLD.
fn run_share(_args: &SearchArgs) -> Result<SearchOutcome> {
    // SCAFFOLD: true — emit `openlore://search?<dimension>=<value>` via the
    // pure render::render_share_link; no network call. Lands in Phase 03/04.
    todo!("openlore search --share — emit a query-encoding link (Phase 03/04, I-AV-8)")
}

/// No dimension / mode supplied: a usage error pointing at the dimension flags.
/// SCAFFOLD — the exact usage message lands with the verb behavior.
fn run_no_dimension(_args: &SearchArgs) -> Result<SearchOutcome> {
    // SCAFFOLD: true — render the usage error naming
    // --object/--contributor/--subject. Lands in Phase 03/04.
    todo!("openlore search — usage error (no dimension supplied) (Phase 03/04)")
}
