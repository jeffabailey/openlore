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
//! 2. `lexicon` — same ban list. `scraper-domain` (slice-02 pure derivation,
//!    WD-56/WD-65) — same ban list; its only non-`ports` dep is the pure
//!    `serde_yaml_ng` parser (allowlisted). `scoring` (slice-04 pure
//!    closed-form weight, WD-71/WD-82/ADR-022) — same ban list; its only
//!    non-pure-core deps are `ports` + `claim-domain` + pure `chrono`/`serde`.
//!    `appview-domain` (slice-05 pure ingest-gate + anti-merging search,
//!    WD-103/WD-104/ADR-026/I-AV-1/I-AV-2) — same ban list; its only
//!    non-pure-core deps are `claim-domain` + pure `chrono`/`serde`.
//! 3. `ports` MAY depend on `async-trait` (the `PdsPort` trait is
//!    inherently async per ADR-004) but MUST NOT depend on a tokio
//!    runtime or any other I/O crate.
//! 4. No `adapter-*` crate transitively depends on another `adapter-*`.
//! 5. Only the `cli` crate depends on `adapter-*` crates. (`xtask` and
//!    `openlore-test-support` are first-party tooling, not shipped — they
//!    are exempt by name.)

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// Banned I/O crates the pure core (claim-domain / lexicon / ports)
/// MUST NOT pull in transitively. `atrium-*` is matched by prefix.
const BANNED_IO_CRATES: &[&str] = &["tokio", "reqwest", "duckdb", "keyring"];
const BANNED_IO_PREFIXES: &[&str] = &["atrium-"];

/// Pure-core allowlist (WD-35 / ADR-015): dependencies explicitly
/// adjudicated as PURE — no I/O, no async runtime — and therefore
/// permitted inside `claim-domain` / `lexicon`. The ban list above is a
/// deny-list, so a non-I/O crate is permitted by default; this constant
/// is the EXPLICIT record of that adjudication so a reviewer sees WD-35
/// was honored and a future tightening of the rule (deny-by-default)
/// keeps these names allowed. `is_banned` skips any allowlisted name
/// even if a future prefix/name rule would otherwise match it. It does
/// NOT loosen the rule for I/O crates — only these audited pure crates.
///
/// - `unicode-normalization`: NFC normalization of `--reason` text
///   (`claim-domain::normalize_reason`); Servo's pure NFC crate.
/// - `serde_yaml_ng`: pure YAML parse of the embedded `signal_predicate_mapping`
///   SSOT snapshot in `scraper-domain` (WD-65 / Q-DELIVER-1). The maintained
///   drop-in fork of the deprecated `serde_yaml`; no I/O, no async runtime.
const PURE_CORE_ALLOWED_CRATES: &[&str] = &["unicode-normalization", "serde_yaml_ng"];

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

