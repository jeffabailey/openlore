# Component Boundaries — openlore-foundation (slice-01)

- **Wave**: DESIGN
- **Date**: 2026-05-25
- **Architect**: Morgan
- **Style**: Hexagonal + Modular Monolith (ADR-009)
- **Paradigm**: Functional-leaning Rust (ADR-007 Proposed)

This document specifies each component's responsibility, public contract,
forbidden dependencies, and probe responsibilities. It does NOT specify
internal implementation — software-crafter owns that during DELIVER's
RED -> GREEN -> REFACTOR cycles.

## Crate (component) layout

```
openlore/                          # workspace root
  Cargo.toml                       # workspace manifest
  crates/
    claim-domain/                  # PURE
    lexicon/                       # PURE
    ports/                         # PURE (traits only)
    adapter-duckdb/                # EFFECT
    adapter-atproto-did/           # EFFECT
    adapter-atproto-pds/           # EFFECT
    adapter-system-clock/          # EFFECT
    cli/                           # DRIVER (composition root)
  xtask/                           # workspace member (architecture tests, code gen)
  lexicons/
    org/openlore/claim.json
    org/openlore/philosophy.json
  scripts/
    check-probes.sh                # pre-commit hook
```

## Component contracts

### `crates/claim-domain` (PURE)

**Responsibility**: define the claim value model (unsigned + signed); implement
the pure transformations canonicalize, compute-CID, sign, verify, reference-
rules-validate, confidence-bucket-render. NO I/O.

