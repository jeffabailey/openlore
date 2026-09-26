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
fn publish_init_never_changes_the_signing_identity_or_local_store() {
    // Given Maria's signing identity, her own reachable instance, and a local
    // store holding one signed claim.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let instance = FakeInstance::fresh();
    seed_one_signed_local_claim(&env, 0.86);
    let identity_before = std::fs::read(env.identity_toml_path()).expect("read identity.toml");
    let store_before = local_store_bytes(&env);

    // When she registers the instance.
    let url = instance.endpoint_url().to_string();
    let init = run_openlore_publish(&env, &["init", &url], &instance);
    assert_eq!(
        init.status, 0,
        "`publish init` must register a reachable openlore instance;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );

    // Then her author DID is UNCHANGED — reported as-is, and her persisted
    // identity is byte-identical.
    assert!(
        init.stdout
            .contains(&format!("author_did: {}", env.identity.author_did())),
        "`publish init` must report the unchanged author DID;\n--- stdout ---\n{}",
        init.stdout
    );
    assert_eq!(
        std::fs::read(env.identity_toml_path()).expect("read identity.toml"),
        identity_before,
        "`publish init` must never change the signing identity (D-7)"
    );

    // And the instance was only probed (read) — no signing key, seed, or key
    // material crossed the seam; the instance holds nothing.
    assert!(
        !instance.recorded_requests().is_empty(),
        "`publish init` must probe the instance before registering it"
    );
    assert_no_key_material_sent(&instance, &env.identity);
    assert_instance_only_read(&instance);

    // And the local store is byte-unchanged (D-6).
    assert!(
        local_store_bytes(&env) == store_before,
        "`publish init` must leave every local store file byte-unchanged (D-6)"
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
fn publish_status_reports_the_registered_instance_and_leaves_local_store_untouched() {
    // Given Maria registered her first instance, and a local store holding
    // one signed claim.
    let env = TestEnv::initialized();
    let first = FakeInstance::fresh();
    seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);
    let first_url = first.endpoint_url().to_string();
    let init = run_openlore_publish(&env, &["init", &first_url], &first);
    assert_eq!(
        init.status, 0,
        "precondition: first `publish init` must succeed"
    );

    // When she inspects the publish target.
    let status = run_openlore_publish(&env, &["status"], &first);

    // Then status names the registered instance, its card URL, and that it
    // is currently reachable — by reading the instance, never writing it.
    assert_status_reports(&status, &first_url, "reachable");
    assert_instance_only_read(&first);
    assert_local_claims_unchanged(&env, &local_before);

    // When she re-points the target at a second reachable instance.
    let second = FakeInstance::fresh();
    let second_url = second.endpoint_url().to_string();
    let reinit = run_openlore_publish(&env, &["init", &second_url], &second);
    assert_eq!(
        reinit.status, 0,
        "re-running `publish init` with a new reachable URL must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        reinit.stdout, reinit.stderr
    );

    // Then status reflects the NEW target (the fallback seam aims at the old
    // instance, so only the registration can name the new one), and the
    // local claims are still exactly as before.
    let status = run_openlore_publish(&env, &["status"], &first);
    assert_status_reports(&status, &second_url, "reachable");
    assert!(
        !status.stdout.contains(&first_url),
        "status must no longer name the previous target;\n--- stdout ---\n{}",
        status.stdout
    );
    assert_instance_only_read(&second);
    assert_local_claims_unchanged(&env, &local_before);
}
