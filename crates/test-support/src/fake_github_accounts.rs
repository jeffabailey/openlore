//! `FakeGithubAccounts` — a MULTI-account, MUTABLE double of the public GitHub
//! REST API for the bluesky-claim-review-app (DISTILL 2026-10-04).
//!
//! Why not a posture on [`crate::FakeGithub`]: that double is single-target and
//! constructor-pinned (DD-SCR-3), which is right for one `openlore scrape` run.
//! The hosted review app serves several people at once and re-reads a bio
//! before every scan (D-12), so the scenarios need (a) several accounts on one
//! server, (b) a bio that changes mid-scenario (remove / restore the DID),
//! (c) per-repo facts that change between scans (US-BRA-010), (d) a rate-limit
//! window that opens and closes, and (e) an ORDERED request log so the
//! "ownership re-checked before every scrape" invariant (I-BRA-4) is observable.
//!
//! The served shapes are byte-compatible with `FakeGithub` (the shapes
//! `adapter-github` already parses), plus the numeric `id` on `/users/{u}`
//! (component-boundaries §2: `PersonProfile.id`). Read-only: any non-GET is a
//! 405. The token the app sends is recorded (never echoed) so a no-leak check
//! can assert it never reaches logs.
//!
//! github-did-identity-link (DISTILL 2026-10-14) adds the ADR-085 lie
//! catalogue for proof checks: `GET /user/{id}` (the id read), a rename whose
//! old login 404s or redirects ([`OldLogin`]), a deleted account, a
//! re-registered login (a different id), and a profile served ~60 s stale
//! ([`FakeGithubAccounts::serve_stale_profile`] until
//! [`FakeGithubAccounts::end_stale_profile`]).

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::review_http::{
    header, json, json_with_headers, test_runtime, AbortOnDrop, HttpRequest, HttpResponse,
};

/// Public facts a repo exposes (each maps to one shipped detector).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoFacts {
    pub cargo_lock: bool,
    pub changelog: bool,
    pub tags: Vec<String>,
    pub docs_dir: bool,
    pub readme_bytes: Option<u64>,
    pub ci_workflows: bool,
    pub tests_dir: bool,
}

/// One repo listed by `GET /users/{u}/repos`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub name: String,
    pub stars: u32,
    pub language: Option<String>,
    pub fork: bool,
    pub archived: bool,
    pub pushed_at: String,
    pub facts: RepoFacts,
}

impl GithubRepo {
    /// An owned, non-fork, non-archived repo with no detectable facts.
    pub fn owned(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stars: 10,
            language: None,
            fork: false,
            archived: false,
            pushed_at: "2026-09-01T00:00:00Z".to_string(),
            facts: RepoFacts::default(),
        }
    }

    /// A fork (listed, but skipped by `select_person_repos`, BR-3).
    pub fn forked(name: &str) -> Self {
        Self {
            fork: true,
            ..Self::owned(name)
        }
    }

    /// An archived repo (skipped, BR-3).
    pub fn archived(name: &str) -> Self {
        Self {
            archived: true,
            ..Self::owned(name)
        }
    }

    pub fn stars(mut self, stars: u32) -> Self {
        self.stars = stars;
        self
    }

    pub fn language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }

    /// `Cargo.lock` committed → dependency-pinning.
    pub fn with_cargo_lock(mut self) -> Self {
        self.facts.cargo_lock = true;
        self
    }

    /// semver tags + `CHANGELOG.md` → semantic-versioning.
    pub fn with_semver_and_changelog(mut self) -> Self {
        self.facts.changelog = true;
        self.facts.tags = vec!["v1.0.0".into(), "v1.1.0".into(), "v1.2.0".into()];
        self
    }

    /// substantial README + `docs/` → documentation-first.
    pub fn with_docs(mut self) -> Self {
        self.facts.docs_dir = true;
        self.facts.readme_bytes = Some(12_000);
        self
    }

    /// `.github/workflows` + `tests/` → test-driven.
    pub fn with_ci_and_tests(mut self) -> Self {
        self.facts.ci_workflows = true;
        self.facts.tests_dir = true;
        self
    }
}

