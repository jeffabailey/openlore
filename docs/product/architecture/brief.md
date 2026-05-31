# OpenLore Architecture Brief (cross-feature SSOT)

> Bootstrapped at finalize of `openlore-foundation` (slice-01 walking skeleton)
> on 2026-05-27. This brief is the cross-feature single source of truth going
> forward; subsequent features (slice-02..05 and beyond) extend it in place.
> Per-feature detail lives under `docs/feature/{feature-id}/design/` during
> active waves and migrates to `docs/evolution/` at finalize.

## Style

- **Architectural style**: Hexagonal (Ports + Adapters), Modular Monolith,
  single-binary Rust CLI. See ADR-009.
- **Paradigm**: Functional-leaning Rust — pure core + effect shell. See ADR-007.
- **Runtime**: Tokio (async). See ADR-004.
- **Local store**: DuckDB (single embedded file). See ADR-001; revisit in
  slice-04 when graph traversal becomes the dominant workload (locked per WD-8).
- **Federation contract**: ATProto Lexicon under `org.openlore.*` namespace;
  CIDv1 dag-cbor sha2-256 addressing. See ADR-005 + ADR-006.
- **Identity**: User's existing ATProto DID with a per-application derived
  Ed25519 key in the OS keychain (macOS Keychain / Linux Secret Service /
  WSL2 fallback file). See ADR-002.
- **Retraction**: Counter-claim that references the original CID; no
  hard-delete. See ADR-008.

## Component Inventory

Workspace layout — all crates live under `/Users/jeffbailey/Projects/foss/leading/openlore/`:

| Crate                        | Kind        | Purpose                                                                 | Shipped in   |
|------------------------------|-------------|-------------------------------------------------------------------------|--------------|
| `crates/claim-domain`        | pure core   | Canonicalization, CID computation, signing, reference rules, confidence | slice-01     |
| `crates/lexicon`             | pure        | `org.openlore.*` schema + validation                                    | slice-01     |
| `crates/ports`               | pure traits | `StoragePort`, `IdentityPort`, `PdsPort`, `ClockPort`, `ProbeOutcome` ADT | slice-01     |
| `crates/adapter-duckdb`      | effect      | Implements `StoragePort` over DuckDB single-file DB                     | slice-01     |
| `crates/adapter-atproto-did` | effect      | Implements `IdentityPort` over OS keychain + DID resolution             | slice-01     |
| `crates/adapter-atproto-pds` | effect      | Implements `PdsPort` over ATProto XRPC                                  | slice-01     |
| `crates/adapter-system-clock`| effect      | Implements `ClockPort` over `std::time`                                 | slice-01     |
| `crates/cli`                 | driver      | clap-based composition root; threads adapters into pure core            | slice-01     |
| `crates/scraper-domain`      | pure core   | Derives candidate claims from harvested GitHub signals via the `jobs.yaml` signal->predicate SSOT mapping; no I/O | slice-02     |
| `crates/adapter-github`      | effect      | Implements `GithubPort` over the GitHub public REST/HTTPS API; optional PAT; public-data-only probe | slice-02     |
| `crates/scoring`             | pure core   | Transparent no-ML adherence-weight + `WeightBucket` over per-author `Contribution`s; the WD-77 formula SSOT (`ScoringConfig::DEFAULT`); aggregation in Rust, never SQL (anti-merging in aggregates) | slice-04     |
| `crates/appview-domain`      | pure core   | PURE indexer logic: `ingest_decision` (verify-before-index gate), `compose_results` (anti-merging-at-network-scale composition), `annotate_counter_relationship` (shown-not-applied), `near_match_suggestion` (edit-distance ranker); no I/O | slice-05     |
| `crates/adapter-atproto-ingest` | effect   | Implements `IngestSourcePort` over bounded public ATProto `listRecords` PULL (read-only; no write surface)               | slice-05     |
| `crates/adapter-index-store` | effect      | Implements `IndexStorePort` over a SEPARATE `index.duckdb`; non-`Option` `author_did` rows; `verified_against NOT NULL`; no merged/consensus schema | slice-05     |
| `crates/adapter-xrpc-query-server` | effect | `hyper` HTTP server of the `org.openlore.appview.searchClaims` XRPC query method (per-result `author_did` always present) | slice-05     |
| `crates/adapter-index-query` | effect      | CLI-side `IndexQueryPort` XRPC client (bounded timeouts); treats indexer-unreachable as a SOFT non-fatal outcome (graceful degradation) | slice-05     |
| `crates/openlore-indexer`    | driver (binary) | The SECOND composition root (`serve`/`ingest`/`stats`); self-hostable network service; signing-incapable; holds no local store | slice-05     |
| `crates/test-support`        | test-only   | `FakePds`, `FakeKeychain`, `FakeClock`, `TempXdg`, `FakeGithub`, scoring fixtures, `FakeIngestSource`/`FakeIndexStore`/`FakeIndexQuery` + real-`z6Mk` DID-doc + adversarial ingest fixtures — hermetic test doubles | slice-01/02/04/05 |
| `xtask`                      | dev tooling | `check-arch` (hexagonal invariants), `check-probes` (probe contracts)   | slice-01     |

