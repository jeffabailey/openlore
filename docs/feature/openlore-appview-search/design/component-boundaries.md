# Component Boundaries — openlore-appview-search (slice-05) — DELTA from slice-04

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Style**: Hexagonal + Modular Monolith — now TWO single-purpose binaries in one workspace (ADR-009, inherited; ADR-023, extended)
- **Paradigm**: Functional-leaning Rust (ADR-007, inherited)
- **Extends**: `docs/feature/openlore-scoring-graph/design/component-boundaries.md`

This document specifies ONLY the component-boundary deltas for slice-05.
The slice-01/02/03/04 crates are inherited unchanged in their prior
responsibilities. Slice-05 ADDS one pure crate (`appview-domain`), three effect
crates (`adapter-atproto-ingest`, `adapter-index-store`, `adapter-index-query`,
`adapter-xrpc-query-server` — see grouping note), and ONE new binary crate
(`openlore-indexer`); and EXTENDS `ports`, `claim-domain`, `lexicon`,
`adapter-atproto-did`, `cli`, and `xtask`. Everything else is unchanged.

## Crate layout (slice-05 adds the indexer subsystem)

```
openlore/                          # workspace root — now ships TWO binaries
  crates/
    claim-domain/                  # PURE — EXTENDED (adds decode_ed25519_multibase, ADR-026; verify/CID UNCHANGED + reused)
    lexicon/                       # PURE — EXTENDED (adds the org.openlore.appview.searchClaims XRPC query lexicon; a READ query)
    ports/                         # PURE — EXTENDED (IndexQueryPort/IngestSourcePort/IndexStorePort/IdentityResolvePort + IndexedClaim/NetworkResultRow ADTs)
    appview-domain/                # PURE — NEW (ingest-gate decision + search/grouping/anti-merging + result-shaping; NO I/O)
    adapter-duckdb/                # EFFECT — UNCHANGED (the CLI's LOCAL store; the indexer never touches it)
    adapter-atproto-did/           # EFFECT — EXTENDED (verify-only DID-doc → pubkey production decode, ADR-026; seam retained but release-forbidden)
    adapter-atproto-pds/           # EFFECT — UNCHANGED (CLI publish/peer-pull)
    adapter-system-clock/          # EFFECT — UNCHANGED
    adapter-index-query/           # EFFECT — NEW (CLI side): IndexQueryPort over HTTP/XRPC to the indexer; graceful degradation
    adapter-atproto-ingest/        # EFFECT — NEW (indexer side): IngestSourcePort, bounded PULL of public records (ADR-024)
    adapter-index-store/           # EFFECT — NEW (indexer side): IndexStorePort over index.duckdb (ADR-025)
    adapter-xrpc-query-server/     # EFFECT — NEW (indexer side): serves org.openlore.appview.searchClaims over HTTP (ADR-027)
    scraper-domain/                # PURE — UNCHANGED (slice-02)
    adapter-github/                # EFFECT — UNCHANGED (slice-02)
    scoring/                       # PURE — UNCHANGED (slice-04)
    cli/                           # DRIVER — EXTENDED (the `openlore search` verb + wiring/soft-probing adapter-index-query; reuses peer add for the funnel)
    openlore-indexer/              # DRIVER — NEW binary: the SECOND composition root (signing-incapable; holds no local store)
    test-support/                  # test-only — EXTENDED (FakeIngestSource, FakeIndexStore, FakeIndexQuery, real-z6Mk DID-doc fixture, adversarial ingest fixtures)
  xtask/                           # EXTENDED (anti-merging rule → index store; indexer capability-boundary rule; no-pubkey-seam-in-release; appview-domain pure-core allowlist)
```

**New crate count**: +1 pure (`appview-domain`), +4 effect (`adapter-index-query`,
`adapter-atproto-ingest`, `adapter-index-store`, `adapter-xrpc-query-server`),
+1 binary driver (`openlore-indexer`). Production crate count goes from 11 to 17
(+1 test-support +1 xtask). The slice-03 no-new-crate ethos (WD-26) governs
PRODUCTION RUNTIME DEPENDENCIES and unnecessary boundaries; slice-05's new crates
are a GENUINE new operational boundary (the first network service) — a separate
binary with its own composition root, its own store, and its own external
boundaries. This is the architecturally heaviest slice by crate count BECAUSE it
introduces the first network service; the crates map 1:1 to the new ports/adapters
the hexagonal style requires.

