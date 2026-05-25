# Platform Design — openlore-foundation (slice-01)

- **Wave**: DEVOPS (design portion only — production handoff is post-DELIVER)
- **Date**: 2026-05-25
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-foundation (walking-skeleton slice-01)
- **Inherits**: WD-1..WD-13 (DISCUSS), D-1..D-13 (DESIGN), ADR-001..ADR-009
- **Paradigm context**: functional Rust (ADR-007, Accepted)

This is the index document for the DEVOPS-wave platform design. Detailed
specifications are in sibling files; this page describes the shape of the
platform layer, the constraints it operates under, and where each concern
lives.

## 1. Operating model

OpenLore is **local-first** and **solo-developer**. The platform layer:

- Has **no server-side production environment to deploy to** for slice-01.
  The user's machine IS the runtime. "Deployment" means publishing a binary
  release that the user installs.
- Has **no on-call rotation, SLAs, paging, or runbooks for paging**. The
  developer-as-operator debugs their own crash reports.
- Talks to **the user's own ATProto PDS** (typically `bsky.social` or a
  self-hosted PDS). We design the contract; we do NOT operate the PDS.
- Handles **no PII** beyond the user's public DID in slice-01.

The DORA framing remains useful as a *direction*, not a *contract*:

| DORA metric | Target for slice-01 | Mechanism |
|---|---|---|
| Deployment frequency | per release tag (no production fleet) | semver tags drive GitHub Releases + crates.io |
| Lead time for changes | < 1 day commit-to-release | tag-driven release workflow; no manual gates beyond review |
| Change failure rate | tracked via post-release `health.startup.refused` events (opt-in telemetry) | future cohort signal |
| Time to restore | "user re-runs `cargo install openlore@<previous>`" | every prior release stays installable |

## 2. Constraint Impact Analysis (per platform-engineering-foundations)

| Constraint | Source | % delivery affected | Priority |
|---|---|---|---|
| Local-first invariant (KPI-5 guardrail) | DISCUSS WD-3, KPI-5 | 100% (gates every release) | HIGH |
| Claim-integrity zero-tolerance (KPI-4 guardrail) | DISCUSS KPI-4 | 100% (gates every release) | HIGH |
| Pure-core has zero I/O imports | DESIGN ADR-009, D-11 | every PR (CI gate) | HIGH |
| No telemetry endpoint operated by us | DEVOPS choice | KPI-1/2 data collection limited to local-only | MEDIUM |
| ATProto PDS is third-party | Architecture s.6.2 | every release (contract drift risk) | MEDIUM |
| Solo developer, no on-call | Persona P-001 | observability scope (no paging) | INFORMATIONAL |

**Constraint-Free Baseline**: a stable release cadence of "ship when green"
is possible from day one (no environment-promotion ceremony, no change-advisory
boards). The dominant gating concerns are correctness gates, not operational
gates.

**Decision Rule applied**: the local-first and claim-integrity constraints
each affect >50% of delivery, so both are first-class CI gates (see
`ci-cd-pipeline.md` stages `kpi-4-roundtrip` and `kpi-5-offline`).

## 3. What the platform layer ships in slice-01

