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
//! | CORE-4 | DID-list read is total: Loaded(valid, distinct, first-seen) or Refused naming the first bad entry and its problem (literal table) | ADR-081, data-models §2 |
//! | CORE-5 | pass exit code: the one failure → 2 + its literal cause token; else 3 iff every listed DID skipped, else 0 (which failure wins: ingest_pass acceptance tests) | ADR-078 am., data-models §4 |
//! | CORE-6 | single-flight runner: never two passes; busy names the running pass; any ending frees the slot | ADR-080 §3, §7 |
//! | CORE-7 | pass deadline: past the deadline ⇒ exit 2 `pass_deadline_exceeded`, whatever the pass would have said | ADR-080, B13 |
//! | CORE-8 | the new settings accept exactly their ranges | data-models §1 |
//! | CORE-9 | public routes: exactly POST searchClaims + GET /healthz; bounds 8 KiB / 512 B | ADR-083 |
//! | CORE-10 | health projection: 503 `store_unusable` or 200 with exactly status + time | ADR-083 §2, data-models §6 |
//! | CORE-11 | `purge_author` universe: only the target's rows, children and artifact files change (state delta); idempotent | ADR-082, data-models §3 |
//!
//! **Moved (roadmap review F2):** CORE-8 lives in
//! `crates/openlore-indexer/src/deployment_settings_properties.rs`, CORE-9/9b in
//! `crates/adapter-xrpc-query-server/tests/public_surface_properties.rs`, CORE-11 in
//! `crates/adapter-index-store/tests/purge_properties.rs` — verbatim, same fn
//! names. The `cli` target may not link those crates (check-arch
//! CLI_FORBIDDEN_INDEXER_DEPS) and the indexer is bin-only. The rest stay here
//! and bind to pure functions in `appview-domain`.
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

use std::collections::BTreeSet;

use proptest::prelude::*;

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
    /// Names the first bad entry and what is wrong with it.
    Malformed(String, EntryProblemView),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryProblemView {
    NotADid,
    NotWellFormed,
    NamesNoRepo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DidOutcome {
    Read,
    Skipped,
}

/// The local fault that ended a pass (data-models §4 `cause`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PassFailureView {
    ListMalformed,
    ListUnreadable,
    UpsertFailed,
    PurgeFailed,
    PassPanicked,
    PassDeadlineExceeded,
}

/// What a pass reports: AT MOST ONE failure (the pass ends at its first,
/// ingest_pass.rs) plus each listed DID's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PassOutcomeView {
    failure: Option<PassFailureView>,
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

// =============================================================================
// RED binding seam — DELIVER binds each to ONE production call
// =============================================================================

fn sut_plan_purge(listed: &[String], indexed_authors: &BTreeSet<String>) -> PurgePlanView {
    let listed: Vec<claim_domain::Did> = listed.iter().cloned().map(claim_domain::Did).collect();
    match appview_domain::plan_purge(&listed, indexed_authors.iter().map(String::as_str)) {
        appview_domain::PurgePlan::Purge(authors) => {
            PurgePlanView::Purge(authors.into_iter().map(|author| author.0).collect())
        }
        appview_domain::PurgePlan::Suppressed(_) => PurgePlanView::Suppressed,
    }
}

fn sut_read_did_list(text: &str) -> ListReadView {
    appview_domain::did_list::read_did_list(text).map_or_else(
        |bad| {
            use appview_domain::did_list::EntryProblem;
            let problem = match bad.problem {
                EntryProblem::NotADid => EntryProblemView::NotADid,
                EntryProblem::NotWellFormed => EntryProblemView::NotWellFormed,
                EntryProblem::NamesNoRepo => EntryProblemView::NamesNoRepo,
            };
            ListReadView::Malformed(bad.entry, problem)
        },
        |dids| ListReadView::Loaded(dids.into_iter().map(|did| did.0).collect()),
    )
}

