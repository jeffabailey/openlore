//! The in-`serve` pass runner (ADR-080 §2/§5/§6, B2) — the effect shell around
//! the pure single-flight transition (`appview_domain::pass_runner`).
//!
//! The pass runs on a DEDICATED OS thread (its own runtime is built by the pass
//! body), never on the HTTP executor: the existing pass blocks on its runtime
//! in the gate phase, which would panic inside the query server's executor and
//! stall search. The runner holds at most one pass: a request while a pass
//! runs starts nothing and names the running pass. Each pass runs inside
//! `catch_unwind`, so a panic still frees the slot.
//!
//! The runner publishes a [`PassStatus`] that readers can only read: its fields
//! are private to this module and nothing outside the runner can set them.
//! `/healthz` reads it through a [`StatusReader`].
//!
//! Every pass carries a [`PassLabel`]: the runner's boot nonce (the Unix time
//! in milliseconds at which this runner started) and the pass's sequence
//! number, written `<boot>-<seq>`. The sequence alone repeats after a restart;
//! the boot nonce keeps a label unique across restarts, so "exactly one
//! `pass_summary` per `pass_id`" stays checkable over a whole log stream.
//!
//! The runner is the ONLY holder of the purge capability (ADR-082, check-arch
//! `index_purge_only_in_pass_runner`): [`purge_unlisted_authors`] plans the
//! purge from the loaded list (pure `plan_purge`) and purges each removed
//! author through `IndexPurgePort`, at pass start, before any fetch.

use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::Scope;
use std::time::{SystemTime, UNIX_EPOCH};

use appview_domain::ingest_pass::EXIT_PASS_COMPLETED;
use appview_domain::pass_runner::{
    single_flight, PassEnding, PassId, RunnerEvent, RunnerReply, RunnerSlot,
};
use appview_domain::{plan_purge, BareDid, PurgePlan, PurgeSuppressed};
use claim_domain::Did;
use ports::{IndexPurgePort, IndexStoreError, ProbeOutcome, PurgeReport};

/// The exit code a panicked pass reports (ADR-080 §7: `pass_panicked` → 2).
const EXIT_PASS_PANICKED: i32 = 2;

/// The process-unique label of one pass: `<boot nonce>-<sequence>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassLabel {
    boot: BootNonce,
    pass_id: PassId,
}

impl PassLabel {
    /// The pass's sequence number within this runner (1 = its first pass).
    #[must_use]
    pub fn pass_id(&self) -> PassId {
        self.pass_id
    }
}

impl fmt::Display for PassLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.boot.0, self.pass_id.0)
    }
}

/// When a runner started (Unix milliseconds); distinguishes its passes from
/// those of an earlier run of the same process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BootNonce(u128);

impl BootNonce {
    fn now() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_millis()),
        )
    }
}

/// The last pass the runner finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinishedPass {
    pub pass_id: PassId,
    pub ending: PassEnding,
    pub exit_code: i32,
}

/// The read-only view of the runner, published for readers (health, the
/// control channel). No setter is reachable outside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassStatus {
    slot: RunnerSlot,
    last_finished: Option<FinishedPass>,
    last_successful_pass_at: Option<SystemTime>,
}

impl PassStatus {
    const IDLE: Self = Self {
        slot: RunnerSlot::Idle,
        last_finished: None,
        last_successful_pass_at: None,
    };

    /// The pass running now, if any.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn running_pass(&self) -> Option<PassId> {
        match self.slot {
            RunnerSlot::Idle => None,
            RunnerSlot::Running(pass_id) => Some(pass_id),
        }
    }

    /// The last pass that ended, if any.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn last_finished(&self) -> Option<FinishedPass> {
        self.last_finished
    }

    /// When the last exit-0 pass ended (in memory only: `None` after a restart
    /// until the next one).
    #[must_use]
    pub fn last_successful_pass_at(&self) -> Option<SystemTime> {
        self.last_successful_pass_at
    }

    /// The status after `pass_id` ended at `ended_at`; only an exit-0 pass
    /// moves the last-success time.
    fn after_pass(self, slot: RunnerSlot, finished: FinishedPass, ended_at: SystemTime) -> Self {
        Self {
            slot,
            last_finished: Some(finished),
            last_successful_pass_at: if finished.exit_code == EXIT_PASS_COMPLETED {
                Some(ended_at)
            } else {
                self.last_successful_pass_at
            },
        }
    }
}

