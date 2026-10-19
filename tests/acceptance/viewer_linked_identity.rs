//! github-did-identity-link — slice 4 acceptance: the viewer's linked-identity
//! line (design `slice-4-viewer-linked-identity.md`, D-VL-1..11, C-VL-1..3;
//! stories US-VL-1..3, recorded as UC-11).
//!
//! The rule under test: every page about one person — a contributor
//! (`/score?contributor=`), a GitHub person (`/project?subject=github:`), a
//! claim's author (`/claims/{cid}`) and each author group of network search —
//! shows that person's linked identities exactly as `identity list` prints
//! them, from this reader's last local checks. The viewer never asks GitHub,
//! never writes, never shows a link that is not one (third-party, flawed,
//! retracted, superseded, from a removed peer), and never hides a contested
//! or failed proof.
//!
//! Lifecycle of what a page shows for one (DID, account) pair (C2a):
//!
//! ```text
//!   no link ──link published──▶ Not checked ──reader checks: verified──▶ Verified (≤30 d)
//!      ▲                           │    ▲                                   │ 31 d ▶ Verified + re-check hint
//!      │                           │    └──check fails── Proof gone ◀───────┤
//!      │                           └─ second DID verified ─▶ Contested ◀────┘
//!      └── retracted / superseded / peer removed (hidden) ◀── any state
//!   any state ──link reads fail──▶ "could not be read" (never "not checked")
//! ```
//!
//! Strategy C (house, DD-GDL-2): REAL `openlore ui` viewer subprocess over the
//! REAL DuckDB store, seeded through the REAL identity / peer / claim verbs
//! (`support::identity_links`); FAKE (driven-external only)
//! `FakeGithubAccounts` (wired to the viewer only to prove it is never asked),
//! `PeerPds`, `FakePds`, `FakeIdentity`, `FakeIngestServer` (the network index
//! for `/search` is the REAL `openlore-indexer`). Layer 3/4, example-only
//! (Mandate 9); same-shape scenarios are table loops with the row label in
//! every assertion message (DD-GDL-5). PRIMARY Thens observe the rendered page;
//! the store universe is a secondary guardrail (D-e), used in the invariants
//! file.
//!
//! Every scenario is `#[ignore]`d until DELIVER enables it (one at a time; the
//! step numbers are DISTILL's proposed order — see `distill/test-scenarios.md`
//! § Slice 4). Each fails today for a business reason: the viewer shows no
//! linked-identity region yet (`distill/red-classification.md`).
//!
//! Build-before-run: `cargo build -p cli --bin openlore` (and
//! `-p openlore-indexer` for the `/search` scenario).

mod support;

#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::viewer_identities::*;
#[allow(unused_imports)]
use support::*;

/// A world where Rachel published her link to github "rachelk", her bio names
/// her DID and Jeff checked it (verified today).
fn rachels_checked_link() -> LinkWorld {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    given_i_verified(&world, Some(LINKED_AT));
    world
}

// =============================================================================
// Walking skeleton
// =============================================================================

/// VL-1 (walking skeleton for slice 4). Environment: clean store. The
/// with-stale-config environment is N/A for the skeleton (covered by VL-2's
/// 31-day rows and VL-15's future-dated check; `test-scenarios.md` § Slice 4).
/// ```gherkin
/// @walking_skeleton @driving_port @driving_adapter @real-io @us-vl-1 @d-vl-1 @d-vl-5 @kpi-gdl-1
/// @contract-shape:unbounded-preservation
/// Scenario: Rachel's contributor page shows her GitHub account as verified
///   Given Jeff follows Rachel, who published a link to github "rachelk" that her bio proves
///   And Jeff checked her link on 2026-10-01 at 09:00
///   When Jeff opens Rachel's contributor page in his viewer on 2026-10-01 at 18:00
///   Then he sees "did:plc:rachel-test is github:rachelk — verified today" under "Linked identities"
///   And her DID leads to her contributor page and the account to the GitHub person's page
///   And a legend says these are as he last checked them and the viewer never asks GitHub
///   And GitHub was not asked anything while the page was shown
/// ```
#[test]
#[ignore = "DELIVER 04-01: walking skeleton — the linked-identity region on /score"]
fn rachels_contributor_page_shows_her_github_account_as_verified() {
    let world = rachels_checked_link();
    let asked_before = world.github_paths().len();

    let page = when_i_open_page(&world, Some(SAME_DAY), &PersonPage::Contributor(RACHEL_DID));

    assert_region_with_legend("Rachel's page", &page);
    let listing = region_listing(&page);
    assert_shown(&listing, RACHEL_DID, RACHEL_GH, Shown::Associated);
    assert_check_age(&listing, RACHEL_DID, RACHEL_GH, "today", false);
    assert_line_links_to_both_people("Rachel's page", &page, RACHEL_DID, RACHEL_GH);
    assert!(
        world.github_paths_since(asked_before).is_empty(),
        "the viewer asks GitHub nothing: {:?}",
        world.github_paths_since(asked_before)
    );
}

