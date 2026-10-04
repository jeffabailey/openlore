//! bluesky-claim-review-app — sign-in and GitHub ownership proof
//! (US-BRA-000, US-BRA-001, US-BRA-002; walking-skeleton release).
//!
//! Focused scenarios behind the walking skeleton: the app's public identity
//! and transport guarantees, every sign-in sad path (including the SPIKE-2
//! finding that a failed code exchange must not crash the app), and every
//! ownership-proof verdict (ADR-076: exact DID token, per-DID link, no scrape
//! while unverified).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP. Fakes: ATProto
//! network + GitHub only. Layer 4 (real HTTP): example-only; sad paths are
//! named examples (Mandate 11). All `#[ignore]`d for one-at-a-time DELIVER.

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::{ConsentPosture, TokenPosture};
use review_app::*;

// =============================================================================
// US-BRA-000 — reachable at a public address Bluesky can trust
// =============================================================================

/// SI-1
/// ```gherkin
/// @US-BRA-000 @AC-000.1 @NFR-BRA-3 @driving_port @real-io @contract-shape:pure-function
/// Scenario: Bluesky can identify the review app from its published client details
///   Given the review app is deployed at its public address
///   When Priya's PDS looks up the app's published client details
///   Then it finds the name "OpenLore review", the app's own address as its identity,
///        its one sign-in return address, tokens bound to the app's key, and only claim + post creation asked for
///   And the published signing keys contain no private part
/// ```
#[test]
fn bluesky_identifies_the_review_app_from_its_published_client_details() {
    let world = ReviewWorld::new();
    let (status, _, body) = world.app.get("/oauth/client-metadata.json");
    assert_eq!(status, 200);
    let meta: serde_json::Value = serde_json::from_str(&body).expect("client details are JSON");
    assert_eq!(meta["client_name"], "OpenLore review");
    assert_eq!(meta["client_id"], world.app.client_metadata_url().as_str());
    assert_eq!(
        meta["redirect_uris"],
        serde_json::json!([format!("{}/oauth/callback", world.app.origin())])
    );
    assert_eq!(meta["dpop_bound_access_tokens"], true);
    assert_eq!(meta["token_endpoint_auth_method"], "private_key_jwt");
    assert_eq!(meta["scope"], app::OAUTH_SCOPES);
    let (status, _, jwks) = world.app.get("/oauth/jwks.json");
    assert_eq!(status, 200);
    let jwks: serde_json::Value = serde_json::from_str(&jwks).expect("keys are JSON");
    let keys = jwks["keys"].as_array().expect("a key set");
    assert!(!keys.is_empty());
    for key in keys {
        assert_eq!(key["alg"], "ES256");
        assert!(
            key.get("d").is_none(),
            "a published key never carries its private part"
        );
    }
}

/// SI-2
/// ```gherkin
/// @US-BRA-000 @AC-000.3 @error @contract-shape:unbounded-preservation
/// Scenario: Unavailability is explained, not raw
///   Given Priya's PDS cannot fetch the app's client details
///   When Priya tries to sign in
///   Then she sees that the app is temporarily unavailable and nothing changed
///   And she holds no session and nothing was written anywhere
/// ```
#[test]
fn an_unreachable_identity_check_is_explained_as_temporarily_unavailable_and_changes_nothing() {
    let world = ReviewWorld::new();
    world
        .atproto
        .block_client_metadata_fetch(PdsHost::BSKY, true);
    let mut browser = world.browser();
    when_signs_in(&mut browser, Persona::Priya);
    assert!(
        browser.page.shows("temporarily unavailable"),
        "{}",
        browser.page.text()
    );
    assert!(browser.page.shows("Nothing changed") || browser.page.shows("nothing changed"));
    assert!(!has_session(&mut browser));
    assert!(world.atproto.write_attempts().is_empty());
}

