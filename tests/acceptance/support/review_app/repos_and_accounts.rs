//! review-app-repos-and-accounts — the step vocabulary and domain types for
//! choosing repos, several GitHub accounts and repos you don't own (DISTILL
//! 2026-10-19, Quinn). Mandate-12 SSOT: every account, repo and page phrase a
//! scenario names is a typed value or a named constant here; scenario bodies
//! never spell a login, repo path or message inline.
//!
//! Chained on the shipped vocabulary (`given_signed_in` → `when_verifies_github`
//! → `when_scan_finishes`): `given_owner_verified` → `given_saved_choice` →
//! `given_scanned` → `given_imported`. Step bodies only drive the app's HTTP
//! driving port (through [`Browser`]) or read a driven-EXTERNAL double (the
//! owner's PDS, GitHub's ordered request log). They never open the app's store;
//! the migration scenarios observe only the files the update leaves BESIDE it
//! (`review-app.v1.<secs>.bak`, an operator-visible side effect, ADR-088 §5.2)
//! and what a fresh app shows when started on such a copy.
//!
//! ## Page contract (DISTILL-proposed; DELIVER owns the final markup)
//!
//! The wording pins come from the DISCUSS stories and journey mockups. The
//! markup pins below are the minimum the vocabulary reads; renaming one is a
//! one-line change here.
//!
//! | Surface | Pin |
//! |---|---|
//! | Choose repos | `GET /repos`; one `<form>` per verified account whose text names `github.com/<login>` and whose button is "Save choice"; each repo is an `<input type="checkbox">` whose `aria-label` names `<owner>/<name>` (NFR-RRA-5) |
//! | Repos you don't own | on `GET /repos`: a form with field `repo` and button "Add repo"; each imported repo has a form with a "Remove" button whose markup names `<owner>/<name>` |
//! | Accounts | `GET /github`: the add form keeps field `github_login` + "Verify"; each listed account has a "Remove" form whose markup names its login |
//! | Remove confirm | a page with "Remove" (confirm) and "Cancel" buttons |
//! | Imported card | an `<article>` with "Not yours to publish", only a "Decline" control |
//! | Account line | an owned card says "from github.com/<login>" |

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use openlore_test_support::{
    BlueskyAccount, FakeAtprotoNetwork, FakeGithubAccounts, GithubAccount, GithubRepo,
};

use super::app::ReviewApp;
use super::browser::{Browser, Page};
use super::domain::*;
use super::html::{self, Form};
use super::world::*;

// =============================================================================
// Domain constants (one source each, DESIGN §6.1)
// =============================================================================

/// The one total per-scan repo limit (D-RRA-16, `SCAN_REPO_CAP`).
pub const SCAN_REPO_CAP: usize = 10;
/// Up to 3 GitHub accounts per sign-in (D-RRA-15).
pub const MAX_GITHUB_ACCOUNTS: usize = 3;
/// Default ticks per account without a saved choice (`DEFAULT_PERSON_REPO_COUNT`).
pub const DEFAULT_TICKS: usize = 5;

// Wording pins (DISCUSS user-stories / journey mockups, verbatim).
pub const SAVE_CHOICE: &str = "Save choice";
pub const ADD_REPO: &str = "Add repo";
pub const REMOVE: &str = "Remove";
pub const CANCEL: &str = "Cancel";
pub const DECLINE: &str = "Decline";
pub const NOT_YOURS: &str = "Not yours to publish";
pub const REPOS_YOU_DONT_OWN: &str = "Repos you don't own";
pub const NEW_SINCE_CHOICE: &str = "new since your last choice";
pub const NOTHING_PUBLISHED: &str = "Nothing was published.";
pub const NOTHING_LOST: &str = "Nothing was lost.";
pub const NO_ACCOUNT_TO_RECHECK: &str = "No GitHub account to re-check";
pub const NOTHING_TO_SCAN: &str =
    "Choose at least one repo, or name a public repo, before scanning.";
pub const DAILY_LIMIT_REACHED: &str = "You have scanned 6 times today";
pub const IMPORT_FIELD: &str = "repo";
/// AC-001.5 wording pin (DISTILL: DISCUSS gives the content, not the sentence).
pub const LISTING_RATE_LIMITED: &str = "GitHub is rate-limiting the app, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes.";
pub const ACCOUNT_FIELD: &str = "github_login";

/// The scan summary line of an account whose bio check failed (journey mockup 4).
pub fn failed_check_summary(account: Account) -> String {
    format!(
        "Your DID is not in the bio of {}, so its repos were not read and its suggestions are hidden. Put your DID back, then verify.",
        account.page_name()
    )
}

/// "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
pub fn save_over_limit_message(total: usize) -> String {
    format!(
        "You can choose up to {SCAN_REPO_CAP} repos per scan in total. You have {total} chosen. Untick {} to save.",
        total - SCAN_REPO_CAP
    )
}

/// "You can choose up to 10 repos per scan in total. You have 10 chosen. Untick or remove 1 repo before adding another."
pub fn import_over_limit_message(total: usize) -> String {
    format!(
        "You can choose up to {SCAN_REPO_CAP} repos per scan in total. You have {total} chosen. Untick or remove {} repo before adding another.",
        total + 1 - SCAN_REPO_CAP
    )
}

// =============================================================================
// Domain types (Mandate-12: typed accounts and repos)
// =============================================================================

/// The GitHub accounts in the scenarios (personas' accounts + third parties).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Account {
    /// Priya's personal account (the one verified before this feature).
    Priyaraman,
    /// Priya's work account (US-RRA-003).
    PriyaTidal,
    /// Dmitri's account with 12 owned repos (limit scenarios).
    Dvolkov,
    DvolkovWork,
    DvolkovLab,
    DvolkovOld,
    /// Aisha: only forks.
    Aishab,
    /// Sam (adversarial): no repos.
    Samortega,
}

