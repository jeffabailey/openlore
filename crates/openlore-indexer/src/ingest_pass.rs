//! `ingest_pass` — the body of one ingest pass (ADR-024, ADR-077/078/080):
//! load the DID list, purge removed authors (through the pass runner,
//! ADR-082), fetch each repo under its budget, gate and upsert its records,
//! and end with exactly one `indexer.ingest.pass_summary`.
//!
//! Runs either one-shot (`ingest`) or inside `serve` under a pass label; the
//! wiring it runs over is built and probed by the composition root (`run`).

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::time::{Duration, Instant};

use futures_util::stream::{self, StreamExt};

use appview_domain::did_list::read_did_list;
use appview_domain::ingest_pass::{
    classify_fetch_failure, pass_exit, refusal_cause_of, summarize_outcomes, within_deadline,
    ClassifiedSkip, FetchFailure, PassExit, PassFailure, RefusalCause, SkipReason,
};
use appview_domain::pass_runner::PassId;
use appview_domain::{
    ingest_repo_record, origin_of, plan_listing, records_of, DidFetch, IngestOutcome,
    ListingBudget, ListingPlan, ListingSource, PassSummary, RejectReason, ResolutionFailure,
};
use claim_domain::{ClaimRecord, Did, VerificationKey};
use ports::{IdentityLookupError, IngestError};

use crate::config::{TestFault, REPO_DIDS_FILE_VAR};
use crate::events::{emit_in, emit_in_unchecked};
use crate::pass_runner::{PassLabel, PurgeStep, PurgedAuthor, RunnerPurge};
use crate::run::{current_thread_runtime, IndexerWiring};

/// One pass inside `serve`, ending with exactly one summary whatever happens
/// (B4): a refused list, a store failure and a panic are summarised like a
/// completed pass, each with its cause. `purge` is the runner's purge, lent
/// for this pass when purging is enabled (ADR-082).
pub(crate) fn in_serve_pass(
    wiring: &IndexerWiring,
    pass: PassLabel,
    purge: Option<RunnerPurge<'_>>,
) -> i32 {
    let started = Instant::now();
    let work = catch_unwind(AssertUnwindSafe(|| {
        provoke_test_fault(wiring, pass);
        listed_pass_work(wiring, pass, purge, started)
    }))
    .unwrap_or_else(|_panic| PassWork::failed(PassFailure::PassPanicked));
    summarised(
        || finish_pass(Some(pass), &work, started, wiring.pass_deadline),
        || emit_in_unchecked(Some(pass), panicked_summary_event(started)),
    )
}

/// Run `finish`, which writes the pass's summary last; if it panics first,
/// write the `fallback` summary instead (exit 2, `pass_panicked`), so the
/// alarm on exit-2 summaries still fires (review L1).
fn summarised(finish: impl FnOnce() -> i32, fallback: impl FnOnce()) -> i32 {
    catch_unwind(AssertUnwindSafe(finish)).unwrap_or_else(|_panic| {
        fallback();
        PANICKED.code()
    })
}

/// How a pass whose summary panicked ended.
const PANICKED: PassExit = PassExit::Failed(PassFailure::PassPanicked);

/// The summary of a pass whose own summary panicked: nothing it did is known.
fn panicked_summary_event(started: Instant) -> serde_json::Value {
    pass_summary_event(
        &PassWork::failed(PassFailure::PassPanicked),
        PANICKED,
        started,
    )
}

/// The TEST-ONLY pass faults (debug builds only; config refuses the seam in a
/// release build): the first pass panics, or the store is poisoned.
fn provoke_test_fault(wiring: &IndexerWiring, pass: PassLabel) {
    match wiring.test_fault {
        Some(TestFault::FirstPassPanics) if pass.pass_id() == PassId(1) => {
            panic!("test fault: the first pass panics")
        }
        Some(TestFault::StorePoisoned) => wiring.store_condition.poison_for_test_fault(),
        _ => {}
    }
}

