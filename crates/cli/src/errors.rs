//! Typed error → user-friendly stderr renderer for the cli composition root.
//!
//! Step 05-10 (WS-10): when `claim publish` (standalone OR chained from
//! `claim add` Y branch) fails because the configured PDS is
//! unreachable, the user MUST see an actionable retry hint naming the
//! standalone publish verb with the relevant CID baked in. Without it,
//! KPI-5's "local-first" promise becomes hollow — the user is left
//! holding a locally-signed claim with no obvious way to federate it
//! once their network or the remote PDS recovers.
//!
//! ## Design shape (functional / railway-oriented)
//!
//! `render_pds_error` is a pure function: `(PdsError, Cid) -> String`.
//! It is the only piece of policy in the codebase that knows the exact
//! retry verb syntax (`openlore claim publish <cid>`). Centralising it
//! here lets the composition root (`lib.rs::dispatch`) and the chained
//! Y branch in `claim_add::run` both emit identical, copy-pasteable
//! retry guidance regardless of which entry point hit the failure.
//!
//! ## Universe / observable surface
//!
//! The rendered string is the WS-10 universe slot
//! `cli.stderr.pds_unreachable_retry_hint`. Two load-bearing substrings:
//!
//! 1. `PDS` — the user-recognizable name of the federation boundary
//!    (US-003 vocabulary). WS-10's first stderr-contains assertion.
//! 2. `retry with \`openlore claim publish <cid>\`` — the exact retry
//!    verb the user should paste into their shell to recover.
//!
//! The substrings are intentionally lowercase `retry` (not `Retry`) so
//! they read naturally inside a sentence; WS-10 pins this casing.
//!
//! ## What `render_pds_error` does NOT do
//!
//! - It does NOT decide the exit code. The caller (dispatcher or
//!   `claim_add::run`) sets a non-zero exit code; this function only
//!   produces the stderr text.
//! - It does NOT rewrite or hide the underlying message. The raw error
//!   message from the adapter is appended so DevOps observability has
//!   the full classification (network error, TLS handshake, etc.) for
//!   debugging.
//! - It does NOT touch the local artefact. The KPI-5 invariant ("local
//!   <cid>.json persists intact on publish failure") is enforced
//!   structurally by `claim_add::run` writing locally BEFORE attempting
//!   to publish — there is no rollback path. This renderer only shapes
//!   what the user reads after the fact.

use claim_domain::Cid;
use ports::PdsError;

/// Render a `PdsError` into the user-facing stderr block for a failed
/// publish attempt. The output is a single line ending in a newline so
/// the caller can write it directly with `eprint!`.
///
/// The two load-bearing substrings WS-10 asserts on:
///
/// 1. `PDS` — appears verbatim so users recognise the federation
///    boundary that failed.
/// 2. `retry with \`openlore claim publish <cid>\`` — the actionable
///    retry hint with the exact verb. The backticks are part of the
///    rendered text (terminal-friendly) so users can copy-paste the
///    contents directly.
///
/// All PdsError variants render with the same retry-hint shape because
/// the user remediation is identical regardless of root cause: re-run
/// the standalone publish verb once the PDS is reachable again. The
/// raw error message is appended so operators can diagnose the failure
/// class (network vs. TLS vs. PDS rejecting the body).
pub fn render_pds_error(err: &PdsError, cid: &Cid) -> String {
    format!(
        "openlore: publish to PDS failed for claim {cid}: {err}. \
         The local claim file is intact; \
         retry with `openlore claim publish {cid}` once the PDS is reachable.\n",
        cid = cid.0,
        err = err,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WS-10 universe slot `cli.stderr.pds_unreachable_retry_hint`: the
    /// rendered string contains the `PDS` substring AND the
    /// actionable retry verb with the CID baked in. Pure-function
    /// unit test mirrors the acceptance-level assertion to keep the
    /// renderer's contract stable across refactors.
    #[test]
    fn render_pds_error_unreachable_includes_pds_and_retry_hint_with_cid() {
        let cid = Cid("bafy_ws10_test".to_string());
        let err = PdsError::Unreachable {
            message: "error sending request for url".to_string(),
        };

        let rendered = render_pds_error(&err, &cid);

        assert!(
            rendered.contains("PDS"),
            "expected rendered stderr to name the PDS boundary; got: {rendered}"
        );
        assert!(
            rendered.contains("retry with `openlore claim publish bafy_ws10_test`"),
            "expected actionable retry hint with the CID; got: {rendered}"
        );
    }

    /// The TLS variant takes the same retry-hint shape — the user
    /// remediation (re-run publish once the PDS is reachable) is the
    /// same across PdsError variants; only the diagnostic message
    /// portion changes.
    #[test]
    fn render_pds_error_tls_failure_also_includes_retry_hint() {
        let cid = Cid("bafy_tls".to_string());
        let err = PdsError::TlsHandshakeFailed {
            message: "certificate verify failed".to_string(),
        };

        let rendered = render_pds_error(&err, &cid);

        assert!(rendered.contains("PDS"));
        assert!(
            rendered.contains("retry with `openlore claim publish bafy_tls`"),
            "expected retry hint for TLS failure variant too; got: {rendered}"
        );
    }
}