// =============================================================================
// Every standing, word for word (US-VL-1, D-VL-5)
// =============================================================================

/// VL-2 + VL-18 (parity, boundary and contested-age rows; layer-2 companion:
/// VL-2p in `viewer_linked_identity_invariants.rs`). Every check is recorded
/// by the real `identity verify` against the GitHub double (review VL-M3); no
/// row is seeded.
/// ```gherkin
/// @driving_port @real-io @boundary @us-vl-1 @d-vl-5 @u-8 @u-9 @c1b @c5a @contract-shape:pure-function
/// Scenario Outline: Every link standing reads in the viewer exactly as Jeff's identity list prints it
///   Given Jeff linked github "jeffbailey" and github "jeff-at-work" on 2026-10-01 at 09:00
///   And Rachel published two link records R1 and R2 for github "rachelk", one record R3 for github "rachel-old", and one for github "jeff-at-work"
///   And the bio of github "jeff-at-work" names both Jeff's DID and Rachel's DID, and Jeff checked that account on 2026-10-01 at 09:00
///   And Jeff's latest check of github "rachelk" is <latest check of rachelk>
///   When Jeff opens the contributor page of <did> on <opened on>
///   Then its linked identities read exactly as Jeff's identity list for <did> prints them at that moment
///   And it shows the line "<line>"
///   Examples:
///     | did                 | latest check of rachelk                                       | opened on        | line |
///     | did:plc:test-jeff   | none                                                               | 2026-10-01 18:00 | did:plc:test-jeff is github:jeffbailey — verified today |
///     | did:plc:test-jeff   | none                                                               | 2026-10-01 18:00 | github:jeff-at-work is contested — did:plc:test-jeff claims it |
///     | did:plc:test-jeff   | none                                                               | 2026-11-01 09:00 | did:plc:test-jeff is github:jeffbailey — verified 31 days ago; re-check with openlore identity verify |
///     | did:plc:test-jeff   | none                                                               | 2026-11-01 09:00 | github:jeff-at-work is contested — did:plc:test-jeff claims it |
///     | did:plc:rachel-test | none                                                               | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachelk — not checked (records R1, R2) |
///     | did:plc:rachel-test | none                                                               | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachel-old — not checked (record R3) |
///     | did:plc:rachel-test | 10:00, after its bio stopped naming a DID                          | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachelk — proof gone (did_missing), checked today (records R1, R2) |
///     | did:plc:rachel-test | 11:00, after its bio named a stranger's DID                        | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachelk — proof gone (different_did), checked today (records R1, R2) |
///     | did:plc:rachel-test | 12:00, after its bio was removed                                   | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachelk — proof gone (no_bio), checked today (records R1, R2) |
///     | did:plc:rachel-test | 13:00, after the account was deleted                               | 2026-10-01 18:00 | did:plc:rachel-test claims github:rachelk — proof gone (account_not_found), checked today (records R1, R2) |
/// ```
/// "none" means Jeff never checked github "rachelk"; a timed entry means he
/// checked it with `identity verify` on 2026-10-01 at that time, after the
/// stated change on GitHub. R1..R3 are the record IDs `peer pull` stored (in ascending order on a line);
/// row 4 is VL-18 (a contested line past 30 days carries no re-check hint).
#[test]
#[ignore = "DELIVER 04-02: every standing reads as `identity list` prints it"]
fn every_link_standing_reads_in_the_viewer_exactly_as_identity_list_prints_it() {
    let mut world = LinkWorld::with(&[JEFF_GH, JEFF_WORK_GH, RACHEL_GH, RACHEL_OLD_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    given_i_linked(&world, JEFF_WORK_GH, Some(LINKED_AT));
    let first = link_spec(RACHEL_DID, RACHEL_GH);
    let second = PeerClaimSpec {
        composed_at: "2026-10-01T08:30:00Z".to_string(),
        ..first.clone()
    };
    let cids = given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![
            first,
            second,
            link_spec(RACHEL_DID, RACHEL_OLD_GH),
            link_spec(RACHEL_DID, JEFF_WORK_GH),
        ],
    );
    bio_names_both(&world, JEFF_WORK_GH, MY_DID, RACHEL_DID);
    let checked = when_i_verify_at(&world, &["--github", JEFF_WORK_GH.login], Some(LINKED_AT));
    assert_verify_reported(&checked, JEFF_WORK_GH, Verdict::Verified);
    let r1_r2 = records_named(&cids[..2]);
    let r3 = records_named(&cids[2..3]);

    // One row = (label, whose page, opened on, what happens to rachelk on
    // GitHub before Jeff checks it at the given instant, expected lines).
    // Rows run in order: a row's check is the newest when its page is opened.
    type GithubFate = Option<(fn(&LinkWorld), Verdict, &'static str)>;
    type Row = (&'static str, &'static str, &'static str, GithubFate);
    let rows: [Row; 6] = [
        ("jeff, same day", MY_DID, SAME_DAY, None),
        ("jeff, 31 days later", MY_DID, THIRTY_ONE_DAYS_LATER, None),
        ("rachel, not checked", RACHEL_DID, SAME_DAY, None),
        (
            "rachel, did_missing",
            RACHEL_DID,
            SAME_DAY,
            Some((
                |w| bio_drops_the_did(w, RACHEL_GH),
                Verdict::DidMissing,
                "2026-10-01T10:00:00Z",
            )),
        ),
        (
            "rachel, different_did",
            RACHEL_DID,
            SAME_DAY,
            Some((
                |w| bio_names(w, RACHEL_GH, STRANGER_DID),
                Verdict::DifferentDid,
                "2026-10-01T11:00:00Z",
            )),
        ),
        (
            "rachel, no_bio",
            RACHEL_DID,
            SAME_DAY,
            Some((
                |w| bio_is(w, RACHEL_GH, None),
                Verdict::NoBio,
                "2026-10-01T12:00:00Z",
            )),
        ),
    ];
    let deleted: Row = (
        "rachel, account_not_found",
        RACHEL_DID,
        SAME_DAY,
        Some((
            |w| w.github.delete_account(RACHEL_GH.login),
            Verdict::AccountNotFound,
            "2026-10-01T13:00:00Z",
        )),
    );

    for (row, did, now, fate) in rows.into_iter().chain([deleted]) {
        if let Some((happens, verdict, at)) = fate {
            happens(&world);
            let rechecked = when_i_verify_at(&world, &["--github", RACHEL_GH.login], Some(at));
            assert_verify_reported(&rechecked, RACHEL_GH, verdict);
        }
        let listed = when_i_list_with(&world, &["--did", did], Some(now));

        let page = when_i_open_page(&world, Some(now), &PersonPage::Contributor(did));

        assert_reads_as_listed(row, &page, &listed);
        let l = region_listing(&page);
        match (did, fate) {
            (MY_DID, _) if now == SAME_DAY => {
                assert_shown_in(row, &l, MY_DID, JEFF_GH, Shown::Associated);
                assert_check_age(&l, MY_DID, JEFF_GH, "today", false);
                assert_shown_in(row, &l, MY_DID, JEFF_WORK_GH, Shown::Contested);
            }
            (MY_DID, _) => {
                assert_check_age(&l, MY_DID, JEFF_GH, "31 days ago", true);
                assert_shown_in(row, &l, MY_DID, JEFF_WORK_GH, Shown::Contested);
                let contested = link_line(&l.stdout, MY_DID, JEFF_WORK_GH).unwrap_or_default();
                assert!(
                    !contested.contains("re-check"),
                    "[{row}] a contested line carries no re-check hint (U-8, VL-18): {contested:?}"
                );
            }
            (_, None) => {
                assert_shown_in(row, &l, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
                assert_shown_in(row, &l, RACHEL_DID, RACHEL_OLD_GH, Shown::NotChecked);
                let single = link_line(&l.stdout, RACHEL_DID, RACHEL_OLD_GH).unwrap_or_default();
                assert!(
                    single.ends_with(&r3),
                    "[{row}] one record reads ` (record R3)`, `{r3}`: {single:?}"
                );
                assert_shown_in(row, &l, RACHEL_DID, JEFF_WORK_GH, Shown::Contested);
            }
            (_, Some((_, verdict, _))) => {
                assert_shown_in(row, &l, RACHEL_DID, RACHEL_GH, Shown::ProofGone(verdict));
                let line = link_line(&l.stdout, RACHEL_DID, RACHEL_GH).unwrap_or_default();
                assert!(
                    line.contains("checked today"),
                    "[{row}] the check's age: {line:?}"
                );
            }
        }
        if did == RACHEL_DID {
            let line = link_line(&l.stdout, RACHEL_DID, RACHEL_GH).unwrap_or_default();
            assert!(
                line.ends_with(&r1_r2),
                "[{row}] Rachel's two records read as one line naming both `{r1_r2}`: {line:?}"
            );
        }
    }
}

// =============================================================================
// The GitHub person's page (US-VL-2, D-VL-6)
// =============================================================================

/// VL-3
/// ```gherkin
/// @driving_port @real-io @edge @us-vl-2 @d-vl-6 @u-6 @c1a @contract-shape:pure-function
/// Scenario Outline: A GitHub person's page shows the DID that linked the account, in any spelling
///   Given Jeff linked github "jeffbailey" and it is verified
///   When Jeff opens the GitHub person page for github "<spelling>"
///   Then he sees "did:plc:test-jeff is github:jeffbailey — verified today", word for word the same on every spelling
///   Examples:
///     | spelling   |
///     | jeffbailey |
///     | JeffBailey |
///     | JEFFBAILEY |
/// ```
#[test]
#[ignore = "DELIVER 04-03: the GitHub person page matches the last-seen login in any case"]
fn a_github_persons_page_shows_the_did_that_linked_the_account_in_any_spelling() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    let spellings = ["jeffbailey", "JeffBailey", "JEFFBAILEY"];
    let pages: Vec<PersonPage> = spellings
        .iter()
        .map(|s| PersonPage::GithubPerson(s.to_string()))
        .collect();

    let opened = when_i_open_pages(&world, None, &pages);

    let canonical = region_lines(&opened[0].body);
    for (spelling, page) in spellings.iter().zip(&opened) {
        let row = format!("github:{spelling}");
        assert_region_with_legend(&row, page);
        let listing = region_listing(page);
        assert_shown_in(&row, &listing, MY_DID, JEFF_GH, Shown::Associated);
        assert_line_links_to_both_people(&row, page, MY_DID, JEFF_GH);
        assert_eq!(
            region_lines(&page.body),
            canonical,
            "[{row}] the same words on every spelling"
        );
    }
}

/// VL-17a (D-VL-6 known limit; chained on VL-3)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @error @us-vl-2 @d-vl-6 @contract-shape:pure-function
/// Scenario Outline: Until Jeff re-checks, a renamed account's line stays on its old login's page
///   Given Jeff linked github "jeffbailey" on 2026-10-01 at 09:00 and it is verified
///   And he renamed his account to "jeff-b", and a stranger took the login "jeffbailey"
///   When Jeff opens the GitHub person page for "<login>" on 2026-10-01 at 18:00
///   Then it shows <linked identities>
///   Examples:
///     | login      | linked identities |
///     | jeffbailey | "did:plc:test-jeff is github:jeffbailey — verified today", as he last checked it |
///     | jeff-b     | no linked-identities region |
/// ```
#[test]
#[ignore = "DELIVER 04-03: a renamed account's line stays on its old login until re-check (D-VL-6)"]
fn until_jeff_rechecks_a_renamed_accounts_line_stays_on_its_old_logins_page() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    my_account_meets(&world, AccountFate::RenamedAndLoginReRegistered);

    let opened = when_i_open_pages(
        &world,
        Some(SAME_DAY),
        &[
            PersonPage::of_account(JEFF_GH),
            PersonPage::of_account(JEFF_RENAMED),
        ],
    );

    assert_shown_in(
        "github:jeffbailey",
        &region_listing(&opened[0]),
        MY_DID,
        JEFF_GH,
        Shown::Associated,
    );
    assert_no_region("github:jeff-b", &opened[1]);
}

/// VL-17b (D-VL-6; chained on VL-17a: its Given is VL-17a's Given + When)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @error @us-vl-2 @d-vl-6 @c4a @contract-shape:bounded-change
/// Scenario: When Jeff re-checks, a renamed account's line moves to its new login's page
///   Given Jeff linked github "jeffbailey" on 2026-10-01 at 09:00, renamed it to "jeff-b", and a stranger took "jeffbailey"
///   And Jeff has not re-checked since, and has the GitHub person page for "jeffbailey" open
///   When Jeff re-checks his links on 2026-10-01 at 09:00
///   Then on 2026-10-01 at 18:00 the page for "jeff-b" shows "did:plc:test-jeff is github:jeff-b — verified today"
///   And the page for "jeffbailey" shows no linked-identities region
/// ```
#[test]
#[ignore = "DELIVER 04-03: a renamed account's line moves on re-check (D-VL-6)"]
fn when_jeff_rechecks_a_renamed_accounts_line_moves_to_its_new_logins_page() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    my_account_meets(&world, AccountFate::RenamedAndLoginReRegistered);
    when_i_open_page(&world, Some(SAME_DAY), &PersonPage::of_account(JEFF_GH));

    given_i_verified(&world, Some(LINKED_AT));

    let after = when_i_open_pages(
        &world,
        Some(SAME_DAY),
        &[
            PersonPage::of_account(JEFF_GH),
            PersonPage::of_account(JEFF_RENAMED),
        ],
    );
    assert_no_region("after re-check, github:jeffbailey", &after[0]);
    let listing = region_listing(&after[1]);
    assert_shown_in(
        "after re-check, github:jeff-b",
        &listing,
        MY_DID,
        JEFF_RENAMED,
        Shown::Associated,
    );
    assert_check_age(&listing, MY_DID, JEFF_RENAMED, "today", false);
}

/// VL-4
/// ```gherkin
/// @driving_port @real-io @error @us-vl-1 @us-vl-2 @gdl-d14 @c-vl-3 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario Outline: A contested account reads contested on every page about it
///   Given the bio of github "jeffbailey" names both Jeff's DID and Rachel's DID
///   And both Jeff and Rachel linked github "jeffbailey", and Jeff re-checked every link
///   When Jeff opens <page>
///   Then it shows "github:jeffbailey is contested — <did> claims it"
///   And it never says anyone is github:jeffbailey
///   Examples:
///     | page                                   | did                 |
///     | did:plc:test-jeff's contributor page   | did:plc:test-jeff   |
///     | did:plc:rachel-test's contributor page | did:plc:rachel-test |
///     | the GitHub person page jeffbailey      | did:plc:test-jeff   |
///     | the GitHub person page jeffbailey      | did:plc:rachel-test |
/// ```
#[test]
#[ignore = "DELIVER 04-03: a contested account reads contested on each page"]
fn a_contested_account_reads_contested_on_every_page_about_it() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_names_both(&world, JEFF_GH, MY_DID, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, JEFF_GH)],
    );
    given_i_verified(&world, None);
    let rows: [(&str, PersonPage, &[&str]); 3] = [
        ("Jeff's page", PersonPage::Contributor(MY_DID), &[MY_DID]),
        (
            "Rachel's page",
            PersonPage::Contributor(RACHEL_DID),
            &[RACHEL_DID],
        ),
        (
            "github:jeffbailey",
            PersonPage::of_account(JEFF_GH),
            &[MY_DID, RACHEL_DID],
        ),
    ];
    let pages: Vec<PersonPage> = rows.iter().map(|(_, p, _)| p.clone()).collect();

    let opened = when_i_open_pages(&world, None, &pages);

    for ((row, _, dids), page) in rows.iter().zip(&opened) {
        let listing = region_listing(page);
        for did in *dids {
            assert_shown_in(row, &listing, did, JEFF_GH, Shown::Contested);
        }
        assert!(
            !listing
                .stdout
                .contains(&format!(" is {}", JEFF_GH.subject())),
            "[{row}] no page says anyone is github:jeffbailey;{}",
            listing.stderr
        );
    }
}

