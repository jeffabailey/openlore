//! Slice-26 acceptance — `openlore graph query` ALIAS TRIANGULATION (US-PV-005,
//! AC-005.1..2) per ADR-059 §5 row 26: "lexicon (`equivalence_class`);
//! `adapter-duckdb::store_read` (widen `query_philosophy_survey` filter to the
//! class); `scoring` (group under canonical) | Read-time derivation only; stored
//! objects immutable (AC-005.2); UNION-ALL still projects `author_did`
//! (anti-merging)".
//!
//! The PAYOFF read-time slice (J-002 discovery/triangulation + J-004 breadth):
//! at READ time, claims authored against a philosophy's ALIAS aggregate under its
//! CANONICAL object in `graph query --object` (and `--weighted`/`--score`) — so a
//! claim on `org.openlore.philosophy.mem-safety` and one on
//! `org.openlore.philosophy.memory-safety` (near-synonyms) CONNECT. Two hard
//! properties bound it: (1) it is a DERIVED read-time view — it NEVER rewrites the
//! stored claim objects (the signed `<cid>.json` bytes / the DB `object` column
//! stay immutable; AC-005.2), and (2) per-author attribution is PRESERVED — the
//! triangulated claims stay two attributed rows with distinct CIDs, never a merged
//! consensus (anti-merging, WD-73 / `xtask check-arch::no_cross_table_join_elides_author`).
//!
//! SCOPE (settled in these scenarios): SEED-alias equivalence only — the pure
//! `lexicon::equivalence_class` over `seeds()` (e.g. `memory-safety` ⟷
//! `mem-safety` / `memory-safe`). Minted-philosophy aliases (which would need a
//! `philosophies`-table read) are OUT of scope (a follow-up). Querying by an
//! UNKNOWN or non-philosophy object → singleton class → today's exact-match
//! behavior (no regression, no over-widening).
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline. Every scenario enters through the CLI driving adapter via the real
//! `openlore` binary (subprocess), reads over a REAL local DuckDB seeded through
//! the PRODUCTION write paths (own claims via `claim add`; peer claims via
//! `peer add` + `peer pull` against a `PeerPds` double built with
//! `build_verifiable_peer_records_for_triples` — REAL Ed25519 + CID recompute, the
//! slice-03/04 seam, NO new external fake). Per Mandate 11 the sad/edge paths are
//! EXAMPLE-ONLY — enumerated explicitly, never PBT-generated at this layer (the
//! pure `equivalence_class` is exhaustively property-tested at layer 1 in
//! `crates/lexicon` by DELIVER).
//!
//! Assertions are format-TOLERANT: they scan the observable CLI surface (stdout /
//! exit code) and the on-disk signed artifact, and NEVER couple to a `lexicon` /
//! `claim_domain` struct field. The grouping/wording/layout of the `graph query`
//! view is DELIVER's to choose (slice-04 owns it); these tests pin only the
//! domain observables the ACs fix: BOTH triangulated claims appear under the
//! canonical query, EACH attributed to its author DID, as TWO distinct-cid rows
//! (anti-merging), and the stored alias-object bytes stay verbatim. Objects /
//! canonical / alias are hard-pinned constants verified against
//! `crates/lexicon/src/seeds.json` (the `memory-safety` seed carries the aliases
//! `["mem-safety", "memory-safe"]`); the persisted-artifact assertions read the
//! JSON as TEXT so DELIVER stays free to choose the signed-record serialization.
//!
//! RED TODAY: the CLI `graph query --object` / `--weighted` read path filters the
//! `object` column by EXACT string match — the plain dimension read via
//! `adapter-duckdb::graph_query::query_by_object` (own ∪ peer `UNION ALL WHERE
//! object = ?`), the weighted read via the scoring `ByObject` filter. So a claim
//! authored on the ALIAS `…mem-safety` is INVISIBLE to a query for the CANONICAL
//! `…memory-safety`. AT-1/AT-2 (plain `--object`) and AT-3 (`--weighted`) assert
//! the alias-authored claim IS included + attributed → they FAIL against today's
//! exact-match survey = MISSING_FUNCTIONALITY (the alias-widening seam
//! `equivalence_class` does not exist yet). No harness/import error: the file
//! imports only the `support` harness + `std`, no new production symbol, no typed
//! deserialization. AT-4 (immutability, LOAD-BEARING) asserts the stored alias
//! object bytes are unchanged after a triangulated read — this PASSES today
//! (nothing rewrites the payload) and PINS the AC-005.2 invariant so DELIVER's
//! read-time widening cannot corrupt it (stays green). AT-5 (no-regression /
//! singleton) is GREEN-TODAY: a query for a philosophy in a DIFFERENT class must
//! return only its exact matches (no cross-class leak) — DELIVER must keep it so
//! (an over-widening `equivalence_class` would red it). BUILD-BEFORE-RUN: the AT
//! spawns the real `openlore` bin (`cargo build --bin openlore`), not rebuilt by
//! `cargo test`.
//!
//! Covers:
//! - US-PV-005 / AC-005.1 (WS): a claim on the CANONICAL object + a claim on an
//!   ALIAS object, by DIFFERENT authors → `graph query --object <canonical>`
//!   INCLUDES both, each attributed to its author, grouped under the canonical.
//! - US-PV-005 / AC-005.1 (anti-merging): the two triangulated claims stay TWO
//!   attributed rows (distinct CIDs, both authors present) — never one consensus.
//! - US-PV-005 / AC-005.1 (via `--weighted`): the weighted/score view over the
//!   canonical object AGGREGATES the alias-authored claim, grouped under canonical.
//! - US-PV-005 / AC-005.2 (immutability, LOAD-BEARING): after the triangulated
//!   read, the stored claim authored on `…mem-safety` STILL has object
//!   `…mem-safety` verbatim on disk — resolution NEVER rewrote it to the canonical.
//! - US-PV-005 no-regression: a query for a philosophy WITHOUT seeded-alias claims
//!   (a DIFFERENT class) returns exactly the exact-match claims (no over-widening).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use std::path::PathBuf;