/// Load the DID list afresh (the file when one is configured, ADR-081),
/// announce it under the pass's label, do the pass's work. A refused list
/// fetches nothing.
fn listed_pass_work(
    wiring: &IndexerWiring,
    pass: PassLabel,
    purge: Option<RunnerPurge<'_>>,
    started: Instant,
) -> PassWork {
    match pass_list(wiring) {
        Ok((repo_dids, source)) => {
            emit_pass_config_loaded(wiring, pass, &repo_dids, source);
            match purge_removed_authors(purge, &repo_dids, pass) {
                Ok(purged_authors) => PassWork {
                    purged_authors,
                    ..pass_work(wiring, &repo_dids, Some(pass), started)
                },
                Err(purged_authors) => PassWork {
                    purged_authors,
                    ..PassWork::failed(PassFailure::PurgeFailed)
                },
            }
        }
        Err(refusal) => {
            emit_pass_refused(pass, &refusal);
            PassWork::failed(refusal.cause)
        }
    }
}

/// At pass start, before any fetch: when purging is enabled, purge the
/// authors this pass's loaded list no longer names (ADR-082), announcing
/// each. `Ok`/`Err` carry how many authors were purged; `Err` = the store
/// failed (exit 2, nothing is fetched; the next pass finishes the purge).
fn purge_removed_authors(
    purge: Option<RunnerPurge<'_>>,
    repo_dids: &[Did],
    pass: PassLabel,
) -> Result<u64, u64> {
    let Some(purge) = purge else {
        return Ok(0);
    };
    match purge.purge_unlisted(repo_dids) {
        PurgeStep::Suppressed(reason) => {
            emit_in(
                Some(pass),
                serde_json::json!({
                    "event": "indexer.ingest.purge_suppressed",
                    "reason": reason.token(),
                }),
            );
            Ok(0)
        }
        PurgeStep::Purged(purged) => Ok(emit_purged(pass, &purged)),
        PurgeStep::Failed { purged, error } => {
            eprintln!("openlore-indexer: purge failed: {error}");
            Err(emit_purged(pass, &purged))
        }
    }
}

/// `indexer.ingest.author_purged` per purged author (bare DID and count
/// only, no claim content); how many there were.
fn emit_purged(pass: PassLabel, purged: &[PurgedAuthor]) -> u64 {
    for author in purged {
        emit_in(
            Some(pass),
            serde_json::json!({
                "event": "indexer.ingest.author_purged",
                "did": author.did.0,
                "claims_removed": author.claims_removed,
            }),
        );
    }
    purged.len() as u64
}

/// Where a pass's DID list came from (`indexer.config.loaded.repo_dids_source`).
#[derive(Debug, Clone, Copy)]
enum ListSource {
    Env,
    /// The list file, last rendered `age_secs` ago (its mtime, B15); `None`
    /// when the platform keeps no modification time.
    File {
        age_secs: Option<u64>,
    },
}

impl ListSource {
    const fn token(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::File { .. } => "file",
        }
    }

    /// `repo_dids_age_secs` — reported for the file only.
    const fn age_secs(self) -> Option<u64> {
        match self {
            Self::Env => None,
            Self::File { age_secs } => age_secs,
        }
    }
}

/// Why a pass refused its DID list: the cause token and what to name.
#[derive(Debug)]
struct ListRefusal {
    cause: PassFailure,
    variable: &'static str,
    value: String,
}

