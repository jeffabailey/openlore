//! Slice-05 acceptance — the `openlore search` network-discovery verb
//! (`--object`, `--contributor`, `--subject`, `--show <cid>`, `--share`) per
//! ADR-027.
//!
//! The user-visible discovery surface for J-005 (discover signed claims across
//! the network without knowing whom to follow). Every result row carries its
//! author DID + a `[verified]` marker; results group by author (or by subject
//! under author) with NO faceless "network consensus" row; the public-data
//! banner is shown up front; an unreachable indexer degrades to a clear
//! local-only message without a fatal error; the follow affordance reuses the
//! slice-03 `peer add` verbatim; `--share` emits a stable query-encoding link.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix + DD-AV-1. Every scenario enters through the CLI driving
//! adapter via the REAL `openlore` binary (subprocess via `assert_cmd`),
//! exercises the real `adapter-index-query` HTTP/XRPC client against a REAL
//! `openlore-indexer serve` over LOCALHOST (B1 transport, the production
//! composition root) — bound to an ephemeral port (`:0`, read back) for
//! parallel-safety (DEVOPS open-q 8) — over a real `index.duckdb` seeded with
//! verified attributed claims. Per Mandate 11 the sad paths are EXAMPLE-ONLY,
//! enumerated explicitly, never PBT-generated. The anti-merging composition
//! PROPERTIES live at layer 2 in `appview_core.rs`.
//!
//! Hermetic seam (DD-AV-2): a localhost `openlore-indexer serve` over an
//! `index.duckdb` seeded via the slice-05 ingest harness (FakeIngestSource +
//! the fixture real-z6Mk PLC resolver). The `--show`/footer/banner/relationship-
//! label rendering is asserted against stdout (the CLI driving-port observable),
//! honoring the slice-03/04 anti-merging assertion discipline (count attributed
//! rows; assert NO merged row; exclude footer-template mentions from author/row
//! counts).
//!
//! Covers:
//! - US-AV-002: search by object (philosophy) at network scale, attributed
//! - US-AV-003: search by contributor / subject at network scale
//! - US-AV-004: trust a result — [verified] marker + --show + public-data banner
//! - US-AV-005: subscribe to a discovered author (discovery -> federation funnel)
//! - US-AV-006: share a network search result as a stable query-encoding link
//! - Release gates: network_result_preserves_attribution (KPI-AV-2),
//!   local_first_preserved (KPI-5), public_data_banner_shown (KPI-AV-5),
//!   verified_marker_is_universal (I-AV-1), search_succeeds_with_indexer_localhost (B1)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-AV-002 — search by philosophy (object) at network scale, attributed
// =============================================================================

/// AV-8 (US-AV-002 happy; WALKING SKELETON beat 2 for slice-05): Maria runs
/// `openlore search --object org.openlore.philosophy.reproducible-builds` over a
/// network index populated with verified claims from authors she does NOT
/// follow. The results group by author, each claim under its author DID with
/// numeric confidence + display-only bucket + evidence + cid + a `[verified]`
/// marker; unfollowed authors are labeled `(not subscribed)`; the footer states
/// the distinct-author count + the no-merge guarantee + the `peer add` pointer.
/// The thinnest "search surfaces unfollowed authors, trustworthily" proof.
///
/// @us-av-002 @real-io @driving_port @walking_skeleton @j-005 @kpi-av-1 @kpi-av-2 @happy
#[test]
fn search_by_object_surfaces_verified_claims_by_unfollowed_authors_attributed() {
    // -- Precondition: a localhost `openlore-indexer serve` over an index.duckdb
    // seeded with the US-AV-002 Example 1 corpus — verified claims asserting
    // reproducible-builds by 9 authors across 7 subjects, including Priya
    // (did:plc:priya-test, NOT followed) on bazel (0.82) and Rachel
    // (did:plc:rachel-test, a SUBSCRIBED peer) on nixpkgs (0.88). The CLI's
    // identity.toml points [appview] indexer_url at the localhost serve port. --
    //
    // -- Action: the object-dimension network read through the CLI driving port:
    // `openlore search --object org.openlore.philosophy.reproducible-builds`. --
    //
    // -- Observable outcome (port-exposed; asserted against stdout):
    //   1. exit 0; the public-data banner precedes the results;
    //   2. results include claims by authors Maria does NOT follow, labeled
    //      "(not subscribed)"; Rachel labeled "(subscribed peer)";
    //   3. every result row shows author DID + numeric confidence + display
    //      bucket + evidence + cid + "[verified]";
    //   4. NO row collapses multiple authors into a single entry;
    //   5. the footer states the distinct-author count (9) + the no-merge
    //      guarantee + "openlore peer add <did>".
    //
    // Universe (port-exposed observable surface of the --object view):
    // search.exit_code (0); search.distinct_authors_in_output (9 — counted from
    // attributed rows, EXCLUDING the footer-template "distinct authors" mention);
    // search.rows_collapsed (0); presence of the banner before results; the
    // relationship labels {priya:(not subscribed), rachel:(subscribed peer)};
    // every row carries "[verified]". Asserted against stdout, not internals.
    todo!(
        "DELIVER (slice-05): seed localhost indexer with the US-AV-002 Ex1 corpus \
         (9 authors / 7 subjects reproducible-builds incl. unfollowed Priya + \
         subscribed Rachel); run `openlore search --object \
         org.openlore.philosophy.reproducible-builds`; assert exit 0, banner \
         first, unfollowed labeled (not subscribed), every row [verified] + \
         author DID + numeric confidence + bucket + evidence + cid, no collapsed \
         row, footer = distinct-author count + no-merge + peer add pointer. \
         WALKING-SKELETON beat-2."
    );
}

