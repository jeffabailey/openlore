# CI/CD Pipeline — openlore-foundation (slice-01)

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex
- **Tool**: GitHub Actions (no decision needed — solo dev, repo hosted on GitHub)
- **Branching**: GitHub Flow (per `platform-design.md` §7)

This document specifies the CI/CD pipeline in **prose**. DELIVER
(software-crafter) translates it into `.github/workflows/*.yml`. No YAML is
written here.

## 1. Workflow files (planned)

| File | Trigger | Purpose |
|---|---|---|
| `.github/workflows/ci.yml` | `pull_request: [main]`, `push: [main]` | Per-PR + per-push to main: full commit + acceptance stages |
| `.github/workflows/nightly.yml` | `schedule: cron 02:00 UTC daily`, `workflow_dispatch` | Mutation testing + full substrate matrix dry-run; opens issue on regression |
| `.github/workflows/release.yml` | `push: tags: ['v*']` | Substrate gold-matrix gate, build-release matrix, supply-chain, publish to crates.io + GitHub Releases |

## 2. Toolchain pinning

- **Rust toolchain**: `rust-toolchain.toml` at repo root pins to a stable
  version (e.g., `channel = "1.83.0"` — DELIVER picks the exact version
  that satisfies all crate MSRVs from `technology-stack.md`).
- **MSRV declaration**: each crate in `crates/*` declares `rust-version = "X.Y"`
  in its `Cargo.toml`. The workspace MSRV is the maximum of all crate MSRVs.
- **CI uses `dtolnay/rust-toolchain@stable`** action with the pinned version.
- **Cache**: `Swatinem/rust-cache@v2` keyed on `Cargo.lock`. Optional sccache
  if compile times become a pain point — defer until measured.

Rationale: pin both stable AND MSRV. The substrate matrix exercises both
(see §8). This catches "works on my Rust, breaks on user's older toolchain"
class of regressions.

## 3. Pipeline stages (commit stage)

Target wall-clock: **< 10 minutes per PR** (per `cicd-and-deployment` skill
§Commit Stage). All stages in this section run in CI on every PR and every
push to `main`.

### 3.1 `fmt` (format check)
- **Command**: `cargo fmt --all -- --check`
- **Maps to**: code-quality baseline (no specific ADR; idiomatic Rust)
- **Failure mode**: blocking
- **Local mirror**: pre-commit hook (`cargo fmt --check`)

### 3.2 `lint` (clippy)
- **Command**: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Maps to**: code-quality baseline (ADR-007 functional-Rust idiom enforcement happens here)
- **Failure mode**: blocking
- **Local mirror**: pre-commit hook

### 3.3 `supply-chain` (cargo-deny)
- **Command**: `cargo deny check`
- **Maps to**: ADR-009 §Architecture Enforcement (license + bans); proposed ADR-012 (supply-chain)
- **Config**: `deny.toml` at repo root (DELIVER writes; see §11 for spec)
- **Checks**: licenses (whitelist: MIT, Apache-2.0, BSD-3-Clause, Unicode-DFS-2016 per `technology-stack.md` §License posture), bans (no `openssl-sys`, no `serde_cbor`, no GPL/LGPL), advisories (RUSTSEC database), sources (crates.io + git deps only from trusted orgs).
- **Failure mode**: blocking
- **Local mirror**: pre-commit (cheap subset: `cargo deny check bans`)

### 3.4 `arch-check` (dependency-graph enforcement)
- **Command**: `cargo xtask check-arch`
- **Maps to**: ADR-009 D-11; component-boundaries.md cross-component invariants
- **What it does**: parses `cargo metadata`; asserts `claim-domain` and `lexicon` have NO transitive dep on `tokio`/`reqwest`/`duckdb`/`keyring`; asserts no `adapter-*` depends on another `adapter-*`; asserts only `cli` depends on `adapter-*`.
- **Failure mode**: blocking
- **Local mirror**: pre-push hook