/// SI-3
/// ```gherkin
/// @US-BRA-000 @AC-000.2 @NFR-BRA-2 @contract-shape:pure-function
/// Scenario: Every page is served with HTTPS-only and anti-framing protections
///   Given the review app is deployed
///   When Priya opens its landing page
///   Then the browser is told to use HTTPS only, to never frame the page, and to run only the app's own scripts
/// ```
#[test]
fn every_page_tells_the_browser_to_use_https_only_and_never_frame_it() {
    let world = ReviewWorld::new();
    let (status, headers, _) = world.app.get("/");
    assert_eq!(status, 200);
    let h = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    assert!(
        h("strict-transport-security").contains("max-age="),
        "HSTS (AC-000.2, the app half; Caddy redirects HTTP)"
    );
    assert!(h("content-security-policy").contains("default-src 'self'"));
    assert!(h("content-security-policy").contains("frame-ancestors 'none'"));
    assert_eq!(h("x-content-type-options"), "nosniff");
    assert_eq!(h("referrer-policy"), "same-origin");
}

// =============================================================================
// US-BRA-001 — sign in with my Bluesky handle
// =============================================================================

/// SI-4
/// ```gherkin
/// @US-BRA-001 @AC-001.2 @NFR-BRA-8 @contract-shape:pure-function
/// Scenario: Priya sees what the app will never do before signing in
///   Given Priya opens the review app for the first time
///   Then before the handle field she sees that it never publishes unapproved suggestions,
///        never shows her pending or declined suggestions to anyone, and never posts unless she presses Post
///   And she sees that only public GitHub data is read
/// ```
#[test]
fn priya_sees_what_the_app_will_never_do_before_she_signs_in() {
    let world = ReviewWorld::new();
    let mut browser = world.browser();
    let page = browser.open("/").clone();
    let text = page.text();
    let handle_field = page.html.find("name=\"handle\"").expect("a handle field");
    let before_field = html::visible_text(&page.html[..handle_field]);
    for commitment in ["never publishes", "never shows", "never posts"] {
        assert!(
            before_field.contains(commitment),
            "{commitment:?} must precede the handle field:\n{text}"
        );
    }
    assert!(
        text.contains("public"),
        "the public-data banner (NFR-BRA-8):\n{text}"
    );
}

/// SI-5
/// ```gherkin
/// @US-BRA-001 @AC-001.3 @I-BRA-7 @contract-shape:bounded-change
/// Scenario: Dmitri signs in through his self-hosted PDS
///   Given Dmitri's handle "dmitri.volkov.dev" is hosted at his own PDS
///   When he signs in with Bluesky
///   Then he authorizes at his own PDS, not bsky.social
///   And he returns signed in as @dmitri.volkov.dev
/// ```
#[test]
fn dmitri_signs_in_through_his_self_hosted_pds() {
    let world = ReviewWorld::new();
    let mut browser = world.browser();
    when_signs_in(&mut browser, Persona::Dmitri);
    then_sees_signed_in_as(&browser.page, Persona::Dmitri);
    let auths = world.atproto.authorizations();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].host, PdsHost::VOLKOV, "authorized at his own PDS");
}

/// SI-6
/// ```gherkin
/// @US-BRA-001 @AC-001.4 @error @contract-shape:unbounded-preservation
/// Scenario: A mistyped handle gets a helpful message
///   Given no account has the handle "priyaramen.bsky.social"
///   When Priya tries to sign in with it
///   Then she sees "We couldn't find that Bluesky handle. Check the spelling."
///   And no authorization was started anywhere and she holds no session
/// ```
#[test]
fn a_mistyped_handle_gets_a_helpful_message_and_starts_nothing() {
    let world = ReviewWorld::new();
    let mut browser = world.browser();
    when_signs_in_with_handle(&mut browser, MISTYPED_HANDLE);
    assert!(
        browser
            .page
            .shows("We couldn't find that Bluesky handle. Check the spelling."),
        "{}",
        browser.page.text()
    );
    assert!(world.atproto.authorizations().is_empty());
    assert!(
        world.atproto.client_metadata_seen().is_empty(),
        "no authorization request was made"
    );
    assert!(!has_session(&mut browser));
}

/// SI-7
/// ```gherkin
/// @US-BRA-001 @AC-001.5 @error @contract-shape:unbounded-preservation
/// Scenario: Cancelling authorization changes nothing
///   Given Priya is on her PDS's authorization screen
///   When she cancels
///   Then she is back on the start page with "No access granted. Nothing changed."
///   And she holds no session
/// ```
#[test]
fn cancelling_authorization_at_her_pds_changes_nothing() {
    let world = ReviewWorld::new();
    world
        .atproto
        .set_consent(Persona::Priya.did(), ConsentPosture::Cancel);
    let mut browser = world.browser();
    when_signs_in(&mut browser, Persona::Priya);
    assert!(
        browser.page.shows("No access granted. Nothing changed."),
        "{}",
        browser.page.text()
    );
    assert!(
        browser.page.offers("Sign in with Bluesky"),
        "back on the start page"
    );
    assert_eq!(world.atproto.token_exchanges(), 0);
    assert!(!has_session(&mut browser));
}