// =============================================================================
// What is never a linked identity (C-GDL-1, U-39, U-19)
// =============================================================================

/// VL-5 (+ the five ADR-084 §3 flaws)
/// ```gherkin
/// @driving_port @real-io @error @us-vl-1 @c-gdl-1 @adr-084-3 @c6a @kpi-gdl-1 @contract-shape:unbounded-preservation
/// Scenario Outline: A stranger's link about Tobias and flawed link records never show as anyone's linked identity
///   Given Jeff follows Rachel, who published her own link R1 to github "rachelk"
///   And Rachel also published "did:plc:tobias-test sameAs github:tobiasw" about Tobias
///   And Rachel published five flawed link records for github "rachelk" (one per flaw)
///   When Jeff opens <page>
///   Then its linked identities are <linked identities>
///   Examples:
///     | page                               | linked identities |
///     | Tobias's contributor page          | absent (no region) |
///     | the GitHub person page tobiasw     | absent (no region) |
///     | Rachel's contributor page          | exactly the line "did:plc:rachel-test claims github:rachelk — not checked (record R1)" |
///     | the GitHub person page rachelk     | exactly the line "did:plc:rachel-test claims github:rachelk — not checked (record R1)" |
/// ```
#[test]
#[ignore = "DELIVER 04-04: third-party and flawed link records never show"]
fn a_strangers_link_about_tobias_and_flawed_link_records_never_show_as_anyones_linked_identity() {
    let mut world = LinkWorld::with(&[RACHEL_GH, TOBIAS_GH]);
    let mut specs = vec![
        link_spec(RACHEL_DID, RACHEL_GH),
        link_spec(TOBIAS_DID, TOBIAS_GH),
    ];
    specs.extend(LinkFlaw::ALL.map(|flaw| flawed_link_spec(RACHEL_DID, RACHEL_GH, flaw)));
    let cids = given_peer_published(&mut world, Peer::Rachel, specs);
    let sound_record = records_named(&cids[..1]);
    let rows = [
        ("Tobias's page", PersonPage::Contributor(TOBIAS_DID), false),
        ("github:tobiasw", PersonPage::of_account(TOBIAS_GH), false),
        ("Rachel's page", PersonPage::Contributor(RACHEL_DID), true),
        ("github:rachelk", PersonPage::of_account(RACHEL_GH), true),
    ];
    let pages: Vec<PersonPage> = rows.iter().map(|(_, p, _)| p.clone()).collect();

    let opened = when_i_open_pages(&world, None, &pages);

    for ((row, _, rachels), page) in rows.iter().zip(&opened) {
        if !rachels {
            assert_no_region(row, page);
            continue;
        }
        let lines = region_lines(&page.body);
        assert_eq!(lines.len(), 1, "[{row}] exactly one line: {lines:#?}");
        let listing = region_listing(page);
        assert_shown_in(row, &listing, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
        assert!(
            lines[0].ends_with(&sound_record),
            "[{row}] the line names only the sound record `{sound_record}`: {:?}",
            lines[0]
        );
    }
}

/// VL-6 (U-39; IR-8c/8d in the viewer)
/// ```gherkin
/// @driving_port @real-io @error @us-vl-1 @u-39 @gdl-d5 @gdl-d9 @c2b @contract-shape:unbounded-preservation
/// Scenario Outline: A retracted or superseded link, and the retraction itself, never show
///   Given Jeff follows Rachel, who published links to github "rachelk" and github "rachel-old"
///   And Rachel replaced her link to github "rachelk" with a new record R2 and retracted her link to github "rachel-old"
///   When Jeff opens <page>
///   Then its linked identities are <linked identities>
///   Examples:
///     | page                              | linked identities |
///     | Rachel's contributor page         | exactly the line "did:plc:rachel-test claims github:rachelk — not checked (record R2)" |
///     | the GitHub person page rachel-old | absent (no region) |
/// ```
#[test]
#[ignore = "DELIVER 04-04: retracted and superseded links never show"]
fn a_retracted_or_superseded_link_and_the_retraction_itself_never_show() {
    let mut world = LinkWorld::with(&[RACHEL_GH, RACHEL_OLD_GH]);
    let kept = link_spec(RACHEL_DID, RACHEL_GH);
    let withdrawn = link_spec(RACHEL_DID, RACHEL_OLD_GH);
    let cids = given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![kept.clone(), withdrawn.clone()],
    );
    let markers = given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![
            peer_supersession_of(&kept, &cids[0]),
            peer_retraction_of(&withdrawn, &cids[1]),
        ],
    );
    let new_record = records_named(&markers[..1]);

    let opened = when_i_open_pages(
        &world,
        None,
        &[
            PersonPage::Contributor(RACHEL_DID),
            PersonPage::of_account(RACHEL_OLD_GH),
        ],
    );

    let lines = region_lines(&opened[0].body);
    assert_eq!(
        lines.len(),
        1,
        "[Rachel's page] exactly one line: {lines:#?}"
    );
    assert_shown_in(
        "Rachel's page",
        &region_listing(&opened[0]),
        RACHEL_DID,
        RACHEL_GH,
        Shown::NotChecked,
    );
    assert!(
        lines[0].ends_with(&new_record),
        "[Rachel's page] the line names only the new record `{new_record}`: {:?}",
        lines[0]
    );
    assert_no_region("github:rachel-old", &opened[1]);
}

