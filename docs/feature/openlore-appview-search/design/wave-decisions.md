# Wave Decisions — DESIGN — openlore-appview-search (slice-05)

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Inherits from**: DISCUSS WD-100..WD-110 + OD-AV-1..7 (feature-delta.md); WD-1..WD-93 + ADR-001..022 (slices 01/02/03/04)
- **Format**: WD-XX entries; one decision per row; rationale + status + locks downstream

## DESIGN-wave decisions

| # | Decision | Rationale | Status | Locks |
|---|---|---|---|---|
| WD-111 | Slice-05 is an ADDITIVE EXTENSION introducing the FIRST network service (a second binary), not a re-architecture. Same hexagonal modular monolith + functional paradigm; now TWO single-purpose binaries (`openlore` CLI + `openlore-indexer`) in one workspace. | Conservative scope; slice-05 validates the network-discovery thesis ON TOP of the proven local-first/federated surface. No new architectural STYLE; the new crates map 1:1 to the new ports the first network service genuinely requires. | LOCKED | DELIVER extends existing crates + adds the indexer subsystem (1 pure + 4 effect + 1 binary); introducing any further crate/store/style requires returning to DESIGN. |
| WD-112 | **OD-AV-1 RESOLVED: the AppView indexer is a SELF-HOSTABLE single binary (`openlore-indexer`), signing-INCAPABLE by construction.** A hosted/community service is a documented future option, not slice-05's call. | Preserves data sovereignty (the P-001 non-negotiable) + the single-binary Rust ethos; matches the brief's "adds an indexer service (separate binary)". A hosted service is a central authority — the trust/centralization concern the product exists to avoid — and harder to walk back; a hosted deployment is purely additive later (the CLI talks to a configured URL, WD-115). Signing-incapable mirrors the slice-02 `adapter-github` human-gate (I-SCR-1). | LOCKED. **RESOLVES OD-AV-1.** Per ADR-023. | DELIVER ships `openlore-indexer` as a second composition root (wire→probe→use); the indexer holds no signing identity + no local-store handle; `xtask check-arch` enforces the capability boundary (I-AV-5). |
| WD-113 | **OD-AV-5 RESOLVED: a NEW top-level `openlore search` verb** (not a `--network` flag on `graph query`), with `--object`/`--contributor`/`--subject`/`--show <cid>`/`--share`. | A distinct verb makes the LOCAL-vs-NETWORK corpus boundary unambiguous (WD-109) and preserves the load-bearing "`graph query` is always local/offline" mental model — a `--network` flag would make `graph query` sometimes cross the wire. Mirrors slice-03 adding `peer` verbs for a new concern; the dimensions mirror slice-04 for habit-continuity. | LOCKED. **RESOLVES OD-AV-5.** Per ADR-027. | DELIVER implements the `search` verb on the `openlore` CLI; `graph query` stays unambiguously local; the dimensions reuse the slice-04 grammar. |
| WD-114 | **OD-AV-4 RESOLVED (the ADR-016 Firehose re-evaluation): PULL-based bounded ingestion.** ATProto Firehose is a documented FUTURE option, NOT slice-05's mechanism. | Bounded PULL suffices to validate the J-005 discovery thesis (KPI-AV-1 is a per-session discovery-rate metric, not a freshness metric); it is far simpler than Firehose (no reconnection/cursor/back-pressure/daemon), hermetically testable (essential for the cardinal `indexer_rejects_unverified_claim` gate), and reuses the slice-03 verification discipline verbatim. Firehose's real-time advantage is not on the slice-05 critical path; it is purely additive later. | LOCKED. **RESOLVES OD-AV-4; RE-AFFIRMS + revisits ADR-016.** Per ADR-024. | DELIVER implements bounded pull-ingest (seed DIDs + optional relay) with per-record/per-source fault isolation (reuses ADR-016); NO firehose consumer; documented revisit trigger. |
| WD-115 | **OD-AV-2 RESOLVED: CLI→indexer transport is HTTP carrying an XRPC-style query method (`org.openlore.appview.searchClaims`); the CLI queries a CONFIGURED URL** (default `http://127.0.0.1:<port>` for the self-hosted case). | Deployment-independent (the CLI talks to a URL; localhost or remote is config — makes the WD-112 hosted revisit purely additive); ATProto-stack consistent (XRPC idiom); reuses workspace `reqwest`/rustls (no new transport crate, like slice-02). An in-process/IPC transport would couple to deployment. | LOCKED. **RESOLVES OD-AV-2.** Per ADR-027. | DELIVER implements `adapter-index-query` (CLI client) + `adapter-xrpc-query-server` (indexer server); the response shape carries per-result `author_did` (anti-merging across the transport). |
| WD-116 | **OD-AV-3 RESOLVED: graceful local-only degradation.** An unreachable indexer is a SOFT non-fatal `IndexQueryError::Unreachable`; `search` prints a clear local-only message + points to `graph query` + exits non-fatally; the indexer is NOT probed at CLI startup (a per-`search`-soft check, not a global hard-fail). | KPI-5 cardinal guardrail: `search` must never block the local-first flows. The CLI links no indexer code; `search` is the only network verb; probing the indexer at every CLI startup would block `claim add` — so the reachability check is per-verb-soft (the ADR-016 per-peer-soft / global-hard distinction). | LOCKED. **RESOLVES OD-AV-3.** Per ADR-027. | DELIVER ships the soft `Unreachable` outcome + the local-only message; `local_first_preserved` release gate (KPI-5); the indexer probe is skipped-or-soft at CLI startup. |
| WD-117 | **The index store is a SEPARATE DuckDB file (`index.duckdb`), reusing DuckDB (NOT a search engine).** | The walking-skeleton search is EXACT dimensional keyed lookup, not free-text relevance ranking — DuckDB indexed lookups handle it. The DECISIVE reason: reusing DuckDB reuses the proven `no_cross_table_join_elides_author` anti-merging enforcement substrate for the cardinal WD-103 guarantee; a non-SQL search engine would need a NEW enforcement substrate (+ a new dependency, + an external service in the worst case, re-introducing the central-service concern ADR-023 avoids). A SEPARATE file keeps the indexer from touching the user's source-of-truth store (WD-106). | LOCKED. **RESOLVES the index-store choice.** Per ADR-025. | DELIVER ships `index.duckdb` with the `indexed_claims` schema (non-Option author_did; NO merged schema); the DuckDB FTS extension is the documented revisit path for free-text search. |
| WD-118 | **OD-AV-6 (pubkey) RESOLVED: implement the production PLC `z6Mk...` multibase pubkey decode NOW** (resolving the slice-03 DV-4 test-only seam). The PURE multibase decode helper lives in `claim-domain`; the EFFECT DID-document resolution lives in the verify-only identity adapter. The test seam is RETAINED but release-forbidden. | KPI-AV-3 CANNOT hold against REAL network data with a test seam (arbitrary network authors have no seam populated). The decode has been deferred since slice-03; the brief mandates resolving it. The pure-core/effect-shell split keeps the decode pure (testable) + the resolution at the edge. Reuses `claim_domain::verify` (no second verification path, WD-104). | LOCKED. **RESOLVES OD-AV-6 (the pubkey-decode dependency).** Per ADR-026. | DELIVER implements `decode_ed25519_multibase` (pure) + `IdentityResolvePort::resolve_verification_key` (effect, PLC resolution); `xtask check-arch` `no_pubkey_seam_in_release_build` (I-AV-6); the gold test runs the REAL decode. |
| WD-119 | **OD-AV-7 RESOLVED: countered/soft-retracted public verified claims appear NORMALLY in network search; the counter relationship is SHOWN when known, never silently applied as a filter or down-weight.** A retraction-aware search FILTER is deferred. | Mirrors slice-03 coexist semantics (countered claims stay visible) + slice-04 WD-85 (counter shown in --explain, not applied). Silently filtering would hide provenance the user must judge; the `indexed_claim_references` table supports the annotation. A filter is a richer concern (a future WD + ADR). | LOCKED. **RESOLVES OD-AV-7 at default.** | DELIVER indexes countered claims like any verified claim; `appview_domain` annotates the counter relationship (`countered_claim_still_appears` test); no filter/down-weight code path. |
| WD-120 | **Anti-merging at network scale (I-AV-2) is enforced at THREE semantically orthogonal layers** (type / structural / behavioral), extending slice-03 WD-30/ADR-014 + slice-04 WD-88/ADR-022. Aggregation (counts/groupings) happens in the PURE `appview-domain` core (Rust), NEVER in SQL. The index has NO merged/consensus schema. | The cardinal trust guarantee (WD-103; KPI-AV-2 unshippable on any violation) carried into its hardest failure surface — a network-scale aggregating index. A single-layer bypass is caught by the other two. Computing counts in Rust (not SQL GROUP BY across authors) keeps the aggregate decomposable; the load-bearing absence (no merged schema) makes a consensus row un-writable. | LOCKED. Per ADR-025. | DELIVER ships: non-Option author_did on IndexedClaim/NetworkResultRow + a per-author NetworkSearchResult with no merged-row API (type); `no_cross_table_join_elides_author` extended to `adapter-index-store` SQL (structural); `network_result_preserves_attribution` release gate (behavioral; KPI-AV-2). |
| WD-121 | **The verified-before-index gate (I-AV-1) reuses the PURE `claim-domain` verification core** (verify + CID recompute) at ingest; `verified_against NOT NULL` makes "every row verified" a schema invariant. No second verification path. | The cardinal trust precondition (WD-104; KPI-AV-3). Reusing the pure core (no second path that could drift) carries the slice-03 KPI-FED-6 discipline to network scale verbatim. Verification at INGEST (not query) centralizes the trust decision → the `[verified]` marker is a construction guarantee. | LOCKED. Per ADR-024/026. | DELIVER: `appview_domain::ingest_decision` calls `claim_domain::verify` + `compute_cid`; only `Index` reaches the store; `verified_against NOT NULL`; `indexer_rejects_unverified_claim` release gate (KPI-AV-3). |
| WD-122 | **The discovery→federation funnel reuses the slice-03 `peer add` VERBATIM** (a render-only affordance; no parallel subscription path; no auto-follow); the `--share` link encodes the QUERY, not a snapshot. | WD-110: the funnel is what makes the AppView STRENGTHEN the local-first graph instead of competing; reusing `peer add` preserves the slice-03 sovereignty model (revocable, no residue, I-FED-5). Encoding the query (not a snapshot) keeps the shared artifact attribution-preserving + always-current (anti-merging across the share boundary). | LOCKED. Per ADR-027. | DELIVER: the follow affordance prints the slice-03 command (no executable follow); `--share` encodes dimension+value only; `discovery_follow_reuses_slice03_path` + `share_link_encodes_query_not_snapshot` tests (KPI-AV-4/6). |
| WD-123 | **Slice-05 adds TWO external/cross-process integration boundaries** (CLI→indexer XRPC; indexer→network-author-PDS/PLC), both annotated for consumer-driven contract tests (handoff to DEVOPS). | The first external boundaries since slice-01/02; per principle 10 they are the highest-risk surfaces. Contract tests pin the response shapes the anti-merging (per-result author_did) + verified-before-index (record + DID-document shapes) gates depend on, catching breaking changes at build time. | LOCKED. | DEVOPS plans consumer-driven contracts (Pact-style) in CI; DELIVER's hermetic fixtures model the shapes. |
| WD-124 | The five DESIGN-wave ADRs (023-027) are accepted with this handoff; no further DESIGN iterations required pending peer review. | Each ADR has 2+ alternatives considered, the DISCUSS locks as binding inputs, and an Earned Trust section translating to concrete probe/test contracts. Slice-05 is the architecturally heaviest slice but a disciplined extension; the novel risks (the network lies; the container substrate lies about fsync; the real pubkey decode) are each met head-on by a dedicated probe + gold test. | LOCKED pending Atlas (solution-architect-reviewer) approval. | Reviewer may flag issues for an iteration-2 pass. |

