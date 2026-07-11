//! Slice-02 acceptance — the read-only `openlore ui` `/search?hide_retracted=1`
//! RETRACTION-FILTER toggle (feature `retraction-aware-search-filter`,
//! US-RF-002; ADR-060). Browser parity for the slice-01 CLI `--hide-retracted`,
//! over the SAME single pure `appview_domain::partition_retracted` decision.
//!
//! The slice-08 `/search` view (ADR-036/037/038) queries the slice-05 network
//! index and renders verified + attributed rows as a full page WITHOUT
//! `HX-Request` and the `#search-results` fragment WITH it (the slice-07 `Shape`
//! fork). This slice adds a plain `?hide_retracted=1` GET-param toggle + a
//! "Hide retracted claims" control on the form: on submit, the viewer runs
//! `partition_retracted` over the RAW attributed rows (BEFORE mapping to
//! `IndexedClaim`/`compose_results`, ADR-060 §subtlety-1), renders survivors, and
//! shows a results-region notice disclosing the hidden EVENT count — in BOTH htmx
//! shapes. Default (no param) is byte-identical to slice-08. The toggle is a
//! READ-only GET-param control — no write/sign/subscribe route, no key in the
//! process, loopback bind, offline chrome, full page without `HX-Request`,
//! not persisted (I-RF-6/7).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET `/search` (with/without the
//! `HX-Request` header — the slice-07 `get`/`get_htmx` pair). The network index is
//! the ONLY mocked boundary — a REAL slice-05 `openlore-indexer serve` over a
//! corpus seeded via the REAL ingest gate (an ORIGINAL claim + a same-author
//! `Retracts` marker + a different-author `Counters` marker), NOT a hand-rolled
//! HTTP double. NO scenario calls the `viewer-domain` render fns directly (those
//! are DELIVER's layer-1/2 unit + PBT surface).
//!
//! Layer 3/5 (subprocess + real-I/O), EXAMPLE-only (Mandate 9/11): sad/edge paths
//! (empty-after-filter, third-party-not-hidden, read-only) are enumerated
//! explicitly, never PBT-generated at this layer.
//!
//! Build-before-run: `cargo build --bin openlore` AND `cargo build --bin
//! openlore-indexer` before running (the harness spawns BOTH).
//!
//! RED gate (Mandate 7): `?hide_retracted=1` is an UNKNOWN query param today, so
//! the viewer IGNORES it → renders every row with NO notice and NO "Hide retracted
//! claims" control. The feature scenarios (RF-V1/V3/V4/V5/V6) assert the notice /
//! filtered rows / control, so they fail on the MISSING behavior (genuine RED,
//! not BROKEN). The DEFAULT-UNCHANGED gold guard (RF-V2) runs WITHOUT the param
//! and characterizes today's preserved slice-08 render (green-by-design; MUST stay
//! green through DELIVER). The `partition_retracted` scaffold
//! (`crates/appview-domain/src/retraction.rs`, `// SCAFFOLD: true`) panics until
//! DELIVER wires it into the `GET /search` handler.
//!
//! Covers US-RF-002: RF-V1 (walking-skeleton hide + notice, full page), RF-V2
//! (default unchanged), RF-V3 (htmx fragment parity), RF-V4 (empty-after-filter
//! buffer), RF-V5 (third-party counter shown while self-retracted hidden — D-3
//! contrast), RF-V6 (read-only GET-param control, no write surface).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::RawRecordSpec;

const OBJECT: &str = "org.openlore.philosophy.reproducible-builds";
const TOBIAS_DID: &str = "did:plc:tobias-test";

// --- OD-RF-2 (control label) + OD-RF-3 (notice wording) — DISTILL's proposal;
// DELIVER may freeze as content-frozen consts. Asserted as substrings. ---
const HIDE_CONTROL_LABEL: &str = "Hide retracted claims";
const VIEWER_NOTICE_FRAGMENT: &str = "retracted claim(s) hidden";
const VIEWER_UNTICK_GUIDANCE: &str = "Untick";
const EMPTY_AFTER_FILTER_FRAGMENT: &str = "were soft-retracted";