/// This pass's DID list: the file read now when configured, else the
/// environment's list.
fn pass_list(wiring: &IndexerWiring) -> Result<(Vec<Did>, ListSource), ListRefusal> {
    let Some(file) = &wiring.repo_dids_file else {
        return Ok((wiring.repo_dids.clone(), ListSource::Env));
    };
    let (text, age_secs) = read_list_file(file).map_err(|_| ListRefusal {
        cause: PassFailure::ListUnreadable,
        variable: REPO_DIDS_FILE_VAR,
        value: file.display().to_string(),
    })?;
    read_did_list(&text)
        .map(|dids| (dids, ListSource::File { age_secs }))
        .map_err(|bad| ListRefusal {
            cause: PassFailure::ListMalformed,
            variable: REPO_DIDS_FILE_VAR,
            value: bad.entry,
        })
}

/// The list file's text and age, opened by PATH now (never a cached handle:
/// the host replaces the file by rename, H1) and aged from the same handle.
fn read_list_file(file: &Path) -> std::io::Result<(String, Option<u64>)> {
    use std::io::Read;
    let mut handle = std::fs::File::open(file)?;
    let mut text = String::new();
    handle.read_to_string(&mut text)?;
    let age_secs = handle
        .metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .map(|rendered| {
            std::time::SystemTime::now()
                .duration_since(rendered)
                .unwrap_or_default()
                .as_secs()
        });
    Ok((text, age_secs))
}

/// `indexer.ingest.pass_refused`: the list was refused; nothing is fetched.
fn emit_pass_refused(pass: PassLabel, refusal: &ListRefusal) {
    emit_in(
        Some(pass),
        serde_json::json!({
            "event": "indexer.ingest.pass_refused",
            "cause": refusal.cause.token(),
            "variable": refusal.variable,
            "value": refusal.value,
        }),
    );
}

/// `indexer.config.loaded` at a pass's start, after its list loaded.
fn emit_pass_config_loaded(
    wiring: &IndexerWiring,
    pass: PassLabel,
    repo_dids: &[Did],
    source: ListSource,
) {
    let mut loaded = serde_json::json!({
        "event": "indexer.config.loaded",
        "repo_did_count": repo_dids.len(),
        "repo_dids_source": source.token(),
        "fallback_configured": wiring.fallback.is_some(),
        "max_concurrent_fetches": wiring.max_concurrent_fetches,
        "per_did_time_budget_secs": wiring.per_did_time_budget.as_secs(),
        "transport_policy": wiring.policy.token(),
    });
    if let Some(age_secs) = source.age_secs() {
        loaded["repo_dids_age_secs"] = age_secs.into();
    }
    emit_in(Some(pass), loaded);
}

/// `openlore-indexer ingest` — a one-shot bounded PULL pass (ADR-024, ADR-077).
///
/// Fetch phase, per configured repo DID (DWD-9): resolve the DID's document
/// afresh (never cached across passes) → the PURE `plan_listing` decides the
/// listing source (its own PDS, else the fallback, else skip) → `listRecords`
/// with `repo=<DID>`, every cursor followed within the page bound.
///
/// Gate phase, in configured order: the PURE `records_of` keeps the repo's own
/// records → `origin_of` the listing source → resolve the author key for an
/// app-signed record → the PURE `ingest_repo_record` gate → on `Index` upsert
/// the attributed row; on `Reject` count the reason.
///
/// Emits `indexer.ingest.verified`, `indexer.ingest.rejected` and, last,
/// `indexer.ingest.pass_summary` on stdout (structural counts + DIDs only, NO
/// claim-content telemetry, WD-105).
pub(crate) fn ingest(wiring: &IndexerWiring) -> i32 {
    run_pass(wiring, &wiring.repo_dids, None)
}

/// One pass over `repo_dids`, ending with its one summary; every event
/// carries `pass`'s label when the pass runs inside `serve`.
fn run_pass(wiring: &IndexerWiring, repo_dids: &[Did], pass: Option<PassLabel>) -> i32 {
    let started = Instant::now();
    let work = pass_work(wiring, repo_dids, pass, started);
    finish_pass(pass, &work, started, wiring.pass_deadline)
}

