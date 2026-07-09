//! Slice-25 acceptance — `openlore claim add` COMPOSE ADVISORY (US-PV-004,
//! AC-004.1..3) per ADR-059 §5 row 25: "cli (`claim_add` preview); lexicon
//! (`VocabularyIndex`) | One advisory line; signed bytes byte-unchanged
//! (AC-004.3)".
//!
//! `claim add` already composes → previews → SIGNS → persists a claim (slice-01
//! walking skeleton, SHIPPED). Slice-25 adds ONE display-only ADVISORY line to
//! the compose preview for the `--object`:
//!   - a KNOWN philosophy object          → `↳ resolves to <canonical>`
//!   - an object matching a seed ALIAS    → `↳ resolves to <canonical> (alias)`
//!   - an in-namespace but UNKNOWN object → `⚠ not a known philosophy — will be
//!                                            signed as-is`  (NON-BLOCKING)
//!   - an object OUTSIDE the philosophy namespace → NO advisory (no nagging;
//!                                            the slice-01 preview is unchanged).
//! The advisory is a nudge, NEVER a gate: an unknown philosophy object STILL
//! signs unchanged if the user confirms (AC-004.2, D3 "claims not truth"), and
//! the resolution is LOCAL/offline and does NOT alter the signed payload — the
//! object bytes the user TYPED are exactly what gets signed (AC-004.3). The
//! alias-aware resolution is a small PURE `lexicon` seam (`VocabularyIndex` over
//! `seeds()` + each seed's `aliases` + `normalize`/`object_id`). The read-time
//! aggregation of stored claims under a canonical (in `graph query`/`score`) is
//! slice-26 — OUT of scope here.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline. Every scenario enters through the CLI driving adapter via the real
//! `openlore` binary (subprocess), composing/signing through the production
//! composition root and persisting to the real local store under an isolated
//! `OPENLORE_HOME`. Per Mandate 11 the sad/edge paths (unknown object,
//! out-of-namespace) are EXAMPLE-ONLY — enumerated explicitly, never
//! PBT-generated at this layer (the alias-aware `VocabularyIndex` resolver is
//! exhaustively property-tested at layer 1 in `crates/lexicon` by DELIVER).
//!
//! Assertions are format-TOLERANT: they scan the observable CLI surface (stdout /
//! exit code) and the on-disk signed artifact, and NEVER couple to a `lexicon`
//! struct field. The advisory-line WORDING/LAYOUT is DELIVER's to choose; these
//! tests pin only the domain SUBSTRINGS the ACs themselves fix ("resolves to",
//! "alias", "not a known philosophy" — see feature-delta.md US-PV-004 / the
//! `ADVISORY_*` constants below). The objects/canonical/alias are hard-pinned
//! constants verified against `crates/lexicon/src/seeds.json` (stable test
//! vocabulary); the persisted-artifact assertions read the JSON as TEXT so
//! DELIVER stays free to choose the signed-record serialization.
//!
//! RED TODAY: `claim add` composes/signs/persists fine, but today's
//! `render_compose_preview` (claim_add.rs) prints NO advisory line — the preview
//! has no "resolves to" / "alias" / "not a known philosophy" text. So the
//! advisory-substring assertions (CA-1/CA-2/CA-3/CA-4) FAIL against today's
//! output = MISSING_FUNCTIONALITY, never a harness/import error (the file imports
//! only the `support` harness + `std`, no new production symbol). The
//! byte-parity guarantee (AC-004.3) ALREADY holds today (claim add signs the
//! object verbatim), so CA-4's parity assertions PASS and the missing advisory is
//! the RED driver; CA-3's exit-0 + persist PASS and the missing warning is the
//! RED driver. CA-5 (out-of-namespace) is GREEN-TODAY: no advisory fires on a
//! non-philosophy object today, and DELIVER must keep it that way — an
//! over-firing regression guard. BUILD-BEFORE-RUN: the AT spawns the real
//! `openlore` bin (built by `cargo build --bin openlore`), not rebuilt by
//! `cargo test`.
//!
//! Covers:
//! - US-PV-004 / AC-004.1 (WS): a KNOWN philosophy object → the preview shows the
//!   resolution advisory naming the canonical; on confirm, signs + persists.
//! - US-PV-004 / AC-004.1 (alias): an ALIAS object → the preview resolves it to
//!   the canonical AND marks it an alias resolution.
//! - US-PV-004 / AC-004.2 (unknown, NON-BLOCKING): an in-namespace unknown object
//!   → the preview shows the non-blocking warning AND, on confirm, the claim STILL
//!   signs + persists unchanged (never rejects).
//! - US-PV-004 / AC-004.3 (byte-parity, LOAD-BEARING): the signed `<cid>.json`
//!   object bytes are EXACTLY the user-typed `--object` — the advisory (even the
//!   tempting alias case) does NOT rewrite the payload to the canonical.
//! - US-PV-004 no-regression: an out-of-namespace `--object` → NO advisory line;
//!   the slice-01 preview is byte-unchanged (no nagging on non-philosophy claims).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use std::path::PathBuf;

