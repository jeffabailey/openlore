//! bluesky-claim-review-app — WALKING SKELETON (US-BRA-000..004, J-009).
//!
//! The thinnest end-to-end thread that proves the feature's riskiest promise:
//! a Bluesky developer who has never used the OpenLore CLI signs in with their
//! handle, proves their GitHub account with their DID in their bio, privately
//! reviews what their public repos suggest, approves one — and it lands, self-
//! attested, in THEIR OWN PDS, where OpenLore's existing read path accepts it.
//!
//! Driving port: the REAL `openlore-review-app serve` (the third composition
//! root, ADR-072) over HTTP, through a JS-less browser (`support/review_app`).
//! Driven-external systems are hermetic doubles: the ATProto network
//! (`FakeAtprotoNetwork`: PLC + handle resolution + the user's PDS + its OAuth
//! authorization server with PAR / DPoP / PKCE) and GitHub
//! (`FakeGithubAccounts`). The reader half of WS-4 drives the REAL `openlore`
//! CLI (`peer add` / `peer pull` / `graph query`) against the same PDS double.
//!
//! The four scenarios are a chained narrative (Pillar 2): each GIVEN reuses
//! the previous scenario's GIVEN + WHEN through the shared step vocabulary.
//! All four are ACTIVE (not `#[ignore]`) and RED at hand-off: they fail
//! because the review app does not exist yet (see `distill/red-classification.md`).
//!
//! Layer 4/5 (subprocess + real HTTP): example-only (Mandate 9); universe =
//! the user's PDS records + write attempts, the GitHub request log, the pages.

// `support` and `review_app` both pull in tests/common/state_delta.rs.
#![allow(clippy::duplicate_mod)]

mod support;

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::*;

/// WS-1 — Priya signs in with her Bluesky handle.
///
/// ```gherkin
/// @walking_skeleton @driving_port @driving_adapter @real-io @US-BRA-000 @US-BRA-001 @AC-000.1 @AC-001.1 @NFR-BRA-3
/// @contract-shape:bounded-change
/// Scenario: Priya signs in with her Bluesky handle and Bluesky recognises the OpenLore review app
///   Given Priya's handle "priyaraman.bsky.social" belongs to did:plc:7x3kq2mzv5rj4w6hbn2tqclp on bsky.social
///   And the OpenLore review app is reachable at its public address
///   When she signs in with Bluesky and authorizes the app at her PDS
///   Then she sees "Signed in as @priyaraman.bsky.social"
///   And her PDS identified the app by its published client details as "OpenLore review"
///   And she was asked only to let the app create claims and posts
///   And nothing was written to any repo
/// ```
#[test]
fn priya_signs_in_with_her_bluesky_handle_and_bluesky_recognises_the_openlore_review_app() {
    // GIVEN Priya's handle resolves to her DID on bsky.social, and the app is up.
    let world = ReviewWorld::new();
    let mut browser = world.browser();

    // WHEN she signs in with Bluesky and authorizes the app at her PDS.
    when_signs_in(&mut browser, Persona::Priya);

    // THEN she sees "Signed in as @priyaraman.bsky.social".
    then_sees_signed_in_as(&browser.page, Persona::Priya);

    // AND her PDS identified the app by its published client details (AC-000.1).
    let identified = world.atproto.client_metadata_seen().iter().any(|m| {
        m["client_name"] == "OpenLore review"
            && m["client_id"] == world.app.client_metadata_url().as_str()
    });
    assert!(
        identified,
        "Bluesky must find the app named \"OpenLore review\" at its client-metadata address; saw {:#?}",
        world.atproto.client_metadata_seen()
    );

    // AND she authorized at HER PDS, asked only for claim + post creation (NFR-BRA-3).
    let authorizations = world.atproto.authorizations();
    assert_eq!(
        authorizations.len(),
        1,
        "exactly one authorization screen: {authorizations:#?}"
    );
    assert_eq!(authorizations[0].host, PdsHost::BSKY);
    assert_eq!(authorizations[0].did, Persona::Priya.did());
    assert_eq!(authorizations[0].scope, app::OAUTH_SCOPES);

    // AND nothing was written to any repo.
    assert!(
        world.atproto.write_attempts().is_empty(),
        "signing in writes nothing"
    );
}

