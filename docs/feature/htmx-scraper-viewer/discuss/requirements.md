# Requirements: htmx-scraper-viewer (slice-06)

> **Brownfield DELTA** on a 19-crate functional-Rust workspace. Prior slices 01–05 SHIPPED.
> This slice adds a **read-only htmx viewer** for the **node operator** on **localhost**.
> Completeness target: > 0.95 (functional + non-functional + business rules + error paths
> + inherited invariants all captured). Tech choices are deferred to DESIGN as OD-VIEW-*.

## Domain glossary (ubiquitous language, reused from prior slices)

| Term | Meaning |
|------|---------|
| **Node operator** | The human running an OpenLore node on their own machine; the sole user of this feature. |
| **Claim** | A signed assertion: subject, predicate, object, evidence[], confidence (numeric), author_did, composed_at, cid. Persisted in `claims` (slice-01). |
| **Peer claim** | A claim federated from another node, persisted in `peer_claims` (+evidence) with peer provenance (slice-03). |
| **Candidate claim** | An *unsigned, in-memory* `CandidateClaim` derived by the scraper's propose step (slice-02). NOT persisted; dies with the request. |
| **derived-from** | Display-only provenance on a candidate (WD-62). NEVER persisted; once signed, a claim is byte-identical to a hand-authored one. |
| **Store** | The operator's local DuckDB file (via `adapter-duckdb`) holding `claims` + `peer_claims`. |
| **Viewer** | The read-only web process this slice adds. |
| **Human gate** | The invariant (I-SCR-1) that signing requires explicit human action — and, here, that it stays exclusively in the CLI. |

---

## Functional Requirements (FR)

| ID | Requirement | Job | Source |
|----|-------------|-----|--------|
| FR-VIEW-1 | The viewer starts as a local process that serves HTTP on a localhost address and reports its listen URL. | Both | Journey step 1 |
| FR-VIEW-2 | The viewer renders the persisted `claims` rows (subject, predicate, object, confidence, author_did, composed_at, cid) as an HTML list, with a total count. | Job 1 | Journey step 2 |
| FR-VIEW-3 | The viewer renders a single claim's full detail, including its evidence[] array, addressed by its CID. | Job 1 | Journey step 3 |
| FR-VIEW-4 | The viewer renders the persisted `peer_claims` rows on a surface distinct from own claims, showing peer provenance. | Job 1 | Journey step 4 |
| FR-VIEW-5 | The viewer provides a live-scrape view: accept a target, run the slice-02 propose step (live harvest, no persistence), and render the resulting `CandidateClaim` values as HTML with their display-only derived-from provenance. | Job 2 | Journey steps 5–6 |
| FR-VIEW-6 | All persisted-claim views must paginate so large stores remain renderable. | Job 1 | Journey step 2/4 edge |
| FR-VIEW-7 | Each view has a guided empty/zero-result state (no claims yet; no peers yet; no candidates derived) directing the operator appropriately (e.g., to the CLI for signing). | Both | Journey edge states |
| FR-VIEW-8 | Confidence is displayed as the stored numeric value (e.g., 0.90), not silently reformatted. | Job 1 | Shared-artifact `claim_row` |

## Non-Functional Requirements (NFR) — including the hard guardrails

| ID | Requirement | Measurable criterion |
|----|-------------|----------------------|
| NFR-VIEW-1 (**read-only**) | No web route writes to the store or triggers signing. | Zero write/sign code paths reachable from any route; verified by route audit + tests. (KPI-VIEW-2) |
| NFR-VIEW-2 (**no key in web process**) | The viewer process never loads or holds the signing key. | Process never reads the key material; signing pipeline is not linked into the web binary's reachable paths. (inherits I-SCR-1) |
| NFR-VIEW-3 (**localhost only**) | The viewer binds a localhost/loopback address only; it is a personal dashboard, not a public surface. | Bind address is loopback; no external interface bound by default. |
| NFR-VIEW-4 (**local-first / offline**) | The store view (My Claims, Peer Claims, claim detail) works fully offline against local DuckDB. | All Job 1 views render with no network; verified offline. (inherits slice-01 KPI-5; KPI-VIEW-5) |
| NFR-VIEW-5 (**performance**) | First store view (My Claims) renders in < 10 s for a representative store, from cold viewer start to visible HTML. | Time-to-see-store-contents < 10 s, zero SQL typed. (KPI-VIEW-1) |
| NFR-VIEW-6 (**error legibility**) | Errors (store unreadable, network down, CID not found, no candidates) state what happened and the next step; no raw stack traces. | Each error path shows plain-language cause + action (Nielsen 9). |
| NFR-VIEW-7 (**network honesty**) | The live-scrape view requires network; its failure message clarifies that the store view still works offline. | Network-error scenario passes. |
| NFR-VIEW-8 (**accessibility**) | Pages use semantic HTML, labeled inputs, keyboard-operable controls, ≥ 4.5:1 text contrast. | WCAG 2.2 AA minimums (POUR). |

