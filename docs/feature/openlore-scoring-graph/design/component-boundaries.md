# Component Boundaries — openlore-scoring-graph (slice-04) — DELTA from slice-03

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Style**: Hexagonal + Modular Monolith (ADR-009, inherited)
- **Paradigm**: Functional-leaning Rust (ADR-007, inherited)
- **Extends**: `docs/feature/openlore-federated-read/design/component-boundaries.md`

This document specifies ONLY the component-boundary deltas for slice-04.
The slice-01/02/03 crates are inherited unchanged in their prior
responsibilities. Slice-04 ADDS one pure crate (`scoring`) and EXTENDS
`ports`, `adapter-duckdb`, `cli`, and `xtask`. Everything else is unchanged.

## Crate layout (slice-04 adds ONE pure crate)

```
openlore/                          # workspace root
  crates/
    claim-domain/                  # PURE — UNCHANGED (confidence_bucket reused for display)
    lexicon/                       # PURE — UNCHANGED (nothing new is signed)
    ports/                         # PURE — extended (new StoragePort read methods + AttributedClaim/GraphEdge/TraversalBound ADTs)
    scoring/                       # PURE — NEW (the transparent closed-form weight; formula constants SSOT; NO I/O)
    adapter-duckdb/                # EFFECT — AUGMENTED (recursive-CTE traversal + scoring-feed reads; NO new tables; NO store swap)
    adapter-atproto-did/           # EFFECT — UNCHANGED
    adapter-atproto-pds/           # EFFECT — UNCHANGED
    adapter-system-clock/          # EFFECT — UNCHANGED
    scraper-domain/                # PURE — UNCHANGED (slice-02)
    adapter-github/                # EFFECT — UNCHANGED (slice-02)
    cli/                           # DRIVER — extended (6 explorer flags on graph query + scoring wiring + renderers)
    test-support/                  # test-only — extended (scoring fixtures + cyclic-graph traversal fixture)
  xtask/                           # extended (anti-merging rule extends to scoring/traversal queries; scoring pure-core allowlist)
```

