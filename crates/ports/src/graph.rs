//! `graph` — the slice-04 attributed-claim feed + bounded-traversal ADTs.
//!
//! These are the in-memory value types the slice-04 read methods on
//! [`crate::StoragePort`] return (declarations only here — the
//! `adapter-duckdb` impls land in step 01-03). They live in `ports` (NOT
//! `scoring`) because BOTH the pure `scoring` core AND the `cli` composition
//! root consume them, and `scoring` already depends on `ports` (never the
//! reverse) — so this is the non-cyclic home (`data-models.md` §"In-memory
//! value types"; `component-boundaries.md` §`crates/ports`).
//!
//! **Invariant I-GRAPH-2 layer 1 (type-level anti-merging)**, the direct
//! descendant of slice-03's I-FED-1: every row the aggregate decomposes into
//! carries `author_did: Did` as a NON-`Option` field. [`AttributedClaim`] (the
//! pure scoring feed) and [`GraphEdge`] (one traversal edge) both make
//! dropping attribution a compile error, not a runtime check. Aggregation (the
//! weight) happens later in pure Rust, NEVER in SQL — so the per-claim /
//! per-edge rows always exist as the decomposition (WD-73).
//!
//! **Invariant I-GRAPH-5 (traversal invents no edges)**: [`GraphEdge`] carries
//! `claim_cid: Cid` (non-`Option`) — every edge maps to exactly one signed
//! claim row (Gate 5); the recursive CTE selects FROM existing rows only.
//
// SCAFFOLD: false  (data types are real; behavior arrives via StoragePort)

use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did};

use crate::federated_row::AuthorRelationship;

// -----------------------------------------------------------------------------
// AttributedClaim — the fully-attributed claim feed (pure scoring input)
// -----------------------------------------------------------------------------

/// A fully-attributed claim — the boundary value the pure `scoring` core
/// consumes and the per-row unit the `--object` / `--contributor` dimension
/// renderers display.
///
/// Defined HERE (not in `scoring`) so the single definition is shared by the
/// `cli` composition root, the extended [`crate::StoragePort`] read methods,
/// and `scoring::score` — `scoring` re-exports it (`pub use ports::AttributedClaim`).
/// Placing it in `ports` is the non-cyclic home: `scoring -> ports`, never the
/// reverse.
///
/// Mirrors the slice-03 [`crate::FederatedRow`] non-`Option<Did>` discipline
/// that makes attribution unviolatable (Gate 1 / I-GRAPH-2). Aggregation is
/// done in pure Rust over a `&[AttributedClaim]`, NEVER in SQL — so these
/// per-claim rows are the aggregate's decomposition.
///
/// `PartialEq` (not `Eq`) because `confidence: f64` cannot derive `Eq` (NaN).
#[derive(Debug, Clone, PartialEq)]
pub struct AttributedClaim {
    /// LOAD-BEARING: non-`Option` per I-GRAPH-2. A claim without an author DID
    /// is a compile error, not a runtime check.
    pub author_did: Did,
    /// The signed claim this attributed row maps to (Gate 5 analog).
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Numeric `[0.0, 1.0]` — the scoring input; the value shown equals the
    /// value scored (Gate 6).
    pub confidence: f64,
    pub composed_at: DateTime<Utc>,
    /// `You | SubscribedPeer | UnsubscribedCache` (slice-03 reuse) — drives the
    /// `--contributor` relationship label.
    pub relationship: AuthorRelationship,
}

// -----------------------------------------------------------------------------
// Query filters / graph nodes — the read-method inputs
// -----------------------------------------------------------------------------

/// The scoring-feed selector — which attributed claims to read for the pure
/// `scoring::score` call (the SQL is a `UNION ALL` over `claims` + `peer_claims`
/// with explicit `author_did`; NEVER a SQL aggregate — `data-models.md`
/// §"Scoring-feed query").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoringFilter {
    ByObject { object: String },
    BySubject { subject: String },
    ByContributor { author_did: Did },
}

/// A node in the contributor↔project↔philosophy graph — the seed for a
/// bounded [`crate::StoragePort::traverse_graph`] walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphNode {
    Philosophy { object: String },
    Project { subject: String },
    Contributor { author_did: Did },
}