/// VL-7 (U-19, U-20; IR-20/IR-20c in the viewer)
/// ```gherkin
/// @driving_port @real-io @error @us-vl-1 @us-vl-2 @u-19 @u-20 @c2a @c4a @contract-shape:bounded-change
/// Scenario Outline: A link from a peer Jeff removed does not show; it shows again once he follows her again
///   Given Jeff followed Rachel, whose link to github "rachelk" he checked
///   And since then Jeff <what Jeff did since>
///   When Jeff opens <page>
///   Then it shows <show>
///   Examples:
///     | what Jeff did since       | page                           | show |
///     | did nothing               | Rachel's contributor page      | "did:plc:rachel-test is github:rachelk — verified today" |
///     | did nothing               | the GitHub person page rachelk | "did:plc:rachel-test is github:rachelk — verified today" |
///     | removed her               | Rachel's contributor page      | no linked-identities region |
///     | removed her               | the GitHub person page rachelk | no linked-identities region |
///     | removed her, then added her | Rachel's contributor page    | "did:plc:rachel-test is github:rachelk — verified today" |
///     | removed her, then added her | the GitHub person page rachelk | "did:plc:rachel-test is github:rachelk — verified today" |
/// ```
#[test]
#[ignore = "DELIVER 04-04: a removed peer's link is hidden until she is followed again"]
fn a_link_from_a_peer_jeff_removed_does_not_show_until_he_follows_her_again() {
    let world = rachels_checked_link();
    let pages = [
        PersonPage::Contributor(RACHEL_DID),
        PersonPage::of_account(RACHEL_GH),
    ];
    type Step = fn(&LinkWorld);
    let rows: [(&str, Step, bool); 3] = [
        ("followed", |_| {}, true),
        (
            "removed",
            |w| given_i_removed_the_peer(w, Peer::Rachel),
            false,
        ),
        (
            "followed again",
            |w| {
                let added = given_i_followed_the_peer_again(w, Peer::Rachel);
                assert_eq!(
                    added.status, 0,
                    "precondition: `peer add` follows Rachel again;\n{}",
                    added.stderr
                );
            },
            true,
        ),
    ];

    for (row, step, shown) in rows {
        step(&world);

        let opened = when_i_open_pages(&world, None, &pages);

        for (page_name, page) in ["Rachel's page", "github:rachelk"].iter().zip(&opened) {
            let label = format!("{row}, {page_name}");
            if shown {
                assert_shown_in(
                    &label,
                    &region_listing(page),
                    RACHEL_DID,
                    RACHEL_GH,
                    Shown::Associated,
                );
            } else {
                assert_no_region(&label, page);
            }
        }
    }
}

