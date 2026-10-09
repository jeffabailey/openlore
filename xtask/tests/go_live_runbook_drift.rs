//! Go-live runbook drift guard (fix-go-live-runbook-gaps, step 01-03).
//!
//! The operator runbooks drifted from the code and from each other: they named an admin route
//! the review app never served (`/admin/test-alarm`), `deploy.sh` modes the scripts never
//! accepted, and a replacement plan without `-replace=` (an in-place stop/start that never
//! re-runs user-data). These tests read the runbooks AND the code as text and compare the names.
//! The code side is never linked or executed: the admin routes are the string literals in the
//! review app's route table, the modes are the `case` arms of each `deploy.sh`, and the
//! `OPENLORE_*` settings are the names written in non-Markdown sources (a doc cannot vouch for
//! itself).
//!
//! The scanner is a set of pure text functions; `the_drift_scanner_flags_literal_drift_fixtures`
//! proves each check can fail by injecting literal drift into generated Markdown.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use proptest::prelude::*;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------------------------
// The scanner (pure)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    AdminRoute,
    DeployMode,
    Setting,
    PlainReplacementPlan,
}

/// One name a runbook uses that the code does not have, at `file:line`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    kind: Kind,
    file: String,
    line: usize,
    token: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {:?} `{}`",
            self.file, self.line, self.kind, self.token
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum App {
    Indexer,
    ReviewApp,
}

/// What the code actually has.
#[derive(Debug, Clone, Default)]
struct Truth {
    admin_routes: BTreeSet<String>,
    indexer_modes: BTreeSet<String>,
    review_app_modes: BTreeSet<String>,
    settings: BTreeSet<String>,
}

impl Truth {
    fn knows_mode(&self, app: Option<App>, mode: &str) -> bool {
        match app {
            Some(App::Indexer) => self.indexer_modes.contains(mode),
            Some(App::ReviewApp) => self.review_app_modes.contains(mode),
            None => self.indexer_modes.contains(mode) || self.review_app_modes.contains(mode),
        }
    }

    /// `OPENLORE_PDS_` (as in "the `OPENLORE_PDS_*` lines") is a prefix: some setting must
    /// start with it.
    fn knows_setting(&self, name: &str) -> bool {
        self.settings.contains(name)
            || (name.ends_with('_') && self.settings.iter().any(|known| known.starts_with(name)))
    }
}

/// A Markdown line with its 1-based number and whether it sits inside a fenced block.
struct MdLine<'a> {
    number: usize,
    text: &'a str,
    in_fence: bool,
    fence_marker: bool,
}

fn md_lines(doc: &str) -> Vec<MdLine<'_>> {
    let mut in_fence = false;
    doc.lines()
        .enumerate()
        .map(|(index, text)| {
            let fence_marker = text.trim_start().starts_with("```");
            if fence_marker {
                in_fence = !in_fence;
            }
            MdLine {
                number: index + 1,
                text,
                in_fence: in_fence && !fence_marker,
                fence_marker,
            }
        })
        .collect()
}

/// `prefix` followed by a non-empty run of `accepted` characters, each returned whole.
fn tokens_with_prefix(text: &str, prefix: &str, accepted: impl Fn(char) -> bool) -> Vec<String> {
    text.match_indices(prefix)
        .filter_map(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            let run: String = rest.chars().take_while(|c| accepted(*c)).collect();
            (!run.is_empty()).then(|| format!("{prefix}{run}"))
        })
        .collect()
}

fn admin_routes_in(text: &str) -> Vec<String> {
    tokens_with_prefix(text, "/admin/", |c| c.is_ascii_lowercase() || c == '-')
}

fn settings_in(text: &str) -> Vec<String> {
    tokens_with_prefix(text, "OPENLORE_", |c| {
        c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
    })
}

/// The code a line carries: the whole line inside a fence, else its inline code spans.
fn code_segments<'a>(line: &MdLine<'a>) -> Vec<&'a str> {
    if line.fence_marker {
        Vec::new()
    } else if line.in_fence {
        vec![line.text]
    } else {
        line.text.split('`').skip(1).step_by(2).collect()
    }
}

