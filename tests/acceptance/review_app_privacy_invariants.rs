//! bluesky-claim-review-app — cross-cutting privacy & consent invariants
//! (I-BRA-1..8, KPI-BRA-4/4b/5 guardrails) and operability
//! (DEVOPS: health/readiness, startup refusal, log privacy, aggregate KPIs).
//!
//! Each invariant is checked over a WHOLE journey, not one step: the oracle
//! is the user's PDS (every write attempt, every forbidden attempt), the
//! GitHub request log, other people's browsers, and the app's own log stream.
//!
//! Driving port: the REAL `openlore-review-app` (public + loopback admin
//! listeners, one-shot subcommands). Fakes: ATProto network + GitHub.
//! Layer 4: example-only. `#[ignore]`d for one-at-a-time DELIVER.

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::*;

/// The full happy journey for Priya used by the whole-journey invariants:
/// sign in → verify → scan → edit+publish one → decline one → share → retract.
/// Returns her browser. Counts her explicit confirms in `confirms`.
fn priyas_full_journey(world: &ReviewWorld, confirms: &mut usize) -> Browser {
    let mut b = given_pending_suggestions(world, Persona::Priya);
    when_edits_and_previews(&mut b, priya::TIDEPOOL_MEMORY_SAFETY, None, Some("0.70"));
    when_confirms_publish(&mut b);
    *confirms += 1;
    when_previews_approval(&mut b, priya::TIDEPOOL_TEST_DRIVEN);
    when_confirms_publish(&mut b);
    *confirms += 1;
    when_declines(&mut b, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    when_opens_share_preview(&mut b, Persona::Priya);
    b.press("Post to Bluesky");
    *confirms += 1;
    when_opens_retract_preview(&mut b, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    b.press("Confirm retraction");
    *confirms += 1;
    b
}

// =============================================================================
// I-BRA-1 — pending suggestions are private until approved
// =============================================================================

/// PV-1
/// ```gherkin
/// @property @I-BRA-1 @AC-003.4 @NFR-BRA-1 @kpi-bra-4 @adversarial @contract-shape:unbounded-preservation
/// Scenario: Only Priya can see her queue — not Dmitri, not an anonymous visitor
///   Given Priya has 5 pending suggestions
///   When Dmitri (signed in) and an anonymous visitor open her queue, and replay every one of her
///        suggestion actions (preview, edit, decline, approve) with her suggestion ids
///   Then each gets "not found" or a sign-in page, and none sees anything of her suggestions
///   And nothing is written to any PDS and her queue is unchanged
/// ```
#[test]
fn only_priya_can_see_her_queue_not_dmitri_not_an_anonymous_visitor() {
    let world = ReviewWorld::new();
    let mut priya_browser = given_pending_suggestions(&world, Persona::Priya);
    let her_forms: Vec<html::Form> = html::elements(&priya_browser.page.html, "article")
        .into_iter()
        .flat_map(html::forms)
        .collect();
    assert!(!her_forms.is_empty(), "her cards carry actions");
    let before = pending_on_queue(&mut priya_browser);

    let mut dmitri_browser = given_signed_in(&world, Persona::Dmitri);
    let mut anonymous = world.browser();
    for (who, browser) in [
        ("Dmitri", &mut dmitri_browser),
        ("anonymous", &mut anonymous),
    ] {
        let queue = browser.open("/review").text();
        for s in priya::FIRST_SCAN {
            assert!(
                !queue.contains(&s.repo_path()),
                "{who} sees nothing of Priya's queue"
            );
        }
        let own_csrf = csrf_token(browser);
        for form in &her_forms {
            let replay = Browser::with_field(form, "csrf", &own_csrf);
            for (label, _, _) in &replay.buttons {
                browser.submit(&replay, Some(label));
                let page = browser.page.clone();
                assert!(
                    page.status == 404 || page.status == 403 || page.offers("Sign in with Bluesky"),
                    "{who} replaying {label:?} on {} must be refused (got {})",
                    replay.action,
                    page.status
                );
                for s in priya::FIRST_SCAN {
                    assert!(
                        !page.text().contains(&s.repo_path()),
                        "{who} sees nothing of {}",
                        s.phrase()
                    );
                }
            }
        }
    }
    assert!(world.atproto.write_attempts().is_empty());
    assert_eq!(pending_on_queue(&mut priya_browser), before);
}

/// PV-2
/// ```gherkin
/// @property @I-BRA-1 @AC-003.5 @AC-007.2 @AC-008.3 @kpi-bra-4 @contract-shape:unbounded-preservation
/// Scenario: Pending suggestions are never exposed anywhere public
///   Given Priya has 5 pending suggestions and nothing published
///   Then her PDS holds no record of any kind
///   And her public profile mentions none of them
///   And no share preview can be opened for them
/// ```
#[test]
fn pending_suggestions_are_never_exposed_anywhere_public() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    assert!(world.atproto.all_records().is_empty());
    assert!(world.atproto.write_attempts().is_empty());
    let profile = when_opens_profile(&mut browser, Persona::Priya).clone();
    assert!(profile.embodies_phrases().is_empty());
    for s in priya::FIRST_SCAN {
        assert!(
            !profile.text().contains(s.philosophy.slug()),
            "{} not on the profile",
            s.phrase()
        );
    }
    assert!(!profile.offers("Share on Bluesky…"));
}

// =============================================================================
// I-BRA-2 / I-BRA-3 / I-BRA-7 / I-BRA-8 — over a whole journey
// =============================================================================

/// PV-3
/// ```gherkin
/// @property @I-BRA-3 @I-BRA-2 @kpi-bra-4 @kpi-bra-4b @contract-shape:bounded-change
/// Scenario: Every write to her PDS follows one of her explicit confirms, and declines write nothing
///   Given Priya publishes two suggestions (one edited), declines one, shares her profile and retracts one claim
///   Then her PDS received exactly one accepted write per confirm she pressed, and no other write attempt
///   And the declined suggestion appears in no record
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (I-BRA-3 write audit over a journey)"]
fn every_write_to_her_pds_follows_one_of_her_explicit_confirms_and_declines_write_nothing() {
    let world = ReviewWorld::new();
    let mut confirms = 0;
    let _browser = priyas_full_journey(&world, &mut confirms);
    let writes = world.atproto.write_attempts();
    assert_eq!(writes.len(), confirms, "one write per confirm: {writes:#?}");
    assert!(writes
        .iter()
        .all(|w| w.status == 200 && w.repo == Persona::Priya.did()));
    assert!(world
        .atproto
        .all_records()
        .iter()
        .all(|r| !r.value.to_string().contains("documentation-first")));
}

/// PV-4
/// ```gherkin
/// @property @I-BRA-7 @I-BRA-8 @contract-shape:unbounded-preservation
/// Scenario: Writes only ever go to the author's own PDS, and nothing is ever updated or deleted
///   Given Priya and Dmitri each run their full journeys at the same time
///   Then every record in Priya's repo is authored by Priya and was written to her PDS host,
///        and every record in Dmitri's repo by Dmitri on his own PDS host
///   And no update, put or delete was ever attempted on any repo
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (I-BRA-7 + I-BRA-8 over two journeys)"]
fn writes_only_ever_go_to_the_authors_own_pds_and_nothing_is_ever_updated_or_deleted() {
    let world = ReviewWorld::new();
    let mut confirms = 0;
    let _priya = priyas_full_journey(&world, &mut confirms);
    let mut dmitri_browser = given_published(
        &world,
        Persona::Dmitri,
        &[dmitri::FERRITE_DEPENDENCY_PINNING],
    );
    when_opens_retract_preview(
        &mut dmitri_browser,
        Persona::Dmitri,
        dmitri::FERRITE_DEPENDENCY_PINNING,
    );
    dmitri_browser.press("Confirm retraction");
    for w in world.atproto.write_attempts() {
        let owner = if w.repo == Persona::Dmitri.did() {
            Persona::Dmitri
        } else {
            Persona::Priya
        };
        assert_eq!(w.repo, owner.did());
        assert_eq!(w.host, owner.pds_host(), "written to the author's own PDS");
    }
    for r in world.atproto.all_records() {
        if r.collection == openlore_test_support::CLAIM_COLLECTION {
            assert_eq!(
                r.value["author"],
                r.repo.as_str(),
                "every claim is authored by its repo's owner"
            );
        }
    }
    assert!(
        world.atproto.forbidden_write_attempts().is_empty(),
        "create-only (I-BRA-8)"
    );
}

/// PV-5
/// ```gherkin
/// @property @I-BRA-4 @kpi-bra-5 @contract-shape:pure-function
/// Scenario: No repository is ever read without a passing ownership check just before
///   Given Priya scans, rescans, loses her DID from the bio, tries again, restores it and rescans
///   Then every scrape of her repos was immediately preceded by a bio read that found her DID
///   And no repo was read while the DID was missing
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (I-BRA-4 over a journey)"]
fn no_repository_is_ever_read_without_a_passing_ownership_check_just_before() {
    let world = ReviewWorld::new();
    let mut b = given_pending_suggestions(&world, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut b), "completed");
    removes_did_from_bio(&world, Persona::Priya);
    let reads_while_missing = world.github.scrape_reads_of("priyaraman");
    assert_eq!(when_scan_finishes(&mut b), "ownership_failed");
    assert_eq!(
        world.github.scrape_reads_of("priyaraman"),
        reads_while_missing
    );
    puts_did_in_bio(&world, Persona::Priya);
    when_verifies_github(&mut b, "priyaraman");
    assert_eq!(when_scan_finishes(&mut b), "completed");
    assert!(world
        .github
        .scrapes_without_preceding_bio_check("priyaraman")
        .is_empty());
}

