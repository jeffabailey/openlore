# ADR-022: Pure `scoring` Core + Anti-Merging-in-Aggregates Invariant — Transparent Display-Only Adherence Weight

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-71/WD-72/WD-73/WD-74/WD-77 locks from Luna (nw-product-owner) for openlore-scoring-graph
- **Feature**: openlore-scoring-graph (slice-04)
- **Extends**: ADR-007 (functional Rust paradigm — pure core), ADR-009 (hexagonal — pure-core isolation), and slice-03 ADR-014 (anti-merging invariant I-FED-1). The slice-03 anti-merging guarantee is CARRIED into a new failure surface: aggregates.

## Context

Slice-04 introduces the adherence WEIGHT — a derived value that ranks
(subject, object) pairings by how well-supported a philosophy is. This is the
single most trust-sensitive surface in the product: an opaque or merging score
would re-trigger the exact aggregator distrust (HN/Reddit "X people value
this", attribution-erasing consensus numbers) that OpenLore exists to avoid
(J-002).

DISCUSS locked five load-bearing constraints:

- **WD-71**: scoring is transparent + auditable; a small closed-form function;
  NO ML; the formula is displayed; `--explain` reproduces the arithmetic.
- **WD-72**: weights/buckets are DERIVED + DISPLAY-ONLY; never persisted,
  signed, or published.
- **WD-73**: anti-merging extends to aggregates; a score is an aggregate VIEW
  that decomposes to its `(author_did, claim_cid)` contributions; every output
  row keeps its author DID.
- **WD-74**: sparse renders sparse; thin evidence is labeled `[SPARSE]` with an
  honesty line; no manufactured confidence.
- **WD-77**: the default formula shape (constants tunable; function small /
  closed-form / no-ML).

DESIGN owns:

1. WHERE the formula lives (a new crate vs a module in `claim-domain`).
2. The exact ADTs + signature + the constants' home.
3. How `--explain` reproduces the arithmetic from the same single source.
4. How the anti-merging-in-aggregates invariant is ENFORCED (not just
   achieved).
5. How sparse-honesty is made structural (driven by evidence breadth).

## Decision

**Place the formula in a NEW PURE crate `crates/scoring` (ADR-007), compute
the weight in Rust (never in SQL), and enforce the anti-merging-in-aggregates
invariant at three semantically orthogonal layers.**

### The `scoring` crate (pure; new — WD-82)

```rust
pub fn score(claims: &[AttributedClaim], cfg: &ScoringConfig) -> WeightedView;

pub struct AttributedClaim { author_did: Did, cid: Cid, subject, predicate, object, confidence: f64, composed_at, relationship }
pub struct Contribution    { author_did: Did, cid: Cid, base: f64, author_distinct_bonus: f64, cross_project_triangulation_bonus: f64, subtotal: f64 }
pub struct WeightedPairing { subject, object, weight: f64, bucket: WeightBucket, claim_count, distinct_author_count, max_confidence, cross_project_span, contributions: Vec<Contribution> /* NON-EMPTY */ }
pub struct WeightedView    { ranked: Vec<WeightedPairing> }
pub enum   WeightBucket    { Strong, Moderate, Sparse }
pub struct ScoringConfig   { author_distinct_bonus: f64, cross_project_triangulation_bonus: f64, strong_threshold: f64, moderate_threshold: f64 }
impl ScoringConfig { pub const DEFAULT: ScoringConfig = /* WD-77 defaults: 0.25, 0.50, thresholds */ ; }
```

A NEW crate (not a module in `claim-domain`) because: the formula is a
distinct pure-domain concept with its own ADTs, the constants as SSOT, and a
clean mutation-test surface; it is the symmetric counterpart to slice-02's
`scraper-domain`; and the slice-03 no-new-crate ethos (WD-26) governs
production runtime dependencies + storage, not a pure workspace member with no
I/O. It adds NO external dependency.

### The formula (WD-77; the transparency contract)

```
Constants (ScoringConfig::DEFAULT; the SSOT; compile-time const; small/closed-form/no-ML):
    author_distinct_bonus              = 0.25
    cross_project_triangulation_bonus  = 0.50

Per (subject, object), for each contributing claim c by author A:
    base(c)            = c.confidence
    author_multiplier  = 1.0 + author_distinct_bonus * (distinct_authors_on_pairing - 1)   # apportioned per claim
    triangulation(c)   = cross_project_triangulation_bonus  if A asserts `object` on >=2 distinct subjects else 0.0
    subtotal(c)        = base(c) * <author-multiplier share> + triangulation(c)
    weight             = sum over contributing claims of subtotal(c)      # weight == sum(subtotals) — Gate 2
```

The constants are a compile-time `const`, not config or learned weights — a
constant change is a code change (WD-71). `--explain` renders the
`Contribution` list (the per-claim subtotals); the running sum reproduces the
weight by hand. There is ONE arithmetic path: `--weighted` shows the summed
weight, `--explain` shows the contributions; both come from the SAME
`score()` output (no second weight-computing path).

### Display-only discipline (WD-72)

`adherence_weight` and `weight_bucket` are RETURNED by `score()` and rendered;
nothing writes them. There is NO persistence code path. The no-persist test
(extending the slice-01/03 confidence-bucket test) scans for
`STRONG|MODERATE|SPARSE` + `adherence_weight` substrings across every table
and on-disk artifact (Gate 4). A future need to persist requires a WD + ADR.

### Sparse honesty driven by evidence breadth (WD-74)

```
weight_bucket(weight, claim_count, distinct_author_count):
    if claim_count <= 1 OR distinct_author_count <= 1 (and no cross-project triangulation breadth) -> Sparse
    else if weight >= cfg.strong_threshold   -> Strong
    else if weight >= cfg.moderate_threshold -> Moderate
    else                                     -> Sparse
```

The bucket takes BREADTH inputs, not just weight: a single claim at confidence
0.95 buckets `[SPARSE]`, never `[STRONG]`. Cross-project triangulation by the
same author counts as evidence breadth for the bucket (the worked-example
nuance is locked to the user stories via Q-DELIVER-SCORE-1).

### Anti-merging-in-aggregates (I-GRAPH-2) — three-layer enforcement (WD-88)

The invariant: **NO weighted/traversed aggregate may exist that cannot
enumerate the individually-attributed `(author_did, claim_cid)` claims that
produced it.** Carries slice-03's I-FED-1 into the aggregate surface.

| Layer | What it checks | Tool |
|---|---|---|
| **Subtype / type** | `Contribution.author_did` and `GraphEdge.author_did` are `Did`, not `Option<Did>` (compile error if dropped). `WeightedPairing.contributions` is non-empty by construction — a pairing cannot exist without its decomposition. There is NO API returning a bare weight without contributions. | Rust type system |
| **Structural / SQL** | `cargo xtask check-arch` rule `no_cross_table_join_elides_author` (slice-03) EXTENDS to the slice-04 scoring-feed + traversal SQL string literals in `adapter-duckdb`: any literal mentioning BOTH `claims` and `peer_claims` MUST project `author_did`. Critically, **aggregation (the weight) happens in the pure Rust core, NEVER in SQL** — the SQL returns per-claim rows; it never `SUM`/`GROUP BY`s across authors. | `xtask check-arch` (extended rule) |
| **Behavioral / acceptance** | Integration test `scoring_aggregate_preserves_attribution`: a (subject, object) with claims from N distinct authors yields a `WeightedPairing` whose `--explain` decomposes to exactly N attributed `Contribution`s; NO faceless "consensus weight" appears; two identical-content claims from different authors stay TWO contributions. | `tests/scoring_aggregate_preserves_attribution.rs` (DISTILL gate; KPI-GRAPH-2) |

A single-layer bypass is caught by ≥1 other.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **ML / learned scoring model** | Locked rejected (WD-71). An ML score is unauditable and non-reproducible; it re-triggers the aggregator distrust the product exists to avoid. The weight MUST be reproducible by hand from `--explain`. |
| **Formula as a module in `claim-domain`** | Would muddy that crate's signing/CID focus; the formula is a distinct pure-domain concept with its own ADTs, constants SSOT, and mutation-test surface. A new PURE crate adds no external dependency and no operational boundary (WD-82). |
| **Aggregate the weight in SQL (`SUM`/`GROUP BY`)** | Would eliminate `author_did` at the SQL boundary — the anti-merging-in-aggregates violation. Computing in Rust keeps the per-claim contributions as discrete rows the aggregate decomposes into. Caught by the structural xtask rule. |
| **Persist/cache computed weights for query speed** | Locked rejected (WD-72). A persisted score goes stale and tempts federation of a derived value. Computed at query time; bounded depth + columnar scan keep it fast (KPI-GRAPH-6). |
| **Bucket by weight magnitude alone (no breadth guard)** | Would let a single high-confidence claim render `[STRONG]` — manufacturing confidence from thin evidence (WD-74 violation). The bucket takes claim_count + distinct_author_count so thin = sparse regardless of magnitude. |
| **Down-weight countered/retracted claims** | Deferred (WD-85 / OD-GRAPH-2 default): silently subtracting a counter would make the weight non-reproducible from the visible claims (Gate 2 violation). Slice-04 contributes all signed claims per confidence; the counter is SHOWN in `--explain`/traversal, not silently applied. |

## Consequences

### Positive

- The weight is reproducible by hand from `--explain` — the strongest form of
  the J-002 transparency promise.
- The pure `scoring` crate is trivially unit + mutation-testable (Earned Trust
  applied to the formula); no I/O to mock.
- Anti-merging-in-aggregates is enforced at three independent layers; a
  developer cannot accidentally write a merging aggregate without ≥1 layer
  flagging it.
- Display-only by construction: there is no persistence code path to misuse.
- Sparse honesty is structural (the bucket function's breadth guard), not a
  render-layer afterthought.

### Negative

- A genuinely strong single-source claim is "under-sold" as `[SPARSE]`.
  **Accepted** per WD-74: thin evidence MUST look thin; this is the
  load-bearing UX, not a bug.
- A new crate (the first since slice-02) grows the workspace. **Mitigation**:
  it is pure, adds no external dependency, and is the right home for a distinct
  concept (WD-82).
- The two tables-like-rows feeding one Rust aggregation invites a future "DRY
  it in SQL" refactor that would merge in SQL. **Mitigation**: the three-layer
  enforcement + a comment citing this ADR; the structural xtask rule catches
  the SQL-merge attempt.
- The constants are a product judgment call (auto-mode, no validation
  interviews). **Mitigation**: KPI-GRAPH-3 + the day-30 interview; the
  constants are a trivial code change in the pure core.

### Earned Trust

`scoring` is a PURE crate — it has no `probe()` (it touches no substrate). Its
Earned-Trust analog is property-based + mutation testing (TDD RED→GREEN is
Earned Trust applied to code; mutation testing is Earned Trust applied to the
tests):

1. `weight_is_deterministic` — same input -> byte-identical `WeightedView`.
2. `weight_equals_sum_of_contributions` — Gate 2; the by-hand reproduction.
3. `single_claim_is_sparse_even_at_high_confidence` — WD-74 breadth guard.
4. `two_distinct_authors_outrank_one_at_equal_confidence` — triangulation.
5. Mutation testing on `formula.rs` + `bucket.rs` — the surviving-mutant gate
   proves the tests actually pin the arithmetic the user must reproduce.

The anti-merging-in-aggregates invariant's behavioral layer
(`scoring_aggregate_preserves_attribution`) is the storage-side Earned-Trust
check; the AUGMENTED `adapter-duckdb` probe (ADR-021) exercises the
scoring-feed attribution round-trip at startup.

## Revisit Trigger

- KPI-GRAPH-3 / the day-30 interview shows users find the WD-77 default
  constants unintuitive — tune the constants (a code change in the pure core).
- A JTBD emerges for counter-aware down-weighting (WD-85 / OD-GRAPH-2) — a
  future WD + ADR; the formula must stay reproducible (the counter contribution
  shown, not hidden).
- A future slice genuinely needs to persist/cache a weight (WD-72) — requires
  a WD + ADR with a stated staleness/federation-hazard mitigation.
- A second derived metric beyond adherence weight emerges — extend the
  `scoring` crate with a new pure function + ADTs, same template.
