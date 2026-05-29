# Technology Stack — openlore-appview-search (slice-05) — DELTA from slice-04

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Extends**: `docs/feature/openlore-scoring-graph/design/technology-stack.md`

**Slice-05 is the first slice since slice-02 to add new EXTERNAL production
dependencies — but deliberately MINIMAL ones.** The conservative, reuse-heavy
posture: reuse DuckDB for the index (ADR-025; no search engine), reuse `reqwest`
(rustls) for all HTTP (ingest + CLI→indexer + PLC resolution; no new transport
crate), reuse the PURE `claim-domain::verify` core (no second verification path).
The only genuinely new external dependencies are (a) an HTTP SERVER framework for
the indexer's query API, and (b) a small base58 crate for the PLC multibase decode
(if not derivable from existing deps). Everything else is workspace reuse.

This is the architecturally heaviest slice (the first network service, the first
cross-process boundary, the first adversarial-input external boundary) but the
dependency growth is held to the minimum the new ports genuinely require.

## Production crates — slice-05 surface (reuse)

| Crate (already in slice-01..04) | New use in slice-05 | License | Justification |
|---|---|---|---|
| `duckdb` (Rust crate) | The SEPARATE `index.duckdb` network-index store (ADR-025): `indexed_claims` + evidence/references; dimensional indexed lookups; NO new tables in `openlore.duckdb`. | MIT | Already used per ADR-001. Reusing DuckDB for the index (vs a search engine) reuses the proven `no_cross_table_join_elides_author` anti-merging enforcement substrate (the decisive WD-103 reason) + zero new heavy dependency. |
| `reqwest` (rustls) | ALL slice-05 HTTP: CLI→indexer query (ADR-027), the indexer's bounded PULL ingest (ADR-024), and PLC DID-document resolution (ADR-026). | MIT / Apache-2.0 | Already in-workspace (slice-02 `adapter-github`). No new transport crate — exactly the slice-02 reuse pattern. |
| `claim-domain` (workspace) | `verify` + `compute_cid` REUSED by the indexer's ingest gate (no second verification path, WD-104); EXTENDED with the pure `decode_ed25519_multibase` helper (ADR-026). | (workspace) | The cardinal verified-before-index reuse. |
| `serde` / `serde_json` | Deserialize fetched records → `RawRecord`/`SignedClaim` at ingest; serialize the `org.openlore.appview.searchClaims` query response (ADR-027). | MIT / Apache-2.0 | Already used. |
| `chrono` | `IndexedClaim.composed_at`/`indexed_at` (pure value types). | MIT / Apache-2.0 | Already used; pure dependency — permitted in `appview-domain`'s pure-core allowlist. |
| `tokio` | The indexer's async runtime (ingest loop + serving queries); the CLI's existing async (the index-query client). | MIT | Already used per ADR-004. NOW exercised by the indexer binary + the new async ports (`IndexQueryPort`/`IngestSourcePort`/`IdentityResolvePort`). |
| `async-trait` | The new async ports (`IndexQueryPort`, `IngestSourcePort`, `IdentityResolvePort`), following the existing `PdsPort` pattern. | MIT / Apache-2.0 | Already used (PdsPort). |
| `thiserror` | New error enums (`IndexQueryError`, `IngestError`, `IndexStoreError`, `ResolveError`, `DecodeError`). | MIT / Apache-2.0 | Already used. |
| `clap` | The new `openlore search` verb + flags (ADR-027); the `openlore-indexer serve`/`ingest` subcommands. | MIT / Apache-2.0 | Already used. |
| `tracing` | The new `health.startup.refused{reason}` indexer probe variants + the KPI events (`search.discovery.*`, `indexer.ingest.*`). | MIT | Already used per ADR-010. |

## NEW production crates (workspace members)

| Crate | Kind | External deps | License | Purpose |
|---|---|---|---|---|
| `crates/appview-domain` | PURE workspace member | NONE beyond `std` + workspace `chrono` + the pure value types from `ports`/`claim-domain` | (workspace; MIT/Apache-2.0) | The PURE ingest-gate decision + search/grouping/anti-merging composition. The symmetric counterpart to `scraper-domain`/`scoring`. No I/O; pure-core allowlist. |
| `crates/adapter-atproto-ingest` | EFFECT | workspace `reqwest` (+ `tokio`/`async-trait`/`serde`) | (workspace) | `IngestSourcePort` — bounded PULL of public records (ADR-024). Read-only; no new external crate. |
| `crates/adapter-index-store` | EFFECT | workspace `duckdb` | (workspace) | `IndexStorePort` over `index.duckdb` (ADR-025). No new external crate. |
| `crates/adapter-index-query` | EFFECT | workspace `reqwest` (+ `tokio`/`async-trait`) | (workspace) | `IndexQueryPort` — the CLI→indexer client (ADR-027); graceful degradation. No new external crate. |
| `crates/adapter-xrpc-query-server` | EFFECT | an HTTP SERVER framework (NEW — see below) | (TBD; MIT/Apache-2.0 required) | Serves `org.openlore.appview.searchClaims` over HTTP (ADR-027). |
| `crates/openlore-indexer` | DRIVER (binary) | workspace `tokio` + `clap` + the above | (workspace) | The SECOND composition root; signing-incapable; holds no local store (ADR-023). |

