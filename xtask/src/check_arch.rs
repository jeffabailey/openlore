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
//!    `publish-domain` (serverless-philosophy-federation pure publish core,
//!    ADR-062) — same ban list; its only deps are `claim-domain` + `lexicon`
//!    + `ports` + pure `serde_json`.
//! 3. `ports` MAY depend on `async-trait` (the `PdsPort` trait is
//!    inherently async per ADR-004) but MUST NOT depend on a tokio
//!    runtime or any other I/O crate.
//! 4. No `adapter-*` crate transitively depends on another `adapter-*`.
//! 5. Only the `cli` crate depends on `adapter-*` crates. (`xtask` and
//!    `openlore-test-support` are first-party tooling, not shipped — they
//!    are exempt by name.)
//! 6. The `atproto/` Worker's npm tree (`package.json` + `package-lock.json`)
//!    MUST NOT contain any `@ipld/*`, `multiformats` or `cbor*` package — it
//!    is an opaque byte store; Rust `claim-domain` is the sole canonicalizer
//!    (ADR-062 §1/§2; [`classify_atproto_npm_manifest`]).
//! 7. `publish_write_capability_isolated`: only the `openlore publish` verb
//!    (and the wiring it calls) may reference the write-capable `PublishPort`
//!    (`publish_port_for`, `for_owner`, `dyn PublishPort`); every other verb
//!    and read path uses the read-only `InstanceReadPort` (ADR-062 §6, D-7;
//!    [`classify_publish_write_capability`]). The cross-instance `peer pull`
//!    (US-SF-006) is held tighter still: it may reach a peer's instance ONLY
//!    through `wiring::instance_reader_for` — it names no instance write
//!    method and never constructs the HTTP adapter itself.
//! 8. `contribution_links_append_only`: no production SQL literal in
//!    `adapter-duckdb` deletes, drops, truncates or bare-`UPDATE`s the
//!    `contribution_links` table — only the snapshot upsert's `DO UPDATE`
//!    arm may refresh a row (DDD-5 / DDD-16;
//!    [`classify_contribution_links_literal`]).
//! 9. `adapter_github_holds_no_storage_port`: `adapter-github` names no
//!    storage / identity / publish port (DDD-16 / I-SCR-1;
//!    [`classify_adapter_github_port_reference`]).

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
/// - `maud` / `maud_macros`: pure compile-time HTML template macro used by the
///   slice-06 `viewer-domain` read-only viewer core (ADR-029). The macro expands
///   to string building at compile time — no I/O, no async runtime.
const PURE_CORE_ALLOWED_CRATES: &[&str] = &[
    "unicode-normalization",
    "serde_yaml_ng",
    "maud",
    "maud_macros",
];

/// `ports` is async-shaped (PdsPort) so `async-trait` is the one allowed
/// async dep; the runtime itself (tokio) and HTTP/DB I/O crates remain
/// banned. Per component-boundaries.md §`crates/ports`.
const PORTS_BANNED_IO_CRATES: &[&str] = &["tokio", "reqwest", "duckdb", "keyring"];
const PORTS_BANNED_IO_PREFIXES: &[&str] = &["atrium-"];

/// Workspace member crates that are first-party tooling, not shipped
/// product. They're allowed to depend on adapter-* crates because they
/// don't compose runtime behavior.
const ADAPTER_DEPENDENT_EXEMPT_MEMBERS: &[&str] = &["xtask", "openlore-test-support"];

/// The shipped composition roots, per ADR-009/023. The ONLY crates allowed to
/// depend on `adapter-*` at runtime. Slice-05 (ADR-023) adds the SECOND root,
/// `openlore-indexer` (the network indexer binary): invariant 5 (I-3) covers
/// BOTH. The disjointness of the two roots' adapter sets — neither wires the
/// other's — is enforced separately by
/// [`check_indexer_capability_boundary`] (I-AV-5 + the I-3 second axis).
///
/// bluesky-claim-review-app (ADR-072) adds the THIRD root, the hosted
/// `openlore-review-app`; its adapter set is kept disjoint from the other two
/// by [`check_review_app_capability_boundary`].
const COMPOSITION_ROOTS: &[&str] = &["cli", "openlore-indexer", "openlore-review-app"];

/// The user's CLI composition root, per ADR-009. Named separately because the
/// indexer capability-boundary rule keys on it specifically (the `cli`-side I-3
/// axis: the CLI links the index-query CLIENT, never the indexer's server).
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

