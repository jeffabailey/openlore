//! `cargo xtask check-arch` — enforces hexagonal dependency invariants.
//!
//! Pure core: [`check_workspace`] takes an in-memory [`Workspace`] graph
//! (`name -> direct-deps`) plus the rule set and returns a `Vec<Violation>`.
//! No I/O, no `cargo_metadata` types — that's what makes the inner-TDD
//! unit tests trivial to write with hand-constructed fixtures.
//!
//! Effect shell: [`load_workspace`] shells out to `cargo metadata` (via the
//! `cargo_metadata` crate) and projects it into the pure `Workspace` shape.
//! [`run`] composes the two and renders violations to stderr.
//!
//! Invariants enforced (per `docs/feature/openlore-foundation/design/
//! component-boundaries.md` §Cross-component invariants, ADR-009 D-11):
//!
//! 1. `claim-domain` MUST NOT transitively depend on any banned I/O crate
//!    (`tokio`, `reqwest`, `duckdb`, `keyring`, any `atrium-*`).
//! 2. `lexicon` — same ban list.
//! 3. `ports` MAY depend on `async-trait` (the `PdsPort` trait is
//!    inherently async per ADR-004) but MUST NOT depend on a tokio
//!    runtime or any other I/O crate.
//! 4. No `adapter-*` crate transitively depends on another `adapter-*`.
//! 5. Only the `cli` crate depends on `adapter-*` crates. (`xtask` and
//!    `openlore-test-support` are first-party tooling, not shipped — they
//!    are exempt by name.)

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Banned I/O crates the pure core (claim-domain / lexicon / ports)
/// MUST NOT pull in transitively. `atrium-*` is matched by prefix.
const BANNED_IO_CRATES: &[&str] = &["tokio", "reqwest", "duckdb", "keyring"];
const BANNED_IO_PREFIXES: &[&str] = &["atrium-"];

/// `ports` is async-shaped (PdsPort) so `async-trait` is the one allowed
/// async dep; the runtime itself (tokio) and HTTP/DB I/O crates remain
/// banned. Per component-boundaries.md §`crates/ports`.
const PORTS_BANNED_IO_CRATES: &[&str] = &["tokio", "reqwest", "duckdb", "keyring"];
const PORTS_BANNED_IO_PREFIXES: &[&str] = &["atrium-"];

/// Workspace member crates that are first-party tooling, not shipped
/// product. They're allowed to depend on adapter-* crates because they
/// don't compose runtime behavior.
const ADAPTER_DEPENDENT_EXEMPT_MEMBERS: &[&str] = &["xtask", "openlore-test-support"];

/// The single shipped composition root, per ADR-009. The only crate
/// allowed to depend on `adapter-*` at runtime.
const COMPOSITION_ROOT: &str = "cli";

/// Pure in-memory view of the workspace dep graph. `members` is the set
/// of workspace-member package names; `deps` maps every package name
/// (workspace member or external) to its direct dependencies' names.
/// The reachability walk treats the graph as directed.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    pub members: BTreeSet<String>,
    pub deps: BTreeMap<String, BTreeSet<String>>,
}

impl Workspace {
    /// Returns the set of crate names transitively reachable from `root`
    /// (excluding `root` itself). Missing nodes are treated as leaves —
    /// the graph projection from `cargo_metadata` always includes every
    /// node it references, so a missing node means an external crate we
    /// don't care to descend into; that's safe for the ban-list check
    /// because the *name* is what we match on at every edge.
    pub fn transitive_deps(&self, root: &str) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        if let Some(direct) = self.deps.get(root) {
            for d in direct {
                if seen.insert(d.clone()) {
                    queue.push_back(d.clone());
                }
            }
        }
        while let Some(node) = queue.pop_front() {
            if let Some(children) = self.deps.get(&node) {
                for c in children {
                    if seen.insert(c.clone()) {
                        queue.push_back(c.clone());
                    }
                }
            }
        }
        seen
    }
}

/// A single architecture-invariant violation. The pure core returns a
/// `Vec<Violation>` and never panics; the effect shell renders them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub package: String,
    pub forbidden: String,
    pub rule: &'static str,
}

