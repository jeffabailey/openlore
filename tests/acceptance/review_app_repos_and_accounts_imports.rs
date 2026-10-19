//! review-app-repos-and-accounts — Release 3 (one shipping unit): private,
//! never-publishable suggestions for any public repo the owner names
//! (US-RRA-005), the publish guard over every request path (US-RRA-006,
//! BR-RRA-1, ADR-089), and removing an imported repo (US-RRA-009).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP. Fakes:
//! `FakeGithubAccounts` (orgs, transfers, private = 404, outage) and the
//! ATProto network. Layer 4: example-only. The BR-RRA-1 guarantee is stated
//! here as an exhaustive table over every publish route × every kind of
//! not-owned suggestion (layer 4 enumerates; Mandate 9/11); its pure rule
//! table is generated in `review_app_repos_and_accounts_core.rs` (CORE-RRA-6).
//!
//! R4 (US-RRA-007, administered repos) is DEFERRED (D-RRA-12): the only
//! assertion here is that today's owned-only rule keeps an imported org repo
//! "Not yours to publish" (IC-1 table).
//!
//! Every scenario is `#[ignore = "DELIVER 03-NN: …"]`.
//!
//! State machine (C2a) of an imported repo, per signed-in DID:
//! `(not imported) --Add repo: owner/name, public, not owned by a listed account, room left--> imported`;
//! `imported --Remove--> (not imported)` (pending deleted, declined keys kept);
//! `imported --a listed account owns it (verify or scan)--> on that account's owned list` (BR-RRA-5).

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::state_delta::{assert_state_delta, set_to, Delta};
use review_app::*;

const BOTH: [Account; 2] = [Account::Priyaraman, Account::PriyaTidal];

/// "N of 10 repos chosen." — the total the choose page prints (BR-RRA-3).
fn chosen_total(browser: &mut Browser) -> String {
    let text = when_opens_choose_repos(browser).text();
    let marker = format!(" of {SCAN_REPO_CAP} repos chosen");
    let at = text
        .find(&marker)
        .unwrap_or_else(|| panic!("no total line on the choose page:\n{text}"));
    text[..at]
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_string()
}

// =============================================================================
// US-RRA-005 — private suggestions for any public repo I name
// =============================================================================

/// IM-1 (Scenario Outline from DISCUSS)
/// ```gherkin
/// @US-RRA-005 @AC-005.1 @AC-005.3 @AC-005.4 @happy @kpi @KPI-RRA-3 @contract-shape:bounded-change
/// Scenario Outline: Priya names a public repo she doesn't own and gets private suggestions
///   Given Priya is signed in and <repo> is a public repo none of her accounts owns
///   And she added "<repo>" (<kind>)
///   When she presses "Scan again"
///   Then <repo> is listed under "Repos you don't own" and is read by that scan
///   And her private queue shows exactly <suggestions>, each marked "Not yours to publish"
///   And the import is counted
///   Examples:
///     | repo            | kind          | suggestions                                                                 |
///     | rust-lang/cargo | ordinary repo | rust-lang/cargo embodies dependency-pinning, semantic-versioning, test-driven |
///     | aishab/ripgrep  | a fork        | aishab/ripgrep embodies dependency-pinning                                  |
/// ```
#[test]
fn priya_names_a_public_repo_she_doesnt_own_and_gets_private_suggestions() {
    let rows = [
        (
            CARGO,
            "ordinary repo",
            vec![
                CARGO.embodies(Philosophy::DependencyPinning),
                CARGO.embodies(Philosophy::SemanticVersioning),
                CARGO.embodies(Philosophy::TestDriven),
            ],
        ),
        (
            AISHAB_RIPGREP_FORK,
            "a fork",
            vec![AISHAB_RIPGREP_FORK.embodies(Philosophy::DependencyPinning)],
        ),
    ];
    for (repo, kind, suggestions) in rows {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Priyaraman);
        given_imported(&mut browser, repo);

        let (status, read) = when_scans(&world, &mut browser);

        assert_eq!(status, "completed", "{kind}");
        assert_eq!(imported_list(&mut browser), [repo.full()], "{kind}");
        assert!(
            read.contains(&repo.full()),
            "{kind} {repo:?} read: {read:?}"
        );
        assert_eq!(
            phrases_about(&mut browser, repo),
            phrases(&suggestions),
            "{kind}"
        );
        let cards = cards_about(&mut browser, repo);
        assert!(
            cards.iter().all(|c| c.text.contains(NOT_YOURS)),
            "{cards:?}"
        );
        assert_eq!(kpi(&world, "import.added"), 1);
    }
}

