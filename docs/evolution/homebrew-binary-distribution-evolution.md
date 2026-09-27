<!-- markdownlint-disable MD013 -->
# Evolution: homebrew-binary-distribution (an in-repo Homebrew tap that installs the prebuilt, sha256-verified `openlore` CLI, auto-bumped on every GA release)

> Feature archive, finalized retroactively on 2026-09-27. The feature shipped on
> 2026-07-13. The workspace `docs/feature/homebrew-binary-distribution/`
> (`feature-delta.md` with the DISCUSS and DESIGN sections, `design/wave-decisions.md`,
> `design/upstream-changes.md`, `slices/slice-01-*.md`, `slices/slice-02-*.md`) is still
> the source of truth for detail. This file is the post-mortem summary. Anchor ADR:
> **ADR-061** (Accepted 2026-07-12; it promotes ADR-011's reserved "Homebrew tap"
> channel). The feature consumes ADR-011's tarball contract and the
> `github-release-binaries` pipeline (see `docs/evolution/github-release-binaries-evolution.md`).
>
> **Process note.** DISCUSS (Luna) and DESIGN (Morgan, PROPOSE mode) ran through nWave
> on 2026-07-12 (commit `e79dea5`). DISTILL and DELIVER were **not** run through nWave.
> The two carpaccio slices were implemented directly from the slice briefs. So there is
> **no** `distill/` acceptance corpus, roadmap, `execution-log.json`, DES trace or
> mutation report. The feature adds no Rust. It is a Ruby formula plus shell and YAML,
> so there was nothing to mutation-test. The verification evidence that does exist is
> listed under "Verification evidence" below: the maintainer dogfood install, a live
> formula smoke-test workflow, and the bot-authored autobump commit from the real
> `v0.1.0` release.

## Summary

J-006 users can now install and upgrade the `openlore` CLI through Homebrew, with no Rust
toolchain and no hand-verified checksum. The tap lives **in this repo**:
`Formula/openlore.rb` is a single multi-platform formula (OD-HB-3) with
`on_macos`/`on_linux` × `on_arm`/`on_intel` `url`+`sha256` pairs. Each pair points at the
ADR-011 GitHub-Release tarball for its triple. Homebrew verifies the download against the
published `.sha256` and installs only the `openlore` binary (D-2, D-3, D-4). There is no
service block, no `depends_on "rust"` and no build step (D-2, D-5).

The shipped install flow has **three** steps. DESIGN planned two:

```sh
brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore   # OD-HB-1 explicit-URL tap
brew trust jeffabailey/openlore                                        # Homebrew 6.0.0+ (not foreseen in DESIGN)
brew install jeffabailey/openlore/openlore
```

On each **GA** `v*` tag, a `bump-formula` job in `release.yml` (`needs: release`) runs
after the Release is published. It downloads the four `.sha256` files, re-renders the
formula from `Formula/openlore.rb.tmpl` via `scripts/release/bump-formula.sh`, and commits
it to `main` as `github-actions[bot]` (OD-HB-2, trunk-based, no PR). This path is
proven: `d32749e chore(release): bump homebrew formula to 0.1.0` was produced by the real
`v0.1.0` release run with no manual edit.

### Business context / job

**J-006** (`docs/product/jobs.yaml`): *"When I want to try or keep `openlore` current on
macOS (or Homebrew-Linux) and I do not want to compile from source or hand-verify a
tarball, I want to install it with one `brew install` and upgrade with one `brew upgrade`,
so I get a verified, working `openlore` binary on my PATH in seconds, with no Rust
toolchain, no manual checksum and no background updater."* Persona is P-001 (Maria)
using Homebrew. J-006 is `primary: false`: a distribution convenience channel next to the
raw GitHub-Release tarball (and a future crates.io channel).

The main anxiety force was that the formula might add an updater, a service or a
phone-home. The formula answers this structurally: it defines only `install` (`bin.install
"openlore"`) and a `test` block.

### Wave timeline

| Wave    | Date       | Owner / mode |
|---------|------------|--------------|
| DISCUSS | 2026-07-12 | Luna (nw-product-owner), lean. J-006, US-HB-001/002, 2 slices, journey `install-via-homebrew.yaml` |
| DESIGN  | 2026-07-12 | Morgan (nw-solution-architect), PROPOSE. ADR-061, DDD-1..7. Found **CA-1**: the release pipeline did not exist, so `github-release-binaries` was split out as a blocking prerequisite |
| DEVOPS  | 2026-07-12 | Covered by the prerequisite feature (`github-release-binaries`, `e88ab15`), which reserved the `bump-formula` slot |
| DISTILL | —          | **Not run through nWave** |
| DELIVER | 2026-07-13 | **Implemented directly** (no nWave DELIVER): slice-01 `2b4574c`, slice-02 `a79d2d1`, then post-ship fixes `5f26b9b`, `96c0f19` |