/// AV-9 / RELEASE GATE `network_result_preserves_attribution` (US-AV-002;
/// I-AV-2 / WD-103 / KPI-AV-2 — load-bearing, release-blocking): two unfollowed
/// authors each published a verified claim asserting the SAME (subject, object);
/// `openlore search --object` returns BOTH as distinct rows under the subject,
/// each attributed to a distinct author DID labeled "(not subscribed)", and
/// there is NO row that represents both claims combined. The cardinal KPI-AV-2
/// disprover: any merged/consensus row or attribution loss is UNSHIPPABLE.
///
/// @us-av-002 @real-io @driving_port @release-gate @i-av-2 @kpi-av-2 @anti-merging @edge
#[test]
fn network_result_preserves_attribution() {
    // -- Precondition: a localhost indexer over an index seeded with the
    // US-AV-002 Example 2 pairing — Priya (0.70) and Sven (0.65), BOTH unfollowed,
    // both asserting github:denoland/deno embodies dependency-pinning. --
    //
    // -- Action: `openlore search --object org.openlore.philosophy.dependency-pinning`. --
    //
    // -- Observable outcome (the cardinal anti-merging gate):
    //   1. BOTH claims appear as DISTINCT rows under github:denoland/deno;
    //   2. each row is attributed to a distinct author DID (priya, sven), each
    //      labeled "(not subscribed)";
    //   3. there is NO "deno: 2 authors agree" / "the network says X" merged row
    //      anywhere in the output; the mean/aggregate confidence NEVER appears as
    //      a row;
    //   4. the footer's distinct-author count is 2 (a COUNT over attributed rows,
    //      not a merge).
    //
    // The pure composition preserves both (proven generatively in appview_core.rs
    // AVC-2/AVC-5); this layer-3 example pins the user-visible RENDERING + the
    // real B1 transport (the wire carries per-result author_did, D-D36). Anti-
    // merging assertion discipline (slice-03/04): count the attributed rows for
    // the (subject,object) pair == 2; assert NO line matches a merged/consensus/
    // "N authors agree"/mean-confidence template; EXCLUDE the footer's
    // "distinct authors" count line from the row count.
    //
    // Universe (port-exposed): the count of attributed rows for (deno,
    // dependency-pinning) == 2; the author-set of those rows {priya, sven};
    // absence of any merged/consensus/mean-confidence ROW; footer
    // distinct_author_count == 2.
    todo!(
        "DELIVER (slice-05): RELEASE GATE. Seed two unfollowed-author claims same \
         (deno,dependency-pinning) priya(0.70)+sven(0.65); run `openlore search \
         --object org.openlore.philosophy.dependency-pinning`; assert TWO distinct \
         attributed rows under deno (both (not subscribed)), NO merged/consensus/ \
         mean-confidence row anywhere, footer distinct-author count == 2 \
         (excluding the footer template from the row count)."
    );
}

/// AV-10 / RELEASE GATE `public_data_banner_shown` (US-AV-004; I-AV-4 / WD-105 /
/// KPI-AV-5): every `openlore search` session prints, BEFORE the results, a
/// banner stating discovery indexes only PUBLIC signed claims, each verified
/// before indexing, and that nothing private is read or aggregated.
///
/// @us-av-002 @us-av-004 @real-io @driving_port @release-gate @i-av-4 @kpi-av-5 @public-data
#[test]
fn public_data_banner_shown() {
    // -- Precondition: a reachable localhost indexer over a seeded index. --
    //
    // -- Action: ANY `openlore search` query (use the headline --object). --
    //
    // -- Observable outcome: a banner is printed UP FRONT (before the first
    // result row) stating (a) discovery indexes only PUBLIC signed claims
    // published to authors' PDSs, (b) each result is the author's own signed
    // record verified before indexing, (c) nothing private is read or
    // aggregated. Sets the indexing expectation honestly (the framing ADR-014
    // deferred to slice-05; KPI-AV-5).
    //
    // Universe (port-exposed): the banner present in stdout AND positioned before
    // the first result row (banner_precedes_results == true); the banner asserts
    // public-only + verified-before-indexing + nothing-private.
    todo!(
        "DELIVER (slice-05): RELEASE GATE. Run any `openlore search`; assert a \
         public-data banner precedes the results stating public-signed-only + \
         verified-before-indexing + nothing-private-read/aggregated (KPI-AV-5 / \
         I-AV-4)."
    );
}

/// AV-11 / RELEASE GATE `verified_marker_is_universal` (US-AV-004; I-AV-1): every
/// row in every `openlore search` result carries a `[verified]` marker; no
/// result is ever shown in an `[unverified]` / unknown-signature state (because
/// verification is an ingest precondition — US-AV-001 — there is no unverified
/// claim to render).
///
/// @us-av-002 @us-av-004 @real-io @release-gate @i-av-1 @verified-marker
#[test]
fn verified_marker_is_universal() {
    // -- Precondition: a localhost indexer over an index of verified claims by
    // several authors across dimensions. --
    //
    // -- Action: run `openlore search` across object + contributor + subject
    // dimensions. --
    //
    // -- Observable outcome: EVERY result row carries "[verified]"; NO row ever
    // shows "[unverified]" or "[unknown signature]". The marker is a construction
    // guarantee (the ingest gate, AV-3 + appview_core.rs AVC-7), not a per-result
    // runtime guess. There is no mixed-trust list.
    //
    // Universe (port-exposed): for every result row across all three dimensions,
    // row carries "[verified]"; the strings "[unverified]"/"unknown signature"
    // never appear in any search output.
    todo!(
        "DELIVER (slice-05): RELEASE GATE. Run `openlore search` across object/ \
         contributor/subject; assert EVERY row carries [verified] and NO row \
         shows [unverified]/unknown-signature (I-AV-1 construction guarantee)."
    );
}