/// SI-8
/// ```gherkin
/// @US-BRA-001 @AC-001.6 @error @integration_checkpoint @contract-shape:unbounded-preservation
/// Scenario: A sign-in that returns a different account than the handle named is refused
///   Given Priya's PDS answers her sign-in with a token for another account
///   When she signs in with Bluesky
///   Then the sign-in is refused and she holds no session
/// ```
#[test]
fn a_sign_in_that_returns_a_different_account_than_the_handle_named_is_refused() {
    let world = ReviewWorld::new();
    world.atproto.set_token_posture(
        Persona::Priya.did(),
        TokenPosture::SubjectMismatch(Persona::Sam.did().to_string()),
    );
    let mut browser = world.browser();
    when_signs_in(&mut browser, Persona::Priya);
    assert!(
        !browser.page.shows("Signed in as"),
        "{}",
        browser.page.text()
    );
    assert!(!has_session(&mut browser));
}

/// SI-9
/// ```gherkin
/// @US-BRA-001 @SPIKE-2-finding-3 @error @infrastructure-failure @contract-shape:unbounded-preservation
/// Scenario: A failed sign-in exchange is explained and never takes the app down
///   Given Priya's PDS refuses to exchange her sign-in code
///   When she signs in with Bluesky
///   Then she sees that signing in failed and nothing changed
///   And the app keeps serving: Dmitri can still sign in
/// ```
#[test]
fn a_failed_sign_in_exchange_is_explained_and_never_takes_the_app_down() {
    let world = ReviewWorld::new();
    world
        .atproto
        .set_token_posture(Persona::Priya.did(), TokenPosture::ExchangeFails);
    let mut priya = world.browser();
    when_signs_in(&mut priya, Persona::Priya);
    assert!(
        priya.page.status < 500 && priya.page.status != 0,
        "a sign-in error page, not a crash"
    );
    assert!(priya.page.shows("Nothing changed"), "{}", priya.page.text());
    assert!(!has_session(&mut priya));
    assert_eq!(world.app.get("/healthz").0, 200, "the app is still alive");
    let dmitri = given_signed_in(&world, Persona::Dmitri);
    drop(dmitri);
}

/// SI-10
/// ```gherkin
/// @US-BRA-001 @SPIKE-2-finding-3 @error @contract-shape:unbounded-preservation
/// Scenario: A replayed sign-in return link signs nobody in
///   Given Priya signed in and her sign-in return link was captured
///   When the same return link is opened again in another browser
///   Then that browser is not signed in and the app keeps serving
/// ```
#[test]
fn a_replayed_sign_in_return_link_signs_nobody_in() {
    let world = ReviewWorld::new();
    let _priya = given_signed_in(&world, Persona::Priya);
    // The code Priya's PDS issued was exchanged once; replaying a return link
    // (with that or any other code) in another browser must sign nobody in.
    let mut attacker = world.browser();
    for code in ["code-1-replayed", "code-2-forged"] {
        let replay = format!(
            "{}/oauth/callback?code={code}&state=s&iss=x",
            world.app.origin()
        );
        attacker.open(&replay);
        assert!(attacker.page.status < 500, "a refusal page, not a crash");
        assert!(
            !attacker.page.shows("Signed in as"),
            "{}",
            attacker.page.text()
        );
    }
    assert!(!has_session(&mut attacker));
    assert_eq!(world.app.get("/healthz").0, 200);
    assert_eq!(
        world.atproto.token_exchanges(),
        1,
        "only Priya's own code was ever exchanged"
    );
    assert!(
        !world.app.logs().contains("code="),
        "return-link codes are never logged"
    );
}

/// SI-11
/// ```gherkin
/// @US-BRA-001 @AC-001.7 @contract-shape:bounded-change
/// Scenario: Signing out ends the session
///   Given Priya is signed in
///   When she signs out
///   Then her queue asks her to sign in again
/// ```
#[test]
fn signing_out_ends_the_session() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    when_signs_out(&mut browser);
    assert!(!has_session(&mut browser));
}