**Slice-01 ships 8 production crates + 1 test-support crate + 1 xtask binary.
Slice-02 adds 2 production crates (`scraper-domain` + `adapter-github`); slice-04
adds 1 (`scoring`); slice-05 adds 6 (1 pure `appview-domain` + 4 effect adapters +
1 binary `openlore-indexer` — the indexer subsystem / first network service);
slice-06 adds 2 (1 pure `viewer-domain` + 1 effect `adapter-http-viewer` — the
read-only localhost htmx viewer), bringing the production count to 19 + 1
test-support + 1 xtask binary (21 workspace members total; `cargo xtask check-arch`
reports 21).**

Shipped slice extensions:

- **slice-06 (htmx-scraper-viewer): SHIPPED 2026-05-31 — TWO-CRATE ADDITIVE
  EXTENSION (the read-only localhost htmx store viewer).** Makes the node
  operator's node LEGIBLE (J-001): a new `openlore ui [--port <P>]` verb (default
  8788, binds **127.0.0.1 only, no auth**) serves server-rendered HTML (htmx-ready,
  progressive enhancement) over the operator's local DuckDB store — read-only,
  zero SQL. Routes: `GET /` (read-only landing), `GET /claims` (paginated, size
  50), `GET /claims/{cid}` (detail + evidence), `GET /peer-claims` (federated,
  origin = `author_did` + `fetched_from_pds`), `GET/POST /scrape` (live ephemeral
  GitHub propose reusing the slice-02 `GithubPort` + `derive_candidates`; sign
  stays in the CLI). Adds 2 crates + extends slice-01/02/03 crates in place;
  **zero new persisted types, zero new table, zero new CID path**.
  - **NEW `crates/viewer-domain` (PURE)**: the viewer's pure core — `maud` render
    (`view-model → HTML string`, no I/O) + view-model ADTs + pure pagination
    arithmetic (offset/limit, clamp). Deps `maud` + `ports` only (`check-arch`
    pure-core allowlist, ADR-029).
  - **NEW `crates/adapter-http-viewer` (EFFECT)**: the read-only HTTP listener — a
    hand-rolled `hyper` 1.x handler (`axum`/`actix` are `deny.toml`-banned, DV-3);
    binds loopback-only; holds no signing key.
  - `crates/ports`: adds `StoreReadPort` (NO mutation method) + the `ClaimRow` /
    `ClaimDetail` / `PeerClaimRow` / `PageRequest` / `Page` / `StoreReadError`
    ADTs (ADR-030).
  - `crates/adapter-duckdb`: adds the read-only `StoreReadPort` impl over the SAME
    shared `Arc<Mutex<Connection>>` (no new table, no store swap; ADR-030 / WD-8).
  - `crates/cli`: the NEW `openlore ui` verb wired as a read-only composition root
    routed BEFORE `Wiring::production` (no signing identity in the web root).
  - `xtask`: the `maud` pure-core allowlist entry + the viewer capability rule
    (the web process may not link signing / mutation; exclusion set independently
    unit-pinned) + the pure-core arm.
  - **Invariants I-VIEW-1..6** (read-only [3-layer TYPE/STRUCTURAL/BEHAVIORAL];
    no key in the web process; human gate preserved; derived-from honesty; same-
    store / zero-new-schema; offline store views + loopback-only bind) —
    slice-06-scoped.
  - **ADRs ADR-028..030** (viewer architecture: `ui` verb + pure/effect split +
    read-only + loopback/no-auth; `maud` templating + pure-core allowlist;
    read-only DuckDB store-read port + column mapping + offset/limit pagination
    size 50).
  - See ADR-028..ADR-030, `docs/evolution/htmx-scraper-viewer-evolution.md`, and
    `docs/feature/htmx-scraper-viewer/design/`.

