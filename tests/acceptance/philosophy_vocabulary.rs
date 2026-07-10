//! Slice-22 acceptance — `openlore philosophy list`: discover the shared
//! philosophy vocabulary (US-PV-001, AC-001.1..4) per ADR-059.
//!
//! The user-visible surface for J-002 (discover a shared vocabulary so a
//! classification triangulates instead of stranding on a private string).
//! Before this slice there is NO way to see which philosophies exist — the
//! user invents `org.openlore.philosophy.<something>` and hopes. After it,
//! `openlore philosophy list` prints the ~10 embedded well-known seeds (each a
//! stable object id + name + one-line description), so the user picks a SHARED
//! object for the claim `--object`.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix. Every scenario enters through the CLI driving adapter via
//! the real `openlore` binary (subprocess), exercises the real `lexicon`
//! embedded-seed set (ADR-059 D3 — `include_str!` constants, no signer, no
//! store, no network), and (per Mandate 11) is EXAMPLE-ONLY — sad paths are
//! enumerated explicitly, never PBT-generated. The pure `validate_philosophy_json`
//! accept/reject arms are pinned at layer 2 in `crates/lexicon/src/lib.rs`.
//!
//! READ-ONLY slice: NO scenario writes or signs a record. The list verb reads
//! the embedded seeds; offline by construction (AC-001.4).
//!
//! RED TODAY: the `philosophy` subcommand does not exist, so clap rejects the
//! args and the process exits non-zero. Every scenario asserts `status == 0`
//! FIRST and then on the EXPECTED business output (the seed object ids / names /
//! descriptions), so the failure is MISSING_FUNCTIONALITY (no `philosophy list`
//! verb + no seeds), never a harness/import error. BUILD-BEFORE-RUN: the AT
//! spawns the real `openlore` bin (built by `cargo build --bin openlore`), not
//! rebuilt by `cargo test`.
//!
//! Covers:
//! - US-PV-001 / AC-001.1: list prints each seed's object id + name + description
//! - US-PV-001 / AC-001.2 (KPI-PV-1): ≥10 well-known seeds, each a valid record
//! - US-PV-001 / AC-001.1 (ADR-059 D1): derived object id is backward-compatible
//!   with the slice-01 claim `object` bytes (`org.openlore.philosophy.<name>`)
//! - US-PV-001 / AC-001.3: `--json` emits the vocabulary as JSON (opt-in)
//! - US-PV-001 / AC-001.3: text is the DEFAULT (JSON is strictly opt-in)
//! - US-PV-001 / AC-001.4 (I-9): listing succeeds with the network disabled
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use std::collections::HashSet;

/// The six well-known philosophies AC-001.2 REQUIRES the seed set to contain.
/// The slice ships ≥10 total (DELIVER adds ≥4 more, e.g. immutability,
/// composition-over-inheritance, local-first, backwards-compatibility); these
/// tests hard-pin ONLY the six named-required seeds + the ≥10 count, leaving
/// DELIVER free to choose the remaining curated records.
const NAMED_WELL_KNOWN: &[&str] = &[
    "memory-safety",
    "type-safety",
    "test-driven",
    "documentation-first",
    "dependency-pinning",
    "semantic-versioning",
];

const NSID_PREFIX: &str = "org.openlore.philosophy.";

/// Collect the DISTINCT `org.openlore.philosophy.*` object-id tokens present in
/// a `philosophy list` stdout (whitespace-delimited, trailing punctuation
/// trimmed). The port-exposed observable for the "how many philosophies did the
/// user see" universe slot.
fn distinct_object_ids(stdout: &str) -> HashSet<String> {
    stdout
        .split_whitespace()
        .filter(|token| token.starts_with(NSID_PREFIX))
        .map(|token| {
            token
                .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '.')
                .to_string()
        })
        .filter(|id| id.len() > NSID_PREFIX.len())
        .collect()
}

// =============================================================================
// US-PV-001 — discover the shared philosophy vocabulary (`philosophy list`)
// =============================================================================