/// True if `dep` matches any banned name or banned prefix. Pure-core
/// allowlisted crates (WD-35) are exempt: an explicitly-adjudicated pure
/// dep is never reported as banned, even if a future rule would match it.
fn is_banned(dep: &str, names: &[&str], prefixes: &[&str]) -> Option<String> {
    if PURE_CORE_ALLOWED_CRATES.contains(&dep) {
        return None;
    }
    if names.contains(&dep) {
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

// -----------------------------------------------------------------------------
// Anti-merging rule — `no_cross_table_join_elides_author` (WD-30 / I-FED-1)
// -----------------------------------------------------------------------------
//
// Structural enforcement layer 2 of 3 (layer 1 = `FederatedRow.author_did`
// non-Option from 01-01; layer 3 = integration test
// `federation_attribution_preserved`, Phase 05). Per component-boundaries.md
// §xtask and data-models.md §"Cross-store query examples": any SQL string
// literal in `adapter-duckdb` that mentions BOTH the standalone `claims` table
// AND the `peer_claims` table MUST also project `author_did` in its SELECT
// list, else the query could silently MERGE attribution across stores
// (KPI-FED-1 / KPI-FED-2 regression). The classifier is a pure word-boundary
// regex pass over a single literal; the effect shell extracts literals with
// `syn` (so comments are never matched).

/// A cross-store SQL literal that elides `author_did` — the anti-merging
/// violation. Carries an excerpt for the error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlAntiMergingViolation {
    /// First ~80 chars of the offending literal, for the operator's error.
    pub excerpt: String,
}

/// True if `haystack` contains `needle` as a whole word (the chars on either
/// side, if any, are not `[A-Za-z0-9_]`). Distinguishes the `claims` table
/// from the `peer_claims` substring and from `claim_references` etc.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let nlen = needle.len();
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let at = start + pos;
        let before_ok = at == 0 || !is_word_byte(bytes[at - 1]);
        let after_idx = at + nlen;
        let after_ok = after_idx >= bytes.len() || !is_word_byte(bytes[after_idx]);
        if before_ok && after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

/// True for ASCII word characters (`[A-Za-z0-9_]`) — the boundary alphabet
/// for SQL identifiers.
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Pure classifier for the anti-merging rule. Given one SQL string literal,
/// return `Some(violation)` iff it references BOTH the `claims` table AND the
/// `peer_claims` table but does NOT mention `author_did`. Otherwise `None`
/// (single-table query, or cross-table query that projects attribution).
///
/// Word-boundary matching ensures `peer_claims` (which contains the substring
/// `claims`) does NOT count as a `claims`-table mention on its own.
pub fn classify_sql_literal(literal: &str) -> Option<SqlAntiMergingViolation> {
    let mentions_peer_claims = contains_word(literal, "peer_claims");
    let mentions_own_claims = contains_word(literal, "claims");
    // `contains_word("...peer_claims...", "claims")` is false (the `_` before
    // `claims` is a word byte), so `mentions_own_claims` is only true for a
    // standalone `claims` table reference. A cross-store query is one that
    // names both tables.
    let is_cross_store = mentions_peer_claims && mentions_own_claims;
    if !is_cross_store {
        return None;
    }
    if contains_word(literal, "author_did") {
        return None;
    }
    Some(SqlAntiMergingViolation {
        excerpt: excerpt_of(literal),
    })
}

/// First non-empty line (trimmed) of a literal, capped at 80 chars — enough
/// for an operator to locate the offending query.
fn excerpt_of(literal: &str) -> String {
    let first = literal
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if first.len() > 80 {
        format!("{}…", &first[..80])
    } else {
        first.to_string()
    }
}

// -----------------------------------------------------------------------------
// Anti-merging rule — index-store extension (WD-120 / I-AV-2)
// -----------------------------------------------------------------------------
//
// Structural enforcement layer of the slice-05 3-layer anti-merging defense
// (TYPE layer = `IndexedClaim.author_did` non-Option from 01-02; BEHAVIORAL
// layer = AV-9/AV-2/AVC-2 in later phases). Per data-models.md §"Read-side
// query shapes" / §"FORBIDDEN pattern" + ADR-025: the slice-05 index store is a
// SINGLE `indexed_claims` table — unlike the slice-03/04 CROSS-STORE
// `claims`+`peer_claims` rule above, the index-store risk is a SINGLE-TABLE
// AGGREGATE (`GROUP BY` / `COUNT` / `SUM` / `AVG` across authors) that DROPS
// `author_did`, fabricating a faceless "network consensus" row (WD-103).
//
// The aggregation MUST happen in the PURE `appview-domain` core (Rust); the
// index-store SQL stays per-claim + attribution-projecting. This classifier is
// a pure word-boundary pass over a single literal; the effect shell
// (`scan_adapter_index_store_sql`) extracts literals with `syn` so comments are
// never matched.

/// An index-store SQL literal that aggregates over `indexed_claims` without
/// projecting `author_did` — the index-store anti-merging violation. Carries an
/// excerpt for the operator's error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStoreAntiMergingViolation {
    /// First ~80 chars of the offending literal, for the operator's error.
    pub excerpt: String,
}