impl Violation {
    pub fn render(&self) -> String {
        format!(
            "forbidden dep: `{}` transitively depends on `{}` ({})",
            self.package, self.forbidden, self.rule
        )
    }
}

/// True if `dep` matches any banned name or banned prefix.
fn is_banned(dep: &str, names: &[&str], prefixes: &[&str]) -> Option<String> {
    if names.iter().any(|n| *n == dep) {
        return Some(dep.to_string());
    }
    for p in prefixes {
        if dep.starts_with(p) {
            return Some(dep.to_string());
        }
    }
    None
}

/// True if the package name is an `adapter-*` workspace crate (by name
/// convention, matches component-boundaries.md §Crate layout).
fn is_adapter_crate(name: &str) -> bool {
    name.starts_with("adapter-")
}

/// Pure check: invariant 1+2 — pure-core crates have NO transitive I/O.
fn check_pure_core_no_io(
    workspace: &Workspace,
    package: &str,
    rule: &'static str,
) -> Vec<Violation> {
    if !workspace.members.contains(package) {
        // Crate not in workspace; nothing to check (skip silently rather
        // than fail — keeps the check robust to incremental workspace
        // changes).
        return Vec::new();
    }
    let transitive = workspace.transitive_deps(package);
    transitive
        .iter()
        .filter_map(|d| is_banned(d, BANNED_IO_CRATES, BANNED_IO_PREFIXES))
        .map(|forbidden| Violation {
            package: package.to_string(),
            forbidden,
            rule,
        })
        .collect()
}

/// Pure check: invariant 3 — `ports` MAY depend on async-trait but NOT
/// on tokio runtime or any other I/O crate.
fn check_ports_async_trait_only(workspace: &Workspace) -> Vec<Violation> {
    if !workspace.members.contains("ports") {
        return Vec::new();
    }
    let transitive = workspace.transitive_deps("ports");
    transitive
        .iter()
        .filter_map(|d| is_banned(d, PORTS_BANNED_IO_CRATES, PORTS_BANNED_IO_PREFIXES))
        .map(|forbidden| Violation {
            package: "ports".to_string(),
            forbidden,
            rule: "ports MAY depend on async-trait but NOT on a tokio runtime or any I/O crate",
        })
        .collect()
}

/// Pure check: invariant 4 — no `adapter-*` depends on another `adapter-*`.
fn check_no_adapter_depends_on_adapter(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    for member in workspace.members.iter().filter(|m| is_adapter_crate(m)) {
        let transitive = workspace.transitive_deps(member);
        for dep in transitive {
            if is_adapter_crate(&dep) && dep != *member {
                violations.push(Violation {
                    package: member.clone(),
                    forbidden: dep,
                    rule: "no adapter-* may depend on another adapter-*",
                });
            }
        }
    }
    violations
}

/// Pure check: invariant 5 — only `cli` (composition root) depends on
/// `adapter-*` crates. `xtask` and `openlore-test-support` are exempt
/// (first-party tooling, not shipped).
fn check_only_cli_depends_on_adapters(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    for member in &workspace.members {
        if member == COMPOSITION_ROOT
            || ADAPTER_DEPENDENT_EXEMPT_MEMBERS.contains(&member.as_str())
            || is_adapter_crate(member)
        {
            continue;
        }
        let transitive = workspace.transitive_deps(member);
        for dep in transitive {
            if is_adapter_crate(&dep) {
                violations.push(Violation {
                    package: member.clone(),
                    forbidden: dep,
                    rule: "only `cli` (composition root) may depend on `adapter-*` crates",
                });
            }
        }
    }
    violations
}

/// Pure entry point — given a workspace shape, return every violation.
/// Empty vec = healthy.
pub fn check_workspace(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    violations.extend(check_pure_core_no_io(
        workspace,
        "claim-domain",
        "claim-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-*",
    ));
    violations.extend(check_pure_core_no_io(
        workspace,
        "lexicon",
        "lexicon MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-*",
    ));
    violations.extend(check_ports_async_trait_only(workspace));
    violations.extend(check_no_adapter_depends_on_adapter(workspace));
    violations.extend(check_only_cli_depends_on_adapters(workspace));
    violations
}

