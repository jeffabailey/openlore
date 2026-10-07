//! indexer-deployment — the long-running `serve` harness (DISTILL 2026-10-06).
//!
//! Included by the `indexer_deployment_*` suites with
//! `#[path = "support/indexer_live.rs"] mod indexer_live;` AFTER `mod support;`
//! and `#[path = "support/indexer_network.rs"] mod indexer_network;` (it builds
//! on [`IndexerWorld`]: the scenario's home, the fake ATProto network and the
//! operator's configuration).
//!
//! It starts `openlore-indexer serve` the way production does (ADR-080,
//! DEVOPS infrastructure-integration §2): ONE long-running process that owns
//! the index, reads the DID list from a FILE each pass (ADR-081), listens on a
//! control socket for the host timer's `openlore-indexer trigger` (ADR-080 §3),
//! serves the public search + `/healthz` (ADR-083), and purges unlisted authors
//! (ADR-082). Only the external systems are fakes (PLC + every PDS).
//!
//! | Port | Treatment here |
//! |---|---|
//! | `openlore-indexer serve` (driving, long-running) | REAL binary, subprocess, ephemeral `127.0.0.1:0` |
//! | `openlore-indexer trigger` (driving, the host timer's `docker exec`) | REAL binary, subprocess, the serve environment (docker exec inherits it) |
//! | public HTTP surface (`searchClaims`, `/healthz`) | REAL listener, in-test HTTP client |
//! | `openlore search` (driving, Maria) | REAL binary, subprocess |
//! | `index.duckdb` (driven internal) | REAL file; read directly ONLY after `serve` stopped (one holder per file) |
//! | DID list file (driven internal, host-rendered) | REAL file in a config DIRECTORY, replaced by rename (what `render-dids.sh` does) |
//! | PLC + PDSes (driven external) | `FakeAtprotoNetwork` (via `IndexerWorld`) |
//!
//! Every observable is port-exposed: `serve`'s stdout/stderr event lines, exit
//! codes of `serve` and `trigger`, HTTP status + body of the public surface,
//! `openlore search` output, the fake network's request log, and the index rows
//! once `serve` is stopped.
//!
//! ## Fault seam (DISTILL-proposed; DELIVER owns the final name)
//!
//! A panicking pass, a poisoned store, a failing purge and a failing store read
//! cannot be provoked from outside a real process, so the scenarios that need
//! them set the TEST-ONLY `OPENLORE_INDEXER_TEST_FAULT` (debug builds only; a
//! release build must refuse it exactly like `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP`,
//! DD-IPF-5). Precedent: the viewer's `#[cfg(debug_assertions)]` seams
//! (`OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`). Renaming it is a one-line change in
//! [`var::TEST_FAULT`] / [`Fault::token`].

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::indexer_network::{run_bounded, IndexerWorld, PassReport};
use crate::support::{self, CliOutcome};

// =============================================================================
// Vocabulary
// =============================================================================

/// The configuration variables this feature adds (data-models.md §1).
pub mod var {
    pub const REPO_DIDS: &str = "OPENLORE_INDEXER_REPO_DIDS";
    pub const REPO_DIDS_FILE: &str = "OPENLORE_INDEXER_REPO_DIDS_FILE";
    pub const CONTROL_SOCKET: &str = "OPENLORE_INDEXER_CONTROL_SOCKET";
    pub const PURGE_UNLISTED: &str = "OPENLORE_INDEXER_PURGE_UNLISTED";
    pub const DUCKDB_MEMORY_LIMIT_MB: &str = "OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB";
    pub const DUCKDB_THREADS: &str = "OPENLORE_INDEXER_DUCKDB_THREADS";
    pub const PASS_DEADLINE_SECS: &str = "OPENLORE_INDEXER_PASS_DEADLINE_SECS";
    pub const PER_DID_TIMEOUT: &str = "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS";
    pub const INDEX_PATH: &str = "OPENLORE_INDEXER_INDEX_PATH";
    pub const LISTEN_ADDR: &str = "OPENLORE_INDEXER_LISTEN_ADDR";
    /// TEST-ONLY fault seam (see the module docs).
    pub const TEST_FAULT: &str = "OPENLORE_INDEXER_TEST_FAULT";
}

