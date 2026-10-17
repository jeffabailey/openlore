//! github-did-identity-link — slice 2 acceptance: how every reader treats a
//! link (ADR-085 §5/§6/§8, GDL-D3/D8/D11/D12/D14, KPI-GDL-1/2/3/4).
//!
//! The rule under test: a link is an ASSOCIATION only when THIS reader's own
//! latest check is Verified and no other DID is Verified for the same account;
//! otherwise it is an ASSERTION ("claims … — not checked / proof gone"), and
//! two Verified DIDs make the account CONTESTED. A squatter can neither hijack
//! nor grief a genuine link. Links never change a weighted result, never merge
//! people, and are never inferred from a scraped bio or a rename flag.
//!
//! Strategy C: REAL `openlore` (`identity list/verify`, `infer people`,
//! `scrape github <user>`, `claim retract`, `graph query --weighted`, `search`)
//! and the REAL `openlore ui` viewer (`/score`). Peers' links arrive app-signed
//! through the unchanged `peer add`/`peer pull` path. FAKE (driven-external
//! only): `FakeGithubAccounts`, `FakeGithub` (repo scrapes), `PeerPds`,
//! `FakePds`, `FakeIdentity`, `FakeIngestServer`.
//!
//! Layer 3/4 — example-only (Mandate 9). PRIMARY Thens through driving ports;
//! the store universe is a secondary guardrail (D-e). All `#[ignore]`d except
//! IR-13, IR-17, IR-19 (F14) and IR-16b (U-7), regression guards that hold today (D-d).

mod support;

#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

// =============================================================================
// Association, assertion, contest (ADR-085 §5)
// =============================================================================

/// IR-1
/// ```gherkin
/// @driving_port @real-io @gdl-d11 @gdl-d14 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario: Rachel's link reads as her claim until Jeff checks it himself
///   Given Jeff follows Rachel, who published a link to github "rachelk"
///   When Jeff lists linked identities
///   Then he sees "did:plc:rachel-test claims github:rachelk — not checked"
///   And GitHub was not asked anything
/// ```
#[test]
fn rachels_link_reads_as_her_claim_until_jeff_checks_it_himself() {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );

    let listed = when_i_list(&world, None);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
    assert!(
        world.github_paths().is_empty(),
        "{:?}",
        world.github_paths()
    );
}

/// IR-2 (KPI-GDL-4 analog; chained on IR-1)
/// ```gherkin
/// @driving_port @real-io @gdl-d11 @gdl-d14 @kpi-gdl-4 @contract-shape:bounded-change
/// Scenario: Once Jeff has checked Rachel's link himself, it shows as her GitHub account
///   Given Jeff sees Rachel's link to github "rachelk" as not checked
///   And Rachel's GitHub bio names her DID
///   When Jeff re-checks Rachel's links
///   Then he is told github:rachelk is verified
///   And his linked identities show "did:plc:rachel-test is github:rachelk (verified today)"
/// ```
#[test]
fn once_jeff_has_checked_rachels_link_himself_it_shows_as_her_github_account() {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    let before = capture_link_universe(&world.env);

    let checked = when_i_verify(&world, &["--did", RACHEL_DID]);

    assert_verify_reported(&checked, RACHEL_GH, Verdict::Verified);
    let listed = when_i_list(&world, None);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::Associated);
    assert_check_age(&listed, RACHEL_DID, RACHEL_GH, "today", false);
    assert_link_store_delta(
        &before,
        &world.env,
        &[(
            "local.identity_link_checks",
            checks_with(
                &[],
                &[CheckRow::of(RACHEL_DID, RACHEL_GH, Verdict::Verified)],
            ),
        )],
    );
}

/// IR-3 (error, squatter)
/// ```gherkin
/// @driving_port @real-io @error @gdl-d14 @h-2 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario: A stranger's unchecked link to Jeff's account never displaces Jeff
///   Given Jeff linked github "jeffbailey" and it is verified
///   And Rachel published a link claiming github "jeffbailey" is her
///   When Jeff lists linked identities
///   Then he is still shown as github:jeffbailey, verified
///   And Rachel's link reads only as her unchecked claim, and the account is not contested
/// ```
#[test]
fn a_strangers_unchecked_link_to_jeffs_account_never_displaces_jeff() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, JEFF_GH)],
    );

    let listed = when_i_list(&world, None);

    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_shown(&listed, RACHEL_DID, JEFF_GH, Shown::NotChecked);
    assert!(!listed.stdout.contains("contested"), "{}", listed.stdout);
}