// -----------------------------------------------------------------------------
// Stable pins for slice-25. Objects/canonical/alias are verified against
// `crates/lexicon/src/seeds.json`: the `memory-safety` seed carries the aliases
// `["mem-safety", "memory-safe"]`. We hard-pin the objects but scan the
// OBSERVABLE preview text (never a `lexicon` struct) and read the persisted
// artifact as TEXT — DELIVER owns the advisory wording + the record layout.
// -----------------------------------------------------------------------------

/// A KNOWN philosophy object: the derived object id of the `memory-safety` seed
/// (AC-004.1 "known"). Byte-identical to `lexicon::object_id("memory-safety")`.
const KNOWN_OBJECT: &str = "org.openlore.philosophy.memory-safety";

/// The canonical philosophy name the advisory must NAME. For the alias case
/// (`ALIAS_OBJECT`) this string is NOT a substring of the object line the preview
/// already prints (`org.openlore.philosophy.mem-safety` does not contain
/// "memory-safety"), so asserting the preview shows it proves the advisory
/// surfaced the CANONICAL — not merely echoed the typed object.
const CANONICAL_NAME: &str = "memory-safety";

/// An ALIAS object (AC-004.1 "alias"): `mem-safety` is a real `aliases` entry of
/// the `memory-safety` seed (seeds.json line 5), so this in-namespace object
/// resolves — via the alias, NOT via a bare-name/object-id match — to the
/// canonical `memory-safety`. `find()` today matches only name/object-id (NOT
/// aliases), so the alias-aware resolution is genuinely new slice-25 work.
const ALIAS_OBJECT: &str = "org.openlore.philosophy.mem-safety";

/// An object IN the philosophy namespace that is NEITHER a known philosophy NOR
/// any seed's alias (AC-004.2 "unknown"). Verified absent from seeds.json (no
/// seed name/alias normalizes to `not-a-real-one`). The advisory must warn
/// (non-blocking) — and the claim must STILL sign if confirmed.
const UNKNOWN_OBJECT: &str = "org.openlore.philosophy.not-a-real-one";

/// An ordinary claim object OUTSIDE the `org.openlore.philosophy.*` namespace
/// (AC-004 no-regression). A `--object` like this must draw NO advisory line —
/// the advisory targets philosophy objects only; non-philosophy claims keep the
/// slice-01 preview verbatim (no nagging).
const NONPHILOSOPHY_OBJECT: &str = "github:rust-lang/rust";

// --- Advisory SUBSTRINGS (the DELIVER wording contract; format-tolerant) -------
// These are the domain tokens the ACs / feature-delta.md US-PV-004 fix verbatim
// (`↳ resolves to memory-safety (alias)` / `⚠ not a known philosophy — will be
// signed as-is`). Matched case-insensitively so DELIVER owns capitalization +
// glyphs (↳ / ⚠) + surrounding layout. If DELIVER chooses different wording, it
// updates these three constants — they are the ONLY layout coupling.

/// The resolution advisory phrase for a known/alias object (AC-004.1).
const ADVISORY_RESOLVES: &str = "resolves to";
/// The alias-resolution marker distinguishing an alias hit from a direct hit
/// (AC-004.1 `(alias)`).
const ADVISORY_ALIAS: &str = "alias";
/// The non-blocking warning phrase for an in-namespace unknown object (AC-004.2).
const ADVISORY_UNKNOWN: &str = "not a known philosophy";