/// PV-6
/// ```gherkin
/// @NFR-BRA-2 @I-BRA-3 @adversarial @C6a @contract-shape:unbounded-preservation
/// Scenario: A confirmation without her page's anti-forgery token is refused
///   Given Priya is on a publish preview
///   When the confirmation is submitted without (or with a wrong) anti-forgery token
///   Then it is refused and nothing is written
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (CSRF on every POST)"]
fn a_confirmation_without_her_pages_anti_forgery_token_is_refused() {
    let world = ReviewWorld::new();
    let mut b = given_pending_suggestions(&world, Persona::Priya);
    when_previews_approval(&mut b, priya::TIDEPOOL_DEPENDENCY_PINNING);
    let confirm = b
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, "Publish to my repo"))
        })
        .expect("confirm form");
    for token in ["", "forged-token"] {
        let forged = Browser::with_field(&confirm, "csrf", token);
        b.submit(&forged, Some("Publish to my repo"));
        assert!(
            b.page.status == 403 || b.page.status == 400,
            "refused (got {})",
            b.page.status
        );
    }
    assert!(world.atproto.write_attempts().is_empty());
}

// =============================================================================
// Operability (DEVOPS) — health, readiness, startup refusal, logs, KPIs
// =============================================================================

/// OP-1
/// ```gherkin
/// @DEVOPS @DV-BRA-11 @smoke @contract-shape:pure-function
/// Scenario: A healthy app reports live and ready
///   Given the app started with every probe passing
///   Then "/healthz" and "/readyz" both answer 200
/// ```
#[test]
fn a_healthy_app_reports_live_and_ready() {
    let world = ReviewWorld::new();
    assert_eq!(world.app.get("/healthz").0, 200);
    assert_eq!(world.app.get("/readyz").0, 200);
    assert!(
        world.app.logs().contains("\"app.ready\""),
        "an app.ready event"
    );
}

