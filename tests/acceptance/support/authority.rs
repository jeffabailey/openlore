//! owner-only-signing — shared acceptance support (DISTILL 2026-10-20, Quinn).
//!
//! Step vocabulary for `owner_only_signing.rs` (slice 1, person gate + the
//! walking skeleton), `owner_only_signing_repos.rs` (slice 2 repo authority +
//! slice 5 imported candidates) and `owner_only_signing_readers.rs` (slice 3
//! labels and weight). Every WHEN drives the REAL `openlore` binary (the
//! production composition root, Pillar 3); every GIVEN seeds state through
//! REAL verbs (`identity link`, `authority verify`, `scrape`, `claim add`,
//! `peer add/pull`). The only doubles are driven-EXTERNAL and in the project
//! policy: `FakeGithubAccounts` (+ the ADR-087 repo-authority lie catalogue,
//! `AuthorityFile`), `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! **Observation order** (GDL decision D-e, applied as orchestrator ruling
//! R1 of review iteration 1): a scenario's PRIMARY Then observes through
//! driving ports — exit code, the refusal and hand-off text, the reader
//! labels, `graph query` / `identity list` / `authority verify` /
//! `candidates list` output ([`WhatJeffSees`]), the wire JSON my PDS
//! received, and GitHub's request log (an external side effect with a
//! budget, observed at the driven-port boundary — GDL D-VL-T13, ruling R2).
//! The fail-closed store universe (`guardrail_*`: the house
//! `store_universe_slots` + `identity_link_checks` + `authority.repo_observations`
//! + `candidates.imported.count`) is a labelled SECONDARY guardrail under the
//! explicit Mandate-8 waiver in `distill/wave-decisions.md`, never the only
//! evidence for a Then.
//!
//! **Wording pins** (Q-OOS-D1, DISTILL-owned, DWD-OOS-3): the constants and
//! `Refusal::phrase` / `CanSign::phrase` below are the only places a rendered
//! phrase is spelled. DELIVER may change a pin with a one-line edit here.
//!
//! Mandate-12: domain nouns (subject, repo, authority label, refusal reason,
//! who can sign, authority-file shape, instant) are typed values; scenario
//! bodies never spell a login, repo path, label or rendered phrase inline.

#![allow(dead_code)]

use super::identity_links::*;
use super::people::*;
use super::state_delta::{assert_state_delta, set_to, Delta};
use super::*;
pub use openlore_test_support::AuthorityFile;
use openlore_test_support::{FakeContributor, GithubAccount, GithubRepo};
use std::collections::{HashMap, HashSet};

// =============================================================================
// Domain vocabulary
// =============================================================================

/// An organization that owns a repo (no bio, never an owner tier, ADR-087 §2).
pub const ACME: Account = Account {
    login: "acme-tools",
    id: 8_200_001,
};

/// A repo, as its `github:<owner>/<name>` subject names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: &'static str,
    pub name: &'static str,
}

impl RepoRef {
    pub const fn new(owner: &'static str, name: &'static str) -> Self {
        Self { owner, name }
    }

    /// `owner/name`.
    pub fn full(self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// `github:owner/name`.
    pub fn subject(self) -> String {
        format!("github:{}", self.full())
    }
}

/// Jeff's personal repo (owned by his linked account `jeffbailey`).
pub const TIDEPOOL: RepoRef = RepoRef::new("jeffbailey", "tidepool");
/// Jeff's personal repo after he renames it on GitHub.
pub const TIDEPOOL_RENAMED: RepoRef = RepoRef::new("jeffbailey", "tidepool-rs");
/// Jeff's repo after he transfers it to the acme-tools organization.
pub const TIDEPOOL_AT_ACME: RepoRef = RepoRef::new("acme-tools", "tidepool");
/// An organization repo (acme-tools); only its authority file can grant.
pub const ANVIL: RepoRef = RepoRef::new("acme-tools", "anvil");
/// Rachel's personal repo.
pub const KELP: RepoRef = RepoRef::new("rachelk", "kelp");
/// A repo GitHub does not have (404).
pub const VANISHED: RepoRef = RepoRef::new("acme-tools", "vanished");

/// What a claim is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// `github:<login>` — a person (adheresToPhilosophy).
    Person(Account),
    /// `github:<owner>/<repo>` — a repo (embodiesPhilosophy).
    Repo(RepoRef),
}

impl Subject {
    pub fn uri(self) -> String {
        match self {
            Subject::Person(a) => a.subject(),
            Subject::Repo(r) => r.subject(),
        }
    }

    pub fn predicate(self) -> &'static str {
        match self {
            Subject::Person(_) => ADHERES,
            Subject::Repo(_) => EMBODIES,
        }
    }

    pub fn evidence(self) -> String {
        match self {
            Subject::Person(a) => a.profile_url(),
            Subject::Repo(r) => format!("https://github.com/{}", r.full()),
        }
    }
}

/// The reader label set — one spelling everywhere (ADR-086 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    Owner,
    RepoListed,
    ThirdParty,
    NotChecked,
}

impl Authority {
    pub fn label(self) -> &'static str {
        match self {
            Authority::Owner => "owner",
            Authority::RepoListed => "repo-listed",
            Authority::ThirdParty => "third-party",
            Authority::NotChecked => "authority not checked",
        }
    }
}

/// Network search's label for every `github:` assertion row (UC-7, widened
/// from GDL's link-rows-only `SEARCH_LABEL`).
pub const NOT_CHECKED_HERE: &str = "authority not checked here";

/// Why a signature was refused (ADR-086/087 `Unattested` reasons + the
/// signing-window refusals). `phrase()` is the pinned sentence fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No identity link of mine names the account the person subject names
    /// (incl. somebody else's account: ADR-087 §1 has no other reason).
    NoLink,
    /// My link's latest check is not verified (proof gone).
    LinkNotVerified,
    /// The subject login is not the current login of my linked account.
    LoginNotCurrent,
    /// A user-owned repo whose owner is not my linked account.
    NotRepoOwner,
    /// An organization owns the repo and its authority file does not list me.
    OrgOwnedNotListed,
    /// The authority file exists but is not a valid authority file.
    FileInvalid,
    /// GitHub does not serve the repo.
    RepoNotFound,
    /// GitHub serves the repo under another name now.
    Renamed,
    /// The latest local check is older than the signing window (7 days).
    CheckTooOld,
    /// No local authority check exists (or it is dated in the future).
    NotChecked,
    /// GitHub could not answer (rate limit, outage, refused redirect).
    GithubUnfinished,
}

