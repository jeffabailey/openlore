//! `store_read` — the slice-06 READ-ONLY store port (ADR-030).
//!
//! The `openlore ui` viewer reads the operator's OWN node store as
//! server-rendered HTML over a port that exposes NO write/sign surface. This is
//! the structural read-only guarantee (I-VIEW-1 / ADR-030): a `StoreReadPort`
//! trait object cannot mutate the store because the trait declares no mutation
//! method. The adapter (`adapter-duckdb`) implements it over the SAME shared
//! connection the CLI writes through — there is no second handle, no second
//! file (BR-VIEW-4).
//!
//! ## Boundary ADTs (data-models.md §"Read-side query shapes")
//!
//! - [`ClaimRow`] — one own-claim row projected from the `claims` table:
//!   subject/predicate/object/confidence (the DOUBLE numeric, rendered VERBATIM
//!   by the pure viewer core, FR-VIEW-8)/author_did/composed_at/cid. A FLAT,
//!   serialization-friendly shape (DTO, not the rich `SignedClaim`).
//! - [`PageRequest`] — the offset/limit pagination request the viewer derives
//!   from the `?page=N` query (page size 50, ADR-030).
//! - [`Page<T>`] — a page of rows plus the total count, so the renderer can
//!   show the "N–M of TOTAL" position indicator (FR-VIEW-6).
//! - [`StoreReadError`] — read failures, surfaced as a plain-language error by
//!   the viewer (NFR-VIEW-6), never a raw stack trace.

use chrono::{DateTime, Utc};
use claim_domain::Did;

use crate::AttributedClaim;

/// One own-claim row from the `claims` table, projected for the read-only
/// viewer. A FLAT DTO (not the rich `claim_domain::SignedClaim`): the viewer
/// renders these fields verbatim and never needs the signature/canonical-CBOR
/// shape. `confidence` is the stored DOUBLE — the pure viewer core renders it
/// VERBATIM as `0.90` (FR-VIEW-8), never `0.9` nor `90%`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimRow {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub author_did: String,
    pub composed_at: DateTime<Utc>,
}

/// One own-claim's FULL detail, projected for the read-only detail view
/// (`/claims/{cid}`, US-VIEW-002). The flat claim fields PLUS the COMPLETE
/// `evidence[]` array the list view summarizes away (FR-VIEW-3) — ordered by
/// the `claim_evidence.ordinal` column so the operator sees the evidence in the
/// order it was attached. A FLAT DTO (not the rich `SignedClaim`): the detail
/// renderer reads these fields verbatim. `confidence` is the stored DOUBLE,
/// rendered VERBATIM (FR-VIEW-8).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimDetail {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub author_did: String,
    pub composed_at: DateTime<Utc>,
    /// The claim's evidence URLs, ordered by `claim_evidence.ordinal` ascending
    /// (the order they were attached). Empty when the claim was signed without
    /// evidence (the detail view then shows an explicit "no evidence attached"
    /// state — step 02-02).
    pub evidence: Vec<String>,
}

/// Where a federated peer claim came from — its peer ORIGIN (US-VIEW-003 /
/// FR-VIEW-4). There is no `peer_origin` column in the slice-03 schema; the
/// origin IS the pair (`author_did`, `fetched_from_pds`) per data-models.md.
///
/// Modeled as an ADT so the "mine vs federated never ambiguous" contract
/// (BR-VIEW-5) and the future unknown-origin path (step 03-03 / V-10) are both
/// total at the type level:
///
/// - [`PeerOrigin::Known`] — the schema-guaranteed common case: `author_did` is
///   NON-EMPTY (the slice-03 CHECK enforces `author_did <> ''`). Carries the
///   peer's DID + the PDS it was fetched from.
/// - [`PeerOrigin::Unknown`] — the DEFENSIVE path: a row whose `author_did` is
///   blank/absent (data that predates/bypasses the CHECK). Step 03-01 only
///   PRODUCES `Known` (the production `peer pull` path always sets a non-empty
///   `author_did`); the `Unknown` variant is here so step 03-03 (V-10) is a
///   clean, total extension — the renderer matches both arms, never drops a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerOrigin {
    /// A peer claim with a known origin: the peer's `author_did` (NON-EMPTY) and
    /// the PDS endpoint it was `fetched_from`.
    Known {
        /// The peer's DID — the bare `did:plc:...` stored in
        /// `peer_claims.author_did`. Rendered VERBATIM (attribution discipline,
        /// I-FED-1 / FR-VIEW-4): the operator sees exactly who authored it.
        author_did: String,
        /// The PDS endpoint the claim was fetched from
        /// (`peer_claims.fetched_from_pds`).
        fetched_from_pds: String,
    },
    /// A peer claim whose origin is absent/blank (defensive). Renders labeled
    /// "unknown" rather than being dropped (step 03-03 / V-10).
    Unknown,
}