/// OP-2 — parametrised over the hard probe arms that must refuse start.
/// ```gherkin
/// @DEVOPS @DV-BRA-11 @infrastructure-failure @error @C7a @contract-shape:unbounded-preservation
/// Scenario Outline: The app refuses to start rather than run half-wired
///   Given <fault>
///   When the app starts
///   Then it exits without serving, logging health.startup.refused naming the failing probe
///   Examples:
///     | GitHub rejects the server token (expired PAT)  |
///     | the data key secret is missing                 |
///     | the client signing key secret is missing       |
/// ```
#[test]
fn the_app_refuses_to_start_rather_than_run_half_wired() {
    type Prepare =
        fn(&openlore_test_support::FakeGithubAccounts, &openlore_test_support::FakeAtprotoNetwork);
    type Tweak = fn(&mut AppSettings);
    let cases: [(&str, Prepare, Tweak); 3] = [
        (
            "expired GitHub token",
            |gh, _| gh.reject_token(true),
            |_| {},
        ),
        (
            "missing data key",
            |_, _| {},
            |s| s.omit_secrets.push("data-key"),
        ),
        (
            "missing client key",
            |_, _| {},
            |s| s.omit_secrets.push("client-jwk"),
        ),
    ];
    for (fault, prepare, tweak) in cases {
        let (_gh, _net, startup) = ReviewWorld::launch(prepare, tweak);
        match startup {
            Startup::Ready(_) => panic!("the app must refuse to start with {fault}"),
            Startup::Refused { exit_code, logs } => {
                assert!(
                    exit_code.is_some() && exit_code != Some(0),
                    "{fault}: a non-zero exit"
                );
                assert!(
                    logs.contains("health.startup.refused"),
                    "{fault}: logs\n{logs}"
                );
                assert!(
                    !logs.contains(SERVER_GITHUB_TOKEN),
                    "{fault}: the token never reaches the logs"
                );
            }
        }
    }
}