/// What a pass's work came to, before its summary is written: the per-DID
/// accounting and the local failure that ended it, if one did.
struct PassWork {
    summary: PassSummary,
    failure: Option<PassFailure>,
    /// Authors purged at the pass's start (ADR-082).
    purged_authors: u64,
}

impl PassWork {
    /// A pass that ended on `failure` before it fetched anything.
    fn failed(failure: PassFailure) -> Self {
        Self {
            summary: PassSummary::default(),
            failure: Some(failure),
            purged_authors: 0,
        }
    }
}

/// Write the pass's ONE summary (last of its events) and return its exit code;
/// a pass that ran past `deadline` failed, whatever it came to (B13).
fn finish_pass(
    pass: Option<PassLabel>,
    work: &PassWork,
    started: Instant,
    deadline: Duration,
) -> i32 {
    let exit = within_deadline(
        pass_exit(work.failure, &work.summary),
        started.elapsed(),
        deadline,
    );
    emit_in(pass, pass_summary_event(work, exit, started));
    exit.code()
}

/// The pass's work. Each DID is gated as soon as its fetch is in (configured
/// order), so an author's claims are searchable while slower authors are
/// still being fetched; the first store failure ends the pass (ADR-078 §2).
/// The pass ends with an explicit checkpoint (ADR-080 §6). Whatever is still
/// outstanding when the pass's deadline (from `started`) passes is abandoned:
/// the pass ends there, keeping every upsert already committed (B13).
fn pass_work(
    wiring: &IndexerWiring,
    repo_dids: &[Did],
    pass: Option<PassLabel>,
    started: Instant,
) -> PassWork {
    let deadline = tokio::time::Instant::from_std(started + wiring.pass_deadline);
    let runtime = match current_thread_runtime() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("openlore-indexer: failed to build async runtime: {err}");
            return PassWork::failed(PassFailure::PassPanicked);
        }
    };

    let mut fetches = Box::pin(
        stream::iter(repo_dids)
            .map(|repo_did| fetch_repo(wiring, repo_did))
            .buffered(wiring.max_concurrent_fetches),
    );
    let mut outcomes = Vec::with_capacity(repo_dids.len());
    let mut tally = IngestTally::default();
    let mut author_keys = AuthorKeys::new(deadline);
    let mut failure = None;
    loop {
        let fetch = match runtime
            .block_on(async { tokio::time::timeout_at(deadline, fetches.next()).await })
        {
            Ok(Some(fetch)) => fetch,
            Ok(None) => break,
            Err(_deadline_passed) => {
                failure = Some(PassFailure::PassDeadlineExceeded);
                break;
            }
        };
        emit_fallback_read(pass, &fetch);
        emit_source_skipped(pass, &fetch);
        outcomes.push(fetch.outcome());
        if let Err(stop) = gate_fetch(wiring, &runtime, &fetch, &mut author_keys, &mut tally) {
            failure = Some(stop.failure());
            break;
        }
    }
    tally.emit(pass);
    checkpoint(wiring);
    PassWork {
        summary: summarize_outcomes(outcomes),
        failure,
        purged_authors: 0,
    }
}

/// The end-of-pass checkpoint. Everything it folds in is already committed,
/// so a failure is reported and changes nothing the pass reports.
fn checkpoint(wiring: &IndexerWiring) {
    if let Err(err) = wiring.index_store.checkpoint() {
        eprintln!("openlore-indexer: end-of-pass checkpoint failed: {err}");
    }
}

