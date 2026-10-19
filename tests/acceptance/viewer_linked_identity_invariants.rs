//! github-did-identity-link — slice 4 guardrails: the viewer's linked-identity
//! region over the WHOLE set of pages (C-VL-1..3, D-VL-5, D-VL-7, D-VL-8).
//!
//! - VL-8 + VL-16: showing linked identities asks GitHub nothing, records no
//!   check and writes nothing; the lines read the same with GitHub down
//!   (universe-bound store guardrail, Mandate 8 waiver D-e).
//! - VL-9 + VL-14: when a link read fails — per request, or at the startup
//!   probe — every page still loads and says the links could not be read;
//!   never "not checked", never a partial line (Earned Trust, D-VL-7).
//! - VL-13: the htmx results swap and the full page carry the same region.
//! - VL-2p (layer 2, proptest): for every listing, the viewer region's text is
//!   the `identity list` text with the backticks removed (D-VL-5 parity).
//!
//! Layers: VL-8/9/13/14/16 are layer-3/4 subprocess tests (REAL `openlore ui`
//! over the REAL DuckDB store; example-only, Mandate 9/11 — each failure mode
//! named). VL-2p is layer 2 (pure renders, 256 generated cases). Its viewer
//! side is a `sut_*` binding that is `todo!()` until DELIVER binds it to ONE
//! call into the production `viewer-domain` render (DD-GDL-16 practice).
//!
//! Every scenario is `#[ignore]`d until DELIVER enables it; each fails today
//! for a business reason (`distill/red-classification.md` § Slice 4).

mod support;

#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::viewer_identities::*;
#[allow(unused_imports)]
use support::*;

use chrono::{DateTime, Duration, TimeZone, Utc};
use claim_domain::identity_link::{
    CheckVerdict, IdentityLink, LinkStanding, LinkedIdentity, ListedLink,
};
use openlore_test_support::PRIYA_DID;
use proptest::prelude::*;

/// Jeff linked github "jeffbailey" and signed one claim; he follows Rachel,
/// whose link to github "rachelk" he checked. Returns the world and the CID
/// of Jeff's claim.
fn two_linked_people() -> (LinkWorld, String) {
    let mut world = LinkWorld::with(&[JEFF_GH, RACHEL_GH]);
    given_i_linked(&world, JEFF_GH, None);
    let cid = given_my_claim(&world);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    given_i_verified(&world, None);
    (world, cid)
}

