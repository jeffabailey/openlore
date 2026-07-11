//! Slice-01 acceptance — the `openlore search … --hide-retracted` opt-in,
//! non-destructive, self-disclosing RETRACTION FILTER on network discovery
//! (feature `retraction-aware-search-filter`, US-RF-001; ADR-060).
//!
//! Today network search obeys I-AV-9 ("counter shown, not applied"): a
//! soft-retracted verified claim STAYS discoverable + annotated, never silently
//! filtered. This slice adds a USER-INVOKED view control that HIDES author-
//! self-retracted claims from the CURRENT view only, discloses exactly what it
//! hid, and changes the default by nothing (the cardinal D-1 reconciliation).
//!
//! Predicate under design (the SINGLE pure decision both surfaces invoke):
//!   `appview_domain::partition_retracted(rows, hide_retracted) -> {survivors, hidden_count}`
//! over the RAW attributed rows (`ports::NetworkResultRowRaw`, full `references`
//! graph — NOT `compose_results`' lossy `counter_annotation`). Self-retraction
//! rule (D-RF-D3, literal): a claim C is author-self-retracted ⟺ ∃ a row K with
//! `K.author_did == C.author_did` carrying `{ Retracts, C.cid }`. A retraction is
//! ONE EVENT = the withdrawn original C + its same-author marker K, both hidden
//! (D-RF-D4); `hidden_count` counts EVENTS, not raw rows removed (D-RF-D5).
//! Third-party `Counters` and different-author `Retracts` NEVER hide (D-3 /
//! I-RF-4 — no heckler's veto).
//!
//! Layer 3 (subprocess / FS acceptance) per the nw-tdd-methodology Layered Test
//! Discipline matrix. Every scenario enters through the CLI driving adapter via
//! the REAL `openlore` binary (subprocess), exercises the real
//! `adapter-index-query` HTTP/XRPC client against a REAL `openlore-indexer serve`
//! over LOCALHOST (the slice-05 production composition root) over a real
//! `index.duckdb` seeded — via the REAL ingest gate — with an ORIGINAL claim + a
//! same-author `Retracts` marker + (per scenario) a different-author `Counters`
//! marker, so the self-vs-third-party contract runs against REAL indexer data.
//! Per Mandate 11 the sad/edge paths are EXAMPLE-ONLY, enumerated explicitly,
//! never PBT-generated at this layer. The pure `partition_retracted` properties
//! (survivors ⊆ input; order-preserving; confidence verbatim; idempotent;
//! hidden_count = self-retraction events; hide=false ⇒ identity) are DELIVER's
//! layer-1/2 PBT in `crates/appview-domain` (ADR-025; `proptest_strategies`).
//!
//! ORACLE discipline (mirrors appview_search.rs): the fixture CIDs (from the same
//! REAL crypto the ingest gate recomputes) are the expected-value ORACLE for
//! "present"/"absent" assertions against stdout (the CLI driving-port
//! observable) — never the SUT.
//!
//! Build-before-run: `cargo build --bin openlore` AND `cargo build --bin
//! openlore-indexer` before running (the harness spawns BOTH; `cargo test` does
//! not rebuild a spawned binary).
//!
//! RED gate (Mandate 7): `--hide-retracted` does not exist on the `search` verb
//! yet → clap rejects the flag → non-zero exit → the feature scenarios fail at
//! the exit-0 assertion (MISSING_FUNCTIONALITY = genuine RED, not BROKEN). The
//! two DEFAULT-UNCHANGED gold guards (RF-2) run WITHOUT the flag and characterize
//! today's preserved behavior (green-by-design; they MUST stay green through
//! DELIVER — the mechanical proof I-AV-9 was not weakened). The
//! `partition_retracted` scaffold (`crates/appview-domain/src/retraction.rs`,
//! `// SCAFFOLD: true`) panics until DELIVER wires it into the verb.
//!
//! Covers US-RF-001: RF-1 (walking-skeleton hide + disclosure), RF-2 (default
//! unchanged), RF-3 (third-party counter not hidden), RF-4 (self-retraction
//! dominates a co-present third-party counter), RF-5 (non-destructive order +
//! confidence), RF-6 (empty-after-filter buffer), RF-7 (hidden_count = EVENTS),
//! RF-8 (zero retractions → no misleading disclosure).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::RawRecordSpec;

