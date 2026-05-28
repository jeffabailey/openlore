# Wave Decisions — DESIGN — openlore-scoring-graph (slice-04)

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Inherits from**: DISCUSS WD-69..WD-79 (feature-delta.md); WD-1..WD-68 + ADR-001..019 (slices 01/02/03)
- **Format**: WD-XX entries; one decision per row; rationale + status + locks downstream

## DESIGN-wave decisions

| # | Decision | Rationale | Status | Locks |
|---|---|---|---|---|
| WD-80 | Slice-04 is a port-method + pure-crate EXTENSION of slice-01/03, not a re-architecture. Same hexagonal modular monolith; same single binary; same single DuckDB file. | Conservative scope; slice-04 validates the scoring/traversal thesis ON TOP of the proven federated read surface. No new architectural style, no new store, no new external dependency. | LOCKED | DELIVER extends existing crates + adds the one pure `scoring` crate; introducing any other crate or a store requires returning to DESIGN. |
| WD-81 | **THE WD-8 STORE REVISIT: AUGMENT `adapter-duckdb` with recursive-CTE graph traversal + scoring-feed queries. DO NOT swap/add a graph store.** | Slice-04's traversal workload (low-thousands of claims, bounded default depth 2 per WD-76) is comfortably handled by DuckDB recursive CTEs. A graph store adds a new dependency, a new adapter + probe, a second backup target, a claims↔graph sync problem, and a NEW anti-merging enforcement substrate — large cost for marginal benefit at slice-04 scale. AUGMENT keeps zero new dependency, the single-file local-first simplicity, and lets the slice-03 `no_cross_table_join_elides_author` xtask SQL rule extend naturally onto the same SQL substrate. Full trade-off table in architecture-design.md §9. | LOCKED. **RESOLVES OD-GRAPH-1.** Per ADR-021. | DELIVER implements recursive-CTE traversal (cycle-safe + depth-bounded) over the existing schema; NO graph-store crate; NO new table. Revisit trigger in ADR-021. |
| WD-82 | **The scoring formula lives in a NEW PURE crate `crates/scoring`** (not a module in `claim-domain`). | The formula is a genuinely new pure-domain concept (distinct ADTs `AttributedClaim`/`Contribution`/`WeightedView`, the constants as SSOT, a clean unit/mutation-test surface) — the symmetric counterpart to slice-02's `scraper-domain`. The slice-03 no-new-crate ethos (WD-26) governs PRODUCTION RUNTIME DEPENDENCIES and STORAGE; a pure workspace member with NO I/O adds no external dependency, no operational boundary, and no probe surface. Burying it in `claim-domain` would muddy that crate's signing/CID focus. | LOCKED. Per ADR-022. | DELIVER creates `crates/scoring` (pure); `xtask check-arch` adds it to the pure-core allowlist; production crate count 10 -> 11; external-dependency count unchanged. |
| WD-83 | The slice-04 read surface (dimensions, traversal, scoring-feed) is an EXTENSION of `StoragePort` — NOT a new port. | All four new methods are SYNC local reads against the same store; the boundary they cross is identical to the existing storage boundary. Mirrors slice-03's choice to add `query_federated_by_subject` to `StoragePort` rather than spawn a port. A new read port would split one storage concern across two ports with no compositional benefit. | LOCKED. | DELIVER adds `query_by_object`, `query_by_contributor`, `query_attributed_for_scoring`, `traverse_graph` to `StoragePort`; the AUGMENTED `adapter-duckdb` impl gains them; the extended `StoragePort::probe` covers them. |
| WD-84 | **`graph query` is amended with explorer flags** (`--object`, `--contributor`, `--traverse`, `--depth K`, `--weighted`, `--explain <subject>`), NOT new verbs. | A `graph query` result is still "attributed claims, possibly ranked/traversed" — a flag modifies scope, not the verb's observable kind. Symmetric with the slice-03 `--federated` flag precedent (ADR-013). Keeps the verb count flat. | LOCKED. Per ADR-020. | DELIVER implements the flags on the existing verb; the two combinable surfaces (`--weighted --explain`, `--traverse --depth`) are validated by DISTILL. |
| WD-85 | **Retraction/counter-aware scoring: countered/soft-retracted claims CONTRIBUTE NORMALLY in slice-04** (per their confidence), with the counter relationship SHOWN in `--explain`/traversal, NOT silently applied. Down-weighting deferred. | Slice-03 keeps countered claims visible (coexist, never overwrite). Down-weighting is a richer scoring concern; the slice-04 transparent default is "all signed claims contribute per their confidence; the counter is shown, not silently subtracted." Silently down-weighting would make the weight non-reproducible by the visible claims (violating WD-71/Gate 2). | LOCKED. **RESOLVES OD-GRAPH-2** at default. | DELIVER scores all signed claims uniformly; `--explain` + traversal surface the counter relationship; down-weighting requires a future WD + ADR. |
| WD-86 | **Scoring formula constants ship at the WD-77 defaults** (author bonus 0.25, triangulation bonus 0.5, bucket thresholds), in `scoring::ScoringConfig::DEFAULT` as compile-time `const` (the SSOT). DESIGN may tune; constants MUST stay small/closed-form/no-ML. | A concrete default makes the transparency contract testable now (Gate 2). The constants are a compile-time `const`, not config or learned weights (WD-71). KPI-GRAPH-3 + the day-30 interview surface whether the weighting is sensible; the constants are trivially revisable in the pure core. | LOCKED. **RESOLVES OD-GRAPH-3** at default. | DELIVER ships the defaults; tuning is a code change + a test update; a config-file path requires a WD + ADR (Q-DELIVER #4). |
| WD-87 | **Explorer verbs IMPLY federated scope by default** (own + subscribed peers + unsubscribed-cache + scraper-signed), with `--federated` still accepted for symmetry/explicitness. | The explorer reads the WHOLE local graph; requiring `--federated` on every explorer query is friction that kills exploration (KPI-GRAPH-6). The slice-04 query SHAPES already `UNION ALL` own + peer with attribution preserved (I-GRAPH-2). `--subject` WITHOUT any explorer/federated flag stays byte-identical to slice-01 (own-claims-only) for backward compatibility. | LOCKED. **RESOLVES OD-GRAPH-4.** | DELIVER: `--object`/`--contributor`/`--traverse`/`--weighted` include peers by default; bare `--subject` (no new flags) is unchanged slice-01 behavior; `--federated` remains a no-op-when-implied accepted flag. DISTILL asserts the default scope. |
| WD-88 | **Anti-merging-in-aggregates (I-GRAPH-2) is enforced at THREE semantically orthogonal layers** (type / structural / behavioral), mirroring slice-03 WD-30 / ADR-014. Aggregation (the weight) happens in the PURE scoring core in Rust, NEVER in SQL. | A single-layer bypass is caught by the other two. Computing the weight in Rust (not SQL `SUM`/`GROUP BY`) keeps the aggregate decomposable: the per-claim `Contribution`s exist as rows the way the SQL returns them. This is the load-bearing carry of I-FED-1 into the new aggregate surface. | LOCKED. Per ADR-022. | DELIVER ships: non-`Option` `author_did` on `Contribution`/`GraphEdge` + non-empty `WeightedPairing.contributions` (type); `no_cross_table_join_elides_author` extended to scoring/traversal queries (structural); `scoring_aggregate_preserves_attribution` test (behavioral). SQL never aggregates across authors. |
| WD-89 | **Weights/buckets have NO persistence code path** (I-GRAPH-3; WD-72). The scoring core RETURNS values; nothing writes them. | A persisted score goes stale and tempts federation of a derived value (the aggregator failure J-002 distrusts). Computing at query time keeps the weight honestly tied to the current local graph. The no-persist invariant is enforced like the slice-01/03 confidence buckets. | LOCKED. Per WD-72 + ADR-022. | DELIVER ships NO write path for `adherence_weight`/`weight_bucket`; the no-persist unit test extends to scan for `STRONG|MODERATE|SPARSE` + `adherence_weight` in all tables + artifacts (Gate 4). |
| WD-90 | **Sparse honesty (I-GRAPH-4; WD-74) is driven by EVIDENCE BREADTH, not weight magnitude.** A pairing with `claim_count <= 1 OR distinct_author_count <= 1` (and no cross-project triangulation breadth) buckets `[SPARSE]` regardless of confidence. | The direct mitigation of the J-002 sparse-data anxiety. A single high-confidence claim must look thin, never dressed as `[STRONG]`. The bucket function takes breadth inputs, not just weight. | LOCKED. Per ADR-022. | DELIVER implements the `weight_bucket` breadth guard; `single_claim_is_sparse_even_at_high_confidence` unit test + `sparse_renders_sparse` acceptance test (Gate 3). The cross-project-triangulation-counts-as-breadth nuance is locked to the worked examples (Q-DELIVER-SCORE-1). |
| WD-91 | **Traversal invents no edges + is bounded + cycle-safe** (I-GRAPH-5/6; WD-76). Every `GraphEdge` carries a backing `claim_cid`; the recursive CTE walks existing rows only, depth-bounded (default 2), with a visited-set cycle guard. | Traversal that interpolated edges would fabricate reasoning no author signed (breaks WD-71 auditability). DuckDB recursive CTEs do NOT auto-detect cycles — the design refuses to trust the substrate and bounds + dedupes explicitly (the slice-04 "what if the substrate lies" check). | LOCKED. Per ADR-021. | DELIVER implements the cycle-safe depth-bounded CTE; `GraphEdge.claim_cid` non-Option; `traversal_invents_no_edges` test (Gate 5) + adapter probe #2/#3 (termination + depth bound within 250ms). |
| WD-92 | **Slice-04 adds NO external integration and NO new network surface** (WD-79). The explorer path is local-only; every explorer verb succeeds with the network disabled. | Read-only LOCAL slice; cross-user aggregation is slice-05 (WD-79). This is an architectural property verified by the local-first probe (extends I-9 / KPI-5). | LOCKED. | DELIVER ships no write/network code path in slice-04; cli probe #8 asserts network-disabled success. DEVOPS handoff: no external contract test needed. |
| WD-93 | The three DESIGN-wave ADRs (020-022) are accepted with this handoff; no further DESIGN iterations required pending peer review. | Each ADR has 2+ alternatives considered, the DISCUSS locks as binding inputs, and an Earned Trust section translating to concrete probe/test contracts. Slice-04 is a disciplined extension; the only novel architectural risk (recursive-CTE cycle safety) is met head-on by a dedicated probe. | LOCKED pending Atlas (solution-architect-reviewer) approval. | Reviewer may flag issues for an iteration-2 pass. |

## OD-GRAPH resolutions (consolidated)

| OD | DISCUSS default | DESIGN resolution |
|---|---|---|
| OD-GRAPH-1 (store: swap vs augment) | DESIGN's call; recommend augment | **WD-81 LOCKED: AUGMENT DuckDB with recursive CTEs. No graph store.** Per ADR-021. |
| OD-GRAPH-2 (countered claims in scoring) | Contribute normally; counter shown not applied | **WD-85 LOCKED: contribute normally; counter visible in --explain/traversal; down-weighting deferred.** |
| OD-GRAPH-3 (formula constants) | Ship WD-77 defaults; DESIGN may tune | **WD-86 LOCKED: ship WD-77 defaults as compile-time `const` SSOT; tunable; no ML.** Per ADR-022. |
| OD-GRAPH-4 (explorer verbs imply federated?) | Imply federated; keep --federated for symmetry | **WD-87 LOCKED: explorer verbs imply federated; bare --subject unchanged; --federated still accepted.** |

## Decisions DEFERRED to DELIVER

| # | Question | Default for DELIVER | Why deferred |
|---|---|---|---|
| Q-DELIVER-SCORE-1 | The exact bucket rule when cross-project triangulation raises a single-claim pairing's weight (does Rachel's cargo+nixpkgs span lift cargo out of `[SPARSE]`?) | Cross-project triangulation by the SAME author counts toward evidence breadth for the bucket (so a triangulated single-claim pairing is NOT sparse); a single claim with NO triangulation and NO co-author stays `[SPARSE]` regardless of confidence | The worked examples in user-stories.md narrate cargo as `[STRONG]` via triangulation AND require single-claim honesty; DELIVER picks one consistent rule, DISTILL asserts it against the worked arithmetic. Flagged `# DISTILL: confirm`. |
| Q-DELIVER-1 | Exact recursive-CTE SQL (visited-set representation; depth column type; omitted-edge counting) | The illustrative shape in data-models.md (delimited visited-path string + depth column) | ADR-021 fixes the cycle-safety + depth-bound CONTRACT; the exact SQL is DELIVER's, subject to adapter probe #2/#3 + Gate 5. |
| Q-DELIVER-2 | One `query_attributed_for_scoring(ScoringFilter)` method vs three thin per-dimension methods | One filtered method (smaller anti-merging enforcement surface) | No invariant blocks either; crafter confirms by ergonomics. |
| Q-DELIVER-3 | `crates/scoring` module split (single `lib.rs` vs `formula.rs`+`bucket.rs`+`types.rs`) | Split for mutation-test clarity | Crafter's call; no invariant. |
| Q-DELIVER-4 | `ScoringConfig` compile-time `const` vs future config file | Compile-time `const` (WD-86; a constant change is a code change, not a learned weight) | A config path would require a WD + ADR; not needed for slice-04. |
| Q-DELIVER-5 | Exact `[STRONG]/[MODERATE]/[SPARSE]` threshold constants | The ADR-022 / WD-77-derived defaults | DELIVER may tune within the small/closed-form constraint; a change is code + test update. |
| Q-DELIVER-6 | Exact "Connections found" callout + omitted-edge line format | Match the journey/user-story mockup lines byte-for-byte where possible | DISTILL acceptance tests assert specific lines; DELIVER fills the format that satisfies them. |
| Q-DELIVER-7 | Whether the once-per-user explorer orientation message ships in slice-04 (mirroring slice-03 OrientationState) | Optional; not load-bearing | DELIVER's call against DISTILL scenarios; `[explorer]` key in identity.toml if shipped. |