impl Refusal {
    pub fn phrase(self) -> &'static str {
        match self {
            Refusal::NoLink => "you have no identity link to",
            Refusal::LinkNotVerified => "your identity link is not verified",
            Refusal::LoginNotCurrent => "is no longer the login of your linked GitHub account",
            Refusal::NotRepoOwner => "is not owned by your linked GitHub account",
            Refusal::OrgOwnedNotListed => "is owned by the organization",
            Refusal::FileInvalid => "is not a valid authority file",
            Refusal::RepoNotFound => "was not found on GitHub",
            Refusal::Renamed => "GitHub serves this repo as",
            Refusal::CheckTooOld => "authority was last checked",
            Refusal::NotChecked => "authority has not been checked",
            Refusal::GithubUnfinished => "GitHub could not be asked",
        }
    }
}

/// Who the hand-off names as able to sign (ADR-086 §10, OOS-D12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanSign {
    /// The account's owner, after linking it.
    Login(&'static str),
    /// A DID listed in the repo's authority file.
    ListedDid,
    /// Either the owner login or a listed DID (user-owned repo).
    LoginOrListedDid(&'static str),
}

/// The fixed hand-off lead.
pub const CAN_BE_SIGNED_BY: &str = "Can be signed by";
/// The fixed authority file path, as the hand-off names it.
pub const AUTHORITY_FILE: &str = ".github/openlore-authority";

impl CanSign {
    /// The phrases the hand-off must contain.
    pub fn phrases(self) -> Vec<String> {
        let link = |login: &str| format!("openlore identity link github {login}");
        match self {
            CanSign::Login(login) => vec![login.to_string(), link(login)],
            CanSign::ListedDid => vec![AUTHORITY_FILE.to_string()],
            CanSign::LoginOrListedDid(login) => {
                vec![login.to_string(), link(login), AUTHORITY_FILE.to_string()]
            }
        }
    }
}

/// The re-check command a stale or missing check names.
pub fn verify_command(subject: Subject) -> String {
    format!("openlore authority verify --subject {}", subject.uri())
}

/// The command the hand-off names for the owner of a repo.
pub fn scrape_handoff_command(repo: RepoRef) -> String {
    format!("openlore scrape github {} --sign", repo.full())
}

/// The per-subject line for a subject the signer may sign (scrape / infer views).
pub const YOU_CAN_SIGN: &str = "You can sign";

/// Jeff's unpublished claim about `github:BurntSushi/ripgrep` in the frozen
/// pre-v7 store (`tests/fixtures/identity-link-pre-v7-store`): a claim signed
/// before owner-only signing existed, never published.
pub const PRE_FEATURE_UNPUBLISHED_CID: &str =
    "bafyreiaskxsxh27xm4rzydzzjhbsp6nh33jfkh64jfjak3oezvbri6dlgq";
/// The repo that pre-feature claim is about.
pub const RIPGREP: RepoRef = RepoRef::new("BurntSushi", "ripgrep");
/// Rachel's pulled claim in the same frozen store.
pub const REGEX: RepoRef = RepoRef::new("rust-lang", "regex");

/// A philosophy's short name as readers print it (`memory-safety`); it is
/// also a substring of the full id, so it matches either rendering.
pub fn short_name(philosophy: &str) -> &str {
    philosophy.rsplit('.').next().unwrap_or(philosophy)
}

/// `authority verify` with no `github:` claim to re-check.
pub const NOTHING_TO_RECHECK: &str = "No GitHub claims to re-check";

/// Imported-candidates empty-list line (slice 5).
pub const NO_IMPORTED_CANDIDATES: &str = "No imported candidates";
/// The weighted footer's lead ("N claims not counted: …").
pub const NOT_COUNTED: &str = "not counted";
/// The `--include-unattested` section heading.
pub const UNATTESTED_SECTION: &str = "Unattested claims, by author";
/// The usage error for `--include-unattested` without `--weighted` (P-OOS-1).
pub const INCLUDE_UNATTESTED_NEEDS_WEIGHTED: &str = "--include-unattested requires --weighted";
/// The budget refusal's retry hint (ADR-087 §6).
pub const RETRY_AFTER: &str = "retry after";

// -----------------------------------------------------------------------------
// Pinned instants (`OPENLORE_TEST_NOW`); LINKED_AT is GDL's 2026-10-01T09:00Z.
// -----------------------------------------------------------------------------

pub const CHECK_PLUS_2_DAYS: &str = "2026-10-03T09:00:00Z";
pub const CHECK_PLUS_7_DAYS: &str = "2026-10-08T09:00:00Z";
pub const CHECK_PLUS_7_DAYS_1_MIN: &str = "2026-10-08T09:01:00Z";
pub const CHECK_PLUS_9_DAYS: &str = "2026-10-10T09:00:00Z";
pub const CHECK_PLUS_20_DAYS: &str = "2026-10-21T09:00:00Z";

// =============================================================================
// The world: my store, my linked account, repos on GitHub
// =============================================================================

/// My store beside GitHub serving Jeff (personal repo tidepool), Rachel
/// (personal repo kelp), Tobias, and the acme-tools organization (repo anvil,
/// no authority file yet). Every repo carries a Cargo.lock, so a scrape
/// derives a dependency-pinning candidate (candidate 1).
pub fn authority_world() -> LinkWorld {
    let world = LinkWorld::with(&[JEFF_GH, RACHEL_GH, TOBIAS_GH]);
    world.github.put_repo(
        JEFF_GH.login,
        GithubRepo::owned(TIDEPOOL.name).with_cargo_lock(),
    );
    world.github.put_repo(
        RACHEL_GH.login,
        GithubRepo::owned(KELP.name).with_cargo_lock(),
    );
    world.github.register(
        GithubAccount::new(ACME.login, ACME.id)
            .organization()
            .repo(GithubRepo::owned(ANVIL.name).with_cargo_lock()),
    );
    world
}

/// GIVEN: Jeff linked his GitHub account `jeffbailey` (bio names his DID) at
/// `LINKED_AT`; the link's check is verified at that instant.
pub fn given_jeff_linked_his_account(world: &LinkWorld) -> String {
    given_i_linked(world, JEFF_GH, Some(LINKED_AT))
}

/// GIVEN (outside OpenLore): `repo`'s default branch holds `file`.
pub fn the_repo_holds_authority_file(world: &LinkWorld, repo: RepoRef, file: AuthorityFile) {
    world
        .github
        .set_authority_file(repo.owner, repo.name, Some(file));
}

/// An authority file that lists exactly these DIDs, one per line, with a
/// leading comment.
pub fn listing(dids: &[&str]) -> AuthorityFile {
    let mut text = String::from("# OpenLore signers for this repo\n");
    for did in dids {
        text.push_str(did);
        text.push('\n');
    }
    AuthorityFile::Lists(text)
}

/// An authority file listing `dids`, padded with a comment line to exactly
/// `size` bytes (the 4 KiB limit is inclusive, ADR-087 §3).
pub fn listing_of_exactly(size: usize, dids: &[&str]) -> AuthorityFile {
    let AuthorityFile::Lists(mut text) = listing(dids) else {
        unreachable!("listing() builds a regular file")
    };
    let pad = size
        .checked_sub(text.len() + 2)
        .expect("room for the padding comment");
    text.push('#');
    text.push_str(&"x".repeat(pad));
    text.push('\n');
    assert_eq!(text.len(), size);
    AuthorityFile::Lists(text)
}

/// GIVEN (outside OpenLore): Jeff transfers tidepool to acme-tools.
pub fn jeff_transfers_tidepool_to_acme(world: &LinkWorld) {
    world
        .github
        .transfer_repo(TIDEPOOL.owner, TIDEPOOL.name, ACME.login);
}

/// GIVEN (outside OpenLore): Jeff renames tidepool to tidepool-rs.
pub fn jeff_renames_tidepool(world: &LinkWorld) {
    world
        .github
        .rename_repo(TIDEPOOL.owner, TIDEPOOL.name, TIDEPOOL_RENAMED.name);
}

/// GIVEN: Rachel (a followed peer) published these claims and Jeff pulled them.
pub fn given_rachel_published(world: &mut LinkWorld, specs: Vec<PeerClaimSpec>) -> Vec<String> {
    given_peer_published(world, Peer::Rachel, specs)
}

/// GIVEN: Tobias (a followed peer) published these claims and Jeff pulled them.
pub fn given_tobias_published(world: &mut LinkWorld, specs: Vec<PeerClaimSpec>) -> Vec<String> {
    given_peer_published(world, Peer::Tobias, specs)
}

/// A peer's memory-safety claim about `subject` (repo or person).
pub fn peer_claim_about(subject: Subject) -> PeerClaimSpec {
    PeerClaimSpec {
        subject: subject.uri(),
        predicate: subject.predicate().to_string(),
        object: MEMORY_SAFETY.to_string(),
        confidence: 0.6,
        evidence: vec![subject.evidence()],
        references: Vec::new(),
        composed_at: "2026-09-02T10:00:00Z".to_string(),
    }
}

/// The frozen pre-v7 store (Jeff's unpublished claim about BurntSushi/ripgrep,
/// Rachel's pulled claim about rust-lang/regex) beside GitHub serving both
/// repos' owners (BurntSushi a user, rust-lang an organization), no
/// authority files.
pub fn frozen_store_world() -> LinkWorld {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH, RACHEL_GH]);
    world.github.register(
        GithubAccount::new(BURNTSUSHI_GH.login, BURNTSUSHI_GH.id)
            .repo(GithubRepo::owned(RIPGREP.name)),
    );
    world.github.register(
        GithubAccount::new(REGEX.owner, 5_430_905)
            .organization()
            .repo(GithubRepo::owned(REGEX.name)),
    );
    world
}