/// The exit codes of one pass as the host timer sees them through `trigger`
/// (ADR-078 §2, ADR-080 §3; infrastructure-integration §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassExit {
    /// The pass completed (skips allowed), or it coalesced into a running pass.
    Completed,
    /// Config / list / store / purge / deadline / panic failure.
    Failed,
    /// Every listed DID was skipped.
    TotalOutage,
    /// `serve` could not be reached: no pass ran (`trigger` only).
    ServeUnreachable,
}

impl PassExit {
    pub const fn code(self) -> i32 {
        match self {
            PassExit::Completed => 0,
            PassExit::Failed => 2,
            PassExit::TotalOutage => 3,
            PassExit::ServeUnreachable => 4,
        }
    }
}

/// Why a pass ended with exit 2 (`pass_summary.cause`, data-models.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCause {
    ListMalformed,
    ListUnreadable,
    UpsertFailed,
    PurgeFailed,
    PassPanicked,
    PassDeadlineExceeded,
}

impl FailureCause {
    pub const fn token(self) -> &'static str {
        match self {
            FailureCause::ListMalformed => "repo_dids_malformed",
            FailureCause::ListUnreadable => "repo_dids_unreadable",
            FailureCause::UpsertFailed => "upsert_failed",
            FailureCause::PurgeFailed => "purge_failed",
            FailureCause::PassPanicked => "pass_panicked",
            FailureCause::PassDeadlineExceeded => "pass_deadline_exceeded",
        }
    }
}

/// A fault only a test can provoke (the TEST-ONLY seam).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The first pass panics inside its gate phase; later passes are normal.
    FirstPassPanics,
    /// The store's lock is poisoned during the next pass.
    StorePoisonedAtNextPass,
    /// Every store read made by a search fails (the store itself stays usable).
    SearchCannotReadTheStore,
    /// The purge step of the next pass fails.
    PurgeFails,
}

impl Fault {
    pub const fn token(self) -> &'static str {
        match self {
            Fault::FirstPassPanics => "first_pass_panics",
            Fault::StorePoisonedAtNextPass => "store_poisoned",
            Fault::SearchCannotReadTheStore => "search_store_read_fails",
            Fault::PurgeFails => "purge_fails",
        }
    }
}

/// The two public routes (ADR-083 §1).
pub const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
pub const HEALTH_PATH: &str = "/healthz";

/// The production values of the new settings (DEVOPS compose, §2).
pub const PRODUCTION_MEMORY_LIMIT_MB: &str = "48";
pub const PRODUCTION_THREADS: &str = "1";
pub const PRODUCTION_PASS_DEADLINE_SECS: &str = "1500";

/// Ceiling for one `trigger` run (a pass can legitimately take ~60 s in the
/// deadline scenario).
const TRIGGER_CEILING: Duration = Duration::from_secs(150);
/// Time `serve` has to report it is listening (or exit refusing).
const READY_CEILING: Duration = Duration::from_secs(30);

// =============================================================================
// The operator's deployment choices
// =============================================================================

/// How the operator deployed `serve` (defaults = production, compose §2).
#[derive(Debug, Clone)]
pub struct Deployment {
    /// The DID list file's initial text (`None` = no file at all).
    pub list_text: Option<String>,
    /// `OPENLORE_INDEXER_PURGE_UNLISTED=1`.
    pub purge_unlisted: bool,
    /// Extra or overriding settings (`(variable, value)`); an empty value
    /// REMOVES the variable.
    pub settings: Vec<(String, String)>,
    /// The TEST-ONLY fault, if any.
    pub fault: Option<Fault>,
}

impl Deployment {
    /// Production posture, listing exactly `dids`.
    pub fn listing(dids: &[&str]) -> Self {
        Self {
            list_text: Some(dids.join(",")),
            purge_unlisted: true,
            settings: Vec::new(),
            fault: None,
        }
    }