### 3.5 `probe-check` (probe-shape enforcement)
- **Command**: `cargo xtask check-probes`
- **Maps to**: ADR-009 D-10 layer (b); component-boundaries.md probe-responsibility table
- **What it does**: `syn`-based AST walker over every `impl <Port> for <Adapter>` block; rejects stub `probe()` bodies (single-expression `Ok(_)`, `unimplemented!()`, `todo!()`, body containing only `// TODO`).
- **Failure mode**: blocking
- **Local mirror**: pre-commit hook (per ADR-009 §Architecture Enforcement)

### 3.6 `test-unit` (fast tests)
- **Command**: `cargo nextest run --workspace --no-fail-fast`
- **Maps to**: every component's unit tests (per `component-boundaries.md` probe-responsibility tables — most probe tests live as unit tests)
- **Failure mode**: blocking
- **Local mirror**: pre-push hook
- **Coverage measurement**: `cargo llvm-cov nextest --workspace --lcov --output-path lcov.info` then `cargo llvm-cov report --summary-only` against a project-standard floor of **80% line coverage on pure-core crates** (`claim-domain`, `lexicon`, `ports`); adapter crates measured but not gated (their integration is gated separately in §4.3).

### 3.7 `test-property` (proptest)
- **Command**: covered by `cargo nextest run` since proptest tests are
  ordinary `#[test]` functions. NOT split behind a feature flag — running
  property tests every PR is cheap and they're the load-bearing gate for
  canonicalization stability (per ADR-006).
- **Maps to**: ADR-006 Earned Trust ("property-tested AND mutation-tested"); ADR-008 Earned Trust (reference rules property tests)
- **Failure mode**: blocking
- **Coverage**: proptest runs with `PROPTEST_CASES=256` default in CI;
  bumps to `PROPTEST_CASES=4096` in the nightly workflow.

### 3.8 Commit-stage summary

| Stage | Wall-clock target | Type |
|---|---|---|
| fmt | < 10 s | blocking |
| lint | 30-60 s | blocking |
| supply-chain | 30-60 s | blocking |
| arch-check | < 5 s | blocking |
| probe-check | < 5 s | blocking |
| test-unit + property | 2-4 min | blocking |

**Parallelism**: `fmt`, `lint`, `supply-chain`, `arch-check`, `probe-check`
run in a single job (sequential, fast); `test-unit` runs in a separate job
in parallel. Total commit-stage wall-clock target: **~5 minutes**.

## 4. Pipeline stages (acceptance stage)

Target wall-clock: **< 30 minutes per PR** (per `cicd-and-deployment` skill
§Acceptance Stage).

### 4.1 `test-acceptance` (DISTILL handoff)
- **Command**: `cargo nextest run --test acceptance` (assumes DISTILL lands acceptance tests at `tests/acceptance/*.rs` — coordinated with parallel DISTILL agent)
- **Conditional execution**: skip with a green status if `tests/acceptance/` directory is absent. Achieved via a guard step:
  ```
  - name: Check acceptance test directory
    id: check_accept
    run: |
      if [ -d tests/acceptance ]; then echo "exists=true" >> $GITHUB_OUTPUT
      else echo "exists=false" >> $GITHUB_OUTPUT; fi
  - name: Run acceptance suite
    if: steps.check_accept.outputs.exists == 'true'
    run: cargo nextest run --test acceptance
  ```
- **Maps to**: every UAT scenario in `discuss/user-stories.md` and `discuss/gherkin-scenarios-expanded.md`; the two-prompt observable contract (ADR-003)
- **Failure mode**: blocking once the directory exists