fn leading(text: &str, accepted: impl Fn(char) -> bool) -> usize {
    text.chars()
        .take_while(|c| accepted(*c))
        .map(char::len_utf8)
        .sum()
}

fn mode_word(text: &str) -> &str {
    match text.chars().next() {
        Some(first) if first.is_ascii_lowercase() => {
            &text[..leading(text, |c| c.is_ascii_lowercase() || c == '-')]
        }
        _ => "",
    }
}

/// Every `deploy.sh <mode>` in a code segment (with `a | b | c` lists expanded), and the app
/// its path names (`indexer/deploy.sh`, `review-app/deploy.sh`), if any.
fn deploy_modes_in(segment: &str) -> Vec<(Option<App>, String)> {
    let blank = |c: char| c == ' ' || c == '\t';
    let mut found = Vec::new();
    for (at, script) in segment.match_indices("deploy.sh") {
        let before = &segment[..at];
        let app = if before.ends_with("indexer/") {
            Some(App::Indexer)
        } else if before.ends_with("review-app/") {
            Some(App::ReviewApp)
        } else {
            None
        };
        let mut rest = &segment[at + script.len()..];
        let gap = leading(rest, blank);
        if gap == 0 {
            continue;
        }
        rest = &rest[gap..];
        let mut word = mode_word(rest);
        while !word.is_empty() {
            found.push((app, word.to_string()));
            rest = &rest[word.len()..];
            let before_bar = leading(rest, blank);
            if !rest[before_bar..].starts_with('|') {
                break;
            }
            rest = &rest[before_bar + 1..];
            rest = &rest[leading(rest, blank)..];
            word = mode_word(rest);
        }
    }
    found
}

/// The app a runbook belongs to when a `deploy.sh` mention does not name its path.
fn doc_app(path: &str) -> Option<App> {
    if path.starts_with("deploy/indexer/") || path.starts_with("docs/feature/indexer-deployment/") {
        Some(App::Indexer)
    } else if path.starts_with("deploy/review-app/")
        || path.starts_with("docs/feature/bluesky-claim-review-app/")
    {
        Some(App::ReviewApp)
    } else {
        None
    }
}

/// Whether a line opens a Markdown list item (`- `, `* `, `4. `), which starts a new block.
fn opens_list_item(text: &str) -> bool {
    let trimmed = text.trim_start();
    let digits = leading(trimmed, |c| c.is_ascii_digit());
    trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || (digits > 0 && trimmed[digits..].starts_with(". "))
}

/// A replacement plan is a `tofu plan ... -out=` line whose block (up to a blank line, a fence
/// or the next list item) also carries `OPENLORE_ALLOW_DELETE=1`, or which sits under an `R-REPLACE` heading.
/// It must say `-replace=`, or the apply is an in-place stop/start and user-data never re-runs.
fn plain_replacement_plans(doc: &str) -> Vec<(usize, String)> {
    let lines = md_lines(doc);
    let mut block = 0usize;
    let mut heading = String::new();
    let mut block_of = Vec::with_capacity(lines.len());
    let mut heading_of = Vec::with_capacity(lines.len());
    for line in &lines {
        if line.fence_marker
            || line.text.trim().is_empty()
            || (!line.in_fence && opens_list_item(line.text))
        {
            block += 1;
        }
        if !line.in_fence && !line.fence_marker && line.text.starts_with('#') {
            heading = line.text.to_string();
        }
        block_of.push(block);
        heading_of.push(heading.clone());
    }
    let block_text = |id: usize| -> String {
        lines
            .iter()
            .zip(&block_of)
            .filter(|(line, b)| **b == id && !line.fence_marker && !line.text.trim().is_empty())
            .map(|(line, _)| line.text)
            .collect::<Vec<_>>()
            .join("\n")
    };
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.text.contains("tofu plan")
                && line.text.contains("-out=")
                && !line.text.contains("-replace=")
        })
        .filter(|(index, _)| {
            heading_of[*index].contains("R-REPLACE")
                || block_text(block_of[*index]).contains("OPENLORE_ALLOW_DELETE=1")
        })
        .map(|(_, line)| (line.number, line.text.trim().to_string()))
        .collect()
}