/// One public GitHub user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubAccount {
    pub login: String,
    pub id: u64,
    pub bio: Option<String>,
    pub repos: Vec<GithubRepo>,
}

impl GithubAccount {
    pub fn new(login: &str, id: u64) -> Self {
        Self {
            login: login.to_string(),
            id,
            bio: None,
            repos: Vec::new(),
        }
    }

    pub fn bio(mut self, bio: &str) -> Self {
        self.bio = Some(bio.to_string());
        self
    }

    pub fn repo(mut self, repo: GithubRepo) -> Self {
        self.repos.push(repo);
        self
    }
}

/// One request the app made, in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRequest {
    pub seq: u64,
    pub path: String,
}

impl GithubRequest {
    /// `GET /users/{login}` — a profile (bio) read.
    pub fn is_profile_read_of(&self, login: &str) -> bool {
        self.path.eq_ignore_ascii_case(&format!("/users/{login}"))
    }

    /// Any read of a repo or repo list belonging to `login` (a scrape).
    pub fn is_scrape_of(&self, login: &str) -> bool {
        let lower = self.path.to_ascii_lowercase();
        let login = login.to_ascii_lowercase();
        lower == format!("/users/{login}/repos") || lower.starts_with(&format!("/repos/{login}/"))
    }
}

/// What GitHub answers on the OLD login after a rename (ADR-085 §4, A-10 —
/// partly unverified, so both answers are modelled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OldLogin {
    /// `GET /users/{old}` → 404 (the check falls back to the id read).
    NotFound,
    /// `GET /users/{old}` → 301 to `/users/{new}` (same host).
    Redirects,
    /// `GET /users/{old}` → 301 to `http://elsewhere.invalid/users/{new}`
    /// (another host — the adapter must refuse it, ADR-085 §4).
    RedirectsToAnotherHost,
}

/// How a `200` profile answer breaks the account shape (review D1). Applies
/// to every profile read of the account, by login and by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFlaw {
    /// The body has no `id` field.
    NoId,
    /// `"id": 0` — no GitHub account has that number.
    ZeroId,
    /// `"id"` is a JSON string, not a number.
    IdAsText,
    /// The body has no `login` field.
    NoLogin,
}

impl ProfileFlaw {
    pub const ALL: [ProfileFlaw; 4] = [
        ProfileFlaw::NoId,
        ProfileFlaw::ZeroId,
        ProfileFlaw::IdAsText,
        ProfileFlaw::NoLogin,
    ];

    fn apply(self, profile: &mut serde_json::Value) {
        let Some(fields) = profile.as_object_mut() else {
            return;
        };
        match self {
            ProfileFlaw::NoId => {
                fields.remove("id");
            }
            ProfileFlaw::ZeroId => {
                fields.insert("id".to_string(), serde_json::json!(0));
            }
            ProfileFlaw::IdAsText => {
                let text = fields
                    .get("id")
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                fields.insert("id".to_string(), serde_json::Value::String(text));
            }
            ProfileFlaw::NoLogin => {
                fields.remove("login");
            }
        }
    }
}

#[derive(Default)]
struct GhState {
    accounts: Mutex<BTreeMap<String, GithubAccount>>,
    rate_limited_until: Mutex<Option<u64>>,
    remaining: AtomicU64,
    /// Seconds from now until the reported budget resets.
    reset_in: AtomicU64,
    token_rejected: AtomicBool,
    token_expiration: Mutex<Option<String>>,
    offline: AtomicBool,
    requests: Mutex<Vec<GithubRequest>>,
    seen_tokens: Mutex<Vec<String>>,
    seq: AtomicU64,
    /// Renamed logins that redirect: old (lowercase) → `Location` value.
    redirects: Mutex<BTreeMap<String, String>>,
    /// Accounts whose profile is served stale (lowercase login → stale bio).
    stale_bio: Mutex<BTreeMap<String, Option<String>>>,
    /// Accounts whose profile is served with a broken shape (lowercase login).
    flawed: Mutex<BTreeMap<String, ProfileFlaw>>,
}