impl Account {
    pub fn login(self) -> &'static str {
        match self {
            Account::Priyaraman => "priyaraman",
            Account::PriyaTidal => "priya-tidal",
            Account::Dvolkov => "dvolkov",
            Account::DvolkovWork => "dvolkov-work",
            Account::DvolkovLab => "dvolkov-lab",
            Account::DvolkovOld => "dvolkov-old",
            Account::Aishab => "aishab",
            Account::Samortega => "samortega",
        }
    }

    /// The immutable numeric GitHub id (accounts are keyed by it, C-RRA-D3).
    pub fn id(self) -> u64 {
        match self {
            Account::Priyaraman => 4_210_001,
            Account::PriyaTidal => 4_210_077,
            Account::Dvolkov => 5_310_002,
            Account::DvolkovWork => 5_310_010,
            Account::DvolkovLab => 5_310_011,
            Account::DvolkovOld => 5_310_012,
            Account::Aishab => 6_410_003,
            Account::Samortega => 7_510_004,
        }
    }

    /// Whose DID this account's bio carries in the canonical world.
    pub fn owner(self) -> Persona {
        match self {
            Account::Priyaraman | Account::PriyaTidal => Persona::Priya,
            Account::Dvolkov | Account::DvolkovWork | Account::DvolkovLab | Account::DvolkovOld => {
                Persona::Dmitri
            }
            Account::Aishab => Persona::Aisha,
            Account::Samortega => Persona::Sam,
        }
    }

    /// `github.com/<login>` as every page names it.
    pub fn page_name(self) -> String {
        format!("github.com/{}", self.login())
    }
}

/// A public GitHub repo by `owner/name`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Repo {
    pub owner: &'static str,
    pub name: &'static str,
}

impl Repo {
    pub const fn new(owner: &'static str, name: &'static str) -> Self {
        Self { owner, name }
    }

    /// `priyaraman/tidepool`
    pub fn full(self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// `github:priyaraman/tidepool` (the claim subject).
    pub fn subject(self) -> String {
        format!("github:{}", self.full())
    }

    /// A suggestion about this repo (`<owner>/<name> embodies <slug>`).
    pub const fn embodies(self, philosophy: Philosophy) -> Suggestion {
        Suggestion::new(self.owner, self.name, philosophy)
    }
}

/// Every repo the scenarios name, by account (DISCUSS domain examples).
pub mod repos {
    use super::Repo;

    // priyaraman: 7 owned public repos ranked by stars, 1 fork, 1 archived.
    pub const TIDEPOOL: Repo = Repo::new("priyaraman", "tidepool");
    pub const QUILL_DOCS: Repo = Repo::new("priyaraman", "quill-docs");
    pub const KELP: Repo = Repo::new("priyaraman", "kelp");
    pub const HARBOR_CLI: Repo = Repo::new("priyaraman", "harbor-cli");
    pub const DRIFTWOOD: Repo = Repo::new("priyaraman", "driftwood");
    pub const FERROUS_NOTES: Repo = Repo::new("priyaraman", "ferrous-notes");
    pub const SANDBAR: Repo = Repo::new("priyaraman", "sandbar");
    pub const SERDE_FORK: Repo = Repo::new("priyaraman", "serde");
    pub const OLD_TIDES_ARCHIVED: Repo = Repo::new("priyaraman", "old-tides");
    /// Created after her choice was saved (US-RRA-002).
    pub const SANDPIPER: Repo = Repo::new("priyaraman", "sandpiper");

    /// Today's default top 5 (stars, then last push, then name).
    pub const PRIYA_TOP_5: [Repo; 5] = [TIDEPOOL, QUILL_DOCS, KELP, HARBOR_CLI, DRIFTWOOD];
    /// Every owned, non-fork, non-archived repo of priyaraman, ranked.
    pub const PRIYA_OWNED_RANKED: [Repo; 7] = [
        TIDEPOOL,
        QUILL_DOCS,
        KELP,
        HARBOR_CLI,
        DRIFTWOOD,
        FERROUS_NOTES,
        SANDBAR,
    ];
    /// Her own choice (US-RRA-001 "Priya saves her own choice").
    pub const PRIYA_CHOICE: [Repo; 5] = [TIDEPOOL, QUILL_DOCS, KELP, HARBOR_CLI, FERROUS_NOTES];
    /// Her choice after unticking quill-docs (US-RRA-002, BR-RRA-4).
    pub const PRIYA_CHOICE_WITHOUT_QUILL: [Repo; 4] = [TIDEPOOL, KELP, HARBOR_CLI, FERROUS_NOTES];

    // priya-tidal: 4 owned public repos.
    pub const INGEST_BENCH: Repo = Repo::new("priya-tidal", "ingest-bench");
    pub const TIDAL_SDK: Repo = Repo::new("priya-tidal", "tidal-sdk");
    pub const REEF_UI: Repo = Repo::new("priya-tidal", "reef-ui");
    pub const NOTES_SITE: Repo = Repo::new("priya-tidal", "notes-site");
    pub const TIDAL_RANKED: [Repo; 4] = [INGEST_BENCH, TIDAL_SDK, REEF_UI, NOTES_SITE];
    /// Imported by Priya, then owned by priya-tidal (BR-RRA-5, US-RRA-006).
    pub const TIDAL_SDK_NOTES: Repo = Repo::new("priya-tidal", "tidal-sdk-notes");

