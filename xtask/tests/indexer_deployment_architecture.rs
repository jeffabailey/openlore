//! indexer-deployment (DISTILL 2026-10-06) — CI-checkable structural
//! guardrails of the in-`serve` pass runner, purge and public surface, asserted
//! against the REAL workspace sources (architecture-design.md §11,
//! component-boundaries.md §4):
//!
//! * XD-1 the three new rules are part of `cargo xtask check-arch`
//! * XD-2 `indexer_search_handler_read_only` — the search handler names no write or purge capability
//! * XD-3 `index_purge_only_in_pass_runner` — only the pass runner (and the composition root wiring) can purge
//! * XD-4 `index_store_delete_only_in_purge` — parent-table DELETE/DROP/TRUNCATE SQL only in `purge*.rs`
//! * XD-5 the control channel is a Unix socket, never a TCP listener (boundary rule 4)
//! * XD-6 `check-probes` sees a `probe()` on the purge port implementation
//! * XD-7 non-vacuity: the scanners see planted violations
//!
//! These complement the fixture-based unit tests of the rule functions.

use std::path::{Path, PathBuf};

use xtask::check_arch::classify_forbidden_tokens;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Non-test Rust sources under `dir`: each file's text up to its first
/// `#[cfg(test)]` (test modules sit at the end of a file by convention).
fn production_sources(dir: &str) -> Vec<(PathBuf, String)> {
    walkdir::WalkDir::new(root().join(dir))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .map(|e| {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            let production = text.split("#[cfg(test)]").next().unwrap_or("").to_string();
            (e.path().to_path_buf(), production)
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// String literals in a Rust source (plain and raw strings; enough for SQL scanning).
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

/// The two upsert child clears that stay legal outside `purge*.rs` (§11).
const CHILD_CLEARS: [&str; 2] = [
    "DELETE FROM indexed_claim_evidence WHERE cid = ?",
    "DELETE FROM indexed_claim_references WHERE referencing_cid = ?",
];

/// Pure classifier of one SQL literal in `adapter-index-store` (the shape the
/// DELIVER rule must have): a destructive statement outside a purge file is a
/// finding unless it is exactly one of the child clears.
fn destructive_sql_outside_purge(file: &str, literal: &str) -> bool {
    let upper = literal.to_ascii_uppercase();
    let destructive = ["DELETE", "DROP", "TRUNCATE"].iter().any(|kw| {
        upper
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|w| w == *kw)
    });
    destructive
        && !file.contains("purge")
        && !CHILD_CLEARS.contains(
            &literal
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .as_str(),
        )
}

/// XD-1 @architecture-design-11 @contract-shape:pure-function
#[test]
fn the_three_new_rules_are_part_of_check_arch() {
    let check_arch = std::fs::read_to_string(root().join("xtask/src/check_arch.rs"))
        .expect("read xtask/src/check_arch.rs");
    for rule in [
        "index_store_delete_only_in_purge",
        "index_purge_only_in_pass_runner",
        "indexer_search_handler_read_only",
    ] {
        assert!(
            check_arch.contains(rule),
            "`cargo xtask check-arch` reports the rule `{rule}`"
        );
    }
}

/// XD-2 @US-IXD-001 @AC-001.3 @FR-IXD-2 @DD-IXD-9 @B7 @contract-shape:pure-function
#[test]
fn the_search_handler_can_only_read_the_index() {
    let handlers: Vec<(PathBuf, String)> = production_sources("crates/openlore-indexer/src")
        .into_iter()
        .filter(|(p, _)| file_name(p).contains("search"))
        .collect();
    assert!(
        !handlers.is_empty(),
        "non-vacuity: the indexer has a search-handler module (crates/openlore-indexer/src/*search*.rs)"
    );
    for (path, src) in handlers {
        let found = classify_forbidden_tokens(
            &src,
            &["IndexStorePort", "IndexPurgePort", "upsert", "purge"],
        );
        assert!(
            found.is_empty(),
            "{}: the search handler names a write capability:\n{}",
            path.display(),
            found.join("\n")
        );
        assert!(
            src.contains("IndexReadPort"),
            "{}: the search handler holds IndexReadPort",
            path.display()
        );
    }
}

/// XD-3 @US-IXD-003 @ADR-082 @DD-IXD-6 @B6 @contract-shape:pure-function
#[test]
fn only_the_pass_runner_can_purge() {
    let sources = production_sources("crates/openlore-indexer/src");
    let runner: Vec<_> = sources
        .iter()
        .filter(|(p, _)| file_name(p).contains("runner"))
        .collect();
    assert!(
        runner.iter().any(|(_, src)| src.contains("purge_author")),
        "non-vacuity: a pass-runner module (crates/openlore-indexer/src/*runner*.rs) calls purge_author"
    );
    let composition_root = ["main.rs", "run.rs"];
    for (path, src) in &sources {
        let name = file_name(path);
        if name.contains("runner") {
            continue;
        }
        let tokens: &[&str] = if composition_root.contains(&name.as_str()) {
            &["purge_author", "plan_purge"]
        } else {
            &["IndexPurgePort", "purge_author", "plan_purge"]
        };
        let found = classify_forbidden_tokens(src, tokens);
        assert!(
            found.is_empty(),
            "{}: purge outside the pass runner:\n{}",
            path.display(),
            found.join("\n")
        );
    }
}

/// XD-4 @US-IXD-003 @ADR-082 @DD-IXD-6 @B6 @contract-shape:pure-function
#[test]
fn the_index_store_deletes_only_in_its_purge_module() {
    let sources = production_sources("crates/adapter-index-store/src");
    assert!(
        sources.iter().any(|(p, src)| file_name(p).contains("purge")
            && string_literals(src)
                .iter()
                .any(|l| l.to_ascii_uppercase().contains("DELETE"))),
        "non-vacuity: crates/adapter-index-store/src/purge.rs holds the purge DELETE"
    );
    let findings: Vec<String> = sources
        .iter()
        .flat_map(|(path, src)| {
            let name = file_name(path);
            string_literals(src)
                .into_iter()
                .filter(move |l| destructive_sql_outside_purge(&name, l))
                .map(move |l| format!("{}: {l}", path.display()))
        })
        .collect();
    assert!(
        findings.is_empty(),
        "destructive SQL outside purge*.rs:\n{}",
        findings.join("\n")
    );
}

/// XD-5 @US-IXD-002 @ADR-080 @component-boundaries-4.4 @contract-shape:pure-function
#[test]
fn the_control_channel_is_a_unix_socket_never_a_network_listener() {
    let control: Vec<_> = production_sources("crates/openlore-indexer/src")
        .into_iter()
        .filter(|(p, _)| file_name(p).contains("control"))
        .collect();
    assert!(
        control.iter().any(|(_, src)| src.contains("UnixListener")),
        "non-vacuity: a control module (crates/openlore-indexer/src/*control*.rs) binds a UnixListener"
    );
    for (path, src) in control {
        let found = classify_forbidden_tokens(&src, &["TcpListener", "TcpStream", "SocketAddr"]);
        assert!(found.is_empty(), "{}: {}", path.display(), found.join("\n"));
    }
}

/// XD-6 @ADR-082 @architecture-design-11 @contract-shape:pure-function
#[test]
fn the_purge_port_implementation_is_probed() {
    let purge = production_sources("crates/adapter-index-store/src")
        .into_iter()
        .find(|(p, _)| file_name(p).contains("purge"))
        .expect("crates/adapter-index-store/src/purge.rs exists");
    assert!(
        purge.1.contains("IndexPurgePort"),
        "purge.rs implements IndexPurgePort"
    );
    assert!(
        purge.1.contains("fn probe"),
        "check-probes needs a probe() on the purge port"
    );
}

/// XD-7 @contract-shape:pure-function — non-vacuity of the scanners used above.
#[test]
fn the_scanners_see_planted_violations() {
    assert!(destructive_sql_outside_purge(
        "lib.rs",
        "DELETE FROM indexed_claims WHERE author_did = ?"
    ));
    assert!(destructive_sql_outside_purge(
        "lib.rs",
        "drop table indexed_claims"
    ));
    assert!(!destructive_sql_outside_purge("lib.rs", CHILD_CLEARS[0]));
    assert!(!destructive_sql_outside_purge(
        "purge.rs",
        "DELETE FROM indexed_claims WHERE author_did = ?"
    ));
    assert!(!destructive_sql_outside_purge(
        "lib.rs",
        "SELECT deleted_at FROM t"
    ));
    let planted =
        "fn h(p: &dyn IndexPurgePort) { p.purge_author(x); }\n// purge_author in a comment\n";
    assert_eq!(
        classify_forbidden_tokens(planted, &["purge_author"]).len(),
        1,
        "the token scan sees code, not comments"
    );
    assert_eq!(
        string_literals(r#"let a = "DELETE FROM x"; let b = 'c';"#),
        vec!["DELETE FROM x".to_string()]
    );
}
