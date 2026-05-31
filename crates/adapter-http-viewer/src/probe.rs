//! `probe` — the viewer server's Earned-Trust startup probe (ADR-030).
//!
//! Real (non-stub) — NOT on the bootstrap allowlist. The composition root runs
//! it BEFORE the serve loop and refuses to serve on any refusal. Three load-
//! bearing checks:
//!
//! 1. **Store-readable** — a sentinel `count_claims()` read succeeds. A locked /
//!    missing store surfaces here as a plain-language startup refusal naming the
//!    store (NFR-VIEW-6), NOT a per-request crash (ADR-030 §Earned-Trust step 1).
//! 2. **Read-only capability** — the store is held behind `StoreReadPort`, whose
//!    trait surface exposes NO write/sign method (I-VIEW-1). This is structural
//!    (a type-level guarantee the probe re-states), so the probe asserts it by
//!    construction: a `&dyn StoreReadPort` cannot mutate.
//! 3. **Loopback** — the bound address is `127.0.0.1` (I-VIEW-4); a non-loopback
//!    bind is refused (defense-in-depth on top of [`super::ViewerServer::bind`]'s
//!    own loopback guard).

use std::net::SocketAddr;

use ports::{ProbeOutcome, ProbeRefusalReason, StoreReadPort};

/// Run the viewer's startup probe over the read-only `store` + the bound
/// `local_addr`. Returns [`ProbeOutcome::Ok`] only when the store reads, the port
/// is read-only (structural), and the bind is loopback.
pub fn run_probe(store: &dyn StoreReadPort, local_addr: &SocketAddr) -> ProbeOutcome {
    // Check 3: loopback (I-VIEW-4). Checked first because it needs no I/O.
    if !local_addr.ip().is_loopback() {
        return ProbeOutcome::Refused {
            reason: ProbeRefusalReason::ViewerNotLoopback,
            detail: format!(
                "viewer bound a non-loopback address {local_addr}; the viewer is \
                 localhost-only (I-VIEW-4)"
            ),
            structured: serde_json::json!({
                "contract": "viewer_loopback_only",
                "bound_addr": local_addr.to_string(),
            }),
        };
    }

    // Check 1: store-readable (a sentinel COUNT(*) read). A locked / missing
    // store surfaces here as a clean refusal, not a per-request crash.
    match store.count_claims() {
        Ok(_) => ProbeOutcome::Ok,
        Err(err) => ProbeOutcome::Refused {
            reason: ProbeRefusalReason::ViewerStoreUnreadable,
            detail: format!(
                "the viewer could not read your store — is another process using \
                 it? ({err})"
            ),
            structured: serde_json::json!({
                "contract": "viewer_store_readable",
                "error": err.to_string(),
            }),
        },
    }
    // Check 2 (read-only capability) is STRUCTURAL: `store: &dyn StoreReadPort`
    // exposes no mutation method, so no runtime assertion is needed — the type
    // system already proves it (I-VIEW-1). The probe re-states this contract in
    // its doc comment so the Earned-Trust audit trail records it.
}