// =============================================================================
// A claim's author and network search (US-VL-1, US-VL-3)
// =============================================================================

/// VL-10
/// ```gherkin
/// @driving_port @real-io @us-vl-1 @d-vl-1 @contract-shape:pure-function
/// Scenario: A claim's detail page shows its author's linked identity
///   Given Jeff linked github "jeffbailey" and it is verified
///   And Jeff signed a claim that "owner/repo-linked-identity" is maintained by The Maintainers
///   When Jeff opens that claim's detail page
///   Then beneath the claim he sees "did:plc:test-jeff is github:jeffbailey — verified today"
///   And the line reads exactly as on his contributor page
/// ```
#[test]
#[ignore = "DELIVER 04-05: the claim detail page shows its author's linked identity"]
fn a_claims_detail_page_shows_its_authors_linked_identity() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    let cid = given_my_claim(&world);

    let opened = when_i_open_pages(
        &world,
        None,
        &[
            PersonPage::ClaimDetail(cid.clone()),
            PersonPage::Contributor(MY_DID),
        ],
    );

    let detail = &opened[0];
    assert_region_with_legend("claim detail", detail);
    assert_shown_in(
        "claim detail",
        &region_listing(detail),
        MY_DID,
        JEFF_GH,
        Shown::Associated,
    );
    let claim_at = detail
        .body
        .find(&cid)
        .unwrap_or_else(|| panic!("the claim is shown;\n{}", detail.body));
    let region_at = detail.body.find(LINKED_IDENTITIES_HEADING).unwrap_or(0);
    assert!(
        region_at > claim_at,
        "[claim detail] the region sits beneath the claim"
    );
    assert_eq!(
        region_lines(&detail.body),
        region_lines(&opened[1].body),
        "[claim detail] the same line as on Jeff's contributor page"
    );
}