/// IM-2 (refusal table — every row: nothing added, nothing else changes)
/// ```gherkin
/// @US-RRA-005 @AC-005.1 @AC-005.2 @AC-005.3 @AC-005.6 @D-RRA-T2 @D-RRA-T3 @error @C2b @C6a @C6b @C1b @infrastructure-failure @contract-shape:unbounded-preservation
/// Scenario Outline: A repo that cannot be imported is refused with the cause and the fix
///   Given <setup>
///   When <who> adds "<typed>"
///   Then <who> sees "<message>"
///   And nothing is added and nothing else changes
///   And for a name not shaped owner/name, GitHub is not asked about any repo
///   Examples:
///     | setup                                              | who    | typed                     | message |
///     | Priya is verified as priyaraman                    | Priya  | cargo                     | Enter the repo as owner/name, for example rust-lang/cargo. |
///     | Priya is verified as priyaraman                    | Priya  | rust-lang/                | Enter the repo as owner/name, for example rust-lang/cargo. |
///     | Priya is verified as priyaraman                    | Priya  | /cargo                    | Enter the repo as owner/name, for example rust-lang/cargo. |
///     | Priya is verified as priyaraman                    | Priya  | rust-lang/cargo/issues    | Enter the repo as owner/name, for example rust-lang/cargo. |
///     | Priya is verified as priyaraman                    | Priya  | rust lang/cargo           | Enter the repo as owner/name, for example rust-lang/cargo. |
///     | Priya is verified as priyaraman                    | Priya  | tidal-labs/secret-roadmap | We couldn't find a public repo at github.com/tidal-labs/secret-roadmap. Check the spelling. Private repos can't be read. |
///     | Priya is verified as priyaraman                    | Priya  | priyaraman/sandbar        | github.com/priyaraman/sandbar is yours. Tick it in your repo list instead. |
///     | priya-tidal is listed but needs re-verifying       | Priya  | priya-tidal/reef-ui       | github.com/priya-tidal/reef-ui is yours. Tick it in your repo list instead. |
///     | Priya already added rust-lang/cargo                | Priya  | rust-lang/cargo           | github:rust-lang/cargo is already in your list. |
///     | Dmitri has 10 repos chosen                         | Dmitri | rust-lang/cargo           | You can choose up to 10 repos per scan in total. You have 10 chosen. Untick or remove 1 repo before adding another. |
///     | GitHub drops every connection                      | Priya  | rust-lang/cargo           | GitHub could not be reached, so we couldn't check github.com/rust-lang/cargo. Please try again in a few minutes. Nothing was lost. |
///     | GitHub says it is too busy to answer       | Priya  | rust-lang/cargo           | GitHub could not be reached, so we couldn't check github.com/rust-lang/cargo. Please try again in a few minutes. Nothing was lost. |
/// ```
#[test]
fn a_repo_that_cannot_be_imported_is_refused_with_the_cause_and_the_fix() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Setup {
        PriyaVerified,
        TidalListedUnverified,
        CargoAlreadyAdded,
        DmitriAtLimit,
        GithubOffline,
        GithubRateLimited,
    }
    const SHAPE: &str = "Enter the repo as owner/name, for example rust-lang/cargo.";
    const UNREACHABLE: &str = "GitHub could not be reached, so we couldn't check github.com/rust-lang/cargo. Please try again in a few minutes. Nothing was lost.";
    let rows: [(Setup, &str, String); 12] = [
        (Setup::PriyaVerified, "cargo", SHAPE.into()),
        (Setup::PriyaVerified, "rust-lang/", SHAPE.into()),
        (Setup::PriyaVerified, "/cargo", SHAPE.into()),
        (Setup::PriyaVerified, "rust-lang/cargo/issues", SHAPE.into()),
        (Setup::PriyaVerified, "rust lang/cargo", SHAPE.into()),
        (
            Setup::PriyaVerified,
            "tidal-labs/secret-roadmap",
            "We couldn't find a public repo at github.com/tidal-labs/secret-roadmap. Check the spelling. Private repos can't be read.".into(),
        ),
        (
            Setup::PriyaVerified,
            "priyaraman/sandbar",
            "github.com/priyaraman/sandbar is yours. Tick it in your repo list instead.".into(),
        ),
        (
            Setup::TidalListedUnverified,
            "priya-tidal/reef-ui",
            "github.com/priya-tidal/reef-ui is yours. Tick it in your repo list instead.".into(),
        ),
        (
            Setup::CargoAlreadyAdded,
            "rust-lang/cargo",
            "github:rust-lang/cargo is already in your list.".into(),
        ),
        (
            Setup::DmitriAtLimit,
            "rust-lang/cargo",
            import_over_limit_message(SCAN_REPO_CAP),
        ),
        (Setup::GithubOffline, "rust-lang/cargo", UNREACHABLE.into()),
        (Setup::GithubRateLimited, "rust-lang/cargo", UNREACHABLE.into()),
    ];
    for (setup, typed, message) in rows {
        let world = rra_world();
        let (mut browser, persona) = match setup {
            Setup::DmitriAtLimit => {
                let mut b = given_owner_verified(&world, Account::Dvolkov);
                given_saved_choice(&mut b, Account::Dvolkov, &dmitri_repos()[..SCAN_REPO_CAP]);
                (b, Persona::Dmitri)
            }
            Setup::TidalListedUnverified => {
                let mut b = given_accounts_verified(&world, &BOTH);
                removes_did_from_account_bio(&world, Account::PriyaTidal);
                given_scanned(&world, &mut b);
                (b, Persona::Priya)
            }
            Setup::CargoAlreadyAdded => {
                let mut b = given_owner_verified(&world, Account::Priyaraman);
                given_imported(&mut b, CARGO);
                (b, Persona::Priya)
            }
            _ => (
                given_owner_verified(&world, Account::Priyaraman),
                Persona::Priya,
            ),
        };
        let before = capture_rra(&world, &mut browser, persona);
        match setup {
            Setup::GithubOffline => world.github.set_offline(true),
            Setup::GithubRateLimited => world.github.rate_limit_for(600),
            _ => {}
        }

        when_opens_choose_repos(&mut browser);
        let asked_before = github_mark(&world);

        browser.fill(IMPORT_FIELD, typed);
        browser.press(ADD_REPO);

        // Repo reads the import made (the refusal page may list her own
        // repos again; that listing is not a check of the typed name).
        let repo_reads: Vec<String> = world
            .github
            .requests()
            .into_iter()
            .filter(|r| r.seq > asked_before && r.path.starts_with("/repos/"))
            .map(|r| r.path)
            .collect();
        world.github.set_offline(false);
        world.github.clear_rate_limit();
        assert!(
            browser.page.shows(&message),
            "{setup:?} {typed:?}: expected {message:?}:\n{}",
            browser.page.text()
        );
        if message == SHAPE {
            assert!(
                repo_reads.is_empty(),
                "{typed:?}: a malformed name asks GitHub nothing, but it read {repo_reads:?}"
            );
        }
        let after = capture_rra(&world, &mut browser, persona);
        assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    }
}

