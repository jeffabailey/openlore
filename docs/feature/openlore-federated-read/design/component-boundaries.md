# Component Boundaries — openlore-federated-read (slice-03) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-27
- **Architect**: Morgan
- **Style**: Hexagonal + Modular Monolith (ADR-009, inherited)
- **Paradigm**: Functional-leaning Rust (ADR-007, inherited)
- **Extends**: `docs/feature/openlore-foundation/design/component-boundaries.md`

This document specifies ONLY the component-boundary deltas for slice-03.
Slice-01's `claim-domain`, `lexicon`, `ports`, `adapter-duckdb`,
`adapter-atproto-did`, `adapter-atproto-pds`, `adapter-system-clock`, and
`cli` crates are inherited unchanged in their slice-01 responsibilities.
Slice-03 EXTENDS several of them and ADDS no new crates.

## Crate layout (unchanged from slice-01)

```
openlore/                          # workspace root
  crates/
    claim-domain/                  # PURE — extended
    lexicon/                       # PURE — extended (one new optional field)
    ports/                         # PURE — extended (one new trait + extensions to existing)
    adapter-duckdb/                # EFFECT — extended (implements new port + adds tables)
    adapter-atproto-did/           # EFFECT — extended (adds resolve_peer)
    adapter-atproto-pds/           # EFFECT — extended (adds peer-read methods)
    adapter-system-clock/          # EFFECT — unchanged
    cli/                           # DRIVER — extended (4 new verbs + 1 flag + OrientationState)
  xtask/                           # extended (new check-arch rule)
```

No new crates. Slice-03 is a deliberately conservative extension; introducing a crate would require its own ADR (Component Inventory in `docs/product/architecture/brief.md`).

## Component contract deltas

### `crates/claim-domain` (PURE) — extensions

**Slice-03 additions to public surface**:

- `pub fn normalize_reason(s: &str) -> String` — NFC-normalize the counter-claim reason. Pure; idempotent.
- `pub fn validate_counter_claim(claim: &UnsignedClaim, lookup: &dyn ClaimLookup, current_user_did: &Did) -> Result<(), ClaimError>` — REJECTS if `references[]` contains a `Counters` entry AND `reason` is None or empty; REJECTS if the target_cid resolves (via `lookup`) to a claim whose `author_did == current_user_did` (self-counter); delegates cycle/self-reference detection to `reference_rules_validate`.
- `pub enum ClaimError` — extended with `CounterReasonMissing | SelfCounter`. Existing variants unchanged.

**Forbidden dependencies** (unchanged): `tokio`, `reqwest`, `duckdb`, `keyring`, `atrium-api`, `std::fs`, `std::net`, `std::time::SystemTime`, any `adapter-*` crate.

**Probe responsibilities** (slice-03 additions):

- Property test: `normalize_reason` is idempotent.
- Property test: two strings with identical NFC form produce equal output.
- Property test: a claim with `reason: None` has the same CID a slice-01-era binary would produce for the same content (CID stability across the slice-01 -> slice-03 upgrade).
- Unit tests: `validate_counter_claim` rejects empty reason, self-counter, missing target.

### `crates/lexicon` (PURE) — extensions

**Slice-03 additions to public surface**:

- `lexicons/org/openlore/claim.json`: adds `reason` to `defs.main.record.properties` as optional `string` with `minLength: 1, maxLength: 1000` (per ADR-015). NOT added to `required[]`.
- `pub struct Claim`: gains `pub reason: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

**Forbidden dependencies** (unchanged): same as `claim-domain`.

**Probe responsibilities** (slice-03 additions to ADR-005 probe):

- Module-level startup probe extended:
  1. Validate `reason` field declaration: type string, minLength 1, maxLength 1000, NOT in required[].
  2. Serde round-trip a sentinel `Claim` with `reason: Some("test")` — byte-equal on deserialize.
  3. Serde round-trip a sentinel `Claim` with `reason: None` — serialized JSON does NOT contain the `"reason"` key.
  4. CID stability: a fixture slice-01 claim from gold fixtures produces the same CID under slice-03's `compute_cid` (no `reason` field present; serialization byte-equal).

### `crates/ports` (PURE) — extensions and new port

**Slice-03 additions to public surface**:

```rust
// Existing IdentityPort — slice-03 extension:
pub trait IdentityPort {
    // ... slice-01 methods unchanged ...
    fn resolve_peer(&self, peer_did: &Did) -> Result<PeerInfo, IdentityError>;
}