/// True if `haystack` contains any of the aggregation tokens that, over the
/// `indexed_claims` table, would merge across authors: a `GROUP BY` clause or a
/// `COUNT(` / `SUM(` / `AVG(` aggregate function call. Case-insensitive (SQL
/// keywords are case-insensitive); the function-call tokens keep the `(` so a
/// column literally named `count` is not mistaken for the aggregate.
fn mentions_aggregation(literal: &str) -> bool {
    let upper = literal.to_ascii_uppercase();
    // Normalize internal whitespace so `GROUP   BY` / `GROUP\nBY` still match.
    let collapsed: String = upper.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.contains("GROUP BY")
        || upper.contains("COUNT(")
        || upper.contains("SUM(")
        || upper.contains("AVG(")
}

/// Pure classifier for the index-store anti-merging rule. Given one SQL string
/// literal, return `Some(violation)` iff it AGGREGATES over `indexed_claims`
/// (mentions the `indexed_claims` table AND an aggregation construct) but does
/// NOT mention `author_did`. Otherwise `None` (a per-claim attributed read, a
/// non-aggregating statement, or one that does project `author_did`).
///
/// Word-boundary matching on `indexed_claims` ensures the child tables
/// `indexed_claim_evidence` / `indexed_claim_references` (which do NOT contain
/// `indexed_claims` as a whole word) are not mistaken for the parent table.
pub fn classify_index_store_sql_literal(literal: &str) -> Option<IndexStoreAntiMergingViolation> {
    if !contains_word(literal, "indexed_claims") {
        return None;
    }
    if !mentions_aggregation(literal) {
        return None;
    }
    if contains_word(literal, "author_did") {
        return None;
    }
    Some(IndexStoreAntiMergingViolation {
        excerpt: excerpt_of(literal),
    })
}

// -----------------------------------------------------------------------------
// Autoconfirm release-build guard rule (D-D20)
// -----------------------------------------------------------------------------
//
// WD-21 forbids a `--yes` flag in production. The test escape hatch
// `OPENLORE_TEST_AUTOCONFIRM` (crates/cli/src/verbs/peer_remove.rs) MUST NOT
// compile into release builds. Source-level contract (acceptable per task /
// component-boundaries.md): every occurrence of the `OPENLORE_TEST_AUTOCONFIRM`
// token must sit inside a `#[cfg(...)]`-gated item. A leak into an ungated item
// would ship the env-var read in a release binary.

/// The build-time-gated escape-hatch token D-D20 requires to be cfg-guarded.
const AUTOCONFIRM_TOKEN: &str = "OPENLORE_TEST_AUTOCONFIRM";

/// A leaked autoconfirm token — present in source but NOT behind a `#[cfg]`
/// gate, so it would compile into a release binary (D-D20 violation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoconfirmGuardViolation {
    /// The line (trimmed) where the ungated token appears.
    pub line: String,
}