/// AV-12 (US-AV-002 error): Maria typos the philosophy URI; the index finds zero
/// matches and prints "No network claims found for object <typo>. Did you mean
/// <near-match>?" (the near-match suggestion from appview_core.rs AVC-8) and
/// exits 0 — a valid empty result, NOT an error.
///
/// @us-av-002 @real-io @error @suggestion @edge
#[test]
fn search_by_object_unknown_philosophy_returns_empty_with_suggestion_exit_zero() {
    // -- Precondition: a reachable localhost indexer whose index has
    // reproducible-builds claims but NONE for the typo
    // "org.openlore.philosophy.reproducable-builds". --
    //
    // -- Action: `openlore search --object org.openlore.philosophy.reproducable-builds`. --
    //
    // -- Observable outcome: stdout states no network claims were found for that
    // object AND suggests the near-matching URI (reproducible-builds); exit code
    // is 0 (a valid empty result, US-AV-002 Ex 4 — distinct from the --show usage
    // error which exits non-zero). Mandate 11: a NAMED example sad path, not PBT.
    //
    // Universe (port-exposed): search.exit_code (0); stdout states empty + a
    // near-match suggestion line.
    todo!(
        "DELIVER (slice-05): run `openlore search --object \
         org.openlore.philosophy.reproducable-builds` (typo) against an index \
         with no match; assert exit 0 + 'no network claims found' + 'Did you \
         mean reproducible-builds?' (US-AV-002 Ex4 valid empty, not an error)."
    );
}

// =============================================================================
// US-AV-002 / US-AV-003 — LOCAL-FIRST degradation (release gate; KPI-5)
// =============================================================================

/// AV-13 / RELEASE GATE `local_first_preserved` (US-AV-002 + inherited;
/// I-AV-3 / WD-106 / KPI-5 — load-bearing, release-blocking): with the indexer
/// UNREACHABLE and the network disabled, `claim add` / offline `claim publish` /
/// `graph query` ALL succeed, and `openlore search` degrades to a clear
/// local-only message pointing to `graph query`, exits NON-fatally, and never
/// hangs. The cardinal KPI-5 disprover: any regression that breaks offline
/// authoring (or makes search a startup hard-fail) is UNSHIPPABLE.
///
/// @us-av-002 @us-av-003 @real-io @driving_port @release-gate @i-av-3 @kpi-5 @local-first
#[test]
fn local_first_preserved() {
    // -- Precondition: a TestEnv with NO reachable indexer (the [appview]
    // indexer_url points at a closed/unbound port) AND the network disabled
    // (the slice-01/03 network-disabled seam). --
    //
    // -- Action + observable outcome (FOUR sub-assertions, one observable each):
    //   1. `openlore claim add ...` succeeds (exit 0; the claim is composed/
    //      stored LOCALLY) — the indexer is NOT probed at CLI startup (WD-116);
    //   2. offline `openlore claim publish` (the slice-01 offline path) succeeds;
    //   3. `openlore graph query --object ...` succeeds (exit 0, LOCAL graph) —
    //      links no indexer code;
    //   4. `openlore search --object ...` prints a clear "Network index
    //      unavailable. Showing/See LOCAL results via `openlore graph query
    //      --object ...`" message, exits NON-fatally (the soft Unreachable
    //      outcome, ADR-027/WD-116), and does NOT hang (bounded wall-clock).
    //
    // The CLI links NO indexer store/ingest/server code (the structural layer is
    // DELIVER's xtask dependency-graph check); this asserts the BEHAVIORAL layer:
    // offline authoring is untouched and search degrades softly. Mandate 11: a
    // named example, not PBT.
    //
    // Universe (port-exposed): claim_add.exit_code (0); claim_publish.exit_code
    // (0, offline); graph_query.exit_code (0); search.exit_code (non-fatal, the
    // soft Unreachable contract — clear local-only message + graph-query pointer);
    // search.hung (false). The local store mutated only by the authoring verbs,
    // never by search.
    todo!(
        "DELIVER (slice-05): RELEASE GATE. With indexer UNREACHABLE + network \
         disabled: assert `claim add`, offline `claim publish`, `graph query \
         --object` ALL exit 0, AND `openlore search --object` prints a clear \
         local-only message + `graph query` pointer, exits non-fatally, no hang \
         (KPI-5 / I-AV-3; the indexer is not probed at CLI startup, WD-116)."
    );
}

