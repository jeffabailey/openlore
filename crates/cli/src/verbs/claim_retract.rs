//! `claim retract` — publish a counter-claim that retracts a previously
//! signed claim (ADR-008 §Behavioral rule 1 + WD-11 soft-retract).
//!
//! Slice-01 contract (WS-14 in `tests/acceptance/walking_skeleton.rs`):
//!
//! 1. Look up the original `SignedClaim` from `StoragePort::read_signed_claim`.
//!    Missing CID → friendly error pointing at `openlore claim publish`.
//! 2. Construct a fresh `UnsignedClaim` whose body mirrors the original
//!    (subject + predicate + object) — the retraction is a meta-claim
//!    *about* the original — with:
//!      - `confidence = 1.0` (you ARE certain about retracting);
//!      - `evidence = []` (the body of the retraction is the
//!        `references[]` pointer, not an external citation — the
//!        original is its own evidence);
//!      - `references = [{ ref_type: Retracts, cid: <original_cid> }]`
//!        per ADR-008 §Adapter implications;
//!      - `composed_at` from `ClockPort::now_utc()` so the retraction
//!        gets its own timestamp distinct from the original;
//!      - `author_did` from `IdentityPort::author_did()`.
//! 3. Run `reference_rules_validate` against a `StoragePort`-backed
//!    `ClaimLookup` adapter — this catches the self-reference + 2-hop
//!    cycle cases (ADR-008 Earned Trust 2/3) at sign-time.
//! 4. Canonicalize → `compute_cid` → sign via `IdentityPort::sign` →
//!    `StoragePort::write_signed_claim`. The on-disk JSON for the new
//!    retract claim is the artefact `assert_claim_references_retract`
//!    asserts against.
//! 5. Publish via the shared `claim_publish::publish_signed_claim`
//!    helper — the single publish code path per ADR-003. The success
//!    block (at-uri + retract-hint) is the same WD-6 contract the
//!    chained Y branch + standalone `claim publish <cid>` emit.
//!
//! ## WD-11 invariant — soft-retract only
//!
//! The original `SignedClaim` is NEVER touched by this verb. The
//! retraction is purely additive: a new signed artefact joins the local
//! store + the PDS, pointing at the original. Reading the original back
//! through `StoragePort::read_signed_claim` after a retract still
//! returns `Some(original)` — slice-04 graph-query annotates it as
//! "retracted by author" by walking `query_referencing`.

use anyhow::{anyhow, Context, Result};
use claim_domain::{
    canonicalize, compute_cid, reference_rules_validate, Cid, ClaimLookup, ClaimReference,
    ReferenceType, SignedClaim, UnsignedClaim,
};
use ports::StoragePort;

use crate::wiring::Wiring;

/// Argument struct for the `claim retract` verb.
#[derive(Debug, Clone)]
pub struct ClaimRetractArgs {
    /// CID of the original signed claim to retract. Must exist in local storage.
    pub cid: String,
}