/// One federated PEER-claim row from the `peer_claims` table (slice-03),
/// projected for the read-only Peer Claims view (`/peer-claims`, US-VIEW-003).
/// A FLAT DTO (not the rich `SignedClaim`). DISTINCT from [`ClaimRow`] (own
/// claims) so the viewer can render peers on a SEPARATE surface where
/// "mine vs federated" is never ambiguous (BR-VIEW-5).
///
/// `confidence` is the stored DOUBLE, rendered VERBATIM (FR-VIEW-8). `origin`
/// carries the peer ORIGIN ([`PeerOrigin`]) — the peer's `author_did` +
/// `fetched_from_pds`, projected verbatim (there is no `peer_origin` column).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerClaimRow {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    /// The peer ORIGIN — who authored this federated claim + the PDS it came
    /// from. The "distinct from own" / attribution surface (BR-VIEW-5).
    pub origin: PeerOrigin,
    pub composed_at: DateTime<Utc>,
}

/// One row of an entity SURVEY — a single signed claim about the queried entity,
/// projected for the read-only graph-traversal views (`/project` + `/philosophy`,
/// slice-10 / ADR-042/043/044/045 / data-models.md §1). A FLAT DTO (not the rich
/// `SignedClaim`): the pure `viewer-domain` grouper reads these fields verbatim.
///
/// Both `author_did` and `cid` are NON-`Option` (the load-bearing anti-merging +
/// no-invented-edge contract, I-GT-3 / I-GT-4): every survey edge maps to exactly
/// ONE signed claim and is ALWAYS attributed to its author — two same-content
/// claims by different authors stay TWO `SurveyRow`s (never merged/averaged).
/// `confidence` is the stored DOUBLE, rendered VERBATIM (`0.90`, never `0.9`/`90%`;
/// I-GT-5). `origin` carries the peer ORIGIN ([`PeerOrigin`]: `Own` projects to a
/// `Known { author_did, fetched_from_pds: "" }` marker; a pulled peer carries its
/// PDS) so the viewer can distinguish "mine vs peer" (BR-VIEW-5 carried).
/// `composed_at` is used ONLY for ordering/tiebreak — it is NOT displayed on an
/// edge row.
#[derive(Debug, Clone, PartialEq)]
pub struct SurveyRow {
    /// The claim's author DID — NON-`Option`, NEVER elided (anti-merging, I-GT-3).
    pub author_did: String,
    /// The claim's canonical CID — NON-`Option`; every edge maps to exactly one
    /// signed claim (no invented edges, I-GT-4).
    pub cid: String,
    /// The claim subject (the project key, e.g. `github:rust-lang/cargo`).
    pub subject: String,
    /// The claim predicate (e.g. `embodiesPhilosophy`) — carried, not grouped on.
    pub predicate: String,
    /// The claim object (the philosophy key, e.g. a philosophy NSID).
    pub object: String,
    /// The stored confidence DOUBLE — rendered VERBATIM (I-GT-5), never rounded.
    pub confidence: f64,
    /// The peer ORIGIN: an `Own` row projects to `Known { author_did,
    /// fetched_from_pds: "" }`; a pulled peer row carries its PDS endpoint. Lets the
    /// viewer distinguish "mine vs peer" (BR-VIEW-5).
    pub origin: PeerOrigin,
    /// The claim `composed_at` — used ONLY for ordering/tiebreak, never displayed.
    pub composed_at: DateTime<Utc>,
}