/// Pure classifier for the autoconfirm release-build guard (D-D20). Given the
/// full source text of `peer_remove.rs`, return `Some(violation)` iff the
/// `OPENLORE_TEST_AUTOCONFIRM` token appears inside an item that is NOT
/// cfg-gated (and would therefore compile into a release binary).
///
/// Approach: brace-depth tracking. At top level (depth 0) an attribute run is
/// accumulated; when the item's body opens (`{`, depth 0→1) the gate state for
/// that whole item is fixed from the run (any `#[cfg(...)]` ⇒ gated). The gate
/// stays in force for every line until the body closes back to depth 0. A token
/// seen while the enclosing item is ungated is a violation; a token at top
/// level with no enclosing gated item is also a violation.
pub fn classify_autoconfirm_guard(source: &str) -> Option<AutoconfirmGuardViolation> {
    if !source.contains(AUTOCONFIRM_TOKEN) {
        return None;
    }

    let mut depth: i32 = 0;
    // Gate state of the top-level item currently open (depth >= 1). `None`
    // when at depth 0 (between items).
    let mut item_gated: Option<bool> = None;
    // Whether any `#[cfg(...)]` was seen in the attribute run preceding the
    // next top-level item.
    let mut pending_cfg = false;

    for raw in source.lines() {
        let line = raw.trim();

        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // Accumulate cfg attributes at top level before an item opens.
        if depth == 0 && (line.starts_with("#[") || line.starts_with("#![")) {
            if line.contains("cfg(") {
                pending_cfg = true;
            }
            continue;
        }

        // Token check uses the gate state currently in force: inside an open
        // item, that item's `item_gated`; at top level (e.g. a `const` or
        // `static` on one line), the pending attribute run's cfg state.
        if line.contains(AUTOCONFIRM_TOKEN) {
            let gated = match item_gated {
                Some(g) => g,
                None => pending_cfg,
            };
            if !gated {
                return Some(AutoconfirmGuardViolation {
                    line: line.to_string(),
                });
            }
        }

        // Update brace depth from this line. When we transition 0→>=1, the
        // top-level item just opened: freeze its gate state from pending_cfg.
        let opens = line.matches('{').count() as i32;
        let closes = line.matches('}').count() as i32;
        let was_top = depth == 0;
        depth += opens - closes;
        if was_top && depth > 0 {
            item_gated = Some(pending_cfg);
        }
        if depth <= 0 {
            depth = 0;
            item_gated = None;
            pending_cfg = false;
        }
    }

    None
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
    violations.extend(check_pure_core_no_io(
        workspace,
        "scraper-domain",
        "scraper-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (WD-56/I-2)",
    ));
    violations.extend(check_pure_core_no_io(
        workspace,
        "scoring",
        "scoring MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (WD-71/WD-82/ADR-022/I-GRAPH-1)",
    ));
    violations.extend(check_pure_core_no_io(
        workspace,
        "appview-domain",
        "appview-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (WD-103/WD-104/ADR-026/I-AV-1/I-AV-2)",
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

/// Locate the workspace root by walking up from the current dir looking for a
/// `Cargo.toml` that declares `[workspace]`. xtask is always invoked from
/// somewhere inside the workspace.
fn locate_workspace_root() -> anyhow::Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let body = std::fs::read_to_string(&manifest)?;
            if body.contains("[workspace]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            return Err(anyhow::anyhow!("no workspace Cargo.toml found"));
        }
    }
}

/// `syn` visitor that collects every string-literal value in a source file.
/// Used by the anti-merging rule so it scans REAL string literals (SQL) and
/// never matches `claims`/`peer_claims` mentions inside comments.
struct StringLiteralCollector {
    literals: Vec<String>,
}

impl<'ast> Visit<'ast> for StringLiteralCollector {
    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        self.literals.push(lit.value());
        syn::visit::visit_lit_str(self, lit);
    }
}

/// Effect shell for the anti-merging rule: scan `adapter-duckdb` source for
/// SQL string literals and classify each. Returns one rendered violation per
/// offending literal. A missing crate dir is treated as "nothing to scan"
/// (the rule is for slice-03 onward; absence is not a failure).
fn scan_adapter_duckdb_sql(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let src_dir = workspace_root.join("crates/adapter-duckdb/src");
    if !src_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for entry in walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !(path.is_file() && path.extension().is_some_and(|e| e == "rs")) {
            continue;
        }
        let src = std::fs::read_to_string(path)?;
        let file = match syn::parse_file(&src) {
            Ok(f) => f,
            Err(e) => return Err(anyhow::anyhow!("syn parse {}: {e}", path.display())),
        };
        let mut collector = StringLiteralCollector {
            literals: Vec::new(),
        };
        collector.visit_file(&file);
        for literal in &collector.literals {
            if let Some(v) = classify_sql_literal(literal) {
                findings.push(format!(
                    "{}: SQL literal joins `claims`+`peer_claims` without \
                     projecting `author_did` (I-FED-1 / no_cross_table_join_elides_author): {}",
                    path.display(),
                    v.excerpt
                ));
            }
        }
    }
    Ok(findings)
}

