//! serverless-philosophy-federation — cross-instance federated read.
//!
//! Slice-05: US-SF-006 (expand my dataset by pulling from ANOTHER user's
//! Cloudflare instance — federated read, J-003 transport delta; requirement 4).
//! This REUSES the shipped J-003 `openlore peer add` / `openlore peer pull`
//! flow verbatim; the ONLY delta is that the peer's DID document
//! `serviceEndpoint` resolves to their Cloudflare instance instead of a
//! bsky.social PDS (OD-SF-3 / DDD-5). Per ADR-062 §4 the record transport is
//! the byte-preserving OPAQUE read (`GET /manifest` + `GET /records/:cid`), and
//! the Rust side recomputes the CID + verifies the signature LOCALLY — reusing
//! J-003's verify / per-author attribution / anti-merging (peer claims land in
//! `peer_claims`, never merged with the reader's own).
//!
//! Only the peer's CLI↔Worker seam is faked (`FakeInstance` as a peer, with the
//! DID-doc resolver pointing at it); the local verify/store path is REAL.
//! Layer 3; example-only (Mandate 11 — tampered/unreachable sad paths are named
//! examples, never PBT). The J-003 verify machinery itself is already shipped
//! and unit-tested; these scenarios assert the TRANSPORT DELTA only.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-006 — happy path
// =============================================================================

/// CI-1: Rachel's DID `did:plc:rachel-test` resolves her serviceEndpoint to
/// `https://openlore.rachel.workers.dev`. Maria has subscribed. When she runs
/// `openlore peer pull`, Rachel's claims are fetched from her Cloudflare
/// instance via the opaque read, each signature is verified and each CID
/// recomputed before storage, and the claims land in `peer_claims` attributed
/// to Rachel — never merged with Maria's own. (US-SF-006 · AC 1-2-3 · UAT #1 ·
/// Domain example 1.)
///
/// @us-sf-006 @driving_port @real-io @j-003 @happy
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-006 pull from peer Cloudflare instance)"]
fn peer_pull_fetches_a_peers_claims_from_their_cloudflare_instance_into_peer_claims_attributed() {
    todo!(
        "DELIVER: Given a peer FakeInstance for did:plc:rachel-test (DID-doc resolves to it) \
         holding 7 records, 5 new to Maria, and a subscription created via real `peer add`; \
         When `openlore peer pull`; Then Rachel's records are read via the opaque transport, \
         5/5 verified (sig + recomputed CID), stored in peer_claims attributed to Rachel, \
         author_claims UNCHANGED (anti-merging). \
         Universe: peer_storage.claims.row_count_by_author[rachel], author_claims.row_count (unchanged)."
    );
}

// =============================================================================
// US-SF-006 — error (tampered peer claim)
// =============================================================================

/// CI-2: One of Rachel's published records fails signature verification. When
/// Maria runs `openlore peer pull`, that record is rejected and reported, the
/// valid records are stored normally, and the exit code is non-zero to flag the
/// rejection (J-003 semantics carry over unchanged). (US-SF-006 · AC 3 · UAT #2
/// · Domain example 3.)
///
/// @us-sf-006 @error @j-003
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-006 tampered peer claim rejected)"]
fn peer_pull_rejects_a_tampered_peer_claim_and_stores_the_valid_ones() {
    todo!(
        "DELIVER: Given a peer FakeInstance whose record set includes one tampered signature; \
         When `openlore peer pull`; Then the tampered record is rejected + reported, the honest \
         records are stored in peer_claims, and exit is non-zero. Reuses J-003 KPI-FED-6 discipline."
    );
}

// =============================================================================
// US-SF-006 — boundary (unreachable peer instance)
// =============================================================================

/// CI-3: Rachel's Cloudflare instance is unreachable. When Maria runs
/// `openlore peer pull` with multiple subscribed peers, Rachel's pull is skipped
/// and reported, the other peers' pulls proceed, and the exit code is non-zero
/// overall (same fault isolation as any unreachable PDS in J-003). (US-SF-006 ·
/// UAT #3 · Domain example 2.)
///
/// @us-sf-006 @error @j-003
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-006 unreachable peer instance skips only that peer)"]
fn peer_pull_skips_only_the_unreachable_peer_instance_and_proceeds_with_others() {
    todo!(
        "DELIVER: Given two subscribed peers, one reachable and one \
         FakeInstance::unreachable(); When `openlore peer pull`; Then the unreachable peer is \
         skipped + reported, the reachable peer's claims are stored, exit is non-zero overall."
    );
}

// =============================================================================
// US-SF-006 — guardrail (anti-merging / peer_claims separation)
// =============================================================================

/// CI-4: A cross-instance pull NEVER merges peer claims with the reader's own.
/// Peer claims land in the separate `peer_claims` layer; `author_claims` are
/// never modified (J-003 invariant, carried over). (US-SF-006 · AC 2 ·
/// anti-merging.)
///
/// @us-sf-006 @anti-merging @j-003 @guardrail
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-006 peer_claims never merged with own)"]
fn peer_pull_from_an_instance_never_merges_peer_claims_with_my_own() {
    todo!(
        "DELIVER: capture author_claims universe BEFORE; When `openlore peer pull` from a peer \
         instance; Then peer_storage.claims grows, author_claims.{{row_count, cids}} UNCHANGED, \
         and no row is attributed to a DID other than its true author (anti-merging)."
    );
}

// =============================================================================
// US-SF-006 — verify-before-trust on the opaque read (KPI-SF-1 / J-003)
// =============================================================================

/// CI-5: The opaque read is byte-preserving, so the Rust side recomputes each
/// peer claim's CID from the returned bytes and verifies it BEFORE store —
/// exactly the round-trip integrity discipline that makes cross-instance reuse
/// "free" (ADR-062 §4). A byte-divergent read would fail the recompute and be
/// rejected. (US-SF-006 · KPI-SF-1 · J-003 verify-before-trust.)
///
/// @us-sf-006 @kpi-sf-1 @j-003
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-006 recompute + verify CID before store)"]
fn peer_pull_recomputes_each_peer_claim_cid_from_the_opaque_read_before_store() {
    todo!(
        "DELIVER: Given a peer FakeInstance serving byte-verbatim records; When `openlore peer \
         pull`; Then each stored record's CID was recomputed in Rust from the returned bytes and \
         byte-matched before insert (opaque byte-preserving read — ADR-062 §4)."
    );
}