/// Pure check: invariant 5 (I-3) — only a composition root (`cli` OR the
/// slice-05 `openlore-indexer`, ADR-023) depends on `adapter-*` crates. `xtask`
/// and `openlore-test-support` are exempt (first-party tooling, not shipped).
/// The two roots' adapter sets are kept DISJOINT by
/// [`check_indexer_capability_boundary`] (this rule only governs WHO may touch
/// `adapter-*`; that rule governs WHICH adapters each root may touch).
fn check_only_cli_depends_on_adapters(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    for member in &workspace.members {
        if COMPOSITION_ROOTS.contains(&member.as_str())
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
                    rule: "only a composition root (`cli` / `openlore-indexer` / `openlore-review-app`) may depend on `adapter-*` crates (I-3)",
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
/// Delegates to the shared [`classify_cfg_gated_token`] brace-depth pass.
pub fn classify_autoconfirm_guard(source: &str) -> Option<AutoconfirmGuardViolation> {
    classify_cfg_gated_token(source, AUTOCONFIRM_TOKEN)
        .map(|line| AutoconfirmGuardViolation { line })
}

/// Pure, token-generic release-build cfg-gate classifier. Returns `Some(line)`
/// iff `token` appears (as a substring of a line) inside an item that is NOT
/// `#[cfg(...)]`-gated (and would therefore compile into a release binary);
/// `None` when every occurrence sits inside a cfg-gated item (or the token never
/// appears).
///
/// Approach: brace-depth tracking. At top level (depth 0) an attribute run is
/// accumulated; when the item's body opens (`{`, depth 0→1) the gate state for
/// that whole item is fixed from the run (any `#[cfg(...)]` ⇒ gated). The gate
/// stays in force for every line until the body closes back to depth 0. A token
/// seen while the enclosing item is ungated is a violation; a token at top level
/// with no enclosing gated item is also a violation. Shared by the D-D20
/// autoconfirm guard and the I-AV-6 pubkey-seam guard so the gating logic lives
/// in exactly one place.
fn classify_cfg_gated_token(source: &str, token: &str) -> Option<String> {
    if !source.contains(token) {
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
        if line.contains(token) {
            let gated = match item_gated {
                Some(g) => g,
                None => pending_cfg,
            };
            if !gated {
                return Some(line.to_string());
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
        // Reset the gate state ONLY when an item actually CLOSED (a `}` brought
        // depth back to 0) — NOT on lines that merely stay at depth 0 (e.g. a
        // multi-line `fn(...)` signature, where the `#[cfg]` attribute precedes
        // the opening `{` across several depth-0 lines). Resetting on every
        // depth-0 line would drop a pending cfg gate before the item opens,
        // misclassifying a correctly-gated multi-line-signature item as ungated.
        if depth <= 0 && closes > 0 {
            depth = 0;
            item_gated = None;
            pending_cfg = false;
        } else if depth < 0 {
            depth = 0;
        }
    }

    None
}

// -----------------------------------------------------------------------------
// Pubkey-seam release-build guard rule (I-AV-6 / ADR-026)
// -----------------------------------------------------------------------------
//
// The slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam is RETAINED for tests
// but RELEASE-FORBIDDEN: production verification resolves + decodes the author's
// REAL PLC `z6Mk...` key (the pure `claim_domain::decode_ed25519_multibase`),
// never a key handed in via the environment. AV-4 (step 03-04) runs the REAL
// decode with the seam UNSET. Source-level contract (mirrors D-D20): every
// occurrence of the `OPENLORE_PEER_PUBKEY_HEX_` token must sit inside a
// `#[cfg(...)]`-gated item; an ungated occurrence would ship the env-var read in
// a release binary.

/// The release-forbidden pubkey-seam env-var PREFIX. The full var name appends a
/// per-DID suffix (`format!("OPENLORE_PEER_PUBKEY_HEX_{}", did)`), so the rule
/// keys on the prefix the source literal carries.
const PUBKEY_SEAM_TOKEN: &str = "OPENLORE_PEER_PUBKEY_HEX_";

/// A leaked pubkey-seam token — present in source but NOT behind a `#[cfg]` gate,
/// so it would compile into a release binary (I-AV-6 violation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubkeySeamViolation {
    /// The line (trimmed) where the ungated token appears.
    pub line: String,
}

/// Pure classifier for the pubkey-seam release-build guard (I-AV-6 / ADR-026).
/// Given the full source text of the verify-only adapter, return
/// `Some(violation)` iff the `OPENLORE_PEER_PUBKEY_HEX_` token appears inside an
/// item that is NOT cfg-gated (and would therefore ship the env-seam read in a
/// release binary). Mirrors [`classify_autoconfirm_guard`].
pub fn classify_pubkey_seam_guard(source: &str) -> Option<PubkeySeamViolation> {
    classify_cfg_gated_token(source, PUBKEY_SEAM_TOKEN).map(|line| PubkeySeamViolation { line })
}

// -----------------------------------------------------------------------------
// Indexer capability-boundary rule — `indexer_holds_no_signing_or_local_store`
// (I-AV-5 / ADR-023) + the I-3 extension to BOTH composition roots
// -----------------------------------------------------------------------------
//
// The `openlore-indexer` binary is the SECOND composition root and is
// signing-INCAPABLE + holds NO local store (ADR-023). Encoded as the ABSENCE of
// two dep classes in the indexer's transitive dep graph: the user's local-store
// adapter (`adapter-duckdb`, the `StoragePort` impl) and any PDS-write surface
// (`adapter-atproto-pds`, which carries `create_record`/`put_record`). The
// indexer MAY depend on `adapter-atproto-did` (the verify-only
// `IdentityResolvePort` resolve path — resolve/verify-only, no signing).
//
// The rule also extends I-3 to the second composition-root axis: the `cli` crate
// must link NO HTTP server (`adapter-xrpc-query-server`) and none of the
// indexer-side store/ingest crates — the two roots wire disjoint adapter sets,
// neither wires the other's.

/// The indexer composition-root crate name.
const INDEXER_ROOT: &str = "openlore-indexer";

/// Dep classes the signing-incapable, store-less indexer MUST NOT reach (I-AV-5
/// / ADR-023): the user's local store (`StoragePort` impl) + any PDS-write
/// surface. `adapter-atproto-did` is intentionally NOT here — the indexer wires
/// it ONLY for the verify-only `IdentityResolvePort` (resolve/verify-only).
const INDEXER_FORBIDDEN_DEPS: &[&str] = &["adapter-duckdb", "adapter-atproto-pds"];

/// Indexer-side adapters the `cli` composition root MUST NOT link (I-3 second
/// axis): the HTTP query server + the indexer's store/ingest crates. The CLI
/// links the index-query CLIENT (`adapter-index-query`) instead — that is
/// permitted (NOT on this list).
const CLI_FORBIDDEN_INDEXER_DEPS: &[&str] = &[
    "adapter-xrpc-query-server",
    "adapter-index-store",
    "adapter-atproto-ingest",
];

/// Pure dep-graph check (I-AV-5 + I-3 extension): the `openlore-indexer` crate's
/// transitive dep graph excludes the signing/local-store deps, AND the `cli`
/// crate links no indexer-side server/store/ingest crate. Returns one
/// [`Violation`] per offending edge; empty vec = compliant. A missing root crate
/// is silently skipped (robust to incremental workspace changes — the rule only
/// fires when the root is actually present).
pub fn check_indexer_capability_boundary(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();

    if workspace.members.contains(INDEXER_ROOT) {
        let transitive = workspace.transitive_deps(INDEXER_ROOT);
        for forbidden in INDEXER_FORBIDDEN_DEPS {
            if transitive.contains(*forbidden) {
                violations.push(Violation {
                    package: INDEXER_ROOT.to_string(),
                    forbidden: (*forbidden).to_string(),
                    rule: "openlore-indexer MUST NOT depend on the signing identity / \
                           local store / PDS-write surface (I-AV-5 / ADR-023)",
                });
            }
        }
    }

    if workspace.members.contains(COMPOSITION_ROOT) {
        let transitive = workspace.transitive_deps(COMPOSITION_ROOT);
        for forbidden in CLI_FORBIDDEN_INDEXER_DEPS {
            if transitive.contains(*forbidden) {
                violations.push(Violation {
                    package: COMPOSITION_ROOT.to_string(),
                    forbidden: (*forbidden).to_string(),
                    rule: "cli MUST NOT link the indexer's HTTP server / store / ingest \
                           crates (I-3: disjoint composition roots)",
                });
            }
        }
    }

    violations
}

// -----------------------------------------------------------------------------
// Viewer capability-boundary rule — `viewer_holds_no_signing_surface`
// (I-VIEW-3 / ADR-028/030)
// -----------------------------------------------------------------------------
//
// The slice-06 `adapter-http-viewer` is the read-only `openlore ui` viewer's
// HTTP shell. It holds a `Box<dyn StoreReadPort>` (no write/sign method) and
// NOTHING that can sign or publish: the signing key never enters the viewer
// process (I-VIEW-3). Encoded as the ABSENCE of the signing-identity + PDS-write
// surfaces from the adapter's transitive dep graph. Additionally, `cli` is the
// ONLY crate that may link the viewer adapter (the viewer capability invariant);
// no pure core / other adapter / the indexer root may reach it.

/// The viewer HTTP-shell adapter crate name.
const VIEWER_ADAPTER: &str = "adapter-http-viewer";

/// Dep classes the read-only viewer adapter MUST NOT reach (I-VIEW-3 / I-NS-1):
/// the signing identity adapter + any PDS-write surface, AND the indexer-side
/// SERVER / store / ingest crates (slice-08; ADR-036/037). The viewer reads a
/// read-only store, queries a READ-ONLY network index, and renders HTML — it
/// cannot sign, publish, resolve peers, NOR host/build the index. `adapter-duckdb`
/// is intentionally NOT here: the viewer's read-only `StoreReadPort` is implemented
/// in `adapter-duckdb`, but the viewer ADAPTER crate links only `ports` +
/// `viewer-domain` + the pure `appview-domain`/`scraper-domain` cores (the cli
/// wires the concrete `DuckDbStoreReadAdapter`), so `adapter-http-viewer` never
/// reaches `adapter-duckdb` directly. `adapter-index-query` (the read-only index
/// CLIENT) is ALSO intentionally NOT here — the viewer MAY hold the read-only
/// `IndexQueryPort` for the `/search` route (the cli wires the concrete
/// `HttpIndexQueryAdapter` and passes it in as `Arc<dyn IndexQueryPort>`), so the
/// viewer adapter links only the trait (via `ports`), never the client adapter
/// directly. What it must NOT reach is the indexer's SERVER/store/ingest surface
/// (`adapter-xrpc-query-server`, `adapter-index-store`, `adapter-atproto-ingest`) —
/// the viewer is a query CLIENT, never the indexer itself (I-NS-1 / disjoint roots).
const VIEWER_FORBIDDEN_DEPS: &[&str] = &[
    "adapter-atproto-did",
    "adapter-atproto-pds",
    "adapter-xrpc-query-server",
    "adapter-index-store",
    "adapter-atproto-ingest",
];

/// Pure dep-graph check (I-VIEW-3 + the viewer capability invariant): the
/// `adapter-http-viewer` crate's transitive dep graph excludes the signing
/// identity + PDS-write surfaces, AND only the `cli` composition root links the
/// viewer adapter. Returns one [`Violation`] per offending edge; empty vec =
/// compliant. A missing viewer adapter crate is silently skipped (robust to
/// incremental workspace changes).
pub fn check_viewer_capability_boundary(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();

    if workspace.members.contains(VIEWER_ADAPTER) {
        let transitive = workspace.transitive_deps(VIEWER_ADAPTER);
        for forbidden in VIEWER_FORBIDDEN_DEPS {
            if transitive.contains(*forbidden) {
                violations.push(Violation {
                    package: VIEWER_ADAPTER.to_string(),
                    forbidden: (*forbidden).to_string(),
                    rule: "adapter-http-viewer MUST NOT depend on the signing identity / \
                           PDS-write surface NOR the indexer SERVER/store/ingest crates — \
                           the viewer holds no signing key and is a read-only query CLIENT, \
                           never the indexer (I-VIEW-3 / I-NS-1)",
                });
            }
        }
    }

    // Only `cli` may link the viewer adapter (the viewer capability invariant).
    // Every other member (pure cores, other adapters, the indexer root, but NOT
    // the exempt tooling) that reaches `adapter-http-viewer` is a violation.
    for member in &workspace.members {
        if member == COMPOSITION_ROOT
            || member == VIEWER_ADAPTER
            || ADAPTER_DEPENDENT_EXEMPT_MEMBERS.contains(&member.as_str())
        {
            continue;
        }
        if workspace.transitive_deps(member).contains(VIEWER_ADAPTER) {
            violations.push(Violation {
                package: member.clone(),
                forbidden: VIEWER_ADAPTER.to_string(),
                rule: "only `cli` may link `adapter-http-viewer` (the viewer capability \
                       invariant — the viewer is the single read-only surface)",
            });
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
    violations.extend(check_pure_core_no_io(
        workspace,
        "viewer-domain",
        "viewer-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (ADR-029; pure read-only view-model + maud HTML, allowed deps: maud + ports + the pure appview-domain/scoring cores)",
    ));
    violations.extend(check_pure_core_no_io(
        workspace,
        "publish-domain",
        "publish-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (ADR-062; the pure publish core — manifest marker, display projection, round-trip CID verdict)",
    ));
    violations.extend(check_pure_core_no_io(
        workspace,
        "review-domain",
        "review-domain MUST NOT transitively depend on tokio/reqwest/duckdb/keyring/atrium-* (ADR-072; the pure review/consent core)",
    ));
    violations.extend(check_ports_async_trait_only(workspace));
    violations.extend(check_no_adapter_depends_on_adapter(workspace));
    violations.extend(check_only_cli_depends_on_adapters(workspace));
    // Slice-05 (I-AV-5 / ADR-023 + I-3 extension): the indexer capability
    // boundary — `openlore-indexer` holds no signing/local-store; both
    // composition roots wire disjoint adapter sets.
    violations.extend(check_indexer_capability_boundary(workspace));
    // Slice-06 (I-VIEW-3 / ADR-028/030): the viewer capability boundary —
    // `adapter-http-viewer` holds no signing/PDS surface; only `cli` links it.
    violations.extend(check_viewer_capability_boundary(workspace));
    // bluesky-claim-review-app (ADR-072): the third root's disjoint adapters.
    violations.extend(check_review_app_capability_boundary(workspace));
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

/// The verify-only adapter source files the pubkey-seam guard scans (I-AV-6 /
/// ADR-026). Covers BOTH the slice-05 verify-only `IdentityResolvePort`
/// production resolve+decode path (`lib.rs`) AND `peer_resolve.rs` — the home of
/// the slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` seam plus the slice-05
/// `resolve_verification_key` dispatcher's seam read.
///
/// BROADENED (ADR-026): the seam is RELEASE-FORBIDDEN. It is RETAINED for the
/// slice-03 + slice-05 debug acceptance tests, but every `OPENLORE_PEER_PUBKEY_HEX_`
/// read MUST be `#[cfg(debug_assertions)]`-gated so it compiles ONLY in debug/test
/// builds, never release (where resolution falls through to the REAL PLC `z6Mk...`
/// decode). An UNGATED read ANYWHERE in `adapter-atproto-did` — `lib.rs` OR
/// `peer_resolve.rs` — would ship the verification-bypass seam in a release binary
/// and fails the rule. The earlier `lib.rs`-only narrowing existed only to dodge
/// the then-ungated `peer_resolve.rs` seam; that seam is now gated, so the scan
/// covers it too.
const PUBKEY_SEAM_GUARDED_SOURCES: &[&str] = &[
    "crates/adapter-atproto-did/src/lib.rs",
    "crates/adapter-atproto-did/src/peer_resolve.rs",
];

/// Effect shell for the pubkey-seam release-build guard (I-AV-6 / ADR-026): scan
/// the verify-only resolve+decode path for an UNGATED `OPENLORE_PEER_PUBKEY_HEX_`
/// read. A missing file is "nothing to scan". Reads the source as text (not
/// `syn`) because the rule is brace-depth / cfg-attribute aware, like the
/// autoconfirm guard — comments are stripped by the classifier's own line filter.
///
/// Public so the broadened-scope integration test (`xtask/tests/pubkey_seam.rs`)
/// can drive it against a temp workspace-root and assert `peer_resolve.rs` is in
/// scope (the load-bearing scope axis of this rule).
pub fn scan_pubkey_seam_guard(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut findings = Vec::new();
    for rel in PUBKEY_SEAM_GUARDED_SOURCES {
        let path = workspace_root.join(rel);
        if !path.is_file() {
            continue;
        }
        let src = std::fs::read_to_string(&path)?;
        if let Some(v) = classify_pubkey_seam_guard(&src) {
            findings.push(format!(
                "{}: `OPENLORE_PEER_PUBKEY_HEX_` seam read is NOT behind a `#[cfg(...)]` \
                 gate — it would ship in a release binary (I-AV-6 / ADR-026): {}",
                path.display(),
                v.line
            ));
        }
    }
    Ok(findings)
}

// -----------------------------------------------------------------------------
// Viewer active-set-read fault-injection seam release-build guard
// (slice-16 / US-SF-001 / Theme E / C-7 / ADR-053 §Earned-Trust)
// -----------------------------------------------------------------------------
//
// The viewer fault-injection env seams exist ONLY to let acceptance scenarios
// INDUCE a mid-request read failure and observe the production graceful-degrade
// path:
//   - slice-16 `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`: a `/search` active-set read
//     failure → empty set → all-NetworkUnfollowed (the slice-08 status quo).
//   - slice-17 `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`: a `GET /` peer-claims count
//     read failure → `.ok() → None → MISSING_COUNT_MARKER "—"` (ADR-054 D2; the other
//     two counts still render, the page stays 200, never a 5xx / fabricated 0).
//   - slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`: a `GET /` + `GET /claims`
//     countered-own-claims count read failure → `.ok() → None → render_countered(None)
//     → "(— countered)"` (ADR-055 D4; the own-claims count + the `/claims` list rows
//     still render, the page stays 200, never a 5xx / fabricated "(0 countered)").
//   - slice-20 `OPENLORE_VIEWER_FAIL_OWN_DIDS_READ` + `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ`:
//     a `/search` own-DID / cached-peer-DID presence read failure → `unwrap_or_default()
//     → empty set` → that arm (`You` / `UnsubscribedCache`) degrades INDEPENDENTLY to
//     `NetworkUnfollowed` while the other arms still resolve (ADR-057 D4 — the OQ-1
//     escalation: the real-binary subprocess harness cannot inject a per-read `Err` via a
//     fake `StoreReadPort`, so each read gets its own DISTINCT cfg-gated fault token).
// Exactly like the ADR-026 pubkey seam, EACH fault injector is RELEASE-FORBIDDEN:
// every read of EACH token MUST sit behind a `#[cfg(debug_assertions)]` gate so it
// compiles ONLY in debug/test builds and can NEVER force a degrade in a release
// binary. An UNGATED read in the viewer would ship a fault-injection backdoor — a
// guard violation.

/// The release-forbidden viewer fault-injection env-var tokens — EACH must sit
/// behind a `#[cfg(debug_assertions)]` gate. The active-set-read token is slice-16;
/// the peer-claims-count token is slice-17 (ADR-054 D2); the countered-own-count token is
/// slice-18 (ADR-055 D4); the countered-PEER-count token is slice-19 (ADR-056 D4 — a 4th
/// DISTINCT token so the PEER count fails INDEPENDENTLY of the slice-18 own count). New
/// per-count/per-read fault seams append their token here so the ONE guard covers them all.
const VIEWER_FAIL_SEAM_TOKENS: &[&str] = &[
    "OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ",
    "OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT",
    "OPENLORE_VIEWER_FAIL_COUNTERED_COUNT",
    "OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT",
    "OPENLORE_VIEWER_FAIL_OWN_DIDS_READ",
    "OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ",
];

/// The viewer source file the fault-seam guard scans.
const VIEWER_FAIL_SEAM_GUARDED_SOURCES: &[&str] = &["crates/adapter-http-viewer/src/lib.rs"];

/// Effect shell for the viewer fault-seam release-build guard (slice-16 / ADR-053 +
/// slice-17 / ADR-054): scan the viewer for an UNGATED read of ANY
/// [`VIEWER_FAIL_SEAM_TOKENS`] entry. Reuses the cfg-attribute-aware
/// [`classify_cfg_gated_token`] (same brace-depth / `#[cfg(...)]` discipline as the
/// pubkey + autoconfirm guards) once per token. A missing file is "nothing to scan".
pub fn scan_viewer_fail_seam_guard(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut findings = Vec::new();
    for rel in VIEWER_FAIL_SEAM_GUARDED_SOURCES {
        let path = workspace_root.join(rel);
        if !path.is_file() {
            continue;
        }
        let src = std::fs::read_to_string(&path)?;
        for token in VIEWER_FAIL_SEAM_TOKENS {
            if let Some(line) = classify_cfg_gated_token(&src, token) {
                findings.push(format!(
                    "{}: `{token}` fault-injection seam read is NOT behind a `#[cfg(...)]` gate \
                     — it would ship a degrade backdoor in a release binary (slice-16 / ADR-053 \
                     + slice-17 / ADR-054 + slice-18 / ADR-055 §Earned-Trust): {}",
                    path.display(),
                    line
                ));
            }
        }
    }
    Ok(findings)
}

/// The `atproto/` Worker's npm manifests the no-IPLD/CBOR guard reads
/// (serverless-philosophy-federation, ADR-062 §1/§2).
const ATPROTO_NPM_MANIFESTS: &[&str] = &["atproto/package.json", "atproto/package-lock.json"];

/// Pure rule (ADR-062): the `atproto/` Worker is an OPAQUE byte store — the
/// Rust `claim-domain` is the SOLE canonicalizer, so the Worker must never
/// be able to decode, re-encode or CID a record. Returns every banned
/// package named anywhere in an npm manifest (`package.json` dependency keys
/// or `package-lock.json` `node_modules/<name>` keys, transitive included):
/// any `@ipld/*`, `multiformats`, or `cbor*` package. Sorted, de-duplicated.
pub fn classify_atproto_npm_manifest(manifest_text: &str) -> Vec<String> {
    json_object_keys(manifest_text)
        .iter()
        .map(|key| npm_package_name_of(key))
        .filter(|name| is_banned_atproto_package(name))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Every JSON object key (a string literal followed by `:`) in `text`.
fn json_object_keys(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut keys = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        let end = end.min(bytes.len());
        let after = text[end.saturating_add(1).min(text.len())..].trim_start();
        if after.starts_with(':') {
            keys.push(text[start..end].to_string());
        }
        index = end + 1;
    }
    keys
}

/// `node_modules/a/node_modules/@scope/b` → `@scope/b`; a bare name is itself.
fn npm_package_name_of(key: &str) -> &str {
    key.rsplit_once("node_modules/")
        .map_or(key, |(_, name)| name)
}

fn is_banned_atproto_package(name: &str) -> bool {
    name.starts_with("@ipld/") || name == "multiformats" || name.starts_with("cbor")
}

/// Effect shell for the `atproto/` no-IPLD/CBOR guard: read the Worker's
/// `package.json` + `package-lock.json` and report every banned package. A
/// missing manifest is "nothing to scan".
pub fn scan_atproto_npm_dependencies(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut findings = Vec::new();
    for rel in ATPROTO_NPM_MANIFESTS {
        let path = workspace_root.join(rel);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        findings.extend(
            classify_atproto_npm_manifest(&text)
                .into_iter()
                .map(|name| {
                    format!(
                "{}: the atproto/ Worker depends on `{name}` — it must stay an OPAQUE byte \
                 store with no IPLD/CBOR/multiformats codec; Rust claim-domain is the SOLE \
                 canonicalizer (ADR-062 §1/§2)",
                path.display()
            )
                }),
        );
    }
    Ok(findings)
}

// -----------------------------------------------------------------------------
// `publish_write_capability_isolated` (serverless-philosophy-federation,
// ADR-062 §6 Earned-Trust STRUCTURAL layer, D-7 / DV-4)
// -----------------------------------------------------------------------------
//
// The instance capability is split by SUBTYPE (01-01): the READ-ONLY
// `InstanceReadPort` (probe / manifest / record reads) and its write-capable
// extension `PublishPort` (`put_record`). This rule is the structural half:
// ONLY the `openlore publish` verb module may name / construct / wire the
// write-capable port (`wiring::publish_port_for`, the owner-authed
// `HttpPublishAdapter::for_owner`, `dyn PublishPort`), plus the composition
// root that DEFINES that wiring. `verbs/peer_pull.rs`, the viewer, the card /
// read paths and every other crate must reach an instance ONLY through the
// read-only port. The crates that DEFINE the trait (`ports`) and implement it
// (`adapter-publish-http`) are out of scope.

/// The identifiers that name, construct or wire the write-capable publish
/// capability.
const PUBLISH_WRITE_CAPABILITY_TOKENS: &[&str] = &["publish_port_for", "for_owner", "PublishPort"];

/// The cross-instance pull verb (US-SF-006, ADR-062 §4/§6): it reads a PEER's
/// instance, so beyond the write-port tokens it may name no instance write
/// method and may not construct the HTTP adapter directly — its instance
/// transport can only be the read-only port handed out by
/// `wiring::instance_reader_for`.
const PEER_PULL_SOURCE: &str = "crates/cli/src/verbs/peer_pull.rs";

/// The extra identifiers forbidden in [`PEER_PULL_SOURCE`].
const PEER_PULL_INSTANCE_WRITE_TOKENS: &[&str] =
    &["put_record", "commit_manifest_entry", "HttpPublishAdapter"];

/// The ONLY sources (workspace-relative) allowed to reference a
/// [`PUBLISH_WRITE_CAPABILITY_TOKENS`] entry: the `openlore publish` verb and
/// the composition-root wiring it calls.
const PUBLISH_WRITE_CAPABILITY_ALLOWED_SOURCES: &[&str] = &[
    "crates/cli/src/verbs/publish.rs",
    "crates/cli/src/wiring.rs",
];

/// The crates that DEFINE the capability (trait + adapter) — not consumers,
/// so not scanned.
const PUBLISH_WRITE_CAPABILITY_DEFINING_CRATES: &[&str] =
    &["crates/ports/", "crates/adapter-publish-http/"];

/// Pure rule: every reference to the write-capable publish capability in
/// `source` (workspace-relative `rel_path`) that is NOT in an allowed or
/// defining source, as `line N: <code>` findings. Comment lines are ignored
/// (a doc mention is not a wiring).
pub fn classify_publish_write_capability(rel_path: &str, source: &str) -> Vec<String> {
    if !publish_write_capability_in_scope(rel_path) {
        return Vec::new();
    }
    let forbidden = forbidden_publish_tokens(rel_path);
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .filter_map(|(index, line)| {
            forbidden
                .iter()
                .find(|token| contains_word(line, token))
                .map(|token| format!("line {}: `{token}` in `{}`", index + 1, line.trim()))
        })
        .collect()
}

/// The write-capability tokens forbidden in `rel_path`: the write-port tokens
/// everywhere, plus the instance write methods / direct adapter construction
/// in the cross-instance pull verb.
fn forbidden_publish_tokens(rel_path: &str) -> Vec<&'static str> {
    let peer_pull_extra: &[&str] = if rel_path.replace('\\', "/") == PEER_PULL_SOURCE {
        PEER_PULL_INSTANCE_WRITE_TOKENS
    } else {
        &[]
    };
    PUBLISH_WRITE_CAPABILITY_TOKENS
        .iter()
        .chain(peer_pull_extra)
        .copied()
        .collect()
}

/// Is `rel_path` a CONSUMER of the instance capability that the rule polices
/// (i.e. neither an allowed publish source nor a defining crate)?
fn publish_write_capability_in_scope(rel_path: &str) -> bool {
    let rel_path = rel_path.replace('\\', "/");
    !PUBLISH_WRITE_CAPABILITY_ALLOWED_SOURCES.contains(&rel_path.as_str())
        && !PUBLISH_WRITE_CAPABILITY_DEFINING_CRATES
            .iter()
            .any(|prefix| rel_path.starts_with(prefix))
}

/// Effect shell for `publish_write_capability_isolated`: walk every
/// `crates/*/src/**/*.rs` under `workspace_root` and report each reference to
/// the write-capable publish capability outside the `openlore publish` verb.
pub fn scan_publish_write_capability(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let crates_dir = workspace_root.join("crates");
    if !crates_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut findings = Vec::new();
    for entry in walkdir::WalkDir::new(&crates_dir).sort_by_file_name() {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path
            .strip_prefix(workspace_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let is_crate_source = rel_path.split('/').nth(2) == Some("src");
        if !entry.file_type().is_file() || !rel_path.ends_with(".rs") || !is_crate_source {
            continue;
        }
        let source = std::fs::read_to_string(path)?;
        findings.extend(
            classify_publish_write_capability(&rel_path, &source)
                .into_iter()
                .map(|finding| {
                    format!(
                        "{rel_path}: {finding} — only the `openlore publish` verb may wire the \
                         write-capable PublishPort; every other verb and read path reaches an \
                         instance through the read-only InstanceReadPort \
                         (publish_write_capability_isolated, ADR-062 §6 / D-7)"
                    )
                }),
        );
    }
    Ok(findings)
}

// `contribution_links_append_only` + `adapter_github_holds_no_storage_port`
// (contributor-philosophy-inference DDD-5 / DDD-16).
//
// The `contribution_links` table is append-only: a link is added or refreshed
// by the snapshot upsert (`INSERT … ON CONFLICT … DO UPDATE`) and never
// deleted, dropped, truncated or rewritten by a bare `UPDATE`. And the
// GitHub adapter — the scraper's only network edge — names no storage or
// identity port, so a scrape can never write or sign on its own.

/// The append-only table the rule guards.
const CONTRIBUTION_LINKS_TABLE: &str = "contribution_links";

/// Statements that remove link rows or the table itself.
const DESTRUCTIVE_SQL_KEYWORDS: &[&str] = &["DELETE", "DROP", "TRUNCATE"];

/// Ports that store, sign or publish — none may appear in `adapter-github`.
const STORAGE_AND_IDENTITY_PORTS: &[&str] = &[
    "StoragePort",
    "PeerStoragePort",
    "StoreReadPort",
    "IndexStorePort",
    "ContributionLinkPort",
    "IdentityPort",
    "IdentityResolvePort",
    "PdsPort",
    "PublishPort",
];

/// Pure rule: `Some(excerpt)` iff one SQL literal names `contribution_links`
/// together with a `DELETE` / `DROP` / `TRUNCATE`, or with an `UPDATE` that
/// is not the upsert's `DO UPDATE` arm.
pub fn classify_contribution_links_literal(literal: &str) -> Option<String> {
    if !contains_word(literal, CONTRIBUTION_LINKS_TABLE) {
        return None;
    }
    let words: Vec<String> = literal
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_uppercase)
        .collect();
    let destructive = words
        .iter()
        .any(|word| DESTRUCTIVE_SQL_KEYWORDS.contains(&word.as_str()));
    let bare_update = words
        .iter()
        .enumerate()
        .any(|(index, word)| word == "UPDATE" && (index == 0 || words[index - 1] != "DO"));
    (destructive || bare_update).then(|| excerpt_of(literal))
}

/// `syn` visitor collecting string literals of PRODUCTION code only: items
/// gated `#[cfg(test)]` (and `#[test]` fns) are skipped, so a test fixture
/// that resets the table is not a violation.
struct ProductionLiteralCollector {
    literals: Vec<String>,
}

fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || (attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test"))
    })
}

impl<'ast> Visit<'ast> for ProductionLiteralCollector {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !is_test_only(&item.attrs) {
            syn::visit::visit_item_mod(self, item);
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !is_test_only(&item.attrs) {
            syn::visit::visit_item_fn(self, item);
        }
    }

    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        self.literals.push(lit.value());
    }
}

/// Every `.rs` file under `dir` (sorted), or none when `dir` is absent.
fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    walkdir::WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "rs"))
        .collect()
}

