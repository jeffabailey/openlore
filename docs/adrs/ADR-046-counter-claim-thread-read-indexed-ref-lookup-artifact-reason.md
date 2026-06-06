# ADR-046: Counter-Claim Thread Read — Indexed Reference Lookup + On-Disk Reason Decode

- **Status**: Accepted
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect), resolving the schema-feasibility flag raised by Luna (nw-product-owner) in the slice-11 DISCUSS delta
- **Feature**: viewer-counter-claim-threads (slice-11)
- **Extends**: ADR-008 (counter = reference of type `counters`), ADR-014 (peer-storage anti-merging schema), ADR-015 (`reason` top-level optional, stored in the signed artifact), ADR-030 (read-only `StoreReadPort`), ADR-042 (UNION-ALL two-store anti-merging reads)

## Context

slice-11 adds a counter-claim thread beneath `GET /claims/{cid}`. It needs a read-only
method `StoreReadPort::query_counter_claims(target_cid) -> Vec<CounterClaimRow>` that,
for a target CID, returns every signed claim (own or peer) that counters it — each with
its `author_did`, own `cid`, verbatim `reason`, `confidence`, `composed_at`, and
`origin`.

Luna flagged a feasibility risk: *are counter references queryable by target CID
(normalized table / column), or are they stored only inside a serialized record blob?*
If blob-only, the read would be a full decode-and-filter, an O(n) scan over every claim.

DESIGN must decide the exact read mechanism and confirm its performance shape.

## Investigation (what the code actually stores)

| Concern | Finding | Source |
|---|---|---|
| Own references | normalized `claim_references (referencing_cid, referenced_cid, ref_type)` with `ref_type IN (...,'counters',...)` | `adapter-duckdb/src/schema.rs` (v1) |
| Own index by target | `idx_claim_references_referenced ON claim_references (referenced_cid)` | `adapter-duckdb/src/schema.rs` |
| Peer references | normalized `peer_claim_references` (same shape) | `adapter-duckdb/src/schema_v3.rs` (v3) |
| Peer index by target | `idx_peer_claim_refs_referenced ON peer_claim_references (referenced_cid)` | `adapter-duckdb/src/schema_v3.rs` |
| Existing own read | `query_referencing(target)` SELECTs `claim_references WHERE referenced_cid = ?` | `adapter-duckdb/src/lib.rs:444` |
| Existing peer read | `query_peer_referencing(target)` JOINs `peer_claim_references`→`peer_claims`, returns attributed triples | `adapter-duckdb/src/peer_storage.rs:759` |
| `reason` storage | NOT a DB column — top-level optional on the signed record, stored in the on-disk `SignedClaim` JSON (`unsigned.reason`) | ADR-015; `claims.artifact_path` / `peer_claims.signed_record_path` |
| Artifact read path | `read_artifact_at(path)` already used by `query_federated_by_subject` | `adapter-duckdb/src/lib.rs:204` |

**Conclusion: the store is NOT blob-only.** The reference graph is normalized AND
indexed on `referenced_cid` in both the own and peer stores. The "who counters this CID"
query is the established BACKWARD half of the slice-08 annotation. Only the `reason`
lives outside the DB (in the authoritative artifact).

## Decision

Implement `query_counter_claims(target_cid)` as a **two-step indexed read**:

1. **Step A — DB (indexed, anti-merging)**: a UNION-ALL over the two stores, each arm an
   intra-store JOIN of the claims table to its OWN reference table, filtered on
   `referenced_cid = ?` and `ref_type = 'counters'`, projecting `author_did`, `cid`,
   `composed_at`, the artifact pointer, and a `source_table` discriminant, ordered
   `composed_at, source_table, cid`.
2. **Step B — filesystem (per row)**: read each counter's `reason` from its on-disk
   artifact via the existing `read_artifact_at` (own `artifact_path` / peer
   `signed_record_path`).

Return `Ok(vec![])` for an un-countered target (never an error). The method is read-only
(SELECT only) and LOCAL (no network).

## Alternatives considered

| Option | Pro | Con | Verdict |
|---|---|---|---|
| **Indexed ref lookup + artifact reason read** (chosen) | reuses the indexed reference graph + the byte-faithful artifact read; no migration; `reason` single-sourced | one `fs::read` per counter (depth-1, bounded) | **ACCEPTED** |
| Denormalize `reason` into a DB column (migration v4) | reason readable in one SELECT, no per-row file read | a schema migration is out of a ~1-day reuse-first slice; duplicates the artifact's authoritative reason → drift surface; the artifact is already the source of truth | REJECTED |
| Full decode-and-filter over all claims (blob-style) | works even with no reference index | O(n) over the whole store; ignores the existing indexes; would not scale | REJECTED (premise — blob-only — is false) |

## Consequences

- **Positive**: feasibility flag retired; no migration; anti-merging preserved by the
  UNION-ALL explicit-attribution form (passes `no_cross_table_join_elides_author`);
  performance is depth-1, indexed, LOCAL, with bounded per-counter artifact reads —
  identical in shape to the shipping `query_federated_by_subject`.
- **Negative**: one filesystem read per counter (acceptable: counters per claim are
  typically 0–few; the deferred list-row annotation that WOULD risk N+1 is explicitly
  out of this slice, slice-12). An unreadable artifact surfaces a `StoreReadError`
  degraded to the guided read handling (NFR-VIEW-6), never a stack trace.
- **Enforcement**: read-only by trait shape (no mutation method on `StoreReadPort`);
  anti-merging by the explicit `author_did` + `cid` projection across the two stores.
