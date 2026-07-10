//! Slice-23 acceptance — `openlore philosophy show <name-or-object>`: inspect
//! ONE philosophy in full (US-PV-002, AC-002.1..2) per ADR-059 §5 slice-23.
//!
//! The user-visible surface for J-002 (decide whether a philosophy fits before
//! classifying against it). Slice-22 shipped `philosophy list` — the user can
//! now SEE the ~12 embedded seeds. But `list` is a one-liner per seed; before
//! this slice there is NO way to read a philosophy's FULL definition, the alias
//! strings that triangulate onto it, or its see-also links. After it,
//! `openlore philosophy show memory-safety` prints the name, the full
//! description verbatim, `aliases: [mem-safety, memory-safe]`, and the seeAlso
//! link — so the user confirms this is the right philosophy (and which alias
//! strings resolve) before copying its exact object into a claim.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix. Every scenario enters through the CLI driving adapter via
//! the real `openlore` binary (subprocess), exercises the real `lexicon`
//! embedded-seed set (ADR-059 D3 — `include_str!` constants, no signer, no
//! store, no network), and (per Mandate 11) is EXAMPLE-ONLY — the unknown-name
//! sad path is enumerated explicitly, never PBT-generated at this layer.
//!
//! READ-ONLY slice: NO scenario writes or signs a record. `show` reads the
//! embedded seeds; offline by construction (mirrors AC-001.4 for `list`).
//! Minting (`philosophy add`) is slice-24 — OUT of scope, so `show` reads only
//! the 12 embedded seeds. Accepts EITHER a bare name (`memory-safety`) OR the
//! full derived object id (`org.openlore.philosophy.memory-safety`).
//!
//! RED TODAY: the `philosophy` subcommand exists (slice-22 shipped `list`), but
//! `show` is NOT a recognized subcommand of `philosophy`, so clap rejects the
//! args (`unrecognized subcommand 'show'`) and the process exits 2. PS-1/PS-2/PS-4
//! assert `status == 0` FIRST and then on the EXPECTED business output (the seed
//! name / description / aliases / seeAlso); PS-3 asserts the plain unknown-name
//! guidance substrings (absent at exit 2). So every failure is
//! MISSING_FUNCTIONALITY (no `philosophy show` verb), never a harness/import
//! error. BUILD-BEFORE-RUN: the AT spawns the real `openlore` bin (built by
//! `cargo build --bin openlore`), not rebuilt by `cargo test`.
//!
//! Covers:
//! - US-PV-002 / AC-002.1 (WS): `show <name>` prints name + full description +
//!   aliases + seeAlso verbatim from the record
//! - US-PV-002 / AC-002.1: `show <object-id>` renders the SAME record (name-or-object)
//! - US-PV-002 / AC-002.2: an UNKNOWN name exits non-zero with plain guidance
//!   (naming the miss + hinting `philosophy list`/`philosophy add`), NEVER a panic
//! - US-PV-002 / AC-002.1 (local-first): `show` renders the record with the
//!   network disabled and attempts no outbound PDS call
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// -----------------------------------------------------------------------------
// Stable pins for the `memory-safety` well-known seed (slice-22 shipped seed
// content; these fields are frozen vocabulary, safe to hard-pin like slice-22
// hard-pinned the six well-known names). We hard-pin the aliases, the seeAlso
// link, and two VERBATIM description fragments — NOT the exact description prose
// nor the layout — so DELIVER stays free to choose the `show` rendering format.
// -----------------------------------------------------------------------------

const NAME: &str = "memory-safety";
const OBJECT_ID: &str = "org.openlore.philosophy.memory-safety";

/// Aliases the `memory-safety` seed carries (US-PV-002 elevator pitch:
/// `aliases: [mem-safety, memory-safe]`). `mem-safety` is the load-bearing pin
/// (unambiguous — not a substring of the name); `memory-safe` is included for
/// completeness (it is a prefix of the name, so a weaker signal).
const ALIASES: &[&str] = &["mem-safety", "memory-safe"];

/// The seeAlso link on the `memory-safety` seed.
const SEE_ALSO: &str = "https://en.wikipedia.org/wiki/Memory_safety";

/// Two VERBATIM fragments of the `memory-safety` description. Pinning two
/// distinct in-prose fragments proves the FULL description rendered verbatim
/// (AC-002.1) without over-pinning the exact string or whitespace/layout.
const DESC_FRAGMENTS: &[&str] = &["use-after-free", "buffer overruns"];

/// Panic / stack-trace markers that MUST NOT leak on the unknown-name sad path
/// (AC-002.2 "never a stack trace"). A plain, actionable message only.
const PANIC_MARKERS: &[&str] = &["panicked", "RUST_BACKTRACE", "stack backtrace", "note: run with"];