fn sut_pass_exit(outcome: &PassOutcomeView) -> (i32, Option<String>) {
    use appview_domain::ingest_pass as pass;
    let failure = outcome.failure.map(|failure| match failure {
        PassFailureView::ListMalformed => pass::PassFailure::ListMalformed,
        PassFailureView::ListUnreadable => pass::PassFailure::ListUnreadable,
        PassFailureView::UpsertFailed => pass::PassFailure::UpsertFailed,
        PassFailureView::PurgeFailed => pass::PassFailure::PurgeFailed,
        PassFailureView::PassPanicked => pass::PassFailure::PassPanicked,
        PassFailureView::PassDeadlineExceeded => pass::PassFailure::PassDeadlineExceeded,
    });
    let summary = pass::summarize_outcomes(outcome.dids.iter().map(|did| match did {
        DidOutcome::Read => pass::PassOutcome::ReadFromOwnPds,
        DidOutcome::Skipped => pass::PassOutcome::Skipped,
    }));
    let exit = pass::pass_exit(failure, &summary);
    (
        exit.code(),
        exit.cause().map(|cause| cause.token().to_string()),
    )
}

fn sut_single_flight(
    state: RunnerView,
    event: RunnerEvent,
    next_id: u64,
) -> (RunnerView, RunnerReply) {
    use appview_domain::pass_runner as runner;
    let slot = match state {
        RunnerView::Idle => runner::RunnerSlot::Idle,
        RunnerView::Running(id) => runner::RunnerSlot::Running(runner::PassId(id)),
    };
    let event = match event {
        RunnerEvent::RunPassRequested => runner::RunnerEvent::RunPassRequested,
        RunnerEvent::PassEnded(end) => runner::RunnerEvent::PassEnded(match end {
            EndKind::Completed => runner::PassEnding::Completed,
            EndKind::Panicked => runner::PassEnding::Panicked,
            EndKind::DeadlineExceeded => runner::PassEnding::DeadlineExceeded,
        }),
    };
    let (after, reply) = runner::single_flight(slot, event, runner::PassId(next_id));
    let after = match after {
        runner::RunnerSlot::Idle => RunnerView::Idle,
        runner::RunnerSlot::Running(id) => RunnerView::Running(id.0),
    };
    let reply = match reply {
        runner::RunnerReply::Started(id) => RunnerReply::Started(id.0),
        runner::RunnerReply::Busy { running_pass_id } => RunnerReply::Busy {
            running_pass_id: running_pass_id.0,
        },
        runner::RunnerReply::Freed => RunnerReply::Freed,
        runner::RunnerReply::Ignored => RunnerReply::Ignored,
    };
    (after, reply)
}

fn sut_deadline_outcome(
    elapsed_ms: u64,
    deadline_secs: u64,
    natural_exit: i32,
) -> (i32, Option<String>) {
    use appview_domain::ingest_pass as pass;
    let natural = match natural_exit {
        0 => pass::PassExit::Completed,
        3 => pass::PassExit::TotalOutage,
        _ => pass::PassExit::Failed(pass::PassFailure::UpsertFailed),
    };
    let exit = pass::within_deadline(
        natural,
        std::time::Duration::from_millis(elapsed_ms),
        std::time::Duration::from_secs(deadline_secs),
    );
    (
        exit.code(),
        exit.cause().map(|cause| cause.token().to_string()),
    )
}

fn sut_health_view(
    store_usable: bool,
    last_success_epoch_secs: Option<i64>,
) -> (u16, serde_json::Value) {
    use appview_domain::health::{health_of, StoreHealth};
    let store = if store_usable {
        StoreHealth::Usable
    } else {
        StoreHealth::Unusable
    };
    let last = last_success_epoch_secs.and_then(|secs| chrono::DateTime::from_timestamp(secs, 0));
    let response = health_of(store, last);
    (response.status_code(), response.body())
}

// =============================================================================
// Oracles (from the ADR / data-model text)
// =============================================================================

/// BareDid: the text of an `author_did` before the first `#`.
fn bare(author_did: &str) -> String {
    author_did.split('#').next().unwrap_or("").to_string()
}

