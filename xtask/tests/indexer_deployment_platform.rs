//! indexer-deployment (DISTILL 2026-10-06) — CI-checkable deploy posture,
//! asserted against the REAL repo files DELIVER creates (DEVOPS
//! infrastructure-integration §2-§7, ci-cd-pipeline §3/§5, monitoring-alerting §1):
//!
//! * XP-1  compose: exactly the data (rw) and config DIRECTORY (ro) mounts, `init`, caps, OOM order, no credentials
//! * XP-2  review-app compose (same release, B11): `init`, DuckDB 48 MB / 1 thread, 192m
//! * XP-3  Caddy site: only POST search + GET /healthz, 8 KB body cap, no Server header, HTTPS only
//! * XP-4  systemd: 15-minute aligned persistent timer; oneshot pass unit that renders then triggers; timeout ≥ deadline
//! * XP-5  `render-dids.sh` REAL run: two consecutive edits replace the FILE by rename, never the directory (H1)
//! * XP-6  `render-dids.sh` REAL run: a failed or empty read keeps the last good list byte-identical and exits 0 (AC-003.4)
//! * XP-7  `deploy.sh` REAL run: tags, branches, short shas are refused before any tool runs (AC-006.1)
//! * XP-8  `deploy.sh` REAL run: an unsigned digest and a red-CI sha are refused before the host is touched
//! * XP-9  alarms: exactly 3, on the existing topic with recoveries, A1 = 2 of 2 × 900 s Minimum, A3 missing = breaching (AC-004.1..4, 004.6)
//! * XP-10 metric filters key on the events the binary emits; no default_value; queries read no claim content (AC-005.4)
//! * XP-11 host IAM: read the DID parameter, write/filter the indexer's own logs, nothing else (AC-003.5)
//! * XP-12 image + CI: distroless non-root image by digest; build, smoke, scan, sign in CI (AC-001.6, AC-006.1)
//! * XP-13 `health-timer.sh` REAL run: a failing `FilterLogEvents` makes the health line `not_live = 1` (fail closed, A3)
//! * XP-14 `health-timer.sh` REAL run: a hung `FilterLogEvents` is cut off, still not live, and the line is still written
//! * XP-15 `deploy.sh host install` REAL run: the IMDS probe fails CLOSED (digest-pinned image, positive control, exit 7/28 only)
//! * XP-16 `deploy.sh deploy <digest>` REAL run: the image's revision label must have green CI (both deploy scripts)
//! * XP-17 ci.yml: an image is only built, signed and pushed after every test job of the commit is green
//! * XP-18 compose: X-Forwarded-For is trusted only from the Docker bridge ranges pds_default comes from
//! * XP-19 `health-timer.sh` REAL run: a search answering 500 makes the health line `not_live = 1` (D2)
//! * XP-20 `health-timer.sh` REAL run: a 503/429/408 or unanswered search is `search_ok = 0` but not `not_live`
//! * XP-21 review-app `render-secrets.sh` REAL run: two renders replace each secret FILE by rename, drop stale ones, never swap the directory (D3)
//! * XP-22 review-app `render-secrets.sh` REAL run: a render missing a required secret changes nothing in the directory (D3)
//! * XP-23..XP-30 `deploy.sh host install/deploy/redeploy` REAL run (both apps): IMDS isolation, the PDS Caddy sites mount + import, adapt + validate before reload (S7, N3)
//! * XP-31 `deploy.sh host deploy` REAL run: the indexer readiness wait is overridable for the first deploy (N4)
//!
//! The shell scripts run under `bash` with stub `aws`/`gh`/`cosign`/… on PATH
//! that record every call; nothing reaches AWS, GitHub or a registry. Live
//! deploy, rollback drill, alarm test-fire and the memory gate stay in the
//! runbook (`deploy/indexer/README.md`; DEVOPS I-4..I-6).
//!
//! ## Testability contract for the scripts
//!
//! * `render-dids.sh` honours `INDEXER_CONFIG_DIR` (default `/pds/indexer/config`)
//!   and resolves `aws`, `timeout`, `chown` from `PATH`.
//! * review-app `render-secrets.sh` honours `REVIEW_APP_SECRETS_DIR` and `REVIEW_APP_SSM_PATH`
//!   and resolves `aws`, `chown` from `PATH`.
//! * `deploy.sh` resolves `gh`, `cosign`, `crane`, `aws`, `curl`, `docker` from `PATH`.
//! * `deploy.sh host install` honours `INDEXER_CADDY_SITES_DIR` / `REVIEW_APP_CADDY_SITES_DIR`
//!   (default `/pds/caddy/sites`) and `INDEXER_BASE_DIR` / `REVIEW_APP_BASE_DIR` (default
//!   `/pds/indexer`, `/pds/app`)
//!   and resolves `docker`, `install` from `PATH`.
//! * `health-timer.sh run` honours `INDEXER_CONFIG_DIR`, `INDEXER_STATE_DIR` and
//!   `INDEXER_AWS_TIMEOUT_S` (default 15), resolves `docker`, `curl`, `aws`, `timeout` from `PATH`, prints its `indexer.host.health` line on stdout,
//!   tolerates a host without `/proc` (fields default to 0) and always exits 0.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel} exists: {e}"))
}

/// The value of `key: value` lines (YAML-ish, first match, quotes stripped).
fn yaml_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix(&format!("{key}:")))
        .map(|v| {
            v.split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string()
        })
}

/// The `- /…` volume lines of a compose file (quoted or not).
fn mounts(compose: &str) -> Vec<String> {
    compose
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(|l| {
            l.split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .filter(|l| l.starts_with('/'))
        .collect()
}

/// `resource "<kind>" "<name>" { … }` blocks of an HCL file, as (kind, name, text).
fn hcl_resources(hcl: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let parts: Vec<&str> = hcl.split("\nresource \"").collect();
    for part in parts.iter().skip(1) {
        let mut head = part.splitn(3, '"');
        let kind = head.next().unwrap_or("").to_string();
        let _ = head.next();
        let rest = head.next().unwrap_or("");
        let name = rest.split('"').next().unwrap_or("").to_string();
        out.push((kind, name, part.to_string()));
    }
    out
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A directory of stub executables that append `<name> <args>` to `calls.log`.
struct Stubs {
    dir: tempfile::TempDir,
}

impl Stubs {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("stub dir"),
        }
    }

    fn add(&self, name: &str, body: &str) {
        let path = self.dir.path().join(name);
        let log = self.dir.path().join("calls.log");
        std::fs::write(
            &path,
            format!(
                "#!/usr/bin/env bash\necho \"{name} $*\" >> '{}'\n{body}\n",
                log.display()
            ),
        )
        .expect("write stub");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod stub");
        }
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("calls.log")).unwrap_or_default()
    }

    fn path_env(&self) -> String {
        format!(
            "{}:{}",
            self.dir.path().display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }
}

struct Run {
    status: i32,
    out: String,
}