    // Third-party public repos (imports).
    pub const CARGO: Repo = Repo::new("rust-lang", "cargo");
    pub const CARGO_PLUGIN_DEMO: Repo = Repo::new("rust-lang", "cargo-plugin-demo");
    /// Same repo after its transfer to priya-tidal.
    pub const CARGO_PLUGIN_DEMO_MOVED: Repo = Repo::new("priya-tidal", "cargo-plugin-demo");
    pub const RIPGREP: Repo = Repo::new("BurntSushi", "ripgrep");
    pub const AISHAB_RIPGREP_FORK: Repo = Repo::new("aishab", "ripgrep");
    pub const STREAMLINE: Repo = Repo::new("tidal-labs", "streamline");
    /// Private: GitHub answers 404 (NFR-BRA-8).
    pub const SECRET_ROADMAP: Repo = Repo::new("tidal-labs", "secret-roadmap");
    /// kelp after Priya transferred it to tidal-labs (US-RRA-010).
    pub const KELP_AT_TIDAL_LABS: Repo = Repo::new("tidal-labs", "kelp");
}

/// Dmitri's 12 owned repos `dvolkov/forge-01..12`, ranked by stars.
pub fn dmitri_repos() -> Vec<Repo> {
    const NAMES: [&str; 12] = [
        "forge-01", "forge-02", "forge-03", "forge-04", "forge-05", "forge-06", "forge-07",
        "forge-08", "forge-09", "forge-10", "forge-11", "forge-12",
    ];
    NAMES.iter().map(|n| Repo::new("dvolkov", n)).collect()
}

// =============================================================================
// The world (canonical GitHub for this feature; the shared one stays untouched)
// =============================================================================

fn bio_with(persona: Persona) -> String {
    format!("Builder. {}", persona.did())
}

/// GitHub as the feature's domain examples describe it.
pub fn rra_github_fixtures() -> Vec<GithubAccount> {
    let priya = GithubAccount::new("priyaraman", Account::Priyaraman.id())
        .bio(&bio_with(Persona::Priya))
        .repo(
            GithubRepo::owned("tidepool")
                .stars(412)
                .language("Rust")
                .pushed_at("2026-10-11T00:00:00Z")
                .with_cargo_lock()
                .with_ci_and_tests()
                .with_semver_and_changelog(),
        )
        .repo(
            GithubRepo::owned("quill-docs")
                .stars(88)
                .pushed_at("2026-09-30T00:00:00Z")
                .with_docs(),
        )
        .repo(
            GithubRepo::owned("kelp")
                .stars(51)
                .pushed_at("2026-08-02T00:00:00Z")
                .with_cargo_lock(),
        )
        .repo(
            GithubRepo::owned("harbor-cli")
                .stars(37)
                .pushed_at("2026-10-02T00:00:00Z")
                .with_ci_and_tests(),
        )
        .repo(
            GithubRepo::owned("driftwood")
                .stars(12)
                .pushed_at("2025-03-14T00:00:00Z")
                .with_docs()
                .with_semver_and_changelog()
                .with_cargo_lock(),
        )
        .repo(
            GithubRepo::owned("ferrous-notes")
                .stars(3)
                .pushed_at("2026-10-17T00:00:00Z")
                .with_cargo_lock(),
        )
        .repo(
            GithubRepo::owned("sandbar")
                .stars(0)
                .pushed_at("2026-06-01T00:00:00Z")
                .with_ci_and_tests(),
        )
        .repo(GithubRepo::forked("serde").stars(900))
        .repo(GithubRepo::archived("old-tides").stars(700).with_docs());
    let tidal = GithubAccount::new("priya-tidal", Account::PriyaTidal.id())
        .bio(&bio_with(Persona::Priya))
        .repo(
            GithubRepo::owned("ingest-bench")
                .stars(30)
                .with_ci_and_tests(),
        )
        .repo(GithubRepo::owned("tidal-sdk").stars(25).with_docs())
        .repo(
            GithubRepo::owned("reef-ui")
                .stars(15)
                .with_semver_and_changelog(),
        )
        .repo(GithubRepo::owned("notes-site").stars(5).with_cargo_lock());
    let mut dmitri = GithubAccount::new("dvolkov", Account::Dvolkov.id())
        .bio(&format!("Systems hacker. {DMITRI_OLD_DID}"));
    for (rank, repo) in dmitri_repos().iter().enumerate() {
        dmitri = dmitri.repo(
            GithubRepo::owned(repo.name)
                .stars(200 - rank as u32 * 10)
                .with_cargo_lock(),
        );
    }
    let dmitri_extra = |account: Account, repo: &str| {
        GithubAccount::new(account.login(), account.id())
            .bio(&bio_with(Persona::Dmitri))
            .repo(GithubRepo::owned(repo).with_cargo_lock())
    };
    vec![
        priya,
        tidal,
        dmitri,
        dmitri_extra(Account::DvolkovWork, "work-bench"),
        dmitri_extra(Account::DvolkovLab, "lab-notes"),
        dmitri_extra(Account::DvolkovOld, "old-crate"),
        GithubAccount::new("aishab", Account::Aishab.id())
            .bio(&bio_with(Persona::Aisha))
            .repo(GithubRepo::forked("rustlings"))
            .repo(GithubRepo::forked("book"))
            .repo(GithubRepo::forked("ripgrep").with_cargo_lock()),
        GithubAccount::new("samortega", Account::Samortega.id()),
        GithubAccount::new(BURNTSUSHI, 456_674)
            .bio("I like Rust.")
            .repo(
                GithubRepo::owned("ripgrep")
                    .language("Rust")
                    .with_cargo_lock(),
            ),
        GithubAccount::new("rust-lang", 5_430_000)
            .organization()
            .repo(
                GithubRepo::owned("cargo")
                    .stars(12_000)
                    .with_cargo_lock()
                    .with_ci_and_tests()
                    .with_semver_and_changelog(),
            )
            .repo(GithubRepo::owned("cargo-plugin-demo").stars(40).with_docs()),
        GithubAccount::new("tidal-labs", 8_100_000)
            .organization()
            .repo(
                GithubRepo::owned("streamline")
                    .stars(60)
                    .with_semver_and_changelog(),
            ),
    ]
}

fn rra_bluesky_accounts() -> Vec<BlueskyAccount> {
    Persona::ALL
        .iter()
        .map(|p| BlueskyAccount::new(p.handle(), p.did(), p.pds_host()))
        .collect()
}

/// The canonical world for this feature: the REAL app against the feature's
/// GitHub and the shared ATProto network.
pub fn rra_world() -> ReviewWorld {
    let github = FakeGithubAccounts::start(rra_github_fixtures());
    let atproto = FakeAtprotoNetwork::start(rra_bluesky_accounts());
    let app = ReviewApp::start(settings_for(&github, &atproto));
    ReviewWorld {
        github,
        atproto,
        app,
    }
}

// =============================================================================
// The frozen v1 store (ADR-088 migration scenarios)
// =============================================================================

/// `tests/fixtures/review-app-store-v1/` — a `review-app.duckdb` written by
/// the shipped v1 app (see its README).
pub fn v1_store_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/review-app-store-v1")
        .canonicalize()
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/review-app-store-v1")
        })
}