## ADR proposals (this DESIGN wave)

| ADR | Title | Status | Replaces / amends |
|---|---|---|---|
| ADR-020 | Graph-Query Verb Amendment — Explorer Flags (`--object`, `--contributor`, `--traverse`/`--depth`, `--weighted`, `--explain`) | Proposed | Amends ADR-003 + ADR-013 (verb contract) |
| ADR-021 | DuckDB Recursive-CTE Graph Traversal — the WD-8 Store Revisit Resolution (AUGMENT, not swap) | Proposed | Revisits/affirms ADR-001 (DuckDB store) |
| ADR-022 | Pure `scoring` Core + Anti-Merging-in-Aggregates Invariant — Transparent Display-Only Adherence Weight | Proposed | Extends ADR-007 (functional) + ADR-009 (hexagonal) + slice-03 ADR-014 (anti-merging) |

## Inherited locks summary (do NOT relitigate)

| Source | Locks |
|---|---|
| Slice-01 | ADR-001..012; WD-1..WD-13; the 12 cross-feature invariants in `docs/product/architecture/brief.md` |
| Slice-02 | ADR-017..019; WD-46..WD-68; I-SCR-1..7 (scraper-signed claims are normal author claims; participate in scoring with no special weight, WD-58) |
| Slice-03 | ADR-013..016; WD-26..WD-45; I-FED-1..7 (anti-merging at storage/query/display/test — EXTENDED to aggregates this slice) |
| Slice-04 DISCUSS | WD-69..WD-79 (feature-delta.md) + OD-GRAPH-1..4 (resolved above) |
| Slice-04 DESIGN | WD-80..WD-93 (this file) + ADR-020..022 |

