//! scale-to-zero-hosting (DISTILL 2026-10-27, revised 2026-10-30) — structural
//! guardrails of the Lambda root and the self-host boundary (R-STZ-9, ADR-023,
//! D-STZ-3, the "xtask deltas" of the DESIGN component table, KPI-STZ-G6),
//! asserted against the REAL workspace graph and sources:
//!
//! * XS-1 the new rules are part of `cargo xtask check-arch`, and the Lambda
//!   root is a composition root (RED until DELIVER adds them with the first
//!   `aws-*` dependency, P-2)
//! * XS-2 the self-host `openlore-indexer` reaches no `aws-*` / `lambda_*`
//!   crate (GUARD: green today, must stay green)
//! * XS-2b it reaches no PDS write path either (GUARD, D-STZ-50)
//! * XS-3 only `adapter-s3-index-object` and `openlore-indexer-lambda` reach
//!   `aws-*` / `lambda_*` (GUARD)
//! * XS-4 the CLI links neither new crate (GUARD, the I-3 second axis)
//! * XS-5 the Lambda root holds no claim store, no PDS write path and no GitHub
//!   client (GUARD)
//! * XS-6 the Lambda search and probe modes name no write capability, and the
//!   pass mode is the one that publishes (GUARD, RED until the pass exists)
//! * XS-7 the CLI's graph, dev edges included, does not reach `openlore-indexer`
//!   (GUARD, D-STZ-45)
//! * XS-8 the CLI's manifest no longer names `openlore-indexer` (RED, D-STZ-45)
//! * XS-9 `indexer_search_handler_read_only` refuses each write capability in
//!   the Lambda read modes (RED, the extended rule)
//! * XS-10 the source scan of both indexer roots refuses a signing capability
//!   and requires a `resolve_only` site (RED, D-STZ-50)
//! * XS-11 `lambda_no_publish_is_read_only` refuses a write capability in the
//!   no-publish module (RED, D-STZ-44)
//! * XS-12 the Lambda environment holds no secret (RED until the file exists)
//!
//! The guards run at hand-off (they protect the self-host contract during
//! DELIVER). A guard over a graph with no `aws-*` crate in it yet cannot fail,
//! so each one is a function that `the_guards_catch_a_planted_violation` runs
//! over a planted graph. That is their positive control.
//!
//! XS-5 does not say "cannot sign", because a graph cannot show it. The pass
//! needs `adapter-atproto-did` to verify claims (D-STZ-3 keeps the ADR-077 path),
//! and that crate also holds the keychain path and `ed25519-dalek`; `claim-domain`
//! holds the pure `sign`. Which crates write to a PDS: `adapter-atproto-pds` and
//! `adapter-atproto-oauth`, both forbidden here. XS-10 is the source half of the
//! claim D-STZ-50 does make.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use xtask::check_arch::{
    load_workspace, scan_indexer_family_holds_no_signing, scan_indexer_search_handler_read_only,
    scan_lambda_no_publish_is_read_only, Workspace,
};

const LAMBDA_ROOT: &str = "openlore-indexer-lambda";
const LAMBDA_ADAPTER: &str = "adapter-s3-index-object";
const SELF_HOST: &str = "openlore-indexer";
const LAMBDA_FORBIDDEN: [&str; 4] = [
    "adapter-duckdb",
    "adapter-atproto-pds",
    "adapter-atproto-oauth",
    "adapter-github",
];

/// The write capabilities no read-only Lambda mode may name (D-STZ-38).
const WRITE_TOKENS: [&str; 4] = [
    "publish_if_match",
    "put_last_pass",
    "IndexObjectPort",
    "IndexObjectWritePort",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// The crates that write to a PDS.
const PDS_WRITERS: [&str; 2] = ["adapter-atproto-pds", "adapter-atproto-oauth"];

fn is_aws_or_lambda(name: &str) -> bool {
    name.starts_with("aws-") || name.starts_with("lambda_") || name.starts_with("lambda-")
}

/// A file's production code: comment lines and tests dropped.
fn code_of(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let production = text.split("#[cfg(test)]").next().unwrap_or("");
    production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of one `const NAME: … = …;`, however rustfmt wrapped it.
fn const_body<'a>(code: &'a str, name: &str) -> &'a str {
    let at = code
        .find(&format!("const {name}"))
        .unwrap_or_else(|| panic!("no const {name}"));
    let rest = &code[at..];
    &rest[..rest.find(';').expect("the const ends")]
}