/// Every name in one runbook that the code does not have.
fn scan_doc(path: &str, doc: &str, truth: &Truth) -> Vec<Finding> {
    let finding = |kind, line, token: String| Finding {
        kind,
        file: path.to_string(),
        line,
        token,
    };
    let mut findings = Vec::new();
    for line in md_lines(doc) {
        for route in admin_routes_in(line.text) {
            if !truth.admin_routes.contains(&route) {
                findings.push(finding(Kind::AdminRoute, line.number, route));
            }
        }
        for setting in settings_in(line.text) {
            if !truth.knows_setting(&setting) {
                findings.push(finding(Kind::Setting, line.number, setting));
            }
        }
        for segment in code_segments(&line) {
            for (named_app, mode) in deploy_modes_in(segment) {
                if !truth.knows_mode(named_app.or(doc_app(path)), &mode) {
                    findings.push(finding(Kind::DeployMode, line.number, mode));
                }
            }
        }
    }
    for (line, text) in plain_replacement_plans(doc) {
        findings.push(finding(Kind::PlainReplacementPlan, line, text));
    }
    findings
}

// --- code-side truth, read as text --------------------------------------------------------------

/// The `"/admin/..."` string literals of the review app's route table.
fn admin_routes_of(admin_rs: &str) -> BTreeSet<String> {
    admin_rs
        .lines()
        .filter(|line| line.contains("=>"))
        .flat_map(|line| {
            tokens_with_prefix(line, "\"/admin/", |c| c.is_ascii_lowercase() || c == '-')
        })
        .map(|quoted| quoted.trim_start_matches('"').to_string())
        .collect()
}

/// The arms of the top-level `case "$cmd" in` and of `host_main`'s `case "$mode" in`.
fn script_modes(script: &str) -> BTreeSet<String> {
    let mut modes = BTreeSet::new();
    let mut inside = false;
    for line in script.lines().map(str::trim) {
        if line == "case \"$cmd\" in" || line == "case \"$mode\" in" {
            inside = true;
        } else if line == "esac" {
            inside = false;
        } else if inside {
            if let Some((pattern, _)) = line.split_once(')') {
                let is_arm = !pattern.is_empty()
                    && pattern
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '-' || c == '|' || c == ' ');
                if is_arm {
                    modes.extend(
                        pattern
                            .split('|')
                            .map(str::trim)
                            .filter(|mode| !mode.is_empty())
                            .map(str::to_string),
                    );
                }
            }
        }
    }
    modes
}

// ---------------------------------------------------------------------------------------------
// The real runbooks
// ---------------------------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("cannot read {relative}: {e}"))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .expect("under the repo root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn files_under(dir: &str, keep: impl Fn(&str) -> bool) -> Vec<String> {
    let mut files: Vec<String> = WalkDir::new(repo_root().join(dir))
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".terraform" && entry.file_name() != "target")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| relative(entry.path()))
        .filter(|path| keep(path))
        .collect();
    files.sort();
    files
}

/// The scanner's universe: every runbook and devops design doc for the two apps.
fn runbooks() -> Vec<String> {
    let mut docs = files_under("deploy", |p| p.ends_with(".md"));
    for dir in [
        "docs/feature/bluesky-claim-review-app/devops",
        "docs/feature/indexer-deployment/devops",
    ] {
        docs.extend(files_under(dir, |p| p.ends_with(".md")));
    }
    docs.push("docs/evolution/indexer-deployment-evolution.md".to_string());
    docs
}

fn code_truth() -> Truth {
    let sources = [
        files_under("crates", |p| p.contains("/src/") && p.ends_with(".rs")),
        files_under("deploy", |p| {
            p.ends_with(".sh")
                || p.ends_with("/compose.yaml")
                || (p.starts_with("deploy/tofu/") && p.ends_with(".tf"))
        }),
    ]
    .concat();
    Truth {
        admin_routes: admin_routes_of(&read("crates/openlore-review-app/src/admin.rs")),
        indexer_modes: script_modes(&read("deploy/indexer/deploy.sh")),
        review_app_modes: script_modes(&read("deploy/review-app/deploy.sh")),
        settings: sources.iter().flat_map(|p| settings_in(&read(p))).collect(),
    }
}

