# Feature Delta — github-release-binaries (DEVOPS)

- **Wave**: DEVOPS
- **Date**: 2026-07-12
- **Architect**: Apex (nw-platform-architect)
- **Status**: design (release.yml is greenfield — this is the DELIVER handoff spec)

**Context.** This feature builds `.github/workflows/release.yml`, which does not
yet exist. Requirements derive from **ADR-011** (4-triple release matrix +
`openlore-{version}-{triple}.tar.gz` naming — a LOCKED contract), **ADR-012**
(supply-chain: cosign keyless, CycloneDX SBOM, SLSA L2/L3, `Cargo.lock`-built),
and `docs/feature/openlore-foundation/devops/ci-cd-pipeline.md §7` (release-stage
narrative). It is the **blocking prerequisite** that unblocks the parked
`homebrew-binary-distribution` feature (that formula consumes ADR-011's tarball
names and the `.sha256`/cosign artifacts published here).

Scope is deliberately narrow: the four native builds, first-party supply-chain
integrity, and publish to GitHub Releases. Heavier pre-release gates named in
`ci-cd-pipeline.md §7.1` are intentionally deferred (see `## Changed
Assumptions`). No new ADR is required — ADR-011 + ADR-012 already govern this;
a short scoped-deferral ADR is *recommended, not written* (see Open questions).

No DISCUSS/DESIGN feature-delta precedes this file; this DEVOPS delta is the
feature's first design artifact.

---

## Wave: DEVOPS / [REF] Environment matrix

Four native builds, one per ADR-011 target triple, each on its **native runner**
(no cross-compile — ADR-011 rejection rationale is preserved). Each job emits
`openlore-{version}-{triple}.tar.gz` (containing the `openlore` binary) plus a
`.sha256` companion. Build command is scoped to the CLI binary only:
`cargo build --release -p cli --bin openlore` (built against the committed
`Cargo.lock` per ADR-012).

| # | Target triple | Runner label | Arch | Notes / preconditions |
|---|---|---|---|---|
| 1 | `aarch64-apple-darwin` | `macos-14` | Apple Silicon | Default arm64 macOS hosted runner. |
| 2 | `x86_64-apple-darwin` | `macos-13` | Intel | Last Intel-hosted macOS image; watch deprecation (see Open questions). |
| 3 | `x86_64-unknown-linux-gnu` | `ubuntu-latest` | x86_64 glibc | Same image family as `ci.yml`. |
| 4 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | aarch64 glibc | GitHub-hosted arm64 Linux runner (GA, free for public repos). **Risk + fallback below.** |

`{version}` derives from the tag by stripping the leading `v` (`v0.1.0` ->
`0.1.0`). See `[REF] Pre-requisites` for the tag==Cargo-version assertion.

**arm64-linux runner risk.** `ubuntu-24.04-arm` (and `ubuntu-22.04-arm`) are
GitHub-hosted Linux/arm64 runners, GA and free for **public** repositories;
this repo is public FOSS, so they should be available at zero cost. If the
account/runner-group cannot schedule arm64-linux (private-repo billing, org
runner-group policy, or a future GitHub availability gap), the documented
fallbacks, in preference order, are:
1. **`cargo-zigbuild` / `cross` from `ubuntu-latest`** — cross-compile
   `aarch64-unknown-linux-gnu`. This *relaxes* ADR-011's no-cross-compile rule
   for exactly one cell; note it in the release notes and open an ADR-011
   amendment if it becomes permanent.
2. **QEMU emulation** (`docker/setup-qemu-action` + arm64 container) — correct
   but ~5-10x slower; acceptable for a manually-fired release.
3. **Self-hosted arm64 runner** — last resort; adds custody/maintenance burden
   that contradicts the solo-dev posture.

Per-runner build preconditions (all four): pinned toolchain via
`dtolnay/rust-toolchain@stable` (the repo pins `channel = "stable"` in
`rust-toolchain.toml`; workspace MSRV `1.91`), `Swatinem/rust-cache@v2`,
committed `Cargo.lock` present. macOS runners need the target's native linker
(system default — no extra install). Linux gnu needs no extra sysroot (native).

Full machine schema: `environments.yaml` (sibling).

---

## Wave: DEVOPS / [REF] CI/CD pipeline outline

