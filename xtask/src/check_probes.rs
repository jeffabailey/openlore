//! `cargo xtask check-probes` — enforces ADR-009 D-10 layer-(b) structural
//! contract: every `impl <Port> for <Adapter>` block in `crates/adapter-*`
//! exposes a `probe()` method whose body is **not** a stub.
//!
//! ## Why a structural check at all
//!
//! The Earned-Trust §self-application invariant says adapters must
//! demonstrate readiness through their probe, not merely declare it.
//! Acceptance tests cover the *behavior* of each probe end-to-end, but
//! nothing previously stopped a future contributor from regressing a
//! probe body to `Ok(_)` and shipping it. This xtask plus the
//! `scripts/check-probes.sh` pre-commit hook closes that loop: the same
//! check runs locally and in CI before any byte of adapter source lands.
//!
//! ## Architecture
//!
//! Mirrors `check_arch.rs`:
//!
//! - **Pure core**: [`classify_probe_body`] takes a `&syn::ImplItemFn`
//!   plus the implementor type name and returns a [`Classification`]
//!   variant. No I/O, no filesystem. This is what the unit tests exercise
//!   against hand-rolled `syn::parse_str` source snippets.
//! - **Effect shell**: [`run`] walks `crates/adapter-*/src/**/*.rs`,
//!   parses each file with `syn`, finds every `impl Trait for Type` block
//!   where the trait name ends in `Port`, classifies the `probe` method,
//!   and renders violations to stderr.
//!
//! ## Classification rules
//!
//! A body REJECTS as a stub when it matches any of:
//!
//! 1. Bare `todo!()`, `unimplemented!()`, or `panic!(...)`.
//! 2. A single expression `Ok(ProbeOutcome::Ok)` (or `Ok(_)` /
//!    `ProbeOutcome::Ok`) — no real work was done before declaring the
//!    adapter ready.
//! 3. A body that contains nothing but comments / a final trivial
//!    success literal.
//!
//! A body ACCEPTS when it contains at least one statement that performs
//! work: a `let` binding, a `match`, an `if`, a `for`, a method/function
//! call, or any compound expression. Heuristic: more than one statement
//! OR a single non-trivial expression.
//!
//! ## Documented exception
//!
//! `SystemClockAdapter` is allowed a trivial `ProbeOutcome::Ok` body
//! because `chrono::Utc::now()` has no failure modes a probe could
//! meaningfully gate on (ADR-009 §degenerate-adapter, mirrored in
//! `adapter-system-clock/src/lib.rs` module comment). The classifier
//! special-cases this one implementor by name.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use syn::visit::Visit;
use syn::{Expr, ImplItem, ImplItemFn, Item, ItemImpl, Macro, Stmt};

/// Adapters that are allowed a degenerate `Ok` probe body. ADR-009
/// §degenerate-adapter — see module doc. Match by the implementor
/// type's last path segment (i.e. `SystemClockAdapter`, not the full
/// path).
const DEGENERATE_ADAPTERS: &[&str] = &["SystemClockAdapter"];

/// Suffix every legitimate port trait shares. Used to filter
/// `impl Trait for Type` blocks down to the ones the check cares about
/// (`StoragePort`, `IdentityPort`, `PdsPort`, `ClockPort`, `PeerStoragePort`).
const PORT_TRAIT_SUFFIX: &str = "Port";

/// Bootstrap stub allowlist (step 01-06, slice-03). The check-probes rule is
/// trait-generic and ALREADY covers `impl PeerStoragePort for <Adapter>` (per
/// component-boundaries.md §xtask I-FED-3). At this bootstrap step the
/// `DuckDbPeerStorageAdapter` probe is still the 01-02 `todo!()` SCAFFOLD; the
/// real Earned-Trust probe lands in Phase 03/04 (driven by PS-* / PP-*
/// scenarios). To keep CI green NOW without weakening the rule, a known-stub
/// probe site on this allowlist is reported as a WARNING (no exit-1) instead of
/// a hard violation.
///
/// SELF-HEALING: when the real probe lands the body classifies as `Accept` (not
/// a violation at all), so the allowlist entry simply goes unused — no manual
/// flip from warning→error is required. If a NON-allowlisted adapter regresses
/// to a stub, it is still a hard violation (exit 1). The list is intentionally
/// pinned by adapter name so it cannot mask a different adapter's regression.
const BOOTSTRAP_STUB_ALLOWLIST: &[&str] = &["DuckDbPeerStorageAdapter"];