/// The checked-in copy of a store made by the binary at schema head v7
/// (`tests/fixtures/owner-only-signing-v7-store`, README there), BEFORE
/// DELIVER adds v8: Jeff's link to `jeffbailey`, first checked on
/// 20 September (`BEFORE_THE_CHECK`) and re-checked on 1 October (`LINKED_AT`).
pub fn v7_fixture_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/owner-only-signing-v7-store")
}

/// The frozen v7 store beside GitHub serving Jeff (no network needed).
pub fn frozen_v7_store_world() -> LinkWorld {
    LinkWorld::with_frozen_store_from(&v7_fixture_dir(), &[JEFF_GH])
}

/// `openlore identity list` as the v7 binary printed it at `V7_LISTED_AT`.
pub fn v7_golden_identity_list() -> String {
    std::fs::read_to_string(v7_fixture_dir().join("identity_list.txt"))
        .expect("golden identity list")
}

/// The instant the v7 golden `identity list` was printed at.
pub const V7_LISTED_AT: &str = CHECK_PLUS_2_DAYS;

/// Fixture regeneration only: freeze `env`'s v7 store and its golden
/// `identity list` output.
pub fn freeze_v7_store(env: &TestEnv, golden_identity_list: &str) {
    let dir = v7_fixture_dir();
    freeze_store_into(env, &dir);
    std::fs::write(dir.join("identity_list.txt"), golden_identity_list)
        .expect("write golden identity list");
}

/// A network-index record: `author`'s memory-safety claim about `subject`.
pub fn index_claim_about(author: &str, subject: Subject) -> openlore_test_support::RawRecordSpec {
    openlore_test_support::RawRecordSpec {
        predicate: subject.predicate().to_string(),
        ..openlore_test_support::RawRecordSpec::valid(author, &subject.uri(), MEMORY_SAFETY, 0.7)
    }
}

/// A peer's claim that `subject` embodies (or adheres to) `object`.
pub fn peer_claim_about_object(subject: Subject, object: &str) -> PeerClaimSpec {
    PeerClaimSpec {
        object: object.to_string(),
        ..peer_claim_about(subject)
    }
}

// =============================================================================
// Driving the production binary
// =============================================================================

fn on_github<'a>(world: &'a LinkWorld, stdin: &'a str, now: Option<&'a str>) -> Invocation<'a> {
    Invocation {
        github_base: Some(world.github.base_url()),
        stdin,
        now,
        ..Invocation::default()
    }
}

/// stdin for `claim add`: Enter to sign, then answer the publish prompt.
fn sign_then(publish: bool) -> &'static str {
    if publish {
        "\nY\n"
    } else {
        "\nn\n"
    }
}

