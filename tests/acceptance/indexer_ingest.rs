//! Slice-05 acceptance — the `openlore-indexer` binary's verified, attributed
//! ingest pipeline (`openlore-indexer ingest` / `serve`) per ADR-023/024/025/026.
//!
//! The `@infrastructure` surface for US-AV-001: the SECOND binary aggregates
//! PUBLIC signed claims from across the network, verifies each signature +
//! recomputes each CID BEFORE indexing (the cardinal verified-before-index gate,
//! WD-104 / I-AV-1 / KPI-AV-3), persists every indexed record with a non-`Option`
//! `author_did` and a non-empty `verified_against` into the SEPARATE
//! `index.duckdb` (anti-merging at ingest, WD-103 / I-AV-2), and holds NO signing
//! capability + NO local-store handle by construction (the capability boundary,
//! ADR-023 / I-AV-5).
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix + DD-AV-1. Every scenario enters through the REAL
//! `openlore-indexer` binary (subprocess via `assert_cmd::cargo_bin`) against a
//! FAKE network ingest source (`FakeIngestSource` — a bounded fixture record
//! source hosting `listRecords`) + a real-`z6Mk...` DID-document fixture
//! resolver, exercises the real `appview-domain` ingest gate + the REAL
//! `index.duckdb` store, and (per Mandate 11) is EXAMPLE-ONLY — the adversarial
//! sad paths are enumerated explicitly, never PBT-generated. The pure ingest-gate
//! PROPERTIES live at layer 2 in `appview_core.rs`.
//!
//! Hermetic seam (DD-AV-2): the indexer is wired against a FAKE ingest source
//! (bounded fixture records incl. the adversarial set: unsigned / tampered-sig /
//! cid-mismatch) + a fixture PLC DID-document resolver carrying a real `z6Mk...`
//! value (a known test keypair) so the ADR-026 decode runs the REAL decode path
//! (NOT the slice-03 env seam). No live network is contacted.
//!
//! Covers:
//! - US-AV-001: bootstrap the indexer + verified, attributed ingest pipeline
//! - WD-104 / I-AV-1: verify-before-index (the SAME pure core; no second path)
//! - WD-103 / I-AV-2: anti-merging at the ingest layer (non-Option author_did)
//! - ADR-023 / I-AV-5: indexer signing-incapable + holds no local store
//! - ADR-026 / I-AV-6: production PLC z6Mk multibase decode is real (gold path)
//! - Release gate `indexer_rejects_unverified_claim` (KPI-AV-3) — load-bearing
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-AV-001 — bootstrap + the walking-skeleton verified attributed ingest
// =============================================================================