/// AV-14 / RELEASE GATE `search_succeeds_with_indexer_localhost` (US-AV-002;
/// B1 transport, WD-115 / D-D36): with a REAL `openlore-indexer serve` reachable
/// over LOCALHOST (the B1 CLI<->indexer XRPC boundary), `openlore search` reaches
/// it, the wire response carries per-result `author_did` (anti-merging across the
/// transport, D-D36), and the CLI renders attributed verified results. Proves the
/// production CLI->indexer transport wiring end-to-end.
///
/// @us-av-002 @real-io @driving_port @release-gate @b1-transport @wd-115 @kpi-av-2
#[test]
fn search_succeeds_with_indexer_localhost() {
    // -- Precondition: a REAL `openlore-indexer serve` bound to an EPHEMERAL
    // localhost port (`:0`, read back — DEVOPS open-q 8, parallel-safe) over a
    // seeded index; the CLI's [appview] indexer_url points at that port. --
    //
    // -- Action: `openlore search --object <philosophy>`. --
    //
    // -- Observable outcome: the CLI reaches the indexer over localhost HTTP/XRPC
    // (org.openlore.appview.searchClaims); the response carries EVERY result's
    // author_did (the B1 contract, D-D36 — a response dropping it is an
    // anti-merging violation across the transport); the CLI renders the
    // attributed verified results. This pins the production B1 transport that the
    // consumer-driven Pact contract (D-D36) covers at the wire level — the AT
    // proves the CLI driving port reaches the real server and renders the result.
    //
    // Universe (port-exposed): search.exit_code (0); the rendered result is
    // non-empty + attributed (every row has author_did + [verified]); the
    // transport reached the localhost serve port (a result was returned, not the
    // Unreachable degradation).
    todo!(
        "DELIVER (slice-05): RELEASE GATE / B1. With a REAL `openlore-indexer \
         serve` on an ephemeral localhost port + indexer_url pointed at it, run \
         `openlore search --object <philosophy>`; assert exit 0, the CLI reached \
         the real server (not the Unreachable path), and rendered attributed \
         verified results (every wire result carried author_did; D-D36/WD-115)."
    );
}

// =============================================================================
// US-AV-003 — search by contributor / subject at network scale (Release 2)
// =============================================================================

/// AV-15 (US-AV-003 happy — contributor trail before following): Maria runs
/// `openlore search --contributor github:priya` (resolves to did:plc:priya-test,
/// unfollowed); the index returns her whole verified network reasoning trail
/// (8 claims / 6 subjects) under one DID, each with subject/object/confidence/cid/
/// [verified], and the footer states "one developer's reasoning trail, not a
/// community consensus" + a `peer add` offer.
///
/// @us-av-003 @real-io @driving_port @j-005 @kpi-av-1 @kpi-av-4 @happy
#[test]
fn search_by_contributor_lists_full_network_trail_with_honest_framing() {
    // -- Precondition: a localhost indexer seeded with Priya's 8 verified claims
    // across 6 subjects (US-AV-003 Example 1); Maria does NOT follow her;
    // github:priya resolves to did:plc:priya-test (slice-02/04 handle->DID). --
    //
    // -- Action: `openlore search --contributor github:priya`. --
    //
    // -- Observable outcome: all 8 verified claims listed under did:plc:priya-test
    // with subject/object/confidence/cid/[verified]; the trail is labeled
    // "(not subscribed)"; the footer states this is ONE developer's reasoning
    // trail, NOT a community consensus, and offers
    // "openlore peer add did:plc:priya-test". No merged row (a single author by
    // construction, but the honesty framing is the load-bearing assertion).
    //
    // Universe (port-exposed): search.exit_code (0); the count of attributed rows
    // under priya (8); the "(not subscribed)" label; the footer states the
    // one-developer-not-consensus framing + the peer-add offer.
    todo!(
        "DELIVER (slice-05): seed Priya's 8 verified claims / 6 subjects; run \
         `openlore search --contributor github:priya`; assert exit 0, 8 \
         attributed rows under did:plc:priya-test each [verified], label \
         (not subscribed), footer = 'one developer's reasoning trail, not a \
         community consensus' + `peer add did:plc:priya-test`."
    );
}

/// AV-16 (US-AV-003 happy — subject survey at network scale): Tobias runs
/// `openlore search --subject github:bazelbuild/bazel`; the index returns
/// verified claims about bazel from 5 distinct network authors, grouped by
/// author, each with philosophy/confidence/cid/[verified]; NO "bazel: the network
/// thinks X" merged row.
///
/// @us-av-003 @real-io @driving_port @kpi-av-2 @anti-merging @happy
#[test]
fn search_by_subject_surfaces_every_authors_verified_claims_attributed() {
    // -- Precondition: a localhost indexer seeded with verified bazel claims by
    // 5 distinct authors (US-AV-003 Example 2). --
    //
    // -- Action: `openlore search --subject github:bazelbuild/bazel`. --
    //
    // -- Observable outcome: claims grouped by author (5 distinct author groups),
    // each row with philosophy/confidence/cid/[verified]; NO row collapses
    // multiple authors into a single "network consensus" entry. The subject
    // dimension's anti-merging RENDER (the pure composition proven in AVC-2).
    //
    // Universe (port-exposed): the count of distinct author groups (5, from
    // attributed rows, excluding any footer template); absence of a
    // merged/consensus row; every row [verified].
    todo!(
        "DELIVER (slice-05): seed bazel claims by 5 distinct authors; run \
         `openlore search --subject github:bazelbuild/bazel`; assert grouped by \
         author (5 distinct groups), each row [verified], NO merged consensus \
         row (KPI-AV-2 subject-dimension render)."
    );
}