`release.yml`. Trigger: `on: push: tags: ['v*']` **only** (never per-PR, never
per-push — trunk-based; `ci.yml` already owns PR/push validation).

**Concurrency**: `group: release-${{ github.ref }}`, `cancel-in-progress:
false` — a release tag must run to completion; never cancel a half-published
release (see `environments.yaml` deployment assumptions: no double-publish).

**Top-level permissions** (least privilege; elevated only where needed):
`contents: write` (create Release + upload assets), `id-token: write` (GitHub
OIDC for cosign keyless + SLSA attestation), `attestations: write` (record
build-provenance attestations).

### Job DAG (in scope)

```
verify ──▶ build-release (matrix ×4) ──▶ sign-sbom-provenance ──▶ publish
                                                                     │
                                          (future extension point) ──┘
                                          bump-formula  needs: [publish]   ← OD-HB-2, placeholder only
```

**Job 1 — `verify`** (reuse existing CI gates on the tagged ref).
Re-runs the commit-stage + acceptance-stage checks from `ci.yml` against the
tag: `fmt`, `clippy -D warnings`, `cargo deny check`, `cargo xtask check-arch`
(preceded by `cargo test -p xtask` as `ci.yml` does), `cargo xtask
check-probes`, and `cargo nextest run --workspace --test-threads=1` (the
`--test-threads=1` guard is mandatory — documented adapter-system-clock
`OPENLORE_TEST_NOW` env-race). Match `ci.yml` conventions exactly:
`actions/checkout@v4`, `dtolnay/rust-toolchain@stable`,
`Swatinem/rust-cache@v2`, `taiki-e/install-action@v2` for `cargo-deny` /
`nextest`. Do **not** invent new gates. Blocking: no artifact is built unless
`verify` is green. (Reuse is DRY-by-reference; DELIVER MAY factor the shared
job into a reusable `workflow_call` if it reduces drift, but MUST NOT change
the checks.)

**Job 2 — `build-release`** (`needs: [verify]`, `strategy.matrix` ×4 from the
Environment matrix, `fail-fast: false` so one platform failure does not abort
the others). Each cell: checkout tag -> toolchain (+ `rustup target add` where
the runner's default host triple differs, e.g. explicit
`aarch64-unknown-linux-gnu` on the arm64 runner is native so no add needed) ->
`cargo build --release -p cli --bin openlore` -> package
`openlore-{version}-{triple}.tar.gz` -> emit `.sha256` companion -> upload both
as a build artifact keyed by triple for the downstream jobs. Assert
`tag-version == cli-Cargo-version` here (fail fast if mismatched).

**Job 3 — `sign-sbom-provenance`** (`needs: [build-release]`; requires
`id-token: write`, `attestations: write`). Downloads all four tarballs, then:
- **cosign keyless** (`sigstore/cosign-installer` -> `cosign sign-blob
  --yes`) per tarball via GitHub OIDC -> `.sig` + `.cert` per tarball. No
  long-lived keys (ADR-012).
- **CycloneDX SBOM** (release-wide): `cargo cyclonedx` — pin
  **`CycloneDX/cyclonedx-rust-cargo`** installed via
  `taiki-e/install-action@v2` (tool: `cargo-cyclonedx`, pinned version) to
  match repo tooling convention -> `sbom.cdx.json`. Generated from the same
  committed `Cargo.lock` the binaries were built against.
- **SLSA build provenance**: `actions/attest-build-provenance@v1` over the four
  tarballs -> in-toto attestation (target SLSA L3 via GitHub OIDC; L2
  acceptable per ADR-012).

**Job 4 — `publish`** (`needs: [sign-sbom-provenance]`; `contents: write`).
Uploads to the GitHub Release for the tag: 4× tarball + 4× `.sha256` + 4× `.sig`
+ 4× `.cert` + `sbom.cdx.json` + CHANGELOG excerpt. Tool:
`softprops/action-gh-release` (or `gh release create/upload`). Idempotent on
re-run of the same tag (see deployment strategy + `environments.yaml`
assumptions). The Release body embeds the ADR-012 verify commands (cosign
verify-blob, sha256 -c, cyclonedx analyze) so consumers can verify end-to-end.