fn bash(script: &str, args: &[&str], env: &[(&str, String)]) -> Run {
    let mut cmd = Command::new("bash");
    cmd.arg(root().join(script)).args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("run bash");
    Run {
        status: output.status.code().unwrap_or(-1),
        out: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

// =============================================================================
// Compose (infrastructure-integration §2; ci-cd-pipeline §5.4)
// =============================================================================

/// XP-1 @US-IXD-001 @US-IXD-006 @AC-001.6 @AC-003.5 @AC-006.5 @C-1 @NFR-IXD-8 @infrastructure
/// @contract-shape:pure-function
#[test]
fn the_indexer_container_has_exactly_its_two_mounts_and_the_production_posture() {
    let compose = read("deploy/indexer/host/compose.yaml");
    let mut found = mounts(&compose);
    found.sort();
    assert_eq!(
        found,
        vec![
            "/pds/indexer/config:/config:ro".to_string(),
            "/pds/indexer/data:/data".to_string(),
        ],
        "exactly the data mount and the config DIRECTORY, read-only (a file mount pins the old inode, ADR-081)"
    );
    for (key, want) in [
        ("init", "true"),
        ("read_only", "true"),
        ("restart", "unless-stopped"),
        ("stop_grace_period", "5s"),
        ("mem_limit", "128m"),
        ("memswap_limit", "128m"),
        ("container_name", "openlore-indexer"),
        ("user", "65532:65532"),
    ] {
        assert_eq!(yaml_value(&compose, key).as_deref(), Some(want), "{key}");
    }
    let oom: i64 = yaml_value(&compose, "oom_score_adj")
        .and_then(|v| v.parse().ok())
        .expect("oom_score_adj");
    assert!(
        oom >= 900,
        "the kernel kills the indexer before the review app and the PDS"
    );
    assert!(
        compose.contains("cap_drop: [ALL]") || compose.contains("- ALL"),
        "cap_drop ALL"
    );
    assert!(
        compose.contains("no-new-privileges:true"),
        "no-new-privileges"
    );
    assert!(
        compose.contains("/tmp"),
        "tmpfs /tmp for the control socket"
    );
    assert!(
        compose.contains("@${INDEXER_DIGEST"),
        "the image is pinned by digest"
    );
    assert!(
        compose.contains("command: [\"serve\"]"),
        "one long-running serve"
    );
    for forbidden in [
        "privileged",
        "network_mode: host",
        "pid: host",
        "cap_add",
        "docker.sock",
        "ports:",
        "AWS_",
        "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP",
        "OPENLORE_INDEXER_REPO_DIDS:",
    ] {
        assert!(
            !compose.contains(forbidden),
            "compose must not contain {forbidden}"
        );
    }
    for (var, value) in [
        ("OPENLORE_INDEXER_REPO_DIDS_FILE", "/config/repo-dids"),
        (
            "OPENLORE_INDEXER_CONTROL_SOCKET",
            "/tmp/openlore-indexer.sock",
        ),
        ("OPENLORE_INDEXER_PURGE_UNLISTED", "1"),
        ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", "48"),
        ("OPENLORE_INDEXER_DUCKDB_THREADS", "1"),
        ("OPENLORE_INDEXER_PASS_DEADLINE_SECS", "1500"),
        ("OPENLORE_INDEXER_INDEX_PATH", "/data/index.duckdb"),
    ] {
        assert_eq!(yaml_value(&compose, var).as_deref(), Some(value), "{var}");
    }
}

/// XP-2 @US-IXD-006 @AC-006.4 @AC-006.5 @B11 @DV-IXD-14 @infrastructure @contract-shape:pure-function
#[test]
fn the_review_app_container_is_capped_for_sharing_the_host() {
    let compose = read("deploy/review-app/host/compose.yaml");
    assert_eq!(
        yaml_value(&compose, "init").as_deref(),
        Some("true"),
        "init: true"
    );
    assert_eq!(yaml_value(&compose, "mem_limit").as_deref(), Some("192m"));
    assert_eq!(
        yaml_value(&compose, "REVIEW_DB_MEMORY_LIMIT_MB").as_deref(),
        Some("48")
    );
    assert_eq!(
        yaml_value(&compose, "REVIEW_DB_THREADS").as_deref(),
        Some("1")
    );
    assert_eq!(
        yaml_value(&compose, "OPENLORE_REVIEW_SCAN_CONCURRENCY").as_deref(),
        Some("2"),
        "the scan concurrency is set explicitly (fail-path knob, fix-go-live-runbook-gaps S6)"
    );
    let oom: i64 = yaml_value(&compose, "oom_score_adj")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    assert!(
        (800..900).contains(&oom),
        "the review app dies after the indexer, before the PDS: {oom}"
    );
}

// =============================================================================
// Caddy (infrastructure-integration §5; ADR-083 §1)
// =============================================================================

/// XP-3 @US-IXD-001 @AC-001.2 @AC-001.3 @NFR-IXD-7 @ADR-083 @infrastructure @contract-shape:pure-function
#[test]
fn the_public_site_serves_only_search_and_health_over_https() {
    let caddy = read("deploy/indexer/host/index.caddy");
    let flat = compact(&caddy);
    assert!(
        caddy.contains("index.{$PDS_HOSTNAME}"),
        "the index.<host> site"
    );
    assert!(
        !caddy.contains("http://"),
        "no plain-HTTP site address (automatic HTTPS redirects)"
    );
    assert!(
        !caddy.contains("auto_https off"),
        "automatic HTTPS stays on"
    );
    assert!(flat.contains("header -Server"), "no Server header");
    assert!(flat.contains("max_size 8KB"), "8 KB body cap");
    assert_eq!(
        caddy.matches("reverse_proxy").count(),
        2,
        "exactly two proxied routes"
    );
    assert!(
        flat.contains("method POST path /xrpc/org.openlore.appview.searchClaims"),
        "POST search"
    );
    assert!(flat.contains("method GET path /healthz"), "GET /healthz");
    assert!(flat.contains("respond 404"), "everything else is 404");
    assert!(
        flat.contains("502, 503, 504") || flat.contains("502 503 504"),
        "only proxy failures become the 503 page (413 stays 413)"
    );
}

// =============================================================================
// Schedule (infrastructure-integration §3)
// =============================================================================

/// XP-4 @US-IXD-002 @AC-002.4 @AC-002.5 @FR-IXD-3 @DV-IXD-4 @infrastructure @contract-shape:pure-function
#[test]
fn the_pass_runs_every_15_minutes_never_stacks_and_survives_a_reboot() {
    let timer = read("deploy/indexer/host/openlore-indexer-pass.timer");
    let service = read("deploy/indexer/host/openlore-indexer-pass.service");
    assert!(
        timer.contains("OnCalendar=*-*-* *:00/15:00"),
        "aligned to :00/:15/:30/:45"
    );
    assert!(
        timer.contains("Persistent=true"),
        "a slot missed while down runs at boot"
    );
    assert!(
        service.contains("Type=oneshot"),
        "oneshot: a timer elapse during a pass merges, never stacks"
    );
    assert!(
        service.contains("ExecStartPre=/pds/indexer/bin/render-dids.sh"),
        "render the list before every pass"
    );
    assert!(
        service
            .contains("ExecStart=/usr/bin/docker exec openlore-indexer openlore-indexer trigger"),
        "trigger one pass inside serve"
    );
    assert!(
        !service.contains("SuccessExitStatus"),
        "exit 2/3/4 fail the unit (visible)"
    );
    let minutes: u64 = service
        .lines()
        .find_map(|l| l.trim().strip_prefix("TimeoutStartSec="))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.trim_end_matches("min").parse().ok())
        .expect("TimeoutStartSec=<n>min");
    assert!(
        minutes >= 26,
        "the unit outlives the 25-minute pass deadline: {minutes} min"
    );
    let health = read("deploy/indexer/host/openlore-indexer-health.timer");
    assert!(
        health.contains("OnCalendar="),
        "the liveness health timer is scheduled"
    );
}

// =============================================================================
// render-dids.sh (infrastructure-integration §4; ADR-081; H1)
// =============================================================================

struct Render {
    stubs: Stubs,
    config: tempfile::TempDir,
    value: PathBuf,
}

impl Render {
    /// A config directory and stub `aws` (answers the parameter from a file,
    /// or fails when the file is absent), `timeout` and `chown`.
    fn new() -> Self {
        let stubs = Stubs::new();
        let value = stubs.dir.path().join("ssm-value");
        stubs.add(
            "aws",
            &format!(
                "if [ \"$1 $2\" = \"ssm get-parameter\" ]; then [ -f '{v}' ] || exit 254; cat '{v}'; echo; exit 0; fi\nexit 0",
                v = value.display()
            ),
        );
        stubs.add("timeout", "shift; exec \"$@\"");
        stubs.add("chown", "exit 0");
        Self {
            stubs,
            config: tempfile::tempdir().expect("config dir"),
            value,
        }
    }

    fn parameter_is(&self, text: &str) {
        std::fs::write(&self.value, text).expect("ssm value");
    }

    fn parameter_unreadable(&self) {
        let _ = std::fs::remove_file(&self.value);
    }

    fn run(&self) -> Run {
        bash(
            "deploy/indexer/host/render-dids.sh",
            &[],
            &[
                ("PATH", self.stubs.path_env()),
                (
                    "INDEXER_CONFIG_DIR",
                    self.config.path().display().to_string(),
                ),
            ],
        )
    }

    fn list(&self) -> PathBuf {
        self.config.path().join("repo-dids")
    }
}

#[cfg(unix)]
fn inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).expect("metadata").ino()
}

/// XP-5 @US-IXD-003 @AC-003.1 @H1 @ADR-081 @DV-IXD-5 @infrastructure @real-io @contract-shape:bounded-change
#[cfg(unix)]
#[test]
fn two_consecutive_list_edits_replace_the_file_and_never_the_directory() {
    use std::os::unix::fs::PermissionsExt;
    let r = Render::new();
    let dir_inode = inode(r.config.path());

    r.parameter_is("did:plc:priyaraman7x2k,did:plc:dvolkov3m9q");
    let first = r.run();
    let first_inode = inode(&r.list());
    r.parameter_is("did:plc:priyaraman7x2k,did:plc:dvolkov3m9q,did:plc:therrera2v6w");
    let second = r.run();

    assert_eq!(first.status, 0, "{}", first.out);
    assert_eq!(second.status, 0, "{}", second.out);
    assert_eq!(
        std::fs::read_to_string(r.list()).expect("list").trim(),
        "did:plc:priyaraman7x2k,did:plc:dvolkov3m9q,did:plc:therrera2v6w"
    );
    assert_eq!(
        inode(r.config.path()),
        dir_inode,
        "the directory is never swapped (the container pins it)"
    );
    assert_ne!(
        inode(&r.list()),
        first_inode,
        "the FILE is replaced by rename"
    );
    assert_eq!(
        std::fs::metadata(r.list())
            .expect("meta")
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
    assert!(
        r.config.path().join(".rendered-at").exists(),
        ".rendered-at touched"
    );
    assert!(
        !r.config.path().join(".repo-dids.new").exists(),
        "no staging file left"
    );
    assert!(
        !second.out.contains("did:plc:"),
        "the list value is never printed: {}",
        second.out
    );
}

/// XP-6 @US-IXD-003 @AC-003.4 @FR-IXD-7 @ADR-081 @C7a @error @infrastructure @real-io
/// @contract-shape:unbounded-preservation
#[cfg(unix)]
#[test]
fn a_failed_or_empty_read_keeps_the_last_good_list_and_never_fails_the_pass() {
    let r = Render::new();
    r.parameter_is("did:plc:priyaraman7x2k,did:plc:dvolkov3m9q");
    assert_eq!(r.run().status, 0);
    let good = std::fs::read(r.list()).expect("last good list");

    for (label, arrange) in [
        (
            "parameter unreadable",
            Box::new(|r: &Render| r.parameter_unreadable()) as Box<dyn Fn(&Render)>,
        ),
        ("parameter empty", Box::new(|r: &Render| r.parameter_is(""))),
    ] {
        arrange(&r);
        let run = r.run();

        assert_eq!(
            run.status, 0,
            "[{label}] never fails the pass unit\n{}",
            run.out
        );
        assert_eq!(
            std::fs::read(r.list()).expect("list"),
            good,
            "[{label}] byte-identical last good list"
        );
        assert!(
            run.out.contains("indexer.dids.render_failed"),
            "[{label}] logs render_failed\n{}",
            run.out
        );
        assert!(
            !run.out.contains("did:plc:"),
            "[{label}] no value in the log\n{}",
            run.out
        );
        assert!(
            !r.config.path().join(".repo-dids.new").exists(),
            "[{label}] staging removed"
        );
    }
}

// =============================================================================
// review-app render-secrets.sh (bluesky-claim-review-app infrastructure-integration §5; D3)
// =============================================================================

/// A secrets directory inside its own parent (so siblings can be inspected) and a stub `aws`
/// whose `get-parameters-by-path` prints the names in `names` and whose `get-parameter` prints
/// `values/<last segment>` (nothing when absent). `chown` is a no-op stub.
struct Secrets {
    stubs: Stubs,
    parent: tempfile::TempDir,
    names: PathBuf,
    values: PathBuf,
}

impl Secrets {
    fn new() -> Self {
        let stubs = Stubs::new();
        let names = stubs.dir.path().join("names");
        let values = stubs.dir.path().join("values");
        std::fs::create_dir(&values).expect("values dir");
        stubs.add(
            "aws",
            &format!(
                "if [ \"$1 $2\" = \"ssm get-parameters-by-path\" ]; then cat '{n}'; exit 0; fi\n\
                 if [ \"$1 $2\" = \"ssm get-parameter\" ]; then\n\
                 while [ $# -gt 0 ]; do if [ \"$1\" = --name ]; then n=\"$2\"; fi; shift; done\n\
                 f='{v}'/\"${{n##*/}}\"\n\
                 if [ -f \"$f.fail\" ]; then echo 'An error occurred (ThrottlingException)' >&2; exit 254; fi\n\
                 if [ -f \"$f\" ]; then cat \"$f\"; fi; exit 0; fi\nexit 0",
                n = names.display(),
                v = values.display()
            ),
        );
        stubs.add("chown", "exit 0");
        let parent = tempfile::tempdir().expect("secrets parent");
        std::fs::create_dir(parent.path().join("secrets")).expect("secrets dir");
        Self {
            stubs,
            parent,
            names,
            values,
        }
    }

    /// SSM holds exactly these `(last segment, value)` parameters.
    fn parameters_are(&self, parameters: &[(&str, &str)]) {
        let _ = std::fs::remove_dir_all(&self.values);
        std::fs::create_dir(&self.values).expect("values dir");
        let names: Vec<String> = parameters
            .iter()
            .map(|(name, _)| format!("/openlore/test/review-app/{name}"))
            .collect();
        std::fs::write(&self.names, names.join("\t")).expect("names");
        for (name, value) in parameters {
            std::fs::write(self.values.join(name), value).expect("value");
        }
    }

    /// `aws ssm get-parameter` for this parameter exits non-zero (until the next
    /// `parameters_are`).
    fn reading_fails_for(&self, name: &str) {
        std::fs::write(self.values.join(format!("{name}.fail")), "").expect("fail marker");
    }

    fn dir(&self) -> PathBuf {
        self.parent.path().join("secrets")
    }

    fn run(&self) -> Run {
        bash(
            "deploy/review-app/host/render-secrets.sh",
            &[],
            &[
                ("PATH", self.stubs.path_env()),
                ("REVIEW_APP_SECRETS_DIR", self.dir().display().to_string()),
                (
                    "REVIEW_APP_SSM_PATH",
                    "/openlore/test/review-app/".to_string(),
                ),
            ],
        )
    }

    /// Every entry of a directory (dotfiles included) mapped to its bytes.
    fn listing(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir)
            .expect("read dir")
            .map(|entry| {
                let entry = entry.expect("entry");
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path()).unwrap_or_default(),
                )
            })
            .collect()
    }
}