// The self-retracted author's ORIGINAL-claim confidence — the port-exposed
// observable that identifies Priya's withdrawn claim in the rendered HTML (the
// viewer renders `[verified]` + author DID + subject/predicate/object + confidence
// per row, but NOT the row's own cid, so presence/absence is keyed on the author
// DID + this confidence, not on a cid).
const PRIYA_ORIGINAL_CONFIDENCE: &str = "0.82";

/// The headline self-retraction corpus (Corpus A): an ORIGINAL claim C by Priya +
/// Priya's OWN same-author `Retracts` marker K (ONE event), plus two standing
/// claims by other authors (Sven, Tobias). Priya authors ONLY the withdrawn pair,
/// so under the filter she disappears entirely — the port-exposed absence witness.
fn corpus_self_retraction() -> Vec<RawRecordSpec> {
    let c = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let c_cid = c.clone().into_raw_record().published_cid.0;
    let k = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c_cid);
    let s1 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.65);
    let s2 = RawRecordSpec::valid(TOBIAS_DID, "github:guix/guix", OBJECT, 0.71);
    vec![c, k, s1, s2]
}

// =============================================================================
// RF-V1 — WALKING SKELETON: ticking "Hide retracted claims" focuses the browser
// survey AND discloses the count (full page, no HX-Request)
// =============================================================================

/// RF-V1 / WALKING SKELETON (US-RF-002 happy): `GET /search?object=<phil>&hide_retracted=1`
/// (no `HX-Request`, full page) over an index holding an author-self-retracted
/// claim + standing claims returns only the standing claims, with the
/// self-retracted claim ABSENT and a results-region notice disclosing
/// "1 retracted claim(s) hidden". Browser parity with the CLI walking skeleton.
///
/// Given Maria has run a philosophy search with a reachable indexer, and one
///   result was author-self-retracted;
/// When she submits with `?hide_retracted=1` (no htmx, full page);
/// Then the results region shows only the standing claims, the self-retracted
///   claim is absent, and a notice states "1 retracted claim(s) hidden".
///
/// @us-rf-002 @walking_skeleton @driving_adapter @real-io @adapter-integration
/// @full-page @j-005 @kpi-rf-1 @i-rf-1 @i-rf-3 @happy
#[test]
fn hide_retracted_full_page_removes_self_retracted_claim_and_shows_the_notice() {
    let env = TestEnv::initialized();
    let indexer = seed_network_index_from_specs(&env, corpus_self_retraction());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT}&hide_retracted=1"));

    assert_eq!(
        response.status, 200,
        "GET /search?…&hide_retracted=1 over a reachable seeded index must be 200; body:\n{}",
        response.body
    );
    // The standing survivors remain, attributed + verified.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[SVEN_DID, TOBIAS_DID]);
    assert_search_html_has_no_merged_consensus_row(&response.body);
    // The author-self-retracted claim is ABSENT — its author Priya authored only
    // the withdrawn pair, so she (and her original-claim confidence) disappear
    // entirely (the viewer renders no per-row cid, so absence is keyed on the
    // author DID + the original confidence).
    assert!(
        !response.body.contains(PRIYA_DID),
        "Priya (who authored only the withdrawn pair) must be absent under hide_retracted=1:\n{}",
        response.body
    );
    assert!(
        !response.body.contains(PRIYA_ORIGINAL_CONFIDENCE),
        "the self-retracted claim's confidence {PRIYA_ORIGINAL_CONFIDENCE} must be ABSENT under \
         hide_retracted=1:\n{}",
        response.body
    );
    // The results-region notice discloses the hidden EVENT count (1) + how to restore.
    assert!(
        response.body.contains(&format!("1 {VIEWER_NOTICE_FRAGMENT}")),
        "expected the results-region notice '1 {VIEWER_NOTICE_FRAGMENT}':\n{}",
        response.body
    );
    assert!(
        response.body.contains(VIEWER_UNTICK_GUIDANCE),
        "expected the notice to tell Maria to untick the control to restore the rows:\n{}",
        response.body
    );
}