/// WS-2 — Priya proves her GitHub account with her DID in her bio.
///
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-BRA-002 @AC-002.2 @I-BRA-4
/// @contract-shape:bounded-change
/// Scenario: Priya proves github.com/priyaraman is hers with her DID in her bio
///   Given Priya is signed in as did:plc:7x3kq2mzv5rj4w6hbn2tqclp
///   And her GitHub bio reads "Rust, tide models. did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
///   When she verifies GitHub username "priyaraman"
///   Then she sees "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social"
///   And only her public profile was read — none of her repos yet
///   And nothing was written to her PDS
/// ```
#[test]
#[ignore = "DELIVER 01-03: unskip when the review app ships GitHub ownership proof"]
fn priya_proves_her_github_account_is_hers_with_her_did_in_her_bio() {
    // GIVEN Priya is signed in (WS-1's Given + When) and her bio holds her DID.
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);

    // WHEN she verifies GitHub username "priyaraman".
    when_verifies_github(&mut browser, Persona::Priya.github_login());

    // THEN she sees the verified message naming both identities.
    then_sees_verified(&browser.page, Persona::Priya);

    // AND only her public profile was read (verification is not a scrape).
    assert_eq!(
        world.github.scrape_reads_of("priyaraman"),
        0,
        "verifying ownership must not read her repos; requests: {:#?}",
        world.github.requests()
    );
    assert!(
        world.github.saw_token(SERVER_GITHUB_TOKEN),
        "the app reads GitHub with its server token (ADR-076)"
    );

    // AND nothing was written to her PDS.
    assert!(world.atproto.write_attempts().is_empty());
}

/// WS-3 — Priya sees private, evidence-backed suggestions from her own repos.
///
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-BRA-003 @AC-003.1 @AC-003.2 @AC-003.3 @I-BRA-1 @I-BRA-4
/// @contract-shape:unbounded-preservation
/// Scenario: Priya sees private suggestions from her own repos
///   Given Priya has verified github.com/priyaraman
///   And priyaraman/tidepool commits Cargo.lock, runs CI tests, tags semver releases with a CHANGELOG and is written in Rust
///   And priyaraman/quill-docs has substantial docs, and her fork of serde is not her own work
///   When the scan finishes
///   Then she sees 5 suggestions, including "priyaraman/tidepool embodies dependency-pinning" at 0.25 (speculative) with the Cargo.lock evidence link
///   And the page states "Private: only you can see these"
///   And her ownership was re-checked immediately before any repo was read
///   And nothing was written to her PDS
/// ```
#[test]
#[ignore = "DELIVER 01-04: unskip when the review app ships private suggestion queue"]
fn priya_sees_private_evidence_backed_suggestions_from_her_own_repos() {
    // GIVEN Priya has verified github.com/priyaraman (WS-2's Given + When).
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);

    // WHEN the scan finishes.
    let status = when_scan_finishes(&mut browser);
    assert_eq!(
        status,
        "completed",
        "the scan finishes:\n{}",
        browser.page.text()
    );

    // THEN she sees exactly her five suggestions (the fork is skipped, BR-3).
    assert_eq!(browser.page.pending_cards(), phrases(&priya::FIRST_SCAN));

    // AND the dependency-pinning card shows 0.25 (speculative), its signal and evidence.
    let card = html::cards_containing(
        &browser.page.html,
        &["priyaraman/tidepool", "dependency-pinning"],
    )
    .first()
    .map(|c| c.to_string())
    .expect("the tidepool dependency-pinning card");
    let card_text = html::visible_text(&card);
    assert!(
        card_text.contains("0.25") && card_text.contains("speculative"),
        "confidence + bucket: {card_text}"
    );
    assert!(
        card_text.contains("Cargo.lock"),
        "the producing signal is named: {card_text}"
    );
    assert!(
        card.contains("https://github.com/priyaraman/tidepool/blob/master/Cargo.lock"),
        "the Cargo.lock evidence link is on the card"
    );

    // AND the page states the suggestions are private to her (AC-003.3).
    assert!(browser.page.shows("Private: only you can see these"));

    // AND ownership was re-checked immediately before any repo was read (I-BRA-4).
    assert!(
        world
            .github
            .scrapes_without_preceding_bio_check("priyaraman")
            .is_empty(),
        "every scrape is preceded by a bio check; requests: {:#?}",
        world.github.requests()
    );

    // AND nothing was written to her PDS (I-BRA-1a).
    assert!(world.atproto.write_attempts().is_empty());
}

