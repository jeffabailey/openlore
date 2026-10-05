//! indexer-per-did-pds-fetch (DISTILL 2026-10-05) — CI-checkable structural
//! guardrails of the per-DID pass, asserted against the REAL workspace sources
//! (component-boundaries.md §"check_arch changes"):
//!
//! * XA-1 `indexer_origin_only_via_listing_source` — the indexer derives a
//!   record's origin only through `appview_domain::origin_of(ListingSource)`:
//!   no `RecordOrigin::of` / `RecordOrigin::AuthorPds` in its non-test sources.
//! * XA-2 `indexer_guarded_clients_only` — the indexer wires only the SSRF-
//!   guarded constructors: no `AtProtoIngestAdapter::new` / `IdentityLookup::new`.
//! * XA-3 both rules are part of `cargo xtask check-arch` (not just tested here).
//! * XA-4 the token scanner flags a planted violation (non-vacuity guard).
//!
//! These complement DELIVER's own fixture-based unit tests of the rule
//! functions. `#[ignore]`d until DELIVER lands the rules and the rewiring.

use std::path::{Path, PathBuf};

use xtask::check_arch::classify_forbidden_tokens;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// The indexer's non-test Rust sources: each file's text up to its first
/// `#[cfg(test)]` (the test module sits at the end of a file by convention).
fn indexer_production_sources() -> Vec<(PathBuf, String)> {
    let dir = root().join("crates/openlore-indexer/src");
    let sources: Vec<(PathBuf, String)> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .map(|e| {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            let production = text.split("#[cfg(test)]").next().unwrap_or("").to_string();
            (e.path().to_path_buf(), production)
        })
        .collect();
    assert!(
        sources.iter().any(|(p, _)| p.ends_with("run.rs")),
        "non-vacuity: the indexer's run.rs is scanned (found {} files)",
        sources.len()
    );
    sources
}

fn violations(tokens: &[&str]) -> Vec<String> {
    indexer_production_sources()
        .into_iter()
        .flat_map(|(path, src)| {
            classify_forbidden_tokens(&src, tokens)
                .into_iter()
                .map(move |finding| format!("{}: {finding}", path.display()))
        })
        .collect()
}

/// XA-1 @US-IPF-003 @I-IPF-2 @ADR-077 @DD-IPF-12 @contract-shape:pure-function
#[test]
#[ignore = "DELIVER 01-02: origin only via ListingSource (indexer_origin_only_via_listing_source)"]
fn the_indexer_derives_origin_only_through_the_listing_source() {
    let found = violations(&["RecordOrigin::of", "RecordOrigin::AuthorPds"]);
    assert!(
        found.is_empty(),
        "the indexer derives origin only through appview_domain::origin_of(ListingSource) \
         (ADR-077):\n{}",
        found.join("\n")
    );
}

/// XA-2 @US-IPF-002 @AC-002.9 @DD-IPF-5 @R-IPF-7 @contract-shape:pure-function
#[test]
#[ignore = "DELIVER 02-02: guarded clients only (indexer_guarded_clients_only)"]
fn the_indexer_wires_only_address_guarded_clients() {
    let found = violations(&["AtProtoIngestAdapter::new", "IdentityLookup::new"]);
    assert!(
        found.is_empty(),
        "the indexer must wire the ::guarded constructors (ADR-077 §4 SSRF guard):\n{}",
        found.join("\n")
    );
}

/// XA-3 @DD-IPF-12 @contract-shape:pure-function
#[test]
#[ignore = "DELIVER 02-02: both rules registered in cargo xtask check-arch"]
fn both_rules_are_part_of_check_arch() {
    let check_arch = std::fs::read_to_string(root().join("xtask/src/check_arch.rs"))
        .expect("read xtask/src/check_arch.rs");
    for rule in [
        "indexer_origin_only_via_listing_source",
        "indexer_guarded_clients_only",
    ] {
        assert!(
            check_arch.contains(rule),
            "`cargo xtask check-arch` reports the rule `{rule}`"
        );
    }
}

/// XA-4 @contract-shape:pure-function — non-vacuity: the scanner sees a
/// planted violation and ignores a commented one.
#[test]
#[ignore = "DELIVER 01-02: enable with XA-1 (guards XA-1/XA-2 against vacuity)"]
fn the_token_scan_sees_a_planted_violation() {
    let planted = "fn origin() { let o = RecordOrigin::of(a, b); }\n// RecordOrigin::AuthorPds in a comment\n";
    let found =
        classify_forbidden_tokens(planted, &["RecordOrigin::of", "RecordOrigin::AuthorPds"]);
    assert_eq!(found.len(), 1, "{found:?}");
}