fn claim_add_args(subject: Subject) -> Vec<String> {
    vec![
        "claim".into(),
        "add".into(),
        "--subject".into(),
        subject.uri(),
        "--predicate".into(),
        subject.predicate().into(),
        "--object".into(),
        MEMORY_SAFETY.into(),
        "--evidence".into(),
        subject.evidence(),
        "--confidence".into(),
        "0.6".into(),
    ]
}

fn run(world: &LinkWorld, args: &[String], inv: Invocation<'_>) -> CliOutcome {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    openlore_with(&world.env, &args, inv)
}

/// WHEN: Jeff signs (and publishes) a memory-safety claim about `subject`
/// with `claim add`, at `now`. GitHub is wired only to observe that signing
/// asks it nothing (C-OOS-9).
pub fn when_jeff_signs(world: &LinkWorld, subject: Subject, now: Option<&str>) -> CliOutcome {
    run(
        world,
        &claim_add_args(subject),
        on_github(world, sign_then(true), now),
    )
}

/// WHEN: Jeff signs a claim about `subject` with the network disabled (no
/// PDS wired; GitHub wired only to observe zero requests), at `now`.
pub fn when_jeff_signs_offline(world: &LinkWorld, subject: Subject, now: &str) -> CliOutcome {
    run(
        world,
        &claim_add_args(subject),
        Invocation {
            offline: true,
            ..on_github(world, sign_then(false), Some(now))
        },
    )
}

/// GIVEN: Jeff signed a claim about `subject` (declining to publish) while he
/// held authority. Returns its CID.
pub fn given_jeff_signed_unpublished(world: &LinkWorld, subject: Subject, now: &str) -> String {
    let out = run(
        world,
        &claim_add_args(subject),
        on_github(world, sign_then(false), Some(now)),
    );
    assert_eq!(
        out.status,
        0,
        "precondition: Jeff may sign about {};\n--- stdout ---\n{}\n--- stderr ---\n{}",
        subject.uri(),
        out.stdout,
        out.stderr
    );
    signed_cid_from_stdout(&out.stdout)
}

/// GIVEN: Jeff signed and published a claim about `subject` while he held
/// authority. Returns its CID.
pub fn given_jeff_signed_and_published(world: &LinkWorld, subject: Subject, now: &str) -> String {
    let out = when_jeff_signs(world, subject, Some(now));
    assert_eq!(
        out.status,
        0,
        "precondition: Jeff may sign and publish about {};\n--- stdout ---\n{}\n--- stderr ---\n{}",
        subject.uri(),
        out.stdout,
        out.stderr
    );
    signed_cid_from_stdout(&out.stdout)
}

/// WHEN: Jeff disputes claim `cid` (`claim counter`, reason given) and publishes.
pub fn when_jeff_counters(world: &LinkWorld, cid: &str, now: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &[
            "claim",
            "counter",
            cid,
            "--reason",
            "the evidence does not show it",
        ],
        on_github(world, "\nY\n", Some(now)),
    )
}

/// WHEN: Jeff withdraws his own claim `cid` (`claim retract`).
pub fn when_jeff_retracts(world: &LinkWorld, cid: &str, now: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["claim", "retract", cid],
        on_github(world, "\nY\n", Some(now)),
    )
}

/// WHEN: Jeff publishes the stored claim `cid` (`claim publish <cid>`).
pub fn when_jeff_publishes(world: &LinkWorld, cid: &str, now: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["claim", "publish", cid],
        on_github(world, "", Some(now)),
    )
}

/// WHEN: Jeff publishes the stored claim `cid` with the network disabled.
pub fn when_jeff_publishes_offline(world: &LinkWorld, cid: &str, now: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["claim", "publish", cid],
        Invocation {
            offline: true,
            ..on_github(world, "", Some(now))
        },
    )
}

/// WHEN: Jeff re-checks authority (`authority verify [filters…]`) at `now`.
pub fn when_jeff_verifies_authority(
    world: &LinkWorld,
    filters: &[&str],
    now: Option<&str>,
) -> CliOutcome {
    let mut args = vec!["authority", "verify"];
    args.extend_from_slice(filters);
    openlore_with(&world.env, &args, on_github(world, "", now))
}

/// A personal access token the GitHub double accepts; must never be echoed.
pub const JEFFS_GITHUB_TOKEN: &str = "ghp_ownerOnlySigningFixture00000000000000";

/// How Jeff's GitHub budget stands when he re-checks authority (RA-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubBudget {
    /// No token; the anonymous budget runs out at the authority-file read.
    AnonymousExhausted,
    /// A token is configured and its budget runs out at the authority-file read.
    TokenExhausted,
    /// The configured token is rejected (expired or revoked).
    TokenRejected,
}

impl GithubBudget {
    /// The token Jeff has configured, if any.
    pub fn token(self) -> Option<&'static str> {
        match self {
            GithubBudget::AnonymousExhausted => None,
            GithubBudget::TokenExhausted => Some(JEFFS_GITHUB_TOKEN),
            GithubBudget::TokenRejected => Some(REJECTED_GITHUB_TOKEN),
        }
    }
}

/// GIVEN (outside OpenLore): GitHub's budget for Jeff stands as `budget`.
pub fn githubs_budget_is(world: &LinkWorld, budget: GithubBudget) {
    match budget {
        GithubBudget::AnonymousExhausted | GithubBudget::TokenExhausted => {
            world.github.exhaust_budget_at_authority_reads(true)
        }
        GithubBudget::TokenRejected => world.github.reject_token(true),
    }
}

/// WHEN: Jeff re-checks authority (`authority verify [filters…]`) with his
/// configured token (if any) at `now`.
pub fn when_jeff_verifies_authority_with(
    world: &LinkWorld,
    filters: &[&str],
    budget: GithubBudget,
    now: &str,
) -> CliOutcome {
    let mut args = vec!["authority", "verify"];
    args.extend_from_slice(filters);
    openlore_with(
        &world.env,
        &args,
        Invocation {
            github_token: budget.token(),
            ..on_github(world, "", Some(now))
        },
    )
}

/// How GitHub fails Jeff's own profile at signing time (ruling R3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileTrouble {
    /// GitHub says it is too busy (rate-limited).
    TooBusy,
    /// GitHub cannot be reached.
    Unreachable,
}