/// Result of classifying one `probe()` body. The variants carry the
/// minimum context needed to render a useful error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// Body performs real work; passes the check.
    Accept,
    /// Body is `todo!()`, `unimplemented!()`, or `panic!(...)`.
    RejectStubMacro { adapter: String, macro_name: String },
    /// Body is a single `Ok(...)` / `ProbeOutcome::Ok` expression and the
    /// adapter is not on the degenerate allow-list.
    RejectTrivialOk { adapter: String },
    /// Body has no executable statements (comments only / empty).
    RejectEmpty { adapter: String },
}

impl Classification {
    /// True when this classification means "this is a violation".
    pub fn is_violation(&self) -> bool {
        !matches!(self, Classification::Accept)
    }

    /// Render a single violation as a human-readable string.
    pub fn render(&self, file: &Path) -> String {
        match self {
            Classification::Accept => String::new(),
            Classification::RejectStubMacro {
                adapter,
                macro_name,
            } => format!(
                "{}: {} probe() body is `{}!(...)` — a stub, not Earned Trust",
                file.display(),
                adapter,
                macro_name
            ),
            Classification::RejectTrivialOk { adapter } => format!(
                "{}: {} probe() body is a single `Ok(ProbeOutcome::Ok)` — no real \
                 readiness check. Add an inspection / I/O / sentinel arm, or add \
                 {} to DEGENERATE_ADAPTERS with an ADR-009 justification.",
                file.display(),
                adapter,
                adapter
            ),
            Classification::RejectEmpty { adapter } => format!(
                "{}: {} probe() body is empty (comments only) — implement at least \
                 one Earned-Trust arm before shipping the adapter.",
                file.display(),
                adapter
            ),
        }
    }
}

/// Pure classifier. Given the body of a `fn probe(&self) -> ProbeOutcome`
/// and the name of the implementor type, decide whether it's a stub.
///
/// The signature is `(adapter_name, &ImplItemFn) -> Classification` — pure,
/// re-runnable, fixture-testable.
pub fn classify_probe_body(adapter_name: &str, probe_fn: &ImplItemFn) -> Classification {
    // Degenerate-adapter allow list: short-circuit before any
    // stub-classification kicks in. This is the *only* place we permit
    // a trivial `Ok` body.
    if DEGENERATE_ADAPTERS.contains(&adapter_name) {
        return Classification::Accept;
    }

    let stmts = &probe_fn.block.stmts;

    // Empty body (comments-only): syn discards comments, so a body that
    // has zero statements after parsing is comment-only or genuinely empty.
    if stmts.is_empty() {
        return Classification::RejectEmpty {
            adapter: adapter_name.to_string(),
        };
    }

    // Single-statement body: inspect what that statement is.
    if stmts.len() == 1 {
        if let Some(class) = classify_single_stmt(adapter_name, &stmts[0]) {
            return class;
        }
    }

    // Multi-statement body: the only way it's still a violation is if
    // every statement is itself trivial (e.g. a stub `todo!` followed by
    // a placeholder return). Look for at least one non-trivial statement.
    let any_real_work = stmts.iter().any(stmt_does_real_work);
    if any_real_work {
        return Classification::Accept;
    }

    // Multi-statement but no real work: treat as trivial-ok if any
    // statement is a stub macro; else fall back to trivial-ok.
    for stmt in stmts {
        if let Some(macro_name) = stmt_stub_macro(stmt) {
            return Classification::RejectStubMacro {
                adapter: adapter_name.to_string(),
                macro_name,
            };
        }
    }
    Classification::RejectTrivialOk {
        adapter: adapter_name.to_string(),
    }
}