/// The pending cards the v1 app showed Priya when the fixture was frozen.
pub fn v1_golden_pending() -> Vec<String> {
    let path = v1_store_fixture_dir().join("pending.txt");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// GIVEN the app's store was written by the release before this feature
/// (the frozen v1 file), and the new app starts on it.
pub fn rra_world_on_v1_store() -> ReviewWorld {
    let github = FakeGithubAccounts::start(rra_github_fixtures());
    let atproto = FakeAtprotoNetwork::start(rra_bluesky_accounts());
    let app = ReviewApp::start_on_store(
        settings_for(&github, &atproto),
        &v1_store_fixture_dir().join("store"),
    );
    ReviewWorld {
        github,
        atproto,
        app,
    }
}

/// The backups the migration left beside the store (`review-app.v1.<secs>.bak`).
pub fn store_backups(world: &ReviewWorld) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(world.app.data_dir())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("review-app.v1.") && n.ends_with(".bak"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// `review-app.v1.<unix-seconds>.bak` (ADR-088 §5.2 step 1).
pub fn is_v1_backup_name(name: &str) -> bool {
    name.strip_prefix("review-app.v1.")
        .and_then(|rest| rest.strip_suffix(".bak"))
        .is_some_and(|secs| !secs.is_empty() && secs.chars().all(|c| c.is_ascii_digit()))
}

/// The bytes of one backup beside the store.
pub fn backup_bytes(world: &ReviewWorld, name: &str) -> Vec<u8> {
    let path = world.app.data_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A world whose app starts on a COPY of one store file (e.g. a backup).
pub fn rra_world_on_store_file(file: &Path) -> ReviewWorld {
    let dir = tempfile::tempdir().expect("store copy dir");
    std::fs::copy(file, dir.path().join("review-app.duckdb"))
        .unwrap_or_else(|e| panic!("copy {}: {e}", file.display()));
    let github = FakeGithubAccounts::start(rra_github_fixtures());
    let atproto = FakeAtprotoNetwork::start(rra_bluesky_accounts());
    let app = ReviewApp::start_on_store(settings_for(&github, &atproto), dir.path());
    ReviewWorld {
        github,
        atproto,
        app,
    }
}

/// The development-build fault seam that stops the v1 → v2 update just
/// before it commits (DELIVER honours it in debug builds only).
pub const MIGRATION_FAULT: (&str, &str) = ("REVIEW_APP_TEST_FAULT", "migrate_before_commit");

/// The doubles of a world whose app is not running.
pub struct Doubles {
    pub github: FakeGithubAccounts,
    pub atproto: FakeAtprotoNetwork,
}

/// A first start on the v1 store that was cut off before the update committed.
pub struct CutOff {
    not_ready: super::app::NotReady,
    /// The copies of the old data the cut-off start left beside the store.
    pub backups: Vec<String>,
    /// The bytes of the first copy, as the cut-off start left it.
    pub first_copy_bytes: Vec<u8>,
}

impl CutOff {
    /// WHEN the app starts again, normally, on the same data.
    pub fn start_again(self, doubles: Doubles) -> ReviewWorld {
        let app = self
            .not_ready
            .start_again(settings_for(&doubles.github, &doubles.atproto));
        ReviewWorld {
            github: doubles.github,
            atproto: doubles.atproto,
            app,
        }
    }
}

/// GIVEN the new release's first start on the v1 store was cut off before
/// the update committed (the fault seam).
pub fn rra_world_on_v1_store_cut_off_mid_update() -> (Doubles, CutOff) {
    let github = FakeGithubAccounts::start(rra_github_fixtures());
    let atproto = FakeAtprotoNetwork::start(rra_bluesky_accounts());
    let mut settings = settings_for(&github, &atproto);
    settings
        .extra_env
        .push((MIGRATION_FAULT.0.to_string(), MIGRATION_FAULT.1.to_string()));
    let not_ready =
        match ReviewApp::launch_on_store(settings, &v1_store_fixture_dir().join("store")) {
            Err(not_ready) => not_ready,
            Ok(_) => panic!(
            "MISSING_FUNCTIONALITY: precondition: the update was not cut off — the app started \
             normally with {}={} (no v1 → v2 update / fault seam yet)",
            MIGRATION_FAULT.0, MIGRATION_FAULT.1
        ),
        };
    let data = not_ready.data_dir();
    let mut backups: Vec<String> = std::fs::read_dir(&data)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| is_v1_backup_name(n))
                .collect()
        })
        .unwrap_or_default();
    backups.sort();
    let first_copy_bytes = backups
        .first()
        .map(|b| std::fs::read(data.join(b)).expect("read the first copy"))
        .unwrap_or_default();
    (
        Doubles { github, atproto },
        CutOff {
            not_ready,
            backups,
            first_copy_bytes,
        },
    )
}

// =============================================================================
// GitHub-side actions the people take outside the app
// =============================================================================

/// The account's owner puts her signed-in DID into that account's bio.
pub fn puts_did_in_account_bio(world: &ReviewWorld, account: Account) {
    world
        .github
        .set_bio(account.login(), Some(&bio_with(account.owner())));
}

/// The account's owner removes her DID from that account's bio.
pub fn removes_did_from_account_bio(world: &ReviewWorld, account: Account) {
    world
        .github
        .set_bio(account.login(), Some("Rust, tide models."));
}

/// The owner creates a new public repo on an account.
pub fn creates_repo(world: &ReviewWorld, repo: Repo) {
    world.github.put_repo(
        repo.owner,
        GithubRepo::owned(repo.name).stars(1).with_docs(),
    );
}

// =============================================================================
// Accounts (US-RRA-003 / 008)
// =============================================================================

/// GIVEN `persona` is signed in and has verified each account, in order.
pub fn given_accounts_verified(world: &ReviewWorld, accounts: &[Account]) -> Browser {
    let persona = accounts
        .first()
        .map(|a| a.owner())
        .unwrap_or(Persona::Priya);
    for account in accounts {
        puts_did_in_account_bio(world, *account);
    }
    let mut browser = given_signed_in(world, persona);
    for account in accounts {
        when_adds_account(&mut browser, *account);
        then_sees_account_verified(&browser.page, *account, persona);
    }
    browser
}

/// GIVEN the owner of `account` is signed in with only that account verified.
pub fn given_owner_verified(world: &ReviewWorld, account: Account) -> Browser {
    given_accounts_verified(world, &[account])
}

/// WHEN the signed-in owner adds (and verifies) a GitHub username.
pub fn when_adds_account(browser: &mut Browser, account: Account) -> &Page {
    when_verifies_github(browser, account.login())
}

/// WHEN the signed-in owner adds a GitHub username that is not an [`Account`].
pub fn when_adds_login<'b>(browser: &'b mut Browser, login: &str) -> &'b Page {
    when_verifies_github(browser, login)
}

pub fn then_sees_account_verified(page: &Page, account: Account, persona: Persona) {
    let expected = format!(
        "Verified: {} belongs to {}",
        account.page_name(),
        persona.at_handle()
    );
    assert!(
        page.shows(&expected),
        "expected {expected:?}; page:\n{}",
        page.text()
    );
}