/// The traversal depth bound (WD-76). `max_depth` defaults to 2; the
/// `--depth K` flag overrides it. The recursive CTE is bounded by this AND
/// cycle-safe (visited-set guard, ADR-021).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraversalBound {
    pub max_depth: u8,
}

impl TraversalBound {
    /// The WD-76 default traversal depth.
    pub const DEFAULT_MAX_DEPTH: u8 = 2;
}

impl Default for TraversalBound {
    fn default() -> Self {
        Self {
            max_depth: Self::DEFAULT_MAX_DEPTH,
        }
    }
}

// -----------------------------------------------------------------------------
// GraphEdge / TraversalResult — the bounded-traversal output
// -----------------------------------------------------------------------------

/// One traversal edge — the auditable unit a `--traverse` tree renders.
///
/// **I-GRAPH-5 (traversal invents no edges)**: `claim_cid` is `Cid`, NOT
/// `Option<Cid>` — every edge maps to exactly one signed claim row (Gate 5);
/// the recursive CTE never fabricates or interpolates an edge.
///
/// **I-GRAPH-2 (anti-merging)**: `author_did` is `Did`, NOT `Option<Did>` —
/// every edge carries its backing claim's attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: GraphNode,
    pub to: GraphNode,
    /// LOAD-BEARING: non-`Option` per I-GRAPH-5. Every edge maps to exactly
    /// one signed claim (Gate 5).
    pub claim_cid: Cid,
    /// LOAD-BEARING: non-`Option` per I-GRAPH-2. The edge's attribution
    /// (anti-merging) — the author DID of its backing claim.
    pub author_did: Did,
    /// 1-based depth at which this edge was discovered (`<= bound.max_depth`).
    pub depth: u8,
}

/// The result of a bounded [`crate::StoragePort::traverse_graph`] walk.
///
/// `omitted_edge_count > 0` ↔ edges existed beyond `max_depth` (renders the
/// "N edges omitted" line). `reached_bound` records whether the walk halted at
/// the depth bound (vs. exhausting the reachable subgraph first).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraversalResult {
    pub edges: Vec<GraphEdge>,
    pub omitted_edge_count: u32,
    pub reached_bound: bool,
}