pub struct PeerInfo {
    pub did: Did,
    pub handle: String,
    pub pds_endpoint: Url,
    pub verification_methods: Vec<VerificationMethod>,
}

// Existing PdsPort — slice-03 extension:
#[async_trait::async_trait]
pub trait PdsPort {
    // ... slice-01 methods unchanged ...
    async fn list_peer_records(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        cursor: Option<String>,
    ) -> Result<PeerRecordPage, PdsError>;

    async fn get_peer_record(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        rkey: &str,
    ) -> Result<SignedRecord, PdsError>;
}

pub struct PeerRecordPage {
    pub records: Vec<SignedRecord>,    // each is a JSON value validated against org.openlore.claim
    pub next_cursor: Option<String>,
}

// Existing StoragePort — slice-03 extension:
pub trait StoragePort {
    // ... slice-01 methods unchanged ...
    fn query_federated_by_subject(
        &self,
        subject: &str,
    ) -> Result<Vec<FederatedRow>, StorageError>;
}

pub struct FederatedRow {
    pub author_did: Did,                 // LOAD-BEARING: non-Option; compile-error if dropped
    pub author_relationship: AuthorRelationship,
    pub signed_claim: SignedClaim,
    pub source_table: SourceTable,       // Own | Peer (for renderer's "(you)" vs "(subscribed peer)" vs "(unsubscribed cache)")
}

pub enum AuthorRelationship {
    You,
    SubscribedPeer,
    UnsubscribedCache,                   // peer was soft-removed; claims retained
}

pub enum SourceTable {
    Own,                                 // from `claims`
    Peer,                                // from `peer_claims`
}

// NEW port for slice-03:
pub trait PeerStoragePort {
    fn probe(&self) -> ProbeOutcome;

    // Subscription management.
    fn add_subscription(&self, sub: PeerSubscription) -> Result<AddSubscriptionOutcome, PeerStorageError>;
    fn list_active_subscriptions(&self) -> Result<Vec<PeerSubscription>, PeerStorageError>;
    fn lookup_subscription(&self, peer_did: &Did) -> Result<Option<PeerSubscription>, PeerStorageError>;

    // Soft vs hard remove (two distinct transactions per ADR-014).
    fn soft_remove(&self, peer_did: &Did) -> Result<SoftRemoveOutcome, PeerStorageError>;
    fn hard_purge(&self, peer_did: &Did) -> Result<HardPurgeOutcome, PeerStorageError>;

    // Peer-claim CRUD.
    fn write_peer_claim(
        &self,
        peer_did: &Did,            // attribution; LOAD-BEARING
        signed: &SignedClaim,
        fetched_from_pds: &Url,
        fetched_at: DateTime<Utc>,
    ) -> Result<WritePeerClaimOutcome, PeerStorageError>;

    fn get_peer_claim_by_cid(&self, cid: &Cid) -> Result<Option<(Did, SignedClaim)>, PeerStorageError>;
    fn list_peer_claims_by_subject(&self, subject: &str) -> Result<Vec<(Did, SignedClaim)>, PeerStorageError>;
    fn query_peer_referencing(&self, target_cid: &Cid) -> Result<Vec<(Did, Cid, ReferenceType)>, PeerStorageError>;
}

pub struct PeerSubscription {
    pub peer_did: Did,
    pub peer_handle: String,
    pub peer_pds_endpoint: Url,
    pub subscribed_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,  // None while active; soft-remove sets this
}

pub enum AddSubscriptionOutcome {
    Added { subscribed_at: DateTime<Utc> },
    AlreadyExisted { since: DateTime<Utc> },     // idempotent re-subscribe
}

pub struct SoftRemoveOutcome {
    pub was_subscribed: bool,
    pub cached_claim_count: u32,                 // for the user-facing "5 cached peer claims retained" line
}

