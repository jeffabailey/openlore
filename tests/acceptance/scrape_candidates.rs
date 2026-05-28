//! Slice-02 acceptance — derived candidate-claim rendering + auditability.
//!
//! After harvest, the CLI runs the PURE `scraper-domain` derivation and
//! renders a numbered candidate list. This file pins the OBSERVABLE
//! candidate-list contract at the CLI driving port (layer 3, subprocess,
//! example-only per Mandate 11): every candidate names its exact source
//! signal (auditability — gate `candidate_names_source_signal`, KPI-SCR-3),
//! every candidate's confidence is the conservative 0.25 default and never
//! above 0.3 (gate `candidate_confidence_no_autoinflate`, KPI-SCR-2), and
//! multiple signals for one predicate collapse into a single candidate
//! (I-SCR-4).
//!
//! The EXHAUSTIVE derivation properties (determinism, source-signals
//! non-empty, confidence == 0.25, collapse) are exercised as `@property`
//! tests at layer 2 in `scraper_domain.rs` — here we pin the user-visible
//! rendering of those properties through the real CLI.
//!
//! Covers:
//! - US-SCR-002: derive auditable candidate claims from signals
//! - WD-52 / I-SCR-3: confidence 0.25, never auto-inflate, never above 0.3
//! - WD-53 / I-SCR-4: every candidate names its source signal
//! - US-SCR-002 Ex 4: multi-signal-one-predicate collapse
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SCR-002 — auditability (gate candidate_names_source_signal; KPI-SCR-3)
// =============================================================================

/// SC-1 (gate `candidate_names_source_signal`, KPI-SCR-3 — load-bearing):
/// every rendered candidate names the EXACT public signal that produced it.
/// A candidate the user cannot trace to a signal is unauditable and breaks
/// J-004b; this is the auditability guardrail.
///
/// Given Maria has harvested 5 matching public signals from rust-lang/cargo;
/// When the CLI renders the candidate list; Then each of the 5 candidates
/// names the exact public signal that produced it (5 distinct source-signal
/// lines, one per candidate).
///
/// @us-scr-002 @real-io @driving_port @j-004b @kpi-scr-3 @happy @release-gate
#[test]
fn scrape_candidates_each_names_its_exact_source_signal() {
    // GIVEN an initialized env + a public repo serving the 5 canonical public
    // signals (one per jobs.yaml mapping entry → 5 derived candidates).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN Maria scrapes the public repo (no --sign — this is a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    assert_eq!(
        outcome.status, 0,
        "scrape must exit 0 on the happy path; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN each of the 5 numbered candidates NAMES the exact public signal that
    // produced it (auditability — gate candidate_names_source_signal, KPI-SCR-3
    // / I-SCR-4). The renderer emits, per candidate:
    //
    //   [N] <predicate>  <object>
    //       from signal : <signal value>
    //       confidence  : ...
    //
    // so the originating signal substring MUST appear on a `from signal :` line
    // WITHIN candidate N's block (between the `[N]` marker and the `[N+1]`
    // marker). Each expected substring is a distinct fragment of its fixture
    // signal's detail string — a candidate the user cannot trace back to its
    // signal is unauditable and breaks J-004b.
    let expected: &[(usize, &str)] = &[
        (1, "Cargo.lock committed"),
        (2, "docs/"),
        (3, "test/source ratio 0.61"),
        (4, "CHANGELOG"),
        (5, "Rust"),
    ];

    let stdout = &outcome.stdout;
    for &(number, signal_substring) in expected {
        // Slice candidate N's block: from its `[N]` marker up to the next
        // candidate's `[N+1]` marker (or end of output for the last one).
        let start = stdout.find(&format!("[{number}]")).unwrap_or_else(|| {
            panic!(
                "expected a numbered candidate [{number}] in the candidate list; \
                 \n--- stdout ---\n{stdout}"
            )
        });
        let rest = &stdout[start..];
        let block_end = rest[1..]
            .find(&format!("[{}]", number + 1))
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let block = &rest[..block_end];

        // The originating signal substring appears on a `from signal :` line
        // INSIDE this candidate's block — the auditability contract.
        let names_signal = block
            .lines()
            .filter(|line| line.contains("from signal :"))
            .any(|line| line.contains(signal_substring));
        assert!(
            names_signal,
            "candidate [{number}] must name its exact source signal on a \
             `from signal :` line containing {signal_substring:?} (auditability — \
             candidate_names_source_signal, KPI-SCR-3); \
             \n--- candidate [{number}] block ---\n{block}\n--- full stdout ---\n{stdout}"
        );
    }
}