/// PV-1 (US-PV-001 happy; WALKING SKELETON for slice-22, AC-001.1): from an
/// initialized store the user runs `openlore philosophy list`. It prints each
/// seeded philosophy as a greppable block carrying its stable object id
/// (`org.openlore.philosophy.<name>`), its human name, and a one-line
/// description — so the user can copy an EXACT shared object for a claim
/// instead of inventing a private string. This is the thin end-to-end discovery
/// skeleton (embedded seeds → CLI driving adapter → user-visible stdout).
///
/// GIVEN an initialized store,
/// WHEN the user runs `openlore philosophy list`,
/// THEN it exits 0 and prints each of the six well-known seeds' object id, name,
///      and a non-empty one-line description, one per greppable block.
///
/// @us-pv-001 @driving_port @real-io @walking_skeleton @j-002 @kpi-pv-2 @happy
#[test]
fn philosophy_list_prints_each_seed_object_id_name_and_description() {
    let env = TestEnv::initialized();

    // Action: the discovery read through the CLI driving port. No store writes,
    // no network — the verb reads the embedded seed constants (ADR-059 D3).
    let outcome = run_openlore(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observable surface of the `philosophy list` text
    // view, all asserted against stdout — the CLI driving-port observable):
    //   cli.philosophy_list.object_ids_present   — each well-known seed's derived id
    //   cli.philosophy_list.description_prose     — each block carries a name + one-liner
    let stdout = &outcome.stdout;

    // 1. Each of the six well-known seeds renders its stable, greppable object id
    //    `org.openlore.philosophy.<name>` — the exact string the user copies into
    //    a claim `--object` so the classification triangulates.
    for name in NAMED_WELL_KNOWN {
        let object_id = format!("{NSID_PREFIX}{name}");
        assert!(
            stdout.contains(&object_id),
            "philosophy list must print the seed object id {object_id} (greppable, one per block);\n\
             --- stdout ---\n{stdout}"
        );
    }

    // 2. Each block is more than a bare id: the human name + a one-line
    //    description render alongside it (AC-001.1). Format-tolerant check — strip
    //    every well-known object id, then assert substantial descriptive prose
    //    survives (the names + one-line definitions). If the verb printed ONLY
    //    ids (no descriptions), this trips.
    let mut prose = stdout.to_string();
    for name in NAMED_WELL_KNOWN {
        prose = prose.replace(&format!("{NSID_PREFIX}{name}"), "");
    }
    let prose_alpha = prose.chars().filter(|c| c.is_alphabetic()).count();
    assert!(
        prose_alpha >= 120,
        "each seeded philosophy must render its name + a one-line description alongside its object \
         id (AC-001.1, greppable block); found only {prose_alpha} prose chars beyond the object \
         ids — the list looks id-only;\n--- stdout ---\n{stdout}"
    );
}

/// PV-2 (US-PV-001 happy, AC-001.2 / KPI-PV-1): the seed set is a real
/// vocabulary, not a token pair. Running `openlore philosophy list` surfaces at
/// least ten DISTINCT philosophy object ids, and every one of the six named
/// well-known philosophies AC-001.2 requires is among them.
///
/// GIVEN the embedded seed set,
/// WHEN the user runs `openlore philosophy list`,
/// THEN the output carries ≥10 distinct `org.openlore.philosophy.*` object ids,
///      including all six named well-known philosophies.
///
/// @us-pv-001 @driving_port @real-io @j-002 @kpi-pv-1 @happy
#[test]
fn the_seed_set_contains_at_least_ten_well_known_philosophies() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list.distinct_object_id_count (≥10 — the
    // vocabulary is a real set), cli.philosophy_list.named_well_known_present
    // (all six required names appear). Asserted against stdout.
    let stdout = &outcome.stdout;

    let ids = distinct_object_ids(stdout);
    assert!(
        ids.len() >= 10,
        "AC-001.2/KPI-PV-1: the seed set must contain at least 10 distinct philosophies; \
         `philosophy list` surfaced only {} distinct object id(s): {ids:?};\n--- stdout ---\n{stdout}",
        ids.len()
    );

    for name in NAMED_WELL_KNOWN {
        let object_id = format!("{NSID_PREFIX}{name}");
        assert!(
            ids.contains(&object_id),
            "AC-001.2: the seed set must include the well-known philosophy {object_id};\n\
             surfaced ids: {ids:?};\n--- stdout ---\n{stdout}"
        );
    }
}

/// PV-3 (US-PV-001, AC-001.1 / ADR-059 D1 backward-compat): the derived object
/// id is EXACTLY `org.openlore.philosophy.<normalize(name)>` in dotted,
/// lowercase form — byte-identical to the object strings slice-01 claims already
/// signed (e.g. `org.openlore.philosophy.memory-safety`). It is the join between
/// the claim graph and the vocabulary, so a drift (`:` separator, CamelCase, an
/// `id` field with a different shape) would silently break triangulation.
///
/// GIVEN the embedded seeds,
/// WHEN the user runs `openlore philosophy list`,
/// THEN each well-known id renders in the exact dotted-lowercase form, and no
///      CamelCased / mis-separated variant appears.
///
/// @us-pv-001 @driving_port @real-io @j-002 @backward-compat @edge
#[test]
fn each_seed_object_id_matches_the_slice_one_claim_object_bytes() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list.object_id_form (exact dotted-lowercase id per
    // seed), cli.philosophy_list.no_drift_variant (no CamelCase / `:`-separated
    // variant). Asserted against stdout.
    let stdout = &outcome.stdout;

    for name in NAMED_WELL_KNOWN {
        let canonical = format!("{NSID_PREFIX}{name}"); // e.g. org.openlore.philosophy.memory-safety
        assert!(
            stdout.contains(&canonical),
            "the derived object id must be the exact dotted-lowercase form {canonical} \
             (backward-compatible with slice-01 claim `object` bytes; ADR-059 D1);\n\
             --- stdout ---\n{stdout}"
        );

        // Guard against a normalize/render drift that would fork the join key:
        // a colon-separated NSID variant must NOT appear.
        let colon_variant = format!("org.openlore.philosophy:{name}");
        assert!(
            !stdout.contains(&colon_variant),
            "the object id must use the dotted NSID form, never a `:`-separated variant \
             ({colon_variant}) — a drift would strand the claim-graph join;\n--- stdout ---\n{stdout}"
        );
    }
}

/// PV-4 (US-PV-001, AC-001.3): the machine-readable view. `openlore philosophy
/// list --json` emits the vocabulary as a JSON array; every element is a valid
/// record with a non-empty `name` and `description` (AC-001.2 "each a valid
/// record"), and all six well-known names are present — so a script can consume
/// the shared vocabulary, not just a human.
///
/// GIVEN the `--json` flag,
/// WHEN the user runs `openlore philosophy list --json`,
/// THEN stdout parses as a JSON array of ≥10 records, each carrying a non-empty
///      name + description, including the six well-known philosophies.
///
/// @us-pv-001 @driving_port @real-io @j-002 @json @happy
#[test]
fn philosophy_list_json_emits_each_record_with_name_and_description() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "list", "--json"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list --json must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list_json.parses (stdout is valid JSON),
    // cli.philosophy_list_json.record_count (≥10), cli.philosophy_list_json.
    // each_record_has_name_and_description (structural validity), cli.
    // philosophy_list_json.named_well_known_present. Asserted against parsed JSON.
    let parsed: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap_or_else(|err| {
        panic!(
            "`philosophy list --json` stdout must be valid JSON (AC-001.3); parse error: {err};\n\
             --- stdout ---\n{}",
            outcome.stdout
        )
    });

    let records = parsed.as_array().unwrap_or_else(|| {
        panic!(
            "`philosophy list --json` must emit a JSON ARRAY of records;\n--- stdout ---\n{}",
            outcome.stdout
        )
    });
    assert!(
        records.len() >= 10,
        "AC-001.2/KPI-PV-1: the JSON vocabulary must carry ≥10 records; got {};\n--- stdout ---\n{}",
        records.len(),
        outcome.stdout
    );

    // Every record is a VALID record: non-empty `name` + `description` present
    // (AC-001.2 completes the same contract `validate_philosophy_json` pins at
    // layer 2). Collect the names for the well-known presence check.
    let mut names: HashSet<String> = HashSet::new();
    for record in records {
        let name = record
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "each JSON record must carry a string `name` (valid record);\n\
                     --- record ---\n{record}\n--- stdout ---\n{}",
                    outcome.stdout
                )
            });
        let description = record
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "each JSON record must carry a string `description` (valid record);\n\
                     --- record ---\n{record}\n--- stdout ---\n{}",
                    outcome.stdout
                )
            });
        assert!(
            !name.trim().is_empty() && !description.trim().is_empty(),
            "each JSON record's name + description must be non-empty (AC-001.2 valid record);\n\
             --- record ---\n{record}"
        );
        names.insert(name.to_string());
    }

    for name in NAMED_WELL_KNOWN {
        assert!(
            names.contains(*name),
            "AC-001.3: the JSON vocabulary must include the well-known philosophy {name:?};\n\
             surfaced names: {names:?};\n--- stdout ---\n{}",
            outcome.stdout
        );
    }
}