// =============================================================================
// RF-V2 — DEFAULT UNCHANGED (gold regression guard; green-by-design, I-RF-1)
// =============================================================================

/// RF-V2 / GOLD REGRESSION GUARD (US-RF-002; I-RF-1): the SAME search WITHOUT the
/// `hide_retracted` param renders identically to slice-08 — the self-retracted
/// claim is STILL shown and NO "hidden" notice appears. GREEN-BY-DESIGN at DISTILL
/// (characterizes today's preserved default) and MUST STAY GREEN through DELIVER.
///
/// Given the same self-retraction index with a reachable indexer;
/// When Maria submits WITHOUT the hide param;
/// Then every result including the self-retracted one is shown and no notice appears.
///
/// @us-rf-002 @driving_adapter @real-io @adapter-integration @i-rf-1
/// @default-unchanged @gold @regression @edge
#[test]
fn default_search_without_the_param_renders_identically_with_no_notice() {
    let env = TestEnv::initialized();
    let indexer = seed_network_index_from_specs(&env, corpus_self_retraction());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // No hide_retracted param — today's slice-08 default (green-by-design).
    let response = viewer.get(&format!("/search?object={OBJECT}"));

    assert_eq!(
        response.status, 200,
        "GET /search?object=… (no hide param) must be 200; body:\n{}",
        response.body
    );
    // The self-retracted claim is STILL shown (I-AV-9 default unchanged) — Priya
    // and her original-claim confidence render on the default path.
    assert!(
        response.body.contains(PRIYA_DID) && response.body.contains(PRIYA_ORIGINAL_CONFIDENCE),
        "without the param, the self-retracted claim (Priya @ {PRIYA_ORIGINAL_CONFIDENCE}) must \
         STILL be shown (I-RF-1):\n{}",
        response.body
    );
    // NO "hidden" notice on the default path.
    assert!(
        !response.body.contains(VIEWER_NOTICE_FRAGMENT),
        "the default path must render NO '{VIEWER_NOTICE_FRAGMENT}' notice (I-RF-1):\n{}",
        response.body
    );
}

// =============================================================================
// RF-V3 — htmx fragment parity (hide_retracted=1 WITH HX-Request → fragment + notice)
// =============================================================================

/// RF-V3 (US-RF-002; I-RF-6 / slice-07 parity): the filtered submit WITH the
/// `HX-Request` header returns ONLY the `#search-results` fragment (no chrome),
/// carrying the survivors + the hidden-count notice — structurally the same region
/// the full page renders for `?…&hide_retracted=1`.
///
/// Given Maria has JavaScript enabled and a reachable indexer with a self-retracted
///   result;
/// When she submits `?hide_retracted=1` WITH the htmx header;
/// Then only the search-results region updates (a fragment), the self-retracted
///   claim is absent, and the hidden-count notice is present in the fragment.
///
/// @us-rf-002 @driving_adapter @real-io @adapter-integration @htmx-fragment
/// @i-rf-6 @edge
#[test]
fn hide_retracted_with_htmx_returns_only_the_filtered_results_fragment_with_the_notice() {
    let env = TestEnv::initialized();
    let indexer = seed_network_index_from_specs(&env, corpus_self_retraction());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get_htmx(&format!("/search?object={OBJECT}&hide_retracted=1"));

    assert_eq!(
        response.status, 200,
        "GET /search?…&hide_retracted=1 WITH HX-Request must be 200; body:\n{}",
        response.body
    );
    assert!(
        response.is_fragment(),
        "the htmx shape must return ONLY the #search-results fragment (no full-page chrome):\n{}",
        response.body
    );
    // The survivors are present + attributed; the self-retracted claim (Priya) is
    // absent (keyed on the author DID — the viewer renders no per-row cid).
    assert_search_html_every_row_verified_and_attributed(&response.body, &[SVEN_DID, TOBIAS_DID]);
    assert!(
        !response.body.contains(PRIYA_DID),
        "the self-retracted author (Priya) must be ABSENT from the filtered fragment:\n{}",
        response.body
    );
    // The notice is present IN the swapped fragment (both shapes carry it).
    assert!(
        response.body.contains(&format!("1 {VIEWER_NOTICE_FRAGMENT}")),
        "the swapped fragment must carry the '1 {VIEWER_NOTICE_FRAGMENT}' notice:\n{}",
        response.body
    );
}