## OD-AV resolutions (consolidated)

| OD | DISCUSS default | DESIGN resolution |
|---|---|---|
| OD-AV-1 (deployment shape) | DESIGN's call; recommend self-hostable single binary | **WD-112 LOCKED: self-hostable single binary `openlore-indexer`, signing-incapable.** Hosted is an additive future option. Per ADR-023. |
| OD-AV-2 (CLI→indexer transport) | DESIGN's call; recommend HTTP/XRPC | **WD-115 LOCKED: HTTP carrying the `org.openlore.appview.searchClaims` XRPC query method; the CLI queries a configured URL (localhost default).** Per ADR-027. |
| OD-AV-3 (graceful degradation) | Requirement LOCKED (WD-106); mechanism DESIGN's | **WD-116 LOCKED: soft `Unreachable` outcome; clear local-only message + `graph query` pointer; indexer not probed at CLI startup.** Per ADR-027. |
| OD-AV-4 (ingestion model; the ADR-016 re-eval) | DESIGN's call (WD-108); pull may suffice; Firehose an option | **WD-114 LOCKED: PULL-based bounded ingestion. Firehose is a documented future option, NOT slice-05's mechanism.** Per ADR-024. |
| OD-AV-5 (discovery surface grammar) | Recommend a new `search` verb | **WD-113 LOCKED: a new top-level `openlore search` verb.** Per ADR-027. |
| OD-AV-6 (share resolver) | CLI re-run only; web AppView OUT | **WD-122 LOCKED: `--share` encodes the query (not a snapshot); CLI-re-run resolver; web AppView OUT of scope.** Per ADR-027. |
| OD-AV-6 (pubkey decode) | Resolve the production PLC decode now (preferred) | **WD-118 LOCKED: implement the production PLC `z6Mk...` multibase decode now; pure decode helper in `claim-domain` + effect resolution in the verify-only adapter; the test seam release-forbidden.** Per ADR-026. |
| OD-AV-7 (retraction-aware search) | Appear normally; counter shown, filtering deferred | **WD-119 LOCKED: countered/retracted public verified claims appear normally; the counter relationship is shown when known, never applied; a filter is deferred.** |