/// Classify a single-statement body. Returns `Some(class)` for any
/// recognised stub shape, `None` when the statement does real work
/// (the caller then returns `Classification::Accept`).
fn classify_single_stmt(adapter_name: &str, stmt: &Stmt) -> Option<Classification> {
    // Case A: the statement is a stub macro (`todo!()`, `unimplemented!()`,
    // `panic!(...)`).
    if let Some(macro_name) = stmt_stub_macro(stmt) {
        return Some(Classification::RejectStubMacro {
            adapter: adapter_name.to_string(),
            macro_name,
        });
    }

    // Case B: the statement is a trivial Ok-like expression.
    let expr = match stmt {
        Stmt::Expr(e, _) => e,
        _ => return None,
    };
    if expr_is_trivial_ok(expr) {
        return Some(Classification::RejectTrivialOk {
            adapter: adapter_name.to_string(),
        });
    }

    // Anything else in a single-statement body counts as real work
    // (e.g. a match expression, an if, a method call). Accept.
    None
}

/// Does a statement do real work? "Real work" = anything that is NOT a
/// stub macro and NOT a trivial Ok literal.
fn stmt_does_real_work(stmt: &Stmt) -> bool {
    if stmt_stub_macro(stmt).is_some() {
        return false;
    }
    match stmt {
        Stmt::Local(_) => true, // a `let` binding
        Stmt::Item(_) => true,
        Stmt::Macro(_) => true, // any non-stub macro (already filtered)
        Stmt::Expr(e, _) => !expr_is_trivial_ok(e),
    }
}

/// If `stmt` is a `todo!()`, `unimplemented!()`, or `panic!(...)` macro
/// invocation (as a statement or an expression), return the macro name.
fn stmt_stub_macro(stmt: &Stmt) -> Option<String> {
    match stmt {
        Stmt::Macro(m) => stub_macro_name(&m.mac),
        Stmt::Expr(Expr::Macro(em), _) => stub_macro_name(&em.mac),
        _ => None,
    }
}

/// Return `Some("todo")` / `Some("unimplemented")` / `Some("panic")` for
/// the recognised stub macros; `None` otherwise.
fn stub_macro_name(mac: &Macro) -> Option<String> {
    let last = mac.path.segments.last()?;
    let ident = last.ident.to_string();
    match ident.as_str() {
        "todo" | "unimplemented" | "panic" => Some(ident),
        _ => None,
    }
}