/// Effect shell for `contribution_links_append_only`: classify every
/// production SQL literal of `adapter-duckdb`.
pub fn scan_contribution_links_append_only(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut findings = Vec::new();
    for path in rust_sources_under(&workspace_root.join("crates/adapter-duckdb/src")) {
        let source = std::fs::read_to_string(&path)?;
        let file = syn::parse_file(&source)
            .map_err(|e| anyhow::anyhow!("syn parse {}: {e}", path.display()))?;
        let mut collector = ProductionLiteralCollector {
            literals: Vec::new(),
        };
        collector.visit_file(&file);
        findings.extend(
            collector
                .literals
                .iter()
                .filter_map(|literal| classify_contribution_links_literal(literal))
                .map(|excerpt| {
                    format!(
                        "{}: SQL deletes, drops, truncates or bare-updates `contribution_links`, \
                         which is append-only — only the snapshot upsert may write it \
                         (contribution_links_append_only, DDD-5 / DDD-16): {excerpt}",
                        path.display()
                    )
                }),
        );
    }
    Ok(findings)
}

/// Pure rule: every non-comment line of an `adapter-github` source naming a
/// storage / identity port, as `line N: <code>` findings.
pub fn classify_adapter_github_port_reference(source: &str) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .filter_map(|(index, line)| {
            STORAGE_AND_IDENTITY_PORTS
                .iter()
                .find(|port| contains_word(line, port))
                .map(|port| format!("line {}: `{port}` in `{}`", index + 1, line.trim()))
        })
        .collect()
}