/// One COUNTER targeting a given claim, projected for the read-only
/// counter-claim thread on the detail view (`/claims/{cid}`, slice-11 /
/// US-CT-002 / ADR-046/047). A FLAT DTO (not the rich `SignedClaim`): the pure
/// `viewer-domain` projection reads these fields verbatim into a `CounterThread`.
///
/// A counter is an ordinary signed claim carrying a `references[].type ==
/// counters` entry whose `cid` is the countered target (ADR-015). The thread is
/// the ADR-046 2-step read: step A is the INDEXED `claim_references` /
/// `peer_claim_references` lookup by `referenced_cid` (UNION ALL, attributed) for
/// the counter's `author_did` + `cid` + `source_table`; step B reads each
/// counter's on-disk `SignedClaim` artifact for its free-text `reason` (the
/// reason is NOT a DB column — it lives in the artifact, ADR-015).
///
/// Both `author_did` and `cid` are NON-`Option` (the anti-merging + attribution
/// contract, I-CT-3): every counter maps to exactly ONE signed claim and is
/// ALWAYS attributed to its author — two counters by different authors stay TWO
/// `CounterClaimRow`s (never merged into a "disputed by N" aggregate). `reason`
/// is `Option<String>` (ADR-015 wire-optional): `None` for a counter authored by
/// a non-OpenLore client that omitted the reason (the viewer then renders an
/// explicit "no reason provided" state — never a blank line). `confidence` is the
/// stored DOUBLE; `composed_at` is used ONLY for deterministic ordering;
/// `origin` carries the peer ORIGIN ([`PeerOrigin`]: an own counter projects to
/// `Known { author_did, fetched_from_pds: "" }`; a pulled peer counter carries
/// its PDS) so the viewer can distinguish "mine vs peer".
#[derive(Debug, Clone, PartialEq)]
pub struct CounterClaimRow {
    /// The counter's author DID — NON-`Option`, NEVER elided (anti-merging,
    /// I-CT-3). Rendered VERBATIM as the counter's attribution.
    pub author_did: String,
    /// The counter's own content-addressed CID — NON-`Option`; the render-only
    /// `<a href="/claims/{cid}">` one-hop drill-link target (depth-1, ADR-047).
    pub cid: String,
    /// The counter's free-text `--reason`, read from its on-disk `SignedClaim`
    /// artifact (`unsigned.reason`). `None` for the ADR-015 wire-optional
    /// empty-reason edge → the viewer renders "no reason provided".
    pub reason: Option<String>,
    /// The counter's stored confidence DOUBLE — carried for completeness;
    /// rendered VERBATIM if shown (never rounded).
    pub confidence: f64,
    /// The counter `composed_at` — used ONLY for deterministic ordering, never a
    /// re-weight of the countered claim (shown-never-applied, I-CT-2).
    pub composed_at: DateTime<Utc>,
    /// The counter's peer ORIGIN: an own counter projects to `Known {
    /// author_did, fetched_from_pds: "" }`; a pulled peer counter carries its PDS
    /// endpoint. Lets the viewer distinguish "mine vs peer".
    pub origin: PeerOrigin,
}

/// One ACTIVE peer-subscription row, projected for the read-only Peer
/// Subscriptions view (`/peers`, slice-15 / US-PS-002 / ADR-052). A FLAT DTO
/// (not the rich `peer_storage::PeerSubscription`): the pure `viewer-domain`
/// `PeersView` renders these fields verbatim into one attributed row per peer.
///
/// Materialized by [`StoreReadPort::list_active_peer_subscriptions`] from ONE
/// aggregate query — `peer_subscriptions LEFT JOIN peer_claims ON author_did =
/// peer_did`, `WHERE removed_at IS NULL`, `GROUP BY` the subscription identity,
/// `COUNT(pc.cid)` per peer (ADR-052 Q1/Q2). Only ACTIVE subscriptions
/// (`removed_at IS NULL`) ever become a `PeerSubscriptionSummary`: a soft-removed
/// row is residue, never a summary (I-PS-2).
///
/// `peer_did` is NON-`Option` (the load-bearing per-peer attribution contract,
/// I-PS-3): each peer is its own attributed row keyed by its DID, NEVER merged
/// into a faceless "all peers" aggregate. `peer_handle` is NON-`Option` (the
/// stored handle column). `local_claim_count` is the per-peer `COUNT(pc.cid)`
/// from the LEFT JOIN — a never-pulled peer stays at `0` (the LEFT JOIN keeps the
/// row; `COUNT(pc.cid)` counts the NULL right side as `0`, not `1` — DD-PS-2),
/// NEVER summed/averaged across peers (anti-merging, J-003a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSubscriptionSummary {
    /// The peer's DID — NON-`Option`, the bare `did:plc:...` stored in
    /// `peer_subscriptions.peer_did`. Rendered VERBATIM (attribution, I-PS-3) +
    /// the bare-DID source for the render-only `openlore peer remove <did>`
    /// revocation command.
    pub peer_did: String,
    /// The peer's stored handle (`peer_subscriptions.peer_handle`). NON-`Option`
    /// (the schema column is non-null); carried for display alongside the DID.
    pub peer_handle: String,
    /// When the operator subscribed (`peer_subscriptions.subscribed_at`). Carried
    /// for display/ordering; the active-subscriptions read orders by it.
    pub subscribed_at: DateTime<Utc>,
    /// The PER-PEER cached local claim count — `COUNT(pc.cid)` from the LEFT JOIN
    /// to `peer_claims` on `author_did = peer_did`. `0` for a subscribed-but-
    /// never-pulled peer (DD-PS-2); NEVER a merged total across peers (J-003a).
    pub local_claim_count: u64,
}