/// Effect shell: load real workspace via `cargo metadata`, project into
/// the pure `Workspace` shape. Uses the resolved dep graph
/// (`metadata.resolve`) so transitive dep names match what actually
/// compiles, not just what's declared in Cargo.toml.
pub fn load_workspace() -> anyhow::Result<Workspace> {
    use cargo_metadata::MetadataCommand;

    let metadata = MetadataCommand::new()
        .exec()
        .map_err(|e| anyhow::anyhow!("cargo metadata failed: {e}"))?;

    // Build pkg-id -> name map so we can translate resolve.nodes ids to
    // human names (cargo_metadata's resolve graph speaks in PackageIds).
    let id_to_name: BTreeMap<String, String> = metadata
        .packages
        .iter()
        .map(|p| (p.id.repr.clone(), p.name.to_string()))
        .collect();

    let members: BTreeSet<String> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| id_to_name.get(&id.repr).cloned())
        .collect();

    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if let Some(resolve) = metadata.resolve {
        for node in resolve.nodes {
            let name = match id_to_name.get(&node.id.repr) {
                Some(n) => n.clone(),
                None => continue,
            };
            let direct: BTreeSet<String> = node
                .dependencies
                .iter()
                .filter_map(|dep_id| id_to_name.get(&dep_id.repr).cloned())
                .collect();
            deps.insert(name, direct);
        }
    }

    Ok(Workspace { members, deps })
}

