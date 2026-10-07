//! The pass runner's single-flight decision (ADR-080 §5, CORE-6) — pure.
//!
//! The runner holds at most one pass. A request while idle starts a pass under
//! the next pass id; a request while a pass runs starts nothing and names the
//! running pass (`busy`); any ending — completed, panicked, deadline — frees
//! the slot; an ending while idle changes nothing.
//!
//! NO I/O, NO threads: the effect shell (`openlore-indexer`'s pass runner)
//! applies this transition under its lock and does the running.

/// The identifier of one pass started by the runner. Ids are minted in
/// increasing order by the runner, so a pass id never repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PassId(pub u64);

impl PassId {
    /// The id the runner mints after this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// The runner's one slot: free, or held by exactly one running pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerSlot {
    Idle,
    Running(PassId),
}

/// How a pass ended. Every ending frees the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassEnding {
    Completed,
    Panicked,
    DeadlineExceeded,
}

/// What the runner is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerEvent {
    RunPassRequested,
    PassEnded(PassEnding),
}

/// What the runner answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerReply {
    /// A pass started under this id.
    Started(PassId),
    /// A pass is already running; nothing started (the request coalesces).
    Busy { running_pass_id: PassId },
    /// The running pass ended; the slot is free.
    Freed,
    /// An ending arrived with no pass running; nothing changed.
    Ignored,
}

/// The single-flight transition: the slot after `event`, and the reply.
/// `next_id` is the id a pass started now would carry.
#[must_use]
pub fn single_flight(
    slot: RunnerSlot,
    event: RunnerEvent,
    next_id: PassId,
) -> (RunnerSlot, RunnerReply) {
    match (slot, event) {
        (RunnerSlot::Idle, RunnerEvent::RunPassRequested) => {
            (RunnerSlot::Running(next_id), RunnerReply::Started(next_id))
        }
        (RunnerSlot::Running(running_pass_id), RunnerEvent::RunPassRequested) => {
            (slot, RunnerReply::Busy { running_pass_id })
        }
        (RunnerSlot::Running(_), RunnerEvent::PassEnded(_)) => {
            (RunnerSlot::Idle, RunnerReply::Freed)
        }
        (RunnerSlot::Idle, RunnerEvent::PassEnded(_)) => (RunnerSlot::Idle, RunnerReply::Ignored),
    }
}