## Handoff

This file is the canonical DESIGN-wave decision record. It is consumed by:

- **Atlas (solution-architect-reviewer)** for peer review iteration 1.
- **DISTILL (nw-acceptance-designer)** for resolving `# confirm` flags
  (especially Q-DELIVER-SCORE-1 bucket rule + WD-87 default federated scope)
  and turning the 6 integration gates into executable acceptance tests.
- **DEVOPS (nw-platform-architect)** for instrumentation (KPI-GRAPH-1
  `graph.connection.surfaced` + KPI-GRAPH-6 `graph.query.duration_seconds`,
  both privacy-preserving structural counts) and confirming NO external
  contract test is needed (read-only local slice, WD-92).
- **DELIVER (nw-software-crafter, functional paradigm per ADR-007)** for
  implementation; Q-DELIVER-SCORE-1 + Q-DELIVER-1..7 are crafter's call within
  the locked contracts.

### Component Inventory update (for finalize; NOT applied to brief.md now)

At finalize (slice-03 precedent — DESIGN does not edit the SSOT brief
mid-wave), the brief's Component Inventory gains one row:

| Crate | Kind | Purpose | Shipped in |
|---|---|---|---|
| `crates/scoring` | pure core | Transparent closed-form adherence weight (display-only); formula constants SSOT; no I/O | slice-04 |

Production crate count: 10 -> 11 (+1 test-support +1 xtask). External
dependency count: unchanged (zero new). The WD-8 line in the brief's Style
section ("revisit DuckDB at slice-04") resolves to AUGMENT (WD-81 / ADR-021)
and should be annotated as resolved at finalize.

### Handoff-ready?

**YES.** WD-80..WD-93 LOCKED; OD-GRAPH-1..4 resolved; ADR-020..022 proposed
(pending Atlas review); the WD-8 store revisit decided (AUGMENT); zero new
external dependency; the anti-merging-in-aggregates invariant designed with
three-layer enforcement; the recursive-CTE cycle-safety substrate-lie probe
specified. DISTILL has the 6 gates; DEVOPS has the KPI events; DELIVER has the
contracts + the deferred Q-DELIVER set. No blockers.