> Note: the DISCUSS open-decisions table labels TWO distinct concerns "OD-AV-6"
> (the share resolver in the table; the pubkey-decode dependency in the
> feature-delta Risks + Handoff "Decide" list). Both are resolved above (WD-122 +
> WD-118 respectively); the numbering ambiguity is recorded as a non-blocking
> upstream observation (see Upstream Issues, feature-delta DESIGN section).

## Decisions DEFERRED to DELIVER

| # | Question | Default for DELIVER | Why deferred |
|---|---|---|---|
| Q-DELIVER-AV-1 | Exact `index.duckdb` migration SQL + the `indexed_claims/<did>/<cid>.json` DID→filename encoding | The ADR-025 schema shape + the slice-03 `did_plc_...` encoding | ADR-025 fixes the schema + anti-merging constraints; exact DDL subject to the index-store probe. |
| Q-DELIVER-AV-2 | The HTTP server framework (`axum` vs hand-rolled `hyper`) | `axum` (tokio-native, minimal for one route) OR a hand-rolled `hyper` handler | ADR-027 fixes the XRPC method + response shape; the framework is DELIVER's within the rustls-ecosystem + `cargo deny` allowlist. |
| Q-DELIVER-AV-3 | `openlore://search?...` link format + the CLI re-run parser grammar | A query-string encoding dimension+value | ADR-027 fixes the query-encoding-not-snapshot contract; DELIVER fills the exact format DISTILL asserts. |
| Q-DELIVER-AV-4 | Ingest cadence defaults (`--ingest-interval`, batch size) | A conservative interval; per-record verify dominates cost | ADR-024 fixes the bounded-pull + fault-isolation contract; DELIVER tunes against the freshness budget. |
| Q-DELIVER-AV-5 | One `IndexQueryPort`/`IndexStorePort` method with a dimension enum vs three thin methods | One filtered method (smaller anti-merging enforcement surface, mirrors slice-04) | No invariant blocks either; crafter confirms by ergonomics. |
| Q-DELIVER-AV-6 | PLC directory endpoint config + the DID-method support boundary | `did:plc:*` Ed25519 `z6Mk...` only; unsupported types rejected explicitly | ADR-026 fixes the decode + the explicit-rejection boundary; DELIVER fixes the config key + messages DISTILL asserts. |
| Q-DELIVER-AV-7 | `search` degraded mode: DELEGATE to local `graph query` or just POINT to it | Either (US-AV-002 Ex 3 allows both); the contract is "clear local-only message, no hang, no fatal error" | DELIVER's call against DISTILL scenarios. |
| Q-DELIVER-AV-8 | The base58 multibase decode: `bs58` crate vs hand-rolled inline | Either; MUST stay in the pure-core allowlist (no I/O) | base58btc is ~40 lines; DELIVER picks dep-vs-inline within the allowlist. |
| Q-DELIVER-AV-9 | Whether a once-per-user first-search orientation message ships (mirroring slice-03/04 OrientationState) | Optional; not load-bearing | DELIVER's call against DISTILL scenarios; an `[appview]` orientation key in identity.toml if shipped. |

