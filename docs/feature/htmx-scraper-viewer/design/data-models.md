# Data Models: htmx-scraper-viewer (slice-06)

> **DELTA** — the viewer adds **ZERO persisted types** and **ZERO new schema**. It is
> read + render only over the EXISTING slice-01 `claims` and slice-03 `peer_claims`
> tables (BR-VIEW-4). This doc fixes OD-VIEW-3 (column → displayed-field mapping) and
> OD-VIEW-4 (pagination), grounded in the REAL schema in `crates/adapter-duckdb/src/`.

## 0. Source schema (verbatim from the shipped code — the SSOT)

`claims` (slice-01, `schema.rs`):

```
cid VARCHAR PK, subject VARCHAR, predicate VARCHAR, object VARCHAR,
confidence DOUBLE CHECK [0,1], author_did VARCHAR, composed_at TIMESTAMP,
published_at TIMESTAMP NULL, at_uri VARCHAR NULL, artifact_path VARCHAR, inserted_at TIMESTAMP
-- child: claim_evidence(cid, evidence, ordinal)  [FK cid → claims.cid]
-- index: idx_claims_composed_at on (composed_at)
```

`peer_claims` (slice-03, `schema_v3.rs`):

```
cid VARCHAR PK, author_did VARCHAR NOT NULL CHECK(<>''), subject VARCHAR, predicate VARCHAR,
object VARCHAR, confidence DOUBLE CHECK [0,1], composed_at TIMESTAMP, fetched_at TIMESTAMP,
fetched_from_pds VARCHAR NOT NULL, signed_record_path VARCHAR NOT NULL
-- child: peer_claim_evidence(cid, evidence, ordinal)  [FK cid → peer_claims.cid]
-- index: idx_peer_claims_composed_at on (composed_at)
```

There is **no `peer_origin` column**. The peer origin IS `author_did` (who authored it)
+ `fetched_from_pds` (the PDS it was fetched from). `confidence` is numeric `DOUBLE` in
BOTH tables. `derived_from` is in NEITHER table — it is a slice-02 in-memory display-only
artifact (WD-62), so it CANNOT be shown on persisted claims (I-VIEW-5).

---

## 1. OD-VIEW-3 — column → displayed-field mapping

### `claims` → `/claims` list (`ClaimRow`) and `/claims/{cid}` detail (`ClaimDetail`)

