//! github-did-identity-link — shared acceptance support (DISTILL 2026-10-14,
//! Quinn; revised after review iteration 1).
//!
//! Step vocabulary for `identity_link.rs` (slice 1), `identity_link_readers.rs`
//! (slice 2) and `review_app_identity_link_readers.rs` (slice 3 readers).
//! Every WHEN drives the REAL `openlore` binary (the production composition
//! root, Pillar 3); every GIVEN seeds state through REAL verbs. The only doubles
//! are driven-EXTERNAL and in the project policy: `FakeGithubAccounts` (the
//! ADR-085 lie catalogue), `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! **Observation order (review decision D-e).** A scenario's PRIMARY Then
//! observes only through driving ports: `identity list` / `identity verify`
//! output and exit code, the person views, and the wire JSON the user's PDS
//! received (`FakePds`). "A check was recorded" is asserted through the
//! `identity list` line (verdict + age). The fail-closed store universe
//! (`assert_link_store_*`, which adds the DESIGN-pinned `identity_link_checks`
//! rows to the house `store_universe_slots` of `people.rs`) is a SECONDARY
//! guardrail only — see the Mandate-8 waiver in `distill/wave-decisions.md`.
//!
//! Output-wording pins (Q-GDL-D1, DISTILL-owned) live here, as constants and
//! `assert_*` helpers; scenario text speaks plain language (F10).
//!
//! Mandate-12: domain nouns (account, verdict, what a reader shows, link flaw,
//! GitHub trouble, account fate) are typed values; scenario bodies never spell a
//! login, id, verdict token or rendered phrase inline.

use super::people::*;
use super::state_delta::{assert_state_delta, set_to, Delta};
use super::*;
pub use openlore_test_support::ProfileFlaw;
use openlore_test_support::{FakeGithubAccounts, GithubAccount, OldLogin};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

// =============================================================================
// Domain vocabulary (ADR-084 wire contract, ADR-085 reader semantics)
// =============================================================================

/// The reserved identity-link predicate (ADR-084 §2).
pub const SAME_AS: &str = "sameAs";

/// My bare DID (`FakeIdentity::jeff()`, the default `TestEnv` identity).
pub const MY_DID: &str = "did:plc:test-jeff";

/// A DID whose bio token is a strict extension of mine (prefix collision).
pub const MY_DID_PREFIX_COLLIDER: &str = "did:plc:test-jeffrey";

/// A DID nobody in the scenarios owns.
pub const STRANGER_DID: &str = "did:plc:stranger-test";

/// A token the GitHub double rejects (401); must never be echoed.
pub const REJECTED_GITHUB_TOKEN: &str = "ghp_identitylinkfixture0000000000000000000";

/// A distinctive bio phrase that must never be repeated by any verb (F2).
pub const PRIVATE_BIO_PHRASE: &str = "Secret hobby: orchids";

/// Search's one label for link rows (decision D-c, owner-only-signing UC-7).
/// Slice 2 labels link rows only (U-7); a `github:` row that is not a link
/// carries no label, and IR-16b pins that. Widening the label to every
/// `github:` row, as D-c and ADR-085 §8 read, is a follow-up that would
/// change IR-16b.
pub const SEARCH_LABEL: &str = "authority not checked here";

/// Pinned clock instants (`OPENLORE_TEST_NOW`).
pub const LINKED_AT: &str = "2026-10-01T09:00:00Z";
pub const SAME_DAY: &str = "2026-10-01T18:00:00Z";
pub const TWELVE_DAYS_LATER: &str = "2026-10-13T09:00:00Z";
pub const THIRTY_DAYS_LATER: &str = "2026-10-31T09:00:00Z";
pub const THIRTY_ONE_DAYS_LATER: &str = "2026-11-01T09:00:00Z";
pub const FORTY_TWO_DAYS_LATER: &str = "2026-11-12T09:00:00Z";
pub const BEFORE_THE_CHECK: &str = "2026-09-20T09:00:00Z";

/// A GitHub account: the login GitHub serves and its stable numeric id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Account {
    pub login: &'static str,
    pub id: u64,
}

impl Account {
    /// The person subject (`github:<login>`, ADR-064).
    pub fn subject(self) -> String {
        format!("github:{}", self.login)
    }

    /// The profile URL (first evidence entry, ADR-084 §2).
    pub fn profile_url(self) -> String {
        format!("https://github.com/{}", self.login)
    }

    /// The stable id URI (second evidence entry, ADR-084 §2).
    pub fn id_uri(self) -> String {
        format!("https://api.github.com/user/{}", self.id)
    }

    /// The login read path.
    pub fn login_path(self) -> String {
        format!("/users/{}", self.login)
    }

    /// The id read path.
    pub fn id_path(self) -> String {
        format!("/user/{}", self.id)
    }

    /// The account as the fake serves it, with no bio yet.
    pub fn on_github(self) -> GithubAccount {
        GithubAccount::new(self.login, self.id)
    }
}

/// My GitHub account.
pub const JEFF_GH: Account = Account {
    login: "jeffbailey",
    id: 1_234_567,
};
/// My second (work) GitHub account (A-4: one DID, several accounts).
pub const JEFF_WORK_GH: Account = Account {
    login: "jeff-at-work",
    id: 7_654_321,
};
/// My account's login after I rename it.
pub const JEFF_RENAMED: Account = Account {
    login: "jeff-b",
    id: 1_234_567,
};
/// My account's login after a second rename.
pub const JEFF_RENAMED_TWICE: Account = Account {
    login: "jeff-c",
    id: 1_234_567,
};
/// A stranger who re-registers my freed login (a DIFFERENT id).
pub const JEFF_LOGIN_RE_REGISTERED: Account = Account {
    login: "jeffbailey",
    id: 9_999_999,
};
/// Rachel's GitHub account.
pub const RACHEL_GH: Account = Account {
    login: "rachelk",
    id: 2_345_678,
};
/// Tobias's GitHub account.
pub const TOBIAS_GH: Account = Account {
    login: "tobiasw",
    id: 3_456_789,
};
/// BurntSushi (the person the contributor-inference fixtures know).
pub const BURNTSUSHI_GH: Account = Account {
    login: "BurntSushi",
    id: BURNTSUSHI_ID,
};

/// The five recorded proof verdicts (ADR-085 §4). `token()` is the
/// machine-readable label the CLI prints (ADR-076 spelling); `plainly()` is
/// how the scenarios say it (F10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Verified,
    DidMissing,
    DifferentDid,
    NoBio,
    AccountNotFound,
}

