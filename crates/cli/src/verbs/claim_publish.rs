//! `claim publish` — publish a previously-signed claim to the configured PDS.
//!
//! Step 05-08 (WS-8 + WS-9 precondition): the single publish code path
//! per ADR-003. Invoked two ways:
//!
//! 1. Standalone verb: `openlore claim publish <cid>`. Reads the
//!    `SignedClaim` from local DuckDB via `StoragePort::read_signed_claim`,
//!    serializes it to JSON (the over-the-wire body), posts it to the
//!    configured PDS via `PdsPort::create_record("org.openlore.claim",
//!    <cid>, <body>)`, records publication metadata via
//!    `StoragePort::record_publication`, and prints the success block
//!    (at-uri + retract-hint).
//!
//! 2. Chained from `claim add`'s Y branch: same publish helper is called
//!    with the just-signed claim's CID. ADR-003's "single publish code
//!    path" mandates this — there is exactly one place in the codebase
//!    that knows how to publish, regardless of how the user got there.
//!
//! ## Slice-01 contract (WS-8)
//!
//! Post-publish stdout MUST contain:
//! - `at-uri: at://<author_did>/org.openlore.claim/<cid>` (FR-2: rkey =
//!   CID; FR-3: at-uri reconstructible from DID + CID)
//! - `Run \`openlore claim retract <cid>\` to retract this claim.` —
//!   the WD-6 retract-hint UX moment.
//!
//! ## Idempotency (WS-9, deferred to a later step but the shape lands here)
//!
//! When the PDS responds with a 409 conflict (rkey already exists), the
//! `AtProtoPdsAdapter` returns the synthesized at-uri of the existing
//! record without error. We surface this to stdout with an
//! `already published` annotation so users running `claim publish` twice
//! see a clean, non-alarming message — the lifecycle is "the claim is
//! at this at-uri", not "we just wrote it".

use anyhow::{anyhow, Context, Result};
use claim_domain::{Cid, SignedClaim};

use crate::wiring::Wiring;

/// Argument struct for the `claim publish` verb.
#[derive(Debug, Clone)]
pub struct ClaimPublishArgs {
    /// The CID of a previously-signed claim. Must exist in local storage.
    pub cid: String,
}

/// Outcome of one `claim publish` invocation.
pub struct ClaimPublishOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Pure-data result of one publish attempt. Built by [`run`] and
/// rendered by [`render_publish_success`] so the rendering function can
/// be unit-tested without spawning a runtime.
#[derive(Debug, Clone)]
pub struct PublishOutcome {
    pub cid: String,
    pub at_uri: String,
    /// True when the publish was idempotent (the PDS already had the
    /// record). False when this invocation actually inserted it.
    pub already_present: bool,
}

/// Run the `claim publish` verb. Looks up the signed claim, posts it to
/// the PDS, records publication metadata locally, and renders the
/// success block to stdout. Returns non-zero exit code on hard errors
/// (lookup miss, PDS unreachable, persistence failure) per the
/// composition root's `i32` contract.
pub fn run(wiring: &Wiring, args: &ClaimPublishArgs) -> Result<ClaimPublishOutcome> {
    let cid = Cid(args.cid.clone());
    let signed = wiring
        .storage
        .read_signed_claim(&cid)
        .with_context(|| format!("looking up signed claim for cid {}", args.cid))?
        .ok_or_else(|| {
            anyhow!(
                "no signed claim with cid {} in local storage. Run `openlore claim add` first.",
                args.cid
            )
        })?;

    let outcome = publish_signed_claim(wiring, &signed)?;
    let rendered = render_publish_success(&outcome);

    Ok(ClaimPublishOutcome {
        exit_code: 0,
        stdout: rendered,
    })
}