    /// Production posture with this exact DID-list file text.
    pub fn with_list_text(text: &str) -> Self {
        Self {
            list_text: Some(text.to_string()),
            ..Self::listing(&[])
        }
    }

    pub fn setting(mut self, variable: &str, value: &str) -> Self {
        self.settings
            .push((variable.to_string(), value.to_string()));
        self
    }

    pub fn without_purge(mut self) -> Self {
        self.purge_unlisted = false;
        self
    }

    pub fn with_fault(mut self, fault: Fault) -> Self {
        self.fault = Some(fault);
        self
    }
}

// =============================================================================
// The running index
// =============================================================================

/// A running `openlore-indexer serve` in production posture (killed on drop).
pub struct LiveIndex {
    pub url: String,
    child: Child,
    env: Vec<(String, String)>,
    config_dir: PathBuf,
    socket: PathBuf,
    /// Keeps the short socket directory alive.
    _socket_dir: Arc<tempfile::TempDir>,
    stdout: Arc<Mutex<Vec<String>>>,
    stderr: Arc<Mutex<Vec<String>>>,
}

/// What happened when the operator started `serve`.
pub enum Startup {
    Ready(LiveIndex),
    Refused(PassReport),
}

/// The list file inside the config DIRECTORY the container mounts read-only.
pub fn list_file_of(config_dir: &Path) -> PathBuf {
    config_dir.join("repo-dids")
}

fn spawn_line_collector<R: std::io::Read + Send + 'static>(
    stream: R,
    sink: Arc<Mutex<Vec<String>>>,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(l) => sink.lock().expect("collector lock").push(l),
                Err(_) => break,
            }
        }
    });
}

fn events_of(lines: &[String]) -> Vec<serde_json::Value> {
    lines
        .iter()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .filter(|v| v.get("event").is_some())
        .collect()
}

impl LiveIndex {
    /// GIVEN Jeff deployed `serve` like this over `world`'s index (requires it
    /// to become ready; a refusal is reported as MISSING_FUNCTIONALITY).
    pub fn deploy(world: &IndexerWorld, deployment: Deployment) -> Self {
        match Self::launch(world, deployment) {
            Startup::Ready(live) => live,
            Startup::Refused(report) => panic!(
                "MISSING_FUNCTIONALITY: `openlore-indexer serve` did not become ready in \
                 production posture (DID list file, control socket, purge, DuckDB caps).\n{}",
                report.dump()
            ),
        }
    }

    /// Start `serve` and report whether it became ready or refused.
    pub fn launch(world: &IndexerWorld, deployment: Deployment) -> Startup {
        let config_dir = world.env.home.join("indexer-config");
        std::fs::create_dir_all(&config_dir).expect("create the DID-list config directory");
        if let Some(text) = &deployment.list_text {
            render_list(&config_dir, text);
        }
        let socket_dir = Arc::new(short_socket_dir());
        let socket = socket_dir.path().join("openlore-indexer.sock");
        let env = Self::production_env(world, &deployment, &config_dir, &socket);
        Self::spawn(env, config_dir, socket, socket_dir)
    }

