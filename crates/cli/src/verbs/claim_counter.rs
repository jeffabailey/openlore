//! `claim counter <target_cid> --reason "..."` — author a counter-claim
//! against another claim (slice-03; US-FED / ADR-015).
//!
//! A counter-claim is the user's OWN signed claim that carries a
//! `references[]` entry of type `counters` pointing at `target_cid`, plus
//! a mandatory free-text `reason`. It NEVER overwrites the target —
//! counter-claims coexist with the claims they counter (the "never
//! overwrite" UX contract; compose preview literal text).
//!
//! ## Single-publish-path (I-FED-5 / WD-22)
//!
//! The publish step of `claim counter` MUST reuse `VerbClaimPublish`
//! internals — there is NO parallel publish code path (preserves ADR-003
//! single-publish-path). This bootstrap step (01-04) declares the handler
//! ONLY; it does NOT fork publish logic. The live wiring through
//! `claim_domain::validate_counter_claim` → compose preview → sign →
//! `claim_publish` internals lands with the CC-* acceptance scenarios in
//! a later slice-03 phase.
//!
//! SCAFFOLD: true (slice-03)

use anyhow::Result;

use crate::wiring::Wiring;

/// Argument struct for the `claim counter` verb (mirrors the clap
/// subcommand). `reason` is REQUIRED at the CLI level (WD-20); clap
/// rejects the invocation if absent, so this field is non-optional.
#[derive(Debug, Clone)]
pub struct ClaimCounterArgs {
    /// CID of the claim being countered (the user's own OR a peer's).
    pub cid: String,
    /// Mandatory free-text explanation, NFC-normalized at compose time.
    pub reason: String,
}

/// Outcome of one `claim counter` invocation — exit code + stdout chunk,
/// uniform with the other verbs so the dispatcher routes identically.
pub struct ClaimCounterOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `claim counter` verb.
///
/// SCAFFOLD: true (slice-03) — the body is a `todo!()` stub at this
/// bootstrap step. The CC-* acceptance scenarios drive the real
/// implementation (validate_counter_claim → compose preview → sign →
/// VerbClaimPublish internals) in a later slice-03 phase.
pub fn run(_wiring: &Wiring, _args: &ClaimCounterArgs) -> Result<ClaimCounterOutcome> {
    // SCAFFOLD: true (slice-03)
    todo!(
        "VerbClaimCounter — wire claim_domain::normalize_reason + \
         validate_counter_claim → compose preview → sign → VerbClaimPublish \
         internals (I-FED-5 single-publish-path). Driven by CC-* scenarios."
    )
}