impl Verdict {
    pub fn token(self) -> &'static str {
        match self {
            Verdict::Verified => "verified",
            Verdict::DidMissing => "did_missing",
            Verdict::DifferentDid => "different_did",
            Verdict::NoBio => "no_bio",
            Verdict::AccountNotFound => "account_not_found",
        }
    }

    pub fn plainly(self) -> &'static str {
        match self {
            Verdict::Verified => "the bio names the DID",
            Verdict::DidMissing => "the bio names no DID",
            Verdict::DifferentDid => "the bio names someone else's DID",
            Verdict::NoBio => "the account has no bio",
            Verdict::AccountNotFound => "the account no longer exists",
        }
    }
}

/// How a reader shows one (DID, GitHub account) pair (ADR-085 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// This reader's latest check is Verified and no other DID is Verified.
    Associated,
    /// An assertion, never checked by this reader.
    NotChecked,
    /// An assertion whose latest check failed with this verdict.
    ProofGone(Verdict),
    /// Two or more DIDs are Verified for the account; none associated.
    Contested,
    /// Not a link at all (third-party, malformed, retracted, or absent).
    Nothing,
}

/// A subscribed peer who publishes links (app-signed, through `PeerPds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Peer {
    Rachel,
    Tobias,
}

impl Peer {
    pub fn did(self) -> &'static str {
        match self {
            Peer::Rachel => RACHEL_DID,
            Peer::Tobias => TOBIAS_DID,
        }
    }

    fn seed(self) -> [u8; 32] {
        match self {
            Peer::Rachel => RACHEL_SEED,
            Peer::Tobias => TOBIAS_SEED,
        }
    }
}

/// The ways a well-formed-looking `sameAs` record is NOT a link (ADR-084 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFlaw {
    EvidenceLoginDiffers,
    IdUriMissing,
    TwoIdUris,
    ObjectNotAGithubLogin,
    PredicateNearMiss,
}

impl LinkFlaw {
    pub const ALL: [LinkFlaw; 5] = [
        LinkFlaw::EvidenceLoginDiffers,
        LinkFlaw::IdUriMissing,
        LinkFlaw::TwoIdUris,
        LinkFlaw::ObjectNotAGithubLogin,
        LinkFlaw::PredicateNearMiss,
    ];
}

/// What GitHub does to an already-linked account between two checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountFate {
    RenamedAndLoginReRegistered,
    DeletedAndLoginReRegistered,
    Deleted,
}

/// How GitHub fails every read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubTrouble {
    /// Anonymous budget exhausted (403 + rate-limit headers).
    RateLimitedAnonymously,
    /// Connections dropped.
    Unreachable,
    /// The configured `GITHUB_TOKEN` is rejected (401).
    TokenRejected,
}

impl GithubTrouble {
    pub const ALL: [GithubTrouble; 3] = [
        GithubTrouble::RateLimitedAnonymously,
        GithubTrouble::Unreachable,
        GithubTrouble::TokenRejected,
    ];
}

// =============================================================================
// The world: my store + the GitHub double
// =============================================================================

/// My hermetic world: my real store (`TestEnv`) and the GitHub accounts double.
pub struct LinkWorld {
    pub env: TestEnv,
    pub github: FakeGithubAccounts,
    peers: BTreeMap<&'static str, SubscribedPeer>,
    github_token: Cell<Option<&'static str>>,
}

impl LinkWorld {
    /// My store, initialised, beside GitHub serving `accounts` (no bios).
    pub fn with(accounts: &[Account]) -> Self {
        Self {
            env: TestEnv::initialized(),
            github: FakeGithubAccounts::start(accounts.iter().map(|a| a.on_github()).collect()),
            peers: BTreeMap::new(),
            github_token: Cell::new(None),
        }
    }

    /// The paths GitHub was asked for, in order.
    pub fn github_paths(&self) -> Vec<String> {
        self.github.request_paths()
    }

    /// The GitHub paths asked for after the first `since` requests.
    pub fn github_paths_since(&self, since: usize) -> Vec<String> {
        self.github_paths().into_iter().skip(since).collect()
    }

    fn invocation<'a>(&'a self, stdin: &'a str, now: Option<&'a str>) -> Invocation<'a> {
        Invocation {
            github_base: Some(self.github.base_url()),
            github_token: self.github_token.get(),
            stdin,
            now,
            ..Invocation::default()
        }
    }
}

/// A bio that names `did` among ordinary words.
pub fn bio_naming(did: &str) -> String {
    format!("Builder of small tools. {did}")
}

/// GIVEN/WHEN (outside OpenLore): the account holder sets their bio.
pub fn bio_is(world: &LinkWorld, account: Account, bio: Option<&str>) {
    world.github.set_bio(account.login, bio);
}

/// GIVEN: the account's bio names `did`.
pub fn bio_names(world: &LinkWorld, account: Account, did: &str) {
    bio_is(world, account, Some(&bio_naming(did)));
}

/// GIVEN: the account's bio names `did` beside a private phrase (F2).
pub fn bio_names_with_private_phrase(world: &LinkWorld, account: Account, did: &str) {
    bio_is(
        world,
        account,
        Some(&format!("{PRIVATE_BIO_PHRASE}. {did}")),
    );
}

/// GIVEN: the account's bio names both DIDs (a shared or contested account).
pub fn bio_names_both(world: &LinkWorld, account: Account, first: &str, second: &str) {
    bio_is(
        world,
        account,
        Some(&format!("Shared account. {first} {second}")),
    );
}

/// WHEN (outside OpenLore): the account holder removes the DID from the bio.
pub fn bio_drops_the_did(world: &LinkWorld, account: Account) {
    bio_is(world, account, Some("Builder of small tools."));
}

/// GIVEN (outside OpenLore): GitHub fails every read with `trouble`.
pub fn github_has(world: &LinkWorld, trouble: GithubTrouble) {
    match trouble {
        GithubTrouble::RateLimitedAnonymously => world.github.rate_limit_for(600),
        GithubTrouble::Unreachable => world.github.set_offline(true),
        GithubTrouble::TokenRejected => {
            world.github.reject_token(true);
            world.github_token.set(Some(REJECTED_GITHUB_TOKEN));
        }
    }
}

/// GIVEN (outside OpenLore): my linked account meets `fate`.
pub fn my_account_meets(world: &LinkWorld, fate: AccountFate) {
    let squatter = JEFF_LOGIN_RE_REGISTERED
        .on_github()
        .bio("Not Jeff. did:plc:squatter-test");
    match fate {
        AccountFate::RenamedAndLoginReRegistered => {
            world
                .github
                .rename(JEFF_GH.login, JEFF_RENAMED.login, OldLogin::NotFound);
            world.github.register(squatter);
        }
        AccountFate::DeletedAndLoginReRegistered => {
            world.github.delete_account(JEFF_GH.login);
            world.github.register(squatter);
        }
        AccountFate::Deleted => world.github.delete_account(JEFF_GH.login),
    }
}