/// AV-17 (US-AV-003 edge — contributor absent from the index): Aanya searches a
/// contributor with no verified network claims; the CLI prints a clear "no
/// network claims found for contributor <handle>" message and exits 0.
///
/// @us-av-003 @real-io @error @edge
#[test]
fn search_by_contributor_absent_from_index_degrades_gracefully_exit_zero() {
    // -- Precondition: a reachable localhost indexer whose index has no claims by
    // github:nobody-here. --
    //
    // -- Action: `openlore search --contributor github:nobody-here`. --
    //
    // -- Observable outcome: stdout states no network claims were found for that
    // contributor (they may not publish OpenLore claims, or are not yet
    // ingested); exit code 0 (a valid empty result). Mandate 11 named example.
    //
    // Universe (port-exposed): search.exit_code (0); stdout states the empty-
    // contributor message.
    todo!(
        "DELIVER (slice-05): run `openlore search --contributor github:nobody- \
         here` against an index with no such claims; assert exit 0 + 'no network \
         claims found for contributor' message (US-AV-003 Ex3)."
    );
}

/// AV-18 (US-AV-003 edge — followed author labeled correctly in network search):
/// Maria searches a contributor she already follows; that author's network claims
/// are labeled "(subscribed peer)" rather than "(not subscribed)", preserving the
/// slice-03 relationship labeling even in network search.
///
/// @us-av-003 @us-av-005 @real-io @relationship-label @edge
#[test]
fn search_labels_a_followed_author_as_subscribed_peer() {
    // -- Precondition: Maria already subscribes to did:plc:rachel-test (a slice-03
    // peer add); the localhost index has Rachel's verified network claims. --
    //
    // -- Action: `openlore search --contributor github:rachel`. --
    //
    // -- Observable outcome: Rachel's network claims are labeled "(subscribed
    // peer)" (NOT "(not subscribed)"); every claim retains its author DID +
    // [verified] marker; NO "Follow this author" affordance is shown for her (she
    // is already followed — chains into AV-20). The relationship is resolved
    // CLI-side against the user's peer_subscriptions (the index is per-user-
    // neutral, data-models.md).
    //
    // Universe (port-exposed): Rachel's rows labeled "(subscribed peer)"; absence
    // of the "Follow this author" affordance for Rachel; every row [verified].
    todo!(
        "DELIVER (slice-05): with Maria subscribed to did:plc:rachel-test, run \
         `openlore search --contributor github:rachel`; assert Rachel's rows \
         labeled (subscribed peer) [not (not subscribed)], each [verified], and \
         NO follow affordance for her (US-AV-003 Ex4)."
    );
}

// =============================================================================
// US-AV-005 — discovery -> federation funnel (Release 2)
// =============================================================================

/// AV-19 (US-AV-005 happy — discover, follow, pull into local graph): Maria
/// discovers Priya's claim via `openlore search`; the result ends with "Follow
/// this author: `openlore peer add did:plc:priya-test`"; Maria runs THAT slice-03
/// command verbatim; after `openlore peer pull`, Priya's claims appear in Maria's
/// LOCAL `graph query --contributor` and participate in `--weighted` views. The
/// funnel that makes the AppView strengthen the local-first graph (KPI-AV-4).
///
/// @us-av-005 @real-io @driving_port @j-005 @kpi-av-4 @i-av-7 @happy
#[test]
fn discovery_follow_reuses_slice03_path() {
    // -- Precondition: a localhost indexer with Priya's verified network claim
    // (unfollowed); a slice-03 PeerPds double hosting Priya's claims for the pull
    // (the SAME federation seam slice-03 uses). --
    //
    // -- Action (the chained funnel — reusing earlier step-methods):
    //   1. `openlore search --object reproducible-builds` -> the result for Priya
    //      ends with "Follow this author: openlore peer add did:plc:priya-test";
    //   2. run THAT command verbatim (the slice-03 `peer add` — NO new verb);
    //   3. `openlore peer pull`;
    //   4. `openlore graph query --contributor did:plc:priya-test`. --
    //
    // -- Observable outcome: after the funnel, Priya's claims are in Maria's LOCAL
    // graph (graph query --contributor returns them) and participate in
    // --weighted / --traverse exactly like any pulled peer. The follow affordance
    // is a RENDER-ONLY hint printing the EXISTING slice-03 command (no parallel
    // subscription path, no auto-follow; I-AV-7); the subscription is created by
    // the SAME slice-03 `peer add` (proven by AV-22's purge symmetry).
    //
    // Universe (port-exposed): the search result's follow-affordance line (the
    // slice-03 `peer add <did>` command verbatim); after peer add + pull,
    // graph_query_contributor(priya).rows non-empty; the subscription appears in
    // `peer list` exactly as a slice-03 add (no parallel state).
    todo!(
        "DELIVER (slice-05): chain `openlore search --object reproducible-builds` \
         -> run the rendered `openlore peer add did:plc:priya-test` affordance \
         verbatim -> `openlore peer pull` -> `openlore graph query --contributor \
         did:plc:priya-test`; assert Priya's claims now in the LOCAL graph + in \
         --weighted; the affordance reused slice-03 peer add (no parallel path; \
         KPI-AV-4 / I-AV-7)."
    );
}