### 4.2 `kpi-4-roundtrip` (KPI-4 guardrail integration test)
- **Command**: `cargo nextest run --test kpi_4_roundtrip` (a dedicated integration test that DELIVER writes per Morgan's data-models.md §Validation rules row "Graph query output exactly matches compose-time field values")
- **What it does**: composes a claim with diverse field values (unicode, leading/trailing whitespace, integers, floats at boundary, RFC3339 with non-UTC offset), signs it, writes to DuckDB + on-disk JSON, queries back, asserts field-for-field byte equality.
- **Maps to**: KPI-4 (zero silent normalization); `outcome-kpis.md` Handoff to DEVOPS item 2 (field-mismatch counter)
- **Failure mode**: blocking; this is a GUARDRAIL gate per `outcome-kpis.md` §Guardrails

### 4.3 `kpi-5-offline` (KPI-5 guardrail integration test)
- **Command**: `cargo nextest run --test kpi_5_offline -- --test-threads=1` (network-namespace test; serial)
- **What it does** (on Linux): runs the binary under `unshare -n` (network namespace with no interfaces); asserts `openlore claim add ... --no-tty` produces a signed claim on disk; asserts the subsequent `openlore claim publish` call fails CLEANLY with a non-zero exit and a user-actionable stderr message (per US-003 failure-mode AC).
- **What it does** (on macOS, no network namespaces): falls back to setting `HTTPS_PROXY=http://127.0.0.1:1` to force network failure; weaker but still validates the offline-compose path. Test annotated `#[cfg(target_os = "linux")]` for the strict variant.
- **Maps to**: KPI-5 (local-first invariant); `outcome-kpis.md` §Guardrails
- **Failure mode**: blocking

### 4.4 `test-integration-pds` (mock-PDS integration)
- **Command**: `cargo nextest run --test integration_pds`
- **What it does**: spins up `wiremock` as a mock PDS; exercises `adapter-atproto-pds` write paths against canned XRPC fixtures; verifies idempotency on rkey collision; verifies retry behavior on 5xx.
- **Maps to**: Morgan ADR-004 §probe; architecture-design.md §6.2
- **Failure mode**: blocking

### 4.5 `contract-pact-pds` (Pact contract replay)
- **Command**: `cargo nextest run --test pact_pds`
- **What it does**: replays consumer-driven Pact contracts for `com.atproto.repo.createRecord`, `com.atproto.repo.getRecord`, `com.atproto.repo.listRecords`, `com.atproto.identity.resolveHandle`. Provider is a recorded fixture from `bsky.social`'s public PDS (read paths) + a `wiremock`-driven stub for write paths.
- **Maps to**: component-boundaries.md DEVOPS annotation; architecture-design.md §6.5; proposed `external-contract-tests.md` (this doc, §6)
- **Failure mode**: blocking
- **Real-PDS variant**: a `--against-real-pds` flag (env var `PACT_REAL_PDS=1`) drives the suite against `bsky.social` directly. Runs MANUALLY before a release tag (gated to release workflow only — never per PR; respects bsky.social's rate limits).

### 4.6 Acceptance-stage summary

| Stage | Wall-clock target | Type | Conditional? |
|---|---|---|---|
| test-acceptance | 1-3 min | blocking once exists | yes (skip if dir absent) |
| kpi-4-roundtrip | < 30 s | blocking (GUARDRAIL) | no |
| kpi-5-offline | < 30 s | blocking (GUARDRAIL); Linux-only strict | no |
| test-integration-pds | 1-2 min | blocking | no |
| contract-pact-pds | 30-60 s | blocking | no |

All acceptance-stage jobs run in parallel after commit-stage gates pass.

## 5. Pipeline stages (capacity stage)

**Skip with revisit trigger**: no performance/load/stress tests in slice-01.

- **Rationale**: single-binary CLI driven by 1 user, 1 invocation at a time.
  Capacity is not a quality-attribute driver per Morgan's architecture-design.md §2.
- **Revisit when**: a sibling slice introduces a daemon process (e.g.,
  slice-05 AppView), OR when a user reports the CLI takes >2s for a single
  claim operation (violates KPI-1 implicit budget).

This decision is logged as D-D6 in `wave-decisions.md`.

## 6. Pipeline stages (mutation testing — nightly)

- **Command**: `cargo mutants --package claim-domain --package lexicon --in-diff target-baseline=main` for per-PR optional, OR `cargo mutants --package claim-domain --package lexicon` for full nightly run
- **Trigger**: `schedule: cron 02:00 UTC daily` in `.github/workflows/nightly.yml`
- **Mutation strategy (per Apex Core Principle 9)**: project LOC for slice-01 is well under 50k (pure core ~2-3k lines projected). The principle's per-feature 5-15 min target is technically reachable, but `cargo mutants` overhead on a Rust workspace makes per-PR painful. **Strategy: nightly-delta with per-PR diff variant available via `workflow_dispatch`**.
- **Target**: ≥95% kill rate on `claim-domain` (per ADR-006 Earned Trust); ≥80% on `lexicon` (per Apex Core Principle 9 default).
- **Failure mode (nightly)**: ADVISORY — opens a GitHub issue auto-labeled `mutation-regression` when kill rate drops below threshold. Does NOT block any merge.
- **Failure mode (release tag)**: BLOCKING — release workflow re-runs mutation and refuses to publish if kill rate is below threshold (see §9).

Acknowledged trade-off: nightly mutation creates a ~12-hour feedback lag.
Acceptable for slice-01 because the pure-core surface is small and changes
infrequent. Re-evaluate at slice-04 when the graph-store swap doubles
pure-core LOC.

## 7. Pipeline stages (release tag)

Triggered by `push: tags: ['v*']` per semantic versioning. `release.yml`
workflow.

### 7.1 Pre-release gates (all must pass before any artifact is built)

1. Re-run full commit stage on the tagged ref.
2. Re-run full acceptance stage on the tagged ref.
3. Re-run `cargo mutants` full sweep — BLOCKING on regression vs main baseline.
4. Run `contract-pact-pds` with `PACT_REAL_PDS=1` (real `bsky.social`) — manual approval gate via GitHub Actions environment protection rules (solo dev = self-approve, but the click is the ceremony).
5. Run substrate gold-matrix (8 cells) — see `substrate-matrix.md`. Every cell must pass.

### 7.2 `build-release` matrix

Per Morgan architecture-design.md §7 "Distribution":

| Target triple | OS | Arch | Linker | Use |
|---|---|---|---|---|
| `aarch64-apple-darwin` | macOS | arm64 | system | Apple Silicon |
| `x86_64-apple-darwin` | macOS | x86_64 | system | Intel Mac |
| `x86_64-unknown-linux-gnu` | Linux | x86_64 | glibc | mainstream Linux |
| `aarch64-unknown-linux-gnu` | Linux | aarch64 | glibc | ARM Linux, RPi |

**Windows is deferred** (revisit trigger: ≥3 user requests on GitHub issues
for Windows support, OR slice-05 AppView creates the need).

`x86_64-unknown-linux-musl` was considered for "portable static Linux binary"
per Morgan's note but DEFERRED — `keyring` crate's Linux backend (Secret
Service) requires DBus which complicates musl static linking. Reconsider once
WSL2 fallback file mechanism (per ADR-002) is exercised; users on musl distros
get the WSL2-fallback codepath in the meantime.

### 7.3 `release` (publish step)

- **GitHub Releases**: upload all 4 binaries + their `.sig` cosign signatures + SBOM (CycloneDX JSON, per ADR-012 proposal) + SHA-256 checksums + CHANGELOG excerpt.
- **crates.io**: `cargo publish --package openlore` (only the `cli` crate publishes — workspace's internal crates use `publish = false`).
- **Homebrew tap / AUR / nix flake**: NOT in slice-01 (per Morgan distribution.md §Tertiary). Reserve namespace; ship later.

### 7.4 Release stage summary

Estimated wall-clock for a release tag: **15-30 minutes** (commit re-run + acceptance re-run + mutation full + 8-cell substrate matrix + 4-cell build matrix + publish). Acceptable for an event that fires manually a few times per slice.

## 8. Cache strategy

- **Rust toolchain & target cache**: `Swatinem/rust-cache@v2` with key based on `Cargo.lock` hash + workflow file hash. Cleared automatically per branch.
- **Cargo registry & git cache**: cached at `~/.cargo/registry/cache` and `~/.cargo/git/db` per the same action.
- **`sccache` distributed cache**: NOT used in slice-01 — solo dev, single repo, GitHub-hosted runners; the rust-cache action is sufficient. Revisit if cumulative CI wall-clock exceeds 15 min/PR.
- **Mutation testing cache**: `cargo mutants` writes to `target/mutants/`; this directory is cached keyed on `Cargo.lock` to skip re-compilation of unchanged mutants between nightly runs.

## 9. Quality-gate enforcement summary

| Gate | Pre-PR (local) | PR | Nightly | Release-tag |
|---|---|---|---|---|
| fmt | pre-commit | ✓ blocking | – | ✓ blocking |
| lint (clippy -D warnings) | pre-commit | ✓ blocking | – | ✓ blocking |
| supply-chain (cargo-deny) | pre-commit (bans only) | ✓ blocking | ✓ full | ✓ blocking |
| arch-check | pre-push | ✓ blocking | – | ✓ blocking |
| probe-check | pre-commit | ✓ blocking | – | ✓ blocking |
| unit + property tests | pre-push | ✓ blocking | ✓ proptest_cases=4096 | ✓ blocking |
| coverage (pure-core >= 80%) | – | ✓ blocking | – | ✓ blocking |
| acceptance (when DISTILL ready) | – | ✓ blocking (conditional skip while absent) | – | ✓ blocking |
| KPI-4 round-trip | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| KPI-5 offline (Linux strict) | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| integration (wiremock) | – | ✓ blocking | – | ✓ blocking |
| Pact contracts (mocked) | – | ✓ blocking | – | ✓ blocking |
| Pact contracts (real PDS) | – | – | – | ✓ manual approval gate |
| mutation testing | – | – (optional via dispatch) | ✓ advisory | ✓ blocking on regression |
| substrate matrix (4 cells) | – | – | ✓ | – |
| substrate matrix (8 cells) | – | – | – | ✓ blocking |
| build-release matrix (4 triples) | – | – | – | ✓ blocking |
| SBOM + signatures | – | – | – | ✓ blocking |

## 10. Branch protection rules (recommended for `main`)

DELIVER configures these via repo settings or `.github/settings.yml`:

- Require PR before merging.
- Require status checks to pass: `ci / commit-stage`, `ci / acceptance-stage`.
- Require branches to be up to date before merging.
- Require signed commits (per `cicd-and-deployment` §Branch Protection Rules).
- Require linear history.
- Restrict force-push to nobody on `main`.
- Tag protection: `v*` tags can only be created from `main`.

Solo dev caveat: PR approvals from "other reviewers" are skipped (none
exist); the PR mechanic remains valuable as a self-pause + CI checkpoint.

## 11. `deny.toml` content spec (for DELIVER)

DELIVER writes `deny.toml` at repo root with these sections (per cargo-deny
docs):

```
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-DFS-2016", "ISC", "Zlib"]
confidence-threshold = 0.93

[bans]
deny = [
  { name = "openssl", reason = "rustls is the locked TLS per ADR-004" },
  { name = "openssl-sys", reason = "same" },
  { name = "serde_cbor", reason = "ciborium per technology-stack.md" },
  { name = "actix-web", reason = "no HTTP server in slice-01" },
  { name = "axum", reason = "no HTTP server in slice-01" },
]
multiple-versions = "warn"

[advisories]
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

DELIVER may revise the allow-git list if any chosen crate has a transitive
git dep (e.g., a pre-release `atrium` snapshot); each addition needs a
comment justifying the trust.

## 12. References

- `platform-design.md` (sibling) — pipeline rationale lives there
- `substrate-matrix.md` (sibling) — full matrix definition
- `observability.md` (sibling) — what the binary emits when a probe refuses
- `kpi-instrumentation.md` (sibling) — KPI-4 and KPI-5 test specs
- `distribution.md` (sibling) — release artifacts
- Morgan: `architecture-design.md` §7, §8; `component-boundaries.md` §Cross-component invariants; ADR-009 §Architecture Enforcement
- Apex proposed ADRs: ADR-010 (telemetry-opt-in), ADR-011 (release matrix), ADR-012 (supply-chain)