## ADR proposals (this DESIGN wave)

| ADR | Title | Status | Replaces / amends |
|---|---|---|---|
| ADR-023 | AppView Indexer = Self-Hostable Single Binary, Signing-Incapable by Construction | Proposed | Extends ADR-009 (hexagonal; second composition root) + ADR-007; inherits the slice-02 I-SCR-1 human-gate |
| ADR-024 | Pull-Based Bounded Network Ingestion — the ADR-016 Firehose Re-Evaluation | Proposed | Revisits/affirms ADR-016 (pull-on-demand; Firehose deferred again) |
| ADR-025 | Network Index = a Separate `index.duckdb` + Anti-Merging at Network Scale | Proposed | Extends ADR-001 (DuckDB) + ADR-014 (anti-merging three-layer) + ADR-022 (anti-merging-in-aggregates) |
| ADR-026 | Production PLC DID-Document Multibase Pubkey Decode — Resolving the slice-03 DV-4 Seam | Proposed | Extends ADR-002 (identity/DID) + ADR-016 (pull-time verification); resolves the DV-4 test-only seam |
| ADR-027 | `openlore search` Verb + CLI→Indexer HTTP/XRPC Transport + Graceful Degradation | Proposed | Amends ADR-003 + ADR-013 + ADR-020 (verb/flag contract); reuses ADR-013 `peer add` for the funnel |