/// The load-bearing slice-01 preview literal (WD-6) — present on EVERY compose
/// preview, philosophy object or not. CA-5 asserts it to prove the preview still
/// rendered (the advisory's absence is a no-op, not a broken preview).
const NOT_AS_TRUTH: &str = "not as truth";

// -----------------------------------------------------------------------------
// Shared claim-compose flags (Pillar 2: the `Given` of the compose is defined
// ONCE and reused across scenarios — only the `--object` and the stdin
// confirmation differ, never copy-pasted with drift). The subject/predicate/
// evidence/confidence mirror the slice-01 walking skeleton exactly.
// -----------------------------------------------------------------------------

const SUBJECT: &str = "github:rust-lang/rust";
const PHILOSOPHY_PREDICATE: &str = "embodiesPhilosophy";
const EVIDENCE: &str = "https://www.rust-lang.org/";
const CONFIDENCE: &str = "0.86";

/// Compose a claim asserting `SUBJECT embodiesPhilosophy <object>` — the shared
/// philosophy-claim `Given`. `object` is the only axis that varies (known /
/// alias / unknown), so the advisory is exercised against ONE stable claim shape.
fn philosophy_claim_args(object: &str) -> Vec<&str> {
    vec![
        "claim",
        "add",
        "--subject",
        SUBJECT,
        "--predicate",
        PHILOSOPHY_PREDICATE,
        "--object",
        object,
        "--evidence",
        EVIDENCE,
        "--confidence",
        CONFIDENCE,
    ]
}

/// Compose an ORDINARY (non-philosophy) claim: `<crate> dependsOn <repo>` with a
/// `--object` outside the philosophy namespace. Used by CA-5 to prove the
/// advisory does NOT fire on non-philosophy claims.
fn nonphilosophy_claim_args() -> Vec<&'static str> {
    vec![
        "claim",
        "add",
        "--subject",
        "github:rust-lang/cargo",
        "--predicate",
        "dependsOn",
        "--object",
        NONPHILOSOPHY_OBJECT,
        "--evidence",
        "https://github.com/rust-lang/cargo/blob/master/Cargo.toml",
        "--confidence",
        CONFIDENCE,
    ]
}

