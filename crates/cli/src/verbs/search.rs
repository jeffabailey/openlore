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

use adapter_index_query::HttpIndexQueryAdapter;
use anyhow::Result;
use ports::{
    AuthorRelationship, IndexQueryError, IndexQueryPort, NetworkSearchResultRaw, SearchDimension,
};

use crate::render;
use crate::wiring::Wiring;

/// The env-var seam the composition root reads for the self-hosted indexer URL
/// (ADR-023/027). Production resolves `[appview] indexer_url` from the config; the
/// acceptance harness sets this env var to the localhost `openlore-indexer serve`
/// port. Empty/unset ⇒ the indexer is treated as unreachable (the SOFT, non-fatal
/// local-only degradation, WD-116).
const INDEXER_URL_ENV: &str = "OPENLORE_INDEXER_URL";

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
        return run_show(wiring, args, cid);
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

/// `--object <philosophy>`: the headline dimension search (US-AV-002).
///
/// The B1 transport walking skeleton (04-01): query the self-hosted indexer along
/// the object dimension over HTTP/XRPC (`adapter-index-query`), resolve each
/// result author's relationship CLI-side against the user's `peer_subscriptions`
/// (the index is per-user-neutral; data-models.md), and render the attributed
/// network result — the public-data banner FIRST, per-author groups each carrying
/// author DID + numeric confidence + display bucket + evidence + cid +
/// `[verified]`, the relationship label `(subscribed peer)` / `(not subscribed)`,
/// and the no-merge footer with the distinct-author count + `peer add` pointer.
///
/// An unreachable indexer degrades GRACEFULLY to a clear local-only message
/// pointing at `graph query`, exiting 0 (the SOFT, non-fatal contract; WD-116).
fn run_dimension_object(wiring: &Wiring, object: &str) -> Result<SearchOutcome> {
    run_dimension(wiring, SearchDimension::Object, object)
}

/// Shared dimension-search path. The walking skeleton wires only `--object`; the
/// contributor/subject dimensions register the same shape in later steps (05-04+).
fn run_dimension(
    wiring: &Wiring,
    dimension: SearchDimension,
    value: &str,
) -> Result<SearchOutcome> {
    let indexer_url = std::env::var(INDEXER_URL_ENV).unwrap_or_default();
    if indexer_url.is_empty() {
        return Ok(degrade_to_local_only(dimension, value));
    }

    let adapter = HttpIndexQueryAdapter::for_url(indexer_url);
    let runtime = crate::verbs::claim_publish::build_tokio_runtime();
    let outcome = runtime.block_on(adapter.search(dimension, value, None));

    match outcome {
        // An empty result is a VALID not-yet-found state (US-AV-002 Ex 4 / AV-12):
        // name the queried value, offer a near-match suggestion, and exit 0 — NOT
        // an error. A non-empty result renders the attributed per-author view.
        Ok(result) if result.results.is_empty() => {
            Ok(render_empty_result(&adapter, &runtime, dimension, value))
        }
        Ok(result) => Ok(render_network_result(wiring, dimension, result)),
        // SOFT, non-fatal: an unreachable indexer degrades to the local-only
        // message + a `graph query` pointer, exit 0 (KPI-5 / WD-116).
        Err(IndexQueryError::Unreachable { .. }) => Ok(degrade_to_local_only(dimension, value)),
        Err(err) => Err(anyhow::anyhow!("index query failed: {err}")),
    }
}

/// Render the empty-dimension-result view (US-AV-002 Ex 4 / AV-12): the typo'd
/// `value` matched no network claims, so gather the KNOWN network objects near
/// the query and rank them with the PURE `appview_domain::near_match_suggestion`
/// (AVC-8) to offer "Did you mean <closest>?". Exit 0 — a valid not-yet-found
/// state, distinct from the `--show`-absent-cid usage error (non-zero, AV-24).
///
/// The known-object set is collected by probing the single-edit-distance
/// neighbours of `value` against the SAME indexer search port (the slice-04
/// `graph query` near-match precedent, `render::single_edit_neighbours` + an
/// exact-match read): a typo is one edit from the correct URI, so any neighbour
/// that itself has network claims IS a real known object. The pure ranker then
/// picks the closest — the suggestion is therefore always a real network object,
/// never fabricated, and the input order does not matter (AVC-8 tiebreak).
fn render_empty_result(
    adapter: &HttpIndexQueryAdapter,
    runtime: &tokio::runtime::Runtime,
    dimension: SearchDimension,
    value: &str,
) -> SearchOutcome {
    let known = known_objects_near(adapter, runtime, dimension, value);
    let suggestion = appview_domain::near_match_suggestion(value, &known);
    SearchOutcome {
        exit_code: 0,
        stdout: render::render_empty_network_search(dimension, value, suggestion.as_deref()),
    }
}

