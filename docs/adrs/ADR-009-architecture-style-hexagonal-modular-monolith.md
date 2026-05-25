# ADR-009: Architecture Style = Hexagonal (Ports + Adapters), Modular Monolith, Single-Binary CLI

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

slice-01 is a CLI tool with three external integrations (filesystem, OS
keychain, ATProto PDS) and one embedded database (DuckDB). The walking
skeleton must validate the OpenLore thesis end-to-end while leaving room for
sibling features (scrapers, federation, scoring, AppView) to plug in without
re-architecting the core.

Conway's Law: team = 1 developer (P-001 self). No multi-team coordination
forces a microservice split. Single-binary deployment is optimal.

Quality attributes driving the choice (in priority order, derived from
DISCUSS):

1. **Claim integrity** (cryptographic; KPI-4 guardrail) — drives strong
   isolation of canonicalization and signing from I/O.
2. **Local-first latency** (KPI-1 < 2 min e2e, KPI-5 offline guardrail) —
   drives single-process, no network in the hot path until publish.
3. **Federation interop** (slice-03 dependency) — drives stable on-the-wire
   Lexicon and CID contract NOW.
4. **Auditability** ("Not as truth" framing) — drives explicit pipeline
   visibility (paradigm choice in ADR-007).
5. **Testability** (mutation testing standard in DELIVER) — drives
   pure-core + effect-shell separation.

## Decision

**Architecture style: Hexagonal (Ports and Adapters), implemented as a Modular
Monolith, deployed as a single Rust binary.**

### Component layout

```
openlore (single Rust workspace, single shipped binary)
|
+-- crates/
|     +-- claim-domain/          # PURE: canonicalization, CID, signing,
|     |                          #       retraction-reference rules, confidence
|     |                          #       buckets (display-only helpers)
|     |
|     +-- lexicon/               # PURE: org.openlore.* Lexicon schemas,
|     |                          #       serde models, validation
|     |
|     +-- ports/                 # PURE: trait definitions for all adapters
|     |                          #       (StoragePort, IdentityPort, PdsPort,
|     |                          #       ClockPort) + ProbeOutcome ADT
|     |
|     +-- adapter-duckdb/        # EFFECT: implements StoragePort
|     +-- adapter-atproto-did/   # EFFECT: implements IdentityPort
|     +-- adapter-atproto-pds/   # EFFECT: implements PdsPort (uses atrium-api)
|     +-- adapter-system-clock/  # EFFECT: implements ClockPort
|     |
|     +-- cli/                   # DRIVER: clap-based CLI, composition root,
|                                #         "wire then probe then use"
```

### Dependency rules (enforced — see "Earned Trust" below)

- `claim-domain` MUST NOT depend on any `adapter-*` crate.
- `claim-domain` MUST NOT depend on `tokio`, `reqwest`, `duckdb`, `keyring`,
  `std::fs`, `std::net`.
- `lexicon` MAY depend on `serde`; MUST NOT depend on any `adapter-*`.
- `ports` MAY depend on `lexicon` and `claim-domain`; MUST NOT depend on
  `adapter-*`.
- `adapter-*` crates MAY depend on `ports`, `lexicon`, `claim-domain`; MUST
  NOT depend on each other.
- `cli` is the only crate that depends on every `adapter-*` (composition
  root); it wires adapters into ports and runs the probe sequence.

### Composition root invariant: "Wire then probe then use"

The `cli::main` MUST follow this sequence:

```
fn main() -> ExitCode {
    let cfg = load_config_or_default();

    // 1. WIRE: construct concrete adapters
    let clock      = SystemClockAdapter::new();
    let storage    = DuckDbStorageAdapter::open(&cfg.storage_path)?;
    let identity   = AtProtoDidAdapter::resolve(&cfg.identity)?;
    let pds        = AtProtoPdsAdapter::new(&cfg.pds_endpoint, &identity)?;

    // 2. PROBE: every adapter must demonstrate it can honor its contract
    for probe_outcome in [
        storage.probe(),
        identity.probe(),
        pds.probe(),     // skipped if `--offline` (per KPI-5 guardrail)
        clock.probe(),
    ] {
        if let Err(refused) = probe_outcome {
            emit_health_event(refused);   // structured `health.startup.refused`
            return ExitCode::from(2);     // hard refuse to start
        }
    }

    // 3. USE: dispatch to the requested subcommand
    cli::dispatch(cfg, storage, identity, pds, clock)
}
```