/// A read-only handle on a runner's published status, for readers that
/// outlive the runner's start handle (`/healthz`).
#[derive(Clone)]
pub struct StatusReader(Arc<Mutex<RunnerState>>);

impl StatusReader {
    /// The runner's status now.
    #[must_use]
    pub fn status(&self) -> PassStatus {
        lock(&self.0).status
    }
}

/// The answer to a pass request.
#[derive(Debug)]
pub enum PassRequestReply {
    /// A pass started; its exit code arrives on `outcome` when it ends.
    Started {
        pass_id: PassId,
        outcome: mpsc::Receiver<i32>,
    },
    /// A pass is already running; nothing started.
    Busy { running_pass_id: PassId },
}

/// The runner's shared state: the published status and the next id to mint.
struct RunnerState {
    status: PassStatus,
    next_id: PassId,
}

/// One pass handed to the runner thread.
struct PassOrder {
    pass_id: PassId,
    reply: mpsc::Sender<i32>,
}

/// The start handle of a runner whose pass thread lives in a scope. Dropping
/// it stops the thread once any running pass ends.
pub struct PassRunner {
    boot: BootNonce,
    state: Arc<Mutex<RunnerState>>,
    orders: mpsc::Sender<PassOrder>,
}

impl PassRunner {
    /// Spawn the runner's dedicated pass thread in `scope`. `pass` is the pass
    /// body (the same `ingest` pass, told its label); it returns the pass's
    /// exit code.
    pub fn spawn_scoped<'scope, 'env, F>(scope: &'scope Scope<'scope, 'env>, pass: F) -> Self
    where
        F: FnMut(PassLabel) -> i32 + Send + 'scope,
    {
        let boot = BootNonce::now();
        let state = Arc::new(Mutex::new(RunnerState {
            status: PassStatus::IDLE,
            next_id: PassId(1),
        }));
        let (orders, inbox) = mpsc::channel();
        let thread_state = Arc::clone(&state);
        scope.spawn(move || run_orders(&thread_state, &inbox, boot, pass));
        Self {
            boot,
            state,
            orders,
        }
    }

    /// The label a pass of this runner carries in events and replies.
    #[must_use]
    pub fn label(&self, pass_id: PassId) -> PassLabel {
        PassLabel {
            boot: self.boot,
            pass_id,
        }
    }

    /// Ask for one pass. Starts it when idle; otherwise reports the running pass.
    pub fn request_pass(&self) -> PassRequestReply {
        let mut state = lock(&self.state);
        let (slot, reply) = single_flight(
            state.status.slot,
            RunnerEvent::RunPassRequested,
            state.next_id,
        );
        state.status.slot = slot;
        match reply {
            RunnerReply::Started(pass_id) => {
                state.next_id = pass_id.next();
                let (reply, outcome) = mpsc::channel();
                if self.orders.send(PassOrder { pass_id, reply }).is_err() {
                    // The pass thread is gone: nothing will run, so free the slot.
                    state.status.slot = RunnerSlot::Idle;
                }
                PassRequestReply::Started { pass_id, outcome }
            }
            RunnerReply::Busy { running_pass_id } => PassRequestReply::Busy { running_pass_id },
            RunnerReply::Freed | RunnerReply::Ignored => {
                unreachable!("a pass request never frees or is ignored")
            }
        }
    }

    /// The runner's published status.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn status(&self) -> PassStatus {
        lock(&self.state).status
    }

    /// A reader of the published status that cannot start a pass.
    #[must_use]
    pub fn status_reader(&self) -> StatusReader {
        StatusReader(Arc::clone(&self.state))
    }
}

