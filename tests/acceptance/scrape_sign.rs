//! Slice-02 acceptance — `openlore scrape github <target> --sign N[,N...]`:
//! review, edit, and sign a candidate (or several) via the slice-01 pipeline.
//!
//! The `--sign` continuation (WD-50 / WD-60 / ADR-017) is the value-capture
//! surface: it carries a candidate through the SAME slice-01
//! compose-sign-publish pipeline a hand-authored claim uses (WD-66 /
//! I-SCR-6 — the single-publish-path invariant, gate
//! `scraper_reuses_slice01_publish_path`), pre-filling the editable compose
//! fields. The human ALWAYS signs (WD-49 / J-004c); the scraper never
//! asserts. With no edits the signed claim's fields equal the candidate's
//! proposed values byte-for-byte and the confidence stays 0.25 (gate
//! `candidate_confidence_no_autoinflate`, the sign-time half).
//!
//! Provenance is DISPLAY-ONLY in slice-02 (WD-62 / ADR-018 / I-SCR-7): a
//! `derived-from: openlore-github-scraper (signal: ...)` line appears in the
//! compose preview and the publish output, but it is NEVER a signed-payload
//! field — the signed claim is byte-identical in shape to a hand-authored
//! one, so the CID path is unchanged (no Lexicon change ships).
//!
//! Layer placement: layer 3 / layer 5 subprocess, example-only (Mandate 11).
//! The sign path reuses the slice-01 `FakePds` / `FakeIdentity` doubles +
//! real DuckDB + real filesystem; `FakeGithub` provides the harvest.
//!
//! Covers:
//! - US-SCR-003: review, edit, and sign a candidate (happy + 4 sad/edge)
//! - US-SCR-005: select and sign several candidates in one pass (batch)
//! - WD-66 / I-SCR-6: single publish path (gate
//!   `scraper_reuses_slice01_publish_path`)
//! - WD-62 / I-SCR-7: provenance display-only; CID stability unchanged
//! - WD-52 / I-SCR-3: no auto-inflation at sign time
//! - inherited I-7 ("not as truth") + I-8 (retract hint)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SCR-003 — single-candidate sign happy path
// =============================================================================