/// SI-12
/// ```gherkin
/// @US-BRA-001 @NFR-BRA-2 @contract-shape:pure-function
/// Scenario: The session cookie is out of reach of page scripts and never carries tokens
///   Given Priya is signed in
///   Then her session cookie is host-only, secure, HTTP-only and same-site
///   And no page she sees contains any token her PDS issued
/// ```
#[test]
fn the_session_cookie_is_out_of_reach_of_page_scripts_and_pages_never_carry_tokens() {
    let world = ReviewWorld::new();
    let mut browser = world.browser();
    when_signs_in(&mut browser, Persona::Priya);
    let cookie = browser
        .page
        .set_cookies
        .iter()
        .find(|c| c.starts_with("__Host-"))
        .cloned()
        .expect("a __Host- session cookie");
    let lower = cookie.to_ascii_lowercase();
    for flag in ["httponly", "secure", "samesite=lax", "path=/"] {
        assert!(lower.contains(flag), "cookie flag {flag}: {cookie}");
    }
    for path in ["/review", "/github", "/settings"] {
        let html = browser.open(path).html.clone();
        assert!(
            !html.contains("fake-access-") && !html.contains("fake-refresh-"),
            "no token on {path}"
        );
    }
}

// =============================================================================
// US-BRA-002 — prove my GitHub account is mine with my DID in my bio
// =============================================================================

/// OW-1
/// ```gherkin
/// @US-BRA-002 @AC-002.1 @contract-shape:pure-function
/// Scenario: Priya gets the exact DID to copy and where to put it
///   Given Priya is signed in and has not linked GitHub
///   When she opens the GitHub step
///   Then she sees her exact DID with a copy action and instructions to add it to her GitHub bio
/// ```
#[test]
fn priya_gets_the_exact_did_to_copy_and_where_to_put_it() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    let page = browser.open("/github").clone();
    assert!(page.shows(Persona::Priya.did()));
    assert!(page.offers("Copy"), "a copy action");
    assert!(page.shows("bio"), "says where to put it:\n{}", page.text());
}

/// OW-2
/// ```gherkin
/// @US-BRA-002 @AC-002.3 @AC-002.4 @I-BRA-4 @error @adversarial @contract-shape:unbounded-preservation
/// Scenario: Sam cannot claim someone else's account
///   Given Sam is signed in as did:plc:q9rt5wz2b8kd3m1xv7pn4ahe
///   And github.com/BurntSushi's bio does not contain Sam's DID
///   When Sam verifies GitHub username "BurntSushi"
///   Then he sees that his DID was not found in that bio and how to add it to his own
///   And no scan starts, no repo is read and no suggestion is created
/// ```
#[test]
fn sam_cannot_claim_someone_elses_github_account() {
    let world = ReviewWorld::new();
    let mut sam = given_signed_in(&world, Persona::Sam);
    when_verifies_github(&mut sam, BURNTSUSHI);
    let text = sam.page.text();
    assert!(
        text.contains("couldn't find") && text.contains("github.com/BurntSushi"),
        "{text}"
    );
    assert!(!text.contains("Verified:"));
    sam.open("/review");
    assert!(
        sam.try_press("Scan my repos").is_err() || sam.page.shows("verify"),
        "no scan while unverified"
    );
    assert_eq!(world.github.scrape_reads_of(BURNTSUSHI), 0);
    assert!(sam.open("/review").pending_cards().is_empty());
}

/// OW-3
/// ```gherkin
/// @US-BRA-002 @AC-002.3 @error @contract-shape:unbounded-preservation
/// Scenario: A different DID in the bio is explained
///   Given Dmitri's GitHub bio holds his old DID did:plc:ab12cd34ef56gh78ij90klmn
///   When he verifies "dvolkov" while signed in with his current DID
///   Then he sees that the bio holds a different DID that must match his signed-in account
/// ```
#[test]
fn a_different_did_in_the_bio_is_explained() {
    let world = ReviewWorld::new();
    let mut dmitri = given_signed_in(&world, Persona::Dmitri);
    when_verifies_github(&mut dmitri, Persona::Dmitri.github_login());
    let text = dmitri.page.text();
    assert!(
        text.contains("different DID") && text.contains("did:plc:ab12"),
        "{text}"
    );
    assert!(text.contains("must match"), "{text}");
}