/// Effect shell for `adapter_github_holds_no_storage_port`.
pub fn scan_adapter_github_port_references(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut findings = Vec::new();
    for path in rust_sources_under(&workspace_root.join("crates/adapter-github/src")) {
        let source = std::fs::read_to_string(&path)?;
        findings.extend(
            classify_adapter_github_port_reference(&source)
                .into_iter()
                .map(|finding| {
                    format!(
                        "{}: {finding} — adapter-github may name no storage / identity port; \
                         a scrape reads GitHub and nothing else \
                         (adapter_github_holds_no_storage_port, DDD-16 / I-SCR-1)",
                        path.display()
                    )
                }),
        );
    }
    Ok(findings)
}

// -----------------------------------------------------------------------------
// Review-app rules (bluesky-claim-review-app, component-boundaries §5)
// -----------------------------------------------------------------------------
//
// The hosted review app is the THIRD composition root (ADR-072). It reaches a
// user's PDS only through the OAuth client (create-only, I-BRA-8) and keeps
// private state in its own owner-scoped store (ADR-074). Rules:
//
// * `review_app_capability_boundary` (dep graph): the app links none of the
//   CLI's store/PDS/viewer/publish or the indexer's server/store/query
//   adapters; `cli` / `openlore-indexer` link neither review adapter nor the
//   app; only the app (and exempt tooling) reaches the two review adapters.
// * `review_app_holds_no_signing_identity` (source scan of the app).
// * `user_repo_write_is_create_only` (source scan of the OAuth adapter).
// * `review_store_owner_scoped_sql` (SQL literals of the review store).
// * `review_app_log_field_allowlist` (tracing events in the app + adapters).

