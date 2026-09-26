//! serverless-philosophy-federation — `openlore publish init` + `publish status`.
//!
//! Slice-01: US-SF-001 (deploy + register my OWN serverless instance, J-007).
//! Registration records the user's own Cloudflare Worker URL as the configured
//! publish target (the ADR-027 configurable URL, D-5) WITHOUT changing the
//! signing identity or the local store (D-6/D-7). The `wrangler deploy` half is
//! user-run Cloudflare tooling; these tests drive the CLI registration only.
//!
//! Only the external CLI↔Worker seam is faked (`FakeInstance`); the local store
//! + identity are REAL. Layer 3 (subprocess/FS acceptance); example-only.
//!
//! Q-SF-D5 (opaque-instance detection marker) is RESOLVED here and bound by
//! PI-4: `publish init` probes `GET /manifest` and requires the response to
//! carry the explicit openlore opaque-instance marker envelope (an `openlore`
//! discriminator field). A URL that is reachable but does NOT return that
//! marker is an arbitrary URL, not an openlore instance, and is REFUSED — the
//! same "wire then probe then use" gate ADR-062 §6 mandates for the publish
//! adapter's `probe()`.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-001 — happy path
// =============================================================================

/// PI-1: Maria has deployed the `atproto/` Worker to her own Cloudflare account
/// at `https://openlore.maria.workers.dev`. When she runs
/// `openlore publish init https://openlore.maria.workers.dev`, the CLI records
/// the URL as her configured publish target, prints her (unchanged) author DID
/// `did:plc:maria-test`, prints the derived `card_url`, and confirms the
/// instance is HERS with no central authority in the trust path (D-1/D-4). Her
/// local store is unchanged. (US-SF-001 · AC 1-2 · UAT #1.)
///
/// @us-sf-001 @driving_port @real-io @j-007 @happy
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-001 register-own-instance happy path)"]
fn publish_init_registers_reachable_instance_and_prints_owned_identity() {
    todo!(
        "DELIVER: Given a reachable FakeInstance::fresh() with the openlore marker manifest; \
         When `openlore publish init <url>`; Then the target is recorded, stdout names \
         instance_url + the unchanged author_did + the derived card_url + the 'no central \
         authority' confirmation, and local.claims.row_count is UNCHANGED. \
         Universe: config.publish_target, cli.stdout.identity_block, local.claims.row_count."
    );
}

// =============================================================================
// US-SF-001 — boundary / error
// =============================================================================

/// PI-2: No Worker is reachable at `https://typo.workers.dev` (Björn ran init
/// before deploying, or fat-fingered the URL). The CLI reports it cannot reach
/// the instance and does NOT register the dead URL — offline-first is not
/// speculative-register. (US-SF-001 · AC 3 · UAT #2 · Domain example 2.)
///
/// @us-sf-001 @error @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-001 unreachable URL not registered)"]
fn publish_init_refuses_to_register_an_unreachable_instance_url() {
    todo!(
        "DELIVER: Given a FakeInstance::unreachable() at the given URL; When `publish init`; \
         Then exit non-zero with an 'cannot reach instance' message and config.publish_target \
         stays UNSET (nothing recorded)."
    );
}

/// PI-3: Registering never makes the instance a signing authority. Maria's
/// signing identity is `did:plc:maria-test`. When she runs `publish init`, her
/// author DID is unchanged and the instance holds no signing key — signing
/// stays local (D-6/D-7). (US-SF-001 · AC 4 · UAT #3 · Domain example 3.)
///
/// @us-sf-001 @error @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-001 registration never changes signing identity)"]
fn publish_init_never_changes_the_signing_identity_or_local_store() {
    todo!(
        "DELIVER: Given signing identity did:plc:maria-test; When `publish init`; \
         Then identity.author_did is UNCHANGED, no signing key is sent to or held by the \
         instance, and local.store is byte-unchanged."
    );
}

/// PI-4 (Q-SF-D5 — opaque-instance detection marker): A URL is reachable but
/// serves an ordinary web page, not an openlore opaque instance (its
/// `GET /manifest` lacks the openlore marker envelope). `publish init` probes
/// the manifest, finds no openlore discriminator, and REFUSES to register the
/// URL — the CLI only registers a URL it can confirm speaks the ADR-062 opaque
/// contract. (US-SF-001 · Q-SF-D5 · ADR-062 §1/§6.)
///
/// @us-sf-001 @error @q-sf-d5 @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-001 Q-SF-D5 non-openlore URL refused)"]
fn publish_init_refuses_a_reachable_url_that_is_not_an_openlore_instance() {
    todo!(
        "DELIVER: Given a FakeInstance::not_an_openlore_instance() (reachable, manifest lacks \
         the openlore marker); When `publish init`; Then exit non-zero with a 'not an openlore \
         instance' message and config.publish_target stays UNSET. Binds Q-SF-D5: detection \
         marker = the openlore discriminator in GET /manifest."
    );
}

// =============================================================================
// US-SF-001 — status (DDD-4 `publish {init,push,pull,status}` grammar)
// =============================================================================

/// PI-5: After registering, `openlore publish status` reports the registered
/// instance URL, the derived card URL, and whether the instance is currently
/// reachable — a greppable inspection of the publish target that never touches
/// local claims. Re-running `publish init` with a new URL re-points the target
/// without mutating the local store. (US-SF-001 · DDD-4 · edge.)
///
/// @us-sf-001 @edge @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-001 publish status inspection)"]
fn publish_status_reports_the_registered_instance_and_leaves_local_store_untouched() {
    todo!(
        "DELIVER: Given a registered FakeInstance; When `openlore publish status`; \
         Then stdout names the instance_url + card_url + reachability, and \
         local.claims.row_count is UNCHANGED."
    );
}