    fn production_env(
        world: &IndexerWorld,
        deployment: &Deployment,
        config_dir: &Path,
        socket: &Path,
    ) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = world
            .indexer_env()
            .into_iter()
            .filter(|(k, _)| k != var::REPO_DIDS)
            .collect();
        let mut set = |k: &str, v: String| {
            env.retain(|(key, _)| key != k);
            if !v.is_empty() {
                env.push((k.to_string(), v));
            }
        };
        set(var::LISTEN_ADDR, "127.0.0.1:0".to_string());
        set(
            var::REPO_DIDS_FILE,
            list_file_of(config_dir).display().to_string(),
        );
        set(var::CONTROL_SOCKET, socket.display().to_string());
        set(
            var::DUCKDB_MEMORY_LIMIT_MB,
            PRODUCTION_MEMORY_LIMIT_MB.to_string(),
        );
        set(var::DUCKDB_THREADS, PRODUCTION_THREADS.to_string());
        set(
            var::PASS_DEADLINE_SECS,
            PRODUCTION_PASS_DEADLINE_SECS.to_string(),
        );
        if deployment.purge_unlisted {
            set(var::PURGE_UNLISTED, "1".to_string());
        }
        if let Some(fault) = deployment.fault {
            set(var::TEST_FAULT, fault.token().to_string());
        }
        for (k, v) in &deployment.settings {
            set(k, v.clone());
        }
        env
    }

    fn spawn(
        env: Vec<(String, String)>,
        config_dir: PathBuf,
        socket: PathBuf,
        socket_dir: Arc<tempfile::TempDir>,
    ) -> Startup {
        let bin = support::resolve_workspace_bin("openlore-indexer");
        let mut cmd = Command::new(&bin);
        cmd.arg("serve").env_clear();
        for (k, v) in &env {
            cmd.env(k, v);
        }
        let started = Instant::now();
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn openlore-indexer serve: {e}"));
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        spawn_line_collector(child.stdout.take().expect("serve stdout"), stdout.clone());
        spawn_line_collector(child.stderr.take().expect("serve stderr"), stderr.clone());

        loop {
            let listening = events_of(&stdout.lock().expect("lock"))
                .into_iter()
                .find(|e| e["event"] == "indexer.serve.listening")
                .and_then(|e| e["addr"].as_str().map(str::to_string));
            if let Some(addr) = listening {
                return Startup::Ready(LiveIndex {
                    url: format!("http://{addr}"),
                    child,
                    env,
                    config_dir,
                    socket,
                    _socket_dir: socket_dir,
                    stdout,
                    stderr,
                });
            }
            let exited = child.try_wait().ok().flatten();
            if exited.is_some() || started.elapsed() > READY_CEILING {
                if exited.is_none() {
                    let _ = child.kill();
                }
                let status = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                std::thread::sleep(Duration::from_millis(100)); // let the collectors drain
                let report = PassReport {
                    status,
                    stdout: stdout.lock().expect("lock").join("\n"),
                    stderr: stderr.lock().expect("lock").join("\n"),
                    elapsed: started.elapsed(),
                };
                return Startup::Refused(report);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    // ------------------------------------------------------------ the host side

    /// GIVEN/WHEN the host renders a new DID list: stage a file in the SAME
    /// directory and rename it over `repo-dids` (exactly what `render-dids.sh`
    /// does; the directory itself is never replaced, ADR-081 / H1).
    pub fn operator_saves_list(&self, dids: &[&str]) {
        render_list(&self.config_dir, &dids.join(","));
    }

    /// … with this exact text (typos, BOMs, CRLFs).
    pub fn operator_saves_list_text(&self, text: &str) {
        render_list(&self.config_dir, text);
    }

    /// GIVEN the list file is gone (never rendered, or deleted by hand).
    pub fn list_file_is_missing(&self) {
        let _ = std::fs::remove_file(list_file_of(&self.config_dir));
    }

    /// GIVEN something that cannot be read as a file sits where the list
    /// belongs (works whatever user the tests run as, unlike `chmod 000`).
    pub fn list_file_is_unreadable(&self) {
        let path = list_file_of(&self.config_dir);
        let _ = std::fs::remove_file(&path);
        std::fs::create_dir_all(&path).expect("place a directory where the list file belongs");
    }

    /// GIVEN the list file was last rendered `age` ago (its mtime).
    pub fn list_was_rendered_ago(&self, age: Duration) {
        // The rendered list is 0444: set the time through a read-only handle
        // (the owner may set explicit times without write permission).
        let file = std::fs::File::open(list_file_of(&self.config_dir)).expect("open the list file");
        file.set_modified(std::time::SystemTime::now() - age)
            .expect("set the list file's modification time");
    }

    /// The config directory's inode (unchanged by a rename of the file inside).
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    // ------------------------------------------------------------------ When

    /// WHEN the host timer fires: `docker exec … openlore-indexer trigger`
    /// (the exec inherits the container's environment).
    pub fn timer_fires(&self) -> PassReport {
        run_bounded(self.trigger_command(&self.env), TRIGGER_CEILING)
    }

    /// WHEN the timer fires while the scenario keeps acting (the pass runs in
    /// the background; join with [`PendingPass::finish`]).
    pub fn timer_fires_in_background(&self) -> PendingPass {
        let cmd = self.trigger_command(&self.env);
        PendingPass {
            join: std::thread::spawn(move || run_bounded(cmd, TRIGGER_CEILING)),
        }
    }

    /// WHEN `trigger` runs with ONLY these variables (plus `PATH`).
    pub fn trigger_with_only(&self, env: &[(&str, &str)]) -> PassReport {
        let owned: Vec<(String, String)> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        run_bounded(self.trigger_command(&owned), TRIGGER_CEILING)
    }

    fn trigger_command(&self, env: &[(String, String)]) -> Command {
        let bin = support::resolve_workspace_bin("openlore-indexer");
        let mut cmd = Command::new(&bin);
        cmd.arg("trigger").env_clear();
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd
    }

    /// The control socket path (as configured).
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The environment `serve` runs with (what `docker exec` inherits).
    pub fn environment(&self) -> &[(String, String)] {
        &self.env
    }

    /// WHEN Maria runs `openlore search <args>` against this index.
    pub fn maria_searches(&self, world: &IndexerWorld, args: &[&str]) -> CliOutcome {
        crate::indexer_network::search_against(&world.env, args, &self.url)
    }

    /// WHEN anyone sends `method path` with `body` to the public listener.
    /// Returns (status, body text, time taken).
    pub fn request(&self, method: &str, path: &str, body: Vec<u8>) -> (u16, String, Duration) {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("http client");
        let method = reqwest::Method::from_bytes(method.as_bytes()).expect("http method");
        let started = Instant::now();
        let response = client
            .request(method, format!("{}{}", self.url, path))
            .header("content-type", "application/json")
            .body(body)
            .send();
        let elapsed = started.elapsed();
        match response {
            Ok(r) => {
                let status = r.status().as_u16();
                (status, r.text().unwrap_or_default(), elapsed)
            }
            Err(e) => (0, format!("[harness] no HTTP response: {e}"), elapsed),
        }
    }

    /// WHEN anyone searches the public method directly (dimension, value).
    pub fn public_search(&self, dimension: &str, value: &str) -> (u16, String, Duration) {
        let body = serde_json::json!({"dimension": dimension, "value": value}).to_string();
        self.request("POST", SEARCH_PATH, body.into_bytes())
    }

    /// WHEN anyone reads the health response: (status, JSON body).
    pub fn health(&self) -> (u16, serde_json::Value) {
        let (status, body, _) = self.request("GET", HEALTH_PATH, Vec::new());
        (
            status,
            serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
        )
    }

    /// WHEN the container is killed hard (SIGKILL: OOM kill, host crash).
    pub fn gets_killed(mut self) -> RestartableIndex {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let restart = RestartableIndex {
            env: std::mem::take(&mut self.env),
            config_dir: self.config_dir.clone(),
            socket: self.socket.clone(),
            socket_dir: self._socket_dir.clone(),
        };
        drop(self);
        restart
    }

    /// Stop `serve` (so the index file can be read directly).
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    // ------------------------------------------------------------------ Then

    /// Every structured event `serve` printed so far (stdout then stderr).
    pub fn events(&self) -> Vec<serde_json::Value> {
        let mut all = events_of(&self.stdout.lock().expect("lock"));
        all.extend(events_of(&self.stderr.lock().expect("lock")));
        all
    }

    pub fn events_named(&self, name: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|e| e["event"] == name)
            .collect()
    }

    /// Every `indexer.ingest.pass_summary` so far, in order.
    pub fn pass_summaries(&self) -> Vec<serde_json::Value> {
        self.events_named("indexer.ingest.pass_summary")
    }

    /// The events of one pass (those carrying its `pass_id`).
    pub fn events_of_pass(&self, pass_id: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|e| e["pass_id"] == pass_id)
            .collect()
    }

    /// Wait until `serve` printed an event named `name` with `pass_id` set
    /// (a pass has started), or `timeout` passed.
    pub fn wait_for_pass_event(&self, name: &str, timeout: Duration) -> Option<serde_json::Value> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(e) = self
                .events_named(name)
                .into_iter()
                .find(|e| e.get("pass_id").map(|p| !p.is_null()).unwrap_or(false))
            {
                return Some(e);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    /// `serve`'s exit code if it exits within `timeout` (None = still running).
    pub fn exits_within(&mut self, timeout: Duration) -> Option<i32> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status.code().unwrap_or(-1));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    /// How many times `serve` reported it started listening (1 = never restarted).
    pub fn times_started(&self) -> usize {
        self.events_named("indexer.serve.listening").len()
    }

    /// Diagnostic dump for assertion messages.
    pub fn dump(&self) -> String {
        format!(
            "--- serve stdout ---\n{}\n--- serve stderr ---\n{}",
            self.stdout.lock().expect("lock").join("\n"),
            self.stderr.lock().expect("lock").join("\n")
        )
    }
}