| Concern | Document | Key decision |
|---|---|---|
| CI/CD pipeline (lint, test, mutation, supply-chain, arch enforcement, release) | `ci-cd-pipeline.md` | GitHub Actions; 11 stages; one workflow per trigger class |
| Observability (logs, metrics, "traces skipped with revisit", health checks) | `observability.md` | `tracing` + structured JSON to a local rolling file; telemetry opt-in default OFF |
| KPI instrumentation (per-KPI mapping to data source + read mechanism) | `kpi-instrumentation.md` | All 6 KPIs are CLI-readable; no dashboards |
| Distribution (cargo install + GitHub Releases binaries) | `distribution.md` | Primary: crates.io; secondary: signed binaries on 4-platform matrix |
| Substrate gold-test matrix (per Morgan's ADR-009 §Architecture Enforcement) | `substrate-matrix.md` | 8-cell matrix for release tag gating; 4-cell subset per PR |
| DEVOPS wave decisions (numbered D-D1..D-Dn parallel to Morgan's D-series) | `wave-decisions.md` | All D-D1..D-D9 locked; ADRs 10-12 proposed |

## 4. Simplest Solution Check (per cicd-and-deployment skill)

Before committing to the platform shape above, three simpler alternatives
were considered and rejected:

### Alternative 1: "Just `cargo install` and call it done"
- **What**: no CI/CD beyond `cargo test` on PR; no release matrix; users build from source.
- **Expected Impact**: meets ~30% of requirements (no enforcement of KPI-4, KPI-5, architecture rules, supply-chain hygiene; no signed binaries; no contract tests).
- **Why insufficient**: KPI-4 + KPI-5 are guardrails — a release that silently regresses them is unacceptable per DISCUSS Outcome KPIs §Guardrails. CI must enforce them.

### Alternative 2: "CI on PR; manual release"
- **What**: lint + test on PR; release artifacts built by hand on the developer's laptop and uploaded manually to GitHub Releases.
- **Expected Impact**: meets ~60% of requirements (covers correctness gates but not reproducibility, signing, or supply-chain).
- **Why insufficient**: violates supply-chain hygiene (no SBOM, no provenance) and the substrate gold-matrix invariant (laptop-built binaries don't exercise the 8-cell matrix). Also: solo dev = single point of failure for the release ceremony.

### Alternative 3: "Build a managed service for the binaries"
- **What**: containerize, deploy to a cloud, build a CDN.
- **Expected Impact**: <0% (over-engineered).
- **Why rejected**: there is no service. The binary runs on the user's machine. Adding a service to ship a non-service is the textbook YAGNI violation.

The chosen shape (GitHub Actions + crates.io + matrix binaries + Pact + gold-matrix) is the minimum that satisfies the KPI guardrails and the architecture-enforcement rules.

## 5. Local quality gates (per cicd-and-deployment skill §Local Quality Gates)

OpenLore is a solo-dev project; the "remote commit stage" the local gates
mirror is the `commit-stage` job in `ci-cd-pipeline.md`. The intent is
"developer never pushes a commit that CI will reject for a trivial reason."

| Gate | Trigger | Checks | Tool |
|---|---|---|---|
| Pre-commit | `git commit` | `cargo fmt --check`; `cargo clippy -- -D warnings`; `cargo xtask check-probes`; `cargo deny check bans` (cheap subset) | `lefthook` (recommended) or `pre-commit` framework |
| Pre-push | `git push` | `cargo nextest run` (unit + property); `cargo xtask check-arch` | same hook tool, `pre-push` stage |
| Local CI mirror | manual | `cargo xtask ci-local` (a thin wrapper that runs every gate in order) | `xtask` workspace member |

**Design principle**: local hooks call the SAME `cargo`/`xtask` commands as
CI. There is exactly one canonical command per gate; CI and local both
invoke it. No duplication of logic.

Mutation testing is **excluded from local hooks** — too slow (minutes per
crate). It runs on the nightly CI cadence (per Apex Core Principle 9; LOC
< 50k => per-feature is theoretically allowed but mutation runs against
`claim-domain` only and the loop is too slow to live in pre-push). See
`ci-cd-pipeline.md` for the nightly schedule.

DELIVER (software-crafter) implements the actual hook config (the
`.lefthook.yml` or `.pre-commit-config.yaml`) — this design specifies the
gates, not the YAML.

## 6. Quality-gate inventory (per cicd-and-deployment §Gate Taxonomy)

| Category | Where | Type | What it gates |
|---|---|---|---|
| Local | pre-commit | blocking (developer) | format, lint, probe-shape, bans |
| Local | pre-push | blocking (developer) | unit + property tests, arch rules |
| PR | GitHub Actions on `pull_request` | blocking (merge) | all of commit + acceptance stages; reviewer approval |
| CI | commit-stage job | blocking (pipeline) | build, fmt, clippy, unit, property, deny, arch, probes |
| CI | acceptance-stage job | blocking (pipeline) | integration, Pact contract replay, KPI-4 round-trip, KPI-5 offline |
| CI (nightly) | mutation-stage | advisory, opens issue on regression | `cargo mutants` on `claim-domain` |
| CI (release) | release-tag workflow | blocking (release) | substrate gold-matrix (8 cells), build-release matrix, supply-chain SBOM, signing |
| Production (post-install) | first-run `probe_all` | blocking (binary refuses to start) | adapter probes (ADR-009 §wire-probe-use) |
| Production (post-install, advisory) | `health.startup.refused` log line | advisory | user reads it from local log; opt-in telemetry counts aggregate |

There is no "deploy-time canary" gate because there is no fleet.
The `probe_all` startup gate is functionally the "post-deploy smoke test"
for this project — it runs on every user's machine, every invocation.

## 7. Branching strategy (per cicd-and-deployment §Branch and Release Strategies)

**Selected**: **GitHub Flow** (feature branches from `main`, PR + review,
merge to `main`, releases via tags from `main`).

- Pipeline triggers: `pull_request` on any branch, `push` to `main`, `tags: ['v*']`.
- `main` is always releasable; tagging is the release ceremony.
- Solo dev means "reviewer approval" is "self-review" — but the PR mechanic
  enforces a pause/checkpoint that catches WIP commits before they ship.

Rejected: GitFlow (no parallel release lines needed), trunk-based with
direct-to-main commits (loses the PR checkpoint), release branching (no
multi-version support burden for slice-01).

If a sibling-feature wave introduces multi-version support (e.g., slice-04
ships a graph-store swap that needs a long-tail v0.x line), revisit at that
point — flagged in `wave-decisions.md` D-D7.

## 8. Risk register (platform-layer specific)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `atrium-api` pre-1.0 breaking changes between releases | HIGH | acceptance suite breaks on dep update | Pact contract tests pin to consumed lexicons; CI catches breakage before merge |
| PDS implementation drift (e.g., `bsky.social` changes idempotency behavior) | MEDIUM | live publish breaks; probe catches it | `health.startup.refused{reason: pds.idempotency_violation}` per Morgan ADR-002 §probe; user sees actionable error |
| Substrate-lie (fsync on tmpfs, WSL2 DrvFs) | MEDIUM | data corruption silently | gold-matrix exercises all 4 substrate cells per release |
| Mutation test wall-clock too slow to be useful | MEDIUM | dev ignores nightly issue, kill rate erodes | Scoped to `claim-domain` only (~2-5 min target); see ADR-006 |
| Supply-chain compromise (typosquat, malicious release) | LOW (but tail-risk HIGH) | shipped binary compromised | `cargo deny check` advisories; pin `Cargo.lock`; lockfile committed; signed releases (sigstore/cosign — see ADR-012 proposal) |
| User on Windows reports broken behavior | MEDIUM | unsupported platform | Windows explicitly out-of-scope for slice-01; documented in `distribution.md` |

## 9. Handoff to DELIVER (software-crafter)

Files DELIVER will translate from spec into code/config:
- `ci-cd-pipeline.md` → `.github/workflows/{ci.yml, release.yml, mutation.yml}`
- `observability.md` → `tracing-subscriber` init code in `crates/cli/src/observability.rs`
- `kpi-instrumentation.md` → event-emission points across `crates/cli/` and `crates/claim-domain/`
- `distribution.md` → `Cargo.toml` package metadata + `xtask/release` helper
- `substrate-matrix.md` → CI workflow matrix definitions

Files DELIVER does NOT translate (DEVOPS owns post-DELIVER):
- The crates.io token rotation procedure (post-release-tag, manual)
- The GitHub Releases artifact-signing key custody (proposed ADR-012)

## 10. References

- `docs/feature/openlore-foundation/feature-delta.md`
- `docs/feature/openlore-foundation/discuss/outcome-kpis.md`
- `docs/feature/openlore-foundation/design/architecture-design.md`
- `docs/feature/openlore-foundation/design/technology-stack.md`
- `docs/feature/openlore-foundation/design/component-boundaries.md`
- `docs/feature/openlore-foundation/design/data-models.md`
- `docs/feature/openlore-foundation/design/wave-decisions.md`
- `docs/adrs/ADR-001-local-storage-duckdb.md` through `docs/adrs/ADR-009-architecture-style-hexagonal-modular-monolith.md`
- Sibling DEVOPS docs: `ci-cd-pipeline.md`, `observability.md`, `kpi-instrumentation.md`, `substrate-matrix.md`, `distribution.md`, `wave-decisions.md`
- Proposed ADRs: `docs/adrs/ADR-010-telemetry-opt-in-policy.md`, `docs/adrs/ADR-011-release-matrix-and-channels.md`, `docs/adrs/ADR-012-supply-chain-policy.md`