/// IR-4 (error, squatter after a check; chained on IR-3)
/// ```gherkin
/// @driving_port @real-io @error @gdl-d14 @h-2 @kpi-gdl-1 @contract-shape:bounded-change
/// Scenario: A stranger's disproved link to Jeff's account never displaces Jeff
///   Given Jeff linked github "jeffbailey", whose bio names only Jeff's DID
///   And Rachel published an unchecked link claiming github "jeffbailey" is her
///   When Jeff re-checks every link
///   Then he is told Rachel's claim fails because the bio names someone else's DID
///   And his linked identities show Rachel's link as "proof gone", Jeff still verified, nothing contested
/// ```
#[test]
fn a_strangers_disproved_link_to_jeffs_account_never_displaces_jeff() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, JEFF_GH)],
    );

    let checked = when_i_verify(&world, &[]);

    assert_verify_reported(&checked, JEFF_GH, Verdict::DifferentDid);
    let listed = when_i_list(&world, None);
    assert_shown(
        &listed,
        RACHEL_DID,
        JEFF_GH,
        Shown::ProofGone(Verdict::DifferentDid),
    );
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert!(!listed.stdout.contains("contested"), "{}", listed.stdout);
}

/// IR-5 (edge, contested)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d14 @a-4 @contract-shape:pure-function
/// Scenario: An account whose bio names two people who both linked it reads as contested
///   Given the bio of github "jeffbailey" names both Jeff's DID and Rachel's DID
///   And both Jeff and Rachel linked github "jeffbailey"
///   And Jeff re-checked every link
///   When Jeff lists linked identities
///   Then github:jeffbailey is shown as contested, naming both DIDs, and neither as its owner
/// ```
#[test]
fn an_account_whose_bio_names_two_people_who_both_linked_it_reads_as_contested() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_names_both(&world, JEFF_GH, MY_DID, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, JEFF_GH)],
    );
    given_i_verified(&world, None);

    let listed = when_i_list(&world, None);

    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Contested);
    assert_shown(&listed, RACHEL_DID, JEFF_GH, Shown::Contested);
}

/// IR-6 (error, third party)
/// ```gherkin
/// @driving_port @real-io @error @gdl-d3 @c-gdl-1 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario: Rachel cannot link Tobias's GitHub account to Tobias
///   Given Rachel published "did:plc:tobias-test sameAs github:tobiasw"
///   And the bio of github "tobiasw" names Tobias's DID
///   And Jeff re-checked every link
///   When Jeff lists linked identities
///   Then nothing links did:plc:tobias-test to github:tobiasw
///   And GitHub was never asked about "tobiasw"
/// ```
#[test]
fn rachel_cannot_link_tobiass_github_account_to_tobias() {
    let mut world = LinkWorld::with(&[TOBIAS_GH]);
    bio_names(&world, TOBIAS_GH, TOBIAS_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(TOBIAS_DID, TOBIAS_GH)],
    );
    given_i_verified(&world, None);

    let listed = when_i_list(&world, None);

    assert_shown(&listed, TOBIAS_DID, TOBIAS_GH, Shown::Nothing);
    assert!(
        world.github_paths().is_empty(),
        "{:?}",
        world.github_paths()
    );
}

