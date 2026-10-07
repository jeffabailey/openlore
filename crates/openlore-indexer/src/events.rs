//! `events` — the indexer's structured stdout event lines (the DevOps
//! observability contract, WD-105): one JSON object per line.

use std::io::Write;

use crate::pass_runner::PassLabel;

/// Print one structured event as a stdout line.
pub(crate) fn emit(event: serde_json::Value) {
    println!("{event}");
}

/// Print one event and flush stdout, so a line-reading supervisor sees it
/// before whatever the process does next.
pub(crate) fn emit_flushed(event: serde_json::Value) {
    emit(event);
    let _ = std::io::stdout().flush();
}

/// Print one pass event, stamped with the pass's label when it has one.
pub(crate) fn emit_in(pass: Option<PassLabel>, mut event: serde_json::Value) {
    if let Some(pass) = pass {
        event["pass_id"] = pass.to_string().into();
    }
    emit(event);
}

/// [`emit_in`] that never panics: the last-resort writer for a pass whose
/// summary could not be written (a failed write is dropped).
pub(crate) fn emit_in_unchecked(pass: Option<PassLabel>, mut event: serde_json::Value) {
    if let Some(pass) = pass {
        event["pass_id"] = pass.to_string().into();
    }
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{event}");
    let _ = stdout.flush();
}