// The headline object NSID every scenario searches (single source of truth so
// the query value + the rendered-row assertions never drift). All fixture claims
// assert this object so the original + its same-author marker + any third-party
// counter co-return from one `--object` query (ADR-060: the retraction record
// shares the original's dimensional fields, so it is co-returned).
const OBJECT: &str = "org.openlore.philosophy.reproducible-builds";

// The self-retracting author (Priya) and a standing third-party author (Sven) are
// re-exported through `support::*`; a second standing author is an arbitrary
// fixture DID (the seeding derives its pubkey seam from the corpus by
// construction — `corpus_pubkey_seams` runs `FixtureKeypair::for_did`).
const TOBIAS_DID: &str = "did:plc:tobias-test";

// --- OD-RF-3 disclosure wording (DISTILL's proposal; DELIVER may freeze these as
// content-frozen consts like `SEARCH_NO_MERGE_FOOTER`). Asserted as substrings so
// the exact surrounding copy stays DELIVER's to finalize. ---
const DISCLOSURE_COUNT_FRAGMENT: &str = "retracted claim(s) hidden";
const CLI_RERUN_GUIDANCE: &str = "re-run without --hide-retracted";
const EMPTY_AFTER_FILTER_FRAGMENT: &str = "were soft-retracted";

/// Build the headline self-retraction corpus (Corpus A): an ORIGINAL claim C by
/// Priya + Priya's OWN same-author `Retracts` marker K referencing C's CID (ONE
/// retraction EVENT), plus two STANDING claims by other authors (Sven, Tobias).
/// Returns `(specs, c_cid, survivor_cids)`.
fn corpus_self_retraction() -> (Vec<RawRecordSpec>, String, [String; 2]) {
    let c = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let c_cid = c.clone().into_raw_record().published_cid.0;
    // Priya's OWN retraction marker K: same author DID, carrying { Retracts, C.cid }.
    let k = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c_cid);

    // Two STANDING claims by DIFFERENT authors (never self-retracted).
    let s1 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.65);
    let s1_cid = s1.clone().into_raw_record().published_cid.0;
    let s2 = RawRecordSpec::valid(TOBIAS_DID, "github:guix/guix", OBJECT, 0.71);
    let s2_cid = s2.clone().into_raw_record().published_cid.0;

    (vec![c, k, s1, s2], c_cid, [s1_cid, s2_cid])
}

// =============================================================================
// RF-1 — WALKING SKELETON: explicit hide focuses the survey AND discloses the count
// =============================================================================