**Future extension point (documented, NOT built here).** The
`homebrew-binary-distribution` feature (OD-HB-2) will ADD a `bump-formula` job
to *this* `release.yml` with `needs: [publish]`, reading the published
`.sha256` files to update the tap formula. Leave a placeholder comment in the
YAML at the DAG tail so that job slots in cleanly:
`# EXTENSION POINT (homebrew-binary-distribution OD-HB-2): bump-formula, needs: [publish]`.

A `[HOW] ci-pipeline-yaml` skeleton follows for DAG clarity; the Tier-1
deliverable is this design, not the YAML (DELIVER writes the workflow).

---

## Wave: DEVOPS / [REF] Monitoring contracts

There is **no running service** and **no KPI file** for this feature (it
publishes immutable artifacts, not a service) — no `outcome-kpis.md` exists,
so no KPI instrumentation is designed. "Monitoring" here is deployment-success
signalling, not runtime telemetry:

- **Release-success signal** = the GitHub Release for the tag carries the
  complete artifact set: 4× tarball + 4× `.sha256` + 4× `.sig` + 4× `.cert` +
  `sbom.cdx.json` + CHANGELOG excerpt. Absence of any of these is itself the
  failure signal (ADR-012 Earned Trust: missing `.sig`/`.cert`/SBOM = something
  is wrong).
- **Failure blocks publish**: `verify`, `build-release`, and
  `sign-sbom-provenance` are all blocking `needs` upstream of `publish`; a red
  job means no (or partial) Release. Partial-publish is prevented by ordering
  publish last and by `cancel-in-progress: false`.
- **Observation surface**: the GitHub Actions run log + the Actions "Attestations"
  tab (SLSA provenance) + the Release assets page. No external monitoring stack,
  no SLO, no alerting tier — consistent with openlore-foundation D-D-series
  deferrals for a local-first CLI.

---

## Wave: DEVOPS / [REF] Deployment strategy

**Immutable, tag-driven publish.** A `v*` tag is the immutable release unit;
artifacts are content-addressed by `.sha256` and anchored in sigstore's Rekor
transparency log via cosign. There is no rolling/blue-green/canary strategy —
those apply to running services; this publishes files. The strategy is
"build-once-from-the-tagged-commit, publish-once."

**Rollback** (design-rollback-first). Because artifacts are immutable and may
already be downloaded, rollback is *withdraw-and-recut*, not mutate:
1. Delete the GitHub Release (removes the published assets) **and** delete the
   `v*` tag (`git push --delete origin vX.Y.Z`).
2. Fix forward on `main` (trunk-based; no PR).
3. Re-cut a **new** patch tag (`vX.Y.(Z+1)`) — never reuse a burned tag, since
   consumers/caches and Rekor may already reference the old one. cosign/Rekor
   entries for the withdrawn tag remain in the transparency log (immutable by
   design); the withdrawn Release simply ceases to be the "latest."
Automated rollback triggers do not apply (no live traffic); rollback is a manual
decision keyed on a broken artifact, a failed post-publish verify, or a
supply-chain finding.

---

## Wave: DEVOPS / [REF] Mutation testing strategy

**Unchanged: per-feature** (the project CLAUDE.md strategy is NOT rewritten by
this feature). Mutation testing lives in `nightly.yml` (advisory,
`cargo mutants -p claim-domain`). A **pre-release full mutation sweep as a
release gate is DEFERRED** (see `## Changed Assumptions`) — its infrastructure
(a release-blocking mutation job) is unbuilt, and `ci-cd-pipeline.md §7.1.3`
names it but it does not exist. `release.yml` carries a TODO marker referencing
the deferred slice; it does not run mutation.

---

## Wave: DEVOPS / [REF] Observability stack

GitHub Actions run logs + Release artifact presence + the Actions Attestations
tab. **No external observability stack** (no Grafana/Datadog/Prometheus, no
`tracing` sink — this is CI, not the running binary). This matches
openlore-foundation D-D2/D-D3 (no remote sink, no distributed tracing). The
only durable "observability" artifact of a release is the immutable evidence
trail: Rekor entry (cosign), SLSA attestation (Actions), and the SBOM.

---

## Wave: DEVOPS / [REF] Branching strategy

**Trunk-based development** (house rule: commit to `main`, no PRs). `release.yml`
triggers on `tags: ['v*']` **only** — never per-PR, never per-push; `ci.yml`
already validates every push to `main`. Tags are cut from `main`.