/// VL-11 + VL-19 (D-VL-10, U-7)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @us-vl-3 @d-vl-10 @u-7 @adr-085-8 @c5a @contract-shape:pure-function
/// Scenario Outline: Network search shows each author's linked identity and labels link rows
///   Given Jeff follows Rachel and checked her link to github "rachelk"
///   And <the network index>
///   When Jeff searches the network for github:rachelk in his viewer
///   Then <what he sees>
///   Examples:
///     | the network index                                                           | what he sees |
///     | holds Rachel's link and Sven's claim that rust-lang/regex is maintained by github "rachelk" | Rachel's group shows "did:plc:rachel-test is github:rachelk — verified today"; her link row is labelled "authority not checked here"; Sven's group has no linked identity and his row no label |
///     | is not configured                                                           | the "network index is unavailable" notice, and no linked identity |
/// ```
#[test]
#[ignore = "DELIVER 04-06: search author groups show the line; link rows are labelled"]
fn network_search_shows_each_authors_linked_identity_and_labels_link_rows() {
    let world = rachels_checked_link();
    let search = PersonPage::Search(RACHEL_GH.subject());
    let rachels_link_row = format!("{RACHEL_DID} {SAME_AS} {}", RACHEL_GH.subject());
    let svens_row = format!(
        "github:rust-lang/regex maintainedBy {}",
        RACHEL_GH.subject()
    );
    let index = given_the_network_index_holds(
        &world.env,
        vec![
            indexed_link(RACHEL_DID, RACHEL_GH),
            indexed_github_row(
                openlore_test_support::SVEN_DID,
                "github:rust-lang/regex",
                RACHEL_GH,
            ),
        ],
    );

    let reachable = {
        let viewer = open_viewer(
            &world,
            ViewerOpts {
                indexer: Some(index),
                ..ViewerOpts::default()
            },
        );
        when_i_open(&viewer, &search)
    };
    let unconfigured = when_i_open_page(&world, None, &search);

    assert_page_loads("index reachable", &reachable);
    let rachels = search_group_html(&reachable.body, RACHEL_DID);
    let svens = search_group_html(&reachable.body, openlore_test_support::SVEN_DID);
    let rachels_listing = CliOutcome {
        status: 0,
        stdout: region_lines(&rachels).join("\n"),
        stderr: rachels.clone(),
    };
    assert_shown_in(
        "Rachel's group",
        &rachels_listing,
        RACHEL_DID,
        RACHEL_GH,
        Shown::Associated,
    );
    assert!(
        html_text(&rachels).contains(LINKED_IDENTITIES_LEGEND),
        "[Rachel's group] the legend sits beneath her line;\n{rachels}"
    );
    let label_on = |group: &str, triple: &str| {
        search_rows_html(group)
            .iter()
            .find(|r| html_text(r).contains(triple))
            .map(|r| html_text(r).contains(SEARCH_LABEL))
    };
    assert_eq!(
        label_on(&rachels, &rachels_link_row),
        Some(true),
        "[Rachel's group] her link row is labelled `{SEARCH_LABEL}`;\n{rachels}"
    );
    assert!(
        regions_html(&svens).is_empty(),
        "[Sven's group] no linked identity;\n{svens}"
    );
    assert_eq!(
        label_on(&svens, &svens_row),
        Some(false),
        "[Sven's group] a github row that is not a link is listed and unlabelled (U-7);\n{svens}"
    );

    assert!(
        html_text(&unconfigured.body).contains(SEARCH_UNAVAILABLE),
        "[index not configured] the unavailable notice;\n{}",
        unconfigured.body
    );
    assert_no_region("index not configured", &unconfigured);
}

