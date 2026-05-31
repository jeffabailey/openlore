# ADR-030: The Read-Only DuckDB Store-Read Port Seam (Structural Read-Only Enforcement)

- **Status**: Proposed
- **Date**: 2026-05-31
- **Deciders**: Morgan (nw-solution-architect), per OD-VIEW-6 + I-VIEW-1 / I-VIEW-2 (inherited I-SCR-1) for htmx-scraper-viewer (slice-06)
- **Feature**: htmx-scraper-viewer (slice-06)
- **Extends**: ADR-007 (function-signature ports, pure/effect split), ADR-009 (hexagonal composition root, WIRE→PROBE→USE, Earned-Trust probe), ADR-014 (the slice-03 single-file DuckDB store), and the slice-04 precedent of AUGMENTING `adapter-duckdb` with new read methods over the SAME store (no new table, no store swap, WD-8).
- **Resolves**: OD-VIEW-6 (read-only DuckDB access seam under the pure/effect split), OD-VIEW-3 (column → field mapping; see data-models.md for the full table), OD-VIEW-4 (pagination strategy).

## Context

The viewer must read the operator's `claims` (slice-01) and `peer_claims` (slice-03)
from the SAME local DuckDB the CLI writes (BR-VIEW-4), and it must do so under a
STRUCTURAL read-only guarantee (I-VIEW-1): no route may write or sign, and the web
process must hold no signing key (I-VIEW-2, inherited I-SCR-1). The open question:

- **OD-VIEW-6**: how does the viewer obtain the read-only DuckDB connection within the
  functional pure/effect split so read-only is enforced at the effect shell — a new
  read-only port, or reuse of an existing read path?

The existing read surface:

- `StoragePort` (in `crates/ports`) carries BOTH reads (`read_signed_claim`,
  `query_by_subject`, `query_federated_by_subject`, `query_by_object`,
  `query_by_contributor`, `traverse_graph`) AND writes (`write_signed_claim`,
  `record_publication`). Handing the viewer the full `StoragePort` would make a
  write/sign path reachable from a route by construction — a direct I-VIEW-1 breach.
- The existing reads are keyed by `subject` / `object` / `cid` / `contributor`. There is
  NO existing "list all own claims, paginated" or "list all peer claims, paginated" read
  — the `/claims` and `/peer-claims` list views (FR-VIEW-2, FR-VIEW-4, FR-VIEW-6) need a
  new unkeyed, paginated read shape.
- `DuckDbStorageAdapter` already shares one `Arc<Mutex<Connection>>` (the single-writer
  constraint, Q-DELIVER-3) and resolves `claims`/`peer_claims` columns + on-disk
  artifacts (lib.rs / peer_storage.rs).

## Decision

**Define a NEW narrow `StoreReadPort` trait (function-signature-as-port, ADR-007) that
exposes ONLY read operations the viewer needs, and implement it on `DuckDbStorageAdapter`
by AUGMENTING the adapter with new paginated list reads over the SAME store (no new
table, no second handle — the WD-8 / slice-04 precedent). The viewer's effect shell
(`adapter-http-viewer`) and the `ui` verb wiring hold ONLY a `Box<dyn StoreReadPort>` —
NEVER the write-capable `StoragePort`, NEVER `IdentityPort`/`PdsPort`. Read-only is
therefore STRUCTURAL: there is no write/sign method in the type the web process can
reach.**

### The `StoreReadPort` surface (new trait in `crates/ports`)

