//! The `openlore-review-app` process (the THIRD composition root, ADR-072),
//! started exactly as production starts it — `openlore-review-app serve` with
//! its env configuration and a secrets directory — but with every external
//! system pointed at the hermetic doubles.
//!
//! ## Configuration contract (DISTILL-proposed; DELIVER owns the final names)
//!
//! Production names come from DEVOPS `infrastructure-integration.md` §4.1
//! (`APP_ORIGIN`, `REVIEW_DB`, `SECRETS_DIR`, `LOG_FORMAT`, `OAUTH_SCOPES`).
//! The test seams below are the minimum DELIVER must honour; renaming one is a
//! one-line change in [`ReviewApp::command`]:
//!
//! | Env | Meaning |
//! |---|---|
//! | `LISTEN_ADDR` / `ADMIN_LISTEN_ADDR` | public and loopback-admin listeners |
//! | `OPENLORE_GITHUB_API_BASE` | the EXISTING `adapter-github` seam |
//! | `REVIEW_APP_PLC_URL` | PLC directory base |
//! | `REVIEW_APP_HANDLE_RESOLVER_URL` | `com.atproto.identity.resolveHandle` base |
//! | `REVIEW_APP_ALLOW_LOOPBACK_HTTP=1` | accept `http://127.0.0.1` origin + upstreams (test builds only; mirrors the `test-autoconfirm` feature discipline) |
//! | `REVIEW_APP_CLOCK_SKEW_SECS` | run the clock ahead (development builds only; release refuses) |
//! | `REVIEW_APP_TEST_FAULT=migrate_before_commit` | review-app-repos-and-accounts MG-3: stop the v1 → v2 update just before COMMIT and exit (development builds only; a release build must refuse to start with it — DELIVER adds it to the xtask release-build guard) |
//! | `REVIEW_APP_TEST_FAULT=refuse_mark_repo_read` / `=refuse_record_account_checks` | the store refuses that one scan write every time it is called, everything else behaving normally (development builds only; a release build refuses to start with it). [`ReviewApp::restart_refusing`] is "her store refuses that write from now on" and [`ReviewApp::restart_without_test_fault`] is "the store accepts those writes again" |
//!
//! Launches retry while the app loses the port race (it must know `APP_ORIGIN`
//! before it starts, so it cannot bind `:0`): fresh ports on a first start; on
//! [`ReviewApp::restart`] the SAME ports after a pause (the origin, and with it
//! the OAuth client id and cookies, must not change).
//!
//! Secrets files (DV-BRA-4): `client-jwk` (from `openlore-review-app
//! gen-client-jwk`, a DELIVER deliverable), `data-key`, `github-token`,
//! `log-salt`.
//!
//! [`show_queue_evidence`] reads back, in the owner's own words, what her
//! queue says at one moment of a scenario: a scenario that asserts what she
//! sees also shows it.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::browser::Browser;
use super::html;

/// The development-build fault seam: which fault the app is asked to hold.
pub const TEST_FAULT_VAR: &str = "REVIEW_APP_TEST_FAULT";

/// The fault that makes her store refuse to save that a planned repo was
/// read, every time a scan saves it.
pub const REFUSE_MARK_REPO_READ: &str = "refuse_mark_repo_read";

/// The fault that lets her store save the FIRST read a run saves and refuse
/// every read that run saves after it, so a run saves a read of its own and
/// is then cut short.
pub const REFUSE_MARK_REPO_READ_AFTER_FIRST: &str = "refuse_mark_repo_read_after_first";

/// The granular scope set (ADR-073 / SPIKE-3).
pub const OAUTH_SCOPES: &str =
    "atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create";

/// A fixed 32-byte data key (hex) — test only.
/// The documented `{"kid","key"}` data key (base64 of bytes 0..=31).
const DATA_KEY_JSON: &str = r#"{"kid":"t1","key":"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="}"#;
const LOG_SALT: &str = "review-app-acceptance-log-salt";