/// The multi-account GitHub double, served on loopback.
pub struct FakeGithubAccounts {
    state: Arc<GhState>,
    base_url: String,
    runtime: Option<tokio::runtime::Runtime>,
    _task: Option<AbortOnDrop>,
}

impl std::fmt::Debug for FakeGithubAccounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeGithubAccounts")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl FakeGithubAccounts {
    pub fn start(accounts: Vec<GithubAccount>) -> Self {
        let runtime = test_runtime("fake-gh-accounts-rt");
        let state = Arc::new(GhState::default());
        state.remaining.store(4_990, Ordering::SeqCst);
        state.reset_in.store(3_600, Ordering::SeqCst);
        *state.accounts.lock().unwrap() = accounts
            .into_iter()
            .map(|a| (a.login.to_ascii_lowercase(), a))
            .collect();
        let (listener, base_url) = runtime.block_on(async {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("FakeGithubAccounts: bind");
            let addr = l.local_addr().expect("local_addr");
            (l, format!("http://{addr}"))
        });
        let st = state.clone();
        let task = runtime.spawn(async move {
            use hyper::server::conn::http1;
            use hyper_util::rt::TokioIo;
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(io) => io,
                    Err(_) => return,
                };
                if st.offline.load(Ordering::SeqCst) {
                    drop(stream);
                    continue;
                }
                let st2 = st.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req| {
                        let st3 = st2.clone();
                        async move { Ok::<_, Infallible>(route(&st3, req)) }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                });
            }
        });
        Self {
            state,
            base_url,
            runtime: Some(runtime),
            _task: Some(AbortOnDrop(task)),
        }
    }

    /// The base URL fed to `adapter-github` via `OPENLORE_GITHUB_API_BASE`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // -------------------------------------------------------------- mutation

    /// Replace (or clear, with `None`) a user's bio — the user edits GitHub.
    pub fn set_bio(&self, login: &str, bio: Option<&str>) {
        let mut accounts = self.state.accounts.lock().unwrap();
        let account = accounts
            .get_mut(&login.to_ascii_lowercase())
            .unwrap_or_else(|| panic!("FakeGithubAccounts: unknown login {login}"));
        account.bio = bio.map(str::to_string);
    }

    /// Add (or replace by name) a repo — the user pushes new work.
    pub fn put_repo(&self, login: &str, repo: GithubRepo) {
        let mut accounts = self.state.accounts.lock().unwrap();
        let account = accounts
            .get_mut(&login.to_ascii_lowercase())
            .unwrap_or_else(|| panic!("FakeGithubAccounts: unknown login {login}"));
        account.repos.retain(|r| r.name != repo.name);
        account.repos.push(repo);
    }

    /// Give the login a different numeric id IN PLACE (the review app reads
    /// this as `IdentityChanged`). For an identity-link check, model a rename
    /// with [`Self::rename`] and a re-registration with [`Self::register`].
    pub fn set_user_id(&self, login: &str, id: u64) {
        let mut accounts = self.state.accounts.lock().unwrap();
        if let Some(a) = accounts.get_mut(&login.to_ascii_lowercase()) {
            a.id = id;
        }
    }

    /// GitHub rate-limits every request for `seconds` from now.
    pub fn rate_limit_for(&self, seconds: u64) {
        *self.state.rate_limited_until.lock().unwrap() = Some(now_secs() + seconds);
    }

    /// The rate-limit window has passed.
    pub fn clear_rate_limit(&self) {
        *self.state.rate_limited_until.lock().unwrap() = None;
    }

    /// Every response reports the budget resetting `secs` from now
    /// (`x-ratelimit-reset`; 3600 unless set).
    pub fn set_reset_in(&self, secs: u64) {
        self.state.reset_in.store(secs, Ordering::SeqCst);
    }

    /// The `x-ratelimit-remaining` value every response reports.
    pub fn set_remaining(&self, remaining: u64) {
        self.state.remaining.store(remaining, Ordering::SeqCst);
    }

    /// Every request is refused with `401 Bad credentials` (expired/revoked PAT).
    pub fn reject_token(&self, rejected: bool) {
        self.state.token_rejected.store(rejected, Ordering::SeqCst);
    }

    /// Serve `github-authentication-token-expiration: <value>` on responses.
    pub fn set_token_expiration(&self, value: Option<&str>) {
        *self.state.token_expiration.lock().unwrap() = value.map(str::to_string);
    }

    /// Drop every connection (GitHub unreachable).
    pub fn set_offline(&self, offline: bool) {
        self.state.offline.store(offline, Ordering::SeqCst);
    }

    /// The account `old` renames itself to `new` (same numeric id, same bio).
    /// `GET /user/{id}` serves the new login; the old login 404s or redirects.
    pub fn rename(&self, old: &str, new: &str, old_login: OldLogin) {
        let mut accounts = self.state.accounts.lock().unwrap();
        let mut account = accounts
            .remove(&old.to_ascii_lowercase())
            .unwrap_or_else(|| panic!("FakeGithubAccounts: unknown login {old}"));
        account.login = new.to_string();
        accounts.insert(new.to_ascii_lowercase(), account);
        let location = match old_login {
            OldLogin::NotFound => None,
            OldLogin::Redirects => Some(format!("/users/{new}")),
            OldLogin::RedirectsToAnotherHost => {
                Some(format!("http://elsewhere.invalid/users/{new}"))
            }
        };
        if let Some(location) = location {
            self.state
                .redirects
                .lock()
                .unwrap()
                .insert(old.to_ascii_lowercase(), location);
        }
    }

    /// The account is deleted: its login and its id both 404 from now on.
    pub fn delete_account(&self, login: &str) {
        self.state
            .accounts
            .lock()
            .unwrap()
            .remove(&login.to_ascii_lowercase());
    }

    /// A new account appears — e.g. someone re-registers a freed login under
    /// a DIFFERENT numeric id. Takes precedence over any rename redirect.
    pub fn register(&self, account: GithubAccount) {
        let login = account.login.to_ascii_lowercase();
        self.state.redirects.lock().unwrap().remove(&login);
        self.state.accounts.lock().unwrap().insert(login, account);
    }

    /// GitHub's ~60 s profile cache (ADR-076 §6): every profile read of
    /// `login` (by login or by id) serves `stale_bio` instead of the current
    /// bio until [`Self::end_stale_profile`].
    pub fn serve_stale_profile(&self, login: &str, stale_bio: Option<&str>) {
        self.state
            .stale_bio
            .lock()
            .unwrap()
            .insert(login.to_ascii_lowercase(), stale_bio.map(str::to_string));
    }

    /// The cache window has passed: the current bio is served again.
    pub fn end_stale_profile(&self, login: &str) {
        self.state
            .stale_bio
            .lock()
            .unwrap()
            .remove(&login.to_ascii_lowercase());
    }

    /// Every profile read of `login` (by login or by id) answers `200` with
    /// `flaw` applied to the body, until the account is renamed or deleted.
    pub fn serve_flawed_profile(&self, login: &str, flaw: ProfileFlaw) {
        self.state
            .flawed
            .lock()
            .unwrap()
            .insert(login.to_ascii_lowercase(), flaw);
    }

    // ---------------------------------------------------------- observations

    /// Every request, in order.
    pub fn requests(&self) -> Vec<GithubRequest> {
        self.state.requests.lock().unwrap().clone()
    }

    /// Every requested path, in order (`/users/{login}`, `/user/{id}`, …).
    pub fn request_paths(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.path).collect()
    }

    /// `true` iff the app sent exactly this token value.
    pub fn saw_token(&self, token: &str) -> bool {
        self.state
            .seen_tokens
            .lock()
            .unwrap()
            .iter()
            .any(|t| t == token)
    }

    /// Number of scrape reads (repo list or per-repo reads) of `login`.
    pub fn scrape_reads_of(&self, login: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.is_scrape_of(login))
            .count()
    }

    /// I-BRA-4 oracle: every maximal run of scrape reads of `login` is
    /// immediately preceded (no other scrape of `login` in between) by a
    /// profile read of `login`. Returns the offending request sequence
    /// numbers (empty = invariant holds).
    pub fn scrapes_without_preceding_bio_check(&self, login: &str) -> Vec<u64> {
        let mut offending = Vec::new();
        let mut checked_since_last_scan = false;
        let mut in_scan = false;
        for r in self.requests() {
            if r.is_profile_read_of(login) {
                checked_since_last_scan = true;
                in_scan = false;
            } else if r.is_scrape_of(login) {
                if r.path.to_ascii_lowercase().ends_with("/repos") && r.path.starts_with("/users/")
                {
                    // a repo listing starts a scan
                    if !checked_since_last_scan {
                        offending.push(r.seq);
                    }
                    checked_since_last_scan = false;
                    in_scan = true;
                } else if !in_scan {
                    offending.push(r.seq);
                }
            }
        }
        offending
    }
}