- **slice-05 (openlore-appview-search): SHIPPED 2026-05-29 — SIX-CRATE ADDITIVE
  EXTENSION (the indexer subsystem; the FIRST network service + the SECOND shipped
  binary).** The architecturally headline + FINAL umbrella slice (J-005 network
  discoverability). It introduces the network INDEXER (the ATProto AppView
  pattern): a self-hostable `openlore-indexer` binary that ingests PUBLIC signed
  claims from across the network, verifies each signature + recomputes each CID
  BEFORE indexing, and serves network-scale discovery — WITHOUT the AppView ever
  becoming an authority over the CLI-first, local-first source of truth. Adds 6
  crates (1 pure + 4 effect + 1 binary) + extends slice-01..04 crates in place. Per
  WD-13 (federation -> scrapers -> scoring -> appview) slice-05 is the last in the
  sequence.
  - **NEW `crates/appview-domain` (PURE)**: the indexer's pure core (the symmetric
    counterpart to `scraper-domain` + `scoring`). `ingest_decision(record,
    resolved_key) -> IngestOutcome` is the verify-before-index gate — it reuses
    `claim_domain::verify` + `compute_cid` (NO second verification path, WD-104);
    `compose_results(rows, dimension) -> NetworkSearchResult` groups per-author and
    NEVER merges (anti-merging at network scale, WD-103);
    `annotate_counter_relationship` adds a counter annotation without filtering
    (OD-AV-7); `near_match_suggestion` is the empty-result edit-distance ranker. No
    I/O (`check-arch` pure-core allowlist).
  - **NEW `crates/adapter-atproto-ingest` (EFFECT)**: implements `IngestSourcePort`
    — bounded PULL of public `org.openlore.claim` records via ATProto `listRecords`
    (ADR-024; Firehose deferred). Read-only by construction; reuses the workspace
    `reqwest` (no new transport crate).
  - **NEW `crates/adapter-index-store` (EFFECT)**: implements `IndexStorePort` over
    a SEPARATE `index.duckdb` (ADR-025) — a 2nd DuckDB store, NOT a graph-DB swap
    (re-affirming ADR-001/WD-8); non-`Option` `author_did` rows; `verified_against
    NOT NULL`; anti-merging-preserving per-author queries (no `GROUP BY author`); NO
    merged/consensus schema (the load-bearing absence).
  - **NEW `crates/adapter-xrpc-query-server` (EFFECT)**: serves
    `org.openlore.appview.searchClaims` over HTTP via a hand-rolled `hyper` handler
    (`axum` is `deny.toml`-banned, DV-3); every response row carries `author_did`
    (anti-merging across the transport).
  - **NEW `crates/adapter-index-query` (EFFECT, CLI side)**: implements
    `IndexQueryPort` as the CLI->indexer XRPC client with bounded timeouts; treats
    indexer-unreachable as a SOFT non-fatal `IndexQueryError::Unreachable` (graceful
    degradation, ADR-027 / KPI-5).
  - **NEW `crates/openlore-indexer` (DRIVER, BINARY)**: the SECOND composition root
    (ADR-023) — `serve` / `ingest` / `stats`. Wire -> probe -> use; signing-INCAPABLE
    + holds no local-store handle by construction. Disjoint from the CLI root (the
    CLI never wires the indexer's adapters; the indexer never wires the user's
    signing identity / `openlore.duckdb`).
  - `crates/ports`: adds `IndexQueryPort` (CLI) + `IngestSourcePort` /
    `IndexStorePort` / `IdentityResolvePort` (indexer) + the `IndexedClaim` /
    `RawRecord` / `SearchDimension` / `CounterRef` ADTs (non-`Option` `author_did`)
    + the `AuthorRelationship::NetworkUnfollowed` variant.
  - `crates/claim-domain`: adds the PURE `decode_ed25519_multibase` helper — the
    REAL `z6Mk...` PLC multibase decode (ADR-026, resolving the slice-03 DV-4 seam);
    `verify` / `compute_cid` UNCHANGED and reused by the indexer.
  - `crates/cli`: the NEW `openlore search` verb (`--object` / `--contributor` /
    `--subject` / `--show` / `--share` + the `openlore search <openlore://search?...>`
    link re-run resolver) + `render.rs` network renderer; the discovery->federation
    funnel reuses slice-03 `peer add` VERBATIM (render-only hint; no auto-follow).
  - `crates/lexicon`: adds the `org.openlore.appview.searchClaims` XRPC query DTOs
    (a READ query; no signed payload).
  - `crates/adapter-atproto-did`: adds the verify-only `IdentityResolvePort` impl +
    the release-gated pubkey seam (the slice-03 `OPENLORE_PEER_PUBKEY_HEX` env seam
    retained but `cfg(debug_assertions)`-gated; release-forbidden, ADR-026).
  - `xtask`: the anti-merging SQL rule extended to `adapter-index-store`; the new
    `indexer_holds_no_signing_or_local_store` + `no_pubkey_seam_in_release_build`
    rules; `appview-domain` added to the pure-core allowlist; I-3 (composition-root
    rule) broadened to BOTH binaries.
  - **Invariants I-AV-1..9** (verify-before-index; anti-merging at network scale
    [3-layer TYPE/STRUCTURAL/BEHAVIORAL]; local-first / disjoint composition roots;
    public-data-only; capability boundary [signing-incapable + no-local-store]; real
    z6Mk decode; discovery-funnel reuses `peer add` verbatim; share encodes
    query-not-snapshot; counter shown-not-applied) — slice-05-scoped (see below).
  - **ADRs ADR-023..027** (self-hostable single-binary indexer; bounded PULL with
    Firehose deferred; separate `index.duckdb` + anti-merging schema; production PLC
    z6Mk decode + release-forbidden seam; `search` verb + CLI<->indexer XRPC +
    graceful degradation).
  - See ADR-023..ADR-027, `docs/evolution/openlore-appview-search-evolution.md`,
    and `docs/feature/openlore-appview-search/design/`.

