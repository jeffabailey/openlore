# ADR-060: Retraction-Aware Search Filter — a Pure Predicate over the EXISTING Reference Graph (OD-RF-1 = Branch A), reconciling `--hide-retracted` with I-AV-9

- **Status**: Proposed
- **Date**: 2026-07-11
- **Deciders**: Morgan (nw-solution-architect), per D-1..D-7 + I-RF-1..8 of `retraction-aware-search-filter` (DISCUSS, DoR 16/16)
- **Feature**: retraction-aware-search-filter (slices 01 CLI + 02 viewer)
- **Extends**: ADR-025 (network-index DuckDB schema + `indexed_claim_references`), ADR-027 (`search` verb + CLI↔indexer XRPC transport), ADR-008 (soft-retract counter-claim; no hard-delete), ADR-015 (counter-claim `reason`), ADR-007 (functional-Rust pure core), ADR-037/038 (viewer `SearchState` + `/search` route). Reconciles I-AV-9 (counter shown-not-applied) with a new user-invoked filter.
- **Resolves**: OD-RF-1 (does the shipped DTO distinguish author-self-retraction from a third-party counter?) + the D-1 reconciliation of an opt-in filter with the "never silently filter" surface.

## Context

`openlore search` (ADR-027) obeys I-AV-9: a soft-retracted or countered public verified
claim STAYS discoverable and is annotated — never silently filtered or down-weighted. This
feature adds a **user-invoked, opt-in, non-destructive, self-disclosing** view control that
HIDES author-soft-retracted claims from the current view only: `openlore search …
--hide-retracted` (slice 01) and `/search?hide_retracted=1` (slice 02). D-1 locks the
reconciliation; D-3 locks the scope to **author self-withdrawal ONLY** — third-party
disagreement counters must stay shown + annotated (no heckler's veto; preserves anti-merging
I-AV-2).

The load-bearing risk (R-1 / OD-RF-1, HIGH): can a **pure** predicate distinguish an author
self-retraction from a third-party counter using ONLY what already crosses the CLI↔indexer
wire, or is an additive ingest/schema/DTO change required?

### OD-RF-1 investigation — resolved against the real code (Branch A: SUFFICIENT AS-IS)

The shipped reference graph already carries `{ref_type, referencing_author_did,
referenced_cid}` end-to-end. Evidence, file by file:

1. **The retraction record model** (`crates/cli/src/verbs/claim_retract.rs:98-115`): `claim
   retract` builds the counter-claim mirroring the original's `subject` + `predicate` +
   `object`, authored by the retracting user's OWN DID (`identity.author_did()`), carrying
   `references = [{ ref_type: Retracts, cid: <original_cid> }]`. Because the retraction
   shares the original's dimensional fields, ANY `--object` / `--subject` / `--contributor`
   query that returns the original ALSO returns its retraction record — the predicate always
   sees both rows.
2. **The wire DTO** (`crates/lexicon/src/appview_query.rs:70-107`): `SearchResultDto` carries
   `author_did: String` (non-empty, I-AV-2) and `references: Vec<ClaimReferenceDto>`, where
   `ClaimReferenceDto = { ref_type: String, cid: String }` and `cid` is the REFERENCED
   claim's CID. `ref_type ∈ {"retracts","corrects","counters","supersedes"}`.
3. **The index schema** (ADR-025, `indexed_claim_references`): `referencing_cid` (FK to
   `indexed_claims`, so the referencing author is recoverable), `referenced_cid`, and
   `ref_type` with the `CHECK (ref_type IN ('retracts','corrects','counters','supersedes'))`
   domain. The three fields the predicate needs are all persisted and serialized.
4. **The CLI-side decode** (`crates/adapter-index-query/src/lib.rs:204-251`): decodes each
   row into `ports::NetworkResultRowRaw { author_did: Did, cid: Cid, references:
   Vec<ClaimReference>, … }` (`crates/ports/src/index_query.rs:42-54`), preserving both the
   row's own `author_did` and its typed `references`.
5. **A precedent projection already exists** (`crates/cli/src/render/search.rs:223-243`):
   `network_counter_relationships` already walks the raw rows and emits, for every
   Counters/Retracts reference, `{ counter_cid, counter_author (= the referencing row's
   author_did), countered_cid }`. Exactly the tuple the retraction predicate needs.