/// OW-4
/// ```gherkin
/// @US-BRA-002 @AC-002.3 @error @infrastructure-failure @contract-shape:unbounded-preservation
/// Scenario: A rate limit is explained and retry works
///   Given GitHub is rate-limiting the app for 4 minutes
///   When Priya verifies
///   Then she sees a rate-limit message with the wait time and that nothing was lost
///   And after the wait, pressing Verify again succeeds
/// ```
#[test]
fn a_github_rate_limit_is_explained_and_retrying_after_the_wait_succeeds() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    world.github.rate_limit_for(240);
    when_verifies_github(&mut browser, Persona::Priya.github_login());
    let text = browser.page.text();
    assert!(
        text.contains("GitHub is rate-limiting us") && text.contains("minute"),
        "{text}"
    );
    assert!(text.contains("Nothing was lost"), "{text}");
    world.github.clear_rate_limit();
    when_verifies_github(&mut browser, Persona::Priya.github_login());
    then_sees_verified(&browser.page, Persona::Priya);
}

/// OW-5
/// ```gherkin
/// @US-BRA-002 @AC-002.3 @error @contract-shape:unbounded-preservation
/// Scenario: A GitHub username that does not exist is named in the message
///   Given no GitHub account "priyaramn" exists
///   When Priya verifies "priyaramn"
///   Then she sees that github.com/priyaramn was not found and can retry
/// ```
#[test]
fn a_github_username_that_does_not_exist_is_named_in_the_message() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    when_verifies_github(&mut browser, "priyaramn");
    let text = browser.page.text();
    assert!(
        text.contains("github.com/priyaramn") && !text.contains("Verified:"),
        "{text}"
    );
    assert!(browser.page.offers("Verify"), "she can retry");
}

/// OW-6
/// ```gherkin
/// @US-BRA-002 @AC-002.5 @adversarial @contract-shape:unbounded-preservation
/// Scenario: A verified link belongs to one DID only
///   Given github.com/priyaraman is verified for Priya's DID
///   When Sam, signed in with his own DID, verifies "priyaraman"
///   Then the verification fails for Sam
///   And Priya's link is unaffected
/// ```
#[test]
fn a_verified_github_link_belongs_to_one_did_only() {
    let world = ReviewWorld::new();
    let mut priya = given_github_verified(&world, Persona::Priya);
    let mut sam = given_signed_in(&world, Persona::Sam);
    when_verifies_github(&mut sam, "priyaraman");
    assert!(!sam.page.shows("Verified:"), "{}", sam.page.text());
    assert_eq!(
        when_scan_finishes(&mut priya),
        "completed",
        "Priya's link still works"
    );
}

/// OW-7 — parametrised over bios that must NOT verify (exact-token rule, ADR-076).
/// ```gherkin
/// @US-BRA-002 @AC-002.2 @AC-002.6 @boundary @adversarial @contract-shape:pure-function
/// Scenario Outline: Only the exact signed-in DID token proves ownership
///   Given Priya's GitHub bio reads "<bio>"
///   When she verifies "priyaraman"
///   Then she is <outcome>
///   Examples:
///     | bio                                              | outcome      |
///     | Rust. @priyaraman.bsky.social                    | not verified |
///     | Rust. did:plc:7x3kq2mzv5rj4w6hbn2tqclpx          | not verified |
///     | Rust. xdid:plc:7x3kq2mzv5rj4w6hbn2tqclp          | not verified |
///     | Rust. DID:PLC:7X3KQ2MZV5RJ4W6HBN2TQCLP           | not verified |
///     | (empty)                                          | not verified |
///     | Rust (did:plc:7x3kq2mzv5rj4w6hbn2tqclp).         | verified     |
///     | did:plc:ab12cd34ef56gh78ij90klmn did:plc:7x3kq2mzv5rj4w6hbn2tqclp | verified |
/// ```
#[test]
fn only_the_exact_signed_in_did_token_proves_ownership() {
    let did = Persona::Priya.did();
    let cases: [(String, bool); 7] = [
        ("Rust. @priyaraman.bsky.social".to_string(), false),
        (format!("Rust. {did}x"), false),
        (format!("Rust. x{did}"), false),
        (format!("Rust. {}", did.to_uppercase()), false),
        (String::new(), false),
        (format!("Rust ({did})."), true),
        (format!("{DMITRI_OLD_DID} {did}"), true),
    ];
    let world = ReviewWorld::new();
    for (bio, verifies) in cases {
        world
            .github
            .set_bio("priyaraman", if bio.is_empty() { None } else { Some(&bio) });
        let mut browser = given_signed_in(&world, Persona::Priya);
        when_verifies_github(&mut browser, "priyaraman");
        assert_eq!(
            browser.page.shows("Verified: github.com/priyaraman"),
            verifies,
            "bio {bio:?} must {}verify:\n{}",
            if verifies { "" } else { "NOT " },
            browser.page.text()
        );
    }
}