/// Assert the combined `show memory-safety` output renders the FULL record:
/// name + verbatim description fragments + both aliases + the seeAlso link.
/// Format-tolerant (scans the observable stdout; does not pin layout).
fn assert_renders_memory_safety_record(stdout: &str) {
    assert!(
        stdout.contains(NAME),
        "philosophy show must print the philosophy name {NAME:?};\n--- stdout ---\n{stdout}"
    );
    for fragment in DESC_FRAGMENTS {
        assert!(
            stdout.contains(fragment),
            "philosophy show must print the FULL description verbatim (AC-002.1); missing the \
             in-prose fragment {fragment:?};\n--- stdout ---\n{stdout}"
        );
    }
    for alias in ALIASES {
        assert!(
            stdout.contains(alias),
            "philosophy show must print the record's aliases verbatim (AC-002.1); missing alias \
             {alias:?};\n--- stdout ---\n{stdout}"
        );
    }
    assert!(
        stdout.contains(SEE_ALSO),
        "philosophy show must print the record's seeAlso link verbatim (AC-002.1); missing \
         {SEE_ALSO:?};\n--- stdout ---\n{stdout}"
    );
}

// =============================================================================
// US-PV-002 — inspect one philosophy (`philosophy show <name-or-object>`)
// =============================================================================

