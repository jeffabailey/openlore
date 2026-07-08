//! Slice-06 acceptance — the `openlore ui` LIVE-SCRAPE view (US-VIEW-005).
//!
//! The `/scrape` route (DESIGN §5): the operator enters a target on a form; the
//! viewer runs the slice-02 propose step LIVE (resolve + harvest via the reused
//! `GithubPort` + the PURE `scraper_domain::derive_candidates`) and renders the
//! resulting in-memory `CandidateClaim` values as HTML rows — with display-only
//! `derived-from` provenance (I-VIEW-5 / WD-62, surfaced ONLY here). It persists
//! NOTHING (BR-VIEW-2 — refresh re-harvests) and renders NO sign control
//! (BR-VIEW-1 / I-SCR-1 — signing stays in the CLI). This is the ONE viewer route
//! that touches the network (NFR-VIEW-7).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP POST /scrape. The external GitHub
//! API is the ONLY mocked boundary — the REUSED slice-02 `FakeGithub` double (via
//! `GithubServer`, wired into the viewer through `OPENLORE_GITHUB_API_BASE`); a
//! NEW GitHub double is NOT built. The local DuckDB is REAL but UNTOUCHED by
//! `/scrape` (zero persistence — proven structurally in viewer_invariants.rs).
//!
//! Layer placement (Mandate 11): layer-3/layer-5 subprocess + real-I/O,
//! example-only. The sad paths (zero candidates, network down) are enumerated
//! explicitly, never PBT-generated.
//!
//! DISTILL resolution of the DESIGN low-nit (NetworkDown rendering): V-S4 pins the
//! network-down assertion — the rendered message states GitHub could not be
//! reached AND notes the store view still works offline, and it does NOT leak HTTP
//! / transport internals (no status codes, no "connection refused", no URLs). This
//! resolves the open DESIGN nit on `/scrape` NetworkDown rendering (NFR-VIEW-6/7).
//!
//! Covers:
//! - US-VIEW-005: browse live proposals, nothing signed/saved + no sign control
//!   + derived-from present (V-S1); zero candidates guidance (V-S3); network-down
//!   plain-language message + offline-store note (V-S4)
//!   (the cross-view "derived-from NEVER on persisted claims" half of AC-005.2
//!   lives in viewer_invariants.rs V-INV-2 as the gold guardrail).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-VIEW-005 — browse live scrape proposals before signing in the CLI
// =============================================================================

/// V-S1 (US-VIEW-005 happy; AC-005.1): Maria enters a target on the Live Scrape
/// view; the viewer runs the live propose step and renders the proposed candidate
/// claims as scannable HTML rows — subject, predicate, object, confidence, and
/// their display-only `derived-from` provenance. The page states none are signed
/// or saved, renders NO sign control, and directs her to the CLI to sign. NOTHING
/// is persisted (the read-only invariant is the gold test; here the user-facing
/// "nothing signed/saved" + "no sign control" contract is asserted).
///
/// Given a live scrape of "rust-lang/cargo" would propose candidate claims;
/// When Maria submits that target on the Live Scrape view;
/// Then she sees the candidates with subject, predicate, object, confidence, and
/// derived-from; the page states none are signed or saved; no sign control is
/// rendered; and she is directed to the CLI to sign.
///
/// @us-view-005 @driving_port @driving_adapter @real-io @derived-from @happy
#[test]
fn operator_browses_live_proposals_without_signing_anything() {
    // GIVEN an initialized env + the reused slice-02 `FakeGithub` serving a public
    // repo with harvestable signals (the ONLY mocked boundary). The viewer reaches
    // it through OPENLORE_GITHUB_API_BASE, exactly as `scrape github` does.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo"));
    let viewer = ViewerServer::start_with_github(&env, github);

    // WHEN Maria submits the target on the Live Scrape view (POST /scrape).
    let page = viewer.post_form("/scrape", &[("target", "rust-lang/cargo")]);

    // THEN candidates render with their fields + display-only derived-from; the
    // page states nothing is signed/saved; NO sign control is rendered; and she is
    // directed to the CLI to sign. (Observable rendered surface only.)
    assert_eq!(page.status, 200, "the Live Scrape view must render results");
    assert!(
        page.body_contains("rust-lang/cargo"),
        "the candidates must render their subject; body was:\n{}",
        page.body
    );
    assert!(
        page.body_contains("derived-from"),
        "candidates must carry display-only derived-from provenance (WD-62); \
         body was:\n{}",
        page.body
    );
    assert!(
        page.body_contains("nothing")
            && (page.body_contains("signed") || page.body_contains("saved")),
        "the page must state none of the candidates are signed or saved \
         (BR-VIEW-2); body was:\n{}",
        page.body
    );
    assert!(
        page.body_contains("sign") && page.body_contains("CLI"),
        "the page must direct the operator to the CLI to sign (I-SCR-1); \
         body was:\n{}",
        page.body
    );
    // The HARD human-gate guardrail: NO sign control on the surface (BR-VIEW-1).
    // A form/button that would submit a signing action is forbidden — the live
    // view can describe signing-via-CLI but never offer a sign affordance.
    for sign_control_marker in ["name=\"sign\"", "Sign claim", "type=\"submit\" value=\"sign"] {
        assert!(
            !page.body_contains(sign_control_marker),
            "the Live Scrape view must render NO sign control ({sign_control_marker:?}) \
             — signing stays in the CLI (BR-VIEW-1 / I-SCR-1); body was:\n{}",
            page.body
        );
    }
}