/// GIVEN (outside OpenLore): every GitHub answer about `account` — by login
/// and by account number — comes back with `flaw` (review D1).
pub fn github_answers_for_with(world: &LinkWorld, account: Account, flaw: ProfileFlaw) {
    world.github.serve_flawed_profile(account.login, flaw);
}

/// GIVEN (outside OpenLore): I renamed my account to `to`; the old login
/// answers as `old_login`.
pub fn i_renamed_my_account(world: &LinkWorld, from: Account, to: Account, old_login: OldLogin) {
    world.github.rename(from.login, to.login, old_login);
}

// =============================================================================
// Driving the production binary
// =============================================================================

/// `openlore identity <args…>` with GitHub wired to the double.
pub fn identity(world: &LinkWorld, args: &[&str], stdin: &str, now: Option<&str>) -> CliOutcome {
    let mut full = vec!["identity"];
    full.extend_from_slice(args);
    openlore_with(&world.env, &full, world.invocation(stdin, now))
}

/// WHEN: I link `account` and confirm (`identity link github <login> --yes`).
pub fn when_i_link(world: &LinkWorld, account: Account) -> CliOutcome {
    when_i_link_at(world, account, None)
}

/// WHEN: I link `account` and confirm, at a pinned instant.
pub fn when_i_link_at(world: &LinkWorld, account: Account, now: Option<&str>) -> CliOutcome {
    identity(world, &["link", "github", account.login, "--yes"], "", now)
}

/// WHEN: I start linking `account` but decline at the confirmation.
pub fn when_i_preview_a_link(world: &LinkWorld, account: Account) -> CliOutcome {
    identity(world, &["link", "github", account.login], "n\n", None)
}

/// GIVEN: I linked `account` (bio names my DID) at `now`. Returns the CID.
pub fn given_i_linked(world: &LinkWorld, account: Account, now: Option<&str>) -> String {
    bio_names(world, account, MY_DID);
    let linked = when_i_link_at(world, account, now);
    assert_eq!(
        linked.status, 0,
        "precondition: `identity link github {} --yes` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        account.login, linked.stdout, linked.stderr
    );
    published_cid_from_stdout(&linked.stdout)
}

/// WHEN: I re-check links (`identity verify [filters…]`).
pub fn when_i_verify(world: &LinkWorld, filters: &[&str]) -> CliOutcome {
    when_i_verify_at(world, filters, None)
}

/// WHEN: I re-check links at a pinned instant.
pub fn when_i_verify_at(world: &LinkWorld, filters: &[&str], now: Option<&str>) -> CliOutcome {
    let mut args = vec!["verify"];
    args.extend_from_slice(filters);
    identity(world, &args, "", now)
}

/// GIVEN: I re-checked every link in my store (and it ran).
pub fn given_i_verified(world: &LinkWorld, now: Option<&str>) {
    let verified = when_i_verify_at(world, &[], now);
    assert_eq!(
        verified.status, 0,
        "precondition: `identity verify` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        verified.stdout, verified.stderr
    );
}

/// WHEN/THEN-observation: I list linked identities (offline verb: no PDS wired;
/// GitHub wired only so any request it made would be observed).
pub fn when_i_list(world: &LinkWorld, now: Option<&str>) -> CliOutcome {
    when_i_list_with(world, &[], now)
}

/// `identity list [--did <did>]` (F7).
pub fn when_i_list_with(world: &LinkWorld, filters: &[&str], now: Option<&str>) -> CliOutcome {
    let mut full = vec!["identity", "list"];
    full.extend_from_slice(filters);
    openlore_with(
        &world.env,
        &full,
        Invocation {
            offline: true,
            ..world.invocation("", now)
        },
    )
}

/// WHEN: I erase one DID's local proof history.
pub fn when_i_forget(world: &LinkWorld, did: &str) -> CliOutcome {
    identity(world, &["forget", "--did", did], "", None)
}

/// WHEN: I read a person (`scrape github <user>`) with GitHub wired.
pub fn when_i_read_the_person(world: &LinkWorld, account: Account) -> CliOutcome {
    openlore_with(
        &world.env,
        &["scrape", "github", account.login],
        world.invocation("", None),
    )
}

/// WHEN: I list inferred people with GitHub wired (to observe it is not asked).
pub fn when_i_infer_people(world: &LinkWorld) -> CliOutcome {
    openlore_with(
        &world.env,
        &["infer", "people"],
        Invocation {
            offline: true,
            ..world.invocation("", None)
        },
    )
}

/// GIVEN: `peer` published these claims and I subscribed and pulled them.
/// One peer per store (`peer pull` resolves every subscription).
pub fn given_peer_published(
    world: &mut LinkWorld,
    peer: Peer,
    specs: Vec<PeerClaimSpec>,
) -> Vec<String> {
    match world.peers.get_mut(peer.did()) {
        Some(subscribed) => subscribed.publish_and_pull(&world.env, specs),
        None => {
            let subscribed =
                SubscribedPeer::subscribe_and_pull(&world.env, peer.did(), peer.seed(), specs);
            let cids = subscribed.cids.clone();
            world.peers.insert(peer.did(), subscribed);
            cids
        }
    }
}

/// GIVEN: I removed `peer` (soft `peer remove`: `removed_at` set, the pulled
/// claims stay cached). U-19: a removed peer's links are candidates for no reader.
pub fn given_i_removed_the_peer(world: &LinkWorld, peer: Peer) {
    world
        .peers
        .get(peer.did())
        .unwrap_or_else(|| panic!("precondition: {} was followed first", peer.did()))
        .unsubscribe(&world.env);
}

/// GIVEN: I followed `peer` again after removing it (real `peer add`; U-20).
/// Returns what `peer add` said.
pub fn given_i_followed_the_peer_again(world: &LinkWorld, peer: Peer) -> CliOutcome {
    world
        .peers
        .get(peer.did())
        .unwrap_or_else(|| panic!("precondition: {} was followed first", peer.did()))
        .follow_again(&world.env)
}

/// A well-formed identity link: `subject sameAs github:<login>` (ADR-084 §2).
pub fn link_spec(subject_did: &str, account: Account) -> PeerClaimSpec {
    PeerClaimSpec {
        subject: subject_did.to_string(),
        predicate: SAME_AS.to_string(),
        object: account.subject(),
        confidence: 1.0,
        evidence: vec![account.profile_url(), account.id_uri()],
        references: Vec::new(),
        composed_at: "2026-10-01T08:00:00Z".to_string(),
    }
}