/// Where the app's upstreams live and how it is configured.
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub github_base: String,
    pub plc_url: String,
    pub handle_resolver_url: String,
    pub github_token: String,
    /// Secrets file names to leave OUT (startup-refusal scenarios).
    pub omit_secrets: Vec<&'static str>,
    /// Extra env (e.g. a pinned clock).
    pub extra_env: Vec<(String, String)>,
}

/// The outcome of launching the app.
pub enum Startup {
    Ready(Box<ReviewApp>),
    Refused {
        exit_code: Option<i32>,
        logs: String,
    },
}

/// A running review app.
pub struct ReviewApp {
    child: Child,
    origin: String,
    admin_origin: String,
    settings: AppSettings,
    state: TempDir,
    run: u32,
}

/// Path of the `openlore-review-app` binary, or a RED-classified panic when the
/// composition root has not been built yet.
pub fn review_app_binary() -> PathBuf {
    // `CARGO_BIN_EXE_*` is set when these suites live in the app's own crate;
    // otherwise the binary sits in target/<profile>, above this test binary:
    // next to `deps/` under cargo's classic layout, further up under the newer
    // build-dir layout (target/<profile>/build/<pkg>/<hash>/...). Walk up to the
    // first directory that holds it.
    // (`assert_cmd::cargo_bin` would panic before we could classify the miss.)
    let name = format!("openlore-review-app{}", std::env::consts::EXE_SUFFIX);
    let bin = std::env::var_os("CARGO_BIN_EXE_openlore-review-app")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let exe = std::env::current_exe().expect("current test executable");
            let deps = exe.parent().expect("deps dir");
            deps.ancestors()
                .map(|dir| dir.join(&name))
                .find(|candidate| candidate.is_file())
                .unwrap_or_else(|| deps.parent().unwrap_or(deps).join(&name))
        });
    if !bin.exists() {
        panic!(
            "MISSING_FUNCTIONALITY: the review app is not available — the \
             `openlore-review-app` composition root (ADR-072) has not been built \
             (expected at {}). DELIVER bootstraps crates/openlore-review-app.",
            bin.display()
        );
    }
    bin
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("reserve a loopback port")
}

/// Two distinct free loopback ports (public, admin).
fn free_ports() -> (u16, u16) {
    loop {
        let (port, admin) = (free_port(), free_port());
        if port != admin {
            return (port, admin);
        }
    }
}

/// How many times a launch is retried when the app could not listen (the
/// reserved port was taken between `free_port` and the app's own bind — the
/// app must know its origin before it starts, so it cannot bind `:0`).
const BIND_ATTEMPTS: u32 = 6;

/// The app refused because a listener port was taken (wiring.rs `bind`).
fn lost_port_race(logs: &str) -> bool {
    logs.contains("cannot listen on")
}

/// A launch that did not become ready, with its state handed back (the
/// store stays on disk, so a later start can open it again).
pub struct NotReady {
    pub exit_code: Option<i32>,
    pub logs: String,
    state: TempDir,
}

impl NotReady {
    /// The data directory the refused start left on disk.
    pub fn data_dir(&self) -> PathBuf {
        self.state.path().join("data")
    }

    /// Launch the app again on the same state, handing the state back if it
    /// is refused again (e.g. a crash-looping start).
    pub fn launch_again(self, settings: AppSettings) -> Result<ReviewApp, NotReady> {
        let (port, admin_port) = free_ports();
        ReviewApp::try_launch_in(settings, self.state, port, admin_port, 2)
    }

    /// Start the app again on the same state (same store) with `settings`.
    pub fn start_again(self, settings: AppSettings) -> ReviewApp {
        match ReviewApp::launch_retrying(settings, self.state, None, 2) {
            Startup::Ready(app) => *app,
            Startup::Refused { exit_code, logs } => panic!(
                "MISSING_FUNCTIONALITY: the review app refused to start again on the same store (exit {exit_code:?}).\n--- logs ---\n{logs}"
            ),
        }
    }
}