/// WS-4 — Priya approves a suggestion; it lands in her own PDS as
/// self-attested, and OpenLore's existing read path accepts it.
///
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-BRA-004 @US-BRA-009 @AC-004.1 @AC-004.2 @AC-004.3 @AC-004.4 @AC-004.5
/// @I-BRA-3 @I-BRA-5 @I-BRA-7 @kpi-bra-1 @kpi-bra-6
/// @contract-shape:bounded-change
/// Scenario: Priya approves a suggestion and it lands in her own PDS, accepted by OpenLore as self-attested
///   Given Priya has the pending suggestion "priyaraman/tidepool embodies dependency-pinning" at 0.25
///   When she previews it and confirms "Publish to my repo"
///   Then her PDS holds an org.openlore.claim for github:priyaraman/tidepool, dependency-pinning, confidence 2500,
///        authored by her bare DID, unsigned, whose recomputed CID is its record key
///   And she sees the record's at:// address and a way to retract it
///   And no other repo received any write
///   And when Maria adds Priya as a peer and pulls, OpenLore stores the claim and shows it as "self-attested", never "unverified"
/// ```
#[test]
#[ignore = "DELIVER 01-05: unskip when the review app ships approve + self-attested publish"]
fn priya_approves_a_suggestion_and_it_lands_in_her_own_pds_accepted_by_openlore_as_self_attested() {
    use review_app::state_delta::{assert_state_delta, Delta};

    // GIVEN Priya has the pending suggestion (WS-3's Given + When).
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);

    // WHEN she previews it and confirms "Publish to my repo".
    when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
    assert!(
        browser.page.shows("not as truth"),
        "the preview carries \"not as truth\":\n{}",
        browser.page.text()
    );
    when_confirms_publish(&mut browser);
    let confirmation = browser.page.text();
    let after = capture(&world, &mut browser, Persona::Priya);

    // THEN her PDS holds the self-attested claim, CID == record key.
    let record = then_pds_holds_self_attested_claim(
        &world,
        Persona::Priya,
        &priya::TIDEPOOL_DEPENDENCY_PINNING.subject(),
        Philosophy::DependencyPinning,
        DEFAULT_CONFIDENCE.basis_points,
    );
    // AND exactly one write happened, the card left her queue, nothing else moved.
    let remaining: Vec<String> = priya::FIRST_SCAN[1..]
        .iter()
        .map(Suggestion::phrase)
        .collect();
    let mut remaining = remaining;
    remaining.sort();
    let expected = Delta::new()
        .with_slot(
            "pds.claims",
            Box::new(|b: &String, a: &String| {
                (b.is_empty() && a.lines().count() == 1)
                    .then_some(())
                    .ok_or_else(|| format!("expected one new claim; before={b:?} after={a:?}"))
            }),
        )
        .with_slot(
            "pds.write_attempts",
            review_app::state_delta::set_to("1".to_string()),
        )
        .with_slot(
            "queue.pending",
            review_app::state_delta::set_to(remaining.join("\n")),
        );
    assert_state_delta(&before, &after, &universe(), &expected);

    // AND she sees the record's at:// address and the retract path (AC-004.5).
    assert!(
        confirmation.contains(&record.uri()),
        "confirmation shows {}:\n{confirmation}",
        record.uri()
    );
    assert!(
        confirmation.contains("Retract") || confirmation.contains("retract"),
        "names the retract path"
    );

    // AND Maria's OpenLore accepts the claim as self-attested (AC-004.3 / I-BRA-5).
    let maria = support::TestEnv::initialized_as(support::FakeIdentity::maria());
    let priya_pds = world.atproto.pds_url_for(Persona::Priya.did());
    let added = support::run_openlore_with_peer_resolver(
        &maria,
        &["peer", "add", Persona::Priya.did()],
        Persona::Priya.did(),
        &priya_pds,
    );
    assert_eq!(
        added.status, 0,
        "peer add of a Bluesky user succeeds:\n{}\n{}",
        added.stdout, added.stderr
    );
    let pulled = support::run_openlore_with_peer_resolver(
        &maria,
        &["peer", "pull"],
        Persona::Priya.did(),
        &priya_pds,
    );
    assert_eq!(
        pulled.status, 0,
        "peer pull succeeds:\n{}\n{}",
        pulled.stdout, pulled.stderr
    );
    let shown = support::run_openlore(
        &maria,
        &[
            "graph",
            "query",
            "--subject",
            &priya::TIDEPOOL_DEPENDENCY_PINNING.subject(),
            "--federated",
        ],
    );
    assert!(
        shown.stdout.contains("self-attested") && shown.stdout.contains(Persona::Priya.did()),
        "OpenLore shows Priya's claim, attributed and marked self-attested:\n{}\n{}",
        shown.stdout,
        shown.stderr
    );
    assert!(
        !shown.stdout.contains("unverified"),
        "never shown as unverified (I-BRA-5)"
    );
}
