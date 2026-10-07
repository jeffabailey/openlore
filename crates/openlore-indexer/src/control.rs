//! The control channel (ADR-080 §3, B3): how the host timer's
//! `openlore-indexer trigger` asks the running `serve` for one pass.
//!
//! Unix only, and only a Unix socket: the channel is never a network listener
//! (XD-5). `serve` binds the socket at an absolute path (replacing a stale
//! file a crash left behind), proves it with a connect round-trip before it
//! reports ready, and answers each connection with one reply line. `trigger`
//! reads ONLY the socket variable (M4): it parses no configuration and opens
//! no store, so it can never take the index lock from `serve`.
//!
//! The protocol is one JSON line each way (data-models §5):
//!   request  `{"request":"run_pass"}` | `{"request":"probe"}`
//!   reply    `{"reply":"completed","pass_id":…,"exit_code":0|2|3}`
//!            `{"reply":"busy","running_pass_id":…}` | `{"reply":"ready"}`
//!
//! The encoding, decoding and the trigger's exit-code mapping are pure; the
//! socket I/O sits in the shell functions below them.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::pass_runner::{PassRequestReply, PassRunner};

/// The variable `trigger` reads, and the only one.
pub const CONTROL_SOCKET_VAR: &str = crate::config::CONTROL_SOCKET_VAR;

/// `trigger`'s exit code when `serve` could not be reached: no pass ran.
pub const EXIT_SERVE_UNREACHABLE: i32 = 4;
/// `trigger`'s exit code when it joined a pass already running.
pub const EXIT_COALESCED: i32 = 0;

/// How long `serve` waits for a connected client's request line.
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the startup round-trip probe may take.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// How often the accept loop looks for a connection or the stop signal.
const ACCEPT_POLL: Duration = Duration::from_millis(25);

// =============================================================================
// The protocol (pure)
// =============================================================================

/// What a client asks `serve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRequest {
    /// Run one pass (or join the one running).
    RunPass,
    /// The startup round-trip check: is this socket answered by this `serve`?
    Probe,
}

/// What `serve` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlReply {
    /// The requested pass ran and ended with this exit code.
    Completed { pass_id: String, exit_code: i32 },
    /// A pass was already running; nothing new started.
    Busy { running_pass_id: String },
    /// The probe was answered.
    Ready,
}

/// Why `trigger` could not get an answer from `serve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreachable {
    /// The socket variable is unset or not an absolute path.
    SocketNotConfigured,
    /// Nothing accepted a connection at the socket.
    ConnectFailed,
    /// The connection closed before a reply (e.g. `serve` died mid-pass).
    ConnectionLost,
    /// The reply was not one this protocol knows.
    UnexpectedReply,
}

impl Unreachable {
    /// The `cause` token of `indexer.trigger.unreachable`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::SocketNotConfigured => "socket_not_configured",
            Self::ConnectFailed => "connect_failed",
            Self::ConnectionLost => "connection_lost",
            Self::UnexpectedReply => "unexpected_reply",
        }
    }
}

/// How one `trigger` run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerOutcome {
    /// The pass this trigger started ended with `exit_code`.
    Completed { exit_code: i32 },
    /// It joined the running pass.
    Coalesced { running_pass_id: String },
    /// `serve` could not be reached; no pass ran.
    Unreachable(Unreachable),
}

/// The exit code `trigger` hands the host timer: the pass's own code, 0 when
/// it joined a running pass, 4 when `serve` was unreachable.
#[must_use]
pub fn trigger_exit_code(outcome: &TriggerOutcome) -> i32 {
    match outcome {
        TriggerOutcome::Completed { exit_code } => *exit_code,
        TriggerOutcome::Coalesced { .. } => EXIT_COALESCED,
        TriggerOutcome::Unreachable(_) => EXIT_SERVE_UNREACHABLE,
    }
}

/// What a reply means to `trigger` (`None` reply = the connection was lost).
#[must_use]
pub fn trigger_outcome_of(reply: Option<ControlReply>) -> TriggerOutcome {
    match reply {
        Some(ControlReply::Completed { exit_code, .. }) => TriggerOutcome::Completed { exit_code },
        Some(ControlReply::Busy { running_pass_id }) => {
            TriggerOutcome::Coalesced { running_pass_id }
        }
        Some(ControlReply::Ready) => TriggerOutcome::Unreachable(Unreachable::UnexpectedReply),
        None => TriggerOutcome::Unreachable(Unreachable::ConnectionLost),
    }
}

/// One request line.
#[must_use]
pub fn encode_request(request: ControlRequest) -> String {
    let name = match request {
        ControlRequest::RunPass => "run_pass",
        ControlRequest::Probe => "probe",
    };
    format!("{}\n", json!({ "request": name }))
}