/// True when `expr` is one of the recognised trivial-success shapes:
///
/// - `ProbeOutcome::Ok` (path expression)
/// - `Ok(...)` where the inner is `ProbeOutcome::Ok` or `_` or a unit
/// - bare `()` or unit-only block
fn expr_is_trivial_ok(expr: &Expr) -> bool {
    match expr {
        // Bare path: e.g. `ProbeOutcome::Ok` or `ports::ProbeOutcome::Ok`.
        Expr::Path(p) => {
            let last = match p.path.segments.last() {
                Some(seg) => seg.ident.to_string(),
                None => return false,
            };
            last == "Ok"
        }
        // `Ok(...)` call — inspect the argument.
        Expr::Call(c) => {
            let callee_ident = call_callee_ident(&c.func);
            if callee_ident.as_deref() != Some("Ok") {
                return false;
            }
            // `Ok()` with zero args, or `Ok(<trivial>)` — both trivial.
            if c.args.is_empty() {
                return true;
            }
            if c.args.len() == 1 {
                return expr_is_trivial_ok(&c.args[0]);
            }
            false
        }
        // `()` unit expression.
        Expr::Tuple(t) if t.elems.is_empty() => true,
        // A block that contains a single trivial expression (e.g.
        // `{ ProbeOutcome::Ok }`).
        Expr::Block(b) if b.block.stmts.len() == 1 => {
            if let Stmt::Expr(inner, _) = &b.block.stmts[0] {
                expr_is_trivial_ok(inner)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Extract the final identifier of a call's callee expression. Used to
/// detect `Ok(...)` regardless of leading path segments.
fn call_callee_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// Effect shell — file enumeration + syn parsing
// -----------------------------------------------------------------------------

/// One probe site found in a source file. The `run` driver walks adapter
/// crates, collects every site, classifies them, and reports violations.
///
/// `adapter` is retained for `Debug` output / future structured reporting
/// even though `report()` currently composes its message from `file +
/// classification` (the classification variants already embed the adapter
/// name).
#[derive(Debug)]
pub struct ProbeSite {
    pub file: PathBuf,
    pub adapter: String,
    pub classification: Classification,
}

impl ProbeSite {
    /// True when this site is a violation AND its adapter is on the bootstrap
    /// stub allowlist — i.e. a known, tracked, in-flight scaffold that should
    /// warn rather than fail CI at this bootstrap step.
    fn is_allowlisted_stub(&self) -> bool {
        self.classification.is_violation()
            && BOOTSTRAP_STUB_ALLOWLIST.contains(&self.adapter.as_str())
    }
}

/// Top-level entry point invoked from `main.rs`. Returns the process
/// exit code: 0 = all probes pass; 1 = at least one violation; 2 = an
/// internal error walking the workspace.
pub fn run() -> Result<u8> {
    let workspace_root = locate_workspace_root()?;
    let sites = collect_probe_sites(&workspace_root)?;
    report(&sites)
}

/// Pure-ish: collect every `impl <Port> for <Adapter>` probe site under
/// `workspace_root/crates/adapter-*/src/**/*.rs`. Errors propagate
/// (I/O failure, syn parse failure) so the caller can surface them.
pub fn collect_probe_sites(workspace_root: &Path) -> Result<Vec<ProbeSite>> {
    let mut sites = Vec::new();
    let crates_dir = workspace_root.join("crates");
    if !crates_dir.is_dir() {
        return Err(anyhow!(
            "expected workspace `crates/` dir at {}",
            crates_dir.display()
        ));
    }

    let mut adapter_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in std::fs::read_dir(&crates_dir)
        .with_context(|| format!("read_dir {}", crates_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with("adapter-") {
            adapter_dirs.insert(path);
        }
    }

    for adapter_dir in adapter_dirs {
        let src_dir = adapter_dir.join("src");
        if !src_dir.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() && p.extension().is_some_and(|e| e == "rs") {
                sites.extend(scan_file(p)?);
            }
        }
    }

    Ok(sites)
}

/// Parse one Rust source file and collect every probe site (one per
/// `impl <Port> for <Adapter>` block).
fn scan_file(path: &Path) -> Result<Vec<ProbeSite>> {
    let src = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file = syn::parse_file(&src).with_context(|| format!("syn parse {}", path.display()))?;

    let mut visitor = ProbeVisitor {
        file: path.to_path_buf(),
        sites: Vec::new(),
    };
    visitor.visit_file(&file);
    Ok(visitor.sites)
}

/// AST walker — for every `impl Trait for Type` block where the trait
/// name ends in `Port`, finds the `probe` method and classifies its body.
struct ProbeVisitor {
    file: PathBuf,
    sites: Vec<ProbeSite>,
}

impl<'ast> Visit<'ast> for ProbeVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        if let Item::Impl(impl_block) = item {
            self.inspect_impl(impl_block);
        }
        syn::visit::visit_item(self, item);
    }
}

impl ProbeVisitor {
    fn inspect_impl(&mut self, impl_block: &ItemImpl) {
        let (_, trait_path, _) = match &impl_block.trait_ {
            Some(t) => t,
            None => return,
        };
        let trait_name = match trait_path.segments.last() {
            Some(seg) => seg.ident.to_string(),
            None => return,
        };
        if !trait_name.ends_with(PORT_TRAIT_SUFFIX) {
            return;
        }

        let adapter_name = match impl_type_name(&impl_block.self_ty) {
            Some(n) => n,
            None => return,
        };

        // Find the `probe` method. Adapters MAY define probe with
        // arbitrary signatures; we trust the port trait to constrain
        // the shape and only inspect the body.
        for item in &impl_block.items {
            if let ImplItem::Fn(f) = item {
                if f.sig.ident == "probe" {
                    let classification = classify_probe_body(&adapter_name, f);
                    self.sites.push(ProbeSite {
                        file: self.file.clone(),
                        adapter: adapter_name.clone(),
                        classification,
                    });
                }
            }
        }
    }
}

/// Extract the implementor type's last path segment. Returns `None` for
/// non-path types (e.g. references, tuples) — those don't pattern-match
/// our "<Port> for <Adapter>" shape.
fn impl_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// Find the workspace root by walking up from the current dir looking
/// for `Cargo.toml` that declares `[workspace]`. xtask is always invoked
/// from somewhere inside the workspace.
fn locate_workspace_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("current_dir")?;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let body = std::fs::read_to_string(&manifest)
                .with_context(|| format!("read {}", manifest.display()))?;
            if body.contains("[workspace]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            return Err(anyhow!(
                "no workspace Cargo.toml found from {}",
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<?>".to_string())
            ));
        }
    }
}