/// GIVEN (outside OpenLore): GitHub answers Jeff's profile `served` more
/// times (what the import itself reads), then fails it with `trouble` — so
/// the sign-time refresh of his own link cannot finish.
pub fn github_stops_answering_jeffs_profile_after(
    world: &LinkWorld,
    served: usize,
    trouble: ProfileTrouble,
) {
    match trouble {
        ProfileTrouble::TooBusy => world.github.rate_limit_profile_after(JEFF_GH.login, served),
        ProfileTrouble::Unreachable => world.github.drop_profile_after(JEFF_GH.login, served),
    }
}

/// GIVEN: Jeff re-checked authority for every `github:` claim in his store.
pub fn given_jeff_verified_authority(world: &LinkWorld, now: &str) {
    let out = when_jeff_verifies_authority(world, &[], Some(now));
    assert_eq!(
        out.status, 0,
        "precondition: `authority verify` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.stdout, out.stderr
    );
}

/// GIVEN: Jeff re-checked the authority of one subject (`--subject`) at `now`.
pub fn given_jeff_verified_authority_of(world: &LinkWorld, subject: Subject, now: &str) {
    let uri = subject.uri();
    let out = when_jeff_verifies_authority(world, &["--subject", &uri], Some(now));
    assert_eq!(
        out.status, 0,
        "precondition: `authority verify --subject {uri}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.stdout, out.stderr
    );
}

/// WHEN: Jeff imports `repo` (`scrape github <owner/repo>`, nothing signed).
pub fn when_jeff_imports(world: &LinkWorld, repo: RepoRef, now: Option<&str>) -> CliOutcome {
    openlore_with(
        &world.env,
        &["scrape", "github", &repo.full()],
        on_github(world, "", now),
    )
}

/// GIVEN: Jeff imported `repo` (and it ran).
pub fn given_jeff_imported(world: &LinkWorld, repo: RepoRef, now: &str) -> CliOutcome {
    let out = when_jeff_imports(world, repo, Some(now));
    assert_eq!(
        out.status,
        0,
        "precondition: `scrape github {}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        repo.full(),
        out.stdout,
        out.stderr
    );
    out
}

/// WHEN: Jeff imports `repo` and signs candidate 1 (accepting every field,
/// publishing) — `scrape github <owner/repo> --sign 1`.
pub fn when_jeff_imports_and_signs(world: &LinkWorld, repo: RepoRef, now: &str) -> CliOutcome {
    let stdin = sign_stdin("", true);
    openlore_with(
        &world.env,
        &["scrape", "github", &repo.full(), "--sign", "1"],
        on_github(world, &stdin, Some(now)),
    )
}

/// WHEN: Jeff reads `account` as a person and signs person candidate 1 —
/// `scrape person <login> --sign 1`.
pub fn when_jeff_scrapes_person_and_signs(
    world: &LinkWorld,
    account: Account,
    now: &str,
) -> CliOutcome {
    let stdin = sign_stdin("", true);
    openlore_with(
        &world.env,
        &["scrape", "person", account.login, "--sign", "1"],
        on_github(world, &stdin, Some(now)),
    )
}

/// WHEN: Jeff lists inferred people (no `--sign`).
pub fn when_jeff_lists_inferred_people(world: &LinkWorld) -> CliOutcome {
    openlore_with(
        &world.env,
        &["infer", "people"],
        Invocation {
            offline: true,
            ..on_github(world, "", None)
        },
    )
}

/// WHEN: Jeff signs inferred candidate `n` (publishing), `infer people --sign n`.
pub fn when_jeff_signs_inferred(world: &LinkWorld, n: usize, now: &str) -> CliOutcome {
    let n = n.to_string();
    let stdin = sign_stdin("", true);
    openlore_with(
        &world.env,
        &["infer", "people", "--sign", &n],
        on_github(world, &stdin, Some(now)),
    )
}

/// WHEN: Jeff lists his imported candidates offline (`candidates list [filters]`).
pub fn when_jeff_lists_imported_candidates(world: &LinkWorld, filters: &[&str]) -> CliOutcome {
    let mut args = vec!["candidates", "list"];
    args.extend_from_slice(filters);
    openlore_with(
        &world.env,
        &args,
        Invocation {
            offline: true,
            ..on_github(world, "", None)
        },
    )
}

/// WHEN: Jeff reads every claim about `philosophy` (`graph query --object`),
/// with extra flags (`--weighted`, `--include-unattested`).
pub fn when_jeff_reads_philosophy(
    world: &LinkWorld,
    philosophy: &str,
    flags: &[&str],
    now: Option<&str>,
) -> CliOutcome {
    let mut args = vec!["graph", "query", "--object", philosophy];
    args.extend_from_slice(flags);
    openlore_with(
        &world.env,
        &args,
        Invocation {
            offline: true,
            ..on_github(world, "", now)
        },
    )
}

/// WHEN: Jeff reads every claim about `subject`, his own and his peers'
/// (`graph query --subject <s> --federated`), offline.
pub fn when_jeff_reads_subject(world: &LinkWorld, subject: Subject) -> CliOutcome {
    let uri = subject.uri();
    openlore_with(
        &world.env,
        &["graph", "query", "--subject", &uri, "--federated"],
        Invocation {
            offline: true,
            ..on_github(world, "", None)
        },
    )
}

/// WHEN: Jeff reads one contributor's claims, weighted (`graph query
/// --contributor <did> --weighted`), offline.
pub fn when_jeff_weighs_contributor(world: &LinkWorld, did: &str) -> CliOutcome {
    openlore_with(
        &world.env,
        &["graph", "query", "--contributor", did, "--weighted"],
        Invocation {
            offline: true,
            ..on_github(world, "", None)
        },
    )
}

/// Contributors of Rachel's repo kelp: Rachel alone.
pub fn rachel_contributes() -> Vec<FakeContributor> {
    vec![FakeContributor::human(RACHEL_GH.login, RACHEL_GH.id, 300)]
}

