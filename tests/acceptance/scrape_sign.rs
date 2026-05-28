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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-4. GIVEN a 5-candidate list; WHEN --sign 9; THEN exit \
         non-zero, stderr contains 'candidate 9 does not exist; valid range 1..5', NO compose \
         preview reached stdout (assert the 'not as truth' literal is ABSENT), \
         assert_no_claim_persisted(&env)."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-5. WHEN --sign 1 with stdin '...\\n1.5\\n0.55\\n\\nY\\n' \
         (accept fields, type 1.5 then 0.55 for confidence); THEN stdout re-prompts \
         'confidence must be between 0.0 and 1.0', and the claim is only written after the \
         valid 0.55 — assert the eventual signed claim records 0.55 and that no claim file \
         existed at the moment of the re-prompt."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-6. WHEN --sign 2 with stdin '\\nn\\n' (Enter sign, n decline \
         publish); THEN exit 0, the claim file exists under claims_dir (local sign \
         succeeded), assert_no_pds_call_was_made(&env) (NOT published), and stdout hints \
         `openlore claim publish <cid>`."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-7. WHEN --sign 1,3,4 with stdin signing each of the three \
         previews in turn; THEN stdout shows three SEPARATE compose previews (each with 'not \
         as truth'), the progress lines '(1 of 3 signed)' and '(2 of 3 signed)', three \
         records on the user's own PDS, and NO 'sign all' affordance anywhere in output."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-8 (skip gesture per Q-DELIVER-5 — behavior asserted, not the \
         keystroke). WHEN --sign 1,2,5, sign 1, cancel 2's compose, sign 5; THEN stdout \
         contains 'skipped candidate 2', the flow proceeds to candidate 5, the summary \
         reports '2 signed, 1 skipped', and exactly TWO records exist on the user's own PDS."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-9. GIVEN a 5-candidate list; WHEN --sign 1,1,9; THEN exit \
         non-zero, stderr names BOTH the duplicate index 1 AND the out-of-range index 9 \
         (valid 1..5), no compose preview reached stdout, assert_no_claim_persisted(&env). \
         (A single index --sign 2 behaving identically to SS-1's single sign is covered by \
         SS-1's contract; batch is a superset.)"
    )
}