/// An offset/limit pagination request over the own-claim store. The viewer
/// translates a `?page=N` query (page size 50, ADR-030) into one of these: the
/// offset/limit selects one page, the bounds + position indicator are projected
/// by the pure `viewer-domain` `PageView` over the returned [`Page::total`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Zero-based row offset into the ordered result set.
    pub offset: u64,
    /// Maximum number of rows to return.
    pub limit: u64,
}

/// A page of rows plus the total matching count. `total` lets the renderer show
/// the "N–M of TOTAL" position indicator + decide whether pagination controls
/// are needed (FR-VIEW-6) — it is the whole-set `COUNT(*)`, not `rows.len()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: u64,
}

/// Why a read-only store read failed. The viewer surfaces these as
/// plain-language messages (NFR-VIEW-6), never a raw stack trace. `Unreadable`
/// is the store-readability probe failure (another process holds the file;
/// ADR-030 §Earned-Trust step 1).
#[derive(Debug, thiserror::Error)]
pub enum StoreReadError {
    /// The store could not be opened/read (locked by another process, missing,
    /// permissions). Carries a plain-language detail for the operator.
    #[error("store unreadable: {detail}")]
    Unreadable { detail: String },
    /// A read query failed for a reason other than store-unreadability.
    #[error("store read query failed: {detail}")]
    QueryFailed { detail: String },
}

/// The READ-ONLY store port the `openlore ui` viewer reads (ADR-030). Exposes
/// ONLY read methods — there is NO `write_*` / `sign` method on this trait, so a
/// `Box<dyn StoreReadPort>` is structurally incapable of mutating the store
/// (I-VIEW-1). The adapter shares the CLI's connection (BR-VIEW-4).
pub trait StoreReadPort: Send + Sync {
    /// List own claims ordered for display (composed_at DESC per ADR-030),
    /// paginated by `request` (the `?page=N` offset/limit). Read-only SQL only;
    /// returns the page rows plus the whole-set `total` for the indicator.
    fn list_claims(&self, request: PageRequest) -> Result<Page<ClaimRow>, StoreReadError>;

    /// Total number of own claims in the store. Used by the store-readability
    /// startup probe (a `COUNT(*)` sentinel read) AND the position indicator.
    fn count_claims(&self) -> Result<usize, StoreReadError>;

    /// Fetch ONE own claim by CID together with its COMPLETE, ordinal-ordered
    /// `evidence[]` (US-VIEW-002 / FR-VIEW-3). Returns `Ok(None)` when no claim
    /// with that CID exists (the viewer renders a guided not-found — step 02-03),
    /// `Ok(Some(detail))` for a known CID. Read-only SQL only: a SELECT over
    /// `claims` joined to `claim_evidence` by `cid`, ordered by `ordinal`.
    fn get_claim(&self, cid: &str) -> Result<Option<ClaimDetail>, StoreReadError>;

    /// List federated PEER claims ordered for display (composed_at DESC, mirroring
    /// `list_claims`), paginated by `request` (US-VIEW-003 / FR-VIEW-4). Read-only
    /// SQL only — a SELECT over the SAME shared connection's `peer_claims` table
    /// (slice-03). Each row carries its peer ORIGIN ([`PeerOrigin`]: the peer's
    /// `author_did` + `fetched_from_pds`) so the viewer renders peers on a SEPARATE
    /// surface, "mine vs federated" never ambiguous (BR-VIEW-5).
    fn list_peer_claims(&self, request: PageRequest) -> Result<Page<PeerClaimRow>, StoreReadError>;