/// XP-21 @D3 @infrastructure @real-io @contract-shape:bounded-change
#[cfg(unix)]
#[test]
fn two_renders_replace_secret_files_and_never_the_directory() {
    use std::os::unix::fs::PermissionsExt;
    let s = Secrets::new();
    let dir_inode = inode(&s.dir());

    s.parameters_are(&[
        ("client-jwk", "sentinel-client-jwk-one"),
        ("data-key", "sentinel-data-key-one"),
        ("github-token", "sentinel-github-token-one"),
        ("log-salt", "sentinel-log-salt-one"),
        ("retired-token", "sentinel-retired-token-one"),
    ]);
    let first = s.run();
    assert_eq!(first.status, 0, "{}", first.out);
    let first_inodes: Vec<u64> = ["client-jwk", "data-key", "github-token", "log-salt"]
        .iter()
        .map(|name| inode(&s.dir().join(name)))
        .collect();

    s.parameters_are(&[
        ("client-jwk", "sentinel-client-jwk-two"),
        ("data-key", "sentinel-data-key-two"),
        ("github-token", "sentinel-github-token-two"),
        ("log-salt", "sentinel-log-salt-two"),
    ]);
    let second = s.run();
    assert_eq!(second.status, 0, "{}", second.out);

    assert_eq!(
        inode(&s.dir()),
        dir_inode,
        "the secrets directory is never swapped (the container pins it)"
    );
    let expected: std::collections::BTreeMap<String, Vec<u8>> = [
        ("client-jwk", "sentinel-client-jwk-two"),
        ("data-key", "sentinel-data-key-two"),
        ("github-token", "sentinel-github-token-two"),
        ("log-salt", "sentinel-log-salt-two"),
    ]
    .iter()
    .map(|(name, value)| (name.to_string(), value.as_bytes().to_vec()))
    .collect();
    assert_eq!(
        Secrets::listing(&s.dir()),
        expected,
        "new contents, the stale parameter removed, no staging or temp file left"
    );
    for (name, first_inode) in ["client-jwk", "data-key", "github-token", "log-salt"]
        .iter()
        .zip(first_inodes)
    {
        let path = s.dir().join(name);
        assert_ne!(inode(&path), first_inode, "{name}: the FILE is replaced");
        assert_eq!(
            std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777,
            0o400,
            "{name}: mode 0400"
        );
    }
    let siblings: Vec<String> = std::fs::read_dir(s.parent.path())
        .expect("parent")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        siblings,
        vec!["secrets".to_string()],
        "no .secrets.old / .secrets.new.* sibling"
    );
    for run in [&first, &second] {
        assert!(
            !run.out.contains("sentinel"),
            "no secret value is printed: {}",
            run.out
        );
    }
}

/// XP-22 @D3 @error @infrastructure @real-io @contract-shape:unbounded-preservation
#[cfg(unix)]
#[test]
fn a_render_missing_a_required_secret_changes_nothing() {
    let s = Secrets::new();
    s.parameters_are(&[
        ("client-jwk", "sentinel-client-jwk-one"),
        ("data-key", "sentinel-data-key-one"),
        ("github-token", "sentinel-github-token-one"),
        ("log-salt", "sentinel-log-salt-one"),
    ]);
    let good = s.run();
    assert_eq!(good.status, 0, "{}", good.out);
    std::fs::write(s.dir().join(".operator-note"), "kept").expect("dotfile");
    let dir_inode = inode(&s.dir());
    let before = Secrets::listing(&s.dir());

    for (label, parameters, failing_read) in [
        (
            "log-salt missing",
            [
                ("client-jwk", "sentinel-client-jwk-two"),
                ("data-key", "sentinel-data-key-two"),
                ("github-token", "sentinel-github-token-two"),
                ("extra", "sentinel-extra-two"),
            ],
            None,
        ),
        (
            "data-key empty",
            [
                ("client-jwk", "sentinel-client-jwk-two"),
                ("data-key", ""),
                ("github-token", "sentinel-github-token-two"),
                ("log-salt", "sentinel-log-salt-two"),
            ],
            None,
        ),
        (
            "get-parameter fails on the 2nd parameter",
            [
                ("client-jwk", "sentinel-client-jwk-two"),
                ("data-key", "sentinel-data-key-two"),
                ("github-token", "sentinel-github-token-two"),
                ("log-salt", "sentinel-log-salt-two"),
            ],
            Some("data-key"),
        ),
    ] {
        s.parameters_are(&parameters);
        if let Some(name) = failing_read {
            s.reading_fails_for(name);
        }
        let run = s.run();

        assert_ne!(run.status, 0, "[{label}] the render fails\n{}", run.out);
        assert_eq!(
            Secrets::listing(&s.dir()),
            before,
            "[{label}] the full directory listing is unchanged"
        );
        assert_eq!(inode(&s.dir()), dir_inode, "[{label}] same directory");
        let temp_files: Vec<String> = Secrets::listing(&s.dir())
            .into_keys()
            .filter(|name| name.ends_with(".new"))
            .collect();
        assert!(
            temp_files.is_empty(),
            "[{label}] no temp file remains: {temp_files:?}"
        );
        assert!(
            !run.out.contains("sentinel"),
            "[{label}] no secret value is printed: {}",
            run.out
        );
    }
}

// =============================================================================
// health-timer.sh (observability-design §4; monitoring-alerting A3)
// =============================================================================

/// One `health-timer.sh run` against stub `docker` (container running), `curl`
/// (`/healthz` ok, search answering 200), a fresh `.rendered-at`, and a stub `aws` whose
/// `filter-log-events` either finds a `pass_summary` or fails. Returns the run
/// and its `indexer.host.health` line, whitespace removed.
#[cfg(unix)]
fn health_run(filter_log_events_fails: bool) -> (Run, String) {
    let filter = if filter_log_events_fails {
        "echo 'An error occurred (AccessDeniedException) when calling the FilterLogEvents operation' >&2; exit 254"
    } else {
        FOUND_SUMMARY
    };
    health_run_with(filter, &[])
}

/// A `filter-log-events` stub answer that finds one `pass_summary`.
const FOUND_SUMMARY: &str =
    "echo '{\"events\":[{\"message\":\"{\\\"event\\\":\\\"indexer.ingest.pass_summary\\\"}\"}]}'; exit 0";

/// `health_run` with an arbitrary `filter-log-events` stub body and extra env.
#[cfg(unix)]
fn health_run_with(filter: &str, extra_env: &[(&str, String)]) -> (Run, String) {
    health_run_answering(filter, extra_env, SearchAnswer::Status("200"))
}

/// How the stub `curl` answers the health probe's canned search.
#[derive(Clone, Copy, Debug)]
enum SearchAnswer {
    /// An HTTP answer with this status (curl exits 0: the script calls it without `-f`).
    Status(&'static str),
    /// No HTTP answer at all: curl writes `000` and exits with this code
    /// (7 = could not connect, 28 = timed out).
    NoAnswer(&'static str),
}

/// The stub `curl`: `/healthz` answers ok; the search honours `-w` by
/// substituting `%{http_code}` and `%{time_total}` (0.012 s) into the format,
/// and `-f` by exiting 22 for a status of 400 or more (as real curl does).
const CURL_STUB: &str = "case \"$*\" in\n  *healthz*) echo '{\"status\":\"ok\",\"last_successful_pass_at\":null}'; exit 0 ;;\nesac\nfmt=''\nfail=0\nwhile [ $# -gt 0 ]; do\n  case \"$1\" in\n    -w) fmt=\"$2\" ;;\n    --*) ;;\n    -*f*) fail=1 ;;\n  esac\n  shift\ndone\nfmt=\"${fmt//%\\{http_code\\}/@STATUS@}\"\nfmt=\"${fmt//%\\{time_total\\}/0.012}\"\nprintf '%s' \"$fmt\"\nif [ \"$fail\" = 1 ] && [ @STATUS@ -ge 400 ]; then exit 22; fi\nexit @EXIT@";

/// `health_run_with` whose canned search gets `search` from the stub `curl`.
#[cfg(unix)]
fn health_run_answering(
    filter: &str,
    extra_env: &[(&str, String)],
    search: SearchAnswer,
) -> (Run, String) {
    let (status, exit) = match search {
        SearchAnswer::Status(status) => (status, "0"),
        SearchAnswer::NoAnswer(exit) => ("000", exit),
    };
    let stubs = Stubs::new();
    stubs.add(
        "docker",
        "case \"$*\" in\n  *ps*) echo 0123456789abcdef ;;\n  *Running*) echo true ;;\n  *RestartCount*) echo 0 ;;\n  *OOMKilled*) echo false ;;\n  *stats*) echo '61MiB / 128MiB' ;;\nesac\nexit 0",
    );
    stubs.add(
        "curl",
        &CURL_STUB
            .replace("@STATUS@", status)
            .replace("@EXIT@", exit),
    );
    stubs.add(
        "aws",
        &format!("case \"$*\" in\n  *filter-log-events*) {filter} ;;\nesac\nexit 0"),
    );
    let config = tempfile::tempdir().expect("config dir");
    std::fs::write(config.path().join(".rendered-at"), "").expect(".rendered-at");
    let state = tempfile::tempdir().expect("state dir");
    let mut env = vec![
        ("PATH", stubs.path_env()),
        ("INDEXER_CONFIG_DIR", config.path().display().to_string()),
        ("INDEXER_STATE_DIR", state.path().display().to_string()),
    ];
    env.extend(extra_env.iter().cloned());
    let run = bash("deploy/indexer/host/health-timer.sh", &["run"], &env);
    let line = run
        .out
        .lines()
        .find(|l| l.contains("indexer.host.health"))
        .map(compact)
        .unwrap_or_default()
        .replace(' ', "");
    (run, line)
}

/// XP-13 @US-IXD-004 @US-IXD-005 @AC-004.1 @AC-005.1 @DV-IXD-8 @U-1 @C7a @error @infrastructure @real-io
/// @contract-shape:pure-function
/// ```gherkin
/// Scenario: The liveness line fails closed when the shipped heartbeat cannot be read
///   Given the indexer container is running, /healthz answers ok and the DID list is fresh
///   When the host health check runs and FilterLogEvents finds a pass_summary
///   Then the health line reports summary_45m 1 and not_live 0
///   When the host health check runs and FilterLogEvents fails
///   Then the health line reports summary_check error, summary_45m 0 and not_live 1
///   And the check itself still exits 0
/// ```
#[cfg(unix)]
#[test]
fn the_liveness_line_fails_closed_when_the_shipped_heartbeat_cannot_be_read() {
    let (ok, ok_line) = health_run(false);
    assert_eq!(ok.status, 0, "{}", ok.out);
    assert!(
        ok_line.contains("\"summary_45m\":1") && ok_line.contains("\"not_live\":0"),
        "non-vacuity: a found heartbeat with everything else healthy is live\n{}",
        ok.out
    );

    let (failed, line) = health_run(true);
    assert_eq!(
        failed.status, 0,
        "the health check never fails its unit\n{}",
        failed.out
    );
    for needle in [
        "\"summary_check\":\"error\"",
        "\"summary_45m\":0",
        "\"not_live\":1",
    ] {
        assert!(
            line.contains(needle),
            "a FilterLogEvents error counts as not live: missing {needle}\n{}",
            failed.out
        );
    }
}

/// XP-19 @fix-indexer-deployment-follow-ups @D2 @ADR-080-7 @error @infrastructure @real-io
/// @contract-shape:pure-function
/// ```gherkin
/// Scenario: A search that answers 500 makes the host not live
///   Given the indexer container is running, /healthz answers ok, a pass_summary was
///     shipped and the DID list is fresh
///   When the host health check runs and the canned search answers HTTP 500
///   Then the health line reports search_ok 0, search_status 500 and not_live 1
///   And the check itself still exits 0
/// ```
#[cfg(unix)]
#[test]
fn a_search_500_makes_the_host_not_live() {
    let (run, line) = health_run_answering(FOUND_SUMMARY, &[], SearchAnswer::Status("500"));
    assert_eq!(
        run.status, 0,
        "the health check never fails its unit\n{}",
        run.out
    );
    let fields = health_fields(&line);
    for (field, want) in [
        ("healthz_ok", 1),
        ("summary_45m", 1),
        ("search_ok", 0),
        ("search_status", 500),
        ("not_live", 1),
    ] {
        assert_eq!(
            fields[field],
            serde_json::json!(want),
            "a search the store cannot serve counts as not live: {field}\n{}",
            run.out
        );
    }
}

/// The health line parsed as a JSON object; an invalid line fails the test.
#[cfg(unix)]
fn health_fields(line: &str) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(serde_json::Value::Object(fields)) => fields,
        other => panic!("the health line is one JSON object: {other:?}\n{line}"),
    }
}