/// When a peer withdraws or replaces a link (after [`link_spec`]'s instant).
pub const LINK_WITHDRAWN_AT: &str = "2026-10-02T08:00:00Z";

/// The peer's own retraction of the link `link` (CID `link_cid`), in the
/// shape the production `claim retract` verb signs (`claim_retract::run`):
/// the original's subject, predicate and object, NO evidence, confidence 1.0,
/// one `retracts` reference to the original, a fresh instant. Its empty
/// evidence means it is never itself a link (ADR-084 §3).
pub fn peer_retraction_of(link: &PeerClaimSpec, link_cid: &str) -> PeerClaimSpec {
    PeerClaimSpec {
        evidence: Vec::new(),
        confidence: 1.0,
        references: vec![(claim_domain::ReferenceType::Retracts, link_cid.to_string())],
        composed_at: LINK_WITHDRAWN_AT.to_string(),
        ..link.clone()
    }
}

/// The peer's replacement of the link `link` (CID `link_cid`): the same
/// well-formed link (the `LinkDraft` shape — profile URL, id URI, 1.0)
/// re-stated later with one `supersedes` reference to the original, the way
/// the production signer marks a successor (`infer_people::references_for`,
/// DDD-7: only the author's own `supersedes` replaces a claim).
pub fn peer_supersession_of(link: &PeerClaimSpec, link_cid: &str) -> PeerClaimSpec {
    PeerClaimSpec {
        references: vec![(
            claim_domain::ReferenceType::Supersedes,
            link_cid.to_string(),
        )],
        composed_at: LINK_WITHDRAWN_AT.to_string(),
        ..link.clone()
    }
}

/// A link record with exactly one ADR-084 §3 flaw.
pub fn flawed_link_spec(subject_did: &str, account: Account, flaw: LinkFlaw) -> PeerClaimSpec {
    let sound = link_spec(subject_did, account);
    match flaw {
        LinkFlaw::EvidenceLoginDiffers => PeerClaimSpec {
            evidence: vec![
                "https://github.com/someone-else".to_string(),
                account.id_uri(),
            ],
            ..sound
        },
        LinkFlaw::IdUriMissing => PeerClaimSpec {
            evidence: vec![account.profile_url()],
            ..sound
        },
        LinkFlaw::TwoIdUris => PeerClaimSpec {
            evidence: vec![
                account.profile_url(),
                account.id_uri(),
                "https://api.github.com/user/42".to_string(),
            ],
            ..sound
        },
        LinkFlaw::ObjectNotAGithubLogin => PeerClaimSpec {
            object: format!("github:{}/some-repo", account.login),
            ..sound
        },
        LinkFlaw::PredicateNearMiss => PeerClaimSpec {
            predicate: "SameAs".to_string(),
            ..sound
        },
    }
}

/// WHEN: I try `claim add --predicate sameAs` for my own account.
pub fn when_i_add_a_same_as_claim(world: &LinkWorld) -> CliOutcome {
    openlore_with(
        &world.env,
        &[
            "claim",
            "add",
            "--subject",
            MY_DID,
            "--predicate",
            SAME_AS,
            "--object",
            &JEFF_GH.subject(),
            "--evidence",
            &JEFF_GH.profile_url(),
            "--evidence",
            &JEFF_GH.id_uri(),
            "--confidence",
            "1.0",
        ],
        Invocation {
            stdin: "\nY\n",
            ..Invocation::default()
        },
    )
}

/// WHEN: I counter `cid` with a reason, sign and publish (decision D-b).
pub fn when_i_counter_and_publish(world: &LinkWorld, cid: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["claim", "counter", cid, "--reason", "not the same person"],
        Invocation {
            stdin: "\nY\n",
            ..Invocation::default()
        },
    )
}

/// WHEN: I sign inferred candidate 1 with the predicate edited to `sameAs`
/// (the shared sign batch's predicate prompt) and decline publishing.
pub fn when_i_sign_an_inference_as_same_as(world: &LinkWorld) -> CliOutcome {
    infer_people_with(&world.env, &["--sign", "1"], "\nsameAs\n\n\n\n\nn\n", None)
}

// =============================================================================
// Observations at driving ports (PRIMARY)
// =============================================================================

fn all_output(outcome: &CliOutcome) -> String {
    format!("{}\n{}", outcome.stdout, outcome.stderr)
}

fn context(outcome: &CliOutcome) -> String {
    format!(
        "\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    )
}

/// `line` says the pair is verified (and never "unverified", M-5).
fn says_verified(line: &str) -> bool {
    line.contains("verified") && !line.contains("unverified")
}

/// The line a reader prints for `did` and `account`, if any.
pub fn link_line(stdout: &str, did: &str, account: Account) -> Option<String> {
    let object = account.subject();
    stdout
        .lines()
        .find(|l| l.contains(did) && l.contains(&object))
        .map(str::to_string)
}

/// How an `identity list` line names the records stating its link (U-13,
/// GDL-D9): ` (record <id>)` for one, ` (records <id>, <id>)` for several, in
/// ascending record-ID order, at the end of the line after the age and any
/// re-check hint. No records, no suffix.
pub fn records_named(record_ids: &[String]) -> String {
    let mut ids: Vec<&str> = record_ids.iter().map(String::as_str).collect();
    ids.sort_unstable();
    match ids.as_slice() {
        [] => String::new(),
        [only] => format!(" (record {only})"),
        many => format!(" (records {})", many.join(", ")),
    }
}

/// THEN: `identity list` shows exactly one line for the (did, account) pair,
/// and that line ends naming every record in `record_ids` in ascending order
/// ([`records_named`]).
pub fn assert_one_line_naming_records(
    outcome: &CliOutcome,
    did: &str,
    account: Account,
    record_ids: &[String],
) {
    let associated = format!("{did} is {}", account.subject());
    let lines: Vec<&str> = outcome
        .stdout
        .lines()
        .filter(|l| l.contains(&associated))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "exactly one line shows `{associated}`;{}",
        context(outcome)
    );
    let named = records_named(record_ids);
    assert!(
        lines[0].ends_with(&named),
        "the line ends naming its records as `{named}`;{}",
        context(outcome)
    );
}

/// THEN: the reader shows the (did, account) pair as `shown`.
///
/// Pinned phrases: association `<did> is github:<login>` + `verified`;
/// assertion `<did> claims github:<login>` + `not checked` | `proof gone
/// (<verdict token>)`; contested `github:<login> is contested` naming the DID.
pub fn assert_shown(outcome: &CliOutcome, did: &str, account: Account, shown: Shown) {
    assert_shown_in("", outcome, did, account, shown)
}