/// Contributors of a repo where Jeff (`jeffbailey`) and BurntSushi both commit.
pub fn jeff_and_burntsushi_contribute() -> Vec<FakeContributor> {
    vec![
        FakeContributor::human(BURNTSUSHI_GH.login, BURNTSUSHI_GH.id, 900),
        FakeContributor::human(JEFF_GH.login, JEFF_GH.id, 400),
    ]
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

/// THEN: the run succeeded and my PDS now holds a claim about `subject`
/// (published through the single publish path).
pub fn assert_signed_and_published(world: &LinkWorld, outcome: &CliOutcome, subject: Subject) {
    assert_eq!(
        outcome.status,
        0,
        "Jeff may sign about {};{}",
        subject.uri(),
        context(outcome)
    );
    assert!(
        pds_bodies(&world.env)
            .iter()
            .any(|(_, body)| body["subject"] == subject.uri().as_str()),
        "a claim about {} reached Jeff's PDS;{}",
        subject.uri(),
        context(outcome)
    );
}

/// THEN: signing was refused for `subject`, naming `reason`, who can sign
/// (`can_sign`) and — when given — the command to run next.
pub fn assert_refused(
    outcome: &CliOutcome,
    subject: Subject,
    reason: Refusal,
    can_sign: Option<CanSign>,
    next_command: Option<&str>,
    row: &str,
) {
    let text = all_output(outcome);
    assert!(
        text.contains(&subject.uri()),
        "[{row}] the refusal names {};{}",
        subject.uri(),
        context(outcome)
    );
    assert!(
        text.contains(reason.phrase()),
        "[{row}] the refusal says why: `{}`;{}",
        reason.phrase(),
        context(outcome)
    );
    if let Some(who) = can_sign {
        assert!(
            text.contains(CAN_BE_SIGNED_BY),
            "[{row}] the hand-off says `{CAN_BE_SIGNED_BY}`;{}",
            context(outcome)
        );
        for phrase in who.phrases() {
            assert!(
                text.contains(&phrase),
                "[{row}] the hand-off names `{phrase}`;{}",
                context(outcome)
            );
        }
    }
    if let Some(command) = next_command {
        assert!(
            text.contains(command),
            "[{row}] the refusal names the next command `{command}`;{}",
            context(outcome)
        );
    }
}

/// THEN: `claim add`-style refusal: non-zero exit plus [`assert_refused`].
pub fn assert_signing_refused(
    outcome: &CliOutcome,
    subject: Subject,
    reason: Refusal,
    can_sign: Option<CanSign>,
    next_command: Option<&str>,
    row: &str,
) {
    assert_ne!(
        outcome.status,
        0,
        "[{row}] a refused signature exits non-zero;{}",
        context(outcome)
    );
    assert_refused(outcome, subject, reason, can_sign, next_command, row);
}

/// THEN: GitHub was asked nothing since the first `since` requests.
pub fn assert_github_not_asked(world: &LinkWorld, since: usize, row: &str) {
    let asked = world.github_paths_since(since);
    assert!(
        asked.is_empty(),
        "[{row}] signing never asks GitHub (C-OOS-9); asked: {asked:#?}"
    );
}

/// The lines of `stdout` that show `author`'s claim about `subject`. Readers
/// print multi-line entries (`graph query --object`: a `subject:` header,
/// then one `author_did:` entry per author; `search`: an `author:` header,
/// then `subject:` and detail lines). An entry starts at a line naming
/// `author` and runs to the next blank line or the next `author…` line; it
/// is about `subject` when the entry, or the nearest `subject:` header above
/// it, names `subject`.
pub fn rows_about<'a>(stdout: &'a str, author: &str, subject: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = stdout.lines().collect();
    let names_subject = |l: &str| {
        l.split_whitespace()
            .any(|w| w.trim_end_matches([',', ';', ':']) == subject)
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].contains(author) {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < lines.len()
            && !lines[end].trim().is_empty()
            && !(lines[end].trim_start().starts_with("author") && !lines[end].contains(author))
        {
            end += 1;
        }
        let entry = &lines[i..end];
        let header_names = lines[..i]
            .iter()
            .rev()
            .find(|l| l.trim_start().starts_with("subject:"))
            .is_some_and(|l| names_subject(l));
        if entry.iter().any(|l| names_subject(l)) || header_names {
            out.extend_from_slice(entry);
        }
        i = end.max(i + 1);
    }
    out
}

/// THEN: the reader shows `author`'s claim about `subject` labelled `label`
/// (and no other label of the set).
pub fn assert_labelled(outcome: &CliOutcome, author: &str, subject: Subject, label: Authority) {
    let rows = rows_about(&outcome.stdout, author, &subject.uri());
    assert!(
        !rows.is_empty(),
        "[{}] {author}'s claim about {} is shown;{}",
        label.label(),
        subject.uri(),
        context(outcome)
    );
    let joined = rows.join("\n");
    assert!(
        joined.contains(label.label()),
        "[{}] {author}'s claim about {} reads `{}`:\n{joined}",
        label.label(),
        subject.uri(),
        label.label()
    );
    for other in [
        Authority::Owner,
        Authority::RepoListed,
        Authority::ThirdParty,
        Authority::NotChecked,
    ] {
        if other != label && !label.label().contains(other.label()) {
            assert!(
                !joined.contains(other.label()),
                "[{}] {author}'s claim about {} carries only its own label, not `{}`:\n{joined}",
                label.label(),
                subject.uri(),
                other.label()
            );
        }
    }
}

/// The number of the candidate whose headline names `subject` (`[n] <subject> …`).
pub fn candidate_number_naming(outcome: &CliOutcome, subject: Subject) -> usize {
    let uri = subject.uri();
    (1..=numbered_count(&outcome.stdout))
        .find(|n| {
            candidate_line(&outcome.stdout, *n)
                .is_some_and(|l| l.contains(&format!("{uri} ")) || l.ends_with(&uri))
        })
        .unwrap_or_else(|| panic!("a candidate about {uri} is listed;{}", context(outcome)))
}

/// THEN: the candidate block about `subject` says who can sign it.
pub fn assert_candidate_says_who_can_sign(
    outcome: &CliOutcome,
    subject: Subject,
    who: Option<CanSign>,
) {
    let n = candidate_number_naming(outcome, subject);
    let block = candidate_block(&outcome.stdout, n).unwrap_or_default();
    match who {
        None => assert!(
            block.contains(YOU_CAN_SIGN),
            "candidate {n} about {} says `{YOU_CAN_SIGN}`:\n{block}",
            subject.uri()
        ),
        Some(who) => {
            assert!(
                block.contains(CAN_BE_SIGNED_BY) && !block.contains(YOU_CAN_SIGN),
                "candidate {n} about {} says `{CAN_BE_SIGNED_BY}`:\n{block}",
                subject.uri()
            );
            for phrase in who.phrases() {
                assert!(
                    block.contains(&phrase),
                    "candidate {n} about {} names `{phrase}`:\n{block}",
                    subject.uri()
                );
            }
        }
    }
}