/// IM-3
/// ```gherkin
/// @US-RRA-005 @AC-005.5 @C-OOS-7 @D-RRA-8 @edge @contract-shape:bounded-change
/// Scenario: Someone with no verified GitHub account can still look at a repo
///   Given Dmitri is signed in, has no verified GitHub account, and has used 5 of his 6 scans today
///   When he adds "rust-lang/cargo" and presses "Scan again"
///   Then the summary says "No GitHub account to re-check · 1 repo read"
///   And his private queue shows suggestions for rust-lang/cargo marked "Not yours to publish"
///   And that scan was the last of his 6 today
/// ```
#[test]
fn someone_with_no_verified_github_account_can_still_look_at_a_repo() {
    let world = rra_world();
    let mut browser = given_signed_in(&world, Persona::Dmitri);
    given_imported(&mut browser, CARGO);
    given_scans_used(&world, &mut browser, 5);

    given_scanned(&world, &mut browser);

    assert!(
        scan_summary(&mut browser).contains("No GitHub account to re-check · 1 repo read"),
        "{}",
        browser.page.text()
    );
    let cards = cards_about(&mut browser, CARGO);
    assert!(!cards.is_empty() && cards.iter().all(|c| c.text.contains(NOT_YOURS)));
    when_presses_scan(&mut browser);
    assert!(
        browser.page.shows(DAILY_LIMIT_REACHED),
        "{}",
        browser.page.text()
    );
}

// =============================================================================
// US-RRA-006 — suggestions about repos I don't own can never be published
// =============================================================================

/// IC-1 (table: an ordinary repo, and an org repo she may administer — R4 deferred)
/// ```gherkin
/// @US-RRA-006 @AC-006.1 @D-RRA-10 @D-RRA-12 @contract-shape:unbounded-preservation
/// Scenario Outline: An imported suggestion offers no way to publish and says who can
///   Given Priya has a pending suggestion for <repo>, a repo she imported
///   When she opens her queue
///   Then its card says "Not yours to publish. Only an owner of github.com/<owner> can publish claims about this repo."
///   And it has no Select, Approve or Edit control, only Decline
///   Examples:
///     | repo                  | owner      |
///     | rust-lang/cargo       | rust-lang  |
///     | tidal-labs/streamline | tidal-labs |
/// ```
#[test]
fn an_imported_suggestion_offers_no_way_to_publish_and_says_who_can() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut browser, CARGO);
    given_imported(&mut browser, STREAMLINE);
    given_scanned(&world, &mut browser);

    for repo in [CARGO, STREAMLINE] {
        let cards = cards_about(&mut browser, repo);
        assert!(!cards.is_empty(), "{repo:?}");
        let who = format!(
            "Not yours to publish. Only an owner of github.com/{} can publish claims about this repo.",
            repo.owner
        );
        for card in cards {
            assert!(card.text.contains(&who), "{}", card.text);
            assert_eq!(card.controls, [DECLINE], "{repo:?}: Decline only");
        }
    }
}