/// PV-5 (US-PV-001 edge, AC-001.3): text is the DEFAULT; JSON is strictly
/// opt-in (P-001 ux_guardrail). Running `openlore philosophy list` WITHOUT
/// `--json` emits the human-readable text view — its stdout is NOT a JSON array
/// (so a user piping to a pager sees prose, and a script only gets JSON when it
/// asks).
///
/// GIVEN no `--json` flag,
/// WHEN the user runs `openlore philosophy list`,
/// THEN stdout renders the human text view and does NOT parse as a JSON array.
///
/// @us-pv-001 @driving_port @real-io @j-002 @text-default @edge
#[test]
fn philosophy_list_defaults_to_human_text_not_json() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list.text_default (default stdout is human text,
    // NOT a JSON array — JSON is opt-in). Asserted against stdout.
    let as_json = serde_json::from_str::<serde_json::Value>(&outcome.stdout);
    let is_json_array = matches!(as_json, Ok(serde_json::Value::Array(_)));
    assert!(
        !is_json_array,
        "AC-001.3: the DEFAULT `philosophy list` (no --json) must be the human text view, not a \
         JSON array — JSON is strictly opt-in;\n--- stdout ---\n{}",
        outcome.stdout
    );

    // And the human view still surfaces the vocabulary (a well-known id renders).
    assert!(
        outcome.stdout.contains(&format!("{NSID_PREFIX}memory-safety")),
        "the default text view must still surface the vocabulary (e.g. \
         {NSID_PREFIX}memory-safety);\n--- stdout ---\n{}",
        outcome.stdout
    );
}