// =============================================================================
// RF-V4 — EMPTY-AFTER-FILTER buffer (every result retracted → guided region)
// =============================================================================

/// RF-V4 (US-RF-002; I-RF-3): when `hide_retracted=1` hides ALL results, the
/// results region shows an explicit guided state ("all N results were
/// soft-retracted … untick to see them"), never a blank region and never a crash.
///
/// Given Maria searches a philosophy whose only indexed claims were all
///   author-self-retracted, with a reachable indexer;
/// When she submits with `?hide_retracted=1`;
/// Then the region states all results were soft-retracted + how to untick to see
///   them, and is never blank.
///
/// @us-rf-002 @driving_adapter @real-io @adapter-integration @empty-after-filter
/// @i-rf-3 @error @edge
#[test]
fn hiding_every_result_shows_a_guided_region_not_a_blank_region() {
    let env = TestEnv::initialized();
    // Two self-retraction EVENTS (each original + its same-author marker).
    let c1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let c1_cid = c1.clone().into_raw_record().published_cid.0;
    let k1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c1_cid);
    let c2 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.65);
    let c2_cid = c2.clone().into_raw_record().published_cid.0;
    let k2 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c2_cid);
    let indexer = seed_network_index_from_specs(&env, vec![c1, k1, c2, k2]);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT}&hide_retracted=1"));

    assert_eq!(
        response.status, 200,
        "an all-retracted hide_retracted=1 result is a valid guided state and must be 200; \
         body:\n{}",
        response.body
    );
    // Guided buffer copy naming the soft-retraction + the untick guidance.
    assert!(
        response.body.contains(EMPTY_AFTER_FILTER_FRAGMENT),
        "expected the guided empty-after-filter region ('{EMPTY_AFTER_FILTER_FRAGMENT}'):\n{}",
        response.body
    );
    assert!(
        response.body.contains(VIEWER_UNTICK_GUIDANCE),
        "expected the guided region to tell Maria to untick the control to see them:\n{}",
        response.body
    );
    // Never blank: the region carries meaningful copy (asserted above), not an
    // empty results region — and the viewer never crashed (200 above).
}

// =============================================================================
// RF-V5 — a third-party COUNTER is shown while a self-retraction is hidden (D-3)
// =============================================================================

