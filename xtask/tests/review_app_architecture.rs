//! bluesky-claim-review-app (DISTILL 2026-10-04) — CI-checkable
//! architecture and deployment guardrails, asserted against the REAL
//! workspace and repo files (not fixtures).
//!
//! These are the structural layer of the privacy invariants (component-
//! boundaries.md §5, ADR-072/073/074/075, DEVOPS ci-cd-pipeline §2/§5):
//!
//! * AR-1..3 capability boundary of the third composition root (dep graph)
//! * AR-4    `review-domain` is a pure core
//! * AR-5    create-only PDS writes (no delete/put/applyWrites in the OAuth adapter)
//! * AR-6    the review app holds no signing identity
//! * AR-7    every owner-table SQL literal is owner-scoped; KPI counters are not
//! * AR-8    the compose service mounts only its data + secrets and has no AWS credentials
//! * AR-9    the Caddy site serves the app over HTTPS only (AC-000.2)
//! * AR-10   tracing events in the app never use a forbidden field name
//!
//! They complement (do not replace) DELIVER's `cargo xtask check-arch` rules,
//! which are unit-tested against fixtures. All `#[ignore]`d until DELIVER
//! creates the crates / deploy files they inspect. Live deploy, rollback and
//! IMDS checks stay in the runbook (`deploy/review-app/README.md`, DEVOPS R-2/R-5).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use xtask::check_arch::load_workspace;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn rust_sources(dir: &Path) -> Vec<(PathBuf, String)> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .map(|e| {
            (
                e.path().to_path_buf(),
                std::fs::read_to_string(e.path()).unwrap_or_default(),
            )
        })
        .collect()
}