pub struct HardPurgeOutcome {
    pub was_subscribed: bool,
    pub deleted_peer_claim_count: u32,
    pub preserved_user_counter_claim_count: u32, // for the "your counter-claims survived" UX
}

pub struct WritePeerClaimOutcome {
    pub written: bool,                           // false if CID already present (idempotent re-pull)
}

pub enum PeerStorageError {
    SchemaMismatch,
    Io(std::io::Error),
    DuckDb(/*duckdb error wrapped*/ String),
    SelfAttribution,                             // attempted to write a peer claim with author_did == local user's DID
    AntiMergingInvariantViolated { detail: String },  // defensive: should never fire if probe + check-arch pass
}

pub enum ProbeRefusalReason {
    // ... slice-01 variants ...
    StoragePeerSchemaMismatch,
    StoragePeerSelfAttribution,
    StoragePeerSoftRemoveBleed,                  // soft-remove deleted peer_claims rows
    StoragePeerPurgeIncomplete,                  // hard-purge left orphans
    PdsPeerCidRoundTripFailed,
    IdentityPeerResolutionFailed,
}
```

**Forbidden dependencies** (unchanged): traits may reference `lexicon` and `claim-domain` types. The async runtime (`tokio`, `async_trait`) is permitted ONLY because `PdsPort` is inherently async. `PeerStoragePort` is sync (local DB only); follows `StoragePort`'s sync trait pattern.

**Probe responsibilities**: none (traits don't probe; implementations do).

### `crates/adapter-duckdb` (EFFECT) — extensions

**Slice-03 additions to responsibility**: implement `PeerStoragePort` over the SAME single-file DuckDB store. Manage the slice-03 schema migration v3 (adds `peer_subscriptions`, `peer_claims`, `peer_claim_references`, `peer_claim_evidence`). Maintain a parallel `peer_claims/<did>/<cid>.json` filesystem tree alongside the existing `claims/<cid>.json`. Implement `StoragePort::query_federated_by_subject` as a SQL `UNION ALL` with explicit `author_did` projection (NOT a `JOIN`).

**Slice-03 additions to public surface**:

```rust
// Existing struct gains a new impl:
pub struct DuckDbStorageAdapter { /* unchanged */ }
impl StoragePort for DuckDbStorageAdapter { /* extended with query_federated_by_subject */ }