/// Render violations to stderr and return the exit code: 0 if clean,
/// 1 if any violations.
fn report(sites: &[ProbeSite]) -> Result<u8> {
    // Partition violations: hard (fail CI) vs allowlisted bootstrap stubs
    // (warn only). A non-violation site never lands in either bucket.
    let hard_violations: Vec<&ProbeSite> = sites
        .iter()
        .filter(|s| s.classification.is_violation() && !s.is_allowlisted_stub())
        .collect();
    let warnings: Vec<&ProbeSite> = sites.iter().filter(|s| s.is_allowlisted_stub()).collect();

    if sites.is_empty() {
        eprintln!(
            "xtask check-probes: no `impl <Port> for <Adapter>` blocks found under \
             crates/adapter-*/src/**. Did the directory layout change?"
        );
        return Ok(1);
    }

    // Bootstrap stub warnings are informational; they NEVER set the exit code.
    for w in &warnings {
        eprintln!(
            "xtask check-probes: WARNING (bootstrap allowlist, exit-code unaffected): {}",
            w.classification.render(&w.file)
        );
    }

    if hard_violations.is_empty() {
        eprintln!(
            "xtask check-probes: OK ({} probe site{} inspected{})",
            sites.len(),
            if sites.len() == 1 { "" } else { "s" },
            if warnings.is_empty() {
                String::new()
            } else {
                format!(", {} bootstrap-allowlisted stub warning(s)", warnings.len())
            }
        );
        return Ok(0);
    }

    eprintln!(
        "xtask check-probes: {} violation{} (of {} probe site{} inspected):",
        hard_violations.len(),
        if hard_violations.len() == 1 { "" } else { "s" },
        sites.len(),
        if sites.len() == 1 { "" } else { "s" },
    );
    for v in &hard_violations {
        eprintln!("  - {}", v.classification.render(&v.file));
    }
    Ok(1)
}