const REVIEW_APP_ROOT: &str = "openlore-review-app";

/// The two adapters only the review app may link.
const REVIEW_APP_ADAPTERS: &[&str] = &["adapter-atproto-oauth", "adapter-review-store"];

/// The CLI / indexer capabilities the review app must never reach.
const REVIEW_APP_FORBIDDEN_DEPS: &[&str] = &[
    "adapter-duckdb",
    "adapter-atproto-pds",
    "adapter-publish-http",
    "adapter-http-viewer",
    "adapter-xrpc-query-server",
    "adapter-index-store",
    "adapter-index-query",
];

/// Pure dep-graph check for the third root. A missing crate skips its arm.
pub fn check_review_app_capability_boundary(workspace: &Workspace) -> Vec<Violation> {
    let mut violations = Vec::new();
    if workspace.members.contains(REVIEW_APP_ROOT) {
        let transitive = workspace.transitive_deps(REVIEW_APP_ROOT);
        violations.extend(
            REVIEW_APP_FORBIDDEN_DEPS
                .iter()
                .filter(|dep| transitive.contains(**dep))
                .map(|dep| Violation {
                    package: REVIEW_APP_ROOT.to_string(),
                    forbidden: (*dep).to_string(),
                    rule: "openlore-review-app MUST NOT reach the CLI's store/PDS/viewer/publish \
                           or the indexer's server/store/query adapters (ADR-072 / I-BRA-7)",
                }),
        );
    }
    for root in ["cli", INDEXER_ROOT] {
        if !workspace.members.contains(root) {
            continue;
        }
        let transitive = workspace.transitive_deps(root);
        violations.extend(
            REVIEW_APP_ADAPTERS
                .iter()
                .chain([REVIEW_APP_ROOT].iter())
                .filter(|dep| transitive.contains(**dep))
                .map(|dep| Violation {
                    package: root.to_string(),
                    forbidden: (*dep).to_string(),
                    rule: "cli / openlore-indexer MUST NOT link the review app or its OAuth / \
                           private-store adapters (ADR-072: disjoint composition roots)",
                }),
        );
    }
    for member in &workspace.members {
        let exempt = member == REVIEW_APP_ROOT
            || ADAPTER_DEPENDENT_EXEMPT_MEMBERS.contains(&member.as_str())
            || REVIEW_APP_ADAPTERS.contains(&member.as_str());
        if exempt {
            continue;
        }
        let direct = workspace.deps.get(member).cloned().unwrap_or_default();
        violations.extend(
            REVIEW_APP_ADAPTERS
                .iter()
                .filter(|dep| direct.contains(**dep))
                .map(|dep| Violation {
                    package: member.clone(),
                    forbidden: (*dep).to_string(),
                    rule: "only openlore-review-app may depend on adapter-atproto-oauth / \
                           adapter-review-store (ADR-072)",
                }),
        );
    }
    violations
}

/// Names the review app's sources must never contain (ADR-072 / D-5): the
/// claim-signing identity port, the OS keychain, a signing key type.
const REVIEW_APP_SIGNING_TOKENS: &[&str] = &["IdentityPort", "keyring", "SigningKey"];

/// XRPC verbs the OAuth adapter must never contain (I-BRA-8): only creates.
const NON_CREATE_REPO_VERBS: &[&str] = &["deleteRecord", "putRecord", "applyWrites"];

/// Pure: every non-comment line naming one of `tokens`, as findings.
pub fn classify_forbidden_tokens(source: &str, tokens: &[&str]) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .flat_map(|(index, line)| {
            tokens
                .iter()
                .filter(move |token| line.contains(**token))
                .map(move |token| format!("line {}: `{token}` in `{}`", index + 1, line.trim()))
        })
        .collect()
}

/// Tables keyed by `owner_did` in `review-app.duckdb` (ADR-074).
const REVIEW_OWNER_TABLES: &[&str] = &[
    "accounts",
    "github_links",
    "suggestions",
    "scan_runs",
    "plans",
    "oauth_sessions",
    "web_sessions",
];

/// Pure SQL rule for one review-store literal in `file_name`. Owner-table
/// SQL must name `owner_did`; `kpi_counters` must never name it; `DELETE`
/// lives only in the purge / expiry modules.
pub fn classify_review_store_sql_literal(file_name: &str, literal: &str) -> Option<String> {
    let sql = literal.to_ascii_lowercase();
    let is_sql = ["select ", "insert ", "update ", "delete ", "create table"]
        .iter()
        .any(|k| sql.contains(k));
    if !is_sql {
        return None;
    }
    if sql.contains("kpi_counters") {
        return sql
            .contains("owner_did")
            .then(|| format!("kpi_counters SQL names an owner: {}", excerpt_of(literal)));
    }
    let names_owner_table = REVIEW_OWNER_TABLES.iter().any(|t| sql.contains(t));
    if names_owner_table && !sql.contains("create table") && !sql.contains("owner_did") {
        return Some(format!(
            "owner-table SQL is not scoped by owner_did: {}",
            excerpt_of(literal)
        ));
    }
    if sql.contains("delete ") && !(file_name.contains("purge") || file_name.contains("expiry")) {
        return Some(format!(
            "DELETE outside the purge / expiry modules: {}",
            excerpt_of(literal)
        ));
    }
    None
}

/// Field names a tracing event in the review app must never use (DV-BRA-9).
const FORBIDDEN_LOG_FIELDS: &[&str] = &[
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

const TRACING_EVENT_MACROS: &[&str] = &[
    "info!(", "warn!(", "error!(", "debug!(", "trace!(", "event!(",
];

/// Pure: every tracing-event line that records a forbidden field name.
pub fn classify_log_fields(source: &str) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| TRACING_EVENT_MACROS.iter().any(|m| line.contains(m)))
        .filter_map(|(index, line)| {
            FORBIDDEN_LOG_FIELDS
                .iter()
                .find(|field| {
                    [
                        format!("{field} ="),
                        format!("{field}="),
                        format!("?{field}"),
                        format!("%{field}"),
                    ]
                    .iter()
                    .any(|shape| {
                        [" ", "(", ","]
                            .iter()
                            .any(|lead| line.contains(&format!("{lead}{shape}")))
                    })
                })
                .map(|field| {
                    format!(
                        "line {}: forbidden log field `{field}` in `{}`",
                        index + 1,
                        line.trim()
                    )
                })
        })
        .collect()
}