Adapters that fail their `probe()` MUST cause the system to refuse to start.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Layered architecture** (N-tier) | Possible but weaker testability story; the pure-core/effect-shell split is more honest about which parts can be property-tested. |
| **Pure modular monolith without explicit ports** (just modules with internal coupling) | Loses the testability and the "swap DuckDB for Kùzu in slice-04" capability. The cost of port traits is one trait definition per adapter — negligible. |
| **Microservices** (CLI calls a local daemon) | Solo dev, single user; no team scaling pressure; operational complexity is unacceptable. Hard reject per architecture skill defaults. |
| **Single-crate monolith** (no workspace) | Cheaper to start but the dependency-rule enforcement (pure-core has no I/O imports) is harder to verify without crate boundaries. The workspace cost is one `Cargo.toml` per crate. |
| **Plugin/dynamic-library architecture** | Overkill for 1 developer; reconsider only if third parties want to ship adapters. |

## Consequences

### Positive

- Each adapter has one job and can be swapped (DuckDB->Kùzu in slice-04 is a
  swap of one adapter crate).
- Pure core enables aggressive property testing and mutation testing.
- The "wire then probe then use" invariant turns startup into a self-test
  surface; bugs that would have been silent runtime failures become
  startup-time refusals with structured diagnostic events.
- Earned Trust is enforceable: every adapter ships a probe; the probe contract
  is verified by tooling (next section).

### Negative

- Six crates feels heavy for one feature. **Mitigation**: this is a workspace;
  `cargo build` builds them all together; the developer experience is one
  `cargo` command.
- Probe code is non-trivial; some teams treat probes as "we'll add them later"
  and don't. **Mitigation**: the probe contract is enforced at three layers
  (see Earned Trust); a missing probe is a CI failure, not a runtime surprise.

## Architecture Enforcement

**Style**: Hexagonal (Ports + Adapters) + Modular Monolith
**Language**: Rust
**Tool**: `cargo-deny` (license + dependency policy) + custom `xtask`
            architecture-tests crate + pre-commit AST hook
            (`scripts/check-probes.sh` — see below)

**Rules to enforce**:

1. **Pure-core isolation**: `claim-domain` and `lexicon` MUST NOT (transitively)
   depend on `tokio`, `reqwest`, `duckdb`, `keyring`, or use `std::fs |
   std::net | std::process | std::time::SystemTime | std::env`.
   *Enforcement*: `xtask check-arch` parses `cargo metadata` and `cargo deny
   check bans`; runs in CI as the first gate.
2. **Adapter isolation**: no `adapter-*` crate may depend on another
   `adapter-*` crate.
   *Enforcement*: same `xtask check-arch`.
3. **Composition root rule**: only `cli` may depend on `adapter-*` crates.
   *Enforcement*: same `xtask check-arch`.
4. **Probe contract**: every `impl XxxPort for YyyAdapter` block MUST also
   contain a `probe()` method. (Three-layer enforcement, per principle 12.)
   *Enforcement layers* (semantically orthogonal):
   - **Subtype check (compile-time)**: the port trait declares
     `fn probe(&self) -> ProbeOutcome;` as a required method; mypy/Protocol-
     equivalent is Rust's trait method requirement — the compiler refuses to
     accept an `impl` lacking `probe`.
   - **Structural check (pre-commit hook)**: `scripts/check-probes.sh` is an
     AST walker (Rust `syn` crate from an `xtask`) that visits every
     `impl <Port> for <Adapter>` block and asserts `probe()` returns
     `ProbeOutcome`, not a stub `Ok(())`. Forbidden body patterns:
     `Ok(()) // TODO`, single-expression `Ok(())`.
   - **Behavioral check (CI gold-test runner)**: every adapter's `probe()`
     MUST exercise at least one catalogued substrate-lie scenario (e.g.,
     `duckdb-adapter` probes `fsync` on tmpfs/overlayfs; `atproto-pds-adapter`
     probes TLS handshake against a misconfigured endpoint). Gold tests run
     in CI on a matrix of substrates (native FS, tmpfs, overlayfs, WSL2 DrvFs
     stub). A `probe()` that passes against ONLY native FS is a CI failure.

A single-layer bypass is caught by at least one of the other two.

`import-linter` (Python) was investigated and rejected — its contracts are
import-graph only with no API for method-presence enforcement on classes;
the Rust equivalent (`cargo-deny`) covers dependency rules but not method
presence, so we add the `xtask`-based AST walker for that.

## Earned Trust

This ADR's own probe enforcement is itself an Earned Trust contract. Per
principle 12 self-application: there MUST be a probe that verifies adapters
actually implement their probes. The `xtask check-probes` AST walker IS that
meta-probe; it runs in CI on every commit. A change to an adapter that
removes its `probe()` body fails CI before the change can land.

## Revisit Trigger

- A future slice requires hot-pluggable adapters (dynamic loading) — revisit
  for a plugin architecture.
- The crate count grows beyond ~10 — consider grouping adapters under a single
  meta-crate.