impl Drop for LiveIndex {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A killed `serve` the container runtime will start again (same data, same
/// config directory, same socket path — `restart: unless-stopped`).
pub struct RestartableIndex {
    env: Vec<(String, String)>,
    config_dir: PathBuf,
    socket: PathBuf,
    socket_dir: Arc<tempfile::TempDir>,
}

impl RestartableIndex {
    /// WHEN the container runtime restarts `serve`.
    pub fn restarts(self) -> Startup {
        LiveIndex::spawn(self.env, self.config_dir, self.socket, self.socket_dir)
    }

    /// GIVEN a regular file (not a socket) sits at the socket path, as after a crash.
    pub fn leaves_a_stale_socket_file(&self) {
        std::fs::write(&self.socket, b"stale").expect("leave a stale socket file");
    }
}

/// WHEN the host timer fires with exactly this environment and no `serve`
/// object in hand (e.g. after `serve` stopped).
pub fn timer_fires_with(env: &[(String, String)]) -> PassReport {
    let bin = support::resolve_workspace_bin("openlore-indexer");
    let mut cmd = Command::new(&bin);
    cmd.arg("trigger").env_clear();
    cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
    for (k, v) in env {
        cmd.env(k, v);
    }
    run_bounded(cmd, TRIGGER_CEILING)
}

/// A pass the timer started in the background.
pub struct PendingPass {
    join: std::thread::JoinHandle<PassReport>,
}

impl PendingPass {
    /// Wait for the `trigger` client to exit.
    pub fn finish(self) -> PassReport {
        self.join.join().expect("trigger thread")
    }

    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }
}

/// Stage the list text in the SAME directory, make it read-only, rename it
/// over `repo-dids` (rename(2) replaces the FILE; the directory keeps its inode).
pub fn render_list(config_dir: &Path, text: &str) {
    let staging = config_dir.join(".repo-dids.new");
    let target = list_file_of(config_dir);
    if target.is_dir() {
        std::fs::remove_dir_all(&target).expect("clear a directory squatting on the list path");
    }
    std::fs::write(&staging, text).expect("stage the DID list");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o444))
            .expect("chmod 0444 the staged list");
    }
    std::fs::rename(&staging, &target).expect("rename the staged list over repo-dids");
    std::fs::write(config_dir.join(".rendered-at"), b"").expect("touch .rendered-at");
}