/// One repo DID's fetch: resolve its PDS, plan where to list it, list it.
/// Resolving and an own-PDS listing share ONE deadline; a fallback listing
/// runs under a fresh one (the pure plan says which — ADR-078 §4 amended).
/// Any failure skips only this DID, with its reason (ADR-078); nothing about
/// the skip is stored, so the DID is retried on the next pass.
async fn fetch_repo(wiring: &IndexerWiring, repo_did: &Did) -> DidFetch {
    let shared_deadline = tokio::time::Instant::now() + wiring.per_did_time_budget;
    let resolution = resolve_pds(wiring, repo_did, shared_deadline).await;
    let pds_url = resolution.as_ref().ok().cloned();
    match plan_listing(resolution, wiring.policy, wiring.fallback.as_ref()) {
        ListingPlan::List(source) => {
            let deadline = match source.budget() {
                ListingBudget::RemainingOfShared => shared_deadline,
                ListingBudget::Fresh => tokio::time::Instant::now() + wiring.per_did_time_budget,
            };
            list_source(wiring, repo_did, source, deadline).await
        }
        ListingPlan::Skip(reason) => DidFetch::Skipped {
            did: repo_did.clone(),
            skip: ClassifiedSkip::planned(reason),
            pds_url,
        },
    }
}

/// Resolve `repo_did`'s PDS afresh before `deadline` (never cached).
async fn resolve_pds(
    wiring: &IndexerWiring,
    repo_did: &Did,
    deadline: tokio::time::Instant,
) -> Result<String, ResolutionFailure> {
    match tokio::time::timeout_at(deadline, wiring.pds_lookup.resolve_pds(&repo_did.0)).await {
        Ok(resolved) => resolved.map_err(resolution_failure_of),
        Err(_elapsed) => Err(ResolutionFailure::TimedOut),
    }
}

/// List `repo_did` from its planned source before `deadline`; a failed or
/// timed-out listing becomes a classified skip naming the source that failed.
async fn list_source(
    wiring: &IndexerWiring,
    repo_did: &Did,
    source: ListingSource,
    deadline: tokio::time::Instant,
) -> DidFetch {
    let listing = wiring
        .repo_listing
        .list_repo_claims(source.base(), &repo_did.0);
    let failure = match tokio::time::timeout_at(deadline, listing).await {
        Ok(Ok(listing)) => {
            return DidFetch::Read {
                did: repo_did.clone(),
                source,
                listing,
            }
        }
        Ok(Err(err)) => {
            eprintln!("openlore-indexer: listing {} failed: {err}", repo_did.0);
            fetch_failure_of(&err)
        }
        Err(_elapsed) => {
            eprintln!("openlore-indexer: listing {} timed out", repo_did.0);
            FetchFailure::TimedOut
        }
    };
    DidFetch::Skipped {
        did: repo_did.clone(),
        skip: classify_fetch_failure(&source, failure),
        pds_url: Some(source.base().to_string()),
    }
}

/// How a listing error is classified by the pure core.
fn fetch_failure_of(error: &IngestError) -> FetchFailure {
    match error {
        IngestError::Unreachable { .. } => FetchFailure::Unreachable,
        IngestError::BadResponse { .. } | IngestError::ProbeRefused { .. } => {
            FetchFailure::BadResponse
        }
        IngestError::AddressRefused { .. } => FetchFailure::AddressRefused,
    }
}

/// Why a DID's PDS could not be resolved, as the pure planner sees it.
fn resolution_failure_of(error: IdentityLookupError) -> ResolutionFailure {
    match error {
        IdentityLookupError::NotFound => ResolutionFailure::NotFound,
        IdentityLookupError::Unavailable { .. } => ResolutionFailure::Unavailable,
    }
}

/// Run one fetched repo's records through the verify/provenance gate and
/// upsert the admitted ones. `Err` only for a store failure (fatal, exit 2).
fn gate_fetch(
    wiring: &IndexerWiring,
    runtime: &tokio::runtime::Runtime,
    fetch: &DidFetch,
    author_keys: &mut AuthorKeys,
    tally: &mut IngestTally,
) -> Result<(), GateStop> {
    let DidFetch::Read {
        did: repo_did,
        source,
        listing,
    } = fetch
    else {
        return Ok(());
    };
    let origin = origin_of(source, &listing.fetched_from);
    let bound = records_of(repo_did, &listing.records);
    tally.count(RefusalCause::ForeignRepo, bound.foreign);
    for (rkey, decoded) in bound.own {
        let Ok(record) = decoded else {
            tally.reject(&RejectReason::SchemaUnknown);
            continue;
        };
        let key = author_keys.of(wiring, runtime, &record)?;
        let outcome = ingest_repo_record(&record, &rkey, repo_did, origin, key.as_ref());
        record_outcome(wiring, repo_did, &rkey, outcome, tally).map_err(|err| {
            eprintln!("openlore-indexer: index upsert failed: {err}");
            GateStop::UpsertFailed
        })?;
    }
    Ok(())
}