**Public surface (sketch — exact shape is DELIVER's)**:

- `pub struct UnsignedClaim { subject, predicate, object, evidence, confidence, author_did, composed_at, references }`
- `pub struct SignedClaim { unsigned: UnsignedClaim, signature: SignatureBlock }`
- `pub enum ReferenceType { Retracts, Corrects, Counters, Supersedes }`
- `pub enum ConfidenceBucket { Speculative, Weighted, WellEvidenced, Triangulated }`
- `pub enum ClaimError { OutOfRangeConfidence, SelfReference, CycleDetected, CanonicalizationFailed, InvalidLexiconShape, SignatureFailed, VerificationFailed }`
- `pub fn canonicalize(claim: &UnsignedClaim) -> Result<Vec<u8>, ClaimError>` — RFC 8949 canonical CBOR
- `pub fn compute_cid(canonical_bytes: &[u8]) -> Cid` — CIDv1 dag-cbor sha2-256
- `pub fn sign(unsigned_cid: &Cid, key: &SigningKey) -> SignatureBlock`
- `pub fn verify(signed: &SignedClaim, public_key: &VerifyingKey) -> Result<(), ClaimError>`
- `pub fn reference_rules_validate(claim: &UnsignedClaim, lookup: Option<&dyn ClaimLookup>) -> Result<(), ClaimError>` where `ClaimLookup` is a tiny pure-shaped trait that the storage adapter can implement; pure unit tests pass `None`.
- `pub fn confidence_bucket(numeric: f64) -> ConfidenceBucket` — display-only helper

**Forbidden dependencies**:
- `tokio`, `reqwest`, `duckdb`, `keyring`, `atrium-api`, `atrium-xrpc`
- `std::fs`, `std::net`, `std::process`, `std::env`, `std::time::SystemTime`
- Any `adapter-*` crate

**Probe responsibilities** (per ADR-006 + ADR-008):
- Property test: round-trip canonicalize.
- Gold-fixture CID stability test in CI.
- Self-reference + cycle detection unit tests in `reference_rules_validate`.

### `crates/lexicon` (PURE)

**Responsibility**: hold the `org.openlore.*` Lexicon schemas (JSON files
plus serde-derived Rust models), validate inbound JSON against the schemas,
serve as the canonical type vocabulary for `claim-domain` and adapters.

**Public surface (sketch)**:

- `pub mod claim { pub struct Claim { ... } pub const NSID: &str = "org.openlore.claim"; }`
- `pub mod philosophy { pub struct Philosophy { ... } pub const NSID: &str = "org.openlore.philosophy"; }`
- `pub fn validate_claim_json(value: &serde_json::Value) -> Result<lexicon::claim::Claim, LexiconError>`

**Forbidden dependencies**: same as `claim-domain`.

**Probe responsibilities** (per ADR-005): module-level startup probe that
loads every Lexicon JSON, validates against Lexicon schema-of-schemas, and
asserts serde round-trip byte-equality.

### `crates/ports` (PURE)

**Responsibility**: define trait contracts for every effect adapter, plus the
shared `ProbeOutcome` ADT and the `ClaimLookup` trait imported by
`claim-domain`.

**Public surface (sketch)**:

```rust
pub enum ProbeOutcome {
    Ok,
    Refused { reason: ProbeRefusalReason, detail: String, structured: serde_json::Value },
}

pub enum ProbeRefusalReason {
    StorageFsyncUnreliable,
    StorageSchemaMismatch,
    IdentityKeyPermsUnsafe,
    IdentityKeychainUnreachable,
    IdentityDidDocumentMismatch,
    PdsTlsHandshakeFailed,
    PdsDidMismatch,
    PdsIdempotencyViolation,
    LexiconInvalid,
    LexiconSerdeRoundTripFailed,
    // ...extensible
}

pub trait StoragePort {
    fn probe(&self) -> ProbeOutcome;
    fn write_signed_claim(&self, signed: &SignedClaim) -> Result<(), StorageError>;
    fn read_signed_claim(&self, cid: &Cid) -> Result<Option<SignedClaim>, StorageError>;
    fn query_by_subject(&self, subject: &str) -> Result<Vec<SignedClaim>, StorageError>;
    fn query_referencing(&self, target_cid: &Cid) -> Result<Vec<(Cid, ReferenceType)>, StorageError>;
}

#[async_trait::async_trait]
pub trait PdsPort {
    fn probe(&self) -> ProbeOutcome;                          // sync probe is fine; uses tokio block_on internally if needed
    async fn create_record(&self, collection: &str, rkey: &str, body: serde_json::Value) -> Result<AtUri, PdsError>;
    async fn get_record(&self, collection: &str, rkey: &str) -> Result<Option<serde_json::Value>, PdsError>;
    async fn list_records(&self, collection: &str) -> Result<Vec<serde_json::Value>, PdsError>;
}

pub trait IdentityPort {
    fn probe(&self) -> ProbeOutcome;
    fn author_did(&self) -> &Did;
    fn sign(&self, unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError>;
    fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError>;
}

pub trait ClockPort {
    fn probe(&self) -> ProbeOutcome;     // trivial; always Ok
    fn now_utc(&self) -> DateTime<Utc>;  // RFC3339-serializable
}
```

**Forbidden dependencies**: anything that brings I/O symbols into the trait
definitions themselves. Traits MAY reference types from `lexicon` and
`claim-domain`. The async runtime (`tokio`, `async-trait`) is permitted ONLY
because the `PdsPort` trait is inherently async (network).

**Probe responsibilities**: none; traits don't probe — implementations do.

### `crates/adapter-duckdb` (EFFECT)

**Responsibility**: implement `StoragePort` over an embedded DuckDB single-file
DB at the configured path. Manage schema migrations. Write canonical
signed-claim JSON files alongside the DB.

**Public surface**: `pub struct DuckDbStorageAdapter; impl StoragePort for DuckDbStorageAdapter { ... }`

**Forbidden dependencies**: other `adapter-*` crates.

**Probe responsibilities** (per ADR-001):
1. Schema version match.
2. Sentinel round-trip (write, read, assert byte-equal).
3. `fsync` honored on storage medium (detects tmpfs / overlayfs / WSL2 DrvFs lies).

### `crates/adapter-atproto-did` (EFFECT)

**Responsibility**: implement `IdentityPort` by managing the per-app derived
Ed25519 keypair (stored in OS keychain), exposing `sign()` and `verify()`,
and resolving the user's DID document for verification-method discovery.

**Public surface**: `pub struct AtProtoDidAdapter; impl IdentityPort for AtProtoDidAdapter { ... }`

**Forbidden dependencies**: other `adapter-*` crates.

**Probe responsibilities** (per ADR-002):
1. DID document resolves; OpenLore verification method present.
2. Sentinel sign/verify with the local key matches the DID document key.
3. Keychain accessibility round-trip.
4. WSL2 fallback key file perms = `0600`.

### `crates/adapter-atproto-pds` (EFFECT)

**Responsibility**: implement `PdsPort` over `atrium-api` XRPC calls. Handles
auth refresh, retries, idempotency on rkey collision.

**Public surface**: `pub struct AtProtoPdsAdapter; #[async_trait] impl PdsPort for AtProtoPdsAdapter { ... }`

**Forbidden dependencies**: other `adapter-*` crates.

**Probe responsibilities** (per ADR-004 + section 6.2 of architecture-design):
1. TLS handshake succeeds against configured PDS.
2. `describeServer.did` matches user's PDS DID.
3. rkey-collision idempotency probe: write sentinel twice; assert no overwrite.
   On overwrite detected, refuse to start.

### `crates/adapter-system-clock` (EFFECT)

**Responsibility**: implement `ClockPort` over `std::time::SystemTime` /
`chrono::Utc::now()`.

**Probe responsibilities**: trivial. Always `Ok`. Documented as a degenerate
adapter so the contract symmetry holds.

### `crates/cli` (DRIVER / COMPOSITION ROOT)

**Responsibility**: parse args with `clap`; wire concrete adapters; run the
probe gauntlet; dispatch to verb handlers; render the two-prompt interactive
flow (ADR-003).

**Public surface**: `pub fn main() -> ExitCode { ... }` (entry point).

**Forbidden dependencies**: none — this is the only crate allowed to depend on
every `adapter-*`. By construction this is what makes it the composition root.

**Probe responsibilities** (per ADR-003 + cli component section):
1. After sign-success, kill the process; signed file survives.
2. Re-publish of an already-published CID exits 0.
3. Compose preview contains the literal "not as truth".

### `xtask/` (workspace member, NOT shipped)

**Responsibility**: architecture tests (dependency rules), probe-contract AST
checks, codegen tasks (if any). Run via `cargo xtask check-arch`,
`cargo xtask check-probes`.

**Public surface**: CLI subcommands, internal to development. Not published.

## Cross-component invariants (enforced)

| Invariant | Enforced by |
|---|---|
| `claim-domain` + `lexicon` have NO I/O dependencies | `cargo xtask check-arch` (parses `cargo metadata`) |
| No `adapter-*` depends on another `adapter-*` | `cargo xtask check-arch` |
| Only `cli` depends on `adapter-*` | `cargo xtask check-arch` |
| Every `impl <Port> for <Adapter>` has a non-stub `probe()` | `cargo xtask check-probes` + `scripts/check-probes.sh` pre-commit |
| License whitelist | `cargo deny check licenses` |
| No banned crates (e.g., `openssl-sys`) | `cargo deny check bans` |

## Annotation for software-crafter (DELIVER)

```markdown
## Architecture Enforcement

Style: Hexagonal + Modular Monolith
Language: Rust
Tools:
  - cargo-deny (license + bans)
  - cargo xtask check-arch (dependency-graph rules via cargo metadata)
  - cargo xtask check-probes (AST walker over impl Port for Adapter blocks)
  - scripts/check-probes.sh (pre-commit hook, runs check-probes)

Rules to enforce:
- claim-domain MUST NOT transitively depend on tokio, reqwest, duckdb, keyring
- lexicon MUST NOT transitively depend on I/O crates
- No adapter-* crate depends on another adapter-* crate
- Only cli depends on adapter-* crates
- Every impl <Port> for <Adapter> block MUST contain a non-stub probe() body
- License whitelist: MIT OR Apache-2.0 OR BSD-3-Clause OR Unicode-DFS-2016
```

## Annotation for acceptance-designer (DISTILL)

```markdown
## Two-Prompt Observable Contract (ADR-003)

Every acceptance test driving `openlore claim add` MUST assert:
- Compose preview rendered to stdout BEFORE any signing
- Compose preview contains the literal text "not as truth"
- A confirmation prompt is presented (`Enter`)
- Signing completes and the signed file path is announced BEFORE the publish prompt
- A SEPARATE publish prompt is then presented (`Y/n`)
- The standalone `openlore claim publish <cid>` verb does NOT re-render the compose preview
- All scripts MUST be runnable with `--no-tty` (programmatic Enter+Y)
  and the "not as truth" text MUST still appear in stdout

Reference: feature-delta.md US-001..US-003 + this design's section 5.2.
```

## Annotation for platform-architect (DEVOPS)

```markdown
## External Integrations Requiring Contract Tests

- ATProto PDS (XRPC over HTTPS):
  - Consumed lexicons: com.atproto.repo.{createRecord, getRecord, listRecords},
    com.atproto.identity.resolveHandle, and the verification-method update
    lexicon (or equivalent).
  - Recommended: consumer-driven contracts via Pact in CI acceptance stage.
  - Read paths replay against a recorded fixture from bsky.social public PDS.
  - Write paths verified against a local mock PDS (atrium-test-utils or
    wiremock-based XRPC stub).

## Earned Trust Telemetry Hooks

- Every adapter's probe() failure emits a structured `health.startup.refused`
  event via `tracing` (ADR-009 composition root). DEVOPS should:
  - Capture stderr from the CLI when run with --log-json
  - Ingest health.startup.refused into the OPS dashboard (post-slice-05)
  - Slice-01 ships local emission only; no central aggregation yet.
```