fn real_findings(kind: Kind) -> Vec<Finding> {
    let truth = code_truth();
    runbooks()
        .iter()
        .flat_map(|path| scan_doc(path, &read(path), &truth))
        .filter(|finding| finding.kind == kind)
        .collect()
}

fn assert_no_drift(kind: Kind, what: &str) {
    let findings = real_findings(kind);
    assert!(
        findings.is_empty(),
        "{what}:\n{}",
        findings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_code_side_truth_is_not_empty() {
    let truth = code_truth();
    assert_eq!(
        truth.admin_routes,
        BTreeSet::from(["/admin/kpi".to_string(), "/admin/purge".to_string()])
    );
    for mode in [
        "deploy", "install", "redeploy", "stop", "rollback", "status", "host",
    ] {
        assert!(truth.review_app_modes.contains(mode), "review app: {mode}");
    }
    for mode in ["start", "trigger", "host-status"] {
        assert!(truth.indexer_modes.contains(mode), "indexer: {mode}");
        assert!(
            !truth.review_app_modes.contains(mode),
            "review app has no {mode}"
        );
    }
    assert!(truth.settings.contains("OPENLORE_REVIEW_SCAN_CONCURRENCY"));
    assert!(truth.settings.contains("OPENLORE_ALLOW_DELETE"));
}

#[test]
fn runbooks_name_only_admin_routes_the_review_app_serves() {
    assert_no_drift(
        Kind::AdminRoute,
        "runbooks name admin routes that crates/openlore-review-app/src/admin.rs does not serve",
    );
}

#[test]
fn every_replacement_plan_in_a_runbook_carries_replace() {
    assert_no_drift(
        Kind::PlainReplacementPlan,
        "replacement plans without -replace=module.pds.aws_instance.pds (an in-place stop/start)",
    );
}

#[test]
fn runbooks_name_only_deploy_sh_modes_the_scripts_accept() {
    assert_no_drift(
        Kind::DeployMode,
        "runbooks name deploy.sh modes the script does not accept",
    );
}

#[test]
fn runbooks_name_only_openlore_settings_the_code_reads() {
    assert_no_drift(
        Kind::Setting,
        "runbooks name OPENLORE_* settings that no source, script, compose file or tofu file has",
    );
}

// ---------------------------------------------------------------------------------------------
// The scanner can fail: literal drift injected into generated Markdown is caught, exactly.
// ---------------------------------------------------------------------------------------------

fn fixture_truth() -> Truth {
    let set = |names: &[&str]| names.iter().map(|n| (*n).to_string()).collect();
    Truth {
        admin_routes: set(&["/admin/kpi", "/admin/purge"]),
        indexer_modes: set(&[
            "deploy",
            "install",
            "redeploy",
            "stop",
            "start",
            "trigger",
            "host-status",
            "rollback",
            "status",
            "host",
        ]),
        review_app_modes: set(&[
            "deploy", "install", "redeploy", "stop", "status", "rollback", "host",
        ]),
        settings: set(&[
            "OPENLORE_ALLOW_DELETE",
            "OPENLORE_REVIEW_SCAN_CONCURRENCY",
            "OPENLORE_PDS_ENDPOINT",
        ]),
    }
}

/// Clean paragraphs, including the negatives the real docs rely on.
const CLEAN_PARAGRAPHS: &[&str] = &[
    "Read the sums with `GET /admin/kpi?from=2026-10-01&to=2026-10-07`.",
    "Forget a person with `POST /admin/purge`.",
    "`deploy/review-app/deploy.sh redeploy | stop | install`",
    "`deploy/indexer/deploy.sh trigger` starts one pass.",
    "`deploy.sh` lacks a kpi mode; the runbook does it by hand.",
    "Set `OPENLORE_REVIEW_SCAN_CONCURRENCY=1` and redeploy.",
    "the `OPENLORE_PDS_*` lines above",
    "tofu plan -out=tfplan\n../../../check-plan.sh tfplan",
    "AWS_PROFILE=jeff tofu plan -replace=module.pds.aws_instance.pds -out=tfplan\nOPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan",
    "Run `tofu plan -out=tfplan` and `check-plan.sh tfplan` **without** `OPENLORE_ALLOW_DELETE`.",
    "`tofu plan` reports \"No changes.\"",
];

#[derive(Debug, Clone)]
enum Drift {
    Route(String),
    IndexerMode(String),
    ReviewAppMode(String),
    Setting(String),
    PlainPlan,
}

impl Drift {
    fn paragraph(&self) -> String {
        match self {
            Drift::Route(name) => format!("Fire it with `POST /admin/{name}`."),
            Drift::IndexerMode(mode) => format!("Run `deploy/indexer/deploy.sh {mode}` now."),
            Drift::ReviewAppMode(mode) => {
                format!("```sh\ndeploy/review-app/deploy.sh status | {mode}\n```")
            }
            Drift::Setting(name) => format!("Set `OPENLORE_{name}=1` first."),
            Drift::PlainPlan => {
                "tofu plan -out=tfplan\nOPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan"
                    .to_string()
            }
        }
    }

    /// The finding's kind, the line offset inside the paragraph, and the literal token.
    fn expected(&self) -> (Kind, usize, String) {
        match self {
            Drift::Route(name) => (Kind::AdminRoute, 0, format!("/admin/{name}")),
            Drift::IndexerMode(mode) => (Kind::DeployMode, 0, mode.clone()),
            Drift::ReviewAppMode(mode) => (Kind::DeployMode, 1, mode.clone()),
            Drift::Setting(name) => (Kind::Setting, 0, format!("OPENLORE_{name}")),
            Drift::PlainPlan => (
                Kind::PlainReplacementPlan,
                0,
                "tofu plan -out=tfplan".to_string(),
            ),
        }
    }
}

fn drift() -> impl Strategy<Value = Drift> {
    prop_oneof![
        "zz[a-z]{2,10}".prop_map(Drift::Route),
        Just(Drift::Route("test-alarm".to_string())),
        // `kpi` and `measure` never existed; `status` is a review-app mode, not an indexer one…
        prop_oneof![
            Just("kpi".to_string()),
            Just("measure".to_string()),
            "zz[a-z]{2,8}"
        ]
        .prop_map(Drift::IndexerMode),
        // …and `start`/`trigger` are indexer modes, not review-app ones.
        prop_oneof![
            Just("start".to_string()),
            Just("trigger".to_string()),
            "zz[a-z]{2,8}"
        ]
        .prop_map(Drift::ReviewAppMode),
        "ZZ[A-Z0-9]{2,10}".prop_map(Drift::Setting),
        Just(Drift::PlainPlan),
    ]
}

proptest! {
    #[test]
    fn the_drift_scanner_flags_literal_drift_fixtures(
        clean in proptest::collection::vec(proptest::sample::select(CLEAN_PARAGRAPHS), 0..12),
        at in any::<proptest::sample::Index>(),
        injected in drift(),
    ) {
        let position = at.index(clean.len() + 1);
        let mut paragraphs: Vec<String> = clean.iter().map(|p| (*p).to_string()).collect();
        paragraphs.insert(position, injected.paragraph());
        let first_line = 1 + paragraphs[..position]
            .iter()
            .map(|p| p.lines().count() + 1)
            .sum::<usize>();
        let doc = paragraphs.join("\n\n");

        let (kind, offset, token) = injected.expected();
        let expected = vec![Finding {
            kind,
            file: "deploy/README.md".to_string(),
            line: first_line + offset,
            token,
        }];
        prop_assert_eq!(scan_doc("deploy/README.md", &doc, &fixture_truth()), expected, "doc:\n{}", doc);
    }
}

#[test]
fn the_mode_truth_is_read_from_the_case_arms() {
    let script = "host_main() {\n  case \"$mode\" in\n    install) a ;;\n    host-status) b ;;\n    *) die ;;\n  esac\n}\ncase \"$cmd\" in\n  deploy) x ;;\n  install | redeploy | stop) remote \"$cmd\" ;;\n  rollback)\n    case \"${1:-}\" in \"\" | --restore-db) ;; *) die ;; esac\n    ;;\n  *)\n    exit 2\n    ;;\nesac\n";
    let expected: BTreeSet<String> = [
        "install",
        "host-status",
        "deploy",
        "redeploy",
        "stop",
        "rollback",
    ]
    .iter()
    .map(|m| (*m).to_string())
    .collect();
    assert_eq!(script_modes(script), expected);
}

// ---------------------------------------------------------------------------------------------
// One ordered go-live checklist owns the sequence
// ---------------------------------------------------------------------------------------------

const CHECKLIST_HEADING: &str = "## Go-live checklist";
const CHECKLIST_LINK: &str = "README.md#go-live-checklist";

/// The text from `heading` to the next heading of the same or a higher level.
fn section<'a>(doc: &'a str, heading: &str) -> &'a str {
    let start = doc
        .find(&format!("\n{heading}"))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("no `{heading}` section"));
    let level = heading.chars().take_while(|c| *c == '#').count();
    let body = &doc[start + heading.len()..];
    let mut in_fence = false;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if !in_fence
            && offset > 0
            && hashes > 0
            && hashes <= level
            && line[hashes..].starts_with(' ')
        {
            break;
        }
        offset += line.len();
    }
    &doc[start..start + heading.len() + offset]
}