/// IC-2
/// ```gherkin
/// @US-RRA-006 @AC-006.2 @BR-RRA-1 @adversarial @error @kpi @contract-shape:unbounded-preservation
/// Scenario: A forged publish request for a repo he doesn't own writes nothing
///   Given Sam Ortega imported BurntSushi/ripgrep and has a pending suggestion for it
///   When a publish request for that suggestion reaches the app without the page
///   Then no record is written to Sam's Bluesky repo
///   And Sam sees "github.com/BurntSushi/ripgrep is not owned by an account you verified, so nothing was published. Decline this suggestion, or verify the GitHub account that owns the repo."
/// ```
#[test]
fn a_forged_publish_request_for_a_repo_he_doesnt_own_writes_nothing() {
    let world = rra_world();
    let mut sam = given_signed_in(&world, Persona::Sam);
    given_imported(&mut sam, RIPGREP);
    given_scanned(&world, &mut sam);
    let target = RIPGREP.embodies(Philosophy::DependencyPinning);
    let card = html::cards_containing(
        &sam.open("/review").html.clone(),
        &[&RIPGREP.full(), Philosophy::DependencyPinning.slug()],
    )
    .first()
    .map(|c| c.to_string())
    .unwrap_or_else(|| panic!("no card for {}", target.phrase()));
    let decline = html::forms(&card)
        .into_iter()
        .next()
        .expect("the card's form");
    let forged = html::Form {
        action: "/review/approve".to_string(),
        ..decline
    };

    sam.submit(&forged, None);

    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
    assert!(
        sam.page.shows("github.com/BurntSushi/ripgrep is not owned by an account you verified, so nothing was published. Decline this suggestion, or verify the GitHub account that owns the repo."),
        "{}",
        sam.page.text()
    );
    assert!(cards_about(&mut sam, RIPGREP)
        .iter()
        .any(|c| c.phrases.contains(&target.phrase())));
    assert_eq!(kpi(&world, "publish.refused.not_owned"), 1);
}

/// IC-3 (layer-4 exhaustive table; the generated rule is CORE-RRA-6)
/// ```gherkin
/// @US-RRA-006 @AC-006.2 @BR-RRA-1 @BR-RRA-2 @KPI-RRA-4 @exhaustive-table @adversarial @contract-shape:unbounded-preservation
/// Scenario: No request publishes a suggestion about a repo the owner doesn't own
///   Given Priya has pending suggestions whose repo no currently verified account of hers owns:
///     | kind                         | suggestion                                             |
///     | imported, org-owned          | rust-lang/cargo embodies dependency-pinning            |
///     | owned at scan, transferred   | priyaraman/kelp embodies dependency-pinning            |
///     | listed account, DID removed  | priya-tidal/ingest-bench embodies test-driven          |
///   When any preview, approve, approve-selected, edit-preview, retry or confirm request names that suggestion
///   (each from a fresh start; approve-selected and confirm-selected send it in one batch with one of her own suggestions)
///   Then no record about that repo is written to any Bluesky repo
///   And the suggestion stays pending or hidden as it was, never published
/// ```
#[test]
fn no_request_publishes_a_suggestion_about_a_repo_the_owner_doesnt_own() {
    // The owned carrier sorts before priyaraman/kelp, so a permit wrongly
    // shared per account in a batch would be minted for it first.
    let carrier = HARBOR_CLI.embodies(Philosophy::TestDriven);
    let targets = [
        CARGO.embodies(Philosophy::DependencyPinning),
        KELP.embodies(Philosophy::DependencyPinning),
        INGEST_BENCH.embodies(Philosophy::TestDriven),
    ];
    let forbidden: Vec<String> = targets.iter().map(|t| t.subject()).collect();
    for target in targets {
        for route in PublishRoute::ALL {
            // Each row starts fresh: a refused bio check marks priya-tidal
            // unverified and hides its card (ADR-089 §2), as it should.
            let world = rra_world();
            let mut browser = given_accounts_verified(&world, &BOTH);
            given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
            given_imported(&mut browser, CARGO);
            given_scanned(&world, &mut browser);
            world
                .github
                .transfer_repo(KELP.owner, KELP.name, KELP_AT_TIDAL_LABS.owner);
            removes_did_from_account_bio(&world, Account::PriyaTidal);

            when_requests_publish_via(&world, &mut browser, route, carrier, target);

            let written = claim_subjects(&world, Persona::Priya);
            assert!(
                written.iter().all(|s| !forbidden.contains(s)),
                "{route:?} on {}: wrote {written:?}",
                target.phrase()
            );
            assert!(
                world.atproto.forbidden_write_attempts().is_empty(),
                "{route:?} on {}: create-only, nothing else",
                target.phrase()
            );
        }
    }
}