/// The signed `<cid>.json` claim artifacts on disk (port-exposed observable:
/// `storage.local_claim_store.files`). Empty when the dir is absent — so a
/// canceled compose that never wrote reads as "zero artifacts", exactly the
/// no-write proof CA-2 needs. Each scenario uses a FRESH isolated `OPENLORE_HOME`
/// (`TestEnv::initialized`), so a successful compose leaves exactly one file.
fn signed_claim_artifacts(env: &TestEnv) -> Vec<PathBuf> {
    let dir = env.claims_dir();
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

/// Read the SOLE signed claim artifact JSON as TEXT (format-tolerant — no serde
/// into a `claim_domain` struct, so DELIVER owns the record layout). Panics if
/// there is not exactly one artifact, which is itself the on-disk observable the
/// byte-parity scenarios (CA-3 / CA-4) rely on.
fn sole_signed_claim_text(env: &TestEnv) -> String {
    let artifacts = signed_claim_artifacts(env);
    assert_eq!(
        artifacts.len(),
        1,
        "expected exactly one signed claim artifact under {}; found {:?}",
        env.claims_dir().display(),
        artifacts
    );
    std::fs::read_to_string(&artifacts[0])
        .unwrap_or_else(|e| panic!("read signed claim artifact {}: {e}", artifacts[0].display()))
}

// =============================================================================
// US-PV-004 — the compose advisory (`claim add` `--object` resolution)
// =============================================================================

/// CA-1 (US-PV-004 happy; WALKING SKELETON for slice-25, AC-004.1 known):
/// composing a claim whose `--object` is a KNOWN philosophy
/// (`org.openlore.philosophy.memory-safety`) shows a resolution advisory naming
/// the canonical philosophy in the preview, and — on confirm (Enter, then `n` to
/// defer publish) — signs + persists the claim exactly as the slice-01 skeleton
/// does. This is the thin end-to-end advisory skeleton (typed `--object` → CLI
/// driving adapter → `VocabularyIndex` resolution → one advisory line in the
/// preview → unchanged sign/persist).
///
/// GIVEN an initialized store and a claim whose object is a known philosophy,
/// WHEN the user runs `openlore claim add … --object <known>` and presses Enter,
/// THEN the preview shows the resolution advisory naming the canonical, it exits
///      0, and persists exactly one signed claim artifact.
///
/// @walking_skeleton @driving_port @real-io @us-pv-004 @j-001 @happy
#[test]
fn compose_advisory_known_object_shows_resolution_and_signs_normally() {
    let env = TestEnv::initialized();

    // `\nn\n` = <Enter> to sign locally, then `n` to defer publish — the same
    // two-prompt confirmation shape the slice-01 walking skeleton uses (WS-6).
    let outcome = run_openlore_with_stdin(&env, &philosophy_claim_args(KNOWN_OBJECT), "\nn\n");

    // The compose/sign path is unchanged (AC-004.3 display-only) — it still
    // exits 0 and persists. These PASS today; the advisory below is the RED driver.
    assert_eq!(
        outcome.status, 0,
        "claim add with a known-philosophy object (sign confirmed) must exit 0;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables):
    //   cli.claim_add.preview_advisory            — the resolution advisory line renders in the preview
    //   storage.local_claim_store.file_count      — exactly one signed <cid>.json artifact (unchanged sign/persist)
    let preview = outcome.stdout.to_lowercase();
    assert!(
        preview.contains(ADVISORY_RESOLVES) && preview.contains(CANONICAL_NAME),
        "the compose preview must show a resolution advisory naming the canonical philosophy \
         ({CANONICAL_NAME:?}) for a known object (AC-004.1) — expected a {ADVISORY_RESOLVES:?} line;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    let artifacts = signed_claim_artifacts(&env);
    assert_eq!(
        artifacts.len(),
        1,
        "a confirmed compose must still persist exactly one signed claim artifact under {} \
         (advisory is display-only, AC-004.3); found {:?}",
        env.claims_dir().display(),
        artifacts
    );
}

/// CA-2 (US-PV-004 AC-004.1 alias; display-only, local-first cancel): composing a
/// claim whose `--object` matches a seed ALIAS
/// (`org.openlore.philosophy.mem-safety`, an alias of `memory-safety`) shows an
/// advisory that resolves it to the CANONICAL `memory-safety` AND marks it an
/// alias resolution. This scenario inspects the PREVIEW only: it closes stdin
/// without confirming (empty stdin = the slice-01 no-write beat, WS-3), so the
/// advisory is proven to render BEFORE any signing, and NO claim is written.
///
/// GIVEN an initialized store and a claim whose object is a philosophy alias,
/// WHEN the user runs `openlore claim add … --object <alias>` and does not confirm,
/// THEN the preview resolves the alias to the canonical AND marks it an alias, and
///      no claim artifact is written.
///
/// @driving_port @real-io @us-pv-004 @j-001 @alias @edge
#[test]
fn compose_advisory_alias_object_resolves_to_canonical_and_marks_alias() {
    let env = TestEnv::initialized();

    // Empty stdin = the preview is printed, then the process waits/cancels at the
    // sign prompt WITHOUT writing (slice-01 WS-3 no-write-before-confirm seam).
    let outcome = run_openlore_with_stdin(&env, &philosophy_claim_args(ALIAS_OBJECT), "");

    // Universe:
    //   cli.claim_add.preview_advisory        — the alias-resolution advisory naming the canonical + marking alias
    //   storage.local_claim_store.file_count  — 0 (advisory shown pre-sign; no write before confirm)
    let preview = outcome.stdout.to_lowercase();
    assert!(
        preview.contains(ADVISORY_RESOLVES) && preview.contains(CANONICAL_NAME),
        "the compose preview must resolve the alias object to the canonical philosophy \
         ({CANONICAL_NAME:?}) — which the typed object {ALIAS_OBJECT:?} does NOT contain — via a \
         {ADVISORY_RESOLVES:?} advisory (AC-004.1 alias);\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        preview.contains(ADVISORY_ALIAS),
        "the advisory must MARK the resolution as an alias hit (AC-004.1 `(alias)`) so the user \
         sees the typed object was matched via an alias, not directly;\n--- stdout ---\n{}",
        outcome.stdout
    );

    let artifacts = signed_claim_artifacts(&env);
    assert!(
        artifacts.is_empty(),
        "the advisory is display-only and shown BEFORE the sign prompt — an unconfirmed compose \
         must write NO claim artifact (local-first, AC-004.3); found {:?} under {}",
        artifacts,
        env.claims_dir().display()
    );
}

/// CA-3 (US-PV-004 AC-004.2 unknown, NON-BLOCKING): composing a claim whose
/// `--object` is IN the philosophy namespace but is neither a known philosophy
/// nor any seed alias (`org.openlore.philosophy.not-a-real-one`) shows a
/// NON-BLOCKING warning in the preview — and, critically, the claim STILL signs +
/// persists unchanged when the user confirms (Enter). The advisory NEVER rejects
/// (D3 "claims not truth" — the user's word is what gets signed).
///
/// GIVEN an initialized store and a claim whose object is an unknown philosophy,
/// WHEN the user runs `openlore claim add … --object <unknown>` and presses Enter,
/// THEN the preview shows a non-blocking "not a known philosophy" warning, it exits
///      0, and persists exactly one signed claim carrying the typed object verbatim.
///
/// @driving_port @real-io @us-pv-004 @j-001 @unknown @non-blocking @edge
#[test]
fn compose_advisory_unknown_object_warns_but_still_signs_when_confirmed() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(&env, &philosophy_claim_args(UNKNOWN_OBJECT), "\nn\n");

    // NON-BLOCKING: the warning is advisory, never a gate — confirming still
    // signs (exit 0). This PASSES today (claim add signs any object); the warning
    // substring below is the RED driver.
    assert_eq!(
        outcome.status, 0,
        "an unknown-philosophy object must NOT block signing — a confirmed compose must exit 0 \
         (AC-004.2 non-blocking, D3);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe:
    //   cli.claim_add.preview_advisory          — a non-blocking "not a known philosophy" warning
    //   storage.local_claim_store.file_count    — exactly one signed artifact (still signs)
    //   storage.local_claim_store.artifact_object — the typed object is signed verbatim (byte-parity)
    let preview = outcome.stdout.to_lowercase();
    assert!(
        preview.contains(ADVISORY_UNKNOWN),
        "the compose preview must show a non-blocking warning that the object is not a known \
         philosophy (AC-004.2) — expected {ADVISORY_UNKNOWN:?};\n--- stdout ---\n{}",
        outcome.stdout
    );

    let signed_text = sole_signed_claim_text(&env);
    assert!(
        signed_text.contains(UNKNOWN_OBJECT),
        "the confirmed claim must sign the TYPED object {UNKNOWN_OBJECT:?} verbatim — the warning \
         is display-only and never rewrites the payload (AC-004.2 / AC-004.3);\n--- artifact ---\n{signed_text}"
    );
}

/// CA-4 (US-PV-004 AC-004.3 byte-parity; the LOAD-BEARING guarantee): the advisory
/// is display-only and does NOT change the signed payload. The ALIAS object is the
/// STRONGEST proof: resolution COULD tempt rewriting `mem-safety` → the canonical
/// `memory-safety` in the signed record, but AC-004.3 forbids it — the object
/// bytes the user TYPED are exactly what gets signed. This scenario confirms
/// (Enter), then reads the persisted `<cid>.json` and asserts its object is
/// byte-identical to the typed alias AND does NOT contain the canonical rewrite.
/// The byte-parity assertions PASS today (claim add signs verbatim) — they are the
/// guarantee this test PINS so DELIVER's advisory cannot corrupt it; the missing
/// alias advisory is the RED driver.
///
/// GIVEN an initialized store and a claim whose object is a philosophy alias,
/// WHEN the user runs `openlore claim add … --object <alias>` and confirms,
/// THEN the persisted signed claim's object is byte-identical to the typed alias
///      (never the resolved canonical), AND the preview showed the alias advisory.
///
/// @driving_port @real-io @us-pv-004 @j-001 @byte-parity @invariant
#[test]
fn compose_advisory_does_not_alter_the_signed_object_bytes() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(&env, &philosophy_claim_args(ALIAS_OBJECT), "\nn\n");

    assert_eq!(
        outcome.status, 0,
        "the alias compose (sign confirmed) must exit 0 before we can inspect the artifact;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe:
    //   storage.local_claim_store.artifact_object — byte-identical to the TYPED alias object
    //   cli.claim_add.preview_advisory            — the alias-resolution advisory (the RED driver)
    let signed_text = sole_signed_claim_text(&env);

    // AC-004.3 (byte-parity guarantee — PASSES today, the invariant this test pins):
    // the signed object is the typed alias verbatim, NOT rewritten to the canonical.
    assert!(
        signed_text.contains(ALIAS_OBJECT),
        "the signed claim must carry the TYPED alias object {ALIAS_OBJECT:?} verbatim (AC-004.3 \
         display-only — the advisory must not rewrite the payload);\n--- artifact ---\n{signed_text}"
    );
    assert!(
        !signed_text.contains(KNOWN_OBJECT),
        "the advisory must NOT rewrite the signed object to the resolved canonical \
         {KNOWN_OBJECT:?} — the user's typed alias {ALIAS_OBJECT:?} is what gets signed (AC-004.3, \
         D3 'claims not truth');\n--- artifact ---\n{signed_text}"
    );

    // AC-004.1 alias advisory (the RED driver — absent from today's preview):
    let preview = outcome.stdout.to_lowercase();
    assert!(
        preview.contains(ADVISORY_RESOLVES)
            && preview.contains(CANONICAL_NAME)
            && preview.contains(ADVISORY_ALIAS),
        "the preview must have shown the alias-resolution advisory (resolves to \
         {CANONICAL_NAME:?}, marked alias) even though the SIGNED payload stays the typed alias \
         (AC-004.1 + AC-004.3 together — display resolves, bytes do not);\n--- stdout ---\n{}",
        outcome.stdout
    );
}

/// CA-5 (US-PV-004 no-regression / no-nagging): composing an ORDINARY claim whose
/// `--object` is OUTSIDE the `org.openlore.philosophy.*` namespace
/// (`github:rust-lang/rust`) draws NO advisory line — the advisory targets
/// philosophy objects only, and the slice-01 compose preview is byte-unchanged for
/// everything else. This is a GREEN-TODAY guardrail (no advisory exists yet, so
/// none fires); DELIVER must KEEP it green — an over-firing advisory that nagged on
/// non-philosophy claims would red this test.
///
/// GIVEN an initialized store and a claim whose object is NOT a philosophy,
/// WHEN the user runs `openlore claim add … --object <non-philosophy>`,
/// THEN the preview renders as slice-01 (contains "not as truth") with NO advisory
///      line (no "resolves to" / "not a known philosophy").
///
/// @driving_port @real-io @us-pv-004 @j-001 @no-regression @edge
#[test]
fn compose_advisory_absent_for_non_philosophy_object() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(&env, &nonphilosophy_claim_args(), "\nn\n");

    assert_eq!(
        outcome.status, 0,
        "composing an ordinary (non-philosophy) claim must exit 0 unchanged;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe:
    //   cli.claim_add.preview_rendered   — the slice-01 preview still renders ("not as truth")
    //   cli.claim_add.preview_advisory   — ABSENT for a non-philosophy object (no nagging)
    let preview = outcome.stdout.to_lowercase();
    assert!(
        preview.contains(NOT_AS_TRUTH),
        "the slice-01 compose preview must still render for a non-philosophy claim (the advisory's \
         absence is a no-op, not a broken preview);\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !preview.contains(ADVISORY_RESOLVES),
        "a non-philosophy `--object` must draw NO resolution advisory — the advisory targets \
         philosophy objects only (no nagging on ordinary claims);\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !preview.contains(ADVISORY_UNKNOWN),
        "a non-philosophy `--object` must draw NO 'not a known philosophy' warning — it is not IN \
         the philosophy namespace, so it is not a philosophy claim at all;\n--- stdout ---\n{}",
        outcome.stdout
    );
}
