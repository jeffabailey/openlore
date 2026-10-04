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
//!
//! Secrets files (DV-BRA-4): `client-jwk` (from `openlore-review-app
//! gen-client-jwk`, a DELIVER deliverable), `data-key`, `github-token`,
//! `log-salt`.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// The granular scope set (ADR-073 / SPIKE-3).
pub const OAUTH_SCOPES: &str =
    "atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create";

/// A fixed 32-byte data key (hex) — test only.
const DATA_KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
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
    // otherwise the binary sits next to this test's `deps/` directory.
    // (`assert_cmd::cargo_bin` would panic before we could classify the miss.)
    let name = format!("openlore-review-app{}", std::env::consts::EXE_SUFFIX);
    let bin = std::env::var_os("CARGO_BIN_EXE_openlore-review-app")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let exe = std::env::current_exe().expect("current test executable");
            let deps = exe.parent().expect("deps dir");
            deps.parent().unwrap_or(deps).join(&name)
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

impl ReviewApp {
    /// Launch with fresh state; wait for readiness or report the refusal.
    pub fn launch(settings: AppSettings) -> Startup {
        let state = tempfile::tempdir().expect("review-app state dir");
        let port = free_port();
        let admin_port = free_port();
        Self::launch_in(settings, state, port, admin_port, 1)
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

    fn launch_in(
        settings: AppSettings,
        state: TempDir,
        port: u16,
        admin_port: u16,
        run: u32,
    ) -> Startup {
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
            Ok(()) => Startup::Ready(Box::new(app)),
            Err(exit_code) => {
                let logs = app.logs();
                Startup::Refused { exit_code, logs }
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
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        Err(None)
    }

    /// Stop and start the same app (same origin, database and secrets).
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
        match Self::launch_in(settings, state, port, admin_port, run + 1) {
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
        ("data-key", DATA_KEY_HEX.to_string()),
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