/// IC-4
/// ```gherkin
/// @US-RRA-006 @AC-006.3 @I-BRA-1 @NFR-RRA-6 @privacy @contract-shape:unbounded-preservation
/// Scenario: Profile and share preview never show imported suggestions
///   Given Priya has published claims and pending suggestions for rust-lang/cargo
///   When she opens her profile and the share preview
///   Then neither mentions rust-lang/cargo
/// ```
///
/// Note: the share half is a regression guard that cannot fail today — a
/// share draft names only the philosophies of her published claims, never a
/// repo subject; the profile half is the one that observes imported rows.
#[test]
fn profile_and_share_preview_never_show_imported_suggestions() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut browser, CARGO);
    given_scanned(&world, &mut browser);
    given_published_one(
        &world,
        &mut browser,
        TIDEPOOL.embodies(Philosophy::TestDriven),
    );
    given_published_one(
        &world,
        &mut browser,
        QUILL_DOCS.embodies(Philosophy::DocumentationFirst),
    );
    assert!(!cards_about(&mut browser, CARGO).is_empty(), "precondition");

    let profile = when_opens_profile(&mut browser, Persona::Priya).text();
    let share = when_opens_share_preview(&mut browser, Persona::Priya).text();

    assert!(!profile.contains(&CARGO.full()), "{profile}");
    assert!(!share.contains(&CARGO.full()), "{share}");
}

/// IC-5 (table over how GitHub fails at confirm)
/// ```gherkin
/// @US-RRA-006 @AC-006.4 @infrastructure-failure @C7a @contract-shape:unbounded-preservation
/// Scenario Outline: GitHub failing at confirm publishes nothing
///   Given Priya is looking at the preview of "priyaraman/tidepool embodies test-driven"
///   And <failure> when she confirms
///   When she presses "Publish to my repo"
///   Then she sees "GitHub could not be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published. Please try again in a few minutes."
///   And nothing is written to her Bluesky repo, the suggestion is still pending and her account keeps its status
///   Examples:
///     | failure                                      |
///     | GitHub drops every connection                |
///     | GitHub says it is too busy to answer |
/// ```
#[test]
fn github_failing_at_confirm_publishes_nothing() {
    for rate_limited in [false, true] {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Priyaraman);
        given_scanned(&world, &mut browser);
        let before = capture_rra(&world, &mut browser, Persona::Priya);
        when_previews_approval(&mut browser, TIDEPOOL.embodies(Philosophy::TestDriven));
        if rate_limited {
            world.github.rate_limit_for(600);
        } else {
            world.github.set_offline(true);
        }

        when_confirms_publish(&mut browser);

        world.github.set_offline(false);
        world.github.clear_rate_limit();
        assert!(
            browser.page.shows("GitHub could not be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published. Please try again in a few minutes."),
            "rate limited {rate_limited}: {}",
            browser.page.text()
        );
        let after = capture_rra(&world, &mut browser, Persona::Priya);
        assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    }
}

/// IC-6a
/// ```gherkin
/// @US-RRA-006 @AC-006.5 @BR-RRA-5 @kpi @contract-shape:bounded-change
/// Scenario: An imported repo moves to the owned list when its owner account is verified
///   Given Priya imported priya-tidal/tidal-sdk-notes and has a pending suggestion for it
///   And priya-tidal has no saved choice and room is left under the limit
///   When she verifies github.com/priya-tidal
///   Then tidal-sdk-notes is no longer under "Repos you don't own"
///   And it is ticked under "github.com/priya-tidal" in that account's saved choice
///   And its suggestion is kept and now offers Approve
///   And the move is counted
/// ```
#[test]
fn an_imported_repo_moves_to_the_owned_list_when_its_owner_account_is_verified() {
    let world = rra_world();
    let mut browser = given_tidal_sdk_notes_imported_with_a_suggestion(&world);

    when_adds_account(&mut browser, Account::PriyaTidal);

    assert!(imported_list(&mut browser).is_empty());
    let page = when_opens_choose_repos(&mut browser).clone();
    assert!(ticked_repos(&page, Account::PriyaTidal).contains(&TIDAL_SDK_NOTES.full()));
    assert!(
        pending_on_queue(&mut browser).contains(&TIDAL_SDK_NOTES_DOCS.phrase()),
        "kept"
    );
    assert!(
        cards_about(&mut browser, TIDAL_SDK_NOTES).iter().any(|c| c
            .controls
            .iter()
            .any(|l| l.contains("Approve"))
            && !c.text.contains(NOT_YOURS)),
        "its card offers Approve and no longer says Not yours to publish"
    );
    assert_eq!(kpi(&world, "import.moved_to_owned"), 1);
}