/// SS-1 (gate `scraper_reuses_slice01_publish_path`, I-SCR-6 — load-bearing;
/// also the sign half of the SG-1 walking skeleton): `scrape github
/// rust-lang/cargo --sign 1` pre-fills the slice-01 compose editor with
/// candidate 1's fields, lets Maria raise confidence 0.25 -> 0.55, shows the
/// compose preview containing the literal "not as truth" (I-7), the
/// confidence "0.55 (weighted)", and the DISPLAY-ONLY `derived-from:
/// openlore-github-scraper (signal: Cargo.lock committed)` line; on Enter the
/// claim is signed via the SAME `VerbClaimAdd` path; on Y it is published via
/// the SAME `VerbClaimPublish` path as a hand-authored claim; the success
/// message mentions the retract command (I-8). The published claim lands in
/// the user's OWN `claims` table + own PDS exactly as a hand-authored claim.
///
/// Given Maria has a candidate list for github:rust-lang/cargo; When she runs
/// `--sign 1`, raises confidence to 0.55, accepts the other fields, presses
/// Enter to sign and confirms publish; Then the preview contained "not as
/// truth", the claim is signed with Maria's DID via VerbClaimAdd, published
/// via the SAME VerbClaimPublish path, records confidence 0.55 (numeric only,
/// WD-10), shows a display-only derived-from line naming the source signal,
/// and the success message mentions retract.
///
/// @us-scr-003 @walking_skeleton @driving_port @driving_adapter @real-io
/// @j-004 @j-004c @j-001 @kpi-scr-1 @i-scr-6 @happy @release-gate
#[test]
fn scrape_sign_one_candidate_signs_and_publishes_via_slice_01_pipeline() {
    // GIVEN Maria has an initialized env + a public repo serving the five
    // canonical cargo signals → five derived candidates. Candidate 1 is the
    // dependency-pinning proposal derived from "Cargo.lock committed".
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she runs `--sign 1` and walks the slice-01 compose editor: accept
    // subject / predicate / object / evidence (four Enters), raise confidence
    // 0.25 -> 0.55, press Enter to sign, then Y to publish (the two-prompt
    // contract; ADR-017 inherits the slice-01 sign/publish pipeline).
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1"],
        github.base_url(),
        "\n\n\n\n0.55\n\nY\n",
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 1 must exit 0 on the happy path; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN the compose preview carried the inherited "not as truth" framing
    // (I-7) before the sign gesture.
    assert_compose_preview_contains_not_as_truth(&outcome);

    // AND the preview showed the edited confidence as "0.55 (weighted)" (WD-10
    // display bucket; the numeric is what gets signed) ...
    assert!(
        outcome.stdout.contains("0.55 (weighted)"),
        "expected the compose preview to display the edited confidence as \
         '0.55 (weighted)'; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // ... AND a DISPLAY-ONLY `derived-from` provenance line naming the source
    // signal (WD-62 / I-SCR-7 — shown, never a signed-payload field).
    assert!(
        outcome
            .stdout
            .contains("derived-from: openlore-github-scraper (signal: Cargo.lock committed"),
        "expected a display-only 'derived-from: openlore-github-scraper (signal: \
         Cargo.lock committed ...)' line in the compose/publish output (WD-62 / I-SCR-7); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND the success message mentions the retract command (I-8 retract hint).
    assert!(
        outcome.stdout.contains("openlore claim retract"),
        "expected the publish success message to mention `openlore claim retract` \
         (I-8 retract hint); \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // Recover the CID from the success block ("Published claim <cid>.") so the
    // universe-bound assertions below can target the exact record.
    let cid = published_cid_from_stdout(&outcome.stdout);

    // AND the signed claim was published via the SAME slice-01 VerbClaimPublish
    // path: exactly ONE record on the user's OWN PDS under the user's OWN
    // author DID at-uri — no parallel publish path (I-SCR-6 / WD-66, gate
    // scraper_reuses_slice01_publish_path).
    assert_scraper_reuses_slice01_publish_path(&env, &cid);

    // AND the signed payload records the edited confidence 0.55 (the human's
    // conscious raise; no auto-inflation between proposal and sign).
    assert_candidate_confidence_unchanged(&env, &cid, 0.55);
}

/// SS-2 (gate `candidate_confidence_no_autoinflate`, sign-time half; I-SCR-3
/// — load-bearing): accepting ALL candidate defaults unchanged signs exactly
/// what was proposed. The signed claim's subject/predicate/object/evidence
/// equal candidate 3's proposed values byte-for-byte AND the confidence is
/// 0.25 (no auto-inflation). KPI-SCR-5 records this as a zero-edit sign.
///
/// Given Tobias has a candidate list and selects candidate 3; When he
/// presses Enter through every field including the 0.25 confidence; Then the
/// signed claim's fields equal candidate 3's proposed values and its
/// confidence is 0.25.
///
/// @us-scr-003 @driving_port @real-io @j-004c @wd-52 @kpi-scr-2 @release-gate @edge
#[test]
fn scrape_sign_accepting_all_defaults_signs_proposal_byte_for_byte_no_inflation() {
    // GIVEN Tobias has an initialized env + the public repo serving the five
    // canonical cargo signals → five derived candidates. Candidate 3 is the
    // test-driven proposal derived from the "test/source ratio 0.61" signal
    // (signal `TestRatioOrCiMatrix` → `org.openlore.philosophy.test-driven`,
    // its single source_url the candidate's only evidence). Its PROPOSED
    // values are deterministic — the SSOT mapping + the fixture fix them
    // exactly, so the test pins them as the byte-for-byte oracle below.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN he runs `--sign 3` and presses Enter through EVERY pre-filled
    // compose field WITHOUT editing any of them: accept subject / predicate /
    // object / evidence (four Enters) AND accept the conservative 0.25
    // confidence default (a fifth Enter), then Enter to sign and Y to publish.
    // This is the zero-edit sign (KPI-SCR-5): the human consciously signs but
    // changes nothing, so the proposal is signed exactly as derived.
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "3"],
        github.base_url(),
        "\n\n\n\n\n\nY\n",
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 3 accepting all defaults must exit 0; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Recover the CID from the success block so the universe-bound assertions
    // below target the exact signed record.
    let cid = published_cid_from_stdout(&outcome.stdout);

    // THEN the signed-from-scraper claim's payload equals candidate 3's
    // PROPOSED values BYTE-FOR-BYTE — no field drifted between proposal and
    // sign because the human edited nothing (KPI-SCR-5 zero-edit sign; WD-52 /
    // I-SCR-3 the sign half). Read the on-disk signed payload (the same
    // `claims/<cid>.json` → `SignedClaim` surface `assert_candidate_confidence_unchanged`
    // uses) and compare each field to candidate 3's proposed values, which the
    // SSOT mapping + the fixture fix exactly:
    //   subject   : github:rust-lang/cargo            (the resolved target)
    //   predicate : embodiesPhilosophy                (EMBODIES_PHILOSOPHY)
    //   object    : org.openlore.philosophy.test-driven  (mapping entry 3)
    //   evidence  : [the single TestRatioOrCiMatrix source_url]
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    let json_bytes = std::fs::read(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "expected signed-from-scraper claim file at {}; got {e}",
            artifact_path.display()
        )
    });
    let signed: claim_domain::SignedClaim =
        serde_json::from_slice(&json_bytes).unwrap_or_else(|e| {
            panic!(
                "could not deserialize signed claim at {}: {e}\n--- file ---\n{}",
                artifact_path.display(),
                String::from_utf8_lossy(&json_bytes)
            )
        });

    assert_eq!(
        signed.unsigned.subject, "github:rust-lang/cargo",
        "accepting all defaults must sign candidate 3's PROPOSED subject \
         byte-for-byte (no edit); signed payload = {signed:#?}"
    );
    assert_eq!(
        signed.unsigned.predicate, "embodiesPhilosophy",
        "accepting all defaults must sign candidate 3's PROPOSED predicate \
         byte-for-byte (no edit); signed payload = {signed:#?}"
    );
    assert_eq!(
        signed.unsigned.object, "org.openlore.philosophy.test-driven",
        "accepting all defaults must sign candidate 3's PROPOSED object \
         byte-for-byte (no edit); signed payload = {signed:#?}"
    );
    assert_eq!(
        signed.unsigned.evidence,
        vec!["https://github.com/rust-lang/cargo/tree/master/tests".to_string()],
        "accepting all defaults must sign candidate 3's PROPOSED evidence \
         byte-for-byte (no edit); signed payload = {signed:#?}"
    );

    // AND the confidence stayed at the conservative 0.25 default — the scraper
    // NEVER auto-inflated it and the human did not raise it (the sign-time half
    // of `candidate_confidence_no_autoinflate`, KPI-SCR-2 / WD-52 / I-SCR-3).
    assert_candidate_confidence_unchanged(&env, &cid, 0.25);
}