    /// Total number of federated peer claims in the store. The Peer Claims
    /// position indicator + empty-state decision (US-VIEW-003) read this
    /// `COUNT(*)` over `peer_claims`.
    fn count_peer_claims(&self) -> Result<usize, StoreReadError>;

    /// Total number of ACTIVE peer subscriptions in the store — the count-only
    /// sibling of [`StoreReadPort::count_claims`]/[`StoreReadPort::count_peer_claims`]
    /// for the landing dashboard's at-a-glance summary (`GET /`, slice-17 /
    /// US-LD-000/001 / ADR-054 D3). Read-only `COUNT(*)` aggregate — NO mutation
    /// method is added to this trait (ADR-030 / I-VIEW-1): a `Box<dyn StoreReadPort>`
    /// stays structurally incapable of subscribing/unsubscribing.
    ///
    /// Active-only by construction (BR-LD-2): a `COUNT(*)` over `peer_subscriptions`
    /// `WHERE removed_at IS NULL` — the SAME active-only definition as
    /// [`StoreReadPort::list_active_peer_subscriptions`] (ADR-052). A peer soft-removed
    /// via the CLI `peer remove` (`removed_at` set, cached claims retained on disk) is
    /// EXCLUDED — its residue never inflates the front-door active-peer count. Touches
    /// ONLY `peer_subscriptions` (NO JOIN — it counts subscriptions, not claims).
    /// Returns `Ok(0)` for a store with no active subscriptions (a SUCCESSFUL read of
    /// zero, DISTINCT from a failed read).
    fn count_active_peer_subscriptions(&self) -> Result<usize, StoreReadError>;

    /// Total number of the operator's OWN claims that have been COUNTERED — the
    /// count-only sibling of [`StoreReadPort::count_claims`] /
    /// [`StoreReadPort::count_peer_claims`] /
    /// [`StoreReadPort::count_active_peer_subscriptions`] for the landing dashboard's
    /// at-a-glance summary AND the `/claims` list header (`GET /` + `GET /claims`,
    /// slice-18 / US-CC-000/001/002 / ADR-055 D1). Read-only aggregate — NO mutation
    /// method is added to this trait (ADR-030 / C-1): a `Box<dyn StoreReadPort>` stays
    /// structurally incapable of mutating.
    ///
    /// A PRESENCE count by construction (C-4 / BR-CC-1): `COUNT(DISTINCT own cid)`
    /// where the own CID appears as a COUNTERED `referenced_cid` across the two indexed
    /// ref tables (`claim_references` ∪ `peer_claim_references`, `ref_type='counters'`)
    /// via a de-duped `UNION` IN-set membership test — so a claim countered by N peers
    /// counts ONCE (no JOIN-fanout, NEVER a "disputed by N" total). Own-only by query
    /// shape (the outer table is `claims` — a countered PEER claim is excluded, not
    /// filtered). Invariant to store size (both ref columns indexed, ADR-048).
    /// Returns `Ok(0)` for a store where nothing of the operator's is countered (a
    /// SUCCESSFUL read of zero, DISTINCT from a FAILED read — `0 ≠ missing`, the shell
    /// maps the `Result` to `Option` via `.ok()`).
    fn count_countered_own_claims(&self) -> Result<usize, StoreReadError>;

    /// Read the LOCAL attributed-claim feed for ONE contributor (`/score`,
    /// slice-09 / ADR-039/040/041 / I-CS-5): every claim authored by `contributor`
    /// across all subjects, from the operator's OWN `claims` table UNION ALL the
    /// LOCAL `peer_claims` table — NO network. The pure `scoring::score` core
    /// aggregates this `Vec<AttributedClaim>` into the ranked `WeightedView`; the
    /// weight is NEVER computed in SQL, so the per-claim rows always exist as the
    /// aggregate's decomposition (I-GRAPH-2 / WD-73).
    ///
    /// READ-ONLY by construction: this is a SELECT over the SAME shared connection
    /// the CLI writes through (BR-VIEW-4) — there is NO mutation method on this
    /// trait, so a `Box<dyn StoreReadPort>` cannot change the store (I-VIEW-1). The
    /// `UNION ALL` projects `author_did` EXPLICITLY (NEVER a merging `JOIN`/`GROUP
    /// BY`), so two same-content claims by different authors stay TWO attributed
    /// rows (anti-merging). Returns an EMPTY vec for a contributor with no local
    /// rows (the viewer renders the guided `NoClaims` state — never an error).
    fn query_contributor_scoring_feed(
        &self,
        contributor: &Did,
    ) -> Result<Vec<AttributedClaim>, StoreReadError>;

