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
//!
//! The shell scripts run under `bash` with stub `aws`/`gh`/`cosign`/… on PATH
//! that record every call; nothing reaches AWS, GitHub or a registry. Live
//! deploy, rollback drill, alarm test-fire and the memory gate stay in the
//! runbook (`deploy/indexer/README.md`; DEVOPS I-4..I-6).
//!
//! ## Testability contract for the scripts (DISTILL-proposed; DELIVER owns names)
//!
//! * `render-dids.sh` honours `INDEXER_CONFIG_DIR` (default `/pds/indexer/config`)
//!   and resolves `aws`, `timeout`, `chown` from `PATH`.
//! * `deploy.sh` resolves `gh`, `cosign`, `crane`, `aws`, `curl`, `docker` from `PATH`.
//! * `health-timer.sh run` honours `INDEXER_CONFIG_DIR` and `INDEXER_STATE_DIR`, resolves
//!   `docker`, `curl`, `aws` from `PATH`, prints its `indexer.host.health` line on stdout,
//!   tolerates a host without `/proc` (fields default to 0) and always exits 0.
//!
//! `#[ignore]`d until DELIVER creates the files they inspect.

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
// health-timer.sh (observability-design §4; monitoring-alerting A3)
// =============================================================================

/// One `health-timer.sh run` against stub `docker` (container running), `curl`
/// (`/healthz` ok, search ok), a fresh `.rendered-at`, and a stub `aws` whose
/// `filter-log-events` either finds a `pass_summary` or fails. Returns the run
/// and its `indexer.host.health` line, whitespace removed.
#[cfg(unix)]
fn health_run(filter_log_events_fails: bool) -> (Run, String) {
    let stubs = Stubs::new();
    stubs.add(
        "docker",
        "case \"$*\" in\n  *ps*) echo 0123456789abcdef ;;\n  *Running*) echo true ;;\n  *RestartCount*) echo 0 ;;\n  *OOMKilled*) echo false ;;\n  *stats*) echo '61MiB / 128MiB' ;;\nesac\nexit 0",
    );
    stubs.add(
        "curl",
        "case \"$*\" in\n  *healthz*) echo '{\"status\":\"ok\",\"last_successful_pass_at\":null}' ;;\n  *) echo '{\"results\":[]}' ;;\nesac\nexit 0",
    );
    let filter = if filter_log_events_fails {
        "echo 'An error occurred (AccessDeniedException) when calling the FilterLogEvents operation' >&2; exit 254"
    } else {
        "echo '{\"events\":[{\"message\":\"{\\\"event\\\":\\\"indexer.ingest.pass_summary\\\"}\"}]}'; exit 0"
    };
    stubs.add(
        "aws",
        &format!("case \"$*\" in\n  *filter-log-events*) {filter} ;;\nesac\nexit 0"),
    );
    let config = tempfile::tempdir().expect("config dir");
    std::fs::write(config.path().join(".rendered-at"), "").expect(".rendered-at");
    let state = tempfile::tempdir().expect("state dir");
    let run = bash(
        "deploy/indexer/host/health-timer.sh",
        &["run"],
        &[
            ("PATH", stubs.path_env()),
            ("INDEXER_CONFIG_DIR", config.path().display().to_string()),
            ("INDEXER_STATE_DIR", state.path().display().to_string()),
        ],
    );
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
#[ignore = "DELIVER 03-03: deploy/tofu/environments/prod/indexer.tf alarms (DV-IXD-8)"]
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
#[ignore = "DELIVER 03-03: metric filters + saved queries in indexer.tf (DV-IXD-8, observability §5)"]
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
#[ignore = "DELIVER 03-03: deploy/tofu/bootstrap/indexer-iam.tf (DV-IXD-10)"]
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
#[ignore = "DELIVER 03-03: Dockerfile + ci.yml indexer-build/indexer-image + deploy-pds-check indexer-host (DV-IXD-1)"]
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
