//! `openlore` — the binary entry point.
//!
//! Parses args with clap, then delegates to `cli::dispatch`. Verb
//! handlers that need to drive async I/O (currently: `claim publish` /
//! `claim retract` via the `PdsPort` async-trait surface) own their
//! own tokio runtime — see `verbs::claim_publish::publish_signed_claim`.
//! Keeping the runtime construction local to the async verbs avoids
//! the "no runtime in runtime" panic that would fire if main were to
//! enter a runtime here AND a verb later tried `block_on` against it
//! from a sync context.

#![forbid(unsafe_code)]

use clap::Parser;
use cli::{dispatch, Cli};

fn main() -> std::process::ExitCode {
    let parsed = Cli::parse();
    let code = dispatch(parsed);
    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}