fn position(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("the checklist does not contain `{needle}`"))
}

fn assert_before(text: &str, first: &str, then: &str) {
    assert!(
        position(text, first) < position(text, then),
        "`{first}` must come before `{then}` in the checklist"
    );
}

#[test]
fn one_ordered_go_live_checklist_owns_the_sequence() {
    let readme = read("deploy/README.md");
    assert_eq!(
        readme.matches(&format!("\n{CHECKLIST_HEADING}\n")).count(),
        1,
        "deploy/README.md has exactly one `{CHECKLIST_HEADING}` section"
    );
    let checklist = section(&readme, CHECKLIST_HEADING);

    // Step 0: the module release is checked before any ref bump (B1 stays operator work).
    assert_before(checklist, "v1.7.0", "?ref=v1.7.0");
    assert_before(
        checklist,
        "git ls-remote --tags https://github.com/jeffabailey/tofu-aws-pds v1.7.0",
        "?ref=v1.7.0",
    );

    // B2 + S8: the replacement carries -replace=; its rollback stops BOTH apps before v1.6.0.
    assert!(checklist.contains("tofu plan -replace=module.pds.aws_instance.pds -out=tfplan"));
    assert!(checklist.contains("OPENLORE_ALLOW_DELETE=1 ../../../check-plan.sh tfplan"));
    assert_before(checklist, "deploy/review-app/deploy.sh stop", "?ref=v1.6.0");
    assert_before(checklist, "deploy/indexer/deploy.sh stop", "?ref=v1.6.0");

    // S3: alarms are ENABLED, then test-fired, for both apps; the applies are not duplicated.
    assert_before(
        checklist,
        "review_app_alarms_enabled = true",
        "--log-group-name /openlore/prod/review-app --log-stream-name test-fire",
    );
    assert_before(
        checklist,
        "indexer_alarms_enabled = true",
        "--log-group-name /openlore/prod/indexer --log-stream-name test-fire",
    );
    assert_eq!(
        checklist.matches("tofu apply bootstrap.tfplan").count(),
        1,
        "the bootstrap root is applied once"
    );

    // S4 + N1: exact test-fire commands with a millisecond timestamp; the A3 warning.
    for literal in [
        "aws logs create-log-stream --log-group-name /openlore/prod/review-app --log-stream-name test-fire",
        "aws logs create-log-stream --log-group-name /openlore/prod/indexer --log-stream-name test-fire",
        "aws logs put-log-events --log-group-name /openlore/prod/review-app --log-stream-name test-fire",
        "aws logs put-log-events --log-group-name /openlore/prod/indexer --log-stream-name test-fire",
        "$(date +%s000)",
        "{\"event\":\"guardrail.breach\",\"kpi\":\"TEST\"}",
        "{\"event\":\"github.token.expiring\",\"days_left\":-1}",
        "{\"event\":\"indexer.ingest.pass_summary\",\"exit_code\":3,\"pass_id\":\"TEST-a1-1\"}",
        "more than 2 h",
        "fires A3",
    ] {
        assert!(checklist.contains(literal), "the checklist lacks `{literal}`");
    }
    assert!(!checklist.contains("/admin/test-alarm"));

    // N2 + 01-02: prerequisites, DNS before install, the live flag, the install refusals.
    for literal in [
        "ssm:SendCommand",
        "ssm:StartSession",
        "logs:StartQuery",
        "logs:PutLogEvents",
        "gh variable set REVIEW_APP_LIVE --body true",
        "dig +short index.openlore.jeffbailey.us",
        "dig +short app.openlore.jeffbailey.us",
        "INDEXER_READY_WAIT_S=",
        "refuse_unless_isolated",
        "import /etc/caddy/sites/*.caddy",
        "docker pull ghcr.io/jeffabailey/openlore-review-app@sha256:",
        "docker pull ghcr.io/jeffabailey/openlore-indexer@sha256:",
        "Operator decisions",
    ] {
        assert!(
            checklist.contains(literal),
            "the checklist lacks `{literal}`"
        );
    }
    assert_before(
        checklist,
        "dig +short app.openlore.jeffbailey.us",
        "deploy/review-app/deploy.sh install",
    );

    // S9: the compose cap, not 256 MB.
    assert!(!readme.contains("256 MB"), "compose sets mem_limit 192m");
    assert!(readme.contains("192 MB"));
    assert!(!readme.contains("Operator follow-ups before the first deploy"));

    // The other runbooks point to the checklist instead of carrying their own sequence.
    for doc in [
        "deploy/indexer/README.md",
        "deploy/review-app/README.md",
        "docs/evolution/indexer-deployment-evolution.md",
        "docs/feature/bluesky-claim-review-app/devops/infrastructure-integration.md",
        "docs/feature/bluesky-claim-review-app/devops/monitoring-alerting.md",
        "docs/feature/bluesky-claim-review-app/devops/wave-decisions.md",
        "docs/feature/indexer-deployment/devops/infrastructure-integration.md",
    ] {
        assert!(
            read(doc).contains(CHECKLIST_LINK),
            "{doc} does not link the go-live checklist"
        );
    }
    let indexer_runbook = read("deploy/indexer/README.md");
    assert!(
        !indexer_runbook.contains("set `indexer_alarms_enabled = true`"),
        "deploy/indexer/README.md carries its own enable step"
    );
    assert!(
        !read("deploy/review-app/README.md").contains("review_app_alarms_enabled = true"),
        "deploy/review-app/README.md carries its own enable step"
    );
}