/// The account list as the GitHub step shows it: `(login, verified?)` in order.
pub fn account_list(browser: &mut Browser) -> Vec<(String, bool)> {
    let page = browser.open("/github");
    let mut rows = Vec::new();
    for form in html::elements(&page.html, "form") {
        let buttons = html::forms(form);
        let is_remove = buttons.iter().any(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, REMOVE))
        });
        if !is_remove {
            continue;
        }
        for account in ALL_ACCOUNTS {
            if form.contains(account.login()) && !rows.iter().any(|(l, _)| l == account.login()) {
                let verified = account_row_text(&page.html, account)
                    .map(|t| {
                        t.contains("verified")
                            && !t.contains("unverified")
                            && !t.contains("re-verify")
                    })
                    .unwrap_or(false);
                rows.push((account.login().to_string(), verified));
            }
        }
    }
    rows
}

const ALL_ACCOUNTS: [Account; 8] = [
    Account::Priyaraman,
    Account::PriyaTidal,
    Account::Dvolkov,
    Account::DvolkovWork,
    Account::DvolkovLab,
    Account::DvolkovOld,
    Account::Aishab,
    Account::Samortega,
];

/// The visible text of the account's row on the GitHub step (the smallest
/// `li`/`tr`/`article`/`div` naming `github.com/<login>`).
fn account_row_text(page_html: &str, account: Account) -> Option<String> {
    let name = account.page_name();
    ["li", "tr", "article", "div"]
        .iter()
        .flat_map(|tag| html::elements(page_html, tag))
        .map(html::visible_text)
        .filter(|t| t.contains(&name))
        .min_by_key(|t| t.len())
}

/// The form on the current page whose markup names `needle` and offers `label`.
fn form_naming(page: &Page, needle: &str, label: &str) -> Option<Form> {
    html::elements(&page.html, "form")
        .into_iter()
        .filter(|raw| raw.contains(needle))
        .flat_map(html::forms)
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, label))
        })
}

/// WHEN the owner presses "Remove" next to `needle` (an account login or an
/// imported `owner/name`) on `path`; returns the confirm page.
fn when_presses_remove_for<'b>(browser: &'b mut Browser, path: &str, needle: &str) -> &'b Page {
    browser.open(path);
    let form = form_naming(&browser.page, needle, REMOVE).unwrap_or_else(|| {
        panic!(
            "no \"{REMOVE}\" control for {needle} on {path}:\n{}",
            browser.page.text()
        )
    });
    browser.submit(&form, Some(REMOVE))
}

/// WHEN the owner presses "Remove" next to an account (the confirm opens).
pub fn when_presses_remove_account(browser: &mut Browser, account: Account) -> &Page {
    when_presses_remove_for(browser, "/github", account.login())
}

/// WHEN the owner confirms the removal on the confirm page.
pub fn when_confirms_removal(browser: &mut Browser) -> &Page {
    browser.press(REMOVE)
}

/// WHEN the owner cancels the removal on the confirm page.
pub fn when_cancels_removal(browser: &mut Browser) -> &Page {
    browser.press(CANCEL)
}

/// WHEN the owner removes an account and confirms.
pub fn when_removes_account(browser: &mut Browser, account: Account) -> &Page {
    when_presses_remove_account(browser, account);
    when_confirms_removal(browser)
}

// =============================================================================
// Choose repos (US-RRA-001 / 002)
// =============================================================================

/// WHEN the owner opens "Choose repos".
pub fn when_opens_choose_repos(browser: &mut Browser) -> &Page {
    browser.open("/repos")
}

/// The raw `<form>` of `account` on the choose-repos page.
fn account_form_html(page: &Page, account: Account) -> Option<String> {
    let name = account.page_name();
    html::elements(&page.html, "form")
        .into_iter()
        .find(|raw| {
            html::visible_text(raw).contains(&name)
                && html::forms(raw).iter().any(|f| {
                    f.buttons
                        .iter()
                        .any(|(l, _, _)| html::label_matches(l, SAVE_CHOICE))
                })
        })
        .map(str::to_string)
}

/// One repo box: `(owner/name from its label, field name, value, ticked)`.
fn repo_boxes(form_html: &str) -> Vec<(String, String, String, bool)> {
    html::opening_tags(form_html, "input")
        .into_iter()
        .filter(|(_, tag)| {
            html::attr(tag, "type").is_some_and(|t| t.eq_ignore_ascii_case("checkbox"))
        })
        .filter_map(|(_, tag)| {
            let label = html::attr(tag, "aria-label")?;
            let full = label
                .split_whitespace()
                .find(|w| w.contains('/'))
                .map(|w| {
                    w.trim_matches(|c: char| {
                        !c.is_alphanumeric() && c != '/' && c != '-' && c != '_' && c != '.'
                    })
                    .to_string()
                })
                .unwrap_or(label);
            Some((
                full,
                html::attr(tag, "name").unwrap_or_default(),
                html::attr(tag, "value").unwrap_or_default(),
                html::attr(tag, "checked").is_some(),
            ))
        })
        .collect()
}

/// The repos listed for `account` (`owner/name`, in page order).
pub fn listed_repos(page: &Page, account: Account) -> Vec<String> {
    account_form_html(page, account)
        .map(|f| repo_boxes(&f).into_iter().map(|b| b.0).collect())
        .unwrap_or_default()
}

/// The repos ticked for `account` (`owner/name`, in page order).
pub fn ticked_repos(page: &Page, account: Account) -> Vec<String> {
    account_form_html(page, account)
        .map(|f| {
            repo_boxes(&f)
                .into_iter()
                .filter(|b| b.3)
                .map(|b| b.0)
                .collect()
        })
        .unwrap_or_default()
}

/// The visible text of `account`'s section on the choose-repos page.
pub fn account_section_text(page: &Page, account: Account) -> String {
    account_form_html(page, account)
        .map(|f| html::visible_text(&f))
        .unwrap_or_default()
}

pub fn fulls(repos: &[Repo]) -> Vec<String> {
    repos.iter().map(|r| r.full()).collect()
}