/// IR-7 (error, table)
/// ```gherkin
/// @driving_port @real-io @error @adr-084-3 @n-4 @kpi-gdl-1 @contract-shape:pure-function
/// Scenario Outline: A link record with a flaw never links anyone
///   Given Rachel published a link to github "rachelk" whose <flaw>
///   And Rachel's GitHub bio names her DID
///   And Jeff re-checked every link
///   When Jeff lists linked identities
///   Then nothing links did:plc:rachel-test to github:rachelk, and GitHub was never asked
///   Examples:
///     | flaw                                              |
///     | profile page names another login                  |
///     | account number is missing                         |
///     | account number appears twice                      |
///     | object is a repository, not a GitHub user         |
///     | predicate is "SameAs" rather than "sameAs"        |
/// ```
#[test]
fn a_link_record_with_a_flaw_never_links_anyone() {
    for flaw in LinkFlaw::ALL {
        let mut world = LinkWorld::with(&[RACHEL_GH]);
        bio_names(&world, RACHEL_GH, RACHEL_DID);
        given_peer_published(
            &mut world,
            Peer::Rachel,
            vec![flawed_link_spec(RACHEL_DID, RACHEL_GH, flaw)],
        );
        given_i_verified(&world, None);

        let listed = when_i_list(&world, None);

        assert_eq!(
            listed.status, 0,
            "[{flaw:?}] {}\n{}",
            listed.stdout, listed.stderr
        );
        assert_shown_in(
            &format!("{flaw:?}"),
            &listed,
            RACHEL_DID,
            RACHEL_GH,
            Shown::Nothing,
        );
        assert!(
            world.github_paths().is_empty(),
            "[{flaw:?}] GitHub must not be asked: {:?}",
            world.github_paths()
        );
    }
}

/// IR-8a (edge, lifecycle)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d5 @adr-060 @c2a @contract-shape:bounded-change
/// Scenario: A link Jeff retracted no longer shows among his identities
///   Given Jeff linked github "jeffbailey"
///   And Jeff retracted the link
///   When Jeff lists linked identities
///   Then nothing links him to github:jeffbailey
/// ```
#[test]
fn a_link_jeff_retracted_no_longer_shows_among_his_identities() {
    let world = LinkWorld::with(&[JEFF_GH]);
    let cid = given_i_linked(&world, JEFF_GH, None);
    given_i_retracted(&world.env, &cid);

    let listed = when_i_list(&world, None);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Nothing);
}

/// IR-8b (edge, lifecycle)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d5 @c2b @contract-shape:unbounded-preservation
/// Scenario: A link Jeff retracted is no longer re-checked
///   Given Jeff linked github "jeffbailey" and retracted the link
///   When Jeff re-checks his links
///   Then he is told there are no identity links to verify, and GitHub was not asked
/// ```
#[test]
fn a_link_jeff_retracted_is_no_longer_rechecked() {
    let world = LinkWorld::with(&[JEFF_GH]);
    let cid = given_i_linked(&world, JEFF_GH, None);
    given_i_retracted(&world.env, &cid);
    let asked_before = world.github_paths().len();
    let before = capture_link_universe(&world.env);

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert!(
        checked.stdout.contains("No identity links to verify"),
        "{}",
        checked.stdout
    );
    assert!(world.github_paths_since(asked_before).is_empty());
    assert_link_store_unchanged(&before, &world.env);
}

/// IR-9 (boundary, table)
/// ```gherkin
/// @driving_port @real-io @boundary @n-3 @adr-085-5 @contract-shape:pure-function
/// Scenario Outline: Each link line says how old its check is, and suggests a re-check after 30 days
///   Given Jeff linked github "jeffbailey" on 2026-10-01 at 09:00
///   When Jeff lists linked identities <when>
///   Then the line reads "verified <age>" <hint>
///   Examples:
///     | when                         | age         | hint                       |
///     | the same day                 | today       | without a re-check hint    |
///     | 12 days later                | 12 days ago | without a re-check hint    |
///     | 30 days later                | 30 days ago | without a re-check hint    |
///     | 31 days later                | 31 days ago | with a re-check hint       |
///     | before the check was made    | -           | reads "not checked"        |
/// ```
#[test]
fn each_link_line_says_how_old_its_check_is_and_suggests_a_recheck_after_thirty_days() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    let rows = [
        (SAME_DAY, "today", false),
        (TWELVE_DAYS_LATER, "12 days ago", false),
        (THIRTY_DAYS_LATER, "30 days ago", false),
        (THIRTY_ONE_DAYS_LATER, "31 days ago", true),
    ];
    for (now, age, hint) in rows {
        let listed = when_i_list(&world, Some(now));
        assert_shown_in(age, &listed, MY_DID, JEFF_GH, Shown::Associated);
        assert_check_age(&listed, MY_DID, JEFF_GH, age, hint);
    }

    let listed = when_i_list(&world, Some(BEFORE_THE_CHECK));
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::NotChecked);
}