/// AV-20 (US-AV-005 edge — discovery never auto-subscribes): Aanya runs several
/// searches and inspects results without ever running `peer add`; no subscription
/// is created and `openlore peer list` is unchanged. Following is always an
/// explicit, separate human action (no auto-follow).
///
/// @us-av-005 @real-io @i-av-7 @edge
#[test]
fn discovery_never_auto_subscribes() {
    // -- Precondition: a localhost indexer with several unfollowed authors;
    // Aanya's peer_subscriptions empty (or a known baseline). --
    //
    // -- Action: run several `openlore search` queries + a `--show`; do NOT run
    // any `peer add`. --
    //
    // -- Observable outcome: `openlore peer list` is UNCHANGED (no subscription
    // created by search/inspect); discovery is read-only; following is an
    // explicit human action. The render-only affordance never executes a follow
    // (I-AV-7).
    //
    // Universe (port-exposed): peer_subscriptions before == after (search +
    // --show mutate NO subscription state); `peer list` output unchanged.
    todo!(
        "DELIVER (slice-05): run several `openlore search` + `--show` queries \
         WITHOUT any `peer add`; assert `peer list` (peer_subscriptions) is \
         unchanged — discovery never auto-subscribes (US-AV-005 Ex3 / I-AV-7)."
    );
}

/// AV-21 (US-AV-005 edge — already-followed author shows no redundant affordance):
/// Tobias's search result includes Rachel (already followed); her result is
/// labeled "(subscribed peer)" and shows NO "Follow this author" affordance — the
/// funnel affordance appears ONLY for unfollowed authors.
///
/// @us-av-005 @real-io @relationship-label @edge
#[test]
fn already_followed_author_shows_no_redundant_follow_affordance() {
    // -- Precondition: Tobias subscribes to did:plc:rachel-test; the localhost
    // index returns Rachel + an unfollowed author for the same query. --
    //
    // -- Action: `openlore search --object <philosophy Rachel claims>`. --
    //
    // -- Observable outcome: Rachel's row labeled "(subscribed peer)" with NO
    // "Follow this author" affordance; the unfollowed author's row DOES carry the
    // affordance. The affordance is conditioned on the relationship label
    // (resolved CLI-side against peer_subscriptions).
    //
    // Universe (port-exposed): presence of the follow affordance per author
    // (absent for Rachel/subscribed; present for the unfollowed author).
    todo!(
        "DELIVER (slice-05): with Tobias subscribed to Rachel, run a search that \
         returns Rachel + an unfollowed author; assert Rachel (subscribed peer) \
         has NO follow affordance while the unfollowed author DOES (US-AV-005 \
         Ex2)."
    );
}

/// AV-22 (US-AV-005 edge — follow reuses slice-03 path with no parallel state):
/// Maria follows a discovered author via the affordance, then runs
/// `openlore peer remove <did> --purge`; the slice-03 purge semantics apply
/// unchanged, leaving zero residue — discovery introduced no parallel
/// subscription state to leak.
///
/// @us-av-005 @real-io @i-av-7 @edge
#[test]
fn followed_discovery_author_purges_via_slice03_semantics_zero_residue() {
    // -- Precondition: Maria followed a discovered author via the AV-19 funnel
    // (the slice-03 `peer add`) + pulled their claims. --
    //
    // -- Action: `openlore peer remove did:plc:priya-test --purge`. --
    //
    // -- Observable outcome: the slice-03 purge semantics apply UNCHANGED — the
    // subscription + the pulled peer_claims are removed leaving ZERO residue
    // (the same state-delta universe slice-03 PS-6 asserts). Because the author
    // was added via the SAME `peer add` path (no parallel discovery-subscription
    // state, I-AV-7), the purge is indistinguishable from a slice-03 add+purge.
    //
    // Universe (port-exposed): post-purge peer_subscriptions (priya absent),
    // peer_claims/<priya>/ (removed), no orphaned discovery-side subscription
    // record (none exists — the load-bearing absence of a parallel path).
    todo!(
        "DELIVER (slice-05): after following a discovered author via the funnel, \
         run `openlore peer remove <did> --purge`; assert slice-03 purge \
         semantics leave zero residue (no parallel discovery-subscription state \
         to leak; I-AV-7)."
    );
}

// =============================================================================
// US-AV-004 — trust display (--show) (Release 1 trust surface)
// =============================================================================

/// AV-23 (US-AV-004 happy — inspect a verified discovered record): Maria runs
/// `openlore search --object ... --show <cid>` for a result by an unfollowed
/// author; the output prints the full record (subject/object/confidence/evidence/
/// author DID) + "Signature: VERIFIED against <did>" + "CID: <cid> (recomputed,
/// matches published record)". She trusts it as the author's genuine signed
/// claim.
///
/// @us-av-004 @real-io @driving_port @j-005 @kpi-av-3 @happy
#[test]
fn show_inspects_a_verified_record_with_signature_and_cid_match_lines() {
    // -- Precondition: a localhost indexer with Priya's verified bazel/
    // reproducible-builds claim (cid bafy...k2); a prior search listed it. --
    //
    // -- Action: `openlore search --object org.openlore.philosophy.reproducible-
    // builds --show bafy...k2`. --
    //
    // -- Observable outcome: the full record is printed — subject
    // github:bazelbuild/bazel, object reproducible-builds, confidence 0.82,
    // evidence URL, author did:plc:priya-test — PLUS "Signature: VERIFIED against
    // did:plc:priya-test" AND "CID: bafy...k2 (recomputed, matches published
    // record)". These lines render the SAME pure-core verification result the
    // indexer computed at ingest (no second path, US-AV-004 Technical Notes); the
    // display is READ-ONLY (creates/signs/mutates nothing).
    //
    // Universe (port-exposed): the --show output contains the full record fields +
    // the "Signature: VERIFIED against did:plc:priya-test" line + the "CID ...
    // (recomputed, matches published record)" line; no local state mutated.
    todo!(
        "DELIVER (slice-05): run `openlore search --object reproducible-builds \
         --show bafy...k2` for Priya's verified claim; assert the full record + \
         'Signature: VERIFIED against did:plc:priya-test' + 'CID ... (recomputed, \
         matches published record)'; read-only (US-AV-004 Ex1; same pure-core \
         result, no second path)."
    );
}