/// Effect shell for the index-store anti-merging rule (slice-05): scan
/// `adapter-index-store` source for SQL string literals and classify each with
/// [`classify_index_store_sql_literal`]. Returns one rendered violation per
/// offending literal. A missing crate dir is treated as "nothing to scan" (the
/// rule is for slice-05 onward; absence is not a failure). Mirrors
/// [`scan_adapter_duckdb_sql`] — the SAME `no_cross_table_join_elides_author`
/// anti-merging pass, extended to the SINGLE-`indexed_claims`-table store.
fn scan_adapter_index_store_sql(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let src_dir = workspace_root.join("crates/adapter-index-store/src");
    if !src_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for entry in walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !(path.is_file() && path.extension().is_some_and(|e| e == "rs")) {
            continue;
        }
        let src = std::fs::read_to_string(path)?;
        let file = match syn::parse_file(&src) {
            Ok(f) => f,
            Err(e) => return Err(anyhow::anyhow!("syn parse {}: {e}", path.display())),
        };
        let mut collector = StringLiteralCollector {
            literals: Vec::new(),
        };
        collector.visit_file(&file);
        for literal in &collector.literals {
            if let Some(v) = classify_index_store_sql_literal(literal) {
                findings.push(format!(
                    "{}: SQL literal aggregates over `indexed_claims` without \
                     projecting `author_did` (I-AV-2 / no_cross_table_join_elides_author): {}",
                    path.display(),
                    v.excerpt
                ));
            }
        }
    }
    Ok(findings)
}

/// Effect shell for the autoconfirm guard (D-D20): read `peer_remove.rs` and
/// verify the `OPENLORE_TEST_AUTOCONFIRM` token is cfg-gated. A missing file is
/// "nothing to check" (slice-03 verb may not exist yet in older trees).
fn scan_autoconfirm_guard(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let path = workspace_root.join("crates/cli/src/verbs/peer_remove.rs");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let src = std::fs::read_to_string(&path)?;
    Ok(match classify_autoconfirm_guard(&src) {
        Some(v) => vec![format!(
            "{}: `OPENLORE_TEST_AUTOCONFIRM` token is NOT behind a `#[cfg(...)]` \
             gate — it would ship in a release binary (D-D20): {}",
            path.display(),
            v.line
        )],
        None => Vec::new(),
    })
}