/// IR-10 (edge, duplicates)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d9 @c3 @contract-shape:pure-function
/// Scenario: Two link records for the same person and account read as one link naming both
///   Given Rachel published two links to github "rachelk", composed a day apart
///   And Jeff verified them
///   When Jeff lists linked identities
///   Then exactly one line shows "did:plc:rachel-test is github:rachelk", naming both records
/// ```
#[test]
fn two_link_records_for_the_same_person_and_account_read_as_one_link_naming_both() {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    let first = link_spec(RACHEL_DID, RACHEL_GH);
    let second = PeerClaimSpec {
        composed_at: "2026-10-02T08:00:00Z".to_string(),
        ..first.clone()
    };
    let cids = given_peer_published(&mut world, Peer::Rachel, vec![first, second]);
    given_i_verified(&world, None);

    let listed = when_i_list(&world, None);

    let associated = format!("{RACHEL_DID} is {}", RACHEL_GH.subject());
    assert_eq!(
        listed
            .stdout
            .lines()
            .filter(|l| l.contains(&associated))
            .count(),
        1,
        "{}",
        listed.stdout
    );
    for cid in &cids {
        assert!(
            listed.stdout.contains(cid.as_str()),
            "[{cid}] named;\n{}",
            listed.stdout
        );
    }
}

/// IR-18 (edge, F7)
/// ```gherkin
/// @driving_port @real-io @edge @f7 @c5a @contract-shape:pure-function
/// Scenario: Jeff can list one person's linked identities
///   Given Jeff linked github "jeffbailey" and verified Rachel's link to github "rachelk"
///   When Jeff lists linked identities for did:plc:rachel-test only
///   Then he sees Rachel as github:rachelk and nothing about himself
/// ```
#[test]
fn jeff_can_list_one_persons_linked_identities() {
    let mut world = LinkWorld::with(&[JEFF_GH, RACHEL_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    given_i_verified(&world, None);

    let listed = when_i_list_with(&world, &["--did", RACHEL_DID], None);

    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::Associated);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Nothing);
}

// =============================================================================
// ADR-084 Earned Trust: a link survives the peer-pull wire (F14, U-11)
// =============================================================================

/// When Rachel's self-attested link was composed (the production encoder's input).
const SELF_ATTESTED_AT: &str = "2026-10-01T09:00:00Z";

/// Binding seam (Mandate 7): ONE call into the PRODUCTION encoder
/// `claim_domain::identity_link::self_attested_link_record` — the record the
/// review app writes to its owner's repo — never a test-built shape.
fn sut_self_attested_link_record(did: &str, account: Account) -> serde_json::Value {
    let observation = claim_domain::identity_link::GithubObservation {
        login: account.login.to_string(),
        user_id: account.id,
        bio: Some(bio_naming(did)),
    };
    claim_domain::identity_link::self_attested_link_record(did, &observation, SELF_ATTESTED_AT)
        .expect("a bio naming the DID proves the link")
}

/// The record key a self-attested record is published under: its own CID,
/// computed by the production codec (decode → canonicalize → CID).
fn record_key_of(record: &serde_json::Value) -> String {
    claim_domain::decode_claim_record(record, "")
        .expect("the encoder's record decodes as a claim")
        .cid()
        .0
        .clone()
}

/// GIVEN: `peer`'s own repo holds the self-attested link the production
/// encoder wrote for `account`, under its CID as record key, and Jeff
/// subscribed and pulled it (real `peer add` + `peer pull`). Returns the
/// record key and the pull's outcome.
fn given_peer_self_attested_and_jeff_pulled(
    world: &LinkWorld,
    peer: Peer,
    seed: [u8; 32],
    account: Account,
) -> (String, CliOutcome) {
    let record = sut_self_attested_link_record(peer.did(), account);
    let rkey = record_key_of(&record);
    let pds = PeerPds::for_peer(
        peer.did(),
        vec![openlore_test_support::FakePeerRecord::claim(
            rkey.clone(),
            record,
        )],
    );
    let added = run_openlore_with_peer_resolver(
        &world.env,
        &["peer", "add", peer.did()],
        peer.did(),
        pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "precondition: peer add;\n{}\n{}",
        added.stdout, added.stderr
    );
    let (_, _, pubkey) = peer_records(peer.did(), seed, &[]);
    let pulled = run_openlore_pull(
        &world.env,
        &["peer", "pull"],
        peer.did(),
        pds.endpoint_url(),
        &pubkey,
    );
    (rkey, pulled)
}