/// Each failure's exit and `cause` token, written out from data-models §4
/// (never from `PassFailure::token`).
const FAILURE_EXITS: [(PassFailureView, i32, &str); 6] = [
    (PassFailureView::ListMalformed, 2, "repo_dids_malformed"),
    (PassFailureView::ListUnreadable, 2, "repo_dids_unreadable"),
    (PassFailureView::UpsertFailed, 2, "upsert_failed"),
    (PassFailureView::PurgeFailed, 2, "purge_failed"),
    (PassFailureView::PassPanicked, 2, "pass_panicked"),
    (
        PassFailureView::PassDeadlineExceeded,
        2,
        "pass_deadline_exceeded",
    ),
];

/// Bad list entries and their problems, written out from data-models §2 and
/// the reader's contract examples. None contains a separator.
const BAD_ENTRIES: [(&str, EntryProblemView); 10] = [
    ("tomas", EntryProblemView::NotADid),
    ("did:plc", EntryProblemView::NotADid),
    ("\u{feff}did:plc:dvolkov3m9q", EntryProblemView::NotADid),
    ("did:key:z6Mkabc", EntryProblemView::NamesNoRepo),
    ("did:ethr:0xabc", EntryProblemView::NamesNoRepo),
    ("did:PLC:dvolkov3m9q", EntryProblemView::NotWellFormed),
    ("did:plc:", EntryProblemView::NotWellFormed),
    ("did:plc:abc:", EntryProblemView::NotWellFormed),
    ("did::abc", EntryProblemView::NotWellFormed),
    ("did:plc:a/b", EntryProblemView::NotWellFormed),
];

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

fn did_outcomes() -> impl Strategy<Value = Vec<DidOutcome>> {
    prop::collection::vec(
        prop_oneof![Just(DidOutcome::Read), Just(DidOutcome::Skipped)],
        0..6,
    )
}

fn separator() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just(","),
        Just(" "),
        Just(", "),
        Just("\n"),
        Just("\r\n"),
        Just("\t")
    ]
}