/// Effect shell: composes load + dep-graph check + source-scanning rules +
/// render. Returns process exit code (0 = healthy, 1 = violations).
pub fn run() -> anyhow::Result<i32> {
    let workspace = load_workspace()?;
    let dep_violations = check_workspace(&workspace);

    let workspace_root = locate_workspace_root()?;
    let sql_findings = scan_adapter_duckdb_sql(&workspace_root)?;
    let index_store_sql_findings = scan_adapter_index_store_sql(&workspace_root)?;
    let autoconfirm_findings = scan_autoconfirm_guard(&workspace_root)?;

    let mut rendered: Vec<String> = dep_violations.iter().map(Violation::render).collect();
    rendered.extend(sql_findings);
    rendered.extend(index_store_sql_findings);
    rendered.extend(autoconfirm_findings);

    if rendered.is_empty() {
        println!(
            "check-arch: OK ({} workspace members)",
            workspace.members.len()
        );
        Ok(0)
    } else {
        eprintln!("check-arch: {} violation(s) found:", rendered.len());
        for r in &rendered {
            eprintln!("  - {r}");
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
        let w = ws(&[("claim-domain", &["tokio"]), ("lexicon", &[])]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "claim-domain" && x.forbidden == "tokio"),
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
            v.iter()
                .any(|x| x.package == "claim-domain" && x.forbidden == "reqwest"),
            "expected transitive claim-domain→reqwest violation, got: {v:?}"
        );
    }

    #[test]
    fn claim_domain_depending_on_unicode_normalization_is_allowed() {
        // WD-35: `unicode-normalization` is a PURE dep (NFC, no I/O).
        // The pure-core allowlist must permit it in claim-domain while
        // the I/O ban list stays in force for everything else.
        let w = ws(&[("claim-domain", &["serde", "unicode-normalization"])]);
        assert!(
            check_workspace(&w).is_empty(),
            "unicode-normalization must be an allowed pure-core dep (WD-35), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn scraper_domain_with_pure_yaml_dep_is_allowed() {
        // WD-65 / Q-DELIVER-1: scraper-domain is PURE; its only non-ports dep is
        // the pure `serde_yaml_ng` parser, which the pure-core allowlist permits.
        let w = ws(&[
            ("scraper-domain", &["ports", "serde", "serde_yaml_ng"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "scraper-domain + serde_yaml_ng must be an allowed pure-core shape (WD-65), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn scraper_domain_depending_on_reqwest_is_violation() {
        // The pure-core ban list is in force for scraper-domain (I-2).
        let w = ws(&[("scraper-domain", &["reqwest"])]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "scraper-domain" && x.forbidden == "reqwest"),
            "expected scraper-domain→reqwest violation, got: {v:?}"
        );
    }

    #[test]
    fn scoring_with_pure_deps_is_allowed() {
        // WD-71/WD-82/ADR-022: `scoring` is the slice-04 PURE closed-form
        // weight core. Its dep surface is `ports` + `claim-domain` + pure
        // `chrono`/`serde` — no I/O, no ML. The pure-core ban list must pass.
        let w = ws(&[
            ("scoring", &["ports", "claim-domain", "serde", "chrono"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "scoring with pure deps must pass the pure-core allowlist (WD-82), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn scoring_depending_on_duckdb_is_violation() {
        // The pure-core ban list is in force for scoring (I-GRAPH-1): a
        // scoring crate reaching duckdb would mean the weight was computed in
        // the substrate, not the pure core.
        let w = ws(&[("scoring", &["duckdb"])]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "scoring" && x.forbidden == "duckdb"),
            "expected scoring→duckdb violation, got: {v:?}"
        );
    }

    #[test]
    fn appview_domain_with_pure_deps_is_allowed() {
        // WD-103/WD-104/ADR-026: `appview-domain` is the slice-05 PURE
        // ingest-gate + anti-merging search core. Its dep surface is
        // `claim-domain` + pure `chrono`/`serde` — no I/O. The pure-core ban
        // list must pass (I-AV-1/I-AV-2).
        let w = ws(&[
            ("appview-domain", &["claim-domain", "serde", "chrono"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "appview-domain with pure deps must pass the pure-core allowlist (WD-103/WD-104), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn appview_domain_depending_on_duckdb_is_violation() {
        // The pure-core ban list is in force for appview-domain (I-AV-2): an
        // appview-domain crate reaching duckdb would mean the search/grouping
        // ran in the substrate, not the pure core.
        let w = ws(&[("appview-domain", &["duckdb"])]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "appview-domain" && x.forbidden == "duckdb"),
            "expected appview-domain→duckdb violation, got: {v:?}"
        );
    }

    #[test]
    fn lexicon_depending_on_atrium_api_is_violation() {
        let w = ws(&[("lexicon", &["atrium-api"])]);
        let v = check_workspace(&w);
        assert!(v
            .iter()
            .any(|x| x.package == "lexicon" && x.forbidden == "atrium-api"));
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
            v.iter()
                .any(|x| x.package == "ports" && x.forbidden == "tokio"),
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
            v.iter()
                .any(|x| { x.package == "claim-domain" && x.forbidden == "adapter-duckdb" }),
            "expected claim-domain→adapter-duckdb (invariant 5) violation, got: {v:?}"
        );
    }

    #[test]
    fn xtask_is_exempt_from_invariant_5() {
        // xtask is workspace tooling, not shipped — invariant 5 doesn't
        // apply to it. (Today it has no adapter dep, but tomorrow's
        // codegen task might.)
        let w = ws(&[("xtask", &["adapter-duckdb"]), ("adapter-duckdb", &[])]);
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
            ("openlore-test-support", &["ports", "claim-domain", "tokio"]),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.is_empty(),
            "production-like healthy workspace should have zero violations, got: {v:?}"
        );
    }
}