/// [`assert_shown`] inside a table loop: every message starts with `[row]`.
pub fn assert_shown_in(row: &str, outcome: &CliOutcome, did: &str, account: Account, shown: Shown) {
    let out = &outcome.stdout;
    let object = account.subject();
    let associated = format!("{did} is {object}");
    let asserted = format!("{did} claims {object}");
    let ctx = format!("\n[row: {row}]{}", context(outcome));
    match shown {
        Shown::Associated => assert!(
            out.lines()
                .any(|l| l.contains(&associated) && says_verified(l)),
            "expected `{associated} (verified …)`;{ctx}"
        ),
        Shown::NotChecked => {
            assert!(
                out.lines()
                    .any(|l| l.contains(&asserted) && l.contains("not checked")),
                "expected `{asserted} — not checked`;{ctx}"
            );
            assert!(!out.contains(&associated), "never an association;{ctx}");
        }
        Shown::ProofGone(verdict) => {
            assert!(
                out.lines().any(|l| l.contains(&asserted)
                    && l.contains("proof gone")
                    && l.contains(verdict.token())),
                "expected `{asserted} — proof gone ({})` ({});{ctx}",
                verdict.token(),
                verdict.plainly()
            );
            assert!(!out.contains(&associated), "never an association;{ctx}");
        }
        Shown::Contested => {
            assert!(
                out.lines()
                    .any(|l| l.contains(&format!("{object} is contested")) && l.contains(did)),
                "expected `{object} is contested` naming {did};{ctx}"
            );
            assert!(
                !out.contains(&associated),
                "a contested id associates nobody;{ctx}"
            );
        }
        Shown::Nothing => assert!(
            !out.contains(&associated) && !out.contains(&asserted),
            "{did} and {object} must not be shown as a link at all;{ctx}"
        ),
    }
}

/// THEN: the (did, account) line states the check age (`verified today`,
/// `verified 12 days ago`) and carries the re-check hint iff `hint`.
pub fn assert_check_age(outcome: &CliOutcome, did: &str, account: Account, age: &str, hint: bool) {
    let line = link_line(&outcome.stdout, did, account).unwrap_or_else(|| {
        panic!(
            "expected a line for {did} and {};{}",
            account.subject(),
            context(outcome)
        )
    });
    assert!(
        line.contains(&format!("verified {age}")) && says_verified(&line),
        "the line must say `verified {age}`: {line:?}"
    );
    assert_eq!(
        line.contains("re-check with `openlore identity verify`")
            || line.contains("re-check with openlore identity verify"),
        hint,
        "re-check hint expected = {hint}: {line:?}"
    );
}

/// THEN: the link was refused because of `verdict`, and the message says
/// `not linked` (one exit status: 1).
pub fn assert_link_refused(outcome: &CliOutcome, verdict: Verdict, row: &str) {
    let text = all_output(outcome);
    assert_eq!(
        outcome.status,
        1,
        "[{row}] a link without a passing bio check exits 1;{}",
        context(outcome)
    );
    assert!(
        text.contains("not linked")
            && text.contains(verdict.token())
            && text.contains(verdict.plainly()),
        "[{row}] the refusal must say `not linked`, `{}` and `{}`;{}",
        verdict.token(),
        verdict.plainly(),
        context(outcome)
    );
}

/// THEN: GitHub could not be read; nothing was linked or recorded; the
/// token (if any) is never echoed.
pub fn assert_github_unavailable(outcome: &CliOutcome, row: &str) {
    let text = all_output(outcome);
    assert_eq!(
        outcome.status,
        1,
        "[{row}] an unreadable GitHub exits 1;{}",
        context(outcome)
    );
    assert!(
        text.contains("GitHub") && text.contains("nothing recorded"),
        "[{row}] the message must name GitHub and say `nothing recorded`;{}",
        context(outcome)
    );
    assert!(
        !text.contains(REJECTED_GITHUB_TOKEN),
        "[{row}] the token must never be echoed"
    );
}

/// THEN: a usage error (clap exit 2) naming `flag`.
pub fn assert_usage_error(outcome: &CliOutcome, flag: &str, row: &str) {
    assert_eq!(
        outcome.status,
        2,
        "[{row}] usage error exits 2;{}",
        context(outcome)
    );
    assert!(
        outcome.stderr.contains(flag),
        "[{row}] the usage error must name `{flag}`;{}",
        context(outcome)
    );
}

/// THEN: refused as already linked, pointing to `identity verify` (D-a).
pub fn assert_already_linked(outcome: &CliOutcome) {
    let text = all_output(outcome);
    assert_eq!(
        outcome.status,
        1,
        "already linked exits 1;{}",
        context(outcome)
    );
    assert!(
        text.contains("already linked") && text.contains("openlore identity verify"),
        "must say `already linked` and point to `openlore identity verify`;{}",
        context(outcome)
    );
}

/// THEN: the link preview shows the exact public record and its consequences.
pub fn assert_link_preview(outcome: &CliOutcome, did: &str, account: Account) {
    for needle in [
        format!("{did} {SAME_AS} {}", account.subject()),
        account.profile_url(),
        account.id_uri(),
        "not as truth".to_string(),
        "public".to_string(),
        "withdraws it but does not erase it".to_string(),
    ] {
        assert!(
            outcome.stdout.contains(&needle),
            "the link preview must show `{needle}`;{}",
            context(outcome)
        );
    }
}

/// THEN: declining at the confirmation says nothing was linked, published
/// or recorded.
pub fn assert_link_declined(outcome: &CliOutcome) {
    assert!(
        outcome
            .stdout
            .contains("Not linked; nothing published, nothing recorded."),
        "declining must say `Not linked; nothing published, nothing recorded.`;{}",
        context(outcome)
    );
}

/// THEN: the link was published; returns the printed CID. The success message
/// names the retract command (I-8).
pub fn assert_link_published(outcome: &CliOutcome) -> String {
    assert_eq!(
        outcome.status,
        0,
        "the link must publish;{}",
        context(outcome)
    );
    let cid = published_cid_from_stdout(&outcome.stdout);
    assert!(
        outcome
            .stdout
            .contains(&format!("openlore claim retract {cid}")),
        "the publish message must name the retract command (I-8);{}",
        context(outcome)
    );
    cid
}

/// The JSON body of every record my PDS holds.
pub fn pds_bodies(env: &TestEnv) -> Vec<(String, serde_json::Value)> {
    env.pds
        .records()
        .into_iter()
        .map(|r| {
            let body = serde_json::from_str(&r.body)
                .unwrap_or_else(|e| panic!("PDS record body is JSON: {e}"));
            (r.rkey, body)
        })
        .collect()
}