- **slice-02 (openlore-github-scraper): SHIPPED 2026-05-28 — TWO-CRATE ADDITIVE
  EXTENSION (WD-59; the first crate addition since slice-01).** Per WD-13 the
  umbrella sequence is federation -> scrapers -> scoring -> appview, so slice-02
  (scrapers) shipped AFTER slice-03 (federation) — recorded here as shipped
  alongside slice-03. Adds 2 production crates + extends slice-01 crates in place:
  - **NEW `crates/scraper-domain` (PURE)**: derives auditable candidate claims
    from harvested GitHub `Signal`s via the `jobs.yaml` J-004 signal->predicate
    SSOT mapping (embedded at build time via `include_str!` + a pure parse;
    `mapping_matches_ssot` drift gate, WD-67). Every candidate names >=1 source
    signal (I-SCR-4), carries the conservative 0.25 numeric confidence
    (never auto-inflated, WD-52/I-SCR-3), and derives deterministically.
    No I/O (`check-arch` pure-core allowlist, WD-65).
  - **NEW `crates/adapter-github` (EFFECT)**: implements `GithubPort` (a NEW
    port, WD-61/ADR-019 — GitHub shares no contract with ATProto) over the
    GitHub PUBLIC REST/HTTPS API using the workspace `reqwest`; reads the
    optional `GITHUB_TOKEN` PAT from env (WD-63); refuses private/non-existent
    targets; public-data-only `probe()` within the 250ms budget. Holds NO
    `StoragePort`/`IdentityPort`/`PdsPort` reference by construction (the
    human-gate at the architecture layer, I-SCR-1 — it CANNOT sign or publish).
  - `crates/ports`: adds the `GithubPort` trait + `TargetKind`
    (`Repo{owner,repo}` | `User{user}`) + `GithubError` + slice-02
    `ProbeRefusalReason` variants.
  - `crates/cli`: `scrape github <target> [--sign N[,N,...]]` verb +
    `CandidatePrefill` (the ONLY bridge from a candidate to a signed claim,
    reusing `VerbClaimAdd` + `VerbClaimPublish` internals — no parallel publish
    path, WD-66/I-SCR-6) + `SelectionParser`.
  - `crates/lexicon` + `crates/claim-domain`: UNCHANGED — `derived-from`
    provenance is DISPLAY-ONLY (WD-62/ADR-018), so the signed payload is
    byte-identical to a hand-authored claim and CID stability holds with zero
    new CID path (I-SCR-7).
  - `xtask`: `scraper-domain` added to the pure-core allowlist (its
    `serde_yaml_ng` dep whitelisted) + the GitHub public-only enforcement rule +
    the `impl GithubPort for <Adapter>` non-stub `probe()` rule.
  - See ADR-017..ADR-019, `docs/evolution/openlore-github-scraper-evolution.md`,
    and `docs/feature/openlore-github-scraper/design/`.