| Displayed field | Source column | Render policy | Used on |
|-----------------|---------------|---------------|---------|
| Subject | `claims.subject` | verbatim (escaped) | list + detail |
| Predicate | `claims.predicate` | verbatim (escaped) | list + detail |
| Object | `claims.object` | verbatim (escaped) | list + detail |
| Confidence | `claims.confidence` (DOUBLE) | **VERBATIM NUMERIC** — `0.90` shown as the stored f64, never reformatted/rounded (FR-VIEW-8). An optional display LABEL (slice-01 `confidence_bucket` concept) MAY accompany it, but the numeric value is authoritative. | list + detail |
| Author DID | `claims.author_did` | verbatim (may carry `#fragment` signing locator; display as-is or bare — DESIGN leaves the bare-vs-full display to the renderer, but it is the operator's OWN DID on every own claim) | list + detail |
| Composed at | `claims.composed_at` (TIMESTAMP) | ISO-8601 / RFC3339 string | list + detail |
| CID | `claims.cid` | verbatim; links to `/claims/{cid}` | list + detail |
| Evidence[] | `claim_evidence.evidence` WHERE cid=? ORDER BY ordinal | one row per URL; empty → "no evidence attached" (US-VIEW-002 Ex 2) | **detail only** |
| ~~derived-from~~ | — | **NEVER shown** on persisted claims (I-VIEW-5 / WD-62 — not stored) | neither |

Columns intentionally NOT surfaced (slice-06 scope): `published_at`, `at_uri`,
`artifact_path`, `inserted_at` — publication/storage metadata, not part of the
"what did I sign" view. (A future slice could add a "published?" badge from `at_uri`.)

### `peer_claims` → `/peer-claims` list (`PeerClaimRow`)

| Displayed field | Source column | Render policy | Used on |
|-----------------|---------------|---------------|---------|
| Subject | `peer_claims.subject` | verbatim (escaped) | list |
| Predicate | `peer_claims.predicate` | verbatim (escaped) | list |
| Object | `peer_claims.object` | verbatim (escaped) | list |
| Confidence | `peer_claims.confidence` (DOUBLE) | verbatim numeric (FR-VIEW-8) | list |
| **Peer origin** | `peer_claims.author_did` | the peer's DID — the primary "who" of the federated claim; **always non-empty** (schema CHECK). Rendered as the distinguishing "from peer X" label (BR-VIEW-5). A blank/absent value (defensive — predates/bypasses the CHECK) renders "unknown" rather than dropping the row (US-VIEW-003 boundary). | list |
| Fetched from PDS | `peer_claims.fetched_from_pds` | the PDS the claim was fetched from — secondary origin detail | list |
| Composed at | `peer_claims.composed_at` | RFC3339 | list |
| CID | `peer_claims.cid` | verbatim | list |
| Evidence[] | `peer_claim_evidence` (if peer detail surfaced — slice-06 lists only) | — | (deferred) |

**Distinctness (BR-VIEW-5 / I-FED separation)**: peer claims render on a SEPARATE route
(`/peer-claims`) with a peer-origin column own claims do not have — "mine vs federated"
is never ambiguous. The own `/claims` view shows the operator's own DID; the peer view
leads with the peer's DID.

---

## 2. OD-VIEW-4 — pagination model

Offset/limit, fixed page size, deterministic sort (ADR-030 §Pagination).

```text
PageRequest { page: u32 (1-based), page_size: u32 = 50 (fixed slice-06) }

Page<T> {
  rows: Vec<T>,        // at most page_size rows
  total: u64,          // COUNT(*) of the table
  page: u32,           // echoed request page
  page_size: u32,
}
```

- **Query**: `... ORDER BY composed_at DESC, cid ASC LIMIT page_size OFFSET (page-1)*page_size`.
  `total` = `SELECT COUNT(*)`. `composed_at` is indexed in both tables; `cid` (PK) is the
  deterministic tiebreak so page boundaries are stable on the read-only store.
- **Sort default**: `composed_at DESC` (newest first) — most recent signing/federation is
  most relevant to "does my node match what I just did" (US-VIEW-001/003 outcomes).
- **Position indicator** (rendered in `PageView<T>`): `start = (page-1)*page_size + 1`,
  `end = min(page*page_size, total)`, shown as "start–end of total" (e.g. "51–100 of
  312", US-VIEW-004).
- **Bounds**: prev hidden/disabled on page 1; next hidden/disabled when `end >= total`
  (US-VIEW-004 last-page AC). `page` clamped to `[1, ceil(total/page_size)]`.
- **Single-page store** (`total <= page_size`): no pagination controls rendered
  (US-VIEW-004 small-store AC).
- **Empty store** (`total == 0`): zero rows → `EmptyState` guided message (FR-VIEW-7),
  no pagination controls.

---

## 3. In-memory render view-models (`viewer-domain`, PURE)

These are pure value types mapped FROM the `ports` boundary projections (§1) — the
renderer never touches DuckDB. (Field names indicative; crafter owns exact shape.)

```text
ClaimRowView      { subject, predicate, object, confidence: f64, author_did, composed_at, cid }
ClaimDetailView   { row: ClaimRowView, evidence: Vec<String> }          // empty → "no evidence attached"
PeerClaimRowView  { subject, predicate, object, confidence: f64,
                    peer_origin: PeerOrigin, fetched_from_pds, composed_at, cid }
PeerOrigin        = Known(did) | Unknown                                // US-VIEW-003 boundary
CandidateRowView  { subject, predicate, object, confidence: f64, evidence: Vec<String>,
                    derived_from: String }                              // ONLY type with derived_from
PageView<T>       { rows: Vec<T>, start: u64, end: u64, total: u64, prev: Option<u32>, next: Option<u32> }
EmptyState        = NoClaims | NoPeers | NoCandidates                   // each → its guided message
ErrorView         { cause: String, next_step: String }                  // plain-language, no stack trace
ScrapeState       = Form | Results | ZeroCandidates | NetworkDown
```

**Type-level invariant (I-VIEW-5 / WD-62)**: `ClaimRowView` and `ClaimDetailView` have NO
`derived_from` field — only `CandidateRowView` does. The renderer cannot show derived-from
on a persisted claim because the persisted view-model has no slot for it. The mistake is
unrepresentable, not merely untested.

---

## 4. Candidate claim (live `/scrape`) — NOT persisted

`CandidateClaim` (slice-02 `ports::CandidateClaim`) is derived in-memory per `/scrape`
POST by `scraper_domain::derive_candidates` and mapped to `CandidateRowView`. It is NEVER
written to any table (BR-VIEW-2); refreshing re-harvests. `derived_from` is computed
display-only provenance (the candidate's `source_signals()`), shown ONLY here (WD-62).
No CID is computed and no row is inserted — the viewer's `/scrape` path has ZERO write
and ZERO sign reachability (the GithubPort holds no storage/identity/PDS ref, I-SCR-1).

---

## 5. What the viewer does NOT add to the data model

- No new table, no new column, no migration (the slice-06 schema delta is EMPTY).
- No new persisted type, no new CID path (CID stability invariant).
- No second store / second handle (BR-VIEW-4; shares the slice-01/03 DuckDB file + the
  Q-DELIVER-3 single shared `Arc<Mutex<Connection>>`).
- No second source of truth: every displayed datum traces to an existing column above
  (shared-artifacts-registry.md §"Source-of-truth rule").