// -----------------------------------------------------------------------------
// Property-based tests — type-level anti-merging defense (I-GRAPH-2 / 5)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Properties exercised here (port-to-port at the value-type boundary —
    //! the public struct fields ARE the port surface the adapter + scoring +
    //! cli observe):
    //!
    //! 1. `AttributedClaim.author_did` preserves the input `Did` byte-equal
    //!    for ANY constructible claim — the compile-time non-`Option`
    //!    constraint is enforced by the type, but this pins the field contract
    //!    so a future refactor to `Option<Did>` (or a projection that elides
    //!    the DID) reds the test (I-GRAPH-2 layer 1).
    //!
    //! 2. `AttributedClaim.confidence` round-trips the raw numeric `[0.0,1.0]`
    //!    byte-equal (Gate 6: the value shown == the value scored).
    //!
    //! 3. `GraphEdge` preserves BOTH `claim_cid` (Gate 5: every edge maps to
    //!    one signed claim — non-`Option`) AND `author_did` (I-GRAPH-2:
    //!    anti-merging — non-`Option`) byte-equal for any constructible edge.
    //!
    //! Universe (port-exposed observable surface): for `AttributedClaim` it is
    //! `{author_did, cid, confidence, relationship}` (the fields the scoring
    //! core + dimension renderers read). For `GraphEdge` it is
    //! `{claim_cid, author_did, depth}`.

    use super::*;
    use proptest::prelude::*;

    /// Strategy: arbitrary DID strings shaped like real DIDs (PLC + web), so
    /// the property holds across the variants the adapter will see. Reuses the
    /// same shape as `federated_row`'s `arb_did` (no introspection — the
    /// property is preservation, not format validation).
    fn arb_did() -> impl Strategy<Value = Did> {
        prop_oneof![
            "did:plc:[a-z0-9]{8,24}".prop_map(Did),
            "did:web:[a-z0-9.-]{4,32}".prop_map(Did),
        ]
    }

    /// Strategy: CIDv1 base32-lower-shaped strings (the `bafy...` prefix the
    /// claim-domain canonicalization emits).
    fn arb_cid() -> impl Strategy<Value = Cid> {
        "bafy[a-z0-9]{16,52}".prop_map(Cid)
    }

    fn arb_relationship() -> impl Strategy<Value = AuthorRelationship> {
        prop_oneof![
            Just(AuthorRelationship::You),
            Just(AuthorRelationship::SubscribedPeer),
            Just(AuthorRelationship::UnsubscribedCache),
        ]
    }

    proptest! {
        /// Property: `AttributedClaim` preserves `author_did` byte-for-byte AND
        /// round-trips the raw numeric `confidence`. The layer-1 anti-merging
        /// (I-GRAPH-2) + numeric-confidence (Gate 6) compile-time defenses — if
        /// anyone refactored `author_did` to `Option<Did>` or dropped the raw
        /// `f64` confidence in favor of a bucket, this reds.
        #[test]
        fn attributed_claim_preserves_attribution_and_confidence(
            did in arb_did(),
            cid in arb_cid(),
            subject in "[a-z][a-z0-9-]{0,31}",
            predicate in "[a-z][a-z0-9-]{0,31}",
            object in "[a-z][a-z0-9-]{0,31}",
            confidence in 0.0_f64..=1.0,
            composed_at in 0i64..2_000_000_000,
            relationship in arb_relationship(),
        ) {
            let claim = AttributedClaim {
                author_did: did.clone(),
                cid: cid.clone(),
                subject,
                predicate,
                object,
                confidence,
                composed_at: DateTime::<Utc>::from_timestamp(composed_at, 0).unwrap(),
                relationship,
            };

            // I-GRAPH-2 layer 1: author_did is preserved verbatim, non-Option.
            prop_assert_eq!(&claim.author_did, &did,
                "AttributedClaim.author_did must preserve the input Did byte-equal (I-GRAPH-2)");
            prop_assert_eq!(claim.author_did.0, did.0,
                "AttributedClaim.author_did.0 must equal the input Did.0 string");

            // Gate 5 analog: the backing claim cid is preserved verbatim.
            prop_assert_eq!(&claim.cid, &cid);

            // Gate 6: the raw numeric confidence is carried unchanged (the
            // value scored == the value shown — no bucketing at this layer).
            prop_assert!((claim.confidence - confidence).abs() < f64::EPSILON,
                "AttributedClaim.confidence must carry the raw numeric [0.0,1.0] (Gate 6)");
        }

        /// Property: `GraphEdge` preserves BOTH its backing `claim_cid`
        /// (Gate 5 / I-GRAPH-5: every edge maps to one signed claim) AND its
        /// `author_did` (I-GRAPH-2: anti-merging) byte-equal. Both are
        /// non-`Option` at the type level; this pins the accessor contract.
        #[test]
        fn graph_edge_preserves_claim_cid_and_author_did(
            from_object in "[a-z][a-z0-9-]{0,31}",
            to_subject in "[a-z][a-z0-9-]{0,31}",
            claim_cid in arb_cid(),
            author_did in arb_did(),
            depth in 1u8..=8,
        ) {
            let edge = GraphEdge {
                from: GraphNode::Philosophy { object: from_object },
                to: GraphNode::Project { subject: to_subject },
                claim_cid: claim_cid.clone(),
                author_did: author_did.clone(),
                depth,
            };

            // I-GRAPH-5 / Gate 5: every edge maps to exactly one signed claim.
            prop_assert_eq!(&edge.claim_cid, &claim_cid,
                "GraphEdge.claim_cid must preserve the backing claim cid (Gate 5 — traversal invents no edges)");
            // I-GRAPH-2: the edge carries its backing claim's attribution.
            prop_assert_eq!(&edge.author_did, &author_did,
                "GraphEdge.author_did must preserve the backing claim's author (I-GRAPH-2 anti-merging)");
            prop_assert_eq!(edge.depth, depth);
        }
    }

    /// Single-example: the `TraversalBound` default is the WD-76 depth of 2.
    ///
    /// bypass: a single fixed constant has no equivalence class to sample over;
    /// a property would be vacuous. This pins the WD-76 SSOT so a silent edit
    /// to the default depth is caught.
    #[test]
    fn traversal_bound_default_is_wd76_depth_two() {
        assert_eq!(
            TraversalBound::default().max_depth,
            2,
            "WD-76 default traversal depth"
        );
        assert_eq!(TraversalBound::DEFAULT_MAX_DEPTH, 2);
    }
}
