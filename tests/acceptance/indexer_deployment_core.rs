//! indexer-deployment — layer-2 PURE-CORE properties (ADR-007 functional core;
//! Mandate 9: PBT full at layers 1-2).
//!
//! The decisions this feature adds are total pure functions (component-
//! boundaries.md §1 "Pure core", data-models.md §2-§7, architecture-design.md §8
//! "Contract shapes"). Their contracts are stated here over generated inputs and
//! checked against ORACLES written from the ADR / data-model text — never from an
//! implementation:
//!
//! | ID | Contract | Source |
//! |---|---|---|
//! | CORE-1..3 | `plan_purge`: set difference; never intersects the list; ⊆ indexed; empty list → suppressed | ADR-082, data-models §3 |
//! | CORE-4 | DID-list read is total: Loaded(valid, distinct, first-seen) or Refused naming the first bad entry | ADR-081, data-models §2 |
//! | CORE-5 | pass exit code: 2 ≻ 3 ≻ 0; a cause iff exit 2 | ADR-078 am., data-models §4 |
//! | CORE-6 | single-flight runner: never two passes; busy names the running pass; any ending frees the slot | ADR-080 §3, §7 |
//! | CORE-7 | pass deadline: past the deadline ⇒ exit 2 `pass_deadline_exceeded`, whatever the pass would have said | ADR-080, B13 |
//! | CORE-8 | the new settings accept exactly their ranges | data-models §1 |
//! | CORE-9 | public routes: exactly POST searchClaims + GET /healthz; bounds 8 KiB / 512 B | ADR-083 |
//! | CORE-10 | health projection: 503 `store_unusable` or 200 with exactly status + time | ADR-083 §2, data-models §6 |
//! | CORE-11 | `purge_author` universe: only the target's rows, children and artifact files change (state delta); idempotent | ADR-082, data-models §3 |
//!
//! ## Binding seam (RED scaffold, Mandate 7)
//!
//! None of these functions exist yet, so every property calls a `sut_*`
//! binding whose body is `todo!()` — a panic, classified RED (not BROKEN).
//! DELIVER replaces each binding body with ONE call into the production
//! function (the view types here are the observable contract; DELIVER maps its
//! ADTs onto them). Bindings needing a function private to the
//! `openlore-indexer` binary crate (config parsing, routing) may instead move
//! the property verbatim into that crate's unit tests. CORE-11 binds to the
//! REAL `adapter-index-store` over a temp DuckDB (in-process; the same shape as
//! the existing `atomic_upsert_properties`).
//!
//! Closed-world finite tables (2 routes, 3 exit codes) are exhaustive examples
//! beside the generators (falsifier gate).
//
// SCAFFOLD: true

#[path = "../common/state_delta.rs"]
mod state_delta;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use proptest::prelude::*;
use state_delta::{assert_state_delta, set_to, Delta};