// New adapter for the new port (may share the underlying connection pool with DuckDbStorageAdapter):
pub struct DuckDbPeerStorageAdapter { /* shares the same DuckDB file */ }
impl PeerStoragePort for DuckDbPeerStorageAdapter { /* see ADR-014 schema */ }
```

**Forbidden dependencies** (unchanged): other `adapter-*` crates.

**Probe responsibilities** (slice-03 additions, per ADR-014):

For `StoragePort` (extended):
1. Schema_version table contains version=3 row (or higher if forward-compat).
2. Cross-store round-trip: write 1 row to `claims` AND 1 row to `peer_claims`, call `query_federated_by_subject(<S>)`, assert exactly 2 result rows with two distinct author_dids and `author_did != ""`.

For `PeerStoragePort` (new):
3. Sentinel peer-claim round-trip: write a peer_claim row, read it back, assert byte-equal on every column especially `author_did`.
4. SelfAttribution rejection: attempt to write a peer_claim with `author_did == identity.author_did()`, assert error.
5. Soft-remove isolation: write 1 subscription + 3 peer_claims rows, call `soft_remove`, assert subscription `removed_at` is now SET and peer_claims rows COUNT unchanged.
6. Hard-purge transaction: same setup, call `hard_purge`, assert subscription gone AND peer_claims rows gone AND filesystem `peer_claims/<did>/` removed (best-effort).
7. fsync honored (inherited from ADR-001).

### `crates/adapter-atproto-did` (EFFECT) — extensions

**Slice-03 additions to responsibility**: implement `IdentityPort::resolve_peer(peer_did)` by fetching the peer's DID document from the PLC directory (for `did:plc:` DIDs) or the HTTP `.well-known/did.json` (for `did:web:` DIDs), parsing it, and returning `PeerInfo`. The resolution uses the SAME `atrium`/PLC client used at `openlore init` for the user's own DID; no new dependency.

**Public surface addition**: `resolve_peer` method on the existing `AtProtoDidAdapter` struct.

**Forbidden dependencies** (unchanged): other `adapter-*` crates.

**Probe responsibilities** (slice-03 additions):

1. `resolve_peer` against a known-good fixture peer DID returns a `PeerInfo` with non-empty `verification_methods`.
2. `resolve_peer` against a deliberately-unresolvable DID (`did:plc:does-not-exist-test`) returns `IdentityError::PeerResolutionFailed` with the underlying transport error in the detail.
3. `resolve_peer` returns the peer's CURRENT PDS endpoint, not a cached one (probe verifies by issuing two resolutions with a forced no-cache-header).

### `crates/adapter-atproto-pds` (EFFECT) — extensions

**Slice-03 additions to responsibility**: implement `PdsPort::list_peer_records(peer_did, peer_pds_endpoint, cursor)` and `PdsPort::get_peer_record(peer_did, peer_pds_endpoint, rkey)`. The peer PDS endpoint is an input parameter (NOT cached on the adapter) per ADR-016 (re-resolve at every pull). Records returned are raw JSON values; signature verification + CID recomputation are NOT this adapter's job — they happen in `claim-domain` (pure) called from `VerbPeerPull` (cli).

**Public surface addition**: two methods on the existing `AtProtoPdsAdapter` struct.

**Forbidden dependencies** (unchanged): other `adapter-*` crates.

**Probe responsibilities** (slice-03 additions, per ADR-014 + ADR-016):

1. `list_peer_records` against a fixture peer DID at a fixture PDS endpoint returns a known-good sentinel record set; each record's CID is re-computed locally and byte-matches the published rkey.
2. `list_peer_records` against an unreachable PDS endpoint returns `PdsError::PdsUnreachable` (NOT a panic, NOT a silent empty result).
3. `get_peer_record` against a fixture rkey returns the same record bytes as the corresponding entry in `list_peer_records` (probe round-trip).

### `crates/adapter-system-clock` — UNCHANGED.

### `crates/cli` (DRIVER) — extensions

**Slice-03 additions to responsibility**: parse the 4 new verbs and 1 new flag via clap; extend `Wiring` to construct `DuckDbPeerStorageAdapter` alongside slice-01 adapters; extend `ProbeGauntlet` to include the new `PeerStoragePort.probe()` + extended adapter probes; dispatch the new verbs; implement the new verb handlers (`VerbClaimCounter`, `VerbPeerAdd`, `VerbPeerPull`, `VerbPeerRemove`) and extend `VerbGraphQuery` for `--federated`; implement `OrientationState` (read/write `~/.config/openlore/identity.toml` keys); extend `TtyIO` with the `--purge` confirmation prompt helper.

**Public surface addition**: `pub fn main() -> ExitCode` is the existing entry point; clap subcommand definitions extend internally. No new public symbols.

**Forbidden dependencies** (unchanged): none — `cli` is the composition root.

**Probe responsibilities** (slice-03 additions, per ADR-013 + ADR-016):

1. After `claim counter` reaches sign-success, the local store contains the counter-claim file regardless of any subsequent step (extends the slice-01 sign-survives-kill probe; same fault-injection pattern).
2. `claim counter` against an already-published counter-claim CID exits 0 with the existing at-uri (idempotency probe).
3. `claim counter` compose preview output contains BOTH the literal "not as truth" AND the literal "counter-claims coexist, never overwrite" — two string-match probes runnable in CI on every release.
4. `peer pull` against zero subscribed peers exits 0 with a "no peers subscribed" line.
5. `peer remove <did>` (soft) leaves `peer_claims` row count unchanged.
6. `peer remove <did> --purge` against a peer with N cached claims AND M user counter-claims deletes the N and preserves the M (WD-25 invariant).
7. First-pull orientation appears EXACTLY ONCE across 3 consecutive `peer pull` invocations.

### `xtask/` (workspace member) — extensions

**Slice-03 additions to responsibility**:

- Add `check-arch` rule `no_cross_table_join_elides_author` (ADR-014). The rule scans the `adapter-duckdb` crate's source for SQL string literals; any literal mentioning BOTH `claims` and `peer_claims` MUST also mention `author_did` in its SELECT projection. Implementation: regex pass over `quote!`/`format!` SQL strings; AST walker if/when sqlx is added.
- Extend `check-probes` to assert `impl PeerStoragePort for <Adapter>` blocks contain non-stub `probe()` bodies (same mechanism as slice-01 for `impl StoragePort for ...`).

**Public surface**: `cargo xtask check-arch` (extended rule set), `cargo xtask check-probes` (extended trait set). Run from CI on every commit.

## Cross-component invariants — slice-03 additions (enforced)

| # | Invariant | Enforced by |
|---|---|---|
| I-FED-1 | NO SQL query in `adapter-duckdb` joins `claims` and `peer_claims` without explicit `author_did` projection (anti-merging invariant; load-bearing for KPI-FED-1 + KPI-FED-2) | `cargo xtask check-arch` (new rule `no_cross_table_join_elides_author`) + integration test `federation_attribution_preserved` |
| I-FED-2 | `peer_claims.author_did` is NEVER NULL AND NEVER empty string (defense-in-depth; schema CHECK constraint backs the application-layer invariant) | DuckDB schema CHECK constraint + `DuckDbPeerStorageAdapter::write_peer_claim` rejects with `PeerStorageError::SelfAttribution` if `author_did == identity.author_did()` |
| I-FED-3 | Every `PeerStoragePort` implementation MUST ship a non-stub `probe()` (extends I-4 to the new port) | `cargo xtask check-probes` (extended) |
| I-FED-4 | `peer remove --purge` is a SINGLE atomic DuckDB transaction; filesystem cleanup happens AFTER COMMIT (best-effort; orphans are harmless and detectable) | Code review + acceptance test `peer_remove_purge_atomic` |
| I-FED-5 | `VerbClaimCounter` MUST invoke `VerbClaimPublish` internals for the publish step; no parallel publish code path (preserves WD-22 + ADR-003 single-publish-path) | Code review + cli probe #6 above |
| I-FED-6 | The `reason` field on `org.openlore.claim` is OPTIONAL at the Lexicon schema level (forward-compat with slice-01) | `lexicon` probe + ADR-015 |
| I-FED-7 | A claim with `reason: None` produces the same CID a slice-01 binary would produce for the same content (CID stability across upgrade) | `claim-domain` property test + lexicon probe #4 |

These extend the 12 cross-feature invariants in `docs/product/architecture/brief.md`; they are slice-03-scoped and do not need promotion to the brief's invariant table (the brief's I-1..I-12 already cover the meta-invariants like pure-core isolation, probe contract, etc., which slice-03 inherits unchanged).

## Annotation for software-crafter (DELIVER)

```markdown
## Architecture Enforcement (slice-03 additions)