/// Outcome of one `claim retract` invocation.
pub struct ClaimRetractOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `claim retract` verb. Reads the original, builds + signs +
/// persists the counter-claim, then funnels through the single publish
/// code path so the rendered success block (at-uri + retract-hint) is
/// identical to the standalone publish + chained Y branch outputs.
///
/// Errors here surface via `anyhow::Error` (collapsed by the dispatcher
/// into a non-zero exit + `openlore claim retract: <msg>` on stderr).
/// WS-14 asserts the happy path only; a future sad-path scenario will
/// route PDS failures through the WS-10-style retry-hint renderer.
pub fn run(wiring: &Wiring, args: &ClaimRetractArgs) -> Result<ClaimRetractOutcome> {
    let original_cid = Cid(args.cid.clone());

    // Step 1: look up the original. Missing CID is a clean, actionable
    // error — the user typed an unknown CID or hasn't published yet.
    let original = wiring
        .storage
        .read_signed_claim(&original_cid)
        .with_context(|| format!("looking up original claim for cid {}", args.cid))?
        .ok_or_else(|| {
            anyhow!(
                "no signed claim with cid {} in local storage. \
                 Run `openlore claim publish <cid>` first, or check the CID.",
                args.cid
            )
        })?;

    // Step 2: build the unsigned counter-claim. Subject + predicate +
    // object come from the original (the retraction is a meta-claim
    // ABOUT the original assertion). Confidence = 1.0 — you ARE
    // certain about the act of retraction. Fresh composedAt from the
    // clock port so the retraction gets its own canonical-CBOR
    // pre-image distinct from the original.
    let confidence: claim_domain::Confidence = serde_json::from_value(serde_json::json!(1.0))
        .map_err(|e| anyhow!("encoding confidence 1.0 for retraction: {e}"))?;

    let unsigned = UnsignedClaim {
        subject: original.unsigned.subject.clone(),
        predicate: original.unsigned.predicate.clone(),
        object: original.unsigned.object.clone(),
        // Empty evidence — the retraction's body is the `references[]`
        // pointer at the original. The original is its own evidence;
        // no external citation is needed at the slice-01 scope.
        evidence: Vec::new(),
        confidence,
        author_did: wiring.identity.author_did().clone(),
        composed_at: wiring.clock.now_utc().to_rfc3339(),
        references: vec![ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid: original_cid.clone(),
        }],
        // A retraction is not a counter-claim — no reason text.
        reason: None,
    };

    // Step 3: reference-rules validation. Backed by a small adapter
    // bridging `StoragePort` → `ClaimLookup` so the 2-hop cycle arm
    // can fetch any referenced claim B from local storage. The
    // self-reference arm catches the degenerate case where a user
    // somehow points a retraction at its own body CID; the cycle arm
    // catches A→B→A loops introduced by earlier retractions.
    let lookup = StorageClaimLookup {
        storage: wiring.storage.as_ref(),
    };
    reference_rules_validate(&unsigned, Some(&lookup as &dyn ClaimLookup))
        .map_err(|e| anyhow!("reference rules rejected the retraction: {e}"))?;

    // Step 4: canonicalize → CID → sign → persist. Same shape as
    // `claim_add::run` so a future refactor that promotes this to a
    // shared helper is straightforward.
    let canonical_bytes =
        canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing retraction: {e}"))?;
    let unsigned_cid = compute_cid(&canonical_bytes);

    // Emit the `Computing claim CID <cid>` marker — WS-14 reuses the
    // WS-6/WS-7 parser to extract the new CID from stdout.
    println!("Computing claim CID {}", unsigned_cid.0);

    let signature = wiring
        .identity
        .sign(&unsigned_cid)
        .map_err(|e| anyhow!("signing retraction: {e}"))?;

    let signed = SignedClaim {
        unsigned,
        signature,
    };

    wiring
        .storage
        .write_signed_claim(&signed)
        .with_context(|| {
            format!(
                "persisting retraction claim {} to local store",
                signed.signature.signed_cid.0
            )
        })?;

    let artifact_path = wiring
        .paths
        .claims_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));
    println!("Written to local store: {}", artifact_path.display());

    // Step 5: publish via the single publish code path (ADR-003). Same
    // renderer the standalone `claim publish` + chained Y branch use,
    // so the at-uri + retract-hint substrings WS-14 asserts on are
    // produced by exactly the same code that produces them in WS-8.
    let publish_outcome = crate::verbs::claim_publish::publish_signed_claim(wiring, &signed)
        .map_err(|e| anyhow!("{}", e))?;
    let rendered = crate::verbs::claim_publish::render_publish_success(&publish_outcome);
    print!("{}", rendered);

    Ok(ClaimRetractOutcome {
        exit_code: 0,
        stdout: String::new(),
    })
}

/// Tiny adapter bridging `&dyn StoragePort` → `ClaimLookup` for the
/// reference-rules validator. Kept local to the verb because the cycle
/// check is the only caller in slice-01; promoting it to a shared
/// adapter crate would be premature.
struct StorageClaimLookup<'a> {
    storage: &'a dyn StoragePort,
}

impl<'a> ClaimLookup for StorageClaimLookup<'a> {
    fn signed_by_cid(&self, cid: &Cid) -> Option<SignedClaim> {
        // The cycle-check arm tolerates `None` for "store doesn't know
        // this CID" — slice-01 intentionally does not promote that to
        // a hard error (a slice-04 stricter "dangling reference"
        // check may revisit this). Therefore a `read_signed_claim`
        // error here collapses to `None`, matching the contract the
        // validator already expects.
        self.storage.read_signed_claim(cid).ok().flatten()
    }
}