/// String literals in a Rust source (good enough for SQL scanning: plain and raw strings).
fn string_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\'') {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && !(bytes[j] == b'"' && bytes[j - 1] != b'\\') {
                j += 1;
            }
            out.push(src[start..j.min(bytes.len())].to_string());
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

const OWNER_TABLES: [&str; 7] = [
    "accounts",
    "github_links",
    "suggestions",
    "scan_runs",
    "plans",
    "oauth_sessions",
    "web_sessions",
];

/// AR-1 @ADR-072 @component-boundaries-5.1 @contract-shape:pure-function
#[test]
fn the_review_app_is_a_workspace_member_with_its_own_composition_root() {
    let ws = load_workspace().expect("cargo metadata");
    for member in [
        "openlore-review-app",
        "review-domain",
        "adapter-atproto-oauth",
        "adapter-review-store",
    ] {
        assert!(
            ws.members.contains(member),
            "{member} is a workspace member"
        );
    }
    let src = std::fs::read_to_string(root().join("xtask/src/check_arch.rs")).unwrap();
    assert!(
        src.contains("\"openlore-review-app\""),
        "COMPOSITION_ROOTS names the third root"
    );
}

/// AR-2 @ADR-072 @I-BRA-7 @component-boundaries-5.3 @contract-shape:pure-function
#[test]
fn the_review_app_cannot_reach_the_cli_or_indexer_capabilities() {
    let ws = load_workspace().expect("cargo metadata");
    assert!(
        ws.members.contains("openlore-review-app"),
        "non-vacuity: the app crate exists"
    );
    let deps = ws.transitive_deps("openlore-review-app");
    for forbidden in [
        "adapter-duckdb",
        "adapter-atproto-pds",
        "adapter-publish-http",
        "adapter-http-viewer",
        "adapter-xrpc-query-server",
        "adapter-index-store",
        "adapter-index-query",
    ] {
        assert!(
            !deps.contains(forbidden),
            "openlore-review-app must not link {forbidden}"
        );
    }
}

/// AR-3 @ADR-072 @component-boundaries-5.3 @contract-shape:pure-function
#[test]
fn only_the_review_app_reaches_the_oauth_and_private_store_adapters() {
    let ws = load_workspace().expect("cargo metadata");
    for guarded in ["adapter-atproto-oauth", "adapter-review-store"] {
        assert!(
            ws.members.contains(guarded),
            "non-vacuity: {guarded} exists"
        );
    }
    for member in &ws.members {
        if [
            "openlore-review-app",
            "xtask",
            "openlore-test-support",
            "adapter-atproto-oauth",
            "adapter-review-store",
        ]
        .contains(&member.as_str())
        {
            continue;
        }
        let direct: BTreeSet<String> = ws.deps.get(member).cloned().unwrap_or_default();
        for guarded in ["adapter-atproto-oauth", "adapter-review-store"] {
            assert!(
                !direct.contains(guarded),
                "{member} must not depend on {guarded}"
            );
        }
    }
}

/// AR-4 @ADR-007 @component-boundaries-5.2 @contract-shape:pure-function
#[test]
fn review_domain_is_a_pure_core() {
    let ws = load_workspace().expect("cargo metadata");
    assert!(
        ws.members.contains("review-domain"),
        "non-vacuity: the pure core exists"
    );
    let deps = ws.transitive_deps("review-domain");
    for io in [
        "tokio",
        "reqwest",
        "hyper",
        "duckdb",
        "atrium-oauth",
        "atrium-api",
    ] {
        assert!(!deps.contains(io), "review-domain must not depend on {io}");
    }
    assert!(
        !deps.iter().any(|d| d.starts_with("adapter-")),
        "no adapter in the pure core"
    );
}

/// AR-5 @I-BRA-8 @component-boundaries-5.5 @contract-shape:pure-function
#[test]
fn the_oauth_adapter_can_only_create_records() {
    let dir = root().join("crates/adapter-atproto-oauth/src");
    assert!(dir.is_dir(), "adapter-atproto-oauth exists");
    for (path, src) in rust_sources(&dir) {
        for verb in ["deleteRecord", "putRecord", "applyWrites"] {
            assert!(
                !src.contains(verb),
                "{} must not name {verb} (create-only)",
                path.display()
            );
        }
    }
}

/// AR-6 @ADR-072 @D-5 @component-boundaries-5.4 @contract-shape:pure-function
#[test]
fn the_review_app_holds_no_signing_identity() {
    let dir = root().join("crates/openlore-review-app/src");
    assert!(dir.is_dir(), "openlore-review-app exists");
    for (path, src) in rust_sources(&dir) {
        for token in ["IdentityPort", "keyring", "SigningKey"] {
            assert!(
                !src.contains(token),
                "{} must not name {token}",
                path.display()
            );
        }
    }
}

/// AR-7 @I-BRA-1 @ADR-074 @OD-BRA-11 @component-boundaries-5.6 @contract-shape:pure-function
#[test]
fn every_owner_table_query_is_owner_scoped_and_kpi_counters_name_no_owner() {
    let dir = root().join("crates/adapter-review-store/src");
    assert!(dir.is_dir(), "adapter-review-store exists");
    for (path, src) in rust_sources(&dir) {
        for literal in string_literals(&src) {
            let sql = literal.to_ascii_lowercase();
            let is_sql = ["select ", "insert ", "update ", "delete ", "create table"]
                .iter()
                .any(|k| sql.contains(k));
            if !is_sql {
                continue;
            }
            if sql.contains("kpi_counters") {
                assert!(
                    !sql.contains("owner_did"),
                    "{}: kpi_counters never carries an owner",
                    path.display()
                );
                continue;
            }
            if OWNER_TABLES.iter().any(|t| sql.contains(t)) && !sql.contains("create table") {
                assert!(
                    sql.contains("owner_did"),
                    "{}: owner-table SQL must filter by owner_did: {literal}",
                    path.display()
                );
            }
            if sql.contains("delete ") {
                let file = path.file_name().unwrap().to_string_lossy().to_string();
                assert!(
                    file.contains("purge") || file.contains("expiry"),
                    "DELETE only in purge/expiry modules: {}",
                    path.display()
                );
            }
        }
    }
}

/// AR-8 @DV-BRA-4 @DV-BRA-11 @infrastructure @ci-cd-pipeline-5 @contract-shape:pure-function
#[test]
fn the_app_container_mounts_only_its_data_and_secrets_and_has_no_aws_credentials() {
    let compose = std::fs::read_to_string(root().join("deploy/review-app/host/compose.yaml"))
        .expect("deploy/review-app/host/compose.yaml exists");
    let mounts: Vec<String> = compose
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("- /"))
        .map(|l| l.trim_start_matches("- ").trim_matches('"').to_string())
        .collect();
    let allowed: BTreeSet<&str> = ["/pds/app/data:/data", "/pds/app/secrets:/run/secrets:ro"]
        .into_iter()
        .collect();
    for m in &mounts {
        assert!(
            allowed.contains(m.as_str()),
            "unexpected mount {m} (no /pds, no docker.sock)"
        );
    }
    assert_eq!(
        mounts.len(),
        2,
        "exactly the data and secrets mounts: {mounts:?}"
    );
    for forbidden in [
        "privileged",
        "network_mode: host",
        "pid: host",
        "docker.sock",
        "cap_add",
        "AWS_",
    ] {
        assert!(
            !compose.contains(forbidden),
            "compose must not contain {forbidden}"
        );
    }
    for required in [
        "read_only: true",
        "mem_limit",
        "memswap_limit",
        "cap_drop",
        "no-new-privileges",
    ] {
        assert!(
            compose.contains(required),
            "compose must declare {required}"
        );
    }
}