/// The three store-only pages about a person in [`two_linked_people`].
fn store_pages(cid: &str) -> [(&'static str, PersonPage); 4] {
    [
        ("Jeff's page", PersonPage::Contributor(MY_DID)),
        ("Rachel's page", PersonPage::Contributor(RACHEL_DID)),
        ("github:rachelk", PersonPage::of_account(RACHEL_GH)),
        ("claim detail", PersonPage::ClaimDetail(cid.to_string())),
    ]
}

// =============================================================================
// C-VL-1 / C-VL-2: no GitHub, no write (VL-8, VL-16)
// =============================================================================

/// VL-8 + VL-16
/// ```gherkin
/// @driving_port @real-io @guardrail @us-vl-1 @d-vl-8 @c-vl-1 @c-vl-2 @c7a
/// @contract-shape:unbounded-preservation
/// Scenario Outline: Showing linked identities asks GitHub nothing, changes nothing, and reads the same with GitHub <github>
///   Given Jeff linked github "jeffbailey", signed a claim, and checked Rachel's link to github "rachelk"
///   And GitHub is <github>
///   When Jeff browses the pages about himself and Rachel
///   Then each page shows its person's linked identities, the same lines whether GitHub is up or not
///   And GitHub was not asked anything
///   And Jeff's identity list reads exactly as before he browsed
///   Examples:
///     | github      |
///     | up          |
///     | unreachable |
/// ```
/// "The pages about himself and Rachel": his contributor page, Rachel's, the
/// GitHub person page rachelk and his claim's detail page. PRIMARY Thens
/// observe driving ports only (the pages, the GitHub double's request log,
/// `identity list`). A SECONDARY guardrail (D-e waiver, not the Then) checks
/// the fail-closed store universe is unchanged too.
#[test]
fn showing_linked_identities_asks_github_nothing_changes_nothing_and_reads_the_same_with_github_down(
) {
    let (world, cid) = two_linked_people();
    let pages = store_pages(&cid);
    let mut seen: Vec<(GithubStanding, Vec<Vec<String>>)> = Vec::new();

    for standing in [GithubStanding::Up, GithubStanding::Unreachable] {
        if standing == GithubStanding::Unreachable {
            github_has(&world, GithubTrouble::Unreachable);
        }
        let listed_before = what_jeff_lists(&world, None);
        let asked_before = world.github_paths().len();
        let store_before = capture_link_universe(&world.env);

        let opened = when_i_open_pages(
            &world,
            None,
            &pages.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
        );

        let mut lines = Vec::new();
        for ((name, _), page) in pages.iter().zip(&opened) {
            let row = format!("GitHub {standing:?}, {name}");
            assert_region_with_legend(&row, page);
            lines.push(region_lines(&page.body));
        }
        assert!(
            world.github_paths_since(asked_before).is_empty(),
            "[GitHub {standing:?}] the viewer asks GitHub nothing: {:?}",
            world.github_paths_since(asked_before)
        );
        assert_eq!(
            what_jeff_lists(&world, None),
            listed_before,
            "[GitHub {standing:?}] Jeff's identity list reads exactly as before he browsed"
        );
        // SECONDARY guardrail (D-e waiver): the store universe is unchanged.
        assert_link_store_unchanged_in(&format!("GitHub {standing:?}"), &store_before, &world.env);
        seen.push((standing, lines));
    }

    assert_eq!(
        seen[0].1, seen[1].1,
        "the lines read the same with GitHub up and unreachable"
    );
}

// =============================================================================
// D-VL-7: the reads fail, the page says so (VL-9, VL-14)
// =============================================================================

/// VL-9 + VL-14 (Earned Trust; with-stale-config: a damaged store)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @us-vl-1 @us-vl-3 @d-vl-7 @c6b @c6c @c7a @c7b
/// @contract-shape:unbounded-preservation
/// Scenario Outline: When the link store cannot be read, every page still loads and says so
///   Given Jeff linked github "jeffbailey" and signed a claim, and the network index holds Priya's link
///   And <what fails>
///   When Jeff browses <pages>
///   Then every page loads and each linked-identities region says "Linked identities could not be read from your local store."
///   And no page shows a line, "not checked" or any other standing
///   And the rest of each page shows as before
///   And the viewer <probe>
///   Examples:
///     | what fails                                                     | pages                    | probe |
///     | his proof checks cannot be read on a page request              | the four pages           | did not report a refused link probe at startup |
///     | his stored links cannot be read on a page request              | the four pages           | did not report a refused link probe at startup |
///     | the retractions of his links cannot be read on a page request  | the four pages           | did not report a refused link probe at startup |
///     | his proof-check history is unreadable when the viewer starts   | the four pages and Tobias's contributor page (no link) | reported its link probe refused, and still serves |
/// ```
/// "The four pages": his contributor page, the GitHub person page jeffbailey,
/// his claim's detail page and network search for github:jeff-at-work.
/// Documented limit (review F-7): the per-request rows induce the failure with
/// the debug-only seam; real per-request damage (a lock lost mid-request) is
/// not reproducible in the subprocess harness. The startup row uses a real
/// damaged store.
#[test]
fn when_the_link_store_cannot_be_read_every_page_still_loads_and_says_so() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    let cid = given_my_claim(&world);
    let index_of_priya =
        || given_the_network_index_holds(&world.env, vec![indexed_link(PRIYA_DID, JEFF_WORK_GH)]);
    let pages = [
        PersonPage::Contributor(MY_DID),
        PersonPage::of_account(JEFF_GH),
        PersonPage::ClaimDetail(cid.clone()),
        PersonPage::Search(JEFF_WORK_GH.subject()),
    ];
    // What "the rest of the page" is, per page: its own results.
    let page_marks = [
        format!("id=\"{SCORE_RESULTS_ID}\""),
        format!("id=\"{TRAVERSAL_RESULTS_ID}\""),
        cid.clone(),
        format!("Author: {PRIYA_DID}"),
    ];

    for read in LinkRead::ALL {
        let row = format!("{} fail per request", read.token());
        let viewer = open_viewer(
            &world,
            ViewerOpts {
                failing_read: Some(read),
                indexer: Some(index_of_priya()),
                ..ViewerOpts::default()
            },
        );

        for (page, mark) in pages.iter().zip(&page_marks) {
            let shown = when_i_open(&viewer, page);
            let label = format!("{row}, {}", page.path());
            assert_region_unreadable(&label, &shown);
            assert!(
                shown.body.contains(mark.as_str()),
                "[{label}] the rest of the page shows"
            );
        }
        assert_link_probe_not_refused(&row, &viewer);
    }

    // Last row (destructive): the history is unreadable from startup on.
    let index = index_of_priya();
    given_my_proof_history_cannot_be_read(&world.env);
    let viewer = open_viewer(
        &world,
        ViewerOpts {
            indexer: Some(index),
            ..ViewerOpts::default()
        },
    );

    assert_link_probe_refused(&viewer);
    let startup_pages = pages
        .iter()
        .cloned()
        .chain([PersonPage::Contributor(TOBIAS_DID)]);
    for page in startup_pages {
        let shown = when_i_open(&viewer, &page);
        assert_region_unreadable(&format!("probe refused, {}", page.path()), &shown);
    }
}