/// AV-24 (US-AV-004 error — `--show` a CID not in the result set): Maria runs
/// `--show <cid>` for a CID absent from the current result set; the CLI prints a
/// usage error ("CID ... is not in this search result. Run the search without
/// --show to list results, then --show a listed CID.") and exits NON-ZERO —
/// distinct from an empty search (which exits 0).
///
/// @us-av-004 @real-io @error @edge
#[test]
fn show_on_cid_absent_from_result_set_is_a_usage_error_nonzero_exit() {
    // -- Precondition: a reachable localhost indexer; a search whose result set
    // does NOT contain "bafy...nothere". --
    //
    // -- Action: `openlore search --object ... --show bafy...nothere`. --
    //
    // -- Observable outcome: stderr/stdout states the CID is not in this search
    // result + the remediation hint; exit code is NON-ZERO (a usage error). This
    // is the ONE non-zero sad path on the search surface — deliberately distinct
    // from the empty-result exit-0 (AV-12/AV-17), so the user can tell a typo'd
    // --show from an empty query. Mandate 11 named example.
    //
    // Universe (port-exposed): search.exit_code (non-zero); the "CID is not in
    // this search result" usage message + remediation hint.
    todo!(
        "DELIVER (slice-05): run `openlore search --object ... --show \
         bafy...nothere` for a CID absent from the result set; assert NON-ZERO \
         exit + 'CID is not in this search result' usage message (distinct from \
         the empty-search exit-0; US-AV-004 Ex4)."
    );
}

// =============================================================================
// US-AV-002 — counter shown, not applied (OD-AV-7 / I-AV-9) — render layer
// =============================================================================

/// AV-25 (US-AV-002 / OD-AV-7; I-AV-9): a countered/soft-retracted public
/// verified claim STILL appears in network search results; the counter
/// relationship is shown (annotated "countered-by <cid> (by <did>)") when known,
/// and the countered row is NEVER silently filtered or down-weighted. Counter is
/// SHOWN, never applied (WD-119 default).
///
/// @us-av-002 @real-io @od-av-7 @i-av-9 @edge
#[test]
fn countered_claim_still_appears_in_search_with_annotation() {
    // -- Precondition: a localhost indexer whose index has claim C and a later
    // indexed claim K that references C with ref_type=counters (both verified). --
    //
    // -- Action: `openlore search --object <C's object>`. --
    //
    // -- Observable outcome: C STILL appears as a result row (NOT filtered/
    // dropped/down-weighted); its row carries a counter annotation "countered-by
    // <K.cid> (by <K.author_did>)". The counter is SHOWN, never applied as a
    // filter (the pure-core behavior proven in appview_core.rs AVC-6; this pins
    // the user-visible render). Mirrors slice-04 WD-85 (counter shown in
    // --explain, not applied).
    //
    // Universe (port-exposed): presence of C in the search output; C's row
    // carries the counter annotation; C is NOT filtered/down-weighted.
    todo!(
        "DELIVER (slice-05): with claim C countered by indexed claim K, run \
         `openlore search --object <C.object>`; assert C STILL appears with a \
         'countered-by <K.cid> (by <K.author>)' annotation and is NOT filtered/ \
         down-weighted (OD-AV-7 / I-AV-9 shown-not-applied)."
    );
}

// =============================================================================
// US-AV-006 — share a network search result as a stable link (Release 3)
// =============================================================================

/// AV-26 (US-AV-006 happy — share a philosophy query): Maria runs
/// `openlore search --object org.openlore.philosophy.reproducible-builds --share`;
/// the CLI emits a stable link encoding the query dimension+value
/// (`openlore://search?object=...`) and states "the link encodes the query, not a
/// frozen snapshot". Realizes the J-004 shareable-link signal (KPI-AV-6).
///
/// @us-av-006 @real-io @driving_port @j-005 @kpi-av-6 @i-av-8 @happy
#[test]
fn share_emits_stable_query_encoding_link_for_object_search() {
    // -- Precondition: a reachable localhost indexer; the reproducible-builds
    // corpus seeded. --
    //
    // -- Action: `openlore search --object org.openlore.philosophy.reproducible-
    // builds --share`. --
    //
    // -- Observable outcome: stdout prints "Shareable link:
    // openlore://search?object=org.openlore.philosophy.reproducible-builds" (the
    // link encodes the QUERY dimension+value ONLY — no results, no snapshot) AND
    // a line stating the link encodes the query, not a frozen snapshot (the
    // sharing semantics, US-AV-006 Ex1). The exact link grammar is DELIVER's
    // (Q-DELIVER-AV-3); DISTILL asserts the query-encoding-not-snapshot contract.
    //
    // Universe (port-exposed): the printed share link encodes dimension=object +
    // value=<philosophy> and NO result payload/snapshot; the "encodes the query,
    // not a snapshot" semantics line present.
    todo!(
        "DELIVER (slice-05): run `openlore search --object \
         org.openlore.philosophy.reproducible-builds --share`; assert a stable \
         `openlore://search?object=...` link encoding ONLY dimension+value (no \
         result snapshot) + the 'encodes the query, not a snapshot' line \
         (US-AV-006 Ex1 / I-AV-8)."
    );
}