/// The scored pairings of a viewer `/score` page (GDL IR-15 selector): each
/// is a `<section>` carrying its `Weight:` line.
pub fn scored_pairings(body: &str) -> Vec<String> {
    body.split("<section>")
        .skip(1)
        .map(|rest| rest.split("</section>").next().unwrap_or(rest))
        .filter(|section| section.contains("<p>Weight: "))
        .map(str::to_string)
        .collect()
}

/// The weighted pairing rows (GDL `weighted_rows`) that name `subject`.
pub fn weighted_rows_about(stdout: &str, subject: Subject) -> Vec<String> {
    weighted_rows(stdout)
        .into_iter()
        .filter(|r| r.contains(&subject.uri()))
        .collect()
}

// =============================================================================
// What Jeff sees through his own commands (PRIMARY, ruling R1)
// =============================================================================

/// What Jeff can see about one subject through his own commands and his
/// published repo — the PRIMARY evidence for "nothing signed, published or
/// recorded" (D-e / R1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatJeffSees {
    /// `graph query --subject <s> --federated` (offline): every claim about
    /// the subject, his and his peers', with its label.
    pub claims_about: String,
    /// `identity list`: his links and what his latest check of each saw.
    pub identities: String,
    /// The wire JSON of every record his PDS holds.
    pub published: Vec<String>,
}

/// Look at `subject` the way Jeff would (read-only verbs, offline).
pub fn what_jeff_sees(world: &LinkWorld, subject: Subject) -> WhatJeffSees {
    let _first_federated_query_orientation_shown_once = when_jeff_reads_subject(world, subject);
    let claims = when_jeff_reads_subject(world, subject);
    let identities = when_i_list(world, None);
    WhatJeffSees {
        claims_about: claims.stdout,
        identities: identities.stdout,
        published: world
            .env
            .pds
            .records()
            .into_iter()
            .map(|r| r.body)
            .collect(),
    }
}

/// How many of Jeff's own claims about `subject` a reader lists (one `cid:`
/// line per claim entry).
fn jeffs_entries(stdout: &str, subject: Subject) -> usize {
    rows_about(stdout, MY_DID, &subject.uri())
        .iter()
        .filter(|l| l.trim_start().starts_with("cid:"))
        .count()
}

/// THEN (PRIMARY): Jeff sees exactly what he saw before — no claim of his
/// appeared, nothing reached his published repo, and no check he can see
/// was recorded.
pub fn assert_jeff_sees_no_change(
    row: &str,
    before: &WhatJeffSees,
    world: &LinkWorld,
    subject: Subject,
) {
    let after = what_jeff_sees(world, subject);
    assert_eq!(
        after.published, before.published,
        "[{row}] nothing reached Jeff's published repo"
    );
    assert_eq!(
        after.claims_about,
        before.claims_about,
        "[{row}] every claim about {} reads as before (nothing signed)",
        subject.uri()
    );
    assert_eq!(
        after.identities, before.identities,
        "[{row}] his linked identities and their checks read as before (nothing recorded)"
    );
}

/// THEN (PRIMARY) for import verbs, which may record observations: no claim
/// of Jeff's about `subject` appeared and nothing reached his published repo.
pub fn assert_nothing_signed_or_published(
    row: &str,
    before: &WhatJeffSees,
    world: &LinkWorld,
    subject: Subject,
) {
    let after = what_jeff_sees(world, subject);
    assert_eq!(
        after.published, before.published,
        "[{row}] nothing reached Jeff's published repo"
    );
    assert_eq!(
        jeffs_entries(&after.claims_about, subject),
        jeffs_entries(&before.claims_about, subject),
        "[{row}] no claim of Jeff's about {} appeared;\n{}",
        subject.uri(),
        after.claims_about
    );
}

/// THEN (PRIMARY): the claim was signed and kept locally — the run
/// succeeded, Jeff's claim about `subject` is now listed, and nothing
/// reached his published repo (the network is off).
pub fn assert_signed_and_kept_locally(
    row: &str,
    outcome: &CliOutcome,
    before: &WhatJeffSees,
    world: &LinkWorld,
    subject: Subject,
) {
    assert_eq!(
        outcome.status,
        0,
        "[{row}] Jeff may sign;{}",
        context(outcome)
    );
    let after = what_jeff_sees(world, subject);
    assert_eq!(
        jeffs_entries(&after.claims_about, subject),
        jeffs_entries(&before.claims_about, subject) + 1,
        "[{row}] Jeff's claim about {} is now listed;\n{}",
        subject.uri(),
        after.claims_about
    );
    assert_eq!(
        after.published, before.published,
        "[{row}] offline: nothing reached Jeff's published repo"
    );
}

/// THEN (PRIMARY, wire): the newest record on Jeff's PDS is his `kind`
/// (`counters` / `retracts`) of `target`, about `subject`.
pub fn assert_published_marking(
    world: &LinkWorld,
    published_before: usize,
    kind: &str,
    target: &str,
    subject: Subject,
    row: &str,
) {
    let bodies = pds_bodies(&world.env);
    assert_eq!(
        bodies.len(),
        published_before + 1,
        "[{row}] exactly one record reached Jeff's published repo: {bodies:?}"
    );
    let (_, record) = bodies
        .iter()
        .find(|(_, v)| {
            v["references"]
                .as_array()
                .is_some_and(|r| r.iter().any(|x| x["type"] == kind && x["cid"] == target))
        })
        .unwrap_or_else(|| panic!("[{row}] a record that {kind} {target}: {bodies:?}"));
    assert_eq!(
        record["subject"],
        subject.uri().as_str(),
        "[{row}] it is about {}: {record}",
        subject.uri()
    );
    assert_eq!(
        bare(record["author"].as_str().unwrap_or_default()),
        MY_DID,
        "[{row}] it is Jeff's: {record}"
    );
}