Style: Hexagonal + Modular Monolith (inherited from slice-01)
Language: Rust
Tools (slice-01 + slice-03 additions):
  - cargo-deny (license + bans) — unchanged
  - cargo xtask check-arch — adds rule `no_cross_table_join_elides_author`
  - cargo xtask check-probes — adds PeerStoragePort impl checks
  - scripts/check-probes.sh — unchanged pre-commit hook; picks up the new rule

Rules to enforce (additions to slice-01):
- No SQL string literal in adapter-duckdb mentions BOTH `claims` and `peer_claims`
  without ALSO projecting `author_did` in the SELECT list
- Every impl PeerStoragePort for <Adapter> block MUST contain a non-stub probe() body
- claim-domain MUST NOT depend on duckdb (still enforced; slice-03 does not change this)
- claim-domain MAY depend on icu_normalizer or unicode-normalization crate (NFC normalization is pure)
```

## Annotation for acceptance-designer (DISTILL)

```markdown
## Slice-03 Observable Contracts (additions to slice-01)

### Two-prompt contract extends to `claim counter`
Every acceptance test driving `openlore claim counter` MUST assert:
- Compose preview rendered to stdout BEFORE any signing
- Compose preview contains the literal text "not as truth" (inherited from slice-01)
- Compose preview contains the literal text "counter-claims coexist, never overwrite" (NEW in slice-03)
- Compose preview displays the --reason text verbatim, wrapped at 78 cols
- Compose preview displays "counters: <target_cid> (by <peer_did>)" with the peer's DID
- A confirmation prompt is presented (Enter) and signing completes before the publish prompt
- The publish prompt is a SEPARATE [Y/n] beat (NOT fused)