/// The CIDs `peer_claims` holds for `author` (secondary store observation).
fn stored_peer_claim_cids(env: &TestEnv, author: &str) -> Vec<String> {
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB");
    let mut stmt = conn
        .prepare("SELECT cid FROM peer_claims WHERE author_did = ? ORDER BY cid")
        .expect("peer_claims");
    stmt.query_map([author], |r| r.get(0))
        .expect("query peer_claims")
        .map(|r| r.expect("cid"))
        .collect()
}

/// IR-19 (F14, ADR-084 Earned Trust)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @adr-084-earned-trust @f14 @gdl-d11 @contract-shape:bounded-change
/// Scenario: A self-attested link pulled from a peer keeps its record id and reads as a link
///   Given Rachel's repo holds the self-attested link to github "rachelk" that openlore writes, filed under its own record id
///   When Jeff pulls Rachel's claims
///   Then the pull keeps the link, and it is kept under the very record id Rachel filed it under
///   And Jeff's linked identities show "did:plc:rachel-test claims github:rachelk — not checked"
///   And GitHub was not asked anything
/// ```
#[test]
fn a_self_attested_link_pulled_from_a_peer_keeps_its_record_id_and_reads_as_a_link() {
    let world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);

    let (rkey, pulled) =
        given_peer_self_attested_and_jeff_pulled(&world, Peer::Rachel, RACHEL_SEED, RACHEL_GH);

    assert_eq!(
        pulled.status, 0,
        "the pull succeeds;\n{}\n{}",
        pulled.stdout, pulled.stderr
    );
    assert!(
        !pulled.stdout.contains("integrity check failed")
            && !pulled.stderr.contains("integrity check failed"),
        "the link's content hashes to its record id;\n{}\n{}",
        pulled.stdout,
        pulled.stderr
    );
    assert_eq!(
        stored_peer_claim_cids(&world.env, RACHEL_DID),
        vec![rkey.clone()],
        "the stored claim's CID is the record key;\n{}",
        pulled.stdout
    );
    let listed = when_i_list(&world, None);
    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
    assert!(
        world.github_paths().is_empty(),
        "{:?}",
        world.github_paths()
    );
}

// =============================================================================
// Person views stay offline and private (C-GDL-7, GDL-D12, UC-8)
// =============================================================================

/// IR-11
/// ```gherkin
/// @driving_port @real-io @gdl-d10 @c-gdl-7 @contract-shape:pure-function
/// Scenario: The person view shows a verified link even with the network off
///   Given Jeff's store infers that github "BurntSushi" adheres to memory-safety
///   And Rachel (who signed one of those repo claims) published a link to github "BurntSushi", which Jeff verified
///   When Jeff lists inferred people with the network disabled
///   Then BurntSushi's entry shows "did:plc:rachel-test is github:BurntSushi (verified today)"
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn the_person_view_shows_a_verified_link_even_with_the_network_off() {
    // One subscribed peer only: `peer pull` resolves every subscription.
    let world = LinkWorld::with(&[BURNTSUSHI_GH]);
    let (_mine, mut rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&world.env);
    bio_names(&world, BURNTSUSHI_GH, RACHEL_DID);
    rachel.publish_and_pull(&world.env, vec![link_spec(RACHEL_DID, BURNTSUSHI_GH)]);
    given_i_verified(&world, None);

    let inferred = infer_people_offline(&world.env, &[]);

    assert_eq!(
        inferred.status, 0,
        "{}\n{}",
        inferred.stdout, inferred.stderr
    );
    assert_candidate(&inferred, 1, "github:BurntSushi", MEMORY_SAFETY);
    assert_shown(&inferred, RACHEL_DID, BURNTSUSHI_GH, Shown::Associated);
}