/// IC-6b (table — OQ-D2: a moved repo follows the owned-repo rules, with the fresh check)
/// ```gherkin
/// @US-RRA-006 @AC-006.5 @BR-RRA-2 @OQ-D2 @contract-shape:bounded-change
/// Scenario Outline: A moved suggestion is published only while its owner account's bio still holds her DID
///   Given tidal-sdk-notes moved to priya-tidal's list when Priya verified that account
///   And she is looking at the preview of "priya-tidal/tidal-sdk-notes embodies documentation-first"
///   And <change>
///   When she presses "Publish to my repo"
///   Then <outcome>
///   Examples:
///     | change                                               | outcome |
///     | nothing changed on GitHub                            | her Bluesky repo holds that one claim |
///     | she removed her DID from the priya-tidal bio meanwhile | she sees "Your DID is no longer in the bio of github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then publish again." and nothing is written |
/// ```
#[test]
#[ignore = "DELIVER 03-18: a moved suggestion publishes through the fresh owner + bio check (OQ-D2)"]
fn a_moved_suggestion_is_published_only_while_its_owner_accounts_bio_holds_her_did() {
    for bio_removed in [false, true] {
        let world = rra_world();
        let mut browser = given_tidal_sdk_notes_imported_with_a_suggestion(&world);
        when_adds_account(&mut browser, Account::PriyaTidal);
        when_previews_approval(&mut browser, TIDAL_SDK_NOTES_DOCS);
        if bio_removed {
            removes_did_from_account_bio(&world, Account::PriyaTidal);
        }

        when_confirms_publish(&mut browser);

        if bio_removed {
            assert!(
                browser.page.shows("Your DID is no longer in the bio of github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then publish again."),
                "{}",
                browser.page.text()
            );
            assert!(world.atproto.write_attempts().is_empty(), "nothing written");
        } else {
            assert_eq!(
                claim_subjects(&world, Persona::Priya),
                [TIDAL_SDK_NOTES.subject()]
            );
        }
    }
}

const TIDAL_SDK_NOTES_DOCS: Suggestion = TIDAL_SDK_NOTES.embodies(Philosophy::DocumentationFirst);

/// GIVEN Priya (only priyaraman verified) imported priya-tidal/tidal-sdk-notes
/// and a scan gave her a pending suggestion for it.
fn given_tidal_sdk_notes_imported_with_a_suggestion(world: &ReviewWorld) -> Browser {
    creates_repo(world, TIDAL_SDK_NOTES);
    let mut browser = given_owner_verified(world, Account::Priyaraman);
    given_imported(&mut browser, TIDAL_SDK_NOTES);
    given_scanned(world, &mut browser);
    browser
}

/// IC-7
/// ```gherkin
/// @US-RRA-006 @AC-006.5 @BR-RRA-5 @BR-RRA-3 @contract-shape:bounded-change
/// Scenario: A scan moves an imported repo that one of her accounts now owns
///   Given Priya imported rust-lang/cargo-plugin-demo and has a pending suggestion for it
///   And the repo was later transferred to priya-tidal, which is on her list and verified
///   When she presses "Scan again"
///   Then the repo appears as priya-tidal/cargo-plugin-demo under "github.com/priya-tidal" and no longer under "Repos you don't own"
///   And it is ticked in priya-tidal's saved choice
///   And her total of repos toward the limit is unchanged
///   And its pending suggestion is kept and shows Approve
/// ```
#[test]
fn a_scan_moves_an_imported_repo_that_one_of_her_accounts_now_owns() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_imported(&mut browser, CARGO_PLUGIN_DEMO);
    given_scanned(&world, &mut browser);
    let total_before = chosen_total(&mut browser);
    world.github.transfer_repo(
        CARGO_PLUGIN_DEMO.owner,
        CARGO_PLUGIN_DEMO.name,
        CARGO_PLUGIN_DEMO_MOVED.owner,
    );

    given_scanned(&world, &mut browser);

    assert!(imported_list(&mut browser).is_empty());
    let page = when_opens_choose_repos(&mut browser).clone();
    assert!(ticked_repos(&page, Account::PriyaTidal).contains(&CARGO_PLUGIN_DEMO_MOVED.full()));
    assert_eq!(
        chosen_total(&mut browser),
        total_before,
        "the total does not change"
    );
    let approvable = pending_on_queue(&mut browser);
    assert!(
        approvable
            .iter()
            .any(|p| p.contains("/cargo-plugin-demo embodies")),
        "its suggestion is kept and shows Approve: {approvable:?}"
    );
}

// =============================================================================
// US-RRA-009 — stop reading a repo I don't own
// =============================================================================