- **slice-04 (openlore-scoring-graph): SHIPPED 2026-05-28 — ONE-CRATE ADDITIVE
  EXTENSION (the pure `scoring` core) + read-side port/adapter/cli extensions.**
  Per WD-13 (federation -> scrapers -> scoring -> appview) slice-04 shipped after
  slice-02/03. It does NOT swap `adapter-duckdb` for a graph store (ADR-001 / WD-8
  re-evaluated and KEPT — DuckDB recursive CTEs serve the bounded depth-2 traversal;
  no graph DB warranted). Adds:
  - **NEW `crates/scoring` (PURE)**: `score(claims, cfg) -> WeightedView` — a
    transparent no-ML adherence weight (`subtotal = confidence x author_distinct_share
    + cross_project_triangulation`; `weight = Σsubtotals`) decomposed into per-author
    `Contribution`s; `weight_bucket` breadth guard renders thin evidence as `[SPARSE]`
    (WD-74/WD-90). Formula constants are the SSOT in `ScoringConfig::DEFAULT` (WD-77).
    Aggregation happens in Rust, NEVER SQL — the anti-merging-in-aggregates rule
    (ADR-022 / WD-73). No I/O (`check-arch` pure-core allowlist).
  - `crates/ports`: adds `graph.rs` — `GraphEdge` / `AttributedClaim` (non-`Option`
    `author_did` + `claim_cid`), `ScoringFilter`, `GraphNode`, `TraversalBound`
    (default depth 2), `TraversalResult`; extends `StoragePort` with
    `query_by_object`, `query_by_contributor`, `query_attributed_for_scoring`
    (the per-claim scoring feed), `traverse_graph`.
  - `crates/adapter-duckdb`: `graph_query.rs` — dimension/scoring-feed `UNION ALL`
    SQL (per-claim `author_did` projected, no aggregating JOIN) + a `WITH RECURSIVE`
    depth-bounded, visited-path cycle-safe traversal with `omitted_edge_count`.
  - `crates/cli`: 6 OPT-IN `graph query` explorer flags (`--object`, `--contributor`,
    `--traverse`, `--depth`, `--weighted`, `--explain`) + grouped/trail/tree/weighted/
    explain renderers; a bare `--subject` query stays byte-identical to slice-01/03
    (WD-87). Weights are DISPLAY-ONLY, recomputed at query time, never persisted (WD-72).
  - `xtask`: anti-merging rule extended to AGGREGATES (scoring-feed `UNION ALL` +
    recursive-CTE base must project `author_did`).
  - See ADR-020..ADR-022, `docs/evolution/openlore-scoring-graph-evolution.md`,
    and `docs/feature/openlore-scoring-graph/design/`.

- **slice-03 (openlore-federated-read): SHIPPED 2026-05-28 — EXTENSION ONLY,
  ZERO new crates (WD-26).** Extends the slice-01 crates in place:
  - `crates/ports`: adds `PeerStoragePort` (new port, WD-27); extends `PdsPort`
    with peer-read methods (`list_peer_records`, `get_peer_record`, WD-28),
    `IdentityPort` with `resolve_peer` (WD-29), and `StoragePort` with
    `query_federated_by_subject`; adds `FederatedRow` (non-`Option`
    `author_did`), `PeerInfo`, `PeerSubscription`, and the peer-storage outcome/
    error ADTs.
  - `crates/adapter-duckdb`: adds `DuckDbPeerStorageAdapter` implementing
    `PeerStoragePort` (sharing the slice-01 connection pool) + migration v3 with
    **4 new DuckDB tables** (`peer_subscriptions`, `peer_claims`,
    `peer_claim_references`, `peer_claim_evidence`) plus a per-peer-DID
    filesystem partition for auditable hard-purge (WD-31, ADR-014).
  - `crates/adapter-atproto-did` / `adapter-atproto-pds`: peer DID resolution +
    peer XRPC reads (ADR-016).
  - `crates/lexicon` + `crates/claim-domain`: optional top-level `reason` field
    on `org.openlore.claim` (CID-stable when absent, WD-32, ADR-015) +
    `normalize_reason` (NFC) + `validate_counter_claim` pure functions (WD-34/35).
  - `crates/cli`: `peer add | pull | remove`, `claim counter`,
    `graph query --federated` + `OrientationState` habit affordances.
  - `xtask`: `no_cross_table_join_elides_author` anti-merging SQL rule +
    `no_autoconfirm_in_release_build` guard.
  - See ADR-013..ADR-016, `docs/evolution/openlore-federated-read-evolution.md`,
    and `docs/feature/openlore-federated-read/design/`.

Future slices extend this inventory (planned / in-progress):

