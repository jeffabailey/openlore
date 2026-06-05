# ADR-039: The Viewer's LOCAL Contributor-Scoring Read — A Read-Only `StoreReadPort` Seam Returning the slice-04 `AttributedClaim` Feed, Scored by the REUSED Pure `scoring` Core

- **Status**: Accepted (slice-09 viewer-contributor-scoring, DESIGN 2026-06-05). Resolves OD-CS-2 + the US-CS-001 `@infrastructure` capability.
- **Date**: 2026-06-05
- **Deciders**: Morgan (nw-solution-architect), resolving OD-CS-2 for viewer-contributor-scoring (slice-09).
- **Feature**: viewer-contributor-scoring (slice-09)
- **Extends**: ADR-007 (pure/effect split), ADR-009 (hexagonal probe contract), ADR-020 (the slice-04 `query_by_contributor` / `query_attributed_for_scoring` reads + the `ScoringFilter` ADT), ADR-022 (the pure `scoring` core, NO formula in SQL), ADR-030 (the read-only `StoreReadPort`, NO mutation method), ADR-036 (the slice-08 capability-boundary precedent for adding a read to the viewer process).
- **Resolves**: OD-CS-2 (the local-score read shape: extend `StoreReadPort` vs a new viewer port vs reuse the slice-04 `StoragePort` read).

## Context

US-CS-001 (`@infrastructure`) stands up the ONE new capability the `/score` view
needs: given a contributor DID, the viewer process must read that contributor's
attributed scoring feed over the LOCAL store and run the slice-04 pure scorer to
obtain a display-only `WeightedView` (with its per-claim `Contribution`
decomposition). Three hard constraints bind the read (the I-CS-* invariants):

- **Read-only / no key (I-CS-1)**: the read seam MUST hold no signing/identity/PDS
  surface and expose NO mutation method — the viewer process holds no key and has
  no write/sign route. The slice-06 structural guarantee is that
  `Box<dyn StoreReadPort>` *cannot* mutate because the trait declares no mutation
  method (ADR-030).
- **Local-first / offline (I-CS-5)**: the feed is read over the LOCAL DuckDB store
  ONLY — no network. `/score` works fully offline (distinct from `/scrape` +
  `/search`).
- **Pure-scorer reuse (I-CS-6 / WD-CS-6)**: the scoring math is the slice-04 pure
  `scoring::score` core REUSED verbatim — no second formula. The aggregation (the
  weight) happens in Rust, never in SQL, so the weight always decomposes into its
  `Contribution` rows (I-CS-2 / Gate 2).

The slice-04 read surface (`StoragePort::query_by_contributor` +
`query_attributed_for_scoring(ByContributor)`, ADR-020) already returns the
`Vec<AttributedClaim>` feed the pure scorer consumes — but it lives on the FULL
`StoragePort` (which also carries `write_signed_claim`, `record_publication`, …).
The viewer must NOT hold that port (it would hand the viewer a mutation surface,
breaching I-CS-1). The viewer reads through the read-only `StoreReadPort` (ADR-030),
which today exposes only `list_claims`/`get_claim`/`list_peer_claims`/`count_*` — no
contributor-scoring read.

`AttributedClaim` + `ScoringFilter` already live in `crates/ports` (hoisted there in
slice-04 so BOTH the pure `scoring` core AND the `cli` composition root consume them;
`scoring → ports`, never the reverse). So a `StoreReadPort` method can return
`Vec<AttributedClaim>` with NO new boundary type and NO new dependency edge.

## Decision

**Add ONE read-only method to the slice-06 `StoreReadPort` (ADR-030) that returns the
slice-04 `AttributedClaim` feed for a contributor, implemented in `adapter-duckdb`
over the LOCAL store; the effect shell then runs the REUSED pure
`scoring::score(&feed, &ScoringConfig::DEFAULT)` on the feed. No mutation method, no
key, no network, no new persisted type, no new formula.**

### The read method (read-only seam on `StoreReadPort`)