/// RF-V5 (US-RF-002; D-3 / I-RF-4 — browser parity of "no heckler's veto"): with
/// `hide_retracted=1` over a corpus holding BOTH an author-self-retracted claim
/// AND a DIFFERENT author's claim that a third party merely COUNTERED, the
/// self-retracted claim is hidden (with the notice) while the third-party-countered
/// claim STAYS shown. A disagreement never removes an author's row in the browser.
///
/// Given a self-retracted claim AND a separate third-party-countered standing claim,
///   with a reachable indexer;
/// When Maria submits with `?hide_retracted=1`;
/// Then the self-retracted claim is hidden (notice shown) and the countered claim
///   is STILL shown.
///
/// @us-rf-002 @driving_adapter @real-io @adapter-integration @i-av-9 @i-rf-4
/// @no-hecklers-veto @edge
#[test]
fn a_third_party_countered_claim_stays_shown_while_a_self_retraction_is_hidden() {
    let env = TestEnv::initialized();

    // A self-retraction EVENT by Priya (original + her own marker). Priya authors
    // ONLY the withdrawn pair, so she is the port-exposed absence witness.
    let selfr = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let selfr_cid = selfr.clone().into_raw_record().published_cid.0;
    let marker = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &selfr_cid);

    // A DIFFERENT author's (Tobias) standing claim that a THIRD party (Sven) merely
    // countered — Tobias is the port-exposed "stays shown" witness.
    let standing = RawRecordSpec::valid(TOBIAS_DID, "github:guix/guix", OBJECT, 0.71);
    let standing_cid = standing.clone().into_raw_record().published_cid.0;
    let counter = RawRecordSpec::valid(SVEN_DID, "github:guix/guix", OBJECT, 0.40)
        .with_reference(claim_domain::ReferenceType::Counters, &standing_cid);

    let indexer = seed_network_index_from_specs(&env, vec![selfr, marker, standing, counter]);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT}&hide_retracted=1"));

    assert_eq!(
        response.status, 200,
        "GET /search?…&hide_retracted=1 must be 200; body:\n{}",
        response.body
    );
    // The self-retracted claim is hidden (Priya absent) + the notice is shown.
    assert!(
        !response.body.contains(PRIYA_DID),
        "the author-self-retracted claim (Priya) must be hidden:\n{}",
        response.body
    );
    assert!(
        response.body.contains(&format!("1 {VIEWER_NOTICE_FRAGMENT}")),
        "expected the '1 {VIEWER_NOTICE_FRAGMENT}' notice:\n{}",
        response.body
    );
    // The third-party-countered standing claim (Tobias) STAYS shown (no heckler's
    // veto) — keyed on the author DID (the viewer renders no per-row cid).
    assert!(
        response.body.contains(TOBIAS_DID),
        "the third-party-countered standing claim's author (Tobias) must STAY shown under \
         hide_retracted=1 (D-3):\n{}",
        response.body
    );
}

// =============================================================================
// INVARIANTS — read-only GET-param control (RF-V6; I-RF-6 gold guardrail)
// =============================================================================

/// RF-V6 (US-RF-002; I-RF-6 — read-only preserved): the "Hide retracted claims"
/// control is a plain GET-param toggle that adds NO write/sign/subscribe surface.
/// The `/search` page renders the labeled control (a read-only GET affordance) and
/// exposes no authoring/mutating markup — the page stays read-only, no key in the
/// process (structural: xtask check-arch), loopback bind. The "control present"
/// half is RED today (the control does not exist yet); the "no write surface" half
/// is the standing read-only guardrail carried across the new surface.
///
/// Given the `/search` page in the read-only viewer with a reachable indexer;
/// When the page is rendered;
/// Then it shows the "Hide retracted claims" GET-param control and no
///   write/sign/subscribe affordance.
///
/// @us-rf-002 @driving_adapter @real-io @read-only @i-rf-6 @invariant @edge
#[test]
fn the_hide_control_is_a_read_only_get_param_toggle_with_no_write_surface() {
    let env = TestEnv::initialized();
    let indexer = seed_network_index_from_specs(&env, corpus_self_retraction());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // The results surface (with the filter active) is where the control lives.
    let response = viewer.get(&format!("/search?object={OBJECT}&hide_retracted=1"));
    assert_eq!(
        response.status, 200,
        "GET /search?…&hide_retracted=1 must be 200; body:\n{}",
        response.body
    );

    // The page renders the labeled read-only hide control (OD-RF-2) — RED today.
    assert!(
        response.body.contains(HIDE_CONTROL_LABEL),
        "the /search page must render the '{HIDE_CONTROL_LABEL}' GET-param control:\n{}",
        response.body
    );

    // …and the hide control adds NO write/sign/subscribe affordance (I-RF-6 — the
    // toggle is a public-data READ; signing/following stays in the CLI).
    let lowered = response.body.to_ascii_lowercase();
    for banned in [
        "name=\"sign\"",
        "sign claim",
        "sign & publish",
        "subscribe",
        "type=\"password\"",
    ] {
        assert!(
            !lowered.contains(banned),
            "the hide-control surface must expose NO write/sign/subscribe affordance \
             (found {banned:?}, I-RF-6):\n{}",
            response.body
        );
    }
}