**One new crate.** Unlike slice-03 (zero new crates), slice-04 adds the PURE
`scoring` crate. The slice-03 no-new-crate ethos (WD-26) governs **production
runtime dependencies and storage**; a pure workspace member with NO I/O adds
no operational boundary, no new dependency, and no new probe surface. The
scoring formula is a genuinely new pure-domain concept (the symmetric
counterpart to slice-02's `scraper-domain`), with distinct ADTs, the formula
constants as SSOT, and a clean unit/mutation-test surface that a module
buried in `claim-domain` would muddy. Rationale: ADR-022 + WD-82. The
production crate count goes from 10 to 11.

## Component contract deltas

### `crates/scoring` (PURE) — NEW

**Responsibility**: compute the DERIVED, DISPLAY-ONLY adherence weight for
(subject, object) pairings from attributed claims, via a small closed-form,
reproducible, no-ML formula. Hold the formula constants as the SSOT. Expose
the intermediate per-claim `Contribution` list so `--explain` can reproduce
the arithmetic by hand. NO I/O, NO persistence, NO knowledge of DuckDB.

**Public surface**:

```rust
/// The single entry point. Pure; deterministic.
pub fn score(claims: &[AttributedClaim], cfg: &ScoringConfig) -> WeightedView;

/// Input: a fully-attributed claim (mirrors slice-03 FederatedRow's non-Option author_did discipline).
pub struct AttributedClaim {
    pub author_did: Did,            // non-Option; LOAD-BEARING (anti-merging)
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,            // numeric [0.0, 1.0]; the scoring input (Gate 6)
    pub composed_at: DateTime<Utc>,
    pub relationship: AuthorRelationship,   // You | SubscribedPeer | UnsubscribedCache (slice-03 reuse)
}

/// One claim's contribution to a pairing's weight — the auditable unit --explain renders.
pub struct Contribution {
    pub author_did: Did,            // non-Option; LOAD-BEARING
    pub cid: Cid,
    pub base: f64,                  // = confidence
    pub author_distinct_bonus: f64, // 1.0 for first author on (subject,object); +cfg.author_distinct_bonus per additional distinct author
    pub cross_project_triangulation_bonus: f64, // +cfg.triangulation_bonus when this author asserts the object on >=2 distinct subjects
    pub subtotal: f64,              // = base * author_distinct_bonus_multiplier + triangulation (see formula below)
}

/// One ranked pairing. Cannot exist without its contributions (decomposes by construction).
pub struct WeightedPairing {
    pub subject: String,
    pub object: String,
    pub weight: f64,                // == sum of contributions' subtotals (Gate 2)
    pub bucket: WeightBucket,       // DISPLAY-ONLY
    pub claim_count: u32,
    pub distinct_author_count: u32,
    pub max_confidence: f64,
    pub cross_project_span: u32,    // distinct subjects the top contributor spans
    pub contributions: Vec<Contribution>,   // NON-EMPTY; the decomposition (anti-merging in aggregates)
}

pub struct WeightedView {
    pub ranked: Vec<WeightedPairing>,   // sorted by weight desc, stable tiebreak by subject
}

pub enum WeightBucket { Strong, Moderate, Sparse }

/// The constants SSOT (WD-77 defaults; DESIGN-tunable; small/closed-form/no-ML).
pub struct ScoringConfig {
    pub author_distinct_bonus: f64,         // 0.25 per additional distinct author
    pub cross_project_triangulation_bonus: f64, // 0.5 when same author spans >=2 subjects on the object
    pub strong_threshold: f64,              // bucket cut; with breadth guard
    pub moderate_threshold: f64,
}
impl ScoringConfig { pub const DEFAULT: ScoringConfig = /* WD-77 defaults */ ; }
```

**The formula (WD-77; the transparent contract)**, per (subject, object):

```
For each contributing claim c:
    base(c)                        = c.confidence
    author_distinct_multiplier(c)  = 1.0 + author_distinct_bonus * (distinct_authors_on_pairing - 1) applied once per distinct author
    triangulation_bonus(c)         = cross_project_triangulation_bonus  if c.author asserts `object` on >=2 distinct subjects, else 0.0
    subtotal(c)                    = base(c) * <author-distinct multiplier share> + triangulation_bonus(c)

weight(subject, object)            = sum over contributing claims of subtotal(c)
```

DELIVER fixes the exact per-claim apportionment of the author-distinct
multiplier so that `weight == sum(subtotals)` holds exactly (Gate 2); the
example in `data-models.md` shows the worked arithmetic the AC expect. The
constraint is: small, closed-form, reproducible by hand from `--explain`,
NO ML.

**Bucket rule (WD-74 sparse-honesty)**:

```
weight_bucket(weight, claim_count, distinct_author_count):
    if claim_count <= 1 OR distinct_author_count <= 1 -> Sparse   // breadth guard: thin = sparse regardless of magnitude
    else if weight >= cfg.strong_threshold              -> Strong
    else if weight >= cfg.moderate_threshold            -> Moderate
    else                                                -> Sparse
```

**Forbidden dependencies**: `tokio`, `reqwest`, `duckdb`, `keyring`,
`atrium-api`, `std::fs`, `std::net`, `std::time::SystemTime`, any `adapter-*`
crate, any ML/inference crate. MAY depend on `chrono` (pure time types) and
the pure `Did`/`Cid` value types from `ports`/`claim-domain`. Added to the
`xtask check-arch` pure-core allowlist (I-1/I-2).

**Probe responsibilities** (the pure-core analog — property + mutation tests,
NOT a `probe()`; `scoring` touches no substrate):

- Property: `score` is deterministic (same input -> byte-identical `WeightedView`).
- Property: for every `WeightedPairing`, `weight == sum(contributions.subtotal)` (Gate 2).
- Property: every `Contribution` and every `WeightedPairing` carries a non-empty `author_did`; `contributions` is non-empty (Gate 1 type-level).
- Unit: a single claim at confidence 0.95 buckets `Sparse`, never `Strong` (WD-74).
- Unit: a 2-distinct-author pairing scores higher than a 1-author pairing at equal max confidence (triangulation).
- Mutation testing on `formula.rs` + `bucket.rs` (Earned Trust applied to the tests).

### `crates/ports` (PURE) — extensions

**Slice-04 additions to public surface**:

```rust
// Existing StoragePort — slice-04 extension (NO new port; mirrors slice-03 adding query_federated_by_subject):
pub trait StoragePort {
    // ... slice-01 + slice-03 methods unchanged ...

    /// Which claims assert this object (philosophy), across own + peer stores. Grouped by subject in the renderer.
    fn query_by_object(&self, object: &str) -> Result<Vec<AttributedClaim>, StorageError>;

    /// Every claim authored by this DID, across all subjects, own + peer stores.
    fn query_by_contributor(&self, author_did: &Did) -> Result<Vec<AttributedClaim>, StorageError>;

    /// The attributed-claim feed for the pure scoring core. Every row carries author_did (anti-merging).
    fn query_attributed_for_scoring(&self, filter: &ScoringFilter) -> Result<Vec<AttributedClaim>, StorageError>;

    /// Bounded traversal of contributor<->project<->philosophy edges. Each edge maps to exactly one signed claim.
    fn traverse_graph(&self, start: &GraphNode, bound: &TraversalBound) -> Result<TraversalResult, StorageError>;
}

pub struct AttributedClaim {       // (defined here; consumed by cli AND the pure scoring crate)
    pub author_did: Did,           // non-Option; LOAD-BEARING
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub composed_at: DateTime<Utc>,
    pub relationship: AuthorRelationship,   // reuses the slice-03 enum
}

pub enum ScoringFilter {
    ByObject { object: String },
    BySubject { subject: String },
    ByContributor { author_did: Did },
}

pub enum GraphNode {
    Philosophy { object: String },
    Project { subject: String },
    Contributor { author_did: Did },
}

pub struct TraversalBound {
    pub max_depth: u8,             // default 2 (WD-76); --depth K override
}

pub struct GraphEdge {
    pub from: GraphNode,
    pub to: GraphNode,
    pub claim_cid: Cid,           // LOAD-BEARING: every edge maps to exactly one signed claim (Gate 5)
    pub author_did: Did,          // non-Option; the edge's attribution (anti-merging)
    pub depth: u8,
}

pub struct TraversalResult {
    pub edges: Vec<GraphEdge>,
    pub omitted_edge_count: u32,  // edges beyond the depth bound (for the "N edges omitted" line)
    pub reached_bound: bool,
}
```

**Forbidden dependencies** (unchanged): traits may reference `lexicon`,
`claim-domain`, and the pure value types. No async needed (all new methods
are sync local reads).

**Probe responsibilities**: none (traits don't probe; implementations do).

### `crates/adapter-duckdb` (EFFECT) — AUGMENTED (WD-8 resolves to AUGMENT)

**Slice-04 additions to responsibility**: implement the four new `StoragePort`
read methods over the SAME single-file DuckDB store using recursive CTEs
(traversal) and attributed `UNION ALL` projections (dimensions + scoring
feed). NO new tables. NO store swap. NO new dependency. The recursive CTE
MUST be cycle-safe and depth-bounded (ADR-021).

**Public surface addition**: the four new methods on the existing
`DuckDbStorageAdapter` impl. No new struct.

**Forbidden dependencies** (unchanged): other `adapter-*` crates.

**Probe responsibilities** (slice-04 additions, per ADR-021):

For `StoragePort` (extended):
1. **Scoring-feed attribution round-trip**: write 1 own + 2 peer claims on the
   same (subject, object) by THREE distinct authors; call
   `query_attributed_for_scoring(ByObject)`; assert exactly 3 `AttributedClaim`s
   with three distinct, non-empty `author_did`s. (anti-merging-in-aggregates
   substrate check).
2. **Recursive-CTE termination on a cyclic fixture**: build a cyclic claim
   graph (A↔B via two claims); call `traverse_graph` at depth 3; assert it
   TERMINATES within the 250ms budget (I-5) and emits each edge exactly once
   (visited-set guard). DuckDB recursive CTEs do NOT auto-detect cycles —
   this is the slice-04 substrate-lie probe.
3. **Depth-bound honored**: on a depth-4 fixture, `traverse_graph` with
   `max_depth=2` returns only ≤depth-2 edges and reports `omitted_edge_count > 0`.
4. fsync + schema-version probes inherited from ADR-001/014 (unchanged).

### `crates/claim-domain`, `crates/lexicon` — UNCHANGED.

Nothing is signed; no Lexicon field is added; `confidence_bucket` is reused
unchanged for the numeric-confidence display.

### `crates/adapter-atproto-did`, `crates/adapter-atproto-pds`, `crates/adapter-system-clock` — UNCHANGED.

No network, no DID resolution, no new timestamps in slice-04.

### `crates/cli` (DRIVER) — extensions

**Slice-04 additions to responsibility**: parse the 6 explorer flags on
`graph query` via clap (`--object`, `--contributor`, `--traverse`,
`--depth K`, `--weighted`, `--explain <subject>`) per ADR-020; wire the
`scoring` core (pure call, no probe); route attributed claims from the
extended `StoragePort` methods through `scoring::score`; implement the
`WeightedRenderer` (weight + inputs + printed formula + bucket + never-stored
footer + `--explain` `Contribution` breakdown), `TraversalRenderer` (tree +
"Connections found" callout + omitted-edge line), and `SparseHonestyRenderer`
(the "based on N claims by M authors" line); optionally an
`ExplorerOrientation` once-per-user message (deferred-tunable).

**Public surface addition**: none — clap subcommand/flag definitions extend
internally. `graph query` gains flags; no new verb.

**Forbidden dependencies** (unchanged): none — `cli` is the composition root.

**Probe responsibilities** (slice-04 additions, per ADR-020/022):

1. `graph query --object <philosophy>` groups by subject; every row carries
   exactly one `author_did`; the footer states distinct subject + author
   counts + the no-merge guarantee.
2. `graph query --contributor <did>` lists that DID's claims with the correct
   relationship label (`you` / `subscribed peer` / `unsubscribed cache`); the
   footer states "one developer's reasoning trail, not a community consensus".
3. `graph query --weighted` output: the displayed `weight` equals the sum of
   the `--explain` per-claim subtotals (Gate 2, string-comparable in CI).
4. `graph query --weighted` on a 1-claim-1-author pairing renders `[SPARSE]`
   + the honesty line; never `[STRONG]`.
5. `graph query --traverse` on a cyclic/dense fixture returns within budget,
   bounded to depth 2, with the omitted-edge line.
6. `graph query --weighted --explain <absent-subject>` exits non-zero
   (usage error), distinct from an empty dimension query (exit 0).
7. **Weights never persisted**: after any weighted query, no DuckDB table,
   on-disk artifact, or record contains a weight/bucket string (Gate 4).
8. Every explorer verb succeeds with the network disabled (local-first; I-9).

### `crates/test-support` (test-only) — extensions

Adds: a deterministic scoring fixture (the worked-arithmetic claims set from
`data-models.md`), a cyclic-graph fixture (A↔B) for the recursive-CTE
termination probe, and a sparse fixture (1 claim/1 author). No production
surface.

### `xtask/` (workspace member) — extensions

**Slice-04 additions to responsibility**:

- EXTEND the `check-arch` rule `no_cross_table_join_elides_author` (slice-03,
  ADR-014) to cover the slice-04 scoring-feed + traversal SQL string literals
  in `adapter-duckdb`: any literal mentioning BOTH `claims` and `peer_claims`
  MUST also project `author_did`. The new recursive-CTE and `UNION ALL`
  queries are in the same crate, same string-literal pass.
- ADD `scoring` to the `check-arch` pure-core allowlist (alongside
  `claim-domain`, `lexicon`, `ports`, `scraper-domain`); enforce it imports NO
  I/O crate (I-1/I-2).
- `check-probes` already covers `impl StoragePort` non-stub probe bodies; the
  AUGMENTED adapter's extended probe is picked up unchanged. `scoring` has no
  `probe()` (it is not an adapter) — `check-probes` correctly does not require
  one for a pure crate.

**Public surface**: `cargo xtask check-arch` (extended rule scope + new
allowlist entry), `cargo xtask check-probes` (unchanged trait set). Run from
CI on every commit.

## Cross-component invariants — slice-04 additions (enforced)

| # | Invariant | Enforced by |
|---|---|---|
| I-GRAPH-1 | **Scoring transparent / no ML** (WD-71): the adherence weight is a small closed-form function in the pure `scoring` core; the formula is printed in `--weighted` output; `--explain` reproduces the per-claim arithmetic; NO opaque/learned/non-reproducible weight exists | pure `scoring` crate (no ML dep, in pure-core allowlist) + `weight_equals_formula` property test (Gate 2) + cli probe #3 |
| I-GRAPH-2 | **Anti-merging in aggregates** (WD-73; extends I-FED-1): every weighted/traversed aggregate decomposes to its `(author_did, claim_cid)` contributions; every output row carries one non-`Option` `author_did`; no "consensus" row | three layers: (a) `Contribution`/`GraphEdge` non-`Option` `author_did` + non-empty `WeightedPairing.contributions` (type); (b) `xtask check-arch` `no_cross_table_join_elides_author` extended to scoring/traversal queries (structural); (c) `scoring_aggregate_preserves_attribution` integration test (behavioral) |
| I-GRAPH-3 | **Weights/buckets NEVER persisted** (WD-72; extends WD-10 / I-6): `adherence_weight` and `weight_bucket` are not columns in any table, not fields in any artifact, not serialized to any record | `weight_and_bucket_never_persisted` test (Gate 4) + the design has NO persistence code path for them + extends the slice-01/03 confidence-bucket no-persist unit test to scoring outputs |
| I-GRAPH-4 | **Sparse renders sparse** (WD-74): a thin subgraph (claim_count<=1 OR distinct_authors<=1) buckets `Sparse` regardless of weight magnitude, with the "based on N claims by M authors" honesty line | `weight_bucket` breadth guard (5.1) + `single_claim_is_sparse_even_at_high_confidence` unit test + `sparse_renders_sparse` acceptance test (Gate 3) + cli probe #4 |
| I-GRAPH-5 | **Traversal invents no edges** (WD-76): every `GraphEdge` carries a backing `claim_cid`; the recursive CTE selects from existing claim rows only; it never fabricates/interpolates an edge | `GraphEdge.claim_cid` (type, non-Option) + `traversal_invents_no_edges` acceptance test (Gate 5) + cli probe #5 |
| I-GRAPH-6 | **Bounded traversal** (WD-76): traversal is depth-bounded (default 2) and cycle-safe; the recursive CTE terminates and dedupes edges | `TraversalBound.max_depth` + the recursive-CTE visited-set guard (ADR-021) + adapter probe #2/#3 |
| I-GRAPH-7 | **Read-only / local-first** (WD-79): no explorer verb writes a claim/row/record or opens a socket; all succeed with the network disabled | no write/network code path in slice-04 + cli probe #8 (extends I-9 / KPI-5) |
| I-GRAPH-8 | **Scoring uses numeric confidence** (Gate 6): the `f64` confidence the formula consumes equals the value shown in the per-claim rows; buckets are display-only | `scoring_uses_numeric_confidence` test + the `AttributedClaim.confidence: f64` type carries the raw numeric |

These extend the 12 cross-feature invariants in
`docs/product/architecture/brief.md` and the slice-03 I-FED-1..7. They are
slice-04-scoped; promotion to the brief's I-1..I-12 table is not required
(the meta-invariants — pure-core isolation, probe contract, anti-merging
enforcement model — are already covered and inherited). I-GRAPH-2 is the
direct descendant of I-FED-1; if a future slice needs the
anti-merging-in-aggregates rule enforced cross-feature, promote it with the
ADR that generalizes it.

## Annotation for software-crafter (DELIVER)

```markdown
## Architecture Enforcement (slice-04 additions)

Style: Hexagonal + Modular Monolith (inherited)
Language: Rust
Tools (slice-01/03 + slice-04 additions):
  - cargo-deny (license + bans) — unchanged (zero new production dep)
  - cargo xtask check-arch — extends no_cross_table_join_elides_author to scoring/traversal
    queries; adds `scoring` to the pure-core allowlist
  - cargo xtask check-probes — unchanged trait set; the AUGMENTED adapter probe is picked up
  - mutation testing (nightly) — extend to crates/scoring (formula.rs + bucket.rs)

Rules to enforce (additions to slice-03):
- crates/scoring MUST NOT depend on duckdb/tokio/reqwest/std::fs/std::time::SystemTime
  or any adapter crate or any ML crate (pure-core allowlist)
- No SQL string literal in adapter-duckdb (incl. the new recursive CTEs + scoring-feed
  UNION ALLs) mentions BOTH `claims` and `peer_claims` without projecting `author_did`
- The recursive CTE for traverse_graph MUST be cycle-safe (visited-set) AND depth-bounded
- No code path writes adherence_weight or weight_bucket to any table, artifact, or record
- WeightedPairing.contributions MUST be non-empty; Contribution.author_did is Did (not Option)
- GraphEdge.claim_cid is Cid (not Option) — every edge maps to a signed claim
```

## Annotation for acceptance-designer (DISTILL)

```markdown
## Slice-04 Observable Contracts (additions to slice-03)

### Dimension queries — anti-merging
Every acceptance test driving `graph query --object` / `--contributor` MUST assert:
- --object groups by subject; --contributor lists across all subjects
- Every claim row carries exactly one author_did + numeric confidence + display bucket + cid
- Two identical-content claims from different authors render as TWO rows (never merged)
- --object footer: distinct subject count + distinct author count + the no-merge guarantee
- --contributor footer: "one developer's reasoning trail, not a community consensus"
- --contributor relationship labels: (you) / (subscribed peer) / (unsubscribed cache)
- Unknown object/absent contributor: empty result + suggestion/hint + exit code 0

### Weighted view — transparency + sparse honesty + anti-merging-in-aggregates
Every acceptance test driving `graph query --weighted` MUST assert:
- Results ranked by adherence weight
- Each weight shown WITH inputs (claim count, distinct author count, max confidence, span)
- The formula is printed AND the output states "no ML"
- A footer states weights are a display-only aggregate view, never stored
- A 1-claim-1-author pairing renders [SPARSE] + "based on N claims by M authors" + lead-not-conclusion advice; confidence never manufactured
- Multi-author support raises the triangulation weight; both authors stay individually attributed
- Conflicting claims both contribute per confidence; nothing dropped or averaged-away
- No adherence_weight / weight_bucket appears in DuckDB, on-disk artifacts, or records
- Re-running after a peer pull may yield different weights (proves query-time computation)

### --explain — reproduce by hand (the strongest transparency form)
Every acceptance test driving `--weighted --explain <subject>` MUST assert:
- The breakdown enumerates each contributing claim with author DID, CID, confidence
- Each applied bonus (author-distinct, cross-project triangulation) is shown with its contributor
- The running sum equals the displayed weight (reproducible by hand)
- --explain on a [SPARSE] subject repeats the honesty line
- --explain on a subject NOT in the result set is a usage error (non-zero exit)

### Traversal — connection discovery + no invented edges + bounded
Every acceptance test driving `graph query --traverse` MUST assert:
- A tree from the queried node to projects to authors
- A "Connections found" callout names contributors spanning >=2 projects (when such spans exist)
- Every displayed edge maps to a specific signed claim CID (lookuppable via --subject)
- A node with no connecting edges renders "no connecting edges" — no fabrication
- Default depth 2; --depth K overrides; bounded runs report the omitted-edge count
- Every edge carries the author DID of its backing claim (anti-merging)
- The output states "Traversal does not invent edges."

### Local-first
Every explorer acceptance test MUST assert the command succeeds with the network disabled.

Reference: feature-delta.md WD-69..WD-79, ADR-020..022, design's sections 5.1 + 5.2 + 9.
The 6 integration gates in shared-artifacts-registry.md map 1:1 to acceptance tests:
Gate 1 scoring_aggregate_preserves_attribution | Gate 2 weight_equals_formula |
Gate 3 sparse_renders_sparse | Gate 4 weight_and_bucket_never_persisted |
Gate 5 traversal_invents_no_edges | Gate 6 scoring_uses_numeric_confidence.
```

## Annotation for platform-architect (DEVOPS)

```markdown
## External Integrations Requiring Contract Tests (slice-04)
- NONE. Slice-04 is a read-only LOCAL slice (WD-79); no external API consumed; no
  new network surface. Confirm the local-first guardrail (extends slice-01 KPI-5 / I-9):
  the explorer path adds no network call.

## Earned Trust Telemetry Hooks (slice-04 additions)
- New probe failure reasons emitted via tracing `health.startup.refused`:
  - storage.scoring_feed_attribution_lost  (query_attributed_for_scoring dropped author_did)
  - storage.traversal_nonterminating       (recursive CTE failed to terminate within budget)
  - storage.traversal_depth_bound_violated (returned edges beyond max_depth)

## KPI instrumentation (handed off per outcome-kpis.md DEVOPS section)
- graph.connection.surfaced{ session_id, span_kind, project_count } (KPI-GRAPH-1 north star;
  structural counts only — NEVER claim contents)
- graph.query.duration_seconds histogram, labeled by claim-count bucket and dimension
  (KPI-GRAPH-6); P95 > 5s for the <=200-claim bucket is an informational alert
- Release-blocking alerts on KPI-GRAPH-2/3/4 != 100% (anti-merging / transparency / sparse-honesty)
- No new external service to provision (read-only, local-first).
```