### Anti-merging invariant — `graph query --federated`
Every acceptance test driving `openlore graph query --federated` MUST assert:
- Output groups by author DID (one header per author)
- Every claim row carries author_did + confidence + cid
- NO row labels itself as "merged" or "consensus"
- Two identical-content claims from different authors render as TWO rows
- Footer states the count of authors AND the no-merge guarantee text

### Peer pull — fault isolation
Every acceptance test driving `openlore peer pull` against a multi-peer fixture
with mixed success/failure MUST assert:
- Failed-peer pull does NOT abort other peers' pulls
- Per-record failures (signature invalid, CID mismatch) reject ONLY that record
- Overall exit code is non-zero if ANY peer was skipped OR ANY record was rejected
- pull summary includes per-peer counts (fetched / new / verified / rejected)

### `peer remove --purge` confirmation gate
Every acceptance test driving `openlore peer remove --purge` MUST assert:
- An interactive `[y/N]` prompt is presented BEFORE any deletion
- Answering "n" leaves both `peer_subscriptions` and `peer_claims` unchanged
- Answering "y" deletes peer_claims for that peer AND preserves user's counter-claims
- `--no-tty` mode REFUSES to run the --purge branch (per ADR-013)

### First-pull / first-federated-query / first-counter-claim orientation
Every acceptance test sequence must verify:
- The orientation message fires on the FIRST invocation per install (state lives in
  ~/.config/openlore/identity.toml under federation.first_*_completed_at)
- Subsequent invocations do NOT emit the orientation
- The orientation text contains the load-bearing guidance lines verbatim

Reference: feature-delta.md WD-14..WD-25, ADR-013..ADR-016, design's
sections 5.1 + 5.2.
```

## Annotation for platform-architect (DEVOPS)

```markdown
## External Integrations Requiring Contract Tests (slice-03 additions)

- Peer ATProto PDSes (XRPC over HTTPS, READ paths):
  - Consumed lexicons: com.atproto.repo.listRecords (with cursor),
    com.atproto.repo.getRecord, com.atproto.identity.resolveDid
  - Recommended: extend the existing Pact suite (slice-01) with consumer-driven
    contracts for the peer-read paths.
  - Read paths replay against a recorded fixture from bsky.social public PDS.
  - Adversarial peer fixture (NEW): a wiremock-based XRPC stub that publishes
    deliberately-tampered records. Used by the `peer_tampered_signature_rejected`
    acceptance test (KPI-FED-6 release-gate).
- PLC Directory (HTTP GET):
  - Used to resolve peer DID documents at subscribe AND at every pull.
  - Pact-mockable; fixture replay against plc.directory is sufficient.

## Earned Trust Telemetry Hooks (slice-03 additions)

- New probe failure reasons emitted via tracing `health.startup.refused` events:
  - storage.peer_schema_mismatch
  - storage.peer_self_attribution
  - storage.peer_soft_remove_bleed
  - storage.peer_purge_incomplete
  - pds.peer_cid_round_trip_failed
  - identity.peer_resolution_failed

- KPI instrumentation (handed off per outcome-kpis.md DEVOPS section):
  - claim.counter.published{counter_cid, target_cid, target_author_did, reason_len}
    (KPI-FED-3 — counter-claim publication rate behavioral validation)
  - federation.e2e.duration_seconds histogram from `peer pull` start to first
    `graph query --federated` result (KPI-FED-5)
  - Adversarial fixture for KPI-FED-6 in CI acceptance suite.
```