**Conclusion (Branch A):** author self-retraction is a PURE total function of data already on
the wire. For a result set of attributed rows, a claim **C is author-self-retracted iff there
exists a row K in the set with `K.author_did == C.author_did` carrying a reference
`{ ref_type == Retracts, cid == C.cid }`.** A third-party counter (`ref_type == Counters`, or
a `Retracts` authored by a DIFFERENT DID) does NOT match and stays shown + annotated (D-3 /
I-AV-9). **Zero ingest change, zero schema change, zero DTO change.** Slice 01 stays a
pure-core change (no ingest-marker enlargement; R-1 retired).

### The one real subtlety the code surfaced (a lossy annotation; and the marker record)

- The pure `compose_results` (`crates/appview-domain/src/compose.rs:77-100`) collapses ALL of
  a row's counter/retract references into a SINGLE `counter_annotation` slot with a
  lowest-CID tiebreak, merging `Counters` and `Retracts`. That collapse is LOSSY for our
  purpose: a claim both self-retracted AND third-party-countered could surface the counter and
  mask the self-retraction. **Therefore the retraction predicate MUST run on the RAW
  attributed rows (`NetworkResultRowRaw`, which retain the full `references` graph), NOT on
  `compose_results`' output.** Both surfaces already hold the raw rows: the CLI renders
  `NetworkSearchResultRaw.results` directly; the viewer holds them before mapping to
  `IndexedClaim` → `compose_results` (`crates/adapter-http-viewer/src/lib.rs:1229,1514-1539`).
- The retraction is itself a signed claim (the "marker" record K) that shares the original's
  object, so it appears as its OWN row. The locked US-RF-001 domain examples (Ex 1: 12→10;
  Ex 4: all-retracted → empty guided state) are only internally consistent if a retraction is
  treated as ONE withdrawal event that removes BOTH the withdrawn original C AND its
  same-author marker K. Hiding C while leaving an orphaned confidence-1.0 no-evidence marker K
  visible would be incoherent and would break Ex 4's "showing none". See Decision below.

## Decision

**Add one pure total function to `appview-domain` — `partition_retracted` — that operates on
the RAW attributed rows and returns the survivors plus the disclosed count. Both surfaces
(CLI flag, viewer GET-param) invoke this single function; neither re-queries the index and
neither re-implements the decision (D-2 / I-RF-5). No new crate, no ingest/schema/DTO change
(OD-RF-1 Branch A).**

Indicative signature (DELIVER owns exact types; shapes reuse existing `ports` ADTs):

```text
// in crates/appview-domain (pure; depends on ports + claim-domain only)
pub struct RetractionPartition {
    pub survivors: Vec<ports::NetworkResultRowRaw>, // original order, verbatim confidence
    pub hidden_count: u32,                           // retraction EVENTS (see below)
}

pub fn partition_retracted(
    rows: Vec<ports::NetworkResultRowRaw>,
    hide_retracted: bool,
) -> RetractionPartition
```

Decision detail:

- **D-RF-D1 (OD-RF-1 = Branch A).** The predicate reads only `author_did`, `cid`, and typed
  `references` off the raw rows. No `retracted_by_author` DTO field, no ingest marker, no
  schema migration. CID-stability and anti-merging are untouched.
- **D-RF-D2 (single pure decision, both surfaces).** `partition_retracted` lives in
  `appview-domain`; the CLI applies it to `NetworkSearchResultRaw.results` BEFORE its
  render-time grouping; the viewer applies it to the same raw rows BEFORE mapping to
  `IndexedClaim`/`compose_results`. One function, one type (`NetworkResultRowRaw`), no
  duplicated logic (I-RF-5).
- **D-RF-D3 (self-retraction rule = D-3, literal).** C is hidden iff ∃ K in the set with
  `K.author_did == C.author_did` and `K.references ∋ { Retracts, C.cid }`. `Counters`, and any
  `Retracts` by a DIFFERENT author, never hide — shown + annotated (I-AV-9 / I-RF-4). No
  heckler's veto.