## Business Rules (BR)

| ID | Rule |
|----|------|
| BR-VIEW-1 | Signing is performed **only** in the CLI. The viewer renders no sign control on any page (human gate, I-SCR-1). |
| BR-VIEW-2 | Candidate claims are ephemeral and unsigned; the viewer never persists them and never implies they are signed/saved. |
| BR-VIEW-3 | derived-from is shown **only** on live-scrape candidates, **never** on persisted claims (WD-62; it is not stored, so it cannot be shown there). |
| BR-VIEW-4 | The viewer reads the **same** local DuckDB store the CLI writes; it introduces no second store and no separate schema. |
| BR-VIEW-5 | Federated peer claims are presented distinctly from the operator's own claims so authorship is never confused. |

## Inherited Invariants (carried from prior slices)

| ID | Invariant | Origin |
|----|-----------|--------|
| I-VIEW-1 | Every endpoint is read-only — no route writes or signs. | new (this slice) |
| I-VIEW-2 | The viewer process never holds the signing key. | new (this slice) |
| I-VIEW-3 | The human signing gate remains exclusively in the CLI. | inherits **I-SCR-1** (slice-02) |
| I-VIEW-4 | The viewer binds localhost only. | new (this slice) |
| I-VIEW-5 | derived-from is display-only, surfaced only on live-scrape candidates. | inherits **WD-62** (slice-02) |
| I-VIEW-6 | The store view works fully offline. | inherits **slice-01 KPI-5** (local-first) |

## Open Decisions for DESIGN (OD-VIEW-*) — handed to Morgan (solution-architect)

> DISCUSS is solution-neutral. These are explicit questions for the DESIGN wave. The
> infra notes (hyper, adapter-duckdb, axum banned by deny.toml) are **feasibility context,
> not decisions** — DESIGN owns the choice.

| ID | Open question for DESIGN |
|----|--------------------------|
| OD-VIEW-1 | Templating/rendering approach for HTML (which crate / strategy). axum is banned by `deny.toml`; hyper is the established HTTP choice (indexer + xrpc-query-server). Confirm rendering approach. |
| OD-VIEW-2 | HTTP routing approach on hyper for the viewer's read-only routes (and whether to share or fork patterns from the indexer/xrpc-query-server). |
| OD-VIEW-3 | Exact mapping of persisted DuckDB columns → displayed fields for `claims` and `peer_claims` (which columns to surface, labels, confidence formatting policy). |
| OD-VIEW-4 | Pagination strategy for large stores (page size, cursor vs offset, sort defaults). |
| OD-VIEW-5 | Whether the live-scrape view shares the same binary/process as the store view or runs as a separate binary (impacts how the key-free + offline guarantees are enforced/segregated). |
| OD-VIEW-6 | How the viewer obtains the read-only DuckDB connection within the functional pure/effect split (ADR-007) so the read-only guarantee is structurally enforced at the effect shell. |
| OD-VIEW-7 | Localhost binding + port selection/config (default port, conflict handling) and whether any auth is needed for a localhost-only dashboard. |

## Requirements Completeness Self-Assessment

| Category | Captured? | Evidence |
|----------|-----------|----------|
| Functional | Yes | FR-VIEW-1..8 cover both jobs end-to-end |
| Non-functional | Yes | NFR-VIEW-1..8 incl. read-only, no-key, localhost, offline, perf, a11y |
| Business rules | Yes | BR-VIEW-1..5 incl. human-gate, ephemerality, provenance honesty |
| Error / sad paths | Yes | empty store, unreadable store, CID not found, no candidates, network down |
| Inherited invariants | Yes | I-VIEW-1..6 incl. I-SCR-1, WD-62, slice-01 KPI-5 |
| Open decisions tracked | Yes | OD-VIEW-1..7 handed to DESIGN |

**Estimated completeness: ~0.96** (> 0.95 target). Residual gap: exact column mapping and
templating choice are intentionally deferred to DESIGN (OD-VIEW-1, OD-VIEW-3) rather than
guessed here.