/// A short directory for the control socket (Unix socket paths are limited to
/// ~104 bytes; per-scenario homes under `$TMPDIR` can be longer).
fn short_socket_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("olx")
        .tempdir_in("/tmp")
        .or_else(|_| tempfile::tempdir())
        .expect("socket directory")
}

// =============================================================================
// Observable checks shared by the suites
// =============================================================================

/// THEN exactly one `pass_summary` exists for every pass this `trigger` ran,
/// and it agrees with the `trigger` exit code (B4).
pub fn assert_one_summary_matching(
    live: &LiveIndex,
    trigger: &PassReport,
    exit: PassExit,
) -> serde_json::Value {
    assert_eq!(
        trigger.status,
        exit.code(),
        "trigger exits with the pass's code\n{}\n{}",
        trigger.dump(),
        live.dump()
    );
    let summaries = live.pass_summaries();
    let last = summaries.last().cloned().unwrap_or_else(|| {
        panic!(
            "MISSING_FUNCTIONALITY: no indexer.ingest.pass_summary from serve\n{}\n{}",
            trigger.dump(),
            live.dump()
        )
    });
    let pass_id = last["pass_id"]
        .as_str()
        .unwrap_or_else(|| panic!("pass_summary carries a pass_id: {last}"))
        .to_string();
    assert_eq!(
        summaries
            .iter()
            .filter(|s| s["pass_id"] == pass_id.as_str())
            .count(),
        1,
        "exactly one pass_summary for pass {pass_id}\n{}",
        live.dump()
    );
    assert_eq!(
        last["exit_code"].as_i64(),
        Some(exit.code() as i64),
        "the summary's exit_code\n{}",
        live.dump()
    );
    last
}