## Inherited locks summary (do NOT relitigate)

| Source | Locks |
|---|---|
| Slice-01 | ADR-001..012; WD-1..WD-13; the 12 cross-feature invariants in `docs/product/architecture/brief.md` |
| Slice-02 | ADR-017..019; WD-46..WD-68; I-SCR-1..7 (the human-gate at the architecture layer — `adapter-github` holds no storage/identity/pds reference; slice-05's indexer mirrors it) |
| Slice-03 | ADR-013..016; WD-26..WD-45; I-FED-1..7 (anti-merging at storage/query/display/test — EXTENDED to network scale this slice); KPI-FED-6 (pull-time verification — EXTENDED to network ingest); the DV-4 test-only pubkey seam (RESOLVED this slice, ADR-026) |
| Slice-04 | ADR-020..022; WD-80..WD-93; I-GRAPH-1..8 (anti-merging in aggregates — the direct ancestor of I-AV-2) |
| Slice-05 DISCUSS | WD-100..WD-110 (feature-delta.md) + OD-AV-1..7 (resolved above) |
| Slice-05 DESIGN | WD-111..WD-124 (this file) + ADR-023..027 + I-AV-1..9 |

## Handoff

This file is the canonical DESIGN-wave decision record. It is consumed by:

- **Atlas (solution-architect-reviewer)** for peer review iteration 1.
- **DISTILL (nw-acceptance-designer)** for resolving the DISCUSS `# DISTILL: confirm`
  flags (search verb vs --network flag → new `search` verb; deployment shape →
  self-hostable single binary; pull-vs-Firehose → pull; the pubkey-decode mechanism
  → real PLC z6Mk decode) and turning the release gates into executable acceptance
  tests (`indexer_rejects_unverified_claim`, `network_result_preserves_attribution`,
  `local_first_preserved`, `public_data_banner_shown`, `verified_marker_is_universal`,
  `discovery_follow_reuses_slice03_path`, `share_link_encodes_query_not_snapshot`,
  `countered_claim_still_appears`).
- **DEVOPS (nw-platform-architect)** for instrumentation (KPI-AV-1..6 events; the
  index-coverage/freshness dashboard; release-blocking alerts on KPI-AV-2/3 != 100%
  + KPI-5 regression), the new deployable (`openlore-indexer`, ADR-023), and the TWO
  consumer-driven contract tests (CLI→indexer XRPC; indexer→PDS/PLC).
- **DELIVER (nw-functional-software-crafter per ADR-007)** for implementation;
  Q-DELIVER-AV-1..9 are crafter's call within the locked contracts.

### Component Inventory update (for finalize; NOT applied to brief.md now)

At finalize (slice-03/04 precedent — DESIGN does not edit the SSOT brief mid-wave),
the brief's Component Inventory gains six rows:

| Crate | Kind | Purpose | Shipped in |
|---|---|---|---|
| `crates/appview-domain` | pure core | PURE ingest-gate decision + search/grouping/anti-merging composition; no I/O | slice-05 |
| `crates/adapter-atproto-ingest` | effect | `IngestSourcePort` — bounded PULL of public records (read-only) | slice-05 |
| `crates/adapter-index-store` | effect | `IndexStorePort` over the separate `index.duckdb` (non-Option author_did; no merged schema) | slice-05 |
| `crates/adapter-index-query` | effect | `IndexQueryPort` — CLI→indexer HTTP/XRPC client; graceful degradation | slice-05 |
| `crates/adapter-xrpc-query-server` | effect | serves `org.openlore.appview.searchClaims` over HTTP | slice-05 |
| `crates/openlore-indexer` | driver (binary) | the SECOND composition root; self-hostable; signing-incapable; holds no local store | slice-05 |

Production crate count: 11 → 17 (+1 test-support +1 xtask = 19 workspace members).
External dependency count: +2 minimal (an HTTP server framework + a base58 crate,
both MIT, both with hand-rolled fallbacks). The ADR-016 "re-evaluate Firehose at
slice-05" note resolves to PULL (WD-114 / ADR-024); the slice-03 DV-4 pubkey seam
resolves to the real PLC decode (WD-118 / ADR-026); both should be annotated as
resolved in the brief at finalize. The brief's "slice-05: adds an indexer service
(separate binary)" line is realized.

### Handoff-ready?

**YES.** WD-111..WD-124 LOCKED; OD-AV-1..7 ALL resolved; ADR-023..027 proposed
(pending Atlas review); the headline local-first↔network-service architecture
decided (self-hostable single binary, HTTP/XRPC, graceful degradation, pull
ingestion); the production PLC pubkey-decode dependency RESOLVED (the slice-03 DV-4
seam closed); the cardinal anti-merging invariant carried to network scale with
three-layer enforcement + the load-bearing no-merged-schema absence; the
verified-before-index gate reusing the pure core (no second path); the first
network service + the first adversarial-input external boundary each met by a
dedicated "what if X lies?" probe. DISTILL has the 8 release gates + the resolved
`# confirm` flags; DEVOPS has the KPI events + the new deployable + the two contract
tests; DELIVER has the contracts + the Q-DELIVER-AV set. No blockers.