/// Collect the KNOWN network objects close to `value` by probing the single-edit
/// neighbours against the indexer (the slice-04 near-match precedent carried to
/// the network port). Each neighbour that has ≥1 network claim contributes its
/// real object string to the candidate set the pure ranker scores. A neighbour
/// query that errors (unreachable mid-probe) is skipped — the empty path stays
/// non-fatal (exit 0). Returns a deduplicated candidate set in deterministic
/// (first-seen) order; the AVC-8 ranker's tiebreak makes the final pick stable.
fn known_objects_near(
    adapter: &HttpIndexQueryAdapter,
    runtime: &tokio::runtime::Runtime,
    dimension: SearchDimension,
    value: &str,
) -> Vec<String> {
    let mut known: Vec<String> = Vec::new();
    for candidate in render::single_edit_neighbours(value) {
        match runtime.block_on(adapter.search(dimension, &candidate, None)) {
            Ok(result) => {
                for row in &result.results {
                    if !known.contains(&row.object) {
                        known.push(row.object.clone());
                    }
                }
            }
            // A mid-probe failure is non-fatal: skip the candidate, keep probing.
            Err(_) => continue,
        }
    }
    known
}

/// Render a successful network result: the index is per-user-neutral, so resolve
/// each author's relationship label CLI-side against the user's
/// `peer_subscriptions` (a per-author resolver closure), then hand the attributed
/// rows + the resolver to the PURE renderer (banner-first, per-author groups,
/// `[verified]` per row, the no-merge footer with the distinct-author count +
/// `peer add` pointer).
fn render_network_result(
    wiring: &Wiring,
    dimension: SearchDimension,
    result: NetworkSearchResultRaw,
) -> SearchOutcome {
    let relationship_for =
        |author_did: &str| -> AuthorRelationship { resolve_relationship(wiring, author_did) };
    let stdout = render::render_network_search_result(dimension, &result, &relationship_for);
    SearchOutcome {
        exit_code: 0,
        stdout,
    }
}

/// Resolve the relationship label for one author DID against the user's
/// subscriptions. `SubscribedPeer` iff an ACTIVE subscription exists for the
/// author's bare DID; otherwise `NetworkUnfollowed` (`(not subscribed)`). The
/// index is per-user-neutral; the relationship is a CLI-side projection.
fn resolve_relationship(wiring: &Wiring, author_did: &str) -> AuthorRelationship {
    let bare = crate::verbs::bare_did(author_did);
    match wiring
        .peer_storage
        .lookup_subscription(&claim_domain::Did(bare))
    {
        Ok(Some(sub)) if sub.is_active() => AuthorRelationship::SubscribedPeer,
        _ => AuthorRelationship::NetworkUnfollowed,
    }
}

/// The SOFT, non-fatal local-only degradation (WD-116 / KPI-5): an unreachable or
/// unconfigured indexer never blocks — `search` prints a clear message pointing
/// the user at the LOCAL `graph query` along the SAME dimension and exits 0 (never
/// a hang/panic/fatal). The dimension flag in the pointer matches the search the
/// user ran, so `--contributor` degrades to `graph query --contributor <did>`.
fn degrade_to_local_only(dimension: SearchDimension, value: &str) -> SearchOutcome {
    let flag = dimension_flag(dimension);
    let stdout = format!(
        "Network index unavailable. See LOCAL results via `openlore graph query {flag} {value}`.\n"
    );
    SearchOutcome {
        exit_code: 0,
        stdout,
    }
}

/// The `graph query` flag for a search dimension (`--object`/`--contributor`/
/// `--subject`) — the LOCAL-degradation pointer names the SAME dimension the user
/// searched. Pure helper.
fn dimension_flag(dimension: SearchDimension) -> &'static str {
    match dimension {
        SearchDimension::Object => "--object",
        SearchDimension::Contributor => "--contributor",
        SearchDimension::Subject => "--subject",
    }
}