// -----------------------------------------------------------------------------
// Unit tests — pure classifier exercised against hand-rolled `syn::ImplItemFn`
// fixtures. No filesystem, no real adapter crates touched.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    /// Build a fake `impl ... { fn probe(&self) -> X { <body> } }` block,
    /// extract the `probe` `ImplItemFn`, and classify it under
    /// `adapter_name`. Keeps every unit test a one-liner.
    fn classify(adapter_name: &str, body: &str) -> Classification {
        let src =
            format!("impl FakePort for FakeAdapter {{ fn probe(&self) -> Outcome {{ {body} }} }}");
        let impl_block: syn::ItemImpl =
            parse_str(&src).expect("syn must parse the fixture impl block");
        let probe = impl_block
            .items
            .into_iter()
            .find_map(|i| {
                if let ImplItem::Fn(f) = i {
                    Some(f)
                } else {
                    None
                }
            })
            .expect("fixture has a probe fn");
        classify_probe_body(adapter_name, &probe)
    }

    // ---- Stub bodies REJECT ----------------------------------------------

    #[test]
    fn rejects_bare_ok_ok() {
        let c = classify("DuckDbStorageAdapter", "Ok(ProbeOutcome::Ok)");
        assert!(
            matches!(c, Classification::RejectTrivialOk { .. }),
            "Ok(ProbeOutcome::Ok) must reject; got {c:?}"
        );
    }

    #[test]
    fn rejects_bare_probeoutcome_ok() {
        let c = classify("DuckDbStorageAdapter", "ProbeOutcome::Ok");
        assert!(
            matches!(c, Classification::RejectTrivialOk { .. }),
            "ProbeOutcome::Ok must reject; got {c:?}"
        );
    }

    #[test]
    fn rejects_todo_macro() {
        let c = classify("DuckDbStorageAdapter", "todo!()");
        match c {
            Classification::RejectStubMacro { macro_name, .. } => {
                assert_eq!(macro_name, "todo");
            }
            other => panic!("expected RejectStubMacro(todo); got {other:?}"),
        }
    }

    #[test]
    fn rejects_unimplemented_macro() {
        let c = classify("DuckDbStorageAdapter", "unimplemented!()");
        match c {
            Classification::RejectStubMacro { macro_name, .. } => {
                assert_eq!(macro_name, "unimplemented");
            }
            other => panic!("expected RejectStubMacro(unimplemented); got {other:?}"),
        }
    }

    #[test]
    fn rejects_panic_macro() {
        let c = classify("DuckDbStorageAdapter", "panic!(\"nope\")");
        match c {
            Classification::RejectStubMacro { macro_name, .. } => {
                assert_eq!(macro_name, "panic");
            }
            other => panic!("expected RejectStubMacro(panic); got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_body() {
        // Syn drops comments, so a body that's literally `{}` (or only
        // comments) parses to zero statements.
        let c = classify("DuckDbStorageAdapter", "");
        assert!(
            matches!(c, Classification::RejectEmpty { .. }),
            "empty body must reject; got {c:?}"
        );
    }

    // ---- Real bodies ACCEPT ----------------------------------------------

    #[test]
    fn accepts_body_with_let_and_match() {
        // The simplest "real work" shape: one binding + a final match.
        let c = classify(
            "DuckDbStorageAdapter",
            "let c = self.conn.lock(); match c { Ok(_) => ProbeOutcome::Ok, Err(_) => ProbeOutcome::Refused }",
        );
        assert_eq!(c, Classification::Accept, "let + match must accept");
    }

    #[test]
    fn accepts_body_with_method_call() {
        // Single-expression but non-trivial (a method call on self).
        let c = classify("AtProtoDidAdapter", "self.walk_arms()");
        assert_eq!(
            c,
            Classification::Accept,
            "single method-call expression must accept"
        );
    }

    #[test]
    fn accepts_body_with_if() {
        // The pds adapter's pre-flight: `if self.endpoint.is_empty() { return ... } ProbeOutcome::Ok`.
        let c = classify(
            "AtProtoPdsAdapter",
            "if self.endpoint.is_empty() { return ProbeOutcome::Refused; } ProbeOutcome::Ok",
        );
        assert_eq!(c, Classification::Accept, "if-guard + tail must accept");
    }

    // ---- Degenerate-adapter exception -----------------------------------

    #[test]
    fn accepts_system_clock_with_trivial_ok_per_adr_009() {
        // The documented exception: SystemClockAdapter has no failure
        // modes; ADR-009 §degenerate-adapter explicitly permits `Ok`.
        let c = classify("SystemClockAdapter", "ProbeOutcome::Ok");
        assert_eq!(
            c,
            Classification::Accept,
            "SystemClockAdapter must be exempt per ADR-009 §degenerate-adapter"
        );
    }

    #[test]
    fn other_adapters_do_not_get_the_exception() {
        // Sanity: the exception is name-pinned to SystemClockAdapter
        // and does NOT generalize to "any adapter that happens to be
        // simple". A new adapter must earn its own exception entry.
        let c = classify("FutureBoringAdapter", "ProbeOutcome::Ok");
        assert!(
            matches!(c, Classification::RejectTrivialOk { .. }),
            "only SystemClockAdapter gets the exemption; got {c:?}"
        );
    }
}