/// SS-3 (WD-62 / I-SCR-7): the `derived-from` provenance is DISPLAY-ONLY —
/// it appears in the compose preview + publish output but is NEVER a field
/// in the signed payload. The signed-from-scraper claim's canonical payload
/// is byte-identical to a hand-authored claim with identical fields, so its
/// CID equals the hand-authored CID (no new CID path; ADR-018 / no Lexicon
/// change). This is the forward-compat / CID-stability guarantee.
///
/// Given a scraper candidate signed with fields F; When a hand-authored
/// claim is composed with the SAME fields F; Then both produce the SAME CID,
/// and the on-disk signed payload of the scraper claim carries NO
/// `derived-from` / `derivedFrom` key.
///
/// @us-scr-003 @driving_port @real-io @j-004c @wd-62 @i-scr-7 @cid-stability @happy
#[test]
fn scrape_sign_provenance_is_display_only_and_does_not_alter_signed_cid() {
    // GIVEN Maria has an initialized env + the public repo serving the five
    // canonical cargo signals → five derived candidates. Candidate 1 is the
    // dependency-pinning proposal derived from "Cargo.lock committed".
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she signs candidate 1 with fields F, raising confidence 0.25 -> 0.55
    // and accepting the rest, then signs (Enter) and publishes (Y). The
    // compose/publish output shows the DISPLAY-ONLY `derived-from` provenance
    // line — but it is NEVER folded into the signed payload (WD-62 / I-SCR-7).
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1"],
        github.base_url(),
        "\n\n\n\n0.55\n\nY\n",
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 1 must exit 0 on the happy path; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the DISPLAY-ONLY `derived-from` provenance line appeared in the
    // compose/publish output (it is SHOWN — just never signed).
    assert!(
        outcome
            .stdout
            .contains("derived-from: openlore-github-scraper (signal:"),
        "expected a display-only 'derived-from: openlore-github-scraper (signal: ...)' line \
         in the compose/publish output (WD-62 / I-SCR-7); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // Recover the CID from the success block so we can target the exact signed
    // record on disk.
    let cid = published_cid_from_stdout(&outcome.stdout);

    // Read the on-disk signed payload (the same `claims/<cid>.json` →
    // `SignedClaim` surface SS-2 uses).
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    let json_bytes = std::fs::read(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "expected signed-from-scraper claim file at {}; got {e}",
            artifact_path.display()
        )
    });
    let signed: claim_domain::SignedClaim =
        serde_json::from_slice(&json_bytes).unwrap_or_else(|e| {
            panic!(
                "could not deserialize signed claim at {}: {e}\n--- file ---\n{}",
                artifact_path.display(),
                String::from_utf8_lossy(&json_bytes)
            )
        });

    // THEN the scraper claim's CID equals the CID a HAND-AUTHORED claim with
    // the SAME fields F would produce. We reconstruct that hand-authored
    // equivalent INDEPENDENTLY from the signed claim's observable fields
    // (subject / predicate / object / evidence / confidence / author /
    // composedAt / references) — carrying NO derived-from provenance, exactly
    // as `claim add` would build it — then run it through the SAME pure core
    // (canonicalize -> compute_cid). If the provenance leaked into the signed
    // payload, this reconstructed CID would differ (WD-62 / I-SCR-7 / ADR-018:
    // no Lexicon change, no CID path change).
    let hand_authored = claim_domain::UnsignedClaim {
        subject: signed.unsigned.subject.clone(),
        predicate: signed.unsigned.predicate.clone(),
        object: signed.unsigned.object.clone(),
        evidence: signed.unsigned.evidence.clone(),
        // `Confidence`'s inner f64 is crate-private; round-trip the SAME
        // numeric value through serde to reconstruct the wrapper (the trick
        // the support module uses for `build_verifiable_peer_records`).
        confidence: serde_json::from_value(
            serde_json::to_value(signed.unsigned.confidence)
                .expect("re-serialize signed confidence"),
        )
        .expect("reconstruct hand-authored confidence from the same numeric value"),
        author_did: signed.unsigned.author_did.clone(),
        composed_at: signed.unsigned.composed_at.clone(),
        references: signed.unsigned.references.clone(),
        reason: signed.unsigned.reason.clone(),
    };
    let hand_authored_bytes =
        claim_domain::canonicalize(&hand_authored).expect("canonicalize hand-authored claim");
    let hand_authored_cid = claim_domain::compute_cid(&hand_authored_bytes);

    assert_eq!(
        hand_authored_cid.0, cid,
        "the scraper-signed claim's CID must EQUAL the CID a hand-authored claim with the \
         SAME fields produces — the derived-from provenance is display-only and must NOT alter \
         the signed CID (WD-62 / I-SCR-7 / ADR-018, CID stability); \
         scraper cid={cid}, hand-authored cid={}",
        hand_authored_cid.0
    );

    // AND the published CID equals the CID computed from the signed payload's
    // OWN unsigned bytes — pinning that the on-disk signed shape is exactly
    // what the CID covers (no provenance folded in).
    let signed_payload_bytes =
        claim_domain::canonicalize(&signed.unsigned).expect("canonicalize on-disk signed payload");
    assert_eq!(
        claim_domain::compute_cid(&signed_payload_bytes).0,
        cid,
        "the on-disk signed payload must canonicalize to exactly the published CID \
         (the provenance line contributes ZERO bytes to the signed shape)"
    );

    // AND the on-disk signed JSON carries NO `derived-from` / `derivedFrom`
    // key anywhere — the provenance is display-only, never a signed-payload
    // field (the byte-level guard mirroring the canonicalize-omits-extras
    // contract).
    let raw_json = String::from_utf8_lossy(&json_bytes);
    assert!(
        !raw_json.contains("derived-from") && !raw_json.contains("derivedFrom"),
        "the scraper-signed claim's on-disk JSON must contain NO 'derived-from'/'derivedFrom' \
         key (provenance is display-only — WD-62 / I-SCR-7); \n--- {} ---\n{}",
        artifact_path.display(),
        raw_json
    );
}

