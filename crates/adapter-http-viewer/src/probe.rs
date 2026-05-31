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
//!
//! ## Plain-language refusal message — a PURE function
//!
//! The store-unreadable refusal text is constructed by the PURE
//! [`viewer_store_unreadable_message`] function (no I/O, no transport internals).
//! It is the SAME message whether the store could not be OPENED at all (the
//! composition root's `DuckDbStorageAdapter::open` failed because another process
//! holds the file lock) or the open succeeded but the sentinel `count_claims()`
//! read failed. Both surfaces share one operator-facing sentence so the refusal
//! is consistent — naming the store PATH and asking if "another process" holds
//! it, and DELIBERATELY omitting the raw DuckDB / IO error string (no
//! "IO Error", no lock-file dump, no stack trace — NFR-VIEW-6). Pinning the
//! construction here (unit-tested below) makes it a mutation target independent
//! of the effectful open/probe wiring.

use std::net::SocketAddr;

use ports::{ProbeOutcome, ProbeRefusalReason, StoreReadPort};

/// Construct the PLAIN-LANGUAGE store-unreadable refusal message (NFR-VIEW-6).
///
/// Names the `store_path` so the operator knows WHICH file is affected, and asks
/// whether ANOTHER PROCESS is using it (the US-VIEW-001 Example 3 phrasing — a
/// DuckDB write lock held by the CLI or another tool is the common cause).
///
/// Pure + total: same input → same output, no I/O. It NEVER embeds the raw
/// transport error (the DuckDB "IO Error: Could not set lock ..." string, file
/// descriptors, or a stack trace) — the operator gets a calm, actionable
/// sentence, not an exception dump. The effect shell logs the raw cause to the
/// structured `health.startup.refused` event for DevOps; the human line stays
/// clean.
pub fn viewer_store_unreadable_message(store_path: &str) -> String {
    format!(
        "could not read your store at {store_path} — is another process using it? \
         Close any other `openlore` command writing to that store and try again."
    )
}

/// Run the viewer's startup probe over the read-only `store` + the bound
/// `local_addr`, given the resolved `store_path` (used to NAME the store in a
/// store-readable refusal — NFR-VIEW-6). Returns [`ProbeOutcome::Ok`] only when
/// the store reads, the port is read-only (structural), and the bind is loopback.
pub fn run_probe(
    store: &dyn StoreReadPort,
    local_addr: &SocketAddr,
    store_path: &str,
) -> ProbeOutcome {
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
    // store surfaces here as a clean refusal, not a per-request crash. The
    // operator-facing detail is the PURE plain-language message (names the path,
    // asks about another process, hides the raw transport error); the raw cause
    // is preserved ONLY in the structured payload for DevOps.
    match store.count_claims() {
        Ok(_) => ProbeOutcome::Ok,
        Err(err) => viewer_store_unreadable_refusal(store_path, &err.to_string()),
    }
    // Check 2 (read-only capability) is STRUCTURAL: `store: &dyn StoreReadPort`
    // exposes no mutation method, so no runtime assertion is needed — the type
    // system already proves it (I-VIEW-1). The probe re-states this contract in
    // its doc comment so the Earned-Trust audit trail records it.
}

/// Build the `ProbeOutcome::Refused` for a store-unreadable failure. Shared by
/// the probe's `count_claims()` failure path AND the composition root's
/// store-OPEN failure path (`adapter-http-viewer` re-exports this so the `cli`
/// `ui` verb renders the SAME refusal when `DuckDbStorageAdapter::open` itself
/// fails on a held lock — that failure happens before the server can be built,
/// so it cannot flow through `run_probe`). `raw_cause` is preserved ONLY in the
/// structured payload; the operator-facing `detail` is the pure message.
pub fn viewer_store_unreadable_refusal(store_path: &str, raw_cause: &str) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason: ProbeRefusalReason::ViewerStoreUnreadable,
        detail: viewer_store_unreadable_message(store_path),
        structured: serde_json::json!({
            "contract": "viewer_store_readable",
            "store_path": store_path,
            "error": raw_cause,
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Pin the PURE plain-language refusal construction (the mutation target).
    //! These are domain-language unit tests at the pure-function boundary — the
    //! function signature IS the port (port-to-port at domain scope).

    use super::*;

    /// The message NAMES the store path (AC-001.4 / NFR-VIEW-6: the operator must
    /// know WHICH file is affected), mentions the word "store", and asks if
    /// "another process" is using it (US-VIEW-001 Example 3 phrasing).
    #[test]
    fn refusal_message_names_the_path_and_another_process() {
        let path = "/home/maria/.local/share/openlore/openlore.duckdb";
        let message = viewer_store_unreadable_message(path);

        assert!(
            message.contains(path),
            "the refusal must name the store path so the operator knows WHICH file \
             is affected; got: {message}"
        );
        assert!(
            message.contains("store"),
            "the refusal must use the domain word \"store\"; got: {message}"
        );
        assert!(
            message.contains("another process"),
            "the refusal must ask whether another process holds the store \
             (US-VIEW-001 Ex 3); got: {message}"
        );
    }

    /// The message is PLAIN language — it must NOT leak the raw transport error
    /// (the DuckDB "IO Error" / lock-file dump) NOR any stack-trace marker
    /// (NFR-VIEW-6). The raw cause is carried separately in the structured event,
    /// never in the operator-facing sentence.
    #[test]
    fn refusal_message_hides_transport_internals_and_stack_traces() {
        let message = viewer_store_unreadable_message("/var/lib/openlore/openlore.duckdb");

        for leaked in [
            "IO Error",
            "Conflicting lock",
            "panicked at",
            "RUST_BACKTRACE",
            "stack backtrace",
            "StoreReadError",
            "thread '",
        ] {
            assert!(
                !message.contains(leaked),
                "the plain-language refusal must NOT leak {leaked:?} \
                 (NFR-VIEW-6); got: {message}"
            );
        }
    }

    /// The refusal OUTCOME carries the pure message as its operator-facing
    /// `detail`, classifies as `ViewerStoreUnreadable`, and preserves the raw
    /// cause ONLY in the structured payload (DevOps observability) — never in the
    /// operator `detail`. This pins the open-failure / probe-failure SHARED fork.
    #[test]
    fn refusal_outcome_keeps_raw_cause_out_of_operator_detail() {
        let path = "/srv/openlore/openlore.duckdb";
        let raw = "IO Error: Could not set lock on file: Conflicting lock is held";
        let outcome = viewer_store_unreadable_refusal(path, raw);

        match outcome {
            ProbeOutcome::Refused {
                reason,
                detail,
                structured,
            } => {
                assert_eq!(reason, ProbeRefusalReason::ViewerStoreUnreadable);
                assert_eq!(detail, viewer_store_unreadable_message(path));
                assert!(
                    !detail.contains("IO Error") && !detail.contains("Conflicting lock"),
                    "the operator detail must stay plain-language; got: {detail}"
                );
                assert_eq!(
                    structured["error"], raw,
                    "the raw cause must be preserved in the structured payload for DevOps"
                );
                assert_eq!(structured["store_path"], path);
            }
            ProbeOutcome::Ok => panic!("a store-unreadable failure must be Refused, not Ok"),
        }
    }
}