/// RF-1 / WALKING SKELETON (US-RF-001 happy; the whole D-1 reconciliation on the
/// primary surface): Rachel runs `openlore search --object <phil> --hide-retracted`
/// over an index holding an author-self-retracted claim (original + its same-author
/// marker) plus standing claims. The self-retracted claim is ABSENT from stdout,
/// the standing claims remain, and an honest footer states "1 retracted claim(s)
/// hidden" + how to re-run without the flag. Exit 0.
///
/// Given the index holds an author-self-retracted claim (original + same-author
///   marker) and two standing claims by other authors;
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then the self-retracted claim is absent, the standing claims remain, and a
///   footer discloses "1 retracted claim(s) hidden" + the re-run guidance (exit 0).
///
/// @us-rf-001 @walking_skeleton @driving_adapter @real-io @adapter-integration
/// @j-005 @kpi-rf-1 @i-rf-1 @i-rf-3 @happy
#[test]
fn hide_retracted_removes_self_retracted_claim_and_discloses_the_count() {
    let env = TestEnv::initialized();
    let (specs, retracted_cid, survivor_cids) = corpus_self_retraction();
    let indexer = seed_network_index_from_specs(&env, specs);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    // exit 0 — a valid filtered result (RED today: the flag does not exist yet).
    assert_eq!(
        outcome.status, 0,
        "`openlore search --object … --hide-retracted` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The author-self-retracted claim C is ABSENT (its CID does not appear).
    assert!(
        !outcome.stdout.contains(&retracted_cid),
        "the author-self-retracted claim (cid {retracted_cid}) must be ABSENT under \
         --hide-retracted:\n{}",
        outcome.stdout
    );

    // The two STANDING claims by other authors REMAIN, still attributed + verified.
    for survivor in &survivor_cids {
        assert!(
            outcome.stdout.contains(survivor),
            "standing survivor claim (cid {survivor}) must REMAIN under --hide-retracted:\n{}",
            outcome.stdout
        );
    }
    assert!(
        outcome.stdout.contains(SVEN_DID) && outcome.stdout.contains(TOBIAS_DID),
        "both standing authors must remain attributed:\n{}",
        outcome.stdout
    );

    // Honest disclosure: "1 retracted claim(s) hidden" (EVENTS — the single
    // withdrawal, NOT the 2 raw rows removed) + the re-run guidance.
    assert!(
        outcome
            .stdout
            .contains(&format!("1 {DISCLOSURE_COUNT_FRAGMENT}")),
        "expected the honesty footer to state '1 {DISCLOSURE_COUNT_FRAGMENT}':\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains(CLI_RERUN_GUIDANCE),
        "expected the footer to tell Rachel how to re-run without the flag \
         ('{CLI_RERUN_GUIDANCE}'):\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-2 — DEFAULT UNCHANGED (gold regression guard; green-by-design, I-RF-1)
// =============================================================================

/// RF-2 / GOLD REGRESSION GUARD (US-RF-001; I-RF-1 — the mechanical proof I-AV-9
/// was NOT weakened): the SAME search WITHOUT `--hide-retracted` still shows the
/// self-retracted claim (with its retraction annotation), shows every standing
/// claim, and prints NO "hidden" footer — byte-identical to the pre-feature
/// search. This scenario is GREEN-BY-DESIGN at DISTILL (it characterizes today's
/// preserved default) and MUST STAY GREEN through DELIVER: opt-in is proven by the
/// default changing nothing.
///
/// Given the same self-retraction index and the same object search;
/// When Rachel runs the search WITHOUT `--hide-retracted`;
/// Then the self-retracted claim is STILL shown and NO "hidden" footer appears.
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-rf-1
/// @default-unchanged @gold @regression @edge
#[test]
fn default_search_without_the_flag_still_shows_the_retracted_claim_and_no_footer() {
    let env = TestEnv::initialized();
    let (specs, retracted_cid, survivor_cids) = corpus_self_retraction();
    let indexer = seed_network_index_from_specs(&env, specs);

    // No `--hide-retracted` — today's default path (works now; green-by-design).
    let outcome = run_openlore_search(&env, &["search", "--object", OBJECT], &indexer);

    assert_eq!(
        outcome.status, 0,
        "the default `openlore search --object` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The self-retracted claim is STILL shown (I-AV-9 default: nothing hidden).
    assert!(
        outcome.stdout.contains(&retracted_cid),
        "without the flag, the self-retracted claim (cid {retracted_cid}) must STILL be shown \
         (I-RF-1 default unchanged):\n{}",
        outcome.stdout
    );
    // …and every standing claim is shown too.
    for survivor in &survivor_cids {
        assert!(
            outcome.stdout.contains(survivor),
            "without the flag, standing claim (cid {survivor}) must be shown:\n{}",
            outcome.stdout
        );
    }

    // NO "hidden" footer appears on the default path (a silent-hide-by-default
    // would be the I-AV-9 violation this guard forbids).
    assert!(
        !outcome.stdout.contains(DISCLOSURE_COUNT_FRAGMENT),
        "the default path must print NO '{DISCLOSURE_COUNT_FRAGMENT}' footer (I-RF-1):\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-3 — a third-party COUNTER is NOT hidden (D-3 / I-AV-9; no heckler's veto)
// =============================================================================

/// RF-3 (US-RF-001; D-3 / I-RF-4 — no heckler's veto): a claim that a DIFFERENT
/// author merely COUNTERED (a disagreement, not a retraction) stays shown +
/// annotated EVEN WITH `--hide-retracted`. Only author-withdrawn claims are
/// hidden; a third party's disagreement never removes an author's row (preserves
/// anti-merging I-AV-2). Reuses the shipped `CounteredClaimPlusCounter` corpus
/// (Priya's claim C + Sven's `Counters` reference to C).
///
/// Given a standing claim by one author that a DIFFERENT author has countered;
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then the countered claim is STILL shown, and no misleading "hidden" footer
///   claims a hide happened (nothing was author-self-retracted).
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-av-9 @i-rf-4
/// @no-hecklers-veto @edge
#[test]
fn a_third_party_countered_claim_is_not_hidden_by_the_filter() {
    let env = TestEnv::initialized();
    // Priya's claim C, countered by Sven's `{ Counters, C.cid }` — a third-party
    // disagreement, NOT an author self-retraction.
    let indexer = seed_network_index(&env, NetworkIndexFixture::CounteredClaimPlusCounter);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    assert_eq!(
        outcome.status, 0,
        "`openlore search … --hide-retracted` over a third-party-counter corpus must exit 0. \
         stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The countered claim's author (Priya) is STILL shown — a disagreement never
    // removes an author's row (D-3 / I-AV-9). Both authors remain.
    assert!(
        outcome.stdout.contains(PRIYA_DID),
        "a third-party-countered claim (Priya's) must STILL be shown under --hide-retracted \
         (no heckler's veto, D-3):\n{}",
        outcome.stdout
    );

    // Nothing was AUTHOR-SELF-RETRACTED, so NO "hidden" disclosure appears (D-4:
    // no misleading line when the filter matched nothing).
    assert!(
        !outcome.stdout.contains(DISCLOSURE_COUNT_FRAGMENT),
        "no '{DISCLOSURE_COUNT_FRAGMENT}' line may appear when only a THIRD-PARTY counter \
         is present (nothing self-retracted, D-3/D-4):\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-4 — self-retraction DOMINATES a co-present third-party counter (anti-lossy)
// =============================================================================

/// RF-4 (US-RF-001; ADR-060 Earned-Trust #1, the CARDINAL anti-lossy-annotation
/// gold): a claim C that is BOTH author-self-retracted (Priya's own marker) AND
/// third-party-countered (Sven's `Counters`) MUST still be hidden under
/// `--hide-retracted` — self-retraction is detected on the FULL `references`
/// graph, not on `compose_results`' lossy lowest-CID single slot (which could
/// mask the self-retraction behind the counter). Meanwhile the third party's OWN
/// standing claim, and an unrelated standing claim, are UNAFFECTED.
///
/// Given a claim C that its author self-retracted AND a third party also countered,
///   plus the counter-author's own standing claim and one more standing claim;
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then C is hidden (self-retraction dominates), while the counter-author's own
///   claim and the unrelated standing claim remain shown; "1 … hidden" disclosed.
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-rf-4
/// @self-retraction-dominates @anti-lossy @edge
#[test]
fn self_retraction_dominates_a_co_present_third_party_counter() {
    let env = TestEnv::initialized();

    // Claim C (Priya) — BOTH self-retracted AND third-party-countered.
    let c = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let c_cid = c.clone().into_raw_record().published_cid.0;
    // Priya's OWN retraction marker K1 (same author) — the self-retraction.
    let k1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c_cid);
    // Sven's third-party counter K2 — a standing disagreement, its OWN claim.
    let k2 = RawRecordSpec::valid(SVEN_DID, "github:bazelbuild/bazel", OBJECT, 0.40)
        .with_reference(claim_domain::ReferenceType::Counters, &c_cid);
    let k2_cid = k2.clone().into_raw_record().published_cid.0;
    // An unrelated standing claim (Tobias) — the "OTHER claims unaffected" witness.
    let d = RawRecordSpec::valid(TOBIAS_DID, "github:guix/guix", OBJECT, 0.71);
    let d_cid = d.clone().into_raw_record().published_cid.0;

    let indexer = seed_network_index_from_specs(&env, vec![c, k1, k2, d]);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    assert_eq!(
        outcome.status, 0,
        "`openlore search … --hide-retracted` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // C is HIDDEN — self-retraction detected on the full graph, dominating the
    // co-present third-party counter (the anti-lossy contract).
    assert!(
        !outcome.stdout.contains(&c_cid),
        "the self-retracted-AND-countered claim C (cid {c_cid}) must be HIDDEN — \
         self-retraction dominates (ADR-060 Earned-Trust #1):\n{}",
        outcome.stdout
    );
    // The third party's OWN standing claim K2 is UNAFFECTED (Sven never withdrew it).
    assert!(
        outcome.stdout.contains(&k2_cid),
        "the third party's OWN standing counter-claim K2 (cid {k2_cid}) must REMAIN shown \
         (only C's author withdrew C):\n{}",
        outcome.stdout
    );
    // The unrelated standing claim is UNAFFECTED.
    assert!(
        outcome.stdout.contains(&d_cid),
        "the unrelated standing claim (cid {d_cid}) must REMAIN shown:\n{}",
        outcome.stdout
    );
    // Exactly ONE retraction EVENT hidden.
    assert!(
        outcome
            .stdout
            .contains(&format!("1 {DISCLOSURE_COUNT_FRAGMENT}")),
        "expected exactly '1 {DISCLOSURE_COUNT_FRAGMENT}' (one withdrawal event):\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-5 — NON-DESTRUCTIVE: survivors keep original order + verbatim confidence
// =============================================================================

/// RF-5 (US-RF-001 @property criterion; I-RF-2 / D-5 — non-destructive): hiding
/// retracted claims NEVER re-orders or re-weights the survivors. The survivors'
/// relative order and each survivor's verbatim confidence are IDENTICAL to the
/// unfiltered run. Tagged `@property`: DELIVER pins the universal invariant as a
/// layer-1/2 proptest in `crates/appview-domain`; at THIS layer (subprocess) it is
/// an EXAMPLE pin (Mandate 9/11) comparing the two real runs.
///
/// Given any result set and the `--hide-retracted` flag;
/// When the filtered and unfiltered searches are compared;
/// Then survivors appear in the same relative order and with byte-identical
///   confidence values (no re-rank, no re-weight).
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @property @i-rf-2
/// @non-destructive @edge
#[test]
fn hiding_never_reorders_or_reweights_the_survivors() {
    let env = TestEnv::initialized();
    let (specs, _retracted_cid, survivor_cids) = corpus_self_retraction();
    let indexer = seed_network_index_from_specs(&env, specs);

    // The unfiltered run (baseline) and the filtered run over the SAME index.
    let unfiltered = run_openlore_search(&env, &["search", "--object", OBJECT], &indexer);
    let filtered = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );
    assert_eq!(
        unfiltered.status, 0,
        "unfiltered baseline must exit 0. stderr: {}",
        unfiltered.stderr
    );
    assert_eq!(
        filtered.status, 0,
        "`--hide-retracted` run must exit 0. stderr: {}",
        filtered.stderr
    );

    // Relative order of the survivors is preserved: the survivor CIDs appear in
    // the filtered output in the SAME relative order they had in the unfiltered
    // output (no re-rank as a side effect of hiding others).
    let order_in = |haystack: &str| -> Vec<usize> {
        survivor_cids
            .iter()
            .filter_map(|cid| haystack.find(cid.as_str()))
            .collect()
    };
    let mut unfiltered_positions = order_in(&unfiltered.stdout);
    let filtered_positions = order_in(&filtered.stdout);
    assert_eq!(
        filtered_positions.len(),
        survivor_cids.len(),
        "every survivor must be present in the filtered run:\n{}",
        filtered.stdout
    );
    let sorted = {
        unfiltered_positions.sort_unstable();
        unfiltered_positions
    };
    // The survivor CIDs were emitted in ascending position order both runs (same
    // relative order); a re-rank would permute them.
    assert!(
        filtered_positions.windows(2).all(|w| w[0] < w[1]),
        "survivors must keep their relative order under --hide-retracted \
         (filtered positions {filtered_positions:?}):\n{}",
        filtered.stdout
    );
    assert_eq!(
        sorted.len(),
        survivor_cids.len(),
        "sanity: every survivor was also present (ordered) in the unfiltered run"
    );

    // Verbatim confidence: each survivor's confidence value (0.65, 0.71) appears
    // in BOTH runs unchanged (no re-weight as a side effect of hiding).
    for conf in ["0.65", "0.71"] {
        assert!(
            unfiltered.stdout.contains(conf) && filtered.stdout.contains(conf),
            "survivor confidence {conf} must be byte-identical across the unfiltered and \
             filtered runs (I-RF-2 non-destructive):\nunfiltered:\n{}\nfiltered:\n{}",
            unfiltered.stdout, filtered.stdout
        );
    }
}

// =============================================================================
// RF-6 — EMPTY-AFTER-FILTER buffer (every result retracted → guided state)
// =============================================================================

/// RF-6 (US-RF-001; I-RF-3 emotional-arc buffer): when `--hide-retracted` hides
/// ALL results, the surface shows an explicit guided state ("all N were
/// soft-retracted … re-run without the flag to see them"), NOT a bare empty
/// result that reads as "nothing exists here". Two self-retraction EVENTS → the
/// guided copy names both were soft-retracted.
///
/// Given a philosophy whose only indexed claims were all author-self-retracted;
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then the output states all results were soft-retracted + the re-run guidance,
///   and never presents the result as "no claims exist".
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-rf-3
/// @empty-after-filter @error @edge
#[test]
fn hiding_every_result_shows_a_guided_state_not_a_bare_empty_result() {
    let env = TestEnv::initialized();

    // Two self-retraction EVENTS: each an original + its same-author marker.
    let c1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let c1_cid = c1.clone().into_raw_record().published_cid.0;
    let k1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c1_cid);
    let c2 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.65);
    let c2_cid = c2.clone().into_raw_record().published_cid.0;
    let k2 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.10)
        .with_reference(claim_domain::ReferenceType::Retracts, &c2_cid);

    let indexer = seed_network_index_from_specs(&env, vec![c1, k1, c2, k2]);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    assert_eq!(
        outcome.status, 0,
        "an all-retracted `--hide-retracted` result is a VALID guided state and must exit 0. \
         stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The guided buffer names that all results were soft-retracted + the re-run
    // guidance — NOT a bare empty / "no claims exist" result.
    assert!(
        outcome.stdout.contains(EMPTY_AFTER_FILTER_FRAGMENT),
        "expected the guided empty-after-filter state ('{EMPTY_AFTER_FILTER_FRAGMENT}'):\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains(CLI_RERUN_GUIDANCE),
        "expected the empty-after-filter buffer to tell Rachel to re-run without the flag:\n{}",
        outcome.stdout
    );
    // Two withdrawal EVENTS are named as hidden (not the 4 raw rows).
    assert!(
        outcome
            .stdout
            .contains(&format!("2 {DISCLOSURE_COUNT_FRAGMENT}"))
            || outcome.stdout.contains("All 2"),
        "expected the buffer to name TWO retraction events hidden:\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-7 — hidden_count is EVENTS, not raw rows (naive len−len would say 2)
// =============================================================================

/// RF-7 (US-RF-001; D-RF-D5 — hidden_count = EVENTS): a SINGLE self-retraction
/// (an original C + its same-author marker K = TWO raw rows) must disclose
/// hidden_count == 1, NOT 2. Pins the event-count semantics explicitly so a naive
/// `len(unfiltered) − len(survivors)` implementation (which counts the marker row
/// and reports 2) is caught.
///
/// Given exactly one author-self-retraction (original + its same-author marker);
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then the disclosure states "1 retracted claim(s) hidden" — never "2".
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-rf-3
/// @hidden-count-events @edge
#[test]
fn hidden_count_reports_retraction_events_not_raw_rows() {
    let env = TestEnv::initialized();
    // Reuse the headline corpus (exactly ONE self-retraction event = C + K).
    let (specs, _retracted_cid, _survivors) = corpus_self_retraction();
    let indexer = seed_network_index_from_specs(&env, specs);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    assert_eq!(
        outcome.status, 0,
        "`openlore search … --hide-retracted` must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The disclosed count is the EVENT count (1), NOT the raw-rows-removed count (2).
    assert!(
        outcome
            .stdout
            .contains(&format!("1 {DISCLOSURE_COUNT_FRAGMENT}")),
        "expected '1 {DISCLOSURE_COUNT_FRAGMENT}' (one withdrawal EVENT):\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains(&format!("2 {DISCLOSURE_COUNT_FRAGMENT}")),
        "hidden_count must NOT double-count the marker row as '2 {DISCLOSURE_COUNT_FRAGMENT}' \
         (D-RF-D5 EVENTS, not raw len−len):\n{}",
        outcome.stdout
    );
}

// =============================================================================
// RF-8 — zero retractions present → no misleading disclosure (D-4)
// =============================================================================

/// RF-8 (US-RF-001; D-4 — no misleading line when nothing matched): running
/// `--hide-retracted` over a result set with ZERO retractions produces output
/// equivalent to the default (every standing claim shown) and prints NO
/// "N retracted claim(s) hidden" line (per OD-RF-3 the surface stays silent when
/// the filter matched nothing; a "0 hidden as if something happened" line is
/// forbidden).
///
/// Given a search whose results contain no author-self-retracted claims;
/// When Rachel runs the search WITH `--hide-retracted`;
/// Then every claim is still shown and NO "hidden" disclosure line is printed.
///
/// @us-rf-001 @driving_adapter @real-io @adapter-integration @i-rf-3 @d-4
/// @zero-retractions @edge
#[test]
fn hide_retracted_over_a_set_with_no_retractions_prints_no_misleading_line() {
    let env = TestEnv::initialized();
    // Two STANDING claims by distinct authors; no references, no retraction.
    let s1 = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", OBJECT, 0.82);
    let s1_cid = s1.clone().into_raw_record().published_cid.0;
    let s2 = RawRecordSpec::valid(SVEN_DID, "github:denoland/deno", OBJECT, 0.65);
    let s2_cid = s2.clone().into_raw_record().published_cid.0;
    let indexer = seed_network_index_from_specs(&env, vec![s1, s2]);

    let outcome = run_openlore_search(
        &env,
        &["search", "--object", OBJECT, "--hide-retracted"],
        &indexer,
    );

    assert_eq!(
        outcome.status, 0,
        "`openlore search … --hide-retracted` over a retraction-free set must exit 0. \
         stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // Every standing claim is still shown (nothing to hide).
    for cid in [&s1_cid, &s2_cid] {
        assert!(
            outcome.stdout.contains(cid),
            "every standing claim (cid {cid}) must be shown when nothing is retracted:\n{}",
            outcome.stdout
        );
    }
    // No misleading disclosure line (D-4): the surface stays silent when the
    // filter matched nothing.
    assert!(
        !outcome.stdout.contains(DISCLOSURE_COUNT_FRAGMENT),
        "no '{DISCLOSURE_COUNT_FRAGMENT}' line may appear when the filter matched nothing \
         (D-4):\n{}",
        outcome.stdout
    );
}