A SYNC, read-only, local-DuckDB port (mirrors `StoragePort`'s sync shape):

```text
trait StoreReadPort {
    fn probe(&self) -> ProbeOutcome;                                  // Earned-Trust
    fn count_claims(&self) -> Result<u64, StoreReadError>;            // FR-VIEW-2 total count
    fn list_claims(&self, page: PageRequest)                          // FR-VIEW-2 / FR-VIEW-6
        -> Result<Page<ClaimRow>, StoreReadError>;
    fn get_claim(&self, cid: &Cid)                                    // FR-VIEW-3 detail
        -> Result<Option<ClaimDetail>, StoreReadError>;
    fn count_peer_claims(&self) -> Result<u64, StoreReadError>;       // FR-VIEW-4 total count
    fn list_peer_claims(&self, page: PageRequest)                     // FR-VIEW-4 / FR-VIEW-6
        -> Result<Page<PeerClaimRow>, StoreReadError>;
}
```

There is NO `write_*`, NO `sign`, NO `record_publication`, NO `record_*` method. The
trait is the structural defense: the web process literally cannot name a mutation.

- `ClaimRow` / `PeerClaimRow` / `ClaimDetail` are lightweight read projections (the
  columns + the evidence array for detail), owned by `ports` next to the port (the
  slice-04 `AttributedClaim` precedent). `viewer-domain` maps these into its render
  view-models; the boundary types stay in `ports` so the pure renderer has zero adapter
  dependency.
- The adapter impl reuses the existing `Arc<Mutex<Connection>>` (no second DuckDB handle;
  the single-writer/single-reader handle is shared exactly as the slice-03 peer adapter
  shares it). The list reads are new SQL over `claims` / `peer_claims` — single-table per
  call (NOT cross-store), so the `no_cross_table_join_elides_author` rule does not apply,
  but each peer read still projects `author_did` verbatim.

### Why a NEW read-only port (not reuse of `StoragePort` or `query_federated_by_subject`)

| Factor | New `StoreReadPort` — **CHOSEN** | Reuse full `StoragePort` | Reuse `query_federated_by_subject` / `query_by_object` |
|---|---|---|---|
| **Structural read-only (I-VIEW-1)** | The type exposes NO write/sign method; a route physically cannot mutate. Read-only is a compile-time property, not a runtime convention. | `StoragePort` carries `write_signed_claim`/`record_publication`; a route COULD call them — read-only would be convention, not structure. | These are reads, but they are KEYED (by subject/object) — they cannot serve the unkeyed paginated `/claims` + `/peer-claims` list views. |
| **The list views need an unkeyed paginated read (FR-VIEW-2/4/6)** | `list_claims(page)` / `list_peer_claims(page)` are the exact shape the list pages need. | Would still need new list methods added — but on a write-capable port. | No unkeyed list-all exists; would require new methods anyway. |
| **Pure/effect fit (ADR-007)** | A function-signature port the composition root wires + probes (ADR-009), with a test double for hermetic acceptance. | Same wireability, but wrong capability. | Same, but wrong shape. |
| **No second store (BR-VIEW-4 / WD-8)** | Implemented by AUGMENTING `adapter-duckdb` over the SAME single file + SAME shared handle — zero new table, zero store swap (the slice-04 precedent). | Same store, wrong capability. | Same store. |

### Pagination (OD-VIEW-4): offset/limit, fixed page size, deterministic sort

| Factor | Offset/limit — **CHOSEN** | Keyset/cursor pagination |
|---|---|---|
| **Simplicity for a local read-only view** | `LIMIT ? OFFSET ?` over an indexed `ORDER BY composed_at DESC, cid` — trivially correct, trivial to render "X–Y of N" with a separate `COUNT(*)`. | Correct + scales better for huge offsets, but adds opaque-cursor encoding/decoding for no slice-06 benefit on a single-operator local store. |
| **Scale reality (KPI-VIEW-1: first page < 10s)** | The largest representative store in the stories is ~312 own + ~1,840 peer claims; `composed_at`/`cid` are indexed (slice-01 `idx_claims_composed_at`, slice-03 `idx_peer_claims_composed_at`). Offset paging at this scale is comfortably sub-second; the < 10s budget is dominated by cold viewer start, not the query. | Over-engineering at this scale. |
| **Stable ordering** | `ORDER BY composed_at DESC, cid ASC` is deterministic (cid is the PK tiebreak), so page boundaries are stable across requests on a read-only store. | Same ordering needed. |

- **Default page size: 50** (the value US-VIEW-004 already shows: "50 per page",
  "51–100 of 312"). Fixed for slice-06.
- **Position indicator**: `count_claims()` / `count_peer_claims()` give N; the view
  renders "X–Y of N" and disables/omits next/prev at the bounds (US-VIEW-004 ACs).
- **Single-page stores** (N ≤ page size) render no pagination controls (US-VIEW-004 AC).
- **Default sort**: `composed_at DESC` (newest first) — the operator's most recent
  signing activity is most relevant when confirming "does my node match what I just
  signed" (US-VIEW-001 outcome).

### Column → field mapping (OD-VIEW-3) — summary; full table in data-models.md

Grounded in the REAL schema (`schema.rs` `claims`, `schema_v3.rs` `peer_claims`):

- **`claims` → ClaimRow / ClaimDetail**: `subject`, `predicate`, `object`,
  `confidence` (DOUBLE — surfaced VERBATIM numeric per FR-VIEW-8), `author_did`,
  `composed_at`, `cid`; detail adds the `claim_evidence` array (joined by `cid`,
  ordered by `ordinal`). `derived-from` is NEVER surfaced on persisted claims (I-VIEW-5
  / WD-62 — it is not stored).
- **`peer_claims` → PeerClaimRow**: `subject`, `predicate`, `object`, `confidence`
  (DOUBLE verbatim), `author_did` (the peer's DID — surfaced as `peer_origin`),
  `fetched_from_pds` (the PDS the claim was fetched from — surfaced as the secondary
  origin detail), `composed_at`, `cid`. There is NO dedicated `peer_origin` column; the
  peer origin IS `author_did` (who authored it) + `fetched_from_pds` (where it came
  from). A peer row with an absent/blank origin renders "unknown" rather than being
  dropped (US-VIEW-003 boundary case) — but note the slice-03 schema CHECK makes
  `author_did` non-empty, so "unknown" is a defensive render path for data that predates
  or bypasses the CHECK.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Hand the viewer the full `StoragePort`** | Hard reject (I-VIEW-1 / I-VIEW-2). It carries `write_signed_claim`/`record_publication`; the web process could reach a mutation by construction. Read-only would be convention, not structure — the exact failure mode the slice's hard invariant forbids. |
| **Reuse `query_federated_by_subject` / `query_by_object` for the list views** | Rejected (OD-VIEW-6). They are KEYED reads (by subject/object); the `/claims` and `/peer-claims` list pages need an UNKEYED paginated list-all that does not exist. They also live on the write-capable `StoragePort`. |
| **Open a SECOND DuckDB connection in read-only mode (`access_mode=read_only`)** | Rejected (Q-DELIVER-3 / WD-8 / BR-VIEW-4). DuckDB is single-writer; a second handle to the same file races with the CLI's writer and violates "no second store / one shared handle". The shared `Arc<Mutex<Connection>>` + a read-only PORT gives read-only at the type level without a second handle. (The viewer runs in its own process while the CLI is idle in the common case, but the port-level read-only guarantee holds regardless of process topology.) |
| **Keyset/cursor pagination** | Rejected (OD-VIEW-4). Over-engineered for a single-operator local store at the stories' scale (≤ ~2k rows, indexed); offset/limit + `COUNT(*)` is the simplest correct option and renders the "X–Y of N" indicator trivially. Revisit if a store reaches a scale where deep offsets degrade. |
| **Compute pagination/counts in the pure core over a full table load** | Rejected (performance, KPI-VIEW-1). Loading the whole store to count + slice in Rust defeats the indexed `LIMIT/OFFSET/COUNT`. The pure core renders the page; the effect shell does the bounded read. |

## Consequences

### Positive

- Read-only is a STRUCTURAL, compile-time property: the web process holds a type with no
  mutation method, so no route can write or sign — I-VIEW-1 / I-VIEW-2 are enforced by
  the type system + `xtask check-arch`, not by reviewer vigilance.
- No second store, no new table, no second DuckDB handle: the viewer reads the exact
  same store the CLI writes (BR-VIEW-4) via an AUGMENT to `adapter-duckdb` (WD-8 /
  slice-04 precedent).
- Offline by construction: the store reads touch only the local DuckDB; the `/claims`,
  `/claims/{cid}`, and `/peer-claims` views work fully offline (I-VIEW-6 / KPI-VIEW-5).
- The port is wireable + probe-able (ADR-009) and double-able for hermetic acceptance
  (a `FakeStoreRead` mirroring the slice-05 `FakeIndexQuery` pattern).

### Negative

- **A new port trait + new boundary projection types in `ports`** (mild surface growth).
  Accepted: it is the narrow read-only contract the structural guarantee requires; the
  slice-04 `AttributedClaim` precedent shows boundary types belong in `ports`.
- **New list-read SQL on `adapter-duckdb`** to maintain. Accepted and bounded
  (`LIMIT/OFFSET/COUNT` over indexed columns); each peer read projects `author_did`
  verbatim (anti-merging discipline carried forward even though these are single-table).

### Earned Trust (principle 12 — the load-bearing slice-06 self-probes)

`adapter-http-viewer`'s `StoreReadPort`-backed effect shell ships a REAL `probe()`
(non-stub; satisfies `xtask check-probes`; NOT on the bootstrap allowlist) run by the
`ui` verb composition root BEFORE the serve loop (WIRE→PROBE→USE). The probe exercises
the catalogued slice-06 "what if the substrate lies?" scenarios:

1. **Store readability (offline self-probe)**: the probe runs a sentinel
   `count_claims()` against the local DuckDB and refuses to start with a structured
   `health.startup.refused` (reason: store unreadable; e.g. another process holds the
   file) — surfacing US-VIEW-001 Example 3's "is another process using it?" as a
   startup refusal rather than a per-request stack trace.
2. **Read-only capability self-probe (the load-bearing trust event)**: the probe
   asserts the wired port is the read-only `StoreReadPort` and that NO signing key /
   `IdentityPort` / `PdsPort` is reachable from the viewer's wiring — the I-VIEW-1 /
   I-VIEW-2 substrate-lie ("the web process secretly can write/sign") is caught at probe
   time. This is enforced in three orthogonal layers per principle 12: (a) subtype — the
   wiring type is `Box<dyn StoreReadPort>`, mypy-equivalent here being Rust's own type
   check that the viewer struct holds no write port; (b) structural — `xtask check-arch`
   gains a viewer-capability rule that `adapter-http-viewer`'s transitive deps exclude
   `adapter-atproto-pds` (PDS write) and that the crate links no `IdentityPort` signing
   surface (mirrors `check_indexer_capability_boundary`); (c) behavioral — a CI gold
   test (`viewer_is_read_only`) drives every route and asserts the store row count is
   unchanged + no artifact/PDS write occurred.
3. **Loopback-bind self-probe**: the probe asserts the bound address is loopback
   (127.0.0.0/8 or ::1) and refuses otherwise — I-VIEW-4 caught at startup.
4. **derived-from honesty (behavioral, not a startup probe)**: the gold test
   `derived_from_only_on_candidates` drives `/claims` and asserts NO persisted-claim row
   carries derived-from (I-VIEW-5 / WD-62), and drives `/scrape` asserting candidates DO
   carry it — the provenance-honesty contract proven, not trusted.

**External integration note (handoff to platform-architect)**: the read-only store-read
boundary is purely local (DuckDB); it has NO external integration. The only external
boundary in slice-06 is the `/scrape` route's reuse of the slice-02 `GithubPort` (GitHub
public API), already covered by ADR-019's contract-test candidacy.