/// IR-12 (KPI-GDL-3 via the person scrape, UC-8)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @kpi-gdl-3 @uc-8 @contract-shape:bounded-change
/// Scenario: Reading a person shows that their published link has lost its proof, at no extra GitHub cost
///   Given Tobias's link to github "BurntSushi" is verified in Jeff's store
///   And BurntSushi's bio no longer names Tobias's DID
///   When Jeff reads the person github "BurntSushi"
///   Then the person view shows Tobias's claim on github:BurntSushi as "proof gone"
///   And GitHub was asked once, for BurntSushi's profile
///   And his linked identities now show the same
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn reading_a_person_shows_their_link_has_lost_its_proof_at_no_extra_github_cost() {
    let mut world = LinkWorld::with(&[BURNTSUSHI_GH]);
    bio_names(&world, BURNTSUSHI_GH, TOBIAS_DID);
    given_peer_published(
        &mut world,
        Peer::Tobias,
        vec![link_spec(TOBIAS_DID, BURNTSUSHI_GH)],
    );
    given_i_verified(&world, None);
    bio_drops_the_did(&world, BURNTSUSHI_GH);
    let checks_before = link_checks(&world.env);
    let asked_before = world.github_paths().len();
    let before = capture_link_universe(&world.env);

    let read = when_i_read_the_person(&world, BURNTSUSHI_GH);

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    let gone = Shown::ProofGone(Verdict::DidMissing);
    assert_shown(&read, TOBIAS_DID, BURNTSUSHI_GH, gone);
    assert_target_record_read_once(&world.github_paths_since(asked_before), BURNTSUSHI_GH.login);
    assert_shown(&when_i_list(&world, None), TOBIAS_DID, BURNTSUSHI_GH, gone);
    assert_link_store_delta(
        &before,
        &world.env,
        &[(
            "local.identity_link_checks",
            checks_with(
                &checks_before,
                &[CheckRow::of(TOBIAS_DID, BURNTSUSHI_GH, Verdict::DidMissing)],
            ),
        )],
    );
}

/// IR-13 (guardrail, privacy — ACTIVE regression guard, D-d)
/// ```gherkin
/// @driving_port @real-io @guardrail @gdl-d12 @a-2 @contract-shape:unbounded-preservation
/// Scenario: Reading a person whose bio names a DID never surfaces or suggests a link
///   Given nobody published a link to github "BurntSushi"
///   And BurntSushi's bio names a DID
///   When Jeff reads the person github "BurntSushi"
///   Then the view does not mention that DID, shows no link for it, and does not suggest "openlore identity link"
///   And nothing is recorded, signed or published
/// ```
#[test]
fn reading_a_person_whose_bio_names_a_did_never_surfaces_or_suggests_a_link() {
    let world = LinkWorld::with(&[BURNTSUSHI_GH]);
    bio_names(&world, BURNTSUSHI_GH, STRANGER_DID);
    let before = capture_link_universe(&world.env);

    let read = when_i_read_the_person(&world, BURNTSUSHI_GH);

    assert_eq!(read.status, 0, "{}\n{}", read.stdout, read.stderr);
    assert_shown(&read, STRANGER_DID, BURNTSUSHI_GH, Shown::Nothing);
    let text = format!("{}\n{}", read.stdout, read.stderr);
    assert!(
        !text.contains(STRANGER_DID),
        "the bio DID must not surface;\n{text}"
    );
    assert!(
        !text.contains("openlore identity link"),
        "no link suggestion;\n{text}"
    );
    assert_link_store_unchanged(&before, &world.env);
}

/// IR-17 (guardrail, F3 — ACTIVE regression guard, D-d)
/// ```gherkin
/// @driving_port @real-io @guardrail @a-2 @gdl-d12 @f3 @contract-shape:unbounded-preservation
/// Scenario: A possible rename among inferred people never becomes a link
///   Given Jeff's store infers that github "BurntSushi" adheres to memory-safety
///   And another login, "burnt-old", was recorded with BurntSushi's account number
///   When Jeff lists inferred people
///   Then BurntSushi's entry flags "burnt-old" as a possible rename
///   And no line links any DID to either login
///   And the GitHub accounts double was never asked, and nothing was recorded
/// ```
#[test]
fn a_possible_rename_among_inferred_people_never_becomes_a_link() {
    let world = LinkWorld::with(&[BURNTSUSHI_GH]);
    given_burntsushi_builds_two_signed_memory_safe_repos(&world.env);
    given_repo_scraped(
        &world.env,
        "acme/old",
        vec![FakeContributor::human("burnt-old", BURNTSUSHI_ID, 10)],
        None,
    );
    let before = capture_link_universe(&world.env);

    let inferred = when_i_infer_people(&world);

    assert_eq!(
        inferred.status, 0,
        "{}\n{}",
        inferred.stdout, inferred.stderr
    );
    let block = candidate_block(&inferred.stdout, 1).unwrap_or_default();
    assert!(
        block.contains("github:BurntSushi") && block.contains("possible rename: github:burnt-old"),
        "precondition: the rename flag names both logins;\n{}",
        inferred.stdout
    );
    assert!(
        link_lines(&inferred.stdout).is_empty(),
        "no `did:… is/claims github:…` line;\n{}",
        inferred.stdout
    );
    assert!(
        world.github_paths().is_empty(),
        "the GitHub accounts double was asked: {:?}",
        world.github_paths()
    );
    assert_link_store_unchanged(&before, &world.env);
}