/// IR-1
/// ```gherkin
/// @US-RRA-009 @AC-009.1 @D-RRA-17 @happy @kpi @contract-shape:bounded-change
/// Scenario: Removing an imported repo stops reading it
///   Given Priya imported rust-lang/cargo and has 3 pending suggestions for it
///   When she presses "Remove" next to rust-lang/cargo
///   Then she sees "rust-lang/cargo removed. Its 3 pending suggestions were deleted."
///   And her queue holds no rust-lang/cargo suggestion and nothing else changed
///   And her next scan does not read rust-lang/cargo
/// ```
#[test]
#[ignore = "DELIVER 03-11: removing an import deletes its pending suggestions"]
fn removing_an_imported_repo_stops_reading_it() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut browser, CARGO);
    given_scanned(&world, &mut browser);
    let pending = phrases_about(&mut browser, CARGO).len();
    let before = capture_rra(&world, &mut browser, Persona::Priya);

    when_removes_import(&mut browser, CARGO);

    assert!(
        browser.page.shows(&format!(
            "rust-lang/cargo removed. Its {pending} pending suggestions were deleted."
        )),
        "{}",
        browser.page.text()
    );
    let after = capture_rra(&world, &mut browser, Persona::Priya);
    assert_state_delta(
        &before,
        &after,
        &rra_universe(),
        &Delta::new()
            .with_slot("queue.not_yours", set_to(String::new()))
            .with_slot("imports", set_to(String::new())),
    );
    assert!(!given_scanned(&world, &mut browser).contains(&CARGO.full()));
    assert_eq!(kpi(&world, "import.removed"), 1);
}

/// IR-2
/// ```gherkin
/// @US-RRA-009 @AC-009.2 @BR-1 @C4a @contract-shape:bounded-change
/// Scenario: Re-adding offers suggestions again, but never declined ones
///   Given Priya had pending suggestions and 1 declined suggestion for rust-lang/cargo and then removed the repo
///   And she added "rust-lang/cargo" again
///   When she presses "Scan again"
///   Then the earlier pending suggestions are offered again as new suggestions
///   And the declined suggestion is not offered
/// ```
#[test]
#[ignore = "DELIVER 03-12: removal keeps the import's declined keys (BR-1)"]
fn re_adding_an_imported_repo_offers_suggestions_again_but_never_declined_ones() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut browser, CARGO);
    given_scanned(&world, &mut browser);
    let declined = CARGO.embodies(Philosophy::DependencyPinning);
    when_declines_card(&mut browser, declined);
    let pending = phrases_about(&mut browser, CARGO);
    when_removes_import(&mut browser, CARGO);

    given_imported(&mut browser, CARGO);
    given_scanned(&world, &mut browser);

    assert_eq!(phrases_about(&mut browser, CARGO), pending);
    assert!(!phrases_about(&mut browser, CARGO).contains(&declined.phrase()));
}

/// IR-3
/// ```gherkin
/// @US-RRA-009 @AC-009.3 @BR-RRA-3 @boundary @contract-shape:bounded-change
/// Scenario: Removing frees room under the limit
///   Given Dmitri has 10 of 10 repos chosen, including rust-lang/cargo
///   And he removed rust-lang/cargo
///   When he adds "tidal-labs/streamline"
///   Then tidal-labs/streamline is listed under "Repos you don't own"
/// ```
#[test]
#[ignore = "DELIVER 03-13: removing an import frees room at once"]
fn removing_frees_room_under_the_limit() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Dvolkov);
    given_saved_choice(
        &mut browser,
        Account::Dvolkov,
        &dmitri_repos()[..SCAN_REPO_CAP - 1],
    );
    given_imported(&mut browser, CARGO);

    when_removes_import(&mut browser, CARGO);
    when_imports(&mut browser, &STREAMLINE.full());

    assert_eq!(imported_list(&mut browser), [STREAMLINE.full()]);
}

/// IR-4
/// ```gherkin
/// @US-RRA-009 @AC-009.4 @BR-RRA-4 @C7b @interruption @contract-shape:bounded-change
/// Scenario: Removing an imported repo during a scan stops its read
///   Given a scan is running, paused before it reached rust-lang/cargo
///   When Priya removes rust-lang/cargo
///   Then the scan does not read rust-lang/cargo when it resumes
///   And no rust-lang/cargo suggestion is left in her queue
/// ```
#[test]
#[ignore = "DELIVER 03-14: removal during a scan stops the import's read"]
fn removing_an_imported_repo_during_a_scan_stops_its_read() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &[TIDEPOOL, QUILL_DOCS]);
    given_imported(&mut browser, CARGO);
    let before_pause = given_scan_paused_part_way(&world, &mut browser);
    assert!(
        !before_pause.contains(&CARGO.full()),
        "precondition: cargo not read yet"
    );

    when_removes_import(&mut browser, CARGO);
    let resumed = when_paused_scan_resumes(&world, &mut browser);

    assert!(!resumed.contains(&CARGO.full()), "{resumed:?}");
    assert!(cards_about(&mut browser, CARGO).is_empty());
}

// =============================================================================
// Privacy and purge (I-BRA-1, NFR-RRA-6, C-RRA-4)
// =============================================================================