/// The fields a pass event may carry (data-models.md §4; WD-105 / I-IXD-4).
pub const STRUCTURAL_EVENT_FIELDS: [&str; 33] = [
    "event",
    "pass_id",
    "did",
    "reason",
    "fallback_used",
    "pds_url",
    "fallback_failure",
    "detail",
    "configured",
    "own_pds",
    "fallback",
    "skipped",
    "duration_ms",
    "exit_code",
    "purged_authors",
    "cause",
    "variable",
    "value",
    "claims_removed",
    "repo_did_count",
    "repo_dids_source",
    "repo_dids_age_secs",
    "count",
    "by_reason",
    "addr",
    "dimension",
    "cap",
    "running_pass_id",
    "socket",
    "adapter",
    "source",
    "plc_endpoint",
    "max_concurrent_fetches",
];

/// THEN no event carries claim content (subjects, philosophies, evidence).
pub fn assert_events_carry_no_claim_content(events: &[serde_json::Value], extra_allowed: &[&str]) {
    for e in events {
        let object = e.as_object().expect("an event is a JSON object");
        let text = e.to_string();
        for fragment in [
            "org.openlore.philosophy",
            "github:",
            "github.com/",
            "\"evidence\"",
        ] {
            assert!(
                !text.contains(fragment),
                "an event leaks claim content ({fragment}): {e}"
            );
        }
        if e["event"]
            .as_str()
            .map(|n| n.starts_with("indexer.ingest.") || n.starts_with("indexer.trigger."))
            .unwrap_or(false)
        {
            for key in object.keys() {
                assert!(
                    STRUCTURAL_EVENT_FIELDS.contains(&key.as_str())
                        || extra_allowed.contains(&key.as_str()),
                    "unexpected field `{key}` in a pass event: {e}"
                );
            }
        }
    }
}

// =============================================================================
// The journey's shared Given/When/Then steps (Pillar 2: every suite chains
// from these; Mandate-12: one definition per domain fact)
// =============================================================================

pub mod journey {
    use super::*;
    use crate::indexer_network::claims::*;
    use crate::indexer_network::{search_rows, Author, Claim, SearchRow};