/// THEN (wire, D-f): my PDS holds, under record key == the printed CID, exactly
/// the ADR-084 §2 link of `did` to `account`, and the CID re-verifies.
pub fn assert_link_on_pds(env: &TestEnv, printed_cid: &str, did: &str, account: Account) {
    let record = env
        .pds
        .record_at(&at_uri(did, printed_cid))
        .unwrap_or_else(|| panic!("the link is on my PDS at {}", at_uri(did, printed_cid)));
    assert_eq!(record.rkey, printed_cid, "printed CID == PDS record key");
    let v: serde_json::Value = serde_json::from_str(&record.body).expect("wire JSON");
    assert_eq!(v["subject"], did, "subject is the author's bare DID");
    assert_eq!(v["predicate"], SAME_AS);
    assert_eq!(v["object"], account.subject().as_str());
    assert_eq!(
        v["evidence"],
        serde_json::json!([account.profile_url(), account.id_uri()]),
        "evidence is exactly [profile URL, id URI], in that order"
    );
    assert_eq!(
        claim_domain::Confidence::from_wire(&v["confidence"]).map(|c| c.basis_points()),
        Some(10_000),
        "an identity link is asserted at 10000 bp"
    );
    assert_eq!(
        bare(v["author"].as_str().unwrap_or_default()),
        did,
        "author == subject"
    );
    assert!(
        v.get("references")
            .and_then(|r| r.as_array())
            .is_none_or(|r| r.is_empty()),
        "a first link supersedes nothing"
    );
    let decoded = claim_domain::decode_claim_record(&v, "").expect("decodes as a claim");
    assert_eq!(
        decoded.cid().0,
        printed_cid,
        "the wire record re-verifies to its CID"
    );
}

/// THEN (wire): no record on my PDS would be projected as a link (predicate
/// `sameAs` with bare author == subject).
pub fn assert_no_link_on_pds(env: &TestEnv, row: &str) {
    for (rkey, v) in pds_bodies(env) {
        let author = bare(v["author"].as_str().unwrap_or_default());
        assert!(
            !(v["predicate"] == SAME_AS && v["subject"] == author.as_str()),
            "[{row}] record {rkey} on my PDS is a projectable link: {v}"
        );
    }
}

/// THEN: `identity verify` reported `verdict` for `account`. A verified
/// re-check never says the proof is gone.
pub fn assert_verify_reported(outcome: &CliOutcome, account: Account, verdict: Verdict) {
    let object = account.subject();
    let reported = |l: &str| match verdict {
        Verdict::Verified => says_verified(l) && !l.contains("proof gone"),
        other => l.contains(other.token()),
    };
    assert!(
        outcome
            .stdout
            .lines()
            .any(|l| l.contains(&object) && reported(l) && l.contains(verdict.plainly())),
        "`identity verify` must report `{object}: {}` with `{}` on that line;{}",
        verdict.token(),
        verdict.plainly(),
        context(outcome)
    );
}

/// THEN: a generic composer refused `sameAs` and pointed to `identity link`.
pub fn assert_points_to_identity_link(outcome: &CliOutcome, row: &str) {
    let text = all_output(outcome);
    assert!(
        text.contains(SAME_AS) && text.contains("openlore identity link"),
        "[{row}] the refusal must name `{SAME_AS}` and point to `openlore identity link`;{}",
        context(outcome)
    );
}

/// The weighted rows of a `graph query --weighted` render: the ranked lines
/// that carry a number after `weight ` (the formula and footer lines do not).
pub fn weighted_rows(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| {
            l.match_indices("weight ").any(|(i, m)| {
                l[i + m.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
            })
        })
        .map(str::to_string)
        .collect()
}

/// THEN (F2): no verb repeated the private bio phrase on stdout or stderr;
/// stderr — the CLI's log channel (tracing writes there) — names neither the
/// DID nor the login; and stdout carries no structured log event at all.
pub fn assert_no_link_data_leaked(outcome: &CliOutcome, did: &str, account: Account, row: &str) {
    assert!(
        !outcome.stdout.contains(PRIVATE_BIO_PHRASE),
        "[{row}] stdout never repeats the bio;{}",
        context(outcome)
    );
    assert!(
        !outcome.stderr.contains(PRIVATE_BIO_PHRASE),
        "[{row}] stderr never repeats the bio;{}",
        context(outcome)
    );
    assert!(
        !outcome.stderr.contains(did) && !outcome.stderr.contains(account.login),
        "[{row}] the log channel (stderr) carries no DID or login;{}",
        context(outcome)
    );
    assert!(
        !outcome
            .stdout
            .lines()
            .any(|l| l.trim_start().starts_with("{\"") && l.contains("\"event\"")),
        "[{row}] no log event is written to stdout;{}",
        context(outcome)
    );
}

/// THEN (F2, failure path — review D4): a refused or unreadable run never
/// repeats the bio, never names the DID on its log channel (stderr) and prints
/// no log event on stdout. The login MAY appear on stderr, but only in the
/// refusal line about the account the user asked for (every such line says
/// `nothing recorded`).
pub fn assert_no_link_data_leaked_on_refusal(
    outcome: &CliOutcome,
    did: &str,
    account: Account,
    row: &str,
) {
    assert_ne!(
        outcome.status,
        0,
        "[{row}] precondition: the run was refused;{}",
        context(outcome)
    );
    assert!(
        !all_output(outcome).contains(PRIVATE_BIO_PHRASE),
        "[{row}] the bio is never repeated;{}",
        context(outcome)
    );
    assert!(
        !outcome.stderr.contains(did),
        "[{row}] the log channel (stderr) never names the DID;{}",
        context(outcome)
    );
    let login_lines: Vec<&str> = outcome
        .stderr
        .lines()
        .filter(|l| l.contains(account.login))
        .collect();
    assert!(
        login_lines.iter().all(|l| l.contains("nothing recorded")),
        "[{row}] the login appears on stderr only in the refusal line: {login_lines:?};{}",
        context(outcome)
    );
    assert!(
        !outcome
            .stdout
            .lines()
            .any(|l| l.trim_start().starts_with("{\"") && l.contains("\"event\"")),
        "[{row}] no log event is written to stdout;{}",
        context(outcome)
    );
}

/// The lines a reader prints as an identity link: `did:… is github:…` or
/// `did:… claims github:…` (the shape `assert_shown` pins).
pub fn link_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| {
            l.split_whitespace()
                .collect::<Vec<_>>()
                .windows(3)
                .any(|w| {
                    w[0].starts_with("did:")
                        && (w[1] == "is" || w[1] == "claims")
                        && w[2].starts_with("github:")
                })
        })
        .map(str::to_string)
        .collect()
}