/// Effect shell: every review-app source rule over the real tree.
pub fn scan_review_app_rules(workspace_root: &Path) -> anyhow::Result<Vec<String>> {
    let app = workspace_root.join("crates/openlore-review-app/src");
    let oauth = workspace_root.join("crates/adapter-atproto-oauth/src");
    let store = workspace_root.join("crates/adapter-review-store/src");
    let mut findings = Vec::new();
    for path in rust_sources_under(&app) {
        let source = std::fs::read_to_string(&path)?;
        findings.extend(
            classify_forbidden_tokens(&source, REVIEW_APP_SIGNING_TOKENS)
                .into_iter()
                .map(|f| {
                    format!(
                        "{}: {f} — the review app holds no signing identity \
                         (review_app_holds_no_signing_identity, ADR-072 / D-5)",
                        path.display()
                    )
                }),
        );
    }
    for path in rust_sources_under(&oauth) {
        let source = std::fs::read_to_string(&path)?;
        findings.extend(
            classify_forbidden_tokens(&source, NON_CREATE_REPO_VERBS)
                .into_iter()
                .map(|f| {
                    format!(
                        "{}: {f} — user-repo writes are create-only \
                         (user_repo_write_is_create_only, I-BRA-8)",
                        path.display()
                    )
                }),
        );
    }
    for path in rust_sources_under(&store) {
        let source = std::fs::read_to_string(&path)?;
        let file = syn::parse_file(&source)
            .map_err(|e| anyhow::anyhow!("syn parse {}: {e}", path.display()))?;
        let mut collector = StringLiteralCollector {
            literals: Vec::new(),
        };
        collector.visit_file(&file);
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        findings.extend(
            collector
                .literals
                .iter()
                .filter_map(|literal| classify_review_store_sql_literal(&file_name, literal))
                .map(|f| {
                    format!(
                        "{}: {f} (review_store_owner_scoped_sql, I-BRA-1 / ADR-074)",
                        path.display()
                    )
                }),
        );
    }
    for path in [&app, &oauth, &store]
        .into_iter()
        .flat_map(|dir| rust_sources_under(dir))
    {
        let source = std::fs::read_to_string(&path)?;
        findings.extend(classify_log_fields(&source).into_iter().map(|f| {
            format!(
                "{}: {f} (review_app_log_field_allowlist, DV-BRA-9 / NFR-BRA-2)",
                path.display()
            )
        }));
    }
    Ok(findings)
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
    let pubkey_seam_findings = scan_pubkey_seam_guard(&workspace_root)?;
    let viewer_fail_seam_findings = scan_viewer_fail_seam_guard(&workspace_root)?;
    let atproto_npm_findings = scan_atproto_npm_dependencies(&workspace_root)?;
    let publish_write_findings = scan_publish_write_capability(&workspace_root)?;
    let contribution_links_findings = scan_contribution_links_append_only(&workspace_root)?;
    let adapter_github_port_findings = scan_adapter_github_port_references(&workspace_root)?;
    let review_app_findings = scan_review_app_rules(&workspace_root)?;

    let mut rendered: Vec<String> = dep_violations.iter().map(Violation::render).collect();
    rendered.extend(sql_findings);
    rendered.extend(index_store_sql_findings);
    rendered.extend(autoconfirm_findings);
    rendered.extend(pubkey_seam_findings);
    rendered.extend(viewer_fail_seam_findings);
    rendered.extend(atproto_npm_findings);
    rendered.extend(publish_write_findings);
    rendered.extend(contribution_links_findings);
    rendered.extend(adapter_github_port_findings);
    rendered.extend(review_app_findings);

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
    fn publish_domain_depending_on_reqwest_is_violation() {
        // ADR-062: the publish core is pure; the HTTP transport belongs to
        // `adapter-publish-http`, never to `publish-domain`.
        let w = ws(&[
            (
                "publish-domain",
                &["claim-domain", "lexicon", "ports", "reqwest"],
            ),
            ("claim-domain", &["serde"]),
        ]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "publish-domain" && x.forbidden == "reqwest"),
            "expected publish-domain→reqwest violation, got: {v:?}"
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

    // --- slice-06 viewer: pure-core arm + capability boundary ----------

    #[test]
    fn viewer_domain_with_maud_and_ports_is_allowed() {
        // ADR-029: `viewer-domain` is the slice-06 PURE read-only view-model +
        // HTML core. Its only deps are `maud` (allowlisted pure compile-time
        // template macro) + `ports`. The pure-core ban list must pass.
        let w = ws(&[
            ("viewer-domain", &["maud", "maud_macros", "ports"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "viewer-domain + maud must be an allowed pure-core shape (ADR-029), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn viewer_domain_depending_on_hyper_is_violation_via_tokio() {
        // The pure-core ban list is in force for viewer-domain: a render core
        // reaching the hyper/tokio runtime would mean HTML was served from the
        // pure core, not the effect shell.
        let w = ws(&[("viewer-domain", &["tokio"])]);
        let v = check_workspace(&w);
        assert!(
            v.iter()
                .any(|x| x.package == "viewer-domain" && x.forbidden == "tokio"),
            "expected viewer-domain→tokio violation, got: {v:?}"
        );
    }

    #[test]
    fn viewer_adapter_depending_on_signing_identity_is_violation() {
        // I-VIEW-3: the viewer holds no signing key — `adapter-http-viewer` must
        // not reach the signing identity adapter.
        let w = ws(&[
            ("adapter-http-viewer", &["adapter-atproto-did"]),
            ("adapter-atproto-did", &[]),
        ]);
        let v = check_viewer_capability_boundary(&w);
        assert!(
            v.iter().any(|x| {
                x.package == "adapter-http-viewer" && x.forbidden == "adapter-atproto-did"
            }),
            "expected adapter-http-viewer→adapter-atproto-did (I-VIEW-3) violation, got: {v:?}"
        );
    }

    #[test]
    fn viewer_adapter_depending_on_pds_write_surface_is_violation() {
        // I-VIEW-3 (the pds-exclusion axis, AC #3): `adapter-atproto-pds` carries
        // the PDS-write surface (`create_record`/`put_record`). The read-only
        // viewer adapter must NOT reach it — pinning this independently of the
        // signing-identity (`adapter-atproto-did`) axis so weakening EITHER entry
        // of `VIEWER_FORBIDDEN_DEPS` is caught.
        let w = ws(&[
            ("adapter-http-viewer", &["adapter-atproto-pds"]),
            ("adapter-atproto-pds", &[]),
        ]);
        let v = check_viewer_capability_boundary(&w);
        assert!(
            v.iter().any(|x| {
                x.package == "adapter-http-viewer" && x.forbidden == "adapter-atproto-pds"
            }),
            "expected adapter-http-viewer→adapter-atproto-pds (I-VIEW-3 PDS-write) violation, got: {v:?}"
        );
    }

    #[test]
    fn viewer_adapter_transitively_reaching_signing_surface_is_violation() {
        // The viewer capability boundary is TRANSITIVE: even if the viewer adapter
        // reaches a signing/PDS surface via an intermediate crate, it still holds
        // the forbidden capability (mirrors the indexer's transitive guard).
        let w = ws(&[
            ("adapter-http-viewer", &["some-helper"]),
            ("some-helper", &["adapter-atproto-pds"]),
            ("adapter-atproto-pds", &[]),
        ]);
        let v = check_viewer_capability_boundary(&w);
        assert!(
            v.iter().any(|x| {
                x.package == "adapter-http-viewer" && x.forbidden == "adapter-atproto-pds"
            }),
            "a TRANSITIVE adapter-http-viewer→adapter-atproto-pds path MUST be a violation, got: {v:?}"
        );
    }

    #[test]
    fn only_cli_may_link_the_viewer_adapter() {
        // The viewer capability invariant: cli links it (OK); a pure core or the
        // indexer root linking it is a violation.
        let ok = ws(&[
            ("cli", &["adapter-http-viewer"]),
            ("adapter-http-viewer", &["ports", "viewer-domain"]),
            ("viewer-domain", &["maud", "ports"]),
            ("ports", &["claim-domain"]),
        ]);
        assert!(
            check_viewer_capability_boundary(&ok).is_empty(),
            "cli linking the viewer adapter must be allowed, got: {:?}",
            check_viewer_capability_boundary(&ok)
        );

        let bad = ws(&[
            ("openlore-indexer", &["adapter-http-viewer"]),
            ("adapter-http-viewer", &[]),
        ]);
        let v = check_viewer_capability_boundary(&bad);
        assert!(
            v.iter().any(|x| {
                x.package == "openlore-indexer" && x.forbidden == "adapter-http-viewer"
            }),
            "expected openlore-indexer→adapter-http-viewer (only-cli) violation, got: {v:?}"
        );
    }

    // --- slice-08 network-search: pure→pure edge + read-only IndexQueryPort -----

    #[test]
    fn viewer_domain_depending_on_appview_domain_is_an_allowed_pure_to_pure_edge() {
        // slice-08 delta (a) / ADR-037: `viewer-domain` projects the pure slice-05
        // `appview-domain` `NetworkSearchResult` into the `#search-results` fragment
        // (REUSING `compose_results`). `appview-domain` is itself a pure core
        // (claim-domain + pure chrono/serde), so the edge introduces NO banned I/O —
        // the pure-core arm must still pass for `viewer-domain`.
        let w = ws(&[
            ("viewer-domain", &["maud", "ports", "appview-domain"]),
            ("appview-domain", &["claim-domain", "serde", "chrono"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "viewer-domain → appview-domain must be an allowed pure→pure edge (ADR-037), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn viewer_domain_depending_on_scoring_is_an_allowed_pure_to_pure_edge() {
        // slice-09 delta / ADR-039/040/041: `viewer-domain` projects the REUSED
        // slice-04 `scoring` core's `WeightedView` (ranked `WeightedPairing`s + their
        // per-claim `Contribution` decomposition) into the `#score-results` fragment
        // (the `/score` contributor-score view). `scoring` is itself a pure
        // closed-form weight core (ports + claim-domain + pure chrono/serde — no
        // banned I/O, WD-71/WD-82/ADR-022), so the edge introduces NO I/O — the
        // pure-core arm must still pass for `viewer-domain`. Mirrors the
        // `viewer-domain → appview-domain` pure→pure edge above.
        let w = ws(&[
            (
                "viewer-domain",
                &["maud", "ports", "appview-domain", "scoring"],
            ),
            ("scoring", &["ports", "claim-domain", "serde", "chrono"]),
            ("appview-domain", &["claim-domain", "serde", "chrono"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "viewer-domain → scoring must be an allowed pure→pure edge (ADR-039/040/041), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn viewer_domain_depending_on_claim_domain_is_an_allowed_pure_to_pure_edge() {
        // slice-10 delta / ADR-042/043/044/045: `viewer-domain` REUSES `claim-domain`'s
        // display-only `confidence_bucket` (the WD-10 thresholds, one SSOT) to render
        // the per-claim confidence bucket on every graph-traversal edge. `claim-domain`
        // is the pure claim core (serde + the audited pure-core allowlist — no banned
        // I/O), so the edge introduces NO I/O — the pure-core arm must still pass for
        // `viewer-domain`. Mirrors the `viewer-domain → appview-domain` / `→ scoring`
        // pure→pure edges above.
        let w = ws(&[
            (
                "viewer-domain",
                &["maud", "ports", "appview-domain", "scoring", "claim-domain"],
            ),
            ("scoring", &["ports", "claim-domain", "serde", "chrono"]),
            ("appview-domain", &["claim-domain", "serde", "chrono"]),
            ("ports", &["async-trait", "claim-domain"]),
            ("claim-domain", &["serde"]),
        ]);
        assert!(
            check_workspace(&w).is_empty(),
            "viewer-domain → claim-domain must be an allowed pure→pure edge (ADR-045), got: {:?}",
            check_workspace(&w)
        );
    }

    #[test]
    fn viewer_adapter_may_hold_the_read_only_index_query_client() {
        // slice-08 delta (b) / I-NS-1: the viewer adapter MAY reach the read-only
        // index-query CLIENT (`adapter-index-query`) — it holds the read-only
        // `IndexQueryPort` for the `/search` route. That is NOT a capability breach
        // (the cli wires the concrete adapter and passes `Arc<dyn IndexQueryPort>`;
        // in practice the viewer adapter links only the trait via `ports`, but even
        // a direct client edge is permitted — the client is read-only, holds no
        // signing key). The pure `appview-domain` search-core edge is also allowed.
        let ok = ws(&[
            ("cli", &["adapter-http-viewer", "adapter-index-query"]),
            (
                "adapter-http-viewer",
                &["ports", "viewer-domain", "appview-domain"],
            ),
            ("adapter-index-query", &["ports", "reqwest"]),
            ("viewer-domain", &["maud", "ports", "appview-domain"]),
            ("appview-domain", &["claim-domain"]),
            ("ports", &["claim-domain"]),
        ]);
        assert!(
            check_viewer_capability_boundary(&ok).is_empty(),
            "the viewer adapter holding the read-only IndexQueryPort client must be allowed \
             (I-NS-1), got: {:?}",
            check_viewer_capability_boundary(&ok)
        );
    }

    #[test]
    fn viewer_adapter_reaching_the_indexer_server_store_or_ingest_is_a_violation() {
        // slice-08 delta (b) / I-NS-1: the viewer is a read-only query CLIENT, NEVER
        // the indexer. Reaching the indexer's HTTP SERVER / store / ingest surface is
        // a capability breach (the viewer would then host/build the index). Pin each
        // of the three indexer-side crates independently so weakening any single
        // entry of VIEWER_FORBIDDEN_DEPS is caught.
        for forbidden in [
            "adapter-xrpc-query-server",
            "adapter-index-store",
            "adapter-atproto-ingest",
        ] {
            let w = ws(&[("adapter-http-viewer", &[forbidden]), (forbidden, &[])]);
            let v = check_viewer_capability_boundary(&w);
            assert!(
                v.iter()
                    .any(|x| x.package == "adapter-http-viewer" && x.forbidden == forbidden),
                "expected adapter-http-viewer→{forbidden} (I-NS-1 indexer-server breach) \
                 violation, got: {v:?}"
            );
        }
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

    // --- atproto/ Worker: no IPLD/CBOR/multiformats (ADR-062 §1/§2) ---------

    fn lockfile_with(packages: &[String]) -> String {
        let entries: Vec<String> = packages
            .iter()
            .map(|name| format!("\"node_modules/{name}\": {{ \"version\": \"1.0.0\" }}"))
            .collect();
        format!(
            "{{ \"name\": \"openlore-atproto\", \"lockfileVersion\": 3, \"packages\": {{ \
             \"\": {{ \"devDependencies\": {{ \"wrangler\": \"4.0.0\" }} }}, {} }} }}",
            entries.join(", ")
        )
    }

    fn safe_package_name() -> impl proptest::strategy::Strategy<Value = String> {
        proptest::string::string_regex("(@[a-z]{2,6}/)?[a-d][a-z-]{1,10}").expect("regex")
    }

    fn banned_package_name() -> impl proptest::strategy::Strategy<Value = String> {
        use proptest::prelude::*;
        prop_oneof![
            Just("multiformats".to_string()),
            "[a-z-]{1,10}".prop_map(|rest| format!("@ipld/{rest}")),
            "[a-z-]{0,6}".prop_map(|rest| format!("cbor{rest}")),
        ]
    }

    proptest::proptest! {
        /// Property: a banned codec package ANYWHERE in the Worker's
        /// dependency tree (direct, scoped, or nested `node_modules`) is
        /// reported, and ONLY the banned packages are reported.
        #[test]
        fn atproto_guard_reports_exactly_the_banned_codec_packages(
            safe in proptest::collection::vec(safe_package_name(), 0..6),
            banned in banned_package_name(),
            nested_under in proptest::option::of(safe_package_name()),
        ) {
            let banned_key = match &nested_under {
                Some(parent) => format!("{parent}/node_modules/{banned}"),
                None => banned.clone(),
            };
            let mut packages = safe.clone();
            packages.push(banned_key);
            let clean = classify_atproto_npm_manifest(&lockfile_with(&safe));
            let dirty = classify_atproto_npm_manifest(&lockfile_with(&packages));
            proptest::prop_assert_eq!(clean, Vec::<String>::new());
            proptest::prop_assert_eq!(dirty, vec![banned]);
        }
    }

    // --- publish_write_capability_isolated (ADR-062 §6, D-7) -------------

    #[test]
    fn publish_write_guard_fires_on_a_non_publish_verb_or_read_path_wiring_the_write_port() {
        let pull_wiring_writes = "use crate::wiring;\n\
             pub fn run(url: &str) {\n\
                 let instance = wiring::publish_port_for(url).unwrap();\n\
             }\n";
        let viewer_holding_writes = "struct Card { instance: Box<dyn ports::PublishPort> }\n";
        let owner_adapter =
            "let w = adapter_publish_http::HttpPublishAdapter::for_owner(url, token);\n";

        let pull = classify_publish_write_capability(
            "crates/cli/src/verbs/peer_pull.rs",
            pull_wiring_writes,
        );
        let viewer = classify_publish_write_capability(
            "crates/adapter-http-viewer/src/card.rs",
            viewer_holding_writes,
        );
        let other_verb =
            classify_publish_write_capability("crates/cli/src/verbs/search.rs", owner_adapter);

        assert_eq!(
            pull.len(),
            1,
            "peer_pull wiring the write port must be reported: {pull:?}"
        );
        assert!(
            pull[0].contains("line 3") && pull[0].contains("publish_port_for"),
            "{pull:?}"
        );
        assert_eq!(
            viewer.len(),
            1,
            "a read path holding `dyn PublishPort` must be reported: {viewer:?}"
        );
        assert_eq!(
            other_verb.len(),
            1,
            "a non-publish verb constructing the owner adapter: {other_verb:?}"
        );
    }

    #[test]
    fn peer_pull_may_reach_a_peer_instance_only_through_the_read_only_port() {
        let calls_a_write_method = "fn pull(instance: &dyn InstanceReadPort) {\n\
             instance.put_record(&cid, &bytes);\n\
             }\n";
        let commits = "instance.commit_manifest_entry(&cid, &bytes, &entry);\n";
        let constructs_adapter =
            "let instance = adapter_publish_http::HttpPublishAdapter::for_instance(url);\n";
        let read_only = "// never `put_record` / `HttpPublishAdapter` here\n\
             let instance: Box<dyn InstanceReadPort> = wiring::instance_reader_for(url);\n\
             let bytes = instance.get_record(&cid);\n";

        for (source, token) in [
            (calls_a_write_method, "put_record"),
            (commits, "commit_manifest_entry"),
            (constructs_adapter, "HttpPublishAdapter"),
        ] {
            let findings = classify_publish_write_capability(PEER_PULL_SOURCE, source);
            assert_eq!(findings.len(), 1, "{token} must be reported: {findings:?}");
            assert!(findings[0].contains(token), "{findings:?}");
        }
        assert_eq!(
            classify_publish_write_capability(PEER_PULL_SOURCE, read_only),
            Vec::<String>::new(),
            "the read-only port via wiring is the allowed peer-instance transport"
        );
        // The tighter peer-pull set applies to peer_pull.rs only (a test
        // double elsewhere may well name its own `put_record` route).
        assert_eq!(
            classify_publish_write_capability(
                "crates/test-support/src/fake_instance.rs",
                constructs_adapter
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn publish_write_guard_is_quiet_for_the_publish_verb_comments_and_the_read_only_port() {
        let publish_verb = "let instance = wiring::publish_port_for(&url)?;\n\
             fn push_one(instance: &dyn PublishPort) {}\n";
        let read_only_elsewhere = "// the write-capable `PublishPort` is wired only by `publish`\n\
             /// never `publish_port_for` here\n\
             let instance: Box<dyn InstanceReadPort> = wiring::instance_reader_for(url);\n";

        assert_eq!(
            classify_publish_write_capability("crates/cli/src/verbs/publish.rs", publish_verb),
            Vec::<String>::new()
        );
        assert_eq!(
            classify_publish_write_capability("crates/cli/src/wiring.rs", publish_verb),
            Vec::<String>::new()
        );
        assert_eq!(
            classify_publish_write_capability("crates/ports/src/publish.rs", publish_verb),
            Vec::<String>::new()
        );
        assert_eq!(
            classify_publish_write_capability(
                "crates/cli/src/verbs/peer_pull.rs",
                read_only_elsewhere
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn publish_write_guard_scan_reports_a_violating_tree_and_passes_the_real_tree() {
        let violating = tempfile::tempdir().expect("tempdir");
        let verbs = violating.path().join("crates/cli/src/verbs");
        std::fs::create_dir_all(&verbs).expect("mkdir");
        std::fs::write(
            verbs.join("peer_pull.rs"),
            "let w = wiring::publish_port_for(url);\n",
        )
        .expect("write");
        std::fs::write(
            verbs.join("publish.rs"),
            "let w = wiring::publish_port_for(url);\n",
        )
        .expect("write");
        let findings = scan_publish_write_capability(violating.path()).expect("scan");
        assert_eq!(
            findings.len(),
            1,
            "only peer_pull.rs is a violation: {findings:?}"
        );
        assert!(findings[0].contains("peer_pull.rs"), "{findings:?}");

        let real_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        assert_eq!(
            scan_publish_write_capability(real_root).expect("scan real tree"),
            Vec::<String>::new(),
            "the real workspace keeps the write capability inside `openlore publish`"
        );
    }

    #[test]
    fn atproto_guard_passes_the_dumb_worker_manifest() {
        let package_json = r#"{
          "name": "openlore-atproto",
          "description": "opaque store; no @ipld/dag-cbor, no multiformats, no cbor",
          "devDependencies": { "typescript": "5.6.3", "wrangler": "4.0.0",
                               "@cloudflare/workers-types": "4.0.0" }
        }"#;
        assert_eq!(
            classify_atproto_npm_manifest(package_json),
            Vec::<String>::new()
        );
        let with_codec = package_json.replace("\"typescript\"", "\"@ipld/dag-cbor\"");
        assert_eq!(
            classify_atproto_npm_manifest(&with_codec),
            vec!["@ipld/dag-cbor".to_string()]
        );
    }

    // --- contribution_links_append_only (DDD-5 / DDD-16) ------------------

    fn real_workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
    }

    #[test]
    fn append_only_guard_fires_on_every_destructive_statement_against_the_links() {
        for synthetic in [
            "DELETE FROM contribution_links WHERE repo_key = ?",
            "drop table contribution_links",
            "TRUNCATE contribution_links",
            "UPDATE contribution_links SET first_observed_at = ?",
            "update contribution_links\n   set rank = 1",
        ] {
            assert!(
                classify_contribution_links_literal(synthetic).is_some(),
                "must fire on: {synthetic}"
            );
        }
    }

    #[test]
    fn append_only_guard_allows_the_upsert_reads_and_other_tables() {
        for allowed in [
            "INSERT INTO contribution_links (repo_key) VALUES (?) \
             ON CONFLICT (repo_key, person_key) DO UPDATE SET rank = excluded.rank",
            "SELECT rank FROM contribution_links WHERE repo_key = ?",
            "CREATE TABLE IF NOT EXISTS contribution_links (repo_key VARCHAR)",
            "DELETE FROM claims WHERE cid = ?",
            "UPDATE schema_version SET version = 5",
        ] {
            assert_eq!(
                classify_contribution_links_literal(allowed),
                None,
                "must allow: {allowed}"
            );
        }
    }

    #[test]
    fn append_only_scan_reports_production_sql_ignores_test_modules_and_passes_the_real_tree() {
        let violating = tempfile::tempdir().expect("tempdir");
        let src = violating.path().join("crates/adapter-duckdb/src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(
            src.join("links.rs"),
            "const PRUNE: &str = \"DELETE FROM contribution_links WHERE rank > 30\";\n\
             #[cfg(test)]\n\
             mod tests {\n\
                 const RESET: &str = \"DROP TABLE contribution_links\";\n\
             }\n",
        )
        .expect("write");
        let findings = scan_contribution_links_append_only(violating.path()).expect("scan");
        assert_eq!(
            findings.len(),
            1,
            "only the production DELETE: {findings:?}"
        );
        assert!(findings[0].contains("links.rs"), "{findings:?}");

        assert_eq!(
            scan_contribution_links_append_only(real_workspace_root()).expect("scan real tree"),
            Vec::<String>::new(),
            "the real adapter only upserts contribution_links"
        );
    }

    // --- adapter_github_holds_no_storage_port (DDD-16 / I-SCR-1) ----------

    #[test]
    fn github_port_guard_fires_on_a_storage_or_identity_port_but_not_on_comments() {
        let source = "//! holds NO `StoragePort`/`IdentityPort` reference\n\
             use ports::GithubPort;\n\
             struct Leaky { links: Box<dyn ports::ContributionLinkPort> }\n\
             fn sign(id: &dyn IdentityPort) {}\n";
        let findings = classify_adapter_github_port_reference(source);
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(
            findings[0].contains("line 3") && findings[0].contains("ContributionLinkPort"),
            "{findings:?}"
        );
        assert!(
            findings[1].contains("line 4") && findings[1].contains("IdentityPort"),
            "{findings:?}"
        );
    }

    #[test]
    fn github_port_scan_reports_a_violating_tree_and_passes_the_real_tree() {
        let violating = tempfile::tempdir().expect("tempdir");
        let src = violating.path().join("crates/adapter-github/src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(
            src.join("client.rs"),
            "pub struct GithubAdapter { store: Box<dyn ports::StoragePort> }\n",
        )
        .expect("write");
        let findings = scan_adapter_github_port_references(violating.path()).expect("scan");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].contains("client.rs") && findings[0].contains("StoragePort"),
            "{findings:?}"
        );

        assert_eq!(
            scan_adapter_github_port_references(real_workspace_root()).expect("scan real tree"),
            Vec::<String>::new(),
            "the real adapter-github names no storage / identity port"
        );
    }
}

#[cfg(test)]
mod review_app_rule_tests {
    //! Synthetic-violation tests: each review-app rule fires on a hand-built
    //! violation and stays silent on the compliant shape.
    use super::*;

    fn ws(rows: &[(&str, &[&str])]) -> Workspace {
        Workspace {
            members: rows.iter().map(|(n, _)| (*n).to_string()).collect(),
            deps: rows
                .iter()
                .map(|(n, ds)| {
                    (
                        (*n).to_string(),
                        ds.iter().map(|d| (*d).to_string()).collect(),
                    )
                })
                .collect(),
        }
    }

    fn compliant() -> Vec<(&'static str, &'static [&'static str])> {
        vec![
            (
                "openlore-review-app",
                &[
                    "review-domain",
                    "adapter-atproto-oauth",
                    "adapter-review-store",
                ],
            ),
            ("review-domain", &["maud"]),
            ("adapter-atproto-oauth", &["ports"]),
            ("adapter-review-store", &["ports"]),
            ("cli", &["adapter-duckdb"]),
            ("openlore-indexer", &["adapter-index-store"]),
        ]
    }

    #[test]
    fn the_compliant_three_root_shape_passes_every_dep_rule() {
        assert_eq!(check_workspace(&ws(&compliant())), Vec::new());
    }

    #[test]
    fn the_review_app_reaching_a_cli_capability_is_a_violation() {
        for forbidden in REVIEW_APP_FORBIDDEN_DEPS {
            let mut rows = compliant();
            let leaked: &'static [&'static str] = Box::leak(Box::new([*forbidden]));
            rows[0] = ("openlore-review-app", leaked);
            let v = check_review_app_capability_boundary(&ws(&rows));
            assert!(
                v.iter()
                    .any(|x| x.package == "openlore-review-app" && x.forbidden == *forbidden),
                "{forbidden}: {v:?}"
            );
        }
    }

    #[test]
    fn cli_or_indexer_linking_a_review_adapter_is_a_violation() {
        for root in ["cli", "openlore-indexer"] {
            for guarded in [
                "adapter-atproto-oauth",
                "adapter-review-store",
                "openlore-review-app",
            ] {
                let mut rows = compliant();
                let leaked: &'static [&'static str] = Box::leak(Box::new([guarded]));
                rows.retain(|(n, _)| *n != root);
                rows.push((root, leaked));
                let v = check_review_app_capability_boundary(&ws(&rows));
                assert!(
                    v.iter()
                        .any(|x| x.package == root && x.forbidden == guarded),
                    "{root} -> {guarded}: {v:?}"
                );
            }
        }
    }

    #[test]
    fn another_crate_depending_on_a_review_adapter_is_a_violation() {
        let mut rows = compliant();
        rows.push(("viewer-domain", &["adapter-review-store"]));
        let v = check_review_app_capability_boundary(&ws(&rows));
        assert!(v
            .iter()
            .any(|x| x.package == "viewer-domain" && x.forbidden == "adapter-review-store"));
    }

    #[test]
    fn review_domain_reaching_io_is_a_violation() {
        let mut rows = compliant();
        rows[1] = ("review-domain", &["tokio"]);
        let v = check_workspace(&ws(&rows));
        assert!(v
            .iter()
            .any(|x| x.package == "review-domain" && x.forbidden == "tokio"));
    }

    #[test]
    fn the_review_app_is_a_composition_root() {
        assert!(COMPOSITION_ROOTS.contains(&"openlore-review-app"));
        let mut rows = compliant();
        rows.push(("adapter-github", &[]));
        rows[0].1 = &["adapter-github", "adapter-review-store"];
        assert!(check_only_cli_depends_on_adapters(&ws(&rows)).is_empty());
    }

    #[test]
    fn a_signing_identity_in_the_app_is_a_finding() {
        for token in REVIEW_APP_SIGNING_TOKENS {
            let source =
                format!("use ports::Other;\nlet k = {token}::new();\n// {token} in a comment\n");
            let findings = classify_forbidden_tokens(&source, REVIEW_APP_SIGNING_TOKENS);
            assert_eq!(findings.len(), 1, "{token}: {findings:?}");
        }
        assert!(classify_forbidden_tokens(
            "let key = ClientKey::generate();",
            REVIEW_APP_SIGNING_TOKENS
        )
        .is_empty());
    }

    #[test]
    fn a_non_create_repo_verb_in_the_oauth_adapter_is_a_finding() {
        for verb in NON_CREATE_REPO_VERBS {
            let source = format!("let nsid = \"com.atproto.repo.{verb}\";\n");
            assert_eq!(
                classify_forbidden_tokens(&source, NON_CREATE_REPO_VERBS).len(),
                1
            );
        }
        assert!(classify_forbidden_tokens(
            "let nsid = \"com.atproto.repo.createRecord\";",
            NON_CREATE_REPO_VERBS
        )
        .is_empty());
    }

    #[test]
    fn owner_table_sql_without_owner_scope_is_a_finding() {
        for table in REVIEW_OWNER_TABLES {
            let unscoped = format!("SELECT * FROM {table} WHERE id = ?");
            let scoped = format!("SELECT * FROM {table} WHERE owner_did = ?");
            let ddl = format!("CREATE TABLE IF NOT EXISTS {table} (x INT)");
            assert!(
                classify_review_store_sql_literal("lib.rs", &unscoped).is_some(),
                "{table}"
            );
            assert!(
                classify_review_store_sql_literal("lib.rs", &scoped).is_none(),
                "{table}"
            );
            assert!(
                classify_review_store_sql_literal("schema.rs", &ddl).is_none(),
                "{table}"
            );
        }
    }

    #[test]
    fn kpi_counters_naming_an_owner_is_a_finding() {
        let owned = "CREATE TABLE kpi_counters (day DATE, owner_did VARCHAR)";
        let anonymous = "INSERT INTO kpi_counters (day, event, count) VALUES (?, ?, 1)";
        assert!(classify_review_store_sql_literal("schema.rs", owned).is_some());
        assert!(classify_review_store_sql_literal("kpi.rs", anonymous).is_none());
    }

    #[test]
    fn delete_outside_purge_or_expiry_is_a_finding() {
        let delete = "DELETE FROM plans WHERE owner_did = ?";
        assert!(classify_review_store_sql_literal("lib.rs", delete).is_some());
        assert!(classify_review_store_sql_literal("purge.rs", delete).is_none());
        assert!(classify_review_store_sql_literal("expiry.rs", delete).is_none());
    }

    #[test]
    fn a_forbidden_log_field_is_a_finding() {
        for field in FORBIDDEN_LOG_FIELDS {
            for shape in [
                format!("tracing::info!({field} = 1, \"x\");"),
                format!("warn!(probe = 1, ?{field}, \"x\");"),
                format!("error!(%{field}, \"x\");"),
            ] {
                assert_eq!(classify_log_fields(&shape).len(), 1, "{shape}");
            }
        }
        assert!(classify_log_fields("info!(probe = \"github\", \"app.ready\");").is_empty());
        assert!(classify_log_fields("let token = 1;").is_empty());
    }
}