/// AV-1 (US-AV-001 happy; WALKING SKELETON beat 1 for slice-05): the indexer
/// ingests a VALID signed public claim from a fake network source, verifies its
/// signature against the author's REAL `z6Mk...` PLC key and recomputes its CID,
/// and the verified record becomes searchable — attributed to its author DID,
/// with a non-empty `verified_against`. This is the thinnest "the index exists,
/// is trustworthy, and is searchable" proof the walking skeleton rests on.
///
/// @us-av-001 @real-io @driving_port @walking_skeleton @infrastructure @i-av-1 @kpi-av-3
#[test]
fn indexer_ingests_a_verified_attributed_claim_and_it_becomes_searchable() {
    // -- Precondition: a fake network source hosts ONE valid signed claim by
    // Priya (did:plc:priya-test) on github:bazelbuild/bazel embodying
    // reproducible-builds (0.82), with a real-z6Mk DID-doc resolvable for her
    // verification key. Seed via the slice-05 ingest harness (the FakeIngestSource
    // + the fixture PLC resolver). --
    //
    // -- Action: run the REAL `openlore-indexer ingest` one-shot pass against the
    // fake source + fixture resolver (the production indexer composition root;
    // wire -> probe -> use). --
    //
    // -- Observable outcome (port-exposed): a subsequent query (via the index
    // store / the serve query handler) returns Priya's claim with
    // author_did == "did:plc:priya-test" and verified_against != "". The ingest
    // emitted indexer.ingest.verified (count 1) and indexer.ingest.rejected
    // (count 0). This is the SAME pure-core verify decision proven in
    // appview_core.rs AVC-1, now wired through the real binary + real store.
    //
    // Universe (port-exposed observable surface of the ingest pass): the indexed
    // row's author_did, verified_against; indexer.ingest.verified count (1);
    // indexer.ingest.rejected count (0). NOT an internal store struct field.

    // -- Precondition: a fake network source hosts ONE valid signed claim by
    // Priya on github:bazelbuild/bazel embodying reproducible-builds (0.82). The
    // PLC verify key is wired via the slice-03 pubkey seam (the real z6Mk decode
    // is 03-04/AV-4); the seam value is Priya's fixture keypair pubkey hex. --
    let env = TestEnv::fresh();
    let priya = FixtureKeypair::for_did(PRIYA_DID);
    let priya_pubkey_hex = hex_lower(&priya.verifying_key.0);
    let source = FakeIngestServer::start(vec![fixture_ingest_valid_signed()]);

    // -- Action: run the REAL `openlore-indexer ingest` one-shot pass (wire ->
    // probe -> use) against the fake source + the PLC pubkey seam. --
    let outcome = run_openlore_indexer_with_source(
        &env,
        &["ingest"],
        source.source_url(),
        &[(PRIYA_DID, &priya_pubkey_hex)],
    );

    assert_eq!(
        outcome.status, 0,
        "openlore-indexer ingest must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // The ingest emitted indexer.ingest.verified (count 1) + rejected (count 0).
    assert!(
        outcome.stdout.contains("indexer.ingest.verified")
            && outcome.stdout.contains("\"count\":1"),
        "expected indexer.ingest.verified count 1 in stdout; got: {}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("indexer.ingest.rejected")
            && outcome.stdout.contains("\"count\":0"),
        "expected indexer.ingest.rejected count 0 in stdout; got: {}",
        outcome.stdout
    );

    // -- Observable outcome (port-exposed): a subsequent query (a direct
    // index-store read of the REAL index.duckdb) returns Priya's claim, attributed
    // (author_did == did:plc:priya-test#org.openlore.application) with a non-empty
    // verified_against. The index exists + is trustworthy + is searchable. --
    let rows = read_indexed_claims_by_object(&env, "org.openlore.philosophy.reproducible-builds");
    assert_eq!(
        rows.len(),
        1,
        "exactly one verified claim must be searchable by object; got {rows:?}"
    );
    let row = &rows[0];
    assert_eq!(
        row.author_did, "did:plc:priya-test#org.openlore.application",
        "the indexed row must be attributed to Priya (author derived from the signed payload)"
    );
    assert_eq!(row.subject, "github:bazelbuild/bazel");
    assert!(
        !row.verified_against.is_empty(),
        "verified_against must never be empty on an indexed row (WD-104)"
    );
}

/// AV-2 (US-AV-001 anti-merging at ingest): two DISTINCT authors each publish a
/// verified public claim on the SAME (subject, object); the indexer stores TWO
/// individually-attributed `indexed_claims` rows (distinct non-empty author_did)
/// and there is NO merged multi-author "consensus" row/table anywhere in
/// `index.duckdb`. The ingest-layer half of the three-layer anti-merging
/// enforcement (WD-103 / I-AV-2; the structural xtask rule + the behavioral
/// search gate are the other two layers).
///
/// @us-av-001 @real-io @anti-merging @i-av-2 @kpi-av-2
#[test]
fn indexer_stores_two_distinct_author_claims_without_merging_on_same_subject_object() {
    // -- Precondition: a fake source hosts two valid signed claims on the SAME
    // (github:denoland/deno, dependency-pinning) by Priya (0.70) and Sven (0.65),
    // both with resolvable real-z6Mk keys. --
    //
    // -- Action: `openlore-indexer ingest`. --
    //
    // -- Observable outcome: index.duckdb's indexed_claims holds TWO rows with
    // distinct non-empty author_did (priya, sven); a search by that object
    // returns both as separate attributed rows; the store has NO
    // consensus/merged/aggregate table (the no-merge-schema assertion — the
    // load-bearing absence, WD-103). The pure compose preserves both (proven in
    // appview_core.rs AVC-2/AVC-5); this asserts the REAL store mirrors it.
    //
    // Universe: the set of (author_did) in indexed_claims for the (subject,
    // object) pair {priya, sven}; the presence/absence of any
    // consensus/merged/aggregate table (absent).

    // -- Precondition: a fake source hosts TWO valid signed claims on the SAME
    // (github:denoland/deno, dependency-pinning) by Priya (0.70) + Sven (0.65),
    // both with resolvable verify keys (the slice-03 pubkey seam carries each
    // fixture keypair pubkey hex; the real z6Mk decode is AV-4). --
    let env = TestEnv::fresh();
    let priya = FixtureKeypair::for_did(PRIYA_DID);
    let sven = FixtureKeypair::for_did(SVEN_DID);
    let priya_pubkey_hex = hex_lower(&priya.verifying_key.0);
    let sven_pubkey_hex = hex_lower(&sven.verifying_key.0);
    let source = FakeIngestServer::start(corpus_deno_dependency_pinning_two_authors());

    // -- Action: run the REAL `openlore-indexer ingest` one-shot pass (wire ->
    // probe -> use) against the fake source + both PLC pubkey seams. --
    let outcome = run_openlore_indexer_with_source(
        &env,
        &["ingest"],
        source.source_url(),
        &[
            (PRIYA_DID, &priya_pubkey_hex),
            (SVEN_DID, &sven_pubkey_hex),
        ],
    );

    assert_eq!(
        outcome.status, 0,
        "openlore-indexer ingest must exit 0. stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );

    // Both records verified; none rejected (the pure compose preserves both —
    // AVC-2/AVC-5; this asserts the REAL binary mirrors it).
    assert!(
        outcome.stdout.contains("indexer.ingest.verified")
            && outcome.stdout.contains("\"count\":2"),
        "expected indexer.ingest.verified count 2 in stdout; got: {}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("indexer.ingest.rejected")
            && outcome.stdout.contains("\"count\":0"),
        "expected indexer.ingest.rejected count 0 in stdout; got: {}",
        outcome.stdout
    );

    // -- Observable outcome 1+2: a search by the SAME object returns BOTH claims
    // as separate, individually-attributed rows — distinct non-empty author_did
    // {priya, sven}, NEVER merged into one consensus row. --
    let rows = read_indexed_claims_by_object(&env, "org.openlore.philosophy.dependency-pinning");
    assert_eq!(
        rows.len(),
        2,
        "two distinct-author claims on the SAME (subject,object) must stay TWO \
         individually-attributed rows (anti-merging, WD-103); got {rows:?}"
    );

    // Every row is attributed to a distinct, non-empty author DID — the set is
    // exactly {priya, sven}. The author_did carries the codebase `#fragment` form
    // (one app identity per DID), matching the AV-1 attribution convention.
    for row in &rows {
        assert!(
            !row.author_did.is_empty(),
            "each indexed row must carry a non-empty author_did (WD-103); got {row:?}"
        );
        assert_eq!(
            row.subject, "github:denoland/deno",
            "both rows share the same subject; only the author differs"
        );
        assert!(
            !row.verified_against.is_empty(),
            "verified_against must never be empty on an indexed row (WD-104)"
        );
    }
    let mut authors: Vec<&str> = rows.iter().map(|r| r.author_did.as_str()).collect();
    authors.sort_unstable();
    assert_eq!(
        authors,
        vec![
            "did:plc:priya-test#org.openlore.application",
            "did:plc:sven-test#org.openlore.application",
        ],
        "the two rows must be attributed to Priya AND Sven as SEPARATE authors — \
         never collapsed/merged onto a single attributed row"
    );

    // The two rows have distinct CIDs — de-dup at upsert is by CID only (ADR-025);
    // distinct authors yield distinct canonical payloads -> distinct CIDs -> two
    // rows. (A merge would have collapsed them onto one CID.)
    assert_ne!(
        rows[0].cid, rows[1].cid,
        "the two distinct-author claims must have distinct CIDs (no CID-level merge)"
    );

    // -- Observable outcome 3: the load-bearing ABSENCE — `index.duckdb` has NO
    // consensus/merged/aggregate/summary table anywhere in its schema (WD-103).
    // This is the structural complement to the per-row attribution above. --
    assert_no_merged_consensus_table(&env);
}

// =============================================================================
// US-AV-001 — the CARDINAL verified-before-index release gate (KPI-AV-3)
// =============================================================================

/// AV-3 / RELEASE GATE `indexer_rejects_unverified_claim` (US-AV-001;
/// I-AV-1 / WD-104 / KPI-AV-3 — load-bearing, release-blocking): a fake source
/// serves an UNSIGNED record, a TAMPERED-SIGNATURE record, and a CID-MISMATCH
/// record (the adversarial set), plus one VALID signed record. The indexer
/// REJECTS all three adversarial records at ingest (they NEVER enter
/// `index.duckdb`, NEVER appear in any search), while the valid record is
/// indexed and becomes searchable. The cardinal KPI-AV-3 disprover: any
/// unverified claim reaching the index or a search result is UNSHIPPABLE.
///
/// @us-av-001 @real-io @driving_port @release-gate @i-av-1 @kpi-av-3 @error @adversarial
#[test]
fn indexer_rejects_unverified_claim() {
    // -- Precondition: a fake source hosts FOUR records on the same author
    // surface — (a) unsigned, (b) tampered-signature, (c) cid-mismatch (recomputed
    // CID != published CID), (d) one VALID signed record — plus the resolvable
    // real-z6Mk key. This reuses the slice-03 adversarial-peer fixture discipline
    // (D-D15) extended to the ingest source (D-D38). --
    //
    // -- Action: `openlore-indexer ingest` one-shot pass. --
    //
    // -- Observable outcome (the cardinal gate):
    //   1. index.duckdb's indexed_claims contains EXACTLY the one valid record
    //      (the three adversarial records produced NO row);
    //   2. a search across every dimension NEVER returns any of the three
    //      adversarial records;
    //   3. the valid record IS searchable, attributed, verified_against != "";
    //   4. indexer.ingest.rejected emitted with reason in
    //      {unsigned, bad_signature, cid_mismatch} for each adversarial record;
    //      indexer.ingest.verified count 1.
    //
    // The reject path reuses the SAME pure claim_domain::verify + compute_cid
    // (no second verification path; proven generatively in appview_core.rs
    // AVC-1) — this layer-3 example pins the REAL binary + REAL store + the
    // adversarial wire fixtures. Per Mandate 11 the adversarial inputs are NAMED
    // examples, never PBT-generated at layer 3.
    //
    // Universe (port-exposed): count of indexed_claims rows (1); the set of
    // adversarial cids absent from indexed_claims AND absent from every search
    // result; indexer.ingest.rejected{reason} counts; indexer.ingest.verified (1).
    todo!(
        "DELIVER (slice-05): RELEASE GATE. Seed FakeIngestSource with unsigned + \
         tampered-sig + cid-mismatch + one valid record; run `openlore-indexer \
         ingest`; assert ONLY the valid record is in indexed_claims + searchable, \
         the 3 adversarial cids never enter the index nor any search result, and \
         indexer.ingest.rejected fires per reason. Reuses claim_domain::verify."
    );
}

/// AV-4 (US-AV-001 ADR-026 gold path; I-AV-6): the indexer resolves a network
/// author's REAL `z6Mk...` PLC DID-document key, DECODES the multibase value via
/// the production `claim_domain::decode_ed25519_multibase` path, and that decoded
/// key VERIFIES a known-good signature AND REJECTS a tampered one — proving the
/// REAL decode runs (NOT the slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam,
/// which is release-forbidden). A pass that only succeeded against the seam is a
/// CI failure by construction.
///
/// @us-av-001 @real-io @i-av-6 @adr-026 @gold-path
#[test]
fn indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam() {
    // -- Precondition: a fixture PLC DID-document carrying a REAL z6Mk... value
    // for a known test keypair (the real-z6Mk DID-doc fixture); the env seam
    // OPENLORE_PEER_PUBKEY_HEX_<did> is UNSET (so a seam-only impl would fail to
    // resolve). The source hosts one record signed by that keypair + one with a
    // tampered signature. --
    //
    // -- Action: `openlore-indexer ingest` (the resolve-only IdentityResolvePort
    // path, ADR-026). --
    //
    // -- Observable outcome: the good-signature record is indexed (verified
    // against the REAL decoded key); the tampered one is rejected. Because the
    // env seam is unset, success PROVES the real PLC z6Mk decode ran (I-AV-6 gold
    // path; mirrors the adapter probe in DESIGN §6.3). The decode∘encode identity
    // + malformed-input-errors properties are DELIVER's claim-domain unit/mutation
    // tests (out of DISTILL scope); this pins the END-TO-END real-decode wiring.
    //
    // Universe (port-exposed): the good record indexed (verified_against != "");
    // the tampered record rejected (bad_signature); the env seam unset throughout.
    todo!(
        "DELIVER (slice-05): with OPENLORE_PEER_PUBKEY_HEX_<did> UNSET and a real \
         z6Mk PLC DID-doc fixture, run `openlore-indexer ingest`; assert the \
         good-signature record verifies against the REAL decoded key (indexed) \
         and the tampered one is rejected. Seam-unset success proves the real \
         ADR-026 decode ran, not the seam (I-AV-6)."
    );
}

// =============================================================================
// US-AV-001 — the capability boundary (ADR-023 / I-AV-5)
// =============================================================================

/// AV-5 (US-AV-001 capability boundary; ADR-023 / I-AV-5): the `openlore-indexer`
/// binary is signing-INCAPABLE and holds NO local-store handle by construction.
/// Its CLI exposes no author/sign/publish verb; it never opens or writes the
/// user's `openlore.duckdb`; the composition-root `capability_boundary_probe`
/// refuses to start if wired with a signing identity or the local store. Mirrors
/// the slice-02 `adapter-github` human-gate (I-SCR-1).
///
/// @us-av-001 @real-io @i-av-5 @adr-023 @capability-boundary
#[test]
fn indexer_is_signing_incapable_and_touches_no_local_store() {
    // -- Precondition: a TestEnv with a populated user openlore.duckdb (own
    // claims). --
    //
    // -- Action: run `openlore-indexer --help` (and the ingest/serve pass)
    // pointed at its OWN index dir + config. --
    //
    // -- Observable outcome:
    //   1. `openlore-indexer` exposes NO `claim add` / sign / publish verb (the
    //      help/usage surface lists only `serve` + `ingest` + `stats`);
    //   2. after an ingest pass, the user's openlore.duckdb is byte-unchanged
    //      (the indexer never opened or wrote it — it has no handle to it);
    //   3. only the SEPARATE index.duckdb (the indexer's own store) is written.
    //
    // This is the behavioral layer of the three-layer capability-boundary
    // enforcement (type: verify-only/read-only ports; structural: the xtask
    // `indexer_holds_no_signing_or_local_store` rule; behavioral: this +
    // the capability_boundary_probe). The structural + type layers are DELIVER's
    // xtask/type concern.
    //
    // Universe (port-exposed): the indexer help verb-set (no sign/publish/add);
    // openlore.duckdb mtime/bytes (unchanged); index.duckdb (written).
    todo!(
        "DELIVER (slice-05): assert `openlore-indexer` help exposes no \
         sign/publish/add verb; after an ingest pass the user's openlore.duckdb \
         is byte-unchanged and only the separate index.duckdb is written \
         (ADR-023 capability boundary; I-AV-5 behavioral layer)."
    );
}

/// AV-6 (US-AV-001 wire -> probe -> use; ADR-009/023): the indexer runs ALL four
/// driven-adapter probes (ingest source, index store, resolve-only identity,
/// query server) PLUS the capability-boundary probe BEFORE the first ingest pass
/// or query, and REFUSES to start (exit 2, `health.startup.refused`) on any probe
/// failure. The second composition root's startup gauntlet (mirrors the CLI's
/// slice-01 wire->probe->use).
///
/// @us-av-001 @real-io @adr-009 @adr-023 @infrastructure @error
#[test]
fn indexer_refuses_to_start_when_a_driven_adapter_probe_fails() {
    // -- Precondition: configure the indexer with an index store whose substrate
    // LIES about durability (a tmpfs/overlayfs fsync no-op — the container
    // substrate lie, DESIGN §6.3) OR an unreachable required ingest source. --
    //
    // -- Action: `openlore-indexer serve`. --
    //
    // -- Observable outcome: the binary REFUSES to start — exit code 2, a
    // health.startup.refused event with the failing reason
    // (storage.fsync_unhonored | indexer.ingest_source_unreachable |
    // identity.pubkey_decode_failed | indexer.capability_boundary_violated), and
    // NO ingest pass runs / NO query is served. The probes run BEFORE use
    // (ADR-009). This is the indexer analog of the slice-01 startup-refusal gate.
    //
    // Universe (port-exposed): exit code (2); the health.startup.refused reason;
    // absence of any indexed row / served query.
    todo!(
        "DELIVER (slice-05): configure a probe-failing adapter (fsync-lying store \
         OR unreachable required source); run `openlore-indexer serve`; assert \
         exit 2 + health.startup.refused{{reason}} + NO ingest/serve happened \
         (wire->probe->use; ADR-009/023)."
    );
}

// =============================================================================
// US-AV-001 — public-data-only ingest (I-AV-4 / WD-105)
// =============================================================================

/// AV-7 (US-AV-001 public-data-only; I-AV-4 / WD-105): the indexer ingests ONLY
/// PUBLIC signed claim records (the unauthenticated `listRecords` surface); it
/// makes no auth-scoped/private read and exposes no surveillance affordance. The
/// ingest-side half of the public-data honesty contract (the user-visible banner
/// is asserted in `appview_search.rs`).
///
/// @us-av-001 @real-io @i-av-4 @wd-105 @public-data
#[test]
fn indexer_ingests_only_public_records_no_private_read() {
    // -- Precondition: a fake source whose public listRecords surface returns
    // public signed claims, and whose (distinct) auth-scoped surface would return
    // private records if called. --
    //
    // -- Action: `openlore-indexer ingest`. --
    //
    // -- Observable outcome: only the PUBLIC records are read + indexed; the fake
    // source records that the indexer made NO auth-scoped/private call (the
    // public-data-only invariant, WD-105). No telemetry on claim CONTENTS is
    // emitted (DEVOPS privacy constraint) — structural counts + DIDs only.
    //
    // Universe (port-exposed): the set of source endpoints the indexer called
    // (public listRecords only; no auth-scoped read); the indexed rows (public
    // only).
    todo!(
        "DELIVER (slice-05): assert `openlore-indexer ingest` reads ONLY the \
         public listRecords surface (the fake source records zero auth-scoped \
         calls) and indexes only public records; no claim-content telemetry \
         (WD-105 / I-AV-4 public-data-only)."
    );
}