// =============================================================================
// The frozen pre-v7 store (IL-13a/b; regenerated only on a binary before v7)
// =============================================================================

/// Stands for the dump directory inside the checked-in `load.sql`.
const FIXTURE_DIR_TOKEN: &str = "@FIXTURE_DIR@";

/// The checked-in copy of a store made by the binary at schema head v6,
/// BEFORE DELIVER adds v7 (see the fixture's README.md).
pub fn pre_v7_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/identity-link-pre-v7-store")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("mkdir {}: {e}", to.display()));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("read {}: {e}", from.display()))
    {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
        }
    }
}

impl LinkWorld {
    /// My store is the frozen pre-v7 copy, beside GitHub serving `accounts`.
    /// The DuckDB file is rebuilt from the checked-in `EXPORT DATABASE` dump
    /// (schema DDL incl. `schema_version` rows v1..v6, plus data) — a few KB
    /// instead of a multi-MB binary file.
    pub fn with_frozen_pre_v7_store(accounts: &[Account]) -> Self {
        let env = TestEnv::fresh();
        copy_tree(&pre_v7_fixture_dir().join("home"), &env.home);
        let conn = duckdb::Connection::open(env.duckdb_path()).expect("create store");
        let dump = pre_v7_fixture_dir().join("db_export");
        let read = |f: &str| std::fs::read_to_string(dump.join(f)).expect("fixture dump file");
        conn.execute_batch(&read("schema.sql"))
            .expect("frozen v6 schema");
        conn.execute_batch(
            &read("load.sql").replace(FIXTURE_DIR_TOKEN, &dump.display().to_string()),
        )
        .expect("frozen v6 rows");
        drop(conn);
        Self {
            env,
            github: FakeGithubAccounts::start(accounts.iter().map(|a| a.on_github()).collect()),
            peers: BTreeMap::new(),
            github_token: Cell::new(None),
        }
    }
}

impl LinkWorld {
    /// My store is a frozen copy checked in under `fixture` (an `EXPORT
    /// DATABASE` dump in `db_export/` plus the rest of the home in `home/`),
    /// beside GitHub serving `accounts`. Generalises
    /// [`LinkWorld::with_frozen_pre_v7_store`] for later frozen stores
    /// (owner-only-signing RA-12b: the frozen v7 store).
    pub fn with_frozen_store_from(fixture: &Path, accounts: &[Account]) -> Self {
        let env = TestEnv::fresh();
        copy_tree(&fixture.join("home"), &env.home);
        let conn = duckdb::Connection::open(env.duckdb_path()).expect("create store");
        let dump = fixture.join("db_export");
        let read = |f: &str| std::fs::read_to_string(dump.join(f)).expect("fixture dump file");
        conn.execute_batch(&read("schema.sql"))
            .expect("frozen schema");
        conn.execute_batch(
            &read("load.sql").replace(FIXTURE_DIR_TOKEN, &dump.display().to_string()),
        )
        .expect("frozen rows");
        drop(conn);
        Self {
            env,
            github: FakeGithubAccounts::start(accounts.iter().map(|a| a.on_github()).collect()),
            peers: BTreeMap::new(),
            github_token: Cell::new(None),
        }
    }
}

/// Fixture regeneration only: dump `env`'s store into `fixture/db_export`
/// (relocatable) and copy the rest of its home into `fixture/home`.
pub fn freeze_store_into(env: &TestEnv, fixture: &Path) {
    let _ = std::fs::remove_dir_all(fixture.join("home"));
    let _ = std::fs::remove_dir_all(fixture.join("db_export"));
    copy_tree(&env.home, &fixture.join("home"));
    let _ = std::fs::remove_file(fixture.join("home/.local/share/openlore/openlore.duckdb"));
    let _ = std::fs::remove_file(fixture.join("home/.local/share/openlore/openlore.duckdb.wal"));
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open store");
    let dump = fixture.join("db_export");
    conn.execute_batch(&format!("EXPORT DATABASE '{}';", dump.display()))
        .expect("export the store");
    let load = std::fs::read_to_string(dump.join("load.sql")).expect("load.sql");
    std::fs::write(
        dump.join("load.sql"),
        load.replace(&dump.display().to_string(), FIXTURE_DIR_TOKEN),
    )
    .expect("rewrite load.sql");
}

/// The frozen store's recorded `graph query --object memory-safety --federated`
/// output, captured with the pre-v7 binary when the fixture was made.
pub fn pre_v7_golden_claims() -> String {
    std::fs::read_to_string(pre_v7_fixture_dir().join("graph_query_memory_safety.txt"))
        .expect("golden graph query output")
}

/// Fixture regeneration only: dump `env`'s store (`EXPORT DATABASE`), copy the
/// rest of its home, and record the golden read-back.
pub fn freeze_pre_v7_store(env: &TestEnv, golden: &str) {
    let dir = pre_v7_fixture_dir();
    let _ = std::fs::remove_dir_all(dir.join("home"));
    let _ = std::fs::remove_dir_all(dir.join("db_export"));
    copy_tree(&env.home, &dir.join("home"));
    let _ = std::fs::remove_file(dir.join("home/.local/share/openlore/openlore.duckdb"));
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open store");
    let dump = dir.join("db_export");
    conn.execute_batch(&format!("EXPORT DATABASE '{}';", dump.display()))
        .expect("export the v6 store");
    // Make the dump relocatable: the COPY paths become a token.
    let load = std::fs::read_to_string(dump.join("load.sql")).expect("load.sql");
    std::fs::write(
        dump.join("load.sql"),
        load.replace(&dump.display().to_string(), FIXTURE_DIR_TOKEN),
    )
    .expect("rewrite load.sql");
    std::fs::write(dir.join("graph_query_memory_safety.txt"), golden).expect("write golden");
}

/// The applied schema versions, ascending (observation of the migration).
pub fn schema_versions(env: &TestEnv) -> Vec<i64> {
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB");
    let mut stmt = conn
        .prepare("SELECT CAST(version AS BIGINT) FROM schema_version ORDER BY version")
        .expect("schema_version");
    stmt.query_map([], |r| r.get(0))
        .expect("query schema_version")
        .map(|r| r.expect("version"))
        .collect()
}