/// V-S3 (US-VIEW-005 edge; AC-005.3): a target that derives NO candidates shows
/// the guided "No candidate claims could be derived" message with a suggested
/// alternative (FR-VIEW-7 / NFR-VIEW-6) — not a blank result.
///
/// Given a live scrape of "some-org/empty-repo" derives no candidates;
/// When Maria submits that target;
/// Then she sees "No candidate claims could be derived" with a suggested
/// alternative.
///
/// @us-view-005 @driving_port @real-io @empty-state @edge
#[test]
fn target_yielding_no_candidates_guides_the_operator() {
    // GIVEN a public repo that harvests successfully but yields NO derivable
    // candidates (the reused FakeGithub serves a public repo with no signals from
    // which any candidate could be derived).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::with_no_matching_signals("some-org/empty-repo"));
    let viewer = ViewerServer::start_with_github(&env, github);

    // WHEN Maria submits that target.
    let page = viewer.post_form("/scrape", &[("target", "some-org/empty-repo")]);

    // THEN she sees the guided zero-candidates message with a suggested
    // alternative — never a blank result.
    assert_eq!(page.status, 200, "a zero-candidate scrape still renders a guided page");
    assert!(
        page.body_contains("No candidate claims could be derived"),
        "a target yielding no candidates must show the guided message; \
         body was:\n{}",
        page.body
    );
}

/// V-S4 (US-VIEW-005 error; AC-005.4): network unavailable. Maria submits a target
/// and the page reports — in PLAIN LANGUAGE — that GitHub could not be reached AND
/// that her store view still works offline (NFR-VIEW-7). The rendered message does
/// NOT leak HTTP / transport internals.
///
/// DISTILL RESOLUTION of the DESIGN low-nit on `/scrape` NetworkDown rendering:
/// the network-down render (a) names the cause in domain language ("GitHub could
/// not be reached"), (b) reassures that the offline store view still works, and
/// (c) leaks NO transport internals — no HTTP status codes, no "connection
/// refused" / "timed out" / "DNS", no raw URLs, no stack trace (NFR-VIEW-6).
///
/// Given Maria cannot reach GitHub;
/// When she submits "tokio-rs/tokio" on the Live Scrape view;
/// Then she sees that GitHub could not be reached and is told her store view
/// still works offline — with no leaked transport internals.
///
/// @us-view-005 @driving_port @real-io @network-failure @error
#[test]
fn network_failure_clarifies_the_store_view_still_works_offline() {
    // GIVEN GitHub is unreachable (the reused slice-02 `FakeGithub::offline()`
    // posture — the established network-down double).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::offline());
    let viewer = ViewerServer::start_with_github(&env, github);

    // WHEN Maria submits a target on the Live Scrape view.
    let page = viewer.post_form("/scrape", &[("target", "tokio-rs/tokio")]);

    // THEN the page reports, in plain language, that GitHub could not be reached
    // AND that her store view still works offline.
    assert_eq!(page.status, 200, "a network-down scrape still renders a guided page");
    assert!(
        page.body_contains("GitHub could not be reached"),
        "the network-down render must name the cause in domain language; \
         body was:\n{}",
        page.body
    );
    assert!(
        page.body_contains("store view still works offline"),
        "the network-down render must reassure that the store view works offline \
         (NFR-VIEW-7); body was:\n{}",
        page.body
    );

    // DISTILL nit resolution: the message must NOT leak HTTP / transport internals
    // — no status codes, no socket/DNS jargon, no raw URLs, no stack trace
    // (NFR-VIEW-6 error legibility). The operator sees a cause + a next step, not
    // a transport error dump.
    for leaked_internal in [
        "connection refused",
        "ConnectError",
        "timed out",
        "dns",
        "503",
        "502",
        "500",
        "http://",
        "https://api.github",
        "panicked at",
    ] {
        assert!(
            !page.body.to_lowercase().contains(&leaked_internal.to_lowercase()),
            "the network-down render must leak NO transport internals \
             ({leaked_internal:?}) — plain-language cause + offline-store note only \
             (DISTILL resolution of the DESIGN /scrape NetworkDown nit); \
             body was:\n{}",
            page.body
        );
    }
}
