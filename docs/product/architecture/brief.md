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
| `crates/test-support`        | test-only   | `FakePds`, `FakeKeychain`, `FakeClock`, `TempXdg` hermetic test doubles | slice-01     |
| `xtask`                      | dev tooling | `check-arch` (hexagonal invariants), `check-probes` (probe contracts)   | slice-01     |

**Slice-01 ships 8 production crates + 1 test-support crate + 1 xtask binary.**

Future slices extend this inventory (planned):

- slice-02 (github-scraper): adds `adapter-github` + `scraper-domain`.
- slice-03 (federated-read): adds `federation` (port + adapter) + `reader-cli`.
- slice-04 (scoring-graph): may swap or augment `adapter-duckdb` with a graph
  store; revisits ADR-001 / WD-8.
- slice-05 (appview-search): adds an indexer service (separate binary).

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

- ADRs: `docs/adrs/ADR-001-*.md` through `docs/adrs/ADR-012-*.md`
- Slice-01 evolution: `docs/evolution/openlore-foundation-evolution.md`
- Slice-01 architecture design: `docs/feature/openlore-foundation/design/architecture-design.md`
- KPI contracts: `docs/product/kpi-contracts.yaml`
- Jobs (JTBD): `docs/product/jobs.yaml`
- CI policy: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- Supply-chain policy: `deny.toml`
