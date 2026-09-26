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
fn publish_init_registers_reachable_instance_and_prints_owned_identity() {
    // Given Maria's own reachable instance carrying the openlore marker, and a
    // local store holding one signed claim.
    let env = TestEnv::initialized();
    let instance = FakeInstance::fresh();
    seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);
    assert_eq!(
        registered_publish_target(&env),
        None,
        "precondition: no publish target registered yet"
    );

    // When she registers it.
    let url = instance.endpoint_url().to_string();
    let init = run_openlore_publish(&env, &["init", &url], &instance);

    // Then the command succeeds and the target is recorded.
    assert_eq!(
        init.status, 0,
        "`publish init` must register a reachable openlore instance;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );
    assert_eq!(
        registered_publish_target(&env).as_deref(),
        Some(url.as_str()),
        "config.publish_target must record the registered instance URL"
    );

    // And stdout names the instance, the UNCHANGED author DID, the derived
    // card URL, and confirms there is no central authority in the trust path.
    let card_url = format!("{url}/");
    for expected in [
        format!("instance_url: {url}"),
        format!("author_did: {}", env.identity.author_did()),
        format!("card_url: {card_url}"),
        "no central authority".to_string(),
    ] {
        assert!(
            init.stdout.contains(&expected),
            "`publish init` stdout must contain {expected:?};\n--- stdout ---\n{}",
            init.stdout
        );
    }

    // And the local store is untouched (D-6/D-7).
    assert_local_claims_unchanged(&env, &local_before);
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
fn publish_init_refuses_to_register_an_unreachable_instance_url() {
    // Given no instance is reachable at the URL, and a local store holding
    // one signed claim.
    let env = TestEnv::initialized();
    let instance = FakeInstance::unreachable();
    seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);

    // When Björn tries to register it.
    let init = run_openlore_publish(&env, &["init", instance.endpoint_url()], &instance);

    // Then the CLI refuses with a greppable 'cannot reach instance' message.
    assert_ne!(
        init.status, 0,
        "`publish init` must refuse an unreachable URL;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );
    assert_init_refused_with(
        &init,
        "cannot reach instance",
        "publish.instance_unreachable",
    );

    // And NOTHING was registered (offline-first is not speculative-register),
    // and the local store is untouched.
    assert_eq!(
        registered_publish_target(&env),
        None,
        "config.publish_target must stay UNSET after an unreachable-URL refusal"
    );
    assert_local_claims_unchanged(&env, &local_before);
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
fn publish_init_refuses_a_reachable_url_that_is_not_an_openlore_instance() {
    // Given a reachable URL serving an ordinary web page (its /manifest lacks
    // the openlore marker), and a local store holding one signed claim.
    let env = TestEnv::initialized();
    let instance = FakeInstance::not_an_openlore_instance();
    seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);

    // When Maria tries to register it.
    let init = run_openlore_publish(&env, &["init", instance.endpoint_url()], &instance);

    // Then the CLI refuses with a greppable 'not an openlore instance' message
    // (distinct from the unreachable refusal).
    assert_ne!(
        init.status, 0,
        "`publish init` must refuse a non-openlore URL;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );
    assert_init_refused_with(
        &init,
        "not an openlore instance",
        "publish.not_an_openlore_instance",
    );
    assert!(
        !init.stderr.contains("cannot reach instance"),
        "a reachable non-openlore URL must not be reported as unreachable;\n--- stderr ---\n{}",
        init.stderr
    );

    // And NOTHING was registered, and the local store is untouched.
    assert_eq!(
        registered_publish_target(&env),
        None,
        "config.publish_target must stay UNSET after a not-an-openlore-instance refusal"
    );
    assert_local_claims_unchanged(&env, &local_before);
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