/// THEN: no output line names the GitHub token's value (scrape_auth no-leak
/// pattern, US-SCR-004).
pub fn assert_token_never_shown(outcome: &CliOutcome, token: &str) {
    assert_token_value_absent(outcome, token);
}

// =============================================================================
// Store universe (SECONDARY guardrail — Mandate-8 waiver, wave-decisions)
// =============================================================================

/// One recorded repo observation (timestamps and ids excluded).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepoCheckRow {
    pub subject_key: String,
    pub outcome: String,
    pub owner_type: Option<String>,
    pub authority_file: Option<String>,
    pub authority_dids: Vec<String>,
}

fn table_exists(conn: &duckdb::Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) > 0 FROM information_schema.tables WHERE table_name = ?",
        duckdb::params![table],
        |row| row.get(0),
    )
    .unwrap_or_else(|e| panic!("look up table {table}: {e}"))
}

/// GUARDRAIL read: every recorded repo observation, sorted (empty only when
/// the store or the v8 table does not exist yet; any other read failure
/// panics).
pub fn repo_checks(env: &TestEnv) -> Vec<RepoCheckRow> {
    let path = env.duckdb_path();
    if !path.exists() {
        return Vec::new();
    }
    let conn = duckdb::Connection::open(&path)
        .unwrap_or_else(|e| panic!("open DuckDB for repo authority checks: {e}"));
    if !table_exists(&conn, "repo_authority_checks") {
        return Vec::new();
    }
    let mut stmt = conn
        .prepare(
            "SELECT subject_key, outcome, owner_type, authority_file, \
             array_to_string(COALESCE(authority_dids, []), ' ') \
             FROM repo_authority_checks",
        )
        .unwrap_or_else(|e| panic!("prepare the repo authority checks read: {e}"));
    let mut rows: Vec<RepoCheckRow> = stmt
        .query_map([], |row| {
            let dids: Option<String> = row.get(4)?;
            Ok(RepoCheckRow {
                subject_key: row.get(0)?,
                outcome: row.get(1)?,
                owner_type: row.get(2)?,
                authority_file: row.get(3)?,
                authority_dids: dids
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
            })
        })
        .unwrap_or_else(|e| panic!("query repo authority checks: {e}"))
        .map(|r| r.unwrap_or_else(|e| panic!("decode repo authority check: {e}")))
        .collect();
    rows.sort();
    rows
}

/// GUARDRAIL read: number of imported candidates kept locally (0 before
/// slice 5's table exists).
pub fn imported_candidate_count(env: &TestEnv) -> i64 {
    let path = env.duckdb_path();
    if !path.exists() {
        return 0;
    }
    let conn = duckdb::Connection::open(&path).expect("open DuckDB");
    if !table_exists(&conn, "imported_candidates") {
        return 0;
    }
    conn.query_row("SELECT COUNT(*) FROM imported_candidates", [], |row| {
        row.get(0)
    })
    .unwrap_or_else(|e| panic!("count imported candidates: {e}"))
}

/// Slot: the repo authority observations Jeff's store holds.
pub const REPO_OBSERVATIONS: &str = "authority.repo_observations";
/// Slot: how many imported candidates Jeff's store keeps.
pub const IMPORTED_CANDIDATES: &str = "candidates.imported.count";

/// The authority universe: the identity-link universe + repo observations +
/// imported candidates.
pub fn authority_universe_slots() -> HashSet<String> {
    let mut slots = link_universe_slots();
    slots.insert(REPO_OBSERVATIONS.to_string());
    slots.insert(IMPORTED_CANDIDATES.to_string());
    slots
}

/// Capture the authority universe (SECONDARY guardrail).
pub fn capture_authority_universe(env: &TestEnv) -> HashMap<String, String> {
    let mut snapshot = capture_link_universe(env);
    snapshot.insert(
        REPO_OBSERVATIONS.to_string(),
        format!("{:?}", repo_checks(env)),
    );
    snapshot.insert(
        IMPORTED_CANDIDATES.to_string(),
        imported_candidate_count(env).to_string(),
    );
    snapshot
}

/// SECONDARY GUARDRAIL (fail-closed): nothing in the store universe
/// changed. Never the only evidence for a Then. `row` names the table row.
pub fn guardrail_store_unchanged(row: &str, before: &HashMap<String, String>, env: &TestEnv) {
    let after = capture_authority_universe(env);
    let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_state_delta(before, &after, &authority_universe_slots(), &Delta::new())
    }));
    if let Err(payload) = checked {
        let reason = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        panic!("[{row}] guardrail: {reason}");
    }
}

/// SECONDARY GUARDRAIL: exactly the named slots changed; every other slot
/// is unchanged.
pub fn guardrail_store_delta(
    before: &HashMap<String, String>,
    env: &TestEnv,
    changes: &[(&str, String)],
) {
    let after = capture_authority_universe(env);
    let expected = changes.iter().fold(Delta::new(), |d, (slot, value)| {
        d.with_slot(*slot, set_to(value.clone()))
    });
    assert_state_delta(before, &after, &authority_universe_slots(), &expected);
}

/// SECONDARY GUARDRAIL for import verbs (which may record observations):
/// no claim row, claim file or PDS record was added.
pub fn guardrail_nothing_signed(row: &str, before: &HashMap<String, String>, env: &TestEnv) {
    let after = capture_authority_universe(env);
    for slot in [
        "local.claims.row_count",
        "local.claim_files",
        "pds.records.len",
    ] {
        assert_eq!(
            before.get(slot),
            after.get(slot),
            "[{row}] guardrail: `{slot}` must not change"
        );
    }
}

/// The repo-observation slot value after `added` rows.
pub fn repo_checks_with(before: &[RepoCheckRow], added: &[RepoCheckRow]) -> String {
    let mut rows: Vec<RepoCheckRow> = before.iter().chain(added).cloned().collect();
    rows.sort();
    format!("{rows:?}")
}

impl RepoCheckRow {
    /// A found repo observation.
    pub fn found(repo: RepoRef, organization: bool, file: &str, dids: &[&str]) -> Self {
        Self {
            subject_key: repo.subject().to_ascii_lowercase(),
            outcome: "found".to_string(),
            owner_type: Some(if organization { "organization" } else { "user" }.to_string()),
            authority_file: Some(file.to_string()),
            authority_dids: dids.iter().map(|d| d.to_string()).collect(),
        }
    }
}