// -----------------------------------------------------------------------------
// Stable pins for slice-26. Objects / canonical / alias are verified against
// `crates/lexicon/src/seeds.json`: the `memory-safety` seed carries the aliases
// `["mem-safety", "memory-safe"]`, and `dependency-pinning` carries
// `["version-pinning", "lockfile"]` (a DISTINCT equivalence class — the
// over-widening guard). We hard-pin the objects but scan the OBSERVABLE graph
// query stdout (never a `lexicon`/`claim_domain` struct) and read the persisted
// artifact as TEXT — DELIVER owns the render layout + the record serialization.
// -----------------------------------------------------------------------------

/// The CANONICAL philosophy object: the derived object-id of the `memory-safety`
/// seed. Byte-identical to `lexicon::object_id("memory-safety")`.
const CANONICAL_OBJECT: &str = "org.openlore.philosophy.memory-safety";

/// An ALIAS object: `mem-safety` is a real `aliases` entry of the `memory-safety`
/// seed (seeds.json line 5), so a claim on it must triangulate — under slice-26 —
/// into a query for the canonical. It is NOT a substring of the canonical
/// (`…mem-safety` does not contain `…memory-safety`), so a query for the canonical
/// surfacing this claim's subject/author PROVES the alias was triangulated in, not
/// merely echoed.
const ALIAS_OBJECT: &str = "org.openlore.philosophy.mem-safety";

/// A philosophy object in a DIFFERENT equivalence class (its own seed +
/// aliases `["version-pinning", "lockfile"]`), used by the no-regression guard.
/// A query for it must NEVER pull in the `memory-safety` class (no over-widening).
const OTHER_CLASS_OBJECT: &str = "org.openlore.philosophy.dependency-pinning";

