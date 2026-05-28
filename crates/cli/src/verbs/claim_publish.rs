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
use ports::PdsError;

use crate::errors::render_pds_error;
use crate::wiring::Wiring;

/// Typed failure modes for `publish_signed_claim`. Carrying the
/// underlying `PdsError` (rather than collapsing into `anyhow::Error`)
/// preserves the WS-10 universe slot `cli.stderr.pds_unreachable_retry_hint`
/// — the stderr renderer in `crate::errors` needs the original variant +
/// the CID to produce the actionable retry verb. Other failure classes
/// (serialization, local persistence) collapse into `anyhow::Error`
/// because their remediation is not the "retry publish" verb — they
/// indicate a deeper bug or filesystem problem.
#[derive(Debug)]
pub enum PublishError {
    /// The PDS adapter refused the publish. The carried `PdsError`
    /// classifies the network/TLS/rejection root cause; the renderer in
    /// `crate::errors::render_pds_error` shapes the user-facing line
    /// AND the actionable retry hint with the CID baked in. Per US-003
    /// + KPI-5 the local `<cid>.json` artefact remains intact whenever
    /// this variant is returned — the verb does NOT roll back the
    /// preceding local write on a publish-side failure.
    PdsFailed { source: PdsError, cid: Cid },
    /// Anything else (serializing the SignedClaim body, recording
    /// publication metadata, etc.) that prevented publish from
    /// completing. Surfaced via anyhow because the remediation differs
    /// per-error and is not the WS-10 "retry publish" hint.
    Other(anyhow::Error),
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublishError::PdsFailed { source, cid } => {
                write!(f, "PDS publish failed for {}: {source}", cid.0)
            }
            PublishError::Other(err) => write!(f, "{err:#}"),
        }
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PublishError::PdsFailed { source, .. } => Some(source),
            PublishError::Other(_) => None,
        }
    }
}

/// Render a `PublishError` into the stderr block both call sites
/// (standalone verb dispatcher + chained Y branch in claim_add) emit.
/// Pds failures route through the typed renderer in `crate::errors` so
/// the WS-10 retry hint shape stays consistent; other errors fall back
/// to anyhow's chained-cause format.
pub fn render_publish_error(err: &PublishError) -> String {
    match err {
        PublishError::PdsFailed { source, cid } => render_pds_error(source, cid),
        PublishError::Other(other) => format!("openlore claim publish: {other:#}\n"),
    }
}

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
/// success block to stdout. Returns a typed `PublishError` on failure so
/// the dispatcher can route PDS-unreachable failures (WS-10) through
/// the actionable-retry-hint renderer in `crate::errors`.
pub fn run(
    wiring: &Wiring,
    args: &ClaimPublishArgs,
) -> std::result::Result<ClaimPublishOutcome, PublishError> {
    let cid = Cid(args.cid.clone());
    let signed = wiring
        .storage
        .read_signed_claim(&cid)
        .with_context(|| format!("looking up signed claim for cid {}", args.cid))
        .map_err(PublishError::Other)?
        .ok_or_else(|| {
            PublishError::Other(anyhow!(
                "no signed claim with cid {} in local storage. Run `openlore claim add` first.",
                args.cid
            ))
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
/// renderer pins. On PDS failure returns
/// `PublishError::PdsFailed { source, cid }` so the caller can route
/// it through `crate::errors::render_pds_error` for the WS-10 retry
/// hint. Other failure modes (body serialization, persistence) map to
/// `PublishError::Other` because their remediation is not "retry
/// publish" — the user can't make progress by re-running the verb.
pub fn publish_signed_claim(
    wiring: &Wiring,
    signed: &SignedClaim,
) -> std::result::Result<PublishOutcome, PublishError> {
    let cid = signed.signature.signed_cid.clone();
    let cid_str = cid.0.clone();

    // The over-the-wire body is the SignedClaim itself serialized as
    // JSON. The PDS doesn't care about the canonical-CBOR encoding —
    // that is the local artifact contract. ATProto records are JSON
    // shaped per the `org.openlore.claim` Lexicon.
    let body = serde_json::to_value(signed)
        .with_context(|| format!("serializing SignedClaim {cid_str} for PDS body"))
        .map_err(PublishError::Other)?;

    let collection = "org.openlore.claim";

    // PdsPort is async; the cli wires a small tokio runtime here.
    // We use `current_thread` because publish is sequential and we
    // want to keep the binary's runtime footprint minimal.
    let runtime = build_tokio_runtime();
    // Preserve the typed `PdsError` so the dispatcher can render the
    // WS-10 actionable-retry-hint via `crate::errors::render_pds_error`.
    // Collapsing into `anyhow::Error` here would erase the variant the
    // renderer needs (and the cid binding it pairs with).
    let create_outcome = runtime
        .block_on(wiring.pds.create_record(collection, &cid_str, body))
        .map_err(|source| PublishError::PdsFailed {
            source,
            cid: cid.clone(),
        })?;

    // Step 05-09: branch the rendered success message on the port's
    // `was_idempotent` bit. The PdsPort lifts the 409/RecordAlreadyExists
    // path into a normal success carrying `was_idempotent = true`
    // (architecture §6.2). `record_publication` is itself an UPDATE so
    // re-recording the same (cid, at_uri, published_at) is safe and
    // keeps the local index consistent across re-publishes.
    let published_at = wiring.clock.now_utc();
    wiring
        .storage
        .record_publication(
            &signed.signature.signed_cid,
            &create_outcome.at_uri.0,
            published_at,
        )
        .with_context(|| {
            format!(
                "recording publication metadata for {cid_str} at {}",
                create_outcome.at_uri.0
            )
        })
        .map_err(PublishError::Other)?;

    Ok(PublishOutcome {
        cid: cid_str,
        at_uri: create_outcome.at_uri.0,
        already_present: create_outcome.was_idempotent,
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
        out.push_str(&format!("Claim {} already published.\n", outcome.cid));
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