/// The request a line carries, if it is one.
#[must_use]
pub fn decode_request(line: &str) -> Option<ControlRequest> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("request")?.as_str()? {
        "run_pass" => Some(ControlRequest::RunPass),
        "probe" => Some(ControlRequest::Probe),
        _ => None,
    }
}

/// One reply line.
#[must_use]
pub fn encode_reply(reply: &ControlReply) -> String {
    let value = match reply {
        ControlReply::Completed { pass_id, exit_code } => {
            json!({ "reply": "completed", "pass_id": pass_id, "exit_code": exit_code })
        }
        ControlReply::Busy { running_pass_id } => {
            json!({ "reply": "busy", "running_pass_id": running_pass_id })
        }
        ControlReply::Ready => json!({ "reply": "ready" }),
    };
    format!("{value}\n")
}

/// The reply a line carries, if it is one.
#[must_use]
pub fn decode_reply(line: &str) -> Option<ControlReply> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let text = |field: &str| value.get(field)?.as_str().map(str::to_string);
    match value.get("reply")?.as_str()? {
        "completed" => Some(ControlReply::Completed {
            pass_id: text("pass_id")?,
            exit_code: i32::try_from(value.get("exit_code")?.as_i64()?).ok()?,
        }),
        "busy" => Some(ControlReply::Busy {
            running_pass_id: text("running_pass_id")?,
        }),
        "ready" => Some(ControlReply::Ready),
        _ => None,
    }
}

// =============================================================================
// `serve` side (effects)
// =============================================================================

/// Bind the control socket at `path`, replacing a stale socket file a crash
/// left behind, readable and writable by this uid only (0600).
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    remove_stale_socket(path)?;
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Remove whatever non-directory file sits at `path` (a stale socket or a
/// leftover regular file); a directory there is left for `bind` to refuse.
fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() || meta.is_file() => std::fs::remove_file(path),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

/// Answer every connection until `stop` is set. Each connection is answered
/// on its own thread, so a `busy` reply never waits behind a running pass.
pub fn answer_triggers(listener: &UnixListener, runner: &Arc<PassRunner>, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let runner = Arc::clone(runner);
                std::thread::spawn(move || answer(&stream, &runner));
            }
            Err(_) => std::thread::sleep(ACCEPT_POLL),
        }
    }
}

/// Read one request and write its reply (nothing is written for a request
/// that is not one, or a pass whose outcome never arrived).
fn answer(stream: &UnixStream, runner: &PassRunner) {
    // An accepted stream may inherit the listener's non-blocking mode.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT));
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = match decode_request(&line) {
        Some(ControlRequest::Probe) => Some(ControlReply::Ready),
        Some(ControlRequest::RunPass) => run_pass(runner),
        None => None,
    };
    if let Some(reply) = reply {
        let mut writer = stream;
        let _ = writer.write_all(encode_reply(&reply).as_bytes());
    }
}

/// Ask the runner for a pass and wait for its ending, or name the running one.
fn run_pass(runner: &PassRunner) -> Option<ControlReply> {
    match runner.request_pass() {
        PassRequestReply::Started { pass_id, outcome } => {
            outcome
                .recv()
                .ok()
                .map(|exit_code| ControlReply::Completed {
                    pass_id: runner.label(pass_id).to_string(),
                    exit_code,
                })
        }
        PassRequestReply::Busy { running_pass_id } => Some(ControlReply::Busy {
            running_pass_id: runner.label(running_pass_id).to_string(),
        }),
    }
}

/// The startup round-trip (ADR-080 Earned Trust): connect to `path`, ask
/// `probe`, require `ready` — proof the socket is this `serve`'s own.
pub fn probe_round_trip(path: &Path) -> io::Result<()> {
    let stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT))?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT))?;
    match exchange(&stream, ControlRequest::Probe)? {
        Some(ControlReply::Ready) => Ok(()),
        other => Err(io::Error::other(format!(
            "control socket probe answered {other:?}"
        ))),
    }
}

/// Send one request and read its reply (`None` = closed without a reply).
fn exchange(stream: &UnixStream, request: ControlRequest) -> io::Result<Option<ControlReply>> {
    let mut writer = stream;
    writer.write_all(encode_request(request).as_bytes())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(decode_reply(&line))
}

// =============================================================================
// `trigger` side (effects)
// =============================================================================

/// `openlore-indexer trigger`: ask `serve` at `socket` for one pass, report
/// how it went, and return the exit code for the host timer.
pub fn trigger(socket: Option<String>) -> i32 {
    let socket = socket.unwrap_or_default();
    let outcome = request_one_pass(&socket);
    report(&socket, &outcome);
    trigger_exit_code(&outcome)
}