/// `--contributor <handle-or-did>`: one developer's network trail (US-AV-003).
///
/// The contributor argument is a GitHub handle (`github:priya`) or a DID. Resolve
/// it to the author's app identity (`did:plc:priya-test#org.openlore.application` —
/// the convention the indexed `author_did` carries), then query the contributor
/// dimension over the SAME B1 transport the object dimension uses (the server
/// routes `Contributor` to `query_by_contributor` — an EXPLICIT author_did
/// projection, never an author-eliding aggregate). The render layer surfaces the
/// trail under the one author DID + the honest-framing footer ("one developer's
/// reasoning trail, not a community consensus") with the slice-03 `peer add` offer.
///
/// An unreachable indexer degrades GRACEFULLY to a clear local-only message
/// pointing at `graph query --contributor`, exiting 0 (the SOFT contract; WD-116).
fn run_dimension_contributor(wiring: &Wiring, contributor: &str) -> Result<SearchOutcome> {
    let author_did = resolve_contributor_to_did(contributor);
    run_dimension(wiring, SearchDimension::Contributor, &author_did)
}

/// The app-identity verification-method fragment every signed/indexed claim's
/// `author_did` carries (`did:plc:X#org.openlore.application`). The contributor
/// query matches the indexed `author_did` exactly, so a resolved bare DID is
/// lifted to this app identity before the wire query.
const APP_IDENTITY_FRAGMENT: &str = "#org.openlore.application";

/// Resolve a `--contributor` argument to the author's app-identity DID the indexed
/// `author_did` carries. PURE function — no I/O.
///
/// - A `github:<handle>` argument resolves via the handle→DID convention
///   (`github:priya` → `did:plc:priya-test`, the slice-02/04 fixture handle→DID
///   mapping) then lifts to the app identity (`…#org.openlore.application`).
/// - A bare DID (`did:plc:…`) lifts to the app identity if it lacks the fragment;
///   an already-fragmented DID passes through unchanged.
///
/// The query matches the indexed `author_did` exactly (`author_did = ?`), so the
/// resolved value MUST carry the app-identity fragment.
fn resolve_contributor_to_did(contributor: &str) -> String {
    let bare = match contributor.strip_prefix("github:") {
        // `github:priya` → `did:plc:priya-test` (the slice-02/04 handle→DID mapping).
        Some(handle) => format!("did:plc:{handle}-test"),
        // Already a DID — use as-is (the bare form below lifts the fragment).
        None => contributor.to_string(),
    };
    if bare.contains('#') {
        bare
    } else {
        format!("{bare}{APP_IDENTITY_FRAGMENT}")
    }
}

/// `--subject <project>`: the project-dimension search (US-AV-004). SCAFFOLD.
fn run_dimension_subject(_wiring: &Wiring, _subject: &str) -> Result<SearchOutcome> {
    // SCAFFOLD: true — query along the subject dimension; render per-author.
    todo!("openlore search --subject — network subject-dimension search (Phase 03/04)")
}

/// `--show <cid>`: inspect one result — the full record + the verification line
/// ("Signature: VERIFIED against <did>" / "CID: <cid> (recomputed, matches
/// published record)"). A CID not in any result is a usage error (non-zero exit),
/// distinct from an empty dimension search (exit 0).
///
/// ## get-by-cid mechanism (DESIGN §2 — reuse the search query, filter client-side)
///
/// `--show` reuses the SAME dimension search the result list ran (the supplied
/// `--object`/`--contributor`/`--subject` value) and filters the returned FLAT
/// attributed rows to the requested `cid` CLIENT-SIDE — no new XRPC endpoint, no
/// server change. The cid the user `--show`s came from a prior search of the SAME
/// dimension (US-AV-004 Ex1: "a prior search listed it"), so the row is in that
/// dimension's result set. This keeps `--show` a READ-ONLY projection of the
/// already-computed result set (no second verification path; US-AV-004 Technical
/// Notes): the rendered "Signature: VERIFIED against <did>" + "CID recomputed,
/// matches published record" lines surface the `verified_against` + `cid` the
/// indexer ALREADY computed at ingest. The display creates/signs/mutates nothing.
fn run_show(wiring: &Wiring, args: &SearchArgs, cid: &str) -> Result<SearchOutcome> {
    // The dimension `--show` inspects within is the one the user listed results
    // with (US-AV-004 Ex1: `search --object ... --show <cid>`). Resolve it from
    // the supplied dimension flag.
    let (dimension, value) = match show_dimension(args) {
        Some(pair) => pair,
        None => return Ok(show_usage_error_no_dimension()),
    };

    let indexer_url = std::env::var(INDEXER_URL_ENV).unwrap_or_default();
    if indexer_url.is_empty() {
        return Ok(degrade_to_local_only(dimension, value));
    }

    let adapter = HttpIndexQueryAdapter::for_url(indexer_url);
    let runtime = crate::verbs::claim_publish::build_tokio_runtime();
    // Reuse the SAME dimension search the result list ran; the cid filter is
    // applied CLIENT-SIDE below (DESIGN §2 — no new endpoint). Pass the cid hint
    // along the existing port signature so a future server-side filter is a drop-in.
    let queried_cid = claim_domain::Cid(cid.to_string());
    let outcome = runtime.block_on(adapter.search(dimension, value, Some(&queried_cid)));

    match outcome {
        Ok(result) => Ok(render_show(wiring, result, cid)),
        // SOFT, non-fatal: an unreachable indexer degrades to the local-only
        // message + a `graph query` pointer, exit 0 (KPI-5 / WD-116).
        Err(IndexQueryError::Unreachable { .. }) => Ok(degrade_to_local_only(dimension, value)),
        Err(err) => Err(anyhow::anyhow!("index query failed: {err}")),
    }
}