- **D-RF-D4 (a retraction is ONE event = original + its own marker).** When C is hidden, its
  same-author retraction marker K (the row whose `references ∋ { Retracts, C.cid }` and whose
  `author_did == C.author_did`) is hidden too. This keeps the filtered view to standing
  reasoning and makes the locked US-RF-001 Ex 1/Ex 4 consistent. It stays strictly within
  D-3's spirit — only the author's OWN withdrawal machinery is affected; no third-party
  content is removed.
- **D-RF-D5 (disclosed count = retraction EVENTS).** `hidden_count = |{ C : C author-self-
  retracted }|` — the number of withdrawn claims, NOT the raw rows removed (which is ~2× when
  each event contributes an original + a marker). This is the honest, user-meaningful unit
  ("2 retracted claim(s) hidden" ⇔ two withdrawals) and matches the locked BDD. **This refines
  the DISCUSS shared-artifact note `hidden_count = len(unfiltered) − len(survivors)`, which
  double-counts once the retraction marker is understood as a separate indexed row** (back-
  propagated to the slice-01 brief; see feature-delta DESIGN §OD-RF-1 Resolution).
- **D-RF-D6 (opt-in / non-destructive / self-disclosing — I-RF-1/2/3).** `hide_retracted ==
  false` ⇒ `survivors == rows` unchanged and `hidden_count == 0` (byte-identical default; a
  release-blocking regression guard). Survivors keep original relative order and verbatim
  confidence; nothing is re-ranked, re-weighted, re-verified, or written back. When active and
  `hidden_count ≥ 1`, the surface MUST disclose the count (CLI footer / viewer results-region
  notice); when active but `hidden_count == 0`, no misleading "hidden" line. A silent hide is a
  build-fail (I-RF-3).
- **D-RF-D7 (viewer stays read-only, offline).** The viewer control is a plain GET-param
  (`?hide_retracted=1`) toggle / htmx control — no write/sign/subscribe route, no key in the
  process, loopback bind, offline chrome, full page without `HX-Request` (I-RF-6). Not
  persisted; per-invocation / per-request only (I-RF-7 / D-7).
- **D-RF-D8 (enforcement).** The `partition_retracted` non-destructive contract is pinned by
  (a) an in-crate `@property` test (survivor order + each survivor's confidence identical to
  the unfiltered run — DISTILL/DELIVER); (b) `cargo xtask check-arch` keeps `appview-domain`
  on the pure-core allowlist (no I/O added) and the workspace at 21 members; (c) the
  default-unchanged byte-identical guard in the acceptance suite (KPI: the mechanical proof
  I-AV-9 was not weakened). `appview-domain` is pure ⇒ no `probe()` required (I-4/I-5 n/a).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **(B) Additive DTO / ingest marker** (`retracted_by_author: bool` on `SearchResultDto`, or surface the referencing author DID + ref_type as a new field) | Rejected — **unnecessary** (OD-RF-1 = Branch A). The raw `references` graph + per-row `author_did` already cross the wire and already decode to `NetworkResultRowRaw`. A derived boolean would duplicate a fact the graph already carries, enlarge slice 01 beyond pure-core, and add a wire field for zero new information. Kept as the documented fallback ONLY if a future change stopped co-returning the retraction record (it does today, by construction — same object). |
| **Build the predicate on `compose_results`' `counter_annotation`** | Rejected — LOSSY. The single-slot lowest-CID tiebreak (compose.rs:90-96) merges Counters+Retracts and can mask a self-retraction behind a third-party counter, silently under-hiding (a D-3 correctness bug). The predicate must read the FULL `references` graph on the raw rows. |
| **Filter at the index / re-query with a `WHERE NOT retracted`** | Rejected — violates D-2/I-RF-5 (pure decision, no second index round-trip) and would push a trust-critical rule into SQL, off the pure-core mutation-tested substrate. Also risks re-ordering/re-weighting survivors (I-RF-2). |
| **Hide C only, leave the marker K visible (strict-literal D-3)** | Rejected — leaves an orphaned confidence-1.0, no-evidence withdrawal row in the filtered view and breaks the locked US-RF-001 Ex 4 ("all retracted → showing none"). D-RF-D4 hides the event (C + its own marker) instead. |
| **Down-rank / grey-out retracted claims instead of hiding** | Rejected — out of scope (feature-delta Out of Scope) and an I-AV-9 "never down-weighted" violation. The feature HIDES on explicit opt-in; it never re-scores. |
| **A persisted "hide retracted" preference / config default** | Rejected (D-7/I-RF-7) — a stored default drifts toward silent-by-default and violates D-1. Per-invocation intent only. |