/// A list of known-good literal DIDs: the expected distinct DIDs in their
/// first-seen order, and a text naming them in that order, each optionally
/// followed by a repeat of one already named, joined by any separators.
fn good_list() -> impl Strategy<Value = (Vec<String>, String)> {
    Just(DID_POOL.to_vec())
        .prop_shuffle()
        .prop_flat_map(|pool| (0..=pool.len()).prop_map(move |n| pool[..n].to_vec()))
        .prop_flat_map(|distinct| {
            let n = distinct.len();
            (
                Just(distinct),
                prop::collection::vec((any::<prop::sample::Index>(), any::<bool>()), n),
                prop::collection::vec(separator(), 2 * n + 1),
                separator(),
            )
        })
        .prop_map(|(distinct, repeats, seps, lead)| {
            let mut text = lead.to_string();
            let mut seps = seps.into_iter();
            for (i, did) in distinct.iter().enumerate() {
                text.push_str(did);
                text.push_str(seps.next().unwrap_or(","));
                let (pick, repeat) = repeats[i];
                if repeat {
                    text.push_str(distinct[pick.index(i + 1)]);
                    text.push_str(seps.next().unwrap_or(","));
                }
            }
            (distinct.into_iter().map(str::to_string).collect(), text)
        })
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
    /// Reading the list is total: a list of known-good literal DIDs (any
    /// separators, CRLF included, and repeats) loads them distinct in
    /// first-seen order; putting a literal bad entry after any good prefix
    /// refuses the list naming THAT entry and its literal problem, whatever
    /// follows it. The literal examples are pinned in
    /// `the_did_list_examples_load_or_refuse_exactly`.
    #[test]
    fn reading_the_did_list_loads_it_whole_or_names_the_first_bad_entry(
        (expected, good) in good_list(),
        bad in proptest::option::of(prop::sample::select(BAD_ENTRIES.to_vec())),
        sep in separator(),
        rest in prop::collection::vec(
            prop_oneof![
                prop::sample::select(DID_POOL.to_vec()),
                prop::sample::select(BAD_ENTRIES.to_vec()).prop_map(|(entry, _)| entry),
            ],
            0..4,
        ),
    ) {
        match bad {
            None => prop_assert_eq!(sut_read_did_list(&good), ListReadView::Loaded(expected)),
            Some((entry, problem)) => {
                let text = format!("{good}{sep}{entry}{sep}{}", rest.join(sep));
                prop_assert_eq!(
                    sut_read_did_list(&text),
                    ListReadView::Malformed(entry.to_string(), problem)
                );
            }
        }
    }

    /// CORE-5 @US-IXD-004 @AC-004.1 @AC-004.2 @AC-004.3 @B4 @property @contract-shape:pure-function
    /// A pass that ended on a local failure exits 2 naming that failure's
    /// literal cause token, whatever its DIDs did; with no failure it exits 3
    /// when every listed DID was skipped (a total outage) and 0 otherwise,
    /// naming no cause. The code under test receives at most one failure.
    /// Which failure ends a pass (ingest_pass.rs stops at the first) is NOT
    /// pinned by any test today: `with_fault` takes one fault and PS-17/PS-20
    /// cover single failures only. Recorded as a follow-up in
    /// docs/feature/fix-indexer-deployment-follow-ups/rca.md.
    #[test]
    fn a_pass_s_exit_code_puts_local_failures_before_outages_before_success(
        failure in proptest::option::of(prop::sample::select(FAILURE_EXITS.to_vec())),
        dids in did_outcomes(),
    ) {
        let outcome = PassOutcomeView { failure: failure.map(|(failure, _, _)| failure), dids };
        let (code, cause) = sut_pass_exit(&outcome);
        match failure {
            Some((_, exit, token)) => {
                prop_assert_eq!((code, cause.as_deref()), (exit, Some(token)));
            }
            None if !outcome.dids.is_empty()
                && outcome.dids.iter().all(|did| *did == DidOutcome::Skipped) =>
            {
                prop_assert_eq!((code, cause), (3, None));
            }
            None => prop_assert_eq!((code, cause), (0, None)),
        }
    }

    /// CORE-6 @US-IXD-002 @AC-002.4 @ADR-080 @H2 @property @C2b @contract-shape:pure-function
    /// Model-based: over any sequence of requests and pass endings, at most one
    /// pass runs, a request during a pass gets `busy` naming THAT pass, every
    /// ending (completed, panicked, deadline) frees the slot, and pass ids never repeat.
    #[test]
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

    // CORE-8 `each_new_setting_accepts_exactly_its_range` moved VERBATIM to
    // crates/openlore-indexer/src/deployment_settings_properties.rs (settings
    // parsing is private to the indexer binary).
    // CORE-9 `only_two_routes_exist_on_the_public_listener` and CORE-9b
    // `requests_are_admitted_exactly_within_their_bounds` moved VERBATIM to
    // crates/adapter-xrpc-query-server/tests/public_surface_properties.rs
    // (cli may not link that crate: check-arch CLI_FORBIDDEN_INDEXER_DEPS).

    /// CORE-10 @US-IXD-002 @ADR-083-2 @H2 @property @contract-shape:pure-function
    /// An unusable store is always 503 `store_unusable`; a usable one is 200
    /// with exactly `status` and `last_successful_pass_at` (RFC3339 or null).
    #[test]
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

// CORE-11 `purging_an_author_changes_only_that_author_s_claims_and_files` moved
// VERBATIM to crates/adapter-index-store/tests/purge_properties.rs: it binds the
// REAL adapter, which cli may not link (check-arch CLI_FORBIDDEN_INDEXER_DEPS).

// =============================================================================
// Pinned domain examples (readable anchors for reviewers)
// =============================================================================

/// CORE-1x @ADR-082 @adversarial @contract-shape:pure-function — the prefix look-alike is never planned.
#[test]
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

/// A canonical pass: its label, what it reported, and its literal (exit, cause).
type CanonicalPass = (&'static str, PassOutcomeView, (i32, Option<&'static str>));