// =============================================================================
// Absence and the clock (with-stale-config environment)
// =============================================================================

/// VL-12
/// ```gherkin
/// @driving_port @real-io @edge @us-vl-1 @us-vl-2 @c3 @c1a @c6a @contract-shape:pure-function
/// Scenario Outline: A person with no link shows no linked-identities region
///   Given Jeff linked github "jeffbailey" and follows Rachel, who published a memory-safety claim and no link
///   When Jeff opens <page>
///   Then he sees <region>
///   Examples:
///     | page                              | region                          |
///     | his own contributor page          | his one linked identity         |
///     | Rachel's contributor page         | no linked-identities region     |
///     | the GitHub person page tobiasw    | no linked-identities region     |
///     | a GitHub person page for "-jeff", not a GitHub username | no linked-identities region |
///     | a contributor page for "jeffbailey", not a DID          | no linked-identities region |
///
/// Row group 2 (clean environment, review F-3): in an empty store (no claim, no
/// link, no peer) Jeff's contributor page and the GitHub person page jeffbailey
/// load and show no linked-identities region.
/// ```
#[test]
#[ignore = "DELIVER 04-01: a person with no link shows no region"]
fn a_person_with_no_link_shows_no_linked_identities_region() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![PeerClaimSpec::repo_claim(
            "rust-lang/regex",
            MEMORY_SAFETY,
            0.60,
        )],
    );

    let opened = when_i_open_pages(
        &world,
        None,
        &[
            PersonPage::Contributor(MY_DID),
            PersonPage::Contributor(RACHEL_DID),
            PersonPage::of_account(TOBIAS_GH),
            PersonPage::GithubPerson("-jeff".to_string()),
            PersonPage::Contributor("jeffbailey"),
        ],
    );

    assert_region_with_legend("Jeff's page", &opened[0]);
    assert_eq!(
        region_lines(&opened[0].body).len(),
        1,
        "[Jeff's page] his one line"
    );
    assert_no_region("Rachel's page", &opened[1]);
    assert_no_region("github:tobiasw", &opened[2]);
    assert_no_region("github:-jeff (not a username)", &opened[3]);
    assert_no_region("contributor jeffbailey (not a DID)", &opened[4]);

    let empty = LinkWorld::with(&[]);
    let opened = when_i_open_pages(
        &empty,
        None,
        &[
            PersonPage::Contributor(MY_DID),
            PersonPage::of_account(JEFF_GH),
        ],
    );
    assert_no_region("empty store, Jeff's page", &opened[0]);
    assert_no_region("empty store, github:jeffbailey", &opened[1]);
}