**Grouping note**: `adapter-index-query` (CLI side) and `adapter-xrpc-query-server`
(indexer side) are the two ends of the SAME XRPC contract (ADR-027). They are
separate crates because they live in different binaries (the CLI must not link the
server; the indexer must not link the CLI's client). DELIVER MAY co-locate the
shared request/response DTOs in `lexicon` (the query lexicon) to avoid drift.

## Component contract deltas

### `crates/appview-domain` (PURE) — NEW

**Responsibility**: the pure ingest-gate decision (verify-before-index) and the
pure search/grouping/anti-merging/result-shaping logic. Holds NO I/O, NO
persistence, NO knowledge of DuckDB/HTTP/the network. The symmetric counterpart
to slice-02 `scraper-domain` + slice-04 `scoring`.

**Public surface**:

```rust
/// The PURE verify-before-index gate. Calls claim_domain::verify + compute_cid (the SAME
/// pure core; NO second verification path, WD-104). Deterministic; no I/O.
pub fn ingest_decision(record: &RawRecord, resolved_key: &VerificationKey) -> IngestOutcome;

pub enum IngestOutcome {
    Index(IndexedClaim),         // verified + CID-matched; author_did from the SIGNED payload
    Reject(RejectReason),        // unsigned | bad_signature | cid_mismatch | schema_unknown
}

pub enum RejectReason { Unsigned, BadSignature, CidMismatch, SchemaUnknown }

/// The PURE anti-merging-preserving search composition. Groups by author (or by subject
/// under an author); NEVER merges authors; computes distinct_author_count from the rows.
pub fn compose_results(rows: Vec<IndexedClaim>, dimension: SearchDimension) -> NetworkSearchResult;

pub struct NetworkSearchResult {
    pub by_author: Vec<(Did, Vec<NetworkResultRow>)>,  // per-author; NO merged row exists
    pub distinct_author_count: u32,                    // COUNT over attributed rows, never a merge
    pub total_claims: u32,
    pub suggestion: Option<String>,                    // near-match for an empty result (US-AV-002 Ex 4)
}

pub struct NetworkResultRow {
    pub author_did: Did,         // non-Option; LOAD-BEARING (anti-merging, WD-103)
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,         // numeric [0.0,1.0] (WD-10 / I-6)
    pub verified_against: KeyId, // never empty (WD-104)
    pub relationship: AuthorRelationship,  // You|SubscribedPeer|UnsubscribedCache|NetworkUnfollowed
    pub counter_annotation: Option<CounterRef>,  // OD-AV-7: shown, never applied
}

/// Near-match suggestion for an empty dimension result (edit distance over known values).
pub fn near_match_suggestion(query: &str, known: &[String]) -> Option<String>;
```

**Forbidden dependencies**: `tokio`, `reqwest`, `duckdb`, `keyring`, `atrium-api`,
`std::fs`, `std::net`, `std::time::SystemTime`, any `adapter-*` crate. MAY depend
on `chrono` (pure time types) + the pure `Did`/`Cid`/`VerificationKey` value types
from `ports`/`claim-domain`. Added to the `xtask check-arch` pure-core allowlist
(I-1/I-2).

**Probe responsibilities** (the pure-core analog — property + mutation tests, NOT
a `probe()`; `appview-domain` touches no substrate):

- Property: `ingest_decision` returns `Index` iff `verify` + CID both pass; a
  tampered/unsigned/CID-mismatch record returns `Reject` (the gate, WD-104).
- Property: `compose_results` preserves EVERY author — `distinct_author_count`
  equals the number of distinct `author_did`s in the input; no row is dropped or
  merged; identical-content-different-author rows stay separate (WD-103).
- Property: `ingest_decision` + `compose_results` are deterministic.
- Unit: a countered claim still appears in `compose_results`; the annotation is
  present, the row is NOT removed (OD-AV-7).
- Mutation testing on `ingest_decision` + `compose_results` (Earned Trust applied
  to the tests).

### `crates/claim-domain` (PURE) — EXTENDED (ADR-026)

**Slice-05 addition**: the PURE multibase pubkey-decode helper. `verify` +
`compute_cid` are UNCHANGED (and reused by the indexer — no second path).

```rust
/// PURE: decode a z6Mk... base58btc multibase publicKeyMultibase into the Ed25519
/// verification key the pure verify() consumes. NO I/O. (ADR-026)
pub fn decode_ed25519_multibase(s: &str) -> Result<VerificationKey, DecodeError>;

pub enum DecodeError { NotMultibase, BadBase58, BadMulticodecPrefix, WrongKeyLength, UnsupportedKeyType }
```

**Forbidden dependencies** (unchanged pure-core allowlist): MAY add a small pure
base58 dependency (e.g. `bs58`, MIT) if not derivable from existing deps — a PURE
dependency, whitelisted in the pure-core allowlist like slice-03's
`unicode-normalization` + slice-02's `serde_yaml_ng`. Multicodec-prefix handling
is trivial byte ops (no dependency).

**Probe responsibilities**: none (pure). Earned Trust: property test
`decode∘encode == identity` for valid keys; malformed input errors (never panics,
never mis-decodes); mutation testing on the decode + prefix-strip.

### `crates/lexicon` (PURE) — EXTENDED

**Slice-05 addition**: the `org.openlore.appview.searchClaims` XRPC query lexicon
(a `query` type — a READ query; NO signed payload, NO CID-stability concern). Plus
the shared request/response DTOs (`SearchQueryRequest`, `SearchQueryResponse`) so
the CLI client + indexer server agree on the shape without drift. Per ADR-027 the
response carries per-result `author_did` (anti-merging across the transport).

**Forbidden dependencies** (unchanged): MAY depend on `serde`; no `adapter-*`.

**Probe responsibilities**: none (pure). The serde round-trip of the query DTOs is
property-tested (the response shape always carries `author_did`).

### `crates/ports` (PURE) — EXTENDED

**Slice-05 additions to public surface** (FOUR new ports + the `IndexedClaim`
boundary value):

```rust
// CLI side: the CLI→indexer transport. ASYNC (network). Unreachable is a SOFT outcome.
pub trait IndexQueryPort {
    async fn search(&self, dim: SearchDimension, value: &str, cid: Option<&Cid>)
        -> Result<NetworkSearchResultRaw, IndexQueryError>;
    fn probe(&self) -> ProbeOutcome;     // I-4
}
pub enum IndexQueryError { Unreachable, BadResponse, NotFound }   // Unreachable is NON-FATAL (ADR-027/KPI-5)

// Indexer side: bounded PULL of public records. ASYNC (network). Read-only.
pub trait IngestSourcePort {
    async fn enumerate(&self, source: &IngestSource) -> Result<Vec<RawRecord>, IngestError>;
    fn probe(&self) -> ProbeOutcome;     // I-4
    // NO write/sign/publish method exists on this trait (read-only by construction).
}

// Indexer side: index store over index.duckdb. SYNC (local DB).
pub trait IndexStorePort {
    fn upsert(&self, claim: &IndexedClaim) -> Result<(), IndexStoreError>;       // de-dups by CID
    fn query_by_object(&self, object: &str) -> Result<Vec<IndexedClaim>, IndexStoreError>;
    fn query_by_contributor(&self, did: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError>;
    fn query_by_subject(&self, subject: &str) -> Result<Vec<IndexedClaim>, IndexStoreError>;
    fn get_by_cid(&self, cid: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError>;
    fn probe(&self) -> ProbeOutcome;     // I-4
    // Every method returns rows carrying non-Option author_did. NO method aggregates across authors.
}

// Shared (used by the indexer; the verify-only key resolution). ASYNC (network). NO signing.
pub trait IdentityResolvePort {
    async fn resolve_verification_key(&self, did: &Did) -> Result<VerificationKey, ResolveError>;
    fn probe(&self) -> ProbeOutcome;     // I-4
    // NO sign()/publish()/put_record() method exists on this trait (verify-only by construction).
}

pub enum SearchDimension { Object, Contributor, Subject }

pub struct IndexedClaim {            // defined here; consumed by appview-domain + adapters + renderers
    pub author_did: Did,             // non-Option; LOAD-BEARING (anti-merging, WD-103)
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,             // numeric [0.0,1.0]
    pub composed_at: DateTime<Utc>,
    pub verified_against: KeyId,     // never empty (WD-104)
    pub evidence: Vec<String>,
    pub references: Vec<ClaimReference>,
    pub relationship: AuthorRelationship,
}
```

`AuthorRelationship` reuses the slice-03 enum + ONE new variant `NetworkUnfollowed`
(an author in the network index the user does not subscribe to → `(not subscribed)`
label). The variant set: `You | SubscribedPeer | UnsubscribedCache | NetworkUnfollowed`.

**Forbidden dependencies** (unchanged): traits may reference `lexicon`,
`claim-domain`, and the pure value types. The async traits (`IndexQueryPort`,
`IngestSourcePort`, `IdentityResolvePort`) follow the existing `async_trait`/PdsPort
pattern; `IndexStorePort` is sync (like `StoragePort`).

**Probe responsibilities**: none (traits don't probe; implementations do — and the
probe is a REQUIRED trait method per I-4).

### `crates/adapter-atproto-did` (EFFECT) — EXTENDED (ADR-026)

**Slice-05 addition**: implement the verify-only `IdentityResolvePort` production
path — resolve the PLC DID document, locate the `#org.openlore.application`
verification method, and call the pure `claim_domain::decode_ed25519_multibase`.
The slice-03 `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam is RETAINED but
release-forbidden (a release build that reads it fails `xtask check-arch`).

**Public surface addition**: `impl IdentityResolvePort for AtProtoDidAdapter`
(verify-only — the existing signing `IdentityPort` impl is separate + the CLI's;
the indexer wires ONLY the resolve-only variant, ADR-023).

**Forbidden dependencies** (unchanged): other `adapter-*` crates.

**Probe responsibilities** (slice-05 addition, per ADR-026): resolve a FIXTURE DID
document with a real `z6Mk...` value, decode it, assert the key VERIFIES a
known-good signature AND REJECTS a tampered one (the gold test runs the REAL decode
path; a seam-only pass is a CI failure). Within the 250ms budget (I-5).

### `crates/adapter-atproto-ingest` (EFFECT) — NEW (ADR-024)

**Responsibility**: implement `IngestSourcePort` — bounded PULL of public
`org.openlore.claim` records from network sources (seed DIDs → PDS `listRecords`;
an optional configured relay). Read-only; holds NO write/sign/publish capability;
reuses the workspace `reqwest` (rustls) (no new transport dependency, like
`adapter-github`).

**Public surface**: `impl IngestSourcePort for AtProtoIngestAdapter`.

**Forbidden dependencies**: other `adapter-*` crates; ANY signing/publish surface
(read-only by construction).

**Probe responsibilities** (per ADR-024): (a) source reachability + enumeration
shape against a fixture source; (b) **the network-lies check** — a fixture source
returns a tampered-signature + a CID-mismatch record; the probe asserts the ingest
path (via `appview_domain::ingest_decision`) REJECTS both before the index. Within
250ms.

### `crates/adapter-index-store` (EFFECT) — NEW (ADR-025)

**Responsibility**: implement `IndexStorePort` over the SEPARATE `index.duckdb`
store. Non-`Option` `author_did` rows; anti-merging-preserving per-author queries
(NO `GROUP BY author` / merge); `verified_against NOT NULL`. Reuses the slice-01/03
DuckDB connection/migration/probe patterns on a separate file.

**Public surface**: `impl IndexStorePort for IndexStoreAdapter`.

**Forbidden dependencies**: other `adapter-*` crates; the user's `StoragePort` /
`openlore.duckdb` (the indexer holds no local-store handle, ADR-023).

**Probe responsibilities** (per ADR-025): (a) schema-version + **fsync honored on
the container substrate** (overlayfs/DrvFs/tmpfs lie → refuse with
`storage.fsync_unhonored`); (b) attribution round-trip (two rows, same
(subject,object), distinct non-empty `author_did`s read back byte-equal); (c)
no-merge-schema assertion (NO `consensus`/`merged` table). Within 250ms.

### `crates/adapter-xrpc-query-server` (EFFECT) — NEW (ADR-027)

**Responsibility**: serve `org.openlore.appview.searchClaims` over HTTP; read
attributed rows via `IndexStorePort`; call `appview_domain::compose_results`; return
the per-author result with `author_did` always present (anti-merging across the
transport). The HTTP framework (`axum`/`hyper`) is DELIVER's call within the
rustls-ecosystem + `cargo deny` allowlist.

**Public surface**: the HTTP server bind + the query handler. (This is a driving
surface for the indexer — the inbound query API — implemented as an effect adapter
the indexer composition root wires.)

**Forbidden dependencies**: other `adapter-*` crates except via the indexer
composition root.

**Probe responsibilities**: (a) bind + serve a fixture query; (b) the response
shape carries per-result `author_did` (a response dropping it is a contract
violation caught at probe time). Within 250ms.

### `crates/adapter-index-query` (EFFECT) — NEW (ADR-027, CLI side)

**Responsibility**: implement `IndexQueryPort` over HTTP/XRPC to the indexer at a
CONFIGURED URL; treat indexer-unreachable as a SOFT non-fatal
`IndexQueryError::Unreachable` (graceful degradation). Reuses workspace `reqwest`.

**Public surface**: `impl IndexQueryPort for HttpIndexQueryAdapter`.

**Forbidden dependencies**: other `adapter-*` crates; the indexer's server crate
(the CLI must not link the server).

**Probe responsibilities** (per ADR-027): (a) a reachable fixture indexer returns
the expected XRPC shape with `author_did` present; (b) **the inverted/degradation
check** — an UNREACHABLE indexer yields `Unreachable` (soft, non-fatal), NOT a
startup refusal (the CLI MUST start without a reachable indexer; KPI-5). Within
250ms.

### `crates/cli` (DRIVER) — EXTENDED

**Slice-05 additions to responsibility**: add the `openlore search` verb
(`--object`/`--contributor`/`--subject`/`--show <cid>`/`--share`) per ADR-027; wire
+ SOFT-probe the `HttpIndexQueryAdapter` (skipped-or-soft at startup — the CLI must
start without a reachable indexer); render the network search results (per-author
groups + `[verified]` marker + relationship labels + the public-data banner + the
no-merge footer + the `peer add` follow affordance for unfollowed authors); render
`--show` verification lines (the SAME pure-core verification result; no second
path); emit the `--share` query-encoding link. The follow funnel REUSES the
slice-03 `peer add` verbatim (render-only hint; no auto-follow; no parallel state).

**Public surface addition**: the `search` verb (a NEW top-level verb, ADR-027).
The `openlore` binary remains the ONLY composition root wiring the USER's signing
identity + local store (I-3).

**Forbidden dependencies** (unchanged): the CLI must NOT link the indexer's server
crate (`adapter-xrpc-query-server`) nor the indexer's store/ingest crates.

**Probe responsibilities** (slice-05 additions): `search` verb behaviors per
US-AV-002..006 (the DISTILL acceptance gates). The local-first probe: `claim add`
/ offline `claim publish` / `graph query` succeed with the indexer down AND network
disabled; `search` prints the local-only message without a fatal error
(`local_first_preserved`).

### `crates/openlore-indexer` (DRIVER) — NEW binary

**Responsibility**: the SECOND composition root (ADR-009/023). Wire the four driven
adapters (`IngestSourcePort`, `IndexStorePort`, `IdentityResolvePort`, the HTTP
query server); run ALL probes BEFORE ingest/serve; refuse to start on any probe
failure (`health.startup.refused` + exit 2). Run the bounded pull-ingest loop +
serve queries. Hold NO signing/publish capability + NO local-store handle (the
capability boundary, ADR-023).

**Public surface**: the `openlore-indexer` binary with subcommands `serve` (run the
query server + ingest loop) and `ingest` (a one-shot bounded pull pass).

**Forbidden dependencies** (the capability boundary, ADR-023, enforced): MUST NOT
depend on the user's signing `IdentityPort` impl, the user's `StoragePort` /
`adapter-duckdb`, or any PDS-write surface. MAY depend on the verify-only
`IdentityResolvePort` impl, `IngestSourcePort`, `IndexStorePort`, the query server,
and the pure cores (`appview-domain`, `claim-domain`, `lexicon`, `ports`).

**Probe responsibilities**: the composition-root probe gauntlet (wire → probe →
use) + the `capability_boundary_probe` (asserts the store is `index.duckdb` and the
identity adapter is resolve-only; refuses on violation, ADR-023).

### `crates/test-support` (test-only) — EXTENDED

Adds: `FakeIngestSource` (a bounded fixture record source incl. adversarial
records: unsigned / tampered-signature / CID-mismatch), `FakeIndexStore` (an
in-memory `IndexStorePort` double), `FakeIndexQuery` (a `IndexQueryPort` double +
an "unreachable" mode for the degradation test), a real-`z6Mk...` DID-document
fixture (a known test keypair, for the ADR-026 decode gold test), and
network-search fixtures (multi-author same-(subject,object) for anti-merging). No
production surface.

### `xtask/` (workspace member) — EXTENDED

**Slice-05 additions to responsibility**:

- EXTEND the `check-arch` rule `no_cross_table_join_elides_author` (slice-03/04) to
  cover the `adapter-index-store` SQL string literals: any literal aggregating over
  `indexed_claims` (GROUP BY / COUNT / SUM across authors) without projecting
  `author_did` fails CI. (I-AV-2 structural layer.)
- ADD the `check-arch` rule `indexer_holds_no_signing_or_local_store`: the
  `openlore-indexer` crate's dependency graph MUST NOT include the signing
  `IdentityPort` impl, the user's `StoragePort`/`adapter-duckdb`, or any PDS-write
  surface. (I-AV-5 structural layer; mirrors the slice-02 I-SCR-1 rule for
  `adapter-github`.)
- ADD the `check-arch` rule `no_pubkey_seam_in_release_build`: a release binary
  that reads `OPENLORE_PEER_PUBKEY_HEX_<did>` fails the check (the production path
  is the REAL ADR-026 decode). (I-AV-6 structural layer; mirrors slice-03's
  `no_autoconfirm_in_release_build`.)
- ADD `appview-domain` to the `check-arch` pure-core allowlist (alongside
  `claim-domain`, `lexicon`, `ports`, `scraper-domain`, `scoring`); enforce it
  imports NO I/O crate (I-1/I-2).
- EXTEND I-3 (composition-root rule) to cover BOTH binaries: `cli` is the only root
  wiring the USER's adapters; `openlore-indexer` is the only root wiring the
  indexer's adapters; neither wires the other's.
- `check-probes` already covers `impl <Port> for <Adapter>` non-stub probe bodies;
  the four new adapters' probes are picked up unchanged. `appview-domain` has no
  `probe()` (pure crate) — `check-probes` correctly does not require one.

**Public surface**: `cargo xtask check-arch` (3 new rules + extended scope + new
allowlist entry), `cargo xtask check-probes` (extended trait set: the 4 new ports).
Run from CI on every commit, for BOTH binaries.

## Composition-root wiring (the two roots)

### `openlore` CLI (ADR-009, extended)

```
fn main() -> ExitCode {                          // crates/cli
    // WIRE: USER's adapters (UNCHANGED) + the NEW index-query client
    let clock    = SystemClockAdapter::new();
    let storage  = DuckDbStorageAdapter::open(&cfg.storage_path)?;   // LOCAL source of truth
    let identity = AtProtoDidAdapter::resolve(&cfg.identity)?;       // SIGNING-capable (CLI only)
    let pds      = AtProtoPdsAdapter::new(&cfg.pds_endpoint, &identity)?;
    let index_q  = HttpIndexQueryAdapter::new(&cfg.indexer_url)?;    // NEW: graceful-degrading client

    // PROBE: storage/identity/pds hard-probe (unchanged); index_q is SOFT (KPI-5)
    for outcome in [ storage.probe(), identity.probe(), pds.probe() /*skip if --offline*/, clock.probe() ] {
        if let Err(refused) = outcome { emit_health_event(refused); return ExitCode::from(2); }
    }
    // index_q.probe() is informational/soft — an unreachable indexer MUST NOT block CLI startup.

    // USE: dispatch (search uses index_q + graceful degradation; all other verbs UNCHANGED)
    cli::dispatch(cfg, storage, identity, pds, clock, index_q)
}
```

### `openlore-indexer` (ADR-009/023, NEW)

```
fn main() -> ExitCode {                          // crates/openlore-indexer
    // WIRE: the indexer's adapters — NO signing identity, NO local store (capability boundary)
    let clock      = SystemClockAdapter::new();
    let index_store = IndexStoreAdapter::open(&cfg.index_path)?;     // SEPARATE index.duckdb
    let ingest_src  = AtProtoIngestAdapter::new(&cfg.sources, &cfg.relay)?;  // read-only PULL
    let resolve     = AtProtoDidAdapter::resolve_only(&cfg.plc_endpoint)?;   // VERIFY-ONLY (ADR-026)
    let query_server = XrpcQueryServer::bind(&cfg.listen_addr)?;

    // PROBE: wire → probe → use; ANY failure refuses to start; + the capability-boundary probe
    capability_boundary_probe(&index_store, &resolve)?;   // assert index.duckdb + resolve-only (ADR-023)
    for outcome in [ index_store.probe(), ingest_src.probe(), resolve.probe(), query_server.probe() ] {
        if let Err(refused) = outcome { emit_health_event(refused); return ExitCode::from(2); }
    }

    // USE: run the bounded pull-ingest loop + serve queries
    indexer::run(cfg, index_store, ingest_src, resolve, query_server)
}
```

The two roots are DISJOINT: the CLI never wires the index store / ingest / query
server; the indexer never wires the user's signing identity / local store. `xtask
check-arch` enforces I-3 for both + the I-AV-5 capability boundary.

## Cross-component invariants — slice-05 additions (enforced)

| # | Invariant | Enforced by |
|---|---|---|
| I-AV-1 | **Verified-before-index** (WD-104; extends KPI-FED-6): signature-verified (against the REAL PLC key, ADR-026) + CID-recomputed via the pure core BEFORE any record enters the index; no second verification path; every result `[verified]` by construction | pure `appview_domain::ingest_decision` (calls `claim_domain::verify` + `compute_cid`) + `verified_against NOT NULL` (ADR-025) + the ingest-adapter probe (rejects a fixture tampered/CID-mismatch record) + the `indexer_rejects_unverified_claim` release gate (KPI-AV-3) |
| I-AV-2 | **Anti-merging at network scale** (WD-103; extends I-FED-1 / I-GRAPH-1/2): every indexed/searched/shared result carries a non-`Option` author DID; NO merged consensus schema/row anywhere; identical-content-different-author = separate rows | THREE layers: (a) `IndexedClaim`/`NetworkResultRow.author_did` non-`Option`; `compose_results` returns a per-author structure with no merged-row API (type); (b) `xtask check-arch` `no_cross_table_join_elides_author` extended to `adapter-index-store` SQL (structural); (c) `network_result_preserves_attribution` release gate (behavioral; KPI-AV-2) |
| I-AV-3 | **Local-first preserved** (WD-106 / KPI-5): the CLI links no indexer code; `search` is the only network verb + degrades gracefully; the indexer is NOT probed at CLI startup; offline compose/sign + local query unaffected | the CLI dependency graph excludes the indexer's store/ingest/server crates (xtask check-arch) + the soft `index_q` probe + the `local_first_preserved` release gate (KPI-5) |
| I-AV-4 | **Public-data-only** (WD-105 / KPI-AV-5): the indexer ingests ONLY public signed claims, reads no private data, exposes no surveillance affordance; the public-data banner surfaces the expectation | the ingest adapter reads only public `listRecords` (no auth-scoped read) + the `public_data_banner_shown` acceptance test (KPI-AV-5) + no telemetry on claim CONTENTS (DEVOPS) |
| I-AV-5 | **Indexer signing-incapable + holds no local store** (ADR-023; mirrors I-SCR-1): the indexer cannot author/sign/mutate/publish a claim + cannot touch `openlore.duckdb` | THREE layers: (a) verify-only `IdentityResolvePort` + read-only `IngestSourcePort` (no sign/publish method exists; type); (b) `xtask check-arch` `indexer_holds_no_signing_or_local_store` (structural); (c) the composition-root `capability_boundary_probe` (behavioral) |
| I-AV-6 | **Production pubkey decode is real** (ADR-026): production verification resolves + decodes the author's REAL PLC `z6Mk...` key; the test seam is release-forbidden | the verify-only adapter's REAL decode path + the gold test running the real decode + `xtask check-arch` `no_pubkey_seam_in_release_build` (structural) |
| I-AV-7 | **Discovery feeds federation via `peer add` verbatim** (WD-110; reuses I-FED-5): the follow affordance is a render-only hint printing the slice-03 command; no parallel subscription path; no auto-follow | the affordance is a renderer string (no executable follow path) + the `discovery_follow_reuses_slice03_path` acceptance test (KPI-AV-4) |
| I-AV-8 | **Shareable link encodes the query, not a snapshot** (WD-110): `--share` encodes the query dimension+value; resolving re-composes current per-author-attributed verified results; never a stored merged snapshot | the link encodes only query params (no result payload) + the `share_link_encodes_query_not_snapshot` acceptance test (KPI-AV-6) |
| I-AV-9 | **Counter shown, not applied** (OD-AV-7): a countered/retracted public verified claim is indexed + discoverable; the counter relationship is annotated when known, never silently filtered/down-weighted | `compose_results`/`annotate_counter_relationship` add an annotation, never remove a row + a `countered_claim_still_appears` acceptance test |

These extend the 12 cross-feature invariants in
`docs/product/architecture/brief.md` + slice-03 I-FED-1..7 + slice-04
I-GRAPH-1..8. They are slice-05-scoped. I-AV-2 is the direct descendant of
I-FED-1 / I-GRAPH-2; promote with the generalizing ADR if a future slice needs it
cross-feature.

## Annotation for software-crafter (DELIVER)

```markdown
## Architecture Enforcement (slice-05 additions)

Style: Hexagonal + Modular Monolith — now TWO binaries (openlore + openlore-indexer)
Language: Rust (functional paradigm, ADR-007 — pure cores: appview-domain + the claim-domain decode helper)
Tools (slice-01..04 + slice-05 additions):
  - cargo-deny (license + bans) — review the index-store/ingest/server/query crates'
    new deps (reqwest already in-workspace; the HTTP server framework axum/hyper +
    a small base58 crate are the only candidates; all must be MIT/Apache-2.0)
  - cargo xtask check-arch — extends no_cross_table_join_elides_author to adapter-index-store;
    adds indexer_holds_no_signing_or_local_store + no_pubkey_seam_in_release_build;
    adds appview-domain to the pure-core allowlist; I-3 covers BOTH binaries
  - cargo xtask check-probes — the 4 new ports' impls must carry non-stub probe() bodies
  - mutation testing (nightly) — extend to crates/appview-domain (ingest_decision +
    compose_results) + the claim-domain decode helper

Rules to enforce (additions to slice-04):
- crates/appview-domain MUST NOT depend on duckdb/tokio/reqwest/std::fs/std::net/
  std::time::SystemTime or any adapter crate (pure-core allowlist)
- ingest_decision MUST call claim_domain::verify + compute_cid (NO second verification path)
- No SQL literal in adapter-index-store aggregates over indexed_claims (GROUP BY/COUNT/SUM
  across authors) without projecting author_did
- The openlore-indexer crate's dep graph MUST NOT include the signing IdentityPort impl,
  adapter-duckdb / the user's StoragePort, or any PDS-write surface
- A release build MUST NOT read OPENLORE_PEER_PUBKEY_HEX_<did> (production uses the real decode)
- IndexedClaim/NetworkResultRow.author_did is Did (not Option); verified_against is never empty
- The CLI crate MUST NOT link adapter-xrpc-query-server / adapter-index-store / adapter-atproto-ingest
- search degraded mode (indexer unreachable) MUST be non-fatal (no hang, no panic, clear message)
```

## Annotation for acceptance-designer (DISTILL)

```markdown
## Slice-05 Observable Contracts (additions to slice-04)

### Network search by dimension — anti-merging at network scale
Every acceptance test driving `openlore search --object/--contributor/--subject` MUST assert:
- The corpus is the NETWORK index (results include authors the user does NOT follow, labeled "(not subscribed)")
- Every result row carries exactly one author_did + numeric confidence + display bucket + cid + [verified]
- Results grouped by author (or by subject under author); NO multi-author "consensus" row
- Two identical-(subject,object) claims by different authors render as TWO rows (network anti-merging)
- --object footer: distinct author count + the no-merge guarantee + the `peer add` pointer
- --contributor footer: "one developer's reasoning trail, not a community consensus" + `peer add`
- Relationship labels: (you) / (subscribed peer) / (unsubscribed cache) / (not subscribed)
- Unknown object/contributor/subject: empty result + near-match suggestion + exit code 0

### Verified-before-index (release gate; KPI-AV-3)
- indexer_rejects_unverified_claim: tampered-signature + CID-mismatch + unsigned fixtures are
  REJECTED at ingest; none enter the index; none appear in any search result
- Every search result carries [verified] by construction (no [unverified] state exists)
- The ingest gate reuses claim_domain::verify (no second verification path)
- Production verification uses the REAL PLC z6Mk... decode (the seam is release-forbidden)

### Trust display + public-data honesty
- verified_marker_is_universal: every result carries [verified]
- --show <cid>: full record + "Signature: VERIFIED against <did>" + "CID recomputed, matches published record"
- --show <cid not in result>: usage error, non-zero exit (distinct from empty search, exit 0)
- public_data_banner_shown: a banner states discovery indexes only PUBLIC signed claims,
  verified before indexing, nothing private read/aggregated

### Anti-merging at network scale (release gate; KPI-AV-2)
- network_result_preserves_attribution: every result row carries one author DID; identical-content-
  different-author = two rows; the index has no merged/consensus row; the share boundary re-composes per-author

### Discovery → federation funnel (KPI-AV-4)
- discovery_follow_reuses_slice03_path: the follow affordance reuses `peer add`; no auto-subscribe;
  no parallel state; after peer add + peer pull the author's claims appear in local graph query
- An already-followed author is labeled (subscribed peer) with NO follow affordance

### Shareable link (KPI-AV-6)
- share_link_encodes_query_not_snapshot: --share emits a stable query-encoding link; opening it
  re-runs the query → current per-author-attributed verified results; never a stored merged snapshot

### Local-first preserved (release gate; KPI-5)
- local_first_preserved: with the indexer down AND network disabled, claim add / offline claim publish /
  graph query ALL succeed; search degrades to a clear local-only message without a fatal error

### Counter shown, not applied (OD-AV-7)
- countered_claim_still_appears: a countered/retracted public verified claim is still discoverable;
  the counter relationship is shown when known, never silently filtered

Reference: feature-delta.md WD-100..WD-110 + WD-111.., OD-AV-1..7 resolutions,
ADR-023..027, design's sections 5.1 + 5.2 + 9. The DISCUSS `# DISTILL: confirm`
flags (search verb vs --network flag; deployment shape; pull-vs-Firehose; the
pubkey-decode mechanism) are RESOLVED: new `search` verb (ADR-027), self-hostable
single binary (ADR-023), pull-based (ADR-024), real PLC z6Mk decode (ADR-026).
```

## Annotation for platform-architect (DEVOPS)

```markdown
## External Integrations Requiring Contract Tests (slice-05)
- OpenLore Indexer query API (XRPC org.openlore.appview.searchClaims over HTTP): consumer-driven
  contract — the `openlore` CLI is the consumer, the indexer's query server is the provider. Pin the
  response shape (every result carries author_did; no merged/consensus object) so a change that drops
  attribution is caught at build time. Recommended: a Pact-style consumer-driven contract in CI.
- Network Author PDS listRecords + PLC Directory DID-document resolution (indexer is the consumer):
  contract tests pinning the record-enumeration + DID-document/publicKeyMultibase shapes the
  verify-before-index gate + the ADR-026 decode depend on. Recommended: consumer-driven contracts
  against recorded fixtures (the hermetic ingest fixtures model these shapes).
Confirm the local-first guardrail (KPI-5): the CLI's compose/sign/local-query path links NO indexer
code + adds NO network dependency; search is the only network verb and degrades gracefully.

## New deployable (the architectural shift)
- openlore-indexer is a NEW self-hostable single binary (ADR-023). Plan a release artifact (ADR-011
  matrix gains one). For the walking skeleton it ships as `cargo run -p openlore-indexer` (serve | ingest);
  a packaged service unit is a future concern.
- The index store (index.duckdb) is RE-BUILDABLE (re-ingest); its "backup" is re-ingest, not a backup target.

## Earned Trust Telemetry Hooks (slice-05 additions)
- New probe failure reasons via tracing `health.startup.refused`:
  - storage.fsync_unhonored                 (index store fsync no-op on container substrate)
  - indexer.capability_boundary_violated    (wired a signing identity or the local store)
  - indexer.ingest_source_unreachable       (the only/required ingest source is down)
  - identity.pubkey_decode_failed           (the real PLC z6Mk decode failed in the probe)

## KPI instrumentation (handed off per outcome-kpis.md DEVOPS section)
- search.discovery.unfollowed_author_hit{dimension, unfollowed_author_count} (KPI-AV-1 north star)
- search.discovery.follow_funnel{discovered_did, time_from_search_to_add} (KPI-AV-4)
- search.share.link_emitted / search.share.link_opened (KPI-AV-6)
- indexer.ingest.verified vs indexer.ingest.rejected{reason: bad_signature|cid_mismatch|unsigned} (KPI-AV-3)
- Index freshness/coverage dashboard: claims indexed, distinct authors indexed, ingest lag
  (the KPI-AV-1 sparsity diagnosis)
- Release-blocking alerts: KPI-AV-2 != 100% (anti-merging), KPI-AV-3 != 100% (verified-before-index),
  KPI-5 regression (offline compose/sign breaks)
- All telemetry is privacy-preserving: structural counts + DIDs the user already saw; NEVER claim contents
  or user-behavior surveillance (the public-data framing does not extend to surveillance)
```