// =============================================================================
// US-SCR-003 — single-candidate sad / edge paths (example-only; Mandate 11)
// =============================================================================

/// SS-4 / Sad (US-SCR-003 Ex 3): an out-of-range `--sign` index is rejected
/// BEFORE any compose begins, naming the valid range. Nothing is composed,
/// signed, or published.
///
/// Given the candidate list shows 5 candidates; When Aanya runs `--sign 9`;
/// Then the CLI exits non-zero with "candidate 9 does not exist; valid range
/// 1..5", and no claim is composed, signed, or published.
///
/// @us-scr-003 @driving_port @real-io @j-004c @error
#[test]
fn scrape_sign_out_of_range_index_is_rejected_before_compose() {
    // GIVEN Aanya has an initialized env + the public repo serving the five
    // canonical cargo signals → a candidate list of exactly five entries
    // (valid selection range 1..5). Candidate 9 does not exist.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she runs `--sign 9` — an out-of-range index. No stdin is supplied:
    // the rejection must happen BEFORE any compose prompt, so the run never
    // blocks waiting for input.
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "9"],
        github.base_url(),
        "",
    );

    // THEN the CLI exits non-zero (the selection is rejected before compose).
    assert_ne!(
        outcome.status, 0,
        "an out-of-range --sign index must exit non-zero; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND stderr names the invalid selection + the valid range (the
    // domain-shaped error from `parse_selection`, carrying the candidate count
    // for context).
    assert!(
        outcome
            .stderr
            .contains("candidate 9 does not exist; valid range 1..5"),
        "expected stderr to name the out-of-range index + valid range \
         ('candidate 9 does not exist; valid range 1..5'); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND NO compose preview ever reached stdout — the rejection short-circuits
    // BEFORE any candidate is composed, so the inherited "not as truth" framing
    // (I-7), which only the compose preview emits, is ABSENT.
    assert!(
        !outcome.stdout.contains("not as truth"),
        "an out-of-range --sign must be rejected BEFORE any compose begins; \
         the 'not as truth' compose-preview literal must be ABSENT from stdout; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND nothing was composed, signed, or published: zero `claims` rows, zero
    // PDS records, zero local claim artifacts (the human-gate held — no claim
    // crossed the storage boundary).
    assert_no_claim_persisted(&env);
}

/// SS-5 / Sad (US-SCR-003 Ex 4): editing confidence out of `[0.0,1.0]`
/// re-prompts with the constraint and writes NO claim until a valid value is
/// entered.
///
/// Given Maria runs `--sign 1`; When she enters 1.5 for confidence; Then the
/// CLI re-prompts "confidence must be between 0.0 and 1.0", and no claim is
/// written until a valid confidence is entered (she then enters 0.55 and
/// compose proceeds).
///
/// @us-scr-003 @driving_port @real-io @j-004c @error
#[test]
fn scrape_sign_out_of_range_confidence_reprompts_without_writing() {
    // GIVEN Maria has an initialized env + the public repo serving the five
    // canonical cargo signals → five derived candidates. Candidate 1 is the
    // dependency-pinning proposal derived from "Cargo.lock committed".
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she runs `--sign 1` and walks the slice-01 compose editor: accept
    // subject / predicate / object / evidence (four Enters), then types an
    // OUT-OF-RANGE confidence `1.5` (outside [0.0, 1.0]) followed by the VALID
    // `0.55` — the re-prompt loop must reject 1.5, re-ask, and accept 0.55. A
    // final Enter signs and `Y` publishes. The invalid value must NEVER reach a
    // write: nothing is composed/signed/published until a valid confidence is in
    // hand.
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1"],
        github.base_url(),
        "\n\n\n\n1.5\n0.55\n\nY\n",
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 1 must exit 0 once a valid confidence is finally entered; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN the CLI re-prompted with the range constraint when `1.5` was entered
    // (the SS-5 hard AC: "confidence must be between 0.0 and 1.0").
    assert!(
        outcome
            .stdout
            .contains("confidence must be between 0.0 and 1.0"),
        "expected the out-of-range confidence 1.5 to trigger the re-prompt \
         'confidence must be between 0.0 and 1.0'; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND the re-prompt happened BEFORE any write crossed the storage boundary:
    // the validation message appears in stdout strictly before the
    // `Published claim <cid>.` success line — proving 1.5 never produced a claim
    // (nothing was written until the valid 0.55 was in hand).
    let reprompt_at = outcome
        .stdout
        .find("confidence must be between 0.0 and 1.0")
        .expect("re-prompt message must be present in stdout");
    let published_at = outcome
        .stdout
        .find("Published claim ")
        .expect("publish success line must be present in stdout after a valid confidence");
    assert!(
        reprompt_at < published_at,
        "the out-of-range re-prompt must occur BEFORE the claim is written/published \
         (no claim until a valid confidence is entered); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND the eventual signed claim records EXACTLY the valid 0.55 the human
    // finally entered — never the rejected 1.5 (the re-prompt loop accepted only
    // the in-range value).
    let cid = published_cid_from_stdout(&outcome.stdout);
    assert_candidate_confidence_unchanged(&env, &cid, 0.55);

    // AND exactly ONE claim crossed the storage boundary — the valid one. The
    // rejected 1.5 produced ZERO writes, so the user's OWN PDS holds exactly the
    // single signed-from-scraper record at the user's own at-uri (the
    // single-publish-path proof doubles as the "1.5 wrote nothing" guard).
    assert_scraper_reuses_slice01_publish_path(&env, &cid);
}

/// SS-6 / Edge (US-SCR-003 Ex 5): declining the publish prompt retains the
/// LOCALLY signed claim and hints at `openlore claim publish <cid>`. The
/// claim is persisted locally (slice-01 behavior) but NOT published.
///
/// Given Tobias runs `--sign 2` and signs locally; When he answers "N" to
/// the publish prompt; Then the claim is persisted locally and NOT
/// published, and the CLI hints it can be published later with `openlore
/// claim publish <cid>`.
///
/// @us-scr-003 @driving_port @real-io @j-004c @i-scr-6 @edge
#[test]
fn scrape_sign_declining_publish_retains_local_claim_with_publish_hint() {
    // GIVEN Tobias has an initialized env + the public repo serving the five
    // canonical cargo signals → five derived candidates. He selects candidate
    // 2 (a valid 1-based index within the 1..5 range).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN he runs `--sign 2` and walks the slice-01 compose editor accepting
    // every pre-filled field (four Enters: subject / predicate / object /
    // evidence, plus a fifth Enter for the conservative confidence default),
    // presses Enter to sign locally, then answers `n` to DECLINE the SEPARATE
    // publish prompt (the two-prompt contract; ADR-017 inherits the slice-01
    // sign/publish pipeline). The decline is the local-only outcome: signed +
    // stored, never federated.
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "2"],
        github.base_url(),
        "\n\n\n\n\n\nn\n",
    );

    assert_eq!(
        outcome.status, 0,
        "declining publish after a successful local sign must exit 0 \
         (local-only outcome, not an error); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Recover the CID from the `Written to local store: <...>/<cid>.json` line
    // — the decline path emits NO `Published claim <cid>.` block, so the CID is
    // read from the local-store artifact path the sign step printed.
    let cid = local_store_cid_from_stdout(&outcome.stdout);

    // THEN the signed claim IS persisted to the local store: the on-disk
    // `claims/<cid>.json` artifact exists (the slice-01 local-sign behavior the
    // scraper path reuses).
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    assert!(
        artifact_path.exists(),
        "expected the locally-signed claim file to exist at {} after declining \
         publish (the sign step persists locally regardless of the publish answer); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        artifact_path.display(),
        outcome.stdout,
        outcome.stderr
    );

    // AND the claim was NOT published — ZERO `create_record` calls reached the
    // user's PDS (the decline branch makes no PDS call; KPI-5 local-first /
    // I-SCR-6 single-publish-path stays unexercised).
    assert_no_pds_call_was_made(&env);

    // AND stdout hints the claim can be published later with the standalone
    // verb naming the exact CID (`openlore claim publish <cid>`), so the human
    // can federate it at will.
    let expected_hint = format!("openlore claim publish {cid}");
    assert!(
        outcome.stdout.contains(&expected_hint),
        "expected stdout to hint the claim can be published later with \
         {expected_hint:?}; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
}

/// Recover the locally-signed claim's CID from the `Written to local store:
/// <...>/<cid>.json` line the sign step prints. The decline-publish path emits
/// NO `Published claim <cid>.` block (that line only appears after a real
/// publish), so SS-6 reads the CID from the local-store artifact path instead.
fn local_store_cid_from_stdout(stdout: &str) -> String {
    for line in stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("Written to local store:") {
            let path = rest.trim();
            if let Some(name) = std::path::Path::new(path).file_stem() {
                return name.to_string_lossy().into_owned();
            }
        }
    }
    panic!(
        "could not find a 'Written to local store: <path>/<cid>.json' line in stdout \
         to recover the locally-signed CID; \n--- stdout ---\n{stdout}"
    );
}

// =============================================================================
// US-SCR-005 — batch sign (each candidate individually human-signed)
// =============================================================================

/// SS-7 (US-SCR-005 happy path): `--sign 1,3,4` walks each candidate through
/// its OWN slice-01 compose preview, requires the human's individual signing
/// gesture per candidate, shows a running "(k of M signed)" progress line,
/// and offers NO "sign all without review" affordance. Batch is a
/// convenience OVER the human-gate, never a bypass (WD-49 / J-004c).
///
/// Given Maria has a 5-candidate list; When she runs `--sign 1,3,4`; Then
/// the CLI presents candidate 1's compose preview requiring a signing
/// gesture, then "(1 of 3 signed)" + candidate 3, then "(2 of 3 signed)" +
/// candidate 4; each is signed individually via the slice-01 pipeline; there
/// is no "sign all without review" affordance.
///
/// @us-scr-005 @driving_port @real-io @j-004c @kpi-scr-1 @kpi-scr-2 @happy
#[test]
fn scrape_sign_batch_walks_each_candidate_through_individual_compose_and_sign() {
    // GIVEN Maria has an initialized env + the public repo serving the five
    // canonical cargo signals → a candidate list of exactly five entries. She
    // will batch-sign three of them (1, 3, 4) — each carried through its OWN
    // slice-01 compose-sign-publish gesture (US-SCR-005; the human-gate holds
    // PER candidate, batch is convenience never a bypass; WD-49 / J-004c).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she runs `--sign 1,3,4` and walks EACH of the three compose editors
    // in turn, accepting every pre-filled field unchanged (four field Enters +
    // a fifth Enter for the conservative confidence default), pressing Enter to
    // sign, then Y to publish — the SAME zero-edit slice-01 gesture SS-2 uses,
    // repeated once per selected candidate. Three previews, three individual
    // signing gestures: there is NO single "sign all" shortcut.
    let one_sign = "\n\n\n\n\n\nY\n";
    let stdin = one_sign.repeat(3);
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1,3,4"],
        github.base_url(),
        &stdin,
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 1,3,4 must exit 0 once all three candidates are signed + published; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN stdout shows THREE SEPARATE compose previews — each carrying the
    // inherited "not as truth" framing (I-7). Counting the literal occurrences
    // proves each selected candidate was composed individually (one preview per
    // candidate), not a single batched preview.
    let preview_count = outcome.stdout.matches("not as truth").count();
    assert_eq!(
        preview_count, 3,
        "expected THREE separate compose previews (one per selected candidate, each with \
         'not as truth'); found {preview_count}; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND a running per-candidate progress line appears between the signs: after
    // the first candidate is signed the CLI shows "(1 of 3 signed)" before the
    // second candidate's preview, and "(2 of 3 signed)" before the third. Batch
    // is a sequence of individual human-gates, surfaced as progress.
    assert!(
        outcome.stdout.contains("(1 of 3 signed)"),
        "expected the running progress line '(1 of 3 signed)' after the first candidate is \
         signed; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
    assert!(
        outcome.stdout.contains("(2 of 3 signed)"),
        "expected the running progress line '(2 of 3 signed)' after the second candidate is \
         signed; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND there is NO "sign all without review" affordance anywhere in output —
    // batch never bypasses the per-candidate human-gate (WD-49 / J-004c).
    assert!(
        !outcome.stdout.to_lowercase().contains("sign all"),
        "batch must offer NO 'sign all' affordance (the human signs each candidate \
         individually; WD-49 / J-004c); \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND exactly THREE records landed on the user's OWN PDS — one per signed
    // candidate, each published via the SAME slice-01 VerbClaimPublish path
    // (single-publish-path per claim; WD-66 / I-SCR-6). The bare author DID
    // at-uri proves each is the user's OWN artifact (no parallel publish path).
    let bare_author_did = env
        .identity
        .author_did()
        .split('#')
        .next()
        .unwrap_or_else(|| env.identity.author_did())
        .to_string();
    let records = env.pds.records();
    assert_eq!(
        records.len(),
        3,
        "batch --sign 1,3,4 must publish EXACTLY THREE records on the user's OWN PDS \
         (one per signed candidate, each via the single slice-01 publish path); got {}: {:?}",
        records.len(),
        records
    );
    for record in &records {
        assert_eq!(
            record.collection, "org.openlore.claim",
            "each batched record must be in the org.openlore.claim collection; got {}",
            record.collection
        );
        assert!(
            record
                .at_uri
                .starts_with(&format!("at://{bare_author_did}/org.openlore.claim/")),
            "each batched record must live at the user's OWN at-uri (bare author DID, \
             slice-01 publish path); got {}",
            record.at_uri
        );
    }
}

/// SS-8 / Edge (US-SCR-005 Ex 2): a candidate can be SKIPPED mid-batch
/// (cancel its compose) without aborting the rest; the summary reports
/// signed-vs-skipped counts.
///
/// Given Tobias runs `--sign 1,2,5`; When he signs candidate 1 then cancels
/// candidate 2's compose; Then the CLI prints "skipped candidate 2",
/// proceeds to candidate 5, and the final summary reports "2 signed, 1
/// skipped".
///
/// @us-scr-005 @driving_port @real-io @j-004c @edge
#[test]
fn scrape_sign_batch_skip_one_candidate_does_not_abort_the_rest() {
    // GIVEN Tobias has an initialized env + the public repo serving the five
    // canonical cargo signals → a candidate list of exactly five entries. He
    // batch-selects 1, 2, and 5 — but mid-batch he changes his mind about
    // candidate 2 and SKIPS it (cancels its compose). The skip must NOT abort
    // the batch: candidates 1 and 5 still get signed + published (US-SCR-005
    // Ex 2; batch fault isolation, WD-49 / J-004c).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN he runs `--sign 1,2,5`:
    //   - candidate 1: accept every field (four field Enters + the confidence
    //     default Enter), press Enter to sign, `Y` to publish;
    //   - candidate 2: accept every field (the same five Enters reach the
    //     compose preview), then SKIP at the sign prompt with the `skip`
    //     gesture (Q-DELIVER-5; behavior asserted, not the exact keystroke) —
    //     this cancels candidate 2's compose WITHOUT aborting the batch and
    //     consumes NO publish answer (a skipped candidate is never published);
    //   - candidate 5: accept every field, press Enter to sign, `Y` to publish.
    // The skip is in-band (a line on the SAME piped stdin), never EOF — EOF
    // would end the whole stream and starve candidate 5.
    let sign_then_publish = "\n\n\n\n\n\nY\n";
    let skip = "\n\n\n\n\nskip\n";
    let stdin = format!("{sign_then_publish}{skip}{sign_then_publish}");
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1,2,5"],
        github.base_url(),
        &stdin,
    );

    assert_eq!(
        outcome.status, 0,
        "scrape --sign 1,2,5 with one mid-batch skip must still exit 0 \
         (a skip is a normal outcome, not an error); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN stdout announces the skipped candidate by its 1-based selection
    // index — "skipped candidate 2" — so the human sees WHICH candidate was
    // dropped (the skip is visible, not silent).
    assert!(
        outcome.stdout.contains("skipped candidate 2"),
        "expected stdout to announce 'skipped candidate 2' when candidate 2's compose \
         is canceled mid-batch; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND the batch did NOT abort after the skip — it PROCEEDED to candidate 5,
    // which was composed (its compose preview carries the inherited "not as
    // truth" framing, I-7) AFTER the skip announcement. Ordering proves the
    // skip did not short-circuit the remaining selection.
    let skip_at = outcome
        .stdout
        .find("skipped candidate 2")
        .expect("the skip announcement must be present in stdout");
    let last_preview_at = outcome
        .stdout
        .rfind("not as truth")
        .expect("at least one compose preview must reach stdout");
    assert!(
        skip_at < last_preview_at,
        "the skip must NOT abort the batch — candidate 5's compose preview must appear \
         AFTER 'skipped candidate 2' (the batch proceeded past the skip); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND the final summary reports the signed-vs-skipped tally for the whole
    // batch — "2 signed, 1 skipped" (two human-gates held, one was declined).
    assert!(
        outcome.stdout.contains("2 signed, 1 skipped"),
        "expected the final batch summary to report '2 signed, 1 skipped'; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND exactly TWO records landed on the user's OWN PDS — one per SIGNED
    // candidate (1 and 5); the skipped candidate 2 produced ZERO writes (no
    // sign, no publish). The skip is fully fault-isolated: it neither persists
    // a claim nor blocks its neighbors.
    let records = env.pds.records();
    assert_eq!(
        records.len(),
        2,
        "a mid-batch skip must publish EXACTLY TWO records (the two signed \
         candidates); the skipped candidate writes nothing; got {}: {:?}",
        records.len(),
        records
    );
    for record in &records {
        assert_eq!(
            record.collection, "org.openlore.claim",
            "each published record must be in the org.openlore.claim collection; got {}",
            record.collection
        );
    }
}

/// SS-9 / Sad (US-SCR-005 Ex 3): an invalid selection list (duplicate index
/// OR out-of-range index) is rejected BEFORE any compose begins, naming the
/// offending indices. Nothing is composed.
///
/// Given the candidate list shows 5 candidates; When Aanya runs `--sign
/// 1,1,9`; Then the CLI exits non-zero naming the duplicate index 1 AND the
/// out-of-range index 9, and no claim is composed, signed, or published.
///
/// @us-scr-005 @driving_port @real-io @j-004c @error
#[test]
fn scrape_sign_batch_invalid_selection_list_is_rejected_before_compose() {
    // GIVEN Aanya has an initialized env + the public repo serving the five
    // canonical cargo signals → a candidate list of exactly five entries
    // (valid selection range 1..5). The batch selection `1,1,9` is doubly
    // invalid: index 1 is DUPLICATED and index 9 is OUT OF RANGE.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN she runs `--sign 1,1,9` — an invalid batch selection list. No stdin
    // is supplied: the rejection must happen BEFORE any compose prompt, so the
    // run never blocks waiting for input (the validation short-circuits up
    // front, exactly like SS-4's out-of-range single index).
    let outcome = run_openlore_scrape_with_stdin(
        &env,
        &["scrape", "github", "rust-lang/cargo", "--sign", "1,1,9"],
        github.base_url(),
        "",
    );

    // THEN the CLI exits non-zero (the selection list is rejected before any
    // compose begins).
    assert_ne!(
        outcome.status, 0,
        "an invalid --sign selection list (duplicate + out-of-range) must exit non-zero; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND stderr names BOTH offending indices — the DUPLICATE index 1 AND the
    // OUT-OF-RANGE index 9 (valid range 1..5). A single batch validation pass
    // reports every problem at once so the human can fix the whole list in one
    // edit (US-SCR-005 Ex 3; the batch superset of SS-4's single-index reject).
    assert!(
        outcome.stderr.contains("duplicate candidate index 1"),
        "expected stderr to name the DUPLICATE index ('duplicate candidate index 1'); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
    assert!(
        outcome
            .stderr
            .contains("candidate 9 does not exist; valid range 1..5"),
        "expected stderr to name the OUT-OF-RANGE index + valid range \
         ('candidate 9 does not exist; valid range 1..5'); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND NO compose preview ever reached stdout — the rejection short-circuits
    // BEFORE any candidate is composed, so the inherited "not as truth" framing
    // (I-7), which only the compose preview emits, is ABSENT.
    assert!(
        !outcome.stdout.contains("not as truth"),
        "an invalid --sign list must be rejected BEFORE any compose begins; \
         the 'not as truth' compose-preview literal must be ABSENT from stdout; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // AND nothing was composed, signed, or published: zero `claims` rows, zero
    // PDS records, zero local claim artifacts (the human-gate held — no claim
    // crossed the storage boundary).
    assert_no_claim_persisted(&env);
}