/// PV-7 (slice-31, US-PV-001 alias discoverability): `philosophy list` surfaces
/// each seed's alias strings — the shorthand `philosophy show` now resolves
/// (slice-30) — so a user browsing the vocabulary sees which strings map onto a
/// philosophy without having to open each record. The list carries an `aliases:`
/// label and the unambiguous `mem-safety` alias of `memory-safety`.
///
/// GIVEN the embedded seeds,
/// WHEN the user runs `openlore philosophy list`,
/// THEN stdout carries an `aliases:` label and the `mem-safety` alias string.
///
/// @us-pv-001 @driving_port @real-io @j-002 @alias @happy
#[test]
fn philosophy_list_surfaces_seed_aliases() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list.aliases_labelled (the list labels the alias
    // strings it surfaces), cli.philosophy_list.alias_present (an unambiguous
    // alias renders). `mem-safety` is not a substring of any name/description, so
    // its presence proves the alias line, not incidental prose.
    let stdout = &outcome.stdout;
    assert!(
        stdout.contains("aliases:"),
        "philosophy list must label the aliases it surfaces (alias discoverability, slice-31);\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("mem-safety"),
        "philosophy list must surface the `mem-safety` alias of memory-safety (the shorthand \
         `philosophy show` resolves);\n--- stdout ---\n{stdout}"
    );
}

/// PV-6 (US-PV-001 edge, AC-001.4 / I-9): discovery is LOCAL/offline. The
/// embedded seeds are compiled into the binary (ADR-059 D3), so `philosophy
/// list` must render the full vocabulary with the network disabled — no socket,
/// no PDS, no peer. Local-first by construction.
///
/// GIVEN the network is disabled,
/// WHEN the user runs `openlore philosophy list`,
/// THEN it still exits 0 and renders the full vocabulary (the six well-known
///      seeds), and NO outbound PDS call is attempted.
///
/// @us-pv-001 @driving_port @real-io @j-002 @local-first @i-9 @edge
#[test]
fn philosophy_list_succeeds_with_the_network_disabled() {
    let env = TestEnv::initialized();

    // Run the list with the per-process network-disabled seam engaged (no
    // PDS/peer endpoint reachable). A read of embedded seeds must still succeed.
    let outcome = run_openlore_network_disabled(&env, &["philosophy", "list"]);
    assert_eq!(
        outcome.status, 0,
        "openlore philosophy list must succeed with the network disabled (AC-001.4 local-first);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe: cli.philosophy_list.object_ids_present (the FULL vocabulary
    // renders from the embedded seeds with no network), pds.create_record.
    // call_count (0 — no outbound call attempted). Asserted against stdout + the
    // fake PDS call recorder.
    let stdout = &outcome.stdout;
    for name in NAMED_WELL_KNOWN {
        let object_id = format!("{NSID_PREFIX}{name}");
        assert!(
            stdout.contains(&object_id),
            "the network-disabled list must render the full vocabulary from the embedded seeds \
             (missing {object_id});\n--- stdout ---\n{stdout}"
        );
    }

    // I-9 local-first: NO outbound PDS call was attempted — a pure LOCAL read.
    assert_no_pds_call_was_made(&env);
}
