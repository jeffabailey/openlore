//! Slice-24 acceptance — `openlore philosophy add`: compose → SIGN → persist a
//! NEW `org.openlore.philosophy` record (US-PV-003, AC-003.1..4) per ADR-059 §5
//! slice-24. This is the FIRST philosophy slice that WRITES and SIGNS.
//!
//! Slices 22 (seed + list) and 23 (show) are READ-ONLY over the ~12 embedded
//! seeds. Slice-24 lets a user MINT vocabulary the seed set does not carry:
//! `openlore philosophy add --name <n> --description <d> [--alias <a>...]
//! [--see-also <url>...]` composes an `org.openlore.philosophy` record, signs it
//! (reusing `claim_domain::{canonicalize, compute_cid, sign}` — ADR-006, NO new
//! signing model), and persists it locally as a signed `<cid>.json` artifact
//! under `<root>/philosophies/` plus a `philosophies` table row (schema_v4). The
//! mint mirrors `claim add`: local-first (NO write before the user confirms the
//! sign prompt), publish deferrable, author DID recorded. It prints the derived
//! object id `org.openlore.philosophy.<normalize(name)>`.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix. Every scenario enters through the CLI driving adapter via
//! the real `openlore` binary (subprocess), signs via the production composition
//! root, and persists to the real local store under an isolated `OPENLORE_HOME`.
//! Per Mandate 11 the sad paths (seed collision, empty description) are
//! EXAMPLE-ONLY — enumerated explicitly, never PBT-generated at this layer.
//!
//! Assertions are format-TOLERANT: they scan the observable CLI surface (stdout
//! / stderr / exit code) and the on-disk signed artifact, and NEVER couple to a
//! `lexicon` struct field. The novel philosophy name/id/aliases are hard-pinned
//! constants (stable test vocabulary); the persisted-artifact assertions read
//! the JSON as text so DELIVER stays free to choose the exact serialization.
//!
//! RED TODAY: slice-22 shipped the `philosophy` parent verb (with `list`) and
//! slice-23 added `show`, but `add` is NOT yet a recognized subcommand of
//! `philosophy`, so clap rejects the args (`unrecognized subcommand 'add'`) and
//! the process exits 2. PA-1 / PA-2 / PA-5 assert `status == 0` FIRST and then on
//! the EXPECTED business output (the derived object id / the signed artifact);
//! PA-3 / PA-4 assert `status != 0` FIRST (exit 2 satisfies that) and then on the
//! plain guidance / named-field error that does NOT exist at exit 2. So every
//! failure is MISSING_FUNCTIONALITY (no `philosophy add` verb, no compose / sign
//! / persist, no collision guard, no empty-description rejection), never a
//! harness / import error. BUILD-BEFORE-RUN: the AT spawns the real `openlore`
//! bin (built by `cargo build --bin openlore`), not rebuilt by `cargo test`.
//!
//! Covers:
//! - US-PV-003 / AC-003.1 + AC-003.2 (WS): `add <novel>` composes → signs →
//!   persists a `<cid>.json` artifact and prints the derived object id
//! - US-PV-003 / AC-003.2 (local-first): empty stdin = clean cancel; preview
//!   shown, NO artifact written, NO PDS call (mirrors `claim add`'s no-write beat)
//! - US-PV-003 / AC-003.3: a name colliding with a seed is REFUSED with plain
//!   guidance (names the collision + hints `--alias` onto the existing one)
//! - US-PV-003 / AC-003.4: an empty `--description` is rejected with a named-field
//!   error (mentions `description`), NEVER a panic / backtrace
//! - US-PV-003 / AC-003.2 (author DID): a successful mint records the author DID
//!   in the signed artifact
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use std::path::PathBuf;

// -----------------------------------------------------------------------------
// Stable pins for slice-24. The novel philosophy is one the ~12 embedded seeds
// do NOT carry (so the mint has genuinely new work to do); its derived object id
// is `org.openlore.philosophy.<normalize(name)>`. We hard-pin the name, the
// derived id, the aliases, and the seeAlso link (stable test vocabulary), but
// read the persisted artifact as TEXT rather than pinning its serialization —
// DELIVER stays free to choose the signed-record layout.
// -----------------------------------------------------------------------------