/// AV-27 (US-AV-006 happy — opening a shared link yields the same attributed
/// verified results): Tobias opens the link Maria shared; the encoded query runs
/// against the index and he sees the SAME claims grouped by author, each with its
/// author DID + [verified] marker, anti-merging preserved, with the same `peer
/// add` follow affordance.
///
/// @us-av-006 @real-io @driving_port @kpi-av-6 @kpi-av-2 @i-av-8 @anti-merging @happy
#[test]
fn opening_a_shared_link_re_runs_the_query_yielding_same_attributed_results() {
    // -- Precondition: a localhost indexer over the seeded corpus; a share link
    // `openlore://search?object=org.openlore.philosophy.reproducible-builds`
    // (from AV-26). --
    //
    // -- Action: open the link via the CLI re-run resolver (Q-DELIVER-AV-3; web
    // AppView OUT of scope per OD-AV-6) — e.g. `openlore search <link>` or the
    // resolver verb DELIVER picks. --
    //
    // -- Observable outcome: the encoded query re-runs against the index and
    // returns the SAME per-author-attributed verified results (each row author
    // DID + [verified]), anti-merging preserved (NO merged consensus row), with
    // the same `peer add` affordance for unfollowed authors. The link encoded the
    // QUERY (proven deterministic in appview_core.rs AVC-3b), NOT a snapshot —
    // the resolver re-composes per-author rows (anti-merging across the share
    // boundary, I-AV-8/KPI-AV-2).
    //
    // Universe (port-exposed): the resolved result's per-author rows match the
    // original query's (same authors, same [verified] marks); NO merged row; the
    // follow affordance present for unfollowed authors.
    todo!(
        "DELIVER (slice-05): open the `openlore://search?object=...` link via the \
         CLI re-run resolver; assert it re-runs the query -> same per-author- \
         attributed verified results (every row author DID + [verified]), NO \
         merged row, same `peer add` affordance (US-AV-006 Ex2; anti-merging \
         across the share boundary, I-AV-8/KPI-AV-2)."
    );
}

/// AV-28 (US-AV-006 edge — link resolves to CURRENT results, not a stale
/// snapshot): Maria shares a query link; later the indexer ingests two more
/// verified matching claims; when Tobias opens the SAME link, the result set
/// INCLUDES the newly-ingested claims — because the link encodes the QUERY, not a
/// frozen snapshot, and never collapses authors into a stored merged view.
///
/// @us-av-006 @real-io @kpi-av-6 @i-av-8 @edge
#[test]
fn shared_link_resolves_to_current_results_not_a_stale_snapshot() {
    // -- Precondition: a share link from an earlier search; THEN the indexer
    // ingests two MORE verified claims matching that query (a second ingest pass
    // over the localhost serve's index). --
    //
    // -- Action: open the SAME link after the new ingest. --
    //
    // -- Observable outcome: the resolved result set INCLUDES the two
    // newly-ingested verified claims (the link re-runs the QUERY against the
    // current index), each attributed + [verified]; the link NEVER resolves to a
    // stored merged snapshot that loses attribution. Proves the
    // query-encoding-not-snapshot contract end-to-end across an index change
    // (US-AV-006 Ex4 / I-AV-8).
    //
    // Universe (port-exposed): the resolved result count grows by 2 after the new
    // ingest (current, not frozen); the new rows are attributed + [verified]; no
    // merged snapshot.
    todo!(
        "DELIVER (slice-05): share a link; ingest two MORE matching verified \
         claims; re-open the SAME link; assert the result set now INCLUDES the \
         two new claims (current, not a frozen snapshot), each attributed + \
         [verified], no merged view (US-AV-006 Ex4 / I-AV-8)."
    );
}

/// AV-29 (US-AV-006 edge — share a contributor query): `openlore search
/// --contributor github:priya --share` emits
/// `openlore://search?contributor=did:plc:priya-test`; opening it resolves to
/// Priya's verified network trail with the same "one developer's reasoning trail,
/// not a community consensus" framing.
///
/// @us-av-006 @real-io @kpi-av-6 @i-av-8 @edge
#[test]
fn share_encodes_contributor_dimension_resolving_to_the_trail() {
    // -- Precondition: a localhost indexer with Priya's verified trail; github:
    // priya resolves to did:plc:priya-test. --
    //
    // -- Action: `openlore search --contributor github:priya --share`. --
    //
    // -- Observable outcome: the share link encodes
    // dimension=contributor + value=did:plc:priya-test
    // (openlore://search?contributor=did:plc:priya-test); opening it (CLI re-run)
    // resolves to Priya's verified trail with the same "one developer's reasoning
    // trail, not a community consensus" footer. The contributor-dimension share
    // round-trip (parallel to AV-26/AV-27 for object).
    //
    // Universe (port-exposed): the share link encodes contributor=did:plc:priya-
    // test (resolved handle->DID, not the handle); resolving yields the trail +
    // the not-consensus framing.
    todo!(
        "DELIVER (slice-05): run `openlore search --contributor github:priya \
         --share`; assert the link encodes contributor=did:plc:priya-test \
         (resolved DID); opening it resolves to Priya's verified trail + the \
         'one developer's reasoning trail, not a community consensus' framing \
         (US-AV-006 Ex3)."
    );
}