/// Every row of the pre-existing tables, rendered column by column, plus the
/// claim files' bytes — "prior rows unchanged" across a migration.
pub fn prior_rows(env: &TestEnv) -> BTreeMap<String, Vec<String>> {
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB");
    let mut all = BTreeMap::new();
    for table in [
        "claims",
        "peer_claims",
        "contribution_links",
        "peer_subscriptions",
    ] {
        let Ok(mut stmt) = conn.prepare(&format!("SELECT * FROM {table}")) else {
            continue;
        };
        let mut rows: Vec<String> = Vec::new();
        let mut cursor = stmt.query([]).expect("query table");
        while let Some(row) = cursor.next().expect("row") {
            let n = row.as_ref().column_count();
            let cells: Vec<String> = (0..n)
                .map(|i| format!("{:?}", row.get::<_, duckdb::types::Value>(i).expect("cell")))
                .collect();
            rows.push(cells.join("|"));
        }
        rows.sort();
        all.insert(table.to_string(), rows);
    }
    all.insert(
        "claim_files".to_string(),
        claim_files(env)
            .into_iter()
            .map(|(cid, bytes)| format!("{cid}:{}", String::from_utf8_lossy(&bytes)))
            .collect(),
    );
    all
}

// =============================================================================
// SECONDARY guardrail: the fail-closed store universe (waiver in wave-decisions)
// =============================================================================

/// One recorded proof check (timestamps excluded: clock-dependent).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckRow {
    pub subject_did: String,
    pub github_user_id: i64,
    pub login_seen: String,
    pub verdict: String,
}

impl CheckRow {
    pub fn of(did: &str, account: Account, verdict: Verdict) -> Self {
        Self {
            subject_did: did.to_string(),
            github_user_id: account.id as i64,
            login_seen: account.login.to_string(),
            verdict: verdict.token().to_string(),
        }
    }
}

/// Every recorded proof check, sorted (empty only when the store or the
/// table does not exist; any other read failure panics).
pub fn link_checks(env: &TestEnv) -> Vec<CheckRow> {
    let path = env.duckdb_path();
    if !path.exists() {
        return Vec::new();
    }
    let conn = duckdb::Connection::open(&path)
        .unwrap_or_else(|e| panic!("open DuckDB for identity link checks: {e}"));
    // A store made before identity links (pre-v7) has no proof-check table:
    // that, and only that, reads as "no checks". Any other failure to read the
    // table panics, so an "unchanged" guard can never pass on an unread table
    // (review M1).
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM information_schema.tables \
             WHERE table_name = 'identity_link_checks'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("look up the identity link checks table: {e}"));
    if !table_exists {
        return Vec::new();
    }
    let mut stmt = conn
        .prepare(
            "SELECT subject_did, CAST(github_user_id AS BIGINT), login_seen, verdict \
             FROM identity_link_checks",
        )
        .unwrap_or_else(|e| panic!("prepare the identity link checks read: {e}"));
    let mut rows: Vec<CheckRow> = stmt
        .query_map([], |row| {
            Ok(CheckRow {
                subject_did: row.get(0)?,
                github_user_id: row.get(1)?,
                login_seen: row.get(2)?,
                verdict: row.get(3)?,
            })
        })
        .unwrap_or_else(|e| panic!("query identity link checks: {e}"))
        .map(|r| r.unwrap_or_else(|e| panic!("decode identity link check: {e}")))
        .collect();
    rows.sort();
    rows
}

/// GIVEN (seeded store row — no verb can write it, review D7): my store holds
/// a check of `did`'s link to `account` that saw the login `login_seen`, at
/// `at` (RFC 3339, UTC). Used only to model a stored login that is not a
/// GitHub login, which the strict GitHub reads can no longer produce.
pub fn given_a_check_recorded_seeing(
    env: &TestEnv,
    did: &str,
    account: Account,
    login_seen: &str,
    verdict: Verdict,
    at: &str,
) {
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB");
    conn.execute(
        "INSERT INTO identity_link_checks \
         (subject_did, github_user_id, login_seen, verdict, checked_at) \
         VALUES (?, ?, ?, ?, CAST(? AS TIMESTAMP))",
        duckdb::params![
            did,
            account.id as i64,
            login_seen,
            verdict.token(),
            at.trim_end_matches('Z').replace('T', " ")
        ],
    )
    .unwrap_or_else(|e| panic!("seed an identity link check: {e}"));
}

/// The identity verbs' store universe: the house `store_universe_slots` plus
/// the proof-check rows.
pub fn link_universe_slots() -> HashSet<String> {
    let mut slots = store_universe_slots();
    slots.insert("local.identity_link_checks".to_string());
    slots
}

/// Capture the identity-link universe.
pub fn capture_link_universe(env: &TestEnv) -> HashMap<String, String> {
    let mut snapshot = capture_store_universe(env);
    snapshot.insert(
        "local.identity_link_checks".to_string(),
        format!("{:?}", link_checks(env)),
    );
    snapshot
}

/// GUARDRAIL: nothing in my store, my PDS or my proof history changed.
pub fn assert_link_store_unchanged(before: &HashMap<String, String>, env: &TestEnv) {
    let after = capture_link_universe(env);
    assert_state_delta(before, &after, &link_universe_slots(), &Delta::new());
}

/// [`assert_link_store_unchanged`] inside a table loop: the failure names `[row]`.
pub fn assert_link_store_unchanged_in(row: &str, before: &HashMap<String, String>, env: &TestEnv) {
    let after = capture_link_universe(env);
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_state_delta(before, &after, &link_universe_slots(), &Delta::new())
    }));
    if let Err(payload) = checked {
        let reason = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        panic!("[{row}] {reason}");
    }
}

/// GUARDRAIL: exactly the named slots changed; every other slot is unchanged.
pub fn assert_link_store_delta(
    before: &HashMap<String, String>,
    env: &TestEnv,
    changes: &[(&str, String)],
) {
    let after = capture_link_universe(env);
    let expected = changes.iter().fold(Delta::new(), |d, (slot, value)| {
        d.with_slot(*slot, set_to(value.clone()))
    });
    assert_state_delta(before, &after, &link_universe_slots(), &expected);
}

/// The `local.identity_link_checks` slot value for `before` plus `added`.
pub fn checks_with(before: &[CheckRow], added: &[CheckRow]) -> String {
    let mut rows: Vec<CheckRow> = before.iter().chain(added).cloned().collect();
    rows.sort();
    format!("{rows:?}")
}

/// The `local.claims.row_count` slot value after `n` more claims.
pub fn claims_plus(before: &HashMap<String, String>, n: i64) -> String {
    (before["local.claims.row_count"].parse::<i64>().unwrap() + n).to_string()
}

/// The `local.claim_files` slot value now.
pub fn claim_file_slot(env: &TestEnv) -> String {
    format!("{:?}", claim_files(env).keys().collect::<Vec<_>>())
}

/// The `pds.records.len` slot value after `n` more records.
pub fn pds_plus(before: &HashMap<String, String>, n: usize) -> String {
    (before["pds.records.len"].parse::<usize>().unwrap() + n).to_string()
}