/// Act on one record's gate decision: upsert an admitted claim, count a
/// refusal (naming a provenance refusal on stderr). `Err` only for a store
/// failure.
fn record_outcome(
    wiring: &IndexerWiring,
    repo_did: &Did,
    rkey: &str,
    outcome: IngestOutcome,
    tally: &mut IngestTally,
) -> Result<(), String> {
    match outcome {
        IngestOutcome::Index(claim) => {
            wiring
                .index_store
                .upsert(&claim)
                .map_err(|err| err.to_string())?;
            tally.verified += 1;
        }
        IngestOutcome::Reject(reason) => {
            if let RejectReason::Provenance(rejection) = &reason {
                eprintln!(
                    "openlore-indexer: refused at://{}/org.openlore.claim/{rkey}: {rejection}",
                    repo_did.0
                );
            }
            tally.reject(&reason);
        }
    }
    Ok(())
}

/// Emit `indexer.ingest.source_fallback` for a DID read through the fallback
/// (it was unresolvable — the only way onto that arm, ADR-077).
fn emit_fallback_read(pass: Option<PassLabel>, fetch: &DidFetch) {
    if let DidFetch::Read {
        did,
        source: ListingSource::Fallback(fallback),
        ..
    } = fetch
    {
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.source_fallback",
                "did": did.0,
                "reason": SkipReason::DidUnresolvable.token(),
                "fallback_url": fallback.as_str(),
            }),
        );
    }
}

/// Emit `indexer.ingest.source_skipped` for a DID that contributed nothing:
/// its DID and reason, the PDS only when one was resolved — NO claim content.
fn emit_source_skipped(pass: Option<PassLabel>, fetch: &DidFetch) {
    if let DidFetch::Skipped { did, skip, pds_url } = fetch {
        let mut event = serde_json::json!({
            "event": "indexer.ingest.source_skipped",
            "did": did.0,
            "reason": skip.reason.token(),
            "fallback_used": skip.fallback_used(),
        });
        if let Some(pds_url) = pds_url {
            event["pds_url"] = serde_json::Value::from(pds_url.as_str());
        }
        if let Some(fallback_failure) = skip.fallback_failure {
            event["fallback_failure"] = serde_json::Value::from(fallback_failure.token());
        }
        emit_in(pass, event);
    }
}

/// `indexer.ingest.pass_summary` — the pass's LAST stdout event, naming the
/// cause on exit 2, with how many authors it purged.
fn pass_summary_event(work: &PassWork, exit: PassExit, started: Instant) -> serde_json::Value {
    let summary = &work.summary;
    let mut event = serde_json::json!({
        "event": "indexer.ingest.pass_summary",
        "configured": summary.configured,
        "own_pds": summary.own_pds,
        "fallback": summary.fallback,
        "skipped": summary.skipped,
        "duration_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "exit_code": exit.code(),
        "purged_authors": work.purged_authors,
    });
    if let Some(cause) = exit.cause() {
        event["cause"] = cause.token().into();
    }
    event
}

/// Why the gate stopped a pass early.
#[derive(Debug, Clone, Copy)]
enum GateStop {
    /// A store write failed (exit 2 `upsert_failed`).
    UpsertFailed,
    /// The pass's deadline passed while an author key was being resolved.
    DeadlineExceeded,
}