// ---------------------------------------------------------------------------------------------
// The memory gate can be run, and can fail
// ---------------------------------------------------------------------------------------------

#[test]
fn the_memory_gate_is_runnable_and_falsifiable() {
    for runbook in ["deploy/README.md", "deploy/indexer/README.md"] {
        let text = read(runbook);
        assert!(
            !text.contains("xargs -P 10"),
            "{runbook}: 10 parallel requests exceed the per-client burst"
        );
        assert!(
            !text.contains("scope/memory.peak\"\n"),
            "{runbook}: echoes the memory.peak path"
        );
        if text.contains("@search.json") {
            assert!(
                text.contains("> search.json <<"),
                "{runbook}: search.json is never defined"
            );
        }
    }
    let readme = read("deploy/README.md");
    let gate = section(&readme, "### Memory gate");
    for literal in [
        // a 5 s / 20 min sampling loop that prints VALUES
        "for i in $(seq 240); do",
        "sleep 5",
        "cat \"$cg/memory.peak\"",
        "cat \"$cg/memory.current\"",
        "grep MemAvailable /proc/meminfo",
        "grep '^pswpin ' /proc/vmstat",
        // literal PASS thresholds (platform-architecture §6)
        "indexer `memory.peak` ≤ 100 MB",
        "review app `memory.peak` ≤ 128 MB",
        "`MemAvailable` > 128 MB",
        // a defined search body
        "cat > search.json <<'EOF'",
        "{\"dimension\":\"subject\",\"value\":\"rust\"}",
        // a rate under 10/s burst 50, and 429 accounted for
        "xargs -P 4",
        "429",
        // the fail path uses the setting from 01-01
        "OPENLORE_REVIEW_SCAN_CONCURRENCY",
    ] {
        assert!(gate.contains(literal), "the memory gate lacks `{literal}`");
    }
}