// =============================================================================
// Links never weigh (KPI-GDL-2, GDL-D8) and are labelled in network search
// =============================================================================

/// Rachel's memory-safety claim, with or without her link beside it.
fn rachels_store(with_link: bool) -> LinkWorld {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    let mut specs = vec![PeerClaimSpec::repo_claim(
        "rust-lang/regex",
        MEMORY_SAFETY,
        0.60,
    )];
    if with_link {
        specs.push(link_spec(RACHEL_DID, RACHEL_GH));
    }
    given_peer_published(&mut world, Peer::Rachel, specs);
    world
}

/// IR-14 (guardrail, KPI-GDL-2, CLI)
/// ```gherkin
/// @driving_port @real-io @guardrail @gdl-d8 @kpi-gdl-2 @adr-022 @contract-shape:unbounded-preservation
/// Scenario: A link never changes weighted results in the terminal
///   Given two stores holding Rachel's memory-safety claim, one also holding her link to github "rachelk"
///   When Jeff asks each store for the weighted view of memory-safety and of github:rachelk
///   Then memory-safety has the same weighted rows in both stores
///   And github:rachelk has no weighted row in either
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_link_never_changes_weighted_results_in_the_terminal() {
    let without = rachels_store(false);
    let with = rachels_store(true);
    let weighted = |w: &LinkWorld, object: &str| {
        let out = run_openlore(
            &w.env,
            &["graph", "query", "--object", object, "--weighted"],
        );
        assert_eq!(out.status, 0, "[{object}] {}\n{}", out.stdout, out.stderr);
        weighted_rows(&out.stdout)
    };

    let rows_without = weighted(&without, MEMORY_SAFETY);
    let rows_with = weighted(&with, MEMORY_SAFETY);
    assert!(
        !rows_without.is_empty(),
        "precondition: memory-safety is weighted"
    );
    assert_eq!(
        rows_with, rows_without,
        "[memory-safety] identical weighted rows"
    );

    let object = RACHEL_GH.subject();
    let baseline = weighted(&without, &object);
    assert!(
        baseline.is_empty(),
        "[github:rachelk] baseline: {baseline:#?}"
    );
    let linked = weighted(&with, &object);
    assert!(
        linked.is_empty(),
        "[github:rachelk] a link is never weighted: {linked:#?}"
    );
}

/// IR-15 (guardrail, KPI-GDL-2, viewer)
/// ```gherkin
/// @driving_port @real-io @guardrail @gdl-d8 @kpi-gdl-2 @contract-shape:unbounded-preservation
/// Scenario: A link never changes a contributor's score page
///   Given two stores holding Rachel's memory-safety claim, one also holding her link to github "rachelk"
///   When Jeff opens Rachel's score page in each store's viewer
///   Then the scored results are identical, and github:rachelk is not scored
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_link_never_changes_a_contributors_score_page() {
    let path = format!("/score?contributor={RACHEL_DID}");
    let render = |world: &LinkWorld| {
        let viewer = ViewerServer::start(&world.env);
        let page = viewer.get(&path);
        assert_eq!(page.status, 200, "{}", page.body);
        let marker = format!("id=\"{SCORE_RESULTS_ID}\"");
        let at = page
            .body
            .find(&marker)
            .unwrap_or_else(|| panic!("score results region;\n{}", page.body));
        page.body[at..].to_string()
    };
    let without = render(&rachels_store(false));
    let with = render(&rachels_store(true));

    assert_eq!(without, with, "the score page must not change with a link");
    assert!(!with.contains(&RACHEL_GH.subject()), "{with}");
}

