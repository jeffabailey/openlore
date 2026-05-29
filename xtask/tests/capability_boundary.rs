//! Integration tests for the `indexer_holds_no_signing_or_local_store` rule
//! (I-AV-5 / ADR-023) added to `cargo xtask check-arch` in step 01-04.
//!
//! The `openlore-indexer` binary is the SECOND composition root and is
//! signing-INCAPABLE + holds NO local store (the capability boundary, ADR-023).
//! This is enforced STRUCTURALLY as the ABSENCE of two dep classes in the
//! indexer's transitive dependency graph:
//!
//! - the user's LOCAL STORE / `StoragePort` impl (`adapter-duckdb`), and
//! - any PDS-WRITE surface (`adapter-atproto-pds`, which carries
//!   `create_record`/`put_record`).
//!
//! The indexer MAY depend on `adapter-atproto-did` (the verify-only
//! `IdentityResolvePort` resolve path is resolve/verify-only — it does NOT
//! violate the no-signing boundary).
//!
//! The rule ALSO extends the composition-root invariant (I-3) to the second
//! axis: the `cli` crate MUST link NO HTTP server (`adapter-xrpc-query-server`)
//! and none of the indexer's store/ingest crates (`adapter-index-store`,
//! `adapter-atproto-ingest`) — the two roots wire disjoint adapter sets, neither
//! wires the other's.
//!
//! These exercise the PURE dep-graph checker
//! [`xtask::check_arch::check_indexer_capability_boundary`] against hand-rolled
//! in-memory [`Workspace`] fixtures PLUS a proptest over the forbidden /
//! permitted dep equivalence classes — proptest-based, symmetric with the
//! slice-03 autoconfirm_guard / slice-05 index-store anti-merging tests.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use xtask::check_arch::{check_indexer_capability_boundary, Workspace};

// -----------------------------------------------------------------------------
// Fixture helpers.
// -----------------------------------------------------------------------------

/// Build a `Workspace` from `(name, [deps])` rows; every listed name becomes a
/// workspace member. Mirrors the `ws` helper in `check_arch`'s own unit tests.
fn ws(rows: &[(&str, &[&str])]) -> Workspace {
    let mut members = BTreeSet::new();
    let mut deps = BTreeMap::new();
    for (name, ds) in rows {
        members.insert((*name).to_string());
        let mut set = BTreeSet::new();
        for d in *ds {
            set.insert((*d).to_string());
        }
        deps.insert((*name).to_string(), set);
    }
    Workspace { members, deps }
}

/// A COMPLIANT two-root workspace: the indexer wires only its own (verify-only +
/// read-only + index-store + query-server) adapters; the CLI wires only the
/// user's adapters + the index-query CLIENT. Neither wires the other's. MUST
/// produce zero violations.
fn compliant_workspace() -> Workspace {
    ws(&[
        (
            "openlore-indexer",
            &[
                "adapter-atproto-ingest",
                "adapter-index-store",
                "adapter-atproto-did", // verify-only resolve path — permitted
                "adapter-xrpc-query-server",
                "appview-domain",
                "ports",
            ],
        ),
        (
            "cli",
            &[
                "adapter-duckdb",
                "adapter-atproto-pds",
                "adapter-atproto-did",
                "adapter-system-clock",
                "adapter-index-query", // the CLIENT — permitted in the CLI
                "ports",
            ],
        ),
        ("adapter-atproto-ingest", &["ports"]),
        ("adapter-index-store", &["ports"]),
        ("adapter-atproto-did", &["ports"]),
        ("adapter-xrpc-query-server", &["ports"]),
        ("adapter-duckdb", &["ports"]),
        ("adapter-atproto-pds", &["ports"]),
        ("adapter-system-clock", &["ports"]),
        ("adapter-index-query", &["ports"]),
        ("appview-domain", &["ports"]),
        ("ports", &[]),
    ])
}

// -----------------------------------------------------------------------------
// Hand-written assertions.
// -----------------------------------------------------------------------------

#[test]
fn compliant_two_root_workspace_passes() {
    let v = check_indexer_capability_boundary(&compliant_workspace());
    assert!(
        v.is_empty(),
        "the compliant disjoint two-root workspace must have zero capability-\
         boundary violations, got: {v:?}"
    );
}

#[test]
fn indexer_depending_on_local_store_is_violation() {
    // The indexer reaching adapter-duckdb means it holds a local-store handle —
    // forbidden (ADR-023: the indexer never touches openlore.duckdb).
    let mut w = compliant_workspace();
    w.deps
        .get_mut("openlore-indexer")
        .unwrap()
        .insert("adapter-duckdb".to_string());
    let v = check_indexer_capability_boundary(&w);
    assert!(
        v.iter()
            .any(|x| x.package == "openlore-indexer" && x.forbidden == "adapter-duckdb"),
        "openlore-indexer → adapter-duckdb (local store) MUST be a violation, got: {v:?}"
    );
}

