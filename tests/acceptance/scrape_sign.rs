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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-1 — scraper_reuses_slice01_publish_path gate (I-SCR-6) + \
         KPI-SCR-1. GIVEN FakeGithub::for_public_repo(\"rust-lang/cargo\", \
         fixture_cargo_five_signals()); WHEN run_openlore_scrape_with_stdin(&env, \
         &[\"scrape\",\"github\",\"rust-lang/cargo\",\"--sign\",\"1\"], \
         \"\\n\\n\\n\\n0.55\\n\\nY\\n\") (accept subject/predicate/object/evidence, type 0.55 \
         for confidence, Enter to sign, Y to publish); THEN exit 0, \
         assert_compose_preview_contains_not_as_truth(&outcome), stdout shows '0.55 \
         (weighted)' AND a display-only 'derived-from: openlore-github-scraper (signal: \
         Cargo.lock committed)' line, assert_scraper_reuses_slice01_publish_path(&env, \
         &cid) (exactly ONE record on the user's OWN PDS via the slice-01 path; no parallel \
         path), the signed payload records confidence 0.55, and the success message mentions \
         `openlore claim retract`."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-2 — candidate_confidence_no_autoinflate (sign half). WHEN \
         --sign 3 with stdin accepting every field (Enter x5, Enter sign, Y publish); THEN \
         assert_candidate_confidence_unchanged(&env, &cid, 0.25) AND the signed payload's \
         subject/predicate/object/evidence equal candidate 3's PROPOSED values byte-for-byte \
         (no auto-inflation, KPI-SCR-5 zero-edit sign)."
    )
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
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SS-3. Sign a scraper candidate with fields F (provenance line \
         shown), AND hand-author a claim via `claim add` with the SAME fields F; THEN the two \
         CIDs are EQUAL, and the scraper claim's on-disk claims/<cid>.json contains NO \
         `derived-from`/`derivedFrom` key (provenance is display-only; the signed shape is \
         byte-identical to a hand-authored claim — WD-62 / I-SCR-7)."
    )
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
