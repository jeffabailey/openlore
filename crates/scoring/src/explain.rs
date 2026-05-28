//! `explain` — the per-claim decomposition types (the `--explain` contract).
//!
//! Every aggregate weight decomposes, by construction, into the
//! `Contribution` rows that produced it (I-GRAPH-2 anti-merging in
//! aggregates; the `--explain` running sum reproduces the weight by hand,
//! Gate 2). `author_did` is non-`Option` at the type level: a contribution
//! without attribution is a compile error, not a runtime check.

use claim_domain::{Cid, Did};

/// One claim's contribution to a pairing's weight — the auditable unit that
/// `--explain` renders. `subtotal` is the value the contribution adds to the
/// pairing weight; the pairing weight is the sum of its contributions'
/// subtotals (Gate 2).
///
/// `PartialEq` (not `Eq`) because the `f64` fields cannot derive `Eq` (NaN).
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    /// LOAD-BEARING: non-`Option` per I-GRAPH-2. The author this contribution
    /// is attributed to; never merged away.
    pub author_did: Did,

    /// The signed claim this contribution maps to (every contribution is one
    /// claim; Gate 5 analog for weights).
    pub cid: Cid,

    /// `= confidence` — the raw numeric `[0.0, 1.0]` scoring input (Gate 6).
    pub base: f64,

    /// `1.0` for the first author on the pairing; raised by
    /// `cfg.author_distinct_bonus` per additional distinct author.
    pub author_distinct_bonus: f64,

    /// `cfg.cross_project_triangulation_bonus` when this author asserts the
    /// object on `>= 2` distinct subjects, else `0.0`.
    pub cross_project_triangulation_bonus: f64,

    /// `= base * <author-distinct multiplier share> + triangulation`. The
    /// pairing weight is the sum of these (Gate 2).
    pub subtotal: f64,
}

impl Contribution {
    /// Read accessor for the load-bearing attribution (the domain stays
    /// immutable; this is the `--explain` renderer's entry point).
    pub fn author_did(&self) -> &Did {
        &self.author_did
    }
}