// =============================================================================
// Observable view types (the contract)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum PurgePlanView {
    Purge(BTreeSet<String>),
    Suppressed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListReadView {
    Loaded(Vec<String>),
    /// Names the first bad entry.
    Malformed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DidOutcome {
    Read,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PassOutcomeView {
    list_refused: bool,
    purge_failed: bool,
    upsert_failed: bool,
    panicked: bool,
    deadline_exceeded: bool,
    dids: Vec<DidOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerView {
    Idle,
    Running(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndKind {
    Completed,
    Panicked,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerEvent {
    RunPassRequested,
    PassEnded(EndKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerReply {
    Started(u64),
    Busy { running_pass_id: u64 },
    Freed,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Search,
    Health,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    Admitted,
    TooLarge,
    BadRequest,
}

/// One store as the purge universe sees it (data-models §3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct StoreView {
    /// (author_did, cid, signed_record_path)
    claims: BTreeSet<(String, String, String)>,
    /// (cid, evidence url)
    evidence: BTreeSet<(String, String)>,
    /// (referencing_cid, referenced_cid)
    references: BTreeSet<(String, String)>,
    /// artifact file paths present on disk
    artifacts: BTreeSet<String>,
}

// =============================================================================
// RED binding seam — DELIVER binds each to ONE production call
// =============================================================================

fn sut_plan_purge(_listed: &[String], _indexed_authors: &BTreeSet<String>) -> PurgePlanView {
    todo!("DELIVER 05-01: bind to appview_domain::plan_purge (ADR-082)")
}

fn sut_read_did_list(_text: &str) -> ListReadView {
    todo!("DELIVER 05-02: bind to the per-pass DID-file parse (reuses parse_repo_dids, ADR-081)")
}

fn sut_pass_exit(_outcome: &PassOutcomeView) -> (i32, Option<String>) {
    todo!("DELIVER 05-03: bind to the extended appview_domain pass_exit_code (+ cause)")
}

fn sut_single_flight(
    _state: RunnerView,
    _event: RunnerEvent,
    _next_id: u64,
) -> (RunnerView, RunnerReply) {
    todo!("DELIVER 05-04: bind to the pass runner's pure single-flight transition (ADR-080 §3)")
}

fn sut_deadline_outcome(
    _elapsed_ms: u64,
    _deadline_secs: u64,
    _natural_exit: i32,
) -> (i32, Option<String>) {
    todo!("DELIVER 05-05: bind to the pass deadline decision (B13)")
}

fn sut_parse_setting(_variable: &str, _value: &str) -> Result<(), String> {
    todo!("DELIVER 05-06: bind to openlore-indexer config parsing of the new settings (data-models §1)")
}

fn sut_route(_method: &str, _path: &str) -> Route {
    todo!("DELIVER 05-07: bind to adapter-xrpc-query-server routing (ADR-083 §1)")
}

fn sut_admit(_body_len: usize, _value_len: usize) -> Admission {
    todo!("DELIVER 05-08: bind to the public request bounds (ADR-083 §3)")
}

fn sut_health_view(
    _store_usable: bool,
    _last_success_epoch_secs: Option<i64>,
) -> (u16, serde_json::Value) {
    todo!("DELIVER 05-09: bind to the /healthz projection of PassStatus (ADR-083 §2)")
}

fn sut_purge_author(_store: &StoreView, _bare: &str) -> (StoreView, u64) {
    todo!(
        "DELIVER 05-10: seed a temp index.duckdb + artifact dir from `store` through the REAL \
         adapter-index-store, call IndexPurgePort::purge_author(bare), read it back"
    )
}

// =============================================================================
// Oracles (from the ADR / data-model text)
// =============================================================================

/// BareDid: the text of an `author_did` before the first `#`.
fn bare(author_did: &str) -> String {
    author_did.split('#').next().unwrap_or("").to_string()
}

/// `did:(plc|web):[A-Za-z0-9._:%-]+`, not ending in `:`, ≤ 2048 bytes (config.rs `repo_did`).
fn well_formed_repo_did(entry: &str) -> bool {
    let Some((method, id)) = entry.strip_prefix("did:").and_then(|r| r.split_once(':')) else {
        return false;
    };
    entry.len() <= 2048
        && (method == "plc" || method == "web")
        && !id.is_empty()
        && !id.ends_with(':')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-'))
}

fn list_oracle(text: &str) -> ListReadView {
    let mut seen: Vec<String> = Vec::new();
    for entry in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|e| !e.is_empty())
    {
        if !well_formed_repo_did(entry) {
            return ListReadView::Malformed(entry.to_string());
        }
        if !seen.iter().any(|s| s == entry) {
            seen.push(entry.to_string());
        }
    }
    ListReadView::Loaded(seen)
}

fn exit_oracle(o: &PassOutcomeView) -> i32 {
    if o.list_refused || o.purge_failed || o.upsert_failed || o.panicked || o.deadline_exceeded {
        2
    } else if !o.dids.is_empty() && o.dids.iter().all(|d| *d == DidOutcome::Skipped) {
        3
    } else {
        0
    }
}

fn purge_oracle(store: &StoreView, target: &str) -> (StoreView, u64) {
    let purged: BTreeSet<(String, String, String)> = store
        .claims
        .iter()
        .filter(|(author, _, _)| bare(author) == target)
        .cloned()
        .collect();
    let cids: BTreeSet<&String> = purged.iter().map(|(_, cid, _)| cid).collect();
    let paths: BTreeSet<&String> = purged.iter().map(|(_, _, p)| p).collect();
    let after = StoreView {
        claims: store.claims.difference(&purged).cloned().collect(),
        evidence: store
            .evidence
            .iter()
            .filter(|(cid, _)| !cids.contains(cid))
            .cloned()
            .collect(),
        references: store
            .references
            .iter()
            .filter(|(referencing, _)| !cids.contains(referencing))
            .cloned()
            .collect(),
        artifacts: store
            .artifacts
            .iter()
            .filter(|p| !paths.contains(p))
            .cloned()
            .collect(),
    };
    (after, purged.len() as u64)
}

// =============================================================================
// Generators
// =============================================================================

/// A small DID pool with deliberate look-alikes: a prefix pair and a did:web
/// pair whose filesystem segments collide (`did_to_fs_segment` is not injective).
const DID_POOL: [&str; 8] = [
    "did:plc:priyaraman7x2k",
    "did:plc:priyaraman7x2k_x",
    "did:plc:dvolkov3m9q",
    "did:plc:jeffbailey5n2p",
    "did:plc:therrera2v6w",
    "did:web:a:b",
    "did:web:a_b",
    "did:web:example.com",
];

fn did() -> impl Strategy<Value = String> {
    prop::sample::select(DID_POOL.to_vec()).prop_map(str::to_string)
}

fn indexed_author_id() -> impl Strategy<Value = String> {
    (did(), any::<bool>()).prop_map(|(d, app)| {
        if app {
            format!("{d}#org.openlore.application")
        } else {
            d
        }
    })
}

fn segment(author_did: &str) -> String {
    author_did.split('#').next().unwrap_or("").replace(':', "_")
}

/// A store of 0..12 claims by pool authors, with evidence, cross-author
/// references (including references TO claims that will be purged) and one
/// artifact file per claim at its stored `signed_record_path`.
fn store() -> impl Strategy<Value = StoreView> {
    prop::collection::vec(
        (
            indexed_author_id(),
            0u8..3,
            prop::collection::vec(0usize..12, 0..3),
        ),
        0..12,
    )
    .prop_map(|rows| {
        let mut s = StoreView::default();
        for (i, (author, evidence, refs)) in rows.iter().enumerate() {
            let cid = format!("bafyclaim{i:03}");
            let path = format!("indexed_claims/{}/{cid}.json", segment(author));
            s.claims.insert((author.clone(), cid.clone(), path.clone()));
            s.artifacts.insert(path);
            for e in 0..*evidence {
                s.evidence
                    .insert((cid.clone(), format!("https://example.org/evidence/{i}/{e}")));
            }
            for r in refs {
                if *r < rows.len() && *r != i {
                    s.references
                        .insert((cid.clone(), format!("bafyclaim{r:03}")));
                }
            }
        }
        s
    })
}

fn outcome() -> impl Strategy<Value = PassOutcomeView> {
    (
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(
            prop_oneof![Just(DidOutcome::Read), Just(DidOutcome::Skipped)],
            0..6,
        ),
    )
        .prop_map(
            |(list_refused, purge_failed, upsert_failed, panicked, deadline_exceeded, dids)| {
                PassOutcomeView {
                    list_refused,
                    purge_failed,
                    upsert_failed,
                    panicked,
                    deadline_exceeded,
                    dids,
                }
            },
        )
}

/// Text that looks like an operator's list: DIDs, near-DIDs, separators, BOMs, CRLFs.
fn list_text() -> impl Strategy<Value = String> {
    let entry = prop_oneof![
        4 => did(),
        1 => Just("tomas".to_string()),
        1 => Just("did:key:z6Mkabc".to_string()),
        1 => Just("did:plc:".to_string()),
        1 => Just("\u{feff}did:plc:dvolkov3m9q".to_string()),
        1 => "[ -~]{0,12}",
    ];
    let sep = prop_oneof![
        Just(","),
        Just(" "),
        Just(", "),
        Just("\n"),
        Just("\r\n"),
        Just("\t")
    ];
    prop::collection::vec((entry, sep), 0..8)
        .prop_map(|parts| parts.into_iter().map(|(e, s)| format!("{e}{s}")).collect())
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-1/2/3 @US-IXD-003 @AC-003.2 @ADR-082 @property @contract-shape:pure-function
    /// For every loaded list and every set of indexed authors: an empty list
    /// suppresses the purge; otherwise the plan is exactly bare(indexed) − list,
    /// so it never names a listed DID and never names an author not indexed.
    #[test]
    #[ignore = "DELIVER 05-01: plan_purge"]
    fn the_purge_plan_is_the_set_of_indexed_authors_no_longer_listed(
        listed in prop::collection::vec(did(), 0..6),
        indexed in prop::collection::btree_set(indexed_author_id(), 0..8),
    ) {
        let plan = sut_plan_purge(&listed, &indexed);
        if listed.is_empty() {
            prop_assert_eq!(plan, PurgePlanView::Suppressed);
        } else {
            let listed_set: BTreeSet<String> = listed.iter().cloned().collect();
            let indexed_bare: BTreeSet<String> = indexed.iter().map(|a| bare(a)).collect();
            let expected: BTreeSet<String> = indexed_bare.difference(&listed_set).cloned().collect();
            let PurgePlanView::Purge(planned) = plan else {
                return Err(TestCaseError::fail("a non-empty list never suppresses the purge"));
            };
            prop_assert!(planned.is_disjoint(&listed_set), "never purges a listed DID");
            prop_assert!(planned.is_subset(&indexed_bare), "only indexed authors");
            prop_assert_eq!(planned, expected);
        }
    }

    /// CORE-4 @US-IXD-003 @AC-003.3 @ADR-081 @property @C6a @contract-shape:pure-function
    /// Reading the list is total: for any text it either loads the distinct
    /// well-formed DIDs in first-seen order, or refuses naming the FIRST bad
    /// entry. A BOM makes the first entry bad; CRLF is whitespace.
    #[test]
    #[ignore = "DELIVER 05-02: per-pass DID-file parse"]
    fn reading_the_did_list_loads_it_whole_or_names_the_first_bad_entry(text in list_text()) {
        prop_assert_eq!(sut_read_did_list(&text), list_oracle(&text));
    }

    /// CORE-5 @US-IXD-004 @AC-004.1 @AC-004.2 @AC-004.3 @B4 @property @contract-shape:pure-function
    /// A local failure (refused list, purge, store write, panic, deadline) is 2
    /// and wins over a total outage (3, every listed DID skipped), which wins
    /// over 0; a cause is named exactly when the exit is 2.
    #[test]
    #[ignore = "DELIVER 05-03: pass_exit_code extended"]
    fn a_pass_s_exit_code_puts_local_failures_before_outages_before_success(o in outcome()) {
        let (code, cause) = sut_pass_exit(&o);
        prop_assert_eq!(code, exit_oracle(&o));
        prop_assert_eq!(cause.is_some(), code == 2);
    }

    /// CORE-6 @US-IXD-002 @AC-002.4 @ADR-080 @H2 @property @C2b @contract-shape:pure-function
    /// Model-based: over any sequence of requests and pass endings, at most one
    /// pass runs, a request during a pass gets `busy` naming THAT pass, every
    /// ending (completed, panicked, deadline) frees the slot, and pass ids never repeat.
    #[test]
    #[ignore = "DELIVER 05-04: single-flight transition"]
    fn the_runner_never_runs_two_passes_and_always_frees_its_slot(
        events in prop::collection::vec(
            prop_oneof![
                3 => Just(RunnerEvent::RunPassRequested),
                1 => Just(RunnerEvent::PassEnded(EndKind::Completed)),
                1 => Just(RunnerEvent::PassEnded(EndKind::Panicked)),
                1 => Just(RunnerEvent::PassEnded(EndKind::DeadlineExceeded)),
            ],
            0..40,
        ),
    ) {
        let mut state = RunnerView::Idle;
        let mut next_id = 1u64;
        let mut started: Vec<u64> = Vec::new();
        for event in events {
            let (after, reply) = sut_single_flight(state, event, next_id);
            match (state, event) {
                (RunnerView::Idle, RunnerEvent::RunPassRequested) => {
                    prop_assert_eq!(reply, RunnerReply::Started(next_id));
                    prop_assert_eq!(after, RunnerView::Running(next_id));
                    prop_assert!(!started.contains(&next_id), "pass ids never repeat");
                    started.push(next_id);
                    next_id += 1;
                }
                (RunnerView::Running(id), RunnerEvent::RunPassRequested) => {
                    prop_assert_eq!(reply, RunnerReply::Busy { running_pass_id: id });
                    prop_assert_eq!(after, RunnerView::Running(id));
                }
                (RunnerView::Running(_), RunnerEvent::PassEnded(_)) => {
                    prop_assert_eq!(reply, RunnerReply::Freed);
                    prop_assert_eq!(after, RunnerView::Idle);
                }
                (RunnerView::Idle, RunnerEvent::PassEnded(_)) => {
                    prop_assert_eq!(reply, RunnerReply::Ignored, "an ending with no pass changes nothing");
                    prop_assert_eq!(after, RunnerView::Idle);
                }
            }
            state = after;
        }
    }

    /// CORE-7 @US-IXD-002 @M1 @B13 @property @contract-shape:pure-function
    /// Past the deadline the pass is a failure named `pass_deadline_exceeded`,
    /// whatever it would otherwise have ended with; within it nothing changes.
    #[test]
    #[ignore = "DELIVER 05-05: pass deadline decision"]
    fn a_pass_past_its_deadline_fails_whatever_it_would_have_said(
        deadline_secs in 60u64..=7200,
        elapsed_ms in 0u64..8_000_000,
        natural in prop::sample::select(vec![0, 2, 3]),
    ) {
        let (code, cause) = sut_deadline_outcome(elapsed_ms, deadline_secs, natural);
        if elapsed_ms > deadline_secs * 1000 {
            prop_assert_eq!(code, 2);
            prop_assert_eq!(cause.as_deref(), Some("pass_deadline_exceeded"));
        } else {
            prop_assert_eq!(code, natural);
        }
    }

    /// CORE-8 @US-IXD-006 @data-models-1 @property @C1b @C6a @contract-shape:pure-function
    /// Every new numeric setting accepts exactly its range and refuses
    /// anything else, including non-numbers.
    #[test]
    #[ignore = "DELIVER 05-06: new settings parse"]
    fn each_new_setting_accepts_exactly_its_range(
        n in -10i64..10_000,
        junk in "[a-z ]{1,6}",
    ) {
        let ranges: [(&str, i64, i64); 3] = [
            ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", 16, 1024),
            ("OPENLORE_INDEXER_DUCKDB_THREADS", 1, 4),
            ("OPENLORE_INDEXER_PASS_DEADLINE_SECS", 60, 7200),
        ];
        for (variable, lo, hi) in ranges {
            prop_assert_eq!(sut_parse_setting(variable, &n.to_string()).is_ok(), (lo..=hi).contains(&n), "{}={}", variable, n);
            prop_assert!(sut_parse_setting(variable, &junk).is_err(), "{}={:?}", variable, junk);
        }
        prop_assert_eq!(sut_parse_setting("OPENLORE_INDEXER_PURGE_UNLISTED", &n.to_string()).is_ok(), n == 1);
    }

    /// CORE-9 @US-IXD-001 @AC-001.3 @FR-IXD-2 @ADR-083 @property @C6a @contract-shape:pure-function
    /// Of every method and path, exactly POST searchClaims is search and GET
    /// /healthz is health; everything else (near misses included) is not found.
    #[test]
    #[ignore = "DELIVER 05-07: public route allowlist"]
    fn only_two_routes_exist_on_the_public_listener(
        method in prop::sample::select(vec!["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "post"]),
        path in prop_oneof![
            Just("/xrpc/org.openlore.appview.searchClaims".to_string()),
            Just("/healthz".to_string()),
            Just("/xrpc/org.openlore.appview.searchClaims/".to_string()),
            Just("/HEALTHZ".to_string()),
            Just("/healthz/".to_string()),
            Just("/xrpc/com.atproto.repo.createRecord".to_string()),
            "/[a-zA-Z0-9./_-]{0,40}",
        ],
    ) {
        let expected = match (method, path.as_str()) {
            ("POST", "/xrpc/org.openlore.appview.searchClaims") => Route::Search,
            ("GET", "/healthz") => Route::Health,
            _ => Route::NotFound,
        };
        prop_assert_eq!(sut_route(method, &path), expected);
    }

    /// CORE-9b @US-IXD-001 @NFR-IXD-7 @ADR-083-3 @property @C1b @contract-shape:pure-function
    /// A body over 8 KiB is too large (checked first); otherwise a value over
    /// 512 bytes is a bad request; otherwise the search is admitted.
    #[test]
    #[ignore = "DELIVER 05-08: public request bounds"]
    fn requests_are_admitted_exactly_within_their_bounds(body_len in 0usize..20_000, value_len in 0usize..2_000) {
        let expected = if body_len > 8192 {
            Admission::TooLarge
        } else if value_len > 512 {
            Admission::BadRequest
        } else {
            Admission::Admitted
        };
        prop_assert_eq!(sut_admit(body_len, value_len), expected);
    }

    /// CORE-10 @US-IXD-002 @ADR-083-2 @H2 @property @contract-shape:pure-function
    /// An unusable store is always 503 `store_unusable`; a usable one is 200
    /// with exactly `status` and `last_successful_pass_at` (RFC3339 or null).
    #[test]
    #[ignore = "DELIVER 05-09: /healthz projection"]
    fn the_health_response_is_honest_and_minimal(
        usable in any::<bool>(),
        last in prop::option::of(1_600_000_000i64..2_100_000_000),
    ) {
        let (status, body) = sut_health_view(usable, last);
        if !usable {
            prop_assert_eq!(status, 503);
            prop_assert_eq!(body, serde_json::json!({"status": "store_unusable"}));
        } else {
            prop_assert_eq!(status, 200);
            let keys: BTreeSet<&str> = body.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
            prop_assert_eq!(keys, BTreeSet::from(["status", "last_successful_pass_at"]));
            prop_assert_eq!(&body["status"], "ok");
            match last {
                None => prop_assert!(body["last_successful_pass_at"].is_null()),
                Some(secs) => {
                    let text = body["last_successful_pass_at"].as_str().unwrap_or_default();
                    let parsed = chrono::DateTime::parse_from_rfc3339(text).map(|t| t.timestamp());
                    prop_assert_eq!(parsed.ok(), Some(secs));
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// CORE-11 @US-IXD-003 @AC-003.2 @ADR-082 @property @contract-shape:bounded-change
    /// Purging one bare DID changes exactly: its claims (bare and `#fragment`
    /// forms), their evidence and outgoing references, and the artifact files
    /// at THEIR stored paths. Look-alike DIDs (prefix pair, colliding did:web
    /// segments) and references other authors hold TO purged claims are
    /// untouched. Purging again removes nothing (idempotent, C4a).
    #[test]
    #[ignore = "DELIVER 05-10: IndexPurgePort::purge_author over the real adapter"]
    fn purging_an_author_changes_only_that_author_s_claims_and_files(
        before in store(),
        target in did(),
    ) {
        let (after, removed) = sut_purge_author(&before, &target);
        let (expected, expected_removed) = purge_oracle(&before, &target);
        prop_assert_eq!(removed, expected_removed);
        let snap = |s: &StoreView| -> HashMap<String, Vec<String>> {
            HashMap::from([
                ("index.claims".to_string(), s.claims.iter().map(|c| format!("{c:?}")).collect()),
                ("index.evidence".to_string(), s.evidence.iter().map(|c| format!("{c:?}")).collect()),
                ("index.references".to_string(), s.references.iter().map(|c| format!("{c:?}")).collect()),
                ("index.artifact_files".to_string(), s.artifacts.iter().cloned().collect()),
            ])
        };
        let (b, a, e) = (snap(&before), snap(&after), snap(&expected));
        let universe: HashSet<String> = b.keys().cloned().collect();
        let delta = universe.iter().fold(Delta::new(), |d, slot| d.with_slot(slot.clone(), set_to(e[slot].clone())));
        assert_state_delta(&b, &a, &universe, &delta);

        let (again, removed_again) = sut_purge_author(&after, &target);
        prop_assert_eq!(removed_again, 0);
        prop_assert_eq!(again, after);
    }
}

// =============================================================================
// Pinned domain examples (readable anchors for reviewers)
// =============================================================================

/// CORE-1x @ADR-082 @adversarial @contract-shape:pure-function — the prefix look-alike is never planned.
#[test]
#[ignore = "DELIVER 05-01: plan_purge"]
fn the_purge_plan_never_names_a_listed_look_alike() {
    let indexed: BTreeSet<String> = [
        "did:plc:priyaraman7x2k",
        "did:plc:priyaraman7x2k_x#org.openlore.application",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let plan = sut_plan_purge(&["did:plc:priyaraman7x2k_x".to_string()], &indexed);
    assert_eq!(
        plan,
        PurgePlanView::Purge(BTreeSet::from(["did:plc:priyaraman7x2k".to_string()]))
    );
}

/// CORE-5x @AC-004.1 @contract-shape:pure-function — the closed table of exit codes.
#[test]
#[ignore = "DELIVER 05-03: pass_exit_code extended"]
fn the_exit_codes_of_the_canonical_passes() {
    let base = PassOutcomeView {
        list_refused: false,
        purge_failed: false,
        upsert_failed: false,
        panicked: false,
        deadline_exceeded: false,
        dids: vec![],
    };
    let table: BTreeMap<&str, (PassOutcomeView, i32)> = BTreeMap::from([
        ("no DIDs configured", (base.clone(), 0)),
        (
            "one read, one skipped",
            (
                PassOutcomeView {
                    dids: vec![DidOutcome::Read, DidOutcome::Skipped],
                    ..base.clone()
                },
                0,
            ),
        ),
        (
            "every DID skipped",
            (
                PassOutcomeView {
                    dids: vec![DidOutcome::Skipped; 3],
                    ..base.clone()
                },
                3,
            ),
        ),
        (
            "refused list",
            (
                PassOutcomeView {
                    list_refused: true,
                    ..base.clone()
                },
                2,
            ),
        ),
        (
            "purge failed while every DID skipped",
            (
                PassOutcomeView {
                    purge_failed: true,
                    dids: vec![DidOutcome::Skipped],
                    ..base.clone()
                },
                2,
            ),
        ),
    ]);
    for (label, (outcome, code)) in table {
        assert_eq!(sut_pass_exit(&outcome).0, code, "{label}");
    }
}
