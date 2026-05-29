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
    /// A positional `openlore://search?<dim>=<value>` link to OPEN (the CLI
    /// re-run resolver, Q-DELIVER-AV-3 / US-AV-006 Ex2). When supplied, the verb
    /// PARSES the link (the inverse of the 05-12 `--share` emitter grammar),
    /// maps the key to a [`SearchDimension`], then RE-RUNS the SAME dimension
    /// search path against the CURRENT index (the link encoded the QUERY,
    /// deterministic per AVC-3b — NOT a snapshot, I-AV-8). Web AppView is OUT of
    /// scope (OD-AV-6); this is the CLI re-run only.
    pub link: Option<String>,
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
    if let Some(link) = &args.link {
        return run_resolve_link(wiring, link);
    }
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
    // The OBJECT dimension queries + displays the SAME value, and an empty result
    // probes the index for a near-match suggestion (a typo'd philosophy URI is one
    // edit from the correct one — US-AV-002 Ex 4 / AV-12).
    run_dimension(wiring, SearchDimension::Object, object, object, EmptyPolicy::SuggestNearMatch)
}

/// How the empty-dimension-result branch behaves for a given dimension.
///
/// - `SuggestNearMatch` (OBJECT): the empty value is likely a TYPO one edit from a
///   known object, so probe the index for a near-match and offer "Did you mean
///   <near>?" (US-AV-002 Ex 4 / AV-12).
/// - `NoSuggestion` (CONTRIBUTOR/SUBJECT): an absent contributor (or subject) is
///   not a typo — they simply publish no OpenLore claims (or are not yet ingested);
///   there is nothing to suggest, so the empty message names the queried value with
///   NO suggestion (US-AV-003 Ex 3 / AV-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyPolicy {
    SuggestNearMatch,
    NoSuggestion,
}