- The four-slice umbrella (federation -> scrapers -> scoring -> appview, WD-13) is
  COMPLETE as of slice-05. Documented additive future options (NOT yet built):
  ATProto Firehose / real-time ingest (deferred, ADR-024 revisit trigger), a
  hosted/community indexer (deferred, ADR-023 — the CLI talks to a configured URL),
  cross-user / network-scale SCORING (deferred, WD-79), a retraction-aware search
  FILTER (deferred, OD-AV-7 / I-AV-9), and a full presentational web AppView
  (locked OUT, OD-AV-6 — the `--share` resolver is CLI re-run only).

The slice-04 "deferred to a later slice" item — real PLC DID-document multibase
pubkey decode (the slice-03 DV-4 test-only peer-pubkey seam) — is RESOLVED by
slice-05: `claim_domain::decode_ed25519_multibase` ships the real `z6Mk...` decode
(ADR-026) and the seam is release-forbidden (`no_pubkey_seam_in_release_build`).

**Crate count: slice-01 = 8 production + 1 test-support + 1 xtask. slice-02 added
the first 2 production crates since slice-01 (`scraper-domain` + `adapter-github`,
WD-59); slice-03 was EXTENSION ONLY (zero new crates, WD-26); slice-04 adds 1 pure
crate (`scoring`); slice-05 adds 6 (1 pure `appview-domain` + 4 effect adapters + 1
binary `openlore-indexer`, the indexer subsystem); slice-06 adds 2 (1 pure
`viewer-domain` + 1 effect `adapter-http-viewer`, the read-only localhost htmx
viewer). Cumulative: 19 production + 1 test-support + 1 xtask = 21 workspace
members.**

## CLI surface (cumulative)

| Verb | Shipped in | Spec'd by |
|---|---|---|
| `openlore init` | slice-01 | ADR-003 |
| `openlore claim add` | slice-01 | ADR-003 |
| `openlore claim publish` | slice-01 | ADR-003 |
| `openlore claim retract` | slice-01 | ADR-003 + ADR-008 |
| `openlore graph query` | slice-01 | ADR-003 |
| **`openlore scrape github <target> [--sign N[,N,...]]`** | slice-02 | **ADR-017** |
| **`openlore peer add`** | slice-03 | **ADR-013** |
| **`openlore peer pull`** | slice-03 | **ADR-013 + ADR-016** |
| **`openlore peer remove`** (`[--purge]`) | slice-03 | **ADR-013 + ADR-014** |
| **`openlore claim counter`** | slice-03 | **ADR-013 + ADR-015** |
| **`openlore graph query --federated`** (flag, not verb) | slice-03 | **ADR-013 + ADR-014** |
| **`openlore graph query --object\|--contributor\|--traverse\|--depth\|--weighted\|--explain`** (explorer flags, not verbs) | slice-04 | **ADR-020** |
| **`openlore search --object\|--contributor\|--subject\|--show <cid>\|--share`** (NEW network verb; `graph query` stays unambiguously LOCAL) | slice-05 | **ADR-027** |
| **`openlore search <openlore://search?...>`** (link re-run resolver — re-runs the shared query, current results not a snapshot) | slice-05 | **ADR-027** |
| **`openlore-indexer serve\|ingest\|stats`** (the SECOND binary; the self-hostable network service; signing-incapable) | slice-05 | **ADR-023 + ADR-024 + ADR-027** |

## C4 reference

The authoritative C4 diagrams (Level 1 System Context, Level 2 Containers,
Level 3 Components for `claim-domain`) live in the slice-01 architecture
design:

- **`docs/feature/openlore-foundation/design/architecture-design.md`**

These diagrams are versioned with the feature workspace; when slices 02-05
land, each will produce its own architecture-design.md and this brief will
point at the merged successor.

## Cross-feature invariants (enforced)

These invariants hold across every feature in this repo. Each is enforced
mechanically by a tool listed in the **Enforced by** column. Adding a feature
that violates one of these without a documented exception in an ADR is a
build-fail.