/// The pass thread: run each order, free the slot, report the exit code.
fn run_orders<F>(
    state: &Mutex<RunnerState>,
    inbox: &mpsc::Receiver<PassOrder>,
    boot: BootNonce,
    mut pass: F,
) where
    F: FnMut(PassLabel) -> i32,
{
    for order in inbox {
        let label = PassLabel {
            boot,
            pass_id: order.pass_id,
        };
        let (ending, exit_code) = match catch_unwind(AssertUnwindSafe(|| pass(label))) {
            Ok(exit_code) => (PassEnding::Completed, exit_code),
            Err(_panic) => (PassEnding::Panicked, EXIT_PASS_PANICKED),
        };
        end_pass(state, order.pass_id, ending, exit_code);
        let _ = order.reply.send(exit_code);
    }
}

/// Record a pass's ending and free the slot (the pure transition decides).
fn end_pass(state: &Mutex<RunnerState>, pass_id: PassId, ending: PassEnding, exit_code: i32) {
    let mut state = lock(state);
    let (slot, _reply) = single_flight(
        state.status.slot,
        RunnerEvent::PassEnded(ending),
        state.next_id,
    );
    let finished = FinishedPass {
        pass_id,
        ending,
        exit_code,
    };
    state.status = state.status.after_pass(slot, finished, SystemTime::now());
}

/// The runner's lock; a panic elsewhere never leaves the slot unreadable.
fn lock(state: &Mutex<RunnerState>) -> MutexGuard<'_, RunnerState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One author a pass purged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgedAuthor {
    pub did: BareDid,
    pub claims_removed: u64,
}

/// What a pass's purge step did (ADR-082).
#[derive(Debug)]
pub enum PurgeStep {
    /// The list was empty: nobody was purged.
    Suppressed(PurgeSuppressed),
    /// Every removed author was purged (possibly none).
    Purged(Vec<PurgedAuthor>),
    /// The store failed; the authors before the failure stay purged and the
    /// next pass's plan finishes the rest.
    Failed {
        purged: Vec<PurgedAuthor>,
        error: IndexStoreError,
    },
}

/// Purge every indexed author the loaded, non-empty `listed` no longer
/// names. A listed author is never planned, whatever their fetch outcome.
pub fn purge_unlisted_authors(purge: &dyn IndexPurgePort, listed: &[Did]) -> PurgeStep {
    let indexed = match purge.indexed_authors() {
        Ok(indexed) => indexed,
        Err(error) => {
            return PurgeStep::Failed {
                purged: Vec::new(),
                error,
            }
        }
    };
    match plan_purge(listed, indexed.iter().map(String::as_str)) {
        PurgePlan::Suppressed(reason) => PurgeStep::Suppressed(reason),
        PurgePlan::Purge(removed) => purge_each(purge, removed),
    }
}

/// Purge `removed` one author at a time, stopping at the first store failure.
fn purge_each(purge: &dyn IndexPurgePort, removed: impl IntoIterator<Item = BareDid>) -> PurgeStep {
    let mut purged = Vec::new();
    for did in removed {
        match purge.purge_author(&did.0) {
            Ok(report) => purged.push(PurgedAuthor {
                did,
                claims_removed: report.claims_removed,
            }),
            Err(error) => return PurgeStep::Failed { purged, error },
        }
    }
    PurgeStep::Purged(purged)
}

/// TEST-FAULT seam (`OPENLORE_INDEXER_TEST_FAULT=purge_fails`, debug builds
/// only): `purge`, except that its first author purge fails. The purge is
/// resumable, so the next pass finishes it.
pub fn failing_first_purge(
    purge: Arc<dyn IndexPurgePort + Send + Sync>,
) -> Arc<dyn IndexPurgePort + Send + Sync> {
    Arc::new(FailingFirstPurge {
        purge,
        failed_once: AtomicBool::new(false),
    })
}

struct FailingFirstPurge {
    purge: Arc<dyn IndexPurgePort + Send + Sync>,
    failed_once: AtomicBool,
}