// =============================================================================
// An upgraded store (platform review F-2)
// =============================================================================

/// VL-20 (environment: populated pre-v7 store, the frozen IL-13 fixture)
/// ```gherkin
/// @driving_port @real-io @edge @us-vl-1 @us-vl-2 @il-13 @c7a @contract-shape:bounded-change
/// Scenario Outline: A store made before identity links shows every page, and no linked identity it does not hold
///   Given Jeff's store was made before identity links, holding his BurntSushi/ripgrep claim and Rachel's claims
///   And <since then>
///   When Jeff opens <page> on 2026-10-01 at 18:00
///   Then it loads, the viewer reports no refused link probe, and it shows <linked identities>
///   Examples:
///     | since then                                         | page                             | linked identities |
///     | nothing                                            | his contributor page             | no linked-identities region |
///     | nothing                                            | Rachel's contributor page        | no linked-identities region |
///     | nothing                                            | the GitHub person page BurntSushi | no linked-identities region |
///     | Jeff linked github "jeffbailey" on 2026-10-01 09:00 | his contributor page             | "did:plc:test-jeff is github:jeffbailey — verified today" |
///     | Jeff linked github "jeffbailey" on 2026-10-01 09:00 | Rachel's contributor page        | no linked-identities region |
///     | Jeff linked github "jeffbailey" on 2026-10-01 09:00 | the GitHub person page BurntSushi | no linked-identities region |
/// ```
#[test]
fn a_store_made_before_identity_links_shows_every_page_and_no_linked_identity_it_does_not_hold() {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH]);
    let pages = [
        PersonPage::Contributor(MY_DID),
        PersonPage::Contributor(RACHEL_DID),
        PersonPage::GithubPerson("BurntSushi".to_string()),
    ];
    let browse = |row: &str| {
        let viewer = open_viewer(
            &world,
            ViewerOpts {
                now: Some(SAME_DAY),
                ..ViewerOpts::default()
            },
        );
        assert_link_probe_not_refused(row, &viewer);
        pages
            .iter()
            .map(|p| when_i_open(&viewer, p))
            .collect::<Vec<_>>()
    };

    for (page, shown) in pages.iter().zip(browse("as made")) {
        assert_no_region(&format!("as made, {}", page.path()), &shown);
    }

    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));

    let opened = browse("after Jeff linked");
    let listing = region_listing(&opened[0]);
    assert_shown_in(
        "after Jeff linked, his page",
        &listing,
        MY_DID,
        JEFF_GH,
        Shown::Associated,
    );
    assert_check_age(&listing, MY_DID, JEFF_GH, "today", false);
    assert_no_region("after Jeff linked, Rachel's page", &opened[1]);
    assert_no_region("after Jeff linked, github:BurntSushi", &opened[2]);
}

// =============================================================================
// Fragment / full-page parity (VL-13)
// =============================================================================

/// VL-13 (ADR-033 fragment = full-page region)
/// ```gherkin
/// @driving_port @real-io @guardrail @us-vl-1 @us-vl-3 @adr-033 @contract-shape:pure-function
/// Scenario Outline: Results updated in place carry the same linked identities as the full page
///   Given Jeff linked github "jeffbailey", signed a claim, and checked Rachel's link to github "rachelk"
///   And the network index holds Rachel's link
///   And Jeff has <page> open
///   When the page updates its results in place
///   Then the updated results carry the same linked-identities region as the full page, line for line
///   Examples:
///     | page                               |
///     | his contributor page               |
///     | Rachel's contributor page          |
///     | the GitHub person page rachelk     |
///     | his claim's detail page            |
///     | network search for github:rachelk  |
/// ```
#[test]
fn the_results_swap_and_the_full_page_carry_the_same_linked_identities() {
    let (world, cid) = two_linked_people();
    let index =
        given_the_network_index_holds(&world.env, vec![indexed_link(RACHEL_DID, RACHEL_GH)]);
    let viewer = open_viewer(
        &world,
        ViewerOpts {
            indexer: Some(index),
            ..ViewerOpts::default()
        },
    );
    let pages = store_pages(&cid)
        .into_iter()
        .chain([("network search", PersonPage::Search(RACHEL_GH.subject()))]);

    for (name, page) in pages {
        let full = viewer.get(&page.path());
        let swap = viewer.get_htmx(&page.path());

        assert_region_with_legend(&format!("{name}, full page"), &full);
        assert_eq!(
            regions_html(&swap.body),
            regions_html(&full.body),
            "[{name}] the swap carries the same region as the full page;\n--- swap ---\n{}",
            swap.body
        );
    }
}

