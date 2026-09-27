<!-- markdownlint-disable MD013 -->
# Evolution: github-release-binaries (the `v*`-tag release pipeline: 4 prebuilt `openlore` tarballs, cosign keyless, CycloneDX SBOM, SLSA provenance, published to GitHub Releases)

> Feature archive, finalized retroactively on 2026-09-27. The feature shipped on
> 2026-07-13 as `v0.1.0`. The workspace `docs/feature/github-release-binaries/`
> (`feature-delta.md`, `environments.yaml`, `devops/wave-decisions.md`) is still
> the source of truth for design detail. This file is the post-mortem summary.
> Governing ADRs: **ADR-011** (release matrix and tarball naming) and **ADR-012**
> (supply-chain policy). No new ADR was written (GRB-D9).
>
> **Process note.** Only the DEVOPS wave ran through nWave (Apex, 2026-07-12).
> This was an infrastructure feature with no preceding DISCUSS or DESIGN delta.
> DISTILL and DELIVER were **not** run through nWave: `release.yml` was written
> directly from the DEVOPS handoff spec and then hardened by iterating release
> candidates (`v0.1.0-rc1` to `rc6`). So there is **no** `deliver/execution-log.json`,
> roadmap, DES trace, acceptance corpus or mutation report. The verification evidence
> that does exist is listed under "Verification evidence" below: GitHub Actions run
> history and the published `v0.1.0` Release assets.

## Summary