/// Connect, request a pass, wait for its reply.
fn request_one_pass(socket: &str) -> TriggerOutcome {
    let path = Path::new(socket);
    if socket.trim().is_empty() || !path.is_absolute() {
        return TriggerOutcome::Unreachable(Unreachable::SocketNotConfigured);
    }
    let Ok(stream) = UnixStream::connect(path) else {
        return TriggerOutcome::Unreachable(Unreachable::ConnectFailed);
    };
    match exchange(&stream, ControlRequest::RunPass) {
        Ok(reply) => trigger_outcome_of(reply),
        Err(_) => TriggerOutcome::Unreachable(Unreachable::ConnectionLost),
    }
}

/// The trigger's own event: `coalesced` on stdout, `unreachable` on stderr.
fn report(socket: &str, outcome: &TriggerOutcome) {
    match outcome {
        TriggerOutcome::Completed { .. } => {}
        TriggerOutcome::Coalesced { running_pass_id } => println!(
            "{}",
            json!({
                "event": "indexer.trigger.coalesced",
                "running_pass_id": running_pass_id,
            })
        ),
        TriggerOutcome::Unreachable(cause) => eprintln!(
            "{}",
            json!({
                "event": "indexer.trigger.unreachable",
                "socket": socket,
                "cause": cause.token(),
            })
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_pass_id() -> impl Strategy<Value = String> {
        "[0-9]{1,13}-[0-9]{1,6}"
    }

    fn arb_reply() -> impl Strategy<Value = ControlReply> {
        prop_oneof![
            (arb_pass_id(), prop::sample::select(vec![0, 2, 3]))
                .prop_map(|(pass_id, exit_code)| ControlReply::Completed { pass_id, exit_code }),
            arb_pass_id().prop_map(|running_pass_id| ControlReply::Busy { running_pass_id }),
            Just(ControlReply::Ready),
        ]
    }

    fn arb_unreachable() -> impl Strategy<Value = Unreachable> {
        prop::sample::select(vec![
            Unreachable::SocketNotConfigured,
            Unreachable::ConnectFailed,
            Unreachable::ConnectionLost,
            Unreachable::UnexpectedReply,
        ])
    }

    proptest! {
        /// Every reply survives the wire as exactly one line.
        #[test]
        fn a_reply_is_one_line_and_decodes_to_itself(reply in arb_reply()) {
            let line = encode_reply(&reply);
            prop_assert_eq!(line.matches('\n').count(), 1);
            prop_assert!(line.ends_with('\n'));
            prop_assert_eq!(decode_reply(&line), Some(reply));
        }

        /// Every request survives the wire as exactly one line.
        #[test]
        fn a_request_is_one_line_and_decodes_to_itself(
            request in prop::sample::select(vec![ControlRequest::RunPass, ControlRequest::Probe])
        ) {
            let line = encode_request(request);
            prop_assert_eq!(line.matches('\n').count(), 1);
            prop_assert_eq!(decode_request(&line), Some(request));
        }

        /// Arbitrary text is never mistaken for a run_pass request unless it is one.
        #[test]
        fn arbitrary_text_decodes_to_nothing_or_a_known_message(line in "[ -~]{0,60}") {
            if let Some(request) = decode_request(&line) {
                prop_assert_eq!(decode_request(&encode_request(request)), Some(request));
            }
            if let Some(reply) = decode_reply(&line) {
                prop_assert_eq!(decode_reply(&encode_reply(&reply)), Some(reply));
            }
        }

        /// The timer sees the pass's own code for a pass it started, 0 when it
        /// joined a running pass, and 4 whenever serve could not answer.
        #[test]
        fn the_trigger_exits_with_the_pass_code_zero_when_coalesced_four_when_unreachable(
            reply in proptest::option::of(arb_reply()),
            cause in arb_unreachable(),
        ) {
            let outcome = trigger_outcome_of(reply.clone());
            let expected = match reply {
                Some(ControlReply::Completed { exit_code, .. }) => exit_code,
                Some(ControlReply::Busy { .. }) => EXIT_COALESCED,
                Some(ControlReply::Ready) | None => EXIT_SERVE_UNREACHABLE,
            };
            prop_assert_eq!(trigger_exit_code(&outcome), expected);
            prop_assert_eq!(
                trigger_exit_code(&TriggerOutcome::Unreachable(cause)),
                EXIT_SERVE_UNREACHABLE
            );
        }
    }
}

/// Behaviour contracts of the control channel over real Unix sockets: the
/// stale-file rule of `bind`, the probe round-trip, a `trigger` request
/// answered by a running `serve`, and the trigger's refusal causes.
#[cfg(test)]
#[path = "control_contracts.rs"]
mod contracts;