/// OP-3
/// ```gherkin
/// @DEVOPS @DV-BRA-11 @smoke @contract-shape:pure-function
/// Scenario: The image self-test passes with throwaway keys and no network
///   When the operator runs "openlore-review-app probe --self-test"
///   Then it exits 0
/// ```
#[test]
fn the_image_self_test_passes_with_throwaway_keys_and_no_network() {
    let (code, output) = ReviewApp::run_subcommand(&["probe", "--self-test"]);
    assert_eq!(code, Some(0), "{output}");
}

/// OP-4
/// ```gherkin
/// @DEVOPS @monitoring-A-8 @contract-shape:unbounded-preservation
/// Scenario: The operator is warned two weeks before the GitHub token expires
///   Given GitHub reports the server token expires in 10 days
///   When the app starts and reads GitHub
///   Then it logs github.token.expiring with the days left, and keeps serving
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (token-expiry warning A-8)"]
fn the_operator_is_warned_two_weeks_before_the_github_token_expires() {
    let expires = (chrono::Utc::now() + chrono::Duration::days(10))
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let (_gh, _net, startup) =
        ReviewWorld::launch(|gh, _| gh.set_token_expiration(Some(&expires)), |_| {});
    let Startup::Ready(app) = startup else {
        panic!("a token 10 days from expiry still starts")
    };
    let world_logs = app.logs();
    assert!(world_logs.contains("github.token.expiring"), "{world_logs}");
}