impl ReviewApp {
    /// Launch with fresh state; wait for readiness or report the refusal.
    pub fn launch(settings: AppSettings) -> Startup {
        let state = tempfile::tempdir().expect("review-app state dir");
        Self::launch_retrying(settings, state, None, 1)
    }

    /// Launch, retrying while the app loses the port race: fresh ports when
    /// `fixed` is `None`, the same ports (after a pause) on a restart.
    fn launch_retrying(
        settings: AppSettings,
        state: TempDir,
        fixed: Option<(u16, u16)>,
        run: u32,
    ) -> Startup {
        let mut state = state;
        for attempt in 1..=BIND_ATTEMPTS {
            let (port, admin_port) = fixed.unwrap_or_else(free_ports);
            match Self::try_launch_in(settings.clone(), state, port, admin_port, run) {
                Ok(app) => return Startup::Ready(Box::new(app)),
                Err(not_ready) if attempt < BIND_ATTEMPTS && lost_port_race(&not_ready.logs) => {
                    state = not_ready.state;
                    std::thread::sleep(Duration::from_millis(250));
                }
                Err(NotReady {
                    exit_code, logs, ..
                }) => return Startup::Refused { exit_code, logs },
            }
        }
        unreachable!("the last attempt returns")
    }

    /// review-app-repos-and-accounts (ADR-088 migration scenarios): launch on
    /// a COPY of an existing store — every file in `store_dir` (the
    /// `review-app.duckdb` and any `.wal` beside it) is copied into the data
    /// directory first — and require readiness.
    pub fn start_on_store(settings: AppSettings, store_dir: &Path) -> Self {
        match Self::launch_on_store(settings, store_dir) {
            Ok(app) => app,
            Err(NotReady {
                exit_code, logs, ..
            }) => panic!(
                "MISSING_FUNCTIONALITY: the review app refused to start on the existing store (exit {exit_code:?}).\n--- logs ---\n{logs}"
            ),
        }
    }