/// Publish a `SignedClaim` to the configured PDS and record publication
/// metadata. The single publish code path per ADR-003 — both the
/// standalone verb and the `claim add` Y branch funnel through here.
///
/// On success returns a [`PublishOutcome`] carrying the at-uri the
/// renderer pins. On failure surfaces an `anyhow::Error` carrying a
/// retry hint that names the standalone `claim publish` verb — the
/// composition root prints this verbatim to stderr per WS-10.
pub fn publish_signed_claim(wiring: &Wiring, signed: &SignedClaim) -> Result<PublishOutcome> {
    let cid_str = signed.signature.signed_cid.0.clone();

    // The over-the-wire body is the SignedClaim itself serialized as
    // JSON. The PDS doesn't care about the canonical-CBOR encoding —
    // that is the local artifact contract. ATProto records are JSON
    // shaped per the `org.openlore.claim` Lexicon.
    let body = serde_json::to_value(signed)
        .with_context(|| format!("serializing SignedClaim {cid_str} for PDS body"))?;

    let collection = "org.openlore.claim";

    // PdsPort is async; the cli wires a small tokio runtime here.
    // We use `current_thread` because publish is sequential and we
    // want to keep the binary's runtime footprint minimal.
    let runtime = build_tokio_runtime();
    let at_uri = runtime
        .block_on(wiring.pds.create_record(collection, &cid_str, body))
        .with_context(|| {
            format!(
                "publishing claim {cid_str} to PDS. \
                 Retry with `openlore claim publish {cid_str}` once the PDS is reachable."
            )
        })?;

    let published_at = wiring.clock.now_utc();
    wiring
        .storage
        .record_publication(&signed.signature.signed_cid, &at_uri.0, published_at)
        .with_context(|| {
            format!("recording publication metadata for {cid_str} at {}", at_uri.0)
        })?;

    Ok(PublishOutcome {
        cid: cid_str,
        at_uri: at_uri.0,
        // Slice-01: the PdsPort doesn't expose "was-idempotent" — both
        // first-publish and 409-retry return the same Ok(AtUri). WS-9
        // covers the idempotent-retry path; until that step lands the
        // standalone verb always renders the fresh-publish message.
        already_present: false,
    })
}

/// Pure function: render the post-publish success block.
///
/// Two load-bearing substrings per WS-8:
/// 1. `at-uri: <at://did:plc:.../org.openlore.claim/<cid>>` — the
///    canonical AT URI of the published record.
/// 2. `openlore claim retract <cid>` — the retract-hint UX moment from
///    WD-6 that closes the loop: the user just published, here's how
///    to take it back if they change their mind.
pub fn render_publish_success(outcome: &PublishOutcome) -> String {
    let mut out = String::new();
    if outcome.already_present {
        out.push_str(&format!(
            "Claim {} already published.\n",
            outcome.cid
        ));
    } else {
        out.push_str(&format!("Published claim {}.\n", outcome.cid));
    }
    out.push_str(&format!("  at-uri: {}\n", outcome.at_uri));
    out.push_str(&format!(
        "  Run `openlore claim retract {}` to retract this claim.\n",
        outcome.cid
    ));
    out
}

/// Build the tokio runtime the cli uses to bridge the sync verb
/// dispatcher into the async `PdsPort` contract. `current_thread`
/// because publish is sequential and we don't want a worker-thread
/// pool for a single XRPC POST. Centralized so unit tests + the
/// `claim add` Y branch use the same shape.
pub(crate) fn build_tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("build current_thread runtime for cli publish path")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WS-8 hard AC: the success block contains both load-bearing
    /// substrings — the at-uri and the retract-command hint.
    #[test]
    fn render_publish_success_contains_at_uri_and_retract_hint() {
        let outcome = PublishOutcome {
            cid: "bafy_test_001".to_string(),
            at_uri: "at://did:plc:test-jeff/org.openlore.claim/bafy_test_001".to_string(),
            already_present: false,
        };
        let rendered = render_publish_success(&outcome);
        assert!(
            rendered.contains("at-uri: at://did:plc:test-jeff/org.openlore.claim/bafy_test_001"),
            "expected at-uri line in success block; got:\n{rendered}"
        );
        assert!(
            rendered.contains("openlore claim retract bafy_test_001"),
            "expected retract-command hint in success block (WD-6); got:\n{rendered}"
        );
    }

    /// WS-9 precondition: the idempotent-retry render is non-alarming —
    /// it announces "already published" rather than acting like a fresh
    /// publish. Users running `claim publish <cid>` twice see a clean
    /// message.
    #[test]
    fn render_publish_success_announces_already_published_on_idempotent_retry() {
        let outcome = PublishOutcome {
            cid: "bafy_test_002".to_string(),
            at_uri: "at://did:plc:test-jeff/org.openlore.claim/bafy_test_002".to_string(),
            already_present: true,
        };
        let rendered = render_publish_success(&outcome);
        assert!(
            rendered.contains("already published"),
            "expected 'already published' annotation for idempotent retry; got:\n{rendered}"
        );
        assert!(
            rendered.contains("at-uri: at://did:plc:test-jeff/org.openlore.claim/bafy_test_002"),
            "expected at-uri line on idempotent retry too; got:\n{rendered}"
        );
    }
}