/// OP-5
/// ```gherkin
/// @DEVOPS @DV-BRA-9 @NFR-BRA-2 @privacy @contract-shape:unbounded-preservation
/// Scenario: The logs of a whole journey reveal nothing about anyone
///   Given Priya runs her full journey (sign in, verify, scan, edit, publish, decline, share, retract)
///   Then every log line is a JSON event from the closed catalogue
///   And no line contains a token, a sign-in code, her GitHub bio, her handle, her raw DID,
///       any suggestion's subject, philosophy or evidence, the post text, or the server GitHub token
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (log privacy over a journey)"]
fn the_logs_of_a_whole_journey_reveal_nothing_about_anyone() {
    let world = ReviewWorld::new();
    let mut confirms = 0;
    let _b = priyas_full_journey(&world, &mut confirms);
    let mut secrets: Vec<String> = vec![
        "fake-access-".into(),
        "fake-refresh-".into(),
        "code-".into(),
        "Rust, tide models".into(),
        Persona::Priya.handle().into(),
        Persona::Priya.did().into(),
        SERVER_GITHUB_TOKEN.into(),
        "How I build".into(),
        "github.com/priyaraman".into(),
        "__Host-".into(),
    ];
    for s in priya::FIRST_SCAN {
        secrets.push(s.repo_path());
        secrets.push(s.philosophy.object());
    }
    then_logs_are_closed_and_leak_nothing(&world.app.logs(), &secrets);
}

/// OP-6
/// ```gherkin
/// @DEVOPS @DV-BRA-10 @OD-BRA-11 @kpi @contract-shape:pure-function
/// Scenario: The operator reads aggregate counts that identify no one
///   Given Priya published 2 claims (one edited), declined 1, shared and retracted 1
///   When the operator asks the loopback admin listener for today's counts
///   Then the counts include the sign-in, verification, scan, approvals (one edited), decline, share and retraction
///   And the answer contains no DID, handle or claim content
///   And the public address does not answer for the admin routes
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (aggregate KPI counters via admin listener)"]
fn the_operator_reads_aggregate_counts_that_identify_no_one() {
    let world = ReviewWorld::new();
    let mut confirms = 0;
    let _b = priyas_full_journey(&world, &mut confirms);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (status, body) = world
        .app
        .admin_get(&format!("/admin/kpi?from={today}&to={today}"));
    assert_eq!(status, 200, "{body}");
    let counts: serde_json::Value = serde_json::from_str(&body).expect("JSON sums");
    for (event, at_least) in [
        ("signin.completed", 1),
        ("github.verify.ok", 1),
        ("scan.completed", 1),
        ("suggestion.approved", 2),
        ("suggestion.approved.edited", 1),
        ("suggestion.declined", 1),
        ("share.posted", 1),
        ("retract.posted", 1),
    ] {
        let n = counts[event].as_i64().unwrap_or(0);
        assert!(
            n >= at_least,
            "{event} counted at least {at_least}, got {n}: {body}"
        );
    }
    for leak in [
        Persona::Priya.did(),
        Persona::Priya.handle(),
        "tidepool",
        "memory-safety",
    ] {
        assert!(!body.contains(leak), "aggregate counts never carry {leak}");
    }
    assert_eq!(
        world
            .app
            .get(&format!("/admin/kpi?from={today}&to={today}"))
            .0,
        404
    );
}

/// OP-7
/// ```gherkin
/// @DEVOPS @ADR-076 @C7b @interruption @contract-shape:bounded-change
/// Scenario: A scan interrupted by a restart is offered for resume
///   Given Priya's scan is still running when the app restarts
///   When she opens her queue after the restart
///   Then the interrupted scan is offered for resume, the suggestions persisted so far are kept,
///        and resuming completes it without duplicates
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (interrupted scan resumable after restart)"]
fn a_scan_interrupted_by_a_restart_is_offered_for_resume() {
    let world = ReviewWorld::new();
    let mut b = given_github_verified(&world, Persona::Priya);
    world.github.set_remaining(4_990);
    b.open("/review");
    b.press("Scan my repos");
    let world = world.restart_app();
    let mut b = given_signed_in(&world, Persona::Priya);
    let page = b.open("/review").clone();
    assert!(
        page.offers("Resume scan") || page.scan_status().as_deref() == Some("completed"),
        "an interrupted scan is offered for resume:\n{}",
        page.text()
    );
    if page.offers("Resume scan") {
        assert_eq!(when_scan_finishes(&mut b), "completed");
    }
    assert_eq!(b.page.pending_cards(), phrases(&priya::FIRST_SCAN));
}