    /// Launch on a COPY of an existing store, handing the state back when the
    /// app does not become ready (the MG-3 interrupted-upgrade scenario).
    pub fn launch_on_store(settings: AppSettings, store_dir: &Path) -> Result<Self, NotReady> {
        let state = tempfile::tempdir().expect("review-app state dir");
        let data = state.path().join("data");
        fs::create_dir_all(&data).expect("data dir");
        let entries = fs::read_dir(store_dir)
            .unwrap_or_else(|e| panic!("read store fixture {}: {e}", store_dir.display()));
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_file() {
                fs::copy(entry.path(), data.join(entry.file_name()))
                    .unwrap_or_else(|e| panic!("copy {}: {e}", entry.path().display()));
            }
        }
        let mut state = state;
        for attempt in 1..=BIND_ATTEMPTS {
            let (port, admin_port) = free_ports();
            match Self::try_launch_in(settings.clone(), state, port, admin_port, 1) {
                Ok(app) => return Ok(app),
                Err(not_ready) if attempt < BIND_ATTEMPTS && lost_port_race(&not_ready.logs) => {
                    state = not_ready.state;
                }
                Err(not_ready) => return Err(not_ready),
            }
        }
        unreachable!("the last attempt returns")
    }

    /// Stop the app and hand back its state directory (the store stays on
    /// disk for a fixture regenerator to copy).
    pub fn stop_keeping_state(mut self) -> TempDir {
        let _ = self.child.kill();
        let _ = self.child.wait();
        std::mem::replace(
            &mut self.state,
            tempfile::tempdir().expect("placeholder dir"),
        )
    }

    /// The directory holding `review-app.duckdb` (and any migration backup).
    pub fn data_dir(&self) -> PathBuf {
        self.state.path().join("data")
    }

    /// Launch and require readiness.
    pub fn start(settings: AppSettings) -> Self {
        match Self::launch(settings) {
            Startup::Ready(app) => *app,
            Startup::Refused { exit_code, logs } => panic!(
                "MISSING_FUNCTIONALITY: the review app refused to start (exit {exit_code:?}).\n--- logs ---\n{logs}"
            ),
        }
    }

    fn try_launch_in(
        settings: AppSettings,
        state: TempDir,
        port: u16,
        admin_port: u16,
        run: u32,
    ) -> Result<ReviewApp, NotReady> {
        let bin = review_app_binary();
        let secrets = state.path().join("secrets");
        let data = state.path().join("data");
        fs::create_dir_all(&secrets).expect("secrets dir");
        fs::create_dir_all(&data).expect("data dir");
        write_secrets(&bin, &secrets, &settings);

        let origin = format!("http://127.0.0.1:{port}");
        let admin_origin = format!("http://127.0.0.1:{admin_port}");
        let stdout =
            fs::File::create(state.path().join(format!("app-{run}.out.log"))).expect("log file");
        let stderr =
            fs::File::create(state.path().join(format!("app-{run}.err.log"))).expect("log file");
        let mut cmd = Self::command(&bin, &settings, &origin, port, admin_port, &data, &secrets);
        let child = cmd
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()));
        let mut app = ReviewApp {
            child,
            origin,
            admin_origin,
            settings,
            state,
            run,
        };
        match app.wait_ready(Duration::from_secs(45)) {
            Ok(()) => Ok(app),
            Err(exit_code) => {
                let logs = app.logs();
                let _ = app.child.kill();
                let _ = app.child.wait();
                let state = std::mem::replace(
                    &mut app.state,
                    tempfile::tempdir().expect("placeholder dir"),
                );
                Err(NotReady {
                    exit_code,
                    logs,
                    state,
                })
            }
        }
    }

    fn command(
        bin: &Path,
        settings: &AppSettings,
        origin: &str,
        port: u16,
        admin_port: u16,
        data: &Path,
        secrets: &Path,
    ) -> Command {
        let mut cmd = Command::new(bin);
        cmd.arg("serve")
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("APP_ORIGIN", origin)
            .env("LISTEN_ADDR", format!("127.0.0.1:{port}"))
            .env("ADMIN_LISTEN_ADDR", format!("127.0.0.1:{admin_port}"))
            .env("REVIEW_DB", data.join("review-app.duckdb"))
            .env("SECRETS_DIR", secrets)
            .env("LOG_FORMAT", "json")
            .env("OAUTH_SCOPES", OAUTH_SCOPES)
            .env("OPENLORE_GITHUB_API_BASE", &settings.github_base)
            .env("REVIEW_APP_PLC_URL", &settings.plc_url)
            .env(
                "REVIEW_APP_HANDLE_RESOLVER_URL",
                &settings.handle_resolver_url,
            )
            .env("REVIEW_APP_ALLOW_LOOPBACK_HTTP", "1");
        for (k, v) in &settings.extra_env {
            cmd.env(k, v);
        }
        cmd
    }

    fn wait_ready(&mut self, budget: Duration) -> Result<(), Option<i32>> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("client");
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(status.code());
            }
            if let Ok(resp) = client.get(format!("{}/healthz", self.origin)).send() {
                if resp.status().is_success() {
                    // A foreign listener on the reserved port could answer
                    // /healthz while our app is refusing (port race): ready
                    // only if our own process is still running a moment later.
                    std::thread::sleep(Duration::from_millis(150));
                    match self.child.try_wait() {
                        Ok(None) => return Ok(()),
                        Ok(Some(status)) => return Err(status.code()),
                        Err(_) => return Err(None),
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        Err(None)
    }

    /// Restart the same app with its clock `secs` ahead (the development
    /// build's `REVIEW_APP_CLOCK_SKEW_SECS` seam), as if that much time passed.
    pub fn restart_later(mut self, secs: u64) -> Self {
        self.settings
            .extra_env
            .retain(|(name, _)| name != "REVIEW_APP_CLOCK_SKEW_SECS");
        self.settings
            .extra_env
            .push(("REVIEW_APP_CLOCK_SKEW_SECS".to_string(), secs.to_string()));
        self.restart()
    }

    /// Restart the same app (same origin, store and secrets) holding no
    /// development-build test fault: the store accepts again every write it
    /// was refusing, and whatever the refused run left behind is still there.
    pub fn restart_without_test_fault(mut self) -> Self {
        self.settings
            .extra_env
            .retain(|(name, _)| name != TEST_FAULT_VAR);
        self.restart()
    }

    /// Restart the same app (same origin, store and secrets) holding `fault`
    /// on its development-build fault seam: the store now refuses the write
    /// that fault names instead of whatever it was refusing, and whatever the
    /// earlier run left behind is still there.
    pub fn restart_refusing(mut self, fault: &str) -> Self {
        self.settings
            .extra_env
            .retain(|(name, _)| name != TEST_FAULT_VAR);
        self.settings
            .extra_env
            .push((TEST_FAULT_VAR.to_string(), fault.to_string()));
        self.restart()
    }

    /// Stop and start the same app (same origin, database and secrets). If
    /// the old process's ports are not free yet (or another process took
    /// one), the start is retried on the SAME ports after a pause, up to
    /// `BIND_ATTEMPTS` times, because the origin must not change.
    pub fn restart(mut self) -> Self {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let state = std::mem::replace(
            &mut self.state,
            tempfile::tempdir().expect("placeholder dir"),
        );
        let settings = self.settings.clone();
        let run = self.run;
        let port = self
            .origin
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .expect("port");
        let admin_port = self
            .admin_origin
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .expect("port");
        drop(self);
        match Self::launch_retrying(settings, state, Some((port, admin_port)), run + 1) {
            Startup::Ready(app) => *app,
            Startup::Refused { exit_code, logs } => {
                panic!(
                    "the review app did not come back after a restart (exit {exit_code:?})\n{logs}"
                )
            }
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The URL Bluesky fetches to identify the app (the OAuth `client_id`).
    pub fn client_metadata_url(&self) -> String {
        format!("{}/oauth/client-metadata.json", self.origin)
    }

    /// Everything the app has logged so far (stdout + stderr, all runs).
    pub fn logs(&self) -> String {
        let mut all = String::new();
        for run in 1..=self.run {
            for kind in ["out", "err"] {
                if let Ok(s) =
                    fs::read_to_string(self.state.path().join(format!("app-{run}.{kind}.log")))
                {
                    all.push_str(&s);
                }
            }
        }
        all
    }

    /// Anonymous GET on the public listener: (status, headers, body).
    pub fn get(&self, path: &str) -> (u16, Vec<(String, String)>, String) {
        http_get(&format!("{}{path}", self.origin))
    }

    /// GET on the loopback admin listener (operator, via `docker exec`).
    pub fn admin_get(&self, path: &str) -> (u16, String) {
        let (s, _, b) = http_get(&format!("{}{path}", self.admin_origin));
        (s, b)
    }

    /// POST on the loopback admin listener.
    pub fn admin_post(&self, path: &str, body: &str) -> (u16, String) {
        let client = reqwest::blocking::Client::new();
        match client
            .post(format!("{}{path}", self.admin_origin))
            .body(body.to_string())
            .send()
        {
            Ok(r) => {
                let s = r.status().as_u16();
                (s, r.text().unwrap_or_default())
            }
            Err(e) => (0, e.to_string()),
        }
    }

    /// Run a one-shot subcommand of the binary (e.g. `probe --self-test`).
    pub fn run_subcommand(args: &[&str]) -> (Option<i32>, String) {
        let bin = review_app_binary();
        let out = Command::new(&bin)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .unwrap_or_else(|e| panic!("run {}: {e}", bin.display()));
        (
            out.status.code(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }
}

impl Drop for ReviewApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_secrets(bin: &Path, dir: &Path, settings: &AppSettings) {
    let jwk_path = dir.join("client-jwk");
    if !jwk_path.exists() && !settings.omit_secrets.contains(&"client-jwk") {
        let out = Command::new(bin)
            .arg("gen-client-jwk")
            .env_clear()
            .output()
            .unwrap_or_else(|e| panic!("run gen-client-jwk: {e}"));
        assert!(
            out.status.success(),
            "MISSING_FUNCTIONALITY: `openlore-review-app gen-client-jwk` must print a client JWK\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        fs::write(&jwk_path, &out.stdout).expect("write client-jwk");
    }
    let files = [
        ("data-key", DATA_KEY_JSON.to_string()),
        ("github-token", settings.github_token.clone()),
        ("log-salt", LOG_SALT.to_string()),
    ];
    for (name, value) in files {
        let path = dir.join(name);
        if settings.omit_secrets.contains(&name) {
            let _ = fs::remove_file(&path);
        } else {
            fs::write(&path, value).expect("write secret");
        }
    }
}

// =============================================================================
// What the owner's queue tells her, in her own words
// =============================================================================

/// Both wordings the queue uses to account for the repos a run read; a queue
/// that holds neither phrase says nothing at all about repos read.
pub const REPOS_READ_WORDINGS: [&str; 2] = ["repos read", "repo read"];

/// The action a verified owner's queue offers before her first scan.
pub const SCAN_MY_REPOS: &str = "Scan my repos";

/// The action a finished scan offers on the queue.
pub const SCAN_AGAIN: &str = "Scan again";

/// The action an unfinished run offers its owner.
pub const RESUME_SCAN: &str = "Resume scan";

/// Every scan action the queue can offer its owner.
pub const SCAN_ACTIONS: [&str; 3] = [SCAN_MY_REPOS, SCAN_AGAIN, RESUME_SCAN];

/// What the owner's queue tells her at one moment of her scenario.
#[derive(Debug, Clone)]
pub struct QueueEvidence {
    /// The status her latest run is stored with, as her scan-status page says.
    pub stored_run_status: String,
    /// Every status line her queue renders, in page order, as she reads them.
    pub status_region: Vec<String>,
    /// The status line accounting for repos read, when her queue carries one.
    pub repos_read_line: Option<String>,
    /// Whether either repos-read wording appears anywhere on her queue.
    pub says_repos_read_anywhere: bool,
    /// The scan actions her queue offers her.
    pub scan_actions_offered: Vec<String>,
}

impl QueueEvidence {
    /// Whether her queue offers to pick an unfinished run up.
    pub fn offers_resume(&self) -> bool {
        self.scan_actions_offered.iter().any(|a| a == RESUME_SCAN)
    }
}

/// Every `role="status"` / `role="alert"` line of a page, in page order: the
/// status region a queue renders for its owner.
fn status_region(html: &str) -> Vec<String> {
    html::elements(html, "p")
        .into_iter()
        .filter(|paragraph| {
            let open = &paragraph[..paragraph.find('>').map(|e| e + 1).unwrap_or(0)];
            html::attr(open, "role").is_some_and(|role| role == "status" || role == "alert")
        })
        .map(html::visible_text)
        .collect()
}

/// What her queue tells her now, read through her browser over the very pages
/// she reads, and leaving her on her queue.
pub fn queue_evidence(browser: &mut Browser) -> QueueEvidence {
    let stored_run_status = browser
        .open("/scan/status")
        .scan_status()
        .unwrap_or_else(|| "no scan status shown".to_string());
    let page = browser.open("/review");
    let text = page.text();
    let region = status_region(&page.html);
    QueueEvidence {
        stored_run_status,
        repos_read_line: region
            .iter()
            .find(|line| REPOS_READ_WORDINGS.iter().any(|w| line.contains(w)))
            .cloned(),
        says_repos_read_anywhere: REPOS_READ_WORDINGS.iter().any(|w| text.contains(w)),
        scan_actions_offered: SCAN_ACTIONS
            .into_iter()
            .filter(|action| page.offers(action))
            .map(str::to_string)
            .collect(),
        status_region: region,
    }
}

/// Every status line of a queue that accounts for repos read, in page order.
/// A finished run that has a plan carries exactly one; a run with no plan to
/// count carries none, and two lines would account for one scan twice.
pub fn repos_read_lines(status_region: &[String]) -> Vec<String> {
    status_region
        .iter()
        .filter(|line| REPOS_READ_WORDINGS.iter().any(|w| line.contains(w)))
        .cloned()
        .collect()
}

/// The whole number a repos-read line accounts for ("3 repos read" → 3): the
/// number in front of the wording, so a count that merely ends in the true
/// one is read as the number it is.
pub fn repos_read_count(line: &str) -> Option<usize> {
    REPOS_READ_WORDINGS.iter().find_map(|wording| {
        let at = line.find(wording)?;
        let digits: String = line[..at]
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<Vec<char>>()
            .into_iter()
            .rev()
            .collect();
        digits.parse::<usize>().ok()
    })
}

/// Show what she sees at this `moment` of the scenario, beside the assertions
/// made about it. Every run shows it; a run reads it with `--nocapture`, as it
/// reads any other output of a test.
pub fn show_queue_evidence(moment: &str, browser: &mut Browser) -> QueueEvidence {
    let seen = queue_evidence(browser);
    println!("==== what she sees: {moment} ====");
    println!(
        "  the status her latest run is stored with: {}",
        seen.stored_run_status
    );
    println!("  her queue's status region, verbatim, line by line:");
    if seen.status_region.is_empty() {
        println!("    (her queue renders no status line at all)");
    }
    for line in &seen.status_region {
        println!("    | {line}");
    }
    match &seen.repos_read_line {
        Some(line) => println!("  the repos-read line: PRESENT, reading {line:?}"),
        None if seen.says_repos_read_anywhere => println!(
            "  the repos-read line: no status line carries it, yet her queue says \
             \"repos read\" or \"repo read\" somewhere else"
        ),
        None => println!(
            "  the repos-read line: ABSENT, her queue saying neither \"repos read\" \
             nor \"repo read\" anywhere"
        ),
    }
    println!(
        "  \"{RESUME_SCAN}\": {}",
        if seen.offers_resume() {
            "OFFERED"
        } else {
            "not offered"
        }
    );
    println!(
        "  the scan actions her queue offers: {}",
        if seen.scan_actions_offered.is_empty() {
            "(none)".to_string()
        } else {
            seen.scan_actions_offered.join(", ")
        }
    );
    println!("==== end of what she sees ====");
    seen
}

/// Show which of her repos GitHub was asked to read during this `moment` of
/// the scenario, beside the assertions made about them. Every run shows it; a
/// run reads it with `--nocapture`.
pub fn show_repos_read(moment: &str, repos: &std::collections::BTreeSet<String>) {
    println!("==== the repos read: {moment} ====");
    if repos.is_empty() {
        println!("  (no repo was read)");
    }
    for repo in repos {
        println!("  - {repo}");
    }
    println!("  {} in all", repos.len());
    println!("==== end of the repos read ====");
}

/// A plain GET (no cookies): (status, lower-cased headers, body).
pub fn http_get(url: &str) -> (u16, Vec<(String, String)>, String) {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .expect("client");
    match client.get(url).send() {
        Ok(r) => {
            let status = r.status().as_u16();
            let headers = r
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_ascii_lowercase(),
                        v.to_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            (status, headers, r.text().unwrap_or_default())
        }
        Err(e) => (0, Vec::new(), e.to_string()),
    }
}
