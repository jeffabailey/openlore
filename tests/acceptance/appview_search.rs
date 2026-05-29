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
    let env = TestEnv::initialized();

    // -- Precondition (relationship): Maria already SUBSCRIBES to Rachel
    // (did:plc:rachel-test) via the slice-03 `peer add` (resolved against a
    // PeerPds double). Every OTHER corpus author stays unfollowed. The index is
    // per-user-neutral; the relationship label is resolved CLI-side against this
    // subscription. --
    let rachel_did = "did:plc:rachel-test";
    let rachel_peer = PeerPds::for_peer(rachel_did, Vec::new());
    let peer_add = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", rachel_did],
        rachel_did,
        rachel_peer.endpoint_url(),
    );
    assert_eq!(
        peer_add.status, 0,
        "precondition: `openlore peer add {rachel_did}` must exit 0. stdout: {} stderr: {}",
        peer_add.stdout, peer_add.stderr
    );

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-002 Ex1 corpus (9 authors / 7 subjects
    // reproducible-builds, incl. unfollowed Priya on bazel 0.82 + subscribed
    // Rachel on nixpkgs 0.88). The CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: the object-dimension network read through the CLI driving port. --
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );

    // 1. exit 0 + the public-data banner PRECEDES the results.
    assert_eq!(
        outcome.status, 0,
        "`openlore search --object` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );
    assert_public_data_banner_precedes_results(&outcome.stdout);

    // 2. results include claims by authors Maria does NOT follow, labeled
    //    "(not subscribed)"; Rachel labeled "(subscribed peer)".
    assert!(
        outcome
            .stdout
            .contains("did:plc:priya-test#org.openlore.application (not subscribed)"),
        "expected unfollowed Priya labeled (not subscribed):\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("did:plc:rachel-test#org.openlore.application (subscribed peer)"),
        "expected subscribed Rachel labeled (subscribed peer):\n{}",
        outcome.stdout
    );

    // 3. every result row carries [verified] (+ author DID + numeric confidence +
    //    display bucket + evidence + cid). 4. NO row collapses multiple authors.
    assert_verified_marker_is_universal(&outcome.stdout);
    // Numeric confidence + display bucket appear (Priya's bazel claim is 0.82 →
    // the well-evidenced bucket); evidence + cid labels are emitted per row.
    assert!(
        outcome.stdout.contains("0.82") && outcome.stdout.contains("(well-evidenced)"),
        "expected Priya's numeric confidence 0.82 + display bucket (well-evidenced):\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("evidence:") && outcome.stdout.contains("cid:"),
        "expected every row to show an evidence label + a cid:\n{}",
        outcome.stdout
    );
    // No row collapses authors: exactly 9 attributed rows, the author-set includes
    // both Priya (unfollowed) + Rachel (subscribed), and NO merged/consensus row.
    assert_network_result_preserves_attribution(
        &outcome.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        9,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:rachel-test#org.openlore.application",
        ],
    );

    // 5. the footer states the distinct-author count (9) + the no-merge guarantee
    //    + the `openlore peer add <did>` pointer.
    assert!(
        outcome.stdout.contains("9 distinct author(s)."),
        "expected the footer to state the distinct-author count (9):\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("No claims are merged."),
        "expected the footer's no-merge guarantee:\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("openlore peer add"),
        "expected the footer's `openlore peer add <did>` follow pointer:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-002 Ex2 / AVC-5 corpus — Priya (0.70)
    // and Sven (0.65), BOTH UNFOLLOWED, both asserting github:denoland/deno
    // embodies dependency-pinning (the identical-(subject,object) zero-merge
    // fixture). The CLI's indexer_url points at the serve port. NO `peer add`
    // precedes this scenario, so BOTH authors are unfollowed. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::DenoDependencyPinningTwoUnfollowedAuthors,
    );

    // -- Action: the object-dimension network read through the CLI driving port. --
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.dependency-pinning",
        ],
        &indexer,
    );

    // exit 0 (a valid result, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "`openlore search --object dependency-pinning` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // 1 + 2 + 3. The cardinal anti-merging gate (KPI-AV-2): EXACTLY 2 attributed
    // rows for (deno, dependency-pinning), the author-set {priya, sven}, and NO
    // merged/consensus/"N authors agree"/"the network says" row anywhere — the
    // footer template ("N distinct author(s)." / "No claims are merged.") is
    // EXCLUDED from the row count by construction (it never starts with
    // `author_did:`). The wire (B1) carried per-result author_did (D-D36): the two
    // distinct rendered rows can only exist if the transport preserved each row's
    // author attribution end-to-end.
    assert_network_result_preserves_attribution(
        &outcome.stdout,
        "github:denoland/deno",
        "org.openlore.philosophy.dependency-pinning",
        2,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:sven-test#org.openlore.application",
        ],
    );

    // 2 (label). BOTH authors are unfollowed → each labeled "(not subscribed)".
    assert!(
        outcome
            .stdout
            .contains("did:plc:priya-test#org.openlore.application (not subscribed)"),
        "expected unfollowed Priya labeled (not subscribed):\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("did:plc:sven-test#org.openlore.application (not subscribed)"),
        "expected unfollowed Sven labeled (not subscribed):\n{}",
        outcome.stdout
    );

    // Every row carries [verified] (verification is an ingest precondition; there
    // is no [unverified] state) — pins the two rows are real attributed results.
    assert_verified_marker_is_universal(&outcome.stdout);

    // 4. The footer's distinct-author count is 2 — a COUNT over the attributed
    //    rows, NOT a merge (the no-merge guarantee phrasing legitimately mentions
    //    the count; it is the PROMISE, never a merged row).
    assert!(
        outcome.stdout.contains("2 distinct author(s)."),
        "expected the footer to state the distinct-author count (2) as a COUNT, \
         not a merged row:\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("No claims are merged."),
        "expected the footer's no-merge guarantee:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a reachable localhost `openlore-indexer serve` over
    // an index.duckdb seeded with the headline reproducible-builds corpus (9
    // authors / 7 subjects) so the search returns a NON-EMPTY result set — the
    // banner must precede the FIRST result row, which only exists when results do.
    // The CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: ANY `openlore search` query — use the headline --object. --
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );

    // exit 0 (a valid result, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "`openlore search --object` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The RELEASE GATE (KPI-AV-5 / I-AV-4): the public-data banner is present AND
    // positioned BEFORE the first result row (banner_precedes_results == true), and
    // it asserts the three honesty facts — discovery indexes ONLY public, signed
    // claims; each verified before indexing; nothing private is read or aggregated.
    assert_public_data_banner_precedes_results(&outcome.stdout);
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
    //
    // DIMENSION SEQUENCING (this AT, per the renderer's construction guarantee):
    // the `[verified]` marker is emitted by `render_one_network_row` for EVERY
    // row REGARDLESS of dimension (render::render_network_search_result is
    // dimension-AGNOSTIC) — it is the universal ingest-gate guarantee (AV-3 +
    // appview_core.rs AVC-7), never a per-result runtime guess. So the
    // universality claim is proven by exercising the OBJECT dimension over
    // multiple INDEPENDENT corpora — a 9-author survey AND a 2-author
    // identical-(subject,object) pair (the hardest anti-merging shape) — and
    // asserting, over EVERY rendered row of EACH, that the row carries
    // "[verified]" and that "[unverified]"/"unknown signature" appear NOWHERE.
    // (The `--contributor`/`--subject` dimension VERBS land in Phase 05 — AV-15
    // /AV-16; they reuse this SAME dimension-agnostic renderer, so wiring them
    // here would add no universality coverage the render layer does not already
    // guarantee.)
    // Each corpus gets its OWN sealed `TestEnv` so its `index.duckdb` is
    // isolated — a single env would put both indexers' serve processes on the
    // SAME index file, and DuckDB takes an exclusive lock per file (the second
    // ingest would conflict). One env per indexer is the harness's RAII
    // per-scenario isolation contract (AV-8/9/10 each take a fresh env).

    // -- Corpus A: the 9-author reproducible-builds survey (many distinct
    // authors, each with a verified claim). --
    let env_a = TestEnv::initialized();
    let indexer_a = seed_network_index(
        &env_a,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let outcome_a = run_openlore_search(
        &env_a,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer_a,
    );
    assert_eq!(
        outcome_a.status, 0,
        "`openlore search --object reproducible-builds` must exit 0. stdout: {} stderr: {}",
        outcome_a.stdout, outcome_a.stderr
    );
    // EVERY row of the 9-author survey carries [verified]; NO [unverified]/
    // unknown-signature row exists (I-AV-1 universal-marker construction gate).
    assert_verified_marker_is_universal(&outcome_a.stdout);

    // -- Corpus B: the 2-author identical-(subject,object) pair — the hardest
    // anti-merging shape (two distinct authors, the SAME deno/dependency-pinning
    // claim). The universal marker must hold on this shape too: BOTH attributed
    // rows carry [verified], neither is rendered [unverified]. --
    let env_b = TestEnv::initialized();
    let indexer_b = seed_network_index(
        &env_b,
        NetworkIndexFixture::DenoDependencyPinningTwoUnfollowedAuthors,
    );
    let outcome_b = run_openlore_search(
        &env_b,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.dependency-pinning",
        ],
        &indexer_b,
    );
    assert_eq!(
        outcome_b.status, 0,
        "`openlore search --object dependency-pinning` must exit 0. stdout: {} stderr: {}",
        outcome_b.stdout, outcome_b.stderr
    );
    assert_verified_marker_is_universal(&outcome_b.stdout);
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus (9 authors /
    // 7 subjects) — the index HAS `org.openlore.philosophy.reproducible-builds`
    // claims but NONE for the typo `…reproducable-builds`. The CLI's indexer_url
    // points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: query the TYPO'd philosophy URI through the CLI driving port (a
    // single substituted character — `reproducable` for `reproducible`). The index
    // returns ZERO matches for the typo. --
    let typo = "org.openlore.philosophy.reproducable-builds";
    let outcome = run_openlore_search(&env, &["search", "--object", typo], &indexer);

    // The empty result is a VALID not-yet-found state, NOT an error: exit 0
    // (US-AV-002 Ex4 — distinct from the `--show`-absent-cid usage error, AV-24).
    assert_eq!(
        outcome.status, 0,
        "AV-12: an empty `--object` result is a valid not-yet-found state and MUST \
         exit 0 (US-AV-002 Ex4). stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The empty result NAMES the queried object AND offers the closest KNOWN object
    // as a near-match suggestion — `near_match_suggestion` (AVC-8) ranks the known
    // network objects (gathered from the index) and the closest is the correctly
    // spelled `…reproducible-builds`.
    assert_empty_with_near_match_suggestion(
        &outcome.stdout,
        typo,
        "org.openlore.philosophy.reproducible-builds",
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
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.reproducible-builds";

    // -- Precondition: the discovery indexer is UNREACHABLE — `OPENLORE_INDEXER_URL`
    // points at a CLOSED localhost port (bound then dropped; connect refused). The
    // user's OWN PDS stays reachable (the authoring/publish path needs it). If the
    // CLI hard-probed the indexer at startup, `claim add` would fail — the cardinal
    // WD-116 disprover. --
    let closed = ClosedIndexerPort::reserve();

    // === Sub-assertion 1: `openlore claim add` succeeds (exit 0; composed/stored
    // LOCALLY) with the indexer UNREACHABLE — the indexer is NOT probed at CLI
    // startup (WD-116). `\n` confirms the sign prompt; `N` declines publishing so
    // the claim is persisted LOCALLY without an outbound publish yet (the publish
    // is sub-assertion 2). ===
    let claim_add = run_openlore_unreachable_indexer(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            "github:rust-lang/rust",
            "--predicate",
            "embodiesPhilosophy",
            "--object",
            object,
            "--evidence",
            "https://www.rust-lang.org/",
            "--confidence",
            "0.86",
        ],
        &closed,
        "\nN\n",
    );
    assert_eq!(
        claim_add.status, 0,
        "KPI-5 (sub-1): `openlore claim add` MUST exit 0 with the indexer UNREACHABLE \
         (the indexer is NOT probed at CLI startup, WD-116). stdout: {} stderr: {}",
        claim_add.stdout, claim_add.stderr
    );
    // The claim landed in the LOCAL store (the authoring verb mutated it). Capture
    // the local file set so sub-assertion 4 can prove `search` never mutates it.
    let local_after_add = local_claim_file_set(&env);
    assert!(
        !local_after_add.is_empty(),
        "KPI-5 (sub-1): `claim add` must have composed/stored the claim LOCALLY (the \
         claims dir is non-empty); stdout: {}",
        claim_add.stdout
    );
    // The CID the authoring verb signed (printed in `Computing claim CID <cid>`) —
    // published next.
    let cid = claim_add_cid_from_stdout(&claim_add.stdout);

    // === Sub-assertion 2: offline `openlore claim publish <cid>` succeeds (exit 0).
    // "Offline" = the discovery indexer is down; publish posts to the user's OWN PDS
    // (reachable), proving an unreachable indexer never blocks publish (WD-116). ===
    let claim_publish =
        run_openlore_unreachable_indexer(&env, &["claim", "publish", &cid], &closed, "");
    assert_eq!(
        claim_publish.status, 0,
        "KPI-5 (sub-2): offline `openlore claim publish {cid}` MUST exit 0 with the \
         indexer UNREACHABLE (publish goes to the user's own PDS; the indexer is not \
         in the publish path). stdout: {} stderr: {}",
        claim_publish.stdout, claim_publish.stderr
    );

    // === Sub-assertion 3: `openlore graph query --object` succeeds (exit 0, LOCAL
    // graph) with the indexer UNREACHABLE — the LOCAL read path links no indexer
    // code. ===
    let graph_query = run_openlore_unreachable_indexer(
        &env,
        &["graph", "query", "--object", object],
        &closed,
        "",
    );
    assert_eq!(
        graph_query.status, 0,
        "KPI-5 (sub-3): `openlore graph query --object {object}` MUST exit 0 (LOCAL \
         graph) with the indexer UNREACHABLE. stdout: {} stderr: {}",
        graph_query.stdout, graph_query.stderr
    );

    // === Sub-assertion 4: `openlore search --object` prints a clear local-only
    // message pointing at `graph query`, exits NON-fatally (the soft Unreachable
    // outcome, ADR-027/WD-116), and does NOT hang (bounded wall-clock — a connect
    // timeout, not an indefinite block). ===
    let bounded = run_openlore_search_bounded_unreachable(
        &env,
        &["search", "--object", object],
        &closed,
        std::time::Duration::from_secs(30),
    );
    // search.hung == false: the adapter's bounded connect/request timeout returns
    // `Unreachable` promptly (a refused/closed port resolves in well under the
    // 30s bound). A hang here is the KPI-5 / WD-116 violation AV-13 disproves.
    assert!(
        !bounded.hung,
        "KPI-5 (sub-4): `openlore search` MUST NOT hang against an unreachable indexer \
         (bounded wall-clock). stderr: {}",
        bounded.outcome.stderr
    );
    let search = bounded.outcome;
    // search.exit_code is NON-fatal (the soft Unreachable contract is exit 0).
    assert_eq!(
        search.status, 0,
        "KPI-5 (sub-4): `openlore search --object` MUST exit NON-fatally (soft \
         Unreachable, ADR-027/WD-116). stdout: {} stderr: {}",
        search.stdout, search.stderr
    );
    // The clear local-only message + the `graph query` pointer (Q-DELIVER-AV-7: the
    // degraded mode POINTS to the local graph query — the simplest contract).
    assert!(
        search.stdout.contains("Network index unavailable"),
        "KPI-5 (sub-4): `search` must print a clear 'Network index unavailable' \
         local-only message:\n{}",
        search.stdout
    );
    assert!(
        search
            .stdout
            .contains(&format!("openlore graph query --object {object}")),
        "KPI-5 (sub-4): the soft-degradation message must POINT at the LOCAL \
         `openlore graph query --object {object}`:\n{}",
        search.stdout
    );

    // === The local store is mutated ONLY by the authoring verbs, never by search:
    // the local claims file set is UNCHANGED across the `search` invocation (search
    // is a read-only DISCOVERY verb; it links no indexer store/ingest code and
    // touches no local store). ===
    let local_after_search = local_claim_file_set(&env);
    assert_eq!(
        local_after_search, local_after_add,
        "KPI-5: the LOCAL claim store must be mutated ONLY by the authoring verbs, \
         never by `search` — the file set changed across the search invocation \
         (before: {local_after_add:?}, after: {local_after_search:?})"
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a REAL `openlore-indexer serve` bound to an
    // EPHEMERAL localhost port (`:0`, read back — DEVOPS open-q 8, parallel-safe)
    // over an index.duckdb seeded with the US-AV-002 Ex1 reproducible-builds corpus
    // (9 authors / 7 subjects). The CLI's indexer_url points at the serve port. The
    // transport is the production B1 CLI<->indexer localhost HTTP/XRPC path. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: the object-dimension network read through the CLI driving port,
    // over the REAL localhost transport. --
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );

    // 1. exit 0 (a valid network result over the real transport, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "B1 (AV-14): `openlore search --object` over the REAL localhost serve port \
         must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // 2 + 3. The B1 RELEASE GATE: the transport REACHED the localhost serve port (a
    // result was returned, NOT the SOFT `Unreachable` local-only degradation), the
    // rendered result is NON-EMPTY + ATTRIBUTED (every row carries author_did +
    // [verified]), and the wire carried per-result author_did end-to-end (D-D36 /
    // WD-115 — a response dropping it is an anti-merging violation across the
    // transport the client's `BadResponse` arm would catch).
    assert_transport_reached_serve_port(&outcome.stdout);

    // The wire preserved per-result author_did for the headline corpus — the
    // attributed rows for (bazel, reproducible-builds) survive the transport, with
    // both the unfollowed Priya (0.82) and the subscribed-corpus Rachel present, and
    // NO merged/consensus row (anti-merging across the B1 transport, D-D36).
    assert_network_result_preserves_attribution(
        &outcome.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        9,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:rachel-test#org.openlore.application",
        ],
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-003 Ex1 contributor-trail corpus —
    // did:plc:priya-test authors 8 verified claims across 6 subjects (bazel x2,
    // buck2, nixpkgs x2, pants, please, ninja). Maria does NOT follow her. The
    // CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);

    // -- Action: the contributor-dimension network read through the CLI driving
    // port. `github:priya` resolves to did:plc:priya-test (slice-02/04 handle->DID)
    // → the priya app identity the corpus author_did carries. --
    let outcome = run_openlore_search(&env, &["search", "--contributor", "github:priya"], &indexer);

    // 1. exit 0 + the public-data banner PRECEDES the results.
    assert_eq!(
        outcome.status, 0,
        "`openlore search --contributor` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );
    assert_public_data_banner_precedes_results(&outcome.stdout);

    // 2. all 8 verified claims listed under the ONE author DID
    // (did:plc:priya-test#org.openlore.application), each [verified], NO merged row
    // (a single-author trail by construction, but the anti-merging row count + the
    // no-consensus footer are the load-bearing assertions). The contributor trail
    // covers 6 distinct subjects; the (subject, object) anti-merging helper counts
    // ALL attributed rows and asserts the TOTAL attributed-row count (8) plus that
    // priya is the sole author.
    assert_network_result_preserves_attribution(
        &outcome.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        8,
        &["did:plc:priya-test#org.openlore.application"],
    );

    // 3. every row carries [verified] (verification is an ingest precondition;
    //    I-AV-1) — the universal-marker guarantee holds on the contributor trail.
    assert_verified_marker_is_universal(&outcome.stdout);

    // 4. the trail is labeled "(not subscribed)" — Maria does NOT follow Priya.
    assert!(
        outcome
            .stdout
            .contains("did:plc:priya-test#org.openlore.application (not subscribed)"),
        "expected the unfollowed contributor labeled (not subscribed):\n{}",
        outcome.stdout
    );

    // 5. the footer states this is ONE developer's reasoning trail, NOT a community
    //    consensus (KPI-AV-1 honesty — a network trail is not a consensus), and
    //    offers `openlore peer add did:plc:priya-test` (the slice-03 follow path).
    assert!(
        outcome
            .stdout
            .contains("one developer's reasoning trail, not a community consensus"),
        "expected the honest-framing footer (a trail, NOT a consensus):\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("openlore peer add did:plc:priya-test"),
        "expected the `openlore peer add did:plc:priya-test` follow offer:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-003 Ex2 subject-survey corpus —
    // github:bazelbuild/bazel surveyed by 5 DISTINCT network authors (priya,
    // sven, tobias, aanya, lena), each asserting a distinct philosophy with its
    // own confidence. The CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::BazelFiveDistinctAuthors);

    // -- Action: the subject-dimension network read through the CLI driving port.
    // `github:bazelbuild/bazel` is a PROJECT URI matched against the indexed
    // `subject` column EXACTLY (no handle→DID resolution — a subject is the
    // project, not an author). --
    let outcome = run_openlore_search(
        &env,
        &["search", "--subject", "github:bazelbuild/bazel"],
        &indexer,
    );

    // 1. exit 0 + the public-data banner PRECEDES the results.
    assert_eq!(
        outcome.status, 0,
        "`openlore search --subject` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );
    assert_public_data_banner_precedes_results(&outcome.stdout);

    // 2. claims grouped by author — 5 DISTINCT author groups, each attributed to
    //    its own author DID, each with philosophy/confidence/cid/[verified], and
    //    NO row collapses multiple authors into a "the network thinks X" merged
    //    consensus entry (the subject-dimension anti-merging render, KPI-AV-2 /
    //    I-AV-2; the pure composition is AVC-2). The helper counts ALL attributed
    //    `author_did:` rows for the surveyed subject (5 — one per distinct author)
    //    and asserts NO merged/consensus row anywhere; the footer's distinct-author
    //    COUNT line is excluded by construction (it never starts with `author_did:`).
    assert_network_result_preserves_attribution(
        &outcome.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        5,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:sven-test#org.openlore.application",
            "did:plc:tobias-test#org.openlore.application",
            "did:plc:aanya-test#org.openlore.application",
            "did:plc:lena-test#org.openlore.application",
        ],
    );

    // 3. every row carries [verified] (verification is an ingest precondition;
    //    I-AV-1) — the universal-marker guarantee holds on the subject survey too.
    assert_verified_marker_is_universal(&outcome.stdout);

    // 4. the footer states the distinct-author COUNT (5) + the no-merge guarantee
    //    (a COUNT over attributed rows, NOT a merge) — the subject dimension reuses
    //    the OBJECT survey's no-merge footer (dimension-aware render).
    assert!(
        outcome.stdout.contains("5 distinct author(s)."),
        "expected the footer to state the distinct-author count (5):\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("No claims are merged."),
        "expected the footer's no-merge guarantee:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-003 Ex1 contributor-trail corpus — Priya's
    // 8 verified claims across 6 subjects. The index has NO claims authored by
    // github:nobody-here (resolves to did:plc:nobody-here-test, an author absent
    // from the corpus). The CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);

    // -- Action: query a contributor handle ABSENT from the index through the CLI
    // driving port. `github:nobody-here` resolves to
    // did:plc:nobody-here-test#org.openlore.application (slice-02/04 handle→DID
    // convention), which authors ZERO claims in the seeded index. --
    let outcome = run_openlore_search(
        &env,
        &["search", "--contributor", "github:nobody-here"],
        &indexer,
    );

    // The empty result is a VALID not-yet-found state, NOT an error: exit 0
    // (US-AV-003 Ex3 — mirrors AV-12 for the contributor dimension).
    assert_eq!(
        outcome.status, 0,
        "AV-17: an empty `--contributor` result is a valid empty state and MUST \
         exit 0 (US-AV-003 Ex3). stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The empty result NAMES the queried contributor and offers NO near-match
    // suggestion (a contributor is absent, not a typo — distinct from AV-12's
    // object-dimension "Did you mean <near>?" line).
    assert_empty_contributor_message(&outcome.stdout, "github:nobody-here");
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
    let env = TestEnv::initialized();

    // -- Precondition (relationship): Maria SUBSCRIBES to Rachel
    // (did:plc:rachel-test) via the slice-03 `peer add` (resolved against a
    // PeerPds double), the SAME path AV-8 / slice-03 use. The subscription is
    // persisted under OPENLORE_HOME, so the later `search` (sharing the same env
    // home) resolves the relationship CLI-side against the real
    // peer_subscriptions state — the index is per-user-neutral (data-models.md). --
    let rachel_did = "did:plc:rachel-test";
    let rachel_peer = PeerPds::for_peer(rachel_did, Vec::new());
    let peer_add = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", rachel_did],
        rachel_did,
        rachel_peer.endpoint_url(),
    );
    assert_eq!(
        peer_add.status, 0,
        "precondition: `openlore peer add {rachel_did}` must exit 0. stdout: {} stderr: {}",
        peer_add.stdout, peer_add.stderr
    );

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with Rachel's verified network claims (US-AV-003 Ex4 /
    // US-AV-005 Ex2). The index is per-user-neutral — it carries NO relationship
    // state; the (subscribed peer) label is a CLI-side projection of Maria's own
    // peer_subscriptions established above. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::IncludesAlreadyFollowedRachel);

    // -- Action: the contributor-dimension network read through the CLI driving
    // port. `github:rachel` resolves to did:plc:rachel-test (slice-02/04 handle→DID)
    // → the rachel app identity the corpus author_did carries. --
    let outcome = run_openlore_search(&env, &["search", "--contributor", "github:rachel"], &indexer);

    // exit 0 (a valid network result, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "`openlore search --contributor github:rachel` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // 1. Rachel's network claims are labeled "(subscribed peer)" — NOT
    //    "(not subscribed)". The relationship resolves CLI-side against Maria's
    //    peer_subscriptions (the subscription established above), proving the
    //    slice-03 relationship labeling is preserved even in network search.
    assert!(
        outcome
            .stdout
            .contains("did:plc:rachel-test#org.openlore.application (subscribed peer)"),
        "expected the already-followed Rachel labeled (subscribed peer):\n{}",
        outcome.stdout
    );
    // The cardinal AV-18 disprover: a genuinely-subscribed author is NEVER mislabeled
    // "(not subscribed)" in network search (the relationship resolution must read the
    // REAL peer_subscriptions, not default everyone to unfollowed).
    assert!(
        !outcome
            .stdout
            .contains("did:plc:rachel-test#org.openlore.application (not subscribed)"),
        "AV-18: a subscribed author must NOT be mislabeled (not subscribed):\n{}",
        outcome.stdout
    );

    // 2. Every claim retains its author DID + the [verified] marker (verification is
    //    an ingest precondition; I-AV-1) — the universal-marker guarantee holds on a
    //    subscribed author's trail too.
    assert_verified_marker_is_universal(&outcome.stdout);

    // 3. NO "Follow this author" affordance is shown for Rachel — she is ALREADY
    //    followed, so the discovery→federation funnel affordance (which reuses the
    //    slice-03 `peer add` verbatim, I-AV-7) is suppressed for her (chains into
    //    AV-21). The affordance appears ONLY for unfollowed (NetworkUnfollowed)
    //    authors; a subscribed peer would make it redundant.
    assert!(
        !outcome
            .stdout
            .contains("Follow this author: openlore peer add did:plc:rachel-test"),
        "AV-18: NO redundant follow affordance for the already-followed Rachel:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();
    // The bare DID the slice-03 `peer add`/`pull`/local-graph paths use; the
    // NETWORK search renders the app-identity form (`<did>#org.openlore.application`)
    // but the LOCAL `peer_claims` store + `graph query` use the bare DID
    // (`adapter-duckdb::peer_storage::bare_did`, I-FED-2 / GQE-6).
    let priya_did = "did:plc:priya-test";

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus (9 authors
    // / 7 subjects), which includes the UNFOLLOWED Priya's verified bazel/
    // reproducible-builds claim (0.82). The CLI's indexer_url points at the serve
    // port. NO `peer add` precedes the search, so Priya is unfollowed → her result
    // carries the render-only follow affordance. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // === Step 1: discover Priya via `openlore search --object reproducible-builds`.
    // Her result row ends with the RENDER-ONLY affordance
    // `Follow this author: openlore peer add did:plc:priya-test` (the EXISTING
    // slice-03 command, bare DID; I-AV-7). ===
    let discovered = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );
    assert_eq!(
        discovered.status, 0,
        "AV-19: the discovery `search --object reproducible-builds` must exit 0. \
         stdout: {} stderr: {}",
        discovered.stdout, discovered.stderr
    );
    // 1. The result for the unfollowed Priya ends with the EXACT slice-03 command
    //    (bare DID) — a render-only hint, no executable follow path (criterion 1 /
    //    I-AV-7). The funnel runs THIS string verbatim below.
    assert!(
        discovered
            .stdout
            .contains("Follow this author: openlore peer add did:plc:priya-test"),
        "AV-19: the unfollowed Priya's result must end with the verbatim slice-03 \
         follow affordance `Follow this author: openlore peer add did:plc:priya-test`:\n{}",
        discovered.stdout
    );

    // === Steps 2 + 3: run THAT slice-03 command verbatim (NO new verb) against a
    // slice-03 `PeerPds` double serving Priya's SAME claim, then `openlore peer
    // pull` — the discovery→federation funnel REUSES the slice-03 path verbatim
    // (criterion 1/2/3). The affordance is parsed out of the rendered stdout and
    // its argv run UNCHANGED. ===
    let _priya_peer = funnel_follow_and_pull(&env, &discovered.stdout, priya_did);

    // === Step 4: `openlore graph query --contributor did:plc:priya-test` — Priya's
    // pulled claim is now in Maria's LOCAL graph (criterion 1). The local graph
    // keys peer claims by the BARE DID (GQE-6), so query + assert the bare form. ===
    let local_trail = run_openlore(&env, &["graph", "query", "--contributor", priya_did]);
    assert_eq!(
        local_trail.status, 0,
        "AV-19: `graph query --contributor` must exit 0 after the funnel pull. \
         stdout: {} stderr: {}",
        local_trail.stdout, local_trail.stderr
    );
    // Priya's bazel/reproducible-builds claim surfaces in the LOCAL graph,
    // attributed to her DID (the SAME record the index ingested + the PeerPds
    // served — the funnel strengthened the local-first graph; KPI-AV-4).
    assert!(
        local_trail
            .stdout
            .contains(&format!("author_did: {priya_did}")),
        "AV-19: after following + pulling, Priya's claim must appear in the LOCAL \
         `graph query --contributor` trail attributed to {priya_did}:\n{}",
        local_trail.stdout
    );
    assert!(
        local_trail.stdout.contains("github:bazelbuild/bazel")
            && local_trail
                .stdout
                .contains("org.openlore.philosophy.reproducible-builds"),
        "AV-19: Priya's pulled bazel/reproducible-builds claim must be in the LOCAL \
         graph (the funnel surfaced the network claim locally):\n{}",
        local_trail.stdout
    );

    // 2. Priya's claim participates in the LOCAL graph EXACTLY like any pulled peer
    //    — the affordance is a RENDER-ONLY hint reusing the slice-03 path (no
    //    parallel subscription path; I-AV-7). The cardinal "like any peer" proof:
    //    after the funnel pull her claim is an ORDINARY `peer_claims` row (the SAME
    //    store + attribution a slice-03 `peer add`/`pull` writes for Rachel in
    //    GQE-6), with NO claim filed under any other DID (anti-merging, I-FED-1).
    //    The weighted/traverse explorer scopes (slice-04) read this SAME
    //    `peer_claims` store, so a row indistinguishable from a slice-03 pull
    //    participates in them exactly like any pulled peer.
    assert_peer_claims_attributed_to(&env, priya_did, 1);
    // She also surfaces in the LOCAL graph attributed to her bare DID across BOTH
    // her subject + object (criterion 1/2: the network claim is now a first-class
    // local claim, queryable like any pulled peer's).
    assert!(
        local_trail.stdout.contains("subject:")
            && local_trail
                .stdout
                .contains("org.openlore.philosophy.reproducible-builds"),
        "AV-19: Priya's pulled claim must render as a normal local trail row \
         (subject + object), participating like any pulled peer (criterion 2):\n{}",
        local_trail.stdout
    );

    // 3. The subscription appears EXACTLY as a slice-03 add — one ACTIVE
    //    `peer_subscriptions` row, no parallel discovery-subscription state
    //    (I-AV-7). Indistinguishable from a plain slice-03 `peer add`: the funnel
    //    introduced no separate follow store to leak (proven by AV-22's purge).
    assert_funnel_subscription_is_slice03(&env, priya_did);
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus (9 authors /
    // 7 subjects), which carries several UNFOLLOWED authors (incl. Priya on bazel,
    // 0.82) — each rendered with the render-only follow affordance. NO `peer add`
    // precedes (or follows) the discovery, so the affordance is the ONLY follow
    // surface, and it must remain a hint that never executes a follow (I-AV-7). The
    // CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Snapshot the FULL `peer_subscriptions` state (what `peer list` renders)
    // BEFORE any discovery. With NO prior `peer add`, the baseline is EMPTY — the
    // strongest form of "no subscription created by search". --
    let subscriptions_before = peer_subscriptions_snapshot(&env);

    // === Action: run SEVERAL `openlore search` queries across dimensions (object,
    // contributor, subject) + a `--show` on a listed cid — the full discovery +
    // inspect surface — WITHOUT ever running `peer add`. ===

    // (1) The OBJECT dimension over the headline corpus — surfaces unfollowed
    // authors each carrying the render-only follow affordance.
    let object_search = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );
    assert_eq!(
        object_search.status, 0,
        "AV-20: `search --object` must exit 0. stdout: {} stderr: {}",
        object_search.stdout, object_search.stderr
    );
    // The render-only affordance IS present for an unfollowed author (the funnel
    // hint), confirming discovery surfaces the follow command — which the snapshot
    // delta below proves is NEVER auto-executed (I-AV-7).
    assert!(
        object_search
            .stdout
            .contains("Follow this author: openlore peer add"),
        "AV-20: the discovery result must surface the render-only follow affordance \
         (the funnel hint that must NOT auto-execute):\n{}",
        object_search.stdout
    );

    // (2) The CONTRIBUTOR dimension — one developer's network trail (also unfollowed).
    let contributor_search =
        run_openlore_search(&env, &["search", "--contributor", "github:priya"], &indexer);
    assert_eq!(
        contributor_search.status, 0,
        "AV-20: `search --contributor` must exit 0. stdout: {} stderr: {}",
        contributor_search.stdout, contributor_search.stderr
    );

    // (3) The SUBJECT dimension — a project survey across authors.
    let subject_search = run_openlore_search(
        &env,
        &["search", "--subject", "github:bazelbuild/bazel"],
        &indexer,
    );
    assert_eq!(
        subject_search.status, 0,
        "AV-20: `search --subject` must exit 0. stdout: {} stderr: {}",
        subject_search.stdout, subject_search.stderr
    );

    // (4) Inspect a listed result via `--show` (the trust-display read path) — a cid
    // captured from the object-dimension result set (US-AV-004 read-only inspect).
    let priya_bazel_cid = cid_from_search_row(
        &object_search.stdout,
        "did:plc:priya-test",
        "github:bazelbuild/bazel",
    );
    let shown = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
            "--show",
            &priya_bazel_cid,
        ],
        &indexer,
    );
    assert_eq!(
        shown.status, 0,
        "AV-20: `search --object ... --show <listed cid>` must exit 0. stdout: {} \
         stderr: {}",
        shown.stdout, shown.stderr
    );

    // -- Snapshot the `peer_subscriptions` state AFTER all the discovery + inspect
    // actions. --
    let subscriptions_after = peer_subscriptions_snapshot(&env);

    // === The cardinal AV-20 gate (US-AV-005 Ex3 / I-AV-7): the subscription state
    // (`peer list` / peer_subscriptions) is BYTE-IDENTICAL before and after — NO
    // subscription was created by ANY search dimension or by `--show`. Discovery is
    // read-only; following is always an explicit, separate human action; the
    // render-only affordance never executes a follow. ===
    assert_subscriptions_unchanged(&subscriptions_before, &subscriptions_after);
    // The strongest form of the gate on a fresh env: the baseline was empty and
    // STAYS empty — discovery created ZERO subscriptions.
    assert!(
        subscriptions_after.is_empty(),
        "AV-20: with NO `peer add`, discovery must leave peer_subscriptions EMPTY \
         (zero auto-follows); got: {subscriptions_after:?}"
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
    let env = TestEnv::initialized();

    // -- Precondition (relationship): Tobias already SUBSCRIBES to Rachel
    // (did:plc:rachel-test) via the slice-03 `peer add` (resolved against a
    // PeerPds double — the SAME path AV-8 / AV-18 / slice-03 use). The
    // subscription is persisted under OPENLORE_HOME, so the later `search`
    // (sharing the same env home) resolves the relationship CLI-side against the
    // REAL peer_subscriptions state — the index is per-user-neutral. Every OTHER
    // corpus author (incl. the unfollowed Priya) stays unfollowed. --
    let rachel_did = "did:plc:rachel-test";
    let rachel_peer = PeerPds::for_peer(rachel_did, Vec::new());
    let peer_add = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", rachel_did],
        rachel_did,
        rachel_peer.endpoint_url(),
    );
    assert_eq!(
        peer_add.status, 0,
        "precondition: `openlore peer add {rachel_did}` must exit 0. stdout: {} stderr: {}",
        peer_add.stdout, peer_add.stderr
    );

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus — the
    // MIXED-relationship shape the per-row conditioning must handle: the SAME
    // `--object reproducible-builds` query returns BOTH the SUBSCRIBED Rachel
    // (did:plc:rachel-test, nixpkgs 0.88) AND the UNFOLLOWED Priya
    // (did:plc:priya-test, bazel 0.82). The index carries NO relationship state;
    // the labels are a CLI-side projection of Tobias's own peer_subscriptions
    // established above. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: the object-dimension network read through the CLI driving port —
    // one result containing BOTH the subscribed peer + the unfollowed author. --
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );

    // exit 0 (a valid network result, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "AV-21: `openlore search --object reproducible-builds` must exit 0. \
         stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // Criterion 1: Rachel's row is labeled "(subscribed peer)" (NOT
    // "(not subscribed)") — the relationship resolves CLI-side against Tobias's
    // peer_subscriptions, even in a MIXED result.
    assert!(
        outcome
            .stdout
            .contains("did:plc:rachel-test#org.openlore.application (subscribed peer)"),
        "AV-21: the already-followed Rachel must be labeled (subscribed peer) in a \
         mixed result:\n{}",
        outcome.stdout
    );

    // Criterion 1 (cardinal): the SUBSCRIBED Rachel carries NO redundant
    // "Follow this author" affordance — the funnel affordance is SUPPRESSED for a
    // subscribed peer (per-row conditioning on the relationship label; I-AV-7).
    assert!(
        !outcome
            .stdout
            .contains("Follow this author: openlore peer add did:plc:rachel-test"),
        "AV-21: NO redundant follow affordance for the subscribed Rachel (the \
         affordance must be suppressed for a subscribed peer):\n{}",
        outcome.stdout
    );

    // Criterion 2 (the MIXED-case disprover): the UNFOLLOWED Priya — in the SAME
    // result — IS labeled "(not subscribed)" and DOES carry the follow affordance.
    // Per-row conditioning: the affordance follows the relationship label, so one
    // result can suppress it for the subscribed peer while emitting it for the
    // unfollowed author.
    assert!(
        outcome
            .stdout
            .contains("did:plc:priya-test#org.openlore.application (not subscribed)"),
        "AV-21: the unfollowed Priya must be labeled (not subscribed) in the same \
         mixed result:\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("Follow this author: openlore peer add did:plc:priya-test"),
        "AV-21: the unfollowed Priya's row MUST carry the follow affordance (the \
         affordance is conditioned per-row on the relationship — present for the \
         unfollowed author, absent for the subscribed peer):\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();
    // The bare DID the slice-03 `peer add`/`pull`/`remove --purge` paths key on
    // (the network search renders the app-identity form, but the LOCAL
    // peer_subscriptions + peer_claims store + the purge verb use the bare DID;
    // I-FED-2 / GQE-6 / Q-DELIVER-2).
    let priya_did = "did:plc:priya-test";

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus (9 authors
    // / 7 subjects), which includes the UNFOLLOWED Priya's verified bazel/
    // reproducible-builds claim (0.82). The CLI's indexer_url points at the serve
    // port. NO `peer add` precedes the search, so Priya is unfollowed → her result
    // carries the render-only follow affordance the funnel runs verbatim. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // === Precondition (the AV-19 funnel): discover Priya, follow her via the
    // render-only affordance (the slice-03 `peer add` VERBATIM), and `peer pull`
    // her claim into the LOCAL `peer_claims` store. This is the SAME `peer add` +
    // `peer pull` path slice-03 uses — the funnel introduces NO parallel
    // subscription state (I-AV-7); AV-22 proves that by purging via the slice-03
    // verb and observing zero residue. ===
    let discovered = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );
    assert_eq!(
        discovered.status, 0,
        "AV-22 (precondition): the discovery `search --object reproducible-builds` \
         must exit 0. stdout: {} stderr: {}",
        discovered.stdout, discovered.stderr
    );
    // Reuse the 05-06 funnel helper: parse the render-only `peer add` affordance,
    // run it verbatim against a slice-03 PeerPds double, then `peer pull` so
    // Priya's claim lands in the LOCAL `peer_claims` store. Keep the PeerPds alive
    // through the rest of the scenario (the purge reads only the local store).
    let _priya_peer = funnel_follow_and_pull(&env, &discovered.stdout, priya_did);

    // Precondition assertions: the funnel created EXACTLY a slice-03 subscription
    // (one ACTIVE peer_subscriptions row, no parallel discovery-side record) AND
    // Priya's pulled claim is in the LOCAL peer_claims store attributed to her —
    // the state the slice-03 `peer remove --purge` must reduce to zero residue.
    assert_funnel_subscription_is_slice03(&env, priya_did);
    assert_peer_claims_attributed_to(&env, priya_did, 1);

    // DD-FED-10: capture the FULL observable purge universe BEFORE the action —
    // the SAME four-slot universe slice-03 PS-6 asserts (peer_claims row count,
    // author_claims row count, the on-disk peer_claims/<did>/ partition, the
    // subscription row count). The before-state is a followed-via-discovery
    // author: 1 peer_claims row, the partition exists, 1 active subscription.
    let before = capture_purge_universe(&env, priya_did);

    // -- Action: the slice-03 `peer remove <did> --purge` verb VERBATIM. The
    // interactive `[y/N]` confirmation is answered "y" via piped stdin (scripted
    // mode — exactly as slice-03 PS-6 drives the hard-purge branch; WD-21: a real
    // confirmation, no `--yes`). Because the author was followed via the SAME
    // `peer add` path, this purge is byte-indistinguishable from a slice-03
    // add+purge. --
    let outcome = run_openlore_with_stdin(&env, &["peer", "remove", priya_did, "--purge"], "y\n");
    assert_eq!(
        outcome.status, 0,
        "AV-22: `openlore peer remove {priya_did} --purge` (the slice-03 verb \
         verbatim) must exit 0 for a discovery-followed author. stdout: {} \
         stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // === The cardinal AV-22 gate (US-AV-005 / I-AV-7 / slice-03 PS-6): the purge
    // leaves ZERO residue over the SAME state-delta universe slice-03 PS-6 asserts
    // for a plain add+purge —
    //   - peer_storage.claims.row_count_by_author[did] : 1 → 0 (peer claims gone)
    //   - author_claims.row_count                      : UNCHANGED (no user
    //     counter-claims existed; the implicit-unchanged rule still pins it)
    //   - filesystem.peer_claims_dir.exists[did]       : true → false (the on-disk
    //     partition removed — zero filesystem residue)
    //   - peer_storage.subscriptions.row_count[did]    : 1 → 0 (the subscription
    //     hard-deleted; NO orphaned discovery-side record because none exists)
    // Because the funnel added NO parallel subscription state, the slice-03 purge
    // semantics apply UNCHANGED and the after-universe is identical to a slice-03
    // add+purge. ===
    let after = capture_purge_universe(&env, priya_did);
    assert_purge_state_delta(&before, &after);

    // Belt-and-suspenders against the named single-slot helpers (each observable
    // read independently — the SAME assertions slice-03 PS-6 makes):
    //   1. NO peer_claims residue (the pulled claim is gone).
    assert_no_peer_claims_attributed_to(&env, priya_did);
    //   2. the on-disk peer_claims/<priya>/ partition is removed (KPI-FED-4 zero
    //      filesystem residue).
    assert_peer_claims_dir_removed_for(&env, priya_did);
    //   3. ZERO subscriptions remain — and, the load-bearing absence: there is NO
    //      orphaned discovery-side subscription record (none was ever created; the
    //      funnel reused the slice-03 store, so the slice-03 purge removed the ONE
    //      row and left NOTHING behind — I-AV-7). The full `peer_subscriptions`
    //      snapshot (what `peer list` renders) is EMPTY, the strongest form of
    //      "no parallel discovery-subscription record leaked".
    let subscriptions_after_purge = peer_subscriptions_snapshot(&env);
    assert!(
        subscriptions_after_purge.is_empty(),
        "AV-22 (I-AV-7): after the slice-03 `peer remove --purge`, the \
         peer_subscriptions snapshot (`peer list`) must be EMPTY — NO orphaned \
         discovery-side subscription record may remain (the funnel created NO \
         parallel path; the slice-03 purge left zero residue); got: \
         {subscriptions_after_purge:?}"
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the headline reproducible-builds corpus (9 authors /
    // 7 subjects), which includes Priya's verified bazel/reproducible-builds claim
    // (did:plc:priya-test, UNFOLLOWED, 0.82) — the SAME corpus AV-8 lists. The
    // CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Capture the LOCAL store baseline BEFORE any search/--show so the read-only
    // property (search + --show mutate NO local state) is asserted as a delta. --
    let local_before = local_claim_file_set(&env);

    // === Step 1 (chain off AV-8): list the result set with the object dimension and
    // capture Priya's bazel cid from the rendered result row (the cid the user
    // "--show"s came from a prior search, US-AV-004 Ex1). ===
    let listed = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
        ],
        &indexer,
    );
    assert_eq!(
        listed.status, 0,
        "precondition: the AV-8-style `search --object` must exit 0 to list a cid to \
         --show. stdout: {} stderr: {}",
        listed.stdout, listed.stderr
    );
    let priya_bazel_cid = cid_from_search_row(
        &listed.stdout,
        "did:plc:priya-test",
        "github:bazelbuild/bazel",
    );

    // === Step 2 (the action under test): inspect that listed cid via `--show`. ===
    let shown = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
            "--show",
            &priya_bazel_cid,
        ],
        &indexer,
    );

    // exit 0 (a valid inspection of a listed record, never a fatal).
    assert_eq!(
        shown.status, 0,
        "`openlore search --object ... --show <listed cid>` must exit 0. stdout: {} \
         stderr: {}",
        shown.stdout, shown.stderr
    );

    // The trust-inspection contract (US-AV-004 Ex1 / KPI-AV-3): the FULL record
    // (subject github:bazelbuild/bazel, object reproducible-builds, confidence 0.82,
    // evidence, author did:plc:priya-test) PLUS "Signature: VERIFIED against
    // did:plc:priya-test" AND "CID: <cid> (recomputed, matches published record)".
    // These lines render the SAME pure-core verification result the indexer computed
    // at ingest (the row's verified_against + cid) — no second path, no re-verify.
    assert_show_inspects_verified_record(
        &shown.stdout,
        &priya_bazel_cid,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        "0.82",
        "did:plc:priya-test",
    );

    // === READ-ONLY (US-AV-004): the display creates/signs/mutates nothing — the
    // LOCAL claim store is UNCHANGED across the search + --show invocations (search
    // and --show are read-only DISCOVERY paths; they link no local store/ingest
    // code and touch no local state). ===
    let local_after = local_claim_file_set(&env);
    assert_eq!(
        local_after, local_before,
        "US-AV-004: --show (and the listing search) must be READ-ONLY — the LOCAL \
         claim store must be unchanged (before: {local_before:?}, after: {local_after:?})"
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a REACHABLE localhost `openlore-indexer serve` over
    // an index.duckdb seeded with the headline reproducible-builds corpus (9 authors
    // / 7 subjects) — the SAME corpus AV-8/AV-23 list. The search itself SUCCEEDS
    // (a non-empty result set exists); the `--show`n cid is simply absent FROM that
    // result set, which is what makes this a usage error rather than an empty query.
    // The CLI's indexer_url points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: list the object dimension AND `--show` a cid that is NOT in that
    // result set. `bafy...nothere` is a syntactically-shaped-but-fabricated cid the
    // reproducible-builds corpus never carries (the user typo'd / pasted a stale cid),
    // so the client-side cid filter over the dimension result set finds NO matching
    // row (DESIGN §2: --show reuses the same dimension search + filters client-side). --
    let absent_cid = "bafy...nothere";
    let outcome = run_openlore_search(
        &env,
        &[
            "search",
            "--object",
            "org.openlore.philosophy.reproducible-builds",
            "--show",
            absent_cid,
        ],
        &indexer,
    );

    // The ONE non-zero sad path on the search surface (US-AV-004 Ex4): a NON-ZERO
    // exit + the content-frozen "CID <cid> is not in this search result." usage
    // message + the run-without-`--show` remediation hint — deliberately distinct
    // from the empty-result exit-0 (AV-12/AV-17) so the user can tell a typo'd
    // `--show` from an empty query.
    assert_show_absent_cid_usage_error(&outcome, absent_cid);
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with claim C (Priya, bazel, reproducible-builds, 0.82)
    // AND a later indexed claim K (Sven) that references C with ref_type=counters
    // (both verified). K asserts the SAME object so it co-appears in the search
    // result set — the render reconstructs the counter annotation from K's typed
    // `references` (pointing at C's CID) + K's own author_did. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::CounteredClaimPlusCounter);

    // Recompute the SAME published CIDs the ingest gate computes for C + K (the
    // same-store JOIN keys): the annotation names K's CID; presence is asserted on
    // C's CID. The author DIDs carry the app-identity fragment the indexed
    // `author_did` uses (`did:plc:X#org.openlore.application`).
    let object = "org.openlore.philosophy.reproducible-builds";
    let c_spec = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", object, 0.82);
    let c_cid = c_spec.clone().into_raw_record().published_cid;
    let k_spec = RawRecordSpec::valid(SVEN_DID, "github:bazelbuild/bazel", object, 0.40)
        .with_reference(claim_domain::ReferenceType::Counters, &c_cid.0);
    let k_cid = k_spec.into_raw_record().published_cid;
    let priya_app = format!("{PRIYA_DID}#org.openlore.application");
    let sven_app = format!("{SVEN_DID}#org.openlore.application");

    // -- Action: the object-dimension network read through the CLI driving port. --
    let outcome = run_openlore_search(&env, &["search", "--object", object], &indexer);

    // exit 0 (a valid result, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "AV-25: `openlore search --object` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The OD-AV-7 render gate (I-AV-9): C is STILL present + carries the counter
    // annotation `countered-by <K.cid> (by <K.author>)`; the counter is SHOWN,
    // never applied (NOT filtered/dropped/down-weighted).
    assert_counter_annotation_shown_not_applied(
        &outcome.stdout,
        &c_cid.0,
        &priya_app,
        &k_cid.0,
        &sven_app,
    );

    // Both rows survive (C the countered + K the countering) — the counter never
    // drops a row; both are verified attributed results (anti-merging preserved).
    assert_verified_marker_is_universal(&outcome.stdout);
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
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.reproducible-builds";

    // -- Precondition (index): a reachable localhost `openlore-indexer serve` over
    // an index.duckdb seeded with the headline reproducible-builds corpus. --share
    // is render-only (it encodes the QUERY, never the results), but a reachable
    // indexer pins that the share path emits a link INSTEAD OF running the search
    // — the link must be query-encoding even with a populated index. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- Action: the object-dimension SHARE through the CLI driving port. --
    let outcome = run_openlore_search(&env, &["search", "--object", object, "--share"], &indexer);

    // exit 0 (a valid share, never a fatal).
    assert_eq!(
        outcome.status, 0,
        "`openlore search --object <philosophy> --share` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // 1. The printed link encodes the QUERY dimension+value ONLY — dimension=object,
    //    value=<philosophy> — with NO result payload / snapshot (I-AV-8 / KPI-AV-6).
    let link = parse_and_assert_query_encoding_share_link(&outcome.stdout);
    assert_eq!(
        link.dimension, "object",
        "the share link must encode dimension=object:\n{}",
        outcome.stdout
    );
    assert_eq!(
        link.value, object,
        "the share link must encode value=<the queried philosophy>:\n{}",
        outcome.stdout
    );
    // The exact `Shareable link:` line (criterion 1 — the user-visible affordance).
    assert!(
        outcome.stdout.contains(&format!(
            "Shareable link: openlore://search?object={object}"
        )),
        "expected `Shareable link: openlore://search?object={object}`:\n{}",
        outcome.stdout
    );

    // 2. A line states the link encodes the QUERY, not a frozen snapshot (the
    //    sharing semantics, US-AV-006 Ex1).
    assert!(
        outcome.stdout.contains("encodes the query, not a")
            && outcome.stdout.contains("snapshot"),
        "expected a line stating the link encodes the query, NOT a frozen snapshot:\n{}",
        outcome.stdout
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
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.reproducible-builds";

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the SAME headline reproducible-builds corpus AV-26
    // shared (9 authors / 7 subjects, all unfollowed). The CLI's indexer_url points
    // at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- The ORIGINAL query (the reference Tobias's re-run must match): the
    // object-dimension network read through the CLI driving port. Its attributed
    // per-author rows are the SAME set the shared link must re-derive. --
    let original = run_openlore_search(&env, &["search", "--object", object], &indexer);
    assert_eq!(
        original.status, 0,
        "precondition: the original `openlore search --object` must exit 0. \
         stdout: {} stderr: {}",
        original.stdout, original.stderr
    );

    // -- Chain off AV-26: emit the shareable link (`--share` is render-only, I-AV-8),
    // then PARSE it (asserting it encodes the QUERY only, no snapshot payload). This
    // is the exact `openlore://search?object=...` link Maria shared. --
    let share = run_openlore_search(&env, &["search", "--object", object, "--share"], &indexer);
    assert_eq!(
        share.status, 0,
        "precondition: `openlore search --object <philosophy> --share` must exit 0. \
         stdout: {} stderr: {}",
        share.stdout, share.stderr
    );
    let parsed = parse_and_assert_query_encoding_share_link(&share.stdout);
    assert_eq!(
        parsed.dimension, "object",
        "the shared link must encode dimension=object:\n{}",
        share.stdout
    );
    assert_eq!(
        parsed.value, object,
        "the shared link must encode value=<the queried philosophy>:\n{}",
        share.stdout
    );
    let link = format!("openlore://search?{}={}", parsed.dimension, parsed.value);

    // -- Action: Tobias OPENS the shared link via the CLI re-run resolver
    // (Q-DELIVER-AV-3: `openlore search <link>` — a positional link arg the search
    // verb detects; web AppView OUT of scope per OD-AV-6). The resolver PARSES the
    // link, maps the key to the SearchDimension, and RE-RUNS the SAME dimension
    // search path against the CURRENT index. --
    let resolved = run_openlore_search(&env, &["search", &link], &indexer);

    // exit 0 (a valid re-run, never a fatal).
    assert_eq!(
        resolved.status, 0,
        "AV-27: opening the shared link via the CLI re-run resolver must exit 0. \
         stdout: {} stderr: {}",
        resolved.stdout, resolved.stderr
    );

    // -- Observable outcome (the cardinal AV-27 gate): the encoded query re-ran and
    // returned the SAME per-author-attributed verified results as the original query
    // (same authors, same [verified] marks), anti-merging preserved (NO merged
    // consensus row), with the same `peer add` affordance for unfollowed authors.
    // The link encoded the QUERY (deterministic per AVC-3b), NOT a snapshot — the
    // resolver re-composes per-author rows (anti-merging across the share boundary,
    // I-AV-8/KPI-AV-2). --
    assert_resolved_link_matches_original_query(&original.stdout, &resolved.stdout);

    // The re-run preserved attribution for the headline pair end-to-end: 9 attributed
    // rows, both the unfollowed Priya (0.82) + the (subscribed-corpus) Rachel present,
    // NO merged/consensus row — proven independently of the original-vs-resolved set
    // equality so the anti-merging gate stands on its own (KPI-AV-2 across the share
    // boundary).
    assert_network_result_preserves_attribution(
        &resolved.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        9,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:rachel-test#org.openlore.application",
        ],
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
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.reproducible-builds";

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the SAME headline reproducible-builds corpus AV-26/
    // AV-27 shared (9 authors / 7 subjects, all unfollowed). The CLI's indexer_url
    // points at the serve port. --
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );

    // -- The ORIGINAL query (the pre-ingest reference the re-run must GROW beyond):
    // the object-dimension network read through the CLI driving port. Its 9
    // attributed per-author rows are the baseline the post-ingest re-run extends. --
    let original = run_openlore_search(&env, &["search", "--object", object], &indexer);
    assert_eq!(
        original.status, 0,
        "precondition: the original `openlore search --object` must exit 0. \
         stdout: {} stderr: {}",
        original.stdout, original.stderr
    );

    // -- Maria shares the query link (`--share` is render-only, I-AV-8): emit + PARSE
    // it (asserting it encodes the QUERY only, no snapshot payload). This is the
    // exact `openlore://search?object=...` link she shares. --
    let share = run_openlore_search(&env, &["search", "--object", object, "--share"], &indexer);
    assert_eq!(
        share.status, 0,
        "precondition: `openlore search --object <philosophy> --share` must exit 0. \
         stdout: {} stderr: {}",
        share.stdout, share.stderr
    );
    let parsed = parse_and_assert_query_encoding_share_link(&share.stdout);
    assert_eq!(
        parsed.dimension, "object",
        "the shared link must encode dimension=object:\n{}",
        share.stdout
    );
    assert_eq!(
        parsed.value, object,
        "the shared link must encode value=<the queried philosophy>:\n{}",
        share.stdout
    );
    let link = format!("openlore://search?{}={}", parsed.dimension, parsed.value);

    // -- The INDEX CHANGE: the indexer ingests two MORE verified matching claims
    // (two NEW distinct authors on the SAME object) via a SECOND `openlore-indexer
    // ingest` pass into the SAME index.duckdb, then re-serves it. The helper drops
    // the first serve handle FIRST (releasing the exclusive DuckDB lock), runs the
    // second ingest over the union corpus (the 9 originals — idempotently de-duped
    // by CID — PLUS the 2 new), and re-spawns serve so the re-opened link sees the
    // CURRENT (grown) index. Reuses the Phase-03 `openlore-indexer ingest` pass. --
    let new_authors = [
        "did:plc:author10-test#org.openlore.application",
        "did:plc:author11-test#org.openlore.application",
    ];
    let indexer =
        ingest_more_matching_claims_and_respawn(&env, indexer, av28_two_more_matching_claims());

    // -- Action: Tobias OPENS the SAME shared link AFTER the new ingest, via the CLI
    // re-run resolver (Q-DELIVER-AV-3). The resolver PARSES the link, maps the key to
    // the SearchDimension, and RE-RUNS the SAME dimension search against the CURRENT
    // index — so the two newly-ingested claims now appear. --
    let resolved = run_openlore_search(&env, &["search", &link], &indexer);

    // exit 0 (a valid re-run over the grown index, never a fatal).
    assert_eq!(
        resolved.status, 0,
        "AV-28: re-opening the shared link after the second ingest must exit 0. \
         stdout: {} stderr: {}",
        resolved.stdout, resolved.stderr
    );

    // -- Observable outcome (the cardinal AV-28 gate): the re-opened link RE-RAN the
    // QUERY against the CURRENT index, so the resolved result set GREW by 2 (the two
    // newly-ingested claims now present), each attributed + [verified]; the original
    // 9 attributed rows are PRESERVED; NO merged snapshot collapses authors. A stale
    // frozen snapshot would have kept the original count (no growth) — the growth
    // proves the link encodes the QUERY, not a snapshot (US-AV-006 Ex4 / I-AV-8). --
    assert_resolved_link_grew_to_current_results(
        &original.stdout,
        &resolved.stdout,
        2,
        &new_authors,
    );

    // The re-run preserved attribution for the headline pair end-to-end over the
    // grown index: 11 attributed rows now (the original 9 + the 2 new), the headline
    // unfollowed Priya (0.82) + the (subscribed-corpus) Rachel still present, NO
    // merged/consensus row — the anti-merging gate stands across BOTH the share
    // boundary AND the index change (KPI-AV-2 / I-AV-8).
    assert_network_result_preserves_attribution(
        &resolved.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        11,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:rachel-test#org.openlore.application",
            "did:plc:author10-test#org.openlore.application",
            "did:plc:author11-test#org.openlore.application",
        ],
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
    let env = TestEnv::initialized();

    // -- Precondition (index): a localhost `openlore-indexer serve` over an
    // index.duckdb seeded with the US-AV-003 Ex1 contributor-trail corpus —
    // did:plc:priya-test authors 8 verified claims across 6 subjects (the SAME
    // corpus AV-15 lists). Maria does NOT follow her. The CLI's indexer_url points
    // at the serve port. --
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);

    // -- The REFERENCE trail (what opening the shared link must re-derive): the
    // contributor-dimension network read through the CLI driving port —
    // `github:priya` resolves to did:plc:priya-test, the corpus author_did. Its 8
    // attributed per-author rows + the not-consensus footer are the SAME set the
    // shared link must re-derive. --
    let original = run_openlore_search(
        &env,
        &["search", "--contributor", "github:priya"],
        &indexer,
    );
    assert_eq!(
        original.status, 0,
        "precondition: the original `openlore search --contributor github:priya` must \
         exit 0. stdout: {} stderr: {}",
        original.stdout, original.stderr
    );

    // -- Action (share): the contributor-dimension SHARE through the CLI driving
    // port (`--share` is render-only, I-AV-8 — NO network call). --
    let share = run_openlore_search(
        &env,
        &["search", "--contributor", "github:priya", "--share"],
        &indexer,
    );
    assert_eq!(
        share.status, 0,
        "`openlore search --contributor github:priya --share` must exit 0. \
         stdout: {} stderr: {}",
        share.stdout, share.stderr
    );

    // 1. The printed link encodes the QUERY dimension+value ONLY — dimension=
    //    contributor, value=did:plc:priya-test (the RESOLVED handle->DID, NOT the
    //    handle `github:priya`) — with NO result payload / snapshot (I-AV-8 /
    //    KPI-AV-6). Encoding the resolved DID makes the link STABLE regardless of
    //    the opener's handle resolution (US-AV-006 Ex3).
    let link = parse_and_assert_query_encoding_share_link(&share.stdout);
    assert_eq!(
        link.dimension, "contributor",
        "the share link must encode dimension=contributor:\n{}",
        share.stdout
    );
    assert_eq!(
        link.value, "did:plc:priya-test",
        "the share link must encode value=did:plc:priya-test (the RESOLVED \
         handle->DID, NOT the handle `github:priya`):\n{}",
        share.stdout
    );
    // The exact `Shareable link:` line (criterion 1 — the user-visible affordance).
    assert!(
        share
            .stdout
            .contains("Shareable link: openlore://search?contributor=did:plc:priya-test"),
        "expected `Shareable link: openlore://search?contributor=did:plc:priya-test`:\n{}",
        share.stdout
    );
    // The link is whitespace-delimited; reconstruct it verbatim for the open below.
    let opened_link = format!("openlore://search?{}={}", link.dimension, link.value);

    // -- Action (open): Tobias OPENS the shared link via the CLI re-run resolver
    // (Q-DELIVER-AV-3: `openlore search <link>` — a positional link arg the search
    // verb detects; web AppView OUT of scope per OD-AV-6). The resolver PARSES the
    // link, maps `contributor` to the SearchDimension, and RE-RUNS the SAME
    // `author_did` query against the CURRENT index. The link carried the RESOLVED
    // DID, so no second handle->DID resolution is needed. --
    let resolved = run_openlore_search(&env, &["search", &opened_link], &indexer);
    assert_eq!(
        resolved.status, 0,
        "AV-29: opening the shared contributor link via the CLI re-run resolver must \
         exit 0. stdout: {} stderr: {}",
        resolved.stdout, resolved.stderr
    );

    // 2. The encoded query re-ran and yielded Priya's SAME verified network trail as
    //    the original `--contributor github:priya` query (same author rows, same
    //    [verified] marks), anti-merging preserved, with the same `peer add` follow
    //    affordance — the contributor-dimension parallel to AV-27's object round-trip.
    assert_resolved_link_matches_original_query(&original.stdout, &resolved.stdout);

    // The re-run preserved attribution end-to-end: 8 attributed rows under the SOLE
    // author did:plc:priya-test, NO merged/consensus row (anti-merging across the
    // share boundary, KPI-AV-2) — proven independently of the set-equality above so
    // the gate stands on its own.
    assert_network_result_preserves_attribution(
        &resolved.stdout,
        "github:bazelbuild/bazel",
        "org.openlore.philosophy.reproducible-builds",
        8,
        &["did:plc:priya-test#org.openlore.application"],
    );

    // 3. The resolved trail carries the SAME honest framing as AV-15 — "one
    //    developer's reasoning trail, not a community consensus" — preserved across
    //    the share boundary (a network trail is never a consensus; KPI-AV-1 /
    //    US-AV-006 Ex3).
    assert!(
        resolved
            .stdout
            .contains("one developer's reasoning trail, not a community consensus"),
        "AV-29: the resolved contributor trail must carry the honest 'one developer's \
         reasoning trail, not a community consensus' framing:\n{}",
        resolved.stdout
    );
}