## Consequences

### Positive

- **R-1 retired at design time against real data.** Slice 01 is a pure-core change: one
  function in `appview-domain` + a CLI flag + a footer line. No ingest/schema/DTO churn, no
  CID-stability or anti-merging risk.
- **One pure decision, two surfaces.** The CLI and viewer invoke the identical function; the
  honesty count is computed by the same pure pass (no re-derivation drift).
- **I-AV-9 provably intact.** The default path (no flag/param) returns rows unchanged and
  `hidden_count == 0`; the byte-identical guard is the mechanical proof the "never silently
  filter" invariant was not weakened. The filter is user-invoked, disclosed, and reversible —
  not silent filtering.
- **Zero new crate; workspace stays 21; `check-arch` green.** `appview-domain` remains pure.

### Negative

- **The DISCUSS shared-artifact `hidden_count = len(unfiltered) − len(survivors)` is refined**
  (D-RF-D5): once the retraction marker is a separate indexed row, the naive length-difference
  double-counts. DISTILL gold fixtures MUST model the original+marker pair (not the simplified
  12→10 arithmetic) and assert the EVENT count. Back-propagated to the slice-01 brief.
- **The predicate depends on the retraction record being co-returned** with its original. This
  holds by construction today (the retraction shares the original's dimensional fields,
  claim_retract.rs). Documented as a revisit trigger if a future ingest change ever narrows the
  retraction record's indexed fields.
- **Pre-existing marker-rendering wart, unchanged in the DEFAULT view.** Without the flag, the
  retraction marker still renders as its own bare row exactly as it does in slice-05 today
  (I-RF-1 byte-identical). This feature does not fix that default-view wart; OD-RF-5 tracks a
  possible future refinement (annotate/fold markers in the default view) as OUT of scope here.

### Earned Trust

`appview-domain` is a PURE crate (no adapter, no external dependency) — it holds no `probe()`
(I-4/I-5 apply to adapters). Its trust is earned by exhaustive test coverage of the decision,
not a runtime substrate probe:

1. **The self-vs-third-party gold contract (behavioral).** A fixture where the SAME claim C is
   both self-retracted by its author AND countered by a third party MUST hide C (self-
   retraction detected on the full graph) — the direct anti-regression for the lossy-
   `counter_annotation` trap. Its sibling: a third-party `Counters` (and a different-author
   `Retracts`) leaves the row SHOWN + annotated (D-3 / I-AV-9).
2. **The non-destructive `@property` (behavioral).** For any result set + `hide_retracted`,
   survivor order and each survivor's confidence are identical to the unfiltered run
   (I-RF-2 / D-5) — proptest, in-crate (slice-04/05 mutation-gate lesson: keep the killing
   property in `appview-domain`).
3. **The default-unchanged byte-identical guard (behavioral, release-blocking).** `hide_
   retracted == false` ⇒ output byte-identical to pre-feature `search` — the mechanical proof
   I-AV-9 was not weakened.
4. **Mutation gate.** `partition_retracted` joins the `appview-domain` per-feature 100%
   production-function mutation gate + the DEVOPS nightly sweep (matching `compose_results` /
   `ingest_decision`).

The question "what happens if the environment lies?" reduces here to "what if the reference
graph lies about authorship?" — answered by the verify-before-index gate (I-AV-1, ADR-025):
every indexed row (including every retraction record) was signature-verified against its REAL
PLC key and CID-recomputed BEFORE indexing, so `K.author_did` is a verified fact, not an
attacker-asserted one. A forged retraction cannot enter the index to hide a rival's claim.

## Revisit Trigger

- A future ingest change stops co-indexing the retraction record with its original (would
  break the co-return assumption) → surface an explicit `retracted_by_author` marker at ingest
  (the documented Branch B fallback).
- Product decides the DEFAULT view should annotate/fold retraction markers (OD-RF-5) → a
  separate feature; does not touch this filter's pure decision.
- A "hide contested / hide third-party-countered" control is ever wanted → a NEW opt-in filter
  and ADR; explicitly NOT this feature (D-3 forbids folding disagreement into retraction).