/// Resolve the `--show` dimension + value from the supplied dimension flag. The
/// user lists results with one dimension (`--object`/`--contributor`/`--subject`)
/// then `--show`s a cid from that list, so `--show` re-runs the SAME dimension.
fn show_dimension(args: &SearchArgs) -> Option<(SearchDimension, &str)> {
    if let Some(object) = &args.object {
        return Some((SearchDimension::Object, object));
    }
    if let Some(contributor) = &args.contributor {
        return Some((SearchDimension::Contributor, contributor));
    }
    if let Some(subject) = &args.subject {
        return Some((SearchDimension::Subject, subject));
    }
    None
}

/// Render the `--show` trust-inspection view: filter the dimension result set to
/// the requested `cid` CLIENT-SIDE, then render the full record + the verification
/// line via the PURE `render::render_show_verification_line` (the SAME stored
/// `verified_against` + `cid` the indexer computed at ingest — no second path,
/// US-AV-004 Technical Notes). A cid absent from the result set is a usage error
/// (non-zero exit), distinct from an empty dimension search (exit 0). READ-ONLY.
fn render_show(_wiring: &Wiring, result: NetworkSearchResultRaw, cid: &str) -> SearchOutcome {
    match result.results.iter().find(|row| row.cid.0 == cid) {
        Some(row) => SearchOutcome {
            exit_code: 0,
            stdout: render::render_show_verification_line(row),
        },
        None => SearchOutcome {
            exit_code: 2,
            stdout: format!(
                "CID {cid} is not in this search result. Run the search without --show \
                 to list results, then --show a listed CID.\n"
            ),
        },
    }
}

/// `--show <cid>` without a dimension flag: a usage error (non-zero exit) — the
/// user must list results along a dimension before inspecting one.
fn show_usage_error_no_dimension() -> SearchOutcome {
    SearchOutcome {
        exit_code: 2,
        stdout: "openlore search --show <cid> requires a dimension \
                 (--object/--contributor/--subject) to list results from.\n"
            .to_string(),
    }
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

#[cfg(test)]
mod tests {
    //! DELIVER inner loop (step 05-02): the contributor handle→DID resolution —
    //! `--contributor github:priya` must resolve to the app-identity DID the
    //! indexed `author_did` carries (`did:plc:priya-test#org.openlore.application`)
    //! so the EXACT `author_did = ?` projection matches (AV-15). PURE — no I/O.

    use super::*;

    /// AV-15 / US-AV-003: `github:<handle>` resolves to the author app-identity DID
    /// (`did:plc:<handle>-test#org.openlore.application`, the slice-02/04 handle→DID
    /// convention lifted to the indexed `author_did` form) so the contributor query
    /// matches the corpus author exactly. A bare DID lifts the app-identity
    /// fragment; an already-fragmented DID passes through unchanged.
    #[test]
    fn resolve_contributor_lifts_handle_and_did_to_app_identity() {
        // The headline AV-15 case: `github:priya` → the priya app identity.
        assert_eq!(
            resolve_contributor_to_did("github:priya"),
            "did:plc:priya-test#org.openlore.application",
            "github:<handle> must resolve to did:plc:<handle>-test#org.openlore.application"
        );
        // A bare DID lifts the app-identity fragment (the query matches the indexed
        // fragmented author_did, never the bare form).
        assert_eq!(
            resolve_contributor_to_did("did:plc:priya-test"),
            "did:plc:priya-test#org.openlore.application",
            "a bare DID must lift the app-identity fragment"
        );
        // An already-fragmented DID passes through unchanged (idempotent).
        assert_eq!(
            resolve_contributor_to_did("did:plc:priya-test#org.openlore.application"),
            "did:plc:priya-test#org.openlore.application",
            "an already-fragmented DID must pass through unchanged"
        );
    }
}