/// The subject of the ALIAS-object claim (authored by the LOCAL user, "(you)").
/// Distinct from `CANONICAL_SUBJECT` so a query's output tells us WHICH claim
/// surfaced by its subject alone.
const ALIAS_SUBJECT: &str = "github:rust-lang/rust";

/// The subject of the CANONICAL-object claim (authored by a distinct PEER).
const CANONICAL_SUBJECT: &str = "github:denoland/deno";

/// The subject of the other-class claim (no-regression guard).
const OTHER_CLASS_SUBJECT: &str = "github:rust-lang/cargo";

/// The distinct PEER author of the CANONICAL-object claim (a subscribed peer,
/// pulled via the real `peer add` + `peer pull` verbs). Rendered as the bare DID.
const PEER_DID: &str = "did:plc:rachel-test";

/// The peer's deterministic Ed25519 seed (matches the slice-04 harness convention
/// for `did:plc:rachel-test`) — REAL crypto so the production pull verifies it.
const PEER_SEED: [u8; 32] = [7u8; 32];

/// The alias claim's compose-time confidence (LOCAL, "(you)").
const ALIAS_CONFIDENCE: &str = "0.80";
/// The canonical claim's compose-time confidence (PEER).
const CANONICAL_CONFIDENCE: f64 = 0.88;

// -----------------------------------------------------------------------------
// Shared `Given` — the alias-triangulation precondition (Pillar 2: the compose /
// seed of the triangulation pair is defined ONCE and reused across AT-1..AT-4,
// never copy-pasted with drift). Replicates the harness's own+peer seeding recipe
// (`seed_own_plus_peer_graph`) via the PUBLIC support primitives — the private
// slice-04 `FederatedGraphFixture` variants hard-pin OTHER objects, and the
// harness file is frozen, so the memory-safety/mem-safety pair is composed here.
// -----------------------------------------------------------------------------

/// The seeded triangulation pair. Holds the peer's `PeerPds` double alive for the
/// scenario's lifetime (dropping it tears down its in-process HTTP server; the
/// local read does not need it, but the reference suites keep it alive for parity
/// and diagnostics).
struct AliasTriangulation {
    /// Kept alive: the subscribed peer's PDS double.
    _peer: PeerPds,
}

/// Compose the LOCAL user's own philosophy claim args (the "(you)" author). Only
/// the object/subject/confidence vary; predicate/evidence mirror the slice-01
/// walking skeleton exactly.
fn own_philosophy_claim_args<'a>(
    subject: &'a str,
    object: &'a str,
    confidence: &'a str,
) -> Vec<&'a str> {
    vec![
        "claim",
        "add",
        "--subject",
        subject,
        "--predicate",
        "embodiesPhilosophy",
        "--object",
        object,
        "--evidence",
        "https://example.test/own",
        "--confidence",
        confidence,
    ]
}

