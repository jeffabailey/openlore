//! `io` — stdin/stdout helpers + interactive prompt loop (ADR-003).
//!
//! Hosts the two-prompt UX seam in a single small module so verbs stay
//! pure-ish: they CONSTRUCT the preview text and CHOOSE which prompts
//! to issue; this module HANDLES the actual reads.
//!
//! ## TTY vs. scripted mode
//!
//! - **TTY mode** (default when `stdin.is_terminal()`): `prompt_enter`
//!   prints the prompt and blocks reading one line from stdin.
//! - **Scripted mode** (`--no-tty` flag OR stdin is NOT a TTY): each
//!   call to `prompt_enter` consumes one line from the pre-supplied
//!   stdin stream. EOF without input means "user canceled" — we treat
//!   that as a clean exit (not an error) per WS-3, since the user
//!   pressing Ctrl-D before confirming the sign is the canonical "I
//!   changed my mind" signal.
//!
//! The acceptance tests use scripted mode (subprocess + piped stdin)
//! for determinism. Production users run interactively.

use std::io::{IsTerminal, Read, Write};

/// One blocking read of a single line of input. Returns:
/// - `Ok(Some(line))` if a line was read (trailing `\n` stripped).
/// - `Ok(None)` if EOF was hit before any character (user canceled
///   without confirming).
/// - `Err(_)` if the underlying read failed (broken pipe, etc.).
///
/// Reads byte-by-byte from the underlying `Read` rather than wrapping
/// it in a fresh `BufReader` per call. This matters when the same
/// stdin is consumed across MULTIPLE prompts (the two-prompt flow
/// from ADR-003): a `BufReader` would over-read into its internal
/// 8KB buffer on the first call and drop bytes that belong to the
/// second prompt. Unbuffered byte reads keep the reader cursor in
/// lockstep with the wire so the publish prompt sees the "y/n" line
/// the test piped right after the sign prompt's `\n`.
pub fn read_one_line<R: Read>(reader: &mut R) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte)?;
        if n == 0 {
            // EOF. If we already accumulated input, return it (the
            // line ended with EOF instead of `\n`). If nothing was
            // read, this is the "user canceled" signal.
            if line.is_empty() {
                return Ok(None);
            }
            return Ok(Some(line));
        }
        let b = byte[0];
        if b == b'\n' {
            // End of line — strip a trailing `\r` if present (CRLF).
            if line.ends_with('\r') {
                line.pop();
            }
            return Ok(Some(line));
        }
        line.push(b as char);
    }
}

/// Print `prompt` to `writer`, flush, then read one line. The line is
/// returned with the trailing newline stripped. `Ok(None)` is "user
/// closed stdin without typing anything" (EOF) — the verb interprets
/// that as a clean cancel.
pub fn prompt_line<W: Write, R: Read>(
    writer: &mut W,
    reader: &mut R,
    prompt: &str,
) -> std::io::Result<Option<String>> {
    write!(writer, "{prompt}")?;
    writer.flush()?;
    read_one_line(reader)
}

/// Decide whether the current invocation is "scripted" (no interactive
/// TTY on stdin) or "interactive". Scripted mode is what the
/// acceptance-test subprocess fixture observes — stdin is a pipe, not
/// a terminal — and what passing `--no-tty` forces explicitly.
pub fn is_scripted(force_no_tty: bool) -> bool {
    force_no_tty || !std::io::stdin().is_terminal()
}