/// A philosophy the embedded seed set does NOT contain (verified against
/// `crates/lexicon/src/seeds.json`: memory-safety, type-safety, test-driven,
/// documentation-first, dependency-pinning, semantic-versioning,
/// reproducible-builds, local-first, backward-compatibility, immutability,
/// federation-first, minimalism — `capability-security` is absent).
const NEW_NAME: &str = "capability-security";

/// The derived object id the mint MUST print (AC-003.1). Byte-identical to
/// `lexicon::object_id("capability-security")` =
/// `org.openlore.philosophy.` + `normalize("capability-security")`.
const NEW_OBJECT_ID: &str = "org.openlore.philosophy.capability-security";

/// A substantial description for the novel philosophy (AC-003.1 composes
/// name + description into the signed record). Non-empty by construction — the
/// empty-description rejection is exercised separately by PA-4.
const NEW_DESCRIPTION: &str = "Grant each component only the minimum authority it \
    needs to do its job, carried as an unforgeable reference, so a compromised part \
    cannot exceed its explicitly delegated reach.";

/// The `--alias` strings that triangulate onto the novel philosophy (AC-003.1
/// composes aliases into the record).
const NEW_ALIASES: &[&str] = &["ocap", "cap-sec"];

/// The `--see-also` link for the novel philosophy (AC-003.1 composes seeAlso).
const NEW_SEE_ALSO: &str = "https://en.wikipedia.org/wiki/Capability-based_security";

/// A name that COLLIDES with a shipped slice-01 seed (AC-003.3). Minting it again
/// would duplicate an existing object id, so the verb must refuse it.
const SEED_COLLISION_NAME: &str = "memory-safety";

/// Panic / stack-trace markers that MUST NOT leak on the sad paths (AC-003.4
/// "no panic — completes the RED scaffold"). A plain, actionable message only.
const PANIC_MARKERS: &[&str] = &["panicked", "RUST_BACKTRACE", "stack backtrace", "note: run with"];

/// The full flag set for minting the novel philosophy (name + description +
/// both aliases + seeAlso). Shared by the happy-path scenarios (PA-1, PA-2, PA-5)
/// so they compose the SAME record — only the stdin confirmation differs
/// (Pillar 2: chained narrative — the `Given` of the mint is reused, not
/// copy-pasted with drift).
fn mint_novel_philosophy_args() -> Vec<&'static str> {
    vec![
        "philosophy",
        "add",
        "--name",
        NEW_NAME,
        "--description",
        NEW_DESCRIPTION,
        "--alias",
        NEW_ALIASES[0],
        "--alias",
        NEW_ALIASES[1],
        "--see-also",
        NEW_SEE_ALSO,
    ]
}

/// The local philosophies store directory: `<root>/philosophies/` where
/// `<root>` = `{home}/.local/share/openlore` (mirrors `TestEnv::claims_dir`,
/// per architecture-design.md §4.5). Computed inline so the shared `support`
/// harness stays untouched (no new harness fn for a single-slice observable).
fn philosophies_dir(env: &TestEnv) -> PathBuf {
    env.home
        .join(".local")
        .join("share")
        .join("openlore")
        .join("philosophies")
}

/// The signed `<cid>.json` philosophy artifacts on disk (port-exposed
/// observable: `storage.local_philosophy_store.files`). Empty when the dir is
/// absent — so a canceled / refused / invalid mint that never created the dir
/// reads as "zero artifacts", exactly the no-write proof PA-2/PA-3/PA-4 need.
fn signed_philosophy_artifacts(env: &TestEnv) -> Vec<PathBuf> {
    let dir = philosophies_dir(env);
    if !dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect()
}

// =============================================================================
// US-PV-003 — mint a philosophy (`philosophy add`)
// =============================================================================