`appview-domain` adds NO new external dependency (pure Rust over existing types).
The four new effect crates reuse the workspace `reqwest`/`duckdb`/`tokio` — the
ONLY genuinely new external dependencies are the two below.

## NEW external dependencies (the minimal set)

| Crate | License | Maintenance | Purpose | Alternatives considered |
|---|---|---|---|---|
| **An HTTP server framework** (recommended: `axum`, GitHub `tokio-rs/axum`) | MIT | Active (tokio-rs org; frequent releases; large community; >15k stars) | Serve the indexer's `org.openlore.appview.searchClaims` XRPC query method over HTTP (ADR-027). The indexer is the first component to SERVE HTTP (the CLI only ever CLIENT-side `reqwest`). | `hyper` (lower-level; `axum` builds on it — more boilerplate for a query handler); `actix-web` (heavier, its own runtime); a hand-rolled `hyper` handler (viable for one query method; DELIVER may choose this to avoid a framework dep — see note). `axum` is MIT, tokio-ecosystem-native (matches ADR-004), and minimal for a single query route. |
| **A base58 / multibase decode crate** (recommended: `bs58`, MIT) — ONLY IF not derivable from existing deps | MIT | Active; small, stable, widely used in the crypto/IPFS ecosystem | The PURE `decode_ed25519_multibase` helper (ADR-026) needs base58btc decode for the `z6Mk...` multibase value. A PURE dependency (no I/O), whitelisted in the `claim-domain` pure-core allowlist like slice-03's `unicode-normalization`. | Hand-rolled base58btc (small + well-specified; DELIVER may inline it to avoid the dep — base58btc is ~40 lines); `multibase` crate (heavier, pulls more codecs than needed). The multicodec-prefix strip (`0xed01`) is trivial byte ops (no dep). |