// =============================================================================
// VL-2p — layer 2: viewer text == `identity list` text (D-VL-5)
// =============================================================================
//
// Oracle waiver (review F4, D-VL-T11): the oracle is the production CLI render
// `cli::render::render_linked_identities` (used below in the property body,
// `let expected = cli::render::render_linked_identities(&listed, now)`), an internal function, because a
// layer-2 property cannot spawn a subprocess per case. It is the text the
// `identity list` driving port prints (pinned by IR-1..IR-18b through the real
// binary), and the subprocess cross-check of THIS parity — page vs a real
// `openlore identity list --did` run at the same instant — is VL-2 (10 rows)
// and VL-15 (`assert_reads_as_listed`).

/// Binding seam (Mandate 7, DD-GDL-16 practice): ONE call into the production
/// `viewer-domain` render of a region holding `listed` as of `now`
/// (`LinkedIdentities::Lines`), returned as HTML. `cli` reaches it through a
/// `viewer-domain` dev-dependency (D-VL-T11: no production edge).
fn sut_render_region(listed: &[ListedLink], now: DateTime<Utc>) -> String {
    viewer_domain::render_linked_identities_region(&viewer_domain::LinkedIdentities::Lines {
        listed: listed.to_vec(),
        now,
    })
    .into_string()
}

const PARITY_DIDS: [&str; 3] = [
    "did:plc:test-jeff",
    "did:plc:rachel-test",
    "did:web:example.org",
];

fn arb_verdict() -> impl Strategy<Value = CheckVerdict> {
    prop::sample::select(CheckVerdict::ALL.to_vec())
}

fn arb_standing(now: DateTime<Utc>) -> impl Strategy<Value = LinkStanding> {
    let checked = (0i64..90, 0i64..86_400)
        .prop_map(move |(days, secs)| now - Duration::days(days) - Duration::seconds(secs));
    prop_oneof![
        checked
            .clone()
            .prop_map(|checked_at| LinkStanding::Associated { checked_at }),
        Just(LinkStanding::NotChecked),
        (
            arb_verdict().prop_filter("a failed proof", |v| *v != CheckVerdict::Verified),
            checked
        )
            .prop_map(|(verdict, checked_at)| LinkStanding::ProofGone {
                verdict,
                checked_at
            }),
        Just(LinkStanding::Contested),
    ]
}

fn arb_listed_link(now: DateTime<Utc>) -> impl Strategy<Value = ListedLink> {
    (
        prop::sample::select(PARITY_DIDS.to_vec()),
        "[A-Za-z0-9][A-Za-z0-9-]{0,38}",
        1u64..100_000_000,
        arb_standing(now),
        prop::collection::btree_set("bafyrei[a-z2-7]{8}", 0..4),
    )
        .prop_map(|(did, login, user_id, standing, records)| ListedLink {
            identity: LinkedIdentity {
                link: IdentityLink {
                    did: did.to_string(),
                    login,
                    user_id,
                },
                standing,
            },
            record_ids: records.into_iter().collect(),
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// VL-2p
    /// ```gherkin
    /// @property @us-vl-1 @d-vl-5 @c3 @contract-shape:pure-function
    /// Property: For every listing, the viewer's lines are the identity list lines without backticks
    ///   Given any 1 to 6 listed links, each verified, not checked, proof gone or contested, checked up to 90 days ago, naming 0 to 3 records
    ///   When the viewer renders them as one linked-identities region
    ///   Then the region's lines, read as text, equal "identity list" for the same links at the same moment, with the backticks removed
    /// ```
    #[test]
    fn for_every_listing_the_viewers_lines_are_the_identity_list_lines_without_backticks(
        listed in prop::collection::vec(
            arb_listed_link(Utc.with_ymd_and_hms(2026, 10, 19, 12, 0, 0).unwrap()),
            1..6,
        )
    ) {
        let now = Utc.with_ymd_and_hms(2026, 10, 19, 12, 0, 0).unwrap();
        let expected: Vec<String> = cli::render::render_linked_identities(&listed, now)
            .lines()
            .map(|l| l.replace('`', ""))
            .collect();

        let region = sut_render_region(&listed, now);

        let lines: Vec<String> = line_items_html(&region).iter().map(|li| html_text(li)).collect();
        prop_assert_eq!(lines, expected);
    }
}