```text
pub trait StoreReadPort: Send + Sync {
    // ... existing slice-06/07 reads (list_claims, get_claim, list_peer_claims, count_*) ...

    /// The LOCAL attributed scoring feed for one contributor — every claim that
    /// `author_did` authored across all subjects, projected as the slice-04
    /// `AttributedClaim` rows the PURE `scoring::score` core consumes. Read-only
    /// SQL ONLY (no mutation method on this port — I-CS-1). LOCAL store only — no
    /// network (I-CS-5). Returns per-claim rows carrying the non-`Option`
    /// `author_did` (anti-merging; the weight aggregates in pure Rust, NEVER in
    /// SQL — I-CS-2 / WD-73). An unknown contributor yields an EMPTY Vec (the
    /// render layer shows the guided empty state — never a crash).
    fn query_contributor_scoring_feed(
        &self,
        contributor: &Did,
    ) -> Result<Vec<AttributedClaim>, StoreReadError>;
}
```

It returns the SAME `ports::AttributedClaim` shape the slice-04 `StoragePort` reads
return, so the pure scorer consumes it unchanged. It is a NEW method (not a reuse of
the `StoragePort` read) because the viewer must hold the read-only port, not the full
storage port. The `adapter-duckdb` impl is the read-only counterpart of the slice-04
`query_by_contributor` SQL — a `SELECT … FROM claims … UNION ALL … FROM peer_claims`
projecting `author_did` explicitly (NEVER a merging `JOIN`/`GROUP BY`), over the SAME
shared connection the existing `StoreReadPort` methods use (BR-VIEW-4).

> **Scope note (local-first, I-CS-5 / WD-CS-8):** the slice-04 CLI explorer flags
> imply FEDERATED scope (own + peer; WD-87). The slice-09 viewer feed is LOCAL — it
> reads the operator's OWN store + already-pulled peer rows in the local DuckDB
> file, with NO live network call. "Local" here = "the local DuckDB file" (own +
> cached peer claims), never a live PDS/index fetch. DELIVER decides whether the
> local feed spans `claims` only or `claims ∪ peer_claims` (both are local); the
> recommended default mirrors the slice-04 `query_by_contributor` UNION-ALL shape so
> the local peer rows the operator already pulled are included, with NO network.

### Where the pure scorer runs (effect shell, not viewer-domain)

The effect shell (`adapter-http-viewer`) owns the feed-read effect, so it ALSO calls
the pure `scoring::score` on the feed and builds the `ScoreState` the renderer
projects (ADR-040). This mirrors the slice-08 `/search` handler exactly: the shell
calls the pure `appview_domain::compose_results` on the rows it read, then builds
`SearchState`. Here the shell calls `scoring::score(&feed, &ScoringConfig::DEFAULT)`,
then builds `ScoreState`. The scorer is pure, so this is a pure call inside the
effect shell — the effect is the READ, the compute is pure.

```text
// adapter-http-viewer (EFFECT shell) — the /score handler (ADR-041)
let feed = store.query_contributor_scoring_feed(&did)?;          // EFFECT: local read-only store read
let view = scoring::score(&feed, &ScoringConfig::DEFAULT);       // PURE compute (reused slice-04 core)
let state = ScoreState::from_view(contributor, view);            // PURE: build the render input (ADR-040)
// ... fork by Shape, render (ADR-040/041) ...
```