| # | Invariant                                                              | Enforced by                                    |
|---|------------------------------------------------------------------------|------------------------------------------------|
| I-1 | Pure-core crates (`claim-domain`, `lexicon`, `ports`) MUST NOT depend on adapter crates | `cargo xtask check-arch`                       |
| I-2 | Pure-core crates MUST NOT depend on `tokio`, `reqwest`, `duckdb`, `keyring`, or any other I/O crate | `cargo xtask check-arch`                       |
| I-3 | The `cli` crate is the only composition root permitted to wire adapters into ports | `cargo xtask check-arch`                       |
| I-4 | Every adapter MUST implement a `probe() -> ProbeOutcome` for startup health-check | `cargo xtask check-probes`                     |
| I-5 | Every adapter `probe()` MUST run with a 250ms timeout budget and degrade gracefully on timeout | `cargo xtask check-probes`                     |
| I-6 | The signed-claim payload MUST contain only the locked numeric `confidence` (`[0.0, 1.0]`); display buckets MUST NEVER be serialized | `tests/lexicon_conformance.rs` (DISTILL gate)  |
| I-7 | The compose preview MUST contain the literal text "not as truth"        | `tests/walking_skeleton.rs::WS-1`              |
| I-8 | The publish success message MUST mention the retract command            | `tests/walking_skeleton.rs::WS-8`              |
| I-9 | Compose and sign MUST succeed with network disabled (KPI-5)             | `tests/walking_skeleton.rs::WS-10`             |
| I-10 | Graph query output MUST match compose-preview field-for-field (KPI-4)   | `tests/walking_skeleton.rs::WS-12` + `tests/federation_roundtrip.rs` |
| I-11 | Workspace dependencies MUST pass cargo-deny advisories, bans, sources, and licenses | `cargo deny check` (CI gate)                   |
| I-12 | Every git commit on a roadmap step MUST carry a `Step-ID: NN-NN` trailer matching the roadmap | `des-verify-integrity` (Phase 6 gate)          |

**Slice-03 invariants (I-FED-1..7) are slice-03-scoped**, NOT promoted to the
cross-feature I-1..I-12 set (mirroring how slice-01 kept its feature-scoped
invariants in its own workspace). They cover the anti-merging guarantee
(I-FED-1, enforced at three layers per WD-30), the single-publish-path reuse
(I-FED-5), and CID stability of the optional `reason` field (I-FED-6/7). Detail
lives in `docs/feature/openlore-federated-read/design/` + ADR-014/ADR-015.

