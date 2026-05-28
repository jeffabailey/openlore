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
| `crates/test-support`        | test-only   | `FakePds`, `FakeKeychain`, `FakeClock`, `TempXdg`, `FakeGithub` hermetic test doubles | slice-01/02  |
| `xtask`                      | dev tooling | `check-arch` (hexagonal invariants), `check-probes` (probe contracts)   | slice-01     |

**Slice-01 ships 8 production crates + 1 test-support crate + 1 xtask binary.
Slice-02 adds 2 production crates (`scraper-domain` + `adapter-github`),
bringing the production count to 10 + 1 test-support + 1 xtask binary.**

Shipped slice extensions:

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

- slice-04 (scoring-graph): may swap or augment `adapter-duckdb` with a graph
  store; revisits ADR-001 / WD-8. Also lands real PLC DID-document multibase
  pubkey decode (slice-03 shipped a test-only peer-pubkey seam per its DV-4) and
  deep cross-repo contributor triangulation (slice-02 shipped a bounded
  aggregate per WD-64).
- slice-05 (appview-search): adds an indexer service (separate binary).

**Crate count: slice-03 was EXTENSION ONLY (zero new crates, WD-26 — 8
production + 1 test-support + 1 xtask). slice-02 adds the first 2 production
crates since slice-01 (`scraper-domain` + `adapter-github`, WD-59), bringing the
cumulative production count to 10 + 1 test-support + 1 xtask binary.**

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

If a future slice needs one of these (I-FED-* or I-SCR-*) enforced
cross-feature, promote it to the table above in the same commit as the ADR that
generalizes it.

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

- ADRs: `docs/adrs/ADR-001-*.md` through `docs/adrs/ADR-019-*.md`
  (ADR-013..016 accepted with openlore-federated-read; ADR-017..019 accepted/
  shipped with openlore-github-scraper; both shipped 2026-05-28)
- Slice-01 evolution: `docs/evolution/openlore-foundation-evolution.md`
- Slice-02 evolution: `docs/evolution/openlore-github-scraper-evolution.md`
- Slice-03 evolution: `docs/evolution/openlore-federated-read-evolution.md`
- Slice-01 architecture design: `docs/feature/openlore-foundation/design/architecture-design.md`
- Slice-02 architecture design:
  `docs/feature/openlore-github-scraper/design/architecture-design.md`
- Slice-03 architecture design:
  `docs/feature/openlore-federated-read/design/architecture-design.md`
- KPI contracts: `docs/product/kpi-contracts.yaml`
- Jobs (JTBD): `docs/product/jobs.yaml`
- CI policy: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- Supply-chain policy: `deny.toml`