**Note (DELIVER's call within the allowlist)**: both new deps have viable
hand-rolled alternatives (a single-route `hyper` handler; an inlined base58btc).
The DESIGN constraint is: any external crate MUST be MIT/Apache-2.0/BSD under the
existing `deny.toml` allowlist (I-11); the base58 helper MUST stay in the pure-core
allowlist (no I/O); the HTTP server MUST be tokio-ecosystem-compatible (ADR-004).
DELIVER picks the framework vs hand-rolled trade-off; the design fixes only the
contract (the XRPC query method + the response shape with per-result `author_did`).

## NO new STORE engine / NO search engine (the ADR-025 resolution)

The index-store decision (ADR-025) resolves to REUSE DuckDB, NOT a search engine.
Consequently:

- **No Tantivy / Meilisearch / Elasticsearch.** Considered as a "real search
  engine"; rejected because the walking-skeleton search is EXACT dimensional keyed
  lookup over structured fields (`object`/`author_did`/`subject`), not free-text
  relevance ranking — DuckDB indexed lookups handle it. The decisive rejection: a
  non-SQL engine would need a NEW structural-enforcement substrate for the cardinal
  anti-merging invariant (WD-103); DuckDB lets the proven
  `no_cross_table_join_elides_author` rule extend onto the same substrate. An
  external service (Meilisearch/Elasticsearch) would also re-introduce the
  "central service to operate" concern ADR-023 avoids. Full trade-off in ADR-025.
- **No DuckDB FTS extension (yet).** Documented revisit path if free-text claim-
  prose search becomes a J-005 JTBD (a built-in extension, no new heavy dep).
- **No graph store.** Cross-user network-scale traversal/scoring is DEFERRED
  (WD-79); the index ranks/traverses nothing in slice-05.

## NO Firehose dependency (the ADR-024 resolution)

The ingestion decision (ADR-024) resolves to PULL-based bounded ingestion, NOT
Firehose. Consequently:

- **No ATProto Firehose / `subscribeRepos` consumer.** Considered (the ADR-016
  re-evaluation); deferred. A firehose consumer would add reconnection/cursor/
  back-pressure complexity + push the indexer toward an always-on reconnecting
  daemon — orthogonal to the J-005 discovery hypothesis. Bounded PULL reuses
  `reqwest` `listRecords` reads (the slice-03 pattern). Documented additive revisit
  trigger.
- **No WebSocket / SSE crate.** Not needed for request/response PULL.

## Test-only / dev-dependency additions (slice-05)

| Crate | License | Purpose |
|---|---|---|
| (test-support extensions; no new external crate) | — | `FakeIngestSource` (bounded fixture records incl. adversarial: unsigned/tampered/CID-mismatch), `FakeIndexStore`, `FakeIndexQuery` (+ an "unreachable" mode for the degradation test), a real-`z6Mk...` DID-document fixture (a known test keypair, for the ADR-026 decode gold test). All DATA + doubles in `test-support`, not new crates. Mutation testing (existing nightly) extends to `crates/appview-domain` + the `claim-domain` decode helper. A test HTTP server for the CLI↔indexer contract test reuses the chosen server framework's test utilities. |

## License compliance

The new external dependencies MUST satisfy the slice-01 `deny.toml` allowlist
(`MIT OR Apache-2.0 OR BSD-3-Clause OR Unicode-DFS-2016`; I-11):

- `axum` (recommended HTTP server): MIT — PASS.
- `bs58` (recommended base58, if used): MIT — PASS.
- Any transitive dependency the HTTP server pulls (e.g., `tower`, `http`) must
  pass `cargo deny check licenses`; all tokio-ecosystem crates are MIT/Apache-2.0.
  DELIVER runs `cargo deny check` (I-11) as the gate before any dep lands.

No AGPL/GPL/proprietary dependency is introduced. The decisive rejections
(Elasticsearch/Meilisearch as external services; a non-OSS search engine) are also
license-and-sovereignty-motivated.

## Versioning policy

Per slice-01: pin MAJOR.MINOR in `Cargo.toml`; `Cargo.lock` resolves PATCH.
Slice-05 pins the new HTTP server framework + base58 crate at their current stable
MAJOR.MINOR; reuses the pinned `duckdb`/`reqwest`/`tokio` lines (no bump). The new
workspace crates are pinned at the workspace version.

## Supply chain (inherited)

- `cargo deny check advisories | bans | sources | licenses` runs in CI on every
  commit (I-11). Slice-05 adds the new HTTP-server + base58 deps to the review;
  both must pass before landing.
- Reproducible builds via committed `Cargo.lock`.
- No prebuilt binary dependencies. No external service dependency (the indexer is a
  self-hostable single binary; ADR-023 — no Elasticsearch/Meilisearch to provision).

## Rejected alternatives

| Alternative | Rejected because |
|---|---|
| A dedicated search engine (Tantivy embedded) for the index | ADR-025: the slice-05 search is exact dimensional keyed lookup, not free-text relevance ranking; DuckDB handles it; a non-SQL engine needs a NEW anti-merging-enforcement substrate (the cardinal WD-103 reason). Revisit if free-text claim-prose search becomes a JTBD (then the DuckDB FTS extension first). |
| An external search service (Meilisearch / Elasticsearch) | Hard reject (ADR-025): re-introduces a "central service to operate" — the exact concern ADR-023 avoids — plus a network dependency, an ops surface, and a license/sovereignty concern. |
| ATProto Firehose ingestion (`subscribeRepos` consumer) | Deferred (ADR-024; the ADR-016 re-evaluation): real-time but heavy (reconnection/cursor/back-pressure/daemon shape); pull suffices for the discovery thesis; documented additive revisit. |
| A second DuckDB file shared between the CLI + indexer | Rejected (ADR-023/025): the indexer must not touch the user's source-of-truth `openlore.duckdb` (WD-106) + the capability boundary; separate `index.duckdb`. |
| A new transport crate (gRPC / a custom binary protocol) for CLI→indexer | Rejected (ADR-027): re-invents what XRPC-over-HTTP + `reqwest` already provide; couples to deployment in the gRPC case. HTTP-to-a-configured-URL is deployment-independent (makes the ADR-023 hosted revisit additive). |
| A heavyweight web framework (`actix-web`) for the indexer query API | Rejected: heavier, its own runtime model; `axum` (or a hand-rolled `hyper` handler) is minimal for a single query route + tokio-ecosystem-native (ADR-004). |
| A second verification implementation in the indexer | Hard reject (WD-104 / ADR-026): the indexer calls the SAME pure `claim_domain::verify` (no second path that could drift). |
| Keeping the slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` test seam as the production path | Rejected (ADR-026): KPI-AV-3 cannot hold against real network data with a test seam; the real PLC `z6Mk...` decode is implemented, the seam is release-forbidden. |
| `multibase` (full multi-codec) crate for the pubkey decode | Heavier than needed; only base58btc + the `0xed01` Ed25519 prefix is required. `bs58` (or a hand-rolled base58btc) + trivial prefix-strip suffices; stays in the pure-core allowlist. |

## Summary

Slice-05's technology stack is the slice-04 stack PLUS: one PURE workspace crate
(`appview-domain`, zero new external dep), four EFFECT workspace crates (reusing
`reqwest`/`duckdb`/`tokio` — zero new external dep except the HTTP server), one
NEW binary (`openlore-indexer`), and exactly TWO minimal new external dependencies
(an HTTP server framework + a small base58 crate, both MIT, both with hand-rolled
fallbacks). It REUSES DuckDB for the index (no search engine — the cardinal
anti-merging-substrate reuse), REUSES `reqwest` for all HTTP, REUSES the pure
`claim-domain::verify` core (no second verification path), and PULLS rather than
subscribes (no Firehose dependency). The first-network-service slice is delivered
on the most conservative dependency surface the new ports allow.