#[test]
fn indexer_depending_on_pds_write_surface_is_violation() {
    // adapter-atproto-pds carries create_record/put_record — a PDS-write surface
    // the signing-incapable indexer must not link (ADR-023 / I-AV-5).
    let mut w = compliant_workspace();
    w.deps
        .get_mut("openlore-indexer")
        .unwrap()
        .insert("adapter-atproto-pds".to_string());
    let v = check_indexer_capability_boundary(&w);
    assert!(
        v.iter()
            .any(|x| x.package == "openlore-indexer" && x.forbidden == "adapter-atproto-pds"),
        "openlore-indexer → adapter-atproto-pds (PDS-write) MUST be a violation, got: {v:?}"
    );
}

#[test]
fn indexer_depending_on_verify_only_did_adapter_is_allowed() {
    // The verify-only IdentityResolvePort impl lives in adapter-atproto-did,
    // which the indexer legitimately wires (resolve/verify-only, no signing). The
    // compliant fixture already has this edge; it must NOT be flagged.
    let v = check_indexer_capability_boundary(&compliant_workspace());
    assert!(
        !v.iter()
            .any(|x| x.package == "openlore-indexer" && x.forbidden == "adapter-atproto-did"),
        "the indexer's verify-only adapter-atproto-did dep must be PERMITTED \
         (resolve-only, no signing), got: {v:?}"
    );
}

#[test]
fn cli_linking_the_http_query_server_is_violation() {
    // I-3 second axis: the CLI must link NO HTTP server (the indexer's surface).
    let mut w = compliant_workspace();
    w.deps
        .get_mut("cli")
        .unwrap()
        .insert("adapter-xrpc-query-server".to_string());
    let v = check_indexer_capability_boundary(&w);
    assert!(
        v.iter()
            .any(|x| x.package == "cli" && x.forbidden == "adapter-xrpc-query-server"),
        "cli → adapter-xrpc-query-server (HTTP server) MUST be a violation (I-3), got: {v:?}"
    );
}

#[test]
fn cli_linking_an_indexer_store_or_ingest_crate_is_violation() {
    for forbidden in ["adapter-index-store", "adapter-atproto-ingest"] {
        let mut w = compliant_workspace();
        w.deps
            .get_mut("cli")
            .unwrap()
            .insert(forbidden.to_string());
        let v = check_indexer_capability_boundary(&w);
        assert!(
            v.iter().any(|x| x.package == "cli" && x.forbidden == forbidden),
            "cli → {forbidden} (an indexer-side crate) MUST be a violation (I-3), got: {v:?}"
        );
    }
}

#[test]
fn transitive_indexer_local_store_dep_is_violation() {
    // The boundary is TRANSITIVE: even if openlore-indexer reaches the local
    // store via an intermediate crate, it still holds the capability.
    let mut w = compliant_workspace();
    w.deps
        .get_mut("openlore-indexer")
        .unwrap()
        .insert("some-helper".to_string());
    w.members.insert("some-helper".to_string());
    w.deps.insert(
        "some-helper".to_string(),
        ["adapter-duckdb".to_string()].into_iter().collect(),
    );
    let v = check_indexer_capability_boundary(&w);
    assert!(
        v.iter()
            .any(|x| x.package == "openlore-indexer" && x.forbidden == "adapter-duckdb"),
        "a TRANSITIVE openlore-indexer → adapter-duckdb path MUST be a violation, got: {v:?}"
    );
}

// -----------------------------------------------------------------------------
// Property-based: forbidden vs permitted indexer-dep equivalence classes.
// -----------------------------------------------------------------------------

/// The dep names the signing-incapable, store-less indexer MUST NOT reach.
fn forbidden_indexer_dep_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("adapter-duckdb"), Just("adapter-atproto-pds")]
}

/// The dep names the indexer MAY legitimately wire (verify-only + read-only +
/// its own store/server + pure cores).
fn permitted_indexer_dep_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("adapter-atproto-did"),
        Just("adapter-atproto-ingest"),
        Just("adapter-index-store"),
        Just("adapter-xrpc-query-server"),
        Just("appview-domain"),
        Just("ports"),
    ]
}

proptest! {
    /// Property: adding ANY forbidden dep to the indexer fires the rule (the
    /// whole signing/local-store equivalence class is caught).
    #[test]
    fn any_forbidden_indexer_dep_is_rejected(forbidden in forbidden_indexer_dep_strategy()) {
        let mut w = compliant_workspace();
        w.deps
            .get_mut("openlore-indexer")
            .unwrap()
            .insert(forbidden.to_string());
        let v = check_indexer_capability_boundary(&w);
        prop_assert!(
            v.iter()
                .any(|x| x.package == "openlore-indexer" && x.forbidden == forbidden),
            "openlore-indexer → {forbidden} MUST be flagged, got: {v:?}"
        );
    }

    /// Property: the indexer wiring ONLY permitted deps never fires the rule (no
    /// false positives over the permitted equivalence class).
    #[test]
    fn indexer_with_only_permitted_deps_passes(extra in permitted_indexer_dep_strategy()) {
        let mut w = compliant_workspace();
        // The extra is already a member with a ports dep in the fixture; adding
        // the edge is idempotent and must stay clean.
        w.deps
            .get_mut("openlore-indexer")
            .unwrap()
            .insert(extra.to_string());
        let v = check_indexer_capability_boundary(&w);
        prop_assert!(
            v.is_empty(),
            "the indexer with only permitted deps (+{extra}) must pass, got: {v:?}"
        );
    }
}