/// AR-9 @AC-000.2 @ADR-075 @infrastructure @contract-shape:pure-function
#[test]
fn the_caddy_site_serves_the_app_over_https_only() {
    let caddy = std::fs::read_to_string(root().join("deploy/review-app/host/app.caddy"))
        .expect("deploy/review-app/host/app.caddy exists");
    assert!(caddy.contains("app."), "the app.<host> site");
    assert!(
        !caddy.contains("http://"),
        "no plain-HTTP site address (Caddy redirects HTTP to HTTPS)"
    );
    assert!(
        !caddy.contains("auto_https off"),
        "automatic HTTPS (and its HTTP→HTTPS redirect) stays on"
    );
    assert!(
        !caddy.contains("/admin"),
        "the loopback admin listener is never proxied"
    );
}

/// AR-10 @DV-BRA-9 @NFR-BRA-2 @observability-design-3 @contract-shape:pure-function
#[test]
fn tracing_events_in_the_app_never_use_a_forbidden_field_name() {
    let forbidden = [
        "token",
        "access_token",
        "refresh_token",
        "bio",
        "subject",
        "object",
        "evidence",
        "text",
        "handle",
        "did",
        "cookie",
        "code",
        "jwk",
    ];
    for dir in [
        "crates/openlore-review-app/src",
        "crates/adapter-atproto-oauth/src",
        "crates/adapter-review-store/src",
    ] {
        let dir = root().join(dir);
        assert!(dir.is_dir(), "{} exists", dir.display());
        for (path, src) in rust_sources(&dir) {
            for (n, line) in src.lines().enumerate() {
                let l = line.trim();
                let is_event = [
                    "info!(", "warn!(", "error!(", "debug!(", "trace!(", "event!(",
                ]
                .iter()
                .any(|m| l.contains(m));
                if !is_event {
                    continue;
                }
                for field in forbidden {
                    for shape in [
                        format!("{field} ="),
                        format!("{field}="),
                        format!("?{field}"),
                        format!("%{field}"),
                    ] {
                        assert!(
                            !l.contains(&format!("({shape}"))
                                && !l.contains(&format!(" {shape}"))
                                && !l.contains(&format!(",{shape}")),
                            "{}:{}: forbidden log field `{field}`: {l}",
                            path.display(),
                            n + 1
                        );
                    }
                }
            }
        }
    }
}