/// IR-16 (federation label, decision D-c)
/// ```gherkin
/// @driving_port @real-io @gdl-d11 @adr-085-8 @c-gdl-8 @d-c @contract-shape:pure-function
/// Scenario: Network search lists a link as not checked here
///   Given the network index holds Priya's link to github "priyaraman"
///   When Jeff searches the network for github:priyaraman
///   Then the link is listed, attributed to Priya, labelled "authority not checked here"
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn network_search_lists_a_link_as_not_checked_here() {
    let env = TestEnv::initialized();
    let priya = Account {
        login: "priyaraman",
        id: 4_210_001,
    };
    let link = openlore_test_support::RawRecordSpec {
        predicate: SAME_AS.to_string(),
        evidence: vec![priya.profile_url(), priya.id_uri()],
        ..openlore_test_support::RawRecordSpec::valid(
            openlore_test_support::PRIYA_DID,
            openlore_test_support::PRIYA_DID,
            &priya.subject(),
            1.0,
        )
    };
    let indexer = seed_network_index_from_specs(&env, vec![link]);

    let found = run_openlore_search(&env, &["search", "--object", &priya.subject()], &indexer);

    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    assert!(
        found.stdout.contains(openlore_test_support::PRIYA_DID)
            && found.stdout.contains(&priya.subject()),
        "the link is listed and attributed;\n{}",
        found.stdout
    );
    assert!(found.stdout.contains(SEARCH_LABEL), "{}", found.stdout);
}

/// The lines of `author`'s group in network search output: its `author:`
/// header and every row under it, up to the next author's header.
fn search_group_of<'a>(stdout: &'a str, author: &str) -> Vec<&'a str> {
    let header = format!("author: {author}");
    stdout
        .lines()
        .skip_while(|l| !l.starts_with(&header))
        .enumerate()
        .take_while(|(i, l)| *i == 0 || !l.starts_with("author:"))
        .map(|(_, l)| l)
        .collect()
}

/// IR-16b (guardrail, U-7 — ACTIVE regression guard, D-d; chained on IR-16)
/// ```gherkin
/// @driving_port @real-io @guardrail @u-7 @d-c @adr-085-8 @contract-shape:unbounded-preservation
/// Scenario: Network search leaves a GitHub row that is not a link unlabelled
///   Given the network index holds Priya's link to github "priyaraman"
///   And Sven's claim that github "rust-lang/regex" is maintained by github "priyaraman"
///   When Jeff searches the network for github:priyaraman
///   Then both rows are listed
///   And Sven's row is not labelled "authority not checked here"
/// ```
#[test]
fn network_search_leaves_a_github_row_that_is_not_a_link_unlabelled() {
    let env = TestEnv::initialized();
    let priya = Account {
        login: "priyaraman",
        id: 4_210_001,
    };
    let link = openlore_test_support::RawRecordSpec {
        predicate: SAME_AS.to_string(),
        evidence: vec![priya.profile_url(), priya.id_uri()],
        ..openlore_test_support::RawRecordSpec::valid(
            openlore_test_support::PRIYA_DID,
            openlore_test_support::PRIYA_DID,
            &priya.subject(),
            1.0,
        )
    };
    let not_a_link = openlore_test_support::RawRecordSpec {
        predicate: "maintainedBy".to_string(),
        ..openlore_test_support::RawRecordSpec::valid(
            openlore_test_support::SVEN_DID,
            "github:rust-lang/regex",
            &priya.subject(),
            0.8,
        )
    };
    let indexer = seed_network_index_from_specs(&env, vec![link, not_a_link]);

    let found = run_openlore_search(&env, &["search", "--object", &priya.subject()], &indexer);

    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    assert!(
        found.stdout.contains(openlore_test_support::PRIYA_DID),
        "precondition: Priya's link is listed;\n{}",
        found.stdout
    );
    let svens = search_group_of(&found.stdout, openlore_test_support::SVEN_DID);
    assert!(
        svens.iter().any(|l| l.contains("github:rust-lang/regex")),
        "Sven's row is listed;\n{}",
        found.stdout
    );
    assert!(
        svens.iter().all(|l| !l.contains(SEARCH_LABEL)),
        "a github row that is not a link carries no `{SEARCH_LABEL}` label (U-7);\n{}",
        found.stdout
    );
}
