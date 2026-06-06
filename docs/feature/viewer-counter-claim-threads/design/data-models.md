<!-- markdownlint-disable MD013 MD024 -->
# Data Models: viewer-counter-claim-threads (slice-11)

> DESIGN · Morgan · 2026-06-06 · reuse-first DELTA · NO schema migration

No DB schema change. The reference graph + artifacts already store everything needed
(see `architecture-design.md` §1). This document fixes the boundary DTO and the pure
view-model shapes (the contracts the crafter implements against).

---

## 1. Storage (UNCHANGED — read only)

| Table / artifact | Role for slice-11 | Migration |
|---|---|---|
| `claim_references (referencing_cid, referenced_cid, ref_type)` + `idx_claim_references_referenced` | OWN counters: rows where `ref_type='counters'` and `referenced_cid=target` | v1 (present) |
| `peer_claim_references` (same shape) + `idx_peer_claim_refs_referenced` | PEER counters | v3 (present) |
| `claims (cid, author_did, composed_at, artifact_path, …)` | own counter attribution + artifact pointer | v1 (present) |
| `peer_claims (cid, author_did, composed_at, signed_record_path, …)` | peer counter attribution + artifact pointer | v3 (present) |
| on-disk `SignedClaim` JSON (`unsigned.reason`) | the verbatim `reason` (NOT a DB column; ADR-015 top-level optional) | n/a |

**No new table, no new column, no migration v4.** (Rationale + the rejected
add-a-column alternative are in ADR-046.)

---

## 2. Boundary DTO — `ports::CounterClaimRow` (NEW)

The flat, attribution-complete row returned by `query_counter_claims`. One row per
counter (own or peer). Modeled to make anti-merging + verbatim non-negotiable at the
type level.

```rust
/// One counter-claim targeting a given CID, projected for the read-only viewer.
/// One row per signed counter (own or peer) — NEVER a merged aggregate (I-CT-3).
/// `author_did` and `cid` are NON-Option (attribution is never elided).
#[derive(Debug, Clone, PartialEq)]
pub struct CounterClaimRow {
    /// The counter author's DID — NON-Option, rendered VERBATIM (I-CT-3).
    pub author_did: String,
    /// The counter's OWN content-addressed CID — NON-Option; links to
    /// /claims/{cid} (I-CT-3, no invented edge).
    pub cid: String,
    /// The verbatim free-text reason from the signed artifact (`unsigned.reason`).
    /// `None`/empty for a peer record authored by a non-OpenLore client
    /// (ADR-015 wire-optional asymmetry) → the renderer shows "no reason provided".
    pub reason: Option<String>,
    /// The counter's stored confidence DOUBLE — carried verbatim; NOT used to
    /// re-weight the countered claim (I-CT-2). Display is the crafter's choice;
    /// the contract is "verbatim, never a re-score of the target".
    pub confidence: f64,
    /// composed_at — used ONLY for deterministic ordering/tiebreak (not the
    /// primary displayed field; the SQL already orders by it).
    pub composed_at: DateTime<Utc>,
    /// The counter's origin (Own vs Known{author_did, fetched_from_pds}); reuses
    /// the existing PeerOrigin ADT so "mine vs peer" is never ambiguous (BR-VIEW-5).
    pub origin: PeerOrigin,
}
```

> `reason` is `Option<String>` (not `String`) ON PURPOSE: it captures the ADR-015
> asymmetry (required at the OpenLore `claim counter` verb, optional at the wire) so the
> empty-reason edge case is total at the type level, not a runtime surprise.

---

## 3. Pure view-model — `viewer-domain::CounterThread` (NEW)

The render-side ADT. Two arms, NO aggregate arm (anti-merging is structural).

```rust
/// The counter thread for one claim detail. PURE view-model.
/// - `None`   → the claim is un-countered: render the claim ALONE, no section,
///              no "0 counters" noise (US-CT-003 no-noise discipline).
/// - `Countered { counters }` → render the neutral "Countered" flag + one thread
///              item PER counter (never a merged "disputed by N" row, I-CT-3).
#[derive(Debug, Clone, PartialEq)]
pub enum CounterThread {
    None,
    Countered { counters: Vec<CounterEntry> },
}

/// One thread item — one attributed counter, shaped for rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct CounterEntry {
    /// Author DID, rendered verbatim (with the slice-06 `(you)` annotation when
    /// the origin is Own).
    pub author_did: String,
    /// The counter's own CID — the link target /claims/{cid}.
    pub cid: String,
    /// The reason to display: Some(verbatim text) | None → "no reason provided".
    pub reason: Option<String>,
    /// True when this counter is the operator's own (origin Own) → "(you)".
    pub is_own: bool,
}
```

Projection (total): `Vec<CounterClaimRow> -> CounterThread`

- empty vec → `CounterThread::None`
- non-empty → `Countered { counters }`, one `CounterEntry` per row, preserving the
  adapter's deterministic order (composed_at, source_table, cid). `is_own` is derived
  from `origin` (Own → true). `reason` carried as-is (the `None`/empty distinction is
  resolved at render: empty-after-trim is treated as `None` → "no reason provided").

---

## 4. Render contract (the observable HTML)

- **Un-countered** (`None`): the detail renders EXACTLY as slice-06/07 — no
  "Counter-claims" section, no flag, no empty noise.
- **Countered**: a neutral "Countered" presence flag near the claim (no score, no
  count-verdict, no "consensus"); then a "Counter-claims" section with one item per
  `CounterEntry`:
  - author DID (`(you)` when `is_own`),
  - the CID as a link `href="/claims/{cid}"` (percent-encoding discipline per ADR-044
    carried for the href),
  - the verbatim reason, OR the explicit "no reason provided" state.
- Rendered INSIDE `render_claim_detail_fragment` so the htmx fragment and the no-JS
  full page are byte-identical in the swap region (I-CT-6 parity).
- The original claim's confidence renders through the single `render_confidence` site,
  UNCHANGED by the presence of counters (I-CT-2/I-CT-4).

> Exact markup/wording is the crafter's (acceptance tests assert the observable
> contract above), per the principle "architecture owns WHAT, crafter owns HOW".