/// XP-20 @fix-indexer-deployment-follow-ups @D2 @ADR-080-7 @infrastructure @real-io
/// @contract-shape:pure-function
/// ```gherkin
/// Scenario: A busy, throttled or timed-out search does not make the host not live
///   Given the same healthy host
///   When the canned search answers 503, 429 or 408, or gets no HTTP answer at all
///   Then the health line reports search_ok 0 and not_live 0
///   When the canned search answers 200
///   Then the health line reports search_ok 1, search_status 200 and not_live 0
/// ```
#[cfg(unix)]
#[test]
fn a_busy_throttled_or_timed_out_search_does_not_make_the_host_not_live() {
    let table: [(SearchAnswer, i64, i64); 6] = [
        (SearchAnswer::Status("503"), 0, 503),
        (SearchAnswer::Status("429"), 0, 429),
        (SearchAnswer::Status("408"), 0, 408),
        (SearchAnswer::NoAnswer("7"), 0, 0),
        (SearchAnswer::NoAnswer("28"), 0, 0),
        (SearchAnswer::Status("200"), 1, 200),
    ];
    for (answer, search_ok, search_status) in table {
        let (run, line) = health_run_answering(FOUND_SUMMARY, &[], answer);
        assert_eq!(run.status, 0, "[{answer:?}] exits 0\n{}", run.out);
        let fields = health_fields(&line);
        for (field, want) in [
            ("search_ok", search_ok),
            ("search_status", search_status),
            ("not_live", 0),
        ] {
            assert_eq!(
                fields[field],
                serde_json::json!(want),
                "[{answer:?}] {field} in the health line\n{}",
                run.out
            );
        }
    }
}

// =============================================================================
// deploy.sh (infrastructure-integration §6.1; ci-cd-pipeline §5.7)
// =============================================================================