/// WHEN the owner ticks exactly `ticked` on `account` and presses "Save choice".
pub fn when_saves_choice<'b>(
    browser: &'b mut Browser,
    account: Account,
    ticked: &[Repo],
) -> &'b Page {
    when_opens_choose_repos(browser);
    let raw = account_form_html(&browser.page, account).unwrap_or_else(|| {
        panic!(
            "no \"{SAVE_CHOICE}\" list for {} on the choose-repos page:\n{}",
            account.page_name(),
            browser.page.text()
        )
    });
    let boxes = repo_boxes(&raw);
    let form = html::forms(&raw)
        .into_iter()
        .next()
        .expect("the account's choose form");
    let box_names: HashSet<String> = boxes.iter().map(|b| b.1.clone()).collect();
    let mut fields: Vec<(String, String)> = form
        .fields
        .iter()
        .filter(|(k, _)| !box_names.contains(k))
        .cloned()
        .collect();
    let wanted: Vec<String> = fulls(ticked);
    for full in &wanted {
        let found = boxes
            .iter()
            .find(|b| b.0.eq_ignore_ascii_case(full))
            .unwrap_or_else(|| {
                panic!(
                    "{full} is not listed for {}:\n{}",
                    account.page_name(),
                    html::visible_text(&raw)
                )
            });
        fields.push((found.1.clone(), found.2.clone()));
    }
    let explicit = Form { fields, ..form };
    browser.submit(&explicit, Some(SAVE_CHOICE))
}

/// GIVEN the owner saved `ticked` as `account`'s choice (chains a verified account).
pub fn given_saved_choice(browser: &mut Browser, account: Account, ticked: &[Repo]) {
    when_saves_choice(browser, account, ticked);
    assert!(
        browser.page.shows("Saved."),
        "saving {}'s choice must succeed:\n{}",
        account.page_name(),
        browser.page.text()
    );
}

/// `true` when `repo`'s row on the choose page is marked "new since your last choice".
pub fn marked_new(page: &Page, account: Account, repo: Repo) -> bool {
    let Some(raw) = account_form_html(page, account) else {
        return false;
    };
    ["li", "tr", "label", "div"]
        .iter()
        .flat_map(|tag| html::elements(&raw, tag))
        .filter(|el| el.contains(&repo.full()))
        .map(html::visible_text)
        .min_by_key(|t| t.len())
        .is_some_and(|t| t.contains(NEW_SINCE_CHOICE))
}

// =============================================================================
// Imports (US-RRA-005 / 009)
// =============================================================================

/// WHEN the owner names a public repo she doesn't own ("Add repo").
pub fn when_imports<'b>(browser: &'b mut Browser, typed: &str) -> &'b Page {
    when_opens_choose_repos(browser);
    browser.fill(IMPORT_FIELD, typed);
    browser.press(ADD_REPO)
}

/// GIVEN the owner imported `repo` (chains a signed-in browser).
pub fn given_imported(browser: &mut Browser, repo: Repo) {
    when_imports(browser, &repo.full());
    assert!(
        imported_list(browser).contains(&repo.full()),
        "importing {} must list it under \"{REPOS_YOU_DONT_OWN}\":\n{}",
        repo.full(),
        browser.page.text()
    );
}