**Slice-02 invariants (I-SCR-1..7) are likewise slice-02-scoped**, NOT promoted
to I-1..I-12 (same handling as slice-03's I-FED-*). They cover the human-gate
(I-SCR-1: `adapter-github` holds no storage/identity/pds reference and
`CandidatePrefill` is the only bridge), public-data-only (I-SCR-2), confidence
0.25 never auto-inflated (I-SCR-3), candidate auditability / names-its-signal
(I-SCR-4), mapping SSOT no-drift (I-SCR-5), single-publish-path reuse (I-SCR-6),
and display-only-provenance CID stability (I-SCR-7). Detail lives in
`docs/feature/openlore-github-scraper/design/` + ADR-017/ADR-018/ADR-019.

**Slice-04 invariants (I-GRAPH-1..8) are likewise slice-04-scoped**, NOT promoted
to I-1..I-12. They cover anti-merging IN AGGREGATES (I-GRAPH-1/2: a weight is an
aggregate view that decomposes to per-author `Contribution`s — enforced at three
layers: non-`Option` `author_did`/`claim_cid` types + the `xtask check-arch`
scoring-feed/recursive-CTE SQL rule + behavioral GQE-13/27), scoring transparency
(I-GRAPH-3: `weight == formula`, reproducible via `--explain`), display-only/
never-persisted weights (I-GRAPH-4, WD-72), sparse-renders-sparse epistemic honesty
(I-GRAPH-5/6, WD-74), numeric-confidence pass-through with no bucket rounding
(Gate 6), and local-first/no-network scoring + traversal (I-GRAPH-7). Detail lives
in `docs/feature/openlore-scoring-graph/design/` + ADR-020/ADR-021/ADR-022.

**Slice-05 invariants (I-AV-1..9) are likewise slice-05-scoped**, NOT promoted to
I-1..I-12 (same handling as I-FED-*/I-SCR-*/I-GRAPH-*). They cover verified-before-
index (I-AV-1: signature-verified against the REAL PLC key + CID-recomputed via the
pure core BEFORE any record enters the index; no second verification path; every
result `[verified]`; `verified_against NOT NULL`), anti-merging AT NETWORK SCALE
(I-AV-2: non-`Option` author DID + no merged consensus schema/row anywhere; enforced
at three layers — type + the `xtask check-arch` `no_cross_table_join_elides_author`
rule extended to `adapter-index-store` SQL + behavioral
`network_result_preserves_attribution`; the direct descendant of I-FED-1 / I-GRAPH-2),
local-first preserved (I-AV-3: the CLI links no indexer code; `search` is the only
network verb + degrades gracefully; disjoint composition roots; KPI-5), public-data-
only (I-AV-4), the indexer capability boundary (I-AV-5: signing-incapable + holds no
local store, mirroring slice-02 I-SCR-1, three-layer), real production pubkey decode
(I-AV-6: the test seam release-forbidden), the discovery->federation funnel reusing
`peer add` verbatim (I-AV-7, reuses I-FED-5), share-encodes-query-not-snapshot
(I-AV-8), and counter-shown-not-applied (I-AV-9). Detail lives in
`docs/feature/openlore-appview-search/design/` + ADR-023/ADR-024/ADR-025/ADR-026/ADR-027.

If a future slice needs one of these (I-FED-*, I-SCR-*, I-GRAPH-*, or I-AV-*)
enforced cross-feature, promote it to the table above in the same commit as the ADR
that generalizes it.

## Production dependencies (notable additions)

- `unicode-normalization` (slice-03): pure dependency in `crates/claim-domain`
  for NFC normalization of the counter-claim `reason` field (WD-35, ADR-015).
  Required for CID determinism; covered by the existing `deny.toml` MIT/Apache-2.0
  allowlist. Stays within the pure-core allowlist in `xtask check-arch`.
- `serde_yaml_ng` (slice-02): pure dependency in `crates/scraper-domain` for
  parsing the embedded `jobs.yaml` signal->predicate mapping snapshot (DV-5,
  WD-67). A maintained drop-in fork of the archived `serde_yaml`; license-clean
  (MIT/Apache-2.0) under the existing `deny.toml` allowlist; whitelisted in the
  `xtask check-arch` pure-core allowlist (WD-65). `adapter-github` (slice-02)
  adds NO new transport crate — it reuses the workspace `reqwest` (rustls).
- `hyper` (slice-05): effect dependency in `crates/adapter-xrpc-query-server` for
  serving the single `org.openlore.appview.searchClaims` XRPC route. `axum` was
  considered and REJECTED (`deny.toml`-banned, DV-3); a hand-rolled `hyper` handler
  serves one route with no banned dependency. License-clean (MIT) under the existing
  allowlist. `adapter-atproto-ingest` + `adapter-index-query` (slice-05) add NO new
  transport crate — they reuse the workspace `reqwest` (rustls), like `adapter-github`.
- base58 multibase decode (slice-05): the PURE `decode_ed25519_multibase` helper in
  `crates/claim-domain` (ADR-026). base58btc is a small pure decode (a `bs58`-style
  dependency or hand-rolled inline, Q-DELIVER-AV-8); license-clean (MIT/Apache-2.0)
  and within the pure-core allowlist (no I/O), like slice-02/03's pure deps.

## SSOT discipline

- This brief is **cross-feature**. Add a row to **Component Inventory** when a
  feature ships a new crate; never inline per-feature design here.
- Per-feature architecture design (C4 diagrams, ADR proposals,
  component-boundaries.md, data-models.md) belongs in
  `docs/feature/{feature-id}/design/` during active waves, then migrates to
  `docs/evolution/` at finalize.
- ADRs live flat in `docs/adrs/` (cross-feature namespace, monotonically
  numbered).
- When an invariant in the table above gets weakened, raise an ADR and update
  this brief in the same commit.

## Pointers

- ADRs: `docs/adrs/ADR-001-*.md` through `docs/adrs/ADR-027-*.md`
  (ADR-013..016 accepted with openlore-federated-read; ADR-017..019 accepted/
  shipped with openlore-github-scraper, both shipped 2026-05-28; ADR-020..022
  accepted/shipped with openlore-scoring-graph 2026-05-28; ADR-023..027 accepted/
  shipped with openlore-appview-search 2026-05-29)
- Slice-01 evolution: `docs/evolution/openlore-foundation-evolution.md`
- Slice-02 evolution: `docs/evolution/openlore-github-scraper-evolution.md`
- Slice-03 evolution: `docs/evolution/openlore-federated-read-evolution.md`
- Slice-04 evolution: `docs/evolution/openlore-scoring-graph-evolution.md`
- Slice-05 evolution: `docs/evolution/openlore-appview-search-evolution.md`
- Slice-01 architecture design: `docs/feature/openlore-foundation/design/architecture-design.md`
- Slice-02 architecture design:
  `docs/feature/openlore-github-scraper/design/architecture-design.md`
- Slice-03 architecture design:
  `docs/feature/openlore-federated-read/design/architecture-design.md`
- Slice-04 architecture design:
  `docs/feature/openlore-scoring-graph/design/architecture-design.md`
- Slice-05 architecture design:
  `docs/feature/openlore-appview-search/design/architecture-design.md`
- KPI contracts: `docs/product/kpi-contracts.yaml`
- Jobs (JTBD): `docs/product/jobs.yaml`
- CI policy: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- Supply-chain policy: `deny.toml`
