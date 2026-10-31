//! `events` — the indexer's structured event lines (the DevOps observability
//! contract, WD-105): one JSON object per line, through a writer the
//! composition root provides (UI-8; stdout in both binaries).

use std::io::{self, Write};
use std::sync::Arc;

use crate::runner_contract::PassLabel;

/// Where event lines go. The port is fallible so that [`Events`] alone decides
/// what a failed write means.
pub trait EventWriter: Send + Sync {
    fn write_line(&self, line: &str) -> io::Result<()>;
    fn flush(&self) -> io::Result<()>;
}

/// The production writer.
pub struct StdoutEvents;

impl EventWriter for StdoutEvents {
    fn write_line(&self, line: &str) -> io::Result<()> {
        writeln!(io::stdout().lock(), "{line}")
    }

    fn flush(&self) -> io::Result<()> {
        io::stdout().flush()
    }
}

/// A shared handle on the root's event writer.
#[derive(Clone)]
pub struct Events(Arc<dyn EventWriter>);

impl Events {
    #[must_use]
    pub fn new(writer: Arc<dyn EventWriter>) -> Self {
        Self(writer)
    }

    /// Write one structured event as a line. A failed write panics, so a pass
    /// whose summary cannot be written falls to its last-resort summary.
    ///
    /// # Panics
    /// When the writer refuses the line.
    pub fn emit(&self, event: &serde_json::Value) {
        self.0
            .write_line(&event.to_string())
            .expect("failed writing an event line");
    }

    /// Write one event and flush, so a line-reading supervisor sees it before
    /// whatever the process does next.
    pub fn emit_flushed(&self, event: &serde_json::Value) {
        self.emit(event);
        let _ = self.0.flush();
    }

    /// Write one pass event, stamped with the pass's label when it has one.
    pub fn emit_in(&self, pass: Option<PassLabel>, mut event: serde_json::Value) {
        stamp(pass, &mut event);
        self.emit(&event);
    }

    /// [`Self::emit_in`] that never panics: the last-resort writer for a pass
    /// whose summary could not be written (a failed write is dropped).
    pub fn emit_in_unchecked(&self, pass: Option<PassLabel>, mut event: serde_json::Value) {
        stamp(pass, &mut event);
        let _ = self.0.write_line(&event.to_string());
        let _ = self.0.flush();
    }
}

/// Stamp `event` with the pass's label when it has one.
fn stamp(pass: Option<PassLabel>, event: &mut serde_json::Value) {
    if let Some(pass) = pass {
        event["pass_id"] = pass.to_string().into();
    }
}