**Autobump-to-main note (future).** The Homebrew formula bump
(`homebrew-binary-distribution` OD-HB-2) will later push a formula update; when
that lands as the `bump-formula` job here, it commits to the tap repo (or opens
against it) — it does **not** re-trigger `release.yml` (no `v*` tag is created
by the bump). The documented extension point (`needs: [publish]`, placeholder
comment) reserves its slot in the DAG so the future slice adds one job, not a
restructure.

---

## Wave: DEVOPS / [REF] Coexistence matrix

`release.yml` is **purely additive**. `ci.yml` (PR + push-to-main: commit +
acceptance stages) and `nightly.yml` (scheduled mutation, advisory) must keep
working **unchanged**. No shared trigger overlap: `ci.yml` fires on
`pull_request`/`push: [main]`, `nightly.yml` on `schedule`/`workflow_dispatch`,
`release.yml` on `push: tags: ['v*']` — disjoint. The three share *conventions*
(checkout@v4, rust-toolchain@stable, rust-cache@v2, install-action@v2,
nextest `--test-threads=1`) but no files. `deny.toml` is reused read-only by
both `ci.yml` and `release.yml`'s `verify` job. Full matrix:
`environments.yaml` `coexistence_matrix`.

---

## Wave: DEVOPS / [REF] Pre-requisites

Blocking prerequisites before the first release can be cut:

1. **A first `v*` tag** must be created from `main` (e.g. `v0.1.0` per ADR-011's
   `0.1.x` series). No release has ever run.
2. **`Cargo.lock` committed** (ADR-012 anchor) — confirm present at repo root
   before tagging; the release builds against it, not a fresh resolve.
3. **Tag == Cargo version.** The `cli` crate is currently `version = "0.0.1"`;
   ADR-011 names the `0.1.x` series (tag `v0.1.0` -> `0.1.0`). These **do not
   match today**. Before the first tag, bump `crates/cli/Cargo.toml` `version`
   to match the intended tag, and have `build-release` **assert**
   `tag-stripped-of-v == cli Cargo version` (fail the release on mismatch). This
   prevents "tag says 0.1.0, binary reports 0.0.1."
4. **CHANGELOG source decided.** No `CHANGELOG.md` exists. Lightest recommended
   option: derive the release-notes excerpt from the **annotated tag message**
   (`git tag -a vX.Y.Z -m "..."` -> `git for-each-ref` / `gh release create
   --notes-from-tag`), avoiding a maintained CHANGELOG file. If a file is
   preferred later, adopt "Keep a Changelog" + a `## [X.Y.Z]` section extractor.
   Decide before the first tag (see Open questions).
5. **arm64-linux runner availability confirmed** on the account (public repo ->
   `ubuntu-24.04-arm` free); else select a fallback from `[REF] Environment
   matrix`.

---

## Wave: DEVOPS / [REF] Open questions

1. **CHANGELOG mechanism** — annotated-tag-message (recommended, lightest) vs a
   maintained `CHANGELOG.md`. Decide before first tag (Pre-req 4).
2. **arm64-linux runner fallback** — confirm `ubuntu-24.04-arm` schedules on
   this account; if not, pick fallback (zigbuild/QEMU/self-hosted) and note
   whether it warrants an ADR-011 amendment (it relaxes no-cross-compile).
3. **Intel macOS runner deprecation** — `macos-13` is the last Intel-hosted
   image; track GitHub's deprecation timeline for `x86_64-apple-darwin`.
4. **Deferred slices** (each a future feature, not designed here): crates.io
   publish (`cargo publish`, needs `CRATES_IO_TOKEN`); 8-cell substrate
   gold-matrix release gate; full mutation sweep as a release gate;
   Pact-against-real-bsky (`PACT_REAL_PDS=1`). See `## Changed Assumptions`.
5. **Scoped-deferral ADR** — RECOMMENDED (not written): a short ADR recording
   that `release.yml` v1 intentionally omits `ci-cd-pipeline.md §7.1` heavy
   gates. ADR-011 + ADR-012 already cover the in-scope design; this would only
   document the *deferral*. Apex recommends deferring the ADR itself until a
   second reviewer exists — a TODO marker in the workflow is sufficient for now.

---

## Changed Assumptions

`ci-cd-pipeline.md §7.1` specifies a heavier pre-release gate set. This feature
**intentionally defers** the following because their infrastructure is unbuilt;
`release.yml` carries `# TODO(deferred-slice: <name>)` markers referencing each:

| §7.1 gate | Status here | Why deferred | Future slice |
|---|---|---|---|
| Full `cargo mutants` sweep (blocking) | DEFERRED | Only `nightly.yml` runs mutation (advisory); no release-blocking mutation job exists. | mutation-release-gate |
| 8-cell substrate gold-matrix | DEFERRED | `substrate-matrix.md` is a doc; no runnable gate exists. | substrate-gold-matrix-gate |
| Pact vs real `bsky.social` (`PACT_REAL_PDS=1`) | DEFERRED | Manual-approval real-PDS replay is unbuilt; rate-limit + env-protection ceremony out of scope. | pact-real-pds-release-gate |
| crates.io publish (`cargo publish`) | DEFERRED (also ADR-011 channel) | Needs `CRATES_IO_TOKEN` secret custody (ADR-012 §Signing key custody); out of scope for the OIDC/first-party-only v1. | cratesio-publish |

The in-scope `verify` job DOES re-run the full commit + acceptance stages from
`ci.yml` (§7.1.1 + §7.1.2) — only the four heavy gates above are deferred. This
delta refines §7.2 (build matrix) and §7.3 (publish) for the in-scope parts.

---

## [HOW] ci-pipeline-yaml (skeleton — DAG clarity only; DELIVER writes the real file)

Illustrative structure, not the deliverable. Elides step detail; shows the DAG,
triggers, permissions, and the extension point.

```yaml
name: Release
on:
  push:
    tags: ['v*']
concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false          # never cancel a half-published release
permissions:
  contents: write                    # create Release + upload assets
  id-token: write                    # OIDC: cosign keyless + SLSA attest
  attestations: write                # record build-provenance
env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
jobs:
  verify:                            # reuse ci.yml gates on the tagged ref
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with: { tool: cargo-deny,nextest }
      # fmt | clippy -D warnings | cargo deny check |
      # cargo test -p xtask && cargo xtask check-arch | cargo xtask check-probes |
      # cargo nextest run --workspace --test-threads=1   (env-race guard)
      # TODO(deferred-slice: mutation-release-gate | substrate-gold-matrix-gate |
      #                      pact-real-pds-release-gate) — §7.1 heavy gates

  build-release:
    needs: [verify]
    strategy:
      fail-fast: false
      matrix:
        include:
          - { triple: aarch64-apple-darwin,       runner: macos-14 }
          - { triple: x86_64-apple-darwin,        runner: macos-13 }
          - { triple: x86_64-unknown-linux-gnu,   runner: ubuntu-latest }
          - { triple: aarch64-unknown-linux-gnu,  runner: ubuntu-24.04-arm }  # fallback: zigbuild@ubuntu-latest
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      # assert tag(vX.Y.Z -> X.Y.Z) == cli Cargo version, else fail
      # cargo build --release -p cli --bin openlore   (built against Cargo.lock)
      # tar czf openlore-${VERSION}-${{ matrix.triple }}.tar.gz openlore
      # sha256sum > openlore-${VERSION}-${{ matrix.triple }}.tar.gz.sha256
      # upload-artifact keyed by triple

  sign-sbom-provenance:
    needs: [build-release]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4          # all 4 tarballs
      - uses: sigstore/cosign-installer@v3
      # cosign sign-blob --yes  per tarball -> .sig + .cert    (OIDC keyless)
      - uses: taiki-e/install-action@v2
        with: { tool: cargo-cyclonedx }              # pinned
      # cargo cyclonedx -> sbom.cdx.json  (release-wide, from Cargo.lock)
      - uses: actions/attest-build-provenance@v1     # SLSA L3 target / L2 ok
        with: { subject-path: 'openlore-*.tar.gz' }

  publish:
    needs: [sign-sbom-provenance]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            openlore-*.tar.gz
            openlore-*.tar.gz.sha256
            openlore-*.tar.gz.sig
            openlore-*.tar.gz.cert
            sbom.cdx.json
          # body: CHANGELOG excerpt from annotated tag message (Pre-req 4)
      # Release body embeds ADR-012 verify commands for consumers.

  # EXTENSION POINT (homebrew-binary-distribution OD-HB-2):
  # bump-formula:
  #   needs: [publish]        # reads published *.sha256 -> updates tap formula
```
