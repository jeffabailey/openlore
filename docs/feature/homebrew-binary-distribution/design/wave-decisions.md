# DESIGN Wave Decisions — homebrew-binary-distribution

> Wave: DESIGN (application/components scope) · Mode: PROPOSE · Architect: Morgan (nw-solution-architect)
> Date: 2026-07-12 · Anchor ADR: ADR-061 · Feature-delta: `docs/feature/homebrew-binary-distribution/feature-delta.md`

## Key Decisions (with PROPOSE-mode options + trade-offs)

The user may override any RECOMMENDED option below.

### OD-HB-1 — Tap resolution (MED)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED — one-time explicit-URL `brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore`, then `brew install jeffabailey/openlore/openlore`** | Keeps tap truly in-repo (D-1); no second repo; `brew upgrade` works after the one-time tap; one documented command | One extra one-time command vs a bare install (R-2) |
| (b) direct `brew install <url-to-Formula/openlore.rb>` | Zero tap step | **Breaks `brew upgrade`** (no tap registered) → defeats US-HB-002; reject |
| (c) `homebrew-openlore` mirror repo (default resolution) | Bare `brew install jeffabailey/openlore/openlore` works with no tap step | **Violates in-repo D-1**; a 2nd repo to keep byte-synced each release; reject |

**Verdict (DDD-1): (a).** One-time command documented in README.

### OD-HB-2 — Formula auto-bump mechanism (HIGH — headline)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED — a job INSIDE the future `release.yml` that regenerates the formula + commits to `main`, `needs: [upload]`** | Strongest ordering guarantee (in-DAG edge → closes R-1 race); honors trunk-based/no-PR; one source of release sequencing; the autobump EXTENDS the pipeline that already has the artifacts | Requires `release.yml` to exist (it does not yet — CA-1/OQ-D-1) |
| (b) `brew bump-formula-pr` | Standard Homebrew tooling | **Opens a PR → conflicts with the trunk-based/no-PR house rule**; reject |
| (c) separate `formula-bump.yml` on `release: published` | Decoupled from release.yml | Event-driven ordering is **weaker** than an in-DAG `needs:` edge (asset upload vs release-published race); 2nd workflow to maintain; reject |

**Verdict (DDD-2): (a).** In-`release.yml` `bump-formula` job, `needs: [upload]`, commit to `main`,
gated by the per-triple smoke test. Because `release.yml` does not exist, this is a **delta to the
prerequisite feature's workflow**.

### OD-HB-3 — Formula shape (LOW)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED — single multi-platform formula (`on_macos`/`on_linux` × `on_arm`/`on_intel` url+sha256)** | Reuses GH-Release tarballs directly; no ghcr; one `.rb` covers all 4 triples | Slightly larger single file |
| (b) per-platform `bottle do` blocks on ghcr | Homebrew-native bottle UX | Needs a ghcr bottle build/host pipeline this lean channel does not want; reject (future optimization) |

**Verdict (DDD-3): (a).** Single multi-platform formula confirmed.

## Architecture Summary

- **Style**: unchanged repo style (hexagonal / modular-monolith Rust CLI, ADR-009). This feature
  adds **no Rust** — it is a Ruby formula + YAML/shell CI. The `openlore` binary is a black box.
- **Driving port**: the Homebrew CLI surface (`brew tap`/`install`/`upgrade`).
- **Driven ports/adapters**: GitHub Releases (tarball + `.sha256` source) via the formula's
  `url`/`sha256`; repo `git main` (formula commit target) via `bump-formula.sh`.
- **Earned Trust**: the per-triple `brew install` + `openlore --version` smoke test is the formula's
  `probe()` analog — bump → probe → release proceeds; a failed probe refuses to ship.
- **C4**: System Context (L1) + Container (L2) in the feature-delta DESIGN sections. L3 omitted
  (< 5 internal components).

## Reuse Analysis (verdicts)

| Candidate | Verdict |
|---|---|
| `ci.yml` | NEITHER (ubuntu/Rust-scoped, PR-gated; brew smoke test is release-gated + multi-OS) |
| `nightly.yml` | NEITHER (mutation testing; unrelated) |
| `release.yml` | **EXTEND (the future file)** — autobump + smoke test are jobs *inside* it (does not exist yet) |
| `xtask check-arch/check-probes` | NEITHER (Rust-only; formula enforcement analog = `brew audit`/`brew style`) |
| ADR-011 4-tarball contract | **REUSE (contract, unchanged)** — the load-bearing reuse; no new binary production |
| `Formula/openlore.rb` | **CREATE NEW** (no existing formula) |

## Technology Stack

| Layer | Choice | License |
|---|---|---|
| Formula | Homebrew DSL (Ruby, ≥4.x; `on_macos`/`on_linux`/`on_arm`/`on_intel`, `bin.install`) | BSD-2-Clause |
| CI/CD | GitHub Actions (`ubuntu-latest` + `macos-14`/`macos-13` runners) | platform |
| Autobump | POSIX `bash` templating (no new dep) | n/a |
| Artifact contract | ADR-011 naming `openlore-{version}-{triple}.tar.gz` + `.sha256` | contract |
| Enforcement | `brew audit --strict --online` + `brew style` + freshness assertion | BSD-2-Clause |

## Constraints

- Trunk-based, **no PRs** — autobump commits to `main` (never `brew bump-formula-pr`).
- Prebuilt binary only (no `depends_on "rust"`/`cargo`); `openlore` CLI only (never
  `openlore-indexer`); no service/daemon/phone-home; sha256 == published `.sha256`.
- ADRs live in `docs/adrs/` (ADR-061).
- Lean: scope is the formula + autobump + smoke test; no scope beyond it.

## Upstream Changes

- **CA-1 / UC-1 (BLOCKING)**: DISCUSS assumed a shipped `release.yml` pipeline; it **does not
  exist**. Design proceeds against ADR-011's **locked contract**; `release.yml` (native 4-platform
  builds + cosign + SBOM) is a **blocking external prerequisite feature** (DEVOPS). Both slices are
  designed but **not executable** until it + one real tagged release ship. Full detail:
  `design/upstream-changes.md`; tracked as OQ-D-1.

## Peer Review (self-review against nw-sa-critique-dimensions)

- **Bias**: no resume-driven/latest-tech bias — the only "tech" is Homebrew's own DSL + existing
  GitHub Actions; no new runtime, no new service. PASS.
- **ADR quality**: ADR-061 has Context, Decision (D-1..D-6), ≥8 Alternatives with rejection
  rationale, Consequences (±), Enforcement, Earned Trust, Revisit Trigger. PASS.
- **Completeness**: quality attributes addressed — installability/portability (4-triple coverage),
  security/integrity (sha256 + cosign layering), reliability (ordering guard + smoke-test probe),
  maintainability (autobump removes manual drift). PASS.
- **Feasibility**: no new team capability; brew + Actions are in use. **The one blocker is the
  missing `release.yml` prerequisite** — surfaced explicitly (OQ-D-1), not hidden. PASS with the
  documented blocking-prerequisite caveat.
- **Priority (Q1-Q4)**: Q1 largest bottleneck = the missing pipeline (identified, not designed
  around); Q2 alternatives present for every OD; Q3 constraints correctly prioritized
  (trunk-based/no-PR honored); Q4 the freshness/ordering risks (R-1) are addressed with the
  `needs:` DAG edge. PASS.