`scoring` becomes a build-dependency of `adapter-http-viewer` (the shell calls
`score`) AND of `viewer-domain` (the `ScoreState` ADT carries a `WeightedView`,
ADR-040). Both are PURE→PURE edges (`scoring` is a pure core, ADR-022) — the
`xtask check-arch` deltas are in ADR-041 §enforcement.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Hand the viewer the full `StoragePort` and call `query_by_contributor` directly** | Maximal reuse — the exact read already exists (ADR-020). | **Rejected (I-CS-1 / read-only).** `StoragePort` carries `write_signed_claim` / `record_publication` — handing it to the viewer gives the viewer a mutation surface, breaching the slice-06 structural read-only guarantee (`xtask check-arch` viewer capability rule, ADR-030). The viewer must hold a port with NO mutation method. |
| **A brand-new viewer-process port (`ScoringFeedPort`)** | Clean single-purpose seam. | **Rejected (simplest-solution / one read path).** A second read port (with its own probe, wiring, capability-rule entry) for ONE read that returns the EXISTING `AttributedClaim` shape over the SAME store the `StoreReadPort` already reads is needless ceremony. Adding ONE method to the existing read-only port reuses the shared connection, the probe, and the capability rule — and keeps "the viewer's read surface" in ONE place. |
| **Reuse `query_attributed_for_scoring(ByContributor)` shape on the read port** | Symmetric with slice-04. | **Partially adopted.** The slice-04 `ScoringFilter::ByContributor` filter is the conceptual contract; but the viewer read takes a bare `&Did` (the route's only dimension is contributor — OD-CS-5), so a `ScoringFilter`-typed param would be a single-arm over-generalization on the read port. The RETURN type (`Vec<AttributedClaim>`) is identical; the SQL is the read-only sibling of `query_by_contributor`. DELIVER may take `&ScoringFilter` instead of `&Did` if it prefers slice-04 symmetry — the feed contract is the same either way. |
| **Run `scoring::score` inside `viewer-domain` (the renderer calls it)** | Fewer call sites. | **Rejected (effect/pure symmetry; ADR-040).** The shell already owns the feed-read effect; building the render input next to the read (as slice-08 does with `compose_results`) keeps `viewer-domain` a pure projection over an ALREADY-COMPUTED `WeightedView`, header-unaware, with no scoring orchestration. `scoring` is pure either way; placing the `score()` call in the shell mirrors the established slice-08 pattern. (The scoring TYPES still flow into `viewer-domain` via `ScoreState`; see ADR-040.) |
| **Recompute / re-bucket / normalize in the viewer** | — | **Rejected (WD-CS-6 / I-CS-6, out of scope).** A second formula or any bucket recompute is explicitly forbidden — one SSOT in `ScoringConfig::DEFAULT`. The viewer PROJECTS the pure core's output verbatim. |

## Consequences

### Positive
- The read is structurally read-only: it is a method on a port with NO mutation
  method, so a `Box<dyn StoreReadPort>` still cannot mutate (I-CS-1 carries by
  construction).
- Zero new boundary type, zero new persisted type: `AttributedClaim` already lives
  in `ports`; the `WeightedView` is computed per query and never written (I-CS-4).
- The scoring math is the slice-04 pure core REUSED verbatim — the weight decomposes
  into its `Contribution` rows by construction (no second formula; I-CS-2/I-CS-6).
- Local-first / offline: the read is local DuckDB SQL only; the network being down
  never degrades `/score` (I-CS-5 — unlike `/search`).
- One read path: the viewer's whole read surface stays on `StoreReadPort`.

### Negative
- `StoreReadPort` gains a method (and `adapter-duckdb` an impl). Accepted: it is the
  read-only sibling of the slice-04 `query_by_contributor` SQL, over the SAME shared
  connection — minimal new surface.
- `adapter-http-viewer` + `viewer-domain` take a build dep on `scoring`. Accepted:
  `scoring` is a PURE core (ADR-022); both edges are pure→pure (no I/O enters the
  pure cores) — confirmed by `check-arch` (ADR-041).

## Revisit Trigger
- A future `/score` variant needs the OBJECT dimension or `--traverse` → widen the
  read to take `&ScoringFilter` (the slice-04 ADT) instead of `&Did`; the feed
  contract + the pure scorer are unchanged. Out of scope for slice-09 (OD-CS-5).
- The local feed needs to exclude peer rows (own-only scoring) → the read SQL drops
  the `peer_claims` UNION-ALL leg; the port signature + the pure scorer are
  unchanged.