Before this feature openlore had no release workflow. `ci-cd-pipeline.md §7` described
one, but `.github/workflows/release.yml` did not exist, the repo had no tags, and there
were no GitHub Releases. This blocked the parked `homebrew-binary-distribution` feature,
whose formula needs the ADR-011 tarball names and `.sha256` companions (see that
feature's CA-1 / OQ-D-1).

`github-release-binaries` adds `release.yml`, triggered only on `push: tags: ['v*']`.
Its DAG is `verify` → `build-release` (matrix ×4, `fail-fast: false`) → `release`
(cosign keyless `.sig`/`.cert` per tarball, CycloneDX `sbom.cdx.json`, SLSA
`attest-build-provenance`, `softprops/action-gh-release`) → `bump-formula`. The
`bump-formula` job was later added by the homebrew feature, in the extension point
reserved here. The `cli` crate was bumped `0.0.1 → 0.1.0`. `build-release` checks that
the tag version matches the `cli` Cargo version (DA-5).

It shipped as **`v0.1.0` on 2026-07-13** (run 29261084943, all 7 jobs green). That
Release has 17 assets: 4× `openlore-0.1.0-{triple}.tar.gz`, 4× `.sha256`, 4× `.sig`,
4× `.cert` and `sbom.cdx.json`. This is exactly the release-success signal the DEVOPS
monitoring contract defined.

### Business context / job

This is an enabling feature. It has no JTBD of its own and no `outcome-kpis.md`. The
DEVOPS delta says so explicitly: it publishes immutable artifacts, not a service. Its
value is that it (a) opens ADR-011 channel #2 (GitHub Releases tarball + cosign verify)
for macOS and Linux users who do not want a Rust toolchain, and (b) unblocks J-006
(`homebrew-binary-distribution`), which consumes its tarballs and checksums.

### Wave timeline

| Wave    | Date       | Owner / mode |
|---------|------------|--------------|
| DISCUSS | —          | Not run (infrastructure feature; the requirement came from ADR-011/012 and the homebrew CA-1 finding) |
| DESIGN  | —          | Not run; the DEVOPS delta is the feature's first design artifact |
| DEVOPS  | 2026-07-12 | Apex (nw-platform-architect), commit `e88ab15` |
| DISTILL | —          | **Not run through nWave** |
| DELIVER | 2026-07-12 → 2026-07-13 | **Implemented directly** (no nWave DELIVER): `13602a2` plus 5 hardening fixes driven by rc1–rc6 |

## Key decisions (from `devops/wave-decisions.md`)

| # | Decision | Outcome |
|---|---|---|
| GRB-D1 | Trigger on `push: tags: ['v*']` only; trunk-based, tags cut from `main` | Shipped as designed |
| GRB-D2 | 4-stage DAG `verify → build-release ×4 → sign-sbom-provenance → publish` | **Deviated**: sign/SBOM/provenance and publish were merged into one `release` job (see Deviations #1) |
| GRB-D3 | `verify` reuses the `ci.yml` commit and acceptance gates and adds no new ones | Shipped. A pre-build step for the `openlore` and `openlore-indexer` bins was added later (Deviations #6) |
| GRB-D4 | 4 native builds (macos-14, macos-13, ubuntu-latest, ubuntu-24.04-arm) with no cross-compile | **Deviated**: `x86_64-apple-darwin` is cross-compiled on macos-14 (Deviations #2) |
| GRB-D5 | cosign keyless, CycloneDX SBOM from a pinned `cargo-cyclonedx`, SLSA provenance; OIDC only, no long-lived secrets | Shipped. The SBOM tool is not version-pinned and the SBOM is scoped to the `cli` package (Deviations #4) |
| GRB-D6 | Publish 4× tarball/.sha256/.sig/.cert, the SBOM and a CHANGELOG excerpt | Artifacts shipped. **The CHANGELOG excerpt did not** (Deviations #5) |
| GRB-D7 | Rollback by withdraw-and-recut; never reuse a burned tag | Adopted as policy. Never exercised on a GA tag. The rc tags were withdrawn and recut, which follows the same pattern |
| GRB-D8 | Mutation strategy unchanged (per-feature, nightly); pre-release mutation gate deferred | Shipped as designed (no mutation in `release.yml`) |
| GRB-D9 | No new ADR; a scoped-deferral ADR recommended but deferred | Still unwritten (see Deferred) |
| GRB-D10 | Reserve the `bump-formula` extension point (`needs: [publish]`) | Reserved, then consumed by homebrew slice-02 (`a79d2d1`) as `needs: release` |

## What shipped

| Commit | Date | What |
|---|---|---|
| `e88ab15` | 2026-07-12 | DEVOPS design: `feature-delta.md`, `environments.yaml`, `devops/wave-decisions.md` |
| `13602a2` | 2026-07-12 | **`feat`**: `.github/workflows/release.yml` (276 lines); `crates/cli` bumped `0.0.1 → 0.1.0` |
| `60be201` | 2026-07-12 | `style`: `cargo fmt --all` to fix stable-rustfmt drift (rc1 failed `verify` at `cargo fmt --check`) |
| `85279d7` | 2026-07-12 | `fix(clippy)`: stable-clippy drift, so `-D warnings` passes |
| `6602f36` | 2026-07-12 | `fix(test-support)`: resolve the cross-package `openlore-indexer` bin without `CARGO_BIN_EXE` |
| `6500d2c` | 2026-07-12 | `fix(ci)`: pre-build the `openlore` and `openlore-indexer` bins before nextest, in **both** `release.yml` and `ci.yml` (rc2/rc3 failed `verify` at nextest) |
| `5cbc93f` | 2026-07-13 | `fix(release)`: cross-compile `x86_64-apple-darwin` from macos-14 (rc4's macos-13 cell sat QUEUED for about 11h and was cancelled) |
| `0aee783` | 2026-07-13 | `fix(release)`: `cargo-cyclonedx` 0.5.x rejects `--package`, so use `--manifest-path` + `--override-filename` (rc5 failed at the SBOM step) |
| tag `v0.1.0` → `a79d2d1` | 2026-07-13 | First GA release. All 4 builds, sign/SBOM/provenance/publish, and `bump-formula` green |

### Verification evidence (in place of the DELIVER quality gates)

| Evidence | Result |
|---|---|
| Release-candidate burn-down (`gh run list --workflow release.yml`) | rc1 fail (fmt) → rc2 fail (nextest) → rc3 fail (nextest) → rc4 cancelled (Intel-mac cell queued ~11h) → rc5 fail (SBOM step) → **rc6 success** → **v0.1.0 success** (52 min) |
| `v0.1.0` Release assets | 17/17 present: 4 tarballs, 4 `.sha256`, 4 `.sig`, 4 `.cert`, `sbom.cdx.json`. `isPrerelease: false` |
| Tag == Cargo version (DA-5) | Asserted in every `build-release` cell. It allows a `-rcN` suffix by comparing the base version |
| Checksum self-check | Each cell runs `shasum -a 256 -c` on its own `.sha256` before upload |
| Downstream consumer proof | The homebrew formula installs the v0.1.0 tarballs sha256-verified. The formula-smoke run 29297512108 is green on macos-14 and ubuntu-latest (see the homebrew evolution doc) |
| Coexistence | `ci.yml` and `nightly.yml` triggers are unchanged and disjoint. `ci.yml` was edited once (the shared pre-build fix `6500d2c`, plus `--no-fail-fast`), so the "`ci.yml` unchanged" assumption in `environments.yaml` no longer strictly holds |
| Not available | No acceptance tests for the workflow, no mutation report, no DES trace, no peer-review record for DELIVER |

## Deviations: planned (DEVOPS) vs shipped

| # | Planned | Shipped | Disposition |
|---|---|---|---|
| 1 | Separate `sign-sbom-provenance` and `publish` jobs (GRB-D2) | One `release` job that signs, generates the SBOM, attests and publishes | Deliberate, documented in the `release.yml` header ("artifact-locality": avoids re-uploading `.sig`/`.cert` between jobs). DA-3 (no partial publish) still holds because this is the only job with `contents: write` and it runs after all 4 builds |
| 2 | Native build per triple; `x86_64-apple-darwin` on `macos-13` (risk: *medium*, fallback: *none*) | Cross-compiled from `macos-14` with `--target x86_64-apple-darwin`. All 4 cells now pass an explicit `--target` | **This relaxes ADR-011's no-cross-compile rule for one cell.** The design had the risk backwards: the arm64-linux cell (rated *high*) ran natively without trouble, and the Intel-mac cell (no fallback designed) was the one that blocked. **ADR-011 has not been amended** and the v0.1.0 Release body does not mention the cross-compile, although `5cbc93f` said it would. Open item |
| 3 | Top-level `contents: write`, `id-token: write`, `attestations: write` | Top-level `contents: read`. Only `release` elevates (and `bump-formula` gets `contents: write`) | Improvement: tighter least-privilege than designed |
| 4 | `cargo-cyclonedx` pinned; release-wide SBOM | Installed unpinned via `taiki-e/install-action` (0.5.9 when fixed). The SBOM is the `cli` package's tree (`crates/cli/sbom.json` → `sbom.cdx.json`, CycloneDX 1.3, about 309 components) | Unpinned tooling already broke rc5. Pinning is an open item. Scoping to `cli` matches the shipped binary, so it is arguably more accurate than "release-wide" |
| 5 | CHANGELOG excerpt taken from the annotated tag message (Pre-req 4, `changelog_source`) | Static body (verify commands) plus `generate_release_notes: true`. The annotated tag message is **not** included. The inline comment "the tag message body leads" is inaccurate: the v0.1.0 body has no tag-message text | Deviation. The "Full Changelog" link in the release notes points at `v0.1.0-rc6...v0.1.0`, and the `rc6` tag no longer exists on the remote |
| 6 | `verify` mirrors `ci.yml` exactly | Adds `cargo build --bin openlore --bin openlore-indexer` before nextest (and `ci.yml` got the same fix) | This fixed a CI gap the release exposed: the local `target/debug` had hidden a missing bin. Kept in step with `ci.yml` |
| 7 | `actions/attest-build-provenance@v1` | `@v2` | Minor version drift; no contract change |
| 8 | `# TODO(deferred-slice: …)` markers inline | A single `DEFERRED` block in the workflow header lists all four future slices | Same information, placed differently |
| 9 | Tag == Cargo version, exact match | Base-version match (`0.1.0-rc6` passes against `0.1.0`); tags containing `-` are auto-marked pre-release | Needed for RC validation; genuine drift is still caught |

## Deferred items / open questions

- **ADR-011 amendment** for the `x86_64-apple-darwin` cross-compile (Deviation #2). ADR-011 line 31 still says "Each binary is built on its native runner".
- **Pin `cargo-cyclonedx`** (Deviation #4). An unpinned tool upgrade already broke one RC.
- **Release notes from the tag message** (Deviation #5). Either wire in the annotated-tag body or drop the claim from the workflow comment.
- **Four §7.1 heavy gates** are still deferred as designed: `mutation-release-gate`, `substrate-gold-matrix-gate`, `pact-real-pds-release-gate`, `cratesio-publish` (the last needs `CRATES_IO_TOKEN` custody under ADR-012).
- **Scoped-deferral ADR** (GRB-D9 / OQ #5). Still recommended, not written.
- **Rollback (withdraw-and-recut)** has never been rehearsed on a GA tag. The rc series withdrew tags and Releases, but rollback was not tested as a deliberate procedure.
- **DA-1 idempotency** (re-running the same tag must not double-publish) has not been tested explicitly.
- **Out of matrix** per ADR-011: Windows (`x86_64-pc-windows-msvc`), musl.
- **Consumer-side verification** of `cosign verify-blob` against the published `.cert` has not been recorded as run by a consumer. The command is in the Release body.

## Lessons learned

1. **Local state hides CI gaps.** `openlore-indexer` was present in the local `target/debug` from earlier builds, so the acceptance suites passed locally and failed on a fresh CI checkout (rc3). The release `verify` job was the first clean-checkout run of the full suite on a tag, and it caught a gap `ci.yml` also had.
2. **Stable-toolchain drift is a release risk.** rc1 and rc2 failed on rustfmt and clippy drift, not on release logic. Because `rust-toolchain.toml` pins `channel = "stable"`, a tag can fail `verify` even when no code changed since the last green `main`.
3. **Rate runner-availability risk from real behaviour, not from documentation.** The design rated the new arm64-linux runner *high* risk and the Intel-mac runner *medium* with no fallback. The opposite happened. Any scarce or deprecation-tracked runner should get a designed fallback.
4. **Pin tools the pipeline shells out to.** The `cargo-cyclonedx` 0.5.x CLI change (`--package` rejected, output renamed) cost one RC.
5. **An RC ladder is cheap and effective for a greenfield release workflow.** Six RC tags found five independent defects before the GA tag. Tags containing `-` are auto-marked pre-release, and `bump-formula` is GA-gated, so the rcs never touched the tap.
6. **Reserving an extension point worked.** `bump-formula` was added later as one job with no restructure, as GRB-D10 intended.

## Pointers

- Workspace: `docs/feature/github-release-binaries/feature-delta.md`, `environments.yaml`, `devops/wave-decisions.md`
- ADRs: `docs/adrs/ADR-011-release-matrix-and-channels.md`, `docs/adrs/ADR-012-supply-chain-policy.md`
- Shipped: `.github/workflows/release.yml`, `crates/cli/Cargo.toml` (version `0.1.0`), `.github/workflows/ci.yml` (shared pre-build fix)
- Release: <https://github.com/jeffabailey/openlore/releases/tag/v0.1.0>
- Downstream: `docs/evolution/homebrew-binary-distribution-evolution.md`
- Narrative origin: `docs/feature/openlore-foundation/devops/ci-cd-pipeline.md §7`

## Commit trail

`e88ab15` (DEVOPS design) · `13602a2` (release.yml + cli 0.1.0) · `60be201`, `85279d7`, `6602f36`, `6500d2c` (verify-gate fixes, rc1–rc3) · `5cbc93f` (Intel-mac cross-compile, rc4) · `0aee783` (SBOM step, rc5) · tag `v0.1.0` at `a79d2d1`. All on `main` (trunk-based, no PR).