/// CORE-5x @AC-004.1 @contract-shape:pure-function — the closed table of exit codes.
#[test]
fn the_exit_codes_of_the_canonical_passes() {
    use DidOutcome::{Read, Skipped};
    let table: [CanonicalPass; 8] = [
        (
            "no DIDs configured",
            PassOutcomeView {
                failure: None,
                dids: vec![],
            },
            (0, None),
        ),
        (
            "every DID read",
            PassOutcomeView {
                failure: None,
                dids: vec![Read, Read],
            },
            (0, None),
        ),
        (
            "one read, one skipped",
            PassOutcomeView {
                failure: None,
                dids: vec![Read, Skipped],
            },
            (0, None),
        ),
        (
            "one DID, skipped",
            PassOutcomeView {
                failure: None,
                dids: vec![Skipped],
            },
            (3, None),
        ),
        (
            "every DID skipped",
            PassOutcomeView {
                failure: None,
                dids: vec![Skipped; 3],
            },
            (3, None),
        ),
        (
            "refused list",
            PassOutcomeView {
                failure: Some(PassFailureView::ListMalformed),
                dids: vec![],
            },
            (2, Some("repo_dids_malformed")),
        ),
        (
            "purge failed while every DID skipped",
            PassOutcomeView {
                failure: Some(PassFailureView::PurgeFailed),
                dids: vec![Skipped],
            },
            (2, Some("purge_failed")),
        ),
        (
            "store write failed after every DID read",
            PassOutcomeView {
                failure: Some(PassFailureView::UpsertFailed),
                dids: vec![Read, Read],
            },
            (2, Some("upsert_failed")),
        ),
    ];
    for (label, outcome, expected) in table {
        let (code, cause) = sut_pass_exit(&outcome);
        assert_eq!((code, cause.as_deref()), expected, "{label}");
    }
}

// bypass: literal oracle table; a generator would have to mirror the reader.
/// CORE-4x @ADR-081 @contract-shape:pure-function — the reader's literal examples.
#[test]
fn the_did_list_examples_load_or_refuse_exactly() {
    use EntryProblemView::{NamesNoRepo, NotADid, NotWellFormed};
    let loaded = |dids: &[&str]| ListReadView::Loaded(dids.iter().map(|d| d.to_string()).collect());
    let refused = |entry: &str, problem| ListReadView::Malformed(entry.to_string(), problem);
    let longest = format!("did:plc:{}", "a".repeat(2040));
    let too_long = format!("{longest}b");
    let table: Vec<(String, ListReadView)> = vec![
        (String::new(), loaded(&[])),
        (" \r\n\t, ,".to_string(), loaded(&[])),
        (
            "did:plc:a,did:plc:b".to_string(),
            loaded(&["did:plc:a", "did:plc:b"]),
        ),
        (
            "did:plc:a did:web:example.com".to_string(),
            loaded(&["did:plc:a", "did:web:example.com"]),
        ),
        (
            "did:plc:a\r\ndid:plc:b\r\n".to_string(),
            loaded(&["did:plc:a", "did:plc:b"]),
        ),
        (
            "did:plc:b, did:plc:a,did:plc:b did:plc:a".to_string(),
            loaded(&["did:plc:b", "did:plc:a"]),
        ),
        (
            "\u{feff}did:plc:a,did:plc:b".to_string(),
            refused("\u{feff}did:plc:a", NotADid),
        ),
        (
            "did:plc:a tomas did:plc:b".to_string(),
            refused("tomas", NotADid),
        ),
        (
            "did:key:z6Mkabc".to_string(),
            refused("did:key:z6Mkabc", NamesNoRepo),
        ),
        (
            "did:PLC:abc".to_string(),
            refused("did:PLC:abc", NotWellFormed),
        ),
        (
            "did:plc:a,did:key:z6Mk,tomas".to_string(),
            refused("did:key:z6Mk", NamesNoRepo),
        ),
        (
            "did:plc:a,did:plc:,did:key:z6Mk".to_string(),
            refused("did:plc:", NotWellFormed),
        ),
        (longest.clone(), loaded(&[longest.as_str()])),
        (
            format!("did:plc:a {too_long}"),
            refused(&too_long, NotWellFormed),
        ),
    ];
    assert_eq!(longest.len(), 2048);
    assert_eq!(too_long.len(), 2049);
    for (text, expected) in table {
        assert_eq!(sut_read_did_list(&text), expected, "{text:?}");
    }
}