    /// Tomás's self-attested claim (US-IXD-003 Elevator Pitch).
    pub const QUARRY_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:therrera/quarry",
        philosophy: "reproducible-builds",
        basis_points: 7400,
    };

    /// Priya's claim approved after the first pass (US-IXD-002 Elevator Pitch).
    pub const CARGO_PIN_TEST_DRIVEN: Claim = Claim {
        subject: "github:priyaraman/cargo-pin",
        philosophy: "test-driven",
        basis_points: 7400,
    };

    pub const REPRODUCIBLE_BUILDS: &str = "org.openlore.philosophy.reproducible-builds";

    /// The bare DIDs of `authors`, in order.
    pub fn dids(authors: &[Author]) -> Vec<&'static str> {
        authors.iter().map(|a| a.did()).collect()
    }

    /// GIVEN Priya (bsky.social) published 2 self-attested claims on
    /// cargo-pin, Dmitri (pds.volkov.dev) 2 app-signed claims on ferrite,
    /// Jeff 1 app-signed claim on openlore and Tomás 1 self-attested claim on
    /// quarry — each on their own PDS. Nobody is listed yet.
    pub fn given_authors_publish_on_their_own_pdses() -> IndexerWorld {
        let mut world = IndexerWorld::configured_with(&[
            Author::Priya,
            Author::Dmitri,
            Author::Jeff,
            Author::Tomas,
        ]);
        world.publishes_self_attested(
            Author::Priya,
            &[CARGO_PIN_REPRODUCIBLE_BUILDS, CARGO_PIN_DEPENDENCY_PINNING],
        );
        world.publishes_app_signed(
            Author::Dmitri,
            &[FERRITE_REPRODUCIBLE_BUILDS, FERRITE_DEPENDENCY_PINNING],
        );
        world.publishes_app_signed(Author::Jeff, &[OPENLORE_LOCAL_FIRST]);
        world.publishes_self_attested(Author::Tomas, &[QUARRY_REPRODUCIBLE_BUILDS]);
        world
    }

    /// GIVEN Jeff deployed the index (production posture) listing `authors`.
    pub fn given_the_index_is_live_listing(world: &IndexerWorld, authors: &[Author]) -> LiveIndex {
        LiveIndex::deploy(world, Deployment::listing(&dids(authors)))
    }

    /// GIVEN … and one scheduled pass already indexed everyone listed.
    pub fn given_a_pass_indexed(live: &LiveIndex) -> PassReport {
        let pass = live.timer_fires();
        assert_one_summary_matching(live, &pass, PassExit::Completed);
        pass
    }

    /// The rows of Maria's `openlore search` attributed to `author` (bare DID
    /// for self-attested rows, the application identity for app-signed rows).
    pub fn rows_by(rows: &[SearchRow], author: Author) -> Vec<SearchRow> {
        rows.iter()
            .filter(|r| r.author_did == author.did() || r.author_did == author.app_identity())
            .cloned()
            .collect()
    }

    /// WHEN Maria searches one author's whole trail (`--contributor`).
    pub fn maria_looks_up(
        world: &IndexerWorld,
        live: &LiveIndex,
        author: Author,
    ) -> Vec<SearchRow> {
        let out = live.maria_searches(world, &["--contributor", author.did()]);
        assert_eq!(
            out.status, 0,
            "openlore search exits 0\n{}\n{}",
            out.stdout, out.stderr
        );
        search_rows(&out.stdout)
    }

    /// WHEN Maria searches who claims reproducible builds (`--object`).
    pub fn maria_searches_reproducible_builds(
        world: &IndexerWorld,
        live: &LiveIndex,
    ) -> Vec<SearchRow> {
        let out = live.maria_searches(world, &["--object", REPRODUCIBLE_BUILDS]);
        assert_eq!(
            out.status, 0,
            "openlore search exits 0\n{}\n{}",
            out.stdout, out.stderr
        );
        assert!(
            !out.stdout.contains("Network index unavailable"),
            "the live index answered (not the local-only fallback)\n{}",
            out.stdout
        );
        search_rows(&out.stdout)
    }

    /// THEN Maria finds exactly `count` claims by `author`.
    pub fn then_maria_finds(world: &IndexerWorld, live: &LiveIndex, author: Author, count: usize) {
        let rows = maria_looks_up(world, live, author);
        assert_eq!(
            rows_by(&rows, author).len(),
            count,
            "Maria finds {count} claims by {author:?}: {rows:#?}\n{}",
            live.dump()
        );
    }
}