    /// Read the LOCAL attributed SURVEY for ONE project subject (`/project`,
    /// slice-10 / ADR-042/043/044/045 / I-GT-2): every signed claim ABOUT
    /// `subject`, from the operator's OWN `claims` table UNION ALL the LOCAL
    /// `peer_claims` table — NO network. The pure `viewer-domain::group_project`
    /// core groups this `Vec<SurveyRow>` by `object` (the philosophy embodied) into
    /// the `TraversalView`; the grouping is NEVER done in SQL (I-GT-3).
    ///
    /// READ-ONLY by construction: a SELECT over the SAME shared connection the CLI
    /// writes through (BR-VIEW-4) — there is NO mutation method on this trait
    /// (I-VIEW-1). The `UNION ALL` projects `author_did` + `cid` EXPLICITLY (NEVER a
    /// merging `JOIN`/`GROUP BY`/`AVG`), so two same-content claims by different
    /// authors stay TWO attributed rows (anti-merging, I-GT-3 / I-GT-4). LOCAL only;
    /// returns an EMPTY vec for a subject with no local rows (the viewer renders the
    /// guided `NoClaims` state — never an error, I-GT-4).
    fn query_project_survey(&self, subject: &str) -> Result<Vec<SurveyRow>, StoreReadError>;

    /// Read the LOCAL attributed SURVEY for ONE philosophy object (`/philosophy`,
    /// slice-10 / ADR-042/043/044/045 / I-GT-2) — the SYMMETRIC mirror of
    /// [`StoreReadPort::query_project_survey`], swapping subject↔object: every signed
    /// claim whose `object` is the queried philosophy, from the operator's OWN `claims`
    /// table UNION ALL the LOCAL `peer_claims` table — NO network. The pure
    /// `viewer-domain::group_philosophy` core groups this `Vec<SurveyRow>` by `subject`
    /// (the project that embodies the philosophy) into the `TraversalView`; the grouping
    /// is NEVER done in SQL (I-GT-3).
    ///
    /// READ-ONLY by construction: a SELECT over the SAME shared connection the CLI writes
    /// through (BR-VIEW-4) — there is NO mutation method on this trait (I-VIEW-1). The
    /// `UNION ALL` projects `author_did` + `cid` EXPLICITLY (NEVER a merging `JOIN`/`GROUP
    /// BY`/`AVG`), so two same-content claims by different authors stay TWO attributed
    /// rows (anti-merging, I-GT-3 / I-GT-4). LOCAL only; returns an EMPTY vec for a
    /// philosophy with no local rows (the viewer renders the guided `NoClaims` state —
    /// never an error, I-GT-4).
    fn query_philosophy_survey(&self, object: &str) -> Result<Vec<SurveyRow>, StoreReadError>;

    /// Read the LOCAL counter-claim thread for ONE claim CID (`/claims/{cid}`,
    /// slice-11 / US-CT-002 / ADR-046/047): every signed claim that COUNTERS
    /// `target_cid` (carries a `references[].type == counters` entry referencing
    /// it), from the operator's OWN `claims` table UNION ALL the LOCAL
    /// `peer_claims` table — NO network. The pure `viewer-domain` projection turns
    /// this `Vec<CounterClaimRow>` into a `CounterThread` rendered BENEATH the
    /// verbatim claim; the original claim is NEVER re-weighted/filtered/merged by a
    /// counter (shown-never-applied, I-CT-2).
    ///
    /// READ-ONLY by construction: the ADR-046 2-step read over the SAME shared
    /// connection the CLI writes through (BR-VIEW-4) — there is NO mutation method
    /// on this trait (I-VIEW-1). Step A is the INDEXED UNION-ALL ref lookup
    /// (`claims` JOIN `claim_references` ∪ `peer_claims` JOIN `peer_claim_references`,
    /// `WHERE referenced_cid = ? AND ref_type = 'counters'`) projecting the
    /// counter's `author_did` + `cid` + `source_table` EXPLICITLY (NEVER a merging
    /// JOIN/GROUP BY/AVG — two counters by different authors stay TWO rows,
    /// anti-merging I-CT-3). Step B reads each counter's on-disk `SignedClaim`
    /// artifact for its free-text `reason` (the reason is NOT a DB column — ADR-015).
    /// LOCAL only; returns an EMPTY vec for an UN-countered CID (the projection then
    /// yields `CounterThread::None` — the detail renders the claim alone, no
    /// empty-state noise, I-CT-2).
    fn query_counter_claims(
        &self,
        target_cid: &str,
    ) -> Result<Vec<CounterClaimRow>, StoreReadError>;