impl Drop for FakeGithubAccounts {
    fn drop(&mut self) {
        self._task.take();
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}

// =============================================================================
// Routing (shapes mirror `fake_github.rs`)
// =============================================================================

fn route(state: &GhState, req: HttpRequest) -> HttpResponse {
    let path = req.uri().path().to_string();
    let seq = state.seq.fetch_add(1, Ordering::SeqCst) + 1;
    state.requests.lock().unwrap().push(GithubRequest {
        seq,
        path: path.clone(),
    });
    if let Some(auth) = header(&req, "authorization") {
        for prefix in ["token ", "Bearer ", "bearer "] {
            if let Some(t) = auth.strip_prefix(prefix) {
                state.seen_tokens.lock().unwrap().push(t.to_string());
            }
        }
    }

    let remaining = state.remaining.load(Ordering::SeqCst);
    let mut headers: Vec<(&str, String)> = vec![
        ("x-ratelimit-limit", "5000".to_string()),
        ("x-ratelimit-remaining", remaining.to_string()),
        (
            "x-ratelimit-reset",
            (now_secs() + state.reset_in.load(Ordering::SeqCst)).to_string(),
        ),
    ];
    if let Some(exp) = state.token_expiration.lock().unwrap().clone() {
        headers.push(("github-authentication-token-expiration", exp));
    }

    if state.token_rejected.load(Ordering::SeqCst) {
        return json_with_headers(
            401,
            &serde_json::json!({"message": "Bad credentials", "documentation_url": "https://docs.github.com/rest"}),
            &headers,
        );
    }
    if req.method() != hyper::Method::GET {
        return json(
            405,
            serde_json::json!({"message": "FakeGithubAccounts is read-only"}),
        );
    }
    if let Some(until) = *state.rate_limited_until.lock().unwrap() {
        if now_secs() < until {
            return json_with_headers(
                403,
                &serde_json::json!({
                    "message": "API rate limit exceeded",
                    "documentation_url": "https://docs.github.com/rest/overview/rate-limits"
                }),
                &[
                    ("x-ratelimit-limit", "5000".to_string()),
                    ("x-ratelimit-remaining", "0".to_string()),
                    ("x-ratelimit-reset", until.to_string()),
                    ("retry-after", (until - now_secs()).to_string()),
                ],
            );
        }
    }

    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let accounts = state.accounts.lock().unwrap();
    let not_found =
        || json_with_headers(404, &serde_json::json!({"message": "Not Found"}), &headers);
    match segments.as_slice() {
        ["users", login] => match accounts.get(&login.to_ascii_lowercase()) {
            Some(a) => json_with_headers(200, &served_profile(state, a), &headers),
            None => match state
                .redirects
                .lock()
                .unwrap()
                .get(&login.to_ascii_lowercase())
            {
                Some(location) => {
                    let mut moved = headers.clone();
                    moved.push(("location", location.clone()));
                    json_with_headers(
                        301,
                        &serde_json::json!({"message": "Moved Permanently"}),
                        &moved,
                    )
                }
                None => not_found(),
            },
        },
        // The id read (ADR-085 §4): the current profile of the account with
        // this numeric id, whatever its login is now; 404 once deleted.
        ["user", id] => match accounts.values().find(|a| a.id.to_string() == *id) {
            Some(a) => json_with_headers(200, &served_profile(state, a), &headers),
            None => not_found(),
        },
        ["users", login, "repos"] => match accounts.get(&login.to_ascii_lowercase()) {
            Some(a) => json_with_headers(
                200,
                &serde_json::Value::Array(a.repos.iter().map(|r| repo_row(a, r)).collect()),
                &headers,
            ),
            None => not_found(),
        },
        ["repos", owner, name, rest @ ..] => {
            let Some(account) = accounts.get(&owner.to_ascii_lowercase()) else {
                return not_found();
            };
            let Some(repo) = account
                .repos
                .iter()
                .find(|r| r.name.eq_ignore_ascii_case(name))
            else {
                return not_found();
            };
            let (body, status) = repo_endpoint(account, repo, rest);
            json_with_headers(status, &body, &headers)
        }
        _ => not_found(),
    }
}

/// The profile as served now: the account, with its stale bio while a cache window is open.
fn served_profile(state: &GhState, a: &GithubAccount) -> serde_json::Value {
    let stale = state
        .stale_bio
        .lock()
        .unwrap()
        .get(&a.login.to_ascii_lowercase())
        .cloned();
    let mut profile = match stale {
        Some(bio) => profile_json(&GithubAccount { bio, ..a.clone() }),
        None => profile_json(a),
    };
    if let Some(flaw) = state
        .flawed
        .lock()
        .unwrap()
        .get(&a.login.to_ascii_lowercase())
    {
        flaw.apply(&mut profile);
    }
    profile
}

fn profile_json(a: &GithubAccount) -> serde_json::Value {
    serde_json::json!({
        "login": a.login,
        "id": a.id,
        "name": serde_json::Value::Null,
        "bio": a.bio,
        "company": serde_json::Value::Null,
        "location": serde_json::Value::Null,
        "blog": "",
        "followers": 0,
        "following": 0,
        "public_repos": a.repos.len(),
        "created_at": "2015-01-01T00:00:00Z",
        "html_url": format!("https://github.com/{}", a.login),
        "type": "User",
    })
}

fn repo_row(a: &GithubAccount, r: &GithubRepo) -> serde_json::Value {
    serde_json::json!({
        "name": r.name,
        "full_name": format!("{}/{}", a.login, r.name),
        "description": serde_json::Value::Null,
        "language": r.language,
        "stargazers_count": r.stars,
        "fork": r.fork,
        "archived": r.archived,
        "pushed_at": r.pushed_at,
        "html_url": format!("https://github.com/{}/{}", a.login, r.name),
    })
}

fn repo_endpoint(a: &GithubAccount, r: &GithubRepo, rest: &[&str]) -> (serde_json::Value, u16) {
    let blob = |file: &str| {
        format!(
            "https://github.com/{}/{}/blob/master/{file}",
            a.login, r.name
        )
    };
    let tree = |dir: &str| {
        format!(
            "https://github.com/{}/{}/tree/master/{dir}",
            a.login, r.name
        )
    };
    match rest {
        [] => (
            serde_json::json!({
                "target": {"kind": "repo", "owner": a.login, "repo": r.name, "full_name": format!("{}/{}", a.login, r.name), "private": false},
                "language": r.language,
                "html_url": format!("https://github.com/{}/{}", a.login, r.name),
                "auth": {"authenticated": true, "rate_remaining": 4990, "rate_limit": 5000},
            }),
            200,
        ),
        ["tags"] => (
            serde_json::Value::Array(
                r.facts
                    .tags
                    .iter()
                    .map(|t| serde_json::json!({"name": t}))
                    .collect(),
            ),
            200,
        ),
        ["contributors"] => (serde_json::json!([]), 200),
        ["readme"] => match r.facts.readme_bytes {
            Some(size) => (
                serde_json::json!({"name": "README.md", "path": "README.md", "type": "file", "size": size, "html_url": blob("README.md")}),
                200,
            ),
            None => (serde_json::json!({"message": "Not Found"}), 404),
        },
        ["contents", file @ ..] => {
            let file = file.join("/");
            let present = match file.as_str() {
                "Cargo.lock" => r.facts.cargo_lock,
                "CHANGELOG.md" => r.facts.changelog,
                "docs" => r.facts.docs_dir,
                ".github/workflows" => r.facts.ci_workflows,
                "tests" => r.facts.tests_dir,
                _ => false,
            };
            if !present {
                return (serde_json::json!({"message": "Not Found"}), 404);
            }
            if matches!(file.as_str(), "docs" | ".github/workflows" | "tests") {
                let entry = format!("{file}/index.md");
                (
                    serde_json::json!([{"name": "index.md", "path": entry, "type": "file", "html_url": blob(&entry), "_dir_html_url": tree(&file)}]),
                    200,
                )
            } else {
                (
                    serde_json::json!({"name": file, "path": file, "type": "file", "html_url": blob(&file)}),
                    200,
                )
            }
        }
        _ => (serde_json::json!({"message": "Not Found"}), 404),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(fake: &FakeGithubAccounts, path: &str) -> (u16, String) {
        fake.runtime
            .as_ref()
            .unwrap()
            .block_on(crate::review_http::http_get(&format!(
                "{}{path}",
                fake.base_url()
            )))
            .expect("GET")
    }

    fn priya() -> GithubAccount {
        GithubAccount::new("priyaraman", 4_210_001)
            .bio("Rust, tide models. did:plc:7x3kq2mzv5rj4w6hbn2tqclp")
            .repo(
                GithubRepo::owned("tidepool")
                    .language("Rust")
                    .with_cargo_lock(),
            )
            .repo(GithubRepo::forked("serde"))
    }

    #[test]
    fn profile_carries_numeric_id_and_the_current_bio() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        let (status, body) = get(&gh, "/users/priyaraman");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["id"], 4_210_001);
        assert!(v["bio"]
            .as_str()
            .unwrap()
            .contains("did:plc:7x3kq2mzv5rj4w6hbn2tqclp"));

        gh.set_bio("priyaraman", Some("Rust, tide models."));
        let (_, body) = get(&gh, "/users/priyaraman");
        assert!(!body.contains("did:plc"), "the edited bio is served");
    }