## Key decisions (from `design/wave-decisions.md` and the feature-delta DDD table)

| # | Decision | Outcome |
|---|---|---|
| D-1 / DDD-1 (OD-HB-1) | In-repo tap. Users run a one-time explicit-URL `brew tap …/openlore https://github.com/jeffabailey/openlore`, with no `homebrew-openlore` mirror repo | Shipped. The dogfood confirmed the URL is **required**. Homebrew 6.0.0+ also needs `brew trust` (Deviations #1) |
| DDD-2 (OD-HB-2) | Autobump as a job **inside** `release.yml`, ordered by a `needs:` edge after upload, committing to `main` (no `brew bump-formula-pr`, no separate workflow) | Shipped as `bump-formula`, `needs: release`, GA-only. Proven by `d32749e` |
| DDD-3 (OD-HB-3) | A single multi-platform formula, no ghcr bottles | Shipped |
| DDD-4 | brew-native sha256 is the in-channel integrity check; cosign `.sig` stays the separate manual provenance path | Shipped as designed |
| DDD-5 | Per-triple `brew install` + `openlore --version` smoke test on real runners, **blocking the release** | **Deviated**: covers 2 of 4 platforms and runs after the formula is committed, as a separate workflow, not as a release gate (Deviations #2) |
| DDD-6 | Dependency-free bash templating: `bump-formula.sh` + `Formula/openlore.rb.tmpl` | Shipped. Fails loudly on a missing `.sha256` or an unsubstituted placeholder |
| DDD-7 | `release.yml` is a blocking external prerequisite: design first, build later | Honoured. `github-release-binaries` shipped first, and slice-01 dogfooded against `v0.1.0-rc6` |
| OQ-D-3 | Bot identity and token for the `main` commit | Resolved as `GITHUB_TOKEN` with `contents: write`, committing as `github-actions[bot]`. The workflow notes that branch protection would need a bypass |
| OQ-D-4 | Where the template and bump script live | Co-located: `Formula/openlore.rb.tmpl` and `scripts/release/bump-formula.sh` |

## What shipped

| Commit | Date | Slice / kind | What |
|---|---|---|---|
| `e79dea5` | 2026-07-12 | DISCUSS + DESIGN | feature-delta, wave-decisions, upstream-changes, slices, **ADR-061**, J-006 in `jobs.yaml`, `install-via-homebrew.yaml` journey, `brief.md` Homebrew SSOT, ADR-011 cross-reference |
| `2b4574c` | 2026-07-13 | slice-01 (US-HB-001) | `Formula/openlore.rb` (56 lines), pointed at `v0.1.0-rc6`. **Dogfood passed on Apple Silicon**: `brew install` → `openlore 0.1.0`, `brew style` clean, `brew test` green |
| `a79d2d1` | 2026-07-13 | slice-02 (US-HB-002) | `bump-formula` job in `release.yml` (GA-only), `Formula/openlore.rb.tmpl`, `scripts/release/bump-formula.sh`. `openlore.rb` is now generated. This is also the commit tag `v0.1.0` points at |
| `d32749e` | 2026-07-13 | autobump (bot) | `chore(release): bump homebrew formula to 0.1.0`, authored by `github-actions[bot]` in the `v0.1.0` release run. The first end-to-end autobump |
| `8a77822` | 2026-07-13 | fix | Workspace `Cargo.toml` `repository` and README clone URL `jeffbailey` → `jeffabailey`. The formula, `release.yml` and tap already used the correct owner |
| `5f26b9b` | 2026-07-13 | fix (post-ship) | Documents the `brew trust` step and the explicit-URL tap in the formula header, template and README. Adds `scripts/release/smoke-test-formula.sh` and `.github/workflows/formula-smoke.yml` |
| `96c0f19` | 2026-07-13 | fix (post-ship) | Adds Linuxbrew (`/home/linuxbrew/.linuxbrew/bin`) to `$GITHUB_PATH` in the smoke workflow. The first smoke run failed on ubuntu with `brew: command not found` |

### Verification evidence (in place of the DISTILL/DELIVER gates)

| Evidence | Result |
|---|---|
| Slice-01 dogfood (maintainer, `aarch64-apple-darwin`, rc6) | `brew install` → `openlore --version` = `openlore 0.1.0`; `brew style` clean; `brew test` green (recorded in the `2b4574c` message) |
| Slice-02 local validation | `bump-formula.sh 0.1.0-rc6` rendered a `brew style`-clean formula with the correct checksums; `bash -n` clean |
| Autobump in production | `v0.1.0` release run 29261084943, job `bump homebrew formula` succeeded → bot commit `d32749e` |
| Formula smoke test (`formula-smoke.yml`) | Run 29297408727 **failed** (ubuntu: brew not on PATH) → run 29297512108 **passed** on macos-14 and ubuntu-latest in 57s. Tap → trust → install → semver `--version` against the live tap and the v0.1.0 Release |
| Formula content at HEAD | `version "0.1.0"` and 4 v0.1.0 url/sha256 pairs; `bin.install "openlore"` only; no service/`plist`; no `depends_on` |
| Not available | No DISTILL acceptance scenarios for the UAT in US-HB-001/002. No negative (tampered-checksum) test. No mechanical no-service check. No DES trace or review record |

## Deviations: planned (DESIGN) vs shipped

| # | Planned | Shipped | Disposition |
|---|---|---|---|
| 1 | A two-step install: explicit-URL `brew tap`, then `brew install` (DDD-1) | Three steps: `tap` → **`brew trust`** → `install`. Homebrew 6.0.0+ ignores untrusted custom-remote taps, and `brew trust --formula` is rejected for custom remotes | Found after shipping, when a user-facing install failed. Homebrew's tap-trust feature postdates the design. Fixed in docs (`5f26b9b`) with no formula logic change. R-2 (tap-resolution confusion) turned out worse than rated |
| 2 | The per-triple smoke test on **all 4 triples** gates the release (DDD-5, D-6, slice-02 AC: "blocks the release on failure") | `formula-smoke.yml` is a **separate** workflow on `push` to `main` (paths: `Formula/openlore.rb`, script, workflow) plus `workflow_dispatch`, on **2 platforms** (macos-14 arm64, ubuntu-latest x86_64). It runs after the formula is committed, so it does not gate the release | **Material deviation.** `x86_64-apple-darwin` and `aarch64-unknown-linux-gnu` installs are not smoke-tested. The workflow comment defers them to `release.yml`'s build matrix, which builds the tarballs but never `brew install`s them. See also #3 |
| 3 | Smoke test triggered by the autobump commit (workflow header: "including the release.yml `bump-formula` autobump commit") | `bump-formula` pushes with `GITHUB_TOKEN`. **Pushes made with `GITHUB_TOKEN` do not trigger other workflows**, so the bot commit will **not** start `formula-smoke.yml`. (For `d32749e` the smoke workflow did not exist yet.) | **Latent gap.** A broken autobump would reach users unless someone runs `workflow_dispatch` by hand. Fix options: call the smoke test as a job inside `release.yml` after `bump-formula` (closer to DDD-5), dispatch it explicitly from `bump-formula`, or push with a PAT or GitHub App token |
| 4 | Freshness assertion `Formula version == v* tag` (KPI-HB-3, slice-02 AC) | No separate assertion. Freshness holds by construction: `bump-formula.sh` renders `VERSION` from the tag, and the job fails on a missing checksum or leftover placeholder | Acceptable in practice. KPI-HB-3 is met for v0.1.0 but has no mechanical check. A job that fails or is skipped would leave the formula stale without any alarm |
| 5 | Formula `test` asserts `version.to_s` appears in `--version` | Asserts a semver shape (`/^openlore \d+\.\d+\.\d+/`) | Deliberate: an RC formula version (`0.1.0-rc6`) differs from the binary's Cargo version (`0.1.0`). Weaker than designed, but robust |
| 6 | Autobump `needs: [upload]` / `needs: [publish]` | `needs: release`, the merged sign+publish job from `github-release-binaries` | Same ordering guarantee (R-1 closed by an in-DAG edge). Naming only |
| 7 | Autobump on every `v*` tag (D-6) | **GA-only** (`if: !contains(github.ref_name, '-')`) | Improvement: RC tags never move the tap |
| 8 | Enforcement via `brew audit --strict --online` + `brew style` (Technology Stack) | `brew style` was run locally at slice-01/02 time. Neither runs in CI | Open item |
| 9 | Guardrails D-4 (tamper aborts install) and D-5 (no service / no phone-home) "mechanically asserted" (KPI guardrails) | Hold structurally (no service block; brew enforces `sha256`). No test asserts either | Honest status: structural, not behavioural |
| 10 | KPI-HB-1..3 "belong in `docs/product/kpi-contracts.yaml`" | Not added | Open item |

## KPI status

- **KPI-HB-1** (≥95% install success across all 4 triples): **partially evidenced**. 2 of 4 platforms pass in CI (macos-14, ubuntu-latest), and the maintainer dogfood passed on `aarch64-apple-darwin`. Intel-mac and arm64-linux installs are unverified.
- **KPI-HB-2** (`brew install` → `--version` in under 60s): **evidenced informally**. The whole smoke job, including checkout and tap, took 57s. The wall-clock time is not measured separately.
- **KPI-HB-3** (formula version == latest tag, for 100% of releases): **met for v0.1.0** (the only GA release so far), by construction. There is no standing assertion.
- **Guardrails** (no service/phone-home, openlore-only, checksum-verified): hold structurally, not tested behaviourally.

## Deferred items / open questions

- ~~**Close the smoke-test gap**~~ **Resolved 2026-09-27 by relaxing the requirement** (ADR-061 D-6 amendment): `bump-formula` now dispatches `formula-smoke.yml` after it pushes, and the smoke test runs on macos-14 only. It is still a post-release signal, not a gate. Linux and Intel-mac installs are deliberately not smoke-tested for now.
- **Branch protection**: `bump-formula` pushes directly to `main` with `GITHUB_TOKEN`. If `main` is ever protected, the job needs a bypass or a different token (noted in `release.yml`).
- **`brew audit --strict --online` / `brew style` in CI** (Deviations #8).
- **Record KPI-HB-1..3** in `docs/product/kpi-contracts.yaml` (Deviations #10).
- **Negative tests** for D-4 (a tampered checksum aborts install) and D-5 (no `brew services`/launchd/systemd entry after install).
- **Out of scope, unchanged** (per feature-delta): `openlore-indexer`, Windows, build-from-source, homebrew-core submission, cosign verification inside the formula, ghcr bottles, AUR/Nix.
- **ADR-061** is Accepted and was not modified here. The `brew trust` step is a documentation-level change to the DDD-1 install flow. Consider a short ADR-061 note if the install flow changes again.

## Lessons learned

1. **Dogfooding on the maintainer's machine is not the same as a user install.** Slice-01's dogfood passed, but a real user install later failed on two problems stacked together: the explicit-URL tap and Homebrew 6.0.0 tap trust. The commit history does not record why the dogfood passed; a pre-existing tap or an older Homebrew are likely explanations. A clean-slate scripted smoke test (`smoke-test-formula.sh` untaps and uninstalls first) is the correct probe.
2. **Upstream tool behaviour moves under a design.** Homebrew tap trust arrived after OD-HB-1 was decided. Channels built on third-party package managers need a periodic end-to-end install check, not a one-time proof.
3. **Events triggered by `GITHUB_TOKEN` do not chain.** Designing "the bot commit triggers the smoke test" silently fails on GitHub Actions. Put verification in the same DAG as the mutation, which is what DDD-5 originally specified.
4. **CI runner images differ.** The ubuntu runner has Linuxbrew installed but not on PATH, while macOS runners have brew on PATH. The first CI run found this.
5. **Splitting out the prerequisite was correct.** DESIGN's CA-1 finding (no `release.yml`) led to `github-release-binaries` being designed and shipped first. After that, both homebrew slices took less than a day together and the autobump worked first time on GA.
6. **GA-gating the autobump kept the RC ladder safe.** The rc series in the prerequisite feature never touched the tap.

## Pointers

- Workspace: `docs/feature/homebrew-binary-distribution/feature-delta.md`, `design/wave-decisions.md`, `design/upstream-changes.md`, `slices/slice-01-repo-tap-dogfood.md`, `slices/slice-02-release-autobump.md`
- ADRs: `docs/adrs/ADR-061-homebrew-tap-distribution-channel.md` (Accepted), `docs/adrs/ADR-011-release-matrix-and-channels.md`, `docs/adrs/ADR-012-supply-chain-policy.md`
- Product: `docs/product/jobs.yaml` (J-006), `docs/product/journeys/install-via-homebrew.yaml`, `docs/product/architecture/brief.md` (Homebrew SSOT subsections)
- Shipped: `Formula/openlore.rb`, `Formula/openlore.rb.tmpl`, `scripts/release/bump-formula.sh`, `scripts/release/smoke-test-formula.sh`, `.github/workflows/formula-smoke.yml`, `.github/workflows/release.yml` (`bump-formula` job), `README.md` (Homebrew install section)
- Prerequisite: `docs/evolution/github-release-binaries-evolution.md`

## Commit trail

`e79dea5` (DISCUSS + DESIGN, ADR-061) · `2b4574c` (slice-01 formula) · `a79d2d1` (slice-02 autobump; tag `v0.1.0`) · `d32749e` (bot autobump to 0.1.0) · `8a77822` (repo URL fix) · `5f26b9b` (brew trust docs + smoke test) · `96c0f19` (Linuxbrew PATH). All on `main` (trunk-based, no PR).