    /// READ-ONLY counter-PRESENCE lookup for a WHOLE `/claims` list page (`GET /claims`,
    /// slice-12 / US-LF-002/003 / ADR-048): given the page's claim CIDs, return the
    /// SUBSET that has ≥1 counter — a presence SET (`HashSet<String>`), NEVER a count.
    /// The at-a-glance "Countered" list flag is set from membership in this set; the
    /// pure `viewer-domain` render stays a total function of (page, presence).
    ///
    /// ONE aggregate query (NO N+1, I-LF-8): a single `referenced_cid IN (...)`
    /// UNION-ALL DISTINCT read over the INDEXED `claim_references` ∪
    /// `peer_claim_references` tables (`ref_type = 'counters'`), ref-tables-only (no
    /// JOIN to `claims`/`peer_claims`, no per-row artifact read — the flag carries no
    /// reason text). LOCAL only (renders offline, I-LF-5); the input CIDs are BOUND
    /// placeholders, never string-interpolated (injection-safe). An EMPTY input slice
    /// (an empty / all-un-countered page) returns `Ok(HashSet::new())` WITHOUT
    /// preparing a query — an empty `IN ()` is a SQL error.
    ///
    /// READ-ONLY by construction: a SELECT over the SAME shared connection the CLI
    /// writes through (BR-VIEW-4) — there is NO mutation method on this trait
    /// (I-VIEW-1 / I-LF-1). The presence read NEVER re-orders / re-pages / re-counts /
    /// re-weights the list — it is a SEPARATE set lookup the shell maps onto rows AFTER
    /// `list_claims` pages them (shown-never-applied, additive only; I-LF-2 / I-LF-4).
    fn counter_presence_for(
        &self,
        cids: &[String],
    ) -> Result<std::collections::HashSet<String>, StoreReadError>;

    /// List the operator's ACTIVE peer subscriptions for the read-only Peer
    /// Subscriptions view (`GET /peers`, slice-15 / US-PS-002 / ADR-052): every
    /// peer with `removed_at IS NULL`, each carrying its PER-PEER cached local
    /// claim count. Read-only SQL only — NO mutation method is added to this trait
    /// (ADR-030 / I-PS-1): a `Box<dyn StoreReadPort>` stays structurally incapable
    /// of subscribing/unsubscribing. LOCAL only, no network (I-PS-4).
    ///
    /// ONE aggregate query (NO N+1, I-PS-8 / DD-PS-1): `peer_subscriptions ps LEFT
    /// JOIN peer_claims pc ON pc.author_did = ps.peer_did`, `WHERE ps.removed_at IS
    /// NULL`, `GROUP BY` the subscription identity, `COUNT(pc.cid)` as the per-peer
    /// `local_claim_count`. The LEFT JOIN keeps a subscribed-but-never-pulled peer
    /// at count `0` (DD-PS-2): `COUNT(pc.cid)` counts the NULL right side as `0`,
    /// not `1` (NOT an inner JOIN that would drop the row, NOT `COUNT(*)`). The
    /// `GROUP BY peer_did` decomposition is PER-PEER — two peers stay TWO rows,
    /// their counts NEVER summed/averaged into a merged total (anti-merging,
    /// J-003a / I-PS-3).
    ///
    /// Active-only by construction (I-PS-2): a peer soft-removed via the CLI `peer
    /// remove` (`removed_at` set, cached claims retained on disk) is EXCLUDED — its
    /// absence IS the J-003c residue-free promise rendered. Returns an EMPTY vec for
    /// a store with no active subscriptions (the viewer renders the guided
    /// `NoSubscriptions` empty state — never an error, US-PS-003).
    fn list_active_peer_subscriptions(
        &self,
    ) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError>;
}