/// Seed the triangulation pair into the REAL local store through the PRODUCTION
/// write paths:
///   - the LOCAL user's OWN claim on the ALIAS object (`…mem-safety`) via the real
///     `claim add` verb (`\nN\n` = confirm sign, decline publish — local-only);
///   - a distinct PEER's claim on the CANONICAL object (`…memory-safety`) via the
///     real `peer add` + `peer pull` verbs against a `PeerPds` double carrying a
///     REAL Ed25519-signed + CID-recomputed record (the production pull verifies
///     it before storing it attributed to the peer).
///
/// After this, a `graph query --object <canonical>` SHOULD (slice-26) surface BOTH
/// — the peer's exact canonical match AND the local user's alias-object claim —
/// each attributed to its author. TODAY it surfaces ONLY the peer's exact match
/// (the alias claim is filtered out by the exact-`object` read) → the RED driver.
fn seed_alias_triangulation(env: &TestEnv) -> AliasTriangulation {
    // -- The LOCAL user's OWN claim on the ALIAS object (author "(you)"). --
    let own = run_openlore_with_stdin(
        env,
        &own_philosophy_claim_args(ALIAS_SUBJECT, ALIAS_OBJECT, ALIAS_CONFIDENCE),
        "\nN\n",
    );
    assert_eq!(
        own.status, 0,
        "seed: own `claim add` on the alias object must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        own.stdout, own.stderr
    );

    // -- A distinct PEER's claim on the CANONICAL object, via `peer add` +
    // `peer pull` (REAL Ed25519 + CID recompute — the production pull verifies
    // it). This is the SAME seam `seed_federated_graph` uses internally. --
    let (records, pubkey_hex) = build_verifiable_peer_records_for_triples(
        PEER_DID,
        PEER_SEED,
        &[(CANONICAL_SUBJECT, CANONICAL_OBJECT, CANONICAL_CONFIDENCE)],
    );
    let pds = PeerPds::for_peer(PEER_DID, records);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", PEER_DID],
        PEER_DID,
        pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed: `peer add {PEER_DID}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    let seams = [PeerSeam {
        peer_did: PEER_DID,
        peer_endpoint: pds.endpoint_url(),
        peer_pubkey_hex: &pubkey_hex,
    }];
    let pulled = run_openlore_pull_multi(env, &["peer", "pull"], &seams);
    assert_eq!(
        pulled.status, 0,
        "seed: `peer pull` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    AliasTriangulation { _peer: pds }
}

// -----------------------------------------------------------------------------
// Observable-surface helpers (port-exposed only — CLI stdout + on-disk artifact).
// -----------------------------------------------------------------------------

/// Count the canonical per-claim `cid:` field lines in the graph query output —
/// the port-exposed "how many attributed rows surfaced" observable (each row is
/// independently attributable; the renderer NEVER collapses two authors). Mirrors
/// the slice-04 `graph_query_explore.rs` anti-merging assertion style.
fn cid_rows(stdout: &str) -> usize {
    stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count()
}

/// Assert the graph query output contains NO merged/consensus/aggregate row (the
/// content-frozen no-merge FOOTER legitimately contains "merged", so strip it
/// first). Mirrors the slice-04 anti-merging scan.
fn assert_no_merge_row(stdout: &str) {
    for label in ["merged", "consensus", "aggregate"] {
        let scanned = stdout.replace("No claims are merged.", "");
        assert!(
            !scanned.to_lowercase().contains(label),
            "anti-merging (AC-005.1 / WD-73): the triangulated `--object` output must contain NO \
             {label:?} row — the two triangulated claims coexist as attributed rows;\n\
             --- stdout ---\n{stdout}"
        );
    }
}

/// The signed `<cid>.json` claim artifacts in the LOCAL claim store (port-exposed
/// observable: `storage.local_claim_store.files`). The alias-object claim is the
/// LOCAL user's OWN claim, so it lands here (peer claims live under a separate
/// peer store); a fresh isolated `OPENLORE_HOME` per scenario means exactly one
/// own-claim artifact.
fn local_claim_artifacts(env: &TestEnv) -> Vec<PathBuf> {
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

/// Read the SOLE local (own) signed claim artifact as TEXT (format-tolerant — no
/// serde into a `claim_domain` struct, so DELIVER owns the record layout). Panics
/// unless exactly one artifact exists, which is itself the on-disk observable the
/// immutability scenario (AT-4) relies on.
fn sole_local_claim_text(env: &TestEnv) -> String {
    let artifacts = local_claim_artifacts(env);
    assert_eq!(
        artifacts.len(),
        1,
        "expected exactly one local (own) signed claim artifact under {}; found {:?}",
        env.claims_dir().display(),
        artifacts
    );
    std::fs::read_to_string(&artifacts[0])
        .unwrap_or_else(|e| panic!("read local claim artifact {}: {e}", artifacts[0].display()))
}

// =============================================================================
// US-PV-005 — alias triangulation in `graph query`
// =============================================================================

/// AT-1 (US-PV-005 happy; WALKING SKELETON for slice-26, AC-005.1): TWO signed
/// claims by DIFFERENT authors — the LOCAL user's on the ALIAS object
/// (`…mem-safety`) and a PEER's on the CANONICAL object (`…memory-safety`) — then
/// `graph query --object org.openlore.philosophy.memory-safety` INCLUDES BOTH,
/// each attributed to its author, grouped under the canonical philosophy. This is
/// the thin end-to-end triangulation skeleton (near-synonyms connect at read time
/// through the CLI driving adapter → the widened object read → one attributed
/// grouped view).
///
/// GIVEN a claim on the canonical object by a peer AND a claim on an alias object
///       by the local user,
/// WHEN the user runs `openlore graph query --object <canonical>`,
/// THEN both claims appear, each attributed to its author DID, as two distinct
///      rows grouped under the canonical philosophy.
///
/// @walking_skeleton @driving_port @real-io @us-pv-005 @j-002 @j-004 @happy
#[test]
fn graph_query_by_canonical_object_includes_alias_authored_claim_attributed() {
    let env = TestEnv::initialized();
    let _seed = seed_alias_triangulation(&env);

    let outcome = run_openlore(&env, &["graph", "query", "--object", CANONICAL_OBJECT]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object <canonical> must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables of the triangulated `--object` view, all
    // asserted against stdout — the CLI driving-port surface):
    //   cli.graph_query.alias_author_did_present  — the local "(you)" DID (its alias-object claim triangulated in)
    //   cli.graph_query.alias_subject_present      — the alias claim's project (github:rust-lang/rust)
    //   cli.graph_query.canonical_author_present   — the peer's exact canonical claim (green sanity)
    //   cli.graph_query.cid_rows                    — 2 (BOTH claims surface; none merged)
    let stdout = &outcome.stdout;
    let local_did = env.identity.author_did();

    // GREEN SANITY: the PEER's exact-match canonical claim already surfaces today
    // (this is the pre-slice-26 exact-`object` read) — so the failure below is
    // genuinely the MISSING alias-widening, not a broken query.
    assert!(
        stdout.contains(PEER_DID) && stdout.contains(CANONICAL_SUBJECT),
        "sanity: the peer's exact-match canonical claim (author {PEER_DID}, subject \
         {CANONICAL_SUBJECT}) must surface for a canonical `--object` query;\n--- stdout ---\n{stdout}"
    );

    // RED DRIVER 1 (AC-005.1): the LOCAL user's ALIAS-object claim must be
    // INCLUDED under the canonical query — attributed to its author "(you)".
    // Today the exact-`object` read excludes it (the alias `…mem-safety` ≠ the
    // queried canonical `…memory-safety`) → this fails = MISSING_FUNCTIONALITY.
    assert!(
        stdout.contains(local_did),
        "AC-005.1: a claim authored against the ALIAS object {ALIAS_OBJECT} must be INCLUDED in a \
         query for the CANONICAL object {CANONICAL_OBJECT}, attributed to its author ({local_did}) \
         — near-synonyms triangulate at read time; today's exact-match survey excludes it;\n\
         --- stdout ---\n{stdout}"
    );

    // RED DRIVER 2: the alias claim's PROJECT (its subject, present on no other
    // claim) must appear — proving the alias-object row itself was triangulated
    // in, not merely that the local DID appears somewhere.
    assert!(
        stdout.contains(ALIAS_SUBJECT),
        "AC-005.1: the alias-object claim's subject {ALIAS_SUBJECT} must appear under the canonical \
         query — the alias-authored claim itself is triangulated in and grouped under the canonical \
         philosophy;\n--- stdout ---\n{stdout}"
    );

    // RED DRIVER 3: BOTH claims surface as TWO attributed rows (grouped under the
    // canonical). Today only the peer's exact match surfaces → cid_rows == 1.
    assert_eq!(
        cid_rows(stdout),
        2,
        "AC-005.1: both the canonical-object claim and the alias-object claim must surface as TWO \
         attributed rows under the canonical query; got {} (today's exact-match read returns only \
         the canonical claim);\n--- stdout ---\n{stdout}",
        cid_rows(stdout)
    );
}

/// AT-2 (US-PV-005 AC-005.1 anti-merging): the two triangulated claims remain TWO
/// attributed rows — NOT merged into one consensus. Each author's DID is present
/// and their CIDs are distinct; there is NO "merged"/"consensus"/"aggregate" row.
/// This is the anti-merging invariant of the read-time derivation (WD-73): the
/// UNION-ALL read still projects `author_did` per row, so widening the filter to
/// the equivalence class must not collapse the near-synonym claims into one.
///
/// GIVEN the triangulated pair (peer canonical + local alias, different authors),
/// WHEN the user runs `openlore graph query --object <canonical>`,
/// THEN both author DIDs are present on distinct-cid rows and no row merges them.
///
/// @driving_port @real-io @us-pv-005 @j-002 @anti-merging @edge
#[test]
fn graph_query_triangulated_claims_stay_two_attributed_rows_unmerged() {
    let env = TestEnv::initialized();
    let _seed = seed_alias_triangulation(&env);

    let outcome = run_openlore(&env, &["graph", "query", "--object", CANONICAL_OBJECT]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object <canonical> must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables): both author DIDs present on their own
    // rows (cli.graph_query.distinct_authors_present == 2), two distinct cid rows
    // (cli.graph_query.cid_rows == 2), and NO merge row
    // (cli.graph_query.merge_row_absent). Asserted against stdout.
    let stdout = &outcome.stdout;
    let local_did = env.identity.author_did();

    // Both authors are present on the triangulated view — each claim keeps its
    // own attribution (anti-merging). The local "(you)" presence is the RED
    // driver (its alias-object claim is excluded by today's exact-match read).
    assert!(
        stdout.contains(PEER_DID),
        "the canonical claim's author {PEER_DID} must be attributed;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains(local_did),
        "AC-005.1 anti-merging: the alias claim's author ({local_did}) must ALSO be attributed on \
         its OWN row — the two triangulated claims stay per-author, never merged into one \
         consensus; today's exact-match read excludes the alias claim entirely;\n\
         --- stdout ---\n{stdout}"
    );

    // The two triangulated claims are TWO distinct-cid rows (never collapsed).
    assert_eq!(
        cid_rows(stdout),
        2,
        "AC-005.1 anti-merging: the triangulated pair must render as TWO distinct-cid attributed \
         rows (distinct CIDs, one per signed claim); got {};\n--- stdout ---\n{stdout}",
        cid_rows(stdout)
    );

    // No consensus/merge row collapses the two authors.
    assert_no_merge_row(stdout);
}

/// AT-3 (US-PV-005 AC-005.1 via `--weighted`): the weighted/score view over the
/// canonical object INCLUDES the alias-authored claim in the aggregation, grouped
/// under the canonical philosophy. The `--weighted` read feeds the SAME object-
/// filtered survey into the pure `scoring::score` core, so widening the object
/// filter to the equivalence class must flow the alias-object claim into the
/// weighted aggregation too (its project appears as a ranked/aggregated entry).
///
/// GIVEN the triangulated pair (peer canonical + local alias),
/// WHEN the user runs `openlore graph query --object <canonical> --weighted`,
/// THEN the alias-authored claim's project is aggregated into the weighted view
///      under the canonical philosophy.
///
/// @driving_port @real-io @us-pv-005 @j-002 @j-004 @weighted @edge
#[test]
fn graph_query_weighted_over_canonical_aggregates_alias_authored_claim() {
    let env = TestEnv::initialized();
    let _seed = seed_alias_triangulation(&env);

    let outcome = run_openlore(
        &env,
        &["graph", "query", "--object", CANONICAL_OBJECT, "--weighted"],
    );
    assert_eq!(
        outcome.status, 0,
        "graph query --object <canonical> --weighted must exit 0;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables of the `--weighted` view): the alias
    // claim's project is aggregated (cli.graph_query.weighted_alias_subject_present)
    // AND the canonical claim's project is aggregated
    // (cli.graph_query.weighted_canonical_subject_present). Asserted against stdout.
    let stdout = &outcome.stdout;

    // GREEN SANITY: the canonical claim's project is already in the weighted view
    // today (the exact-match survey feeds scoring) — so the failure below is the
    // missing alias-widening, not a broken weighted read.
    assert!(
        stdout.contains(CANONICAL_SUBJECT),
        "sanity: the canonical claim's project {CANONICAL_SUBJECT} must appear in the weighted view \
         over the canonical object;\n--- stdout ---\n{stdout}"
    );

    // RED DRIVER (AC-005.1 via --weighted): the ALIAS-object claim's project must
    // be aggregated into the weighted view under the canonical philosophy. Today
    // the scoring survey filters `object` exactly, so the alias claim's project is
    // absent from the aggregation → this fails = MISSING_FUNCTIONALITY.
    assert!(
        stdout.contains(ALIAS_SUBJECT),
        "AC-005.1: the weighted/score view over the CANONICAL object {CANONICAL_OBJECT} must \
         AGGREGATE the alias-authored claim (its project {ALIAS_SUBJECT} appears), grouped under \
         the canonical philosophy — near-synonyms triangulate in the aggregate too; today's \
         exact-match scoring survey excludes it;\n--- stdout ---\n{stdout}"
    );
}

/// AT-4 (US-PV-005 AC-005.2 immutability; the LOAD-BEARING guarantee): alias
/// triangulation is a DERIVED read-time view — it NEVER rewrites the stored claim
/// objects. After a triangulated read (`graph query --object <canonical>`), the
/// stored claim authored on the ALIAS object STILL has object `…mem-safety`
/// verbatim on disk — resolution did NOT rewrite it to the canonical
/// `…memory-safety`. This PINS the AC-005.2 immutability invariant so DELIVER's
/// read-time widening cannot corrupt the signed bytes. The byte-parity assertions
/// PASS today (nothing rewrites the payload) — this test is RED-SAFE (it must stay
/// green through DELIVER), the read-time-only guard on the payoff slice.
///
/// GIVEN the triangulated pair, AND a triangulated read has been performed,
/// WHEN the stored alias-object claim artifact is read from disk,
/// THEN its object is byte-identical to the typed alias (never the canonical).
///
/// @driving_port @real-io @us-pv-005 @j-002 @immutability @invariant
#[test]
fn alias_triangulation_never_rewrites_the_stored_object_bytes() {
    let env = TestEnv::initialized();
    let _seed = seed_alias_triangulation(&env);

    // Perform the triangulated read (the derivation that COULD tempt a rewrite).
    let outcome = run_openlore(&env, &["graph", "query", "--object", CANONICAL_OBJECT]);
    assert_eq!(
        outcome.status, 0,
        "the triangulated read must exit 0 before we inspect the stored artifact;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observable): the stored alias-object claim's object
    // bytes (storage.local_claim_store.artifact_object) — byte-identical to the
    // TYPED alias, NEVER the resolved canonical.
    let signed_text = sole_local_claim_text(&env);

    // AC-005.2 (immutability — PASSES today, the invariant this test PINS):
    assert!(
        signed_text.contains(ALIAS_OBJECT),
        "AC-005.2: the stored claim must carry the TYPED alias object {ALIAS_OBJECT} verbatim after \
         a triangulated read — resolution is display/aggregation only and NEVER rewrites the signed \
         payload;\n--- artifact ---\n{signed_text}"
    );
    assert!(
        !signed_text.contains(CANONICAL_OBJECT),
        "AC-005.2: alias triangulation must NOT rewrite the stored object to the resolved canonical \
         {CANONICAL_OBJECT} — the signed bytes the user typed ({ALIAS_OBJECT}) are immutable (D3 \
         'claims not truth'); the derivation lives at read time only;\n--- artifact ---\n{signed_text}"
    );
}

/// AT-5 (US-PV-005 no-regression / singleton): a query for a philosophy in a
/// DIFFERENT equivalence class returns exactly the exact-match claims — behavior
/// byte-identical to slice-04 (no over-widening). Seeds a claim on
/// `dependency-pinning` (its own class) alongside the `mem-safety` claim (a
/// DIFFERENT class), then queries `--object dependency-pinning`: the
/// dependency-pinning claim surfaces, and the `mem-safety` claim must NOT leak in.
/// This is a GREEN-TODAY guardrail (the exact-match read already excludes the
/// other class); DELIVER must KEEP it green — an `equivalence_class` that
/// over-widened across classes (or returned all objects) would red it.
///
/// GIVEN a claim on `dependency-pinning` AND an unrelated claim on `mem-safety`,
/// WHEN the user runs `openlore graph query --object <dependency-pinning>`,
/// THEN only the dependency-pinning claim surfaces (the mem-safety class does not
///      leak in).
///
/// @driving_port @real-io @us-pv-005 @j-002 @no-regression @edge
#[test]
fn graph_query_other_class_object_returns_only_its_exact_matches() {
    let env = TestEnv::initialized();

    // Seed TWO of the LOCAL user's own claims in DISTINCT equivalence classes:
    // one on `dependency-pinning`, one on `mem-safety` (the memory-safety class).
    let dep = run_openlore_with_stdin(
        &env,
        &own_philosophy_claim_args(OTHER_CLASS_SUBJECT, OTHER_CLASS_OBJECT, "0.90"),
        "\nN\n",
    );
    assert_eq!(
        dep.status, 0,
        "seed: own claim on {OTHER_CLASS_OBJECT} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        dep.stdout, dep.stderr
    );
    let mem = run_openlore_with_stdin(
        &env,
        &own_philosophy_claim_args(ALIAS_SUBJECT, ALIAS_OBJECT, ALIAS_CONFIDENCE),
        "\nN\n",
    );
    assert_eq!(
        mem.status, 0,
        "seed: own claim on {ALIAS_OBJECT} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        mem.stdout, mem.stderr
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", OTHER_CLASS_OBJECT]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object <dependency-pinning> must exit 0;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Universe (port-exposed observables): the dependency-pinning project surfaces
    // (cli.graph_query.other_class_subject_present), the mem-safety class does NOT
    // leak (cli.graph_query.mem_safety_class_absent), and exactly one row surfaces
    // (cli.graph_query.cid_rows == 1). Asserted against stdout.
    let stdout = &outcome.stdout;

    assert!(
        stdout.contains(OTHER_CLASS_SUBJECT),
        "the dependency-pinning claim's project {OTHER_CLASS_SUBJECT} must surface for its own \
         `--object` query;\n--- stdout ---\n{stdout}"
    );

    // NO over-widening: the memory-safety class must NOT leak into a
    // dependency-pinning query. Guards against an `equivalence_class` that crosses
    // classes (or returns every object). GREEN-today; DELIVER must keep it green.
    assert!(
        !stdout.contains(ALIAS_OBJECT) && !stdout.contains(ALIAS_SUBJECT),
        "no over-widening: a query for {OTHER_CLASS_OBJECT} must NOT pull in the DISTINCT \
         memory-safety class (neither {ALIAS_OBJECT} nor its project {ALIAS_SUBJECT}); the \
         equivalence class is per-philosophy, not global;\n--- stdout ---\n{stdout}"
    );

    assert_eq!(
        cid_rows(stdout),
        1,
        "no-regression: exactly the ONE exact-match dependency-pinning claim must surface (byte-\
         identical to slice-04 exact-match behavior); got {};\n--- stdout ---\n{stdout}",
        cid_rows(stdout)
    );
}