/// IV-1
/// ```gherkin
/// @I-BRA-1 @NFR-RRA-6 @privacy @adversarial @contract-shape:unbounded-preservation
/// Scenario: One owner never sees or acts on another owner's imports
///   Given Priya imported rust-lang/cargo and has pending suggestions for it
///   And Dmitri, signed in as himself, sees no rust-lang/cargo on his repo list or queue
///   When Sam, signed in as himself, sends the app Priya's own "Decline" and "Remove" requests for rust-lang/cargo
///   Then Sam sees no rust-lang/cargo and none of Priya's suggestions
///   And Priya's imports, queue and published claims are exactly as before
/// ```
#[test]
#[ignore = "DELIVER 03-15: imports and their suggestions are owner-scoped"]
fn one_owner_never_sees_or_acts_on_another_owners_imports() {
    let world = rra_world();
    let mut priya = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut priya, CARGO);
    given_scanned(&world, &mut priya);
    let card = html::cards_containing(&priya.open("/review").html.clone(), &[&CARGO.full()])
        .first()
        .map(|c| c.to_string())
        .expect("Priya's imported card");
    let decline = html::forms(&card).into_iter().next().expect("decline form");
    priya.open("/repos");
    let remove = priya
        .forms()
        .into_iter()
        .find(|f| f.buttons.iter().any(|(l, _, _)| l.contains(REMOVE)))
        .expect("Priya's remove form");
    let before = capture_rra(&world, &mut priya, Persona::Priya);

    let mut dmitri = given_signed_in(&world, Persona::Dmitri);
    let seen = format!(
        "{} {}",
        dmitri.open("/repos").text(),
        dmitri.open("/review").text()
    );
    let mut sam = given_signed_in(&world, Persona::Sam);
    let sams_csrf = csrf_token(&sam);
    for form in [&decline, &remove] {
        sam.submit(&Browser::with_field(form, "csrf", &sams_csrf), None);
    }
    let sam_seen = sam.open("/review").text();

    assert!(!seen.contains(&CARGO.full()), "{seen}");
    assert!(!sam_seen.contains(&CARGO.full()), "{sam_seen}");
    let after = capture_rra(&world, &mut priya, Persona::Priya);
    assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
}

/// IV-2
/// ```gherkin
/// @C-RRA-4 @US-BRA-012 @privacy @contract-shape:bounded-change
/// Scenario: "Disconnect and forget me" forgets her accounts, choices and imports
///   Given Priya has priyaraman and priya-tidal, a saved choice and an imported repo
///   And she confirmed "Disconnect and forget me"
///   When she signs in again
///   Then her account list, her repos you don't own and her queue are empty
///   And her choose-repos page lists no account
/// ```
#[test]
#[ignore = "DELIVER 03-16: purge covers accounts, choices and imports"]
fn disconnect_and_forget_me_forgets_her_accounts_choices_and_imports() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_imported(&mut browser, CARGO);
    given_scanned(&world, &mut browser);

    when_opens_forget_me(&mut browser);
    browser.press("Yes, forget me");
    let mut again = given_signed_in(&world, Persona::Priya);

    assert!(account_list(&mut again).is_empty());
    assert!(imported_list(&mut again).is_empty());
    assert!(queue_cards(&mut again).is_empty());
    let page = when_opens_choose_repos(&mut again).clone();
    for account in BOTH {
        assert!(listed_repos(&page, account).is_empty(), "{account:?}");
    }
}

/// IV-3 (two sessions of one owner at once; D-RRA-D8 per-DID scope lock)
/// ```gherkin
/// @BR-RRA-3 @D-RRA-D8 @C7c @concurrency @boundary @contract-shape:bounded-change
/// Scenario: Two changes at once never take the total past the limit
///   Given Dmitri has 9 repos chosen and "Choose repos" open in two browser tabs: a 10th repo ticked in one, "rust-lang/cargo" typed in the other
///   When he submits both tabs at the same moment
///   Then at most one of them is admitted and the other is refused with the limit
///   And his chosen and imported repos total at most 10
/// ```
#[test]
#[ignore = "DELIVER 03-17: scope changes are serialized per DID (no lost limit check)"]
fn two_changes_at_once_never_take_the_total_past_the_limit() {
    let world = rra_world();
    let all = dmitri_repos();
    let mut first = given_owner_verified(&world, Account::Dvolkov);
    given_saved_choice(&mut first, Account::Dvolkov, &all[..SCAN_REPO_CAP - 1]);
    let mut second = given_signed_in(&world, Persona::Dmitri);
    when_opens_choose_repos(&mut first);
    when_opens_choose_repos(&mut second);

    std::thread::scope(|s| {
        s.spawn(|| {
            when_saves_choice(&mut first, Account::Dvolkov, &all[..SCAN_REPO_CAP]);
        });
        s.spawn(|| {
            when_imports(&mut second, &CARGO.full());
        });
    });

    let ticked = ticked_repos(
        &when_opens_choose_repos(&mut first).clone(),
        Account::Dvolkov,
    );
    let imported = imported_list(&mut first);
    assert!(
        ticked.len() + imported.len() <= SCAN_REPO_CAP,
        "{} chosen + {} imported",
        ticked.len(),
        imported.len()
    );
    assert_eq!(
        ticked.len() + imported.len(),
        SCAN_REPO_CAP,
        "exactly one of the two changes was admitted"
    );
}
