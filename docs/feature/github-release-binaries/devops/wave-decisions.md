# Wave Decisions — DEVOPS — github-release-binaries

- **Wave**: DEVOPS
- **Date**: 2026-07-12
- **Architect**: Apex (nw-platform-architect)
- **Feature**: github-release-binaries (greenfield `release.yml`)
- **Inherits**: ADR-011 (release matrix), ADR-012 (supply-chain), openlore-foundation D-D1..D-D13
- **Unblocks**: homebrew-binary-distribution (parked; consumes ADR-011 tarballs + `.sha256` + cosign artifacts)

This feature designs the release workflow that `ci-cd-pipeline.md §7` narrates
but that has never existed. It reuses ADR-011 + ADR-012 wholesale; no new ADR is
written (a scoped-deferral ADR is *recommended, not written* — see GRB-D9).
Decision IDs use the `GRB-D` prefix.

## Key Decisions

| # | Decision | Rationale | Status | Reference |
|---|---|---|---|---|
| GRB-D1 | Trigger = `push: tags: ['v*']` only; never per-PR/per-push. Trunk-based; tags cut from `main`, no PRs. | `ci.yml` already validates every push to `main`; `release.yml` is disjoint and additive. House rule: commit to `main`, no PRs. | LOCKED | feature-delta [REF] Branching strategy; environments.yaml `trigger` |
| GRB-D2 | Job DAG = `verify` -> `build-release` (matrix ×4) -> `sign-sbom-provenance` -> `publish`. | Linear, blocking `needs` chain: no artifact without green gates, no publish without signatures/SBOM/provenance, publish last (no partial publish). | LOCKED | feature-delta [REF] CI/CD pipeline outline |
| GRB-D3 | `verify` REUSES `ci.yml` commit+acceptance gates on the tagged ref (fmt, clippy, cargo-deny, `cargo test -p xtask` + xtask check-arch, check-probes, nextest `--test-threads=1`). No new gates invented. | Existing-infrastructure-first (Core Principle 2). Matches ci.yml conventions exactly; `--test-threads=1` guards the documented adapter-system-clock env-race. | LOCKED | ci.yml; feature-delta [REF] CI/CD pipeline outline |
| GRB-D4 | 4 native builds, one per native runner (NO cross-compile): macos-14, macos-13, ubuntu-latest, ubuntu-24.04-arm. Build only `cargo build --release -p cli --bin openlore`. | ADR-011 LOCKED matrix + no-cross-compile rejection rationale (native build preserves platform-specific link/test coverage). | LOCKED | ADR-011; environments.yaml `target_environments` |
| GRB-D5 | Supply-chain = cosign keyless (OIDC) `.sig`+`.cert` per tarball; CycloneDX `sbom.cdx.json` via pinned `cargo-cyclonedx`; SLSA via `actions/attest-build-provenance` (L3 target, L2 ok). All first-party / OIDC — NO long-lived secrets. | ADR-012 policy; no key-custody failure mode; small cost (3 actions). | LOCKED | ADR-012; environments.yaml `supply_chain_artifacts` |
| GRB-D6 | Publish to GitHub Releases: 4× tarball + 4× `.sha256` + 4× `.sig` + 4× `.cert` + `sbom.cdx.json` + CHANGELOG excerpt, via `softprops/action-gh-release`. Permissions: `contents: write`, `id-token: write`, `attestations: write`. | ADR-011 channel #2; least-privilege elevation only where OIDC/attest need it. | LOCKED | ADR-011; feature-delta [REF] CI/CD pipeline outline |
| GRB-D7 | Rollback = withdraw-and-recut (delete Release + tag, fix forward on `main`, new patch tag). Never reuse a burned tag. No automated rollback (no live traffic). | Immutable artifacts + Rekor entries can't be mutated; a new patch tag is the only safe forward path. Design-rollback-first (Core Principle 7). | LOCKED | feature-delta [REF] Deployment strategy; environments.yaml DA-4 |
| GRB-D8 | Mutation testing UNCHANGED = per-feature (nightly `cargo mutants`, advisory). Pre-release full-sweep gate DEFERRED. CLAUDE.md NOT rewritten. | Project strategy stays per-feature; the release-blocking mutation job is unbuilt (Changed Assumptions). | LOCKED | feature-delta [REF] Mutation testing strategy |
| GRB-D9 | No new ADR written; ADR-011 + ADR-012 govern. A short scoped-deferral ADR is RECOMMENDED but deferred (TODO marker in workflow suffices for a solo dev). | Avoids ceremony without a second reviewer; the deferral is documented in Changed Assumptions + `# TODO(deferred-slice)` markers. | LOCKED (recommendation logged) | feature-delta [REF] Open questions #5 |
| GRB-D10 | Documented extension point: future `bump-formula` job (`homebrew-binary-distribution` OD-HB-2) slots in with `needs: [publish]` via a placeholder comment at the DAG tail. | Reserves the DAG slot so the future slice adds one job, not a restructure. | LOCKED | environments.yaml `extension_points`; feature-delta [REF] CI/CD pipeline outline |