/// PA-1 (US-PV-003 happy; WALKING SKELETON for slice-24, AC-003.1 + AC-003.2):
/// from an initialized store the user runs `openlore philosophy add` for a novel
/// philosophy the seed set does not carry, confirms the sign prompt (Enter), and
/// defers publish (`n` — publish is deferrable, mirroring `claim add`). It exits
/// 0, prints the derived object id `org.openlore.philosophy.capability-security`,
/// AND leaves exactly one signed `<cid>.json` artifact under `philosophies/`.
/// This is the thin end-to-end mint skeleton (typed flags → CLI driving adapter →
/// compose → sign → local signed artifact + printed id).
///
/// GIVEN an initialized store and a novel philosophy name + description,
/// WHEN the user runs `openlore philosophy add ...` and presses Enter to sign,
/// THEN it exits 0, prints the derived object id, and writes exactly one signed
///      `<cid>.json` artifact under the local philosophies store.
///
/// @walking_skeleton @driving_port @real-io @us-pv-003 @j-001 @happy
#[test]
fn philosophy_add_mints_signs_and_persists_a_new_record_printing_the_object_id() {
    let env = TestEnv::initialized();

    // Action: mint through the CLI driving port. `\nn\n` = <Enter> to sign
    // locally, then `n` to defer publish — the same two-prompt confirmation
    // shape `claim add` uses (AC-003.2: no new signing model, publish deferrable).
    let outcome = run_openlore_with_stdin(&env, &mint_novel_philosophy_args(), "\nn\n");

    assert_eq!(
        outcome.status, 0,
        "openlore philosophy add (sign confirmed) must exit 0;\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables):
    //   cli.philosophy_add.object_id_printed      — the derived object id renders on stdout
    //   storage.local_philosophy_store.file_count — exactly one signed <cid>.json artifact
    assert!(
        outcome.stdout.contains(NEW_OBJECT_ID),
        "philosophy add must print the derived object id {NEW_OBJECT_ID:?} (AC-003.1);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    let artifacts = signed_philosophy_artifacts(&env);
    assert_eq!(
        artifacts.len(),
        1,
        "philosophy add must persist exactly one signed <cid>.json artifact under {} \
         (AC-003.1); found {:?}",
        philosophies_dir(&env).display(),
        artifacts
    );
}

/// PA-2 (US-PV-003 local-first, AC-003.2): the mint is LOCAL-FIRST — nothing is
/// signed or written before the user confirms. Running the SAME mint but closing
/// stdin without pressing Enter (empty stdin = EOF at the sign prompt) is a clean
/// cancel: the compose preview is shown, the process exits 0, and NO signed
/// artifact is written (and — since the artifact and the `philosophies` row are
/// written together atomically per architecture-design.md §4.5 — no row either),
/// and NO outbound PDS call is attempted. Mirrors `claim add`'s no-write-before-
/// confirm beat (walking_skeleton.rs WS-3).
///
/// GIVEN an initialized store and a novel philosophy composed for minting,
/// WHEN the user runs `openlore philosophy add ...` but cancels (empty stdin),
/// THEN it exits 0 with the preview shown, writes NO signed artifact, and makes
///      NO PDS call.
///
/// @driving_port @real-io @us-pv-003 @j-001 @local-first @edge
#[test]
fn philosophy_add_with_empty_stdin_cancels_cleanly_without_writing() {
    let env = TestEnv::initialized();

    // Empty stdin = EOF at the sign prompt = clean cancel (the exact no-write
    // seam `claim add` uses; walking_skeleton.rs WS-3).
    let outcome = run_openlore_with_stdin(&env, &mint_novel_philosophy_args(), "");

    assert_eq!(
        outcome.status, 0,
        "a canceled philosophy add (empty stdin) must exit 0 cleanly (AC-003.2 local-first);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe:
    //   cli.philosophy_add.preview_shown             — the compose preview names what would be minted
    //   storage.local_philosophy_store.file_count    — 0 (no write before confirm)
    //   pds.create_record.call_count                 — 0 (no outbound call)
    assert!(
        outcome.stdout.contains(NEW_NAME),
        "the compose preview must be shown before the sign prompt — it should name the \
         philosophy being minted ({NEW_NAME:?}) (AC-003.2);\n--- stdout ---\n{}",
        outcome.stdout
    );

    let artifacts = signed_philosophy_artifacts(&env);
    assert!(
        artifacts.is_empty(),
        "a canceled philosophy add must write NO signed artifact (local-first; artifact + \
         `philosophies` row are written atomically, so no artifact ⟹ no row); found {:?} under {}",
        artifacts,
        philosophies_dir(&env).display()
    );

    assert_no_pds_call_was_made(&env);
}

/// PA-3 (US-PV-003 sad path, AC-003.3): a name that COLLIDES with a shipped seed
/// must be refused. Running `openlore philosophy add --name memory-safety ...`
/// (a slice-01 seed) exits NON-ZERO and prints plain guidance that names the
/// collision and hints the recovery (use the existing one, or `--alias` onto it)
/// — never a silent duplicate id, never a panic. The refusal is a PRE-CHECK
/// against the seed set that runs BEFORE signing, so no stdin confirmation is
/// needed and NO record is persisted.
///
/// GIVEN a philosophy name equal to a shipped seed,
/// WHEN the user runs `openlore philosophy add --name memory-safety ...`,
/// THEN it exits non-zero, prints plain guidance naming the collision + hinting
///      `--alias`, writes NO artifact, and leaks no panic / backtrace.
///
/// @driving_port @real-io @us-pv-003 @j-001 @collision @error @sad
#[test]
fn philosophy_add_refuses_a_name_that_collides_with_a_seed() {
    let env = TestEnv::initialized();

    // The seed collision is refused before any sign prompt, so `run_openlore`
    // (stdin null) is sufficient — no confirmation is ever solicited.
    let outcome = run_openlore(
        &env,
        &[
            "philosophy",
            "add",
            "--name",
            SEED_COLLISION_NAME,
            "--description",
            NEW_DESCRIPTION,
        ],
    );

    // Universe:
    //   cli.philosophy_add.collision_exit_status  — non-zero
    //   cli.philosophy_add.collision_guidance     — names the collision + hints --alias
    //   cli.philosophy_add.no_panic_leak          — no stack-trace markers
    //   storage.local_philosophy_store.file_count — 0 (no duplicate persisted)
    assert_ne!(
        outcome.status, 0,
        "a name colliding with a seed must exit NON-ZERO (AC-003.3);\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);

    for marker in PANIC_MARKERS {
        assert!(
            !combined.contains(marker),
            "the seed-collision path must NOT leak a panic / stack trace (AC-003.3); found the \
             marker {marker:?};\n--- combined output ---\n{combined}"
        );
    }

    assert!(
        combined.contains(SEED_COLLISION_NAME),
        "the collision guidance must NAME the colliding philosophy ({SEED_COLLISION_NAME:?}) so \
         the user knows what already exists (AC-003.3);\n--- combined output ---\n{combined}"
    );

    let lower = combined.to_lowercase();
    assert!(
        lower.contains("exist"),
        "the collision guidance must say the philosophy already exists (AC-003.3);\n\
         --- combined output ---\n{combined}"
    );

    assert!(
        combined.contains("--alias"),
        "the collision guidance must hint `--alias` onto the existing philosophy (AC-003.3);\n\
         --- combined output ---\n{combined}"
    );

    let artifacts = signed_philosophy_artifacts(&env);
    assert!(
        artifacts.is_empty(),
        "a refused seed-collision mint must persist NO duplicate record (AC-003.3); found {:?} \
         under {}",
        artifacts,
        philosophies_dir(&env).display()
    );
}

/// PA-4 (US-PV-003 invalid-record path, AC-003.4): the composed record must pass
/// `validate_philosophy_json` before it is signed. Running `openlore philosophy
/// add --name capability-security --description ""` (an EMPTY description) is
/// rejected with a named-field error that mentions the `description` field — and
/// NEVER a Rust panic / backtrace (which would leak internals and read as a
/// crash, not a handled validation rejection). This completes the RED scaffold
/// for the invalid-record path. The rejection runs before signing, so nothing is
/// persisted.
///
/// GIVEN a novel name but an EMPTY `--description`,
/// WHEN the user runs `openlore philosophy add --name capability-security --description ""`,
/// THEN it exits non-zero with an error naming the `description` field, leaks no
///      panic / backtrace, and writes NO artifact.
///
/// @driving_port @real-io @us-pv-003 @j-001 @invalid @error @sad
#[test]
fn philosophy_add_empty_description_is_rejected_with_a_named_field_error() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(
        &env,
        &[
            "philosophy",
            "add",
            "--name",
            NEW_NAME,
            "--description",
            "", // empty — must be rejected by validate_philosophy_json (AC-003.4)
        ],
    );

    // Universe:
    //   cli.philosophy_add.invalid_exit_status     — non-zero
    //   cli.philosophy_add.named_field_error        — the error names the `description` field
    //   cli.philosophy_add.no_panic_leak            — no stack-trace markers (no panic)
    //   storage.local_philosophy_store.file_count   — 0 (invalid record never persisted)
    assert_ne!(
        outcome.status, 0,
        "an empty --description must exit NON-ZERO (AC-003.4);\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let combined = format!("{}\n{}", outcome.stdout, outcome.stderr);

    for marker in PANIC_MARKERS {
        assert!(
            !combined.contains(marker),
            "the invalid-record path must NOT panic / leak a backtrace (AC-003.4 \"no panic\"); \
             found the marker {marker:?};\n--- combined output ---\n{combined}"
        );
    }

    let lower = combined.to_lowercase();
    assert!(
        lower.contains("description"),
        "the invalid-record error must NAME the offending `description` field (AC-003.4);\n\
         --- combined output ---\n{combined}"
    );

    let artifacts = signed_philosophy_artifacts(&env);
    assert!(
        artifacts.is_empty(),
        "an invalid mint must persist NO record (AC-003.4); found {:?} under {}",
        artifacts,
        philosophies_dir(&env).display()
    );
}

/// PA-5 (US-PV-003 happy, AC-003.2 author DID): a successful mint records the
/// author DID. No CLI read surface exposes a MINTED philosophy's author yet
/// (`philosophy list` / `philosophy show` read the embedded seeds only, per
/// slice-22/23 — a freshly minted philosophy is not in the seed set), so the
/// author DID is asserted on the on-disk signed artifact JSON (mirrors
/// walking_skeleton.rs WS-6's signed-file read). This pins AC-003.2's
/// "author DID recorded" beat: the signing envelope carries the author's DID.
///
/// GIVEN an initialized store as `did:plc:test-jeff`,
/// WHEN the user mints a novel philosophy and signs it,
/// THEN the persisted signed artifact records the author DID.
///
/// @driving_port @real-io @us-pv-003 @j-001 @author-did @happy
#[test]
fn philosophy_add_records_the_author_did_in_the_signed_artifact() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(&env, &mint_novel_philosophy_args(), "\nn\n");

    assert_eq!(
        outcome.status, 0,
        "philosophy add (sign confirmed) must exit 0 before we can inspect the artifact;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe:
    //   storage.local_philosophy_store.file_count       — exactly one signed artifact
    //   storage.local_philosophy_store.artifact_author  — the artifact records the author DID
    let artifacts = signed_philosophy_artifacts(&env);
    assert_eq!(
        artifacts.len(),
        1,
        "expected exactly one signed philosophy artifact under {}; found {:?}",
        philosophies_dir(&env).display(),
        artifacts
    );

    let json = std::fs::read_to_string(&artifacts[0])
        .unwrap_or_else(|e| panic!("read signed philosophy artifact {}: {e}", artifacts[0].display()));

    // The author DID this scenario signed as (`did:plc:test-jeff`), read from the
    // identity port rather than hard-coded — stays port-derived.
    let author_did = env.identity.author_did();
    assert!(
        json.contains(author_did),
        "the signed philosophy artifact must record the author DID {author_did:?} (AC-003.2);\n\
         --- artifact ---\n{json}"
    );
}
