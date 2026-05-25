# Story Map: openlore-foundation

## User: P-001 Senior Engineer Solo Builder

## Goal: turn a philosophical observation about a project into a signed, attributable, federated, locally-owned claim that I can read back, weight, retract, and have others engage with as my reasoning — not as truth.

## Backbone (umbrella view across all sibling features)

The user's lifetime journey with OpenLore spans 5 activities. `openlore-foundation`
delivers Activity 1 end-to-end (walking skeleton). Sibling features born from this
discussion (slices 02-05) deliver Activities 2-5.

| Activity 1: Author | Activity 2: Mine | Activity 3: Federate | Activity 4: Weigh | Activity 5: Explore |
|---|---|---|---|---|
| Compose a claim | Run a scraper | Subscribe to a peer's DID | Triangulate adherence | Browse the graph |
| Sign with my DID | Extract candidate philosophies | Pull their claims | Score contributor↔project | Search by philosophy |
| Persist locally | Stage them as draft claims | Render with attribution | Score project↔philosophy | Surface non-obvious links |
| Publish to PDS | Promote selected drafts | Counter-claim | Configure trust weights | Share a query link |
| Read back via graph | Track scraper provenance | Retract |  | UI/AppView |

### Walking Skeleton (this feature: `openlore-foundation`)

The single-task slice across Activity 1 only, demonstrating Lexicon + signing + DuckDB
persistence + PDS publication + local query end-to-end. Defers everything else.

| Activity 1 task |
|---|
| WS-1: Compose claim via `openlore claim add ...` |
| WS-2: Sign with DID |
| WS-3: Persist to DuckDB at `~/.local/share/openlore/openlore.duckdb` |
| WS-4: Publish to author's PDS |
| WS-5: Read back via `openlore graph query --subject <subject>` |

> This is the walking skeleton because it is the thinnest possible end-to-end path
> through ALL of: Lexicon definition, signing, DuckDB persistence, ATProto publication,
> and local query. Disprove this and the entire OpenLore thesis is in trouble.

## Sibling features birthed from the carpaccio split (NOT in openlore-foundation)

Each becomes its own DISCUSS wave under its own feature directory. Listed here so the
priority chain and dependencies are visible.

| Slice | Sibling feature | Activity | Walking skeleton hypothesis |
|---|---|---|---|
| slice-01 | **openlore-foundation** (this) | Author | Claims model + signing + DuckDB + PDS + local query compose into a coherent end-to-end |
| slice-02 | `openlore-github-scraper` | Mine | A scraper can turn unstructured GitHub data into draft claims that a human promotes |
| slice-03 | `openlore-federated-read` | Federate | Subscribing to another DID's claim stream produces useful attributed reads with no merge confusion |
| slice-04 | `openlore-scoring-graph` | Weigh | Triangulation weighting across multiple projects produces non-obvious, defensible adherence scores |
| slice-05 | `openlore-appview-search` | Explore | An AppView over the federated graph delivers query-driven discovery beyond the local store |

## Priority Rationale

| Priority | Slice | Target outcome | Why this order |
|---|---|---|---|
| 1 | slice-01 (openlore-foundation) | A real signed claim makes it from compose to round-trip read | This IS the walking skeleton. Disprove and OpenLore has no foundation. Validates Lexicon, signing, DuckDB, ATProto, and local query in one slice. |
| 2 | slice-03 (federated-read) | Reader can ingest a peer's claims with attribution preserved | Validates the federation thesis BEFORE investing in scrapers or scoring. Scrapers without federation are just a local opinion DB. |
| 3 | slice-02 (github-scraper) | A scraper produces draft claims a human promotes | Multiplies authoring throughput once federation works. Strictly post-federation because scrapers must serialize to the same claim shape that peers consume. |
| 4 | slice-04 (scoring-graph) | Triangulation weights make non-obvious adherence visible | Only valuable once there are enough claims (from authoring + scraping + federation) to weight. Premature without slices 01-03. |
| 5 | slice-05 (appview-search) | Web UI / AppView delivers discovery beyond the CLI | Last because the CLI must be the source of truth for the model; UI is presentational. Also the longest to build and easiest to scope-creep. |

Rationale at a higher level: validate federation BEFORE scraping (slice-03 before slice-02)
because the federated read contract constrains the claim shape. If we build scrapers
first, we will paint ourselves into a serialization corner.

## Riskiest assumptions (in order)

1. ATProto can carry custom Lexicons for application-defined record collections with usable read latency from a vanilla PDS. (Tested in slice-01 step 3.)
2. Canonical CIDs over claims are stable across re-runs and across machines. (Tested in slice-01 step 2.)
3. The "claims not truth" framing actually lands with the user emotionally — i.e. they will publish opinions they currently self-censor. (Tested behaviorally in slice-01 step 1 + 3; refined post-launch.)
4. A federated read of another DID's claims is useful with attribution-only and no merge. (Tested in slice-03.)
5. Triangulation weighting produces non-obvious adherence scores that a senior engineer would trust. (Tested in slice-04.)