/// PS-1 (US-PV-002 happy, BY NAME; WALKING SKELETON for slice-23, AC-002.1):
/// from an initialized store the user runs `openlore philosophy show
/// memory-safety`. It prints the philosophy's name, its FULL description
/// (verbatim), the alias strings that triangulate onto it
/// (`aliases: [mem-safety, memory-safe]`), and its seeAlso link — so the user
/// confirms this is the right philosophy (and which aliases resolve) before
/// classifying. This is the thin end-to-end inspection skeleton (embedded seed →
/// CLI driving adapter → user-visible stdout).
///
/// GIVEN an initialized store,
/// WHEN the user runs `openlore philosophy show memory-safety`,
/// THEN it exits 0 and prints the name, the full description (verbatim
///      fragments), both aliases, and the seeAlso link.
///
/// @us-pv-002 @driving_port @real-io @walking_skeleton @j-002 @kpi-pv-2 @happy
#[test]
fn philosophy_show_by_name_prints_name_description_aliases_and_see_also() {
    let env = TestEnv::initialized();

    // Action: the inspection read through the CLI driving port. No store writes,
    // no network — the verb reads the embedded seed constants (ADR-059 D3).
    let outcome = run_openlore(&env, &["philosophy", "show", "memory-safety"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy show <name> must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observable surface of the `philosophy show` text
    // view, all asserted against stdout — the CLI driving-port observable):
    //   cli.philosophy_show.name_present        — the philosophy name renders
    //   cli.philosophy_show.description_verbatim — full description (verbatim fragments)
    //   cli.philosophy_show.aliases_present      — both alias strings render
    //   cli.philosophy_show.see_also_present     — the seeAlso link renders
    assert_renders_memory_safety_record(&outcome.stdout);
}

/// PS-2 (US-PV-002 happy, BY OBJECT ID, AC-002.1): `show` accepts EITHER a bare
/// name OR the full derived object id. Running `openlore philosophy show
/// org.openlore.philosophy.memory-safety` renders the SAME record as `show
/// memory-safety` — proving the name-or-object acceptance the AC requires (the
/// object id is exactly the string the user copied out of `philosophy list` or a
/// claim, so `show` must resolve it back to the record).
///
/// GIVEN the embedded seeds,
/// WHEN the user runs `openlore philosophy show org.openlore.philosophy.memory-safety`,
/// THEN it exits 0 and renders the SAME record (name + description + aliases + seeAlso).
///
/// @us-pv-002 @driving_port @real-io @j-002 @name-or-object @happy
#[test]
fn philosophy_show_by_object_id_renders_the_same_record() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "show", OBJECT_ID]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy show <object-id> must exit 0 (name-or-object acceptance, AC-002.1);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_show.resolves_object_id (the full object id
    // resolves to the same record a bare name resolves to). Asserted against
    // stdout — the same observable record surface as PS-1.
    assert_renders_memory_safety_record(&outcome.stdout);
}

/// PS-3 (US-PV-002 sad path, AC-002.2): an UNKNOWN philosophy name must fail
/// gracefully. Running `openlore philosophy show no-such-philosophy-xyz` exits
/// NON-ZERO and prints a plain, actionable guidance message that names the miss
/// and hints the recovery verbs (`philosophy list` / `philosophy add`) — and
/// NEVER a Rust stack trace / panic (which would leak internals and read as a
/// crash, not a handled "no such philosophy").
///
/// GIVEN an unknown philosophy name,
/// WHEN the user runs `openlore philosophy show no-such-philosophy-xyz`,
/// THEN it exits non-zero, prints plain guidance naming the miss + hinting
///      `philosophy list`/`philosophy add`, with NO panic / backtrace markers.
///
/// @us-pv-002 @driving_port @real-io @j-002 @unknown @error @sad
#[test]
fn philosophy_show_unknown_name_exits_non_zero_with_plain_guidance() {
    let env = TestEnv::initialized();

    let missing = "no-such-philosophy-xyz";
    let outcome = run_openlore(&env, &["philosophy", "show", missing]);

    // Universe: cli.philosophy_show.unknown_exit_status (non-zero), cli.
    // philosophy_show.unknown_guidance (names the miss + hints list/add), cli.
    // philosophy_show.no_panic_leak (no stack-trace markers). Asserted against
    // the combined stdout+stderr observable.
    assert_ne!(
        outcome.status, 0,
        "an unknown philosophy name must exit NON-ZERO (AC-002.2);\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Guidance may be emitted on stdout or stderr — scan the combined surface.
    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);

    // 1. NEVER a stack trace / panic (AC-002.2 "never a stack trace").
    for marker in PANIC_MARKERS {
        assert!(
            !combined.contains(marker),
            "the unknown-name path must NOT leak a panic / stack trace (AC-002.2); found the \
             marker {marker:?} — this reads as a crash, not a handled miss;\n\
             --- combined output ---\n{combined}"
        );
    }

    // 2. Plain guidance names the missed input so the user knows WHAT was not found.
    assert!(
        combined.contains(missing),
        "the unknown-name guidance must name the miss ({missing:?}) so the user knows what was \
         not found;\n--- combined output ---\n{combined}"
    );

    // 3. Plain guidance mentions "no such philosophy" and hints the recovery verbs
    //    `philosophy list` (discover the vocabulary) and `philosophy add` (mint it).
    let lower = combined.to_lowercase();
    assert!(
        lower.contains("no such philosophy"),
        "the unknown-name guidance must say plainly \"no such philosophy\" (AC-002.2);\n\
         --- combined output ---\n{combined}"
    );
    assert!(
        combined.contains("philosophy list"),
        "the unknown-name guidance must hint `philosophy list` (discover the vocabulary; \
         AC-002.2);\n--- combined output ---\n{combined}"
    );
    assert!(
        combined.contains("philosophy add"),
        "the unknown-name guidance must hint `philosophy add` (mint the missing philosophy; \
         AC-002.2);\n--- combined output ---\n{combined}"
    );
}

/// slice-30 (US-PV-002, name-or-object-OR-ALIAS): `show` accepts an ALIAS string
/// too. Running `openlore philosophy show mem-safety` — where `mem-safety` is an
/// alias of `memory-safety`, not its canonical name or object id — resolves to
/// and renders the SAME canonical `memory-safety` record. This closes the gap the
/// slice-23 skeleton left: a user who copied an alias (the strings `show` itself
/// advertises under `aliases:`) can inspect the philosophy it triangulates onto.
///
/// GIVEN the embedded seeds,
/// WHEN the user runs `openlore philosophy show mem-safety` (an alias),
/// THEN it exits 0 and renders the canonical `memory-safety` record
///      (name + description + aliases + seeAlso).
///
/// @us-pv-002 @driving_port @real-io @j-002 @alias @happy
#[test]
fn philosophy_show_by_alias_renders_the_canonical_record() {
    let env = TestEnv::initialized();

    // `mem-safety` is the load-bearing alias pin (ALIASES[0]) — unambiguous, not a
    // substring of the canonical name. It is neither `NAME` nor `OBJECT_ID`, so a
    // hit proves alias resolution, not name/object-id resolution.
    let outcome = run_openlore(&env, &["philosophy", "show", ALIASES[0]]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy show <alias> must exit 0 (alias resolution, slice-30);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_show.resolves_alias (an alias resolves to the same
    // canonical record a bare name / object id resolves to). Asserted against the
    // same observable record surface as PS-1/PS-2.
    assert_renders_memory_safety_record(&outcome.stdout);
}

/// PS-4 (US-PV-002 edge, AC-002.1 local-first): inspection is LOCAL/offline. The
/// embedded seeds are compiled into the binary (ADR-059 D3), so `philosophy
/// show` must render the full record with the network disabled — no socket, no
/// PDS, no peer. Local-first by construction (mirrors AC-001.4 for `list`).
///
/// GIVEN the network is disabled,
/// WHEN the user runs `openlore philosophy show memory-safety`,
/// THEN it still exits 0 and renders the full record (name + description +
///      aliases + seeAlso), and NO outbound PDS call is attempted.
///
/// @us-pv-002 @driving_port @real-io @j-002 @local-first @i-9 @edge
#[test]
fn philosophy_show_succeeds_with_the_network_disabled() {
    let env = TestEnv::initialized();

    // Run `show` with the per-process network-disabled seam engaged (no PDS/peer
    // endpoint reachable). A read of embedded seeds must still succeed.
    let outcome = run_openlore_network_disabled(&env, &["philosophy", "show", "memory-safety"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy show must succeed with the network disabled (AC-002.1 local-first);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_show.* (the FULL record renders from the embedded
    // seeds with no network), pds.create_record.call_count (0 — no outbound call
    // attempted). Asserted against stdout + the fake PDS call recorder.
    assert_renders_memory_safety_record(&outcome.stdout);

    // I-9 local-first: NO outbound PDS call was attempted — a pure LOCAL read.
    assert_no_pds_call_was_made(&env);
}