impl GateStop {
    const fn failure(self) -> PassFailure {
        match self {
            Self::UpsertFailed => PassFailure::UpsertFailed,
            Self::DeadlineExceeded => PassFailure::PassDeadlineExceeded,
        }
    }
}

/// The author keys one pass resolved (ADR-080 §8): each author DID is
/// resolved at most once per pass, and never past the pass's deadline.
struct AuthorKeys {
    resolved: BTreeMap<String, Option<VerificationKey>>,
    deadline: tokio::time::Instant,
}

impl AuthorKeys {
    fn new(deadline: tokio::time::Instant) -> Self {
        Self {
            resolved: BTreeMap::new(),
            deadline,
        }
    }

    /// The resolved verification key of an app-signed record's author
    /// (ADR-026 resolve-only path); `None` when it cannot be resolved (the
    /// gate then refuses the record). A self-attested record has no key.
    fn of(
        &mut self,
        wiring: &IndexerWiring,
        runtime: &tokio::runtime::Runtime,
        record: &ClaimRecord,
    ) -> Result<Option<VerificationKey>, GateStop> {
        let ClaimRecord::AppSigned(signed) = record else {
            return Ok(None);
        };
        let author_did = &signed.unsigned.author_did;
        if let Some(key) = self.resolved.get(&author_did.0) {
            return Ok(key.clone());
        }
        let resolving = wiring.identity_resolve.resolve_verification_key(author_did);
        let key = runtime
            .block_on(async { tokio::time::timeout_at(self.deadline, resolving).await })
            .map_err(|_deadline_passed| GateStop::DeadlineExceeded)?
            .ok();
        self.resolved.insert(author_did.0.clone(), key.clone());
        Ok(key)
    }
}

/// The counts one ingest pass reports.
#[derive(Debug, Default)]
struct IngestTally {
    verified: u64,
    refused: BTreeMap<RefusalCause, u64>,
}

impl IngestTally {
    fn count(&mut self, cause: RefusalCause, times: u64) {
        *self.refused.entry(cause).or_default() += times;
    }

    fn reject(&mut self, reason: &RejectReason) {
        self.count(refusal_cause_of(reason), 1);
    }

    /// Emit the structured `indexer.ingest.verified` + `indexer.ingest.rejected`
    /// events to stdout (the DevOps observability contract). Structural counts +
    /// per-reason breakdown ONLY — NO claim-content telemetry (WD-105 privacy).
    fn emit(&self, pass: Option<PassLabel>) {
        let by_reason: serde_json::Map<String, serde_json::Value> = RefusalCause::ALL
            .iter()
            .map(|cause| {
                let count = self.refused.get(cause).copied().unwrap_or_default();
                (cause.token().to_string(), count.into())
            })
            .collect();
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.verified",
                "count": self.verified,
            }),
        );
        emit_in(
            pass,
            serde_json::json!({
                "event": "indexer.ingest.rejected",
                "count": self.refused.values().sum::<u64>(),
                "by_reason": by_reason,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Review L1: a pass whose summary panics still ends with one exit-2
    /// `pass_panicked` summary; a pass whose summary is written writes no other.
    #[test]
    fn a_panic_while_summarising_still_writes_one_exit_2_summary() {
        let fallbacks = Cell::new(0);
        let code = summarised(
            || panic!("stdout is gone"),
            || fallbacks.set(fallbacks.get() + 1),
        );
        assert_eq!((code, fallbacks.get()), (2, 1));

        let code = summarised(|| 3, || fallbacks.set(fallbacks.get() + 1));
        assert_eq!((code, fallbacks.get()), (3, 1));

        let event = panicked_summary_event(Instant::now());
        assert_eq!(event["event"], "indexer.ingest.pass_summary");
        assert_eq!(event["exit_code"], 2);
        assert_eq!(event["cause"], "pass_panicked");
    }
}