/// OW-8
/// ```gherkin
/// @US-BRA-002 @AC-002.4 @I-BRA-4 @adversarial @contract-shape:unbounded-preservation
/// Scenario: A scan cannot be started before ownership is proven
///   Given Priya is signed in but has not verified GitHub
///   When a scan request is sent anyway
///   Then it is refused, no repo is read and no suggestion exists
/// ```
#[test]
fn a_scan_cannot_be_started_before_ownership_is_proven() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    browser.open("/review");
    let token = csrf_token(&browser);
    browser.post("/scan", &[("csrf", token.as_str())]);
    assert!(
        !browser.page.shows("Scanning"),
        "refused, or sent to the GitHub step"
    );
    assert_eq!(world.github.scrape_reads_of("priyaraman"), 0);
    assert!(browser.open("/review").pending_cards().is_empty());
}

/// OW-9
/// ```gherkin
/// @US-BRA-002 @ADR-076 @error @contract-shape:unbounded-preservation
/// Scenario: A renamed or re-registered GitHub account must be verified again
///   Given Priya verified github.com/priyaraman
///   And the account named "priyaraman" now belongs to a different GitHub user
///   When she scans
///   Then nothing is scanned and she is asked to verify again
/// ```
#[test]
fn a_renamed_or_re_registered_github_account_must_be_verified_again() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    world.github.set_user_id("priyaraman", 99_999_999);
    let status = when_scan_finishes(&mut browser);
    assert_eq!(status, "ownership_failed");
    assert_eq!(world.github.scrape_reads_of("priyaraman"), 0);
    assert!(browser.page.shows("verify"), "{}", browser.page.text());
}

/// SI-13 — the scope-fallback mode (DESIGN user decision 2, ADR-073).
/// ```gherkin
/// @US-BRA-001 @NFR-BRA-3 @ADR-073 @C5a @C5b @contract-shape:bounded-change
/// Scenario: When only the broad permission is available, the app says why and still writes only claims and posts
///   Given the app is configured with the broad fallback permission instead of the granular ones
///   When Priya opens the landing page and signs in
///   Then the landing page explains why the broader permission is requested
///   And she is asked for exactly that permission
///   And her approved claim still lands in her claims collection and nowhere else
/// ```
#[test]
#[ignore = "BLOCKED_BY_DEPENDENCY 01-03..01-05: sign-in half green in 01-02; chains given_published (verify/scan/publish)"]
fn with_only_the_broad_permission_the_app_says_why_and_still_writes_only_claims() {
    const FALLBACK: &str = "atproto transition:generic";
    let world =
        ReviewWorld::with_settings(|s| s.extra_env.push(("OAUTH_SCOPES".into(), FALLBACK.into())));
    let mut browser = world.browser();
    let landing = browser.open("/").text();
    assert!(
        landing.contains("broader permission") || landing.contains("broad permission"),
        "{landing}"
    );
    when_signs_in(&mut browser, Persona::Priya);
    assert_eq!(world.atproto.authorizations()[0].scope, FALLBACK);
    drop(browser);
    let mut b = given_published(
        &world,
        Persona::Priya,
        &[priya::TIDEPOOL_DEPENDENCY_PINNING],
    );
    assert!(world
        .atproto
        .write_attempts()
        .iter()
        .all(|w| w.collection == openlore_test_support::CLAIM_COLLECTION));
    assert!(b.open("/review").status == 200);
}
