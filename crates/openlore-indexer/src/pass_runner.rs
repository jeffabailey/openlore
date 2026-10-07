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
//!
//! Every pass carries a [`PassLabel`]: the runner's boot nonce (the Unix time
//! in milliseconds at which this runner started) and the pass's sequence
//! number, written `<boot>-<seq>`. The sequence alone repeats after a restart;
//! the boot nonce keeps a label unique across restarts, so "exactly one
//! `pass_summary` per `pass_id`" stays checkable over a whole log stream.

use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::Scope;
use std::time::{SystemTime, UNIX_EPOCH};

use appview_domain::pass_runner::{
    single_flight, PassEnding, PassId, RunnerEvent, RunnerReply, RunnerSlot,
};

/// The exit code a panicked pass reports (ADR-080 §7: `pass_panicked` → 2).
const EXIT_PASS_PANICKED: i32 = 2;

/// The process-unique label of one pass: `<boot nonce>-<sequence>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassLabel {
    boot: BootNonce,
    pass_id: PassId,
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
}

// `/healthz` (01-04) is the first production reader of the status.
#[cfg_attr(not(test), allow(dead_code))]
impl PassStatus {
    const IDLE: Self = Self {
        slot: RunnerSlot::Idle,
        last_finished: None,
    };

    /// The pass running now, if any.
    #[must_use]
    pub fn running_pass(&self) -> Option<PassId> {
        match self.slot {
            RunnerSlot::Idle => None,
            RunnerSlot::Running(pass_id) => Some(pass_id),
        }
    }

    /// The last pass that ended, if any.
    #[must_use]
    pub fn last_finished(&self) -> Option<FinishedPass> {
        self.last_finished
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

    /// The runner's published status (read by `/healthz`, 01-04).
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn status(&self) -> PassStatus {
        lock(&self.state).status
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
    state.status = PassStatus {
        slot,
        last_finished: Some(FinishedPass {
            pass_id,
            ending,
            exit_code,
        }),
    };
}

/// The runner's lock; a panic elsewhere never leaves the slot unreadable.
fn lock(state: &Mutex<RunnerState>) -> MutexGuard<'_, RunnerState> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    // bypass: single-example wiring test (threads + channels); the single-flight
    // property itself is CORE-6 in tests/acceptance/indexer_deployment_core.rs.
    use super::*;

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