/// Shared dimension-search path. The `query_value` is what the wire query matches
/// against (for the contributor dimension this is the resolved app-identity DID);
/// the `display_value` is what the user typed and is surfaced in the empty/degraded
/// messages (for the contributor dimension this is the original handle, so the
/// empty message reads "for contributor github:nobody-here", AV-17). The
/// `empty_policy` selects whether an empty result probes for a near-match
/// suggestion (OBJECT) or names the value with no suggestion (CONTRIBUTOR/SUBJECT).
fn run_dimension(
    wiring: &Wiring,
    dimension: SearchDimension,
    query_value: &str,
    display_value: &str,
    empty_policy: EmptyPolicy,
) -> Result<SearchOutcome> {
    let indexer_url = std::env::var(INDEXER_URL_ENV).unwrap_or_default();
    if indexer_url.is_empty() {
        return Ok(degrade_to_local_only(dimension, display_value));
    }

    let adapter = HttpIndexQueryAdapter::for_url(indexer_url);
    let runtime = crate::verbs::claim_publish::build_tokio_runtime();
    let outcome = runtime.block_on(adapter.search(dimension, query_value, None));

    match outcome {
        // An empty result is a VALID not-yet-found state (US-AV-002 Ex 4 / AV-12;
        // US-AV-003 Ex 3 / AV-17): name the queried DISPLAY value, optionally offer
        // a near-match suggestion (per `empty_policy`), and exit 0 — NOT an error. A
        // non-empty result renders the attributed per-author view.
        Ok(result) if result.results.is_empty() => Ok(render_empty_result(
            &adapter,
            &runtime,
            dimension,
            query_value,
            display_value,
            empty_policy,
        )),
        Ok(result) => Ok(render_network_result(wiring, dimension, result)),
        // SOFT, non-fatal: an unreachable indexer degrades to the local-only
        // message + a `graph query` pointer, exit 0 (KPI-5 / WD-116).
        Err(IndexQueryError::Unreachable { .. }) => {
            Ok(degrade_to_local_only(dimension, display_value))
        }
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
    query_value: &str,
    display_value: &str,
    empty_policy: EmptyPolicy,
) -> SearchOutcome {
    // The near-match suggestion is OBJECT-only (a typo'd philosophy URI is one edit
    // from a known object). An absent CONTRIBUTOR/SUBJECT is not a typo, so the
    // empty message names the DISPLAY value with NO suggestion (AV-17). The probe
    // runs against the resolved QUERY value (the index is keyed by it); the message
    // names the DISPLAY value the user typed.
    let suggestion = match empty_policy {
        EmptyPolicy::SuggestNearMatch => {
            let known = known_objects_near(adapter, runtime, dimension, query_value);
            appview_domain::near_match_suggestion(query_value, &known)
        }
        EmptyPolicy::NoSuggestion => None,
    };
    SearchOutcome {
        exit_code: 0,
        stdout: render::render_empty_network_search(
            dimension,
            display_value,
            suggestion.as_deref(),
        ),
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
    // The wire query matches the indexed `author_did` exactly, so query with the
    // RESOLVED app-identity DID; but the empty message names the ORIGINAL handle the
    // user typed (`github:nobody-here`, not the resolved DID — AV-17), and an absent
    // contributor is not a typo so it offers NO near-match suggestion.
    let author_did = resolve_contributor_to_did(contributor);
    run_dimension(
        wiring,
        SearchDimension::Contributor,
        &author_did,
        contributor,
        EmptyPolicy::NoSuggestion,
    )
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

/// `--subject <project>`: the project-dimension search (US-AV-003 Ex2; AV-16).
///
/// The subject argument is a PROJECT URI (`github:bazelbuild/bazel`) matched
/// against the indexed `subject` column EXACTLY — no handle→DID resolution (a
/// subject is the project, not an author). The server routes `Subject` to
/// `query_by_subject` (an EXPLICIT per-row `author_did` projection, never a
/// subject-eliding aggregate), so a project surveyed by N distinct authors yields
/// N attributed rows. The render layer surfaces those rows grouped BY AUTHOR (5
/// distinct author groups), each with philosophy/confidence/cid/`[verified]`, and
/// the dimension-aware footer reuses the OBJECT survey's distinct-author COUNT +
/// no-merge guarantee — there is NO "bazel: the network thinks X" merged/consensus
/// row (the subject-dimension anti-merging render, I-AV-2 / KPI-AV-2).
///
/// An unreachable indexer degrades GRACEFULLY to a clear local-only message
/// pointing at `graph query --subject`, exiting 0 (the SOFT contract; WD-116).
fn run_dimension_subject(wiring: &Wiring, subject: &str) -> Result<SearchOutcome> {
    // The SUBJECT dimension queries + displays the SAME project URI; an empty result
    // probes for a near-match (a typo'd project URI is one edit from a known one),
    // mirroring the OBJECT dimension's AV-12 behavior.
    run_dimension(
        wiring,
        SearchDimension::Subject,
        subject,
        subject,
        EmptyPolicy::SuggestNearMatch,
    )
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
/// the dimension + value, never a result snapshot.
///
/// Render-only (I-AV-8): NO network call. The verb resolves which dimension was
/// supplied + the value to encode, then hands them to the PURE
/// [`render::render_share_link`] which emits `openlore://search?<dimension>=<value>`
/// plus the "encodes the query, not a snapshot" semantics line. The contributor
/// dimension encodes the RESOLVED app-identity-bare DID (AV-29, forward-compat),
/// NOT the typed handle, so opening the link re-runs the SAME query.
fn run_share(args: &SearchArgs) -> Result<SearchOutcome> {
    let (dimension, value) = match share_dimension(args) {
        Some(pair) => pair,
        None => return Ok(share_usage_error_no_dimension()),
    };
    Ok(SearchOutcome {
        exit_code: 0,
        stdout: render::render_share_link(dimension, &value),
    })
}

/// Resolve the `--share` dimension + the value to ENCODE in the link. The user
/// shares one dimension (`--object`/`--contributor`/`--subject`); the contributor
/// dimension encodes the RESOLVED app-identity-bare DID (so opening the link
/// re-runs the SAME `author_did` query, AV-29), while object/subject encode the
/// value verbatim. PURE — no I/O.
fn share_dimension(args: &SearchArgs) -> Option<(SearchDimension, String)> {
    if let Some(object) = &args.object {
        return Some((SearchDimension::Object, object.clone()));
    }
    if let Some(contributor) = &args.contributor {
        // Encode the resolved DID (bare of the app-identity fragment), NOT the
        // handle — `openlore://search?contributor=did:plc:priya-test` (AV-29).
        let resolved = resolve_contributor_to_did(contributor);
        let bare = crate::verbs::bare_did(&resolved);
        return Some((SearchDimension::Contributor, bare));
    }
    if let Some(subject) = &args.subject {
        return Some((SearchDimension::Subject, subject.clone()));
    }
    None
}

/// `--share` without a dimension flag: a usage error (non-zero exit) — there is no
/// query to encode. Mirrors [`show_usage_error_no_dimension`].
fn share_usage_error_no_dimension() -> SearchOutcome {
    SearchOutcome {
        exit_code: 2,
        stdout: "openlore search --share requires a dimension \
                 (--object/--contributor/--subject) to encode into the link.\n"
            .to_string(),
    }
}

/// The CLI re-run resolver (Q-DELIVER-AV-3 / US-AV-006 Ex2 / AV-27): open a
/// shared `openlore://search?<dim>=<value>` link by RE-RUNNING the encoded query
/// against the CURRENT index. The link encoded the QUERY (deterministic per
/// AVC-3b, 02-05), NOT a result snapshot — so opening it re-composes the
/// per-author-attributed verified rows from scratch, preserving anti-merging
/// across the share boundary (I-AV-8 / KPI-AV-2). Web AppView is OUT of scope
/// (OD-AV-6); this is the CLI re-run only.
///
/// The resolver PARSES the link (the inverse of the 05-12 `render_share_link`
/// emitter grammar) into a [`SearchDimension`] + the encoded value, then drives
/// the SAME dimension search path the original query ran:
///
/// - `object=<philosophy>`  -> [`run_dimension_object`] (the OBJECT survey, with
///   the AV-12 near-match empty policy — re-running re-derives the same survey).
/// - `subject=<project>`    -> [`run_dimension_subject`] (the SUBJECT survey).
/// - `contributor=<did>`    -> the CONTRIBUTOR trail. The link encodes the
///   RESOLVED app-identity-bare DID (the 05-12 emitter encoded the resolved DID,
///   AV-29), so opening it re-runs the SAME `author_did` query verbatim — the
///   value is passed straight through (no second handle->DID resolution).
///
/// A malformed link (missing the `openlore://search?` prefix, missing the
/// `=`, an unknown dimension key, or an empty value) is a usage error
/// (non-zero exit) — there is no query to re-run.
fn run_resolve_link(wiring: &Wiring, link: &str) -> Result<SearchOutcome> {
    let (dimension, value) = match parse_share_link(link) {
        Some(parsed) => parsed,
        None => return Ok(resolve_link_usage_error(link)),
    };
    // Re-run the SAME dimension search path the original `--share` query encoded.
    // The contributor dimension's link already carries the RESOLVED DID (the
    // 05-12 emitter encoded `bare_did(resolve_contributor_to_did(..))`, AV-29), so
    // re-running matches the indexed `author_did` exactly — no second resolution.
    match dimension {
        SearchDimension::Object => run_dimension_object(wiring, &value),
        SearchDimension::Subject => run_dimension_subject(wiring, &value),
        SearchDimension::Contributor => run_dimension_contributor(wiring, &value),
    }
}

/// Parse an `openlore://search?<dimension>=<value>` link into its
/// [`SearchDimension`] + encoded value — the INVERSE of the 05-12
/// [`render::render_share_link`] emitter grammar (`share_dimension_key`). PURE
/// function — no I/O.
///
/// Returns `None` for a malformed link (missing the `openlore://search?` prefix,
/// missing the `=` separator, an unknown dimension key, or an empty value/
/// dimension) so the caller can surface a usage error. The grammar carries
/// EXACTLY one `<key>=<value>` query parameter (the link encodes the QUERY, never
/// a `&`-joined snapshot), so a second parameter is also malformed.
fn parse_share_link(link: &str) -> Option<(SearchDimension, String)> {
    let query = link.strip_prefix(SHARE_LINK_PREFIX)?;
    // The link encodes EXACTLY one query parameter — a `&` means an extra
    // (snapshot-shaped) field, which is not a valid query-encoding link.
    if query.contains('&') {
        return None;
    }
    let (key, value) = query.split_once('=')?;
    if value.is_empty() {
        return None;
    }
    let dimension = dimension_from_key(key)?;
    Some((dimension, value.to_string()))
}

/// The `openlore://search?` scheme+authority prefix every `--share` link carries
/// (the head of the 05-12 emitter grammar). The resolver strips it to read the
/// `<dimension>=<value>` query string.
const SHARE_LINK_PREFIX: &str = "openlore://search?";

/// Map a share-link `<dimension>` query KEY back to its [`SearchDimension`] — the
/// inverse of the 05-12 emitter's `share_dimension_key`. Returns `None` for an
/// unknown key (a malformed/forward-incompatible link). PURE helper.
fn dimension_from_key(key: &str) -> Option<SearchDimension> {
    match key {
        "object" => Some(SearchDimension::Object),
        "contributor" => Some(SearchDimension::Contributor),
        "subject" => Some(SearchDimension::Subject),
        _ => None,
    }
}

/// A malformed shared link: a usage error (non-zero exit) — the link is not a
/// valid `openlore://search?<dimension>=<value>` query encoding, so there is no
/// query to re-run. Mirrors the other usage-error shapes.
fn resolve_link_usage_error(link: &str) -> SearchOutcome {
    SearchOutcome {
        exit_code: 2,
        stdout: format!(
            "openlore search: `{link}` is not a valid shareable link. Expected \
             `openlore://search?<object|contributor|subject>=<value>` (run \
             `openlore search --object <philosophy> --share` to emit one).\n"
        ),
    }
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
    use proptest::prelude::*;

    /// The link line `render::render_share_link` emits carries the
    /// `openlore://search?<dim>=<value>` token; extract it for the round-trip
    /// property (the parser consumes the link token, not the whole render block).
    fn link_token(rendered: &str) -> String {
        rendered
            .split_whitespace()
            .find(|tok| tok.starts_with(SHARE_LINK_PREFIX))
            .expect("render_share_link emits an openlore://search? link")
            .to_string()
    }

    /// AV-27 / US-AV-006 Ex2 (PBT — the parser is the INVERSE of the 05-12
    /// emitter): for EVERY dimension + (URL-safe) value, parsing the link
    /// `render::render_share_link` emits ROUND-TRIPS back to the SAME
    /// `(SearchDimension, value)`. The link encoded the QUERY deterministically
    /// (AVC-3b), so the resolver re-derives the exact query the share encoded —
    /// the symmetric property that makes opening a shared link re-run the SAME
    /// search (anti-merging across the share boundary, I-AV-8). PURE — no I/O.
    #[test]
    fn parse_share_link_is_the_inverse_of_the_share_emitter() {
        // The dimension space is the closed three-variant set; the value space is
        // the share-grammar character class (philosophy/project URIs + DIDs —
        // `[a-z0-9.:/_#-]`, never an empty value, never the `&`/`=` separators).
        let dimensions = prop_oneof![
            Just(SearchDimension::Object),
            Just(SearchDimension::Contributor),
            Just(SearchDimension::Subject),
        ];
        let value = "[a-z0-9][a-z0-9.:/_#-]{0,60}";
        proptest!(|(dimension in dimensions, value in value)| {
            let rendered = render::render_share_link(dimension, &value);
            let link = link_token(&rendered);
            let parsed = parse_share_link(&link);
            prop_assert_eq!(
                parsed,
                Some((dimension, value.clone())),
                "parse_share_link must invert render_share_link for {:?}={}",
                dimension,
                value
            );
        });
    }

    /// AV-27 sad paths (Mandate 11 — EXAMPLE-only, enumerated): a malformed link
    /// PARSES to `None` (the caller surfaces a usage error) — a missing prefix, a
    /// missing `=`, an unknown dimension key, an empty value, or a `&`-joined
    /// (snapshot-shaped) extra parameter. The resolver re-runs ONLY a valid
    /// query-encoding link; anything else has no query to re-run.
    #[test]
    fn parse_share_link_rejects_malformed_links() {
        for malformed in [
            // Missing the `openlore://search?` scheme+authority prefix.
            "https://example.com/?object=x",
            "object=org.openlore.philosophy.reproducible-builds",
            // Missing the `=` separator (no value).
            "openlore://search?object",
            // An unknown / forward-incompatible dimension key.
            "openlore://search?philosophy=x",
            // An empty value (a dimension with nothing to query).
            "openlore://search?object=",
            // A `&`-joined extra parameter — a snapshot shape, not a query encoding.
            "openlore://search?object=x&author_did=did:plc:y",
        ] {
            assert_eq!(
                parse_share_link(malformed),
                None,
                "a malformed link must parse to None: {malformed}"
            );
        }
    }

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