/// VL-15 (Q-GDL-D4; with-stale-config: a check dated in the future)
/// ```gherkin
/// @driving_port @real-io @boundary @us-vl-1 @q-rm-3 @c1b @contract-shape:pure-function
/// Scenario: A check dated in the future is ignored
///   Given Jeff linked github "jeffbailey" on 2026-10-01 at 09:00
///   And his store also holds a failed check of that link dated 2026-11-12
///   When Jeff opens his contributor page on 2026-10-01 at 18:00
///   Then he sees "did:plc:test-jeff is github:jeffbailey — verified today", as identity list prints it then
/// ```
#[test]
#[ignore = "DELIVER 04-02: a future-dated check is ignored"]
fn a_check_dated_in_the_future_is_ignored() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    given_a_check_recorded_seeing(
        &world.env,
        MY_DID,
        JEFF_GH,
        JEFF_GH.login,
        Verdict::DidMissing,
        FORTY_TWO_DAYS_LATER,
    );
    let listed = when_i_list_with(&world, &["--did", MY_DID], Some(SAME_DAY));

    let page = when_i_open_page(&world, Some(SAME_DAY), &PersonPage::Contributor(MY_DID));

    assert_reads_as_listed("future check", &page, &listed);
    let listing = region_listing(&page);
    assert_shown(&listing, MY_DID, JEFF_GH, Shown::Associated);
    assert_check_age(&listing, MY_DID, JEFF_GH, "today", false);
}