/// SC-2: the candidate-list footer states that NOTHING is a claim until the
/// user signs it (the in-control reassurance beat; WD-49). The footer is
/// always rendered when >=1 candidate is shown.
///
/// Given a harvested target with >=1 matching signal; When the candidate
/// list renders; Then the footer states nothing is a claim until the user
/// signs it, and points at `--sign N` as the next step.
///
/// @us-scr-002 @real-io @driving_port @j-004b @happy
#[test]
fn scrape_candidates_footer_states_nothing_is_signed_until_user_signs() {
    // GIVEN an initialized env + a public repo serving >=1 matching public
    // signal (the five canonical cargo signals → >=1 derived candidate, so the
    // footer is always rendered).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read; nothing is
    // signed or published).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    assert_eq!(
        outcome.status, 0,
        "scrape must exit 0 on the happy path; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN the candidate-list footer carries the human-gate reassurance beat
    // (WD-49 / I-SCR-1): it states nothing is a claim until the user signs it
    // AND points at `--sign N` as the next step. Both fragments are
    // content-frozen UX copy (render::NOTHING_IS_A_CLAIM_FOOTER) — an
    // example-based substring assertion pins the exact literal; property-framing
    // would add no coverage over a fixed string. The footer is always rendered
    // when >=1 candidate is shown.
    let stdout = &outcome.stdout;
    assert!(
        stdout.contains("nothing is a claim until you sign it"),
        "candidate-list footer must state nothing is a claim until the user signs it \
         (human-gate reassurance, WD-49 / I-SCR-1); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
    assert!(
        stdout.contains("--sign N"),
        "candidate-list footer must name `--sign N` as the next step (the human-gate \
         affordance — WD-49); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

// =============================================================================
// US-SCR-002 — confidence (gate candidate_confidence_no_autoinflate; KPI-SCR-2)
// =============================================================================

/// SC-3 (gate `candidate_confidence_no_autoinflate`, KPI-SCR-2 — the
/// proposal-time half): EVERY rendered candidate's confidence is 0.25,
/// displayed as "speculative", and NO candidate is proposed above 0.3. The
/// scraper has weak evidence (one public signal) so the conservative
/// default forces the human to consciously raise confidence (WD-52 / WD-10).
///
/// Given any target has been harvested; When the CLI renders candidates;
/// Then every candidate's proposed confidence is 0.25 (speculative) and no
/// candidate is proposed with a confidence above 0.3.
///
/// @us-scr-002 @real-io @driving_port @j-004b @wd-52 @kpi-scr-2 @happy
#[test]
fn scrape_candidates_all_default_to_speculative_quarter_confidence() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SC-3 — candidate_confidence_no_autoinflate (proposal half). \
         WHEN scrape github rust-lang/cargo; THEN assert_candidate_confidence(&outcome, 0.25, \
         \"speculative\") for EVERY candidate AND assert no candidate displays a confidence \
         above 0.3."
    )
}

/// SC-4 (US-SCR-002 Ex 4; I-SCR-4): when THREE distinct signals all map to
/// the `documentation-first` predicate, exactly ONE candidate is rendered
/// and its source-signal line lists all three contributing signals (no
/// near-duplicate candidates). The collapse is the PURE derivation's job;
/// here we pin its user-visible rendering.
///
/// Given a target with a docs/ directory AND a 400-line README AND high
/// doc-comment density; When the CLI derives candidates; Then exactly one
/// documentation-first candidate is shown and its source-signal line lists
/// all three contributing signals.
///
/// @us-scr-002 @real-io @driving_port @j-004b @i-scr-4 @edge
#[test]
fn scrape_candidates_collapse_multiple_signals_for_one_predicate_into_one() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SC-4. GIVEN \
         FakeGithub::with_multi_signal_single_predicate(\"some-org/well-documented\") backed \
         by fixture_three_docs_signals_one_predicate(); WHEN scrape github \
         some-org/well-documented; THEN exactly ONE documentation-first candidate is \
         rendered and its source-signal line lists all THREE contributing signals."
    )
}

/// SC-5: a candidate the user disagrees with is fully auditable and
/// rejectable WITHOUT signing — because the candidate named its source
/// signal, the user can see WHY it was proposed and simply not select it
/// (US-SCR-002 Ex 3). Not selecting persists nothing. This proves the
/// human-in-the-loop is real: a proposal can be reviewed and dropped, never
/// auto-asserted.
///
/// Given a candidate [1] dependency-pinning is proposed from "Cargo.lock
/// committed"; When the user reviews it and does NOT run `--sign`; Then the
/// derivation named the source signal (so the user could audit it) and zero
/// claims are persisted.
///
/// @us-scr-002 @real-io @driving_port @j-004b @kpi-scr-3 @edge
#[test]
fn scrape_candidates_disagreed_candidate_is_auditable_and_persists_nothing_when_unsigned() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SC-5. WHEN scrape github <repo> with a dependency-pinning \
         candidate; THEN that candidate's source-signal line names 'Cargo.lock committed' \
         (the user can audit the derivation) AND, with no --sign, \
         assert_no_claim_persisted(&env) — a reviewed-and-rejected proposal is never \
         asserted."
    )
}