/// Effect shell: composes load + check + render. Returns process exit
/// code (0 = healthy, 1 = violations).
pub fn run() -> anyhow::Result<i32> {
    let workspace = load_workspace()?;
    let violations = check_workspace(&workspace);
    if violations.is_empty() {
        println!("check-arch: OK ({} workspace members)", workspace.members.len());
        Ok(0)
    } else {
        eprintln!("check-arch: {} violation(s) found:", violations.len());
        for v in &violations {
            eprintln!("  - {}", v.render());
        }
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a tiny `Workspace` from `(name, [deps])` pairs. All
    /// listed names become workspace members; the dep graph is exactly
    /// what the caller supplied.
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

    // --- Invariant 1+2: pure core has no I/O ---------------------------

    #[test]
    fn pure_core_with_no_io_passes() {
        let w = ws(&[
            ("claim-domain", &["serde", "thiserror"]),
            ("lexicon", &["serde", "thiserror"]),
        ]);
        assert!(check_workspace(&w).is_empty());
    }

    #[test]
    fn claim_domain_depending_on_tokio_is_violation() {
        let w = ws(&[
            ("claim-domain", &["tokio"]),
            ("lexicon", &[]),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.iter().any(|x| x.package == "claim-domain" && x.forbidden == "tokio"),
            "expected claim-domain→tokio violation, got: {v:?}"
        );
    }

    #[test]
    fn claim_domain_transitively_depending_on_reqwest_is_violation() {
        // claim-domain -> some-helper -> reqwest
        let w = Workspace {
            members: ["claim-domain".to_string()].into_iter().collect(),
            deps: BTreeMap::from([
                (
                    "claim-domain".to_string(),
                    ["some-helper".to_string()].into_iter().collect(),
                ),
                (
                    "some-helper".to_string(),
                    ["reqwest".to_string()].into_iter().collect(),
                ),
            ]),
        };
        let v = check_workspace(&w);
        assert!(
            v.iter().any(|x| x.package == "claim-domain" && x.forbidden == "reqwest"),
            "expected transitive claim-domain→reqwest violation, got: {v:?}"
        );
    }

    #[test]
    fn lexicon_depending_on_atrium_api_is_violation() {
        let w = ws(&[("lexicon", &["atrium-api"])]);
        let v = check_workspace(&w);
        assert!(v.iter().any(|x| x.package == "lexicon" && x.forbidden == "atrium-api"));
    }

    // --- Invariant 3: ports may have async-trait, not tokio ------------

    #[test]
    fn ports_with_async_trait_only_passes() {
        let w = ws(&[
            ("ports", &["async-trait", "serde", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(check_workspace(&w).is_empty());
    }

    #[test]
    fn ports_pulling_in_tokio_is_violation() {
        let w = ws(&[("ports", &["async-trait", "tokio"])]);
        let v = check_workspace(&w);
        assert!(
            v.iter().any(|x| x.package == "ports" && x.forbidden == "tokio"),
            "expected ports→tokio violation, got: {v:?}"
        );
    }

    // --- Invariant 4: no adapter-* depends on another adapter-* --------

    #[test]
    fn adapter_independent_of_other_adapters_passes() {
        let w = ws(&[
            ("adapter-duckdb", &["ports"]),
            ("adapter-system-clock", &["ports"]),
        ]);
        assert!(check_workspace(&w).is_empty());
    }

    #[test]
    fn adapter_depending_on_another_adapter_is_violation() {
        let w = ws(&[
            ("adapter-duckdb", &["adapter-system-clock"]),
            ("adapter-system-clock", &[]),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.iter().any(|x| {
                x.package == "adapter-duckdb" && x.forbidden == "adapter-system-clock"
            }),
            "expected adapter-duckdb→adapter-system-clock violation, got: {v:?}"
        );
    }

    // --- Invariant 5: only cli depends on adapter-* --------------------

    #[test]
    fn cli_depending_on_adapters_is_allowed() {
        let w = ws(&[
            ("cli", &["adapter-duckdb", "adapter-system-clock"]),
            ("adapter-duckdb", &[]),
            ("adapter-system-clock", &[]),
        ]);
        assert!(check_workspace(&w).is_empty());
    }

    #[test]
    fn non_cli_non_adapter_member_depending_on_adapter_is_violation() {
        // claim-domain has no I/O ban hit, but it must not touch
        // adapter-* either (invariant 5).
        let w = ws(&[
            ("claim-domain", &["adapter-duckdb"]),
            ("adapter-duckdb", &[]),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.iter().any(|x| {
                x.package == "claim-domain" && x.forbidden == "adapter-duckdb"
            }),
            "expected claim-domain→adapter-duckdb (invariant 5) violation, got: {v:?}"
        );
    }

    #[test]
    fn xtask_is_exempt_from_invariant_5() {
        // xtask is workspace tooling, not shipped — invariant 5 doesn't
        // apply to it. (Today it has no adapter dep, but tomorrow's
        // codegen task might.)
        let w = ws(&[
            ("xtask", &["adapter-duckdb"]),
            ("adapter-duckdb", &[]),
        ]);
        let v = check_workspace(&w);
        assert!(
            !v.iter().any(|x| x.package == "xtask"),
            "xtask must be exempt from invariant 5, got: {v:?}"
        );
    }

    #[test]
    fn test_support_is_exempt_from_invariant_5() {
        // openlore-test-support implements port traits — its only
        // adapter-shaped dep is the trait crate itself.
        let w = ws(&[
            ("openlore-test-support", &["adapter-duckdb"]),
            ("adapter-duckdb", &[]),
        ]);
        let v = check_workspace(&w);
        assert!(
            !v.iter().any(|x| x.package == "openlore-test-support"),
            "openlore-test-support must be exempt from invariant 5, got: {v:?}"
        );
    }

    // --- Healthy production-like fixture -------------------------------

    #[test]
    fn healthy_workspace_mirroring_production_shape_passes() {
        // Hand-rolled minimal mirror of the real OpenLore workspace.
        // If this fails, the rules are too strict for the actual design.
        let w = ws(&[
            ("claim-domain", &["serde", "ciborium", "cid"]),
            ("lexicon", &["serde"]),
            ("ports", &["async-trait", "claim-domain", "lexicon"]),
            ("adapter-duckdb", &["ports", "claim-domain", "duckdb"]),
            (
                "adapter-atproto-pds",
                &["ports", "tokio", "reqwest", "async-trait"],
            ),
            ("adapter-atproto-did", &["ports", "claim-domain", "keyring"]),
            ("adapter-system-clock", &["ports", "chrono"]),
            (
                "cli",
                &[
                    "ports",
                    "claim-domain",
                    "lexicon",
                    "adapter-duckdb",
                    "adapter-atproto-pds",
                    "adapter-atproto-did",
                    "adapter-system-clock",
                    "tokio",
                    "clap",
                ],
            ),
            ("xtask", &["cargo_metadata", "anyhow"]),
            (
                "openlore-test-support",
                &["ports", "claim-domain", "tokio"],
            ),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.is_empty(),
            "production-like healthy workspace should have zero violations, got: {v:?}"
        );
    }
}