/// The repos under "Repos you don't own" (`owner/name`).
pub fn imported_list(browser: &mut Browser) -> Vec<String> {
    let page = browser.open("/repos");
    let mut found: Vec<String> = html::elements(&page.html, "form")
        .into_iter()
        .filter(|raw| {
            html::forms(raw).iter().any(|f| {
                f.buttons
                    .iter()
                    .any(|(l, _, _)| html::label_matches(l, REMOVE))
            })
        })
        .flat_map(|raw| {
            html::visible_text(raw)
                .split_whitespace()
                .chain(raw.split('"'))
                .filter(|w| w.contains('/') && !w.contains(' ') && !w.starts_with('/'))
                .map(|w| w.trim_matches(|c: char| c == '.' || c == ',').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|w| w.split('/').count() == 2 && !w.contains(':'))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// WHEN the owner removes an imported repo (no confirm for imports: the
/// story's "Remove" reports the deletion directly; a confirm page, if
/// DELIVER adds one, is confirmed here).
pub fn when_removes_import(browser: &mut Browser, repo: Repo) -> &Page {
    when_presses_remove_for(browser, "/repos", &repo.full());
    if browser.page.offers(CANCEL) && browser.page.offers(REMOVE) {
        browser.press(REMOVE);
    }
    &browser.page
}

// =============================================================================
// Scans (US-RRA-002 / 004)
// =============================================================================

/// A mark in GitHub's ordered request log; reads after it belong to what follows.
pub fn github_mark(world: &ReviewWorld) -> u64 {
    world.github.requests().last().map(|r| r.seq).unwrap_or(0)
}

/// The repos harvested (read past their root) after `mark`, as `owner/name`.
pub fn repos_read_since(world: &ReviewWorld, mark: u64) -> BTreeSet<String> {
    world
        .github
        .requests()
        .into_iter()
        .filter(|r| r.seq > mark)
        .filter_map(|r| {
            let segments: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
            match segments.as_slice() {
                ["repos", owner, name, _rest, ..] => Some(format!("{owner}/{name}")),
                _ => None,
            }
        })
        .collect()
}

pub fn set_of(repos: &[Repo]) -> BTreeSet<String> {
    repos.iter().map(|r| r.full()).collect()
}

/// WHEN the owner runs a scan to its end; returns (final status, repos read).
pub fn when_scans(world: &ReviewWorld, browser: &mut Browser) -> (String, BTreeSet<String>) {
    let mark = github_mark(world);
    let status = when_scan_finishes(browser);
    (status, repos_read_since(world, mark))
}

/// WHEN the owner presses "Scan again" (or "Scan my repos") without waiting.
pub fn when_presses_scan(browser: &mut Browser) -> &Page {
    browser.open("/review");
    let label = ["Scan again", "Scan my repos"]
        .into_iter()
        .find(|l| browser.page.offers(l))
        .unwrap_or_else(|| panic!("no scan action on the queue:\n{}", browser.page.text()));
    browser.press(label)
}

/// GIVEN the owner's latest scan completed (chains her setup).
pub fn given_scanned(world: &ReviewWorld, browser: &mut Browser) -> BTreeSet<String> {
    let (status, read) = when_scans(world, browser);
    assert_eq!(
        status,
        "completed",
        "the scan must complete:\n{}",
        browser.page.text()
    );
    read
}

/// GIVEN a scan paused for GitHub's budget part-way (BR-RRA-4: it is still
/// running — a resume continues the same scan). Returns the repos read so far.
///
/// The budget runs low only once the scan has read its first planned repo
/// (U-RRA-D8), so the pause lands mid-plan and a resume that re-planned
/// instead of reading the frozen plan's rest would read that repo again.
pub fn given_scan_paused_part_way(world: &ReviewWorld, browser: &mut Browser) -> BTreeSet<String> {
    world.github.budget_runs_low_after_repos(1, 120);
    let (status, read) = when_scans(world, browser);
    assert_eq!(
        status,
        "rate_limited",
        "the scan must pause for GitHub's budget:\n{}",
        browser.page.text()
    );
    assert!(
        !read.is_empty(),
        "precondition: the scan read at least one planned repo before it paused"
    );
    read
}

/// WHEN GitHub's budget refills and the paused scan resumes to its end;
/// returns the repos read by the resumed part.
pub fn when_paused_scan_resumes(world: &ReviewWorld, browser: &mut Browser) -> BTreeSet<String> {
    let mark = github_mark(world);
    world.github.set_remaining(4_990);
    let status = when_scan_finishes(browser);
    assert_eq!(status, "completed", "{}", browser.page.text());
    repos_read_since(world, mark)
}

/// The latest scan's status (`running`, `rate_limited`, `completed`, …).
pub fn scan_status(browser: &mut Browser) -> String {
    browser
        .open("/scan/status")
        .scan_status()
        .unwrap_or_else(|| "missing".into())
}

/// THEN, in the scan started after `scan_start` (a [`github_mark`]), no
/// account's owned repos were listed before that account's bio check
/// (U-RRA-D7; choose-page and Save listings happen outside the scan).
pub fn then_no_account_listed_before_its_bio_check(
    world: &ReviewWorld,
    scan_start: u64,
    accounts: &[Account],
) {
    for account in accounts {
        let early = world
            .github
            .listings_before_bio_check_since(account.login(), scan_start);
        assert!(
            early.is_empty(),
            "{} listed before its bio check in the scan (requests {early:?})",
            account.page_name()
        );
    }
}

/// The scan summary text on the queue.
pub fn scan_summary(browser: &mut Browser) -> String {
    browser.open("/review").text()
}

/// GIVEN the owner has used `n` of the day's scans (each one completes).
pub fn given_scans_used(world: &ReviewWorld, browser: &mut Browser, n: usize) {
    for _ in 0..n {
        given_scanned(world, browser);
    }
}

// =============================================================================
// The queue (US-RRA-004 / 006)
// =============================================================================

/// One card on the queue, as the owner sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub phrases: Vec<String>,
    pub text: String,
    pub controls: Vec<String>,
}

/// Every card on the queue (pending, owned or imported).
pub fn queue_cards(browser: &mut Browser) -> Vec<Card> {
    let page = browser.open("/review");
    html::elements(&page.html, "article")
        .into_iter()
        .map(|raw| {
            let text = html::visible_text(raw);
            let mut controls: Vec<String> = html::forms(raw)
                .iter()
                .flat_map(|f| f.buttons.iter().map(|(l, _, _)| l.clone()))
                .chain(html::links(raw).into_iter().map(|(l, _)| l))
                .collect();
            if raw.contains(r#"type="checkbox""#) {
                controls.push("Select".to_string());
            }
            Card {
                phrases: html::embodies_phrases(&text),
                text,
                controls,
            }
        })
        .filter(|c| !c.phrases.is_empty())
        .collect()
}

/// The cards about `repo`.
pub fn cards_about(browser: &mut Browser, repo: Repo) -> Vec<Card> {
    let prefix = format!("{} embodies", repo.full());
    queue_cards(browser)
        .into_iter()
        .filter(|c| c.phrases.iter().any(|p| p.starts_with(&prefix)))
        .collect()
}

/// The "imported" cards: private, "Not yours to publish".
pub fn not_yours_cards(browser: &mut Browser) -> Vec<Card> {
    queue_cards(browser)
        .into_iter()
        .filter(|c| c.text.contains(NOT_YOURS))
        .collect()
}

/// Phrases of every card (pending owned + imported) about `repo`.
pub fn phrases_about(browser: &mut Browser, repo: Repo) -> Vec<String> {
    let mut p: Vec<String> = cards_about(browser, repo)
        .into_iter()
        .flat_map(|c| c.phrases)
        .collect();
    p.sort();
    p.dedup();
    p
}

/// WHEN the owner declines a suggestion ("Not me" on owned cards, "Decline"
/// on imported ones — the same action, D-RRA-10).
pub fn when_declines_card(browser: &mut Browser, s: Suggestion) -> &Page {
    browser.open("/review");
    let parts = s.card_parts();
    let label = html::cards_containing(&browser.page.html, &[parts[0].as_str(), parts[1].as_str()])
        .first()
        .map(|card| {
            if card.contains(NOT_YOURS) {
                DECLINE
            } else {
                "Not me"
            }
        })
        .unwrap_or("Not me");
    browser.press_in_card(&[parts[0].as_str(), parts[1].as_str()], label)
}

/// GIVEN the owner published `s` through preview + confirm.
pub fn given_published_one(world: &ReviewWorld, browser: &mut Browser, s: Suggestion) {
    when_previews_approval(browser, s);
    when_confirms_publish(browser);
    assert!(
        claims_in_pds(world, Persona::Priya)
            .iter()
            .any(|r| r.value["subject"] == s.subject().as_str()),
        "publishing {} must write it:\n{}",
        s.phrase(),
        browser.page.text()
    );
}

/// The subjects of every claim record in `persona`'s PDS.
pub fn claim_subjects(world: &ReviewWorld, persona: Persona) -> Vec<String> {
    let mut s: Vec<String> = claims_in_pds(world, persona)
        .iter()
        .map(|r| r.value["subject"].as_str().unwrap_or("").to_string())
        .collect();
    s.sort();
    s
}

// =============================================================================
// Every publish request path (BR-RRA-1, the @property of US-RRA-006)
// =============================================================================

/// The request paths that can lead to a preview or a PDS write (ADR-089 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishRoute {
    Approve,
    ApproveSelected,
    EditPreview,
    Confirm,
    ConfirmSelected,
    Retry,
}

impl PublishRoute {
    pub const ALL: [PublishRoute; 6] = [
        PublishRoute::Approve,
        PublishRoute::ApproveSelected,
        PublishRoute::EditPreview,
        PublishRoute::Confirm,
        PublishRoute::ConfirmSelected,
        PublishRoute::Retry,
    ];
}

/// The hidden fields that name a suggestion in a card's forms.
fn key_fields(card_html: &str) -> Vec<(String, String)> {
    html::forms(card_html)
        .into_iter()
        .next()
        .map(|f| f.fields.into_iter().filter(|(k, _)| k != "csrf").collect())
        .unwrap_or_default()
}

/// `form` re-aimed from the `own` suggestion at the `target` one: every field
/// naming `own` names `target` instead (by field name, or by value inside a
/// composite value such as the selection box).
fn retarget(form: &Form, own: &[(String, String)], target: &[(String, String)]) -> Form {
    let mut out = form.clone();
    for (name, value) in out.fields.iter_mut() {
        if let Some((_, t)) = target.iter().find(|(k, _)| k == name) {
            *value = t.clone();
            continue;
        }
        for (k, own_value) in own {
            if let Some((_, t)) = target.iter().find(|(tk, _)| tk == k) {
                if !own_value.is_empty() && value.contains(own_value.as_str()) {
                    *value = value.replace(own_value.as_str(), t);
                }
            }
        }
    }
    out
}

fn card_html(browser: &mut Browser, s: Suggestion) -> String {
    let page = browser.open("/review");
    let parts = s.card_parts();
    html::cards_containing(&page.html, &[parts[0].as_str(), parts[1].as_str()])
        .first()
        .unwrap_or_else(|| panic!("no card for {} on the queue:\n{}", s.phrase(), page.text()))
        .to_string()
}

fn button_form(html_fragment: &str, label: &str) -> Form {
    html::forms(html_fragment)
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, label))
        })
        .unwrap_or_else(|| {
            panic!(
                "no \"{label}\" form in:\n{}",
                html::visible_text(html_fragment)
            )
        })
}

