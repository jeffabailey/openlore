# DISCUSS Decisions — viewer-counter-claim-list-flags (slice-12)

## Key Decisions

- [D1] **Trace to J-003b, no new job** — slice-12 realizes the AT-A-GLANCE / list-
  discoverability facet of the VIEW half of J-003b. slice-11 explicitly recommended
  this slice with this scope. (see: feature-delta.md JTBD sections, jobs.yaml J-003b)
- [D2] **Batch presence read, NOT N+1** (I-LF-8) — the per-CID counter-presence lookup
  across the whole list page is ONE aggregate query (`referenced_cid IN (...)` UNION-ALL
  DISTINCT over the indexed `claim_references ∪ peer_claim_references`), NOT one query
  per row. Load-bearing technical commitment for DESIGN. (see: feature-delta.md
  invariants, user-stories.md US-LF-001)
- [D3] **Presence-only boolean, never a count/verdict** (I-LF-3) — the list flag is the
  neutral slice-11 `COUNTERED_PRESENCE_FLAG` ("Countered"), a boolean per row; per-
  counter attribution + reasons stay in the slice-11 thread the flag links to.
- [D4] **Shown-never-applied + no-regression** (I-LF-2) — the flag is additive; the
  slice-06 list ordering/paging/count and each row's confidence are byte-identical with
  and without the flag. The presence set is mapped onto rows AFTER `list_claims`; the
  list SQL is untouched.
- [D5] **Scope fork: `/claims`-only recommended** — flag ONLY the `/claims` own-claims
  list this slice; defer `/project`+`/philosophy`+`/score` flags to a recommended
  slice-13. DECISION flagged for user; expansion breaks the ≤1-day budget.

## Requirements Summary

- Primary job: J-003b (counter-claim as first-class disagreement — at-a-glance facet).
  The operator (P-001 "Maria") wants to spot which of her claims drew a counter while
  scanning the `/claims` list, then drill into the contested claim's thread.
- Walking skeleton scope: N/A (brownfield DELTA, no Feature 0). Thinnest slice =
  US-LF-002 (the flag) backed by US-LF-001 (the batch read).
- Feature type: user-facing (a DELTA on the read-only viewer's `/claims` list).

## Constraints Established

- Read-only (StoreReadPort, no key, 3-layer enforcement); LOCAL/offline (no network);
  no new crates (workspace stays 21); no new route (extends `GET /claims`); no new KPI ID.
- Batch presence read is ONE aggregate query (no N+1); no per-row artifact read (the
  flag carries no reason text).
- Anti-merging: presence-only boolean, never a merged "disputed by N" aggregate.
- No-regression: slice-06 list ordering/paging/count unchanged.

## Upstream Changes

- None. No DISCOVER assumptions changed. No DIVERGE wave existed for this slice (noted
  as a non-blocking risk in feature-delta.md — the job is already validated J-003b and
  the scope was pre-recommended by slice-11).

## SSOT updates to apply

- `docs/product/jobs.yaml` — append a changelog entry (2026-06-07) noting slice-12 traces
  to J-003b (at-a-glance list facet; no new job/sub-job). [Recommended for the
  orchestrator/finalize step — not written by this DISCUSS pass to avoid touching SSOT
  mid-wave.]
- `docs/product/personas/senior-engineer-solo-builder.yaml` — append the
  counter-claim-scanner hat facet (2026-06-07). [Recommended for finalize.]
