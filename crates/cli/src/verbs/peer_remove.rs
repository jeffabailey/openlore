//! `peer remove <did> [--purge]` — unsubscribe from a peer (slice-03;
//! US-FED-005 / PS-5..PS-8).
//!
//! Two modes, two distinct DuckDB transactions (ADR-014):
//!
//! - **default (soft)**: `PeerStoragePort::soft_remove` sets
//!   `peer_subscriptions.removed_at` and RETAINS every cached peer_claims
//!   row (they re-annotate as "(unsubscribed cache)" in
//!   `graph query --federated`).
//! - **`--purge` (hard)**: prompts for interactive `[y/N]` confirmation,
//!   then `PeerStoragePort::hard_purge` deletes the subscription row, the
//!   peer's peer_claims rows, and the `peer_claims/<did>/` directory.
//!   User counter-claims (in the author table) are PRESERVED.
//!
//! ## Autoconfirm test hatch (D-D20 / WD-21)
//!
//! WD-21 forbids a `--yes` flag in production. The acceptance test
//! `at-peer-remove-purge-zero-residue` cannot answer an interactive
//! prompt in CI, so a BUILD-TIME-gated escape hatch
//! ([`autoconfirm_purge`]) lets the test bypass the prompt via the
//! `OPENLORE_TEST_AUTOCONFIRM=1` env var. The hatch is compiled in ONLY
//! under `#[cfg(any(test, feature = "test-autoconfirm"))]`; in a release
//! build it collapses to a `const false` with NO env-var read, so there
//! is provably no auto-confirm path in shipped binaries. `cargo xtask`
//! (step 01-06) greps for this gate; keep the cfg expression and the
//! `OPENLORE_TEST_AUTOCONFIRM` token consistent.
//!
//! SCAFFOLD: true (slice-03) — the verb body is a `todo!()` stub; the
//! autoconfirm guard itself is LIVE so step 01-06's xtask can verify it.

use anyhow::Result;

use crate::wiring::Wiring;

/// Argument struct for the `peer remove` verb (mirrors the clap
/// subcommand). `purge = true` routes to the hard-purge branch; `false`
/// to soft-remove.
#[derive(Debug, Clone)]
pub struct PeerRemoveArgs {
    /// The peer DID to unsubscribe from.
    pub did: String,
    /// `--purge`: hard-delete cached peer claims (gated by interactive
    /// confirmation). Defaults to false (soft-remove).
    pub purge: bool,
}

/// Outcome of one `peer remove` invocation — exit code + stdout chunk.
pub struct PeerRemoveOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `peer remove` verb.
///
/// SCAFFOLD: true (slice-03) — `todo!()` stub at this bootstrap step.
/// The PS-5..PS-8 acceptance scenarios drive the real implementation
/// (soft branch → soft_remove; --purge branch → confirm-or-[`autoconfirm_purge`]
/// → hard_purge; --no-tty refuses the --purge branch) in a later slice-03
/// phase.
pub fn run(_wiring: &Wiring, _args: &PeerRemoveArgs) -> Result<PeerRemoveOutcome> {
    // SCAFFOLD: true (slice-03)
    todo!(
        "VerbPeerRemove — soft branch → PeerStoragePort::soft_remove; \
         --purge branch → TtyIO confirm OR autoconfirm_purge() → hard_purge; \
         --no-tty refuses --purge (WD-36). Driven by PS-5..PS-8 scenarios."
    )
}

/// Build-time-gated test escape hatch for the `--purge` interactive
/// confirmation (D-D20 / WD-21).
///
/// Returns `true` (auto-confirm the purge) ONLY when BOTH hold:
/// 1. the binary was built with `cfg(test)` OR the `test-autoconfirm`
///    feature, AND
/// 2. the `OPENLORE_TEST_AUTOCONFIRM` env var is set to `1`.
///
/// In a release build (neither cfg active) the function below is compiled
/// out and the [`release-build variant`](autoconfirm_purge) is used
/// instead, which is a `const false` with no env-var read — so a shipped
/// binary has provably no auto-confirm path. WD-21 (no production `--yes`)
/// is satisfied by construction.
#[cfg(any(test, feature = "test-autoconfirm"))]
pub fn autoconfirm_purge() -> bool {
    std::env::var("OPENLORE_TEST_AUTOCONFIRM")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Release-build variant of [`autoconfirm_purge`]: there is NO auto-
/// confirm path in shipped binaries. Compiles to a constant `false`; the
/// `OPENLORE_TEST_AUTOCONFIRM` env var is never read (D-D20). The
/// `--purge` confirmation can ONLY be satisfied by an interactive `[y/N]`
/// answer in a release build.
#[cfg(not(any(test, feature = "test-autoconfirm")))]
pub fn autoconfirm_purge() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The autoconfirm hatch is gated on the `OPENLORE_TEST_AUTOCONFIRM`
    /// env var even under test cfg: absent ⇒ false. This documents that
    /// merely building with the test cfg does NOT auto-confirm — the env
    /// var is the explicit opt-in.
    ///
    /// Run serially-safe by saving/restoring the env var; the default
    /// (var unset) path is the one we assert.
    #[test]
    fn autoconfirm_is_false_without_the_env_var() {
        // SAFETY: single-threaded test access; we restore after.
        let saved = std::env::var("OPENLORE_TEST_AUTOCONFIRM").ok();
        std::env::remove_var("OPENLORE_TEST_AUTOCONFIRM");
        assert!(
            !autoconfirm_purge(),
            "autoconfirm must be false when OPENLORE_TEST_AUTOCONFIRM is unset"
        );
        if let Some(v) = saved {
            std::env::set_var("OPENLORE_TEST_AUTOCONFIRM", v);
        }
    }
}