/// WHEN a request on `route` names `target`, sent without the page — the
/// forms are captured from the owner's own `carrier` suggestion and re-aimed
/// (an imported card offers none of them). `Retry` is reached through a
/// failed publish of `carrier`.
pub fn when_requests_publish_via(
    world: &ReviewWorld,
    browser: &mut Browser,
    route: PublishRoute,
    carrier: Suggestion,
    target: Suggestion,
) {
    let own_card = card_html(browser, carrier);
    let own = key_fields(&own_card);
    let target_card = card_html(browser, target);
    let target_key = key_fields(&target_card);
    match route {
        PublishRoute::Approve => {
            let form = retarget(&button_form(&own_card, "Approve"), &own, &target_key);
            browser.submit(&form, Some("Approve"));
        }
        PublishRoute::ApproveSelected => {
            browser.open("/review");
            let parts = carrier.card_parts();
            browser.tick_in_card(&[parts[0].as_str(), parts[1].as_str()]);
            let form = retarget(
                &button_form(&browser.page.html.clone(), "Approve selected"),
                &own,
                &target_key,
            );
            browser.submit(&form, Some("Approve selected"));
        }
        PublishRoute::EditPreview => {
            browser.open("/review");
            let parts = carrier.card_parts();
            browser.press_in_card(&[parts[0].as_str(), parts[1].as_str()], "Edit");
            let form = retarget(
                &button_form(&browser.page.html.clone(), "Preview"),
                &own,
                &target_key,
            );
            browser.submit(&form, Some("Preview"));
        }
        PublishRoute::Confirm => {
            when_previews_approval(browser, carrier);
            let form = retarget(
                &button_form(&browser.page.html.clone(), "Publish to my repo"),
                &own,
                &target_key,
            );
            browser.submit(&form, Some("Publish to my repo"));
        }
        PublishRoute::ConfirmSelected => {
            browser.open("/review");
            let parts = carrier.card_parts();
            browser.tick_in_card(&[parts[0].as_str(), parts[1].as_str()]);
            browser.press("Approve selected");
            let label = browser
                .forms()
                .iter()
                .flat_map(|f| f.buttons.iter().map(|(l, _, _)| l.clone()))
                .find(|l| l.starts_with("Publish"))
                .unwrap_or_else(|| "Publish 1 to my repo".to_string());
            let form = retarget(
                &button_form(&browser.page.html.clone(), &label),
                &own,
                &target_key,
            );
            browser.submit(&form, Some(&label));
        }
        PublishRoute::Retry => {
            when_previews_approval(browser, carrier);
            world
                .atproto
                .set_host_reachable(Persona::Priya.pds_host(), false);
            when_confirms_publish(browser);
            world
                .atproto
                .set_host_reachable(Persona::Priya.pds_host(), true);
            let form = retarget(
                &button_form(&browser.page.html.clone(), "Retry"),
                &own,
                &target_key,
            );
            browser.submit(&form, Some("Retry"));
        }
    }
}

// =============================================================================
// Universe (Mandate 8): port-exposed observable state
// =============================================================================

/// Slots: the owner's PDS claims and every write attempt; the queue (owned
/// pending and "Not yours to publish" cards); the account list; the imports.
pub fn rra_universe() -> HashSet<String> {
    [
        "pds.claims",
        "pds.write_attempts",
        "queue.pending",
        "queue.not_yours",
        "accounts",
        "imports",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Capture the universe through the owner's own pages and the PDS double.
pub fn capture_rra(
    world: &ReviewWorld,
    browser: &mut Browser,
    persona: Persona,
) -> HashMap<String, String> {
    let mut snapshot = HashMap::new();
    snapshot.insert(
        "pds.claims".into(),
        claim_subjects(world, persona).join("\n"),
    );
    snapshot.insert(
        "pds.write_attempts".into(),
        world.atproto.write_attempts().len().to_string(),
    );
    snapshot.insert("queue.pending".into(), pending_on_queue(browser).join("\n"));
    let mut not_yours: Vec<String> = not_yours_cards(browser)
        .into_iter()
        .flat_map(|c| c.phrases)
        .collect();
    not_yours.sort();
    snapshot.insert("queue.not_yours".into(), not_yours.join("\n"));
    snapshot.insert(
        "accounts".into(),
        account_list(browser)
            .iter()
            .map(|(l, v)| format!("{l}:{v}"))
            .collect::<Vec<_>>()
            .join(","),
    );
    snapshot.insert("imports".into(), imported_list(browser).join(","));
    snapshot
}

/// Today's aggregate KPI counters from the loopback admin listener.
pub fn kpi_counts(world: &ReviewWorld) -> serde_json::Value {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (status, body) = world
        .app
        .admin_get(&format!("/admin/kpi?from={today}&to={today}"));
    assert_eq!(status, 200, "{body}");
    serde_json::from_str(&body).expect("JSON sums")
}

pub fn kpi(world: &ReviewWorld, event: &str) -> i64 {
    kpi_counts(world)[event].as_i64().unwrap_or(0)
}