    #[test]
    fn repo_facts_are_per_repo_and_absent_files_are_404() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        assert_eq!(
            get(&gh, "/repos/priyaraman/tidepool/contents/Cargo.lock").0,
            200
        );
        assert_eq!(
            get(&gh, "/repos/priyaraman/tidepool/contents/CHANGELOG.md").0,
            404
        );
        assert_eq!(get(&gh, "/repos/priyaraman/nope").0, 404);
    }

    #[test]
    fn rate_limit_window_opens_and_closes() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        gh.rate_limit_for(240);
        assert_eq!(get(&gh, "/users/priyaraman").0, 403);
        gh.clear_rate_limit();
        assert_eq!(get(&gh, "/users/priyaraman").0, 200);
    }

    #[test]
    fn the_id_read_follows_a_rename_and_the_old_login_404s() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        gh.rename("priyaraman", "priya-r", OldLogin::NotFound);
        assert_eq!(get(&gh, "/users/priyaraman").0, 404);
        let (status, body) = get(&gh, "/user/4210001");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["login"], "priya-r");
        assert_eq!(gh.request_paths(), ["/users/priyaraman", "/user/4210001"]);
    }

    #[test]
    fn a_redirecting_rename_and_a_re_registration_are_distinguishable() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        gh.rename("priyaraman", "priya-r", OldLogin::Redirects);
        assert_eq!(get(&gh, "/users/priyaraman").0, 301);
        gh.register(GithubAccount::new("priyaraman", 9_000_001));
        let (status, body) = get(&gh, "/users/priyaraman");
        assert_eq!(status, 200);
        assert!(body.contains("9000001"), "the new holder of the login");
        gh.delete_account("priya-r");
        assert_eq!(get(&gh, "/user/4210001").0, 404);
    }

    #[test]
    fn chained_and_cross_host_redirects_are_served_as_301s() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        gh.rename("priyaraman", "priya-r", OldLogin::Redirects);
        gh.rename("priya-r", "priya-z", OldLogin::Redirects);
        assert_eq!(get(&gh, "/users/priyaraman").0, 301);
        assert_eq!(get(&gh, "/users/priya-r").0, 301);
        assert_eq!(get(&gh, "/users/priya-z").0, 200);
        gh.rename("priya-z", "priya-x", OldLogin::RedirectsToAnotherHost);
        assert_eq!(get(&gh, "/users/priya-z").0, 301);
    }

    #[test]
    fn a_stale_profile_is_served_until_the_window_ends() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        gh.set_bio("priyaraman", Some("Rust, tide models."));
        gh.serve_stale_profile("priyaraman", Some("did:plc:7x3kq2mzv5rj4w6hbn2tqclp"));
        assert!(get(&gh, "/users/priyaraman").1.contains("did:plc:"));
        assert!(get(&gh, "/user/4210001").1.contains("did:plc:"));
        gh.end_stale_profile("priyaraman");
        assert!(!get(&gh, "/users/priyaraman").1.contains("did:plc:"));
    }

    #[test]
    fn a_flawed_profile_breaks_the_account_shape_on_both_reads() {
        for flaw in ProfileFlaw::ALL {
            let gh = FakeGithubAccounts::start(vec![priya()]);
            gh.serve_flawed_profile("priyaraman", flaw);
            for path in ["/users/priyaraman", "/user/4210001"] {
                let (status, body) = get(&gh, path);
                assert_eq!(status, 200, "[{flaw:?}] {path}");
                let v: serde_json::Value = serde_json::from_str(&body).unwrap();
                let shape_broken = match flaw {
                    ProfileFlaw::NoId => v.get("id").is_none(),
                    ProfileFlaw::ZeroId => v["id"] == 0,
                    ProfileFlaw::IdAsText => v["id"] == "4210001",
                    ProfileFlaw::NoLogin => v.get("login").is_none(),
                };
                assert!(shape_broken, "[{flaw:?}] {path}: {v}");
                assert!(
                    v["bio"].as_str().unwrap().contains("did:plc:"),
                    "[{flaw:?}] bio kept"
                );
            }
        }
    }

    #[test]
    fn scrape_without_a_preceding_bio_check_is_detected() {
        let gh = FakeGithubAccounts::start(vec![priya()]);
        get(&gh, "/users/priyaraman");
        get(&gh, "/users/priyaraman/repos");
        get(&gh, "/repos/priyaraman/tidepool");
        assert!(gh
            .scrapes_without_preceding_bio_check("priyaraman")
            .is_empty());
        get(&gh, "/users/priyaraman/repos");
        assert_eq!(
            gh.scrapes_without_preceding_bio_check("priyaraman").len(),
            1
        );
    }
}