impl IndexPurgePort for FailingFirstPurge {
    fn probe(&self) -> ProbeOutcome {
        self.purge.probe()
    }

    fn indexed_authors(&self) -> Result<std::collections::BTreeSet<String>, IndexStoreError> {
        self.purge.indexed_authors()
    }

    fn purge_author(&self, bare_did: &str) -> Result<PurgeReport, IndexStoreError> {
        if self.failed_once.swap(true, Ordering::SeqCst) {
            self.purge.purge_author(bare_did)
        } else {
            Err(IndexStoreError::QueryFailed {
                message: "test fault: the purge fails".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    // bypass: single-example wiring test (threads + channels); the single-flight
    // property itself is CORE-6 in tests/acceptance/indexer_deployment_core.rs.
    use super::*;
    use proptest::prelude::*;
    use std::time::Duration;

    proptest! {
        /// Universe {slot, last_finished, last_successful_pass_at}: every pass
        /// ending records itself and the freed slot; only an exit-0 ending moves
        /// the last-success time, to when it ended.
        #[test]
        fn only_a_successful_pass_moves_the_last_success_time(
            prior_secs in proptest::option::of(0u64..1_000),
            ended_secs in 1_000u64..2_000,
            exit_code in prop::sample::select(vec![0, 2, 3]),
            panicked in any::<bool>(),
        ) {
            let at = |secs: u64| UNIX_EPOCH + Duration::from_secs(secs);
            let before = PassStatus {
                slot: RunnerSlot::Running(PassId(7)),
                last_finished: None,
                last_successful_pass_at: prior_secs.map(at),
            };
            let finished = FinishedPass {
                pass_id: PassId(7),
                ending: if panicked { PassEnding::Panicked } else { PassEnding::Completed },
                exit_code,
            };

            let after = before.after_pass(RunnerSlot::Idle, finished, at(ended_secs));

            prop_assert_eq!(after.running_pass(), None);
            prop_assert_eq!(after.last_finished(), Some(finished));
            let expected = if exit_code == 0 { Some(at(ended_secs)) } else { prior_secs.map(at) };
            prop_assert_eq!(after.last_successful_pass_at(), expected);
        }
    }

    #[test]
    fn a_pass_runs_off_the_caller_thread_once_at_a_time_and_a_panic_frees_the_slot() {
        let caller = std::thread::current().id();
        std::thread::scope(|scope| {
            let (release, gate) = mpsc::channel::<bool>();
            let runner = PassRunner::spawn_scoped(scope, move |_pass_id| {
                assert_ne!(std::thread::current().id(), caller);
                assert!(gate.recv().unwrap(), "pass told to panic");
                3
            });

            let PassRequestReply::Started { pass_id, outcome } = runner.request_pass() else {
                panic!("an idle runner starts a pass");
            };
            assert_eq!(runner.status().running_pass(), Some(pass_id));
            match runner.request_pass() {
                PassRequestReply::Busy { running_pass_id } => assert_eq!(running_pass_id, pass_id),
                PassRequestReply::Started { .. } => panic!("a second pass started"),
            }
            release.send(true).unwrap();
            assert_eq!(outcome.recv().unwrap(), 3);
            assert_eq!(runner.status().running_pass(), None);
            assert_eq!(
                runner.status().last_finished(),
                Some(FinishedPass {
                    pass_id,
                    ending: PassEnding::Completed,
                    exit_code: 3
                })
            );

            let PassRequestReply::Started {
                pass_id: second,
                outcome,
            } = runner.request_pass()
            else {
                panic!("the slot was freed");
            };
            assert_ne!(second, pass_id);
            release.send(false).unwrap();
            assert_eq!(outcome.recv().unwrap(), EXIT_PASS_PANICKED);
            assert_eq!(runner.status().running_pass(), None);
            assert_eq!(
                runner.status().last_finished().map(|pass| pass.ending),
                Some(PassEnding::Panicked)
            );
        });
    }
}