fn deploy_stubs(ci_conclusion: &str, signature_ok: bool) -> Stubs {
    let stubs = Stubs::new();
    stubs.add("gh", &format!("echo '{ci_conclusion}'"));
    stubs.add(
        "cosign",
        if signature_ok {
            "exit 0"
        } else {
            "echo 'no matching signatures' >&2; exit 1"
        },
    );
    stubs.add(
        "crane",
        "echo sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    stubs.add("aws", "exit 0");
    stubs.add("curl", "exit 0");
    stubs.add("docker", "exit 1");
    stubs
}

/// XP-7 @US-IXD-006 @AC-006.1 @FR-IXD-11 @DV-IXD-2 @error @infrastructure @real-io @contract-shape:unbounded-preservation
#[test]
fn a_deploy_by_tag_branch_or_short_sha_is_refused_before_any_tool_runs() {
    for reference in ["main", "latest", "v1.2.3", "4f2c1ab", "sha256:abc123"] {
        let stubs = deploy_stubs("success", true);
        let run = bash(
            "deploy/indexer/deploy.sh",
            &["deploy", reference],
            &[
                ("PATH", stubs.path_env()),
                ("HOME", stubs.dir.path().display().to_string()),
            ],
        );

        assert_ne!(run.status, 0, "{reference} is refused\n{}", run.out);
        assert!(
            run.out.contains("refusing"),
            "{reference}: says it refuses\n{}",
            run.out
        );
        assert!(
            stubs.calls().is_empty(),
            "{reference}: no tool ran\n{}",
            stubs.calls()
        );
    }
}

/// XP-8 @US-IXD-006 @AC-006.1 @AC-001.6 @FR-IXD-11 @DV-IXD-2 @error @infrastructure @real-io
/// @contract-shape:unbounded-preservation
#[test]
fn an_unsigned_digest_or_a_red_ci_sha_is_refused_before_the_host_is_touched() {
    let digest = format!("sha256:{}", "0".repeat(64));
    let unsigned = deploy_stubs("success", false);
    let run = bash(
        "deploy/indexer/deploy.sh",
        &["deploy", &digest],
        &[
            ("PATH", unsigned.path_env()),
            ("HOME", unsigned.dir.path().display().to_string()),
        ],
    );
    assert_ne!(run.status, 0, "an unsigned digest is refused\n{}", run.out);
    assert!(
        unsigned.calls().contains("cosign verify"),
        "the signature was checked\n{}",
        unsigned.calls()
    );
    assert!(
        !unsigned.calls().contains("ssm send-command"),
        "the host was never touched\n{}",
        unsigned.calls()
    );

    let sha = "4f2c1ab9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3";
    let red = deploy_stubs("failure", true);
    let run = bash(
        "deploy/indexer/deploy.sh",
        &["deploy", sha],
        &[
            ("PATH", red.path_env()),
            ("HOME", red.dir.path().display().to_string()),
        ],
    );
    assert_ne!(
        run.status, 0,
        "a sha whose CI is red is refused\n{}",
        run.out
    );
    assert!(
        red.calls().contains("gh run list"),
        "CI was checked\n{}",
        red.calls()
    );
    assert!(
        !red.calls().contains("cosign"),
        "refused before the signature step\n{}",
        red.calls()
    );
    assert!(
        !red.calls().contains("ssm send-command"),
        "the host was never touched\n{}",
        red.calls()
    );
}

// =============================================================================
// Alarms, filters, queries, IAM (monitoring-alerting §1; infrastructure-integration §7)
// =============================================================================

/// XP-9 @US-IXD-004 @AC-004.1 @AC-004.2 @AC-004.3 @AC-004.4 @AC-004.6 @DV-IXD-8 @infrastructure
/// @contract-shape:pure-function
#[test]
fn exactly_three_alarms_page_the_existing_topic_and_announce_recovery() {
    let tf = read("deploy/tofu/environments/prod/indexer.tf");
    let resources = hcl_resources(&tf);
    let alarms: Vec<_> = resources
        .iter()
        .filter(|(k, _, _)| k == "aws_cloudwatch_metric_alarm")
        .collect();
    assert_eq!(alarms.len(), 3, "exactly three alarms (A1, A2, A3)");
    assert!(
        !resources
            .iter()
            .any(|(k, _, _)| k == "aws_sns_topic" || k == "aws_sns_topic_subscription"),
        "no new topic or subscription"
    );
    for (_, name, body) in &alarms {
        let flat = compact(body);
        assert!(flat.contains("alarm_actions"), "{name}: alarm_actions");
        assert!(flat.contains("ok_actions"), "{name}: recovery is notified");
        assert!(
            body.contains("backup_alarm_topic_arn") || tf.contains("backup_alarm_topic_arn"),
            "{name}: the existing topic"
        );
        assert!(
            flat.contains("actions_enabled = var.indexer_alarms_enabled"),
            "{name}: toggle"
        );
    }
    let by_metric = |metric: &str| -> String {
        alarms
            .iter()
            .find(|(_, _, b)| b.contains(metric))
            .map(|(_, _, b)| compact(b))
            .unwrap_or_else(|| panic!("an alarm on {metric}"))
    };
    let a1 = by_metric("IndexerPassOutage");
    for want in [
        "period = 900",
        "evaluation_periods = 2",
        "datapoints_to_alarm = 2",
        "statistic = \"Minimum\"",
        "treat_missing_data = \"notBreaching\"",
    ] {
        assert!(
            a1.contains(want),
            "A1 (two consecutive exit-3 passes) needs `{want}`: {a1}"
        );
    }
    let a2 = by_metric("IndexerFailure");
    assert!(
        a2.contains("evaluation_periods = 1"),
        "A2 fires on any exit 2: {a2}"
    );
    let a3 = by_metric("IndexerNotLive");
    assert!(
        a3.contains("treat_missing_data = \"breaching\""),
        "A3 pages when nothing reports: {a3}"
    );
}

/// XP-10 @US-IXD-004 @US-IXD-005 @AC-005.1 @AC-005.2 @AC-005.3 @AC-005.4 @I-IXD-4 @infrastructure
/// @contract-shape:pure-function
#[test]
fn alarms_and_queries_read_only_the_structural_events_the_index_emits() {
    let tf = read("deploy/tofu/environments/prod/indexer.tf");
    let resources = hcl_resources(&tf);
    let filters: Vec<String> = resources
        .iter()
        .filter(|(k, _, _)| k == "aws_cloudwatch_log_metric_filter")
        .map(|(_, _, b)| b.clone())
        .collect();
    assert_eq!(filters.len(), 4, "four metric filters");
    assert!(
        filters.iter().all(|f| !f.contains("default_value")),
        "no default_value (A1 could never fire)"
    );
    let all = filters.join("\n");
    for event in [
        "indexer.ingest.pass_summary",
        "exit_code",
        "health.startup.refused",
        "indexer.store.unusable",
        "indexer.host.health",
        "not_live",
    ] {
        assert!(
            all.contains(event),
            "a filter keys on `{event}` (the binary's event contract)"
        );
    }
    let queries: Vec<String> = resources
        .iter()
        .filter(|(k, _, _)| k == "aws_cloudwatch_query_definition")
        .map(|(_, _, b)| b.clone())
        .collect();
    assert!(
        queries.len() >= 3,
        "freshness, exit codes and skips queries"
    );
    for q in &queries {
        for content in ["subject", "object", "evidence", "confidence", " value"] {
            assert!(
                !q.contains(content),
                "a query reads claim content `{content}`: {q}"
            );
        }
    }
    let param = resources
        .iter()
        .find(|(k, _, _)| k == "aws_ssm_parameter")
        .map(|(_, _, b)| compact(b))
        .expect("the DID-list parameter");
    assert!(
        param.contains("type = \"String\""),
        "a Standard String (not secret)"
    );
    assert!(
        param.contains("ignore_changes = [value]"),
        "operator edits never show as drift"
    );
}

/// XP-11 @US-IXD-003 @AC-003.5 @NFR-IXD-8 @DV-IXD-10 @infrastructure @contract-shape:pure-function
#[test]
fn the_host_may_read_the_did_list_and_its_own_logs_and_nothing_more() {
    let iam = read("deploy/tofu/bootstrap/indexer-iam.tf");
    for granted in [
        "ssm:GetParameter",
        "logs:CreateLogStream",
        "logs:PutLogEvents",
        "logs:DescribeLogStreams",
        "logs:FilterLogEvents",
        "parameter/openlore/prod/indexer",
        "log-group:/openlore/prod/indexer",
    ] {
        assert!(iam.contains(granted), "grants {granted}");
    }
    for denied in [
        "ssm:PutParameter",
        "ssm:*",
        "logs:CreateLogGroup",
        "cloudwatch:PutMetricData",
        "kms:Decrypt",
        "\"*\"",
    ] {
        assert!(!iam.contains(denied), "never grants {denied}");
    }
}

/// XP-12 @US-IXD-001 @US-IXD-006 @AC-001.6 @AC-006.1 @DV-IXD-1 @infrastructure @contract-shape:pure-function
#[test]
fn the_image_is_a_signed_non_root_distroless_build_from_ci() {
    let dockerfile = read("crates/openlore-indexer/Dockerfile");
    assert!(
        dockerfile.contains("FROM gcr.io/distroless/cc-debian12:nonroot@sha256:"),
        "distroless by digest"
    );
    assert!(dockerfile.contains("USER 65532:65532"), "non-root");
    assert!(dockerfile.contains("CMD [\"serve\"]"), "serve by default");
    assert!(
        !dockerfile.contains("\nRUN "),
        "runtime-only image (the binary is built in CI)"
    );
    let ci = read(".github/workflows/ci.yml");
    for needle in [
        "indexer-build:",
        "indexer-image:",
        "serve-smoke",
        "trivy",
        "cosign sign",
        "attest-build-provenance",
    ] {
        assert!(ci.contains(needle), "ci.yml has `{needle}`");
    }
    let host_checks = read(".github/workflows/deploy-pds-check.yml");
    for needle in ["indexer-host", "deploy/indexer/**"] {
        assert!(
            host_checks.contains(needle),
            "deploy-pds-check.yml has `{needle}`"
        );
    }
}

/// XP-0 @contract-shape:pure-function — non-vacuity of the text helpers above.
#[test]
fn the_text_helpers_read_what_they_claim() {
    let compose = "services:\n  x:\n    init: true\n    mem_limit: \"128m\" # cap\n    volumes:\n      - /a:/b\n      - \"/c:/d:ro\"\n";
    assert_eq!(yaml_value(compose, "init").as_deref(), Some("true"));
    assert_eq!(yaml_value(compose, "mem_limit").as_deref(), Some("128m"));
    assert_eq!(
        mounts(compose),
        vec!["/a:/b".to_string(), "/c:/d:ro".to_string()]
    );
    let hcl = "x\nresource \"aws_cloudwatch_metric_alarm\" \"a1\" {\n  period = 900\n}\nresource \"aws_ssm_parameter\" \"p\" {\n}\n";
    let r = hcl_resources(hcl);
    assert_eq!(r.len(), 2);
    assert_eq!(
        (r[0].0.as_str(), r[0].1.as_str()),
        ("aws_cloudwatch_metric_alarm", "a1")
    );
    assert!(compact(&r[0].2).contains("period = 900"));
}

// =============================================================================
// Review fixes (DELIVER Phase 4): H1 IMDS probe, M3 green-commit images, L2 bounded aws
// =============================================================================

/// XP-14 @US-IXD-004 @AC-004.1 @DV-IXD-8 @review-L2 @error @infrastructure @real-io
/// @contract-shape:pure-function
/// ```gherkin
/// Scenario: A hung FilterLogEvents is cut off and still counts as not live
///   Given FilterLogEvents would answer a pass_summary, but only after the aws timeout
///   When the host health check runs
///   Then the health line is still written, with summary_check error and not_live 1
/// ```
#[cfg(unix)]
#[test]
fn a_hung_heartbeat_query_is_cut_off_and_the_health_line_is_still_written() {
    let late_summary = format!("sleep 4 >/dev/null 2>&1; {FOUND_SUMMARY}");
    let started = std::time::Instant::now();
    let (run, line) = health_run_with(&late_summary, &[("INDEXER_AWS_TIMEOUT_S", "1".to_string())]);
    assert_eq!(run.status, 0, "{}", run.out);
    for needle in [
        "\"summary_check\":\"error\"",
        "\"summary_45m\":0",
        "\"not_live\":1",
    ] {
        assert!(
            line.contains(needle),
            "a FilterLogEvents that outlives its timeout fails closed: missing {needle}\n{}",
            run.out
        );
    }
    assert!(
        started.elapsed() < std::time::Duration::from_secs(4),
        "the late answer was not waited for ({:?})",
        started.elapsed()
    );
}

// =============================================================================
// Host install fails closed (fix-go-live-runbook-gaps S7, N3, N4): IMDS isolation, the PDS
// Caddy's sites mount + import, and a site Caddy proves it serves before it is reloaded.
// =============================================================================

/// One app's host side as the tests see it (literal values, never read from the scripts).
struct HostApp {
    script: &'static str,
    sites_env: &'static str,
    base_env: &'static str,
    host: &'static str,
    site_file: &'static str,
    render: &'static str,
}

const REVIEW_APP: HostApp = HostApp {
    script: "deploy/review-app/deploy.sh",
    sites_env: "REVIEW_APP_CADDY_SITES_DIR",
    base_env: "REVIEW_APP_BASE_DIR",
    host: "app.openlore.jeffbailey.us",
    site_file: "app.caddy",
    render: "render-secrets.sh",
};

const INDEXER: HostApp = HostApp {
    script: "deploy/indexer/deploy.sh",
    sites_env: "INDEXER_CADDY_SITES_DIR",
    base_env: "INDEXER_BASE_DIR",
    host: "index.openlore.jeffbailey.us",
    site_file: "index.caddy",
    render: "render-dids.sh",
};

const BOTH_APPS: [&HostApp; 2] = [&REVIEW_APP, &INDEXER];

const IMPORTING_CADDYFILE: &str =
    "{\n  admin localhost:2019\n}\n\nimport /etc/caddy/sites/*.caddy\n\nopenlore.jeffbailey.us {\n  reverse_proxy pds:3000\n}\n";
const SEEDED_SHA: &str = "4f2c1ab9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3";

fn seeded_digest() -> String {
    format!("sha256:{}", "1".repeat(64))
}

/// What the stub host answers. Every stub can fail: each field has a value that makes it.
#[derive(Clone)]
struct HostAnswers {
    /// `docker run <probe> --version` (the positive control)
    control_rc: i32,
    /// `docker run <probe> … 169.254.169.254 …` (the IMDS PUT)
    imds_rc: i32,
    /// `docker inspect` of the PDS Caddy container
    inspect_rc: i32,
    /// its Mounts as `Source:Destination` lines; `@SITES@` is the sites dir under test
    mounts: String,
    /// `/etc/caddy/Caddyfile` read inside the Caddy container (`None`: the read fails)
    caddyfile: Option<String>,
    /// `caddy adapt` output; `@HOST@` is the app's host (`None`: adapt fails)
    adapt: Option<String>,
    /// `caddy validate`
    validate_rc: i32,
    /// body of the app's render script (`exit 1` stops a deploy right after it); `@BASE@` = app base
    render: String,
    /// body of the `curl` stub
    curl: String,
    sites_exists: bool,
    releases_seeded: bool,
}

impl HostAnswers {
    fn isolated() -> Self {
        Self {
            control_rc: 0,
            imds_rc: 7,
            inspect_rc: 0,
            mounts: "/pds/caddy/data:/data\n/pds/caddy/etc/caddy:/etc/caddy\n@SITES@:/etc/caddy/sites"
                .to_string(),
            caddyfile: Some(IMPORTING_CADDYFILE.to_string()),
            adapt: Some(
                "{\"apps\":{\"http\":{\"servers\":{\"srv0\":{\"routes\":[{\"match\":[{\"host\":[\"openlore.jeffbailey.us\"]}]},{\"match\":[{\"host\":[\"@HOST@\"]}]}]}}}}}"
                    .to_string(),
            ),
            validate_rc: 0,
            render: "exit 1".to_string(),
            curl: "exit 7".to_string(),
            sites_exists: true,
            releases_seeded: true,
        }
    }
}

struct HostRun {
    run: Run,
    calls: String,
    sites: PathBuf,
    base: PathBuf,
    _keep: (Stubs, tempfile::TempDir, tempfile::TempDir),
}

impl HostRun {
    fn installed_anything(&self) -> bool {
        self.calls.lines().any(|l| l.starts_with("install "))
    }

    fn position(&self, needle: &str) -> Option<usize> {
        self.calls.lines().position(|l| l.contains(needle))
    }

    fn site_path(&self, app: &HostApp) -> PathBuf {
        self.sites.join(app.site_file)
    }
}

#[cfg(unix)]
fn write_recording_stub(path: &Path, log: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\necho \"{name} $*\" >> '{}'\n{body}\n",
            log.display()
        ),
    )
    .expect("write stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

/// `deploy.sh host <args>` for `app` against a stub host answering `answers`; `install`
/// records and succeeds (writing a placeholder for a `*.caddy` target so its removal shows).
#[cfg(unix)]
fn host_run(
    app: &HostApp,
    args: &[&str],
    answers: &HostAnswers,
    extra_env: &[(&str, String)],
) -> HostRun {
    let stubs = Stubs::new();
    let base_dir = tempfile::tempdir().expect("app base dir");
    let sites_root = tempfile::tempdir().expect("sites dir");
    let sites = if answers.sites_exists {
        sites_root.path().to_path_buf()
    } else {
        sites_root.path().join("absent")
    };
    let base = base_dir.path().to_path_buf();
    let files = stubs.dir.path().to_path_buf();
    let log = files.join("calls.log");

    std::fs::write(
        files.join("mounts.txt"),
        answers
            .mounts
            .replace("@SITES@", &sites.display().to_string()),
    )
    .expect("mounts");
    let caddyfile_answer = match &answers.caddyfile {
        Some(text) => {
            std::fs::write(files.join("Caddyfile.txt"), text).expect("Caddyfile");
            format!("cat '{}'", files.join("Caddyfile.txt").display())
        }
        None => "echo 'cat: /etc/caddy/Caddyfile: No such file' >&2; exit 1".to_string(),
    };
    let adapt_answer = match &answers.adapt {
        Some(text) => {
            std::fs::write(files.join("adapt.json"), text.replace("@HOST@", app.host))
                .expect("adapt");
            format!("cat '{}'", files.join("adapt.json").display())
        }
        None => "echo 'adapt: Caddyfile:5 - Error during parsing' >&2; exit 1".to_string(),
    };
    stubs.add(
        "docker",
        &format!(
            "case \"$*\" in\n  run*--version*) exit {control} ;;\n  run*169.254.169.254*) exit {imds} ;;\n  *' ps -q caddy'*) echo pds-caddy-1; exit 0 ;;\n  inspect*) [ {inspect} = 0 ] || exit {inspect}; cat '{mounts}'; exit 0 ;;\n  *'exec -T caddy cat /etc/caddy/Caddyfile'*) {caddyfile_answer} ;;\n  *'caddy adapt'*) {adapt_answer} ;;\n  *'caddy validate'*) exit {validate} ;;\nesac\nexit 0",
            control = answers.control_rc,
            imds = answers.imds_rc,
            inspect = answers.inspect_rc,
            mounts = files.join("mounts.txt").display(),
            validate = answers.validate_rc,
        ),
    );
    stubs.add(
        "install",
        "last=\"${!#}\"\nif [ \"$1\" != -d ]; then\n  case \"$last\" in *.caddy) printf 'site\\n' >\"$last\" ;; esac\nfi\nexit 0",
    );
    stubs.add("systemctl", "exit 0");
    stubs.add("curl", &answers.curl);

    std::fs::create_dir_all(base.join("bin")).expect("bin");
    std::fs::create_dir_all(base.join("state")).expect("state");
    write_recording_stub(
        &base.join("bin").join(app.render),
        &log,
        app.render,
        &answers
            .render
            .replace("@BASE@", &base.display().to_string()),
    );
    write_recording_stub(
        &base.join("bin/health-timer.sh"),
        &log,
        "health-timer.sh",
        "exit 0",
    );
    if answers.releases_seeded {
        std::fs::write(
            base.join("state/releases"),
            format!("2026-10-01T00:00:00Z {SEEDED_SHA} {}\n", seeded_digest()),
        )
        .expect("releases");
    }

    let mut env = vec![
        ("PATH", stubs.path_env()),
        ("HOME", files.display().to_string()),
        (app.sites_env, sites.display().to_string()),
        (app.base_env, base.display().to_string()),
    ];
    env.extend(extra_env.iter().cloned());
    let mut argv = vec!["host"];
    argv.extend_from_slice(args);
    let run = bash(app.script, &argv, &env);
    HostRun {
        run,
        calls: stubs.calls(),
        sites,
        base,
        _keep: (stubs, base_dir, sites_root),
    }
}

/// A refusal is total: non-zero, says so, and the host's state delta is empty (no `install`,
/// no Caddy reload, no site file).
#[cfg(unix)]
fn assert_refused_untouched(app: &HostApp, host: &HostRun, why: &str) {
    assert_ne!(
        host.run.status, 0,
        "{}: {why}\n{}",
        app.script, host.run.out
    );
    assert!(
        host.run.out.contains("refusing"),
        "{}: {why}: says it refuses\n{}",
        app.script,
        host.run.out
    );
    assert!(
        !host.installed_anything(),
        "{}: {why}: no host file written\n{}",
        app.script,
        host.calls
    );
    assert!(
        !host.calls.contains("caddy reload"),
        "{}: {why}: Caddy not reloaded\n{}",
        app.script,
        host.calls
    );
    assert!(
        !host.site_path(app).exists(),
        "{}: {why}: no site file left behind",
        app.script
    );
}

/// `deploy.sh host install` with a probe whose positive control exits `control_rc` and whose
/// IMDS PUT exits `imds_rc` (the Caddy checks pass).
#[cfg(unix)]
fn host_install_with_probe(
    app: &HostApp,
    control_rc: i32,
    imds_rc: i32,
    extra_env: &[(&str, String)],
) -> (Run, String) {
    let answers = HostAnswers {
        control_rc,
        imds_rc,
        ..HostAnswers::isolated()
    };
    let host = host_run(app, &["install"], &answers, extra_env);
    (host.run, host.calls)
}

#[cfg(unix)]
fn assert_the_imds_probe_fails_closed(app: &HostApp) {
    let script = app.script;
    for imds_rc in [7, 28] {
        let (run, calls) = host_install_with_probe(app, 0, imds_rc, &[]);
        assert!(
            calls.contains("install -d"),
            "{script}: curl exit {imds_rc} proves isolation; the install proceeds\n{}\n{calls}",
            run.out
        );
        let runs: Vec<&str> = calls
            .lines()
            .filter(|l| l.starts_with("docker run"))
            .collect();
        assert_eq!(
            runs.len(),
            2,
            "{script}: a positive control, then the IMDS probe\n{calls}"
        );
        assert!(
            runs[0].contains("--version"),
            "{script}: the positive control runs first\n{calls}"
        );
        for line in runs {
            assert!(
                line.contains("--network pds_default") && line.contains("curlimages/curl@sha256:"),
                "{script}: the probe runs on pds_default in a digest-pinned image: {line}"
            );
        }
    }
    for (imds_rc, why) in [
        (0, "IMDS answered: the container can reach the host role"),
        (22, "an HTTP error is still an answer from IMDS"),
        (6, "inconclusive: curl could not resolve"),
        (
            125,
            "inconclusive: docker could not pull or start the probe",
        ),
    ] {
        let (run, calls) = host_install_with_probe(app, 0, imds_rc, &[]);
        assert_ne!(run.status, 0, "{script}: {why}\n{}", run.out);
        assert!(
            run.out.contains("refusing"),
            "{script}: {why}: says it refuses\n{}",
            run.out
        );
        assert!(
            !calls.lines().any(|l| l.starts_with("install ")),
            "{script}: {why}: no host file written\n{calls}"
        );
    }
    let (run, calls) = host_install_with_probe(app, 125, 7, &[]);
    assert_ne!(
        run.status, 0,
        "{script}: a failed positive control proves nothing\n{}",
        run.out
    );
    assert!(run.out.contains("refusing"), "{script}: {}", run.out);
    assert!(
        !calls.contains("169.254.169.254") && !calls.lines().any(|l| l.starts_with("install ")),
        "{script}: refused before the IMDS probe\n{calls}"
    );
    let (run, calls) = host_install_with_probe(
        app,
        0,
        7,
        &[("IMDS_PROBE_IMAGE", "curlimages/curl:8.10.1".to_string())],
    );
    assert_ne!(
        run.status, 0,
        "{script}: a tag-pinned probe image is refused\n{}",
        run.out
    );
    assert!(run.out.contains("refusing"), "{script}: {}", run.out);
    assert!(
        !calls.contains("docker run") && !calls.lines().any(|l| l.starts_with("install ")),
        "{script}: refused before any probe ran\n{calls}"
    );
}

/// XP-15 @US-IXD-006 @AC-001.6 @C-1 @review-H1 @error @infrastructure @real-io
/// @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: The IMDS isolation probe fails closed
///   Given the probe image is pinned by digest and runs curl on pds_default
///   When the IMDS request cannot connect (7) or times out (28)
///   Then the install proceeds
///   When the IMDS request succeeds, or ends any other way, or the positive control fails,
///        or the probe image is not pinned by digest
///   Then the install is refused before any host file is written
/// ```
#[cfg(unix)]
#[test]
fn the_imds_probe_accepts_only_a_proven_unreachable_metadata_service() {
    assert_the_imds_probe_fails_closed(&INDEXER);
}

/// XP-23 @S7 @error @infrastructure @real-io @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: The review-app install runs the same IMDS isolation contract as the indexer
///   Given the review app's host install
///   When the IMDS request ends 7 or 28 after a passing positive control
///   Then the install proceeds
///   When it ends 0, 22, 6 or 125, or the control fails, or the probe image is tag-pinned
///   Then the install is refused before any `install` call
/// ```
#[cfg(unix)]
#[test]
fn the_review_app_imds_probe_accepts_only_a_proven_unreachable_metadata_service() {
    assert_the_imds_probe_fails_closed(&REVIEW_APP);
}

/// XP-24 @S7 @error @infrastructure @real-io @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: An install refuses unless the PDS Caddy bind-mounts the sites directory
///   Given the PDS Caddy container mounts <source> at <destination>
///   When either app is installed
///   Then it proceeds only for source = the sites dir and destination = /etc/caddy/sites
/// ```
#[cfg(unix)]
#[test]
fn an_install_refuses_unless_caddy_mounts_the_sites_dir() {
    for app in BOTH_APPS {
        let ok = host_run(app, &["install"], &HostAnswers::isolated(), &[]);
        assert_eq!(ok.run.status, 0, "{}: proceeds\n{}", app.script, ok.run.out);
        assert!(
            ok.calls.contains("docker inspect") && ok.calls.contains("caddy reload"),
            "{}: the mounts were inspected and the site went live\n{}",
            app.script,
            ok.calls
        );
        for (mounts, inspect_rc, why) in [
            (
                "/pds/caddy/data:/data\n/tmp/other:/etc/caddy/sites",
                0,
                "the right destination from another source",
            ),
            (
                "/pds/caddy/data:/data\n@SITES@:/srv/sites",
                0,
                "the right source at another destination",
            ),
            ("/pds/caddy/data:/data", 0, "no sites mount at all"),
            (
                "/pds/caddy/data:/data\n@SITES@:/etc/caddy/sites",
                1,
                "docker inspect failed",
            ),
        ] {
            let answers = HostAnswers {
                mounts: mounts.to_string(),
                inspect_rc,
                ..HostAnswers::isolated()
            };
            let host = host_run(app, &["install"], &answers, &[]);
            assert_refused_untouched(app, &host, why);
            assert!(
                host.run.out.contains("tofu-aws-pds v1.7.0"),
                "{}: {why}: points at the module release\n{}",
                app.script,
                host.run.out
            );
        }
    }
}

/// XP-25 @S7 @error @infrastructure @real-io @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: An install refuses unless the Caddyfile imports the sites
///   Given the Caddyfile inside the PDS Caddy container
///   When it has a line `import /etc/caddy/sites/*.caddy` (optionally indented)
///   Then the install proceeds
///   When the import is commented out, names another directory, is inside other text, is absent,
///        or the Caddyfile cannot be read
///   Then the install is refused before any host file is written
/// ```
#[cfg(unix)]
#[test]
fn an_install_refuses_unless_the_caddyfile_imports_the_sites() {
    for app in BOTH_APPS {
        for caddyfile in [
            "import /etc/caddy/sites/*.caddy\n",
            "{\n  admin localhost:2019\n}\n\t  import   /etc/caddy/sites/*.caddy\n",
        ] {
            let answers = HostAnswers {
                caddyfile: Some(caddyfile.to_string()),
                ..HostAnswers::isolated()
            };
            let host = host_run(app, &["install"], &answers, &[]);
            assert_eq!(
                host.run.status, 0,
                "{}: {caddyfile:?} imports the sites\n{}",
                app.script, host.run.out
            );
            assert!(host.calls.contains("caddy reload"), "{}", host.calls);
        }
        for (caddyfile, why) in [
            (
                Some("# import /etc/caddy/sites/*.caddy\nopenlore.jeffbailey.us {\n}\n"),
                "a commented import",
            ),
            (
                Some("import /etc/caddy/other/*.caddy\n"),
                "an import of another directory",
            ),
            (
                Some("respond \"import /etc/caddy/sites/*.caddy\"\n"),
                "the import inside other text",
            ),
            (
                Some("openlore.jeffbailey.us {\n  reverse_proxy pds:3000\n}\n"),
                "no import",
            ),
            (None, "the Caddyfile cannot be read"),
        ] {
            let answers = HostAnswers {
                caddyfile: caddyfile.map(str::to_string),
                ..HostAnswers::isolated()
            };
            let host = host_run(app, &["install"], &answers, &[]);
            assert_refused_untouched(app, &host, why);
            assert!(
                host.run.out.contains("tofu-aws-pds v1.7.0"),
                "{}: {why}: points at the module release\n{}",
                app.script,
                host.run.out
            );
        }
    }
}

/// XP-26 @S7 @error @infrastructure @real-io @contract-shape:bounded-change
/// ```gherkin
/// Scenario: A site Caddy does not serve is removed and refused
///   Given the site file is placed in the sites dir
///   When `caddy adapt` of the Caddyfile does not name the app's host (or fails)
///   Then the site file is removed, Caddy is not reloaded, and the install exits non-zero
///   When it names the host
///   Then Caddy is reloaded after adapt and validate
/// ```
#[cfg(unix)]
#[test]
fn a_site_caddy_does_not_serve_is_removed_and_refused() {
    for app in BOTH_APPS {
        for (adapt, why) in [
            (
                Some("{\"apps\":{\"http\":{\"servers\":{\"srv0\":{\"routes\":[{\"match\":[{\"host\":[\"openlore.jeffbailey.us\"]}]}]}}}}}"),
                "the adapted config serves only the PDS",
            ),
            (
                Some("{\"apps\":{\"http\":{\"servers\":{\"srv0\":{\"routes\":[{\"match\":[{\"host\":[\"x@HOST@\"]}]}]}}}}}"),
                "the adapted config serves another host containing it",
            ),
            (None, "caddy adapt failed"),
        ] {
            let answers = HostAnswers {
                adapt: adapt.map(str::to_string),
                ..HostAnswers::isolated()
            };
            let host = host_run(app, &["install"], &answers, &[]);
            assert_ne!(host.run.status, 0, "{}: {why}\n{}", app.script, host.run.out);
            assert!(
                host.calls
                    .contains(&host.site_path(app).display().to_string()),
                "{}: {why}: the site file was placed first\n{}",
                app.script,
                host.calls
            );
            assert!(
                !host.site_path(app).exists(),
                "{}: {why}: the site file is removed",
                app.script
            );
            assert!(
                !host.calls.contains("caddy reload"),
                "{}: {why}: Caddy not reloaded\n{}",
                app.script,
                host.calls
            );
        }
        let ok = host_run(app, &["install"], &HostAnswers::isolated(), &[]);
        assert_eq!(ok.run.status, 0, "{}\n{}", app.script, ok.run.out);
        let adapt = ok.position("caddy adapt").expect("adapt ran");
        let validate = ok.position("caddy validate").expect("validate ran");
        let reload = ok.position("caddy reload").expect("reload ran");
        assert!(
            adapt < reload && validate < reload,
            "{}: reload after adapt and validate\n{}",
            app.script,
            ok.calls
        );
        assert!(ok.site_path(app).exists(), "{}: the site stays", app.script);
    }
}

/// XP-27 @S7 @error @infrastructure @real-io @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: A deploy or redeploy that fails the probe leaves the running app untouched
///   Given a host with a release recorded
///   When `host deploy <digest> <sha>` or `host redeploy` runs and IMDS answers (or the import is commented)
///   Then no secrets are rendered, no image is pulled, nothing is stopped and nothing is started
/// ```
#[cfg(unix)]
#[test]
fn a_deploy_or_redeploy_that_fails_the_probe_leaves_the_running_app_untouched() {
    let digest = format!("sha256:{}", "0".repeat(64));
    for app in BOTH_APPS {
        for args in [
            vec!["deploy", digest.as_str(), SEEDED_SHA],
            vec!["redeploy"],
        ] {
            let reached = host_run(app, &args, &HostAnswers::isolated(), &[]);
            assert!(
                reached.calls.contains(app.render),
                "{} {args:?}: non-vacuity: an isolated host reaches the render\n{}\n{}",
                app.script,
                reached.run.out,
                reached.calls
            );
            for (answers, why) in [
                (
                    HostAnswers {
                        imds_rc: 0,
                        ..HostAnswers::isolated()
                    },
                    "IMDS answered",
                ),
                (
                    HostAnswers {
                        caddyfile: Some("# import /etc/caddy/sites/*.caddy\n".to_string()),
                        ..HostAnswers::isolated()
                    },
                    "the sites import is commented out",
                ),
            ] {
                let host = host_run(app, &args, &answers, &[]);
                let why = format!("{args:?}: {why}");
                assert_refused_untouched(app, &host, &why);
                for forbidden in [app.render, "docker pull"] {
                    assert!(
                        !host.calls.contains(forbidden),
                        "{}: {why}: no {forbidden}\n{}",
                        app.script,
                        host.calls
                    );
                }
                for line in host.calls.lines().filter(|l| l.contains("compose")) {
                    for verb in [" stop", " down", " up "] {
                        assert!(
                            !line.contains(verb),
                            "{}: {why}: the app is not stopped or started: {line}",
                            app.script
                        );
                    }
                }
                assert!(
                    !host.base.join(".env").exists(),
                    "{}: {why}: the running digest is unchanged",
                    app.script
                );
            }
        }
    }
}

/// XP-28 @S7 @error @infrastructure @real-io @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: The review-app install never creates the sites directory
///   Given the sites directory does not exist (tofu-aws-pds v1.7.0 not applied)
///   When the review app is installed
///   Then it refuses, and the directory still does not exist
/// ```
#[cfg(unix)]
#[test]
fn the_review_app_install_never_creates_the_sites_directory() {
    let app = &REVIEW_APP;
    let host = host_run(
        app,
        &["install"],
        &HostAnswers {
            sites_exists: false,
            ..HostAnswers::isolated()
        },
        &[],
    );
    assert_refused_untouched(app, &host, "the sites dir is missing");
    assert!(!host.sites.exists(), "the sites dir was not created");
    let ok = host_run(app, &["install"], &HostAnswers::isolated(), &[]);
    assert_eq!(ok.run.status, 0, "{}", ok.run.out);
    let sites = ok.sites.display().to_string();
    for line in ok.calls.lines() {
        assert!(
            !((line.starts_with("install -d") || line.starts_with("mkdir"))
                && line.contains(&sites)),
            "the install never creates the sites dir: {line}"
        );
    }
    for line in read(app.script).lines() {
        assert!(
            !((line.contains("install -d") || line.contains("mkdir"))
                && (line.contains("/pds/caddy/sites") || line.contains("$SITES"))),
            "no code path creates the sites dir: {line}"
        );
    }
}

/// XP-29 @N3 @error @infrastructure @real-io @contract-shape:bounded-change
/// ```gherkin
/// Scenario: A rejected app site is removed and Caddy is not reloaded
///   Given the site file is placed and Caddy serves its host
///   When `caddy validate` rejects the Caddyfile
///   Then the site file is removed, Caddy is not reloaded, and the install exits non-zero
/// ```
#[cfg(unix)]
#[test]
fn a_rejected_app_site_is_removed_and_caddy_is_not_reloaded() {
    for app in BOTH_APPS {
        let host = host_run(
            app,
            &["install"],
            &HostAnswers {
                validate_rc: 1,
                ..HostAnswers::isolated()
            },
            &[],
        );
        assert_ne!(host.run.status, 0, "{}\n{}", app.script, host.run.out);
        assert!(host.calls.contains("caddy validate"), "{}", host.calls);
        assert!(!host.calls.contains("caddy reload"), "{}", host.calls);
        assert!(
            !host.site_path(app).exists(),
            "{}: the rejected site is removed",
            app.script
        );
        let ok = host_run(app, &["install"], &HostAnswers::isolated(), &[]);
        let validate = ok.position("caddy validate").expect("validate ran");
        let reload = ok.position("caddy reload").expect("reload ran");
        assert!(
            validate < reload,
            "{}: validate before reload\n{}",
            app.script,
            ok.calls
        );
    }
}

/// The text between the shared-contract markers of a deploy script.
fn shared_isolation_contract(script: &str) -> String {
    let text = read(script);
    let start = text
        .find("# >>> shared isolation contract")
        .unwrap_or_else(|| panic!("{script}: has the shared isolation contract"));
    let end = text
        .find("# <<< shared isolation contract")
        .unwrap_or_else(|| panic!("{script}: closes the shared isolation contract"));
    text[start..end].to_string()
}

/// XP-30 @S7 @infrastructure @contract-shape:pure-function
/// ```gherkin
/// Scenario: Both apps refuse with the same isolation contract
///   Then the isolation and Caddy-site functions are identical text in both deploy scripts
///   And both pin the probe image by digest and accept only curl exit 7 or 28
///   And both key the sites dir on SITES
/// ```
#[test]
fn both_apps_refuse_with_the_same_isolation_contract() {
    let indexer = shared_isolation_contract(INDEXER.script);
    let review_app = shared_isolation_contract(REVIEW_APP.script);
    assert_eq!(indexer, review_app, "the shared isolation contract drifted");
    for needle in [
        "refuse_unless_isolated()",
        "curlimages/curl@sha256:d9b4541e214bcd85196d6e92e2753ac6d0ea699f0af5741f8c6cccbfcf00ef4b",
        "7 | 28)",
        "0 | 22)",
        "'^[[:space:]]*import[[:space:]]+/etc/caddy/sites/\\*\\.caddy'",
        "docker inspect",
        "adapt --config",
        "validate --config",
    ] {
        assert!(
            indexer.contains(needle),
            "the contract has {needle}\n{indexer}"
        );
    }
    assert!(read(REVIEW_APP.script)
        .lines()
        .any(|l| l == "SITES=\"${REVIEW_APP_CADDY_SITES_DIR:-/pds/caddy/sites}\""));
    assert!(read(INDEXER.script)
        .lines()
        .any(|l| l == "SITES=\"${INDEXER_CADDY_SITES_DIR:-/pds/caddy/sites}\""));
}

/// XP-31 @N4 @infrastructure @real-io @contract-shape:bounded-change
/// ```gherkin
/// Scenario: The first indexer deploy may wait longer for its certificate
///   Given the operator passes a readiness wait for a deploy (default 60 s)
///   When the new digest never answers /healthz
///   Then the host polls for exactly that long, and a wait that is not a number is refused
/// ```
#[cfg(unix)]
#[test]
fn the_first_indexer_deploy_may_wait_longer_for_its_certificate() {
    let digest = format!("sha256:{}", "0".repeat(64));
    let answers = HostAnswers {
        render: "mkdir -p '@BASE@/config' && echo did:plc:abc > '@BASE@/config/repo-dids'"
            .to_string(),
        releases_seeded: false,
        ..HostAnswers::isolated()
    };
    let host = host_run(
        &INDEXER,
        &["deploy", &digest, SEEDED_SHA, "4"],
        &answers,
        &[],
    );
    assert_ne!(host.run.status, 0, "never ready\n{}", host.run.out);
    let polls = host
        .calls
        .lines()
        .filter(|l| l.starts_with("curl") && l.contains("/healthz"))
        .count();
    assert_eq!(
        polls, 2,
        "a 4 s wait polls twice, 2 s apart\n{}",
        host.calls
    );

    let host = host_run(
        &INDEXER,
        &["deploy", &digest, SEEDED_SHA, "soon"],
        &answers,
        &[],
    );
    assert_ne!(host.run.status, 0, "{}", host.run.out);
    assert!(host.run.out.contains("refusing"), "{}", host.run.out);
    assert!(!host.installed_anything(), "{}", host.calls);

    let stubs = deploy_stubs("success", true);
    let run = bash(
        INDEXER.script,
        &["deploy", &digest],
        &[
            ("PATH", stubs.path_env()),
            ("HOME", stubs.dir.path().display().to_string()),
            ("INDEXER_READY_WAIT_S", "soon".to_string()),
        ],
    );
    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    assert!(
        stubs.calls().is_empty(),
        "refused before any tool ran\n{}",
        stubs.calls()
    );
}

/// Stubs for a digest deploy whose image carries `revision` (or no label when `None`) and
/// whose CI for that revision concluded `ci_conclusion`; the signature verifies.
fn digest_deploy_stubs(ci_conclusion: &str, revision: Option<&str>) -> Stubs {
    let stubs = deploy_stubs(ci_conclusion, true);
    let labels = match revision {
        Some(rev) => format!("{{\\\"org.opencontainers.image.revision\\\":\\\"{rev}\\\"}}"),
        None => "{}".to_string(),
    };
    stubs.add(
        "crane",
        &format!(
            "case \"$1\" in\n  config) echo \"{{\\\"config\\\":{{\\\"Labels\\\":{labels}}}}}\" ;;\n  *) echo sha256:{} ;;\nesac",
            "0".repeat(64)
        ),
    );
    stubs
}

/// XP-16 @US-IXD-006 @AC-006.1 @DV-IXD-2 @review-M3 @error @infrastructure @real-io
/// @contract-shape:unbounded-preservation
/// ```gherkin
/// Scenario: A digest deploy requires green CI for the commit the image was built from
///   Given a signed digest whose org.opencontainers.image.revision is a commit with red CI
///   When the operator deploys that digest (indexer or review app)
///   Then the deploy is refused before the host is touched
///   And a digest without a revision label is refused too
/// ```
#[test]
fn a_digest_deploy_requires_green_ci_for_the_images_revision() {
    let digest = format!("sha256:{}", "0".repeat(64));
    let rev = "4f2c1ab9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3";
    for script in ["deploy/indexer/deploy.sh", "deploy/review-app/deploy.sh"] {
        for (stubs, why) in [
            (
                digest_deploy_stubs("failure", Some(rev)),
                "red CI for the image's revision",
            ),
            (digest_deploy_stubs("success", None), "no revision label"),
        ] {
            let run = bash(
                script,
                &["deploy", &digest],
                &[
                    ("PATH", stubs.path_env()),
                    ("HOME", stubs.dir.path().display().to_string()),
                ],
            );
            let calls = stubs.calls();
            assert_ne!(run.status, 0, "{script}: {why} is refused\n{}", run.out);
            assert!(
                run.out.contains("refusing") || run.out.contains("not success"),
                "{script}: {why}\n{}",
                run.out
            );
            assert!(
                !calls.contains("ssm send-command"),
                "{script}: {why}: host untouched\n{calls}"
            );
        }
        let red = digest_deploy_stubs("failure", Some(rev));
        bash(
            script,
            &["deploy", &digest],
            &[
                ("PATH", red.path_env()),
                ("HOME", red.dir.path().display().to_string()),
            ],
        );
        assert!(
            red.calls().contains(&format!("--commit {rev}")),
            "{script}: CI was looked up for the image's revision\n{}",
            red.calls()
        );
    }
    // Non-vacuity (indexer): green CI for the revision gets past the CI gate to the pre-checks.
    let green = digest_deploy_stubs("success", Some(rev));
    let run = bash(
        "deploy/indexer/deploy.sh",
        &["deploy", &digest],
        &[
            ("PATH", green.path_env()),
            ("HOME", green.dir.path().display().to_string()),
        ],
    );
    assert!(
        run.out.contains("the PDS _health answers") && !green.calls().contains("ssm send-command"),
        "green CI passes the gate and stops at the stubbed pre-checks\n{}",
        run.out
    );
}

/// The `needs:` list of a top-level job in a GitHub Actions workflow.
fn job_needs(workflow: &str, job: &str) -> Vec<String> {
    let header = format!("\n  {job}:\n");
    let start = workflow
        .find(&header)
        .map(|i| i + header.len())
        .unwrap_or_else(|| panic!("job {job}"));
    workflow[start..]
        .lines()
        .take_while(|l| l.is_empty() || l.starts_with("    ") || l.trim_start().starts_with('#'))
        .find_map(|l| l.trim().strip_prefix("needs:"))
        .map(|v| {
            v.trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// XP-17 @US-IXD-006 @AC-001.6 @AC-006.1 @review-M3 @infrastructure @contract-shape:pure-function
/// An image is signed only when every test job of its commit is green: each image job
/// needs the acceptance stage AND the release guard directly (not only through its build).
#[test]
fn images_are_signed_only_for_commits_whose_tests_are_all_green() {
    let ci = read(".github/workflows/ci.yml");
    for job in ["indexer-image", "review-app-image"] {
        let needs = job_needs(&ci, job);
        for test_job in ["test", "release-guard"] {
            assert!(
                needs.iter().any(|n| n == test_job),
                "{job} needs {test_job}: {needs:?}"
            );
        }
    }
    assert_eq!(
        job_needs("jobs:\n  a:\n    name: a\n    needs: [x, y]\n  b:\n", "a"),
        vec!["x".to_string(), "y".to_string()],
        "non-vacuity of job_needs"
    );
}

// =============================================================================
// Review follow-up: the per-client rate limit sees the real client behind Caddy
// =============================================================================

/// An IPv4 address or CIDR network, read the way the indexer's
/// `OPENLORE_INDEXER_TRUSTED_PROXIES` parser reads it (a bare address is a /32).
fn ipv4_network(entry: &str) -> Option<(u32, u8)> {
    let (address, prefix) = entry.split_once('/').unwrap_or((entry, "32"));
    let address: std::net::Ipv4Addr = address.parse().ok()?;
    let prefix: u8 = prefix.parse().ok().filter(|p| *p <= 32)?;
    Some((u32::from(address), prefix))
}

/// Whether network `outer` contains every address of network `inner`.
fn network_contains(outer: (u32, u8), inner: (u32, u8)) -> bool {
    let mask = |prefix: u8| u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    inner.1 >= outer.1 && inner.0 & mask(outer.1) == outer.0 & mask(outer.1)
}

/// XP-18 @US-IXD-001 @NFR-IXD-7 @ADR-083 @review-follow-up @infrastructure @contract-shape:pure-function
/// ```gherkin
/// Scenario: The indexer trusts X-Forwarded-For only from the PDS compose network
///   Given Caddy reaches openlore-indexer over pds_default, whose subnet Docker assigns
///   When the compose sets OPENLORE_INDEXER_TRUSTED_PROXIES
///   Then every entry is an IPv4 network, every Docker default local pool is covered,
///     nothing outside the private 172.16.0.0/12 and 192.168.0.0/16 ranges is trusted,
///     and the runbook says how to narrow it to the real pds_default subnet
/// ```
#[test]
fn the_rate_limit_trusts_forwarded_clients_only_from_the_pds_network() {
    let compose = read("deploy/indexer/host/compose.yaml");
    let value = yaml_value(&compose, "OPENLORE_INDEXER_TRUSTED_PROXIES").expect(
        "compose sets OPENLORE_INDEXER_TRUSTED_PROXIES, or every client shares Caddy's bucket",
    );
    let entries: Vec<&str> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .collect();
    assert!(!entries.is_empty(), "a non-empty trusted-proxy list");
    let networks: Vec<(u32, u8)> = entries
        .iter()
        .map(|entry| ipv4_network(entry).unwrap_or_else(|| panic!("{entry} is an IPv4 network")))
        .collect();
    // Docker's default local address pools (daemon `default-address-pools`):
    // 172.17.0.0/16 .. 172.31.0.0/16, then 192.168.0.0/16 split into /20s. A
    // user-defined network like pds_default gets its subnet from one of them.
    let docker_pools = (17..=31)
        .map(|octet| format!("172.{octet}.0.0/16"))
        .chain(["192.168.0.0/16".to_string()]);
    for pool in docker_pools {
        let pool_network = ipv4_network(&pool).expect("pool");
        assert!(
            networks.iter().any(|n| network_contains(*n, pool_network)),
            "{pool} (a Docker default pool pds_default may come from) is trusted by {value}"
        );
    }
    let private_bridge_ranges = [
        ipv4_network("172.16.0.0/12").expect("range"),
        ipv4_network("192.168.0.0/16").expect("range"),
    ];
    for (entry, network) in entries.iter().zip(&networks) {
        assert!(
            private_bridge_ranges
                .iter()
                .any(|range| network_contains(*range, *network)),
            "{entry} trusts addresses outside the Docker bridge ranges"
        );
    }
    let readme = read("deploy/indexer/README.md");
    for needle in [
        "OPENLORE_INDEXER_TRUSTED_PROXIES",
        "docker network inspect pds_default",
    ] {
        assert!(readme.contains(needle), "the runbook documents {needle}");
    }
    assert!(
        network_contains(
            ipv4_network("172.16.0.0/12").expect("range"),
            ipv4_network("172.31.255.1").expect("address")
        ) && !network_contains(
            ipv4_network("172.16.0.0/12").expect("range"),
            ipv4_network("172.32.0.1").expect("address")
        ) && ipv4_network("caddy").is_none(),
        "non-vacuity of the network helpers"
    );
}