// -----------------------------------------------------------------------------
// The guards, as functions of the graph
// -----------------------------------------------------------------------------

fn what_the_self_host_must_not_reach(ws: &Workspace) -> BTreeSet<String> {
    ws.transitive_deps(SELF_HOST)
        .into_iter()
        .filter(|d| is_aws_or_lambda(d) || d == LAMBDA_ROOT || d == LAMBDA_ADAPTER)
        .collect()
}

fn what_the_self_host_may_not_write_through(ws: &Workspace) -> BTreeSet<String> {
    let reached = ws.transitive_deps(SELF_HOST);
    PDS_WRITERS
        .into_iter()
        .filter(|c| reached.contains(*c))
        .map(str::to_string)
        .collect()
}

fn who_else_reaches_aws(ws: &Workspace) -> Vec<(String, String)> {
    ws.members
        .iter()
        .filter(|m| *m != LAMBDA_ROOT && *m != LAMBDA_ADAPTER)
        .flat_map(|m| {
            ws.transitive_deps(m)
                .into_iter()
                .filter(|d| is_aws_or_lambda(d))
                .map(move |d| (m.clone(), d))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn what_the_cli_must_not_reach(ws: &Workspace) -> BTreeSet<String> {
    let reached = ws.transitive_deps("cli");
    [LAMBDA_ROOT, LAMBDA_ADAPTER, SELF_HOST]
        .into_iter()
        .filter(|c| reached.contains(*c))
        .map(str::to_string)
        .collect()
}

fn what_the_lambda_root_must_not_reach(ws: &Workspace) -> BTreeSet<String> {
    let reached = ws.transitive_deps(LAMBDA_ROOT);
    LAMBDA_FORBIDDEN
        .into_iter()
        .filter(|c| reached.contains(*c))
        .map(str::to_string)
        .collect()
}

/// Not a scenario: the positive control of XS-2, XS-3, XS-4, XS-5 and XS-7. A
/// transitive edge of each forbidden kind, planted in a small graph.
#[test]
fn the_guards_catch_a_planted_violation() {
    let edges: [(&str, &[&str]); 5] = [
        ("cli", &["shared"]),
        ("shared", &[SELF_HOST, LAMBDA_ADAPTER]),
        (SELF_HOST, &["ports-ish"]),
        ("ports-ish", &["aws-config", "adapter-atproto-oauth"]),
        (LAMBDA_ROOT, &["adapter-atproto-oauth"]),
    ];
    let deps: BTreeMap<String, BTreeSet<String>> = edges
        .into_iter()
        .map(|(from, to)| {
            (
                from.to_string(),
                to.iter().map(|d| (*d).to_string()).collect(),
            )
        })
        .collect();
    let planted = Workspace {
        members: deps.keys().cloned().collect(),
        deps,
    };

    assert!(what_the_self_host_must_not_reach(&planted).contains("aws-config"));
    assert!(what_the_self_host_may_not_write_through(&planted).contains("adapter-atproto-oauth"));
    assert!(who_else_reaches_aws(&planted)
        .iter()
        .any(|(member, crate_name)| member == "cli" && crate_name == "aws-config"));
    let cli = what_the_cli_must_not_reach(&planted);
    assert!(
        cli.contains(SELF_HOST) && cli.contains(LAMBDA_ADAPTER),
        "{cli:?}"
    );
    assert!(what_the_lambda_root_must_not_reach(&planted).contains("adapter-atproto-oauth"));
}

/// Not a scenario: the real graph is the one the guards are meant to read, with
/// external crates in it and the names they look for spelled right.
#[test]
fn the_real_graph_names_what_the_guards_look_for() {
    let ws = load_workspace().expect("cargo metadata");

    for member in [SELF_HOST, LAMBDA_ROOT, LAMBDA_ADAPTER, "cli"] {
        assert!(ws.members.contains(member), "{member} is a member");
    }
    let self_host = ws.transitive_deps(SELF_HOST);
    assert!(self_host.contains("ports"), "workspace edges are read");
    assert!(self_host.contains("tokio"), "external edges are read");
    assert!(ws.transitive_deps(LAMBDA_ROOT).contains("ports"));
    assert!(ws.transitive_deps("cli").contains("adapter-index-query"));
}

/// XS-1
/// ```gherkin
/// @R-STZ-9 @D-STZ-3 @KPI-STZ-G6 @kpi @contract-shape:pure-function
/// Scenario: check-arch knows the Lambda root and its rules
///   Given the workspace sources
///   Then check-arch lists openlore-indexer-lambda as a composition root
///   And it has the rules aws_sdk_only_in_lambda_edge and indexer_lambda_holds_no_signing_or_local_store
/// ```
#[test]
fn check_arch_knows_the_lambda_root_and_its_rules() {
    let code = code_of(&root().join("xtask/src/check_arch.rs"));
    let roots = const_body(&code, "COMPOSITION_ROOTS");
    assert!(
        roots.contains("\"openlore-indexer\""),
        "read the right list"
    );
    assert!(roots.contains(&format!("\"{LAMBDA_ROOT}\"")), "{roots}");
    for rule in [
        "aws_sdk_only_in_lambda_edge",
        "indexer_lambda_holds_no_signing_or_local_store",
    ] {
        assert!(code.contains(rule), "check-arch has rule {rule}");
    }
}

/// XS-2
/// ```gherkin
/// @R-STZ-9 @ADR-023 @KPI-STZ-G6 @kpi @guard @contract-shape:unbounded-preservation
/// Scenario: The self-hosted indexer stays free of AWS and Lambda code
///   Given the workspace dependency graph, dev-dependencies included
///   Then nothing openlore-indexer reaches is an aws-* or lambda_* crate
///   And it does not reach the Lambda root or its adapter
/// ```
#[test]
fn the_self_hosted_indexer_stays_free_of_aws_and_lambda_code() {
    let ws = load_workspace().expect("cargo metadata");
    let offending = what_the_self_host_must_not_reach(&ws);
    assert!(
        offending.is_empty(),
        "openlore-indexer reaches {offending:?}"
    );
}

/// XS-2b
/// ```gherkin
/// @D-STZ-50 @ADR-023 @I-AV-5 @guard @contract-shape:unbounded-preservation
/// Scenario: The self-hosted indexer has no path to write to a PDS
///   Given the workspace dependency graph, dev-dependencies included
///   Then openlore-indexer reaches neither adapter-atproto-pds nor adapter-atproto-oauth
/// ```
#[test]
fn the_self_hosted_indexer_has_no_path_to_write_to_a_pds() {
    let ws = load_workspace().expect("cargo metadata");
    let reached = what_the_self_host_may_not_write_through(&ws);
    assert!(reached.is_empty(), "openlore-indexer reaches {reached:?}");
}

/// XS-3
/// ```gherkin
/// @D-STZ-3 @ADR-023 @guard @contract-shape:unbounded-preservation
/// Scenario: Only the Lambda root and its adapter may reach AWS
///   Given the workspace dependency graph
///   Then no other workspace member reaches an aws-* or lambda_* crate
/// ```
#[test]
fn only_the_lambda_root_and_its_adapter_may_reach_aws() {
    let ws = load_workspace().expect("cargo metadata");
    let offenders = who_else_reaches_aws(&ws);
    assert!(offenders.is_empty(), "{offenders:?}");
}

/// XS-4
/// ```gherkin
/// @ADR-023 @I-3 @guard @contract-shape:unbounded-preservation
/// Scenario: The CLI links neither the Lambda root nor its adapter
///   Given the workspace dependency graph
///   Then cli reaches neither openlore-indexer-lambda nor adapter-s3-index-object
/// ```
#[test]
fn the_cli_links_neither_the_lambda_root_nor_its_adapter() {
    let ws = load_workspace().expect("cargo metadata");
    let reached = what_the_cli_must_not_reach(&ws);
    assert!(!reached.contains(LAMBDA_ROOT), "{reached:?}");
    assert!(!reached.contains(LAMBDA_ADAPTER), "{reached:?}");
}

/// XS-5
/// ```gherkin
/// @ADR-023 @I-AV-5 @guard @contract-shape:unbounded-preservation
/// Scenario: The Lambda root holds no claim store, no PDS write path and no GitHub client
///   Given the workspace dependency graph
///   Then openlore-indexer-lambda reaches none of adapter-duckdb, adapter-atproto-pds,
///     adapter-atproto-oauth, adapter-github
/// ```
#[test]
fn the_lambda_root_holds_no_claim_store_no_pds_write_path_and_no_github_client() {
    let ws = load_workspace().expect("cargo metadata");
    let reached = what_the_lambda_root_must_not_reach(&ws);
    assert!(reached.is_empty(), "{LAMBDA_ROOT} reaches {reached:?}");
}

/// XS-6
/// ```gherkin
/// @D-STZ-3 @D-STZ-38 @ADR-083 @guard @contract-shape:unbounded-preservation
/// Scenario: Only the pass can publish the index
///   Given the Lambda root's production sources
///   Then the search and probe modes name no write capability
///   And the search mode does not read the pass record
///   And what the search and probe functions are composed with has no write port
///   And the pass mode is the one that publishes
/// ```
#[test]
fn only_the_pass_can_publish_the_index() {
    let src = root().join("crates/openlore-indexer-lambda/src");
    for mode in ["search.rs", "probe.rs"] {
        let code = code_of(&src.join(mode));
        assert!(!code.is_empty(), "{mode} exists");
        for capability in WRITE_TOKENS {
            assert!(!code.contains(capability), "{mode} names {capability}");
        }
    }
    assert!(!code_of(&src.join("search.rs")).contains("get_last_pass"));
    let lib = code_of(&src.join("lib.rs"));
    for driven in ["SearchDriven", "ProbeDriven"] {
        let at = lib
            .find(&format!("pub struct {driven}"))
            .unwrap_or_else(|| panic!("{driven} exists"));
        let body = &lib[at..at + lib[at..].find('}').expect("the struct ends")];
        assert!(body.contains("IndexObjectReadPort"), "{body}");
        assert!(!body.contains("IndexObjectWritePort"), "{body}");
    }
    assert!(
        code_of(&src.join("pass.rs")).contains("publish_if_match"),
        "the pass publishes"
    );
}

/// XS-7 (it holds today only because Cargo drops a dev-dependency on a crate with
/// no library, so XS-8 is the one that moves first)
/// ```gherkin
/// @D-STZ-45 @I-3 @UI-7 @guard @contract-shape:unbounded-preservation
/// Scenario: The CLI's graph does not reach the self-hosted indexer, dev edges included
///   Given the workspace dependency graph, dev-dependencies included
///   Then cli does not reach openlore-indexer
/// ```
#[test]
fn the_clis_graph_does_not_reach_the_self_hosted_indexer_dev_edges_included() {
    let ws = load_workspace().expect("cargo metadata");
    let reached = what_the_cli_must_not_reach(&ws);
    assert!(!reached.contains(SELF_HOST), "{reached:?}");
}

/// XS-8 (XS-7 holds today only because Cargo drops a dev-dependency on a crate
/// with no library. The lib split ends that, so the manifest has to change.)
/// ```gherkin
/// @D-STZ-45 @UI-7 @contract-shape:pure-function
/// Scenario: The CLI no longer names the self-hosted indexer as a dependency
///   Given the CLI's manifest
///   Then no dependency of any kind names openlore-indexer
/// ```
#[test]
fn the_cli_no_longer_names_the_self_hosted_indexer_as_a_dependency() {
    let manifest =
        std::fs::read_to_string(root().join("crates/cli/Cargo.toml")).expect("the cli manifest");
    let named: Vec<&str> = manifest
        .lines()
        .filter(|line| {
            line.split('=')
                .next()
                .is_some_and(|key| key.trim() == SELF_HOST)
        })
        .collect();
    assert!(named.is_empty(), "{named:?}");
}

/// A tree holding the indexer's real search handler shape, plus Lambda modes.
fn a_tree_with(search: &str, probe: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a tree");
    let write = |path: &str, text: &str| {
        let file = tree.path().join(path);
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("dirs");
        std::fs::write(file, text).expect("a file");
    };
    write(
        "crates/openlore-indexer/src/search.rs",
        "use ports::IndexReadPort;\n",
    );
    write("crates/openlore-indexer-lambda/src/search.rs", search);
    write("crates/openlore-indexer-lambda/src/probe.rs", probe);
    tree
}

const CLEAN_SEARCH: &str = "use ports::{IndexObjectReadPort, IndexReadPort};\n";
const CLEAN_PROBE: &str = "use ports::IndexObjectReadPort;\n";

/// Not a scenario: the positive control of XS-9. Clean modes raise nothing, the
/// probe is not asked for `IndexReadPort`, because it holds no store, and it may
/// read the pass record, because it alone does. Green today partly because the
/// rule does not read these modes yet, so XS-9's planted rows are what prove it
/// reads them.
#[test]
fn check_arch_accepts_clean_read_modes() {
    let probe =
        format!("{CLEAN_PROBE}fn age(p: &dyn IndexObjectReadPort) {{ p.get_last_pass(); }}\n");
    let tree = a_tree_with(CLEAN_SEARCH, &probe);
    let findings = scan_indexer_search_handler_read_only(tree.path()).expect("scan");
    assert!(findings.is_empty(), "{findings:?}");
}

/// XS-9
/// ```gherkin
/// @D-STZ-38 @D-STZ-3 @ADR-083 @error @contract-shape:pure-function
/// Scenario Outline: check-arch refuses a write capability in a read-only Lambda mode
///   Given a Lambda <mode> that names <capability>
///   When check-arch reads it
///   Then it reports the mode
///   Examples:
///     | mode   | capability                        |
///     | search | a publish                         |
///     | search | a record write                    |
///     | search | the old combined port             |
///     | search | the write port                    |
///     | probe  | a publish                         |
///     | probe  | a record write                    |
///     | probe  | the old combined port             |
///     | probe  | the write port                    |
///     | search | a read of the pass record         |
/// ```
#[test]
fn check_arch_refuses_a_write_capability_in_a_read_only_lambda_mode() {
    let naming =
        |clean: &str, capability: &str| format!("{clean}fn holds(_: &dyn {capability}) {{}}\n");
    let mut planted = Vec::new();
    for capability in WRITE_TOKENS {
        planted.push((
            "search.rs",
            capability,
            a_tree_with(&naming(CLEAN_SEARCH, capability), CLEAN_PROBE),
        ));
        planted.push((
            "probe.rs",
            capability,
            a_tree_with(CLEAN_SEARCH, &naming(CLEAN_PROBE, capability)),
        ));
    }
    planted.push((
        "search.rs",
        "get_last_pass",
        a_tree_with(&naming(CLEAN_SEARCH, "get_last_pass"), CLEAN_PROBE),
    ));
    let mut missed = Vec::new();
    for (mode, capability, tree) in &planted {
        let path = format!("crates/openlore-indexer-lambda/src/{mode}");
        let findings = scan_indexer_search_handler_read_only(tree.path()).expect("scan");
        if !findings
            .iter()
            .any(|f| f.contains(capability) && f.contains(&path))
        {
            missed.push((mode, capability));
        }
    }
    assert!(missed.is_empty(), "not refused: {missed:?}");
}

/// What the indexer family's production code may not name (D-STZ-50).
const SIGNING_TOKENS: [&str; 7] = [
    "IdentityPort",
    "for_did",
    "new_with_did_document",
    "ensure_keychain_key",
    "claim_domain::sign",
    "SigningKey",
    "keyring",
];
const INDEXER_ROOTS: [&str; 2] = [
    "crates/openlore-indexer/src",
    "crates/openlore-indexer-lambda/src",
];
const RESOLVE_ONLY_SITE: &str = "fn wire() { let _ = AtProtoDidAdapter::resolve_only(); }\n";

/// Both roots holding `first` and `second` as their one source file.
fn a_family_with(first: &str, second: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a tree");
    for (root, source) in INDEXER_ROOTS.into_iter().zip([first, second]) {
        let dir = tree.path().join(root);
        std::fs::create_dir_all(&dir).expect("dirs");
        std::fs::write(dir.join("wiring.rs"), source).expect("a file");
    }
    tree
}

fn findings_in(tree: &tempfile::TempDir) -> Vec<String> {
    scan_indexer_family_holds_no_signing(tree.path()).expect("scan")
}

/// XS-10 (D-STZ-50: a graph rule cannot show this while the verifier and the
/// signer share a crate, so it is read from what each root composes)
/// ```gherkin
/// @D-STZ-50 @UI-18 @ADR-023 @I-AV-5 @C6c @error @contract-shape:pure-function
/// Scenario Outline: check-arch refuses a signing capability in either indexer root
///   Given an indexer root whose production code <plants>
///   When check-arch reads the family
///   Then it reports that root
///   And a mention in a comment or in test code is not a finding, and the real roots have none
///   Examples:
///     | plants                                  |
///     | names the signing identity port         |
///     | builds the adapter for one DID          |
///     | builds the adapter from a DID document  |
///     | reaches for the keychain key            |
///     | calls the pure signer                   |
///     | names a signing key                     |
///     | names the keyring                       |
///     | has no verify-only construction at all  |
/// ```
#[test]
fn check_arch_refuses_a_signing_capability_in_either_indexer_root() {
    let clean = a_family_with(RESOLVE_ONLY_SITE, RESOLVE_ONLY_SITE);
    assert_eq!(findings_in(&clean), Vec::<String>::new(), "the control");

    let mut missed = Vec::new();
    for (index, root) in INDEXER_ROOTS.into_iter().enumerate() {
        let planted_in = |source: &str| {
            if index == 0 {
                a_family_with(source, RESOLVE_ONLY_SITE)
            } else {
                a_family_with(RESOLVE_ONLY_SITE, source)
            }
        };
        for token in SIGNING_TOKENS {
            let source = format!("{RESOLVE_ONLY_SITE}fn planted() {{ let _ = {token}; }}\n");
            let found = findings_in(&planted_in(&source));
            if !found.iter().any(|f| f.contains(token) && f.contains(root)) {
                missed.push((root, token));
            }
        }
        let no_site = findings_in(&planted_in("fn wire() {}\n"));
        if !no_site
            .iter()
            .any(|f| f.contains("resolve_only") && f.contains(root))
        {
            missed.push((root, "resolve_only"));
        }
        let mentioned = format!(
            "{RESOLVE_ONLY_SITE}// never the signing IdentityPort\n#[cfg(test)]\nmod t {{ use x::SigningKey; }}\n"
        );
        assert_eq!(
            findings_in(&planted_in(&mentioned)),
            Vec::<String>::new(),
            "a comment or a test is not production code"
        );
    }

    assert!(missed.is_empty(), "not refused: {missed:?}");
    let real = scan_indexer_family_holds_no_signing(&root()).expect("scan");
    assert!(real.is_empty(), "{real:?}");
}

/// A tree holding only this no-publish module.
fn a_no_publish_module(source: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a tree");
    let dir = tree.path().join("crates/openlore-indexer-lambda/src");
    std::fs::create_dir_all(&dir).expect("dirs");
    std::fs::write(dir.join("no_publish.rs"), source).expect("a file");
    tree
}

/// XS-11
/// ```gherkin
/// @D-STZ-44 @D-STZ-38 @C6c @error @contract-shape:pure-function
/// Scenario Outline: check-arch refuses a write capability in the no-publish module
///   Given a no-publish module that <plants>
///   When check-arch reads it
///   Then it reports the module
///   And the real module has no finding, and a mention in a comment is not one
///   Examples:
///     | plants                                  |
///     | names the write port                    |
///     | publishes                               |
///     | writes the pass record                  |
///     | takes what the pass is composed with    |
///     | does not name the read port at all      |
///     | does not exist                          |
/// ```
#[test]
fn check_arch_refuses_a_write_capability_in_the_no_publish_module() {
    const CLEAN: &str = "fn run(read: &dyn IndexObjectReadPort) {}\n";
    let findings = |source: &str| {
        scan_lambda_no_publish_is_read_only(a_no_publish_module(source).path()).expect("scan")
    };
    assert_eq!(findings(CLEAN), Vec::<String>::new(), "the control");
    let commented = format!("{CLEAN}// no publish_if_match here\n");
    assert_eq!(findings(&commented), Vec::<String>::new(), "a comment");

    let mut missed = Vec::new();
    for token in [
        "IndexObjectWritePort",
        "publish_if_match",
        "put_last_pass",
        "PassDriven",
    ] {
        let planted = format!("{CLEAN}fn holds(_: &dyn {token}) {{}}\n");
        if !findings(&planted).iter().any(|f| f.contains(token)) {
            missed.push(token);
        }
    }
    if !findings("fn run() {}\n")
        .iter()
        .any(|f| f.contains("IndexObjectReadPort"))
    {
        missed.push("IndexObjectReadPort");
    }

    let nothing = tempfile::tempdir().expect("an empty tree");
    if !scan_lambda_no_publish_is_read_only(nothing.path())
        .expect("scan")
        .iter()
        .any(|f| f.contains("no_publish.rs"))
    {
        missed.push("no_publish.rs");
    }

    assert!(missed.is_empty(), "not refused: {missed:?}");
    assert!(
        root()
            .join("crates/openlore-indexer-lambda/src/no_publish.rs")
            .is_file(),
        "the real module exists"
    );
    let real = scan_lambda_no_publish_is_read_only(&root()).expect("scan");
    assert!(real.is_empty(), "{real:?}");
}

const SECRET_WORDS: [&str; 4] = ["SECRET", "TOKEN", "PASSWORD", "PRIVATE"];

/// DEVOPS § Secret check: what is wrong with this environment, if anything.
fn secrets_in(environment: &serde_json::Value) -> Vec<String> {
    let is_key_id = |word: &str| {
        word.len() == 20
            && (word.starts_with("AKIA") || word.starts_with("ASIA"))
            && word.chars().all(|c| c.is_ascii_alphanumeric())
    };
    let is_secret_shaped = |word: &str| {
        word.len() == 40
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "/+=".contains(c))
    };
    let mut found = Vec::new();
    for (key, value) in environment.as_object().expect("an object") {
        if SECRET_WORDS.iter().any(|w| key.to_uppercase().contains(w)) {
            found.push(format!("key {key}"));
        }
        let value = value.as_str().unwrap_or_default();
        if value.contains("-----BEGIN") || is_key_id(value) || is_secret_shaped(value) {
            found.push(format!("the value of {key}"));
        }
    }
    found
}

/// Not a scenario: the positive control of XS-12.
#[test]
fn the_secret_check_catches_each_shape_and_passes_a_plain_environment() {
    let plain = serde_json::json!({
        "OPENLORE_INDEXER_PASS_DEADLINE_SECS": "780",
        "OPENLORE_INDEXER_REPO_DIDS_FILE": "/tmp/config/repo-dids",
    });
    assert!(secrets_in(&plain).is_empty());
    for planted in [
        serde_json::json!({"AWS_SECRET_ACCESS_KEY": "x"}),
        serde_json::json!({"github_token": "x"}),
        serde_json::json!({"DB_PASSWORD": "x"}),
        serde_json::json!({"PRIVATE_KEY_PATH": "x"}),
        serde_json::json!({"K": "AKIAIOSFODNN7EXAMPLE"}),
        serde_json::json!({"K": "ASIAIOSFODNN7EXAMPLE"}),
        serde_json::json!({"K": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"}),
        serde_json::json!({"K": "-----BEGIN PRIVATE KEY-----"}),
    ] {
        assert_eq!(secrets_in(&planted).len(), 1, "{planted}");
    }
}

/// XS-12
/// ```gherkin
/// @NFR-IXD-8 @D-STZ-1 @contract-shape:pure-function
/// Scenario: The Lambda environment holds no secret and no static AWS key
///   Given the file that becomes every function's environment
///   Then no setting is named like a secret
///   And no value has the shape of an access key, a secret key or a private key
/// ```
#[test]
#[ignore = "RED: DELIVER writes deploy/tofu/environments/prod-indexer/lambda-env.json"]
fn the_lambda_environment_holds_no_secret_and_no_static_aws_key() {
    let path = root().join("deploy/tofu/environments/prod-indexer/lambda-env.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let environment: serde_json::Value = serde_json::from_str(&text).expect("JSON");

    let found = secrets_in(&environment);

    assert!(found.is_empty(), "{found:?}");
}