## Infrastructure Summary

- **Platform**: GitHub Actions (locked). **Branching**: Trunk-Based (commit to
  `main`, no PRs). **Existing infra**: `ci.yml` + `nightly.yml` exist and stay
  unchanged; `release.yml` is greenfield + additive.
- **Deployment target**: GitHub Releases artifact publishing (crates.io
  deferred). No running service, no containers, no orchestration, no external
  observability stack.
- **Monitoring** = release-success signal (complete artifact set present) + a
  failed job blocks publish. No KPI file exists for this infra feature (no
  kpi-contracts delta created).
- **Artifacts**: 4 tarballs (ADR-011 naming) + `.sha256`/`.sig`/`.cert` each +
  release-wide `sbom.cdx.json` + SLSA attestation + CHANGELOG excerpt.

## Constraints

- ADR-011 4-triple matrix + `openlore-{version}-{triple}.tar.gz` naming is a
  LOCKED contract (the Homebrew formula consumes it) — MUST NOT drift.
- ADR-012 first-party/OIDC-only: no long-lived secrets in v1 (rules out
  crates.io publish, which needs `CRATES_IO_TOKEN`).
- Native build per triple (no cross-compile) — except the documented arm64-linux
  fallback, which relaxes it for exactly one cell if the hosted runner is
  unschedulable.
- `Cargo.lock` committed and built-against (ADR-012 anchor).
- Match `ci.yml` conventions exactly (checkout@v4, rust-toolchain@stable,
  rust-cache@v2, install-action@v2, nextest `--test-threads=1`).
- LEAN: only the in-scope pipeline is designed; every deferred item gets a
  one-line note + a future-slice pointer, not a design.

## Upstream Changes (Changed Assumptions)

`ci-cd-pipeline.md §7.1` specifies a heavier pre-release gate set than this
feature builds. **Intentionally deferred** because their infrastructure is
unbuilt; `release.yml` carries `# TODO(deferred-slice: <name>)` markers:

| §7.1 gate | Deferred to slice | Why |
|---|---|---|
| full `cargo mutants` sweep (blocking) | mutation-release-gate | only nightly.yml runs mutation (advisory) |
| 8-cell substrate gold-matrix | substrate-gold-matrix-gate | `substrate-matrix.md` is a doc; no runnable gate |
| Pact vs real bsky (`PACT_REAL_PDS=1`) | pact-real-pds-release-gate | manual-approval real-PDS replay unbuilt |
| crates.io publish (`cargo publish`) | cratesio-publish | needs `CRATES_IO_TOKEN`; OIDC-only v1 |

The in-scope `verify` job DOES re-run §7.1.1 (commit stage) + §7.1.2 (acceptance
stage). Only the four heavy gates above are deferred. This delta refines §7.2
(build matrix) + §7.3 (publish).

**Version drift flagged**: `cli` crate is `version = "0.0.1"`; ADR-011 names the
`0.1.x` series. Before the first `v0.1.0` tag, bump `crates/cli/Cargo.toml` and
have `build-release` assert tag==Cargo-version (GRB-D4 / environments.yaml DA-5).

## Prerequisites (before first release)

1. First `v*` tag cut from `main` (e.g. `v0.1.0`).
2. `Cargo.lock` committed (confirm present).
3. `cli` crate version bumped to match the intended tag; tag==version assertion wired.
4. CHANGELOG source decided — recommend annotated-tag-message (no `CHANGELOG.md` exists).
5. arm64-linux runner (`ubuntu-24.04-arm`) availability confirmed on the account, else pick fallback.

## Handoff

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (nw-functional-software-crafter, ADR-007) | this file + feature-delta.md + environments.yaml + ADR-011 + ADR-012 + ci-cd-pipeline.md §7 | `.github/workflows/release.yml` (the 4-job DAG, matrix, cosign/SBOM/SLSA steps, publish, extension-point placeholder, deferred-gate TODO markers); `cli` version bump + tag==version assertion; annotated-tag CHANGELOG mechanism |
| homebrew-binary-distribution (unblocked) | published tarball names + `.sha256` + cosign artifacts | tap formula (OD-HB-2 `bump-formula` job later added to this release.yml) |

## Changelog

- 2026-07-12 — Apex — initial DEVOPS-wave decisions for github-release-binaries.
  GRB-D1..GRB-D10 LOCKED. No new ADR (ADR-011 + ADR-012 govern; scoped-deferral
  ADR recommended but deferred). Four §7.1 heavy gates deferred with future-slice
  pointers. CLAUDE.md not modified (mutation strategy stays per-feature per GRB-D8).
